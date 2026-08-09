use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::config::{meta_f64, meta_i64};
use crate::error::{PolarisError, Result};
use crate::profile::{
    global_profile_settings, profile_instruments, record_profile_validation_run,
    store_profile_dimension, ProfileDimension, ProfileDimensionInput, ProfileGateStatus,
    ProfileScope, ProfileValidationRunInput,
};

const EMA_OFFER_EVENT: &str = "profile_ema_offer";
const EMA_DECISION_EVENT: &str = "profile_ema_decision";
const PROFILE_MODEL_VERSION: &str = "profile-estimation-v1";
const REQUIRED_SLOW_DIMENSIONS: [&str; 7] = [
    "intellect",
    "competence",
    "achievement_striving",
    "self_discipline",
    "goal_orientation",
    "attribution_tendency",
    "self_efficacy",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileEmaStatus {
    Offered,
    Disabled,
    DisclosureRequired,
    Paused,
    SessionNotClosed,
    AlreadyOffered,
    FlowSuppressed,
    DailyLimit,
    WeeklyLimit,
    NoItems,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEmaPrompt {
    pub instrument_id: String,
    pub instrument_version: String,
    pub item_id: String,
    pub locale: String,
    pub admin_mode: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEmaOffer {
    pub status: ProfileEmaStatus,
    pub event_id: Option<String>,
    pub prompt: Option<ProfileEmaPrompt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileBehaviorSnapshot {
    pub calibration_mean_gap: Option<f64>,
    pub active_gu_patterns: BTreeMap<String, i64>,
    pub move_effect_observations: i64,
    pub valid_session_count: i64,
    pub average_session_attempts: Option<f64>,
    pub average_session_minutes: Option<f64>,
    pub abandon_event_count: i64,
    pub abandon_after_hint_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileMonthlyUpdate {
    pub status: String,
    pub model_version: String,
    pub dimensions: Vec<ProfileDimension>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileValidationFold {
    pub baseline_logloss: f64,
    pub candidate_logloss: f64,
    pub baseline_brier: f64,
    pub candidate_brier: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileValidationInput {
    pub id: String,
    pub scope: ProfileScope,
    pub scope_id: Option<String>,
    pub dimension_key: String,
    pub observed_weeks: i64,
    pub outcome_count: i64,
    pub valid_session_count: i64,
    pub cross_domain_pack_count: i64,
    pub folds: Vec<ProfileValidationFold>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileGateEvaluation {
    pub status: ProfileGateStatus,
    pub logloss_improvement: Option<f64>,
    pub brier_delta: Option<f64>,
    pub improvement_probability: Option<f64>,
    pub sample_ready: bool,
    pub cross_domain_ready: bool,
}

#[derive(Debug)]
struct Measurement {
    event_id: String,
    instrument_id: String,
    item_id: String,
    admin_mode: String,
    dimension: String,
    normalized: f64,
}

pub fn profile_behavior_snapshot(conn: &Connection) -> Result<ProfileBehaviorSnapshot> {
    let (gap_sum, gap_count): (f64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(calib_gap), 0.0), COUNT(calib_gap) FROM mastery_states",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut active_gu_patterns = BTreeMap::new();
    let mut gu = conn.prepare(
        "SELECT pattern, COUNT(*) FROM gu_rules WHERE status='active' GROUP BY pattern ORDER BY pattern",
    )?;
    for row in gu.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (pattern, count) = row?;
        active_gu_patterns.insert(pattern, count);
    }
    let move_effect_observations =
        conn.query_row("SELECT COALESCE(SUM(n), 0) FROM moves_effects", [], |row| {
            row.get(0)
        })?;
    let (session_count, attempt_sum, duration_sum, duration_count): (i64, i64, f64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(ss.attempts_count), 0),
                COALESCE(SUM((julianday(s.ended_at) - julianday(s.started_at)) * 1440.0), 0.0),
                COUNT(CASE WHEN s.started_at IS NOT NULL AND s.ended_at IS NOT NULL THEN 1 END)
         FROM session_summaries ss JOIN sessions s ON s.id=ss.session_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    let abandon_event_count = conn.query_row(
        "SELECT COUNT(*) FROM behavior_events WHERE type='abandon'",
        [],
        |row| row.get(0),
    )?;
    let abandon_after_hint_count = conn.query_row(
        "SELECT COUNT(*) FROM behavior_events abandon
         WHERE abandon.type='abandon' AND EXISTS(
             SELECT 1 FROM behavior_events hint
             WHERE hint.session_id=abandon.session_id AND hint.type='hint'
               AND julianday(hint.at) <= julianday(abandon.at)
         )",
        [],
        |row| row.get(0),
    )?;
    Ok(ProfileBehaviorSnapshot {
        calibration_mean_gap: (gap_count > 0).then(|| gap_sum / gap_count as f64),
        active_gu_patterns,
        move_effect_observations,
        valid_session_count: session_count,
        average_session_attempts: (session_count > 0)
            .then(|| attempt_sum as f64 / session_count as f64),
        average_session_minutes: (duration_count > 0).then(|| duration_sum / duration_count as f64),
        abandon_event_count,
        abandon_after_hint_count,
    })
}

pub fn offer_profile_ema_at(
    conn: &Connection,
    session_id: &str,
    now: &str,
) -> Result<ProfileEmaOffer> {
    let settings = global_profile_settings(conn)?;
    if !settings.enabled {
        return Ok(empty_offer(ProfileEmaStatus::Disabled));
    }
    if settings.disclosure_required {
        return Ok(empty_offer(ProfileEmaStatus::DisclosureRequired));
    }
    if settings
        .paused_until
        .as_deref()
        .is_some_and(|until| until > now)
    {
        return Ok(empty_offer(ProfileEmaStatus::Paused));
    }
    let closed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_summaries WHERE session_id=?1)",
        [session_id],
        |row| row.get(0),
    )?;
    if !closed {
        return Ok(empty_offer(ProfileEmaStatus::SessionNotClosed));
    }
    let already_offered: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM behavior_events WHERE session_id=?1 AND type=?2
         )",
        params![session_id, EMA_OFFER_EVENT],
        |row| row.get(0),
    )?;
    if already_offered {
        return Ok(empty_offer(ProfileEmaStatus::AlreadyOffered));
    }
    if latest_session_state_is_flow(conn, session_id)? {
        return Ok(empty_offer(ProfileEmaStatus::FlowSuppressed));
    }
    let daily = count_ema_offers(conn, now, "start of day")?;
    if daily >= meta_i64(conn, "profile.ema.max_daily")? {
        return Ok(empty_offer(ProfileEmaStatus::DailyLimit));
    }
    let weekly = count_ema_offers(conn, now, "-7 days")?;
    if weekly >= meta_i64(conn, "profile.ema.max_weekly")? {
        return Ok(empty_offer(ProfileEmaStatus::WeeklyLimit));
    }
    let Some(prompt) = least_used_ema_item(conn)? else {
        return Ok(empty_offer(ProfileEmaStatus::NoItems));
    };
    let event_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            event_id,
            session_id,
            now,
            EMA_OFFER_EVENT,
            serde_json::to_string(&prompt)?
        ],
    )?;
    Ok(ProfileEmaOffer {
        status: ProfileEmaStatus::Offered,
        event_id: Some(event_id),
        prompt: Some(prompt),
    })
}

pub fn record_profile_ema_skip_at(conn: &Connection, session_id: &str, now: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, payload_json)
         VALUES (?1, ?2, ?3, ?4, '{\"decision\":\"skip\"}')",
        params![
            Uuid::new_v4().to_string(),
            session_id,
            now,
            EMA_DECISION_EVENT
        ],
    )?;
    Ok(())
}

