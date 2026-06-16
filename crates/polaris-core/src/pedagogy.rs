use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use uuid::Uuid;

use crate::config::{meta_f64, meta_i64};
use crate::error::Result;
use crate::moves::{bloom_move, SelectedMove};
use crate::phase::Phase;

const MOVE_IDS: [&str; 7] = [
    "recall", "explain", "apply", "analyze", "evaluate", "create", "transfer",
];

#[derive(Debug, Clone, PartialEq)]
pub struct PedagogySelection {
    pub selected_move: SelectedMove,
    pub context_hash: String,
    pub randomized: bool,
    pub prereg_id: String,
}

#[derive(Debug, Clone)]
struct MoveCandidate {
    move_id: &'static str,
    posterior_mean: f64,
    friction: f64,
    utility: f64,
    n: i64,
    base: bool,
}

#[derive(Debug, Clone)]
struct DecisionLink {
    prereg_id: String,
    context_hash: String,
    move_id: String,
    concept_id: String,
    selected_at: String,
}

pub fn select_move_for_concept(
    conn: &Connection,
    concept_id: &str,
    concept_name: &str,
    base_move: SelectedMove,
    p_known: f64,
    phase: Phase,
    phase_strategy: Option<&'static str>,
) -> Result<PedagogySelection> {
    let state = latest_strategy_state(conn)?;
    let context_hash = format!("state:{state}|phase:{}", phase.as_str());
    let lambda = meta_f64(conn, "friction.lambda")?;
    let mut candidates = candidate_moves(conn, base_move, p_known, &context_hash, lambda)?;
    let observed_samples = candidates.iter().any(|candidate| candidate.n > 0);
    if observed_samples {
        candidates.sort_by(compare_candidates);
    }

    let incumbent = candidates
        .iter()
        .find(|candidate| candidate.base)
        .map(|candidate| candidate.move_id)
        .unwrap_or(base_move.id);
    let mut selected = if phase_strategy.is_some() {
        base_move.id
    } else if observed_samples {
        candidates
            .first()
            .map(|candidate| candidate.move_id)
            .unwrap_or(base_move.id)
    } else {
        base_move.id
    };

    let epsilon = meta_f64(conn, "mrt.epsilon")?;
    let mut randomized = false;
    let alternatives = candidates
        .iter()
        .filter(|candidate| candidate.move_id != selected)
        .map(|candidate| candidate.move_id)
        .collect::<Vec<_>>();
    if phase_strategy.is_none() && !alternatives.is_empty() && random_unit() < epsilon {
        let index = random_index(alternatives.len());
        selected = alternatives[index];
        randomized = true;
    }

    let prereg_id = format!("mrt-{}", Uuid::new_v4());
    let mut candidate_set = vec![selected.to_owned()];
    if phase_strategy.is_none() {
        for candidate in &candidates {
            if !candidate_set.iter().any(|item| item == candidate.move_id) {
                candidate_set.push(candidate.move_id.to_owned());
            }
        }
    }
    let selected_by = if phase_strategy.is_some() {
        "phase_action_loop"
    } else {
        "signature_friction"
    };
    let main_effect_hypothesis = phase_strategy
        .map(phase_strategy_hypothesis)
        .unwrap_or("selected move improves 7d success under this state and phase context");
    let context_json = json!({
        "kind": "preregistration",
        "window": "7d",
        "epsilon": epsilon,
        "candidate_set": candidate_set,
        "incumbent": incumbent,
        "selected_by": selected_by,
        "phase_strategy": phase_strategy,
        "context_hash": context_hash,
        "concept_id": concept_id,
        "concept_name": concept_name,
        "state": state,
        "phase": phase.as_str(),
        "main_effect_hypothesis": main_effect_hypothesis,
        "min_n": meta_i64(conn, "sig.shrink_n0")?
    })
    .to_string();
    conn.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, ?3, ?4, ?5)",
        params![
            Uuid::new_v4().to_string(),
            context_json,
            if randomized { 1 } else { 0 },
            selected,
            prereg_id,
        ],
    )?;

    Ok(PedagogySelection {
        selected_move: bloom_move(selected),
        context_hash,
        randomized,
        prereg_id,
    })
}

pub fn record_move_effect_for_attempt(
    conn: &Connection,
    attempt_id: &str,
    final_score: f64,
) -> Result<()> {
    record_success_for_attempt(conn, attempt_id, final_score)?;
    record_expired_move_effect_outcomes(conn)?;
    Ok(())
}

