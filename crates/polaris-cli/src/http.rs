use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use polaris_core::ai_profile::AiInteractionProfileInput;
use polaris_core::capture_queue::{CaptureEffect, CaptureInput, LearnerCaptureKind};
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::error::PolarisError;
use polaris_core::inbox_practice::InboxPracticeSubmissionInput;
use polaris_core::knowledge_map::{KnowledgeMapDueStatus, KnowledgeMapQuery, KnowledgeMapScope};
use polaris_core::learner_feedback::LearnerFeedbackInput;
use polaris_core::learner_inbox::LearnerInboxAction;
use polaris_core::prediction_map::PredictionMapQuery;
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
        let (path, query_string) = path
            .split_once('?')
            .map(|(path, query)| (path, Some(query)))
            .unwrap_or((path, None));
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
            ("GET", "/knowledge-map") => self.knowledge_map(query_string),
            ("GET", "/prediction-map") => self.prediction_map(query_string),
            ("GET", "/profile") => self.global_profile(),
            ("GET", "/session") => self.session_summary(query_string),
            ("GET", "/learner-mirror") => Ok(response(
                200,
                serde_json::to_value(self.engine.learner_mirror_snapshot()?)?,
            )),
            ("GET", "/trust") => Ok(response(
                200,
                serde_json::to_value(self.engine.trust_panel()?)?,
            )),
            ("GET", "/ai-profile") => Ok(response(
                200,
                serde_json::to_value(self.engine.ai_interaction_profile()?)?,
            )),
            ("POST", "/ai-profile") => self.ai_profile_update(body),
            ("GET", "/inbox") => self.inbox(),
            ("POST", "/inbox/action") => self.inbox_action(body),
            ("POST", "/inbox/practice") => self.inbox_practice(body),
            ("POST", "/inbox/practice/submit") => self.inbox_practice_submit(body),
            ("POST", "/capture") => self.capture(body),
            ("POST", "/next") => self.next(body),
            ("POST", "/teaching-turn/explanation") => self.teaching_explanation(body),
            ("POST", "/evidence") => self.evidence(body),
            ("POST", "/materials/performance") => self.material_performance(body),
            ("POST", "/feedback") => self.feedback(body),
            ("GET", "/next")
            | ("GET", "/evidence")
            | ("GET", "/materials/performance")
            | ("GET", "/feedback")
            | (_, "/inbox")
            | (_, "/inbox/action")
            | (_, "/inbox/practice")
            | (_, "/inbox/practice/submit")
            | (_, "/capture")
            | ("GET", "/teaching-turn/explanation")
            | ("POST", "/status")
            | ("POST", "/knowledge-map")
            | ("POST", "/prediction-map")
            | ("POST", "/profile")
            | ("POST", "/session")
            | ("POST", "/learner-mirror")
            | (_, "/trust") => Ok(response(405, json!({"error": "method not allowed"}))),
            (_, "/ai-profile") => Ok(response(405, json!({"error": "method not allowed"}))),
            _ => Ok(response(404, json!({"error": "not found"}))),
        }
    }

    fn knowledge_map(
        &self,
        query_string: Option<&str>,
    ) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let query = match parse_knowledge_map_query(query_string) {
            Ok(query) => query,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        match self.engine.knowledge_map(query) {
            Ok(snapshot) => Ok(response(200, serde_json::to_value(snapshot)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }

    fn prediction_map(
        &self,
        query_string: Option<&str>,
    ) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let query: PredictionMapQuery = match parse_knowledge_map_query(query_string) {
            Ok(query) => query,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        match self.engine.prediction_map(query) {
            Ok(snapshot) => Ok(response(200, serde_json::to_value(snapshot)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }

    fn global_profile(&self) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        match self.engine.global_profile_integration_summary() {
            Ok(summary) => Ok(response(200, serde_json::to_value(summary)?)),
            Err(error @ PolarisError::ProfileSummarySharingDisabled) => {
                Ok(response(403, json!({"error": error.to_string()})))
            }
            Err(error) => Err(Box::new(error)),
        }
    }

    fn session_summary(
        &self,
        query_string: Option<&str>,
    ) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let session_id = match parse_required_session_query(query_string) {
            Ok(session_id) => session_id,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        match self.engine.session_close_summary(&session_id)? {
            Some(summary) => Ok(response(200, serde_json::to_value(summary)?)),
            None => Ok(response(404, json!({"error": "session summary not found"}))),
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

        let task_event_id = self.engine.record_next_task_event(session, &task)?;

        let instruction = self.engine.teaching_instruction(&task.concept_id)?;
        let teaching_turn =
            self.engine
                .begin_teaching_turn(session, &task.concept_id, &instruction)?;
        self.engine
            .associate_teaching_turn_with_task_event(&task_event_id, &teaching_turn.id)?;
        Ok(response(
            200,
            json!({
                "task": {
                    "concept_id": task.concept_id,
                    "task_type": task.task_type,
                    "prompt": task.prompt_text,
                    "reason": task.reason,
                    "context": task.context,
                },
                "teaching_turn_id": teaching_turn.id,
                "teaching_instruction": instruction,
            }),
        ))
    }

    fn teaching_explanation(&self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(teaching_turn_id) = optional_str(&arguments, "teaching_turn_id") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: teaching_turn_id"}),
            ));
        };
        let Some(text) = optional_str(&arguments, "text") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: text"}),
            ));
        };
        match self
            .engine
            .record_teaching_explanation(teaching_turn_id, text)
        {
            Ok(receipt) => Ok(response(200, serde_json::to_value(receipt)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }

    fn capture(&mut self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(text) = optional_str(&arguments, "text") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: text"}),
            ));
        };
        let source = optional_str(&arguments, "source").unwrap_or("paste");
        let content_type = optional_str(&arguments, "content_type").unwrap_or("text/plain");
        let learner_kind_text = optional_str(&arguments, "learner_kind").unwrap_or("reference");
        let Some(learner_kind) = LearnerCaptureKind::parse(learner_kind_text) else {
            return Ok(response(
                400,
                json!({"error": "learner_kind must be one of reference, own_answer, error_log, code_change, chat_excerpt, unknown"}),
            ));
        };
        let candidate_concept_ids = match optional_string_array(&arguments, "candidate_concept_ids")
        {
            Ok(values) => values,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };

        let record = match self.engine.capture_learning_evidence(CaptureInput {
            session_id: optional_str(&arguments, "session").map(str::to_owned),
            source: source.to_owned(),
            content_type: content_type.to_owned(),
            text: text.to_owned(),
            learner_kind,
            candidate_concept_ids,
            note: optional_str(&arguments, "note").map(str::to_owned),
        }) {
            Ok(record) => record,
            Err(error) => return Ok(response(400, json!({"error": error.to_string()}))),
        };

        Ok(response(
            200,
            json!({
                "capture_id": record.capture_id,
                "evidence_id": record.evidence_id,
                "status": record.status.as_str(),
                "learner_kind": record.learner_kind.as_str(),
                "recorded_only": record.effect == CaptureEffect::RecordedOnly,
                "message": record.message,
            }),
        ))
    }

    fn inbox(&self) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        Ok(response(
            200,
            json!({ "items": self.engine.learner_inbox(&[], 20)? }),
        ))
    }

    fn ai_profile_update(&self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let input = AiInteractionProfileInput {
            persona: match optional_ai_profile_str(&arguments, "persona") {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
            verbosity: match optional_ai_profile_str(&arguments, "verbosity") {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
            explanation_depth: match optional_ai_profile_str(&arguments, "explanation_depth") {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
            proactivity: match optional_ai_profile_str(&arguments, "proactivity") {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
            intervention_frequency: match optional_ai_profile_str(
                &arguments,
                "intervention_frequency",
            ) {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
            correction_style: match optional_ai_profile_str(&arguments, "correction_style") {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
            custom_notes: match optional_ai_profile_str(&arguments, "custom_notes") {
                Ok(value) => value.map(str::to_owned),
                Err(error) => return Ok(response(400, json!({"error": error}))),
            },
        };
        match self.engine.update_ai_interaction_profile(input) {
            Ok(profile) => Ok(response(200, serde_json::to_value(profile)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }

    fn inbox_action(&self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(capture_id) = optional_str(&arguments, "capture_id") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: capture_id"}),
            ));
        };
        let Some(action_text) = optional_str(&arguments, "action") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: action"}),
            ));
        };
        let Some(action) = LearnerInboxAction::parse(action_text) else {
            return Ok(response(
                400,
                json!({"error": "action must be one of accept, defer, ignore, archive"}),
            ));
        };
        match self.engine.act_on_learner_inbox_item(
            capture_id,
            action,
            optional_str(&arguments, "note").map(str::to_owned),
        ) {
            Ok(receipt) => Ok(response(200, serde_json::to_value(receipt)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }

    fn inbox_practice(&self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(capture_id) = optional_str(&arguments, "capture_id") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: capture_id"}),
            ));
        };
        match self.engine.draft_inbox_practice(capture_id) {
            Ok(draft) => Ok(response(200, serde_json::to_value(draft)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
    }

    fn inbox_practice_submit(
        &mut self,
        body: &str,
    ) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let Some(capture_id) = optional_str(&arguments, "capture_id") else {
            return Ok(response(
                400,
                json!({"error": "missing string field: capture_id"}),
            ));
        };
        let session = optional_str(&arguments, "session").unwrap_or("http");
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

        let draft = match self.engine.draft_inbox_practice(capture_id) {
            Ok(draft) => draft,
            Err(error) => return Ok(response(400, json!({"error": error.to_string()}))),
        };
        let observation = crate::read_behavior_observation_now(
            self.engine.conn(),
            session,
            draft.concept_id.as_str(),
        )?;
        match self
            .engine
            .submit_inbox_practice(InboxPracticeSubmissionInput {
                capture_id: capture_id.to_owned(),
                session_id: session.to_owned(),
                response_text: response_text.to_owned(),
                self_confidence: confidence as i32,
                latency_ms: observation.latency_ms,
                hint_count: observation.hint_count,
            }) {
            Ok(receipt) => Ok(response(200, serde_json::to_value(receipt)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
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
        let no_attempt_reason = match optional_ai_profile_str(&arguments, "no_attempt_reason") {
            Ok(reason) => reason,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        let material_id = optional_str(&arguments, "material_id");
        let observation =
            crate::read_behavior_observation_now(self.engine.conn(), session, concept)?;
        let input = SubmitInput {
            session_id: session.to_owned(),
            concept_id: concept.to_owned(),
            task_type: task_type.to_owned(),
            prompt_text: prompt.to_owned(),
            response_text: response_text.to_owned(),
            self_confidence: confidence as i32,
            latency_ms: observation.latency_ms,
            hint_count: observation.hint_count,
        };

        if let Some(reason) = no_attempt_reason {
            return match self
                .engine
                .submit_no_attempt_with_material(input, reason, material_id)
            {
                Ok(receipt) => Ok(response(
                    200,
                    json!({
                        "attempt_id": receipt.attempt_id,
                        "provisional_score": null,
                        "degraded": false,
                        "no_attempt_reason": receipt.no_attempt_reason,
                    }),
                )),
                Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
            };
        }
        let receipt = self
            .engine
            .submit_provisional_with_material(input, material_id)?;

        Ok(response(
            200,
            json!({
                "attempt_id": receipt.attempt_id,
                "provisional_score": receipt.provisional_score,
                "degraded": receipt.degraded,
                "no_attempt_reason": null,
            }),
        ))
    }

    fn material_performance(&self, body: &str) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let arguments = match json_body(body) {
            Ok(arguments) => arguments,
            Err(error) => return Ok(response(400, json!({"error": error}))),
        };
        match self
            .engine
            .material_performance(optional_str(&arguments, "pack"))
        {
            Ok(report) => Ok(response(200, serde_json::to_value(report)?)),
            Err(error) => Ok(response(400, json!({"error": error.to_string()}))),
        }
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

fn parse_knowledge_map_query(query_string: Option<&str>) -> Result<KnowledgeMapQuery, String> {
    let mut query = KnowledgeMapQuery::default();
    let mut seen = BTreeSet::new();
    let Some(query_string) = query_string.filter(|value| !value.is_empty()) else {
        return Ok(query);
    };

    for pair in query_string.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode_query_component(raw_key)?;
        let value = percent_decode_query_component(raw_value)?;
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate query parameter: {key}"));
        }
        match key.as_str() {
            "scope" => {
                query.scope = match value.as_str() {
                    "pack" => KnowledgeMapScope::Pack,
                    "global" => KnowledgeMapScope::Global,
                    _ => return Err("scope must be one of pack, global".to_owned()),
                };
            }
            "pack" => query.pack = Some(value),
            "root" => query.root = Some(value),
            "depth" => {
                query.depth = Some(
                    value
                        .parse()
                        .map_err(|_| "depth must be an unsigned integer".to_owned())?,
                );
            }
            "phase" => query.phase = Some(value),
            "due" => {
                query.due = Some(match value.as_str() {
                    "new" => KnowledgeMapDueStatus::New,
                    "due" => KnowledgeMapDueStatus::Due,
                    "scheduled" => KnowledgeMapDueStatus::Scheduled,
                    "unscheduled" => KnowledgeMapDueStatus::Unscheduled,
                    _ => {
                        return Err("due must be one of new, due, scheduled, unscheduled".to_owned())
                    }
                });
            }
            "min_confidence" => {
                query.min_confidence = Some(
                    value
                        .parse()
                        .map_err(|_| "min_confidence must be a number".to_owned())?,
                );
            }
            "limit" => {
                query.limit = value
                    .parse()
                    .map_err(|_| "limit must be an unsigned integer".to_owned())?;
            }
            "cursor" => query.cursor = Some(value),
            _ => return Err(format!("unknown query parameter: {key}")),
        }
    }
    Ok(query)
}

fn parse_required_session_query(query_string: Option<&str>) -> Result<String, String> {
    let query_string = query_string.ok_or_else(|| "missing query parameter: session".to_owned())?;
    let mut session = None;
    for pair in query_string.split('&') {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| "invalid query parameter".to_owned())?;
        let key = percent_decode_query_component(key)?;
        if key != "session" {
            return Err(format!("unknown query parameter: {key}"));
        }
        if session.is_some() {
            return Err("duplicate query parameter: session".to_owned());
        }
        let value = percent_decode_query_component(value)?;
        if value.trim().is_empty() {
            return Err("session must not be empty".to_owned());
        }
        session = Some(value);
    }
    session.ok_or_else(|| "missing query parameter: session".to_owned())
}

fn percent_decode_query_component(value: &str) -> Result<String, String> {
    let input = value.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'+' => output.push(b' '),
            b'%' => {
                let Some(high) = input.get(index + 1).and_then(|byte| hex_value(*byte)) else {
                    return Err("invalid percent-encoding in query string".to_owned());
                };
                let Some(low) = input.get(index + 2).and_then(|byte| hex_value(*byte)) else {
                    return Err("invalid percent-encoding in query string".to_owned());
                };
                output.push((high << 4) | low);
                index += 2;
            }
            byte => output.push(byte),
        }
        index += 1;
    }
    String::from_utf8(output).map_err(|_| "query string must be valid UTF-8".to_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn optional_ai_profile_str<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    raw.as_str()
        .map(Some)
        .ok_or_else(|| format!("{key} must be a string"))
}

fn concept_id_argument(value: &Value) -> Option<&str> {
    optional_str(value, "concept_id").or_else(|| optional_str(value, "concept"))
}

fn optional_string_array(value: &Value, key: &str) -> Result<Vec<String>, String> {
    let Some(items) = value.get(key) else {
        return Ok(Vec::new());
    };
    let Some(items) = items.as_array() else {
        return Err(format!("{key} must be an array of strings"));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{key} must be an array of strings"))
        })
        .collect()
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
        403 => "Forbidden",
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
    use polaris_core::profile::ProfileSettingsUpdate;
    use rusqlite::Connection;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::*;

    const API_CONTRACT_DOC: &str = include_str!("../../../docs/API_CONTRACT.md");

    #[test]
    fn http_health_returns_service_metadata() {
        let mut api = test_api();

        let response = api.handle("GET", "/health", "").unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["service"], "polaris-core");
        assert_eq!(response.body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn http_global_profile_requires_opt_in_and_never_returns_raw_answers() {
        let mut api = test_api();

        let blocked = api.handle("GET", "/profile", "").unwrap();
        assert_eq!(blocked.status, 403);
        assert!(blocked.body["error"]
            .as_str()
            .unwrap()
            .contains("summary_sharing_enabled"));

        api.engine()
            .conn()
            .execute(
                "INSERT INTO behavior_events(id, session_id, at, type, payload_json)
                 VALUES ('private-answer', 's1', strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                         'profile_measurement', '{\"response\":4}')",
                [],
            )
            .unwrap();
        api.engine()
            .update_global_profile_settings(ProfileSettingsUpdate {
                acknowledge_disclosure: true,
                summary_sharing_enabled: Some(true),
                ..ProfileSettingsUpdate::default()
            })
            .unwrap();

        let shared = api.handle("GET", "/profile", "").unwrap();
        assert_eq!(shared.status, 200);
        assert!(shared.body["dimensions"].is_array());
        assert!(shared.body.get("measurements").is_none());
        assert!(!shared.body.to_string().contains("response"));

        let post = api.handle("POST", "/profile", "{}").unwrap();
        assert_eq!(post.status, 405);
    }

    #[test]
    fn http_contract_document_names_stable_routes_and_policy() {
        for required in [
            "# API 稳定性合约",
            "## HTTP v1",
            "GET /health",
            "GET /status",
            "GET /knowledge-map",
            "GET /prediction-map",
            "GET /profile",
            "GET /session",
            "GET /learner-mirror",
            "GET /trust",
            "GET /ai-profile",
            "POST /ai-profile",
            "POST /capture",
            "POST /next",
            "POST /evidence",
            "POST /materials/performance",
            "POST /feedback",
            "## 兼容性规则",
            "## 废弃策略",
        ] {
            assert!(
                API_CONTRACT_DOC.contains(required),
                "API contract missing section: {required}"
            );
        }
    }

    #[test]
    fn http_contract_routes_keep_stable_top_level_fields() {
        let mut api = test_api();

        let health = api.handle("GET", "/health", "").unwrap();
        assert_eq!(health.status, 200);
        assert_string_field(&health.body, "service");
        assert_string_field(&health.body, "version");

        let status = api.handle("GET", "/status", "").unwrap();
        assert_eq!(status.status, 200);
        assert_fields(
            &status.body,
            &[
                "current_pack",
                "theta_mode",
                "packs",
                "due_today",
                "phase_counts",
                "concepts",
            ],
        );

        let knowledge_map = api
            .handle("GET", "/knowledge-map?root=ownership&depth=0&limit=1", "")
            .unwrap();
        assert_eq!(knowledge_map.status, 200);
        assert_fields(
            &knowledge_map.body,
            &[
                "generated_at",
                "model_version",
                "query",
                "summary",
                "nodes",
                "edges",
                "next_cursor",
            ],
        );
        assert_eq!(knowledge_map.body["model_version"], "knowledge-map-v1");
        assert_eq!(knowledge_map.body["nodes"][0]["id"], "ownership");
        assert_fields(
            &knowledge_map.body["nodes"][0],
            &[
                "id",
                "kind",
                "pack",
                "retrieval",
                "p_known",
                "phase",
                "due_status",
                "uncertainty",
                "provenance",
            ],
        );

        let prediction_map = api
            .handle("GET", "/prediction-map?root=ownership&depth=0&limit=1", "")
            .unwrap();
        assert_eq!(prediction_map.status, 200);
        assert_fields(
            &prediction_map.body,
            &[
                "generated_at",
                "model_version",
                "query",
                "summary",
                "nodes",
                "anchors",
                "initial_paths",
                "next_cursor",
            ],
        );
        assert_eq!(prediction_map.body["model_version"], "prediction-map-v1");
        assert_eq!(prediction_map.body["nodes"][0]["id"], "ownership");
        assert_fields(
            &prediction_map.body["nodes"][0],
            &["observed", "latent_prediction", "inherited_prior"],
        );
        assert_eq!(
            prediction_map.body["nodes"][0]["latent_prediction"]["source"],
            "latent_prediction"
        );

        let mirror = api.handle("GET", "/learner-mirror", "").unwrap();
        assert_eq!(mirror.status, 200);
        assert_fields(
            &mirror.body,
            &[
                "generated_at",
                "confidence_curve",
                "phase_distribution",
                "recent_assertions",
            ],
        );

        let trust = api.handle("GET", "/trust", "").unwrap();
        assert_eq!(trust.status, 200);
        assert_fields(
            &trust.body,
            &[
                "gates",
                "active_breeding_experiments",
                "active_mrt_experiments",
                "recent_activity",
                "governance",
            ],
        );

        let capture = api
            .handle(
                "POST",
                "/capture",
                &json!({
                    "session": "contract",
                    "source": "paste",
                    "text": "I read a short note about ownership.",
                    "learner_kind": "reference"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(capture.status, 200);
        assert_string_field(&capture.body, "capture_id");
        assert_string_field(&capture.body, "evidence_id");
        assert_bool_field(&capture.body, "recorded_only");
        assert_string_field(&capture.body, "status");
        assert_string_field(&capture.body, "learner_kind");
        assert_string_field(&capture.body, "message");

        let next = api
            .handle("POST", "/next", &json!({"session": "contract"}).to_string())
            .unwrap();
        assert_eq!(next.status, 200);
        assert_fields(
            &next.body,
            &["task", "teaching_turn_id", "teaching_instruction"],
        );
        assert_fields(
            &next.body["task"],
            &["concept_id", "task_type", "prompt", "reason", "context"],
        );

        let evidence = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "contract",
                    "concept_id": next.body["task"]["concept_id"].as_str().unwrap(),
                    "response": "Ownership controls which binding can drop a value.",
                    "confidence": 4
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(evidence.status, 200);
        assert_string_field(&evidence.body, "attempt_id");
        assert_number_field(&evidence.body, "provisional_score");
        assert_bool_field(&evidence.body, "degraded");

        let feedback = api
            .handle(
                "POST",
                "/feedback",
                &json!({
                    "session": "contract",
                    "kind": "state",
                    "concept_id": "ownership",
                    "state": "flow"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(feedback.status, 200);
        assert_fields(&feedback.body, &["session_id", "kind", "effect"]);
    }

    #[test]
    fn http_contract_errors_keep_stable_shape() {
        let mut api = test_api();

        let not_found = api.handle("GET", "/missing", "").unwrap();
        assert_eq!(not_found.status, 404);
        assert_eq!(not_found.body["error"], "not found");

        let method_not_allowed = api.handle("GET", "/next", "").unwrap();
        assert_eq!(method_not_allowed.status, 405);
        assert_eq!(method_not_allowed.body["error"], "method not allowed");

        let capture_get = api.handle("GET", "/capture", "").unwrap();
        assert_eq!(capture_get.status, 405);
        assert_eq!(capture_get.body["error"], "method not allowed");

        let bad_request = api.handle("POST", "/evidence", "{").unwrap();
        assert_eq!(bad_request.status, 400);
        assert!(bad_request.body["error"]
            .as_str()
            .unwrap()
            .starts_with("invalid JSON"));

        let unknown_query = api
            .handle("GET", "/knowledge-map?surprise=true", "")
            .unwrap();
        assert_eq!(unknown_query.status, 400);
        assert_eq!(
            unknown_query.body["error"],
            "unknown query parameter: surprise"
        );

        let duplicate_query = api
            .handle("GET", "/knowledge-map?limit=1&limit=2", "")
            .unwrap();
        assert_eq!(duplicate_query.status, 400);

        let invalid_encoding = api.handle("GET", "/knowledge-map?root=%ZZ", "").unwrap();
        assert_eq!(invalid_encoding.status, 400);

        let knowledge_map_post = api.handle("POST", "/knowledge-map", "{}").unwrap();
        assert_eq!(knowledge_map_post.status, 405);

        let prediction_unknown = api
            .handle("GET", "/prediction-map?surprise=true", "")
            .unwrap();
        assert_eq!(prediction_unknown.status, 400);
        assert_eq!(
            prediction_unknown.body["error"],
            "unknown query parameter: surprise"
        );

        let prediction_map_post = api.handle("POST", "/prediction-map", "{}").unwrap();
        assert_eq!(prediction_map_post.status, 405);
    }

    #[test]
    fn http_session_summary_is_read_only_and_validates_query() {
        let mut api = test_api();
        api.engine()
            .conn()
            .execute(
                "INSERT INTO sessions(id, started_at, context_json) VALUES ('http-close', '2026-08-09T00:00:00Z', '{}')",
                [],
            )
            .unwrap();
        api.engine().close_session("http-close").unwrap();

        let found = api
            .handle("GET", "/session?session=http-close", "")
            .unwrap();
        assert_eq!(found.status, 200);
        assert_eq!(found.body["session_id"], "http-close");
        assert_fields(
            &found.body,
            &[
                "concepts_touched",
                "attempts_count",
                "assertions",
                "generated_at",
            ],
        );
        assert_eq!(api.handle("GET", "/session", "").unwrap().status, 400);
        assert_eq!(
            api.handle("GET", "/session?session=missing", "")
                .unwrap()
                .status,
            404
        );
        assert_eq!(api.handle("POST", "/session", "{}").unwrap().status, 405);
    }

    #[test]
    fn http_knowledge_map_supports_global_aggregate_and_validates_filters() {
        let mut api = test_api();

        let global = api
            .handle("GET", "/knowledge-map?scope=global&limit=10", "")
            .unwrap();
        assert_eq!(global.status, 200);
        assert_eq!(global.body["summary"]["scope"], "global");
        assert_eq!(global.body["nodes"], json!([]));
        assert!(global.body["summary"]["packs"].as_array().is_some());
        assert!(global.body["summary"]["dimensions"].as_array().is_some());

        let invalid = api.handle("GET", "/knowledge-map?depth=1", "").unwrap();
        assert_eq!(invalid.status, 400);
        assert!(invalid.body["error"].as_str().unwrap().contains("depth"));
    }

    #[test]
    fn http_status_includes_current_pack() {
        let mut api = test_api();

        let response = api.handle("GET", "/status", "").unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["current_pack"], "rust");
        assert_eq!(response.body["theta_mode"], "shared");
        assert_eq!(response.body["packs"][0]["id"], "rust");
        assert_eq!(response.body["packs"][0]["active"], true);
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
    fn http_trust_returns_stable_panel_shape() {
        let mut api = test_api();

        let response = api.handle("GET", "/trust", "").unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body["gates"].as_array().is_some());
        assert!(response.body["active_breeding_experiments"]
            .as_array()
            .is_some());
        assert!(response.body["active_mrt_experiments"].as_array().is_some());
        assert!(response.body["recent_activity"].as_object().is_some());
        assert!(response.body["governance"].as_object().is_some());
    }

    #[test]
    fn http_trust_rejects_non_get_methods_as_method_not_allowed() {
        let mut api = test_api();

        let response = api.handle("PUT", "/trust", "").unwrap();

        assert_eq!(response.status, 405);
        assert_eq!(response.body["error"], "method not allowed");
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
    fn http_capture_records_pending_item_without_attempt_or_mastery() {
        let mut api = test_api();

        let response = api
            .handle(
                "POST",
                "/capture",
                &json!({
                    "session": "http-capture",
                    "source": "paste",
                    "content_type": "text/plain",
                    "text": "Ownership means one binding is responsible for dropping a value.",
                    "learner_kind": "reference",
                    "candidate_concept_ids": ["ownership"],
                    "external_score": 1.0,
                    "final_score": 1.0
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["recorded_only"], true);
        assert_eq!(response.body["status"], "pending");
        assert_eq!(response.body["learner_kind"], "reference");
        assert!(response.body["capture_id"].as_str().is_some());
        assert!(response.body["evidence_id"].as_str().is_some());
        assert!(response.body["message"]
            .as_str()
            .unwrap()
            .contains("不会直接算作掌握"));

        let (queued_count, evidence_count, attempt_count, mastery_count, grade_queue_count): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = api
            .engine()
            .conn()
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM capture_queue WHERE status='pending'),
                    (SELECT COUNT(*) FROM evidence_items WHERE source='paste'),
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM mastery_states),
                    (SELECT COUNT(*) FROM grade_queue)",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(queued_count, 1);
        assert_eq!(evidence_count, 1);
        assert_eq!(attempt_count, 0);
        assert_eq!(mastery_count, 0);
        assert_eq!(grade_queue_count, 0);
    }

    #[test]
    fn p12d_http_inbox_lists_and_updates_capture_without_attempt_or_mastery() {
        let mut api = test_api();
        let capture = api
            .handle(
                "POST",
                "/capture",
                &json!({
                    "text": "Borrow checker note from an IDE chat.",
                    "source": "ai-ide",
                    "candidate_concept_ids": ["ownership"]
                })
                .to_string(),
            )
            .unwrap();
        let capture_id = capture.body["capture_id"].as_str().unwrap().to_owned();

        let inbox = api.handle("GET", "/inbox", "").unwrap();

        assert_eq!(inbox.status, 200);
        let items = inbox.body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["capture_id"], capture_id.as_str());
        assert_eq!(items[0]["status"], "pending");
        assert_eq!(items[0]["message"], "已保存，稍后帮你整理");
        assert_eq!(items[0]["actions"].as_array().unwrap().len(), 3);
        assert_eq!(items[0]["actions"][0]["action"], "accept");
        assert_eq!(items[0]["actions"][0]["label"], "转成一道小题");
        assert!(items[0].get("p_known").is_none());
        assert!(items[0].get("theta").is_none());
        assert!(items[0].get("candidate_concept_ids_json").is_none());

        let action = api
            .handle(
                "POST",
                "/inbox/action",
                &json!({
                    "capture_id": capture_id.as_str(),
                    "action": "accept"
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(action.status, 200);
        assert_eq!(action.body["capture_id"], capture_id.as_str());
        assert_eq!(action.body["status"], "practice_ready");
        assert_eq!(action.body["effect"], "recorded_only");
        assert!(action.body["message"].as_str().unwrap().contains("小题"));

        let (attempt_count, mastery_count, grade_queue_count): (i64, i64, i64) = api
            .engine()
            .conn()
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM mastery_states),
                    (SELECT COUNT(*) FROM grade_queue)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(attempt_count, 0);
        assert_eq!(mastery_count, 0);
        assert_eq!(grade_queue_count, 0);
    }

    #[test]
    fn p12e_http_inbox_practice_drafts_and_submits_without_trusting_external_score() {
        let mut api = test_api();
        let capture = api
            .handle(
                "POST",
                "/capture",
                &json!({
                    "text": "Ownership means one value has one owner at a time.",
                    "source": "ai-ide",
                    "candidate_concept_ids": ["ownership"]
                })
                .to_string(),
            )
            .unwrap();
        let capture_id = capture.body["capture_id"].as_str().unwrap().to_owned();
        api.handle(
            "POST",
            "/inbox/action",
            &json!({
                "capture_id": capture_id.as_str(),
                "action": "accept"
            })
            .to_string(),
        )
        .unwrap();

        let draft = api
            .handle(
                "POST",
                "/inbox/practice",
                &json!({"capture_id": capture_id.as_str()}).to_string(),
            )
            .unwrap();

        assert_eq!(draft.status, 200);
        assert_eq!(draft.body["capture_id"], capture_id.as_str());
        assert_eq!(draft.body["status"], "practice_ready");
        assert_eq!(draft.body["concept_hint"], "Ownership");
        assert_eq!(draft.body["task_type"], "explain");
        assert!(draft.body["prompt"]
            .as_str()
            .unwrap()
            .contains("用自己的话"));
        assert!(draft.body.get("p_known").is_none());
        assert!(draft.body.get("theta").is_none());

        let submit = api
            .handle(
                "POST",
                "/inbox/practice/submit",
                &json!({
                    "capture_id": capture_id.as_str(),
                    "session": "http-practice",
                    "response": "Ownership controls which binding can drop the value.",
                    "confidence": 4,
                    "external_score": 1.0,
                    "final_score": 1.0
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(submit.status, 200);
        assert_eq!(submit.body["capture_id"], capture_id.as_str());
        assert_eq!(submit.body["status"], "practiced");
        assert_eq!(submit.body["effect"], "submitted");
        assert!((submit.body["provisional_score"].as_f64().unwrap() - 0.70).abs() < 1e-9);

        let (attempt_count, mastery_count, grade_queue_count, final_score_count): (
            i64,
            i64,
            i64,
            i64,
        ) = api
            .engine()
            .conn()
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM mastery_states),
                    (SELECT COUNT(*) FROM grade_queue),
                    (SELECT COUNT(*) FROM attempts WHERE final_score IS NOT NULL)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(attempt_count, 1);
        assert_eq!(mastery_count, 1);
        assert_eq!(grade_queue_count, 1);
        assert_eq!(final_score_count, 0);
    }

    #[test]
    fn p13c_http_ai_profile_gets_and_updates_preferences() {
        let mut api = test_api();

        let default_profile = api.handle("GET", "/ai-profile", "").unwrap();
        assert_eq!(default_profile.status, 200);
        assert_eq!(default_profile.body["persona"], "balanced_mentor");
        assert_eq!(default_profile.body["verbosity"], "normal");
        assert!(default_profile.body["guidance"]
            .as_str()
            .unwrap()
            .contains("平衡"));

        let updated = api
            .handle(
                "POST",
                "/ai-profile",
                &json!({
                    "persona": "friendly_companion",
                    "verbosity": "detailed",
                    "explanation_depth": "deep",
                    "proactivity": "proactive",
                    "intervention_frequency": "high",
                    "correction_style": "supportive",
                    "custom_notes": "多解释，但别替我直接做题。"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(updated.status, 200);
        assert_eq!(updated.body["persona"], "friendly_companion");
        assert_eq!(updated.body["verbosity"], "detailed");
        assert_eq!(updated.body["custom_notes"], "多解释，但别替我直接做题。");
        assert!(updated.body["guidance"].as_str().unwrap().contains("陪伴"));

        let invalid_type = api
            .handle(
                "POST",
                "/ai-profile",
                &json!({"verbosity": 123}).to_string(),
            )
            .unwrap();
        assert_eq!(invalid_type.status, 400);
        assert!(invalid_type.body["error"]
            .as_str()
            .unwrap()
            .contains("verbosity must be a string"));

        let invalid = api
            .handle(
                "POST",
                "/ai-profile",
                &json!({"verbosity": "endless"}).to_string(),
            )
            .unwrap();
        assert_eq!(invalid.status, 400);

        let persisted = api.handle("GET", "/ai-profile", "").unwrap();
        assert_eq!(persisted.body["verbosity"], "detailed");

        let cleared = api
            .handle(
                "POST",
                "/ai-profile",
                &json!({"custom_notes": ""}).to_string(),
            )
            .unwrap();
        assert_eq!(cleared.status, 200);
        assert_eq!(cleared.body["custom_notes"], Value::Null);
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
        assert!(response.body["teaching_turn_id"].as_str().is_some());
        assert_eq!(response.body["task"]["context"], Value::Null);
        assert_eq!(
            response.body["teaching_instruction"]["context"],
            Value::Null
        );
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
    fn p16j_http_records_explanation_without_creating_learner_attempt() {
        let mut api = test_api();
        let next = api
            .handle(
                "POST",
                "/next",
                &json!({"session": "http-teaching"}).to_string(),
            )
            .unwrap();
        let teaching_turn_id = next.body["teaching_turn_id"].as_str().unwrap();

        let response = api
            .handle(
                "POST",
                "/teaching-turn/explanation",
                &json!({
                    "teaching_turn_id": teaching_turn_id,
                    "text": "A tutor explanation, not a learner answer."
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["teaching_turn_id"], teaching_turn_id);
        assert!(response.body["evidence_id"].as_str().is_some());
        let counts: (i64, i64) = api
            .engine()
            .conn()
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM evidence_items WHERE source='teaching_explanation')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 1));
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
    fn p16i_http_evidence_records_explicit_no_attempt_without_mastery() {
        let mut api = test_api();
        let response = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "http-no-attempt",
                    "concept_id": "ownership",
                    "response": "",
                    "confidence": 1,
                    "no_attempt_reason": "not_understood_prompt"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body["provisional_score"], Value::Null);
        assert_eq!(response.body["no_attempt_reason"], "not_understood_prompt");
        let mastery: i64 = api
            .engine()
            .conn()
            .query_row("SELECT COUNT(*) FROM mastery_states", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mastery, 0);

        let attempts_before: i64 = api
            .engine()
            .conn()
            .query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))
            .unwrap();
        let invalid = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "http-invalid",
                    "concept_id": "ownership",
                    "response": "",
                    "confidence": 1,
                    "no_attempt_reason": "guessed"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(invalid.status, 400);
        let attempts_after: i64 = api
            .engine()
            .conn()
            .query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(attempts_after, attempts_before);
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

    #[test]
    fn p16l_http_accepts_material_id_and_returns_read_only_summary() {
        let mut api = test_api();
        api.engine()
            .conn()
            .execute_batch(
                r#"
                INSERT INTO materials(id, pack, kind, level, title, source_ref, created_at)
                VALUES ('rust-book-ownership', 'rust', 'chapter', 'intro', 'Ownership',
                        'book://rust/ownership', '2026-01-01T00:00:00Z');
                INSERT OR REPLACE INTO meta(key, value)
                VALUES ('pack.rust.material_levels', '["intro"]');
                "#,
            )
            .unwrap();
        let submit = api
            .handle(
                "POST",
                "/evidence",
                &json!({
                    "session": "http-material",
                    "concept_id": "ownership",
                    "response": "Ownership controls the responsible binding.",
                    "confidence": 4,
                    "material_id": "rust-book-ownership"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(submit.status, 200);
        let stored: String = api
            .engine()
            .conn()
            .query_row(
                "SELECT material_id FROM attempts WHERE id=?1",
                [submit.body["attempt_id"].as_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, "rust-book-ownership");
        let summary = api
            .handle(
                "POST",
                "/materials/performance",
                &json!({"pack": "rust"}).to_string(),
            )
            .unwrap();
        assert_eq!(summary.status, 200);
        assert_eq!(summary.body["by_level"][0]["level"], "intro");
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

    fn assert_fields(value: &Value, fields: &[&str]) {
        let object = value.as_object().expect("expected JSON object");
        for field in fields {
            assert!(
                object.contains_key(*field),
                "missing field {field} in {value}"
            );
        }
    }

    fn assert_string_field(value: &Value, field: &str) {
        assert!(
            value[field].as_str().is_some(),
            "expected string field {field} in {value}"
        );
    }

    fn assert_number_field(value: &Value, field: &str) {
        assert!(
            value[field].as_f64().is_some(),
            "expected number field {field} in {value}"
        );
    }

    fn assert_bool_field(value: &Value, field: &str) {
        assert!(
            value[field].as_bool().is_some(),
            "expected bool field {field} in {value}"
        );
    }
}
