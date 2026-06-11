use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::config::meta_f64;
use crate::error::{PolarisError, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphDiagnosis {
    pub concept_id: String,
    pub latest_score: Option<f64>,
    pub latest_failed: bool,
    pub focus: Option<DiagnosisFocus>,
    pub unmet_prerequisites: Vec<PrerequisiteGap>,
    pub confusion_tasks: Vec<DiscriminationTask>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagnosisFocus {
    pub kind: String,
    pub concept_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrerequisiteGap {
    pub concept_id: String,
    pub name: String,
    pub p_known: f64,
    pub threshold: f64,
    pub edge_weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiscriminationTask {
    pub task_type: String,
    pub concept_id: String,
    pub concept_name: String,
    pub contrast_concept_id: String,
    pub contrast_concept_name: String,
    pub prompt: String,
}

pub fn diagnose_concept(conn: &Connection, concept_id: &str) -> Result<GraphDiagnosis> {
    ensure_concept(conn, concept_id)?;

    let latest_score = latest_score(conn, concept_id)?;
    let cut_lo = meta_f64(conn, "bkt.cut_lo")?;
    let latest_failed = latest_score.is_some_and(|score| score <= cut_lo);
    let unmet_prerequisites = unmet_prerequisites(conn, concept_id)?;
    let focus = if latest_failed {
        unmet_prerequisites.first().map(|gap| DiagnosisFocus {
            kind: "prerequisite_gap".to_owned(),
            concept_id: gap.concept_id.clone(),
            reason: "latest_failure_with_unmet_prerequisite".to_owned(),
        })
    } else {
        None
    };

    Ok(GraphDiagnosis {
        concept_id: concept_id.to_owned(),
        latest_score,
        latest_failed,
        focus,
        unmet_prerequisites,
        confusion_tasks: confusion_tasks(conn, concept_id)?,
    })
}

fn ensure_concept(conn: &Connection, concept_id: &str) -> Result<()> {
    let exists: Option<i64> = conn
        .query_row("SELECT 1 FROM concepts WHERE id=?1", [concept_id], |row| {
            row.get(0)
        })
        .optional()?;
    exists
        .map(|_| ())
        .ok_or_else(|| PolarisError::MissingConcept(concept_id.to_owned()))
}

fn latest_score(conn: &Connection, concept_id: &str) -> Result<Option<f64>> {
    conn.query_row(
        "SELECT COALESCE(final_score, provisional_score)
         FROM attempts
         WHERE concept_id=?1 AND COALESCE(final_score, provisional_score) IS NOT NULL
         ORDER BY COALESCE(created_at, '1970-01-01T00:00:00Z') DESC, id DESC
         LIMIT 1",
        [concept_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn unmet_prerequisites(conn: &Connection, concept_id: &str) -> Result<Vec<PrerequisiteGap>> {
    let threshold = meta_f64(conn, "sched.prereq_p")?;
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, COALESCE(ms.p_known, 0.0) AS p_known, COALESCE(e.weight, 1.0)
         FROM edges e
         JOIN concepts c ON c.id=e.src
         LEFT JOIN mastery_states ms ON ms.concept_id=e.src
         WHERE e.dst=?1 AND e.type='prerequisite'
           AND COALESCE(ms.p_known, 0.0) < ?2
         ORDER BY p_known ASC, COALESCE(e.weight, 1.0) DESC, c.id ASC",
    )?;
    let rows = stmt.query_map((concept_id, threshold), |row| {
        Ok(PrerequisiteGap {
            concept_id: row.get(0)?,
            name: row.get(1)?,
            p_known: row.get(2)?,
            threshold,
            edge_weight: row.get(3)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn confusion_tasks(conn: &Connection, concept_id: &str) -> Result<Vec<DiscriminationTask>> {
    let concept_name: String = conn.query_row(
        "SELECT name FROM concepts WHERE id=?1",
        [concept_id],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT other.id, other.name
         FROM edges e
         JOIN concepts other ON other.id = CASE WHEN e.src=?1 THEN e.dst ELSE e.src END
         WHERE e.type='confusion' AND (e.src=?1 OR e.dst=?1)
         ORDER BY other.id ASC",
    )?;
    let rows = stmt.query_map([concept_id], |row| {
        let contrast_concept_id: String = row.get(0)?;
        let contrast_concept_name: String = row.get(1)?;
        Ok(DiscriminationTask {
            task_type: "discriminate".to_owned(),
            concept_id: concept_id.to_owned(),
            concept_name: concept_name.clone(),
            contrast_concept_id: contrast_concept_id.clone(),
            contrast_concept_name: contrast_concept_name.clone(),
            prompt: format!(
                "Discriminate {concept_name} from {contrast_concept_name}: state the boundary, give one counterexample, and name one cue that tells them apart."
            ),
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_kind_is_stable_for_prerequisite_gap() {
        let focus = DiagnosisFocus {
            kind: "prerequisite_gap".to_owned(),
            concept_id: "borrowing".to_owned(),
            reason: "latest_failure_with_unmet_prerequisite".to_owned(),
        };

        assert_eq!(focus.kind, "prerequisite_gap");
        assert_eq!(focus.reason, "latest_failure_with_unmet_prerequisite");
    }
}
