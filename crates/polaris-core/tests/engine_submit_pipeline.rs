mod common;

use common::workspace_pack_path;
use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::fsrs::FsrsState;
use rusqlite::Connection;

#[test]
fn submit_without_llm_records_provisional_mastery_and_retry_queue() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls which binding can drop a value.".to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    assert!((receipt.provisional_score - 0.70).abs() < 1e-9);
    assert!(receipt.degraded);

    let state = engine.mastery_state("ownership").unwrap().expect("mastery");
    assert_eq!(state.attempt_count, 1);

    let queued: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM grade_queue WHERE attempt_id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(queued, 1);
}

#[test]
fn submit_provisional_records_mastery_and_queues_retry() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit_provisional(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls which binding can drop a value.".to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    assert!((receipt.provisional_score - 0.70).abs() < 1e-9);
    assert!(receipt.degraded);
    let state = engine.mastery_state("ownership").unwrap().expect("mastery");
    assert_eq!(state.attempt_count, 1);
    let stored: (Option<f64>, i64) = engine
        .conn()
        .query_row(
            "SELECT final_score, (SELECT COUNT(*) FROM grade_queue WHERE attempt_id=?1)
             FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, (None, 1));
}

#[test]
fn final_score_replay_preserves_provisional_history() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 5,
            latency_ms: 800,
            hint_count: 0,
        })
        .unwrap();
    let before = engine.mastery_state("ownership").unwrap().unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.20).unwrap();
    let after = engine.mastery_state("ownership").unwrap().unwrap();

    let scores: (f64, f64) = engine
        .conn()
        .query_row(
            "SELECT provisional_score, final_score FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(scores.0, receipt.provisional_score);
    assert_eq!(scores.1, 0.20);
    assert_ne!(before.p_known, after.p_known);
}

#[test]
fn grade_pending_processes_queued_attempts() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    engine
        .conn()
        .execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
             VALUES ('ev1', 's1', 'cli-submit', 'text/plain', 'Ownership controls which binding can drop a value.', '[\"ownership\"]', '2026-06-11T00:00:00Z')",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, response_evidence_id, self_confidence, provisional_score, created_at)
             VALUES ('attempt-queued', 's1', 'ownership', 'recall', 'ev1', 4, 0.70, '2026-06-11T00:00:00Z')",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO grade_queue(attempt_id, enqueued_at, retry_count, last_error)
             VALUES ('attempt-queued', '2026-06-11T00:00:00Z', 0, 'llm config missing')",
            [],
        )
        .unwrap();

    let summary = engine
        .grade_pending_with_static_response(
            r#"{"score":0.83,"depth":"explain","citations":[{"evidence_id":"ev1","quote":"controls which binding"}]}"#,
        )
        .unwrap();

    assert_eq!(summary.processed, 1);
    assert_eq!(summary.pending, 0);
    let final_score: f64 = engine
        .conn()
        .query_row(
            "SELECT final_score FROM attempts WHERE id='attempt-queued'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!((final_score - 0.83).abs() < 1e-9);
}

#[test]
fn replay_uses_attempt_created_at_for_fsrs_elapsed_days() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first = engine
        .submit_provisional(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 4,
            latency_ms: 1000,
            hint_count: 0,
        })
        .unwrap();
    let second = engine
        .submit_provisional(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership again.".to_owned(),
            response_text: "Ownership controls drops again.".to_owned(),
            self_confidence: 4,
            latency_ms: 1000,
            hint_count: 0,
        })
        .unwrap();

    for (id, at) in [
        (first.attempt_id.as_str(), "2026-06-01T00:00:00Z"),
        (second.attempt_id.as_str(), "2026-06-04T00:00:00Z"),
    ] {
        engine
            .conn()
            .execute("UPDATE attempts SET created_at=?2 WHERE id=?1", (id, at))
            .unwrap();
    }

    engine.apply_final_score(&first.attempt_id, 0.80).unwrap();
    engine.apply_final_score(&second.attempt_id, 0.80).unwrap();

    let fsrs_json: String = engine
        .conn()
        .query_row(
            "SELECT fsrs_json FROM mastery_states WHERE concept_id='ownership'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let state: FsrsState = serde_json::from_str(&fsrs_json).unwrap();
    assert!(
        state.stability > 2.4,
        "second review should use three elapsed days, got stability {}",
        state.stability
    );
}
