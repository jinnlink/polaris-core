use std::time::{Duration, Instant};

use polaris_core::db::{open_database, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_core::profile::{ProfileDimensionInput, ProfileGateStatus, ProfileScope};
use polaris_desktop_lib::contracts::{
    AiInteractionProfileUpdate, CaptureWorkspaceInput, FullDeleteInput, GoalDimensionInput,
    GoalEditorInput, GoalMilestoneInput, GoalScopeInput, InboxActionInput,
    InboxPracticeSubmitInput, InboxWorkspaceQuery, MapWorkspaceQuery, PracticeSubmitInput,
    ProfileExportInput, ProfileMeasurementSubmitInput, ProfileSettingsUpdateInput,
    ReportFeedbackInput,
};
use polaris_desktop_lib::lifecycle::{save_config, save_pending_jobs, DesktopConfig};
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
fn profile_workspace_keeps_shadow_dimensions_non_authoritative() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("profile-workspace.sqlite");
    let connection = open_database(&database).unwrap();
    let engine = Engine::new(connection);
    engine
        .store_profile_dimension(ProfileDimensionInput {
            scope: ProfileScope::Global,
            scope_id: None,
            dimension_key: "self_efficacy".to_owned(),
            mean: 0.72,
            variance: 0.04,
            evidence_count: 12,
            model_version: "test-v1".to_owned(),
            gate_status: ProfileGateStatus::Shadow,
            provenance: serde_json::json!({"source":"test"}),
            evidence_ids: vec!["evidence-profile-1".to_owned()],
        })
        .unwrap();
    drop(engine);

    let snapshot = DesktopState::open(&database)
        .unwrap()
        .profile_workspace()
        .unwrap();
    let dimension = snapshot
        .dimensions
        .iter()
        .find(|item| item.key == "self_efficacy")
        .unwrap();

    assert_eq!(dimension.gate_status, "shadow");
    assert!(dimension.lower < dimension.mean && dimension.mean < dimension.upper);
    assert!(dimension.will_not_affect.contains("不会参与调度"));
    assert_eq!(dimension.evidence_count, 12);
    assert_eq!(snapshot.actions.len(), 3);
}

