use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::config::{meta_f64, meta_i64, meta_value};
use crate::error::Result;
use crate::mental_state::{
    fit_hazard_model, reestimate_transitions, HazardInputs, HazardTrainingExample, HmmObservation,
    TransitionMatrix, HAZARD_FEATURE_COUNT, STATE_COUNT,
};

const TEN_MINUTES_DAYS: f64 = 10.0 / 1440.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskOutcome {
    pub status: String,
    pub detail: String,
}

impl TaskOutcome {
    fn done(detail: String) -> Self {
        Self {
            status: "done".to_owned(),
            detail,
        }
    }

    fn skipped(detail: String) -> Self {
        Self {
            status: "skipped".to_owned(),
            detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MentalFitSummary {
    pub hazard: TaskOutcome,
    pub state_gate: TaskOutcome,
    pub em: TaskOutcome,
}

#[derive(Debug, Clone)]
struct SnapshotSample {
    inputs: [f64; HAZARD_FEATURE_COUNT],
    abandoned_within_10m: bool,
    non_continue_within_10m: bool,
}

pub fn run_mental_dynamics_fit(conn: &Connection) -> Result<MentalFitSummary> {
    let samples = load_snapshot_samples(conn)?;
    Ok(MentalFitSummary {
        hazard: fit_hazard(conn, &samples)?,
        state_gate: evaluate_state_gate(conn, &samples)?,
        em: reestimate_hmm(conn)?,
    })
}

// ---------------------------------------------------------------------------
// hazard 周拟合
// ---------------------------------------------------------------------------

fn fit_hazard(conn: &Connection, samples: &[SnapshotSample]) -> Result<TaskOutcome> {
    let min_n = meta_i64(conn, "hazard.fit_min_n")?.max(2) as usize;
    if samples.len() < min_n {
        return Ok(TaskOutcome::skipped(format!(
            "insufficient_samples({}<{min_n})",
            samples.len()
        )));
    }

    let holdout_frac = meta_f64(conn, "hazard.holdout_frac")?.clamp(0.05, 0.5);
    let split = holdout_split(samples.len(), holdout_frac);
    let to_example = |sample: &SnapshotSample| HazardTrainingExample {
        inputs: HazardInputs {
            features: sample.inputs,
        },
        abandoned_within_10m: sample.abandoned_within_10m,
    };
    let train = samples[..split].iter().map(to_example).collect::<Vec<_>>();
    let validation = samples[split..].iter().map(to_example).collect::<Vec<_>>();
    if !has_both_classes(
        validation
            .iter()
            .map(|example| example.abandoned_within_10m),
    ) {
        return Ok(TaskOutcome::skipped("validation_single_class".to_owned()));
    }

    let auc_gate = meta_f64(conn, "hazard.auc_gate")?;
    let model = fit_hazard_model(
        &train,
        &validation,
        meta_f64(conn, "hazard.fit_l2")?,
        meta_i64(conn, "hazard.fit_iterations")?.max(1) as usize,
        meta_f64(conn, "hazard.fit_lr")?,
        auc_gate,
    );
    let Some(validation_auc) = model.validation_auc else {
        return Ok(TaskOutcome::skipped(
            "validation_auc_unavailable".to_owned(),
        ));
    };

    conn.execute(
        "INSERT INTO hazard_models(id, fitted_at, beta_json, validation_auc, n_train, n_validation)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, ?3, ?4, ?5)",
        params![
            Uuid::new_v4().to_string(),
            serde_json::to_string(&model.beta.to_vec())?,
            validation_auc,
            train.len() as i64,
            validation.len() as i64,
        ],
    )?;
    Ok(TaskOutcome::done(format!(
        "validation_auc={validation_auc:.3} gate={auc_gate:.2} n_train={} n_validation={}",
        train.len(),
        validation.len()
    )))
}

// ---------------------------------------------------------------------------
// 状态层门控评估（DATA_MODEL §7："下一动作"预测 AUC margin）
// ---------------------------------------------------------------------------

fn evaluate_state_gate(conn: &Connection, samples: &[SnapshotSample]) -> Result<TaskOutcome> {
    let min_n = meta_i64(conn, "hazard.fit_min_n")?.max(2) as usize;
    if samples.len() < min_n {
        return Ok(TaskOutcome::skipped(format!(
            "insufficient_samples({}<{min_n})",
            samples.len()
        )));
    }

    let holdout_frac = meta_f64(conn, "hazard.holdout_frac")?.clamp(0.05, 0.5);
    let split = holdout_split(samples.len(), holdout_frac);
    let state_example = |sample: &SnapshotSample| HazardTrainingExample {
        inputs: HazardInputs {
            features: sample.inputs,
        },
        abandoned_within_10m: sample.non_continue_within_10m,
    };
    // 无状态基线 = 状态后验替换为均匀分布（移除信息但保留隐式截距），同一拟合器。
    let baseline_example = |sample: &SnapshotSample| {
        let mut features = sample.inputs;
        for value in features.iter_mut().take(STATE_COUNT) {
            *value = 1.0 / STATE_COUNT as f64;
        }
        HazardTrainingExample {
            inputs: HazardInputs { features },
            abandoned_within_10m: sample.non_continue_within_10m,
        }
    };

    let state_train = samples[..split]
        .iter()
        .map(state_example)
        .collect::<Vec<_>>();
    let state_validation = samples[split..]
        .iter()
        .map(state_example)
        .collect::<Vec<_>>();
    if !has_both_classes(
        state_validation
            .iter()
            .map(|example| example.abandoned_within_10m),
    ) {
        return Ok(TaskOutcome::skipped("validation_single_class".to_owned()));
    }
    let baseline_train = samples[..split]
        .iter()
        .map(baseline_example)
        .collect::<Vec<_>>();
    let baseline_validation = samples[split..]
        .iter()
        .map(baseline_example)
        .collect::<Vec<_>>();

    let l2 = meta_f64(conn, "hazard.fit_l2")?;
    let iterations = meta_i64(conn, "hazard.fit_iterations")?.max(1) as usize;
    let lr = meta_f64(conn, "hazard.fit_lr")?;
    let state_model = fit_hazard_model(&state_train, &state_validation, l2, iterations, lr, 1.0);
    let baseline_model = fit_hazard_model(
        &baseline_train,
        &baseline_validation,
        l2,
        iterations,
        lr,
        1.0,
    );
    let (Some(state_auc), Some(baseline_auc)) =
        (state_model.validation_auc, baseline_model.validation_auc)
    else {
        return Ok(TaskOutcome::skipped(
            "validation_auc_unavailable".to_owned(),
        ));
    };

    let gate_margin = meta_f64(conn, "hmm.gate_auc_margin")?;
    let margin = state_auc - baseline_auc;
    let passes = margin >= gate_margin;
    conn.execute(
        "INSERT INTO state_gate_evals(id, evaluated_at, baseline_auc, state_auc, margin, passes, n)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            baseline_auc,
            state_auc,
            margin,
            i64::from(passes),
            samples.len() as i64,
        ],
    )?;
    Ok(TaskOutcome::done(format!(
        "state_auc={state_auc:.3} baseline_auc={baseline_auc:.3} margin={margin:+.3} passes={passes}"
    )))
}

// ---------------------------------------------------------------------------
// EM 重估（转移矩阵；发射先验冻结）
// ---------------------------------------------------------------------------

fn reestimate_hmm(conn: &Connection) -> Result<TaskOutcome> {
    let min_n = meta_i64(conn, "hmm.em_min_n")?.max(1);
    let graded: i64 = conn.query_row(
        "SELECT COUNT(*) FROM attempts WHERE final_score IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    if graded < min_n {
        return Ok(TaskOutcome::skipped(format!(
            "insufficient_graded({graded}<{min_n})"
        )));
    }

    let sequences = load_observation_sequences(conn)?;
    let usable = sequences
        .iter()
        .filter(|sequence| sequence.len() >= 2)
        .count();
    if usable == 0 {
        return Ok(TaskOutcome::skipped("no_usable_sequences".to_owned()));
    }

    let transitions = reestimate_transitions(&sequences, 10);
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES ('hmm.transitions', ?1)",
        [serde_json::to_string(&transitions_to_rows(&transitions))?],
    )?;
    Ok(TaskOutcome::done(format!(
        "sequences={usable} graded={graded}"
    )))
}

fn transitions_to_rows(transitions: &TransitionMatrix) -> Vec<Vec<f64>> {
    transitions.iter().map(|row| row.to_vec()).collect()
}

pub fn transitions_from_meta(conn: &Connection) -> Result<Option<TransitionMatrix>> {
    let raw = meta_value(conn, "hmm.transitions")?;
    let rows: Vec<Vec<f64>> = match serde_json::from_str(&raw) {
        Ok(rows) => rows,
        Err(_) => return Ok(None),
    };
    if rows.len() != STATE_COUNT || rows.iter().any(|row| row.len() != STATE_COUNT) {
        return Ok(None);
    }
    let mut transitions = [[0.0; STATE_COUNT]; STATE_COUNT];
    for (target, source) in transitions.iter_mut().zip(rows.iter()) {
        for (value, raw) in target.iter_mut().zip(source.iter()) {
            if !raw.is_finite() || *raw < 0.0 {
                return Ok(None);
            }
            *value = *raw;
        }
    }
    Ok(Some(transitions))
}

// ---------------------------------------------------------------------------
// 数据装载
// ---------------------------------------------------------------------------

fn load_snapshot_samples(conn: &Connection) -> Result<Vec<SnapshotSample>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(session_id, 'default'), julianday(at), payload_json
         FROM behavior_events
         WHERE type='mental_state'
           AND json_extract(payload_json, '$.score_source')='provisional'
         ORDER BY julianday(at) ASC, rowid ASC",
    )?;
    let snapshot_rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT COALESCE(session_id, 'default'), julianday(at), type
         FROM behavior_events
         WHERE type IN ('abandon', 'hint')
         ORDER BY julianday(at) ASC, rowid ASC",
    )?;
    let action_rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut samples = Vec::new();
    for (session_id, at, payload) in snapshot_rows {
        let value: serde_json::Value = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(inputs) = hazard_inputs_from_payload(&value) else {
            continue;
        };
        let follows = |kinds: &[&str]| {
            action_rows.iter().any(|(action_session, action_at, kind)| {
                action_session == &session_id
                    && *action_at > at
                    && *action_at - at <= TEN_MINUTES_DAYS
                    && kinds.contains(&kind.as_str())
            })
        };
        samples.push(SnapshotSample {
            inputs,
            abandoned_within_10m: follows(&["abandon"]),
            non_continue_within_10m: follows(&["abandon", "hint"]),
        });
    }
    Ok(samples)
}