pub fn run_monthly_profile_update_at(conn: &Connection, now: &str) -> Result<ProfileMonthlyUpdate> {
    let already_updated: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM profile_dimensions
             WHERE model_version=?1 AND substr(updated_at, 1, 7)=substr(?2, 1, 7)
         )",
        params![PROFILE_MODEL_VERSION, now],
        |row| row.get(0),
    )?;
    if already_updated {
        return Ok(ProfileMonthlyUpdate {
            status: "already_current".to_owned(),
            model_version: PROFILE_MODEL_VERSION.to_owned(),
            dimensions: Vec::new(),
        });
    }
    let measurements = load_measurements(conn)?;
    let has_measurements = !measurements.is_empty();
    let mut grouped = BTreeMap::<String, Vec<&Measurement>>::new();
    for measurement in &measurements {
        grouped
            .entry(measurement.dimension.clone())
            .or_default()
            .push(measurement);
    }
    let mut dimensions = Vec::new();
    for (dimension_key, observations) in grouped {
        let sum = observations
            .iter()
            .map(|measurement| measurement.normalized)
            .sum::<f64>();
        let n = observations.len() as f64;
        let alpha = 1.0 + sum;
        let beta = 1.0 + n - sum;
        let total = alpha + beta;
        let mean = alpha / total;
        let variance = alpha * beta / (total * total * (total + 1.0));
        let evidence_ids = observations
            .iter()
            .map(|measurement| measurement.event_id.clone())
            .collect::<Vec<_>>();
        let full_items = observations
            .iter()
            .filter(|measurement| measurement.admin_mode == "full_scale")
            .map(|measurement| format!("{}:{}", measurement.instrument_id, measurement.item_id))
            .collect::<BTreeSet<_>>();
        let registered_full_items = registered_items_for_dimension(&dimension_key)?;
        let partial_instrument = full_items.len() < registered_full_items;
        let mut dimension = store_profile_dimension(
            conn,
            ProfileDimensionInput {
                scope: ProfileScope::Global,
                scope_id: None,
                dimension_key,
                mean,
                variance,
                evidence_count: observations.len() as i64,
                model_version: PROFILE_MODEL_VERSION.to_owned(),
                gate_status: ProfileGateStatus::Shadow,
                provenance: serde_json::json!({
                    "method": "beta_fractional_update",
                    "partial_instrument": partial_instrument,
                    "complete_full_scale": registered_full_items > 0 && !partial_instrument,
                    "ema_is_not_normative": true,
                    "updated_for_month": now.get(..7).unwrap_or(now),
                }),
                evidence_ids,
            },
        )?;
        conn.execute(
            "UPDATE profile_dimensions SET updated_at=?1
             WHERE scope='global' AND scope_id='' AND dimension_key=?2",
            params![now, dimension.dimension_key],
        )?;
        dimension.updated_at = now.to_owned();
        dimensions.push(dimension);
    }
    let updated_keys = dimensions
        .iter()
        .map(|dimension| dimension.dimension_key.clone())
        .collect::<BTreeSet<_>>();
    for dimension_key in REQUIRED_SLOW_DIMENSIONS
        .into_iter()
        .filter(|dimension_key| !updated_keys.contains(*dimension_key))
    {
        let mut dimension = store_profile_dimension(
            conn,
            ProfileDimensionInput {
                scope: ProfileScope::Global,
                scope_id: None,
                dimension_key: dimension_key.to_owned(),
                mean: 0.5,
                variance: 1.0 / 12.0,
                evidence_count: 0,
                model_version: PROFILE_MODEL_VERSION.to_owned(),
                gate_status: ProfileGateStatus::Unfit,
                provenance: serde_json::json!({
                    "method": "uninformative_prior",
                    "reason": "no_registered_measurement_evidence",
                    "behavior_is_not_trait_label": true,
                    "updated_for_month": now.get(..7).unwrap_or(now),
                }),
                evidence_ids: Vec::new(),
            },
        )?;
        conn.execute(
            "UPDATE profile_dimensions SET updated_at=?1
             WHERE scope='global' AND scope_id='' AND dimension_key=?2",
            params![now, dimension.dimension_key],
        )?;
        dimension.updated_at = now.to_owned();
        dimensions.push(dimension);
    }
    dimensions.sort_by(|left, right| left.dimension_key.cmp(&right.dimension_key));
    Ok(ProfileMonthlyUpdate {
        status: if has_measurements {
            "updated"
        } else {
            "no_measurements"
        }
        .to_owned(),
        model_version: PROFILE_MODEL_VERSION.to_owned(),
        dimensions,
    })
}

