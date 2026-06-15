use rusqlite::{params, Connection, OptionalExtension};

use crate::config::{meta_f64, meta_i64};
use crate::error::{PolarisError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct MirtParams {
    pub k: usize,
    pub eta: f64,
    pub step_cap: f64,
    pub adagrad_epsilon: f64,
    pub fuse_n0: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LatentPrediction {
    pub concept_id: String,
    pub task_type: String,
    pub p_hat: f64,
    pub theta_version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FusedPKnown {
    pub concept_id: String,
    pub task_type: String,
    pub p_known: f64,
    pub bkt_p_known: f64,
    pub mirt_p_hat: f64,
    pub lambda: f64,
}

impl MirtParams {
    pub fn from_conn(conn: &Connection) -> Result<Self> {
        let k = meta_i64(conn, "latent.k")?;
        let k_max = meta_i64(conn, "latent.k_max")?;
        if k <= 0 || k > k_max {
            return Err(PolarisError::InvalidParameter {
                key: "latent.k".to_owned(),
                value: k.to_string(),
            });
        }
        let adagrad_epsilon = meta_f64(conn, "mirt.adagrad_epsilon")?;
        if adagrad_epsilon <= 0.0 {
            return Err(PolarisError::InvalidParameter {
                key: "mirt.adagrad_epsilon".to_owned(),
                value: adagrad_epsilon.to_string(),
            });
        }
        Ok(Self {
            k: k as usize,
            eta: meta_f64(conn, "mirt.eta")?,
            step_cap: meta_f64(conn, "mirt.step_cap")?,
            adagrad_epsilon,
            fuse_n0: meta_f64(conn, "mirt.fuse_n0")?,
        })
    }
}

pub fn encode_vector(values: &[f64]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| (*value as f32).to_le_bytes())
        .collect()
}

pub fn decode_vector(blob: &[u8]) -> Result<Vec<f64>> {
    if blob.is_empty() || !blob.len().is_multiple_of(4) {
        return Err(PolarisError::InvalidParameter {
            key: "vector_blob".to_owned(),
            value: format!("{} bytes", blob.len()),
        });
    }
    Ok(blob
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f64)
        .collect())
}

pub fn initial_track_q_blob(conn: &Connection) -> Result<Vec<u8>> {
    let params = MirtParams::from_conn(conn)?;
    Ok(encode_vector(&initial_track_q(&params)))
}

pub fn initial_pack_q_blob(conn: &Connection, pack_id: &str) -> Result<Vec<u8>> {
    let params = MirtParams::from_conn(conn)?;
    let label = pack_dimension_label(pack_id);
    let mut dims = latent_dims(conn)?;
    let index = match dims.iter().position(|item| item == &label) {
        Some(index) if index < params.k => index,
        Some(index) => {
            return Err(PolarisError::InvalidParameter {
                key: "latent.dims".to_owned(),
                value: format!("{label} index {index} >= latent.k {}", params.k),
            });
        }
        None => {
            if dims.len() >= params.k {
                return Err(PolarisError::InvalidParameter {
                    key: "latent.dims".to_owned(),
                    value: format!("cannot add {label}; latent.k {} is full", params.k),
                });
            }
            dims.push(label);
            conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES ('latent.dims', ?1)",
                [serde_json::to_string(&dims)?],
            )?;
            dims.len() - 1
        }
    };

    let mut q = vec![0.0; params.k];
    q[index] = 1.0;
    Ok(encode_vector(&q))
}

