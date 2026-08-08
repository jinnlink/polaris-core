use std::path::{Path, PathBuf};

use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::knowledge_map::{
    KnowledgeMapGateStatus, KnowledgeMapScope, KnowledgeMapStateSource,
};
use polaris_core::mirt::{decode_vector, encode_vector};
use polaris_core::pack_state::ThetaMode;
use polaris_core::prediction_map::PredictionMapQuery;
use rusqlite::{params, Connection};

#[test]
fn shared_cold_start_separates_latent_prediction_and_prior_with_three_choices() {
    let engine = engine_with_three_packs();
    engine
        .switch_pack("english", Some(ThetaMode::Shared))
        .unwrap();
    set_shared_theta(engine.conn(), 1.5);
    let counts_before = write_counts(&engine);

    let snapshot = engine
        .prediction_map(PredictionMapQuery::default())
        .unwrap();

    assert_eq!(snapshot.summary.resolved_pack.as_deref(), Some("english"));
    assert_eq!(snapshot.summary.theta_mode.as_deref(), Some("shared"));
    assert!(snapshot.summary.cross_domain_enabled);
    assert_eq!(snapshot.initial_paths.len(), 3);
    assert_eq!(write_counts(&engine), counts_before);

    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.id == "a1_vocabulary")
        .unwrap();
    assert!(node.observed.is_none());
    let latent = node.latent_prediction.as_ref().unwrap();
    assert_eq!(latent.source, KnowledgeMapStateSource::LatentPrediction);
    assert_eq!(latent.gate_status, KnowledgeMapGateStatus::Shadow);
    assert_eq!(latent.theta_scope.as_deref(), Some("shared"));
    assert!(latent.cross_domain);
    assert!(latent.interval.lower <= latent.value);
    assert!(latent.interval.upper >= latent.value);
    let prior = node.inherited_prior.as_ref().unwrap();
    assert_eq!(prior.source, KnowledgeMapStateSource::InheritedPrior);
    assert_eq!(prior.gate_status, KnowledgeMapGateStatus::PriorOnly);
    assert_ne!(latent.value, prior.value);
}

#[test]
fn observed_mastery_remains_separate_from_prediction_and_prior() {
    let mut engine = engine_with_three_packs();
    let receipt = engine
        .submit(SubmitInput {
            session_id: "p16c-observed".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership assigns drop responsibility to one binding.".to_owned(),
            self_confidence: 4,
            latency_ms: 800,
            hint_count: 0,
        })
        .unwrap();
    engine.apply_final_score(&receipt.attempt_id, 0.92).unwrap();

    let snapshot = engine
        .prediction_map(PredictionMapQuery {
            root: Some("ownership".to_owned()),
            depth: Some(0),
            ..PredictionMapQuery::default()
        })
        .unwrap();
    let node = &snapshot.nodes[0];

    let observed = node.observed.as_ref().unwrap();
    assert_eq!(observed.source, KnowledgeMapStateSource::Observed);
    assert_eq!(observed.gate_status, KnowledgeMapGateStatus::Active);
    assert!(!observed.provenance.evidence_ids.is_empty());
    assert!(node.latent_prediction.is_some());
    assert!(node.inherited_prior.is_some());
    assert_eq!(snapshot.summary.observed_nodes, 1);
}

#[test]
fn isolated_pack_prediction_never_reads_shared_theta() {
    let engine = engine_with_three_packs();
    engine
        .switch_pack("algorithms", Some(ThetaMode::Isolated))
        .unwrap();
    let query = PredictionMapQuery {
        root: Some("complexity_basics".to_owned()),
        depth: Some(0),
        ..PredictionMapQuery::default()
    };
    set_shared_theta(engine.conn(), -4.0);
    let before = engine.prediction_map(query.clone()).unwrap();
    set_shared_theta(engine.conn(), 4.0);
    let after = engine.prediction_map(query).unwrap();

    assert!(!after.summary.cross_domain_enabled);
    assert_eq!(after.summary.theta_mode.as_deref(), Some("isolated"));
    let before_latent = before.nodes[0].latent_prediction.as_ref().unwrap();
    let after_latent = after.nodes[0].latent_prediction.as_ref().unwrap();
    assert_eq!(before_latent.value, after_latent.value);
    assert_eq!(after_latent.theta_scope.as_deref(), Some("pack:algorithms"));
    assert!(!after_latent.cross_domain);
}

