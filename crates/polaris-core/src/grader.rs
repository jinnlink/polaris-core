use rusqlite::Connection;

use crate::config::{default_registry, ParameterSpec};
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmConfig {
    Unavailable,
    OpenAiCompatible {
        base_url: String,
        model: String,
        api_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeRequest {
    pub attempt_id: String,
    pub self_confidence: i32,
    pub response_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradeResult {
    pub score: f64,
    pub depth: String,
    pub degraded: bool,
}

pub fn grade_with_config(
    conn: &Connection,
    request: GradeRequest,
    config: LlmConfig,
) -> Result<GradeResult> {
    match config {
        LlmConfig::Unavailable => degrade(conn, &request, "llm config missing"),
        LlmConfig::OpenAiCompatible { .. } => {
            degrade(conn, &request, "llm call not implemented in P01 test path")
        }
    }
}

pub fn heuristic_score(self_confidence: i32) -> f64 {
    let registry = default_registry();
    let base = parse_f64(&registry, "grade.provisional_base");
    let slope = parse_f64(&registry, "grade.provisional_slope");
    let conf_norm = ((self_confidence as f64 - 1.0) / 4.0).clamp(0.0, 1.0);
    (base + slope * conf_norm).clamp(0.0, 1.0)
}

fn degrade(conn: &Connection, request: &GradeRequest, reason: &str) -> Result<GradeResult> {
    conn.execute(
        "INSERT OR REPLACE INTO grade_queue(attempt_id, enqueued_at, retry_count, last_error)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), COALESCE((SELECT retry_count FROM grade_queue WHERE attempt_id=?1), 0), ?2)",
        (&request.attempt_id, reason),
    )?;

    Ok(GradeResult {
        score: heuristic_score(request.self_confidence),
        depth: "recall".to_owned(),
        degraded: true,
    })
}

fn parse_f64(
    registry: &std::collections::BTreeMap<&'static str, ParameterSpec>,
    key: &'static str,
) -> f64 {
    registry[key]
        .default_value
        .parse()
        .expect("valid default f64 parameter")
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrate;

    #[test]
    fn missing_llm_config_degrades_to_heuristic_and_queues_retry() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let result = grade_with_config(
            &conn,
            GradeRequest {
                attempt_id: "attempt-1".to_owned(),
                self_confidence: 4,
                response_text: "Borrowing lets code refer to a value without taking ownership."
                    .to_owned(),
            },
            LlmConfig::Unavailable,
        )
        .unwrap();

        assert!(result.degraded);
        assert!((result.score - 0.70).abs() < 1e-9);

        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM grade_queue WHERE attempt_id='attempt-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(queued, 1);
    }
}
