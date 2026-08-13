use std::path::{Path, PathBuf};

use polaris_core::db::{migrate, open_database, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_core::mastery::{fold_attempt, AttemptObservation, MasteryParams, MasteryState};
use polaris_core::profile::{
    delete_all_learning_data, profile_instruments, validate_profile_registry_json,
    FullDataDeletionRequest, ProfileDimensionInput, ProfileGateStatus, ProfileMeasurementInput,
    ProfileMeasurementStatus, ProfileScope, ProfileSettingsUpdate, ProfileValidationRunInput,
    DELETE_ALL_CONFIRMATION,
};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn schema_v3_creates_profile_tables_and_default_local_settings() {
    let conn = Connection::open_in_memory().unwrap();

    migrate(&conn).unwrap();

    assert_eq!(CURRENT_SCHEMA_VERSION, 10);
    for table in [
        "profile_settings",
        "profile_dimensions",
        "profile_validation_runs",
        "profile_data_actions",
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
    let settings = Engine::new(conn).global_profile_settings().unwrap();
    assert!(settings.enabled);
    assert!(settings.disclosure_required);
    assert!(!settings.summary_sharing_enabled);
    assert_eq!(settings.disclosure_acknowledged_at, None);
}

#[test]
fn schema_v3_upgrades_v2_without_losing_learning_facts_and_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE attempts(
            id TEXT PRIMARY KEY,
            concept_id TEXT NOT NULL,
            created_at TEXT
        );
        INSERT INTO attempts(id, concept_id, created_at)
        VALUES ('keep-attempt', 'ownership', '2026-08-08T00:00:00Z');
        CREATE TABLE schema_migrations(
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );
        INSERT INTO schema_migrations VALUES (1, 'baseline_current_schema', '2026-01-01T00:00:00Z');
        INSERT INTO schema_migrations VALUES (2, 'capture_queue', '2026-01-02T00:00:00Z');
        PRAGMA user_version=2;
        "#,
    )
    .unwrap();

    migrate(&conn).unwrap();
    migrate(&conn).unwrap();

    assert_eq!(table_count(&conn, "attempts"), 1);
    assert_eq!(table_count(&conn, "schema_migrations"), 10);
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_SCHEMA_VERSION);
}

#[test]
fn schema_v3_failure_rolls_back_the_entire_profile_migration_unit() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE schema_migrations(
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );
        INSERT INTO schema_migrations VALUES (1, 'baseline_current_schema', '2026-01-01T00:00:00Z');
        INSERT INTO schema_migrations VALUES (2, 'capture_queue', '2026-01-02T00:00:00Z');
        CREATE TABLE profile_settings(id INTEGER PRIMARY KEY);
        PRAGMA user_version=2;
        "#,
    )
    .unwrap();

    let error = migrate(&conn).unwrap_err();

    assert!(error.to_string().contains("enabled"));
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let v3_ledger: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=3",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let dimensions_table: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='profile_dimensions'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 2);
    assert_eq!(v3_ledger, 0);
    assert_eq!(dimensions_table, 0);
}

#[test]
fn bundled_registry_contains_only_redistributable_ipip_and_gse_items() {
    let instruments = profile_instruments().unwrap();

    assert_eq!(instruments.len(), 2);
    assert!(instruments
        .iter()
        .all(|instrument| instrument.license.redistributable));
    assert!(instruments.iter().all(|instrument| {
        !instrument.citation.trim().is_empty()
            && !instrument.locales.is_empty()
            && !instrument.items.is_empty()
            && !instrument.scoring.method.trim().is_empty()
            && !instrument.admin_modes.is_empty()
    }));
    assert!(instruments.iter().any(|instrument| {
        instrument.id == "ipip_learning_facets" && instrument.items.len() == 16
    }));
    assert!(instruments
        .iter()
        .any(|instrument| instrument.id == "gse" && instrument.items.len() == 10));
    assert!(instruments
        .iter()
        .all(|instrument| !instrument.id.to_ascii_lowercase().contains("pals")));

    let invalid = r#"[{"id":"restricted","title":"Restricted","version":"1","citation":"x","source_url":"https://example.invalid","license":{"name":"unknown","url":"https://example.invalid","redistributable":false,"attribution":"owner"},"locales":["en"],"items":[{"id":"i1","locale":"en","dimension":"x","prompt":"x","keyed":"positive"}],"scoring":{"method":"sum","response_min":1,"response_max":5,"reverse_formula":null},"admin_modes":["full_scale"],"interpretation_notice":"test"}]"#;
    let error = validate_profile_registry_json(invalid).unwrap_err();
    assert!(error.to_string().contains("redistributable"));

    let mut invalid_admin = serde_json::to_value(&instruments).unwrap();
    invalid_admin[0]["admin_modes"][0] = json!("clinical_diagnosis");
    let error = validate_profile_registry_json(&invalid_admin.to_string()).unwrap_err();
    assert!(error.to_string().contains("admin_mode"));
}

