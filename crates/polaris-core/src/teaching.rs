use rusqlite::Connection;
use serde::Serialize;

use crate::diagnosis::{diagnose_concept, DiagnosisFocus};
use crate::error::Result;
use crate::moves::{
    fallback_template, move_template_for_concept, render_move_prompt, select_next_move_for_concept,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeachingFocus {
    pub kind: String,
    pub concept_id: String,
    pub reason: String,
}

pub fn teaching_instruction(conn: &Connection, concept_id: &str) -> Result<TeachingInstruction> {
    let diagnosis = diagnose_concept(conn, concept_id)?;
    if let Some(focus) = diagnosis.focus {
        return Ok(prerequisite_repair_instruction(concept_id, focus));
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
        });
    }

    let name: String = conn.query_row(
        "SELECT name FROM concepts WHERE id=?1",
        [concept_id],
        |row| row.get(0),
    )?;
    let selected_move = select_next_move_for_concept(conn, concept_id)?;
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
        do_text: "先让学习者作答：用自己的话说明核心约束，再追问证据。".to_owned(),
        dont: "不要替学习者完成回答；不要直接改掌握度或调度。".to_owned(),
        anchor: render_move_prompt(&template, &name),
    })
}

fn prerequisite_repair_instruction(
    requested_concept_id: &str,
    focus: DiagnosisFocus,
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
    }
}
