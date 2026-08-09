use std::path::{Path, PathBuf};

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::profile::{
    ProfileDimensionInput, ProfileGateStatus, ProfileMeasurementInput, ProfileScope,
    ProfileSettingsUpdate,
};
use polaris_core::profile_estimation::{
    ProfileEmaStatus, ProfileValidationFold, ProfileValidationInput,
};
use rusqlite::{params, Connection};
use serde_json::json;

#[test]
fn ema_only_follows_completed_sessions_and_respects_daily_weekly_skip_and_rotation() {
    let engine = ready_engine();
    seed_session(&engine, "open", false, "2026-08-01T08:00:00Z");
    assert_eq!(
        engine
            .offer_profile_ema_at("open", "2026-08-01T09:00:00Z")
            .unwrap()
            .status,
        ProfileEmaStatus::SessionNotClosed
    );

    for (session, day) in [("s1", 1), ("s2", 2), ("s3", 3), ("s4", 4)] {
        seed_session(
            &engine,
            session,
            true,
            &format!("2026-08-{day:02}T08:00:00Z"),
        );
    }
    let first = engine
        .offer_profile_ema_at("s1", "2026-08-01T09:00:00Z")
        .unwrap();
    assert_eq!(first.status, ProfileEmaStatus::Offered);
    assert_eq!(
        engine
            .offer_profile_ema_at("s1", "2026-08-02T07:00:00Z")
            .unwrap()
            .status,
        ProfileEmaStatus::AlreadyOffered
    );
    let same_day = engine
        .offer_profile_ema_at("s2", "2026-08-01T10:00:00Z")
        .unwrap();
    assert_eq!(same_day.status, ProfileEmaStatus::DailyLimit);
    let second = engine
        .offer_profile_ema_at("s2", "2026-08-02T09:00:00Z")
        .unwrap();
    let third = engine
        .offer_profile_ema_at("s3", "2026-08-03T09:00:00Z")
        .unwrap();
    assert_eq!(second.status, ProfileEmaStatus::Offered);
    assert_eq!(third.status, ProfileEmaStatus::Offered);
    assert_ne!(
        first.prompt.unwrap().item_id,
        second.prompt.unwrap().item_id
    );
    assert_eq!(
        engine
            .offer_profile_ema_at("s4", "2026-08-04T09:00:00Z")
            .unwrap()
            .status,
        ProfileEmaStatus::WeeklyLimit
    );

    engine
        .record_profile_ema_skip_at("s1", "2026-08-01T09:01:00Z")
        .unwrap();
    let decisions: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type='profile_ema_decision'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(decisions, 1);
}

#[test]
fn ema_pause_flow_suppression_and_close_trigger_are_enforced() {
    let engine = ready_engine();
    seed_session(&engine, "paused", true, "2026-08-01T08:00:00Z");
    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            paused_until: Some("2026-09-01T00:00:00Z".to_owned()),
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    assert_eq!(
        engine
            .offer_profile_ema_at("paused", "2026-08-01T09:00:00Z")
            .unwrap()
            .status,
        ProfileEmaStatus::Paused
    );
    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            clear_pause: true,
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();

    seed_session(&engine, "flow", true, "2026-08-02T08:00:00Z");
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, payload_json)
             VALUES ('flow-state', 'flow', '2026-08-02T08:30:00Z', 'mental_state', ?1)",
            [json!({"posterior": [0.9, 0.05, 0.03, 0.02]}).to_string()],
        )
        .unwrap();
    assert_eq!(
        engine
            .offer_profile_ema_at("flow", "2026-08-02T09:00:00Z")
            .unwrap()
            .status,
        ProfileEmaStatus::FlowSuppressed
    );

    let close_engine = template_engine();
    close_engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            acknowledge_disclosure: true,
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    close_engine
        .conn()
        .execute(
            "INSERT INTO sessions(id, started_at, context_json) VALUES ('close-me', '2026-08-03T08:00:00Z', '{}')",
            [],
        )
        .unwrap();
    close_engine.close_session("close-me").unwrap();
    let offers: i64 = close_engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE session_id='close-me' AND type='profile_ema_offer'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(offers, 1);
}