fn record_success_for_attempt(conn: &Connection, attempt_id: &str, final_score: f64) -> Result<()> {
    let (concept_id, attempt_created_at) = conn.query_row(
        "SELECT concept_id, COALESCE(created_at, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
         FROM attempts WHERE id=?1",
        [attempt_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    let success = final_score >= meta_f64(conn, "bkt.cut_hi")?;
    if success {
        record_success_for_pending_decisions(
            conn,
            &concept_id,
            attempt_id,
            final_score,
            &attempt_created_at,
        )?;
        return Ok(());
    }

    let Some((link, _)) = decision_link_for_attempt(conn, attempt_id)? else {
        return Ok(());
    };
    if timestamp_at_or_after_window_end(conn, &link.selected_at, &attempt_created_at)?
        && success_in_window(conn, &link)?.is_none()
    {
        apply_move_effect_outcome(
            conn,
            &link,
            Some(attempt_id),
            Some(final_score),
            false,
            "window_expired_no_success",
        )?;
    }
    Ok(())
}

fn record_success_for_pending_decisions(
    conn: &Connection,
    concept_id: &str,
    attempt_id: &str,
    final_score: f64,
    attempt_created_at: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT at, concept_id, payload_json
         FROM behavior_events
         WHERE type='next'
           AND concept_id=?1
           AND json_extract(payload_json, '$.mrt_prereg_id') IS NOT NULL
           AND julianday(?2) >= julianday(at)
           AND julianday(?2) <= julianday(at, '+7 days')
         ORDER BY at ASC, id ASC",
    )?;
    let rows = stmt
        .query_map(params![concept_id, attempt_created_at], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (selected_at, concept_id, payload_json) in rows {
        let Some(link) = decision_link_from_event(concept_id, selected_at, &payload_json)? else {
            continue;
        };
        apply_move_effect_outcome(
            conn,
            &link,
            Some(attempt_id),
            Some(final_score),
            true,
            "same_concept_success_within_7d",
        )?;
    }
    Ok(())
}

fn record_expired_move_effect_outcomes(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT at, concept_id, payload_json
         FROM behavior_events
         WHERE type='next'
           AND json_extract(payload_json, '$.mrt_prereg_id') IS NOT NULL
           AND julianday(at, '+7 days') <= julianday('now')
         ORDER BY at ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (selected_at, concept_id, payload_json) in rows {
        let Some(link) = decision_link_from_event(concept_id, selected_at, &payload_json)? else {
            continue;
        };
        if latest_recorded_outcome(conn, &link.prereg_id)?.is_some() {
            continue;
        }
        if let Some((attempt_id, score)) = success_in_window(conn, &link)? {
            apply_move_effect_outcome(
                conn,
                &link,
                Some(&attempt_id),
                Some(score),
                true,
                "same_concept_success_within_7d",
            )?;
        } else {
            apply_move_effect_outcome(conn, &link, None, None, false, "window_expired_no_success")?;
        }
    }
    Ok(())
}

fn apply_move_effect_outcome(
    conn: &Connection,
    link: &DecisionLink,
    source_attempt_id: Option<&str>,
    final_score: Option<f64>,
    success: bool,
    reason: &str,
) -> Result<()> {
    let previous = latest_recorded_outcome(conn, &link.prereg_id)?;
    if previous == Some(success) {
        return Ok(());
    }

    let init_alpha = 1.0 + if success { 1.0 } else { 0.0 };
    let init_beta = 1.0 + if success { 0.0 } else { 1.0 };
    let alpha_delta = if success { 1.0 } else { 0.0 };
    let beta_delta = if success { 0.0 } else { 1.0 };
    let tx = conn.unchecked_transaction()?;
    if let Some(previous) = previous {
        let previous_alpha = if previous { 1.0 } else { 0.0 };
        let previous_beta = if previous { 0.0 } else { 1.0 };
        let changed = tx.execute(
            "UPDATE moves_effects
             SET alpha = alpha + ?3,
                 beta = beta + ?4
            WHERE move=?1 AND context_hash=?2",
            params![
                link.move_id.as_str(),
                link.context_hash.as_str(),
                alpha_delta - previous_alpha,
                beta_delta - previous_beta,
            ],
        )?;
        if changed == 0 {
            tx.execute(
                "INSERT INTO moves_effects(move, context_hash, alpha, beta, n)
                 VALUES (?1, ?2, ?3, ?4, 1)",
                params![
                    link.move_id.as_str(),
                    link.context_hash.as_str(),
                    init_alpha,
                    init_beta,
                ],
            )?;
        }
    } else {
        tx.execute(
            "INSERT INTO moves_effects(move, context_hash, alpha, beta, n)
             VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT(move, context_hash) DO UPDATE SET
                 alpha = alpha + ?5,
                 beta = beta + ?6,
                 n = n + 1",
            params![
                link.move_id.as_str(),
                link.context_hash.as_str(),
                init_alpha,
                init_beta,
                alpha_delta,
                beta_delta,
            ],
        )?;
    }
    let context_json = json!({
        "kind": if previous.is_some() { "outcome_correction" } else { "outcome" },
        "source_attempt_id": source_attempt_id,
        "concept_id": &link.concept_id,
        "context_hash": &link.context_hash,
        "selected_at": &link.selected_at,
        "window": "7d",
        "outcome_reason": reason,
        "outcome": success,
        "final_score": final_score
    })
    .to_string();
    tx.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, 0, ?3, ?4)",
        params![
            Uuid::new_v4().to_string(),
            context_json,
            link.move_id.as_str(),
            link.prereg_id.as_str(),
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn decision_link_for_attempt(
    conn: &Connection,
    attempt_id: &str,
) -> Result<Option<(DecisionLink, String)>> {
    let (session_id, concept_id, task_type, attempt_created_at): (
        Option<String>,
        String,
        String,
        String,
    ) = conn.query_row(
        "SELECT session_id, concept_id, COALESCE(task_type, 'recall'),
                COALESCE(created_at, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
         FROM attempts WHERE id=?1",
        [attempt_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    let event: Option<(String, String)> = conn
        .query_row(
            "SELECT at, payload_json
             FROM behavior_events
             WHERE session_id=?1
               AND concept_id=?2
               AND type='next'
               AND json_extract(payload_json, '$.task_type')=?3
               AND json_extract(payload_json, '$.mrt_prereg_id') IS NOT NULL
               AND julianday(at) <= julianday(?4)
             ORDER BY at DESC, id DESC
             LIMIT 1",
            params![session_id, concept_id, task_type, attempt_created_at],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((selected_at, payload_json)) = event else {
        return Ok(None);
    };
    Ok(
        decision_link_from_event(concept_id, selected_at, &payload_json)?
            .map(|link| (link, attempt_created_at)),
    )
}

fn decision_link_from_event(
    concept_id: String,
    selected_at: String,
    payload_json: &str,
) -> Result<Option<DecisionLink>> {
    let value: serde_json::Value = serde_json::from_str(payload_json)?;
    let Some(prereg_id) = value
        .get("mrt_prereg_id")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };
    let Some(context_hash) = value
        .get("mrt_context_hash")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };
    let move_id = value
        .get("move")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("task_type")
                .and_then(serde_json::Value::as_str)
                .map(move_id_for_task_type)
        })
        .unwrap_or("recall")
        .to_owned();
    Ok(Some(DecisionLink {
        prereg_id: prereg_id.to_owned(),
        context_hash: context_hash.to_owned(),
        move_id,
        concept_id,
        selected_at,
    }))
}

fn latest_recorded_outcome(conn: &Connection, prereg_id: &str) -> Result<Option<bool>> {
    let payload: Option<String> = conn
        .query_row(
            "SELECT context_json
             FROM mrt_log
             WHERE prereg_id=?1
               AND json_extract(context_json, '$.kind') IN ('outcome', 'outcome_correction')
             ORDER BY at DESC, id DESC
             LIMIT 1",
            [prereg_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_str(&payload)?;
    Ok(value.get("outcome").and_then(serde_json::Value::as_bool))
}

fn timestamp_at_or_after_window_end(
    conn: &Connection,
    selected_at: &str,
    at: &str,
) -> Result<bool> {
    let matched: i64 = conn.query_row(
        "SELECT CASE WHEN julianday(?2) >= julianday(?1, '+7 days') THEN 1 ELSE 0 END",
        params![selected_at, at],
        |row| row.get(0),
    )?;
    Ok(matched == 1)
}

fn success_in_window(conn: &Connection, link: &DecisionLink) -> Result<Option<(String, f64)>> {
    let success = conn
        .query_row(
            "SELECT id, final_score
         FROM attempts
         WHERE concept_id=?1
           AND final_score >= ?2
           AND created_at IS NOT NULL
           AND julianday(created_at) >= julianday(?3)
           AND julianday(created_at) <= julianday(?3, '+7 days')
         ORDER BY created_at ASC, id ASC
         LIMIT 1",
            params![
                link.concept_id.as_str(),
                meta_f64(conn, "bkt.cut_hi")?,
                link.selected_at.as_str(),
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(success)
}

fn candidate_moves(
    conn: &Connection,
    base_move: SelectedMove,
    p_known: f64,
    context_hash: &str,
    lambda: f64,
) -> Result<Vec<MoveCandidate>> {
    let base_rank = move_rank(base_move.id);
    let mut candidates = Vec::new();
    for move_id in MOVE_IDS {
        let rank = move_rank(move_id);
        if (rank - base_rank).abs() > 1 {
            continue;
        }
        let posterior = posterior_for_move(conn, move_id, context_hash)?;
        let friction = friction_score(conn, p_known, base_rank, rank)?;
        candidates.push(MoveCandidate {
            move_id,
            posterior_mean: posterior.0,
            friction,
            utility: posterior.0 - lambda * friction,
            n: posterior.1,
            base: move_id == base_move.id,
        });
    }
    if candidates.is_empty() {
        candidates.push(MoveCandidate {
            move_id: base_move.id,
            posterior_mean: 0.5,
            friction: friction_score(conn, p_known, base_rank, base_rank)?,
            utility: 0.5,
            n: 0,
            base: true,
        });
    }
    Ok(candidates)
}

fn posterior_for_move(conn: &Connection, move_id: &str, context_hash: &str) -> Result<(f64, i64)> {
    let stored: Option<(f64, f64, i64)> = conn
        .query_row(
            "SELECT alpha, beta, n FROM moves_effects WHERE move=?1 AND context_hash=?2",
            (move_id, context_hash),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (stored_alpha, stored_beta, n) = stored.unwrap_or((1.0, 1.0, 0));
    let successes = (stored_alpha - 1.0).max(0.0);
    let failures = (stored_beta - 1.0).max(0.0);
    let prior_n = meta_i64(conn, "thompson.prior_n")?.max(2) as f64;
    let prior_mean = literature_prior_mean(move_id);
    let alpha = prior_n * prior_mean + successes;
    let beta = prior_n * (1.0 - prior_mean) + failures;
    Ok((alpha / (alpha + beta).max(f64::EPSILON), n))
}

fn friction_score(conn: &Connection, p_known: f64, base_rank: i32, rank: i32) -> Result<f64> {
    let uncertainty = 1.0 - p_known.clamp(0.0, 1.0);
    let local_stall = 0.0_f64;
    let hint_delay_bucket = if rank <= base_rank {
        0.0
    } else {
        (rank - base_rank).min(2) as f64 / 2.0
    };
    let scaffold_level = (rank as f64 / (MOVE_IDS.len() as f64 - 1.0)).clamp(0.0, 1.0);
    let score = meta_f64(conn, "friction.w1")? * uncertainty
        + meta_f64(conn, "friction.w2")? * local_stall
        + meta_f64(conn, "friction.w3")? * hint_delay_bucket
        + meta_f64(conn, "friction.w4")? * scaffold_level;
    Ok(score.clamp(0.0, 1.0))
}

fn latest_strategy_state(conn: &Connection) -> Result<String> {
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload_json
             FROM behavior_events
             WHERE type='mental_state'
             ORDER BY at DESC, id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Ok("unknown".to_owned());
    };
    let value: serde_json::Value = serde_json::from_str(&payload)?;
    if value
        .get("strategy_enabled")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Ok("unknown".to_owned());
    }
    Ok(value
        .get("dominant_state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_owned())
}

fn compare_candidates(left: &MoveCandidate, right: &MoveCandidate) -> std::cmp::Ordering {
    right
        .utility
        .total_cmp(&left.utility)
        .then_with(|| right.posterior_mean.total_cmp(&left.posterior_mean))
        .then_with(|| left.friction.total_cmp(&right.friction))
        .then_with(|| right.base.cmp(&left.base))
        .then_with(|| move_rank(left.move_id).cmp(&move_rank(right.move_id)))
}

fn move_id_for_task_type(task_type: &str) -> &'static str {
    match task_type {
        "free_explain" | "explain" => "explain",
        "apply" => "apply",
        "analyze" => "analyze",
        "evaluate" => "evaluate",
        "create" => "create",
        "transfer" | "free_produce" => "transfer",
        _ => "recall",
    }
}

fn move_rank(move_id: &str) -> i32 {
    MOVE_IDS
        .iter()
        .position(|candidate| *candidate == move_id)
        .unwrap_or(0) as i32
}

fn literature_prior_mean(move_id: &str) -> f64 {
    let d_lit = 1.0 - (move_rank(move_id) as f64 / (MOVE_IDS.len() as f64 - 1.0));
    (0.5 + 0.1 * d_lit).clamp(0.2, 0.8)
}

fn phase_strategy_hypothesis(phase_strategy: &str) -> &'static str {
    match phase_strategy {
        "phantom_challenge" => {
            "phantom challenge improves 7d success by confirming true understanding with transfer evidence"
        }
        "settling_probe" => {
            "settling probe improves 7d success by collecting transfer evidence in a new context"
        }
        "regression_recovery" => {
            "regression recovery improves 7d success by lowering friction before rebuilding depth"
        }
        _ => "phase action loop improves 7d success under this phase context",
    }
}

fn random_unit() -> f64 {
    let value = Uuid::new_v4().as_u128();
    (value as f64) / (u128::MAX as f64)
}

fn random_index(len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    (Uuid::new_v4().as_u128() as usize) % len
}
