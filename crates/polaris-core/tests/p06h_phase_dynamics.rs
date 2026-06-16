use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::phase::Phase;
use polaris_core::phase_dynamics::{
    phase_dynamics_summary, PhaseDynamicsStatus, PhaseDynamicsSummary,
    PhaseDynamicsValidationStatus,
};
use rusqlite::{params, Connection};

#[test]
fn empty_phase_dynamics_returns_no_data_without_faking_probabilities() {
    let conn = test_conn();

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(summary.status, PhaseDynamicsStatus::NoData);
    assert_eq!(summary.transition_count, 0);
    assert_eq!(summary.ignored_event_count, 0);
    assert_eq!(summary.rows.len(), Phase::ALL.len());
    assert!(summary
        .rows
        .iter()
        .all(|row| row.counts.iter().all(|count| *count == 0)));
    assert!(summary.rows.iter().all(|row| row
        .probabilities
        .iter()
        .all(|probability| *probability == 0.0)));
    assert_eq!(
        summary.validation.status,
        PhaseDynamicsValidationStatus::Skipped
    );
}

#[test]
fn non_empty_but_small_sample_is_explicitly_insufficient_and_serializes_stably() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Settling);

    let summary = phase_dynamics_summary(&conn).unwrap();
    let json = serde_json::to_value(&summary).unwrap();

    assert_eq!(summary.status, PhaseDynamicsStatus::InsufficientData);
    assert_eq!(
        summary.validation.status,
        PhaseDynamicsValidationStatus::Skipped
    );
    assert_eq!(json["status"], "insufficient_data");
    assert_eq!(json["validation"]["status"], "skipped");
    assert_eq!(json["rows"][index_of(Phase::Phantom)]["from"], "phantom");
    assert_eq!(
        json["target_expected_steps"][index_of(Phase::Phantom)]["phase"],
        "phantom"
    );
}

#[test]
fn phase_dynamics_counts_probabilities_and_ignored_events_are_deterministic() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Settling);
    insert_transition(&conn, 2, Phase::Phantom, Phase::Settling);
    insert_transition(&conn, 3, Phase::Settling, Phase::Transfer);
    insert_transition(&conn, 4, Phase::Settling, Phase::Generation);
    insert_transition(&conn, 5, Phase::Transfer, Phase::Generation);
    insert_payload(&conn, 6, r#"{"from":"phantom"}"#);
    insert_payload(&conn, 7, r#"{"from":"phantom","to":"learning-style"}"#);

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(summary.status, PhaseDynamicsStatus::ShadowReady);
    assert_eq!(summary.transition_count, 5);
    assert_eq!(summary.ignored_event_count, 2);

    let phantom = row_for(&summary, Phase::Phantom);
    assert_eq!(phantom.counts[index_of(Phase::Settling)], 2);
    assert_close(phantom.probabilities[index_of(Phase::Settling)], 1.0);

    let settling = row_for(&summary, Phase::Settling);
    assert_eq!(settling.counts[index_of(Phase::Transfer)], 1);
    assert_eq!(settling.counts[index_of(Phase::Generation)], 1);
    assert_close(settling.probabilities[index_of(Phase::Transfer)], 0.5);
    assert_close(settling.probabilities[index_of(Phase::Generation)], 0.5);

    let engine_summary = Engine::new(conn).phase_dynamics().unwrap();
    assert_eq!(engine_summary.transition_count, summary.transition_count);
    assert_eq!(
        engine_summary.ignored_event_count,
        summary.ignored_event_count
    );
}

#[test]
fn phase_dynamics_thresholds_are_read_from_parameter_registry_meta() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Settling);
    insert_transition(&conn, 2, Phase::Settling, Phase::Transfer);
    insert_transition(&conn, 3, Phase::Transfer, Phase::Generation);
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.min_shadow_ready_transitions', '10')",
        [],
    )
    .unwrap();

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(summary.status, PhaseDynamicsStatus::InsufficientData);
}

#[test]
fn phase_dynamics_validation_params_are_read_from_meta() {
    let conn = test_conn();
    for step in 1..=6 {
        insert_transition(&conn, step * 2 - 1, Phase::Phantom, Phase::Settling);
        insert_transition(&conn, step * 2, Phase::Settling, Phase::Transfer);
    }
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.min_validation_transitions', '20')",
        [],
    )
    .unwrap();

    let skipped = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(
        skipped.validation.status,
        PhaseDynamicsValidationStatus::Skipped
    );
    assert_eq!(
        skipped.validation.reason.as_deref(),
        Some("insufficient_transitions(12<20)")
    );

    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.min_validation_transitions', '8')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.holdout_frac', '0.50')",
        [],
    )
    .unwrap();

    let computed = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(
        computed.validation.status,
        PhaseDynamicsValidationStatus::Computed
    );
    assert_eq!(computed.validation.train_count, 6);
    assert_eq!(computed.validation.holdout_count, 6);
}

#[test]
fn phase_dynamics_validation_params_clamp_to_registered_bounds() {
    let conn = test_conn();
    for step in 1..=6 {
        insert_transition(&conn, step * 2 - 1, Phase::Phantom, Phase::Settling);
        insert_transition(&conn, step * 2, Phase::Settling, Phase::Transfer);
    }
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.min_shadow_ready_transitions', '5000')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.min_validation_transitions', '5000')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.holdout_frac', '0.99')",
        [],
    )
    .unwrap();

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(summary.status, PhaseDynamicsStatus::InsufficientData);
    assert_eq!(
        summary.validation.reason.as_deref(),
        Some("insufficient_transitions(12<1000)")
    );
}

