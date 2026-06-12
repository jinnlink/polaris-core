use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::config::{meta_f64, meta_i64};
use crate::error::Result;

const VALID_PATTERNS: &[&str] = &[
    "overgeneralization",
    "boundary-blindness",
    "symbol-referent-confusion",
    "causal-inversion",
    "fluency-illusion",
    "procedural-conceptual-gap",
    "granularity-mismatch",
    "interference-confusion",
];

#[derive(Debug, Clone, PartialEq)]
pub struct GuInductionSummary {
    pub candidates_created: usize,
    pub validated: usize,
    pub activated: usize,
    pub resolved: usize,
    pub retired: usize,
    pub expired: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActiveGuRule {
    pub id: String,
    pub pattern: String,
    pub status: String,
    pub concept_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct PatternAttempt {
    id: String,
    concept_id: String,
    created_at: String,
}

#[derive(Debug, Clone)]
struct GuRuleRow {
    id: String,
    pattern: String,
    concept_ids: Vec<String>,
    attempt_ids: Vec<String>,
    last_seen: String,
    status: String,
    consumed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraderJson {
    #[serde(default)]
    pattern_tags: Vec<String>,
}

pub fn run_gu_induction(conn: &Connection) -> Result<GuInductionSummary> {
    let mut summary = GuInductionSummary {
        candidates_created: 0,
        validated: 0,
        activated: 0,
        resolved: 0,
        retired: 0,
        expired: 0,
    };

    summary.candidates_created = create_candidates(conn)?;
    summary.validated = validate_candidates(conn)?;
    summary.expired = expire_stale_candidates(conn)?;
    let (resolved, retired) = update_active_rules(conn)?;
    summary.resolved = resolved;
    summary.retired = retired;
    Ok(summary)
}

pub fn active_gu_rules_for_concept(
    conn: &Connection,
    concept_id: &str,
) -> Result<Vec<ActiveGuRule>> {
    let mut rules = gu_rules_by_status(conn, &["validated", "active"])?;
    let mut active = Vec::new();
    for rule in &mut rules {
        if !rule.concept_ids.iter().any(|id| id == concept_id) {
            continue;
        }
        if rule.status == "validated" {
            activate_rule(conn, &rule.id, &rule.last_seen, Some(concept_id))?;
            rule.status = "active".to_owned();
        }
        active.push(ActiveGuRule {
            id: rule.id.clone(),
            pattern: rule.pattern.clone(),
            status: rule.status.clone(),
            concept_ids: rule.concept_ids.clone(),
        });
    }
    Ok(active)
}

pub fn concept_has_active_gu_rule(conn: &Connection, concept_id: &str) -> Result<bool> {
    let concept_ids_json = serde_json::to_string(&vec![concept_id])?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM gu_rules
         WHERE status='active'
           AND EXISTS (
             SELECT 1 FROM json_each(gu_rules.concept_ids_json)
             WHERE value IN (SELECT value FROM json_each(?1))
           )",
        [concept_ids_json],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn active_gu_prompt_for_concept(conn: &Connection, concept_id: &str) -> Result<Option<String>> {
    let rules = active_gu_rules_for_concept(conn, concept_id)?;
    if rules.is_empty() {
        return Ok(None);
    }
    let patterns = rules
        .iter()
        .map(|rule| rule.pattern.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(format!(
        "该学习者在相关概念上反复出现 {patterns} 错误；评分时重点核查该行为模式，但不要把它当作个人特质诊断。"
    )))
}

fn create_candidates(conn: &Connection) -> Result<usize> {
    let cut_lo = meta_f64(conn, "bkt.cut_lo")?;
    let min_failures = meta_i64(conn, "gu.min_failures")?.max(3) as usize;
    let failed = failed_pattern_attempts(conn, cut_lo)?;
    let mut created = 0;

    for (pattern, attempts) in failed {
        let trigger = earliest_cross_concept_trigger(&attempts, min_failures);
        if trigger.len() < min_failures {
            continue;
        }
        let concept_ids = sorted_unique(trigger.iter().map(|attempt| attempt.concept_id.clone()));
        let attempt_ids = trigger
            .iter()
            .map(|attempt| attempt.id.clone())
            .collect::<Vec<_>>();
        if has_existing_superset(conn, &pattern, &concept_ids)? {
            continue;
        }

        let first_seen = trigger
            .iter()
            .map(|attempt| attempt.created_at.as_str())
            .min()
            .unwrap_or("1970-01-01T00:00:00Z");
        let last_seen = trigger
            .iter()
            .map(|attempt| attempt.created_at.as_str())
            .max()
            .unwrap_or(first_seen);
        let id = rule_id(&pattern, &concept_ids);
        conn.execute(
            "INSERT INTO gu_rules(id, pattern, concept_ids_json, attempt_ids_json, first_seen, last_seen,
                                  count, status, alpha, beta, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'candidate', 1.0, 1.0, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                id,
                pattern,
                serde_json::to_string(&concept_ids)?,
                serde_json::to_string(&attempt_ids)?,
                first_seen,
                last_seen,
                attempt_ids.len() as i64
            ],
        )?;
        record_lifecycle(conn, &id, "candidate", None)?;
        created += 1;
    }

    Ok(created)
}

fn validate_candidates(conn: &Connection) -> Result<usize> {
    let mut validated = 0;
    let candidates = gu_rules_by_status(conn, &["candidate"])?;
    for rule in candidates {
        let Some((alpha, beta, holdout_last_seen)) = holdout_posterior(conn, &rule)? else {
            continue;
        };
        let retire_p = meta_f64(conn, "gu.retire_p")?;
        let validate_thresh = meta_f64(conn, "gu.validate_thresh")?;
        let probability = beta_probability_ge(alpha as u64, beta as u64, retire_p);
        if probability <= validate_thresh {
            conn.execute(
                "UPDATE gu_rules SET alpha=?1, beta=?2, last_seen=?3, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?4",
                params![alpha, beta, holdout_last_seen, rule.id],
            )?;
            continue;
        }
        conn.execute(
            "UPDATE gu_rules
             SET status='validated', alpha=?1, beta=?2, last_seen=?3, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=?4",
            params![alpha, beta, holdout_last_seen, rule.id],
        )?;
        upsert_misconception_graph(conn, &rule)?;
        record_lifecycle(conn, &rule.id, "validated", None)?;
        validated += 1;
    }
    Ok(validated)
}

fn expire_stale_candidates(conn: &Connection) -> Result<usize> {
    let Some(now) = latest_attempt_time(conn)? else {
        return Ok(0);
    };
    let window_days = meta_i64(conn, "gu.window_days")?;
    let mut expired = 0;
    for rule in gu_rules_by_status(conn, &["candidate"])? {
        if days_between(&rule.last_seen, &now).unwrap_or(0) <= window_days {
            continue;
        }
        set_status(conn, &rule.id, "expired", None)?;
        expired += 1;
    }
    Ok(expired)
}

fn update_active_rules(conn: &Connection) -> Result<(usize, usize)> {
    let mut resolved = 0;
    let mut retired = 0;
    let cut_hi = meta_f64(conn, "bkt.cut_hi")?;
    let cut_lo = meta_f64(conn, "bkt.cut_lo")?;
    let resolve_n = meta_i64(conn, "gu.resolve_n")?.max(1) as usize;
    let retire_p = meta_f64(conn, "gu.retire_p")?;
    let retire_thresh = meta_f64(conn, "gu.retire_thresh")?;

    for rule in gu_rules_by_status(conn, &["validated", "active"])? {
        let since = rule.consumed_at.as_deref().unwrap_or(&rule.last_seen);
        let attempts = related_attempts_after(conn, &rule.concept_ids, since)?;
        if attempts.is_empty() {
            continue;
        }

        let mut correct_streak = 0usize;
        let mut hits = 0usize;
        let mut misses = 0usize;
        for attempt in &attempts {
            let has_pattern = attempt_has_pattern(conn, &attempt.id, &rule.pattern)?;
            let score = attempt_score(conn, &attempt.id)?;
            if has_pattern {
                hits += 1;
                correct_streak = 0;
            } else if score >= cut_hi {
                correct_streak += 1;
            } else if score < cut_lo {
                misses += 1;
                correct_streak = 0;
            }
        }

        let alpha = 1 + hits;
        let beta = 1 + misses;
        conn.execute(
            "UPDATE gu_rules SET alpha=?1, beta=?2, correct_streak=?3, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?4",
            params![alpha as f64, beta as f64, correct_streak as i64, rule.id],
        )?;

        if rule.status == "active" && correct_streak >= resolve_n {
            set_status(conn, &rule.id, "resolved", None)?;
            resolved += 1;
            continue;
        }
        let low_precision_probability = beta_probability_lt(alpha as u64, beta as u64, retire_p);
        if low_precision_probability > retire_thresh {
            set_status(conn, &rule.id, "retired", None)?;
            retired += 1;
        }
    }

    Ok((resolved, retired))
}

fn failed_pattern_attempts(
    conn: &Connection,
    cut_lo: f64,
) -> Result<BTreeMap<String, Vec<PatternAttempt>>> {
    let mut stmt = conn.prepare(
        "SELECT id, concept_id, COALESCE(created_at, '1970-01-01T00:00:00Z'), grader_json
         FROM attempts
         WHERE final_score < ?1 AND grader_json IS NOT NULL
         ORDER BY created_at ASC, id ASC",
    )?;
    let mut grouped = BTreeMap::<String, Vec<PatternAttempt>>::new();
    let rows = stmt.query_map([cut_lo], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (id, concept_id, created_at, grader_json) = row?;
        for pattern in pattern_tags_from_json(&grader_json) {
            grouped.entry(pattern).or_default().push(PatternAttempt {
                id: id.clone(),
                concept_id: concept_id.clone(),
                created_at: created_at.clone(),
            });
        }
    }
    Ok(grouped)
}

fn pattern_tags_from_json(grader_json: &str) -> Vec<String> {
    serde_json::from_str::<GraderJson>(grader_json)
        .ok()
        .map(|value| {
            sorted_unique(
                value
                    .pattern_tags
                    .into_iter()
                    .filter(|pattern| VALID_PATTERNS.contains(&pattern.as_str())),
            )
        })
        .unwrap_or_default()
}

fn earliest_cross_concept_trigger(
    attempts: &[PatternAttempt],
    min_failures: usize,
) -> Vec<PatternAttempt> {
    let mut seen_concepts = BTreeSet::new();
    let mut trigger = Vec::new();
    for attempt in attempts {
        if seen_concepts.insert(attempt.concept_id.clone()) {
            trigger.push(attempt.clone());
        }
        if trigger.len() >= min_failures {
            break;
        }
    }
    trigger
}

fn holdout_posterior(
    conn: &Connection,
    rule: &GuRuleRow,
) -> Result<Option<(usize, usize, String)>> {
    let attempts = related_attempts_after(conn, &rule.concept_ids, &rule.last_seen)?;
    if attempts.is_empty() {
        return Ok(None);
    }
    let mut hits = 0usize;
    let mut misses = 0usize;
    for attempt in &attempts {
        if attempt_has_pattern(conn, &attempt.id, &rule.pattern)? {
            hits += 1;
        } else {
            misses += 1;
        }
    }
    if hits == 0 {
        return Ok(None);
    }
    let baseline = baseline_pattern_rate(conn, &rule.pattern, &rule.concept_ids, &rule.last_seen)?;
    let rate = hits as f64 / (hits + misses) as f64;
    if rate <= baseline {
        return Ok(None);
    }
    let last_seen = attempts
        .iter()
        .map(|attempt| attempt.created_at.as_str())
        .max()
        .unwrap_or(&rule.last_seen)
        .to_owned();
    Ok(Some((1 + hits, 1 + misses, last_seen)))
}

fn related_attempts_after(
    conn: &Connection,
    concept_ids: &[String],
    since: &str,
) -> Result<Vec<PatternAttempt>> {
    let concept_ids_json = serde_json::to_string(concept_ids)?;
    let mut stmt = conn.prepare(
        "SELECT id, concept_id, COALESCE(created_at, '1970-01-01T00:00:00Z')
         FROM attempts
         WHERE final_score IS NOT NULL
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) > julianday(?1)
           AND EXISTS (
             SELECT 1 FROM json_each(?2) WHERE value=attempts.concept_id
           )
         ORDER BY created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![since, concept_ids_json], |row| {
        Ok(PatternAttempt {
            id: row.get(0)?,
            concept_id: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn baseline_pattern_rate(
    conn: &Connection,
    pattern: &str,
    concept_ids: &[String],
    since: &str,
) -> Result<f64> {
    let concept_ids_json = serde_json::to_string(concept_ids)?;
    let mut stmt = conn.prepare(
        "SELECT grader_json
         FROM attempts
         WHERE final_score IS NOT NULL
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) > julianday(?1)
           AND NOT EXISTS (
             SELECT 1 FROM json_each(?2) WHERE value=attempts.concept_id
           )",
    )?;
    let rows = stmt.query_map(params![since, concept_ids_json], |row| {
        row.get::<_, Option<String>>(0)
    })?;
    let mut total = 0usize;
    let mut hits = 0usize;
    for row in rows {
        total += 1;
        if row?
            .as_deref()
            .map(|json| {
                pattern_tags_from_json(json)
                    .iter()
                    .any(|tag| tag == pattern)
            })
            .unwrap_or(false)
        {
            hits += 1;
        }
    }
    if total == 0 {
        Ok(0.0)
    } else {
        Ok(hits as f64 / total as f64)
    }
}

fn attempt_has_pattern(conn: &Connection, attempt_id: &str, pattern: &str) -> Result<bool> {
    let grader_json: Option<String> = conn
        .query_row(
            "SELECT grader_json FROM attempts WHERE id=?1",
            [attempt_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(grader_json
        .as_deref()
        .map(|json| {
            pattern_tags_from_json(json)
                .iter()
                .any(|tag| tag == pattern)
        })
        .unwrap_or(false))
}

fn attempt_score(conn: &Connection, attempt_id: &str) -> Result<f64> {
    conn.query_row(
        "SELECT COALESCE(final_score, provisional_score, 0.0) FROM attempts WHERE id=?1",
        [attempt_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn upsert_misconception_graph(conn: &Connection, rule: &GuRuleRow) -> Result<()> {
    let node_id = graph_node_id(&rule.id);
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, provenance, evidence_ids_json, created_at)
         VALUES (?1, 'engine', ?2, 'misconception_induced', 0, 'engine', ?3, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
         ON CONFLICT(id) DO UPDATE SET
            name=excluded.name,
            kind=excluded.kind,
            provenance=excluded.provenance,
            evidence_ids_json=excluded.evidence_ids_json",
        params![
            node_id,
            format!("G_u {}", rule.pattern),
            serde_json::to_string(&rule.attempt_ids)?
        ],
    )?;
    for concept_id in &rule.concept_ids {
        let edge_id = format!("gu_confusion:{}:{concept_id}", rule.id);
        conn.execute(
            "INSERT INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
             VALUES (?1, ?2, ?3, 'confusion', 1.0, 'engine', ?4, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
             ON CONFLICT(id) DO UPDATE SET
                provenance=excluded.provenance,
                evidence_ids_json=excluded.evidence_ids_json",
            params![
                edge_id,
                node_id,
                concept_id,
                serde_json::to_string(&rule.attempt_ids)?
            ],
        )?;
    }
    Ok(())
}

fn gu_rules_by_status(conn: &Connection, statuses: &[&str]) -> Result<Vec<GuRuleRow>> {
    let status_json = serde_json::to_string(statuses)?;
    let mut stmt = conn.prepare(
        "SELECT id, pattern, concept_ids_json, attempt_ids_json,
                COALESCE(first_seen, '1970-01-01T00:00:00Z'),
                COALESCE(last_seen, '1970-01-01T00:00:00Z'),
                status, consumed_at
         FROM gu_rules
         WHERE EXISTS (SELECT 1 FROM json_each(?1) WHERE value=gu_rules.status)
         ORDER BY updated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([status_json], |row| {
        let concept_ids_json: String = row.get(2)?;
        let attempt_ids_json: String = row.get(3)?;
        Ok(GuRuleRow {
            id: row.get(0)?,
            pattern: row.get(1)?,
            concept_ids: serde_json::from_str(&concept_ids_json).unwrap_or_default(),
            attempt_ids: serde_json::from_str(&attempt_ids_json).unwrap_or_default(),
            last_seen: row.get(5)?,
            status: row.get(6)?,
            consumed_at: row.get(7)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn has_existing_superset(conn: &Connection, pattern: &str, concept_ids: &[String]) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT concept_ids_json FROM gu_rules
         WHERE pattern=?1 AND status IN ('candidate','validated','active')",
    )?;
    let rows = stmt.query_map([pattern], |row| row.get::<_, String>(0))?;
    for row in rows {
        let existing: Vec<String> = serde_json::from_str(&row?).unwrap_or_default();
        if concept_ids.iter().all(|id| existing.contains(id)) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn set_status(
    conn: &Connection,
    rule_id: &str,
    status: &str,
    concept_id: Option<&str>,
) -> Result<()> {
    let consumed_expr = if status == "active" {
        "COALESCE(consumed_at, strftime('%Y-%m-%dT%H:%M:%SZ','now'))"
    } else {
        "consumed_at"
    };
    conn.execute(
        &format!(
            "UPDATE gu_rules
             SET status=?1, consumed_at={consumed_expr}, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=?2"
        ),
        params![status, rule_id],
    )?;
    record_lifecycle(conn, rule_id, status, concept_id)
}

fn activate_rule(
    conn: &Connection,
    rule_id: &str,
    consumed_at: &str,
    concept_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE gu_rules
         SET status='active',
             consumed_at=COALESCE(consumed_at, ?1),
             updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?2",
        params![consumed_at, rule_id],
    )?;
    record_lifecycle(conn, rule_id, "active", concept_id)
}

fn record_lifecycle(
    conn: &Connection,
    rule_id: &str,
    status: &str,
    concept_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (lower(hex(randomblob(16))), 'engine', strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                 'gu_lifecycle', ?1, ?2)",
        params![
            concept_id,
            serde_json::json!({"rule_id": rule_id, "status": status}).to_string()
        ],
    )?;
    Ok(())
}

fn latest_attempt_time(conn: &Connection) -> Result<Option<String>> {
    conn.query_row(
        "SELECT MAX(COALESCE(created_at, '1970-01-01T00:00:00Z')) FROM attempts",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn rule_id(pattern: &str, concept_ids: &[String]) -> String {
    format!("{}:{}", pattern, concept_ids.join("+"))
}

fn graph_node_id(rule_id: &str) -> String {
    format!("gu:{rule_id}")
}

fn beta_probability_ge(alpha: u64, beta: u64, threshold: f64) -> f64 {
    1.0 - beta_probability_lt(alpha, beta, threshold)
}

fn beta_probability_lt(alpha: u64, beta: u64, threshold: f64) -> f64 {
    let n = alpha + beta - 1;
    (alpha..=n)
        .map(|j| binomial(n, j) * threshold.powi(j as i32) * (1.0 - threshold).powi((n - j) as i32))
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

fn binomial(n: u64, k: u64) -> f64 {
    let k = k.min(n - k);
    if k == 0 {
        return 1.0;
    }
    (1..=k).fold(1.0, |acc, i| acc * (n - k + i) as f64 / i as f64)
}

fn days_between(start: &str, end: &str) -> Option<i64> {
    let (start_year, start_month, start_day) = parse_ymd(start)?;
    let (end_year, end_month, end_day) = parse_ymd(end)?;
    Some(
        days_from_civil(end_year, end_month, end_day)
            - days_from_civil(start_year, start_month, start_day),
    )
}

fn parse_ymd(value: &str) -> Option<(i32, u32, u32)> {
    let date = value.get(0..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    Some((year, month, day))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i64
}
