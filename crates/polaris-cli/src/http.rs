use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::learner_feedback::LearnerFeedbackInput;
use serde_json::{json, Value};

const MAX_REQUEST_BYTES: usize = 1_048_576;

pub struct HttpApi {
    engine: Engine,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Value,
}

type HttpRequestParts = (String, String, String);
type ParsedHttpRequest = Result<HttpRequestParts, String>;

impl HttpApi {
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }

    #[cfg(test)]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn handle(
        &mut self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let path = path.split('?').next().unwrap_or(path);
        match (method, path) {
            ("OPTIONS", _) => Ok(response(405, json!({"error": "method not allowed"}))),
            ("GET", "/health") => Ok(response(
                200,
                json!({
                    "service": "polaris-core",
                    "version": env!("CARGO_PKG_VERSION"),
                }),
            )),
            ("GET", "/status") => Ok(response(
                200,
                serde_json::to_value(self.engine.status_snapshot()?)?,
            )),
            ("GET", "/learner-mirror") => Ok(response(
                200,
                serde_json::to_value(self.engine.learner_mirror_snapshot()?)?,
            )),
            ("POST", "/next") => self.next(body),
            ("POST", "/evidence") => self.evidence(body),
            ("POST", "/feedback") => self.feedback(body),
            ("GET", "/next")
            | ("GET", "/evidence")
            | ("GET", "/feedback")
            | ("POST", "/status")
            | ("POST", "/learner-mirror") => {
                Ok(response(405, json!({"error": "method not allowed"})))
            }
            _ => Ok(response(404, json!({"error": "not found"}))),
        }
    }

    fn next(&mut self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let session = optional_str(&arguments, "session").unwrap_or("http");
        let Some(task) = self.engine.next_task()? else {
            return Ok(response(200, json!({ "task": null })));
        };

        self.engine.record_next_task_event(session, &task)?;

        let instruction = self.engine.teaching_instruction(&task.concept_id)?;
        Ok(response(
            200,
            json!({
                "task": {
                    "concept_id": task.concept_id,
                    "task_type": task.task_type,
                    "prompt": task.prompt_text,
                    "reason": task.reason,
                },
                "teaching_instruction": instruction,
            }),
        ))
    }

    fn evidence(&mut self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(session) = optional_str(&arguments, "session") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: session"}),
            ));
        };
        let Some(concept) = concept_id_argument(&arguments) else {
            return Ok(response(
                400,
                json!({"error": "missing string field: concept_id"}),
            ));
        };
        let Some(response_text) = optional_str(&arguments, "response") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: response"}),
            ));
        };
        let Some(confidence) = arguments.get("confidence").and_then(Value::as_i64) else {
            return Ok(response(
                400,
                json!({"error": "missing integer field: confidence"}),
            ));
        };
        if !(1..=5).contains(&confidence) {
            return Ok(response(
                400,
                json!({"error": "confidence must be in 1..=5"}),
            ));
        }

        let task_type = optional_str(&arguments, "task_type").unwrap_or("recall");
        let prompt = optional_str(&arguments, "prompt").unwrap_or("");
        let observation =
            crate::read_behavior_observation_now(self.engine.conn(), session, concept)?;
        let receipt = self.engine.submit_provisional(SubmitInput {
            session_id: session.to_owned(),
            concept_id: concept.to_owned(),
            task_type: task_type.to_owned(),
            prompt_text: prompt.to_owned(),
            response_text: response_text.to_owned(),
            self_confidence: confidence as i32,
            latency_ms: observation.latency_ms,
            hint_count: observation.hint_count,
        })?;

        Ok(response(
            200,
            json!({
                "attempt_id": receipt.attempt_id,
                "provisional_score": receipt.provisional_score,
                "degraded": receipt.degraded,
            }),
        ))
    }

    fn feedback(&mut self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(kind) = optional_str(&arguments, "kind") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: kind"}),
            ));
        };
        let session = optional_str(&arguments, "session").unwrap_or("http");
        let input = LearnerFeedbackInput {
            session_id: session.to_owned(),
            source: "http".to_owned(),
            kind: kind.to_owned(),
            concept_id: concept_id_argument(&arguments).map(str::to_owned),
            state: optional_str(&arguments, "state").map(str::to_owned),
            reason: optional_str(&arguments, "reason").map(str::to_owned),
            note: optional_str(&arguments, "note").map(str::to_owned),
        };

        match self.engine.record_learner_feedback(input) {
            Ok(receipt) => Ok(response(200, serde_json::to_value(receipt)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }
}

pub fn serve_http(engine: Engine, host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("{host}:{port}"))?;
    let local_addr = listener.local_addr()?;
    println!("listening on http://{local_addr}");
    let mut api = HttpApi::new(engine);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_stream(&mut api, stream) {
                    eprintln!("http request failed: {error}");
                }
            }
            Err(error) => eprintln!("http accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_stream(
    api: &mut HttpApi,
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(&mut stream)?;
    let response = match request {
        Ok((method, path, body)) => match api.handle(&method, &path, &body) {
            Ok(response) => response,
            Err(error) => response(500, json!({ "error": error.to_string() })),
        },
        Err(error) => response(400, json!({ "error": error })),
    };
    write_response(&mut stream, &response)?;
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> Result<ParsedHttpRequest, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Ok(Err("request too large".to_owned()));
        }
        if header_end.is_none() {
            if let Some(index) = find_header_end(&bytes) {
                header_end = Some(index);
                let header_text = String::from_utf8_lossy(&bytes[..index]);
                content_length = match parse_content_length(&header_text) {
                    Ok(content_length) => content_length,
                    Err(error) => return Ok(Err(error)),
                };
                if content_length > MAX_REQUEST_BYTES {
                    return Ok(Err("request body too large".to_owned()));
                }
            }
        }
        if let Some(index) = header_end {
            let body_start = index + 4;
            let Some(request_len) = body_start.checked_add(content_length) else {
                return Ok(Err("request too large".to_owned()));
            };
            if bytes.len() >= request_len {
                break;
            }
        }
    }

    let Some(header_end) = header_end else {
        return Ok(Err("missing HTTP headers".to_owned()));
    };
    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = header_text.lines();
    let Some(request_line) = lines.next() else {
        return Ok(Err("missing request line".to_owned()));
    };
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 3 {
        return Ok(Err("invalid request line".to_owned()));
    }
    let method = parts[0].to_owned();
    let path = parts[1].to_owned();
    let body_start = header_end + 4;
    let Some(body_end) = body_start.checked_add(content_length) else {
        return Ok(Err("request too large".to_owned()));
    };
    if bytes.len() < body_end {
        return Ok(Err("incomplete request body".to_owned()));
    }
    let body = String::from_utf8_lossy(&bytes[body_start..body_end]).to_string();
    Ok(Ok((method, path, body)))
}

