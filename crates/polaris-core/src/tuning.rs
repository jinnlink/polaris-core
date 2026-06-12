use std::collections::BTreeMap;

use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::config::{default_registry, meta_f64, meta_i64, ParameterClass, TuningRoute};
use crate::error::{PolarisError, Result};
use crate::mastery::{fold_attempt, AttemptObservation, MasteryParams, MasteryState};

const MIN_HOLDOUT_OUTCOMES: usize = 5;
const PROBABILITY_EPSILON: f64 = 1e-6;
const BKT_METRIC: &str = "bkt_holdout_logloss";
const PROVISIONAL_METRIC: &str = "provisional_holdout_mae";

/// 轮转槽位（白名单）。全部满足 class=B ∧ route=Replay，由单测锁死。
const BKT_SLOT_KEYS: [&str; 5] = [
    "bkt.p_init",
    "bkt.slip",
    "bkt.guess",
    "bkt.guess_explain",
    "bkt.learn",
];
const PROVISIONAL_SLOT: &str = "grade.provisional";
const SLOT_COUNT: usize = BKT_SLOT_KEYS.len() + 1;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TuningOutcome {
    pub param: String,
    pub old_value: String,
    pub new_value: String,
    pub metric: String,
    pub delta: f64,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TuningSummary {
    pub outcomes: Vec<TuningOutcome>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone)]
struct GradedAttempt {
    id: String,
    concept_id: String,
    task_type: String,
    score: f64,
    self_confidence: i32,
    day: f64,
    created_at: String,
}

#[derive(Debug, Clone, Copy)]
struct ConfidenceFinalPair {
    conf_norm: f64,
    final_score: f64,
}

pub fn run_param_tuning(conn: &Connection) -> Result<TuningSummary> {
    let min_attempts = meta_i64(conn, "tuning.min_attempts")?.max(1) as usize;
    let holdout_frac = meta_f64(conn, "tuning.holdout_frac")?.clamp(0.05, 0.5);
    let accept_margin = meta_f64(conn, "tuning.accept_margin")?;
    let max_params = meta_i64(conn, "tuning.max_params_per_run")?.max(1) as usize;

    let mut summary = TuningSummary {
        outcomes: Vec::new(),
        skipped: Vec::new(),
    };

    let attempts = load_graded_attempts(conn)?;
    if attempts.len() < min_attempts {
        summary.skipped.push(format!(
            "all:insufficient_history({}<{min_attempts})",
            attempts.len()
        ));
        return Ok(summary);
    }

    let p_init_overrides = load_p_init_overrides(conn)?;
    let pairs = load_confidence_final_pairs(conn)?;

    let mut cursor =
        meta_i64(conn, "tuning.rotation_cursor")?.rem_euclid(SLOT_COUNT as i64) as usize;
    let mut budget = max_params;
    let mut visited = 0usize;

    while visited < SLOT_COUNT && budget > 0 {
        let slot = cursor % SLOT_COUNT;
        let is_pair = slot == BKT_SLOT_KEYS.len();
        let cost = if is_pair { 2 } else { 1 };
        if cost > budget {
            summary
                .skipped
                .push(format!("{}:budget_exhausted", slot_name(slot)));
            break;
        }

        if is_pair {
            match tune_provisional_pair(conn, &pairs, min_attempts, holdout_frac, accept_margin)? {
                Some(outcomes) => {
                    budget -= cost;
                    summary.outcomes.extend(outcomes);
                }
                None => summary
                    .skipped
                    .push(format!("{PROVISIONAL_SLOT}:metric_unavailable")),
            }
        } else {
            let key = BKT_SLOT_KEYS[slot];
            match tune_bkt_param(
                conn,
                key,
                &attempts,
                &p_init_overrides,
                holdout_frac,
                accept_margin,
            )? {
                Some(outcome) => {
                    budget -= cost;
                    summary.outcomes.push(outcome);
                }
                None => summary.skipped.push(format!("{key}:metric_unavailable")),
            }
        }

        cursor += 1;
        visited += 1;
    }

    conn.execute(
        "UPDATE meta SET value=?1 WHERE key='tuning.rotation_cursor'",
        [(cursor % SLOT_COUNT).to_string()],
    )?;
    Ok(summary)
}