pub fn ensure_theta(conn: &Connection) -> Result<()> {
    let params = MirtParams::from_conn(conn)?;
    let zero = vec![0.0; params.k];
    let zero_blob = encode_vector(&zero);
    conn.execute(
        "INSERT OR IGNORE INTO theta(id, vec, g2, version, updated_at)
         VALUES (1, ?1, ?2, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        params![zero_blob, encode_vector(&zero)],
    )?;
    let g2_blob = conn
        .query_row("SELECT g2 FROM theta WHERE id=1", [], |row| {
            row.get::<_, Option<Vec<u8>>>(0)
        })
        .optional()?
        .flatten();
    let needs_g2 = match g2_blob {
        Some(blob) => decode_vector(&blob)
            .map(|values| values.len() != params.k)
            .unwrap_or(true),
        None => true,
    };
    if needs_g2 {
        conn.execute(
            "UPDATE theta
             SET g2=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=1",
            [encode_vector(&zero)],
        )?;
    }
    Ok(())
}

pub fn latent_prediction(
    conn: &Connection,
    concept_id: &str,
    task_type: &str,
) -> Result<LatentPrediction> {
    let (q, b_difficulty) = concept_q_and_b(conn, concept_id)?;
    let (theta, version) = theta(conn)?;
    if q.len() != theta.len() {
        return Err(PolarisError::InvalidParameter {
            key: "q/theta".to_owned(),
            value: format!("{} != {}", q.len(), theta.len()),
        });
    }

    let d_t = task_difficulty(conn, task_type)?;
    let logit = dot(&q, &theta) - b_difficulty - d_t;
    Ok(LatentPrediction {
        concept_id: concept_id.to_owned(),
        task_type: task_type.to_owned(),
        p_hat: sigmoid(logit),
        theta_version: version,
    })
}

pub fn update_theta_for_attempt(conn: &Connection, attempt_id: &str) -> Result<()> {
    ensure_theta(conn)?;
    let attempt = conn
        .query_row(
            "SELECT concept_id, COALESCE(task_type, 'recall'), final_score
             FROM attempts
             WHERE id=?1",
            [attempt_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))?;
    let (concept_id, task_type, Some(final_score)) = attempt else {
        return Ok(());
    };

    let params = MirtParams::from_conn(conn)?;
    let (q, b_difficulty) = concept_q_and_b(conn, &concept_id)?;
    let (mut theta, mut g2, version) = theta_state(conn)?;
    if q.len() != theta.len() {
        return Err(PolarisError::InvalidParameter {
            key: "q/theta".to_owned(),
            value: format!("{} != {}", q.len(), theta.len()),
        });
    }
    if g2.len() != theta.len() {
        return Err(PolarisError::InvalidParameter {
            key: "theta.g2".to_owned(),
            value: format!("{} != {}", g2.len(), theta.len()),
        });
    }

    let d_t = task_difficulty(conn, &task_type)?;
    let p_hat = sigmoid(dot(&q, &theta) - b_difficulty - d_t);
    let residual = final_score.clamp(0.0, 1.0) - p_hat;
    for ((theta_k, g2_k), q_k) in theta.iter_mut().zip(g2.iter_mut()).zip(q.iter()) {
        let gradient = residual * q_k;
        *g2_k += gradient * gradient;
        let delta = (params.eta * gradient / (*g2_k + params.adagrad_epsilon).sqrt())
            .clamp(-params.step_cap, params.step_cap);
        *theta_k += delta;
    }

    conn.execute(
        "UPDATE theta
         SET vec=?1, g2=?2, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=1",
        params![encode_vector(&theta), encode_vector(&g2)],
    )?;
    conn.execute(
        "UPDATE attempts SET theta_version=?1 WHERE id=?2",
        params![version, attempt_id],
    )?;
    Ok(())
}

pub fn fused_p_known(conn: &Connection, concept_id: &str, task_type: &str) -> Result<FusedPKnown> {
    let prediction = latent_prediction(conn, concept_id, task_type)?;
    let (bkt_p_known, attempt_count): (f64, i64) = conn.query_row(
        "SELECT
            COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
            COALESCE(ms.attempt_count, (SELECT COUNT(*) FROM attempts a WHERE a.concept_id=c.id), 0)
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         WHERE c.id=?1",
        [concept_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let params = MirtParams::from_conn(conn)?;
    let n = (attempt_count.max(0)) as f64;
    let lambda = if params.fuse_n0 <= 0.0 {
        1.0
    } else {
        n / (n + params.fuse_n0)
    }
    .clamp(0.0, 1.0);
    let p_known = lambda * bkt_p_known + (1.0 - lambda) * prediction.p_hat;

    Ok(FusedPKnown {
        concept_id: concept_id.to_owned(),
        task_type: task_type.to_owned(),
        p_known,
        bkt_p_known,
        mirt_p_hat: prediction.p_hat,
        lambda,
    })
}

fn initial_track_q(params: &MirtParams) -> Vec<f64> {
    let mut q = vec![0.0; params.k];
    q[0] = 1.0;
    q
}

fn latent_dims(conn: &Connection) -> Result<Vec<String>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='latent.dims'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(json) = json else {
        return Ok(Vec::new());
    };
    let dims: Vec<String> = serde_json::from_str(&json)?;
    if dims.iter().any(|item| item.trim().is_empty()) {
        return Err(PolarisError::InvalidParameter {
            key: "latent.dims".to_owned(),
            value: json,
        });
    }
    Ok(dims)
}

fn pack_dimension_label(pack_id: &str) -> String {
    format!("pack:{pack_id}")
}

fn concept_q_and_b(conn: &Connection, concept_id: &str) -> Result<(Vec<f64>, f64)> {
    let (q_blob, b_difficulty): (Option<Vec<u8>>, f64) = conn
        .query_row(
            "SELECT q, COALESCE(b_difficulty, 0.0) FROM concepts WHERE id=?1",
            [concept_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| PolarisError::MissingConcept(concept_id.to_owned()))?;
    let q_blob = q_blob.ok_or_else(|| PolarisError::InvalidParameter {
        key: "concept.q".to_owned(),
        value: concept_id.to_owned(),
    })?;
    Ok((decode_vector(&q_blob)?, b_difficulty))
}

fn theta(conn: &Connection) -> Result<(Vec<f64>, i64)> {
    let (blob, version): (Vec<u8>, i64) =
        conn.query_row("SELECT vec, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    Ok((decode_vector(&blob)?, version))
}

fn theta_state(conn: &Connection) -> Result<(Vec<f64>, Vec<f64>, i64)> {
    let (theta_blob, g2_blob, version): (Vec<u8>, Vec<u8>, i64) =
        conn.query_row("SELECT vec, g2, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    Ok((
        decode_vector(&theta_blob)?,
        decode_vector(&g2_blob)?,
        version,
    ))
}

fn task_difficulty(conn: &Connection, task_type: &str) -> Result<f64> {
    let key = match task_type {
        "free_explain" | "explain" => "free_explain",
        other => other,
    };
    meta_f64(conn, &format!("mirt.d.{key}"))
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn sigmoid(logit: f64) -> f64 {
    let x = logit.clamp(-10.0, 10.0);
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_blob_round_trips_f32_little_endian() {
        let values = vec![1.0, -0.5, 0.25];
        let blob = encode_vector(&values);

        assert_eq!(blob.len(), 12);
        assert_eq!(decode_vector(&blob).unwrap(), values);
    }
}
