use std::collections::BTreeMap;

use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::config::{meta_f64, meta_i64};
use crate::error::{PolarisError, Result};
use crate::fsrs::{
    create_initial_state_with_params, retrievability, update_state_with_params, FsrsParams, Rating,
};

const PARAM: &str = "fsrs.w";
const METRIC: &str = "fsrs_holdout_logloss";
const PROBABILITY_EPSILON: f64 = 1e-6;
const MIN_WEIGHT: f64 = 1e-4;
const MAX_WEIGHT: f64 = 50.0;
const SEARCH_FACTORS: [f64; 4] = [0.5, 0.8, 1.25, 1.5];
const SEARCH_PASSES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FsrsFitStatus {
    Skipped,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FsrsFitSummary {
    pub param: String,
    pub status: FsrsFitStatus,
    pub old_value: String,
    pub new_value: String,
    pub old_weights: Vec<f64>,
    pub candidate_weights: Vec<f64>,
    pub metric: String,
    pub current_metric: Option<f64>,
    pub candidate_metric: Option<f64>,
    pub delta: f64,
    pub accepted: bool,
    pub reason: Option<String>,
    pub total_final_attempts: usize,
    pub train_predictions: usize,
    pub holdout_predictions: usize,
    pub candidates_evaluated: usize,
    pub replayed_concepts: usize,
}

#[derive(Debug, Clone)]
struct FinalAttempt {
    concept_id: String,
    score: f64,
    day: f64,
}

#[derive(Debug, Clone, Copy)]
struct FitConfig {
    min_attempts: usize,
    min_holdout_predictions: usize,
    holdout_frac: f64,
    accept_margin: f64,
}

#[derive(Debug, Clone, Copy)]
struct SplitMetrics {
    train: MetricStats,
    holdout: MetricStats,
}

#[derive(Debug, Clone, Copy, Default)]
struct MetricStats {
    count: usize,
    logloss_sum: f64,
}

impl MetricStats {
    fn push(&mut self, prediction: f64, outcome: f64) {
        let p = prediction.clamp(PROBABILITY_EPSILON, 1.0 - PROBABILITY_EPSILON);
        self.logloss_sum += -(outcome * p.ln() + (1.0 - outcome) * (1.0 - p).ln());
        self.count += 1;
    }

    fn logloss(self) -> Option<f64> {
        (self.count > 0).then_some(self.logloss_sum / self.count as f64)
    }
}

impl FitConfig {
    fn from_conn(conn: &Connection) -> Result<Self> {
        Ok(Self {
            min_attempts: meta_i64(conn, "fsrs_fit.min_attempts")?.clamp(1, 100_000) as usize,
            min_holdout_predictions: meta_i64(conn, "fsrs_fit.min_holdout_predictions")?
                .clamp(1, 100_000) as usize,
            holdout_frac: meta_f64(conn, "fsrs_fit.holdout_frac")?.clamp(0.05, 0.50),
            accept_margin: meta_f64(conn, "fsrs_fit.accept_margin")?,
        })
    }
}

pub fn evaluate_fsrs_personal_params(conn: &Connection) -> Result<FsrsFitSummary> {
    let config = FitConfig::from_conn(conn)?;
    let attempts = load_final_attempts(conn)?;
    let current_params = FsrsParams::from_conn(conn)?;
    let current_weights = *current_params.weights();
    let old_value = weights_json(&current_weights)?;
    let old_weights = current_weights.to_vec();

    if attempts.len() < config.min_attempts {
        return Ok(skipped_summary(
            old_value,
            old_weights,
            attempts.len(),
            0,
            0,
            format!(
                "insufficient_history({}<{})",
                attempts.len(),
                config.min_attempts
            ),
        ));
    }

    let Some(total_predictions) = prediction_count(&attempts, &current_params) else {
        return Ok(skipped_summary(
            old_value,
            old_weights,
            attempts.len(),
            0,
            0,
            "metric_unavailable".to_owned(),
        ));
    };
    let holdout_start = holdout_start_index(total_predictions, config.holdout_frac);
    let holdout_predictions = total_predictions.saturating_sub(holdout_start);
    if holdout_predictions < config.min_holdout_predictions {
        return Ok(skipped_summary(
            old_value,
            old_weights,
            attempts.len(),
            holdout_start,
            holdout_predictions,
            format!(
                "insufficient_holdout_predictions({holdout_predictions}<{})",
                config.min_holdout_predictions
            ),
        ));
    }

    let Some(current_metrics) = evaluate_split(&attempts, &current_params, holdout_start) else {
        return Ok(skipped_summary(
            old_value,
            old_weights,
            attempts.len(),
            holdout_start,
            holdout_predictions,
            "metric_unavailable".to_owned(),
        ));
    };
    let Some(current_holdout) = current_metrics.holdout.logloss() else {
        return Ok(skipped_summary(
            old_value,
            old_weights,
            attempts.len(),
            current_metrics.train.count,
            current_metrics.holdout.count,
            "metric_unavailable".to_owned(),
        ));
    };

    let (candidate_weights, candidates_evaluated) =
        search_weights(&attempts, &current_params, holdout_start);
    let candidate_params = current_params.with_weights(candidate_weights);
    let candidate_metrics =
        evaluate_split(&attempts, &candidate_params, holdout_start).unwrap_or(current_metrics);
    let candidate_holdout = candidate_metrics
        .holdout
        .logloss()
        .unwrap_or(current_holdout);
    let improvement = current_holdout - candidate_holdout;
    let changed = !weights_equal(&current_weights, &candidate_weights);
    let accepted = changed && improvement >= config.accept_margin;
    let status = if accepted {
        FsrsFitStatus::Accepted
    } else {
        FsrsFitStatus::Rejected
    };

    Ok(FsrsFitSummary {
        param: PARAM.to_owned(),
        status,
        old_value,
        new_value: weights_json(&candidate_weights)?,
        old_weights,
        candidate_weights: candidate_weights.to_vec(),
        metric: METRIC.to_owned(),
        current_metric: Some(current_holdout),
        candidate_metric: Some(candidate_holdout),
        delta: improvement,
        accepted,
        reason: None,
        total_final_attempts: attempts.len(),
        train_predictions: current_metrics.train.count,
        holdout_predictions: current_metrics.holdout.count,
        candidates_evaluated,
        replayed_concepts: 0,
    })
}

pub(crate) fn apply_fsrs_fit_summary(conn: &Connection, summary: &FsrsFitSummary) -> Result<()> {
    if summary.status == FsrsFitStatus::Skipped {
        return Ok(());
    }
    if summary.accepted {
        let updated = conn.execute(
            "UPDATE meta SET value=?1 WHERE key='fsrs.w' AND value=?2",
            [summary.new_value.as_str(), summary.old_value.as_str()],
        )?;
        if updated != 1 {
            return Err(PolarisError::InvalidParameter {
                key: PARAM.to_owned(),
                value: "changed during fsrs fit".to_owned(),
            });
        }
    }
    write_audit_row(conn, summary)?;
    Ok(())
}

fn skipped_summary(
    old_value: String,
    old_weights: Vec<f64>,
    total_final_attempts: usize,
    train_predictions: usize,
    holdout_predictions: usize,
    reason: String,
) -> FsrsFitSummary {
    FsrsFitSummary {
        param: PARAM.to_owned(),
        status: FsrsFitStatus::Skipped,
        old_value: old_value.clone(),
        new_value: old_value,
        old_weights: old_weights.clone(),
        candidate_weights: old_weights,
        metric: METRIC.to_owned(),
        current_metric: None,
        candidate_metric: None,
        delta: 0.0,
        accepted: false,
        reason: Some(reason),
        total_final_attempts,
        train_predictions,
        holdout_predictions,
        candidates_evaluated: 0,
        replayed_concepts: 0,
    }
}

fn search_weights(
    attempts: &[FinalAttempt],
    base_params: &FsrsParams,
    holdout_start: usize,
) -> ([f64; 17], usize) {
    let mut best_weights = *base_params.weights();
    let mut candidates_evaluated = 1;
    let Some(mut best_metric) = train_logloss(
        attempts,
        &base_params.with_weights(best_weights),
        holdout_start,
    ) else {
        return (best_weights, candidates_evaluated);
    };
    let default_weights = *FsrsParams::defaults().weights();

    for _ in 0..SEARCH_PASSES {
        let mut changed = false;
        for idx in 0..best_weights.len() {
            let current = best_weights[idx];
            let mut values = SEARCH_FACTORS
                .iter()
                .map(|factor| sanitize_weight(current * factor))
                .chain([
                    sanitize_weight((current + default_weights[idx]) / 2.0),
                    sanitize_weight(default_weights[idx]),
                ])
                .collect::<Vec<_>>();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            values.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

            for value in values {
                if (value - current).abs() < 1e-12 {
                    continue;
                }
                let mut candidate = best_weights;
                candidate[idx] = value;
                let params = base_params.with_weights(candidate);
                let Some(metric) = train_logloss(attempts, &params, holdout_start) else {
                    continue;
                };
                candidates_evaluated += 1;
                if metric + 1e-12 < best_metric {
                    best_metric = metric;
                    best_weights = candidate;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    (best_weights, candidates_evaluated)
}

fn train_logloss(
    attempts: &[FinalAttempt],
    params: &FsrsParams,
    holdout_start: usize,
) -> Option<f64> {
    evaluate_split(attempts, params, holdout_start)?
        .train
        .logloss()
}

fn evaluate_split(
    attempts: &[FinalAttempt],
    params: &FsrsParams,
    holdout_start: usize,
) -> Option<SplitMetrics> {
    let mut states = BTreeMap::new();
    let mut prediction_index = 0usize;
    let mut metrics = SplitMetrics {
        train: MetricStats::default(),
        holdout: MetricStats::default(),
    };

    for attempt in attempts {
        let entry = states
            .entry(attempt.concept_id.clone())
            .or_insert_with(|| (create_initial_state_with_params(params), None));
        let elapsed_days = entry
            .1
            .map(|previous| f64::max(0.0, attempt.day - previous))
            .unwrap_or(0.0);
        if entry.0.reps > 0 {
            let prediction = retrievability(entry.0.stability, elapsed_days);
            if !prediction.is_finite() {
                return None;
            }
            let rating = Rating::from_score_with_params(attempt.score, params);
            let outcome = if rating == Rating::Again { 0.0 } else { 1.0 };
            if prediction_index < holdout_start {
                metrics.train.push(prediction, outcome);
            } else {
                metrics.holdout.push(prediction, outcome);
            }
            prediction_index += 1;
        }

        let rating = Rating::from_score_with_params(attempt.score, params);
        let update = update_state_with_params(&entry.0, rating, elapsed_days, params);
        if !update.state.stability.is_finite()
            || update.state.stability <= 0.0
            || !update.state.difficulty.is_finite()
        {
            return None;
        }
        entry.0 = update.state;
        entry.1 = Some(attempt.day);
    }

    Some(metrics)
}

fn prediction_count(attempts: &[FinalAttempt], params: &FsrsParams) -> Option<usize> {
    let metrics = evaluate_split(attempts, params, usize::MAX)?;
    Some(metrics.train.count)
}

fn holdout_start_index(total: usize, holdout_frac: f64) -> usize {
    let holdout = ((total as f64) * holdout_frac).ceil() as usize;
    total.saturating_sub(holdout.max(1))
}

fn sanitize_weight(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(MIN_WEIGHT, MAX_WEIGHT)
    } else {
        MIN_WEIGHT
    }
}

fn weights_equal(left: &[f64; 17], right: &[f64; 17]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| (*left - *right).abs() < 1e-12)
}

fn weights_json(weights: &[f64; 17]) -> Result<String> {
    serde_json::to_string(weights).map_err(Into::into)
}

fn write_audit_row(conn: &Connection, summary: &FsrsFitSummary) -> Result<()> {
    conn.execute(
        "INSERT INTO param_tuning_runs(id, ran_at, param, old_value, new_value, metric, delta, status)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Uuid::new_v4().to_string(),
            summary.param,
            summary.old_value,
            summary.new_value,
            summary.metric,
            summary.delta,
            if summary.accepted { "accepted" } else { "rejected" },
        ],
    )?;
    Ok(())
}

fn load_final_attempts(conn: &Connection) -> Result<Vec<FinalAttempt>> {
    let mut stmt = conn.prepare(
        "SELECT concept_id, final_score,
                COALESCE(julianday(created_at), julianday('1970-01-01T00:00:00Z'))
         FROM attempts
         WHERE final_score IS NOT NULL
         ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FinalAttempt {
                concept_id: row.get(0)?,
                score: row.get::<_, f64>(1)?.clamp(0.0, 1.0),
                day: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::config::{default_registry, ParameterClass, TuningRoute};
    use crate::db::migrate;

    use super::{
        apply_fsrs_fit_summary, evaluate_fsrs_personal_params, FsrsFitStatus, FsrsFitSummary,
    };

    #[test]
    fn fsrs_w_is_class_c_fit_not_b_replay() {
        let registry = default_registry();
        let spec = registry.get("fsrs.w").expect("fsrs.w");
        assert_eq!(spec.class, ParameterClass::C);
        assert_eq!(spec.tuning_route, TuningRoute::Fit);
    }

    #[test]
    fn skipped_fit_summary_application_is_noop() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let before: String = conn
            .query_row("SELECT value FROM meta WHERE key='fsrs.w'", [], |row| {
                row.get(0)
            })
            .unwrap();

        let summary = evaluate_fsrs_personal_params(&conn).unwrap();
        apply_fsrs_fit_summary(&conn, &summary).unwrap();

        assert_eq!(summary.status, FsrsFitStatus::Skipped);
        let after: String = conn
            .query_row("SELECT value FROM meta WHERE key='fsrs.w'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let audit_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM param_tuning_runs WHERE param='fsrs.w'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after, before);
        assert_eq!(audit_count, 0);
    }

    #[test]
    fn accepted_fit_application_rejects_stale_fsrs_w_snapshot() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let old_value: String = conn
            .query_row("SELECT value FROM meta WHERE key='fsrs.w'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let old_weights: Vec<f64> = serde_json::from_str(&old_value).unwrap();
        let mut candidate_weights = old_weights.clone();
        candidate_weights[2] += 0.5;
        let new_value = serde_json::to_string(&candidate_weights).unwrap();
        let mut changed_weights = old_weights.clone();
        changed_weights[2] += 1.0;
        conn.execute(
            "UPDATE meta SET value=?1 WHERE key='fsrs.w'",
            [serde_json::to_string(&changed_weights).unwrap()],
        )
        .unwrap();

        let summary = FsrsFitSummary {
            param: super::PARAM.to_owned(),
            status: FsrsFitStatus::Accepted,
            old_value,
            new_value,
            old_weights,
            candidate_weights,
            metric: super::METRIC.to_owned(),
            current_metric: Some(1.0),
            candidate_metric: Some(0.9),
            delta: 0.1,
            accepted: true,
            reason: None,
            total_final_attempts: 100,
            train_predictions: 80,
            holdout_predictions: 20,
            candidates_evaluated: 2,
            replayed_concepts: 0,
        };

        let error = apply_fsrs_fit_summary(&conn, &summary).unwrap_err();

        assert!(error.to_string().contains("changed during fsrs fit"));
        let audit_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM param_tuning_runs WHERE param='fsrs.w'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audit_count, 0);
    }
}
