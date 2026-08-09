use std::time::{Duration, Instant};

use polaris_core::db::{open_database, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_desktop_lib::state::{notification_receipt, DesktopState};
use tempfile::tempdir;

fn workspace_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join(path)
}

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
fn today_always_offers_three_legal_fallbacks_without_a_pack() {
    let dir = tempdir().unwrap();
    let state = DesktopState::open(dir.path().join("today-empty.sqlite")).unwrap();

    let today = state.today().unwrap();

    assert_eq!(today.actions.len(), 3);
    assert_eq!(
        today
            .actions
            .iter()
            .map(|action| action.kind.as_str())
            .collect::<Vec<_>>(),
        ["evidence", "inbox", "rest"]
    );
    assert!(today.actions.iter().all(|action| !action.title.is_empty()));
}

#[test]
fn today_uses_scheduler_and_pack_switch_is_immediate() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("today-packs.sqlite");
    let connection = open_database(&database).unwrap();
    let mut engine = Engine::new(connection);
    engine.init_pack(workspace_path("packs/rust")).unwrap();
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    drop(engine);
    let state = DesktopState::open(&database).unwrap();

    let receipt = state.switch_pack("algorithms", Some("isolated")).unwrap();
    let today = state.today().unwrap();

    assert_eq!(receipt.active_pack, "algorithms");
    assert_eq!(receipt.theta_mode, "isolated");
    assert_eq!(today.current_pack.as_deref(), Some("algorithms"));
    assert_eq!(today.theta_mode.as_deref(), Some("isolated"));
    assert_eq!(today.actions.len(), 3);
    assert!(today.actions.iter().any(|action| action.kind == "practice"));
}

#[test]
fn flow_gate_suppresses_info_but_never_errors() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("flow.sqlite");
    let connection = open_database(&database).unwrap();
    connection
        .execute(
            "INSERT INTO behavior_events(id, at, type, payload_json)
             VALUES('flow-1', '2026-08-09T09:00:00Z', 'mental_state', ?1)",
            [r#"{"strategy_enabled":true,"dominant_state":"flow"}"#],
        )
        .unwrap();
    drop(connection);
    let state = DesktopState::open(&database).unwrap();
    let policy = state.notification_policy().unwrap();

    let info = notification_receipt("info", &policy).unwrap();
    let error = notification_receipt("error", &policy).unwrap();

    assert!(policy.suppress_non_error);
    assert!(!info.emitted);
    assert!(info.suppressed_by_flow);
    assert!(error.emitted);
    assert!(!error.suppressed_by_flow);
}

#[test]
fn tier_zero_startup_and_today_reads_stay_inside_budget() {
    let dir = tempdir().unwrap();
    let mut cold_starts = Vec::new();
    for index in 0..20 {
        let started = Instant::now();
        DesktopState::open(dir.path().join(format!("startup-{index}.sqlite"))).unwrap();
        cold_starts.push(started.elapsed());
    }
    cold_starts.sort_unstable();
    let cold_start_p95 = cold_starts[18];
    let state = DesktopState::open(dir.path().join("today.sqlite")).unwrap();
    let mut reads = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let today = state.today().unwrap();
        reads.push(started.elapsed());
        assert_eq!(today.actions.len(), 3);
    }
    reads.sort_unstable();
    let today_p95 = reads[18];

    eprintln!("cold start p95={cold_start_p95:?}; Today p95={today_p95:?}");
    assert!(
        cold_start_p95 < Duration::from_secs(2),
        "cold start p95: {cold_start_p95:?}"
    );
    assert!(
        today_p95 < Duration::from_millis(250),
        "Today p95: {today_p95:?}"
    );
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
