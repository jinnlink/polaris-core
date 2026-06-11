use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::config::meta_f64;
use crate::error::{PolarisError, Result};
use crate::mirt::{decode_vector, encode_vector, ensure_theta};

const MIN_COMMON_WEEKS: usize = 4;
const CORRELATION_THRESHOLD: f64 = 0.5;
const MIN_CLUSTER_SIZE: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct ConsolidationSummary {
    pub residual_rows: usize,
    pub proposal_count: usize,
    pub holdout_delta: f64,
    pub accepted: bool,
}

#[derive(Debug, Clone)]
struct AttemptResidualSource {
    concept_id: String,
    task_type: String,
    final_score: f64,
    theta_version: i64,
    week: String,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateProposal {
    kind: String,
    concepts: Vec<String>,
    min_common_weeks: usize,
    correlation_threshold: f64,
    requires_llm_abduction: bool,
    holdout_gate: String,
}

pub fn run_nightly_consolidation(conn: &Connection) -> Result<ConsolidationSummary> {
    snapshot_and_shrink_theta(conn)?;
    let residual_rows = refresh_residual_stats(conn)?;
    let proposals = candidate_proposals(conn)?;
    let proposal_count = proposals.len();
    let holdout_delta = 0.0;
    let accepted = false;
    let status = "rejected";
    let proposals_json = serde_json::to_string(&proposals)?;

    conn.execute(
        "INSERT INTO consolidation_runs(id, ran_at, proposals_json, holdout_delta, status)
         VALUES (lower(hex(randomblob(16))), strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?1, ?2, ?3)",
        params![proposals_json, holdout_delta, status],
    )?;

    Ok(ConsolidationSummary {
        residual_rows,
        proposal_count,
        holdout_delta,
        accepted,
    })
}

fn snapshot_and_shrink_theta(conn: &Connection) -> Result<()> {
    ensure_theta(conn)?;
    let (theta, version) = current_theta(conn)?;
    conn.execute(
        "INSERT OR REPLACE INTO theta_history(version, vec, at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        params![version, encode_vector(&theta)],
    )?;

    let shrink = meta_f64(conn, "mirt.shrink")?;
    let shrunk = theta
        .iter()
        .map(|value| value * (1.0 - shrink))
        .collect::<Vec<_>>();
    conn.execute(
        "UPDATE theta
         SET vec=?1, version=?2, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=1",
        params![encode_vector(&shrunk), version + 1],
    )?;
    Ok(())
}

fn refresh_residual_stats(conn: &Connection) -> Result<usize> {
    let theta_by_version = theta_history(conn)?;
    let attempts = residual_sources(conn)?;
    let mut grouped = BTreeMap::<(String, String), Vec<f64>>::new();
    for attempt in attempts {
        let Some(theta) = theta_by_version.get(&attempt.theta_version) else {
            continue;
        };
        let p_hat = predict_with_theta(conn, &attempt.concept_id, &attempt.task_type, theta)?;
        grouped
            .entry((attempt.concept_id, attempt.week))
            .or_default()
            .push(attempt.final_score.clamp(0.0, 1.0) - p_hat);
    }

    conn.execute("DELETE FROM residual_stats", [])?;
    for ((concept_id, week), residuals) in &grouped {
        let mean = residuals.iter().sum::<f64>() / residuals.len() as f64;
        conn.execute(
            "INSERT INTO residual_stats(concept_id, week, mean_resid, n)
             VALUES (?1, ?2, ?3, ?4)",
            params![concept_id, week, mean, residuals.len() as i64],
        )?;
    }
    Ok(grouped.len())
}

fn residual_sources(conn: &Connection) -> Result<Vec<AttemptResidualSource>> {
    let mut stmt = conn.prepare(
        "SELECT concept_id, COALESCE(task_type, 'recall'), final_score, theta_version,
                COALESCE(created_at, '1970-01-01T00:00:00Z') AS created_at
         FROM attempts
         WHERE final_score IS NOT NULL
           AND theta_version IS NOT NULL
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) >= julianday('now') - 90
         ORDER BY created_at ASC, id ASC",
    )?;
    let mut rows = stmt.query([])?;
    let mut attempts = Vec::new();
    while let Some(row) = rows.next()? {
        let created_at = row.get::<_, String>(4)?;
        attempts.push(AttemptResidualSource {
            concept_id: row.get(0)?,
            task_type: row.get(1)?,
            final_score: row.get(2)?,
            theta_version: row.get(3)?,
            week: iso_week_label(&created_at)?,
        });
    }
    Ok(attempts)
}

