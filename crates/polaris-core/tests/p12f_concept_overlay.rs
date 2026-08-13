use std::path::PathBuf;

use polaris_core::capture_queue::{capture_learning_evidence, CaptureInput, LearnerCaptureKind};
use polaris_core::concept_overlay::{
    accept_overlay, preview_overlay, reject_suggestion, rollback_overlay, suggest_with_config,
    suggest_with_static_response, SuggestionKind,
};
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::grader::LlmConfig;
use rusqlite::Connection;

const SOURCE_TEXT: &str =
    "A stable overlay keeps the official course immutable while personal concepts remain reversible.";

#[test]
fn unmapped_capture_records_four_cited_kinds_without_mastery_effects() {
    let conn = setup();
    let capture_id = capture(&conn, Vec::new());
    let evidence_id = evidence_id(&conn, &capture_id);
    let response = serde_json::json!({"suggestions":[
        {"payload":{"kind":"concept","id":"overlay_immutability","name":"Overlay immutability","seed_order":100},"citation":{"evidence_id":evidence_id,"quote":"official course immutable"},"reason":"不可变边界。"},
        {"payload":{"kind":"schema","id":"overlay_lifecycle","name":"Overlay lifecycle","seed_order":101},"citation":{"evidence_id":evidence_id,"quote":"personal concepts remain reversible"},"reason":"回滚生命周期。"},
        {"payload":{"kind":"typed_edge","id":"overlay_before_lifecycle","src":"overlay_immutability","dst":"overlay_lifecycle","edge_type":"prerequisite","weight":1.0},"citation":{"evidence_id":evidence_id,"quote":"official course immutable while personal concepts remain reversible"},"reason":"前置关系。"},
        {"payload":{"kind":"misconception","id":"overlay_mutates_base","concept_id":"overlay_immutability","title":"误以为会改写官方 Pack"},"citation":{"evidence_id":evidence_id,"quote":"official course immutable"},"reason":"误解候选。"}
    ]});

    let suggestions = suggest_with_static_response(
        &conn,
        &capture_id,
        "template",
        "test-model-1",
        &response.to_string(),
    )
    .unwrap();
    assert_eq!(suggestions.len(), 4);
    assert_eq!(suggestions[0].kind, SuggestionKind::Concept);
    assert!(suggestions.iter().all(|item| item.status == "pending"));
    assert_eq!(table_count(&conn, "attempts"), 0);
    assert_eq!(table_count(&conn, "mastery_states"), 0);

    let ids = suggestions
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let preview = preview_overlay(&conn, "template", &ids).unwrap();
    assert_eq!(preview.status, "awaiting_user_acceptance");
    assert_eq!(preview.validation, "pack_validate_passed");
    assert_eq!(preview.items.len(), 4);
}

#[test]
fn citation_failure_existing_mapping_and_missing_model_keep_raw_capture() {
    let conn = setup();
    let unmapped = capture(&conn, Vec::new());
    let bad = response_for(
        &evidence_id(&conn, &unmapped),
        "quote absent from evidence",
        "new_concept",
    );
    let error = suggest_with_static_response(&conn, &unmapped, "template", "test-model", &bad)
        .unwrap_err()
        .to_string();
    assert!(error.contains("suggestion.citation"));
    assert_eq!(table_count(&conn, "concept_suggestions"), 0);
    assert_eq!(capture_status(&conn, &unmapped), "pending");

    let error = suggest_with_config(&conn, &unmapped, "template", LlmConfig::Unavailable)
        .unwrap_err()
        .to_string();
    assert!(error.contains("raw capture was kept unchanged"));
    assert_eq!(table_count(&conn, "concept_suggestions"), 0);

    let mapped = capture(&conn, vec!["template_core_terms".to_owned()]);
    let response = response_for(
        &evidence_id(&conn, &mapped),
        "official course immutable",
        "should_not_exist",
    );
    let error = suggest_with_static_response(&conn, &mapped, "template", "test-model", &response)
        .unwrap_err()
        .to_string();
    assert!(error.contains("already maps"));
    assert_eq!(table_count(&conn, "concept_suggestions"), 0);
}

#[test]
fn preview_rejects_conflicts_unknown_edges_cycles_and_supports_reject() {
    let conn = setup();
    let conflict = record_one(&conn, "template_core_terms", None);
    assert!(
        preview_overlay(&conn, "template", std::slice::from_ref(&conflict.id))
            .unwrap_err()
            .to_string()
            .contains("concept id collision")
    );
    assert_eq!(
        reject_suggestion(&conn, &conflict.id).unwrap().status,
        "rejected"
    );

    let unknown = record_one(
        &conn,
        "edge_unknown",
        Some(("missing", "template_core_terms")),
    );
    assert!(preview_overlay(&conn, "template", &[unknown.id])
        .unwrap_err()
        .to_string()
        .contains("unknown edge endpoint"));

    let a = record_one(&conn, "cycle_a", None);
    let b = record_one(&conn, "cycle_b", None);
    let ab = record_one(&conn, "cycle_ab", Some(("cycle_a", "cycle_b")));
    let ba = record_one(&conn, "cycle_ba", Some(("cycle_b", "cycle_a")));
    assert!(
        preview_overlay(&conn, "template", &[a.id, b.id, ab.id, ba.id])
            .unwrap_err()
            .to_string()
            .contains("cyclic prerequisite")
    );
}

