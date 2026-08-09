use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

use crate::error::{PolarisError, Result};
use crate::teaching::TeachingInstruction;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeachingTurn {
    pub id: String,
    pub session_id: String,
    pub concept_id: String,
    pub attempt_id: Option<String>,
    pub explanation_evidence_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeachingExplanationReceipt {
    pub teaching_turn_id: String,
    pub evidence_id: String,
}

pub fn begin_teaching_turn(
    conn: &Connection,
    session_id: &str,
    concept_id: &str,
    instruction: &TeachingInstruction,
) -> Result<TeachingTurn> {
    conn.execute(
        "INSERT INTO sessions(id, started_at, context_json)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), '{}')
         ON CONFLICT(id) DO NOTHING",
        [session_id],
    )?;
    let id = Uuid::new_v4().to_string();
    let instruction_json = serde_json::to_string(instruction)?;
    conn.execute(
        "INSERT INTO teaching_turns(
             id, session_id, concept_id, attempt_id, instruction_json,
             explanation_evidence_id, created_at
         ) VALUES (
             ?1, ?2, ?3, NULL, ?4, NULL,
             strftime('%Y-%m-%dT%H:%M:%fZ','now')
         )",
        params![id, session_id, concept_id, instruction_json],
    )?;
    teaching_turn(conn, &id)?
        .ok_or_else(|| PolarisError::InvalidTaskTurn("teaching turn was not persisted".to_owned()))
}

pub fn record_teaching_explanation(
    conn: &Connection,
    teaching_turn_id: &str,
    text: &str,
) -> Result<TeachingExplanationReceipt> {
    let text = text.trim();
    if text.is_empty() {
        return Err(PolarisError::InvalidTaskTurn(
            "teaching explanation must not be empty".to_owned(),
        ));
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<TeachingExplanationReceipt> {
        let Some((session_id, concept_id, existing_evidence_id)) = conn
            .query_row(
                "SELECT session_id, concept_id, explanation_evidence_id
                 FROM teaching_turns WHERE id=?1",
                [teaching_turn_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
        else {
            return Err(PolarisError::InvalidTaskTurn(format!(
                "unknown teaching_turn_id: {teaching_turn_id}"
            )));
        };
        if existing_evidence_id.is_some() {
            return Err(PolarisError::InvalidTaskTurn(
                "teaching turn already has explanation evidence".to_owned(),
            ));
        }

        let evidence_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO evidence_items(
                 id, session_id, source, content_type, text, lang,
                 concept_ids_json, created_at
             ) VALUES (
                 ?1, ?2, 'teaching_explanation', 'text/plain', ?3, NULL, ?4,
                 strftime('%Y-%m-%dT%H:%M:%SZ','now')
             )",
            params![
                evidence_id,
                session_id,
                text,
                serde_json::to_string(&vec![concept_id])?
            ],
        )?;
        conn.execute(
            "UPDATE teaching_turns SET explanation_evidence_id=?1 WHERE id=?2",
            params![evidence_id, teaching_turn_id],
        )?;
        Ok(TeachingExplanationReceipt {
            teaching_turn_id: teaching_turn_id.to_owned(),
            evidence_id,
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

pub fn associate_teaching_turn_with_task_event(
    conn: &Connection,
    task_event_id: &str,
    teaching_turn_id: &str,
) -> Result<()> {
    let changed = conn.execute(
        "UPDATE behavior_events
         SET payload_json=json_set(payload_json, '$.teaching_turn_id', ?1)
         WHERE id=?2 AND type='next' AND json_valid(payload_json)",
        params![teaching_turn_id, task_event_id],
    )?;
    if changed != 1 {
        return Err(PolarisError::InvalidTaskTurn(
            "task_event_id must reference a next event".to_owned(),
        ));
    }
    Ok(())
}

pub fn link_task_teaching_turn_to_attempt(
    conn: &Connection,
    task_event_id: &str,
    attempt_id: &str,
) -> Result<Option<String>> {
    let turn_id = conn
        .query_row(
            "SELECT json_extract(payload_json, '$.teaching_turn_id')
             FROM behavior_events
             WHERE id=?1 AND type='next' AND json_valid(payload_json)",
            [task_event_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    if let Some(turn_id) = &turn_id {
        let changed = conn.execute(
            "UPDATE teaching_turns SET attempt_id=?1 WHERE id=?2 AND attempt_id IS NULL",
            params![attempt_id, turn_id],
        )?;
        if changed != 1 {
            return Err(PolarisError::InvalidTaskTurn(
                "teaching turn is missing or already linked".to_owned(),
            ));
        }
    }
    Ok(turn_id)
}

pub fn teaching_turn(conn: &Connection, id: &str) -> Result<Option<TeachingTurn>> {
    conn.query_row(
        "SELECT id, session_id, concept_id, attempt_id, explanation_evidence_id, created_at
         FROM teaching_turns WHERE id=?1",
        [id],
        |row| {
            Ok(TeachingTurn {
                id: row.get(0)?,
                session_id: row.get(1)?,
                concept_id: row.get(2)?,
                attempt_id: row.get(3)?,
                explanation_evidence_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}