pub fn evaluate_profile_gate(
    conn: &Connection,
    input: ProfileValidationInput,
) -> Result<ProfileGateEvaluation> {
    if input.id.trim().is_empty() || input.dimension_key.trim().is_empty() {
        return Err(PolarisError::InvalidParameter {
            key: "profile.validation.identity".to_owned(),
            value: "id and dimension_key must be non-empty".to_owned(),
        });
    }
    if input.observed_weeks < 0
        || input.outcome_count < 0
        || input.valid_session_count < 0
        || input.cross_domain_pack_count < 0
    {
        return Err(PolarisError::InvalidParameter {
            key: "profile.validation.sample".to_owned(),
            value: "sample counts must be non-negative".to_owned(),
        });
    }
    if input.folds.iter().any(|fold| {
        !fold.baseline_logloss.is_finite()
            || !fold.candidate_logloss.is_finite()
            || !fold.baseline_brier.is_finite()
            || !fold.candidate_brier.is_finite()
            || fold.baseline_logloss < 0.0
            || fold.candidate_logloss < 0.0
            || !(0.0..=1.0).contains(&fold.baseline_brier)
            || !(0.0..=1.0).contains(&fold.candidate_brier)
    }) {
        return Err(PolarisError::InvalidParameter {
            key: "profile.validation.folds".to_owned(),
            value: "metrics must be finite".to_owned(),
        });
    }
    let sample_ready = input.observed_weeks >= meta_i64(conn, "profile.gate.min_weeks")?
        && input.outcome_count >= meta_i64(conn, "profile.gate.min_outcomes")?
        && input.valid_session_count >= meta_i64(conn, "profile.gate.min_sessions")?
        && input.folds.len() as i64 >= meta_i64(conn, "profile.gate.min_folds")?;
    let cross_domain_ready = input.cross_domain_pack_count == 0
        || input.cross_domain_pack_count >= meta_i64(conn, "profile.gate.min_cross_domain_packs")?;
    let (logloss_improvement, brier_delta, improvement_probability) = fold_metrics(&input.folds);
    let min_logloss_improvement = meta_f64(conn, "profile.gate.min_logloss_improvement")?;
    let max_brier_delta = meta_f64(conn, "profile.gate.max_brier_delta")?;
    let min_improvement_probability = meta_f64(conn, "profile.gate.min_improvement_probability")?;
    let metrics_pass = sample_ready
        && cross_domain_ready
        && logloss_improvement.is_some_and(|value| value >= min_logloss_improvement)
        && brier_delta.is_some_and(|value| value <= max_brier_delta)
        && improvement_probability.is_some_and(|value| value >= min_improvement_probability);
    let previous_active: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM profile_dimensions
             WHERE scope=?1 AND scope_id=COALESCE(?2, '') AND dimension_key=?3
               AND gate_status='active'
         )",
        params![scope_name(input.scope), input.scope_id, input.dimension_key],
        |row| row.get(0),
    )?;
    let status = if metrics_pass {
        ProfileGateStatus::Active
    } else if previous_active {
        ProfileGateStatus::Suspended
    } else if sample_ready {
        ProfileGateStatus::Shadow
    } else {
        ProfileGateStatus::Unfit
    };
    let metrics = serde_json::json!({
        "observed_weeks": input.observed_weeks,
        "outcome_count": input.outcome_count,
        "valid_session_count": input.valid_session_count,
        "cross_domain_pack_count": input.cross_domain_pack_count,
        "fold_count": input.folds.len(),
        "logloss_improvement": logloss_improvement,
        "brier_delta": brier_delta,
        "improvement_probability": improvement_probability,
        "sample_ready": sample_ready,
        "cross_domain_ready": cross_domain_ready,
    });
    record_profile_validation_run(
        conn,
        ProfileValidationRunInput {
            id: input.id,
            scope: input.scope,
            scope_id: input.scope_id.clone(),
            dimension_key: input.dimension_key.clone(),
            model_version: PROFILE_MODEL_VERSION.to_owned(),
            status,
            metrics,
            provenance: serde_json::json!({
                "method": "time_forward_folds",
                "causal_claim": false,
            }),
            evidence_ids: Vec::new(),
        },
    )?;
    conn.execute(
        "UPDATE profile_dimensions SET gate_status=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE scope=?2 AND scope_id=COALESCE(?3, '') AND dimension_key=?4",
        params![
            gate_name(status),
            scope_name(input.scope),
            input.scope_id,
            input.dimension_key
        ],
    )?;
    Ok(ProfileGateEvaluation {
        status,
        logloss_improvement,
        brier_delta,
        improvement_probability,
        sample_ready,
        cross_domain_ready,
    })
}

