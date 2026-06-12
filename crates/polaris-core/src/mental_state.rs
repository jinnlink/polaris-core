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

pub fn forward_filter(
    previous: Option<&StatePosterior>,
    observation: HmmObservation,
) -> StatePosterior {
    let previous = previous.cloned().unwrap_or_else(StatePosterior::uniform);
    let mut predicted = [0.0; STATE_COUNT];
    for from in 0..STATE_COUNT {
        for (to, value) in predicted.iter_mut().enumerate() {
            let transition = if from == to {
                TRANSITION_STAY
            } else {
                TRANSITION_SWITCH
            };
            *value += previous.probabilities[from] * transition;
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
