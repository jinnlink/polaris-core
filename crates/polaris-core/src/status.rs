use std::collections::BTreeMap;

use rusqlite::{Connection, Row};
use serde::Serialize;

use crate::error::Result;
use crate::fsrs::{retrievability, FsrsState};
use crate::pack_state::{active_pack, list_packs, theta_mode_for_pack, PackSummary};
use crate::phase::Phase;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "desktop-bindings", derive(ts_rs::TS))]
pub struct StatusSnapshot {
    pub generated_at: String,
    pub current_pack: Option<String>,
    pub theta_mode: Option<String>,
    pub packs: Vec<PackSummary>,
    #[cfg_attr(feature = "desktop-bindings", ts(type = "number"))]
    pub due_today: i64,
    pub phase_counts: Vec<PhaseCount>,
    pub concepts: Vec<ConceptStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "desktop-bindings", derive(ts_rs::TS))]
pub struct PhaseCount {
    pub phase: String,
    #[cfg_attr(feature = "desktop-bindings", ts(type = "number"))]
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "desktop-bindings", derive(ts_rs::TS))]
pub struct ConceptStatus {
    pub concept_id: String,
    pub name: String,
    pub retrieval: Option<f64>,
    pub p_known: f64,
    pub calib_gap: f64,
    pub phase: String,
    pub phase_label: String,
    pub phase_summary: String,
}

pub fn status_snapshot(conn: &Connection) -> Result<StatusSnapshot> {
    let generated_at: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
            row.get(0)
        })?;
    let current_pack = active_pack(conn)?;
    let theta_mode = current_pack
        .as_deref()
        .map(|pack| theta_mode_for_pack(conn, pack).map(|mode| mode.as_str().to_owned()))
        .transpose()?;
    let due_today = due_today(conn, current_pack.as_deref())?;
    let sql = format!(
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
         {}
         ORDER BY c.seed_order ASC, c.id ASC",
        if current_pack.is_some() {
            "WHERE c.pack=?1"
        } else {
            ""
        }
    );
    let mut stmt = conn.prepare(&sql)?;
    let concepts = if let Some(pack) = current_pack.as_deref() {
        stmt.query_map([pack], concept_status_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], concept_status_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };

    Ok(StatusSnapshot {
        generated_at,
        current_pack: current_pack.clone(),
        theta_mode,
        packs: list_packs(conn)?,
        due_today,
        phase_counts: phase_counts(conn, current_pack.as_deref())?,
        concepts,
    })
}

fn due_today(conn: &Connection, active: Option<&str>) -> Result<i64> {
    if let Some(pack) = active {
        return conn
            .query_row(
                "SELECT COUNT(*)
                 FROM mastery_states ms
                 JOIN concepts c ON c.id=ms.concept_id
                 WHERE c.pack=?1
                   AND ms.next_due_at IS NOT NULL
                   AND julianday(ms.next_due_at) <= julianday('now')",
                [pack],
                |row| row.get(0),
            )
            .map_err(Into::into);
    }
    conn.query_row(
        "SELECT COUNT(*) FROM mastery_states
         WHERE next_due_at IS NOT NULL AND julianday(next_due_at) <= julianday('now')",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn concept_status_from_row(row: &Row<'_>) -> rusqlite::Result<ConceptStatus> {
    let fsrs_json: Option<String> = row.get(5)?;
    let elapsed_days: Option<f64> = row.get(6)?;
    let retrieval = fsrs_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<FsrsState>(json).ok())
        .map(|state| retrievability(state.stability, elapsed_days.unwrap_or(0.0).max(0.0)));
    let p_known = row.get::<_, f64>(2)?;
    let calib_gap = row.get::<_, f64>(3)?;
    let phase = Phase::parse(&row.get::<_, String>(7)?).unwrap_or(Phase::Undetermined);

    Ok(ConceptStatus {
        concept_id: row.get(0)?,
        name: row.get(1)?,
        retrieval,
        p_known,
        calib_gap,
        phase: phase.as_str().to_owned(),
        phase_label: phase.label().to_owned(),
        phase_summary: phase.summary().to_owned(),
    })
}

fn phase_counts(conn: &Connection, active: Option<&str>) -> Result<Vec<PhaseCount>> {
    let mut counts = BTreeMap::<String, i64>::new();
    let sql = format!(
        "SELECT COALESCE(ms.phase, 'undetermined'), COUNT(*)
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         {}
         GROUP BY COALESCE(ms.phase, 'undetermined')",
        if active.is_some() {
            "WHERE c.pack=?1"
        } else {
            ""
        }
    );
    let mut stmt = conn.prepare(&sql)?;
    if let Some(pack) = active {
        let rows = stmt.query_map([pack], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            insert_phase_count(&mut counts, row?);
        }
    } else {
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            insert_phase_count(&mut counts, row?);
        }
    }

    Ok(Phase::ALL
        .iter()
        .map(|phase| {
            let phase = phase.as_str().to_owned();
            let count = counts.get(&phase).copied().unwrap_or(0);
            PhaseCount { phase, count }
        })
        .collect())
}

fn insert_phase_count(counts: &mut BTreeMap<String, i64>, row: (String, i64)) {
    let (raw_phase, count) = row;
    let phase = Phase::parse(&raw_phase)
        .unwrap_or(Phase::Undetermined)
        .as_str()
        .to_owned();
    *counts.entry(phase).or_insert(0) += count.max(0);
}
