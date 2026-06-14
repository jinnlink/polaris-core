use std::hint::black_box;
use std::time::Instant;

use polaris_core::mastery::{
    fold_all, fold_attempt, AttemptObservation, MasteryParams, MasteryState,
};
use polaris_core::mental_state::{forward_filter, HmmObservation, StatePosterior};
use polaris_core::phase::Phase;
use polaris_core::scheduler::{rank_candidates_with_params, ScheduleCandidate, SchedulerParams};

const SCHEDULER_10K_BUDGET_NS: f64 = 10_000_000.0;
const FOLD_ATTEMPT_BUDGET_NS: f64 = 50_000.0;
const REPLAY_100_ATTEMPTS_BUDGET_NS: f64 = 1_000_000.0;
const HMM_STEP_BUDGET_NS: f64 = 1_000.0;

#[test]
fn p06e_scheduler_ranks_10k_candidates_within_budget() {
    let mut candidate_batches = (0..7)
        .map(|_| schedule_candidates(10_000))
        .collect::<Vec<_>>();
    let params = SchedulerParams::defaults();

    let observed_ns = median_nanos(7, 1, || {
        let candidates = candidate_batches.pop().expect("prepared candidate batch");
        let ranked = rank_candidates_with_params(black_box(candidates), black_box(&params));
        black_box(ranked.len());
    });

    assert_budget(
        "U(c) 10k candidate ranking",
        observed_ns,
        SCHEDULER_10K_BUDGET_NS,
    );
}

#[test]
fn p06e_single_attempt_fold_stays_within_budget() {
    let params = MasteryParams::defaults();
    let attempt = AttemptObservation::new("budget-fold", "recall", 0.82, 4, 1.0);
    let mut state = MasteryState::initial_with_params(0.20, &params);

    let observed_ns = median_nanos(9, 50_000, || {
        fold_attempt(
            black_box(&mut state),
            black_box(&attempt),
            black_box(&params),
        );
        black_box(state.p_known);
    });

    assert_budget("single fold_attempt", observed_ns, FOLD_ATTEMPT_BUDGET_NS);
}

#[test]
fn p06e_replay_100_attempts_stays_within_budget() {
    let params = MasteryParams::defaults();
    let attempts = replay_attempts(100);

    let observed_ns = median_nanos(9, 200, || {
        let state = fold_all(black_box(0.20), black_box(&attempts), black_box(&params));
        black_box(state.p_known);
    });

    assert_budget(
        "fold_all 100 attempts",
        observed_ns,
        REPLAY_100_ATTEMPTS_BUDGET_NS,
    );
}

#[test]
fn p06e_hmm_forward_step_stays_within_budget() {
    let observation = HmmObservation {
        z_latency: 0.5,
        hints: 1.0,
        residual: -0.2,
        consec_fail: 1.0,
        conf_delta: -0.4,
        interval_bucket: 1.0,
        session_min: 18.0,
    };
    let mut posterior = StatePosterior::uniform();

    let observed_ns = median_nanos(9, 100_000, || {
        posterior = forward_filter(Some(black_box(&posterior)), black_box(observation));
        black_box(&posterior);
    });

    assert_budget("HMM forward_filter step", observed_ns, HMM_STEP_BUDGET_NS);
}

fn schedule_candidates(count: usize) -> Vec<ScheduleCandidate> {
    (0..count)
        .map(|index| ScheduleCandidate {
            id: format!("concept-{index:05}"),
            seed_order: index as i64,
            retrieval: if index % 5 == 0 {
                None
            } else {
                Some((index % 100) as f64 / 100.0)
            },
            calib_gap: (index % 8) as f64 / 20.0 - 0.05,
            misconception_active: index % 17 == 0,
            has_attempts: index % 3 != 0,
            prerequisites_met: index % 11 != 0,
            phase: Phase::ALL[index % Phase::ALL.len()],
        })
        .collect()
}

fn replay_attempts(count: usize) -> Vec<AttemptObservation> {
    (0..count)
        .map(|index| {
            let task_type = match index % 4 {
                0 => "recall",
                1 => "free_explain",
                2 => "apply",
                _ => "transfer",
            };
            let score = match index % 5 {
                0 => 0.25,
                1 => 0.55,
                2 => 0.76,
                3 => 0.88,
                _ => 0.95,
            };
            AttemptObservation::new(
                format!("budget-attempt-{index:03}"),
                task_type,
                score,
                (index % 5 + 1) as i32,
                1.0,
            )
            .with_created_at(format!("{index:020}"))
            .with_occurred_day(index as f64)
        })
        .collect()
}

fn median_nanos(mut samples: usize, batch_size: usize, mut measure: impl FnMut()) -> f64 {
    samples = samples.max(1);
    let batch_size = batch_size.max(1);
    let mut durations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        for _ in 0..batch_size {
            measure();
        }
        durations.push(started.elapsed().as_nanos() as f64 / batch_size as f64);
    }
    durations.sort_by(f64::total_cmp);
    durations[durations.len() / 2]
}

fn assert_budget(label: &str, observed_ns: f64, release_budget_ns: f64) {
    let allowed_ns = release_budget_ns * profile_multiplier();
    assert!(
        observed_ns <= allowed_ns,
        "{label} exceeded budget: observed {:.0}ns, allowed {:.0}ns (release budget {:.0}ns)",
        observed_ns,
        allowed_ns,
        release_budget_ns
    );
}

fn profile_multiplier() -> f64 {
    if cfg!(debug_assertions) {
        100.0
    } else {
        1.0
    }
}
