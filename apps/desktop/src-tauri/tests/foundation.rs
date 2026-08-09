use polaris_core::db::{open_database, CURRENT_SCHEMA_VERSION};
use polaris_desktop_lib::state::DesktopState;
use tempfile::tempdir;

#[test]
fn fresh_database_opens_and_returns_status() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("fresh.sqlite");
    let state = DesktopState::open(&database).unwrap();

    let snapshot = state.status().unwrap();

    assert!(database.exists());
    assert!(snapshot.current_pack.is_none());
    assert!(snapshot.concepts.is_empty());
}

#[test]
fn existing_database_opens_without_losing_schema_or_status() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("existing.sqlite");
    let connection = open_database(&database).unwrap();
    connection
        .execute(
            "INSERT INTO meta(key, value) VALUES('desktop.fixture', 'kept')",
            [],
        )
        .unwrap();
    drop(connection);

    let state = DesktopState::open(&database).unwrap();
    let snapshot = state.status().unwrap();
    let connection = open_database(&database).unwrap();
    let marker: String = connection
        .query_row(
            "SELECT value FROM meta WHERE key='desktop.fixture'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();

    assert_eq!(marker, "kept");
    assert_eq!(version, CURRENT_SCHEMA_VERSION);
    assert!(snapshot.current_pack.is_none());
}

#[test]
fn generated_typescript_contract_is_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/contracts/core.ts");
    let committed = std::fs::read_to_string(&path).unwrap();
    let generated = polaris_desktop_lib::contracts::generated_typescript_contracts();

    assert_eq!(
        committed,
        generated,
        "Rust/TypeScript DTO drift: regenerate {}",
        path.display()
    );
}

#[test]
fn csp_capability_and_telemetry_baseline_are_minimal() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap(),
    )
    .unwrap();
    let capability: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(manifest_dir.join("capabilities/main.json")).unwrap(),
    )
    .unwrap();
    let package = std::fs::read_to_string(manifest_dir.join("../package.json")).unwrap();
    let frontend_events =
        std::fs::read_to_string(manifest_dir.join("../src/lib/events.ts")).unwrap();

    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    let script_policy = csp
        .split(';')
        .find(|policy| policy.trim_start().starts_with("script-src"))
        .unwrap();
    assert_eq!(script_policy.trim(), "script-src 'self'");
    assert_eq!(config["app"]["security"]["capabilities"][0], "main");

    let permissions = capability["permissions"].as_array().unwrap();
    assert_eq!(permissions.len(), 3);
    let serialized_permissions = serde_json::to_string(permissions).unwrap();
    for forbidden in ["fs:", "shell:", "sql:", "process:"] {
        assert!(!serialized_permissions.contains(forbidden));
    }
    for telemetry in ["posthog", "sentry", "analytics", "telemetry"] {
        assert!(!package.to_ascii_lowercase().contains(telemetry));
    }
    assert!(frontend_events.contains(polaris_desktop_lib::DATA_CHANGED_EVENT));
}