#[test]
fn goals_workspace_supports_crud_progress_actions_and_distinct_archive_delete() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("goals-workspace.sqlite");
    let connection = open_database(&database).unwrap();
    let mut engine = Engine::new(connection);
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    drop(engine);
    let state = DesktopState::open(&database).unwrap();
    let mut input = GoalEditorInput {
        id: "desktop-goal".to_owned(),
        title: "掌握算法边界".to_owned(),
        description: Some("用证据验证复杂度和边界条件。".to_owned()),
        status: "active".to_owned(),
        deadline: Some("2026-09-01".to_owned()),
        pace: Some("steady".to_owned()),
        priority: 70,
        scope: GoalScopeInput {
            pack_ids: vec!["algorithms".to_owned()],
            dimension_keys: Vec::new(),
            concept_ids: Vec::new(),
        },
        dimensions: vec![GoalDimensionInput {
            id: "goal-dim".to_owned(),
            dimension_key: "mastery".to_owned(),
            display_name: "平均掌握度".to_owned(),
            metric_type: "mastery_mean".to_owned(),
            target_value: 0.8,
            weight: 1.0,
        }],
        milestones: vec![GoalMilestoneInput {
            id: "goal-milestone".to_owned(),
            title: "达到稳定掌握".to_owned(),
            dimension_key: Some("mastery".to_owned()),
            threshold: Some(0.8),
            manual: false,
        }],
    };
    assert_eq!(state.save_goal(input.clone()).unwrap().effect, "created");
    let workspace = state.goals_workspace(Some("desktop-goal")).unwrap();
    assert_eq!(workspace.goals.len(), 1);
    assert_eq!(workspace.goals[0].dimensions.len(), 1);
    assert!((2..=3).contains(&workspace.actions.len()));
    let connection = open_database(&database).unwrap();
    connection
        .execute(
            "UPDATE goal_dimensions SET current_value=0.4 WHERE id='goal-dim'",
            [],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE goal_milestones SET status='reached', reached_at='2026-08-12T00:00:00Z' WHERE id='goal-milestone'",
            [],
        )
        .unwrap();
    drop(connection);
    input.title = "掌握算法边界（更新）".to_owned();
    assert_eq!(state.save_goal(input.clone()).unwrap().effect, "updated");
    let updated = state.goals_workspace(Some("desktop-goal")).unwrap();
    assert_eq!(updated.goals[0].title, "掌握算法边界（更新）");
    let connection = open_database(&database).unwrap();
    let stored_progress: f64 = connection
        .query_row(
            "SELECT current_value FROM goal_dimensions WHERE id='goal-dim'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let stored_milestone: String = connection
        .query_row(
            "SELECT status FROM goal_milestones WHERE id='goal-milestone'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_progress, 0.4);
    assert_eq!(stored_milestone, "reached");
    drop(connection);
    input.status = "paused".to_owned();
    state.save_goal(input.clone()).unwrap();
    let paused = state.goals_workspace(Some("desktop-goal")).unwrap();
    assert!(paused.actions.is_empty());
    assert_eq!(paused.goals[0].dimensions[0].current_value, 0.4);
    assert_eq!(paused.goals[0].milestones[0].status, "reached");
    assert_eq!(paused.goals[0].overall_progress, 0.5);
    input.status = "active".to_owned();
    state.save_goal(input).unwrap();
    assert_eq!(
        state.refresh_goal("desktop-goal").unwrap().effect,
        "refreshed"
    );
    assert_eq!(
        state.archive_goal("desktop-goal").unwrap().effect,
        "archived"
    );
    assert_eq!(
        state.goals_workspace(Some("desktop-goal")).unwrap().goals[0].status,
        "archived"
    );
    assert_eq!(state.delete_goal("desktop-goal").unwrap().effect, "deleted");
    assert!(state.goals_workspace(None).unwrap().goals.is_empty());
}

#[test]
fn reports_and_trust_expose_evidence_gates_and_record_both_feedback_verdicts() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("governance-workspace.sqlite");
    let state = DesktopState::open(&database).unwrap();
    assert_eq!(state.run_report().unwrap().effect, "generated");
    let report_json = serde_json::json!({
        "schema_version": 1,
        "id": "desktop-report",
        "week": "2099-W01",
        "generated_at": "2099-01-05T00:00:00Z",
        "window_days": 7,
        "assertions": [{
            "id": "assertion-1",
            "kind": "calibration",
            "subject": "concept-1",
            "claim": "把握度高于实际表现。",
            "confidence": 0.8,
            "evidence_ids": ["attempt-1"],
            "stats": {},
            "suggested_action": "继续验证。"
        }],
        "hypotheses": [],
        "suggestions": [],
        "top_signal": {
            "id": "assertion-1",
            "kind": "calibration",
            "subject": "concept-1",
            "claim": "把握度高于实际表现。",
            "confidence": 0.8,
            "evidence_ids": ["attempt-1"],
            "stats": {},
            "suggested_action": "继续验证。"
        },
        "skipped": [],
        "hazard_gate": { "participates": false, "reason": "below gate", "validation_auc": 0.6, "auc_gate": 0.7 },
        "reflection_prompts": ["哪条不符合你的实际？"],
        "narrative": null
    })
    .to_string();
    let connection = open_database(&database).unwrap();
    connection.execute(
        "INSERT INTO mirror_reports(id, week, generated_at, report_json, assertion_count, skipped_count)
         VALUES ('desktop-report', '2099-W01', '2099-01-05T00:00:00Z', ?1, 1, 0)",
        [&report_json],
    ).unwrap();
    drop(connection);
    let workspace = state.reports_workspace().unwrap();
    let report = workspace.report.unwrap();
    assert_eq!(report.top_signal.unwrap().evidence_ids, ["attempt-1"]);
    assert_eq!(report.citation_status, "structured_evidence");
    for verdict in ["accurate", "inaccurate"] {
        let receipt = state
            .report_feedback(ReportFeedbackInput {
                report_id: "desktop-report".to_owned(),
                assertion_id: "assertion-1".to_owned(),
                verdict: verdict.to_owned(),
            })
            .unwrap();
        assert_eq!(receipt.effect, format!("feedback_{verdict}"));
    }
    let trust = state.trust_workspace().unwrap();
    assert_eq!(trust.gates.len(), 5);
    assert_eq!(
        trust
            .gates
            .iter()
            .map(|gate| gate.framework.as_str())
            .collect::<Vec<_>>(),
        ["F1", "F2", "F3", "F4", "F5"]
    );
    assert_eq!(trust.governance.len(), 3);
    let connection = open_database(&database).unwrap();
    let feedback_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type='report_feedback'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(feedback_count, 2);
}