fn hazard_inputs_from_payload(value: &serde_json::Value) -> Option<[f64; HAZARD_FEATURE_COUNT]> {
    let raw = value.pointer("/hazard/inputs")?.as_array()?;
    if raw.len() != HAZARD_FEATURE_COUNT {
        return None;
    }
    let mut inputs = [0.0; HAZARD_FEATURE_COUNT];
    for (target, source) in inputs.iter_mut().zip(raw.iter()) {
        *target = source.as_f64()?;
        if !target.is_finite() {
            return None;
        }
    }
    Some(inputs)
}

fn load_observation_sequences(conn: &Connection) -> Result<Vec<Vec<HmmObservation>>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(session_id, 'default'), payload_json
         FROM behavior_events
         WHERE type='mental_state'
           AND json_extract(payload_json, '$.score_source')='provisional'
         ORDER BY COALESCE(session_id, 'default') ASC, julianday(at) ASC, rowid ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut sequences = Vec::new();
    let mut current_session: Option<String> = None;
    for (session_id, payload) in rows {
        let value: serde_json::Value = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(observation) = observation_from_payload(&value) else {
            continue;
        };
        if current_session.as_deref() != Some(session_id.as_str()) {
            sequences.push(Vec::new());
            current_session = Some(session_id);
        }
        if let Some(sequence) = sequences.last_mut() {
            sequence.push(observation);
        }
    }
    Ok(sequences)
}