fn slot_name(slot: usize) -> &'static str {
    if slot < BKT_SLOT_KEYS.len() {
        BKT_SLOT_KEYS[slot]
    } else {
        PROVISIONAL_SLOT
    }
}

// ---------------------------------------------------------------------------
// bkt.* 三点网格重放
// ---------------------------------------------------------------------------

fn tune_bkt_param(
    conn: &Connection,
    key: &str,
    attempts: &[GradedAttempt],
    p_init_overrides: &BTreeMap<String, f64>,
    holdout_frac: f64,
    accept_margin: f64,
) -> Result<Option<TuningOutcome>> {
    let holdout_start = holdout_start_index(attempts.len(), holdout_frac);
    let base_params = MasteryParams::from_conn(conn)?;
    let current = bkt_param_value(&base_params, key);

    let Some(current_metric) =
        bkt_holdout_logloss(attempts, p_init_overrides, &base_params, holdout_start)
    else {
        return Ok(None);
    };

    let (lo, hi) = registry_bounds(key)?;
    let step = (hi - lo) / 8.0;
    let mut candidates = vec![
        (current - step).clamp(lo, hi),
        current,
        (current + step).clamp(lo, hi),
    ];
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    candidates.dedup();

    let mut best_value = current;
    let mut best_metric = current_metric;
    for candidate in candidates {
        if candidate == current {
            continue;
        }
        let candidate_params = with_bkt_override(&base_params, key, candidate);
        let Some(metric) =
            bkt_holdout_logloss(attempts, p_init_overrides, &candidate_params, holdout_start)
        else {
            continue;
        };
        if metric < best_metric {
            best_metric = metric;
            best_value = candidate;
        }
    }

    let improvement = current_metric - best_metric;
    let accepted = best_value != current && improvement >= accept_margin;
    if accepted {
        conn.execute(
            "UPDATE meta SET value=?1 WHERE key=?2",
            params![best_value.to_string(), key],
        )?;
    }
    write_audit_row(
        conn,
        key,
        &current.to_string(),
        &best_value.to_string(),
        BKT_METRIC,
        improvement,
        accepted,
    )?;
    Ok(Some(TuningOutcome {
        param: key.to_owned(),
        old_value: current.to_string(),
        new_value: best_value.to_string(),
        metric: BKT_METRIC.to_owned(),
        delta: improvement,
        accepted,
    }))
}

/// prequential 重放：时间序逐条 fold，留出段 fold 前先预测 P(正确) 并对二值结果计 logloss。
fn bkt_holdout_logloss(
    attempts: &[GradedAttempt],
    p_init_overrides: &BTreeMap<String, f64>,
    params: &MasteryParams,
    holdout_start: usize,
) -> Option<f64> {
    let mut states: BTreeMap<String, (MasteryState, Option<f64>)> = BTreeMap::new();
    let mut losses = Vec::new();

    for (idx, attempt) in attempts.iter().enumerate() {
        let entry = states.entry(attempt.concept_id.clone()).or_insert_with(|| {
            let p_init = p_init_overrides
                .get(&attempt.concept_id)
                .copied()
                .unwrap_or(params.bkt_p_init);
            (MasteryState::initial_with_params(p_init, params), None)
        });

        if idx >= holdout_start {
            let guess = if attempt.task_type == "free_explain" {
                params.bkt_guess_explain
            } else {
                params.bkt_guess
            };
            let p_correct =
                entry.0.p_known * (1.0 - params.bkt_slip) + (1.0 - entry.0.p_known) * guess;
            let outcome = if attempt.score >= params.bkt_cut_hi {
                Some(1.0)
            } else if attempt.score <= params.bkt_cut_lo {
                Some(0.0)
            } else {
                None
            };
            if let Some(y) = outcome {
                let p = p_correct.clamp(PROBABILITY_EPSILON, 1.0 - PROBABILITY_EPSILON);
                losses.push(-(y * p.ln() + (1.0 - y) * (1.0 - p).ln()));
            }
        }

        let elapsed_days = entry
            .1
            .map(|previous| (attempt.day - previous).max(0.0))
            .unwrap_or(0.0);
        let observation = AttemptObservation {
            id: attempt.id.clone(),
            task_type: attempt.task_type.clone(),
            score: attempt.score,
            self_confidence: attempt.self_confidence,
            elapsed_days,
            created_at: attempt.created_at.clone(),
            occurred_day: None,
            depth: None,
        };
        fold_attempt(&mut entry.0, &observation, params);
        entry.1 = Some(attempt.day);
    }

    if losses.len() < MIN_HOLDOUT_OUTCOMES {
        return None;
    }
    Some(losses.iter().sum::<f64>() / losses.len() as f64)
}

