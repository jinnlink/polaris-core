use polaris_core::capture_queue::{CaptureInput, CaptureStatus, LearnerCaptureKind};
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::learner_inbox::LearnerInboxAction;
use rusqlite::Connection;
use serde_json::Value;

#[test]
fn learner_inbox_lists_open_captures_with_student_readable_actions() {
    let engine = test_engine();
    let record = capture_reference(&engine, "Rust ownership notes from another project.");

    let items = engine.learner_inbox(&[], 20).unwrap();

    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.capture_id, record.capture_id);
    assert_eq!(item.evidence_id, record.evidence_id);
    assert_eq!(item.status, CaptureStatus::Pending);
    assert_eq!(item.learner_kind, LearnerCaptureKind::Reference);
    assert_eq!(item.message, "已保存，稍后帮你整理");
    assert!(item.text_preview.contains("Rust ownership notes"));
    assert_eq!(item.actions.len(), 3);
    assert_eq!(item.actions[0].action, LearnerInboxAction::Accept);
    assert_eq!(item.actions[0].label, "转成一道小题");
    assert_eq!(item.actions[1].action, LearnerInboxAction::Defer);
    assert_eq!(item.actions[1].label, "稍后再看");
    assert_eq!(item.actions[2].action, LearnerInboxAction::Ignore);
    assert_eq!(item.actions[2].label, "忽略");

    let payload = serde_json::to_value(item).unwrap();
    assert_no_internal_student_fields(&payload);
}

#[test]
fn learner_inbox_accept_marks_practice_ready_without_creating_mastery_facts() {
    let engine = test_engine();
    let record = capture_reference(
        &engine,
        "Borrow checker error I want to turn into practice.",
    );
    let before = fact_counts(engine.conn());

    let receipt = engine
        .act_on_learner_inbox_item(&record.capture_id, LearnerInboxAction::Accept, None)
        .unwrap();

    assert_eq!(receipt.capture_id, record.capture_id);
    assert_eq!(receipt.status, CaptureStatus::PracticeReady);
    assert_eq!(receipt.effect, "recorded_only");
    assert!(receipt.message.contains("小题"));
    assert_eq!(fact_counts(engine.conn()), before);

    let status: String = engine
        .conn()
        .query_row(
            "SELECT status FROM capture_queue WHERE id=?1",
            [record.capture_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "practice_ready");

    let items = engine.learner_inbox(&[], 20).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status, CaptureStatus::PracticeReady);
    assert!(items[0].message.contains("小题"));
}

#[test]
fn learner_inbox_ignore_and_archive_hide_items_from_default_open_list() {
    let engine = test_engine();
    let ignored = capture_reference(&engine, "A note I do not need.");
    let archived = capture_reference(&engine, "A note that should be archived.");
    let active = capture_reference(&engine, "A note still waiting.");

    engine
        .act_on_learner_inbox_item(&ignored.capture_id, LearnerInboxAction::Ignore, None)
        .unwrap();
    engine
        .act_on_learner_inbox_item(
            &archived.capture_id,
            LearnerInboxAction::Archive,
            Some("keep for audit only".to_owned()),
        )
        .unwrap();

    let items = engine.learner_inbox(&[], 20).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].capture_id, active.capture_id);

    let archived_note: Option<String> = engine
        .conn()
        .query_row(
            "SELECT note FROM capture_queue WHERE id=?1",
            [archived.capture_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(archived_note.as_deref(), Some("keep for audit only"));
}

#[test]
fn learner_inbox_mapped_item_uses_concept_name_without_exposing_raw_candidate_ids() {
    let engine = test_engine();
    engine
        .conn()
        .execute(
            "INSERT INTO concepts(id, name, provenance, evidence_ids_json, created_at)
             VALUES ('ownership', '所有权', 'pack-seed', '[]', '2026-06-24T00:00:00Z')",
            [],
        )
        .unwrap();
    let record = capture_reference(&engine, "一条关于借用规则的资料。");
    engine
        .conn()
        .execute(
            "UPDATE capture_queue SET status='mapped' WHERE id=?1",
            [record.capture_id.as_str()],
        )
        .unwrap();

    let items = engine.learner_inbox(&[], 20).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status, CaptureStatus::Mapped);
    assert_eq!(items[0].concept_hint.as_deref(), Some("所有权"));
    assert_eq!(items[0].message, "这可能和「所有权」有关");
    let payload = serde_json::to_value(&items[0]).unwrap();
    assert!(payload.get("candidate_concept_ids_json").is_none());
    assert!(payload.get("candidate_concept_ids").is_none());
}

#[test]
fn learner_inbox_can_filter_statuses_and_limits_results() {
    let engine = test_engine();
    let first = capture_reference(&engine, "first pending item");
    let second = capture_reference(&engine, "second pending item");
    engine
        .act_on_learner_inbox_item(&second.capture_id, LearnerInboxAction::Accept, None)
        .unwrap();

    let ready = engine
        .learner_inbox(&[CaptureStatus::PracticeReady], 10)
        .unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].capture_id, second.capture_id);

    let pending = engine.learner_inbox(&[CaptureStatus::Pending], 1).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].capture_id, first.capture_id);
}

fn test_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn capture_reference(engine: &Engine, text: &str) -> polaris_core::capture_queue::CaptureRecord {
    engine
        .capture_learning_evidence(CaptureInput {
            session_id: Some("p12d".to_owned()),
            source: "test".to_owned(),
            content_type: "text/plain".to_owned(),
            text: text.to_owned(),
            learner_kind: LearnerCaptureKind::Reference,
            candidate_concept_ids: vec!["ownership".to_owned()],
            note: None,
        })
        .unwrap()
}

fn fact_counts(conn: &Connection) -> (i64, i64, i64) {
    conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM attempts),
            (SELECT COUNT(*) FROM mastery_states),
            (SELECT COUNT(*) FROM grade_queue)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .unwrap()
}

fn assert_no_internal_student_fields(payload: &Value) {
    for forbidden in [
        "p_known",
        "theta",
        "theta_version",
        "mastery",
        "pack",
        "candidate_concept_ids_json",
    ] {
        assert!(
            payload.get(forbidden).is_none(),
            "student inbox payload leaked internal field {forbidden}: {payload}"
        );
    }
}
