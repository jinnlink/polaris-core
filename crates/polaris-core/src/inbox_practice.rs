use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capture_queue::CaptureStatus;
use crate::error::{PolarisError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxPracticeDraft {
    pub capture_id: String,
    pub evidence_id: String,
    pub status: CaptureStatus,
    #[serde(skip)]
    pub concept_id: String,
    pub concept_hint: Option<String>,
    pub task_type: String,
    pub prompt: String,
    pub source_excerpt: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxPracticeSubmissionInput {
    pub capture_id: String,
    pub session_id: String,
    pub response_text: String,
    pub self_confidence: i32,
    pub latency_ms: i64,
    pub hint_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxPracticeSubmissionReceipt {
    pub capture_id: String,
    pub attempt_id: String,
    pub status: CaptureStatus,
    pub effect: String,
    pub message: String,
    pub provisional_score: f64,
    pub degraded: bool,
}

pub fn draft_inbox_practice(conn: &Connection, capture_id: &str) -> Result<InboxPracticeDraft> {
    let capture_id = normalized_required("inbox_practice.capture_id", capture_id)?;
    let raw: Option<RawPracticeRow> = conn
        .query_row(
            "SELECT cq.id, cq.evidence_id, cq.status, cq.candidate_concept_ids_json, e.text
             FROM capture_queue cq
             JOIN evidence_items e ON e.id = cq.evidence_id
             WHERE cq.id=?1",
            [capture_id.as_str()],
            |row| {
                Ok(RawPracticeRow {
                    capture_id: row.get(0)?,
                    evidence_id: row.get(1)?,
                    status_text: row.get(2)?,
                    candidate_concept_ids_json: row.get(3)?,
                    evidence_text: row.get(4)?,
                })
            },
        )
        .optional()?;
    let Some(raw) = raw else {
        return Err(PolarisError::InvalidParameter {
            key: "inbox_practice.capture_id".to_owned(),
            value: format!("{capture_id} not found"),
        });
    };
    let status =
        CaptureStatus::parse(&raw.status_text).ok_or_else(|| PolarisError::InvalidParameter {
            key: "capture_queue.status".to_owned(),
            value: raw.status_text.clone(),
        })?;
    if status != CaptureStatus::PracticeReady {
        return Err(PolarisError::InvalidParameter {
            key: "capture_queue.status".to_owned(),
            value: format!("expected practice_ready, got {}", status.as_str()),
        });
    }
    let candidate_ids: Vec<String> =
        serde_json::from_str(&raw.candidate_concept_ids_json).unwrap_or_default();
    let (concept_id, concept_name) = first_existing_concept(conn, &candidate_ids)?;
    let source_excerpt = text_preview(&raw.evidence_text, 220);
    let prompt = format!(
        "请用自己的话回答：这条资料和「{}」有什么关系？请解释关键点，并给出一个例子或反例。\n\n资料摘要：{}",
        concept_name, source_excerpt
    );

    Ok(InboxPracticeDraft {
        capture_id: raw.capture_id,
        evidence_id: raw.evidence_id,
        status,
        concept_id,
        concept_hint: Some(concept_name),
        task_type: "explain".to_owned(),
        prompt,
        source_excerpt,
        message: "先回答这道小题，再告诉我你的把握程度。".to_owned(),
    })
}

pub fn mark_inbox_practice_submitted(
    conn: &Connection,
    draft: &InboxPracticeDraft,
    session_id: &str,
    attempt_id: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE capture_queue
         SET status='practiced', updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?1",
        [draft.capture_id.as_str()],
    )?;
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'inbox_practice', ?3, ?4)",
        params![
            Uuid::new_v4().to_string(),
            session_id,
            draft.concept_id.as_str(),
            serde_json::json!({
                "capture_id": draft.capture_id,
                "source_evidence_id": draft.evidence_id,
                "attempt_id": attempt_id,
            })
            .to_string()
        ],
    )?;
    Ok(())
}

#[derive(Debug)]
struct RawPracticeRow {
    capture_id: String,
    evidence_id: String,
    status_text: String,
    candidate_concept_ids_json: String,
    evidence_text: String,
}

fn first_existing_concept(conn: &Connection, ids: &[String]) -> Result<(String, String)> {
    for id in ids {
        if let Some(name) = conn
            .query_row(
                "SELECT name FROM concepts WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok((id.clone(), name));
        }
    }
    Err(PolarisError::InvalidParameter {
        key: "capture_queue.candidate_concept_ids_json".to_owned(),
        value: "practice bridge requires an existing candidate concept".to_owned(),
    })
}

fn text_preview(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut preview = normalized.chars().take(max_chars).collect::<String>();
    preview.push_str("...");
    preview
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