fn empty_offer(status: ProfileEmaStatus) -> ProfileEmaOffer {
    ProfileEmaOffer {
        status,
        event_id: None,
        prompt: None,
    }
}

fn count_ema_offers(conn: &Connection, now: &str, modifier: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM behavior_events
         WHERE type=?1 AND julianday(at) >= julianday(?2, ?3) AND julianday(at) <= julianday(?2)",
        params![EMA_OFFER_EVENT, now, modifier],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn latest_session_state_is_flow(conn: &Connection, session_id: &str) -> Result<bool> {
    let posterior: Option<String> = conn
        .query_row(
            "SELECT json_extract(payload_json, '$.posterior') FROM behavior_events
             WHERE session_id=?1 AND type='mental_state'
             ORDER BY julianday(at) DESC, id DESC LIMIT 1",
            [session_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(posterior) = posterior else {
        return Ok(false);
    };
    let values: Vec<f64> = serde_json::from_str(&posterior)?;
    Ok(values
        .first()
        .is_some_and(|flow| values.iter().all(|value| flow >= value)))
}

fn least_used_ema_item(conn: &Connection) -> Result<Option<ProfileEmaPrompt>> {
    let instruments = profile_instruments()?;
    let mut candidates = Vec::new();
    for instrument in instruments {
        if !instrument
            .admin_modes
            .iter()
            .any(|mode| mode == "ema_single_item")
        {
            continue;
        }
        for item in instrument.items {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM behavior_events
                 WHERE type=?1 AND json_extract(payload_json, '$.item_id')=?2",
                params![EMA_OFFER_EVENT, item.id],
                |row| row.get(0),
            )?;
            candidates.push((
                count,
                ProfileEmaPrompt {
                    instrument_id: instrument.id.clone(),
                    instrument_version: instrument.version.clone(),
                    item_id: item.id,
                    locale: item.locale,
                    admin_mode: "ema_single_item".to_owned(),
                    prompt: item.prompt,
                },
            ));
        }
    }
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.instrument_id.cmp(&right.1.instrument_id))
            .then_with(|| left.1.item_id.cmp(&right.1.item_id))
    });
    Ok(candidates.into_iter().next().map(|item| item.1))
}

