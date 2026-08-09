mod common;

use common::workspace_pack_path;
use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::{Engine, SubmitInput};
use rusqlite::Connection;

#[test]
fn no_attempt_is_recorded_without_changing_mastery_or_theta() {
    let mut engine = seeded_engine();
    let normal = engine.submit_provisional(input("baseline")).unwrap();
    engine.apply_final_score(&normal.attempt_id, 0.82).unwrap();
    let mastery_before = engine.mastery_state("ownership").unwrap().unwrap();
    let theta_before = theta_snapshot(&engine);
    let grade_queue_before = row_count(&engine, "grade_queue");

    let receipt = engine
        .submit_no_attempt(input("no-attempt"), "not_understood_prompt")
        .unwrap();

    assert_eq!(
        mastery_before,
        engine.mastery_state("ownership").unwrap().unwrap()
    );
    assert_eq!(theta_before, theta_snapshot(&engine));
    let stored: (Option<f64>, Option<f64>, Option<String>) = engine
        .conn()
        .query_row(
            "SELECT provisional_score, final_score, no_attempt_reason FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        stored,
        (None, None, Some("not_understood_prompt".to_owned()))
    );
    assert_eq!(row_count(&engine, "grade_queue"), grade_queue_before);
    assert_eq!(row_count(&engine, "evidence_items"), 2);
    assert_eq!(event_count(&engine, "no_attempt"), 1);
    assert!(engine.apply_final_score(&receipt.attempt_id, 0.0).is_err());
    assert_eq!(
        mastery_before,
        engine.mastery_state("ownership").unwrap().unwrap()
    );
    assert_eq!(theta_before, theta_snapshot(&engine));
}

#[test]
fn invalid_reason_is_rejected_before_any_write() {
    let mut engine = seeded_engine();
    let before = counts(&engine);

    let error = engine
        .submit_no_attempt(input("invalid"), "model_guessed_reason")
        .unwrap_err();

    assert!(error.to_string().contains("no_attempt_reason"));
    assert_eq!(counts(&engine), before);
}

#[test]
fn prompt_not_understood_changes_instruction_and_diagnosis_without_changing_schedule() {
    let mut engine = seeded_engine();
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(
                 concept_id, p_known, calib_gap, brier_ewma, attempt_count,
                 max_depth, phase, updated_at
             ) VALUES ('ownership', 0.85, 0.0, 0.0, 4, 'apply', 'solidification',
                       '2026-08-09T00:00:00Z')",
            [],
        )
        .unwrap();
    let next_before = engine.next_task().unwrap().map(|task| task.concept_id);
    let ordinary = engine.teaching_instruction("ownership").unwrap();
    assert_eq!(ordinary.target_depth, "analyze");

    engine
        .submit_no_attempt(input("instruction"), "not_understood_prompt")
        .unwrap();
    let changed = engine.teaching_instruction("ownership").unwrap();
    let diagnosis = engine.diagnose_concept("ownership").unwrap();

    assert_eq!(changed.focus.kind, "prompt_not_understood");
    assert_eq!(changed.target_depth, "recall");
    assert_eq!(changed.move_name, "worked_example");
    assert_eq!(
        diagnosis.latest_no_attempt_reason.as_deref(),
        Some("not_understood_prompt")
    );
    assert_eq!(
        engine.next_task().unwrap().map(|task| task.concept_id),
        next_before
    );
}

#[test]
fn normal_submission_and_session_stuck_behavior_remain_explicit() {
    let mut engine = seeded_engine();
    let receipt = engine.submit_provisional(input("normal")).unwrap();
    assert!(engine.mastery_state("ownership").unwrap().is_some());
    let stored: Option<String> = engine
        .conn()
        .query_row(
            "SELECT no_attempt_reason FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(stored.is_none());

    engine
        .submit_no_attempt(input("stuck"), "no_recall")
        .unwrap();
    let summary = engine.close_session("stuck").unwrap();
    assert_eq!(summary.attempts_count, 0);
    assert_eq!(summary.top_stuck_concept_id.as_deref(), Some("ownership"));
    assert_eq!(summary.concepts_touched[0].no_attempt_count, 1);
}

#[test]
fn schema_v5_registers_the_nullable_enum_column() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    assert_eq!(CURRENT_SCHEMA_VERSION, 7);
    assert!(column_exists(&conn, "attempts", "no_attempt_reason"));
    let migrations: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(migrations, 7);
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn input(session_id: &str) -> SubmitInput {
    SubmitInput {
        session_id: session_id.to_owned(),
        concept_id: "ownership".to_owned(),
        task_type: "recall".to_owned(),
        prompt_text: "Explain ownership.".to_owned(),
        response_text: String::new(),
        self_confidence: 1,
        latency_ms: 800,
        hint_count: 0,
    }
}

fn theta_snapshot(engine: &Engine) -> (Vec<u8>, Vec<u8>, i64) {
    engine
        .conn()
        .query_row("SELECT vec, g2, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
}

fn counts(engine: &Engine) -> (i64, i64, i64, i64) {
    (
        row_count(engine, "sessions"),
        row_count(engine, "evidence_items"),
        row_count(engine, "attempts"),
        row_count(engine, "behavior_events"),
    )
}

fn row_count(engine: &Engine, table: &str) -> i64 {
    engine
        .conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn event_count(engine: &Engine, kind: &str) -> i64 {
    engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type=?1",
            [kind],
            |row| row.get(0),
        )
        .unwrap()
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .iter()
        .any(|name| name == column)
}
