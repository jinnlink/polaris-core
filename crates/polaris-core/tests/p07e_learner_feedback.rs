use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::learner_feedback::LearnerFeedbackInput;
use rusqlite::{params, Connection};
use serde_json::Value;

#[test]
fn learner_feedback_records_state_report_as_behavior_event() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_mastery(&conn, "ownership");
    let engine = Engine::new(conn);

    let receipt = engine
        .record_learner_feedback(LearnerFeedbackInput {
            session_id: "s1".to_owned(),
            source: "cli".to_owned(),
            kind: "state".to_owned(),
            concept_id: Some("ownership".to_owned()),
            state: Some("frustrated".to_owned()),
            reason: None,
            note: Some("I keep missing transfer tasks.".to_owned()),
        })
        .unwrap();

    assert_eq!(receipt.kind, "state");
    assert_eq!(receipt.effect, "recorded_only");
    assert_eq!(receipt.state.as_deref(), Some("frustrated"));
    assert_eq!(receipt.concept_id.as_deref(), Some("ownership"));

    let (session_id, event_type, concept_id, payload): (String, String, String, String) = engine
        .conn()
        .query_row(
            "SELECT session_id, type, concept_id, payload_json
             FROM behavior_events WHERE id=?1",
            [receipt.event_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    let payload: Value = serde_json::from_str(&payload).unwrap();

    assert_eq!(session_id, "s1");
    assert_eq!(event_type, "learner_feedback");
    assert_eq!(concept_id, "ownership");
    assert_eq!(payload["schema_version"], 1);
    assert_eq!(payload["kind"], "state");
    assert_eq!(payload["state"], "frustrated");
    assert_eq!(payload["source"], "cli");
    assert_eq!(payload["effect"], "recorded_only");
    assert_eq!(payload["note"], "I keep missing transfer tasks.");
}

#[test]
fn learner_feedback_records_pause_request_without_changing_mastery_or_attempts() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_mastery(&conn, "ownership");
    seed_attempt(&conn, "a1", "ownership");
    let engine = Engine::new(conn);

    let receipt = engine
        .record_learner_feedback(LearnerFeedbackInput {
            session_id: "s1".to_owned(),
            source: "http".to_owned(),
            kind: "pause".to_owned(),
            concept_id: Some("ownership".to_owned()),
            state: None,
            reason: Some("today is enough".to_owned()),
            note: None,
        })
        .unwrap();

    assert_eq!(receipt.kind, "pause");
    assert_eq!(receipt.reason.as_deref(), Some("today is enough"));
    assert_eq!(receipt.effect, "recorded_only");

    let p_known: f64 = engine
        .conn()
        .query_row(
            "SELECT p_known FROM mastery_states WHERE concept_id='ownership'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let attempts: i64 = engine
        .conn()
        .query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))
        .unwrap();
    let abandon_events: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type='abandon'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(p_known, 0.42);
    assert_eq!(attempts, 1);
    assert_eq!(abandon_events, 0, "pause must not be recorded as abandon");
}

#[test]
fn learner_feedback_rejects_unknown_kind_or_state() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let engine = Engine::new(conn);

    let bad_kind = engine.record_learner_feedback(LearnerFeedbackInput {
        session_id: "s1".to_owned(),
        source: "cli".to_owned(),
        kind: "mood".to_owned(),
        concept_id: None,
        state: Some("flow".to_owned()),
        reason: None,
        note: None,
    });
    let bad_state = engine.record_learner_feedback(LearnerFeedbackInput {
        session_id: "s1".to_owned(),
        source: "cli".to_owned(),
        kind: "state".to_owned(),
        concept_id: None,
        state: Some("sleepy".to_owned()),
        reason: None,
        note: None,
    });

    assert!(bad_kind
        .unwrap_err()
        .to_string()
        .contains("learner_feedback.kind"));
    assert!(bad_state
        .unwrap_err()
        .to_string()
        .contains("learner_feedback.state"));

    let events: i64 = engine
        .conn()
        .query_row("SELECT COUNT(*) FROM behavior_events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(events, 0);
}

fn seed_mastery(conn: &Connection, concept_id: &str) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, calib_gap, brier_ewma, attempt_count, phase, updated_at)
         VALUES (?1, 0.42, 0.0, 0.0, 3, 'phantom', '2026-06-17T00:00:00Z')",
        [concept_id],
    )
    .unwrap();
}

fn seed_attempt(conn: &Connection, id: &str, concept_id: &str) {
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                              provisional_score, final_score, created_at)
         VALUES (?1, 's1', ?2, 'recall', 4, 0.5, NULL, '2026-06-17T00:00:00Z')",
        params![id, concept_id],
    )
    .unwrap();
}