#[test]
fn measurements_require_disclosure_and_stop_immediately_when_disabled() {
    let engine = empty_engine();
    let input = measurement("s1", "gse", "gse_01", 4);

    let before_disclosure = engine.record_profile_measurement(input.clone()).unwrap();
    assert_eq!(
        before_disclosure.status,
        ProfileMeasurementStatus::DisclosureRequired
    );
    assert_eq!(profile_measurement_count(&engine), 0);

    let premature_sharing = engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            summary_sharing_enabled: Some(true),
            ..ProfileSettingsUpdate::default()
        })
        .unwrap_err();
    assert!(premature_sharing.to_string().contains("acknowledge"));

    let settings = engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            acknowledge_disclosure: true,
            summary_sharing_enabled: Some(true),
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    assert!(!settings.disclosure_required);
    assert!(settings.summary_sharing_enabled);

    let recorded = engine.record_profile_measurement(input.clone()).unwrap();
    assert_eq!(recorded.status, ProfileMeasurementStatus::Recorded);
    assert_eq!(profile_measurement_count(&engine), 1);

    let payload: String = engine
        .conn()
        .query_row(
            "SELECT payload_json FROM behavior_events WHERE type='profile_measurement'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(payload["instrument_id"], "gse");
    assert_eq!(payload["item_id"], "gse_01");
    assert_eq!(payload["response"], 4);
    assert_eq!(payload["effect"], "recorded_only");

    let invalid_response = engine.record_profile_measurement(ProfileMeasurementInput {
        response: 5,
        ..input.clone()
    });
    assert!(invalid_response
        .unwrap_err()
        .to_string()
        .contains("profile.measurement.response"));
    assert_eq!(profile_measurement_count(&engine), 1);

    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            paused_until: Some("2099-01-01T00:00:00Z".to_owned()),
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    let paused = engine.record_profile_measurement(input.clone()).unwrap();
    assert_eq!(paused.status, ProfileMeasurementStatus::Paused);
    assert_eq!(profile_measurement_count(&engine), 1);
    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            clear_pause: true,
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();

    let disabled_settings = engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            enabled: Some(false),
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    assert!(!disabled_settings.summary_sharing_enabled);
    assert!(engine.global_profile_integration_summary().is_err());
    let disabled = engine.record_profile_measurement(input).unwrap();
    assert_eq!(disabled.status, ProfileMeasurementStatus::Disabled);
    assert_eq!(profile_measurement_count(&engine), 1);

    let mut mastery = MasteryState::initial(0.2);
    fold_attempt(
        &mut mastery,
        &AttemptObservation::new("a1", "recall", 1.0, 4, 0.0),
        &MasteryParams::defaults(),
    );
    assert_eq!(
        mastery.attempt_count, 1,
        "profile opt-out must not block mastery"
    );
}

#[test]
fn disabled_profile_persists_across_restart_without_recording_answers() {
    let db_path = temp_db_path("disabled-restart");
    cleanup_path(&db_path);
    {
        let engine = Engine::new(open_database(&db_path).unwrap());
        engine
            .update_global_profile_settings(ProfileSettingsUpdate {
                enabled: Some(false),
                acknowledge_disclosure: true,
                ..ProfileSettingsUpdate::default()
            })
            .unwrap();
    }

    let engine = Engine::new(open_database(&db_path).unwrap());
    let settings = engine.global_profile_settings().unwrap();
    assert!(!settings.enabled);
    assert!(!settings.disclosure_required);
    let outcome = engine
        .record_profile_measurement(measurement("restart", "gse", "gse_01", 4))
        .unwrap();
    assert_eq!(outcome.status, ProfileMeasurementStatus::Disabled);
    assert_eq!(profile_measurement_count(&engine), 0);
    drop(engine);
    cleanup_path(&db_path);
}

#[test]
fn local_export_is_complete_while_integration_summary_is_opt_in_and_raw_free() {
    let engine = engine_with_profile_data();

    let blocked = engine.global_profile_integration_summary().unwrap_err();
    assert!(blocked.to_string().contains("summary_sharing_enabled"));

    let export = engine.export_global_profile().unwrap();
    assert_eq!(export.measurements.len(), 1);
    assert_eq!(export.dimensions.len(), 1);
    assert_eq!(export.validation_runs.len(), 1);
    assert_eq!(export.measurements[0].payload["response"], 4);
    let overview = engine.global_profile_overview().unwrap();
    assert_eq!(overview.measurement_count, 1);
    assert!(serde_json::to_value(overview)
        .unwrap()
        .get("measurements")
        .is_none());

    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            summary_sharing_enabled: Some(true),
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    let summary = engine.global_profile_integration_summary().unwrap();
    assert_eq!(summary.dimensions.len(), 1);
    let json = serde_json::to_value(summary).unwrap();
    assert!(json.get("measurements").is_none());
    assert!(!json.to_string().contains("\"response\":4"));
}

