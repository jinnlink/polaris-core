use polaris_core::capture_queue::{CaptureEffect, CaptureInput, CaptureStatus, LearnerCaptureKind};
use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use rusqlite::Connection;

#[test]
fn captured_evidence_is_recorded_only_and_does_not_create_attempt_or_mastery() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let engine = Engine::new(conn);

    let before_attempts = count_rows(engine.conn(), "attempts");
    let before_mastery = count_rows(engine.conn(), "mastery_states");
    let before_grade_queue = count_rows(engine.conn(), "grade_queue");

    let record = engine
        .capture_learning_evidence(CaptureInput {
            session_id: Some("capture-session".to_owned()),
            source: "paste".to_owned(),
            content_type: "text/plain".to_owned(),
            text: "我刚看了 Rust 所有权的一段解释，先存下来稍后处理。".to_owned(),
            learner_kind: LearnerCaptureKind::Reference,
            candidate_concept_ids: Vec::new(),
            note: Some("来自学习项目入口".to_owned()),
        })
        .unwrap();

    assert_eq!(record.effect, CaptureEffect::RecordedOnly);
    assert_eq!(record.status, CaptureStatus::Pending);
    assert_eq!(record.status.as_str(), "pending");
    assert_eq!(record.learner_kind, LearnerCaptureKind::Reference);
    assert_eq!(record.learner_kind.as_str(), "reference");
    assert!(record.message.contains("不会直接算作掌握"));
    assert_eq!(count_rows(engine.conn(), "attempts"), before_attempts);
    assert_eq!(count_rows(engine.conn(), "mastery_states"), before_mastery);
    assert_eq!(count_rows(engine.conn(), "grade_queue"), before_grade_queue);

    let evidence_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM evidence_items WHERE id=?1 AND source='paste'",
            [record.evidence_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(evidence_count, 1);

    let (status, learner_kind, note): (String, String, Option<String>) = engine
        .conn()
        .query_row(
            "SELECT status, learner_kind, note
             FROM capture_queue
             WHERE id=?1 AND evidence_id=?2",
            [record.capture_id.as_str(), record.evidence_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(learner_kind, "reference");
    assert_eq!(note.as_deref(), Some("来自学习项目入口"));
}

#[test]
fn migration_creates_capture_queue_and_records_schema_version() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='capture_queue'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let migration_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=?1",
            [CURRENT_SCHEMA_VERSION],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(table_count, 1);
    assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(migration_count, 1);
}

fn count_rows(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}
