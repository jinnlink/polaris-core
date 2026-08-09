mod common;

use common::workspace_pack_path;
use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use rusqlite::{params, Connection};

#[test]
fn schema_v4_adds_closeout_fields_table_and_atomic_migration() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    assert_eq!(CURRENT_SCHEMA_VERSION, 5);
    assert_eq!(user_version(&conn), 5);
    assert_eq!(migration_count(&conn), 5);
    assert!(column_exists(&conn, "sessions", "ended_at"));
    assert!(column_exists(&conn, "sessions", "closed_at"));
    assert!(table_exists(&conn, "session_summaries"));

    let interrupted = Connection::open_in_memory().unwrap();
    interrupted
        .execute_batch("CREATE VIEW session_summaries AS SELECT 1 AS conflict;")
        .unwrap();
    let error = migrate(&interrupted).unwrap_err().to_string();

    assert!(!error.is_empty());
    assert_eq!(user_version(&interrupted), 3);
    assert_eq!(migration_count(&interrupted), 3);
    assert!(!column_exists(&interrupted, "sessions", "closed_at"));
}

#[test]
fn empty_and_single_attempt_sessions_close_with_stable_shapes() {
    let engine = seeded_engine();
    insert_session(&engine, "empty");

    let empty = engine.close_session("empty").unwrap();
    assert!(empty.concepts_touched.is_empty());
    assert_eq!(empty.attempts_count, 0);
    assert!(empty.top_stuck_concept_id.is_none());
    assert!(empty.assertions.is_empty());
    assert_eq!(empty.next_entry_concept_id.as_deref(), Some("ownership"));
    assert!(empty.ended_at.is_some());
    assert!(!empty.closed_at.is_empty());

    insert_session(&engine, "single");
    insert_attempt(&engine, "single-attempt", "single", "ownership", 0.82);
    let single = engine.close_session("single").unwrap();
    assert_eq!(single.attempts_count, 1);
    assert_eq!(single.concepts_touched.len(), 1);
    assert_eq!(single.concepts_touched[0].min_score, Some(0.82));
    assert_eq!(single.concepts_touched[0].max_score, Some(0.82));
    assert_eq!(single.assertions.len(), 1);
    assert_eq!(single.assertions[0].evidence_ids, vec!["single-attempt"]);
}

#[test]
fn multi_concept_closeout_is_session_bound_evidence_bound_and_idempotent() {
    let engine = seeded_engine();
    insert_session(&engine, "focus");
    insert_session(&engine, "outside");
    for (index, concept, score) in [
        (0, "ownership", 0.91),
        (1, "references", 0.35),
        (2, "borrowing", 0.72),
        (3, "pattern_matching", 0.63),
    ] {
        insert_attempt(
            &engine,
            &format!("focus-attempt-{index}"),
            "focus",
            concept,
            score,
        );
    }
    insert_behavior(&engine, "focus-hint-1", "focus", "references", "hint");
    insert_behavior(&engine, "focus-hint-2", "focus", "references", "hint");
    insert_behavior(&engine, "focus-abandon", "focus", "references", "abandon");
    insert_attempt(&engine, "outside-attempt", "outside", "traits", 0.01);

    let events_before = row_count(&engine, "behavior_events");
    let first = engine.close_session("focus").unwrap();
    let second = engine.close_session("focus").unwrap();

    assert_eq!(first, second);
    assert_eq!(row_count(&engine, "behavior_events"), events_before);
    assert_eq!(first.attempts_count, 4);
    assert_eq!(first.concepts_touched.len(), 4);
    assert_eq!(first.assertions.len(), 3, "hard product cap");
    assert!(first
        .assertions
        .iter()
        .all(|assertion| !assertion.evidence_ids.is_empty()));
    assert_eq!(first.top_stuck_concept_id.as_deref(), Some("references"));
    assert!(!first
        .concepts_touched
        .iter()
        .any(|touch| touch.concept_id == "traits"));
    assert!(!first
        .assertions
        .iter()
        .flat_map(|assertion| assertion.evidence_ids.iter())
        .any(|id| id == "outside-attempt"));

    let stored: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM session_summaries WHERE session_id='focus'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, 1);
    assert_eq!(engine.session_close_summary("focus").unwrap(), Some(first));
}

fn row_count(engine: &Engine, table: &str) -> i64 {
    engine
        .conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .conn()
        .execute("UPDATE meta SET value='0' WHERE key='mrt.epsilon'", [])
        .unwrap();
    engine
}

fn insert_session(engine: &Engine, session_id: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO sessions(id, started_at, context_json)
             VALUES (?1, '2026-08-09T08:00:00Z', '{}')",
            [session_id],
        )
        .unwrap();
}

fn insert_attempt(engine: &Engine, id: &str, session: &str, concept: &str, score: f64) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(
                 id, session_id, concept_id, task_type, self_confidence,
                 provisional_score, final_score, created_at, graded_at
             ) VALUES (?1, ?2, ?3, 'recall', 3, ?4, ?4,
                       '2026-08-09T09:00:00Z', '2026-08-09T09:00:01Z')",
            params![id, session, concept, score],
        )
        .unwrap();
}

fn insert_behavior(engine: &Engine, id: &str, session: &str, concept: &str, kind: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, ?2, '2026-08-09T09:05:00Z', ?3, ?4, '{}')",
            params![id, session, kind, concept],
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

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .iter()
        .any(|name| name == column)
}

fn user_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn migration_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
        row.get(0)
    })
    .unwrap()
}