#[test]
fn derived_profile_metadata_cannot_smuggle_raw_answers_into_read_models() {
    let engine = empty_engine();

    let raw_provenance = engine
        .store_profile_dimension(ProfileDimensionInput {
            scope: ProfileScope::Global,
            scope_id: None,
            dimension_key: "self_efficacy".to_owned(),
            mean: 0.5,
            variance: 0.1,
            evidence_count: 1,
            model_version: "test".to_owned(),
            gate_status: ProfileGateStatus::Shadow,
            provenance: json!({"source": {"raw_response": "private answer"}}),
            evidence_ids: vec!["event-1".to_owned()],
        })
        .unwrap_err();
    assert!(raw_provenance.to_string().contains("raw answer field"));

    let non_id = engine
        .record_profile_validation_run(ProfileValidationRunInput {
            id: "validation-raw-id".to_owned(),
            scope: ProfileScope::Global,
            scope_id: None,
            dimension_key: "self_efficacy".to_owned(),
            model_version: "test".to_owned(),
            status: ProfileGateStatus::Shadow,
            metrics: json!({"folds": 1}),
            provenance: json!({"runner": "test"}),
            evidence_ids: vec!["not an id or raw answer".to_owned()],
        })
        .unwrap_err();
    assert!(non_id.to_string().contains("invalid evidence id"));
    assert_eq!(table_count(engine.conn(), "profile_dimensions"), 0);
    assert_eq!(table_count(engine.conn(), "profile_validation_runs"), 0);
}

#[test]
fn profile_only_reset_is_transactional_and_preserves_learning_attempts() {
    let engine = engine_with_profile_data();
    seed_attempt(engine.conn());

    let receipt = engine.reset_global_profile().unwrap();

    assert_eq!(receipt.measurements_deleted, 1);
    assert_eq!(receipt.dimensions_deleted, 1);
    assert_eq!(receipt.validation_runs_deleted, 1);
    assert_eq!(table_count(engine.conn(), "attempts"), 1);
    assert_eq!(profile_measurement_count(&engine), 0);
    assert_eq!(table_count(engine.conn(), "profile_dimensions"), 0);
    assert_eq!(table_count(engine.conn(), "profile_validation_runs"), 0);
    assert_eq!(table_count(engine.conn(), "profile_data_actions"), 1);
    assert!(engine.global_profile_settings().unwrap().enabled);
}

#[test]
fn profile_reset_failure_rolls_back_every_profile_delete() {
    let engine = engine_with_profile_data();
    engine
        .conn()
        .execute_batch(
            "CREATE TRIGGER reject_profile_dimension_delete
             BEFORE DELETE ON profile_dimensions
             BEGIN SELECT RAISE(ABORT, 'test reset failure'); END;",
        )
        .unwrap();

    let error = engine.reset_global_profile().unwrap_err();

    assert!(error.to_string().contains("test reset failure"));
    assert_eq!(profile_measurement_count(&engine), 1);
    assert_eq!(table_count(engine.conn(), "profile_dimensions"), 1);
    assert_eq!(table_count(engine.conn(), "profile_validation_runs"), 1);
    assert_eq!(table_count(engine.conn(), "profile_data_actions"), 0);
}

#[test]
fn delete_all_requires_confirmation_backs_up_and_removes_database_sidecars() {
    let db_path = temp_db_path("delete-all");
    let backup_path = temp_db_path("delete-all-backup");
    seed_file_database(&db_path);

    let wrong = delete_all_learning_data(
        FullDataDeletionRequest {
            database_path: db_path.clone(),
            backup_path: None,
            confirmation: "yes".to_owned(),
        },
        || Ok(0),
    )
    .unwrap_err();
    assert!(wrong.to_string().contains(DELETE_ALL_CONFIRMATION));
    assert!(db_path.exists());

    let colliding_backup = sqlite_sidecar(&db_path, "-wal");
    let collision = delete_all_learning_data(
        FullDataDeletionRequest {
            database_path: db_path.clone(),
            backup_path: Some(colliding_backup.clone()),
            confirmation: DELETE_ALL_CONFIRMATION.to_owned(),
        },
        || Ok(0),
    )
    .unwrap_err();
    assert!(collision.to_string().contains("SQLite database family"));
    assert!(!colliding_backup.exists());
    let preserved = Connection::open(&db_path).unwrap();
    assert_eq!(table_count(&preserved, "attempts"), 1);
    drop(preserved);

    write_fake_sidecars(&db_path);

    let receipt = delete_all_learning_data(
        FullDataDeletionRequest {
            database_path: db_path.clone(),
            backup_path: Some(backup_path.clone()),
            confirmation: DELETE_ALL_CONFIRMATION.to_owned(),
        },
        || Ok(2),
    )
    .unwrap();

    assert_eq!(receipt.files_deleted, 3);
    assert_eq!(receipt.local_secrets_deleted, 2);
    assert!(receipt.empty_database_created);
    assert!(db_path.exists());
    assert!(!sqlite_sidecar(&db_path, "-wal").exists());
    assert!(!sqlite_sidecar(&db_path, "-shm").exists());
    let empty = Connection::open(&db_path).unwrap();
    assert_eq!(table_count(&empty, "attempts"), 0);
    drop(empty);
    assert!(backup_path.exists());
    let backup = Connection::open(&backup_path).unwrap();
    assert_eq!(table_count(&backup, "attempts"), 1);
    drop(backup);
    cleanup_path(&db_path);
    cleanup_path(&backup_path);
}

