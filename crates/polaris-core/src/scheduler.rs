use rusqlite::Connection;

use crate::config::{default_registry, meta_f64, ParameterSpec};
use crate::error::Result;
use crate::phase::Phase;

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleCandidate {
    pub id: String,
    pub seed_order: i64,
    pub retrieval: Option<f64>,
    pub calib_gap: f64,
    pub misconception_active: bool,
    pub has_attempts: bool,
    pub prerequisites_met: bool,
    pub phase: Phase,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub id: String,
    pub utility: f64,
    pub seed_order: i64,
}

#[derive(Debug, Clone)]
pub struct SchedulerParams {
    pub w_r: f64,
    pub w_cal: f64,
    pub w_mis: f64,
    pub w_new: f64,
    pub w_phase: f64,
}

impl SchedulerParams {
    pub fn defaults() -> Self {
        let registry = default_registry();
        Self {
            w_r: parse_f64(&registry, "sched.w_r"),
            w_cal: parse_f64(&registry, "sched.w_cal"),
            w_mis: parse_f64(&registry, "sched.w_mis"),
            w_new: parse_f64(&registry, "sched.w_new"),
            w_phase: parse_f64(&registry, "sched.w_phase"),
        }
    }

    pub fn from_conn(conn: &Connection) -> Result<Self> {
        Ok(Self {
            w_r: meta_f64(conn, "sched.w_r")?,
            w_cal: meta_f64(conn, "sched.w_cal")?,
            w_mis: meta_f64(conn, "sched.w_mis")?,
            w_new: meta_f64(conn, "sched.w_new")?,
            w_phase: meta_f64(conn, "sched.w_phase")?,
        })
    }
}

pub fn rank_candidates(candidates: Vec<ScheduleCandidate>) -> Vec<RankedCandidate> {
    rank_candidates_with_params(candidates, &SchedulerParams::defaults())
}