fn observation_from_payload(value: &serde_json::Value) -> Option<HmmObservation> {
    let features = value.get("features")?;
    let field = |key: &str| features.get(key).and_then(serde_json::Value::as_f64);
    Some(HmmObservation {
        z_latency: field("z_latency")?,
        hints: field("hints")?,
        residual: field("residual")?,
        consec_fail: field("consec_fail")?,
        conf_delta: field("conf_delta")?,
        interval_bucket: field("interval_bucket")?,
        session_min: field("session_min")?,
    })
}

// ---------------------------------------------------------------------------
// 公共辅助
// ---------------------------------------------------------------------------

fn holdout_split(total: usize, holdout_frac: f64) -> usize {
    let holdout = ((total as f64) * holdout_frac).ceil() as usize;
    total.saturating_sub(holdout.max(1))
}

fn has_both_classes(labels: impl Iterator<Item = bool>) -> bool {
    let mut seen_positive = false;
    let mut seen_negative = false;
    for label in labels {
        if label {
            seen_positive = true;
        } else {
            seen_negative = true;
        }
        if seen_positive && seen_negative {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holdout_split_keeps_last_fraction() {
        assert_eq!(holdout_split(60, 0.20), 48);
        assert_eq!(holdout_split(10, 0.20), 8);
    }

    #[test]
    fn has_both_classes_detects_single_class() {
        assert!(!has_both_classes([true, true].into_iter()));
        assert!(!has_both_classes([false].into_iter()));
        assert!(has_both_classes([true, false].into_iter()));
    }

    #[test]
    fn transitions_round_trip_through_rows() {
        let prior = crate::mental_state::prior_transitions();
        let rows = transitions_to_rows(&prior);
        assert_eq!(rows.len(), STATE_COUNT);
        assert!((rows[0][0] - 0.70).abs() < 1e-12);
    }
}
