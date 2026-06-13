use std::path::Path;

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::phase::Phase;
use polaris_core::simulation::{simulate_learning, VirtualLearner};
use rusqlite::Connection;

#[test]
fn strong_virtual_learner_reaches_transfer_without_deadlock() {
    let mut engine = engine_for_rust_pack();
    let learner = VirtualLearner::strong(32);

    let report = simulate_learning(&learner, 30, &mut engine).unwrap();

    assert_eq!(report.daily_summaries.len(), 30);
    assert!(report.deadlock_days.is_empty(), "{report:#?}");
    assert!(
        report.final_mean_p_known >= 0.70,
        "strong learner should cross mastery floor: {report:#?}"
    );
    assert!(
        report.final_theta_cosine > 0.50,
        "theta should track strong learner ability: {report:#?}"
    );
    assert!(
        report
            .final_phase_counts
            .get(&Phase::Transfer)
            .copied()
            .unwrap_or(0)
            > 0,
        "strong learner should reach transfer: {report:#?}"
    );
    assert!(!report.has_hmm_state_lock(), "{report:#?}");
}

#[test]
fn weak_overconfident_virtual_learner_improves_without_early_transfer() {
    let mut engine = engine_for_rust_pack();
    let learner = VirtualLearner::weak(32);

    let report = simulate_learning(&learner, 30, &mut engine).unwrap();

    assert_eq!(report.daily_summaries.len(), 30);
    assert!(report.deadlock_days.is_empty(), "{report:#?}");
    assert!(
        report.mean_p_known_slope > 0.0,
        "weak learner should improve slowly: {report:#?}"
    );
    assert!(
        report.final_abs_calib_gap < report.initial_abs_calib_gap,
        "overconfidence should start converging: {report:#?}"
    );
    assert!(
        report.early_transfer_violations.is_empty(),
        "weak learner should not transfer before enough evidence: {report:#?}"
    );
    assert!(!report.has_hmm_state_lock(), "{report:#?}");
}

#[test]
fn mixed_virtual_learner_keeps_running_and_reports_daily_summaries() {
    let mut engine = engine_for_rust_pack();
    let learner = VirtualLearner::mixed(32);

    let report = simulate_learning(&learner, 30, &mut engine).unwrap();

    assert_eq!(report.daily_summaries.len(), 30);
    assert!(report.deadlock_days.is_empty(), "{report:#?}");
    assert!(report.final_mean_p_known > report.initial_mean_p_known);
    assert!(
        report
            .daily_summaries
            .iter()
            .all(|summary| summary.active_concepts > 0 && !summary.phase_distribution.is_empty()),
        "daily summaries should expose active concepts and phase distribution: {report:#?}"
    );
    assert!(!report.has_hmm_state_lock(), "{report:#?}");
}

fn engine_for_rust_pack() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn workspace_pack_path(relative: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
