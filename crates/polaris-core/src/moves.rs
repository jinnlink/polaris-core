use serde::{Deserialize, Serialize};

use rusqlite::{Connection, OptionalExtension};

use crate::error::{PolarisError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveTemplate {
    pub id: String,
    pub task_type: String,
    pub template: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedMove {
    pub id: &'static str,
    pub task_type: &'static str,
    pub target_depth: &'static str,
}

pub fn normalize_move_template(
    id: impl Into<String>,
    task_type: Option<String>,
    template: impl Into<String>,
) -> MoveTemplate {
    let id = id.into();
    let task_type = task_type.unwrap_or_else(|| default_task_type_for_move(&id).to_owned());
    MoveTemplate {
        id,
        task_type,
        template: template.into(),
    }
}

pub fn select_next_move(
    max_depth: Option<&str>,
    p_known: f64,
    analyze_evaluate_attempt_count: i64,
) -> SelectedMove {
    if p_known < 0.5 {
        return bloom_move("recall");
    }

    match max_depth {
        None => bloom_move("recall"),
        Some("recall") => bloom_move("explain"),
        Some("explain") => bloom_move("apply"),
        Some("apply") => {
            if analyze_evaluate_attempt_count.rem_euclid(2) == 0 {
                bloom_move("analyze")
            } else {
                bloom_move("evaluate")
            }
        }
        Some("analyze" | "evaluate") => bloom_move("create"),
        Some("create" | "transfer") => bloom_move("transfer"),
        Some(_) => bloom_move("recall"),
    }
}

pub fn select_next_move_for_concept(conn: &Connection, concept_id: &str) -> Result<SelectedMove> {
    let (p_known, max_depth): (f64, Option<String>) = conn
        .query_row(
            "SELECT
                COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                ms.max_depth
             FROM concepts c
             LEFT JOIN mastery_states ms ON ms.concept_id=c.id
             WHERE c.id=?1",
            [concept_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| PolarisError::MissingConcept(concept_id.to_owned()))?;
    let analyze_evaluate_attempt_count = conn.query_row(
        "SELECT COUNT(*)
         FROM attempts
         WHERE concept_id=?1 AND task_type IN ('analyze', 'evaluate')",
        [concept_id],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(select_next_move(
        max_depth.as_deref(),
        p_known,
        analyze_evaluate_attempt_count,
    ))
}

pub fn move_template_for_concept(
    conn: &Connection,
    concept_id: &str,
    move_id: &str,
) -> Result<Option<MoveTemplate>> {
    let pack_id: Option<String> = conn
        .query_row(
            "SELECT pack FROM concepts WHERE id=?1",
            [concept_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let Some(pack_id) = pack_id else {
        return Ok(None);
    };
    let key = format!("pack.{pack_id}.moves");
    let moves_json: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(moves_json) = moves_json else {
        return Ok(None);
    };
    let moves: Vec<MoveTemplate> = serde_json::from_str(&moves_json)?;
    Ok(moves.into_iter().find(|template| template.id == move_id))
}

pub fn render_move_prompt(template: &str, concept_name: &str) -> String {
    template.replace("{concept}", concept_name)
}

pub fn task_type_target_depth(task_type: &str) -> Option<&'static str> {
    match task_type {
        "recall" => Some("recall"),
        "free_explain" | "explain" => Some("explain"),
        "apply" => Some("apply"),
        "analyze" => Some("analyze"),
        "evaluate" => Some("evaluate"),
        "create" => Some("create"),
        "transfer" | "free_produce" => Some("transfer"),
        _ => None,
    }
}

pub fn default_task_type_for_move(move_id: &str) -> &'static str {
    match move_id {
        "explain" => "free_explain",
        "recall" => "recall",
        "apply" => "apply",
        "analyze" => "analyze",
        "evaluate" => "evaluate",
        "create" => "create",
        "transfer" => "transfer",
        _ => "recall",
    }
}

pub fn fallback_template(move_id: &str) -> &'static str {
    match move_id {
        "explain" => "Explain why {concept} constrains possible solutions.",
        "apply" => "Give a minimal example that applies {concept} and name the boundary.",
        "analyze" => "Compare two {concept} designs and name at least one trade-off.",
        "evaluate" => "Review a {concept} example and identify at least one defect.",
        "create" => "Create a small design that uses {concept} and explain the decision.",
        "transfer" => "Apply {concept} in a different project or domain context.",
        _ => "Recall the core constraint of {concept} in your own words.",
    }
}

pub fn bloom_move(move_id: &str) -> SelectedMove {
    match move_id {
        "explain" => SelectedMove {
            id: "explain",
            task_type: "free_explain",
            target_depth: "explain",
        },
        "apply" => SelectedMove {
            id: "apply",
            task_type: "apply",
            target_depth: "apply",
        },
        "analyze" => SelectedMove {
            id: "analyze",
            task_type: "analyze",
            target_depth: "analyze",
        },
        "evaluate" => SelectedMove {
            id: "evaluate",
            task_type: "evaluate",
            target_depth: "evaluate",
        },
        "create" => SelectedMove {
            id: "create",
            task_type: "create",
            target_depth: "create",
        },
        "transfer" => SelectedMove {
            id: "transfer",
            task_type: "transfer",
            target_depth: "transfer",
        },
        _ => SelectedMove {
            id: "recall",
            task_type: "recall",
            target_depth: "recall",
        },
    }
}
