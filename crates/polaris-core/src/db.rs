use std::path::Path;

use rusqlite::Connection;

use crate::config::default_registry;
use crate::error::Result;

pub fn open_database(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS op_log(
            op_id TEXT PRIMARY KEY,
            ts TEXT,
            entity TEXT,
            entity_id TEXT,
            type TEXT,
            payload_json TEXT,
            evidence_refs_json TEXT,
            actor TEXT,
            lamport INTEGER
        );

        CREATE TABLE IF NOT EXISTS evidence_items(
            id TEXT PRIMARY KEY,
            session_id TEXT,
            source TEXT,
            content_type TEXT,
            text TEXT NOT NULL,
            lang TEXT,
            concept_ids_json TEXT,
            created_at TEXT
        );

        CREATE TABLE IF NOT EXISTS attempts(
            id TEXT PRIMARY KEY,
            session_id TEXT,
            concept_id TEXT NOT NULL,
            task_type TEXT,
            prompt_text TEXT,
            response_evidence_id TEXT,
            self_confidence INTEGER,
            latency_ms INTEGER,
            hint_count INTEGER DEFAULT 0,
            provisional_score REAL,
            final_score REAL,
            depth TEXT,
            misconception_id TEXT,
            grader_json TEXT,
            rating TEXT,
            theta_version INTEGER,
            created_at TEXT,
            graded_at TEXT
        );

        CREATE TABLE IF NOT EXISTS concepts(
            id TEXT PRIMARY KEY,
            pack TEXT,
            name TEXT,
            kind TEXT DEFAULT 'concept',
            seed_order INTEGER,
            p_init REAL,
            b_difficulty REAL DEFAULT 0,
            q BLOB,
            embedding BLOB,
            provenance TEXT,
            evidence_ids_json TEXT,
            created_at TEXT
        );

        CREATE TABLE IF NOT EXISTS edges(
            id TEXT PRIMARY KEY,
            src TEXT,
            dst TEXT,
            type TEXT,
            weight REAL DEFAULT 1.0,
            alignment_json TEXT,
            provenance TEXT,
            evidence_ids_json TEXT,
            created_at TEXT
        );

        CREATE TABLE IF NOT EXISTS mastery_states(
            concept_id TEXT PRIMARY KEY,
            p_known REAL,
            fsrs_json TEXT,
            next_due_at TEXT,
            last_review_at TEXT,
            calib_gap REAL DEFAULT 0,
            brier_ewma REAL DEFAULT 0,
            last_depth TEXT,
            max_depth TEXT,
            attempt_count INTEGER DEFAULT 0,
            lapses INTEGER DEFAULT 0,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sessions(
            id TEXT PRIMARY KEY,
            started_at TEXT,
            ended_at TEXT,
            context_json TEXT
        );

        CREATE TABLE IF NOT EXISTS behavior_events(
            id TEXT PRIMARY KEY,
            session_id TEXT,
            at TEXT,
            type TEXT,
            concept_id TEXT,
            payload_json TEXT
        );

        CREATE TABLE IF NOT EXISTS grade_queue(
            attempt_id TEXT PRIMARY KEY,
            enqueued_at TEXT,
            retry_count INTEGER DEFAULT 0,
            last_error TEXT
        );
        "#,
    )?;

    let registry = default_registry();
    let mut stmt = conn.prepare("INSERT OR IGNORE INTO meta(key, value) VALUES (?1, ?2)")?;
    for spec in registry.values() {
        stmt.execute((spec.key, spec.default_value))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn migration_creates_p01_tables_and_default_meta() {
        let conn = Connection::open_in_memory().unwrap();

        migrate(&conn).unwrap();

        for table in [
            "meta",
            "op_log",
            "evidence_items",
            "attempts",
            "concepts",
            "edges",
            "mastery_states",
            "sessions",
            "behavior_events",
            "grade_queue",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "missing table {table}");
        }

        let p_init: String = conn
            .query_row("SELECT value FROM meta WHERE key='bkt.p_init'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(p_init, "0.20");

        let quote_min: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='grade.quote_min'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(quote_min, "8");
    }

    #[test]
    fn file_database_uses_wal() {
        let path = std::env::temp_dir().join(format!(
            "polaris-core-wal-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let conn = open_database(&path).unwrap();

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();

        drop(conn);
        let _ = std::fs::remove_file(path);

        assert_eq!(journal_mode.to_lowercase(), "wal");
    }
}
