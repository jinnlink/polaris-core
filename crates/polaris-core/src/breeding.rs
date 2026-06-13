use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::json;
use uuid::Uuid;

use crate::config::{meta_f64, meta_i64};
use crate::error::{PolarisError, Result};
use crate::report::prob_beta_greater;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BredMoveStatus {
    Preregistered,
    Admitted,
    Retired,
}

impl BredMoveStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preregistered => "preregistered",
            Self::Admitted => "admitted",
            Self::Retired => "retired",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "preregistered" => Ok(Self::Preregistered),
            "admitted" => Ok(Self::Admitted),
            "retired" => Ok(Self::Retired),
            other => Err(PolarisError::InvalidParameter {
                key: "bred_moves.status".to_owned(),
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BredMoveInput {
    pub id: String,
    pub candidate_move: String,
    pub incumbent_move: String,
    pub context_hash: String,
    pub task_type: String,
    pub template: String,
    pub mechanisms: Vec<String>,
    pub main_effect_hypothesis: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BredMove {
    pub id: String,
    pub candidate_move: String,
    pub incumbent_move: String,
    pub context_hash: String,
    pub task_type: String,
    pub template: String,
    pub mechanisms: Vec<String>,
    pub main_effect_hypothesis: String,
    pub status: BredMoveStatus,
    pub posterior_win_prob: f64,
    pub candidate_alpha: f64,
    pub candidate_beta: f64,
    pub incumbent_alpha: f64,
    pub incumbent_beta: f64,
    pub n_candidate: i64,
    pub n_incumbent: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BreedingEvaluationSummary {
    pub evaluated: usize,
    pub admitted: usize,
    pub retired: usize,
}

#[derive(Debug, Clone, Copy)]
struct EffectPosterior {
    alpha: f64,
    beta: f64,
    n: i64,
}

#[derive(Debug, Clone, Copy)]
struct PreregistrationPolicy {
    min_n: i64,
    admit_p: f64,
}

pub fn preregister_bred_move(conn: &Connection, input: BredMoveInput) -> Result<BredMove> {
    validate_input(&input)?;

    let min_n = meta_i64(conn, "breeding.min_n")?;
    let admit_p = meta_f64(conn, "breeding.admit_p")?;
    let mrt_epsilon = meta_f64(conn, "mrt.epsilon")?;
    let mechanisms_json = serde_json::to_string(&input.mechanisms)?;
    let prereg = json!({
        "id": input.id,
        "window": "7d",
        "epsilon": mrt_epsilon,
        "candidate_set": [input.candidate_move],
        "incumbent": input.incumbent_move,
        "context_hash": input.context_hash,
        "task_type": input.task_type,
        "mechanisms": input.mechanisms,
        "main_effect_hypothesis": input.main_effect_hypothesis,
        "min_n": min_n,
        "admit_p": admit_p
    });
    let prereg_json = prereg.to_string();

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO bred_moves(
             id, candidate_move, incumbent_move, context_hash, task_type, template,
             mechanisms_json, main_effect_hypothesis, prereg_json, status,
             created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'preregistered',
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                 strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        params![
            input.id,
            input.candidate_move,
            input.incumbent_move,
            input.context_hash,
            input.task_type,
            input.template,
            mechanisms_json,
            input.main_effect_hypothesis,
            prereg_json,
        ],
    )?;

    tx.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, 0, ?3, ?4)",
        params![
            Uuid::new_v4().to_string(),
            prereg_json,
            input.candidate_move,
            input.id,
        ],
    )?;
    tx.commit()?;

    bred_move_by_id(conn, &input.id)
}

pub fn record_bred_move_outcome(
    conn: &Connection,
    prereg_id: &str,
    move_id: &str,
    success: bool,
) -> Result<()> {
    let record = bred_move_by_id(conn, prereg_id)?;
    let is_candidate = if move_id == record.candidate_move {
        true
    } else if move_id == record.incumbent_move {
        false
    } else {
        return Err(PolarisError::InvalidParameter {
            key: "bred_moves.move".to_owned(),
            value: move_id.to_owned(),
        });
    };

    let init_alpha = if success { 2.0 } else { 1.0 };
    let init_beta = if success { 1.0 } else { 2.0 };
    let alpha_delta = if success { 1.0 } else { 0.0 };
    let beta_delta = if success { 0.0 } else { 1.0 };
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO moves_effects(move, context_hash, alpha, beta, n)
         VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT(move, context_hash) DO UPDATE SET
             alpha = alpha + ?5,
             beta = beta + ?6,
             n = n + 1",
        params![
            move_id,
            record.context_hash,
            init_alpha,
            init_beta,
            alpha_delta,
            beta_delta,
        ],
    )?;

    let count_column = if is_candidate {
        "n_candidate"
    } else {
        "n_incumbent"
    };
    tx.execute(
        &format!(
            "UPDATE bred_moves
             SET {count_column} = {count_column} + 1,
                 updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=?1"
        ),
        [prereg_id],
    )?;

    let role = if is_candidate {
        "candidate"
    } else {
        "incumbent"
    };
    let context_json = json!({
        "prereg_id": prereg_id,
        "context_hash": record.context_hash,
        "role": role,
        "outcome": success
    })
    .to_string();
    tx.execute(
        "INSERT INTO mrt_log(id, at, context_json, randomized, move, prereg_id)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, 1, ?3, ?4)",
        params![Uuid::new_v4().to_string(), context_json, move_id, prereg_id,],
    )?;
    tx.commit()?;

    Ok(())
}

pub fn evaluate_bred_moves(conn: &Connection) -> Result<BreedingEvaluationSummary> {
    let mut summary = BreedingEvaluationSummary::default();
    let mut stmt = conn.prepare(
        "SELECT id FROM bred_moves
         WHERE status IN ('preregistered', 'admitted')
         ORDER BY created_at, id",
    )?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for id in ids {
        summary.evaluated += 1;
        let record = bred_move_by_id(conn, &id)?;
        let candidate = posterior_for_move(conn, &record.candidate_move, &record.context_hash)?;
        let incumbent = posterior_for_move(conn, &record.incumbent_move, &record.context_hash)?;
        let win_prob = prob_beta_greater(
            candidate.alpha,
            candidate.beta,
            incumbent.alpha,
            incumbent.beta,
        );

        conn.execute(
            "UPDATE bred_moves
             SET posterior_win_prob=?1,
                 candidate_alpha=?2,
                 candidate_beta=?3,
                 incumbent_alpha=?4,
                 incumbent_beta=?5,
                 n_candidate=?6,
                 n_incumbent=?7,
                 updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=?8",
            params![
                win_prob,
                candidate.alpha,
                candidate.beta,
                incumbent.alpha,
                incumbent.beta,
                candidate.n,
                incumbent.n,
                id,
            ],
        )?;

        let policy = preregistration_policy(conn, &id)?;
        let has_minimum_samples = candidate.n >= policy.min_n && incumbent.n >= policy.min_n;
        if !has_minimum_samples {
            continue;
        }

        match record.status {
            BredMoveStatus::Preregistered => {
                if win_prob > policy.admit_p {
                    conn.execute(
                        "UPDATE bred_moves
                         SET status='admitted',
                             admitted_at=COALESCE(admitted_at, strftime('%Y-%m-%dT%H:%M:%SZ','now')),
                             updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
                         WHERE id=?1",
                        [id.as_str()],
                    )?;
                    summary.admitted += 1;
                }
            }
            BredMoveStatus::Admitted => {
                if win_prob < meta_f64(conn, "breeding.retire_p")? {
                    conn.execute(
                        "UPDATE bred_moves
                         SET status='retired',
                             retired_at=COALESCE(retired_at, strftime('%Y-%m-%dT%H:%M:%SZ','now')),
                             updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
                         WHERE id=?1",
                        [id.as_str()],
                    )?;
                    summary.retired += 1;
                }
            }
            BredMoveStatus::Retired => {}
        }
    }

    Ok(summary)
}

pub fn admitted_bred_moves(conn: &Connection, context_hash: &str) -> Result<Vec<BredMove>> {
    let mut stmt = conn.prepare(
        "SELECT id, candidate_move, incumbent_move, context_hash, task_type, template,
                mechanisms_json, main_effect_hypothesis, status, posterior_win_prob,
                candidate_alpha, candidate_beta, incumbent_alpha, incumbent_beta,
                n_candidate, n_incumbent
         FROM bred_moves
         WHERE status='admitted' AND context_hash=?1
         ORDER BY posterior_win_prob DESC, admitted_at, id",
    )?;
    let rows = stmt
        .query_map([context_hash], move_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into);
    rows
}

fn preregistration_policy(conn: &Connection, id: &str) -> Result<PreregistrationPolicy> {
    let prereg_json: String = conn.query_row(
        "SELECT prereg_json FROM bred_moves WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    let prereg: serde_json::Value = serde_json::from_str(&prereg_json)?;
    Ok(PreregistrationPolicy {
        min_n: prereg
            .get("min_n")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(meta_i64(conn, "breeding.min_n")?),
        admit_p: prereg
            .get("admit_p")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(meta_f64(conn, "breeding.admit_p")?),
    })
}

fn bred_move_by_id(conn: &Connection, id: &str) -> Result<BredMove> {
    conn.query_row(
        "SELECT id, candidate_move, incumbent_move, context_hash, task_type, template,
                mechanisms_json, main_effect_hypothesis, status, posterior_win_prob,
                candidate_alpha, candidate_beta, incumbent_alpha, incumbent_beta,
                n_candidate, n_incumbent
         FROM bred_moves
         WHERE id=?1",
        [id],
        move_from_row,
    )
    .map_err(Into::into)
}

fn move_from_row(row: &Row<'_>) -> rusqlite::Result<BredMove> {
    let mechanisms_json: String = row.get(6)?;
    let status: String = row.get(8)?;
    let mechanisms = serde_json::from_str(&mechanisms_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let status = BredMoveStatus::from_str(&status).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
    })?;
    Ok(BredMove {
        id: row.get(0)?,
        candidate_move: row.get(1)?,
        incumbent_move: row.get(2)?,
        context_hash: row.get(3)?,
        task_type: row.get(4)?,
        template: row.get(5)?,
        mechanisms,
        main_effect_hypothesis: row.get(7)?,
        status,
        posterior_win_prob: row.get(9)?,
        candidate_alpha: row.get(10)?,
        candidate_beta: row.get(11)?,
        incumbent_alpha: row.get(12)?,
        incumbent_beta: row.get(13)?,
        n_candidate: row.get(14)?,
        n_incumbent: row.get(15)?,
    })
}

fn posterior_for_move(
    conn: &Connection,
    move_id: &str,
    context_hash: &str,
) -> Result<EffectPosterior> {
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
    let prior_n = meta_i64(conn, "thompson.prior_n")?.max(2);
    let prior_successes = prior_n / 2;
    let prior_failures = prior_n - prior_successes;
    Ok(EffectPosterior {
        alpha: prior_successes as f64 + successes,
        beta: prior_failures as f64 + failures,
        n,
    })
}

fn validate_input(input: &BredMoveInput) -> Result<()> {
    for (key, value) in [
        ("breeding.id", input.id.as_str()),
        ("breeding.candidate_move", input.candidate_move.as_str()),
        ("breeding.incumbent_move", input.incumbent_move.as_str()),
        ("breeding.context_hash", input.context_hash.as_str()),
        ("breeding.task_type", input.task_type.as_str()),
        ("breeding.template", input.template.as_str()),
        (
            "breeding.main_effect_hypothesis",
            input.main_effect_hypothesis.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(PolarisError::InvalidParameter {
                key: key.to_owned(),
                value: value.to_owned(),
            });
        }
    }
    if input.candidate_move == input.incumbent_move {
        return Err(PolarisError::InvalidParameter {
            key: "breeding.candidate_move".to_owned(),
            value: input.candidate_move.clone(),
        });
    }
    if input.mechanisms.is_empty() || input.mechanisms.iter().any(|item| item.trim().is_empty()) {
        return Err(PolarisError::InvalidParameter {
            key: "breeding.mechanisms".to_owned(),
            value: serde_json::to_string(&input.mechanisms).unwrap_or_default(),
        });
    }
    Ok(())
}