#[test]
fn anchors_require_cross_pack_structure_gate_and_provenance() {
    let engine = engine_with_three_packs();
    engine.switch_pack("english", None).unwrap();
    seed_anchor_fixture(&engine);

    let snapshot = engine
        .prediction_map(PredictionMapQuery::default())
        .unwrap();

    assert_eq!(snapshot.anchors.len(), 1);
    let anchor = &snapshot.anchors[0];
    assert_eq!(anchor.source_concept_id, "rust_schema_source");
    assert_eq!(anchor.target_id, "english_schema_target");
    assert_eq!(anchor.source_pack.as_deref(), Some("rust"));
    assert_eq!(anchor.target_pack.as_deref(), Some("english"));
    assert!(anchor.structural_score >= 0.4);
    assert!(!anchor.difference.trim().is_empty());
    assert_eq!(anchor.provenance.origin, "engine");
    assert!(!anchor.provenance.evidence_ids.is_empty());
}

#[test]
fn missing_uncertainty_is_unfit_and_low_information_stays_visibly_wide() {
    let engine = engine_with_three_packs();
    let query = PredictionMapQuery {
        root: Some("ownership".to_owned()),
        depth: Some(0),
        ..PredictionMapQuery::default()
    };

    let low_information = engine.prediction_map(query.clone()).unwrap();
    let low = low_information.nodes[0].latent_prediction.as_ref().unwrap();
    assert_eq!(low.gate_status, KnowledgeMapGateStatus::Shadow);
    assert!(low.interval.upper - low.interval.lower >= 0.90);

    engine
        .conn()
        .execute("UPDATE theta SET g2=x'0000' WHERE id=1", [])
        .unwrap();
    let invalid = engine.prediction_map(query).unwrap();
    let unfit = invalid.nodes[0].latent_prediction.as_ref().unwrap();
    assert_eq!(unfit.gate_status, KnowledgeMapGateStatus::Unfit);
    assert_eq!(unfit.interval.lower, 0.0);
    assert_eq!(unfit.interval.upper, 1.0);
}

#[test]
fn no_persisted_structural_anchor_degrades_to_no_anchor_without_blocking_paths() {
    let engine = engine_with_three_packs();
    engine.switch_pack("english", None).unwrap();

    let snapshot = engine
        .prediction_map(PredictionMapQuery::default())
        .unwrap();

    assert!(snapshot.anchors.is_empty());
    assert_eq!(snapshot.initial_paths.len(), 3);
    assert!(snapshot
        .initial_paths
        .iter()
        .all(|path| path.anchor_id.is_none()));
}

#[test]
fn global_scope_keeps_pack_and_dimension_aggregates_without_fake_node_predictions() {
    let engine = engine_with_three_packs();

    let snapshot = engine
        .prediction_map(PredictionMapQuery {
            scope: KnowledgeMapScope::Global,
            ..PredictionMapQuery::default()
        })
        .unwrap();

    assert_eq!(snapshot.summary.scope, KnowledgeMapScope::Global);
    assert_eq!(snapshot.summary.packs.len(), 3);
    assert!(!snapshot.summary.dimensions.is_empty());
    assert!(snapshot.nodes.is_empty());
    assert!(snapshot.anchors.is_empty());
    assert!(snapshot.initial_paths.is_empty());
    assert!(!snapshot.summary.cross_domain_enabled);
    assert!(snapshot.summary.theta_mode.is_none());
}