#[test]
fn monthly_update_reverse_scores_marks_partial_and_never_calls_ema_normative() {
    let engine = ready_engine();
    record_measurement(
        &engine,
        "ipip_learning_facets",
        "ipip_o5_01",
        "full_scale",
        5,
    );
    record_measurement(
        &engine,
        "ipip_learning_facets",
        "ipip_o5_02",
        "ema_single_item",
        1,
    );

    let update = engine
        .run_monthly_profile_update_at("2030-04-30T23:59:59Z")
        .unwrap();
    assert_eq!(update.status, "updated");
    let intellect = update
        .dimensions
        .iter()
        .find(|dimension| dimension.dimension_key == "intellect")
        .unwrap();
    assert_eq!(intellect.evidence_count, 2);
    assert!((intellect.mean - 0.75).abs() < 1e-12);
    assert_eq!(intellect.gate_status, ProfileGateStatus::Shadow);
    assert_eq!(intellect.provenance["partial_instrument"], true);
    assert_eq!(intellect.provenance["complete_full_scale"], false);
    assert_eq!(intellect.provenance["ema_is_not_normative"], true);
    assert_eq!(intellect.updated_at, "2030-04-30T23:59:59Z");
    assert_eq!(
        engine
            .run_monthly_profile_update_at("2030-04-01T00:00:00Z")
            .unwrap()
            .status,
        "already_current"
    );
}

#[test]
fn complete_full_scale_is_distinguished_from_partial_administration() {
    let engine = ready_engine();
    for index in 1..=10 {
        record_measurement(&engine, "gse", &format!("gse_{index:02}"), "full_scale", 4);
    }
    let update = engine
        .run_monthly_profile_update_at("2031-01-31T00:00:00Z")
        .unwrap();
    let gse = update
        .dimensions
        .iter()
        .find(|dimension| dimension.dimension_key == "self_efficacy")
        .unwrap();
    assert_eq!(gse.provenance["partial_instrument"], false);
    assert_eq!(gse.provenance["complete_full_scale"], true);
}

#[test]
fn validation_gate_covers_insufficient_failure_pass_and_drift() {
    let engine = ready_engine();
    seed_shadow_dimension(&engine);

    let insufficient = engine
        .evaluate_profile_gate(validation("insufficient", 1, 5, 2, 0, good_folds()))
        .unwrap();
    assert_eq!(insufficient.status, ProfileGateStatus::Unfit);
    assert!(!insufficient.sample_ready);

    let failed = engine
        .evaluate_profile_gate(validation("failed", 12, 150, 30, 3, weak_folds()))
        .unwrap();
    assert_eq!(failed.status, ProfileGateStatus::Shadow);

    let passed = engine
        .evaluate_profile_gate(validation("passed", 12, 150, 30, 3, good_folds()))
        .unwrap();
    assert_eq!(passed.status, ProfileGateStatus::Active);
    assert!(passed.cross_domain_ready);
    assert!(passed.improvement_probability.unwrap() >= 0.95);

    let drifted = engine
        .evaluate_profile_gate(validation("drifted", 12, 150, 30, 3, weak_folds()))
        .unwrap();
    assert_eq!(drifted.status, ProfileGateStatus::Suspended);
}

#[test]
fn cross_domain_gate_and_behavior_aggregation_are_deterministic() {
    let first = ready_engine();
    let second = ready_engine();
    for engine in [&first, &second] {
        seed_shadow_dimension(engine);
        seed_session(engine, "s1", true, "2026-08-01T08:00:00Z");
        engine
            .conn()
            .execute(
                "INSERT INTO behavior_events(id, session_id, at, type, payload_json)
                 VALUES ('h1', 's1', '2026-08-01T08:10:00Z', 'hint', '{}'),
                        ('a1', 's1', '2026-08-01T08:20:00Z', 'abandon', '{}')",
                [],
            )
            .unwrap();
    }
    assert_eq!(
        first.profile_behavior_snapshot().unwrap(),
        second.profile_behavior_snapshot().unwrap()
    );
    assert_eq!(
        first
            .evaluate_profile_gate(validation("cross", 12, 150, 30, 2, good_folds()))
            .unwrap()
            .status,
        ProfileGateStatus::Shadow
    );
    let first_eval = first
        .evaluate_profile_gate(validation("replay-1", 12, 150, 30, 3, good_folds()))
        .unwrap();
    let second_eval = second
        .evaluate_profile_gate(validation("replay-2", 12, 150, 30, 3, good_folds()))
        .unwrap();
    assert_eq!(first_eval, second_eval);
}