fn write_response(
    stream: &mut TcpStream,
    response: &HttpResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::to_vec(&response.body)?;
    let status_text = status_text(response.status);
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        status_text,
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

fn response(status: u16, body: Value) -> HttpResponse {
    HttpResponse { status, body }
}

fn json_body(body: &str) -> Result<Value, String> {
    if body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn concept_id_argument(value: &Value) -> Option<&str> {
    optional_str(value, "concept_id").or_else(|| optional_str(value, "concept"))
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(header: &str) -> Result<usize, String> {
    let mut content_length = None;
    for line in header.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(
                value
                    .trim()
                    .parse()
                    .map_err(|_| "invalid Content-Length".to_owned())?,
            );
        }
    }
    Ok(content_length.unwrap_or(0))
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

#[cfg(test)]
mod tests {
    use polaris_core::db::migrate;
    use polaris_core::engine::Engine;
    use rusqlite::Connection;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::*;

    #[test]
    fn http_health_returns_service_metadata() {
        let mut api = test_api();

        let response = api.handle("GET", "/health", "").unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["service"], "polaris-core");
        assert_eq!(response.body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn http_status_reuses_p04a_status_snapshot() {
        let mut api = test_api();

        let response = api.handle("GET", "/status", "").unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["due_today"], 0);
        assert_eq!(
            response.body["phase_counts"][0]["phase"],
            Value::String("undetermined".to_owned())
        );
        assert_eq!(response.body["concepts"][0]["concept_id"], "ownership");
    }

    #[test]
    fn http_learner_mirror_returns_static_panel_snapshot() {
        let mut api = test_api();

        let response = api.handle("GET", "/learner-mirror", "").unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body["generated_at"].as_str().is_some());
        assert!(response.body["confidence_curve"].as_array().is_some());
        assert_eq!(
            response.body["phase_distribution"]
                .as_array()
                .unwrap()
                .len(),
            polaris_core::phase::Phase::ALL.len()
        );
        assert!(response.body["recent_assertions"].as_array().is_some());
    }

    #[test]
    fn http_learner_feedback_records_state_report() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/feedback",
                &json!({
                    "session": "http-flow",
                    "kind": "state",
                    "concept_id": "ownership",
                    "state": "frustrated",
                    "note": "transfer feels stuck"
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["kind"], "state");
        assert_eq!(response.body["state"], "frustrated");
        assert_eq!(response.body["effect"], "recorded_only");
        let events: i64 = api
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events
                 WHERE session_id='http-flow'
                   AND concept_id='ownership'
                   AND type='learner_feedback'
                   AND json_extract(payload_json, '$.kind')='state'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 1);
    }

    #[test]
    fn http_learner_feedback_records_pause_request() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/feedback",
                &json!({
                    "kind": "pause",
                    "reason": "today is enough"
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["session_id"], "http");
        assert_eq!(response.body["kind"], "pause");
        assert_eq!(response.body["reason"], "today is enough");
        assert_eq!(response.body["effect"], "recorded_only");
        let abandon_events: i64 = api
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events WHERE type='abandon'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(abandon_events, 0);
    }

    #[test]
    fn http_learner_feedback_rejects_invalid_kind() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/feedback",
                &json!({
                    "kind": "mood",
                    "state": "flow"
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 400);
        assert!(response.body["error"]
            .as_str()
            .unwrap()
            .contains("learner_feedback.kind"));
    }

    #[test]
    fn http_learner_feedback_rejects_malformed_json_with_stable_error() {
        let mut api = test_api();

        let response = api.handle("POST", "/feedback", "{").unwrap();

        assert_eq!(response.status, 400);
        assert!(response.body["error"]
            .as_str()
            .unwrap()
            .contains("invalid JSON"));
    }

    #[test]
    fn http_next_records_behavior_event_and_returns_instruction() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/next",
                &json!({"session": "http-flow"}).to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        let concept_id = response.body["task"]["concept_id"].as_str().unwrap();
        assert_eq!(response.body["teaching_instruction"]["target"], concept_id);
        let next_events: i64 = api
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events
                 WHERE session_id='http-flow' AND concept_id=?1 AND type='next'",
                [concept_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_events, 1);
    }

    #[test]
    fn http_evidence_uses_engine_scoring_without_trusting_external_score() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "http-flow",
                    "concept_id": "ownership",
                    "response": "Ownership controls which binding can drop a value.",
                    "confidence": 4,
                    "external_score": 1.0,
                    "final_score": 1.0
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body["attempt_id"].as_str().is_some());
        assert!((response.body["provisional_score"].as_f64().unwrap() - 0.70).abs() < 1e-9);
        let final_score: Option<f64> = api
            .engine()
            .conn()
            .query_row(
                "SELECT final_score FROM attempts WHERE concept_id='ownership'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(final_score, None);
    }

    #[test]
    fn http_evidence_rejects_invalid_confidence() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "http-flow",
                    "concept_id": "ownership",
                    "response": "Ownership controls drops.",
                    "confidence": 6
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 400);
        assert!(response.body["error"]
            .as_str()
            .unwrap()
            .contains("confidence"));
    }

    #[test]
    fn http_evidence_rejects_malformed_json_with_stable_error() {
        let mut api = test_api();

        let response = api.handle("POST", "/evidence", "{").unwrap();

        assert_eq!(response.status, 400);
        assert!(response.body["error"]
            .as_str()
            .unwrap()
            .contains("invalid JSON"));
    }

    #[test]
    fn http_evidence_queues_without_final_grading() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "http-flow",
                    "concept_id": "ownership",
                    "response": "Ownership controls which binding can drop a value.",
                    "confidence": 4
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["degraded"], true);
        let queued: i64 = api
            .engine()
            .conn()
            .query_row("SELECT COUNT(*) FROM grade_queue", [], |row| row.get(0))
            .unwrap();
        assert_eq!(queued, 1);
    }

    #[test]
    fn http_stream_serves_health_json() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut api = test_api();
            let (stream, _) = listener.accept().unwrap();
            handle_stream(&mut api, stream).unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let mut text = String::new();
        client.read_to_string(&mut text).unwrap();
        server.join().unwrap();

        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(text.contains("Content-Type: application/json"), "{text}");
        assert!(!text.contains("Access-Control-Allow-Origin: *"), "{text}");
        assert!(text.contains(r#""service":"polaris-core""#), "{text}");
    }

    #[test]
    fn http_learner_mirror_stream_does_not_add_wildcard_cors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut api = test_api();
            let (stream, _) = listener.accept().unwrap();
            handle_stream(&mut api, stream).unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"GET /learner-mirror HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let mut text = String::new();
        client.read_to_string(&mut text).unwrap();
        server.join().unwrap();

        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(!text.contains("Access-Control-Allow-Origin: *"), "{text}");
        assert!(text.contains(r#""recent_assertions""#), "{text}");
    }

    #[test]
    fn http_stream_rejects_truncated_body_without_panicking() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut api = test_api();
            let (stream, _) = listener.accept().unwrap();
            handle_stream(&mut api, stream).unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"POST /next HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 20\r\n\r\n{}")
            .unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut text = String::new();
        client.read_to_string(&mut text).unwrap();
        server.join().unwrap();

        assert!(text.starts_with("HTTP/1.1 400 Bad Request"), "{text}");
        assert!(text.contains("incomplete request body"), "{text}");
    }

    fn test_api() -> HttpApi {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
        HttpApi::new(engine)
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }
}
