use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;

use crate::citation::{validate_citations_with_policy, Citation, CitationPolicy, EvidenceText};
use crate::config::{default_registry, meta_f64, ParameterSpec};
use crate::error::{PolarisError, Result};
use crate::fsrs::{FsrsParams, Rating};
use crate::gu::active_gu_prompt_for_concept;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmConfig {
    Unavailable,
    OpenAiCompatible {
        base_url: String,
        model: String,
        api_key: String,
    },
}

impl LlmConfig {
    pub fn from_env() -> Self {
        read_env_config("POLARIS_LLM_FAST")
            .or_else(|| read_env_config("POLARIS_LLM_STRONG"))
            .unwrap_or(Self::Unavailable)
    }
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
    pub misconception_id: Option<String>,
    pub citations: Vec<Citation>,
    pub degraded: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RawGrade {
    score: f64,
    depth: String,
    #[serde(default)]
    misconception_id: Option<String>,
    #[serde(default)]
    pattern_tags: Vec<String>,
    #[serde(default)]
    citations: Vec<Citation>,
}

pub fn grade_with_config(
    conn: &Connection,
    request: GradeRequest,
    config: LlmConfig,
) -> Result<GradeResult> {
    match config {
        LlmConfig::Unavailable => degrade(conn, &request, "llm config missing"),
        LlmConfig::OpenAiCompatible {
            base_url,
            model,
            api_key,
        } => {
            let rubric = rubric_for_attempt(conn, &request.attempt_id)?;
            let evidence_prompt = evidence_prompt_for_attempt(conn, &request.attempt_id)?;
            let mut last_error = String::new();
            for _ in 0..2 {
                match call_openai_compatible(
                    &request,
                    &rubric,
                    &evidence_prompt,
                    &base_url,
                    &model,
                    &api_key,
                )
                .and_then(|response| grade_with_static_response(conn, request.clone(), &response))
                {
                    Ok(result) if !result.degraded => return Ok(result),
                    Ok(_) => last_error = "strict citation validation failed".to_owned(),
                    Err(error) => last_error = error.to_string(),
                }
            }
            degrade(conn, &request, &last_error)
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

pub fn heuristic_score_with_conn(conn: &Connection, self_confidence: i32) -> Result<f64> {
    let base = meta_f64(conn, "grade.provisional_base")?;
    let slope = meta_f64(conn, "grade.provisional_slope")?;
    let conf_norm = ((self_confidence as f64 - 1.0) / 4.0).clamp(0.0, 1.0);
    Ok((base + slope * conf_norm).clamp(0.0, 1.0))
}

pub fn grade_with_static_response(
    conn: &Connection,
    request: GradeRequest,
    response_json: &str,
) -> Result<GradeResult> {
    match parse_and_validate(conn, &request, response_json) {
        Ok(raw) => apply_grade(conn, &request.attempt_id, raw, response_json),
        Err(error) => degrade(conn, &request, &error),
    }
}

pub fn grade_request_for_attempt(conn: &Connection, attempt_id: &str) -> Result<GradeRequest> {
    conn.query_row(
        "SELECT COALESCE(a.self_confidence, 3), COALESCE(e.text, '')
         FROM attempts a
         LEFT JOIN evidence_items e ON e.id=a.response_evidence_id
         WHERE a.id=?1",
        [attempt_id],
        |row| {
            Ok(GradeRequest {
                attempt_id: attempt_id.to_owned(),
                self_confidence: row.get(0)?,
                response_text: row.get(1)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))
}

fn degrade(conn: &Connection, request: &GradeRequest, reason: &str) -> Result<GradeResult> {
    conn.execute(
        "INSERT OR REPLACE INTO grade_queue(attempt_id, enqueued_at, retry_count, last_error)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), COALESCE((SELECT retry_count FROM grade_queue WHERE attempt_id=?1), 0), ?2)",
        (&request.attempt_id, reason),
    )?;

    Ok(GradeResult {
        score: heuristic_score_with_conn(conn, request.self_confidence)?,
        depth: "recall".to_owned(),
        misconception_id: None,
        citations: Vec::new(),
        degraded: true,
    })
}

fn parse_and_validate(
    conn: &Connection,
    request: &GradeRequest,
    response_json: &str,
) -> std::result::Result<RawGrade, String> {
    let mut raw: RawGrade =
        serde_json::from_str(response_json).map_err(|error| error.to_string())?;
    raw.score = raw.score.clamp(0.0, 1.0);
    if !matches!(
        raw.depth.as_str(),
        "recall" | "explain" | "apply" | "analyze" | "evaluate" | "create" | "transfer"
    ) {
        return Err(format!("invalid depth {}", raw.depth));
    }
    raw.misconception_id = raw
        .misconception_id
        .and_then(|value| (!value.trim().is_empty()).then_some(value));
    if raw.citations.is_empty() {
        return Err("missing citation".to_owned());
    }

    let evidence =
        evidence_for_attempt(conn, &request.attempt_id).map_err(|error| error.to_string())?;
    let policy = CitationPolicy::from_conn(conn).map_err(|error| error.to_string())?;
    validate_citations_with_policy(&raw.citations, &evidence, policy)
        .map_err(|error| error.to_string())?;
    Ok(raw)
}

fn evidence_for_attempt(conn: &Connection, attempt_id: &str) -> Result<Vec<EvidenceText>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.text
         FROM attempts a
         JOIN evidence_items e ON e.id=a.response_evidence_id
         WHERE a.id=?1",
    )?;
    let evidence = stmt
        .query_map([attempt_id], |row| {
            Ok(EvidenceText {
                id: row.get(0)?,
                text: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if evidence.is_empty() {
        return Err(PolarisError::MissingAttempt(attempt_id.to_owned()));
    }
    Ok(evidence)
}

fn apply_grade(
    conn: &Connection,
    attempt_id: &str,
    raw: RawGrade,
    response_json: &str,
) -> Result<GradeResult> {
    let fsrs_params = FsrsParams::from_conn(conn)?;
    let rating = Rating::from_score_with_params(raw.score, &fsrs_params);
    let grader_json = serde_json::json!({
        "score": raw.score,
        "depth": raw.depth,
        "misconception_id": raw.misconception_id,
        "pattern_tags": raw.pattern_tags,
        "citations": raw.citations,
        "raw": response_json,
    })
    .to_string();
    conn.execute(
        "UPDATE attempts
         SET final_score=?1,
             depth=?2,
             misconception_id=?3,
             grader_json=?4,
             rating=?5,
             graded_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?6",
        (
            raw.score,
            raw.depth.as_str(),
            raw.misconception_id.as_deref(),
            grader_json,
            format!("{rating:?}").to_lowercase(),
            attempt_id,
        ),
    )?;
    conn.execute("DELETE FROM grade_queue WHERE attempt_id=?1", [attempt_id])?;

    Ok(GradeResult {
        score: raw.score,
        depth: raw.depth,
        misconception_id: raw.misconception_id,
        citations: raw.citations,
        degraded: false,
    })
}

fn call_openai_compatible(
    request: &GradeRequest,
    rubric: &str,
    evidence_prompt: &str,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<String> {
    #[derive(Deserialize)]
    struct ChatResponse {
        choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(Deserialize)]
    struct Message {
        content: String,
    }

    let endpoint = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "temperature": 0,
        "response_format": {"type": "json_object"},
        "messages": [
            {
                "role": "system",
                "content": "You are Polaris Tier 1 grader. Return only JSON with score, depth, optional misconception_id, and citations. Every citation quote must be an exact substring of the submitted evidence."
            },
            {
                "role": "user",
                "content": format!("Attempt id: {}\nSelf confidence: {}\nRubric:\n{}\nAllowed evidence:\n{}", request.attempt_id, request.self_confidence, rubric, evidence_prompt)
            }
        ]
    });

    let response: ChatResponse = reqwest::blocking::Client::new()
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()?
        .error_for_status()?
        .json()?;

    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| PolarisError::InvalidGraderResponse("empty choices".to_owned()))
}

fn evidence_prompt_for_attempt(conn: &Connection, attempt_id: &str) -> Result<String> {
    let evidence = evidence_for_attempt(conn, attempt_id)?;
    Ok(evidence
        .iter()
        .map(|item| format!("evidence_id: {}\ntext: {}", item.id, item.text))
        .collect::<Vec<_>>()
        .join("\n\n"))
}

fn rubric_for_attempt(conn: &Connection, attempt_id: &str) -> Result<String> {
    let (task_type, concept_id, rubric): (String, String, String) = conn
        .query_row(
            "SELECT COALESCE(a.task_type, 'recall'), a.concept_id, COALESCE(
            (SELECT value FROM meta WHERE key='pack.' || c.pack || '.rubric'),
            ''
         )
        FROM attempts a
         LEFT JOIN concepts c ON c.id=a.concept_id
         WHERE a.id=?1",
            [attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))?;
    let gu_prompt = active_gu_prompt_for_concept(conn, &concept_id)?
        .map(|prompt| format!("\n\nG_u risk:\n{prompt}"))
        .unwrap_or_default();
    Ok(format!(
        "Current task_type: {task_type}\nUse the rubric section for this task_type when the pack provides one; otherwise use the closest Bloom depth section.\nReturn optional pattern_tags using only the documented G_u pattern enum when a behavior pattern is evidenced.\n\n{rubric}{gu_prompt}"
    ))
}

fn read_env_config(prefix: &str) -> Option<LlmConfig> {
    let base_url = std::env::var(format!("{prefix}_BASE_URL")).ok()?;
    let model = std::env::var(format!("{prefix}_MODEL")).ok()?;
    let api_key = std::env::var(format!("{prefix}_API_KEY")).ok()?;
    Some(LlmConfig::OpenAiCompatible {
        base_url,
        model,
        api_key,
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

    #[test]
    fn heuristic_score_reads_meta_values() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "UPDATE meta SET value='0.20' WHERE key='grade.provisional_base'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE meta SET value='0.40' WHERE key='grade.provisional_slope'",
            [],
        )
        .unwrap();

        let result = grade_with_config(
            &conn,
            GradeRequest {
                attempt_id: "attempt-2".to_owned(),
                self_confidence: 5,
                response_text: "Borrowing uses references without moving the owner.".to_owned(),
            },
            LlmConfig::Unavailable,
        )
        .unwrap();

        assert!((result.score - 0.60).abs() < 1e-9);
    }

    #[test]
    fn accepted_grade_validates_citations_and_updates_attempt() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
             VALUES ('ev1', 's1', 'cli-submit', 'text/plain', 'Ownership controls which binding can drop a value.', '[\"ownership\"]', '2026-06-11T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, response_evidence_id, self_confidence, provisional_score, created_at)
             VALUES ('attempt-3', 's1', 'ownership', 'recall', 'ev1', 4, 0.70, '2026-06-11T00:00:00Z')",
            [],
        )
        .unwrap();

        let result = grade_with_static_response(
            &conn,
            GradeRequest {
                attempt_id: "attempt-3".to_owned(),
                self_confidence: 4,
                response_text: "Ownership controls which binding can drop a value.".to_owned(),
            },
            r#"{"score":0.82,"depth":"explain","misconception_id":"m1","citations":[{"evidence_id":"ev1","quote":"controls which binding"}]}"#,
        )
        .unwrap();

        assert!(!result.degraded);
        assert!((result.score - 0.82).abs() < 1e-9);

        let stored: (f64, String, String, i64) = conn
            .query_row(
                "SELECT final_score, depth, misconception_id, (SELECT COUNT(*) FROM grade_queue WHERE attempt_id='attempt-3')
                 FROM attempts WHERE id='attempt-3'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(stored, (0.82, "explain".to_owned(), "m1".to_owned(), 0));
    }

    #[test]
    fn invalid_grade_citation_degrades_and_queues_retry() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
             VALUES ('ev1', 's1', 'cli-submit', 'text/plain', 'Ownership controls which binding can drop a value.', '[\"ownership\"]', '2026-06-11T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, response_evidence_id, self_confidence, provisional_score, created_at)
             VALUES ('attempt-4', 's1', 'ownership', 'recall', 'ev1', 4, 0.70, '2026-06-11T00:00:00Z')",
            [],
        )
        .unwrap();

        let result = grade_with_static_response(
            &conn,
            GradeRequest {
                attempt_id: "attempt-4".to_owned(),
                self_confidence: 4,
                response_text: "Ownership controls which binding can drop a value.".to_owned(),
            },
            r#"{"score":0.82,"depth":"explain","citations":[{"evidence_id":"ev1","quote":"not in evidence"}]}"#,
        )
        .unwrap();

        assert!(result.degraded);
        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM grade_queue WHERE attempt_id='attempt-4'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(queued, 1);
    }
}
