use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::config::{meta_f64, meta_i64};
use crate::diagnosis::{diagnose_concept, DiagnosisFocus};
use crate::error::Result;
use crate::gu::ActiveGuRule;
use crate::moves::{
    fallback_template, move_template_for_concept, render_move_prompt, select_next_move_for_concept,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachingInstruction {
    pub focus: TeachingFocus,
    #[serde(rename = "move")]
    pub move_name: String,
    pub target_depth: String,
    pub target: String,
    #[serde(rename = "do")]
    pub do_text: String,
    pub dont: String,
    pub anchor: String,
    pub context: Option<TeachingContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeachingFocus {
    pub kind: String,
    pub concept_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachingContext {
    pub recent_attempts: Vec<TeachingAttemptSummary>,
    pub latest_failed_response: Option<String>,
    pub active_gu_rules: Vec<ActiveGuRule>,
    pub previous_anchor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachingAttemptSummary {
    pub created_at: Option<String>,
    pub task_type: Option<String>,
    pub final_score: Option<f64>,
    pub self_confidence: Option<i32>,
    pub misconception_id: Option<String>,
}

pub fn teaching_context(conn: &Connection, concept_id: &str) -> Result<Option<TeachingContext>> {
    let limit = meta_i64(conn, "teaching.context_attempt_limit")?.clamp(1, 10);
    let mut statement = conn.prepare(
        "SELECT created_at, task_type, final_score, self_confidence, misconception_id
         FROM attempts
         WHERE concept_id=?1
         ORDER BY created_at DESC, id DESC
         LIMIT ?2",
    )?;
    let recent_attempts = statement
        .query_map(params![concept_id, limit], |row| {
            Ok(TeachingAttemptSummary {
                created_at: row.get(0)?,
                task_type: row.get(1)?,
                final_score: row.get(2)?,
                self_confidence: row.get(3)?,
                misconception_id: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let cut_lo = meta_f64(conn, "bkt.cut_lo")?;
    let latest_failed_response = conn
        .query_row(
            "SELECT e.text
             FROM attempts a
             JOIN evidence_items e ON e.id=a.response_evidence_id
             WHERE a.concept_id=?1 AND a.final_score IS NOT NULL AND a.final_score < ?2
             ORDER BY a.created_at DESC, a.id DESC
             LIMIT 1",
            params![concept_id, cut_lo],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|text| summarize_text(&text, 240));

    let concept_ids_json = serde_json::to_string(&vec![concept_id])?;
    let mut statement = conn.prepare(
        "SELECT id, pattern, status, concept_ids_json
         FROM gu_rules
         WHERE status='active'
           AND EXISTS (
             SELECT 1 FROM json_each(gu_rules.concept_ids_json)
             WHERE value IN (SELECT value FROM json_each(?1))
           )
         ORDER BY updated_at DESC, id ASC",
    )?;
    let active_gu_rules = statement
        .query_map([concept_ids_json], |row| {
            let concept_ids_json: String = row.get(3)?;
            let concept_ids = serde_json::from_str(&concept_ids_json).unwrap_or_default();
            Ok(ActiveGuRule {
                id: row.get(0)?,
                pattern: row.get(1)?,
                status: row.get(2)?,
                concept_ids,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let previous_anchor = conn
        .query_row(
            "SELECT json_extract(instruction_json, '$.anchor')
             FROM teaching_turns
             WHERE concept_id=?1 AND json_valid(instruction_json)
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
            [concept_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();

    if recent_attempts.is_empty()
        && latest_failed_response.is_none()
        && active_gu_rules.is_empty()
        && previous_anchor.is_none()
    {
        return Ok(None);
    }
    Ok(Some(TeachingContext {
        recent_attempts,
        latest_failed_response,
        active_gu_rules,
        previous_anchor,
    }))
}

pub fn teaching_instruction(conn: &Connection, concept_id: &str) -> Result<TeachingInstruction> {
    let context = teaching_context(conn, concept_id)?;
    let diagnosis = diagnose_concept(conn, concept_id)?;
    if diagnosis.latest_no_attempt_reason.as_deref() == Some("not_understood_prompt") {
        if let Some(gap) = diagnosis.unmet_prerequisites.first() {
            return Ok(prerequisite_repair_instruction(
                concept_id,
                DiagnosisFocus {
                    kind: "prerequisite_gap".to_owned(),
                    concept_id: gap.concept_id.clone(),
                    reason: "prompt_not_understood_with_unmet_prerequisite".to_owned(),
                },
                context,
            ));
        }
        return prompt_not_understood_instruction(conn, concept_id, context);
    }
    if let Some(focus) = diagnosis.focus {
        return Ok(prerequisite_repair_instruction(concept_id, focus, context));
    }

    if let Some(task) = diagnosis.confusion_tasks.first() {
        return Ok(TeachingInstruction {
            focus: TeachingFocus {
                kind: "confusion".to_owned(),
                concept_id: concept_id.to_owned(),
                reason: "adjacent_confusion_edge".to_owned(),
            },
            move_name: "discriminate".to_owned(),
            target_depth: "analyze".to_owned(),
            target: concept_id.to_owned(),
            do_text: "先让学习者作答：说出边界、一个反例、一个区分线索。".to_owned(),
            dont: "不要直接给结论；不要直接改掌握度或调度。".to_owned(),
            anchor: task.prompt.clone(),
            context,
        });
    }

    let name: String = conn.query_row(
        "SELECT name FROM concepts WHERE id=?1",
        [concept_id],
        |row| row.get(0),
    )?;
    let selected_move = select_next_move_for_concept(conn, concept_id)?;
    let generativity: String = conn.query_row(
        "SELECT generativity FROM concepts WHERE id=?1",
        [concept_id],
        |row| row.get(0),
    )?;
    let selected_move = if generativity == "generative" {
        crate::moves::bloom_move("transfer")
    } else {
        selected_move
    };
    let template = move_template_for_concept(conn, concept_id, selected_move.id)?
        .map(|item| item.template)
        .unwrap_or_else(|| fallback_template(selected_move.id).to_owned());
    Ok(TeachingInstruction {
        focus: TeachingFocus {
            kind: "concept".to_owned(),
            concept_id: concept_id.to_owned(),
            reason: "scheduled_concept".to_owned(),
        },
        move_name: selected_move.id.to_owned(),
        target_depth: selected_move.target_depth.to_owned(),
        target: concept_id.to_owned(),
        do_text: if generativity == "generative" {
            "给一个没教过的同族实例，让学习者推断并说明依据；先收回答，再追问证据。".to_owned()
        } else {
            "先让学习者作答：用自己的话说明核心约束，再追问证据。".to_owned()
        },
        dont: "不要替学习者完成回答；不要直接改掌握度或调度。".to_owned(),
        anchor: render_move_prompt(&template, &name),
        context,
    })
}

fn prompt_not_understood_instruction(
    conn: &Connection,
    concept_id: &str,
    context: Option<TeachingContext>,
) -> Result<TeachingInstruction> {
    let name: String = conn.query_row(
        "SELECT name FROM concepts WHERE id=?1",
        [concept_id],
        |row| row.get(0),
    )?;
    Ok(TeachingInstruction {
        focus: TeachingFocus {
            kind: "prompt_not_understood".to_owned(),
            concept_id: concept_id.to_owned(),
            reason: "latest_no_attempt_not_understood_prompt".to_owned(),
        },
        move_name: "worked_example".to_owned(),
        target_depth: "recall".to_owned(),
        target: concept_id.to_owned(),
        do_text: "先拆解题目在问什么，给一个带步骤的最小工作样例，再让学习者复述约束。".to_owned(),
        dont: "不要在同一深度原样重出题；不要把未作答当成答错或直接改掌握度。".to_owned(),
        anchor: format!("先解释 {name} 题目的关键词、输入与期望输出。"),
        context,
    })
}

fn prerequisite_repair_instruction(
    requested_concept_id: &str,
    focus: DiagnosisFocus,
    context: Option<TeachingContext>,
) -> TeachingInstruction {
    TeachingInstruction {
        focus: TeachingFocus {
            kind: focus.kind,
            concept_id: focus.concept_id.clone(),
            reason: focus.reason,
        },
        move_name: "repair_prerequisite".to_owned(),
        target_depth: "explain".to_owned(),
        target: focus.concept_id.clone(),
        do_text: "先让学习者作答：解释这个前置概念，再回到原目标。".to_owned(),
        dont: "不要直接改掌握度或调度；不要把外部 AI 判断当成评分结果。".to_owned(),
        anchor: format!(
            "target={requested_concept_id}; prerequisite={}; latest failure suggests prerequisite repair",
            focus.concept_id
        ),
        context,
    }
}

fn summarize_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_owned();
    }
    let mut summary = trimmed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    summary.push('…');
    summary
}
