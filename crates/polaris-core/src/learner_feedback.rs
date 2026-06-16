use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::error::{PolarisError, Result};
use crate::mental_state::MentalState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnerFeedbackInput {
    pub session_id: String,
    pub source: String,
    pub kind: String,
    pub concept_id: Option<String>,
    pub state: Option<String>,
    pub reason: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnerFeedbackReceipt {
    pub event_id: String,
    pub kind: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concept_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub effect: String,
}

const EFFECT_RECORDED_ONLY: &str = "recorded_only";

pub fn record_learner_feedback(
    conn: &Connection,
    input: LearnerFeedbackInput,
) -> Result<LearnerFeedbackReceipt> {
    let session_id = normalized_required("learner_feedback.session_id", &input.session_id)?;
    let source = normalized_required("learner_feedback.source", &input.source)?;
    let kind = normalize_kind(&input.kind)?;
    let concept_id = optional_non_empty("learner_feedback.concept_id", input.concept_id)?;
    let note = optional_non_empty("learner_feedback.note", input.note)?;

    let (state, reason) = match kind.as_str() {
        "state" => (
            Some(normalize_state(input.state.as_deref())?),
            reject_unexpected_reason(input.reason)?,
        ),
        "pause" => (
            reject_unexpected_state(input.state)?,
            Some(normalize_pause_reason(input.reason.as_deref())?),
        ),
        _ => unreachable!("normalize_kind only returns supported kinds"),
    };

    let payload = json!({
        "schema_version": 1,
        "kind": kind.as_str(),
        "source": source.as_str(),
        "state": state.as_deref(),
        "reason": reason.as_deref(),
        "note": note.as_deref(),
        "effect": EFFECT_RECORDED_ONLY,
    })
    .to_string();
    let event_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'learner_feedback', ?3, ?4)",
        params![
            event_id.as_str(),
            session_id.as_str(),
            concept_id.as_deref(),
            payload.as_str()
        ],
    )?;

    Ok(LearnerFeedbackReceipt {
        event_id,
        kind,
        session_id,
        concept_id,
        state,
        reason,
        effect: EFFECT_RECORDED_ONLY.to_owned(),
    })
}

fn normalize_kind(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "state" | "state_report" | "learner_state" => Ok("state".to_owned()),
        "pause" | "pause_request" => Ok("pause".to_owned()),
        other => Err(PolarisError::InvalidParameter {
            key: "learner_feedback.kind".to_owned(),
            value: other.to_owned(),
        }),
    }
}

fn normalize_state(value: Option<&str>) -> Result<String> {
    let Some(value) = value else {
        return Err(PolarisError::InvalidParameter {
            key: "learner_feedback.state".to_owned(),
            value: "<missing>".to_owned(),
        });
    };
    let normalized = match value.trim().to_ascii_lowercase().as_str() {
        "tired" | "fatigue" => "fatigued".to_owned(),
        value => value.to_owned(),
    };
    MentalState::from_id(&normalized)
        .map(|state| state.as_str().to_owned())
        .ok_or_else(|| PolarisError::InvalidParameter {
            key: "learner_feedback.state".to_owned(),
            value: value.to_owned(),
        })
}

fn normalize_pause_reason(value: Option<&str>) -> Result<String> {
    let Some(value) = value else {
        return Err(PolarisError::InvalidParameter {
            key: "learner_feedback.reason".to_owned(),
            value: "<missing>".to_owned(),
        });
    };
    normalized_required("learner_feedback.reason", value)
}

fn normalized_required(key: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PolarisError::InvalidParameter {
            key: key.to_owned(),
            value: "<empty>".to_owned(),
        });
    }
    Ok(trimmed.to_owned())
}

fn optional_non_empty(key: &str, value: Option<String>) -> Result<Option<String>> {
    value
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Err(PolarisError::InvalidParameter {
                    key: key.to_owned(),
                    value: "<empty>".to_owned(),
                })
            } else {
                Ok(trimmed.to_owned())
            }
        })
        .transpose()
}

fn reject_unexpected_reason(value: Option<String>) -> Result<Option<String>> {
    match optional_non_empty("learner_feedback.reason", value)? {
        Some(reason) => Err(PolarisError::InvalidParameter {
            key: "learner_feedback.reason".to_owned(),
            value: reason,
        }),
        None => Ok(None),
    }
}

fn reject_unexpected_state(value: Option<String>) -> Result<Option<String>> {
    match optional_non_empty("learner_feedback.state", value)? {
        Some(state) => Err(PolarisError::InvalidParameter {
            key: "learner_feedback.state".to_owned(),
            value: state,
        }),
        None => Ok(None),
    }
}
