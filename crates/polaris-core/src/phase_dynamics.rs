use std::collections::BTreeSet;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::{meta_f64, meta_i64};
use crate::error::Result;
use crate::phase::Phase;

pub const PHASE_COUNT: usize = Phase::ALL.len();

const PROBABILITY_EPSILON: f64 = 1e-9;
const LINEAR_SOLVE_EPSILON: f64 = 1e-12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseDynamicsStatus {
    NoData,
    InsufficientData,
    ShadowReady,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseDynamicsSummary {
    pub status: PhaseDynamicsStatus,
    pub transition_count: usize,
    pub ignored_event_count: usize,
    pub rows: Vec<PhaseTransitionRow>,
    pub target_expected_steps: Vec<PhaseExpectedSteps>,
    pub validation: PhaseDynamicsValidation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseTransitionRow {
    #[serde(serialize_with = "serialize_phase")]
    pub from: Phase,
    pub counts: Vec<u32>,
    pub probabilities: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseExpectedSteps {
    #[serde(serialize_with = "serialize_phase")]
    pub phase: Phase,
    pub expected_steps: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseDynamicsValidationStatus {
    Skipped,
    Computed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseDynamicsValidation {
    pub status: PhaseDynamicsValidationStatus,
    pub reason: Option<String>,
    pub train_count: usize,
    pub holdout_count: usize,
    pub static_accuracy: Option<f64>,
    pub markov_accuracy: Option<f64>,
    pub static_log_loss: Option<f64>,
    pub markov_log_loss: Option<f64>,
}

impl PhaseDynamicsValidation {
    fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: PhaseDynamicsValidationStatus::Skipped,
            reason: Some(reason.into()),
            train_count: 0,
            holdout_count: 0,
            static_accuracy: None,
            markov_accuracy: None,
            static_log_loss: None,
            markov_log_loss: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhaseTransition {
    from: Phase,
    to: Phase,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PhaseDynamicsParams {
    min_shadow_ready_transitions: usize,
    min_validation_transitions: usize,
    holdout_frac: f64,
}

#[derive(Debug, Deserialize)]
struct PhaseTransitionPayload {
    from: Option<String>,
    to: Option<String>,
}

pub fn phase_dynamics_summary(conn: &Connection) -> Result<PhaseDynamicsSummary> {
    let params = PhaseDynamicsParams::from_conn(conn)?;
    let (transitions, ignored_event_count) = load_phase_transitions(conn)?;
    let counts = transition_counts(&transitions);
    let probabilities = transition_probabilities(&counts);
    let rows = transition_rows(&counts, &probabilities);
    let status = if transitions.is_empty() {
        PhaseDynamicsStatus::NoData
    } else if transitions.len() < params.min_shadow_ready_transitions {
        PhaseDynamicsStatus::InsufficientData
    } else {
        PhaseDynamicsStatus::ShadowReady
    };
    let target_expected_steps =
        expected_steps_to_targets(&probabilities, &[Phase::Transfer, Phase::Generation]);
    let validation = validate_markov_shadow(&transitions, params);

    Ok(PhaseDynamicsSummary {
        status,
        transition_count: transitions.len(),
        ignored_event_count,
        rows,
        target_expected_steps,
        validation,
    })
}

impl PhaseDynamicsParams {
    fn from_conn(conn: &Connection) -> Result<Self> {
        Ok(Self {
            min_shadow_ready_transitions: meta_i64(
                conn,
                "phase_dynamics.min_shadow_ready_transitions",
            )?
            .clamp(1, 1000) as usize,
            min_validation_transitions: meta_i64(conn, "phase_dynamics.min_validation_transitions")?
                .clamp(2, 1000) as usize,
            holdout_frac: meta_f64(conn, "phase_dynamics.holdout_frac")?.clamp(0.05, 0.50),
        })
    }
}

fn load_phase_transitions(conn: &Connection) -> Result<(Vec<PhaseTransition>, usize)> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(payload_json, '')
         FROM behavior_events
         WHERE type='phase_transition'
         ORDER BY COALESCE(at, '1970-01-01T00:00:00Z') ASC, id ASC",
    )?;
    let payloads = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut transitions = Vec::new();
    let mut ignored = 0;
    for payload in payloads {
        if let Some(transition) = parse_transition_payload(&payload) {
            transitions.push(transition);
        } else {
            ignored += 1;
        }
    }

    Ok((transitions, ignored))
}

fn parse_transition_payload(payload_json: &str) -> Option<PhaseTransition> {
    let payload = serde_json::from_str::<PhaseTransitionPayload>(payload_json).ok()?;
    let from = Phase::parse(payload.from.as_deref()?)?;
    let to = Phase::parse(payload.to.as_deref()?)?;
    Some(PhaseTransition { from, to })
}

fn transition_counts(transitions: &[PhaseTransition]) -> [[u32; PHASE_COUNT]; PHASE_COUNT] {
    let mut counts = [[0_u32; PHASE_COUNT]; PHASE_COUNT];
    for transition in transitions {
        counts[phase_index(transition.from)][phase_index(transition.to)] += 1;
    }
    counts
}

fn transition_probabilities(
    counts: &[[u32; PHASE_COUNT]; PHASE_COUNT],
) -> [[f64; PHASE_COUNT]; PHASE_COUNT] {
    let mut probabilities = [[0.0; PHASE_COUNT]; PHASE_COUNT];
    for (from_idx, row) in counts.iter().enumerate() {
        let total = row.iter().map(|count| *count as f64).sum::<f64>();
        if total == 0.0 {
            continue;
        }
        for (to_idx, count) in row.iter().enumerate() {
            probabilities[from_idx][to_idx] = *count as f64 / total;
        }
    }
    probabilities
}

fn transition_rows(
    counts: &[[u32; PHASE_COUNT]; PHASE_COUNT],
    probabilities: &[[f64; PHASE_COUNT]; PHASE_COUNT],
) -> Vec<PhaseTransitionRow> {
    Phase::ALL
        .iter()
        .enumerate()
        .map(|(idx, phase)| PhaseTransitionRow {
            from: *phase,
            counts: counts[idx].to_vec(),
            probabilities: probabilities[idx].to_vec(),
        })
        .collect()
}

fn expected_steps_to_targets(
    probabilities: &[[f64; PHASE_COUNT]; PHASE_COUNT],
    targets: &[Phase],
) -> Vec<PhaseExpectedSteps> {
    let target_indices = targets
        .iter()
        .map(|phase| phase_index(*phase))
        .collect::<BTreeSet<_>>();

    Phase::ALL
        .iter()
        .map(|phase| PhaseExpectedSteps {
            phase: *phase,
            expected_steps: expected_steps_from_phase(
                probabilities,
                phase_index(*phase),
                &target_indices,
            ),
        })
        .collect()
}

fn expected_steps_from_phase(
    probabilities: &[[f64; PHASE_COUNT]; PHASE_COUNT],
    source_idx: usize,
    target_indices: &BTreeSet<usize>,
) -> Option<f64> {
    if target_indices.contains(&source_idx) {
        return Some(0.0);
    }

    let states = reachable_non_target_states(probabilities, source_idx, target_indices);
    if states.is_empty() {
        return None;
    }
    let matrix = hitting_time_matrix(probabilities, &states);
    let absorption_rhs = states
        .iter()
        .map(|state| {
            target_indices
                .iter()
                .map(|target| probabilities[*state][*target])
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let absorption_probabilities = solve_linear_system(matrix.clone(), absorption_rhs)?;
    let source_position = states.iter().position(|state| *state == source_idx)?;
    if absorption_probabilities[source_position] < 1.0 - 1e-7 {
        return None;
    }

    let expected = solve_linear_system(matrix, vec![1.0; states.len()])?;
    let value = expected[source_position];
    value.is_finite().then_some(value)
}

fn reachable_non_target_states(
    probabilities: &[[f64; PHASE_COUNT]; PHASE_COUNT],
    source_idx: usize,
    target_indices: &BTreeSet<usize>,
) -> Vec<usize> {
    let mut visited = BTreeSet::new();
    let mut stack = vec![source_idx];
    while let Some(state) = stack.pop() {
        if target_indices.contains(&state) || !visited.insert(state) {
            continue;
        }
        for (next_idx, probability) in probabilities[state].iter().enumerate() {
            if *probability > 0.0 && !target_indices.contains(&next_idx) {
                stack.push(next_idx);
            }
        }
    }
    visited.into_iter().collect()
}

fn hitting_time_matrix(
    probabilities: &[[f64; PHASE_COUNT]; PHASE_COUNT],
    states: &[usize],
) -> Vec<Vec<f64>> {
    states
        .iter()
        .enumerate()
        .map(|(row_idx, state)| {
            states
                .iter()
                .enumerate()
                .map(|(col_idx, next)| {
                    let identity = if row_idx == col_idx { 1.0 } else { 0.0 };
                    identity - probabilities[*state][*next]
                })
                .collect()
        })
        .collect()
}

fn solve_linear_system(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let n = rhs.len();
    if matrix.len() != n || matrix.iter().any(|row| row.len() != n) {
        return None;
    }

    for col in 0..n {
        let pivot = (col..n).max_by(|left, right| {
            matrix[*left][col]
                .abs()
                .total_cmp(&matrix[*right][col].abs())
        })?;
        if matrix[pivot][col].abs() < LINEAR_SOLVE_EPSILON {
            return None;
        }
        matrix.swap(col, pivot);
        rhs.swap(col, pivot);

        let pivot_value = matrix[col][col];
        for value in &mut matrix[col][col..] {
            *value /= pivot_value;
        }
        rhs[col] /= pivot_value;

        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = matrix[row][col];
            if factor.abs() < LINEAR_SOLVE_EPSILON {
                continue;
            }
            let pivot_tail = matrix[col][col..].to_vec();
            for (value, pivot_value) in matrix[row][col..].iter_mut().zip(pivot_tail.iter()) {
                *value -= factor * *pivot_value;
            }
            rhs[row] -= factor * rhs[col];
        }
    }

    rhs.iter().all(|value| value.is_finite()).then_some(rhs)
}

fn validate_markov_shadow(
    transitions: &[PhaseTransition],
    params: PhaseDynamicsParams,
) -> PhaseDynamicsValidation {
    if transitions.len() < params.min_validation_transitions {
        return PhaseDynamicsValidation::skipped(format!(
            "insufficient_transitions({}<{})",
            transitions.len(),
            params.min_validation_transitions
        ));
    }

    let holdout_count = ((transitions.len() as f64) * params.holdout_frac).round() as usize;
    let holdout_count = holdout_count
        .max(1)
        .min(transitions.len().saturating_sub(1));
    let split = transitions.len().saturating_sub(holdout_count);
    if split == 0 || split == transitions.len() {
        return PhaseDynamicsValidation::skipped("invalid_holdout_split");
    }

    let train = &transitions[..split];
    let holdout = &transitions[split..];
    let train_counts = transition_counts(train);
    let train_probabilities = transition_probabilities(&train_counts);

    let mut static_hits = 0_usize;
    let mut markov_hits = 0_usize;
    let mut static_log_loss = 0.0;
    let mut markov_log_loss = 0.0;

    for transition in holdout {
        if transition.from == transition.to {
            static_hits += 1;
        }
        let static_probability = if transition.from == transition.to {
            1.0 - PROBABILITY_EPSILON
        } else {
            PROBABILITY_EPSILON
        };
        static_log_loss += -static_probability.ln();

        let from_idx = phase_index(transition.from);
        let to_idx = phase_index(transition.to);
        let markov_probability = train_probabilities[from_idx][to_idx];
        if most_likely_to_phase(&train_counts, from_idx).is_some_and(|phase| phase == transition.to)
        {
            markov_hits += 1;
        }
        markov_log_loss += -bounded_probability(markov_probability).ln();
    }

    let holdout_len = holdout.len() as f64;
    PhaseDynamicsValidation {
        status: PhaseDynamicsValidationStatus::Computed,
        reason: None,
        train_count: train.len(),
        holdout_count: holdout.len(),
        static_accuracy: Some(static_hits as f64 / holdout_len),
        markov_accuracy: Some(markov_hits as f64 / holdout_len),
        static_log_loss: Some(static_log_loss / holdout_len),
        markov_log_loss: Some(markov_log_loss / holdout_len),
    }
}

fn most_likely_to_phase(
    counts: &[[u32; PHASE_COUNT]; PHASE_COUNT],
    from_idx: usize,
) -> Option<Phase> {
    let row = &counts[from_idx];
    let max_count = row.iter().copied().max()?;
    if max_count == 0 {
        return None;
    }
    let to_idx = row.iter().position(|count| *count == max_count)?;
    Some(Phase::ALL[to_idx])
}

fn bounded_probability(probability: f64) -> f64 {
    probability.clamp(PROBABILITY_EPSILON, 1.0 - PROBABILITY_EPSILON)
}

fn phase_index(phase: Phase) -> usize {
    Phase::ALL
        .iter()
        .position(|candidate| *candidate == phase)
        .expect("phase is in Phase::ALL")
}

fn serialize_phase<S>(phase: &Phase, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(phase.as_str())
}