fn bkt_param_value(params: &MasteryParams, key: &str) -> f64 {
    match key {
        "bkt.p_init" => params.bkt_p_init,
        "bkt.slip" => params.bkt_slip,
        "bkt.guess" => params.bkt_guess,
        "bkt.guess_explain" => params.bkt_guess_explain,
        "bkt.learn" => params.bkt_learn,
        other => unreachable!("unknown bkt tuning key {other}"),
    }
}

fn with_bkt_override(base: &MasteryParams, key: &str, value: f64) -> MasteryParams {
    let mut params = base.clone();
    match key {
        "bkt.p_init" => params.bkt_p_init = value,
        "bkt.slip" => params.bkt_slip = value,
        "bkt.guess" => params.bkt_guess = value,
        "bkt.guess_explain" => params.bkt_guess_explain = value,
        "bkt.learn" => params.bkt_learn = value,
        other => unreachable!("unknown bkt tuning key {other}"),
    }
    params
}

// ---------------------------------------------------------------------------
// grade.provisional_base/slope 直接回归（DATA_MODEL §10 登记途径）
// ---------------------------------------------------------------------------

fn tune_provisional_pair(
    conn: &Connection,
    pairs: &[ConfidenceFinalPair],
    min_attempts: usize,
    holdout_frac: f64,
    accept_margin: f64,
) -> Result<Option<Vec<TuningOutcome>>> {
    if pairs.len() < min_attempts {
        return Ok(None);
    }
    let holdout_start = holdout_start_index(pairs.len(), holdout_frac);
    let (train, holdout) = pairs.split_at(holdout_start);
    if train.is_empty() || holdout.len() < MIN_HOLDOUT_OUTCOMES {
        return Ok(None);
    }

    let current_base = meta_f64(conn, "grade.provisional_base")?;
    let current_slope = meta_f64(conn, "grade.provisional_slope")?;
    let (fitted_base, fitted_slope) = least_squares(train);

    let current_mae = provisional_mae(holdout, current_base, current_slope);
    let fitted_mae = provisional_mae(holdout, fitted_base, fitted_slope);
    let improvement = current_mae - fitted_mae;
    let changed = fitted_base != current_base || fitted_slope != current_slope;
    let accepted = changed && improvement >= accept_margin;

    if accepted {
        conn.execute(
            "UPDATE meta SET value=?1 WHERE key='grade.provisional_base'",
            [fitted_base.to_string()],
        )?;
        conn.execute(
            "UPDATE meta SET value=?1 WHERE key='grade.provisional_slope'",
            [fitted_slope.to_string()],
        )?;
    }

    let mut outcomes = Vec::new();
    for (key, old_value, new_value) in [
        ("grade.provisional_base", current_base, fitted_base),
        ("grade.provisional_slope", current_slope, fitted_slope),
    ] {
        write_audit_row(
            conn,
            key,
            &old_value.to_string(),
            &new_value.to_string(),
            PROVISIONAL_METRIC,
            improvement,
            accepted,
        )?;
        outcomes.push(TuningOutcome {
            param: key.to_owned(),
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
            metric: PROVISIONAL_METRIC.to_owned(),
            delta: improvement,
            accepted,
        });
    }
    Ok(Some(outcomes))
}

