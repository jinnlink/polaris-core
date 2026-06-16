use std::collections::BTreeSet;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::{meta_f64, meta_i64};
use crate::error::Result;

const PROBABILITY_EPSILON: f64 = 1e-12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuPriorShadowStatus {
    NoData,
    InsufficientData,
    ShadowReady,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuPriorShadowSummary {
    pub status: GuPriorShadowStatus,
    pub rules_evaluated: usize,
    pub holdout_attempt_count: usize,
    pub rows: Vec<GuPriorShadowRow>,
    pub validation: GuPriorValidation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuPriorShadowRow {
    pub rule_id: String,
    pub pattern: String,
    pub concept_ids: Vec<String>,
    pub source_attempt_count: usize,
    pub holdout_attempt_count: usize,
    pub flat_prior_alpha: f64,
    pub flat_prior_beta: f64,
    pub hierarchical_prior_alpha: f64,
    pub hierarchical_prior_beta: f64,
    pub flat_logloss: f64,
    pub hierarchical_logloss: f64,
    pub flat_brier: f64,
    pub hierarchical_brier: f64,
    pub flat_accuracy: f64,
    pub hierarchical_accuracy: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuPriorValidationStatus {
    Skipped,
    Computed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuPriorValidation {
    pub status: GuPriorValidationStatus,
    pub reason: Option<String>,
    pub passed: Option<bool>,
    pub flat_logloss: Option<f64>,
    pub hierarchical_logloss: Option<f64>,
    pub flat_brier: Option<f64>,
    pub hierarchical_brier: Option<f64>,
    pub flat_accuracy: Option<f64>,
    pub hierarchical_accuracy: Option<f64>,
}

impl GuPriorValidation {
    fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: GuPriorValidationStatus::Skipped,
            reason: Some(reason.into()),
            passed: None,
            flat_logloss: None,
            hierarchical_logloss: None,
            flat_brier: None,
            hierarchical_brier: None,
            flat_accuracy: None,
            hierarchical_accuracy: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuRule {
    id: String,
    pattern: String,
    concept_ids: Vec<String>,
    last_seen: String,
}

#[derive(Debug, Clone, PartialEq)]
struct AttemptLabel {
    label: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GuPriorParams {
    min_shadow_rules: usize,
    min_holdout_attempts: usize,
    max_prior_strength: f64,
    window_days: i64,
}

#[derive(Debug, Deserialize)]
struct GraderJson {
    #[serde(default)]
    pattern_tags: Vec<String>,
}

pub fn gu_prior_shadow_summary(conn: &Connection) -> Result<GuPriorShadowSummary> {
    let params = GuPriorParams::from_conn(conn)?;
    let rules = load_rules(conn)?;
    if rules.is_empty() {
        return Ok(GuPriorShadowSummary {
            status: GuPriorShadowStatus::NoData,
            rules_evaluated: 0,
            holdout_attempt_count: 0,
            rows: Vec::new(),
            validation: GuPriorValidation::skipped("no_gu_rules"),
        });
    }

    let mut rows = Vec::new();
    for rule in &rules {
        let (hierarchical_alpha, hierarchical_beta, source_attempt_count) =
            hierarchical_prior(conn, rule, &rules, params.max_prior_strength)?;
        let holdout = holdout_attempts(conn, rule, params.window_days)?;
        let metrics = sequential_metrics(&holdout, 1.0, 1.0);
        let hierarchical_metrics =
            sequential_metrics(&holdout, hierarchical_alpha, hierarchical_beta);

        rows.push(GuPriorShadowRow {
            rule_id: rule.id.clone(),
            pattern: rule.pattern.clone(),
            concept_ids: rule.concept_ids.clone(),
            source_attempt_count,
            holdout_attempt_count: holdout.len(),
            flat_prior_alpha: 1.0,
            flat_prior_beta: 1.0,
            hierarchical_prior_alpha: hierarchical_alpha,
            hierarchical_prior_beta: hierarchical_beta,
            flat_logloss: metrics.logloss,
            hierarchical_logloss: hierarchical_metrics.logloss,
            flat_brier: metrics.brier,
            hierarchical_brier: hierarchical_metrics.brier,
            flat_accuracy: metrics.accuracy,
            hierarchical_accuracy: hierarchical_metrics.accuracy,
        });
    }

    let rules_evaluated = rows
        .iter()
        .filter(|row| row.holdout_attempt_count > 0)
        .count();
    let holdout_attempt_count = rows.iter().map(|row| row.holdout_attempt_count).sum();
    let status = if rules_evaluated < params.min_shadow_rules
        || holdout_attempt_count < params.min_holdout_attempts
    {
        GuPriorShadowStatus::InsufficientData
    } else {
        GuPriorShadowStatus::ShadowReady
    };
    let validation = if status == GuPriorShadowStatus::ShadowReady {
        aggregate_validation(&rows)
    } else {
        GuPriorValidation::skipped(format!(
            "insufficient_data(rules={}/{},holdout={}/{})",
            rules_evaluated,
            params.min_shadow_rules,
            holdout_attempt_count,
            params.min_holdout_attempts
        ))
    };

    Ok(GuPriorShadowSummary {
        status,
        rules_evaluated,
        holdout_attempt_count,
        rows,
        validation,
    })
}

impl GuPriorParams {
    fn from_conn(conn: &Connection) -> Result<Self> {
        Ok(Self {
            min_shadow_rules: meta_i64(conn, "gu_prior.min_shadow_rules")?.clamp(1, 1000) as usize,
            min_holdout_attempts: meta_i64(conn, "gu_prior.min_holdout_attempts")?.clamp(1, 10000)
                as usize,
            max_prior_strength: meta_f64(conn, "gu_prior.max_prior_strength")?.clamp(0.0, 1000.0),
            window_days: meta_i64(conn, "gu.window_days")?.max(1),
        })
    }
}

fn load_rules(conn: &Connection) -> Result<Vec<GuRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, concept_ids_json, COALESCE(last_seen, '1970-01-01T00:00:00Z')
         FROM gu_rules
         ORDER BY COALESCE(last_seen, '1970-01-01T00:00:00Z') ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let concept_ids_json: String = row.get(2)?;
        Ok(GuRule {
            id: row.get(0)?,
            pattern: row.get(1)?,
            concept_ids: sorted_unique(serde_json::from_str(&concept_ids_json).unwrap_or_default()),
            last_seen: row.get(3)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn hierarchical_prior(
    conn: &Connection,
    target: &GuRule,
    rules: &[GuRule],
    max_prior_strength: f64,
) -> Result<(f64, f64, usize)> {
    let source_concepts = source_concepts(conn, target, rules)?;
    let evidence = source_evidence(conn, target, &source_concepts)?;
    let total = evidence.len();
    if total == 0 {
        return Ok((1.0, 1.0, 0));
    }

    let hits = evidence.iter().filter(|attempt| attempt.label).count();
    let rate = hits as f64 / total as f64;
    let strength = max_prior_strength.min(total as f64);
    Ok((1.0 + strength * rate, 1.0 + strength * (1.0 - rate), total))
}

fn source_concepts(conn: &Connection, target: &GuRule, rules: &[GuRule]) -> Result<Vec<String>> {
    let target_concepts = target.concept_ids.iter().cloned().collect::<BTreeSet<_>>();
    let neighbors = one_hop_neighbors(conn, &target_concepts, &target.last_seen)?;
    let mut same_pattern_concepts = BTreeSet::new();
    for rule in rules {
        if rule.id == target.id
            || rule.pattern != target.pattern
            || rule.last_seen >= target.last_seen
        {
            continue;
        }
        for concept_id in &rule.concept_ids {
            same_pattern_concepts.insert(concept_id.clone());
        }
    }

    let mut source_concepts = BTreeSet::new();
    for concept_id in neighbors.into_iter().chain(same_pattern_concepts) {
        if target_concepts.contains(&concept_id)
            || !concept_exists_at(conn, &concept_id, &target.last_seen)?
        {
            continue;
        }
        source_concepts.insert(concept_id);
    }
    Ok(source_concepts.into_iter().collect())
}

fn one_hop_neighbors(
    conn: &Connection,
    concept_ids: &BTreeSet<String>,
    cutoff: &str,
) -> Result<BTreeSet<String>> {
    if concept_ids.is_empty() {
        return Ok(BTreeSet::new());
    }
    let concept_ids_json = serde_json::to_string(&concept_ids.iter().collect::<Vec<_>>())?;
    let mut stmt = conn.prepare(
        "SELECT src, dst
         FROM edges
         WHERE julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) < julianday(?2)
           AND (
             EXISTS (SELECT 1 FROM json_each(?1) WHERE value=edges.src)
             OR EXISTS (SELECT 1 FROM json_each(?1) WHERE value=edges.dst)
           )",
    )?;
    let rows = stmt.query_map(params![concept_ids_json, cutoff], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut neighbors = BTreeSet::new();
    for row in rows {
        let (src, dst) = row?;
        if concept_ids.contains(&src) && !concept_ids.contains(&dst) {
            if concept_exists_at(conn, &dst, cutoff)? {
                neighbors.insert(dst);
            }
        } else if concept_ids.contains(&dst)
            && !concept_ids.contains(&src)
            && concept_exists_at(conn, &src, cutoff)?
        {
            neighbors.insert(src);
        }
    }
    Ok(neighbors)
}

fn concept_exists_at(conn: &Connection, concept_id: &str, cutoff: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM concepts
         WHERE id=?1
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) < julianday(?2)",
        params![concept_id, cutoff],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn source_evidence(
    conn: &Connection,
    target: &GuRule,
    source_concepts: &[String],
) -> Result<Vec<AttemptLabel>> {
    if source_concepts.is_empty() {
        return Ok(Vec::new());
    }
    let concepts_json = serde_json::to_string(source_concepts)?;
    let mut stmt = conn.prepare(
        "SELECT grader_json
         FROM attempts
         WHERE final_score IS NOT NULL
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) < julianday(?1)
           AND julianday(COALESCE(graded_at, created_at, '1970-01-01T00:00:00Z')) < julianday(?1)
           AND EXISTS (SELECT 1 FROM json_each(?2) WHERE value=attempts.concept_id)
         ORDER BY COALESCE(created_at, '1970-01-01T00:00:00Z') ASC, id ASC",
    )?;
    attempt_labels_from_rows(
        &mut stmt,
        params![target.last_seen, concepts_json],
        &target.pattern,
    )
}

fn holdout_attempts(
    conn: &Connection,
    rule: &GuRule,
    window_days: i64,
) -> Result<Vec<AttemptLabel>> {
    if rule.concept_ids.is_empty() {
        return Ok(Vec::new());
    }
    let concepts_json = serde_json::to_string(&rule.concept_ids)?;
    let mut stmt = conn.prepare(
        "SELECT grader_json
         FROM attempts
         WHERE final_score IS NOT NULL
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) > julianday(?1)
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) <= julianday(?1, '+' || ?2 || ' days')
           AND EXISTS (SELECT 1 FROM json_each(?3) WHERE value=attempts.concept_id)
         ORDER BY COALESCE(created_at, '1970-01-01T00:00:00Z') ASC, id ASC",
    )?;
    attempt_labels_from_rows(
        &mut stmt,
        params![rule.last_seen, window_days, concepts_json],
        &rule.pattern,
    )
}

fn attempt_labels_from_rows<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
    pattern: &str,
) -> Result<Vec<AttemptLabel>>
where
    P: rusqlite::Params,
{
    let rows = stmt.query_map(params, |row| row.get::<_, Option<String>>(0))?;
    let mut attempts = Vec::new();
    for row in rows {
        let grader_json = row?;
        attempts.push(AttemptLabel {
            label: grader_json
                .as_deref()
                .map(|json| {
                    pattern_tags_from_json(json)
                        .iter()
                        .any(|tag| tag == pattern)
                })
                .unwrap_or(false),
        });
    }
    Ok(attempts)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PredictiveMetrics {
    logloss: f64,
    brier: f64,
    accuracy: f64,
}

fn sequential_metrics(
    attempts: &[AttemptLabel],
    mut alpha: f64,
    mut beta: f64,
) -> PredictiveMetrics {
    if attempts.is_empty() {
        return PredictiveMetrics {
            logloss: 0.0,
            brier: 0.0,
            accuracy: 0.0,
        };
    }

    let mut logloss = 0.0;
    let mut brier = 0.0;
    let mut hits = 0usize;
    for attempt in attempts {
        let probability = bounded_probability(alpha / (alpha + beta));
        let y = if attempt.label { 1.0 } else { 0.0 };
        logloss += if attempt.label {
            -probability.ln()
        } else {
            -(1.0 - probability).ln()
        };
        brier += (probability - y) * (probability - y);
        if (probability >= 0.5) == attempt.label {
            hits += 1;
        }
        if attempt.label {
            alpha += 1.0;
        } else {
            beta += 1.0;
        }
    }

    let n = attempts.len() as f64;
    PredictiveMetrics {
        logloss: logloss / n,
        brier: brier / n,
        accuracy: hits as f64 / n,
    }
}

fn aggregate_validation(rows: &[GuPriorShadowRow]) -> GuPriorValidation {
    let holdout_count = rows
        .iter()
        .map(|row| row.holdout_attempt_count)
        .sum::<usize>();
    if holdout_count == 0 {
        return GuPriorValidation::skipped("no_holdout_attempts");
    }

    let weighted = |value: fn(&GuPriorShadowRow) -> f64| -> f64 {
        rows.iter()
            .map(|row| value(row) * row.holdout_attempt_count as f64)
            .sum::<f64>()
            / holdout_count as f64
    };
    let flat_logloss = weighted(|row| row.flat_logloss);
    let hierarchical_logloss = weighted(|row| row.hierarchical_logloss);

    GuPriorValidation {
        status: GuPriorValidationStatus::Computed,
        reason: None,
        passed: Some(hierarchical_logloss <= flat_logloss + 1e-12),
        flat_logloss: Some(flat_logloss),
        hierarchical_logloss: Some(hierarchical_logloss),
        flat_brier: Some(weighted(|row| row.flat_brier)),
        hierarchical_brier: Some(weighted(|row| row.hierarchical_brier)),
        flat_accuracy: Some(weighted(|row| row.flat_accuracy)),
        hierarchical_accuracy: Some(weighted(|row| row.hierarchical_accuracy)),
    }
}

fn pattern_tags_from_json(grader_json: &str) -> Vec<String> {
    serde_json::from_str::<GraderJson>(grader_json)
        .ok()
        .map(|value| sorted_unique(value.pattern_tags))
        .unwrap_or_default()
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn bounded_probability(probability: f64) -> f64 {
    probability.clamp(PROBABILITY_EPSILON, 1.0 - PROBABILITY_EPSILON)
}
