use polaris_core::capture_queue::{CaptureInput, CaptureStatus, LearnerCaptureKind};
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::inbox_practice::InboxPracticeSubmissionInput;
use polaris_core::learner_inbox::LearnerInboxAction;
use rusqlite::Connection;
use serde_json::Value;

#[test]
fn draft_inbox_practice_builds_prompt_without_creating_mastery_facts() {
    let engine = test_engine();
    let capture = capture_reference(
        &engine,
        "Ownership means one value has one owner at a time.",
    );
    engine
        .act_on_learner_inbox_item(&capture.capture_id, LearnerInboxAction::Accept, None)
        .unwrap();
    let before = fact_counts(engine.conn());

    let draft = engine.draft_inbox_practice(&capture.capture_id).unwrap();

    assert_eq!(draft.capture_id, capture.capture_id);
    assert_eq!(draft.evidence_id, capture.evidence_id);
    assert_eq!(draft.status, CaptureStatus::PracticeReady);
    assert_eq!(draft.concept_hint.as_deref(), Some("Ownership"));
    assert_eq!(draft.task_type, "explain");
    assert!(draft.prompt.contains("用自己的话"));
    assert!(draft.prompt.contains("Ownership"));
    assert!(draft.source_excerpt.contains("one owner"));
    assert_eq!(draft.message, "先回答这道小题，再告诉我你的把握程度。");
    assert_eq!(fact_counts(engine.conn()), before);

    let payload = serde_json::to_value(&draft).unwrap();
    assert_no_internal_student_fields(&payload);
}

#[test]
fn submit_inbox_practice_records_attempt_and_marks_capture_practiced() {
    let mut engine = test_engine();
    let capture = capture_reference(
        &engine,
        "Borrowing lets code use a value without taking ownership.",
    );
    engine
        .act_on_learner_inbox_item(&capture.capture_id, LearnerInboxAction::Accept, None)
        .unwrap();
    let draft = engine.draft_inbox_practice(&capture.capture_id).unwrap();

    let receipt = engine
        .submit_inbox_practice(InboxPracticeSubmissionInput {
            capture_id: capture.capture_id.clone(),
            session_id: "p12e-practice".to_owned(),
            response_text:
                "Borrowing lets a reference read a value while ownership stays elsewhere."
                    .to_owned(),
            self_confidence: 4,
            latency_ms: 321,
            hint_count: 1,
        })
        .unwrap();

    assert_eq!(receipt.capture_id, capture.capture_id);
    assert_eq!(receipt.status, CaptureStatus::Practiced);
    assert_eq!(receipt.effect, "submitted");
    assert!(receipt.message.contains("已提交"));
    assert!((receipt.provisional_score - 0.70).abs() < 1e-9);
    assert!(receipt.degraded);

    let (status, attempt_count, mastery_count, grade_queue_count): (String, i64, i64, i64) = engine
        .conn()
        .query_row(
            "SELECT
                (SELECT status FROM capture_queue WHERE id=?1),
                (SELECT COUNT(*) FROM attempts),
                (SELECT COUNT(*) FROM mastery_states),
                (SELECT COUNT(*) FROM grade_queue)",
            [capture.capture_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(status, "practiced");
    assert_eq!(attempt_count, 1);
    assert_eq!(mastery_count, 1);
    assert_eq!(grade_queue_count, 1);

    let (concept_id, task_type, prompt_text, confidence, latency_ms, hint_count): (
        String,
        String,
        String,
        i64,
        i64,
        i64,
    ) = engine
        .conn()
        .query_row(
            "SELECT concept_id, task_type, prompt_text, self_confidence, latency_ms, hint_count
             FROM attempts
             WHERE id=?1",
            [receipt.attempt_id.as_str()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(concept_id, "ownership");
    assert_eq!(task_type, "explain");
    assert_eq!(prompt_text, draft.prompt);
    assert_eq!(confidence, 4);
    assert_eq!(latency_ms, 321);
    assert_eq!(hint_count, 1);

    let bridge_events: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events
             WHERE type='inbox_practice' AND concept_id='ownership'
               AND payload_json LIKE '%' || ?1 || '%'
               AND payload_json LIKE '%' || ?2 || '%'",
            [capture.capture_id.as_str(), receipt.attempt_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(bridge_events, 1);
}

#[test]
fn inbox_practice_rejects_pending_or_unmapped_capture_without_mastery_facts() {
    let mut engine = test_engine();
    let pending = capture_reference(&engine, "A pending note cannot be practiced yet.");
    let unmapped = engine
        .capture_learning_evidence(CaptureInput {
            session_id: Some("p12e".to_owned()),
            source: "test".to_owned(),
            content_type: "text/plain".to_owned(),
            text: "A note without a concept should wait for suggestion.".to_owned(),
            learner_kind: LearnerCaptureKind::Reference,
            candidate_concept_ids: Vec::new(),
            note: None,
        })
        .unwrap();
    engine
        .act_on_learner_inbox_item(&unmapped.capture_id, LearnerInboxAction::Accept, None)
        .unwrap();
    let before = fact_counts(engine.conn());

    let pending_error = engine
        .draft_inbox_practice(&pending.capture_id)
        .unwrap_err();
    assert!(pending_error.to_string().contains("practice_ready"));

    let unmapped_error = engine
        .submit_inbox_practice(InboxPracticeSubmissionInput {
            capture_id: unmapped.capture_id,
            session_id: "p12e-practice".to_owned(),
            response_text: "I can explain it.".to_owned(),
            self_confidence: 4,
            latency_ms: 1,
            hint_count: 0,
        })
        .unwrap_err();
    assert!(unmapped_error.to_string().contains("candidate concept"));
    assert_eq!(fact_counts(engine.conn()), before);
}

#[test]
fn submit_inbox_practice_rejects_invalid_confidence() {
    let mut engine = test_engine();
    let capture = capture_reference(&engine, "Ownership is checked at compile time.");
    engine
        .act_on_learner_inbox_item(&capture.capture_id, LearnerInboxAction::Accept, None)
        .unwrap();

    let error = engine
        .submit_inbox_practice(InboxPracticeSubmissionInput {
            capture_id: capture.capture_id,
            session_id: "p12e-practice".to_owned(),
            response_text: "Ownership prevents double free.".to_owned(),
            self_confidence: 6,
            latency_ms: 1,
            hint_count: 0,
        })
        .unwrap_err();

    assert!(error.to_string().contains("self_confidence"));
    assert_eq!(fact_counts(engine.conn()), (0, 0, 0));
}

fn test_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn capture_reference(engine: &Engine, text: &str) -> polaris_core::capture_queue::CaptureRecord {
    engine
        .capture_learning_evidence(CaptureInput {
            session_id: Some("p12e".to_owned()),
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

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn assert_no_internal_student_fields(payload: &Value) {
    for forbidden in [
        "p_known",
        "theta",
        "theta_version",
        "mastery",
        "pack",
        "candidate_concept_ids_json",
        "candidate_concept_ids",
    ] {
        assert!(
            payload.get(forbidden).is_none(),
            "student practice payload leaked internal field {forbidden}: {payload}"
        );
    }
}
