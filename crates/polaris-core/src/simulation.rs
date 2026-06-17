use std::collections::BTreeMap;
use std::env;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::config::{meta_f64, meta_i64};
use crate::engine::{Engine, SubmitInput};
use crate::error::{PolarisError, Result};
use crate::mirt::{decode_vector, encode_vector};
use crate::phase::Phase;

static MODEL_ENV_LOCK: Mutex<()> = Mutex::new(());

const LLM_ENV_KEYS: [&str; 6] = [
    "POLARIS_LLM_FAST_BASE_URL",
    "POLARIS_LLM_FAST_MODEL",
    "POLARIS_LLM_FAST_API_KEY",
    "POLARIS_LLM_STRONG_BASE_URL",
    "POLARIS_LLM_STRONG_MODEL",
    "POLARIS_LLM_STRONG_API_KEY",
];

const SANDBOX_ENV_KEYS: [&str; 10] = [
    "POLARIS_TIER0_ONLY",
    "POLARIS_LLM_FAST_BASE_URL",
    "POLARIS_LLM_FAST_MODEL",
    "POLARIS_LLM_FAST_API_KEY",
    "POLARIS_LLM_STRONG_BASE_URL",
    "POLARIS_LLM_STRONG_MODEL",
    "POLARIS_LLM_STRONG_API_KEY",
    "POLARIS_EMBED_BASE_URL",
    "POLARIS_EMBED_MODEL",
    "POLARIS_EMBED_API_KEY",
];

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualLearner {
    pub ability: Vec<f64>,
    pub noise: f64,
    pub confidence_bias: f64,
    pub fatigue_rate: f64,
    pub session_pattern: Vec<usize>,
}

impl VirtualLearner {
    pub fn strong(k: usize) -> Self {
        Self {
            ability: vec![1.5; k],
            noise: 0.1,
            confidence_bias: 0.0,
            fatigue_rate: 0.06,
            session_pattern: vec![3; 30],
        }
    }

    pub fn weak(k: usize) -> Self {
        Self {
            ability: vec![-0.5; k],
            noise: 0.3,
            confidence_bias: 0.3,
            fatigue_rate: 0.08,
            session_pattern: vec![2; 30],
        }
    }

    pub fn mixed(k: usize) -> Self {
        Self {
            ability: (0..k)
                .map(|index| if index % 2 == 0 { 1.0 } else { -0.5 })
                .collect(),
            noise: 0.2,
            confidence_bias: 0.0,
            fatigue_rate: 0.07,
            session_pattern: vec![2; 30],
        }
    }

    fn sessions_for_day(&self, day: usize) -> usize {
        self.session_pattern
            .get(day)
            .copied()
            .or_else(|| self.session_pattern.last().copied())
            .unwrap_or(1)
            .max(1)
    }