#[test]
fn settings_controls_profile_ai_measurement_export_and_profile_only_reset() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("settings.sqlite");
    let state = DesktopState::open(&database).unwrap();
    let initial = state.settings_workspace().unwrap();
    assert!(initial.profile.enabled);
    assert_eq!(initial.privacy_calls.len(), 3);
    assert!(initial.instruments.iter().all(|instrument| instrument
        .admin_modes
        .contains(&"ema_single_item".to_owned())));
    state
        .update_profile_settings(ProfileSettingsUpdateInput {
            enabled: None,
            acknowledge_disclosure: true,
            summary_sharing_enabled: None,
            paused_until: None,
            clear_pause: false,
        })
        .unwrap();
    state
        .update_ai_profile(AiInteractionProfileUpdate {
            persona: Some("socratic_tutor".to_owned()),
            verbosity: None,
            explanation_depth: None,
            proactivity: None,
            intervention_frequency: None,
            correction_style: None,
            custom_notes: Some("先问边界".to_owned()),
        })
        .unwrap();
    let instrument = initial
        .instruments
        .iter()
        .find(|item| item.id == "gse")
        .unwrap();
    let item = instrument.items.first().unwrap();
    state
        .submit_profile_measurement(ProfileMeasurementSubmitInput {
            session_id: "settings-session".to_owned(),
            instrument_id: instrument.id.clone(),
            instrument_version: instrument.version.clone(),
            item_id: item.id.clone(),
            locale: "en".to_owned(),
            admin_mode: "ema_single_item".to_owned(),
            response: instrument.response_min,
        })
        .unwrap();
    let export_path = dir.path().join("profile.json");
    state
        .export_profile(ProfileExportInput {
            output_path: export_path.display().to_string(),
        })
        .unwrap();
    assert!(export_path.is_file());
    let reset = state.reset_profile().unwrap();
    assert!(reset.message.contains("学习尝试保持不变"));
    let updated = state.settings_workspace().unwrap();
    assert_eq!(updated.profile_measurement_count, 0);
    assert_eq!(updated.ai_profile.persona, "socratic_tutor");
}