#[test]
fn shadow_profile_does_not_change_task_sequence_or_mastery() {
    let baseline = template_engine();
    let candidate = template_engine();
    for engine in [&baseline, &candidate] {
        engine
            .conn()
            .execute("UPDATE meta SET value='0' WHERE key='mrt.epsilon'", [])
            .unwrap();
    }
    seed_shadow_dimension(&candidate);
    candidate
        .evaluate_profile_gate(validation("shadow", 1, 5, 2, 0, good_folds()))
        .unwrap();

    let baseline_task = baseline.next_task().unwrap().unwrap();
    let candidate_task = candidate.next_task().unwrap().unwrap();
    assert_eq!(baseline_task.concept_id, candidate_task.concept_id);
    assert_eq!(baseline_task.move_id, candidate_task.move_id);
    assert_eq!(baseline_task.task_type, candidate_task.task_type);
    assert_eq!(baseline_task.prompt_text, candidate_task.prompt_text);
    assert_eq!(mastery_rows(&baseline), mastery_rows(&candidate));
}

fn empty_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn ready_engine() -> Engine {
    let engine = empty_engine();
    engine
        .update_global_profile_settings(ProfileSettingsUpdate {
            acknowledge_disclosure: true,
            ..ProfileSettingsUpdate::default()
        })
        .unwrap();
    engine
}

fn template_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path("packs/template")).unwrap();
    engine
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn seed_session(engine: &Engine, id: &str, closed: bool, at: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO sessions(id, started_at, ended_at, closed_at, context_json)
             VALUES (?1, ?2, ?3, ?3, '{}')",
            params![id, at, closed.then_some(at)],
        )
        .unwrap();
    if closed {
        engine
            .conn()
            .execute(
                "INSERT INTO session_summaries(
                     session_id, concepts_touched_json, attempts_count,
                     assertions_json, generated_at
                 ) VALUES (?1, '[]', 0, '[]', ?2)",
                params![id, at],
            )
            .unwrap();
    }
}

fn record_measurement(
    engine: &Engine,
    instrument: &str,
    item: &str,
    admin_mode: &str,
    response: i64,
) {
    engine
        .record_profile_measurement(ProfileMeasurementInput {
            session_id: "measurement-session".to_owned(),
            instrument_id: instrument.to_owned(),
            instrument_version: "1.0".to_owned(),
            item_id: item.to_owned(),
            locale: "en".to_owned(),
            admin_mode: admin_mode.to_owned(),
            response,
        })
        .unwrap();
}

fn seed_shadow_dimension(engine: &Engine) {
    engine
        .store_profile_dimension(ProfileDimensionInput {
            scope: ProfileScope::Global,
            scope_id: None,
            dimension_key: "self_efficacy".to_owned(),
            mean: 0.5,
            variance: 0.1,
            evidence_count: 1,
            model_version: "profile-estimation-v1".to_owned(),
            gate_status: ProfileGateStatus::Shadow,
            provenance: json!({"method": "test_fixture"}),
            evidence_ids: Vec::new(),
        })
        .unwrap();
}

fn validation(
    id: &str,
    weeks: i64,
    outcomes: i64,
    sessions: i64,
    packs: i64,
    folds: Vec<ProfileValidationFold>,
) -> ProfileValidationInput {
    ProfileValidationInput {
        id: id.to_owned(),
        scope: ProfileScope::Global,
        scope_id: None,
        dimension_key: "self_efficacy".to_owned(),
        observed_weeks: weeks,
        outcome_count: outcomes,
        valid_session_count: sessions,
        cross_domain_pack_count: packs,
        folds,
    }
}

fn good_folds() -> Vec<ProfileValidationFold> {
    (0..5)
        .map(|_| ProfileValidationFold {
            baseline_logloss: 0.50,
            candidate_logloss: 0.48,
            baseline_brier: 0.20,
            candidate_brier: 0.19,
        })
        .collect()
}

fn weak_folds() -> Vec<ProfileValidationFold> {
    (0..5)
        .map(|_| ProfileValidationFold {
            baseline_logloss: 0.50,
            candidate_logloss: 0.495,
            baseline_brier: 0.20,
            candidate_brier: 0.21,
        })
        .collect()
}

fn mastery_rows(engine: &Engine) -> Vec<(String, f64, i64, String)> {
    engine
        .conn()
        .prepare("SELECT concept_id, p_known, attempt_count, phase FROM mastery_states ORDER BY concept_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}