    fn respond(
        &self,
        profile: &ConceptProfile,
        task_type: &str,
        fatigue_factor: f64,
        rng: &mut DeterministicRng,
        conn: &rusqlite::Connection,
    ) -> Result<VirtualResponse> {
        if profile.q.len() != self.ability.len() {
            return Err(PolarisError::InvalidParameter {
                key: "learner.ability".to_owned(),
                value: format!("{} != q{}", self.ability.len(), profile.q.len()),
            });
        }

        let raw_logit = dot(&profile.q, &self.ability) - profile.b_difficulty;
        let task_logit = raw_logit - task_difficulty(conn, task_type)?;
        let noisy_logit = task_logit + rng.normalish() * self.noise.max(0.0);
        let p_correct = sigmoid(noisy_logit);
        let correct = rng.next_unit() <= p_correct;
        let score = if correct {
            0.82 + 0.15 * p_correct
        } else {
            0.20 + 0.25 * p_correct
        }
        .clamp(0.0, 1.0);
        let confidence = (1.0 + 4.0 * sigmoid(raw_logit + self.confidence_bias))
            .round()
            .clamp(1.0, 5.0) as i32;
        let difficulty_factor = 1.0 - p_correct;
        let latency_noise = rng.normalish() * 500.0;
        let latency_ms = (1200.0 * (1.0 + fatigue_factor) * (1.0 + difficulty_factor)
            + latency_noise)
            .max(500.0)
            .round() as i64;
        let hint_count = if score < 0.40 {
            2
        } else if score < 0.75 {
            1
        } else {
            0
        };

        Ok(VirtualResponse {
            score,
            self_confidence: confidence,
            latency_ms,
            hint_count,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationReport {
    pub daily_summaries: Vec<DailySummary>,
    pub deadlock_days: Vec<usize>,
    pub initial_mean_p_known: f64,
    pub final_mean_p_known: f64,
    pub mean_p_known_slope: f64,
    pub initial_abs_calib_gap: f64,
    pub final_abs_calib_gap: f64,
    pub final_theta_cosine: f64,
    pub final_phase_counts: PhaseCounts,
    pub early_transfer_violations: Vec<EarlyTransferViolation>,
}

impl SimulationReport {
    pub fn has_hmm_state_lock(&self) -> bool {
        let mut last_state = "";
        let mut streak = 0_usize;
        for summary in &self.daily_summaries {
            if summary.hmm_sample_count == 0 || summary.dominant_hmm_share <= 0.90 {
                streak = 0;
                last_state = "";
                continue;
            }
            if summary.dominant_hmm_state == last_state {
                streak += 1;
            } else {
                last_state = &summary.dominant_hmm_state;
                streak = 1;
            }
            if streak > 5 {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailySummary {
    pub day: usize,
    pub mean_p_known: f64,
    pub active_concepts: usize,
    pub dominant_hmm_state: String,
    pub dominant_hmm_share: f64,
    pub hmm_sample_count: usize,
    pub phase_distribution: PhaseCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlyTransferViolation {
    pub concept_id: String,
    pub attempt_count: i64,
    pub phase: Phase,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PhaseCounts {
    counts: BTreeMap<String, usize>,
}

impl PhaseCounts {
    pub fn get(&self, phase: &Phase) -> Option<&usize> {
        self.counts.get(phase.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    fn add(&mut self, phase: &str, count: usize) {
        self.counts.insert(phase.to_owned(), count);
    }
}

pub fn simulate_learning(
    learner: &VirtualLearner,
    days: usize,
    engine: &mut Engine,
) -> Result<SimulationReport> {
    let _env_guard = ExternalModelEnvGuard::llm_only();
    simulate_learning_inner(learner, days, engine, true)
}

pub fn simulate_learning_quiet(
    learner: &VirtualLearner,
    days: usize,
    engine: &mut Engine,
) -> Result<SimulationReport> {
    let _env_guard = ExternalModelEnvGuard::llm_only();
    simulate_learning_inner(learner, days, engine, false)
}

pub(crate) fn simulate_learning_quiet_under_env_guard(
    learner: &VirtualLearner,
    days: usize,
    engine: &mut Engine,
) -> Result<SimulationReport> {
    simulate_learning_inner(learner, days, engine, false)
}

fn simulate_learning_inner(
    learner: &VirtualLearner,
    days: usize,
    engine: &mut Engine,
    emit_daily_summary: bool,
) -> Result<SimulationReport> {
    validate_learner(learner, engine)?;
    seed_simulation_latent_surface(learner.ability.len(), engine)?;

    let initial_mean_p_known = mean_p_known(engine)?;
    let mut initial_abs_calib_gap = None;
    let mut daily_summaries = Vec::new();
    let mut deadlock_days = Vec::new();
    let mut rng = DeterministicRng::from_learner(learner);

    for day in 0..days {
        let mut day_task_count = 0_usize;
        let mut day_attempt_index = 0_usize;
        for session_index in 0..learner.sessions_for_day(day) {
            let session_id = format!("sim-day-{day:02}-session-{session_index:02}");
            let batch = engine.get_interleaved_batch(3)?;
            if batch.is_empty() {
                deadlock_days.push(day + 1);
                continue;
            }

            for (slot, assignment) in batch.iter().enumerate() {
                let timestamp = simulated_timestamp(day, session_index, slot);
                let profile = concept_profile(engine, &assignment.concept_id)?;
                let fatigue_factor = learner.fatigue_rate * day_attempt_index as f64;
                let response = learner.respond(
                    &profile,
                    &assignment.task_type,
                    fatigue_factor,
                    &mut rng,
                    engine.conn(),
                )?;
                let receipt = engine.submit(SubmitInput {
                    session_id: session_id.clone(),
                    concept_id: assignment.concept_id.clone(),
                    task_type: assignment.task_type.clone(),
                    prompt_text: assignment.template.clone(),
                    response_text: format!(
                        "virtual response score={:.3} concept={} task={}",
                        response.score, assignment.concept_id, assignment.task_type
                    ),
                    self_confidence: response.self_confidence,
                    latency_ms: response.latency_ms,
                    hint_count: response.hint_count,
                })?;
                stamp_attempt(engine, &receipt.attempt_id, &session_id, &timestamp)?;
                engine.apply_final_score(&receipt.attempt_id, response.score)?;
                stamp_attempt(engine, &receipt.attempt_id, &session_id, &timestamp)?;
                clear_simulated_grade_queue(engine, &receipt.attempt_id)?;
                day_task_count += 1;
                day_attempt_index += 1;
            }
        }

        if day_task_count == 0 && !deadlock_days.contains(&(day + 1)) {
            deadlock_days.push(day + 1);
        }
        let summary = daily_summary(engine, day + 1, &simulated_date(day))?;
        if initial_abs_calib_gap.is_none() && summary.active_concepts > 0 {
            initial_abs_calib_gap = Some(abs_calib_gap(engine)?);
        }
        if emit_daily_summary {
            println!(
                "day {:02}: mean_p_known={:.3} active_concepts={} dominant_hmm={} phase_distribution={:?}",
                summary.day,
                summary.mean_p_known,
                summary.active_concepts,
                summary.dominant_hmm_state,
                summary.phase_distribution
            );
        }
        daily_summaries.push(summary);
    }

    let final_mean_p_known = mean_p_known(engine)?;
    let first_mean = daily_summaries
        .first()
        .map(|summary| summary.mean_p_known)
        .unwrap_or(initial_mean_p_known);
    let last_mean = daily_summaries
        .last()
        .map(|summary| summary.mean_p_known)
        .unwrap_or(final_mean_p_known);
    let denominator = (daily_summaries.len().saturating_sub(1)).max(1) as f64;
    let mean_p_known_slope = (last_mean - first_mean) / denominator;

    Ok(SimulationReport {
        daily_summaries,
        deadlock_days,
        initial_mean_p_known,
        final_mean_p_known,
        mean_p_known_slope,
        initial_abs_calib_gap: initial_abs_calib_gap.unwrap_or(0.0),
        final_abs_calib_gap: abs_calib_gap(engine)?,
        final_theta_cosine: theta_cosine(engine, &learner.ability)?,
        final_phase_counts: phase_counts(engine)?,
        early_transfer_violations: early_transfer_violations(engine)?,
    })
}

#[derive(Debug, Clone)]
struct ConceptProfile {
    q: Vec<f64>,
    b_difficulty: f64,
}

#[derive(Debug, Clone, Copy)]
struct VirtualResponse {
    score: f64,
    self_confidence: i32,
    latency_ms: i64,
    hint_count: i64,
}

fn validate_learner(learner: &VirtualLearner, engine: &Engine) -> Result<()> {
    let latent_k = meta_i64(engine.conn(), "latent.k")? as usize;
    if learner.ability.len() != latent_k {
        return Err(PolarisError::InvalidParameter {
            key: "learner.ability".to_owned(),
            value: format!("{} != latent.k {}", learner.ability.len(), latent_k),
        });
    }
    Ok(())
}

fn seed_simulation_latent_surface(k: usize, engine: &Engine) -> Result<()> {
    let mut stmt = engine
        .conn()
        .prepare("SELECT id FROM concepts ORDER BY seed_order ASC, id ASC")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (index, id) in ids.iter().enumerate() {
        // Early packs use fallback q[0]; P04E needs a K-dimensional surface to verify theta tracking.
        let mut q = vec![0.0; k];
        q[index % k] = 1.0;
        engine.conn().execute(
            "UPDATE concepts SET q=?1, b_difficulty=-0.7 WHERE id=?2",
            params![encode_vector(&q), id],
        )?;
    }
    Ok(())
}

fn concept_profile(engine: &Engine, concept_id: &str) -> Result<ConceptProfile> {
    let (q_blob, b_difficulty): (Vec<u8>, f64) = engine.conn().query_row(
        "SELECT q, COALESCE(b_difficulty, 0.0) FROM concepts WHERE id=?1",
        [concept_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(ConceptProfile {
        q: decode_vector(&q_blob)?,
        b_difficulty,
    })
}

fn stamp_attempt(
    engine: &Engine,
    attempt_id: &str,
    session_id: &str,
    timestamp: &str,
) -> Result<()> {
    engine.conn().execute(
        "UPDATE sessions SET started_at=?1 WHERE id=?2",
        params![timestamp, session_id],
    )?;
    engine.conn().execute(
        "UPDATE attempts
         SET created_at=?1,
             graded_at=CASE WHEN final_score IS NULL THEN graded_at ELSE ?1 END
         WHERE id=?2",
        params![timestamp, attempt_id],
    )?;
    engine.conn().execute(
        "UPDATE behavior_events
         SET at=?1
         WHERE session_id=?2
            OR json_extract(payload_json, '$.attempt_id')=?3",
        params![timestamp, session_id, attempt_id],
    )?;
    Ok(())
}

fn clear_simulated_grade_queue(engine: &Engine, attempt_id: &str) -> Result<()> {
    engine
        .conn()
        .execute("DELETE FROM grade_queue WHERE attempt_id=?1", [attempt_id])?;
    Ok(())
}

fn daily_summary(engine: &Engine, day: usize, date: &str) -> Result<DailySummary> {
    let (dominant_hmm_state, dominant_hmm_share, hmm_sample_count) =
        dominant_hmm_state_for_date(engine, date)?;
    Ok(DailySummary {
        day,
        mean_p_known: mean_p_known(engine)?,
        active_concepts: active_concepts_through_date(engine, date)?,
        dominant_hmm_state,
        dominant_hmm_share,
        hmm_sample_count,
        phase_distribution: phase_counts(engine)?,
    })
}

fn mean_p_known(engine: &Engine) -> Result<f64> {
    engine
        .conn()
        .query_row(
            "SELECT COALESCE(AVG(COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL))), 0.0)
             FROM concepts c
             LEFT JOIN mastery_states ms ON ms.concept_id=c.id",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn abs_calib_gap(engine: &Engine) -> Result<f64> {
    engine
        .conn()
        .query_row(
            "SELECT COALESCE(AVG(ABS(calib_gap)), 0.0)
             FROM mastery_states
             WHERE attempt_count > 0",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn active_concepts_through_date(engine: &Engine, date: &str) -> Result<usize> {
    let count: i64 = engine.conn().query_row(
        "SELECT COUNT(DISTINCT concept_id)
         FROM attempts
         WHERE substr(created_at, 1, 10) <= ?1",
        [date],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as usize)
}

fn phase_counts(engine: &Engine) -> Result<PhaseCounts> {
    let mut stmt = engine.conn().prepare(
        "SELECT COALESCE(ms.phase, 'undetermined'), COUNT(*)
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         GROUP BY COALESCE(ms.phase, 'undetermined')
         ORDER BY 1",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut counts = PhaseCounts::default();
    for row in rows {
        let (phase, count) = row?;
        counts.add(&phase, count.max(0) as usize);
    }
    Ok(counts)
}

fn dominant_hmm_state_for_date(engine: &Engine, date: &str) -> Result<(String, f64, usize)> {
    let mut stmt = engine.conn().prepare(
        "SELECT COALESCE(json_extract(be.payload_json, '$.dominant_state'), 'unknown') AS state,
                COUNT(*)
         FROM behavior_events be
         JOIN attempts a
           ON json_extract(be.payload_json, '$.attempt_id') = a.id
         WHERE be.type='mental_state'
           AND json_extract(be.payload_json, '$.score_source')='final'
           AND substr(a.created_at, 1, 10)=?1
         GROUP BY state
         ORDER BY COUNT(*) DESC, state ASC",
    )?;
    let rows = stmt
        .query_map([date], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let total = rows.iter().map(|(_, count)| *count).sum::<i64>().max(0) as usize;
    let Some((state, count)) = rows.first() else {
        return Ok(("none".to_owned(), 0.0, 0));
    };
    let share = if total == 0 {
        0.0
    } else {
        (*count as f64 / total as f64).clamp(0.0, 1.0)
    };
    Ok((state.clone(), share, total))
}

fn early_transfer_violations(engine: &Engine) -> Result<Vec<EarlyTransferViolation>> {
    let mut stmt = engine.conn().prepare(
        "SELECT concept_id, attempt_count, phase
         FROM mastery_states
         WHERE attempt_count < 5 AND phase IN ('transfer', 'generation')
         ORDER BY concept_id ASC",
    )?;
    let violations = stmt
        .query_map([], |row| {
            let phase_text: String = row.get(2)?;
            Ok(EarlyTransferViolation {
                concept_id: row.get(0)?,
                attempt_count: row.get(1)?,
                phase: Phase::parse(&phase_text).unwrap_or(Phase::Undetermined),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(violations)
}

fn theta_cosine(engine: &Engine, ability: &[f64]) -> Result<f64> {
    let blob: Option<Vec<u8>> = engine
        .conn()
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .optional()?;
    let Some(blob) = blob else {
        return Ok(0.0);
    };
    let theta = decode_vector(&blob)?;
    Ok(cosine(&theta, ability))
}

fn task_difficulty(conn: &rusqlite::Connection, task_type: &str) -> Result<f64> {
    let key = match task_type {
        "free_explain" | "explain" => "free_explain",
        other => other,
    };
    meta_f64(conn, &format!("mirt.d.{key}"))
}

fn simulated_date(day: usize) -> String {
    format!("2026-01-{day:02}", day = day + 1)
}

fn simulated_timestamp(day: usize, session_index: usize, slot: usize) -> String {
    let minute = 8 * 60 + session_index * 45 + slot * 5;
    format!(
        "{}T{:02}:{:02}:00Z",
        simulated_date(day),
        minute / 60,
        minute % 60
    )
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn cosine(left: &[f64], right: &[f64]) -> f64 {
    let numerator = dot(left, right);
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm <= f64::EPSILON || right_norm <= f64::EPSILON {
        0.0
    } else {
        numerator / (left_norm * right_norm)
    }
}

fn sigmoid(logit: f64) -> f64 {
    let x = logit.clamp(-10.0, 10.0);
    1.0 / (1.0 + (-x).exp())
}

#[derive(Debug, Clone)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn from_learner(learner: &VirtualLearner) -> Self {
        let ability_hash = learner
            .ability
            .iter()
            .enumerate()
            .fold(0_u64, |acc, (index, value)| {
                acc ^ (((value.to_bits()).rotate_left((index % 63) as u32))
                    .wrapping_add(index as u64 + 0x9E37_79B9))
            });
        Self {
            state: ability_hash ^ learner.noise.to_bits() ^ 0xA5A5_5A5A_D3C1_B2A0,
        }
    }

    fn next_unit(&mut self) -> f64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let value = self.state >> 11;
        (value as f64) / ((1_u64 << 53) as f64)
    }

    fn normalish(&mut self) -> f64 {
        (0..6).map(|_| self.next_unit()).sum::<f64>() - 3.0
    }
}

pub(crate) struct ExternalModelEnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<String>)>,
}

impl ExternalModelEnvGuard {
    fn llm_only() -> Self {
        Self::acquire(&LLM_ENV_KEYS, false)
    }

    pub(crate) fn sandbox() -> Self {
        Self::acquire(&SANDBOX_ENV_KEYS, true)
    }

    fn acquire(keys: &'static [&'static str], tier0_only: bool) -> Self {
        let lock = MODEL_ENV_LOCK.lock().expect("model env lock poisoned");
        let saved = keys
            .iter()
            .map(|key| (*key, env::var(key).ok()))
            .collect::<Vec<_>>();
        for key in keys {
            if tier0_only && *key == "POLARIS_TIER0_ONLY" {
                env::set_var(key, "1");
            } else {
                env::remove_var(key);
            }
        }
        Self { _lock: lock, saved }
    }
}

impl Drop for ExternalModelEnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}