fn load_measurements(conn: &Connection) -> Result<Vec<Measurement>> {
    let instruments = profile_instruments()?;
    let mut item_meta = BTreeMap::new();
    for instrument in instruments {
        for item in instrument.items {
            item_meta.insert(
                (instrument.id.clone(), item.id.clone()),
                (
                    item.dimension,
                    item.keyed,
                    instrument.scoring.response_min,
                    instrument.scoring.response_max,
                ),
            );
        }
    }
    let mut statement = conn.prepare(
        "SELECT id, payload_json FROM behavior_events
         WHERE type='profile_measurement' ORDER BY julianday(at), id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut measurements = Vec::new();
    for (event_id, payload_json) in rows {
        let payload: Value = serde_json::from_str(&payload_json)?;
        let instrument_id = json_string(&payload, "instrument_id")?;
        let item_id = json_string(&payload, "item_id")?;
        let admin_mode = json_string(&payload, "admin_mode")?;
        let response = payload["response"]
            .as_i64()
            .ok_or_else(|| invalid_payload(&payload))?;
        let Some((dimension, keyed, min, max)) =
            item_meta.get(&(instrument_id.clone(), item_id.clone()))
        else {
            continue;
        };
        let scored = if keyed == "negative" {
            min + max - response
        } else {
            response
        };
        measurements.push(Measurement {
            event_id,
            instrument_id,
            item_id,
            admin_mode,
            dimension: dimension.clone(),
            normalized: (scored - min) as f64 / (max - min) as f64,
        });
    }
    Ok(measurements)
}

fn registered_items_for_dimension(dimension: &str) -> Result<usize> {
    Ok(profile_instruments()?
        .into_iter()
        .flat_map(|instrument| {
            instrument
                .items
                .into_iter()
                .map(move |item| (instrument.id.clone(), item))
        })
        .filter(|(_, item)| item.dimension == dimension)
        .map(|(instrument_id, item)| format!("{instrument_id}:{}", item.id))
        .collect::<BTreeSet<_>>()
        .len())
}

fn fold_metrics(folds: &[ProfileValidationFold]) -> (Option<f64>, Option<f64>, Option<f64>) {
    if folds.is_empty() {
        return (None, None, None);
    }
    let improvements = folds
        .iter()
        .map(|fold| fold.baseline_logloss - fold.candidate_logloss)
        .collect::<Vec<_>>();
    let logloss_improvement = mean(&improvements);
    let brier_deltas = folds
        .iter()
        .map(|fold| fold.candidate_brier - fold.baseline_brier)
        .collect::<Vec<_>>();
    let brier_delta = mean(&brier_deltas);
    let probability = if improvements.len() < 2 {
        (improvements[0] > 0.0).then_some(1.0).or(Some(0.5))
    } else {
        let mean_value = logloss_improvement.unwrap_or(0.0);
        let variance = improvements
            .iter()
            .map(|value| (value - mean_value).powi(2))
            .sum::<f64>()
            / (improvements.len() - 1) as f64;
        let standard_error = (variance / improvements.len() as f64).sqrt();
        Some(if standard_error <= f64::EPSILON {
            if mean_value > 0.0 {
                1.0
            } else {
                0.5
            }
        } else {
            normal_cdf(mean_value / standard_error)
        })
    };
    (logloss_improvement, brier_delta, probability)
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn normal_cdf(value: f64) -> f64 {
    let x = value.abs();
    let t = 1.0 / (1.0 + 0.231_641_9 * x);
    let density = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let tail = density
        * t
        * (0.319_381_530
            + t * (-0.356_563_782
                + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    if value >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

fn json_string(payload: &Value, key: &str) -> Result<String> {
    payload[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid_payload(payload))
}

fn invalid_payload(payload: &Value) -> PolarisError {
    PolarisError::InvalidParameter {
        key: "profile.measurement.payload".to_owned(),
        value: payload.to_string(),
    }
}

fn scope_name(scope: ProfileScope) -> &'static str {
    match scope {
        ProfileScope::Global => "global",
        ProfileScope::Pack => "pack",
        ProfileScope::Goal => "goal",
    }
}

fn gate_name(status: ProfileGateStatus) -> &'static str {
    match status {
        ProfileGateStatus::Unfit => "unfit",
        ProfileGateStatus::Shadow => "shadow",
        ProfileGateStatus::Active => "active",
        ProfileGateStatus::Suspended => "suspended",
    }
}
