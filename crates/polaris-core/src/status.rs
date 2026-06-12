use rusqlite::Connection;
use serde::Serialize;

use crate::error::Result;
use crate::fsrs::{retrievability, FsrsState};
use crate::phase::Phase;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusSnapshot {
    pub due_today: i64,
    pub concepts: Vec<ConceptStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConceptStatus {
    pub concept_id: String,
    pub name: String,
    pub retrieval: Option<f64>,
    pub p_known: f64,
    pub calib_gap: f64,
    pub phase: String,
}

pub fn status_snapshot(conn: &Connection) -> Result<StatusSnapshot> {
    let due_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM mastery_states
         WHERE next_due_at IS NOT NULL AND julianday(next_due_at) <= julianday('now')",
        [],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name,
                COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                COALESCE(ms.calib_gap, 0.0),
                COALESCE(ms.attempt_count, 0),
                ms.fsrs_json,
                CASE
                    WHEN ms.last_review_at IS NULL THEN NULL
                    ELSE julianday('now') - julianday(ms.last_review_at)
                END,
                COALESCE(ms.phase, 'undetermined')
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         ORDER BY c.seed_order ASC, c.id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let fsrs_json: Option<String> = row.get(5)?;
        let elapsed_days: Option<f64> = row.get(6)?;
        let retrieval = fsrs_json
            .as_deref()
            .and_then(|json| serde_json::from_str::<FsrsState>(json).ok())
            .map(|state| retrievability(state.stability, elapsed_days.unwrap_or(0.0).max(0.0)));
        let p_known = row.get::<_, f64>(2)?;
        let calib_gap = row.get::<_, f64>(3)?;
        let phase = Phase::parse(&row.get::<_, String>(7)?)
            .unwrap_or(Phase::Undetermined)
            .as_str()
            .to_owned();

        Ok(ConceptStatus {
            concept_id: row.get(0)?,
            name: row.get(1)?,
            retrieval,
            p_known,
            calib_gap,
            phase,
        })
    })?;
    let concepts = rows.collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(StatusSnapshot {
        due_today,
        concepts,
    })
}
