use std::time::{Duration, Instant};

use polaris_core::db::{open_database, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_desktop_lib::contracts::{
    CaptureWorkspaceInput, InboxActionInput, InboxPracticeSubmitInput, InboxWorkspaceQuery,
    MapWorkspaceQuery, PracticeSubmitInput,
};
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

fn map_query(view: &str, pack: Option<&str>) -> MapWorkspaceQuery {
    MapWorkspaceQuery {
        view: view.to_owned(),
        pack: pack.map(str::to_owned),
        root: None,
        depth: None,
        phase: None,
        due: None,
        min_confidence: None,
        limit: 100,
        cursor: None,
    }
}

#[test]
fn map_workspace_preserves_current_prediction_and_global_contracts() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("maps.sqlite");
    let connection = open_database(&database).unwrap();
    let mut engine = Engine::new(connection);
    engine.init_pack(workspace_path("packs/rust")).unwrap();
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    drop(engine);
    let state = DesktopState::open(&database).unwrap();

    let current = state
        .map_workspace(map_query("current", Some("rust")))
        .unwrap();
    assert_eq!(current.view, "current");
    assert!(!current.nodes.is_empty());
    assert!(current.nodes.len() <= 100);
    assert!(current
        .nodes
        .iter()
        .flat_map(|node| &node.layers)
        .all(|layer| !layer.cross_domain));
    let mut neighborhood_query = map_query("current", Some("rust"));
    neighborhood_query.root = Some(current.nodes[0].id.clone());
    neighborhood_query.depth = Some(1);
    let neighborhood = state.map_workspace(neighborhood_query).unwrap();
    assert!(neighborhood
        .nodes
        .iter()
        .any(|node| node.id == current.nodes[0].id));
    let mut due_query = map_query("current", Some("rust"));
    due_query.due = Some("new".to_owned());
    let new_nodes = state.map_workspace(due_query).unwrap();
    assert!(new_nodes.nodes.iter().all(|node| node.due_status == "new"));

    state.switch_pack("rust", Some("isolated")).unwrap();
    let prediction = state
        .map_workspace(map_query("prediction", Some("rust")))
        .unwrap();
    assert_eq!(prediction.theta_mode.as_deref(), Some("isolated"));
    assert!(prediction
        .nodes
        .iter()
        .flat_map(|node| &node.layers)
        .all(|layer| !layer.cross_domain));

    let global = state.map_workspace(map_query("global", None)).unwrap();
    assert_eq!(global.view, "global");
    assert!(global.nodes.is_empty());
    assert!(global.aggregates.iter().any(|item| item.kind == "pack"));
    assert!(global
        .aggregates
        .iter()
        .any(|item| item.kind == "dimension"));
}

#[test]
fn practice_workspace_resumes_draft_and_submits_without_waiting_for_llm() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("practice.sqlite");
    let connection = open_database(&database).unwrap();
    let mut engine = Engine::new(connection);
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    drop(engine);
    let state = DesktopState::open(&database).unwrap();

    let first = state.practice_workspace("desktop-session").unwrap();
    let resumed = state.practice_workspace("desktop-session").unwrap();
    let task = first.task.unwrap();
    let original_task_event_id = task.task_event_id.clone();
    assert_eq!(resumed.task.unwrap().task_event_id, task.task_event_id);
    assert_eq!(first.actions.len(), 3);

    let invalid = state.submit_practice(PracticeSubmitInput {
        session_id: "desktop-session".to_owned(),
        task_event_id: task.task_event_id.clone(),
        response_text: "我的解释".to_owned(),
        self_confidence: 0,
    });
    assert!(invalid.is_err());

    let receipt = state
        .submit_practice(PracticeSubmitInput {
            session_id: "desktop-session".to_owned(),
            task_event_id: task.task_event_id.clone(),
            response_text: "我会从不变量与边界条件解释这个概念。".to_owned(),
            self_confidence: 3,
        })
        .unwrap();
    let status = state.attempt_grade_status(&receipt.attempt_id).unwrap();
    assert!(!status.evidence_id.is_empty());
    assert_eq!(status.final_score, None);
    assert!(status.queued);

    let duplicate = state.submit_practice(PracticeSubmitInput {
        session_id: "desktop-session".to_owned(),
        task_event_id: task.task_event_id,
        response_text: "重复回答".to_owned(),
        self_confidence: 3,
    });
    assert!(duplicate.is_err());
    assert_ne!(
        state
            .practice_workspace("desktop-session")
            .unwrap()
            .task
            .unwrap()
            .task_event_id,
        original_task_event_id
    );
}

#[test]
fn busy_database_rejects_cleanly_and_keeps_the_issued_task_recoverable() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("practice-busy.sqlite");
    let connection = open_database(&database).unwrap();
    let mut engine = Engine::new(connection);
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    let task = engine
        .issue_or_resume_task("busy-session")
        .unwrap()
        .unwrap();
    engine
        .conn()
        .busy_timeout(Duration::from_millis(20))
        .unwrap();

    let blocker = open_database(&database).unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let result = engine.submit_task_response_provisional(
        "busy-session",
        &task.task_event_id,
        "数据库恢复后仍可重新提交。".to_owned(),
        3,
    );

    assert!(result.is_err());
    let attempt_count: i64 = engine
        .conn()
        .query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attempt_count, 0);
    blocker.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        engine
            .issue_or_resume_task("busy-session")
            .unwrap()
            .unwrap()
            .task_event_id,
        task.task_event_id
    );
}

#[test]
fn inbox_capture_stays_recorded_only_until_a_confident_answer_is_submitted() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("inbox.sqlite");
    let connection = open_database(&database).unwrap();
    let mut engine = Engine::new(connection);
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    drop(engine);
    let state = DesktopState::open(&database).unwrap();

    let capture = state
        .capture_workspace(CaptureWorkspaceInput {
            session_id: Some("inbox-session".to_owned()),
            source: "desktop".to_owned(),
            content_type: "text/plain".to_owned(),
            text: "Dijkstra 在负权边上不能保证正确。".to_owned(),
            learner_kind: "reference".to_owned(),
            candidate_concept_ids: vec!["complexity_basics".to_owned()],
            note: None,
        })
        .unwrap();
    assert_eq!(capture.effect, "recorded_only");
    let connection = open_database(&database).unwrap();
    let attempts_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attempts_before, 0);
    drop(connection);

    let open = state
        .inbox_workspace(InboxWorkspaceQuery {
            statuses: Vec::new(),
            limit: 20,
        })
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].actions.len(), 3);
    state
        .act_on_inbox(InboxActionInput {
            capture_id: capture.capture_id.clone(),
            action: "accept".to_owned(),
            note: None,
        })
        .unwrap();
    let draft = state.draft_inbox_practice(&capture.capture_id).unwrap();
    assert_eq!(
        draft.concept_hint.as_deref(),
        Some("Big-O, Omega, and Theta notation")
    );

    let receipt = state
        .submit_inbox_practice(InboxPracticeSubmitInput {
            capture_id: capture.capture_id,
            session_id: "inbox-session".to_owned(),
            response_text: "贪心松弛依赖已确定距离不会再被更短路径推翻，负权边会破坏它。"
                .to_owned(),
            self_confidence: 4,
            latency_ms: 1_200,
            hint_count: 0,
        })
        .unwrap();
    assert_eq!(receipt.status, "practiced");
    assert!(
        state
            .attempt_grade_status(&receipt.attempt_id)
            .unwrap()
            .queued
    );
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
