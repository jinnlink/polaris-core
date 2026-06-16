use serde::{Deserialize, Serialize};

pub const STATE_COUNT: usize = 6;
pub const HAZARD_FEATURE_COUNT: usize = 12;

const TRANSITION_STAY: f64 = 0.70;
const TRANSITION_SWITCH: f64 = 0.06;

const SESSION_MIN_SCALE: f64 = 40.0;
const EMISSION_FEATURE_COUNT: usize = 7;

const EMISSION_MEANS: [[f64; EMISSION_FEATURE_COUNT]; STATE_COUNT] = [
    [-0.5, 0.2, 0.10, 0.2, 0.2, 0.0, 0.2],
    [0.5, 0.8, -0.20, 1.0, 0.0, 0.5, 0.5],
    [1.0, 1.5, -0.30, 2.5, -0.5, 1.0, 0.8],
    [-0.8, 0.1, 0.00, 0.3, 0.0, 2.0, 0.2],
    [0.5, 0.5, -0.10, 1.0, -0.8, 0.5, 0.6],
    [0.8, 0.6, -0.15, 1.5, -0.2, 1.0, 2.0],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentalState {
    Flow,
    ProductiveConfusion,
    Frustrated,
    Bored,
    Anxious,
    Fatigued,
}

impl MentalState {
    pub const ALL: [MentalState; STATE_COUNT] = [
        MentalState::Flow,
        MentalState::ProductiveConfusion,
        MentalState::Frustrated,
        MentalState::Bored,
        MentalState::Anxious,
        MentalState::Fatigued,
    ];

    pub fn index(self) -> usize {
        match self {
            MentalState::Flow => 0,
            MentalState::ProductiveConfusion => 1,
            MentalState::Frustrated => 2,
            MentalState::Bored => 3,
            MentalState::Anxious => 4,
            MentalState::Fatigued => 5,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MentalState::Flow => "flow",
            MentalState::ProductiveConfusion => "productive_confusion",
            MentalState::Frustrated => "frustrated",
            MentalState::Bored => "bored",
            MentalState::Anxious => "anxious",
            MentalState::Fatigued => "fatigued",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "flow" => Some(MentalState::Flow),
            "productive_confusion" => Some(MentalState::ProductiveConfusion),
            "frustrated" => Some(MentalState::Frustrated),
            "bored" => Some(MentalState::Bored),
            "anxious" => Some(MentalState::Anxious),
            "fatigued" => Some(MentalState::Fatigued),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HmmObservation {
    pub z_latency: f64,
    pub hints: f64,
    pub residual: f64,
    pub consec_fail: f64,
    pub conf_delta: f64,
    pub interval_bucket: f64,
    pub session_min: f64,
}

impl HmmObservation {
    fn emission_features(self) -> [f64; EMISSION_FEATURE_COUNT] {
        [
            finite_or_zero(self.z_latency),
            finite_or_zero(self.hints).clamp(0.0, 3.0),
            finite_or_zero(self.residual),
            finite_or_zero(self.consec_fail),
            finite_or_zero(self.conf_delta),
            finite_or_zero(self.interval_bucket).clamp(0.0, 2.0),
            (finite_or_zero(self.session_min) / SESSION_MIN_SCALE).clamp(0.0, 3.0),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatePosterior {
    pub probabilities: [f64; STATE_COUNT],
}

impl StatePosterior {
    pub fn uniform() -> Self {
        Self {
            probabilities: [1.0 / STATE_COUNT as f64; STATE_COUNT],
        }
    }

    pub fn dominant_state(&self) -> MentalState {
        let mut best = 0;
        for idx in 1..STATE_COUNT {
            if self.probabilities[idx] > self.probabilities[best] {
                best = idx;
            }
        }
        MentalState::ALL[best]
    }

    pub fn probability(&self, state: MentalState) -> f64 {
        self.probabilities[state.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HazardInputs {
    pub features: [f64; HAZARD_FEATURE_COUNT],
}

impl HazardInputs {
    pub fn new(
        posterior: &StatePosterior,
        calib_gap: f64,
        consec_fail: f64,
        hint_rate: f64,
        time_sin: f64,
        time_cos: f64,
        session_min: f64,
    ) -> Self {
        let mut features = [0.0; HAZARD_FEATURE_COUNT];
        features[..STATE_COUNT].copy_from_slice(&posterior.probabilities);
        features[6] = finite_or_zero(calib_gap);
        features[7] = finite_or_zero(consec_fail);
        features[8] = finite_or_zero(hint_rate);
        features[9] = finite_or_zero(time_sin);
        features[10] = finite_or_zero(time_cos);
        features[11] = finite_or_zero(session_min);
        Self { features }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HazardEstimate {
    pub probability: f64,
    pub participates: bool,
    pub validation_auc: Option<f64>,
    pub auc_gate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HazardTrainingExample {
    pub inputs: HazardInputs,
    pub abandoned_within_10m: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HazardModel {
    pub beta: [f64; HAZARD_FEATURE_COUNT],
    pub validation_auc: Option<f64>,
    pub auc_gate: f64,
}

impl HazardModel {
    pub fn estimate(&self, inputs: HazardInputs) -> HazardEstimate {
        estimate_hazard(inputs, &self.beta, self.validation_auc, self.auc_gate)
    }
}

pub type TransitionMatrix = [[f64; STATE_COUNT]; STATE_COUNT];

pub fn prior_transitions() -> TransitionMatrix {
    let mut transitions = [[TRANSITION_SWITCH; STATE_COUNT]; STATE_COUNT];
    for (idx, row) in transitions.iter_mut().enumerate() {
        row[idx] = TRANSITION_STAY;
    }
    transitions
}

pub fn forward_filter(
    previous: Option<&StatePosterior>,
    observation: HmmObservation,
) -> StatePosterior {
    forward_filter_with_transitions(previous, observation, &prior_transitions())
}

pub fn forward_filter_with_transitions(
    previous: Option<&StatePosterior>,
    observation: HmmObservation,
    transitions: &TransitionMatrix,
) -> StatePosterior {
    let previous = previous.cloned().unwrap_or_else(StatePosterior::uniform);
    let mut predicted = [0.0; STATE_COUNT];
    for (probability, row) in previous.probabilities.iter().zip(transitions.iter()) {
        for (value, transition) in predicted.iter_mut().zip(row.iter()) {
            *value += probability * transition;
        }
    }

    let features = observation.emission_features();
    let mut log_weights = [0.0; STATE_COUNT];
    for state in MentalState::ALL {
        let idx = state.index();
        log_weights[idx] = predicted[idx].max(f64::MIN_POSITIVE).ln()
            + diagonal_gaussian_log_likelihood(&features, &EMISSION_MEANS[idx]);
    }

    StatePosterior {
        probabilities: softmax(log_weights),
    }
}

const TRANSITION_FLOOR: f64 = 0.01;

/// Baum-Welch 仅重估转移矩阵；发射先验冻结（DATA_MODEL §7 表）。
/// 行下限防吸收态，行归一保持随机矩阵性质。
pub fn reestimate_transitions(
    sequences: &[Vec<HmmObservation>],
    iterations: usize,
) -> TransitionMatrix {
    let mut transitions = prior_transitions();
    let usable = sequences
        .iter()
        .filter(|sequence| sequence.len() >= 2)
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return transitions;
    }

    for _ in 0..iterations.max(1) {
        let mut xi_sum = [[1e-6; STATE_COUNT]; STATE_COUNT];
        for sequence in &usable {
            accumulate_transition_counts(sequence, &transitions, &mut xi_sum);
        }
        let shrink = 1.0 - TRANSITION_FLOOR * STATE_COUNT as f64;
        for row in &mut xi_sum {
            let total = row.iter().sum::<f64>();
            if total <= 0.0 || !total.is_finite() {
                *row = [1.0 / STATE_COUNT as f64; STATE_COUNT];
                continue;
            }
            for value in row.iter_mut() {
                *value = TRANSITION_FLOOR + shrink * (*value / total);
            }
        }
        transitions = xi_sum;
    }
    transitions
}

fn accumulate_transition_counts(
    sequence: &[HmmObservation],
    transitions: &TransitionMatrix,
    xi_sum: &mut [[f64; STATE_COUNT]; STATE_COUNT],
) {
    let emissions = sequence
        .iter()
        .map(|observation| {
            let features = observation.emission_features();
            let mut row = [0.0; STATE_COUNT];
            for (idx, value) in row.iter_mut().enumerate() {
                *value = diagonal_gaussian_log_likelihood(&features, &EMISSION_MEANS[idx]).exp();
            }
            row
        })
        .collect::<Vec<_>>();

    let len = sequence.len();
    // 缩放前向
    let mut alpha = vec![[0.0; STATE_COUNT]; len];
    let mut scales = vec![0.0; len];
    for state in 0..STATE_COUNT {
        alpha[0][state] = emissions[0][state] / STATE_COUNT as f64;
    }
    scales[0] = normalize_in_place(&mut alpha[0]);
    for t in 1..len {
        let (head, tail) = alpha.split_at_mut(t);
        let previous = &head[t - 1];
        let current = &mut tail[0];
        for (to, value) in current.iter_mut().enumerate() {
            let mut total = 0.0;
            for from in 0..STATE_COUNT {
                total += previous[from] * transitions[from][to];
            }
            *value = total * emissions[t][to];
        }
        scales[t] = normalize_in_place(current);
    }

    // 缩放后向
    let mut beta = vec![[1.0; STATE_COUNT]; len];
    for t in (0..len - 1).rev() {
        let (head, tail) = beta.split_at_mut(t + 1);
        let next = &tail[0];
        let current = &mut head[t];
        for (from, value) in current.iter_mut().enumerate() {
            let mut total = 0.0;
            for to in 0..STATE_COUNT {
                total += transitions[from][to] * emissions[t + 1][to] * next[to];
            }
            *value = total;
        }
        let scale = scales[t + 1].max(f64::MIN_POSITIVE);
        for value in current.iter_mut() {
            *value /= scale;
        }
    }

    for t in 0..len - 1 {
        let mut xi = [[0.0; STATE_COUNT]; STATE_COUNT];
        let mut total = 0.0;
        for from in 0..STATE_COUNT {
            for to in 0..STATE_COUNT {
                let value =
                    alpha[t][from] * transitions[from][to] * emissions[t + 1][to] * beta[t + 1][to];
                xi[from][to] = value;
                total += value;
            }
        }
        if total <= 0.0 || !total.is_finite() {
            continue;
        }
        for from in 0..STATE_COUNT {
            for to in 0..STATE_COUNT {
                xi_sum[from][to] += xi[from][to] / total;
            }
        }
    }
}

fn normalize_in_place(row: &mut [f64; STATE_COUNT]) -> f64 {
    let total = row.iter().sum::<f64>();
    if total <= 0.0 || !total.is_finite() {
        *row = [1.0 / STATE_COUNT as f64; STATE_COUNT];
        return 1.0;
    }
    for value in row.iter_mut() {
        *value /= total;
    }
    total
}

pub fn estimate_hazard(
    inputs: HazardInputs,
    beta: &[f64; HAZARD_FEATURE_COUNT],
    validation_auc: Option<f64>,
    auc_gate: f64,
) -> HazardEstimate {
    let logit = inputs
        .features
        .iter()
        .zip(beta.iter())
        .map(|(feature, coefficient)| feature * coefficient)
        .sum::<f64>();
    HazardEstimate {
        probability: sigmoid(logit),
        participates: validation_auc.map(|auc| auc >= auc_gate).unwrap_or(false),
        validation_auc,
        auc_gate,
    }
}

pub fn fit_hazard_model(
    training: &[HazardTrainingExample],
    validation: &[HazardTrainingExample],
    l2: f64,
    iterations: usize,
    learning_rate: f64,
    auc_gate: f64,
) -> HazardModel {
    let mut beta = [0.0; HAZARD_FEATURE_COUNT];
    if training.is_empty() {
        return HazardModel {
            beta,
            validation_auc: None,
            auc_gate,
        };
    }

    let n = training.len() as f64;
    let l2 = l2.max(0.0);
    let learning_rate = learning_rate.max(0.0);
    for _ in 0..iterations {
        let mut gradient = [0.0; HAZARD_FEATURE_COUNT];
        for example in training {
            let probability = estimate_hazard(example.inputs, &beta, None, auc_gate).probability;
            let y = if example.abandoned_within_10m {
                1.0
            } else {
                0.0
            };
            for (idx, feature) in example.inputs.features.iter().enumerate() {
                gradient[idx] += (probability - y) * feature / n;
            }
        }
        for (idx, coefficient) in beta.iter_mut().enumerate() {
            gradient[idx] += l2 * *coefficient;
            *coefficient -= learning_rate * gradient[idx];
        }
    }

    let scored = validation
        .iter()
        .map(|example| {
            (
                estimate_hazard(example.inputs, &beta, None, auc_gate).probability,
                example.abandoned_within_10m,
            )
        })
        .collect::<Vec<_>>();
    HazardModel {
        beta,
        validation_auc: auc(&scored),
        auc_gate,
    }
}

fn diagonal_gaussian_log_likelihood(
    features: &[f64; EMISSION_FEATURE_COUNT],
    means: &[f64; EMISSION_FEATURE_COUNT],
) -> f64 {
    features
        .iter()
        .zip(means.iter())
        .map(|(feature, mean)| {
            let diff = feature - mean;
            -0.5 * diff * diff
        })
        .sum()
}

fn auc(scored: &[(f64, bool)]) -> Option<f64> {
    let positives = scored.iter().filter(|(_, label)| *label).count();
    let negatives = scored.len().checked_sub(positives)?;
    if positives == 0 || negatives == 0 {
        return None;
    }

    let mut wins = 0.0;
    for (positive_score, _) in scored.iter().filter(|(_, label)| *label) {
        for (negative_score, _) in scored.iter().filter(|(_, label)| !*label) {
            if positive_score > negative_score {
                wins += 1.0;
            } else if (positive_score - negative_score).abs() < f64::EPSILON {
                wins += 0.5;
            }
        }
    }
    Some(wins / (positives * negatives) as f64)
}

fn softmax(log_weights: [f64; STATE_COUNT]) -> [f64; STATE_COUNT] {
    let max = log_weights
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let mut weights = [0.0; STATE_COUNT];
    let mut total = 0.0;
    for (idx, log_weight) in log_weights.iter().enumerate() {
        let weight = (log_weight - max).exp();
        weights[idx] = weight;
        total += weight;
    }

    if total <= 0.0 || !total.is_finite() {
        return StatePosterior::uniform().probabilities;
    }

    for weight in &mut weights {
        *weight /= total;
    }
    weights
}

fn sigmoid(logit: f64) -> f64 {
    if logit >= 0.0 {
        1.0 / (1.0 + (-logit).exp())
    } else {
        let exp = logit.exp();
        exp / (1.0 + exp)
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod em_tests {
    use super::*;

    fn sticky_observation(state: MentalState) -> HmmObservation {
        let means = EMISSION_MEANS[state.index()];
        HmmObservation {
            z_latency: means[0],
            hints: means[1],
            residual: means[2],
            consec_fail: means[3],
            conf_delta: means[4],
            interval_bucket: means[5],
            session_min: means[6] * 40.0,
        }
    }

    #[test]
    fn reestimate_transitions_returns_valid_stochastic_matrix() {
        let mut sequences = Vec::new();
        for _ in 0..4 {
            let mut sequence = Vec::new();
            for _ in 0..12 {
                sequence.push(sticky_observation(MentalState::Flow));
            }
            for _ in 0..12 {
                sequence.push(sticky_observation(MentalState::Frustrated));
            }
            sequences.push(sequence);
        }

        let transitions = reestimate_transitions(&sequences, 10);

        for row in &transitions {
            let total = row.iter().sum::<f64>();
            assert!((total - 1.0).abs() < 1e-9, "row sums to {total}");
            for value in row {
                assert!(*value >= TRANSITION_FLOOR - 1e-12 && value.is_finite());
            }
        }
        let flow = MentalState::Flow.index();
        assert!(
            transitions[flow][flow] > 0.5,
            "sticky data should keep diagonal dominant, got {}",
            transitions[flow][flow]
        );
    }

    #[test]
    fn forward_filter_with_prior_matches_legacy_constants() {
        let observation = sticky_observation(MentalState::Bored);
        let legacy = forward_filter(None, observation);
        let explicit = forward_filter_with_transitions(None, observation, &prior_transitions());
        assert_eq!(legacy, explicit);
    }

    #[test]
    fn reestimate_with_no_usable_sequences_keeps_prior() {
        let transitions = reestimate_transitions(&[vec![]], 5);
        assert_eq!(transitions, prior_transitions());
    }
}
