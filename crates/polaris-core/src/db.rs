use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::config::default_registry;
use crate::error::{PolarisError, Result};

pub const CURRENT_SCHEMA_VERSION: i64 = 3;

const BASELINE_SCHEMA_VERSION: i64 = 1;
const BASELINE_MIGRATION_NAME: &str = "baseline_current_schema";
const CAPTURE_QUEUE_SCHEMA_VERSION: i64 = 2;
const CAPTURE_QUEUE_MIGRATION_NAME: &str = "capture_queue";
const GLOBAL_PROFILE_SCHEMA_VERSION: i64 = 3;
const GLOBAL_PROFILE_MIGRATION_NAME: &str = "global_profile_governance";

pub fn open_database(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;
    ensure_supported_schema_version(&conn)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_database_read_only(path: impl AsRef<Path>) -> Result<Connection> {
    Connection::open_with_flags(path.as_ref(), OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(Into::into)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    ensure_supported_schema_version(conn)?;

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
            theta_scope TEXT DEFAULT 'shared',
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
            phase TEXT DEFAULT 'undetermined',
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

        CREATE TABLE IF NOT EXISTS capture_queue(
            id TEXT PRIMARY KEY,
            evidence_id TEXT NOT NULL,
            status TEXT NOT NULL,
            learner_kind TEXT NOT NULL,
            candidate_concept_ids_json TEXT NOT NULL DEFAULT '[]',
            note TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(evidence_id) REFERENCES evidence_items(id)
        );

        CREATE TABLE IF NOT EXISTS theta(
            id INTEGER PRIMARY KEY CHECK(id=1),
            vec BLOB,
            g2 BLOB,
            version INTEGER,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS theta_history(
            version INTEGER PRIMARY KEY,
            vec BLOB,
            at TEXT
        );

        CREATE TABLE IF NOT EXISTS pack_theta(
            pack TEXT PRIMARY KEY,
            vec BLOB,
            g2 BLOB,
            version INTEGER,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS pack_theta_history(
            pack TEXT,
            version INTEGER,
            vec BLOB,
            at TEXT,
            PRIMARY KEY(pack, version)
        );

        CREATE TABLE IF NOT EXISTS residual_stats(
            concept_id TEXT,
            week TEXT,
            mean_resid REAL,
            n INTEGER,
            PRIMARY KEY(concept_id, week)
        );

        CREATE TABLE IF NOT EXISTS consolidation_runs(
            id TEXT PRIMARY KEY,
            ran_at TEXT,
            proposals_json TEXT,
            holdout_delta REAL,
            status TEXT
        );

        CREATE TABLE IF NOT EXISTS moves_effects(
            move TEXT,
            context_hash TEXT,
            alpha REAL,
            beta REAL,
            n INTEGER,
            PRIMARY KEY(move, context_hash)
        );

        CREATE TABLE IF NOT EXISTS mrt_log(
            id TEXT PRIMARY KEY,
            at TEXT,
            context_json TEXT,
            randomized INTEGER,
            move TEXT,
            prereg_id TEXT
        );

        CREATE TABLE IF NOT EXISTS bred_moves(
            id TEXT PRIMARY KEY,
            candidate_move TEXT NOT NULL,
            incumbent_move TEXT NOT NULL,
            context_hash TEXT NOT NULL,
            task_type TEXT NOT NULL,
            template TEXT NOT NULL,
            mechanisms_json TEXT NOT NULL,
            main_effect_hypothesis TEXT NOT NULL,
            prereg_json TEXT NOT NULL,
            status TEXT NOT NULL,
            posterior_win_prob REAL DEFAULT 0.0,
            candidate_alpha REAL DEFAULT 0.0,
            candidate_beta REAL DEFAULT 0.0,
            incumbent_alpha REAL DEFAULT 0.0,
            incumbent_beta REAL DEFAULT 0.0,
            n_candidate INTEGER DEFAULT 0,
            n_incumbent INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT,
            admitted_at TEXT,
            retired_at TEXT
        );

        CREATE TABLE IF NOT EXISTS param_tuning_runs(
            id TEXT PRIMARY KEY,
            ran_at TEXT,
            param TEXT,
            old_value TEXT,
            new_value TEXT,
            metric TEXT,
            delta REAL,
            status TEXT
        );

        CREATE TABLE IF NOT EXISTS gu_rules(
            id TEXT PRIMARY KEY,
            pattern TEXT NOT NULL,
            concept_ids_json TEXT NOT NULL,
            attempt_ids_json TEXT NOT NULL,
            first_seen TEXT,
            last_seen TEXT,
            count INTEGER DEFAULT 0,
            status TEXT NOT NULL,
            alpha REAL DEFAULT 1.0,
            beta REAL DEFAULT 1.0,
            consumed_at TEXT,
            correct_streak INTEGER DEFAULT 0,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS mirror_reports(
            id TEXT PRIMARY KEY,
            week TEXT NOT NULL,
            generated_at TEXT,
            report_json TEXT NOT NULL,
            assertion_count INTEGER DEFAULT 0,
            skipped_count INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS hazard_models(
            id TEXT PRIMARY KEY,
            fitted_at TEXT,
            beta_json TEXT NOT NULL,
            validation_auc REAL NOT NULL,
            n_train INTEGER,
            n_validation INTEGER
        );

        CREATE TABLE IF NOT EXISTS state_gate_evals(
            id TEXT PRIMARY KEY,
            evaluated_at TEXT,
            baseline_auc REAL NOT NULL,
            state_auc REAL NOT NULL,
            margin REAL NOT NULL,
            passes INTEGER NOT NULL,
            n INTEGER
        );

        CREATE TABLE IF NOT EXISTS goals(
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            deadline TEXT,
            pace TEXT,
            priority INTEGER NOT NULL DEFAULT 50,
            parent_goal_id TEXT,
            completion_summary TEXT
        );

        CREATE TABLE IF NOT EXISTS goal_dimensions(
            id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL REFERENCES goals(id),
            dimension_key TEXT NOT NULL,
            display_name TEXT NOT NULL,
            metric_type TEXT NOT NULL,
            target_value REAL NOT NULL,
            target_label TEXT,
            weight REAL NOT NULL DEFAULT 1.0,
            current_value REAL NOT NULL DEFAULT 0,
            current_updated_at TEXT,
            query_sql TEXT,
            query_hint TEXT,
            UNIQUE(goal_id, dimension_key)
        );

        CREATE TABLE IF NOT EXISTS goal_milestones(
            id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL REFERENCES goals(id),
            title TEXT NOT NULL,
            description TEXT,
            trigger_type TEXT NOT NULL,
            trigger_config TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            reached_at TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            UNIQUE(goal_id, sort_order)
        );

        CREATE TABLE IF NOT EXISTS schema_migrations(
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_attempts_concept_created
            ON attempts(concept_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_attempts_created
            ON attempts(created_at);
        CREATE INDEX IF NOT EXISTS idx_concepts_pack
            ON concepts(pack);
        CREATE INDEX IF NOT EXISTS idx_behavior_type_at
            ON behavior_events(type, at);
        CREATE INDEX IF NOT EXISTS idx_behavior_session_type_at
            ON behavior_events(session_id, type, at);
        CREATE INDEX IF NOT EXISTS idx_behavior_attempt
            ON behavior_events(json_extract(payload_json, '$.attempt_id'));
        CREATE INDEX IF NOT EXISTS idx_gu_rules_status
            ON gu_rules(status);
        CREATE INDEX IF NOT EXISTS idx_edges_src
            ON edges(src);
        CREATE INDEX IF NOT EXISTS idx_edges_dst
            ON edges(dst);
        CREATE INDEX IF NOT EXISTS idx_bred_moves_status_context
            ON bred_moves(status, context_hash);
        CREATE INDEX IF NOT EXISTS idx_goals_updated_at
            ON goals(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_goals_status
            ON goals(status, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_goals_parent
            ON goals(parent_goal_id);
        CREATE INDEX IF NOT EXISTS idx_goal_dim_goal
            ON goal_dimensions(goal_id);
        CREATE INDEX IF NOT EXISTS idx_milestone_goal
            ON goal_milestones(goal_id);
        CREATE INDEX IF NOT EXISTS idx_capture_queue_status_updated
            ON capture_queue(status, updated_at DESC);
        "#,
    )?;

    ensure_column(
        conn,
        "mastery_states",
        "phase",
        "TEXT DEFAULT 'undetermined'",
    )?;
    ensure_column(conn, "theta", "g2", "BLOB")?;
    ensure_column(conn, "attempts", "theta_scope", "TEXT DEFAULT 'shared'")?;

    let registry = default_registry();
    let mut stmt = conn.prepare("INSERT OR IGNORE INTO meta(key, value) VALUES (?1, ?2)")?;
    for spec in registry.values() {
        stmt.execute((spec.key, spec.default_value))?;
    }

    record_schema_migration(conn, BASELINE_SCHEMA_VERSION, BASELINE_MIGRATION_NAME)?;
    record_schema_migration(
        conn,
        CAPTURE_QUEUE_SCHEMA_VERSION,
        CAPTURE_QUEUE_MIGRATION_NAME,
    )?;
    migrate_global_profile_schema(conn)?;

    Ok(())
}

fn migrate_global_profile_schema(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS profile_settings(
            id INTEGER PRIMARY KEY CHECK(id=1),
            enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
            disclosure_acknowledged_at TEXT,
            summary_sharing_enabled INTEGER NOT NULL DEFAULT 0
                CHECK(summary_sharing_enabled IN (0, 1)),
            paused_until TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_dimensions(
            scope TEXT NOT NULL CHECK(scope IN ('global', 'pack', 'goal')),
            scope_id TEXT NOT NULL DEFAULT '',
            dimension_key TEXT NOT NULL,
            mean REAL NOT NULL,
            variance REAL NOT NULL CHECK(variance >= 0),
            evidence_count INTEGER NOT NULL CHECK(evidence_count >= 0),
            model_version TEXT NOT NULL,
            gate_status TEXT NOT NULL
                CHECK(gate_status IN ('unfit', 'shadow', 'active', 'suspended')),
            provenance_json TEXT NOT NULL,
            evidence_ids_json TEXT NOT NULL DEFAULT '[]',
            updated_at TEXT NOT NULL,
            PRIMARY KEY(scope, scope_id, dimension_key),
            CHECK(
                (scope='global' AND scope_id='') OR
                (scope IN ('pack', 'goal') AND length(scope_id) > 0)
            )
        );

        CREATE TABLE IF NOT EXISTS profile_validation_runs(
            id TEXT PRIMARY KEY,
            scope TEXT NOT NULL CHECK(scope IN ('global', 'pack', 'goal')),
            scope_id TEXT NOT NULL DEFAULT '',
            dimension_key TEXT NOT NULL,
            model_version TEXT NOT NULL,
            status TEXT NOT NULL
                CHECK(status IN ('unfit', 'shadow', 'active', 'suspended')),
            metrics_json TEXT NOT NULL,
            provenance_json TEXT NOT NULL,
            evidence_ids_json TEXT NOT NULL DEFAULT '[]',
            ran_at TEXT NOT NULL,
            CHECK(
                (scope='global' AND scope_id='') OR
                (scope IN ('pack', 'goal') AND length(scope_id) > 0)
            )
        );

        CREATE TABLE IF NOT EXISTS profile_data_actions(
            id TEXT PRIMARY KEY,
            action TEXT NOT NULL CHECK(action='profile_reset'),
            measurements_deleted INTEGER NOT NULL CHECK(measurements_deleted >= 0),
            dimensions_deleted INTEGER NOT NULL CHECK(dimensions_deleted >= 0),
            validation_runs_deleted INTEGER NOT NULL CHECK(validation_runs_deleted >= 0),
            at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_profile_dimensions_gate
            ON profile_dimensions(gate_status, scope, scope_id);
        CREATE INDEX IF NOT EXISTS idx_profile_validation_dimension_at
            ON profile_validation_runs(scope, scope_id, dimension_key, ran_at DESC);
        CREATE INDEX IF NOT EXISTS idx_profile_data_actions_at
            ON profile_data_actions(at DESC);

        INSERT OR IGNORE INTO profile_settings(
            id, enabled, disclosure_acknowledged_at, summary_sharing_enabled,
            paused_until, created_at, updated_at
        ) VALUES (
            1, 1, NULL, 0, NULL,
            strftime('%Y-%m-%dT%H:%M:%SZ','now'),
            strftime('%Y-%m-%dT%H:%M:%SZ','now')
        );
        "#,
    )?;
    record_schema_migration(
        &tx,
        GLOBAL_PROFILE_SCHEMA_VERSION,
        GLOBAL_PROFILE_MIGRATION_NAME,
    )?;
    set_schema_version(&tx, CURRENT_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(Into::into)
}

pub fn schema_migration_count(conn: &Connection) -> Result<i64> {
    if !table_exists(conn, "schema_migrations")? {
        return Ok(0);
    }
    conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn ensure_supported_schema_version(conn: &Connection) -> Result<()> {
    let user_version = schema_version(conn)?;
    if user_version > CURRENT_SCHEMA_VERSION {
        return Err(PolarisError::UnsupportedSchemaVersion {
            found: user_version,
            current: CURRENT_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn set_schema_version(conn: &Connection, version: i64) -> Result<()> {
    conn.pragma_update(None, "user_version", version)?;
    Ok(())
}

fn record_schema_migration(conn: &Connection, version: i64, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        (version, name),
    )?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }

    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::mirt::{decode_vector, encode_vector, ensure_theta};

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
            "capture_queue",
            "theta",
            "theta_history",
            "pack_theta",
            "pack_theta_history",
            "residual_stats",
            "consolidation_runs",
            "moves_effects",
            "mrt_log",
            "bred_moves",
            "param_tuning_runs",
            "gu_rules",
            "mirror_reports",
            "hazard_models",
            "state_gate_evals",
            "goals",
            "goal_dimensions",
            "goal_milestones",
            "profile_settings",
            "profile_dimensions",
            "profile_validation_runs",
            "profile_data_actions",
            "schema_migrations",
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

        let user_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, CURRENT_SCHEMA_VERSION);

        let migration_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version=?1",
                [CURRENT_SCHEMA_VERSION],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migration_count, 1);
    }

    #[test]
    fn migration_is_idempotent_and_preserves_user_meta() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE meta(
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta(key, value) VALUES ('bkt.p_init', '0.33');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();
        let first_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        migrate(&conn).unwrap();

        let p_init: String = conn
            .query_row("SELECT value FROM meta WHERE key='bkt.p_init'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let second_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(p_init, "0.33");
        assert_eq!(first_count, 3);
        assert_eq!(second_count, first_count);
    }

    #[test]
    fn migration_baselines_unversioned_existing_database_without_losing_data() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE evidence_items(
                id TEXT PRIMARY KEY,
                session_id TEXT,
                source TEXT,
                content_type TEXT,
                text TEXT NOT NULL,
                lang TEXT,
                concept_ids_json TEXT,
                created_at TEXT
            );
            INSERT INTO evidence_items(id, session_id, source, content_type, text, lang, concept_ids_json, created_at)
            VALUES ('e1', 's1', 'legacy', 'text/plain', 'keep this evidence', 'en', '[]', '2026-01-01T00:00:00Z');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();

        let evidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_items WHERE id='e1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let user_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(evidence_count, 1);
        assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migration_rejects_newer_database_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
            .unwrap();

        let error = migrate(&conn).unwrap_err().to_string();

        assert!(error.contains("unsupported database schema version"));
    }

    #[test]
    fn open_database_rejects_newer_file_database_before_wal_write() {
        let path = temp_db_path("newer-schema");
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
            conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
                .unwrap();
        }

        let error = open_database(&path).unwrap_err().to_string();
        let conn = Connection::open(&path).unwrap();
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        drop(conn);

        let wal_exists = path.with_extension("db-wal").exists();
        let shm_exists = path.with_extension("db-shm").exists();
        cleanup_db_path(&path);

        assert!(error.contains("unsupported database schema version"));
        assert_eq!(journal_mode.to_lowercase(), "delete");
        assert!(!wal_exists);
        assert!(!shm_exists);
    }

    #[test]
    fn open_database_migrates_existing_file_database_to_current_schema_version() {
        let path = temp_db_path("legacy-schema");
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE meta(
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                INSERT INTO meta(key, value) VALUES ('bkt.p_init', '0.33');
                "#,
            )
            .unwrap();
        }

        let conn = open_database(&path).unwrap();

        let user_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let p_init: String = conn
            .query_row("SELECT value FROM meta WHERE key='bkt.p_init'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let migration_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();

        drop(conn);
        cleanup_db_path(&path);

        assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(p_init, "0.33");
        assert_eq!(migration_count, 3);
        assert_eq!(journal_mode.to_lowercase(), "wal");
    }

    #[test]
    fn migration_backfills_theta_adagrad_column_for_existing_database() {
        let conn = Connection::open_in_memory().unwrap();
        let zero = vec![0.0; 32];
        conn.execute_batch(
            r#"
            CREATE TABLE theta(
                id INTEGER PRIMARY KEY CHECK(id=1),
                vec BLOB,
                version INTEGER,
                updated_at TEXT
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO theta(id, vec, version, updated_at)
             VALUES (1, ?1, 1, '2026-01-01T00:00:00Z')",
            [encode_vector(&zero)],
        )
        .unwrap();

        migrate(&conn).unwrap();
        let g2_before: Option<Vec<u8>> = conn
            .query_row("SELECT g2 FROM theta WHERE id=1", [], |row| row.get(0))
            .unwrap();
        assert!(g2_before.is_none());

        ensure_theta(&conn).unwrap();
        let g2_blob: Vec<u8> = conn
            .query_row("SELECT g2 FROM theta WHERE id=1", [], |row| row.get(0))
            .unwrap();
        let g2 = decode_vector(&g2_blob).unwrap();
        assert_eq!(g2.len(), zero.len());
        assert!(g2.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn migration_creates_hot_path_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        for index in [
            "idx_attempts_concept_created",
            "idx_attempts_created",
            "idx_concepts_pack",
            "idx_behavior_type_at",
            "idx_behavior_session_type_at",
            "idx_behavior_attempt",
            "idx_gu_rules_status",
            "idx_edges_src",
            "idx_edges_dst",
            "idx_bred_moves_status_context",
            "idx_goals_updated_at",
            "idx_goals_status",
            "idx_goals_parent",
            "idx_goal_dim_goal",
            "idx_milestone_goal",
            "idx_capture_queue_status_updated",
            "idx_profile_dimensions_gate",
            "idx_profile_validation_dimension_at",
            "idx_profile_data_actions_at",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "missing index {index}");
        }
    }

    #[test]
    fn hot_queries_use_indexes_not_full_scans() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        for (query, expected_index) in [
            (
                "SELECT id FROM behavior_events WHERE type='mental_state' ORDER BY at DESC",
                "idx_behavior_type_at",
            ),
            (
                "SELECT id FROM attempts WHERE concept_id='c1' ORDER BY created_at",
                "idx_attempts_concept_created",
            ),
            ("SELECT id FROM edges WHERE src='c1'", "idx_edges_src"),
            (
                "SELECT id FROM gu_rules WHERE status='active'",
                "idx_gu_rules_status",
            ),
        ] {
            let plan = query_plan(&conn, query);
            assert!(
                plan.contains(expected_index),
                "query `{query}` plan `{plan}` should use {expected_index}"
            );
            assert!(
                !plan.contains("SCAN behavior_events")
                    && !plan.contains("SCAN attempts")
                    && !plan.contains("SCAN edges")
                    && !plan.contains("SCAN gu_rules"),
                "query `{query}` plan `{plan}` must not full-scan"
            );
        }
    }

    fn query_plan(conn: &Connection, query: &str) -> String {
        let mut stmt = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {query}"))
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        rows.join(" | ")
    }

    #[test]
    fn file_database_uses_wal() {
        let path = temp_db_path("wal");
        let conn = open_database(&path).unwrap();

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();

        drop(conn);
        cleanup_db_path(&path);

        assert_eq!(journal_mode.to_lowercase(), "wal");
    }

    fn temp_db_path(prefix: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("polaris-core-{prefix}-{suffix}.db"))
    }

    fn cleanup_db_path(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
}
