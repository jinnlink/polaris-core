mod common;

use common::workspace_pack_path;
use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_core::grader::{evidence_for_attempt, grade_with_static_response, GradeRequest};
use polaris_core::teaching::teaching_context;
use rusqlite::{params, Connection};

#[test]
fn schema_v6_and_empty_history_are_safe() {
    let engine = seeded_engine();
    assert_eq!(CURRENT_SCHEMA_VERSION, 7);
    assert!(table_exists(engine.conn(), "teaching_turns"));
    assert!(teaching_context(engine.conn(), "ownership")
        .unwrap()
        .is_none());
    assert!(engine
        .teaching_instruction("ownership")
        .unwrap()
        .context
        .is_none());
}

#[test]
fn schema_v6_migration_is_atomic_on_conflicting_old_table() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute_batch(
        "DROP TABLE teaching_turns;
         DELETE FROM schema_migrations WHERE version=6;
         PRAGMA user_version=5;
         CREATE TABLE teaching_turns(id TEXT PRIMARY KEY);",
    )
    .unwrap();

    assert!(migrate(&conn).is_err());
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let migration_v6: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=6",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 5);
    assert_eq!(migration_v6, 0);
}

#[test]
fn context_returns_only_latest_configured_attempts_in_stable_order() {
    let engine = seeded_engine();
    for index in 0..5 {
        insert_attempt(&engine, index, if index == 3 { 0.10 } else { 0.80 });
    }

    let context = teaching_context(engine.conn(), "ownership")
        .unwrap()
        .unwrap();
    assert_eq!(context.recent_attempts.len(), 3);
    assert_eq!(
        context
            .recent_attempts
            .iter()
            .map(|attempt| attempt.task_type.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["task-4", "task-3", "task-2"]
    );
    assert_eq!(context.latest_failed_response.as_deref(), Some("answer-3"));
}

#[test]
fn active_gu_rules_are_recalled_without_lifecycle_writes() {
    let engine = seeded_engine();
    engine
        .conn()
        .execute(
            "INSERT INTO gu_rules(
                 id, pattern, concept_ids_json, attempt_ids_json, status, updated_at
             ) VALUES ('gu-active', 'boundary-blindness', '[\"ownership\"]', '[]',
                       'active', '2026-08-09T00:00:00Z')",
            [],
        )
        .unwrap();

    let context = teaching_context(engine.conn(), "ownership")
        .unwrap()
        .unwrap();
    assert_eq!(context.active_gu_rules.len(), 1);
    assert_eq!(context.active_gu_rules[0].pattern, "boundary-blindness");
    let status: String = engine
        .conn()
        .query_row(
            "SELECT status FROM gu_rules WHERE id='gu-active'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "active");
}

#[test]
fn teaching_explanation_never_expands_attempt_citation_evidence() {
    let engine = seeded_engine();
    engine
        .conn()
        .execute(
            "INSERT INTO sessions(id, started_at, context_json)
             VALUES ('teach', '2026-08-09T00:00:00Z', '{}')",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO evidence_items(
                 id, session_id, source, content_type, text, concept_ids_json, created_at
             ) VALUES ('learner-evidence', 'teach', 'learner', 'text/plain',
                       'learner own answer text', '[\"ownership\"]',
                       '2026-08-09T00:00:01Z')",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(
                 id, session_id, concept_id, task_type, prompt_text,
                 response_evidence_id, self_confidence, created_at
             ) VALUES ('attempt-1', 'teach', 'ownership', 'recall', 'prompt',
                       'learner-evidence', 3, '2026-08-09T00:00:02Z')",
            [],
        )
        .unwrap();
    let before = evidence_for_attempt(engine.conn(), "attempt-1").unwrap();
    let instruction = engine.teaching_instruction("ownership").unwrap();
    let turn = engine
        .begin_teaching_turn("teach", "ownership", &instruction)
        .unwrap();
    let receipt = engine
        .record_teaching_explanation(&turn.id, "tutor explanation must not grade itself")
        .unwrap();

    assert_eq!(
        evidence_for_attempt(engine.conn(), "attempt-1").unwrap(),
        before
    );
    let grade = grade_with_static_response(
        engine.conn(),
        GradeRequest {
            attempt_id: "attempt-1".to_owned(),
            self_confidence: 3,
            response_text: "learner own answer text".to_owned(),
        },
        &serde_json::json!({
            "score": 1.0,
            "depth": "recall",
            "citations": [{
                "evidence_id": receipt.evidence_id,
                "quote": "tutor explanation must not grade itself"
            }]
        })
        .to_string(),
    )
    .unwrap();
    assert!(grade.degraded);
}

#[test]
fn delivered_anchor_is_recalled_and_task_choice_is_unchanged() {
    let engine = seeded_engine();
    let before = engine.next_task().unwrap().unwrap();
    let instruction = engine.teaching_instruction(&before.concept_id).unwrap();
    engine
        .begin_teaching_turn("anchor", &before.concept_id, &instruction)
        .unwrap();
    let context = teaching_context(engine.conn(), &before.concept_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        context.previous_anchor.as_deref(),
        Some(instruction.anchor.as_str())
    );

    let after = engine.next_task().unwrap().unwrap();
    assert_eq!(
        (
            &before.concept_id,
            &before.move_id,
            &before.task_type,
            &before.prompt_text,
            &before.reason,
        ),
        (
            &after.concept_id,
            &after.move_id,
            &after.task_type,
            &after.prompt_text,
            &after.reason,
        )
    );
    assert!(before.context.is_none());
    assert!(after.context.is_some());
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .conn()
        .execute("UPDATE meta SET value='0.0' WHERE key='mrt.epsilon'", [])
        .unwrap();
    engine
}

fn insert_attempt(engine: &Engine, index: i64, score: f64) {
    let evidence_id = format!("evidence-{index}");
    let attempt_id = format!("attempt-{index}");
    let task_type = format!("task-{index}");
    let created_at = format!("2026-08-09T00:00:0{index}Z");
    engine
        .conn()
        .execute(
            "INSERT INTO evidence_items(
                 id, source, content_type, text, concept_ids_json, created_at
             ) VALUES (?1, 'learner', 'text/plain', ?2, '[\"ownership\"]', ?3)",
            params![evidence_id, format!("answer-{index}"), created_at],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(
                 id, concept_id, task_type, response_evidence_id, self_confidence,
                 final_score, misconception_id, created_at
             ) VALUES (?1, 'ownership', ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attempt_id,
                task_type,
                evidence_id,
                index + 1,
                score,
                format!("mis-{index}"),
                created_at
            ],
        )
        .unwrap();
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
    .unwrap()
}