fn theta_history(conn: &Connection) -> Result<BTreeMap<i64, Vec<f64>>> {
    let mut map = BTreeMap::new();
    let mut stmt = conn.prepare("SELECT version, vec FROM theta_history")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    for row in rows {
        let (version, blob) = row?;
        map.insert(version, decode_vector(&blob)?);
    }
    let (theta, version) = current_theta(conn)?;
    map.entry(version).or_insert(theta);
    Ok(map)
}

fn current_theta(conn: &Connection) -> Result<(Vec<f64>, i64)> {
    let (blob, version): (Vec<u8>, i64) =
        conn.query_row("SELECT vec, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    Ok((decode_vector(&blob)?, version))
}

fn predict_with_theta(
    conn: &Connection,
    concept_id: &str,
    task_type: &str,
    theta: &[f64],
) -> Result<f64> {
    let (q_blob, b_difficulty): (Vec<u8>, f64) = conn
        .query_row(
            "SELECT q, COALESCE(b_difficulty, 0.0) FROM concepts WHERE id=?1",
            [concept_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                PolarisError::MissingConcept(concept_id.to_owned())
            }
            other => PolarisError::Database(other),
        })?;
    let q = decode_vector(&q_blob)?;
    if q.len() != theta.len() {
        return Err(PolarisError::InvalidParameter {
            key: "q/theta".to_owned(),
            value: format!("{} != {}", q.len(), theta.len()),
        });
    }
    let logit = q.iter().zip(theta).map(|(a, b)| a * b).sum::<f64>()
        - b_difficulty
        - task_difficulty(conn, task_type)?;
    Ok(sigmoid(logit))
}

fn task_difficulty(conn: &Connection, task_type: &str) -> Result<f64> {
    let key = match task_type {
        "free_explain" | "explain" => "free_produce",
        other => other,
    };
    meta_f64(conn, &format!("mirt.d.{key}"))
}

fn sigmoid(logit: f64) -> f64 {
    let x = logit.clamp(-10.0, 10.0);
    1.0 / (1.0 + (-x).exp())
}

fn iso_week_label(created_at: &str) -> Result<String> {
    let (year, month, day) =
        parse_ymd(created_at).ok_or_else(|| PolarisError::InvalidParameter {
            key: "created_at".to_owned(),
            value: created_at.to_owned(),
        })?;
    let ordinal = ordinal_day(year, month, day) as i32;
    let weekday = iso_weekday(year, month, day) as i32;
    let mut week_year = year;
    let mut week = (ordinal - weekday + 10) / 7;
    if week < 1 {
        week_year -= 1;
        week = iso_weeks_in_year(week_year) as i32;
    } else {
        let weeks_in_year = iso_weeks_in_year(year) as i32;
        if week > weeks_in_year {
            week_year += 1;
            week = 1;
        }
    }
    Ok(format!("{week_year:04}-W{week:02}"))
}

