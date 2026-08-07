use std::collections::BTreeSet;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::knowledge_map::{
    KnowledgeMapDueStatus, KnowledgeMapQuery, KnowledgeMapScope, KnowledgeMapStateSource,
};
use polaris_core::ops::doctor_report;
use polaris_core::pack_state::ThetaMode;
use rusqlite::{params, Connection};

const KNOWLEDGE_MAP_10K_BUDGET_NS: f64 = 10_000_000.0;

#[test]
fn empty_database_returns_empty_pack_snapshot() {
    let engine = empty_engine();

    let snapshot = engine.knowledge_map(KnowledgeMapQuery::default()).unwrap();

    assert_eq!(snapshot.summary.scope, KnowledgeMapScope::Pack);
    assert_eq!(snapshot.summary.resolved_pack, None);
    assert_eq!(snapshot.summary.total_nodes, 0);
    assert!(snapshot.nodes.is_empty());
    assert!(snapshot.edges.is_empty());
    assert_eq!(snapshot.next_cursor, None);
}

#[test]
fn default_query_tracks_active_pack_across_three_domains() {
    let engine = engine_with_three_packs();

    let rust = engine.knowledge_map(KnowledgeMapQuery::default()).unwrap();
    assert_eq!(rust.summary.resolved_pack.as_deref(), Some("rust"));
    assert!(rust
        .nodes
        .iter()
        .all(|node| node.pack.as_deref() == Some("rust")));

    engine
        .switch_pack("algorithms", Some(ThetaMode::Isolated))
        .unwrap();
    let algorithms = engine.knowledge_map(KnowledgeMapQuery::default()).unwrap();
    assert_eq!(
        algorithms.summary.resolved_pack.as_deref(),
        Some("algorithms")
    );
    assert!(algorithms
        .nodes
        .iter()
        .all(|node| node.pack.as_deref() == Some("algorithms")));

    engine.switch_pack("english", None).unwrap();
    let english = engine.knowledge_map(KnowledgeMapQuery::default()).unwrap();
    assert_eq!(english.summary.resolved_pack.as_deref(), Some("english"));
    assert!(english
        .nodes
        .iter()
        .any(|node| node.id == "cefr_a1" && node.kind == "schema"));
}

#[test]
fn global_scope_returns_pack_and_dimension_aggregates_without_nodes() {
    let engine = engine_with_three_packs();

    let snapshot = engine
        .knowledge_map(KnowledgeMapQuery {
            scope: KnowledgeMapScope::Global,
            ..KnowledgeMapQuery::default()
        })
        .unwrap();

    assert!(snapshot.nodes.is_empty());
    assert!(snapshot.edges.is_empty());
    assert_eq!(snapshot.summary.packs.len(), 3);
    assert_eq!(snapshot.summary.dimensions.len(), 3);
    assert_eq!(
        snapshot
            .summary
            .packs
            .iter()
            .filter_map(|pack| pack.id.as_deref())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["algorithms", "english", "rust"])
    );
    assert!(snapshot
        .summary
        .dimensions
        .iter()
        .any(|dimension| dimension.id == "pack:english"));
}

#[test]
fn root_depth_and_pagination_return_stable_provenance_complete_subgraphs() {
    let engine = engine_with_three_packs();
    engine.switch_pack("english", None).unwrap();

    let rooted = engine
        .knowledge_map(KnowledgeMapQuery {
            root: Some("cefr_a1".to_owned()),
            depth: Some(1),
            limit: 20,
            ..KnowledgeMapQuery::default()
        })
        .unwrap();
    let ids = rooted
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(ids.contains("cefr_a1"));
    assert!(ids.contains("a1_vocabulary"));
    assert!(rooted
        .edges
        .iter()
        .all(|edge| !edge.provenance.origin.is_empty()));

    let first = engine
        .knowledge_map(KnowledgeMapQuery {
            pack: Some("english".to_owned()),
            limit: 2,
            ..KnowledgeMapQuery::default()
        })
        .unwrap();
    assert_eq!(first.nodes.len(), 2);
    let cursor = first.next_cursor.clone().expect("second page cursor");
    let second = engine
        .knowledge_map(KnowledgeMapQuery {
            pack: Some("english".to_owned()),
            limit: 2,
            cursor: Some(cursor),
            ..KnowledgeMapQuery::default()
        })
        .unwrap();
    assert_eq!(second.nodes.len(), 2);
    assert!(first
        .nodes
        .iter()
        .all(|left| second.nodes.iter().all(|right| left.id != right.id)));
}

