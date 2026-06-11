use serde::{Deserialize, Serialize};

use crate::config::default_registry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

impl Rating {
    pub fn from_score(score: f64) -> Self {
        let params = FsrsParams::defaults();
        if score < params.r_again {
            Self::Again
        } else if score < params.r_hard {
            Self::Hard
        } else if score < params.r_good {
            Self::Good
        } else {
            Self::Easy
        }
    }

    fn value(self) -> f64 {
        match self {
            Self::Again => 1.0,
            Self::Hard => 2.0,
            Self::Good => 3.0,
            Self::Easy => 4.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FsrsState {
    pub stability: f64,
    pub difficulty: f64,
    pub reps: u32,
    pub lapses: u32,
    pub scheduled_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FsrsUpdate {
    pub state: FsrsState,
    pub interval_days: i64,
}

#[derive(Debug, Clone)]
pub struct FsrsParams {
    w: [f64; 17],
    request_retention: f64,
    maximum_interval: i64,
    min_interval: i64,
    initial_stability: f64,
    initial_difficulty: f64,
    r_again: f64,
    r_hard: f64,
    r_good: f64,
}

impl FsrsParams {
    pub fn defaults() -> Self {
        let registry = default_registry();
        let w_values: Vec<f64> = serde_json::from_str(registry["fsrs.w"].default_value)
            .expect("valid default fsrs.w JSON");
        let w: [f64; 17] = w_values.try_into().expect("default fsrs.w has 17 entries");

        Self {
            w,
            request_retention: parse_f64(&registry, "fsrs.request_retention"),
            maximum_interval: parse_i64(&registry, "fsrs.maximum_interval"),
            min_interval: parse_i64(&registry, "fsrs.min_interval"),
            initial_stability: parse_f64(&registry, "fsrs.initial_stability"),
            initial_difficulty: parse_f64(&registry, "fsrs.initial_difficulty"),
            r_again: parse_f64(&registry, "fsrs.r_again"),
            r_hard: parse_f64(&registry, "fsrs.r_hard"),
            r_good: parse_f64(&registry, "fsrs.r_good"),
        }
    }
}

pub fn create_initial_state() -> FsrsState {
    let params = FsrsParams::defaults();
    FsrsState {
        stability: params.initial_stability,
        difficulty: params.initial_difficulty,
        reps: 0,
        lapses: 0,
        scheduled_days: None,
    }
}

pub fn update_state(state: &FsrsState, rating: Rating, elapsed_days: f64) -> FsrsUpdate {
    update_state_with_params(state, rating, elapsed_days, &FsrsParams::defaults())
}

pub fn update_state_with_params(
    state: &FsrsState,
    rating: Rating,
    elapsed_days: f64,
    params: &FsrsParams,
) -> FsrsUpdate {
    let s = state.stability;
    let d = state.difficulty;

    if state.reps == 0 {
        let new_stability = init_stability(rating, params);
        let new_difficulty = init_difficulty(rating, params);
        let interval_days = stability_to_interval_with_params(new_stability, params);

        return FsrsUpdate {
            state: FsrsState {
                stability: new_stability,
                difficulty: new_difficulty,
                reps: 1,
                lapses: u32::from(rating == Rating::Again),
                scheduled_days: Some(interval_days),
            },
            interval_days,
        };
    }

    let r = retrievability(s, elapsed_days);
    let new_difficulty = next_difficulty(d, rating, params);
    let mut new_lapses = state.lapses;

    let new_stability = if rating == Rating::Again {
        new_lapses += 1;
        next_forget_stability(d, s, r, params)
    } else {
        next_recall_stability(d, s, r, rating, params)
    };

    let interval_days = stability_to_interval_with_params(new_stability, params);
    FsrsUpdate {
        state: FsrsState {
            stability: new_stability,
            difficulty: new_difficulty,
            reps: state.reps + 1,
            lapses: new_lapses,
            scheduled_days: Some(interval_days),
        },
        interval_days,
    }
}

pub fn retrievability(stability: f64, elapsed_days: f64) -> f64 {
    (1.0 + elapsed_days / (9.0 * stability)).powf(-1.0)
}

pub fn stability_to_interval(stability: f64) -> i64 {
    stability_to_interval_with_params(stability, &FsrsParams::defaults())
}

fn init_stability(rating: Rating, params: &FsrsParams) -> f64 {
    params.w[(rating.value() as usize) - 1]
}

fn init_difficulty(rating: Rating, params: &FsrsParams) -> f64 {
    params.w[4] - (rating.value() - 3.0) * params.w[5]
}

fn next_difficulty(difficulty: f64, rating: Rating, params: &FsrsParams) -> f64 {
    let new_difficulty = difficulty - params.w[6] * (rating.value() - 3.0);
    new_difficulty.clamp(1.0, 10.0)
}

fn next_recall_stability(
    difficulty: f64,
    stability: f64,
    retrievability: f64,
    rating: Rating,
    params: &FsrsParams,
) -> f64 {
    let hard_penalty = if rating == Rating::Hard {
        params.w[15]
    } else {
        1.0
    };
    let easy_bonus = if rating == Rating::Easy {
        params.w[16]
    } else {
        1.0
    };

    stability
        * (1.0
            + params.w[8].exp()
                * (11.0 - difficulty)
                * stability.powf(-params.w[9])
                * (((1.0 - retrievability) * params.w[10]).exp() - 1.0)
                * hard_penalty
                * easy_bonus)
}

fn next_forget_stability(
    difficulty: f64,
    stability: f64,
    retrievability: f64,
    params: &FsrsParams,
) -> f64 {
    params.w[11]
        * difficulty.powf(-params.w[12])
        * ((stability + 1.0).powf(params.w[13]) - 1.0)
        * ((1.0 - retrievability) * params.w[14]).exp()
}

fn stability_to_interval_with_params(stability: f64, params: &FsrsParams) -> i64 {
    let interval = 9.0 * stability * (1.0 / params.request_retention - 1.0);
    (interval.round() as i64).clamp(params.min_interval, params.maximum_interval)
}

fn parse_f64(
    registry: &std::collections::BTreeMap<&'static str, crate::config::ParameterSpec>,
    key: &'static str,
) -> f64 {
    registry[key]
        .default_value
        .parse()
        .expect("valid default f64 parameter")
}

fn parse_i64(
    registry: &std::collections::BTreeMap<&'static str, crate::config::ParameterSpec>,
    key: &'static str,
) -> i64 {
    registry[key]
        .default_value
        .parse()
        .expect("valid default i64 parameter")
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
    fn score_maps_to_rating_thresholds_from_data_model() {
        assert_eq!(Rating::from_score(0.49), Rating::Again);
        assert_eq!(Rating::from_score(0.50), Rating::Hard);
        assert_eq!(Rating::from_score(0.70), Rating::Good);
        assert_eq!(Rating::from_score(0.90), Rating::Easy);
    }

    #[test]
    fn fsrs_matches_typescript_reference_sequences() {
        // Expected values were generated from
        // C:\MyProject\Polaris\apps\web\src\lib\fsrs.ts with the same rating
        // sequence and elapsed-day inputs.
        let cases = [
            (vec![(Rating::Good, 0.0)], 2.4, 4.93, 1, 0, 2),
            (vec![(Rating::Easy, 0.0)], 5.8, 3.9899999999999998, 1, 0, 6),
            (vec![(Rating::Again, 0.0)], 0.4, 6.81, 1, 1, 1),
            (
                vec![(Rating::Good, 0.0), (Rating::Hard, 3.0)],
                4.414240818867593,
                5.79,
                2,
                0,
                4,
            ),
            (
                vec![
                    (Rating::Good, 0.0),
                    (Rating::Easy, 3.0),
                    (Rating::Again, 6.0),
                    (Rating::Good, 9.0),
                ],
                19.648013528668443,
                5.789999999999999,
                4,
                1,
                20,
            ),
        ];

        for (sequence, expected_s, expected_d, expected_reps, expected_lapses, expected_days) in
            cases
        {
            let mut state = create_initial_state();
            let mut interval_days = 0;
            for (rating, elapsed_days) in sequence {
                let next = update_state(&state, rating, elapsed_days);
                state = next.state;
                interval_days = next.interval_days;
            }

            assert_close(state.stability, expected_s);
            assert_close(state.difficulty, expected_d);
            assert_eq!(state.reps, expected_reps);
            assert_eq!(state.lapses, expected_lapses);
            assert_eq!(interval_days, expected_days);
            assert_eq!(state.scheduled_days, Some(expected_days));
        }
    }

    #[test]
    fn retrievability_and_interval_match_reference() {
        assert_close(retrievability(2.4, 3.0), 0.8780487804878049);
        assert_eq!(stability_to_interval(2.4), 2);
    }
}