#[test]
fn desktop_full_delete_previews_scope_rejects_wrong_confirmation_and_reopens_empty_database() {
    let dir = tempdir().unwrap();
    let database = dir.path().join("delete-all.sqlite");
    let state = DesktopState::open(&database).unwrap();
    state
        .save_goal(GoalEditorInput {
            id: "delete-me".to_owned(),
            title: "待清除目标".to_owned(),
            description: None,
            status: "active".to_owned(),
            deadline: None,
            pace: None,
            priority: 50,
            scope: GoalScopeInput {
                pack_ids: Vec::new(),
                dimension_keys: Vec::new(),
                concept_ids: Vec::new(),
            },
            dimensions: vec![GoalDimensionInput {
                id: "delete-dim".to_owned(),
                dimension_key: "attempts".to_owned(),
                display_name: "回答".to_owned(),
                metric_type: "graded_attempts".to_owned(),
                target_value: 1.0,
                weight: 1.0,
            }],
            milestones: Vec::new(),
        })
        .unwrap();
    let scope = state.full_delete_scope().unwrap();
    assert_eq!(scope.goals, 1);
    let error = state
        .delete_all_data(FullDeleteInput {
            confirmation: "wrong".to_owned(),
            backup_path: None,
        })
        .unwrap_err();
    assert!(error.message.contains("DELETE ALL POLARIS LEARNING DATA"));
    assert_eq!(state.full_delete_scope().unwrap().goals, 1);
    let backup = dir.path().join("explicit-backup.sqlite");
    let receipt = state
        .delete_all_data(FullDeleteInput {
            confirmation: scope.confirmation_phrase,
            backup_path: Some(backup.display().to_string()),
        })
        .unwrap();
    assert!(receipt.empty_database_created);
    assert!(backup.is_file());
    assert_eq!(state.full_delete_scope().unwrap().goals, 0);
    assert!(state.today().is_ok());
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

#[test]
fn bootstrap_quarantines_bad_config_and_keeps_recovery_visible() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("desktop.json"), "{broken").unwrap();
    let state = DesktopState::bootstrap(dir.path()).unwrap();
    let lifecycle = state.lifecycle_snapshot().unwrap();

    assert_eq!(lifecycle.startup_status, "ready");
    assert!(lifecycle.config_warning.unwrap().contains("已隔离"));
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("desktop.corrupt-")
    }));
    state.shutdown(true).unwrap();
}

#[test]
fn bootstrap_recovers_unfinished_job_and_shutdown_releases_database_lock() {
    let dir = tempdir().unwrap();
    save_pending_jobs(dir.path(), &["backup".to_owned()]).unwrap();
    let state = DesktopState::bootstrap(dir.path()).unwrap();
    assert_eq!(
        state
            .lifecycle_snapshot()
            .unwrap()
            .recovered_background_jobs,
        vec!["backup"]
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !state
        .lifecycle_snapshot()
        .unwrap()
        .pending_background_jobs
        .is_empty()
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(state
        .lifecycle_snapshot()
        .unwrap()
        .pending_background_jobs
        .is_empty());
    state.shutdown(true).unwrap();

    let database = dir.path().join("polaris.sqlite");
    let moved = dir.path().join("released.sqlite");
    std::fs::rename(&database, &moved).unwrap();
    std::fs::rename(moved, database).unwrap();
}

#[test]
fn newer_database_stays_untouched_until_user_selects_a_supported_path() {
    let dir = tempdir().unwrap();
    let newer = dir.path().join("newer.sqlite");
    let connection = open_database(&newer).unwrap();
    connection
        .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
        .unwrap();
    drop(connection);
    save_config(
        dir.path(),
        &DesktopConfig {
            database_path: Some(newer.clone()),
            database_path_acknowledged: true,
            startup_enabled: false,
        },
    )
    .unwrap();
    save_pending_jobs(dir.path(), &["grade_queue".to_owned()]).unwrap();
    let state = DesktopState::bootstrap(dir.path()).unwrap();
    assert_eq!(state.lifecycle_snapshot().unwrap().startup_status, "newer");
    assert!(state.status().is_err());
    assert_eq!(
        state.lifecycle_snapshot().unwrap().pending_background_jobs,
        vec!["grade_queue"]
    );

    let supported = dir.path().join("supported.sqlite");
    state
        .select_database_path(supported.to_str().unwrap())
        .unwrap();
    assert!(state.status().is_ok());
    state.shutdown(true).unwrap();
    let version: i64 = rusqlite::Connection::open(newer)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_SCHEMA_VERSION + 1);
}