pub fn rank_candidates_with_params(
    candidates: Vec<ScheduleCandidate>,
    params: &SchedulerParams,
) -> Vec<RankedCandidate> {
    let mut ranked: Vec<RankedCandidate> = candidates
        .into_iter()
        .map(|candidate| {
            let utility = utility(&candidate, params);
            RankedCandidate {
                id: candidate.id,
                utility,
                seed_order: candidate.seed_order,
            }
        })
        .collect();

    ranked.sort_by(|left, right| {
        right
            .utility
            .total_cmp(&left.utility)
            .then_with(|| left.seed_order.cmp(&right.seed_order))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked
}

pub fn utility(candidate: &ScheduleCandidate, params: &SchedulerParams) -> f64 {
    let retrieval_term = candidate
        .retrieval
        .map(|r| params.w_r * (1.0 - r.clamp(0.0, 1.0)))
        .unwrap_or(0.0);
    let calibration_term = params.w_cal * candidate.calib_gap.max(0.0);
    let misconception_term = if candidate.misconception_active {
        params.w_mis
    } else {
        0.0
    };
    let new_concept_term = if !candidate.has_attempts && candidate.prerequisites_met {
        params.w_new
    } else {
        0.0
    };
    let phase_term = params.w_phase * candidate.phase.schedule_bonus();

    retrieval_term + calibration_term + misconception_term + new_concept_term + phase_term
}

#[derive(Debug, Clone, PartialEq)]
pub struct MisconceptionAttempt {
    pub id: String,
    pub at_day: i64,
    pub final_score: Option<f64>,
    pub misconception_id: Option<String>,
}

pub fn misconception_active_from_attempts(
    attempts: &[MisconceptionAttempt],
    now_day: i64,
    window_days: i64,
) -> bool {
    misconception_active_from_attempts_with_cut(attempts, now_day, window_days, 0.75)
}

pub fn misconception_active_from_attempts_with_cut(
    attempts: &[MisconceptionAttempt],
    now_day: i64,
    window_days: i64,
    success_cut: f64,
) -> bool {
    let mut ordered = attempts.to_vec();
    ordered.sort_by(|left, right| {
        left.at_day
            .cmp(&right.at_day)
            .then_with(|| left.id.cmp(&right.id))
    });

    for (index, attempt) in ordered.iter().enumerate() {
        if attempt.misconception_id.is_none() {
            continue;
        }
        if attempt.at_day > now_day {
            continue;
        }
        if now_day - attempt.at_day > window_days {
            continue;
        }
        let later_success = ordered[index + 1..]
            .iter()
            .any(|later| later.final_score.unwrap_or(0.0) >= success_cut);
        if !later_success {
            return true;
        }
    }

    false
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

    #[test]
    fn high_positive_calibration_gap_can_raise_priority() {
        let ranked = rank_candidates(vec![
            ScheduleCandidate {
                id: "plain_due".to_owned(),
                seed_order: 2,
                retrieval: Some(0.50),
                calib_gap: 0.0,
                misconception_active: false,
                has_attempts: true,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
            ScheduleCandidate {
                id: "phantom_like".to_owned(),
                seed_order: 1,
                retrieval: Some(0.80),
                calib_gap: 0.70,
                misconception_active: false,
                has_attempts: true,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
        ]);

        assert_eq!(ranked[0].id, "phantom_like");
    }

    #[test]
    fn misconception_and_new_concept_terms_follow_data_model() {
        let ranked = rank_candidates(vec![
            ScheduleCandidate {
                id: "new_locked".to_owned(),
                seed_order: 1,
                retrieval: None,
                calib_gap: 0.0,
                misconception_active: false,
                has_attempts: false,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
            ScheduleCandidate {
                id: "new_open".to_owned(),
                seed_order: 2,
                retrieval: None,
                calib_gap: 0.0,
                misconception_active: false,
                has_attempts: false,
                prerequisites_met: true,
                phase: Phase::Undetermined,
            },
            ScheduleCandidate {
                id: "misconception".to_owned(),
                seed_order: 3,
                retrieval: Some(0.95),
                calib_gap: 0.0,
                misconception_active: true,
                has_attempts: true,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
        ]);

        assert_eq!(ranked[0].id, "misconception");
        assert_eq!(ranked[1].id, "new_open");
        assert_eq!(ranked[2].id, "new_locked");
    }

    #[test]
    fn ties_sort_by_seed_order_then_id() {
        let ranked = rank_candidates(vec![
            ScheduleCandidate {
                id: "b".to_owned(),
                seed_order: 2,
                retrieval: Some(1.0),
                calib_gap: 0.0,
                misconception_active: false,
                has_attempts: true,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
            ScheduleCandidate {
                id: "a".to_owned(),
                seed_order: 2,
                retrieval: Some(1.0),
                calib_gap: 0.0,
                misconception_active: false,
                has_attempts: true,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
            ScheduleCandidate {
                id: "c".to_owned(),
                seed_order: 1,
                retrieval: Some(1.0),
                calib_gap: 0.0,
                misconception_active: false,
                has_attempts: true,
                prerequisites_met: false,
                phase: Phase::Undetermined,
            },
        ]);

        assert_eq!(
            ranked
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["c", "a", "b"]
        );
    }

    #[test]
    fn misconception_active_respects_window_and_later_success() {
        let attempts = vec![MisconceptionAttempt {
            id: "a1".to_owned(),
            at_day: 10,
            final_score: Some(0.20),
            misconception_id: Some("m1".to_owned()),
        }];
        assert!(misconception_active_from_attempts(&attempts, 20, 14));
        assert!(!misconception_active_from_attempts(&attempts, 25, 14));

        let corrected = vec![
            MisconceptionAttempt {
                id: "a1".to_owned(),
                at_day: 10,
                final_score: Some(0.20),
                misconception_id: Some("m1".to_owned()),
            },
            MisconceptionAttempt {
                id: "a2".to_owned(),
                at_day: 12,
                final_score: Some(0.90),
                misconception_id: None,
            },
        ];
        assert!(!misconception_active_from_attempts(&corrected, 20, 14));
    }

    proptest::proptest! {
        #[test]
        fn misconception_active_property_matches_window_and_later_success_semantics(
            raw in proptest::collection::vec((0_i64..40, proptest::bool::ANY, 0_i32..100), 1..30),
            now_day in 14_i64..50
        ) {
            let attempts = raw
                .iter()
                .enumerate()
                .map(|(idx, (at_day, has_misconception, score))| MisconceptionAttempt {
                    id: format!("a{idx:04}"),
                    at_day: *at_day,
                    final_score: Some(f64::from(*score) / 100.0),
                    misconception_id: has_misconception.then(|| "m1".to_owned()),
                })
                .collect::<Vec<_>>();

            let mut ordered = attempts.clone();
            ordered.sort_by(|left, right| {
                left.at_day
                    .cmp(&right.at_day)
                    .then_with(|| left.id.cmp(&right.id))
            });
            let expected = ordered.iter().enumerate().any(|(index, attempt)| {
                attempt.misconception_id.is_some()
                    && attempt.at_day <= now_day
                    && now_day - attempt.at_day <= 14
                    && !ordered[index + 1..]
                        .iter()
                        .any(|later| later.final_score.unwrap_or(0.0) >= 0.75)
            });

            proptest::prop_assert_eq!(
                misconception_active_from_attempts(&attempts, now_day, 14),
                expected
            );
        }
    }
}
