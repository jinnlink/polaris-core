use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{PolarisError, Result};

pub const RECORDED_ONLY_MESSAGE: &str = "已保存为学习资料，不会直接算作掌握。";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Pending,
    Mapped,
    PracticeReady,
    Practiced,
    Ignored,
    Archived,
}

impl CaptureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Mapped => "mapped",
            Self::PracticeReady => "practice_ready",
            Self::Practiced => "practiced",
            Self::Ignored => "ignored",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "mapped" => Some(Self::Mapped),
            "practice_ready" | "practice-ready" => Some(Self::PracticeReady),
            "practiced" => Some(Self::Practiced),
            "ignored" => Some(Self::Ignored),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnerCaptureKind {
    Reference,
    OwnAnswer,
    ErrorLog,
    CodeChange,
    ChatExcerpt,
    Unknown,
}

impl LearnerCaptureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::OwnAnswer => "own_answer",
            Self::ErrorLog => "error_log",
            Self::CodeChange => "code_change",
            Self::ChatExcerpt => "chat_excerpt",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "reference" | "note" | "reading" => Some(Self::Reference),
            "own_answer" | "own-answer" | "answer" => Some(Self::OwnAnswer),
            "error_log" | "error-log" | "error" => Some(Self::ErrorLog),
            "code_change" | "code-change" | "code" => Some(Self::CodeChange),
            "chat_excerpt" | "chat-excerpt" | "chat" => Some(Self::ChatExcerpt),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureEffect {
    RecordedOnly,
}

impl CaptureEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RecordedOnly => "recorded_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureInput {
    pub session_id: Option<String>,
    pub source: String,
    pub content_type: String,
    pub text: String,
    pub learner_kind: LearnerCaptureKind,
    pub candidate_concept_ids: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureRecord {
    pub capture_id: String,
    pub evidence_id: String,
    pub status: CaptureStatus,
    pub learner_kind: LearnerCaptureKind,
    pub effect: CaptureEffect,
    pub message: String,
}

pub fn capture_learning_evidence(conn: &Connection, input: CaptureInput) -> Result<CaptureRecord> {
    let session_id = optional_non_empty("capture.session_id", input.session_id)?;
    let source = normalized_required("capture.source", &input.source)?;
    let content_type = normalized_required("capture.content_type", &input.content_type)?;
    let text = normalized_required("capture.text", &input.text)?;
    let note = optional_non_empty("capture.note", input.note)?;
    let candidate_concept_ids =
        normalized_id_list("capture.candidate_concept_ids", input.candidate_concept_ids)?;
    let candidate_concept_ids_json = serde_json::to_string(&candidate_concept_ids)?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<CaptureRecord> {
        if let Some(session_id) = session_id.as_deref() {
            conn.execute(
                "INSERT OR IGNORE INTO sessions(id, started_at, context_json)
                 VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), '{}')",
                [session_id],
            )?;
        }

        let evidence_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                evidence_id.as_str(),
                session_id.as_deref(),
                source.as_str(),
                content_type.as_str(),
                text.as_str(),
                candidate_concept_ids_json.as_str(),
            ],
        )?;

        let capture_id = Uuid::new_v4().to_string();
        let status = CaptureStatus::Pending;
        conn.execute(
            "INSERT INTO capture_queue(id, evidence_id, status, learner_kind, candidate_concept_ids_json, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                capture_id.as_str(),
                evidence_id.as_str(),
                status.as_str(),
                input.learner_kind.as_str(),
                candidate_concept_ids_json.as_str(),
                note.as_deref(),
            ],
        )?;

        Ok(CaptureRecord {
            capture_id,
            evidence_id,
            status,
            learner_kind: input.learner_kind,
            effect: CaptureEffect::RecordedOnly,
            message: RECORDED_ONLY_MESSAGE.to_owned(),
        })
    })();

    match result {
        Ok(record) => {
            conn.execute_batch("COMMIT")?;
            Ok(record)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
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
        .map(|value| normalized_required(key, &value))
        .transpose()
}

fn normalized_id_list(key: &str, values: Vec<String>) -> Result<Vec<String>> {
    values
        .into_iter()
        .map(|value| normalized_required(key, &value))
        .collect()
}