fn least_squares(train: &[ConfidenceFinalPair]) -> (f64, f64) {
    let n = train.len() as f64;
    let mean_x = train.iter().map(|pair| pair.conf_norm).sum::<f64>() / n;
    let mean_y = train.iter().map(|pair| pair.final_score).sum::<f64>() / n;
    let covariance = train
        .iter()
        .map(|pair| (pair.conf_norm - mean_x) * (pair.final_score - mean_y))
        .sum::<f64>();
    let variance = train
        .iter()
        .map(|pair| (pair.conf_norm - mean_x).powi(2))
        .sum::<f64>();

    let slope = if variance < 1e-9 {
        0.0
    } else {
        covariance / variance
    };
    let slope = slope.clamp(0.0, 1.0);
    let base = (mean_y - slope * mean_x).clamp(0.0, 1.0);
    (base, slope)
}

fn provisional_mae(holdout: &[ConfidenceFinalPair], base: f64, slope: f64) -> f64 {
    let total = holdout
        .iter()
        .map(|pair| ((base + slope * pair.conf_norm) - pair.final_score).abs())
        .sum::<f64>();
    total / holdout.len() as f64
}

// ---------------------------------------------------------------------------
// 公共辅助
// ---------------------------------------------------------------------------

fn holdout_start_index(total: usize, holdout_frac: f64) -> usize {
    let holdout = ((total as f64) * holdout_frac).ceil() as usize;
    total.saturating_sub(holdout.max(1))
}

fn registry_bounds(key: &str) -> Result<(f64, f64)> {
    let registry = default_registry();
    let spec = registry
        .get(key)
        .ok_or_else(|| PolarisError::InvalidParameter {
            key: key.to_owned(),
            value: "<unregistered>".to_owned(),
        })?;
    debug_assert_eq!(spec.class, ParameterClass::B);
    debug_assert_eq!(spec.tuning_route, TuningRoute::Replay);
    let bounds = spec.bounds.ok_or_else(|| PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: "<no bounds>".to_owned(),
    })?;
    parse_bounds(bounds).ok_or_else(|| PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: bounds.to_owned(),
    })
}

fn parse_bounds(bounds: &str) -> Option<(f64, f64)> {
    let trimmed = bounds.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut parts = trimmed.split(',');
    let lo = parts.next()?.trim().parse::<f64>().ok()?;
    let hi = parts.next()?.trim().parse::<f64>().ok()?;
    if parts.next().is_some() || lo >= hi {
        return None;
    }
    Some((lo, hi))
}

fn write_audit_row(
    conn: &Connection,
    param: &str,
    old_value: &str,
    new_value: &str,
    metric: &str,
    delta: f64,
    accepted: bool,
) -> Result<()> {
    conn.execute(
        "INSERT INTO param_tuning_runs(id, ran_at, param, old_value, new_value, metric, delta, status)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Uuid::new_v4().to_string(),
            param,
            old_value,
            new_value,
            metric,
            delta,
            if accepted { "accepted" } else { "rejected" },
        ],
    )?;
    Ok(())
}