fn parse_ymd(value: &str) -> Option<(i32, u32, u32)> {
    let date = value.get(0..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    let max_day = days_in_month(year, month);
    if !(1..=max_day).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

fn ordinal_day(year: i32, month: u32, day: u32) -> u32 {
    (1..month)
        .map(|previous_month| days_in_month(year, previous_month))
        .sum::<u32>()
        + day
}

fn iso_weeks_in_year(year: i32) -> u32 {
    let jan_first = iso_weekday(year, 1, 1);
    if jan_first == 4 || (jan_first == 3 && is_leap_year(year)) {
        53
    } else {
        52
    }
}

fn iso_weekday(year: i32, month: u32, day: u32) -> u32 {
    let days = days_from_civil(year, month, day);
    (days + 3).rem_euclid(7) as u32 + 1
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

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn candidate_proposals(conn: &Connection) -> Result<Vec<CandidateProposal>> {
    let series = residual_series(conn)?;
    let eligible = series
        .iter()
        .filter(|(_, values)| values.len() >= MIN_COMMON_WEEKS)
        .map(|(concept, _)| concept.clone())
        .collect::<Vec<_>>();
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for concept in &eligible {
        adjacency.entry(concept.clone()).or_default();
    }

    for left_idx in 0..eligible.len() {
        for right in eligible.iter().skip(left_idx + 1) {
            let left = &eligible[left_idx];
            if let Some(corr) = correlation(&series[left], &series[right]) {
                if corr >= CORRELATION_THRESHOLD {
                    adjacency
                        .entry(left.clone())
                        .or_default()
                        .insert(right.clone());
                    adjacency
                        .entry(right.clone())
                        .or_default()
                        .insert(left.clone());
                }
            }
        }
    }

    let mut proposals = Vec::new();
    let mut seen = BTreeSet::new();
    for concept in adjacency.keys() {
        if seen.contains(concept) {
            continue;
        }
        let component = connected_component(concept, &adjacency);
        for item in &component {
            seen.insert(item.clone());
        }
        if component.len() >= MIN_CLUSTER_SIZE {
            proposals.push(CandidateProposal {
                kind: "candidate_latent_dimension".to_owned(),
                concepts: component,
                min_common_weeks: MIN_COMMON_WEEKS,
                correlation_threshold: CORRELATION_THRESHOLD,
                requires_llm_abduction: true,
                holdout_gate: "rejected_without_validated_trial".to_owned(),
            });
        }
    }

    Ok(proposals)
}

fn residual_series(conn: &Connection) -> Result<BTreeMap<String, BTreeMap<String, f64>>> {
    let mut stmt = conn.prepare(
        "SELECT concept_id, week, mean_resid FROM residual_stats ORDER BY concept_id, week",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;
    let mut series = BTreeMap::<String, BTreeMap<String, f64>>::new();
    for row in rows {
        let (concept, week, mean) = row?;
        series.entry(concept).or_default().insert(week, mean);
    }
    Ok(series)
}

fn correlation(left: &BTreeMap<String, f64>, right: &BTreeMap<String, f64>) -> Option<f64> {
    let pairs = left
        .iter()
        .filter_map(|(week, left_value)| {
            right
                .get(week)
                .map(|right_value| (*left_value, *right_value))
        })
        .collect::<Vec<_>>();
    if pairs.len() < MIN_COMMON_WEEKS {
        return None;
    }
    let left_mean = pairs.iter().map(|(left, _)| left).sum::<f64>() / pairs.len() as f64;
    let right_mean = pairs.iter().map(|(_, right)| right).sum::<f64>() / pairs.len() as f64;
    let numerator = pairs
        .iter()
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>();
    let left_var = pairs
        .iter()
        .map(|(left, _)| (left - left_mean).powi(2))
        .sum::<f64>();
    let right_var = pairs
        .iter()
        .map(|(_, right)| (right - right_mean).powi(2))
        .sum::<f64>();
    if left_var == 0.0 || right_var == 0.0 {
        return None;
    }
    Some(numerator / (left_var.sqrt() * right_var.sqrt()))
}

fn connected_component(start: &str, adjacency: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    let mut stack = vec![start.to_owned()];
    let mut seen = BTreeSet::new();
    while let Some(node) = stack.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&node) {
            for neighbor in neighbors {
                stack.push(neighbor.clone());
            }
        }
    }
    seen.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlation_requires_variance() {
        let left = BTreeMap::from([
            ("w1".to_owned(), 1.0),
            ("w2".to_owned(), 1.0),
            ("w3".to_owned(), 1.0),
            ("w4".to_owned(), 1.0),
        ]);
        let right = left.clone();

        assert_eq!(correlation(&left, &right), None);
    }

    #[test]
    fn iso_week_label_uses_week_year_at_calendar_boundary() {
        assert_eq!(iso_week_label("2024-12-30T00:00:00Z").unwrap(), "2025-W01");
        assert_eq!(iso_week_label("2025-01-01T00:00:00Z").unwrap(), "2025-W01");
        assert_eq!(iso_week_label("2027-01-01T00:00:00Z").unwrap(), "2026-W53");
    }
}