#[test]
fn expected_steps_to_transfer_or_generation_are_finite_for_reachable_paths() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Settling);
    insert_transition(&conn, 2, Phase::Settling, Phase::Transfer);
    insert_transition(&conn, 3, Phase::Transfer, Phase::Generation);

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_close(expected_steps(&summary, Phase::Phantom).unwrap(), 2.0);
    assert_close(expected_steps(&summary, Phase::Settling).unwrap(), 1.0);
    assert_close(expected_steps(&summary, Phase::Transfer).unwrap(), 0.0);
    assert_close(expected_steps(&summary, Phase::Generation).unwrap(), 0.0);
}

#[test]
fn expected_steps_are_none_when_target_is_reachable_but_not_absorbing() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Transfer);
    insert_transition(&conn, 2, Phase::Phantom, Phase::Regression);
    insert_transition(&conn, 3, Phase::Regression, Phase::Regression);

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(expected_steps(&summary, Phase::Phantom), None);
    assert_eq!(expected_steps(&summary, Phase::Regression), None);
}

#[test]
fn expected_steps_are_none_when_target_phase_is_unreachable() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Fluctuation);
    insert_transition(&conn, 2, Phase::Fluctuation, Phase::Phantom);

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(expected_steps(&summary, Phase::Phantom), None);
    assert_eq!(expected_steps(&summary, Phase::Fluctuation), None);
}

#[test]
fn holdout_validation_does_not_count_unseen_from_rows_as_markov_hits() {
    let conn = test_conn();
    insert_transition(&conn, 1, Phase::Phantom, Phase::Settling);
    insert_transition(&conn, 2, Phase::Phantom, Phase::Settling);
    insert_transition(&conn, 3, Phase::Settling, Phase::Transfer);
    insert_transition(&conn, 4, Phase::Settling, Phase::Transfer);
    insert_transition(&conn, 5, Phase::Fluctuation, Phase::Fluctuation);
    insert_transition(&conn, 6, Phase::Fluctuation, Phase::Fluctuation);
    insert_transition(&conn, 7, Phase::Regression, Phase::Regression);
    insert_transition(&conn, 8, Phase::Regression, Phase::Regression);
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value)
         VALUES ('phase_dynamics.holdout_frac', '0.25')",
        [],
    )
    .unwrap();

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(
        summary.validation.status,
        PhaseDynamicsValidationStatus::Computed
    );
    assert_eq!(summary.validation.train_count, 6);
    assert_eq!(summary.validation.holdout_count, 2);
    assert_close(summary.validation.static_accuracy.unwrap(), 1.0);
    assert_close(summary.validation.markov_accuracy.unwrap(), 0.0);
    assert!(summary.validation.markov_log_loss.unwrap() > 10.0);
}

#[test]
fn holdout_validation_separates_static_baseline_from_markov_prediction() {
    let conn = test_conn();
    for step in 1..=6 {
        insert_transition(&conn, step * 2 - 1, Phase::Phantom, Phase::Settling);
        insert_transition(&conn, step * 2, Phase::Settling, Phase::Transfer);
    }

    let summary = phase_dynamics_summary(&conn).unwrap();

    assert_eq!(
        summary.validation.status,
        PhaseDynamicsValidationStatus::Computed
    );
    assert!(summary.validation.holdout_count >= 2);
    assert!(
        summary.validation.markov_accuracy.unwrap() >= summary.validation.static_accuracy.unwrap()
    );
    assert!(
        summary.validation.markov_log_loss.unwrap() <= summary.validation.static_log_loss.unwrap()
    );
}

fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn
}

fn insert_transition(conn: &Connection, seq: i64, from: Phase, to: Phase) {
    insert_payload(
        conn,
        seq,
        &serde_json::json!({
            "schema_version": 1,
            "from": from.as_str(),
            "to": to.as_str(),
            "concept_id": "concept-a",
            "attempt_id": format!("attempt-{seq:02}")
        })
        .to_string(),
    );
}

fn insert_payload(conn: &Connection, seq: i64, payload_json: &str) {
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (?1, 'session-a', ?2, 'phase_transition', 'concept-a', ?3)",
        params![
            format!("event-{seq:02}"),
            format!("2026-06-17T00:00:{seq:02}Z"),
            payload_json
        ],
    )
    .unwrap();
}

fn row_for(
    summary: &PhaseDynamicsSummary,
    phase: Phase,
) -> &polaris_core::phase_dynamics::PhaseTransitionRow {
    summary.rows.iter().find(|row| row.from == phase).unwrap()
}

fn expected_steps(summary: &PhaseDynamicsSummary, phase: Phase) -> Option<f64> {
    summary
        .target_expected_steps
        .iter()
        .find(|estimate| estimate.phase == phase)
        .unwrap()
        .expected_steps
}

fn index_of(phase: Phase) -> usize {
    Phase::ALL
        .iter()
        .position(|candidate| *candidate == phase)
        .unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}
