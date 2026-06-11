use crate::config::{default_registry, ParameterSpec};
use crate::fsrs::{create_initial_state, update_state, FsrsState};

#[derive(Debug, Clone)]
pub struct MasteryParams {
    pub bkt_p_init: f64,
    pub bkt_slip: f64,
    pub bkt_guess: f64,
    pub bkt_guess_explain: f64,
    pub bkt_learn: f64,
    pub bkt_cut_hi: f64,
    pub bkt_cut_lo: f64,
    pub calib_ewma: f64,
}

impl MasteryParams {
    pub fn defaults() -> Self {
        let registry = default_registry();
        Self {
            bkt_p_init: parse_f64(&registry, "bkt.p_init"),
            bkt_slip: parse_f64(&registry, "bkt.slip"),
            bkt_guess: parse_f64(&registry, "bkt.guess"),
            bkt_guess_explain: parse_f64(&registry, "bkt.guess_explain"),
            bkt_learn: parse_f64(&registry, "bkt.learn"),
            bkt_cut_hi: parse_f64(&registry, "bkt.cut_hi"),
            bkt_cut_lo: parse_f64(&registry, "bkt.cut_lo"),
            calib_ewma: parse_f64(&registry, "calib.ewma"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MasteryState {
    pub p_known: f64,
    pub fsrs: FsrsState,
    pub calib_gap: f64,
    pub brier_ewma: f64,
    pub attempt_count: u32,
    pub lapses: u32,
    pub last_depth: Option<String>,
    pub max_depth: Option<String>,
}

impl MasteryState {
    pub fn initial(p_known: f64) -> Self {
        Self {
            p_known,
            fsrs: create_initial_state(),
            calib_gap: 0.0,
            brier_ewma: 0.0,
            attempt_count: 0,
            lapses: 0,
            last_depth: None,
            max_depth: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttemptObservation {
    pub id: String,
    pub task_type: String,
    pub score: f64,
    pub self_confidence: i32,
    pub elapsed_days: f64,
    pub depth: Option<String>,
}

impl AttemptObservation {
    pub fn new(
        id: impl Into<String>,
        task_type: impl Into<String>,
        score: f64,
        self_confidence: i32,
        elapsed_days: f64,
    ) -> Self {
        Self {
            id: id.into(),
            task_type: task_type.into(),
            score,
            self_confidence,
            elapsed_days,
            depth: None,
        }
    }
}

pub fn fold_attempt(
    state: &mut MasteryState,
    attempt: &AttemptObservation,
    params: &MasteryParams,
) {
    let score = attempt.score.clamp(0.0, 1.0);
    update_bkt(state, &attempt.task_type, score, params);
    update_calibration(state, attempt.self_confidence, score, params);

    let fsrs_update = update_state(
        &state.fsrs,
        crate::fsrs::Rating::from_score(score),
        attempt.elapsed_days,
    );
    state.fsrs = fsrs_update.state;
    state.lapses = state.fsrs.lapses;
    state.attempt_count += 1;
    state.last_depth = attempt.depth.clone();
    state.max_depth =
        max_depth(state.max_depth.as_deref(), attempt.depth.as_deref()).map(str::to_owned);
}

pub fn fold_all(
    p_init: f64,
    attempts: &[AttemptObservation],
    params: &MasteryParams,
) -> MasteryState {
    let mut ordered = attempts.to_vec();
    ordered.sort_by(|a, b| {
        a.elapsed_days
            .total_cmp(&b.elapsed_days)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut state = MasteryState::initial(p_init);
    for attempt in &ordered {
        fold_attempt(&mut state, attempt, params);
    }
    state
}

pub fn replay_after_final(
    p_init: f64,
    attempts: &[AttemptObservation],
    attempt_id: &str,
    final_score: f64,
    params: &MasteryParams,
) -> MasteryState {
    let mut corrected = attempts.to_vec();
    for attempt in &mut corrected {
        if attempt.id == attempt_id {
            attempt.score = final_score;
        }
    }
    fold_all(p_init, &corrected, params)
}

fn update_bkt(state: &mut MasteryState, task_type: &str, score: f64, params: &MasteryParams) {
    if score >= params.bkt_cut_hi {
        let guess = if task_type == "free_explain" {
            params.bkt_guess_explain
        } else {
            params.bkt_guess
        };
        let numerator = state.p_known * (1.0 - params.bkt_slip);
        let denominator = numerator + (1.0 - state.p_known) * guess;
        let posterior = numerator / denominator;
        state.p_known = posterior + (1.0 - posterior) * params.bkt_learn;
    } else if score <= params.bkt_cut_lo {
        let numerator = state.p_known * params.bkt_slip;
        let denominator = numerator + (1.0 - state.p_known) * (1.0 - params.bkt_guess);
        state.p_known = numerator / denominator;
    }
}

fn update_calibration(
    state: &mut MasteryState,
    self_confidence: i32,
    score: f64,
    params: &MasteryParams,
) {
    let confidence = ((self_confidence as f64 - 1.0) / 4.0).clamp(0.0, 1.0);
    let gap = confidence - score;
    state.calib_gap = (1.0 - params.calib_ewma) * state.calib_gap + params.calib_ewma * gap;

    if score >= params.bkt_cut_hi || score <= params.bkt_cut_lo {
        let outcome = if score >= params.bkt_cut_hi { 1.0 } else { 0.0 };
        let brier = (confidence - outcome) * (confidence - outcome);
        state.brier_ewma = (1.0 - params.calib_ewma) * state.brier_ewma + params.calib_ewma * brier;
    }
}

fn max_depth(current: Option<&str>, next: Option<&str>) -> Option<&'static str> {
    match (depth_rank(current), depth_rank(next)) {
        (None, None) => None,
        (Some((depth, _)), None) | (None, Some((depth, _))) => Some(depth),
        (Some((left_depth, left_rank)), Some((right_depth, right_rank))) => {
            Some(if right_rank > left_rank {
                right_depth
            } else {
                left_depth
            })
        }
    }
}

fn depth_rank(depth: Option<&str>) -> Option<(&'static str, u8)> {
    match depth {
        Some("recall") => Some(("recall", 0)),
        Some("explain") => Some(("explain", 1)),
        Some("apply") => Some(("apply", 2)),
        Some("transfer") => Some(("transfer", 3)),
        _ => None,
    }
}

fn parse_f64(
    registry: &std::collections::BTreeMap<&'static str, ParameterSpec>,
    key: &'static str,
) -> f64 {
    registry[key]
        .default_value
        .parse()
        .expect("valid default f64 parameter")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual {actual} expected {expected}"
        );
    }

    #[test]
    fn bkt_updates_correct_wrong_dead_zone_and_explain_guess() {
        let params = MasteryParams::defaults();

        let mut state = MasteryState::initial(0.20);
        fold_attempt(
            &mut state,
            &AttemptObservation::new("a1", "recall", 0.80, 3, 0.0),
            &params,
        );
        assert_close(state.p_known, 0.5764705882352941);

        let mut state = MasteryState::initial(0.20);
        fold_attempt(
            &mut state,
            &AttemptObservation::new("a2", "recall", 0.30, 3, 0.0),
            &params,
        );
        assert_close(state.p_known, 0.030303030303030304);

        let mut state = MasteryState::initial(0.20);
        fold_attempt(
            &mut state,
            &AttemptObservation::new("a3", "recall", 0.60, 3, 0.0),
            &params,
        );
        assert_close(state.p_known, 0.20);

        let mut state = MasteryState::initial(0.20);
        fold_attempt(
            &mut state,
            &AttemptObservation::new("a4", "free_explain", 0.80, 3, 0.0),
            &params,
        );
        assert_close(state.p_known, 0.8363636363636364);
    }

    #[test]
    fn calibration_updates_gap_and_skips_brier_in_dead_zone() {
        let params = MasteryParams::defaults();
        let mut state = MasteryState::initial(0.20);

        fold_attempt(
            &mut state,
            &AttemptObservation::new("a1", "recall", 0.40, 5, 0.0),
            &params,
        );
        assert_close(state.calib_gap, 0.18);
        assert_close(state.brier_ewma, 0.30);

        fold_attempt(
            &mut state,
            &AttemptObservation::new("a2", "recall", 0.60, 1, 1.0),
            &params,
        );
        assert_close(state.calib_gap, -0.054);
        assert_close(state.brier_ewma, 0.30);
    }

    #[test]
    fn incremental_fold_matches_full_replay_for_final_score_arrival() {
        let params = MasteryParams::defaults();
        let provisional = vec![
            AttemptObservation::new("a1", "recall", 0.90, 5, 0.0),
            AttemptObservation::new("a2", "recall", 0.82, 4, 2.0),
            AttemptObservation::new("a3", "free_explain", 0.34, 5, 5.0),
            AttemptObservation::new("a4", "recall", 0.62, 2, 8.0),
        ];

        let mut incremental = MasteryState::initial(0.20);
        for attempt in &provisional {
            fold_attempt(&mut incremental, attempt, &params);
        }

        let final_arrival = vec![
            AttemptObservation::new("a1", "recall", 0.90, 5, 0.0),
            AttemptObservation::new("a2", "recall", 0.20, 4, 2.0),
            AttemptObservation::new("a3", "free_explain", 0.34, 5, 5.0),
            AttemptObservation::new("a4", "recall", 0.62, 2, 8.0),
        ];

        let replayed = fold_all(0.20, &final_arrival, &params);
        let incrementally_replayed = replay_after_final(0.20, &provisional, "a2", 0.20, &params);

        assert_eq!(incrementally_replayed, replayed);
        assert_ne!(incremental, replayed);
    }
}
