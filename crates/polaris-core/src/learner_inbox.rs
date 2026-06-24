use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::capture_queue::{CaptureStatus, LearnerCaptureKind};
use crate::error::{PolarisError, Result};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnerInboxAction {
    Accept,
    Defer,
    Ignore,
    Archive,
}

impl LearnerInboxAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Defer => "defer",
            Self::Ignore => "ignore",
            Self::Archive => "archive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "accept" | "practice" | "practice_ready" | "practice-ready" => Some(Self::Accept),
            "defer" | "later" => Some(Self::Defer),
            "ignore" => Some(Self::Ignore),
            "archive" => Some(Self::Archive),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Accept => "转成一道小题",
            Self::Defer => "稍后再看",
            Self::Ignore => "忽略",
            Self::Archive => "归档",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnerInboxActionOption {
    pub action: LearnerInboxAction,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnerInboxItem {
    pub capture_id: String,
    pub evidence_id: String,
    pub status: CaptureStatus,
    pub learner_kind: LearnerCaptureKind,
    pub source: String,
    pub content_type: String,
    pub text_preview: String,
    pub concept_hint: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message: String,
    pub actions: Vec<LearnerInboxActionOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnerInboxActionReceipt {
    pub capture_id: String,
    pub status: CaptureStatus,
    pub effect: String,
    pub message: String,
}

pub fn learner_inbox(
    conn: &Connection,
    statuses: &[CaptureStatus],
    limit: usize,
) -> Result<Vec<LearnerInboxItem>> {
    let statuses = normalized_statuses(statuses);
    let status_list = statuses
        .iter()
        .map(|status| format!("'{}'", status.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT cq.id, cq.evidence_id, cq.status, cq.learner_kind,
                cq.candidate_concept_ids_json, cq.note, cq.created_at, cq.updated_at,
                e.source, e.content_type, e.text
         FROM capture_queue cq
         JOIN evidence_items e ON e.id = cq.evidence_id
         WHERE cq.status IN ({status_list})
         ORDER BY cq.updated_at DESC, cq.created_at DESC, cq.id ASC
         LIMIT {}",
        normalized_limit(limit)
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let status_text: String = row.get(2)?;
        let learner_kind_text: String = row.get(3)?;
        let candidate_json: String = row.get(4)?;
        Ok(RawInboxRow {
            capture_id: row.get(0)?,
            evidence_id: row.get(1)?,
            status_text,
            learner_kind_text,
            candidate_concept_ids: serde_json::from_str(&candidate_json).unwrap_or_default(),
            note: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            source: row.get(8)?,
            content_type: row.get(9)?,
            text: row.get(10)?,
        })
    })?;

    rows.map(|row| item_from_row(conn, row?)).collect()
}

pub fn act_on_learner_inbox_item(
    conn: &Connection,
    capture_id: &str,
    action: LearnerInboxAction,
    note: Option<String>,
) -> Result<LearnerInboxActionReceipt> {
    let capture_id = normalized_required("learner_inbox.capture_id", capture_id)?;
    let note = note
        .map(|value| normalized_required("learner_inbox.note", &value))
        .transpose()?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<LearnerInboxActionReceipt> {
        let current_status: Option<String> = conn
            .query_row(
                "SELECT status FROM capture_queue WHERE id=?1",
                [capture_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(current_status) = current_status else {
            return Err(PolarisError::InvalidParameter {
                key: "learner_inbox.capture_id".to_owned(),
                value: format!("{capture_id} not found"),
            });
        };
        let current_status = CaptureStatus::parse(&current_status).ok_or_else(|| {
            PolarisError::InvalidParameter {
                key: "capture_queue.status".to_owned(),
                value: current_status.clone(),
            }
        })?;

        let next_status = match action {
            LearnerInboxAction::Accept => CaptureStatus::PracticeReady,
            LearnerInboxAction::Defer => current_status,
            LearnerInboxAction::Ignore => CaptureStatus::Ignored,
            LearnerInboxAction::Archive => CaptureStatus::Archived,
        };
        conn.execute(
            "UPDATE capture_queue
             SET status=?2,
                 note=COALESCE(?3, note),
                 updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=?1",
            params![capture_id.as_str(), next_status.as_str(), note.as_deref()],
        )?;

        Ok(LearnerInboxActionReceipt {
            capture_id,
            status: next_status,
            effect: "recorded_only".to_owned(),
            message: action_message(action, next_status).to_owned(),
        })
    })();

    match result {
        Ok(receipt) => {
            conn.execute_batch("COMMIT")?;
            Ok(receipt)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

#[derive(Debug)]
struct RawInboxRow {
    capture_id: String,
    evidence_id: String,
    status_text: String,
    learner_kind_text: String,
    candidate_concept_ids: Vec<String>,
    note: Option<String>,
    created_at: String,
    updated_at: String,
    source: String,
    content_type: String,
    text: String,
}

fn item_from_row(conn: &Connection, row: RawInboxRow) -> Result<LearnerInboxItem> {
    let status =
        CaptureStatus::parse(&row.status_text).ok_or_else(|| PolarisError::InvalidParameter {
            key: "capture_queue.status".to_owned(),
            value: row.status_text.clone(),
        })?;
    let learner_kind = LearnerCaptureKind::parse(&row.learner_kind_text).ok_or_else(|| {
        PolarisError::InvalidParameter {
            key: "capture_queue.learner_kind".to_owned(),
            value: row.learner_kind_text.clone(),
        }
    })?;
    let concept_hint = first_concept_name(conn, &row.candidate_concept_ids)?;
    Ok(LearnerInboxItem {
        capture_id: row.capture_id,
        evidence_id: row.evidence_id,
        status,
        learner_kind,
        source: row.source,
        content_type: row.content_type,
        text_preview: text_preview(&row.text, 160),
        concept_hint: concept_hint.clone(),
        note: row.note,
        created_at: row.created_at,
        updated_at: row.updated_at,
        message: status_message(status, concept_hint.as_deref()),
        actions: action_options(status),
    })
}

fn normalized_statuses(statuses: &[CaptureStatus]) -> Vec<CaptureStatus> {
    if statuses.is_empty() {
        return vec![
            CaptureStatus::Pending,
            CaptureStatus::Mapped,
            CaptureStatus::PracticeReady,
        ];
    }
    statuses.to_vec()
}

fn normalized_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    }
}

fn first_concept_name(conn: &Connection, ids: &[String]) -> Result<Option<String>> {
    let Some(first_id) = ids.first() else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT name FROM concepts WHERE id=?1",
        [first_id.as_str()],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn status_message(status: CaptureStatus, concept_hint: Option<&str>) -> String {
    match (status, concept_hint) {
        (CaptureStatus::Pending, _) => "已保存，稍后帮你整理".to_owned(),
        (CaptureStatus::Mapped, Some(hint)) => format!("这可能和「{hint}」有关"),
        (CaptureStatus::Mapped, None) => "这条资料已有候选知识点".to_owned(),
        (CaptureStatus::PracticeReady, _) => "要不要用它做一道小题？".to_owned(),
        (CaptureStatus::Practiced, _) => "已练过".to_owned(),
        (CaptureStatus::Ignored, _) => "已忽略".to_owned(),
        (CaptureStatus::Archived, _) => "已归档".to_owned(),
    }
}

fn action_message(action: LearnerInboxAction, status: CaptureStatus) -> &'static str {
    match action {
        LearnerInboxAction::Accept => "已放入可练习队列，下一步可以把它转成小题。",
        LearnerInboxAction::Defer => "已保留，稍后再看。",
        LearnerInboxAction::Ignore => "已忽略。",
        LearnerInboxAction::Archive => {
            if status == CaptureStatus::Archived {
                "已归档，不再提醒。"
            } else {
                "已处理。"
            }
        }
    }
}

fn action_options(status: CaptureStatus) -> Vec<LearnerInboxActionOption> {
    match status {
        CaptureStatus::Pending | CaptureStatus::Mapped | CaptureStatus::PracticeReady => [
            LearnerInboxAction::Accept,
            LearnerInboxAction::Defer,
            LearnerInboxAction::Ignore,
        ]
        .into_iter()
        .map(|action| LearnerInboxActionOption {
            action,
            label: action.label().to_owned(),
        })
        .collect(),
        CaptureStatus::Practiced | CaptureStatus::Ignored | CaptureStatus::Archived => Vec::new(),
    }
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