#[test]
fn delete_all_restores_database_and_sidecars_when_secret_cleanup_fails() {
    let db_path = temp_db_path("delete-rollback");
    seed_file_database(&db_path);
    write_fake_sidecars(&db_path);

    let error = delete_all_learning_data(
        FullDataDeletionRequest {
            database_path: db_path.clone(),
            backup_path: None,
            confirmation: DELETE_ALL_CONFIRMATION.to_owned(),
        },
        || {
            Err(polaris_core::error::PolarisError::InvalidParameter {
                key: "profile.delete_all.local_secrets".to_owned(),
                value: "simulated failure".to_owned(),
            })
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("simulated failure"));
    assert!(db_path.exists());
    assert!(sqlite_sidecar(&db_path, "-wal").exists());
    assert!(sqlite_sidecar(&db_path, "-shm").exists());
    let conn = Connection::open(&db_path).unwrap();
    assert_eq!(table_count(&conn, "attempts"), 1);
    drop(conn);
    cleanup_path(&db_path);
}

fn empty_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn engine_with_profile_data() -> Engine {
    let engine = empty_engine();
    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            acknowledge_disclosure: true,
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    engine
        .record_profile_measurement(measurement("s1", "gse", "gse_01", 4))
        .unwrap();
    engine
        .store_profile_dimension(ProfileDimensionInput {
            scope: ProfileScope::Global,
            scope_id: None,
            dimension_key: "self_efficacy".to_owned(),
            mean: 0.72,
            variance: 0.04,
            evidence_count: 10,
            model_version: "profile-v1-shadow".to_owned(),
            gate_status: ProfileGateStatus::Shadow,
            provenance: json!({"instrument": "gse", "version": "1.0"}),
            evidence_ids: vec!["measurement-event".to_owned()],
        })
        .unwrap();
    engine
        .record_profile_validation_run(ProfileValidationRunInput {
            id: "validation-1".to_owned(),
            scope: ProfileScope::Global,
            scope_id: None,
            dimension_key: "self_efficacy".to_owned(),
            model_version: "profile-v1-shadow".to_owned(),
            status: ProfileGateStatus::Shadow,
            metrics: json!({"folds": 0, "reason": "insufficient_data"}),
            provenance: json!({"runner": "p16d-test"}),
            evidence_ids: vec![],
        })
        .unwrap();
    engine
}

fn measurement(
    session: &str,
    instrument: &str,
    item: &str,
    response: i64,
) -> ProfileMeasurementInput {
    ProfileMeasurementInput {
        session_id: session.to_owned(),
        instrument_id: instrument.to_owned(),
        instrument_version: "1.0".to_owned(),
        item_id: item.to_owned(),
        locale: "en".to_owned(),
        admin_mode: "ema_single_item".to_owned(),
        response,
    }
}

fn profile_measurement_count(engine: &Engine) -> i64 {
    engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type='profile_measurement'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn seed_attempt(conn: &Connection) {
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                              provisional_score, final_score, created_at)
         VALUES ('keep-attempt', 's1', 'ownership', 'recall', 4, 0.8, 0.8,
                 '2026-08-08T00:00:00Z')",
        [],
    )
    .unwrap();
}

fn seed_file_database(path: &Path) {
    cleanup_path(path);
    let conn = open_database(path).unwrap();
    seed_attempt(&conn);
    drop(conn);
}

fn write_fake_sidecars(path: &Path) {
    std::fs::write(sqlite_sidecar(path, "-wal"), b"wal").unwrap();
    std::fs::write(sqlite_sidecar(path, "-shm"), b"shm").unwrap();
}

fn table_count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn sqlite_sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn temp_db_path(prefix: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("polaris-p16d-{prefix}-{suffix}.sqlite"))
}

fn cleanup_path(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(sqlite_sidecar(path, "-wal"));
    let _ = std::fs::remove_file(sqlite_sidecar(path, "-shm"));
}