#[test]
fn pack_switch_and_path_order_are_deterministic() {
    let engine = engine_with_three_packs();
    engine.switch_pack("english", None).unwrap();
    let first = engine
        .prediction_map(PredictionMapQuery::default())
        .unwrap();
    let second = engine
        .prediction_map(PredictionMapQuery::default())
        .unwrap();
    assert_eq!(first.initial_paths, second.initial_paths);
    assert_eq!(
        first
            .initial_paths
            .iter()
            .map(|path| path.rank)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    engine.switch_pack("rust", None).unwrap();
    let rust = engine
        .prediction_map(PredictionMapQuery::default())
        .unwrap();
    assert_eq!(rust.summary.resolved_pack.as_deref(), Some("rust"));
    assert!(rust
        .initial_paths
        .iter()
        .all(|path| path.pack.as_deref() == Some("rust")));
}

#[test]
fn synthetic_leave_one_pack_out_pipeline_changes_only_shared_prediction() {
    let mut engine = engine_with_three_packs();
    let shared_q: Vec<u8> = engine
        .conn()
        .query_row("SELECT q FROM concepts WHERE id='ownership'", [], |row| {
            row.get(0)
        })
        .unwrap();
    engine
        .conn()
        .execute(
            "UPDATE concepts SET q=?1 WHERE id='a1_vocabulary'",
            [shared_q],
        )
        .unwrap();
    engine
        .switch_pack("english", Some(ThetaMode::Shared))
        .unwrap();
    let query = PredictionMapQuery {
        root: Some("a1_vocabulary".to_owned()),
        depth: Some(0),
        ..PredictionMapQuery::default()
    };
    let before = engine.prediction_map(query.clone()).unwrap().nodes[0]
        .latent_prediction
        .as_ref()
        .unwrap()
        .value;

    engine.switch_pack("rust", Some(ThetaMode::Shared)).unwrap();
    let receipt = engine
        .submit(SubmitInput {
            session_id: "p16c-lopo".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership determines who drops the value.".to_owned(),
            self_confidence: 5,
            latency_ms: 600,
            hint_count: 0,
        })
        .unwrap();
    engine.apply_final_score(&receipt.attempt_id, 1.0).unwrap();

    engine.switch_pack("english", None).unwrap();
    let after = engine.prediction_map(query).unwrap().nodes[0]
        .latent_prediction
        .as_ref()
        .unwrap()
        .value;
    assert!(after > before, "shared theta evidence should transfer");
}

fn empty_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn engine_with_three_packs() -> Engine {
    let mut engine = empty_engine();
    engine.init_pack(workspace_path("packs/rust")).unwrap();
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    engine
        .init_pack(workspace_path("examples/packs/english"))
        .unwrap();
    engine
}

fn workspace_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn set_shared_theta(conn: &Connection, value: f64) {
    let blob: Vec<u8> = conn
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    let len = decode_vector(&blob).unwrap().len();
    conn.execute(
        "UPDATE theta SET vec=?1 WHERE id=1",
        [encode_vector(&vec![value; len])],
    )
    .unwrap();
}

fn write_counts(engine: &Engine) -> (i64, i64, i64) {
    (
        table_count(engine.conn(), "attempts"),
        table_count(engine.conn(), "mastery_states"),
        table_count(engine.conn(), "mrt_log"),
    )
}

fn table_count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn seed_anchor_fixture(engine: &Engine) {
    let rust_q: Vec<u8> = engine
        .conn()
        .query_row("SELECT q FROM concepts WHERE id='ownership'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let english_q: Vec<u8> = engine
        .conn()
        .query_row(
            "SELECT q FROM concepts WHERE id='a1_vocabulary'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    for (id, pack, name, q, order) in [
        (
            "rust_schema_source",
            "rust",
            "Resource lifecycle",
            rust_q.as_slice(),
            900,
        ),
        (
            "english_schema_target",
            "english",
            "Expression lifecycle",
            english_q.as_slice(),
            900,
        ),
        (
            "english_schema_low",
            "english",
            "Low structure",
            english_q.as_slice(),
            901,
        ),
        (
            "english_schema_missing",
            "english",
            "Missing source",
            english_q.as_slice(),
            902,
        ),
    ] {
        engine
            .conn()
            .execute(
                "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, b_difficulty, q, provenance, evidence_ids_json, created_at)
                 VALUES (?1, ?2, ?3, 'schema', ?4, 0.2, 0.0, ?5, 'test-pack', '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![id, pack, name, order, q],
            )
            .unwrap();
    }
    engine
        .conn()
        .execute(
            "INSERT INTO evidence_items(id, source, content_type, text, concept_ids_json, created_at)
             VALUES ('anchor-evidence', 'test', 'text/plain', 'Known resource lifecycle structure.', '[\"rust_schema_source\"]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, response_evidence_id, self_confidence, provisional_score, final_score, created_at, graded_at)
             VALUES ('anchor-attempt', 'anchor', 'rust_schema_source', 'recall', 'anchor-evidence', 4, 0.9, 0.9, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, phase, attempt_count, updated_at)
             VALUES ('rust_schema_source', 0.9, 'transfer', 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();

    let alignment = r#"{"method":"typed_2hop_struct","matched_edges":4,"total_edges":5}"#;
    for (id, target, weight, provenance) in [
        ("anchor-good", "english_schema_target", 0.8, Some("engine")),
        ("anchor-low", "english_schema_low", 0.1, Some("engine")),
        ("anchor-missing", "english_schema_missing", 0.9, None),
    ] {
        engine
            .conn()
            .execute(
                "INSERT INTO edges(id, src, dst, type, weight, alignment_json, provenance, evidence_ids_json, created_at)
                 VALUES (?1, 'rust_schema_source', ?2, 'maps_to', ?3, ?4, ?5, '[\"anchor-evidence\"]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![id, target, weight, alignment, provenance],
            )
            .unwrap();
    }
}