#[test]
fn observed_node_matches_status_and_counts_linked_evidence() {
    let mut engine = engine_with_three_packs();
    let receipt = engine
        .submit(SubmitInput {
            session_id: "knowledge-map-observed".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership gives one binding responsibility for dropping a value."
                .to_owned(),
            self_confidence: 4,
            latency_ms: 900,
            hint_count: 0,
        })
        .unwrap();
    engine.apply_final_score(&receipt.attempt_id, 0.91).unwrap();

    let status = engine.status_snapshot().unwrap();
    let status_node = status
        .concepts
        .iter()
        .find(|node| node.concept_id == "ownership")
        .unwrap();
    let snapshot = engine
        .knowledge_map(KnowledgeMapQuery {
            root: Some("ownership".to_owned()),
            depth: Some(0),
            ..KnowledgeMapQuery::default()
        })
        .unwrap();
    let node = &snapshot.nodes[0];

    assert_eq!(node.p_known, status_node.p_known);
    assert_eq!(node.phase, status_node.phase);
    assert_eq!(node.provenance.source, KnowledgeMapStateSource::Observed);
    assert_eq!(
        node.provenance.origin.as_deref(),
        Some("attempts+mastery_states")
    );
    assert!(node.provenance.complete);
    assert_eq!(node.attempt_count, 1);
    assert_eq!(node.evidence_count, 1);
    assert_eq!(node.provenance.evidence_ids.len(), 1);
    assert!(node.uncertainty.confidence > 0.0);

    let doctor = doctor_report(engine.conn()).unwrap();
    assert!(doctor.replay_mismatches.is_empty());
    assert_eq!(doctor.replay_checked, 1);
}

#[test]
fn phase_due_and_confidence_filters_do_not_turn_priors_into_observations() {
    let mut engine = engine_with_three_packs();
    let receipt = engine
        .submit(SubmitInput {
            session_id: "knowledge-map-filter".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership determines which binding drops a value.".to_owned(),
            self_confidence: 4,
            latency_ms: 900,
            hint_count: 0,
        })
        .unwrap();
    engine.apply_final_score(&receipt.attempt_id, 0.91).unwrap();
    engine
        .conn()
        .execute(
            "UPDATE mastery_states SET next_due_at='2000-01-01T00:00:00Z' WHERE concept_id='ownership'",
            [],
        )
        .unwrap();

    let due = engine
        .knowledge_map(KnowledgeMapQuery {
            due: Some(KnowledgeMapDueStatus::Due),
            min_confidence: Some(0.1),
            phase: Some("undetermined".to_owned()),
            ..KnowledgeMapQuery::default()
        })
        .unwrap();

    assert_eq!(due.nodes.len(), 1);
    assert_eq!(due.nodes[0].id, "ownership");
    assert_eq!(due.nodes[0].due_status, KnowledgeMapDueStatus::Due);
    assert_eq!(due.summary.inherited_prior_count, 0);
}

#[test]
fn edges_without_provenance_are_omitted_and_audited() {
    let engine = engine_with_three_packs();
    engine
        .conn()
        .execute(
            "INSERT INTO edges(id, src, dst, type, weight) VALUES ('legacy-edge', 'ownership', 'moves', 'confusion', 1.0)",
            [],
        )
        .unwrap();

    let snapshot = engine
        .knowledge_map(KnowledgeMapQuery {
            root: Some("ownership".to_owned()),
            depth: Some(1),
            limit: 20,
            ..KnowledgeMapQuery::default()
        })
        .unwrap();

    assert!(snapshot.edges.iter().all(|edge| edge.id != "legacy-edge"));
    assert_eq!(snapshot.summary.omitted_edges_missing_provenance, 1);
}

#[test]
fn default_10k_pack_query_stays_paginated_and_within_tier_zero_budget() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES ('active_pack', 'large')",
        [],
    )
    .unwrap();
    let tx = conn.unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
                 VALUES (?1, 'large', ?2, 'concept', ?3, 0.2, 'pack-seed', '[]', '2026-01-01T00:00:00Z')",
            )
            .unwrap();
        for index in 0..10_000 {
            stmt.execute(params![
                format!("large-{index:05}"),
                format!("Large concept {index}"),
                index as i64
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();
    let engine = Engine::new(conn);
    let query = KnowledgeMapQuery::default();
    let warm = engine.knowledge_map(query.clone()).unwrap();
    assert_eq!(warm.summary.total_nodes, 10_000);
    assert_eq!(warm.nodes.len(), 100);
    assert!(warm.next_cursor.is_some());

    let mut durations = (0..5)
        .map(|_| {
            let started = Instant::now();
            let snapshot = engine.knowledge_map(black_box(query.clone())).unwrap();
            black_box(snapshot.nodes.len());
            started.elapsed().as_nanos() as f64
        })
        .collect::<Vec<_>>();
    durations.sort_by(f64::total_cmp);
    let observed_ns = durations[durations.len() / 2];
    let allowed_ns = KNOWLEDGE_MAP_10K_BUDGET_NS * if cfg!(debug_assertions) { 100.0 } else { 1.0 };
    assert!(
        observed_ns <= allowed_ns,
        "10k knowledge map query exceeded budget: observed {observed_ns:.0}ns, allowed {allowed_ns:.0}ns"
    );
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
