use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::Result;
use crate::phase::Phase;
use crate::report::latest_mirror_report;
use crate::status::status_snapshot;

const CONFIDENCE_CURVE_LIMIT: i64 = 30;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LearnerMirrorSnapshot {
    pub generated_at: String,
    pub confidence_curve: Vec<ConfidenceCurvePoint>,
    pub phase_distribution: Vec<PhaseDistributionItem>,
    pub recent_assertions: Vec<RecentAssertion>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfidenceCurvePoint {
    pub attempt_id: String,
    pub concept_id: String,
    pub created_at: String,
    pub confidence: f64,
    pub actual_score: f64,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhaseDistributionItem {
    pub phase: String,
    pub label: String,
    pub summary: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecentAssertion {
    pub id: String,
    pub kind: String,
    pub claim: String,
    pub confidence: f64,
    pub suggested_action: Option<String>,
}

pub fn learner_mirror_snapshot(conn: &Connection) -> Result<LearnerMirrorSnapshot> {
    let generated_at = snapshot_generated_at(conn)?;
    let status = status_snapshot(conn)?;
    let phase_distribution = status
        .phase_counts
        .into_iter()
        .map(|count| {
            let phase = Phase::parse(&count.phase).unwrap_or(Phase::Undetermined);
            PhaseDistributionItem {
                phase: phase.as_str().to_owned(),
                label: phase.label().to_owned(),
                summary: phase.summary().to_owned(),
                count: count.count,
            }
        })
        .collect();
    let recent_assertions = latest_mirror_report(conn)?
        .map(|report| {
            report
                .assertions
                .into_iter()
                .map(|item| RecentAssertion {
                    id: item.id,
                    kind: item.kind,
                    claim: item.claim,
                    confidence: item.confidence,
                    suggested_action: item.suggested_action,
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(LearnerMirrorSnapshot {
        generated_at,
        confidence_curve: confidence_curve(conn, CONFIDENCE_CURVE_LIMIT)?,
        phase_distribution,
        recent_assertions,
    })
}

fn snapshot_generated_at(conn: &Connection) -> Result<String> {
    conn.query_row(
        "SELECT COALESCE(
             (
                 SELECT ts FROM (
                     SELECT created_at AS ts FROM attempts WHERE created_at IS NOT NULL
                     UNION ALL SELECT graded_at AS ts FROM attempts WHERE graded_at IS NOT NULL
                     UNION ALL SELECT updated_at AS ts FROM mastery_states WHERE updated_at IS NOT NULL
                     UNION ALL SELECT created_at AS ts FROM concepts WHERE created_at IS NOT NULL
                     UNION ALL SELECT generated_at AS ts FROM mirror_reports WHERE generated_at IS NOT NULL
                 )
                 WHERE ts IS NOT NULL AND ts <> ''
                 ORDER BY julianday(ts) DESC, ts DESC
                 LIMIT 1
             ),
             '1970-01-01T00:00:00Z'
         )",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn confidence_curve(conn: &Connection, limit: i64) -> Result<Vec<ConfidenceCurvePoint>> {
    let mut stmt = conn.prepare(
        "SELECT id, concept_id, created_at, self_confidence,
                COALESCE(final_score, provisional_score), final_score IS NOT NULL
         FROM (
             SELECT id, concept_id, COALESCE(created_at, '1970-01-01T00:00:00Z') AS created_at,
                    self_confidence, provisional_score, final_score
             FROM attempts
             WHERE self_confidence IS NOT NULL
               AND COALESCE(final_score, provisional_score) IS NOT NULL
             ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) DESC, id DESC
             LIMIT ?1
         )
         ORDER BY julianday(created_at) ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![limit.max(0)], |row| {
        let confidence_raw: i64 = row.get(3)?;
        let confidence = ((confidence_raw as f64 - 1.0) / 4.0).clamp(0.0, 1.0);
        Ok(ConfidenceCurvePoint {
            attempt_id: row.get(0)?,
            concept_id: row.get(1)?,
            created_at: row.get(2)?,
            confidence,
            actual_score: row.get::<_, f64>(4)?.clamp(0.0, 1.0),
            is_final: row.get::<_, bool>(5)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}