fn load_graded_attempts(conn: &Connection) -> Result<Vec<GradedAttempt>> {
    let mut stmt = conn.prepare(
        "SELECT id, concept_id, COALESCE(task_type, 'recall'),
                COALESCE(final_score, provisional_score),
                COALESCE(self_confidence, 3),
                julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')),
                COALESCE(created_at, '1970-01-01T00:00:00Z')
         FROM attempts
         WHERE COALESCE(final_score, provisional_score) IS NOT NULL
         ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(GradedAttempt {
                id: row.get(0)?,
                concept_id: row.get(1)?,
                task_type: row.get(2)?,
                score: row.get(3)?,
                self_confidence: row.get(4)?,
                day: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_confidence_final_pairs(conn: &Connection) -> Result<Vec<ConfidenceFinalPair>> {
    let mut stmt = conn.prepare(
        "SELECT (self_confidence - 1.0) / 4.0, final_score
         FROM attempts
         WHERE final_score IS NOT NULL AND self_confidence IS NOT NULL
         ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConfidenceFinalPair {
                conf_norm: row.get::<_, f64>(0)?.clamp(0.0, 1.0),
                final_score: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_p_init_overrides(conn: &Connection) -> Result<BTreeMap<String, f64>> {
    let mut stmt = conn.prepare("SELECT id, p_init FROM concepts WHERE p_init IS NOT NULL")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_registry, ParameterClass, TuningRoute};

    #[test]
    fn whitelist_slots_are_class_b_replay_route() {
        let registry = default_registry();
        for key in BKT_SLOT_KEYS
            .iter()
            .chain(["grade.provisional_base", "grade.provisional_slope"].iter())
        {
            let spec = registry
                .get(*key)
                .unwrap_or_else(|| panic!("missing {key}"));
            assert_eq!(spec.class, ParameterClass::B, "{key} must be class B");
            assert_eq!(
                spec.tuning_route,
                TuningRoute::Replay,
                "{key} must be replay route"
            );
        }
    }

    #[test]
    fn whitelist_excludes_gate_manual_and_mrt_params() {
        let forbidden = [
            "bkt.cut_hi",
            "bkt.cut_lo",
            "sched.w_r",
            "friction.w1",
            "friction.lambda",
            "fsrs.r_again",
            "hazard.auc_gate",
            "tuning.accept_margin",
            "tuning.min_attempts",
        ];
        for key in forbidden {
            assert!(!BKT_SLOT_KEYS.contains(&key), "{key} must not be tunable");
        }
    }

    #[test]
    fn parse_bounds_handles_registry_format() {
        assert_eq!(parse_bounds("[0.05,0.50]"), Some((0.05, 0.50)));
        assert_eq!(parse_bounds("[0,0.20]"), Some((0.0, 0.20)));
        assert_eq!(parse_bounds("simplex"), None);
        assert_eq!(parse_bounds("[1,1]"), None);
    }

    #[test]
    fn holdout_start_keeps_last_fraction() {
        assert_eq!(holdout_start_index(30, 0.20), 24);
        assert_eq!(holdout_start_index(10, 0.20), 8);
        assert_eq!(holdout_start_index(3, 0.20), 2);
    }

    #[test]
    fn least_squares_recovers_line_and_clamps() {
        let train = [
            ConfidenceFinalPair {
                conf_norm: 0.25,
                final_score: 0.2,
            },
            ConfidenceFinalPair {
                conf_norm: 0.5,
                final_score: 0.5,
            },
            ConfidenceFinalPair {
                conf_norm: 0.75,
                final_score: 0.8,
            },
        ];
        let (base, slope) = least_squares(&train);
        assert_eq!(slope, 1.0, "raw slope 1.2 clamps to 1.0");
        assert!((base - 0.0).abs() < 1e-12, "raw base -0.1 clamps to 0.0");

        let flat = [
            ConfidenceFinalPair {
                conf_norm: 0.5,
                final_score: 0.9,
            },
            ConfidenceFinalPair {
                conf_norm: 0.5,
                final_score: 0.7,
            },
        ];
        let (base, slope) = least_squares(&flat);
        assert_eq!(slope, 0.0, "zero variance degrades to slope 0");
        assert!((base - 0.8).abs() < 1e-12);
    }
}
