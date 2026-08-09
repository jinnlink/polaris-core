use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::engine::Engine;
use crate::error::{PolarisError, Result};

const ASSERTION_LIMIT: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionConceptTouch {
    pub concept_id: String,
    pub concept_name: String,
    pub attempt_count: i64,
    pub min_score: Option<f64>,
    pub max_score: Option<f64>,
    pub hint_count: i64,
    pub abandon_count: i64,
    #[serde(default)]
    pub no_attempt_count: i64,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAssertion {
    pub concept_id: String,
    pub kind: String,
    pub text: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCloseSummary {
    pub session_id: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub closed_at: String,
    pub concepts_touched: Vec<SessionConceptTouch>,
    pub attempts_count: i64,
    pub top_stuck_concept_id: Option<String>,
    pub next_entry_concept_id: Option<String>,
    pub assertions: Vec<SessionAssertion>,
    pub generated_at: String,
}

#[derive(Debug, Default)]
struct TouchAccumulator {
    concept_name: String,
    attempt_count: i64,
    min_score: Option<f64>,
    max_score: Option<f64>,
    hint_count: i64,
    abandon_count: i64,
    no_attempt_count: i64,
    evidence_ids: BTreeSet<String>,
}

pub fn close_session(engine: &Engine, session_id: &str) -> Result<SessionCloseSummary> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err(PolarisError::InvalidParameter {
            key: "session_id".to_owned(),
            value: session_id.to_owned(),
        });
    }
    if let Some(existing) = session_close_summary(engine.conn(), session_id)? {
        return Ok(existing);
    }

    engine.conn().execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<SessionCloseSummary> {
        if let Some(existing) = session_close_summary(engine.conn(), session_id)? {
            return Ok(existing);
        }
        let exists: bool = engine.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            [session_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(PolarisError::MissingSession(session_id.to_owned()));
        }

        let concepts_touched = load_session_touches(engine.conn(), session_id)?;
        let attempts_count: i64 = concepts_touched
            .iter()
            .map(|touch| touch.attempt_count)
            .sum();
        let top_stuck_concept_id = top_stuck_concept(&concepts_touched);
        let ranked_ids = engine.ranked_concept_ids_for_all_packs()?;
        let assertions = build_assertions(&concepts_touched, &ranked_ids);
        let next_entry_concept_id = engine.next_task_concept_id()?;
        let now: String =
            engine
                .conn()
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
                    row.get(0)
                })?;

        engine.conn().execute(
            "UPDATE sessions
             SET ended_at=COALESCE(ended_at, ?2), closed_at=COALESCE(closed_at, ?2)
             WHERE id=?1",
            params![session_id, now],
        )?;
        engine.conn().execute(
            "INSERT INTO session_summaries(
                 session_id, concepts_touched_json, attempts_count,
                 top_stuck_concept_id, next_entry_concept_id, assertions_json, generated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_id,
                serde_json::to_string(&concepts_touched)?,
                attempts_count,
                top_stuck_concept_id,
                next_entry_concept_id,
                serde_json::to_string(&assertions)?,
                now,
            ],
        )?;
        session_close_summary(engine.conn(), session_id)?.ok_or_else(|| {
            PolarisError::MissingSession(format!("{session_id} summary after close"))
        })
    })();

    match result {
        Ok(summary) => {
            engine.conn().execute_batch("COMMIT")?;
            Ok(summary)
        }
        Err(error) => {
            let _ = engine.conn().execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn session_close_summary(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<SessionCloseSummary>> {
    let row = conn
        .query_row(
            "SELECT s.started_at, s.ended_at, s.closed_at,
                    ss.concepts_touched_json, ss.attempts_count,
                    ss.top_stuck_concept_id, ss.next_entry_concept_id,
                    ss.assertions_json, ss.generated_at
             FROM session_summaries ss
             JOIN sessions s ON s.id=ss.session_id
             WHERE ss.session_id=?1",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        started_at,
        ended_at,
        closed_at,
        concepts_json,
        attempts_count,
        top_stuck_concept_id,
        next_entry_concept_id,
        assertions_json,
        generated_at,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(SessionCloseSummary {
        session_id: session_id.to_owned(),
        started_at,
        ended_at,
        closed_at,
        concepts_touched: serde_json::from_str(&concepts_json)?,
        attempts_count,
        top_stuck_concept_id,
        next_entry_concept_id,
        assertions: serde_json::from_str(&assertions_json)?,
        generated_at,
    }))
}

fn load_session_touches(conn: &Connection, session_id: &str) -> Result<Vec<SessionConceptTouch>> {
    let mut touches = BTreeMap::<String, TouchAccumulator>::new();
    let mut attempts = conn.prepare(
        "SELECT a.id, a.concept_id, c.name, COALESCE(a.final_score, a.provisional_score),
                a.no_attempt_reason
         FROM attempts a
         JOIN concepts c ON c.id=a.concept_id
         WHERE a.session_id=?1
         ORDER BY julianday(COALESCE(a.created_at, '1970-01-01')), a.id",
    )?;
    let attempt_rows = attempts
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (evidence_id, concept_id, concept_name, score, no_attempt_reason) in attempt_rows {
        let touch = touches.entry(concept_id).or_default();
        touch.concept_name = concept_name;
        touch.evidence_ids.insert(evidence_id);
        if no_attempt_reason.is_some() {
            touch.no_attempt_count += 1;
        } else {
            touch.attempt_count += 1;
        }
        if let Some(score) = score.filter(|score| score.is_finite()) {
            touch.min_score = Some(touch.min_score.map_or(score, |value| value.min(score)));
            touch.max_score = Some(touch.max_score.map_or(score, |value| value.max(score)));
        }
    }

    let mut events = conn.prepare(
        "SELECT b.id, b.concept_id, c.name, b.type
         FROM behavior_events b
         JOIN concepts c ON c.id=b.concept_id
         WHERE b.session_id=?1
         ORDER BY julianday(COALESCE(b.at, '1970-01-01')), b.id",
    )?;
    let event_rows = events
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (evidence_id, concept_id, concept_name, kind) in event_rows {
        let touch = touches.entry(concept_id).or_default();
        touch.concept_name = concept_name;
        touch.evidence_ids.insert(evidence_id);
        match kind.as_str() {
            "hint" => touch.hint_count += 1,
            "abandon" => touch.abandon_count += 1,
            _ => {}
        }
    }

    Ok(touches
        .into_iter()
        .map(|(concept_id, touch)| SessionConceptTouch {
            concept_id,
            concept_name: touch.concept_name,
            attempt_count: touch.attempt_count,
            min_score: touch.min_score,
            max_score: touch.max_score,
            hint_count: touch.hint_count,
            abandon_count: touch.abandon_count,
            no_attempt_count: touch.no_attempt_count,
            evidence_ids: touch.evidence_ids.into_iter().collect(),
        })
        .collect())
}

fn top_stuck_concept(touches: &[SessionConceptTouch]) -> Option<String> {
    let has_action_signal = touches
        .iter()
        .any(|touch| touch.hint_count + touch.abandon_count + touch.no_attempt_count > 0);
    touches
        .iter()
        .filter(|touch| {
            if has_action_signal {
                touch.hint_count + touch.abandon_count + touch.no_attempt_count > 0
            } else {
                touch.min_score.is_some()
            }
        })
        .min_by(|left, right| {
            let left_actions = left.hint_count + left.abandon_count + left.no_attempt_count;
            let right_actions = right.hint_count + right.abandon_count + right.no_attempt_count;
            right_actions
                .cmp(&left_actions)
                .then_with(|| {
                    option_score(left.min_score).total_cmp(&option_score(right.min_score))
                })
                .then_with(|| left.concept_id.cmp(&right.concept_id))
        })
        .map(|touch| touch.concept_id.clone())
}

fn option_score(value: Option<f64>) -> f64 {
    value.unwrap_or(f64::INFINITY)
}

fn build_assertions(
    touches: &[SessionConceptTouch],
    ranked_ids: &[String],
) -> Vec<SessionAssertion> {
    let rank = ranked_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = touches
        .iter()
        .filter(|touch| !touch.evidence_ids.is_empty())
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        rank.get(left.concept_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
            .cmp(
                &rank
                    .get(right.concept_id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
            )
            .then_with(|| left.concept_id.cmp(&right.concept_id))
    });
    candidates
        .into_iter()
        .take(ASSERTION_LIMIT)
        .map(|touch| SessionAssertion {
            concept_id: touch.concept_id.clone(),
            kind: "session_concept_summary".to_owned(),
            text: assertion_text(touch),
            evidence_ids: touch.evidence_ids.clone(),
        })
        .collect()
}

fn assertion_text(touch: &SessionConceptTouch) -> String {
    match (touch.min_score, touch.max_score) {
        (Some(min), Some(max)) => format!(
            "{}：本次 {} 次作答，得分范围 {:.2}–{:.2}，提示 {} 次，放弃 {} 次，未作答 {} 次。",
            touch.concept_name,
            touch.attempt_count,
            min,
            max,
            touch.hint_count,
            touch.abandon_count,
            touch.no_attempt_count
        ),
        _ => format!(
            "{}：本次留下了学习行为证据，提示 {} 次，放弃 {} 次，未作答 {} 次。",
            touch.concept_name, touch.hint_count, touch.abandon_count, touch.no_attempt_count
        ),
    }
}
