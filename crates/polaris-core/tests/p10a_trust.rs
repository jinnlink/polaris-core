use polaris_core::db::migrate;
use polaris_core::trust::trust_panel;
use rusqlite::Connection;

fn migrated_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn
}

#[test]
fn trust_panel_reports_empty_state_without_fake_passes() {
    let conn = migrated_conn();

    let panel = trust_panel(&conn).unwrap();

    assert_eq!(panel.gates.len(), 5);
    assert!(panel
        .gates
        .iter()
        .any(|gate| gate.framework == "F5" && gate.gate == "no_data"));
    assert_eq!(panel.governance.breeding_min_n.key, "breeding.min_n");
    assert_eq!(panel.governance.breeding_min_n.default_value, "20");
    assert_eq!(panel.governance.breeding_min_n.current_value, "20");
    assert!(panel.governance.breeding_min_n.is_governance_gate);
    assert!(panel.active_breeding_experiments.is_empty());
    assert!(panel.active_mrt_experiments.is_empty());
}

#[test]
fn trust_panel_excludes_breeding_preregistration_audit_from_active_mrt_and_f1_fit() {
    let conn = migrated_conn();
    conn.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (
            'breed-audit-1',
            datetime('now', '-1 hour'),
            ?1,
            0,
            'short_feedback',
            'breed-1'
         )",
        [r#"{"id":"breed-1","window":"7d","candidate_set":["short_feedback"],"context_hash":"state:flow|phase:active","main_effect_hypothesis":"short feedback improves mastery"}"#],
    )
    .unwrap();

    let panel = trust_panel(&conn).unwrap();

    assert!(panel.active_mrt_experiments.is_empty());
    let f1 = panel
        .gates
        .iter()
        .find(|gate| gate.framework == "F1")
        .unwrap();
    assert_eq!(f1.status, "unfit");
    assert_eq!(f1.gate, "no_data");
}

#[test]
fn trust_panel_marks_teaching_mrt_preregistration_without_effect_samples_as_running() {
    let conn = migrated_conn();
    conn.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (
            'mrt-prereg-1',
            datetime('now', '-1 hour'),
            ?1,
            1,
            'explain_then_quiz',
            'mrt-prereg-1'
         )",
        [r#"{"kind":"preregistration","selected_by":"signature_friction","context_hash":"state:flow|phase:active","window":"7d","main_effect_hypothesis":"short feedback improves mastery"}"#],
    )
    .unwrap();

    let panel = trust_panel(&conn).unwrap();

    assert_eq!(panel.active_mrt_experiments.len(), 1);
    let f1 = panel
        .gates
        .iter()
        .find(|gate| gate.framework == "F1")
        .unwrap();
    assert_eq!(f1.status, "running");
    assert_eq!(f1.gate, "evidence_visible");
    let f3 = panel
        .gates
        .iter()
        .find(|gate| gate.framework == "F3")
        .unwrap();
    assert_eq!(f3.status, "running");
    assert_eq!(f3.gate, "evidence_visible");
}

#[test]
fn trust_panel_surfaces_active_experiments_and_recent_activity() {
    let conn = migrated_conn();

    conn.execute(
        "INSERT INTO moves_effects(move, context_hash, alpha, beta, n)
         VALUES ('explain_then_quiz', 'state:flow|phase:active', 3.0, 2.0, 11)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (
            'mrt-1',
            datetime('now', '-1 hour'),
            ?1,
            1,
            'explain_then_quiz',
            'mrt-prereg-1'
         )",
        [r#"{"kind":"preregistration","context_hash":"state:flow|phase:active","window":"7d","main_effect_hypothesis":"short feedback improves mastery"}"#],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO bred_moves(
            id, candidate_move, incumbent_move, context_hash, task_type, template,
            mechanisms_json, main_effect_hypothesis, prereg_json, status,
            posterior_win_prob, candidate_alpha, candidate_beta, incumbent_alpha, incumbent_beta,
            n_candidate, n_incumbent, created_at, updated_at
         )
         VALUES (
            'breed-1', 'short_feedback', 'long_feedback', 'state:flow|phase:active', 'review', 'template-a',
            '[]', 'short feedback improves mastery', ?1, 'preregistered',
            0.71, 2.0, 1.0, 1.0, 2.0,
            4, 3, datetime('now', '-1 day'), datetime('now', '-1 hour')
         )",
        [r#"{"admit_p":0.82,"retire_p":0.45,"min_n":12}"#],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO gu_rules(id, pattern, concept_ids_json, attempt_ids_json, first_seen, last_seen, count, status, alpha, beta, updated_at)
         VALUES ('rule-1', 'recurring ownership confusion', '[\"ownership\"]', '[\"attempt-1\"]', datetime('now', '-1 day'), datetime('now', '-1 hour'), 3, 'active', 4.0, 1.0, datetime('now', '-1 hour'))",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO hazard_models(id, fitted_at, beta_json, validation_auc, n_train, n_validation)
         VALUES ('hazard-1', datetime('now', '-1 hour'), '{}', 0.72, 30, 10)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO state_gate_evals(id, evaluated_at, baseline_auc, state_auc, margin, passes, n)
         VALUES ('gate-1', datetime('now', '-1 hour'), 0.61, 0.68, 0.05, 1, 25)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO param_tuning_runs(id, ran_at, param, old_value, new_value, metric, delta, status)
         VALUES ('tune-1', datetime('now', '-1 hour'), 'bkt.slip_default', '0.12', '0.10', 'auc', 0.02, 'accepted')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO consolidation_runs(id, ran_at, proposals_json, holdout_delta, status)
         VALUES ('consolidate-1', datetime('now', '-1 hour'), '[]', 0.01, 'applied')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO meta(key, value) VALUES ('current_pack_id', 'rust-core')",
        [],
    )
    .unwrap();

    let panel = trust_panel(&conn).unwrap();

    let f1 = panel
        .gates
        .iter()
        .find(|gate| gate.framework == "F1")
        .unwrap();
    assert_eq!(f1.status, "fitted");
    assert!(f1.metric.as_deref().unwrap().contains("effect_samples=11"));

    let f5 = panel
        .gates
        .iter()
        .find(|gate| gate.framework == "F5")
        .unwrap();
    assert_eq!(f5.status, "running");
    assert!(f5.reason.contains("preregistered=1"));

    assert_eq!(panel.active_breeding_experiments.len(), 1);
    assert_eq!(panel.active_breeding_experiments[0].id, "breed-1");
    assert_eq!(panel.active_breeding_experiments[0].min_n, 12);
    assert_eq!(
        panel.active_breeding_experiments[0].main_effect_hypothesis,
        "short feedback improves mastery"
    );

    assert_eq!(panel.active_mrt_experiments.len(), 1);
    assert_eq!(panel.active_mrt_experiments[0].id, "mrt-1");
    assert_eq!(
        panel.active_mrt_experiments[0].context_hash.as_deref(),
        Some("state:flow|phase:active")
    );
    assert!(panel.recent_activity.param_tuning_runs.count_7d >= 1);
    assert!(panel.recent_activity.nightly_consolidation.count_7d >= 1);
}