#[test]
fn overlay_upgrade_and_full_version_rollback_preserve_base_attempts_and_mastery() {
    let conn = setup();
    let base_name: String = conn
        .query_row(
            "SELECT name FROM concepts WHERE id='template_core_terms'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let first = record_one(&conn, "overlay_reversible", None);
    let receipt = accept_overlay(&conn, "template", &[first.id]).unwrap();
    assert_eq!(receipt.version, 1);
    assert_eq!(receipt.validation, "pass");
    assert_ne!(receipt.sandbox, "fail");

    let second = record_one(&conn, "overlay_second_version", None);
    let upgrade = accept_overlay(&conn, "template", &[second.id]).unwrap();
    assert_eq!(upgrade.version, 2);
    assert_eq!(overlay_concept_count(&conn), 2);

    let rollback = rollback_overlay(&conn, "template").unwrap();
    assert_eq!(rollback.rolled_back_version, 2);
    assert_eq!(rollback.active_version, Some(1));
    assert_eq!(overlay_concept_count(&conn), 1);
    assert_eq!(concept_count(&conn, "overlay_reversible"), 1);
    assert_eq!(concept_count(&conn, "overlay_second_version"), 0);

    let final_rollback = rollback_overlay(&conn, "template").unwrap();
    assert_eq!(final_rollback.active_version, None);
    assert_eq!(overlay_concept_count(&conn), 0);
    assert_eq!(table_count(&conn, "attempts"), 0);
    assert_eq!(table_count(&conn, "mastery_states"), 0);
    assert_eq!(
        conn.query_row(
            "SELECT name FROM concepts WHERE id='template_core_terms'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        base_name
    );
}

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path("packs/template")).unwrap();
    engine.into_connection()
}

fn capture(conn: &Connection, candidate_concept_ids: Vec<String>) -> String {
    capture_learning_evidence(
        conn,
        CaptureInput {
            session_id: None,
            source: "p12f-test".to_owned(),
            content_type: "text/plain".to_owned(),
            text: SOURCE_TEXT.to_owned(),
            learner_kind: LearnerCaptureKind::Reference,
            candidate_concept_ids,
            note: None,
        },
    )
    .unwrap()
    .capture_id
}

fn record_one(
    conn: &Connection,
    id: &str,
    edge: Option<(&str, &str)>,
) -> polaris_core::concept_overlay::ConceptSuggestion {
    let capture_id = capture(conn, Vec::new());
    let evidence_id = evidence_id(conn, &capture_id);
    let payload = match edge {
        Some((src, dst)) => {
            serde_json::json!({"kind":"typed_edge","id":id,"src":src,"dst":dst,"edge_type":"prerequisite","weight":1.0})
        }
        None => serde_json::json!({"kind":"concept","id":id,"name":id,"seed_order":200}),
    };
    let response = serde_json::json!({"suggestions":[{
        "payload":payload,
        "citation":{"evidence_id":evidence_id,"quote":"official course immutable"},
        "reason":"测试候选的证据绑定理由。"
    }]});
    suggest_with_static_response(
        conn,
        &capture_id,
        "template",
        "test-model-1",
        &response.to_string(),
    )
    .unwrap()
    .remove(0)
}

fn response_for(evidence_id: &str, quote: &str, id: &str) -> String {
    serde_json::json!({"suggestions":[{
        "payload":{"kind":"concept","id":id,"name":"Candidate","seed_order":100},
        "citation":{"evidence_id":evidence_id,"quote":quote},
        "reason":"证据说明了一个新知识点。"
    }]})
    .to_string()
}

fn evidence_id(conn: &Connection, capture_id: &str) -> String {
    conn.query_row(
        "SELECT evidence_id FROM capture_queue WHERE id=?1",
        [capture_id],
        |row| row.get(0),
    )
    .unwrap()
}

fn capture_status(conn: &Connection, capture_id: &str) -> String {
    conn.query_row(
        "SELECT status FROM capture_queue WHERE id=?1",
        [capture_id],
        |row| row.get(0),
    )
    .unwrap()
}

fn concept_count(conn: &Connection, id: &str) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM concepts WHERE id=?1", [id], |row| {
        row.get(0)
    })
    .unwrap()
}

fn overlay_concept_count(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM concepts WHERE provenance LIKE 'overlay:%'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

fn table_count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
