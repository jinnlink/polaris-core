use std::io::{self, BufRead, Write};

use polaris_core::engine::{Engine, SubmitInput};
use serde_json::{json, Value};

pub struct McpSession {
    engine: Engine,
}

impl McpSession {
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }

    #[cfg(test)]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn handle_request(
        &mut self,
        message: Value,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(Some(error_response(id, -32600, "missing method")));
        };
        if method.starts_with("notifications/") {
            return Ok(None);
        }

        let params = message.get("params").unwrap_or(&Value::Null);
        let result = match method {
            "initialize" => initialize_result(),
            "tools/list" => json!({ "tools": tool_definitions() }),
            "tools/call" => self.call_tool(params),
            "resources/list" => json!({ "resources": resource_definitions() }),
            "resources/templates/list" => {
                json!({ "resourceTemplates": resource_template_definitions() })
            }
            "resources/read" => self.read_resource(params),
            _ => return Ok(Some(error_response(id, -32601, "method not found"))),
        };

        Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        })))
    }

    fn call_tool(&mut self, params: &Value) -> Value {
        let outcome = (|| {
            let name = required_str(params, "name")?;
            let arguments = params.get("arguments").unwrap_or(&Value::Null);
            match name {
                "get_next_task" => self.get_next_task(arguments),
                "get_interleaved_batch" => self.get_interleaved_batch(arguments),
                "submit_evidence" => self.submit_evidence(arguments),
                "get_teaching_instruction" => self.get_teaching_instruction(arguments),
                other => Err(format!("unknown tool: {other}")),
            }
        })();

        match outcome {
            Ok(payload) => tool_text_result(payload),
            Err(error) => json!({
                "content": [{"type": "text", "text": error}],
                "isError": true,
            }),
        }
    }

    fn read_resource(&self, params: &Value) -> Value {
        let outcome = (|| {
            let uri = required_str(params, "uri")?;
            if uri == "polaris://status" {
                let snapshot = self
                    .engine
                    .status_snapshot()
                    .map_err(|error| error.to_string())?;
                return serde_json::to_string_pretty(&snapshot)
                    .map_err(|error| error.to_string())
                    .map(|text| resource_text(uri, text));
            }
            if let Some(concept_id) = uri
                .strip_prefix("polaris://concept/")
                .and_then(|rest| rest.strip_suffix("/diagnosis"))
            {
                let diagnosis = self
                    .engine
                    .diagnose_concept(concept_id)
                    .map_err(|error| error.to_string())?;
                return serde_json::to_string_pretty(&diagnosis)
                    .map_err(|error| error.to_string())
                    .map(|text| resource_text(uri, text));
            }

            Err(format!("unknown resource: {uri}"))
        })();

        match outcome {
            Ok(contents) => json!({ "contents": [contents] }),
            Err(error) => json!({
                "contents": [{"uri": "polaris://error", "mimeType": "text/plain", "text": error}],
                "isError": true,
            }),
        }
    }

    fn get_next_task(&mut self, arguments: &Value) -> Result<Value, String> {
        let session = optional_str(arguments, "session").unwrap_or("mcp");
        let Some(task) = self.engine.next_task().map_err(|error| error.to_string())? else {
            return Ok(json!({ "task": null }));
        };

        self.engine
            .record_next_task_event(session, &task)
            .map_err(|error| error.to_string())?;

        let instruction = self
            .engine
            .teaching_instruction(&task.concept_id)
            .map_err(|error| error.to_string())?;
        Ok(json!({
            "task": {
                "concept_id": task.concept_id,
                "task_type": task.task_type,
                "prompt": task.prompt_text,
                "reason": task.reason,
            },
            "teaching_instruction": instruction,
        }))
    }

    fn submit_evidence(&mut self, arguments: &Value) -> Result<Value, String> {
        let session = required_str(arguments, "session")?.to_owned();
        let concept = concept_id_argument(arguments)?.to_owned();
        let response = required_str(arguments, "response")?.to_owned();
        let confidence = required_i64(arguments, "confidence")?;
        if !(1..=5).contains(&confidence) {
            return Err("confidence must be in 1..=5".to_owned());
        }
        let task_type = optional_str(arguments, "task_type")
            .unwrap_or("recall")
            .to_owned();
        let prompt = optional_str(arguments, "prompt").unwrap_or("").to_owned();
        let observation = crate::read_behavior_observation_now(
            self.engine.conn(),
            session.as_str(),
            concept.as_str(),
        )
        .map_err(|error| error.to_string())?;
        let receipt = self
            .engine
            .submit(SubmitInput {
                session_id: session,
                concept_id: concept,
                task_type,
                prompt_text: prompt,
                response_text: response,
                self_confidence: confidence as i32,
                latency_ms: observation.latency_ms,
                hint_count: observation.hint_count,
            })
            .map_err(|error| error.to_string())?;

        Ok(json!({
            "attempt_id": receipt.attempt_id,
            "provisional_score": receipt.provisional_score,
            "degraded": receipt.degraded,
        }))
    }

    fn get_interleaved_batch(&self, arguments: &Value) -> Result<Value, String> {
        let batch_size = optional_i64(arguments, "batch_size").unwrap_or(3).max(1) as usize;
        serde_json::to_value(
            self.engine
                .get_interleaved_batch(batch_size)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn get_teaching_instruction(&self, arguments: &Value) -> Result<Value, String> {
        let concept = required_str(arguments, "concept")?;
        serde_json::to_value(
            self.engine
                .teaching_instruction(concept)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }
}

pub fn serve_stdio(engine: Engine) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = McpSession::new(engine);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut writer = io::BufWriter::new(stdout.lock());

    while let Some(message) = read_message(&mut reader)? {
        if let Some(response) = session.handle_request(message)? {
            write_message(&mut writer, &response)?;
            writer.flush()?;
        }
    }

    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {},
        },
        "serverInfo": {
            "name": "polaris-core",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "get_next_task",
            "description": "Return the locally scheduled next learning task and its Tier 2 teaching instruction. Records a next behavior event for latency accounting.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": {"type": "string", "description": "Session id to associate with the next behavior event. Defaults to mcp."}
                }
            }
        },
        {
            "name": "get_interleaved_batch",
            "description": "Return a mini-batch of interleaved learning tasks with move, phase, p_known, and expected_success fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "batch_size": {"type": "integer", "minimum": 1, "description": "Mini-batch size. Defaults to 3."}
                }
            }
        },
        {
            "name": "submit_evidence",
            "description": "Submit learner evidence for engine-owned scoring and optimistic mastery update. External AI judgement is not accepted as mastery state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": {"type": "string", "description": "Session id."},
                    "concept_id": {"type": "string", "description": "Target concept id returned by get_next_task."},
                    "concept": {"type": "string", "description": "Deprecated alias for concept_id."},
                    "response": {"type": "string", "description": "Learner response text to store as evidence."},
                    "confidence": {"type": "integer", "minimum": 1, "maximum": 5, "description": "Learner self-confidence collected before feedback."},
                    "task_type": {"type": "string", "description": "Task type. Defaults to recall."},
                    "prompt": {"type": "string", "description": "Prompt shown to the learner. Defaults to empty."}
                },
                "required": ["session", "response", "confidence"],
                "anyOf": [
                    {"required": ["concept_id"]},
                    {"required": ["concept"]}
                ]
            }
        },
        {
            "name": "get_teaching_instruction",
            "description": "Return Tier 2 teaching guidance for a concept with focus, move, target_depth, do, dont, and anchor fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "concept": {"type": "string", "description": "Concept id to diagnose and teach."}
                },
                "required": ["concept"]
            }
        }
    ])
}

fn resource_definitions() -> Value {
    json!([
        {
            "uri": "polaris://status",
            "name": "Polaris status",
            "description": "Read-only status snapshot with due_today and concept states.",
            "mimeType": "application/json"
        }
    ])
}

fn resource_template_definitions() -> Value {
    json!([
        {
            "uriTemplate": "polaris://concept/{id}/diagnosis",
            "name": "Concept diagnosis",
            "description": "Read-only graph-aware diagnosis for a concept id.",
            "mimeType": "application/json"
        }
    ])
}

fn resource_text(uri: &str, text: String) -> Value {
    json!({
        "uri": uri,
        "mimeType": "application/json",
        "text": text,
    })
}

fn tool_text_result(payload: Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&payload).expect("tool payload is serializable"),
        }]
    })
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string argument: {key}"))
}

fn concept_id_argument(value: &Value) -> Result<&str, String> {
    value
        .get("concept_id")
        .and_then(Value::as_str)
        .or_else(|| value.get("concept").and_then(Value::as_str))
        .ok_or_else(|| "missing string argument: concept_id".to_owned())
}

fn optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn required_i64(value: &Value, key: &str) -> Result<i64, String> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing integer argument: {key}"))
}

fn optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

fn read_message<R: BufRead>(reader: &mut R) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = Some(value.trim().parse::<usize>()?);
            }
        }
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn write_message<W: Write>(
    writer: &mut W,
    message: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use polaris_core::db::migrate;
    use polaris_core::engine::Engine;
    use rusqlite::Connection;
    use serde_json::{json, Value};
    use std::io::Cursor;

    use super::*;

    #[test]
    fn mcp_lists_polaris_tools_and_status_resource() {
        let mut session = test_session();

        let tools = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
            .unwrap()
            .unwrap();
        let tool_names = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            tool_names,
            vec![
                "get_next_task",
                "get_interleaved_batch",
                "submit_evidence",
                "get_teaching_instruction"
            ]
        );

        let resources = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list"}))
            .unwrap()
            .unwrap();
        let resource_uris = resources["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|resource| resource["uri"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(resource_uris.contains(&"polaris://status"));

        let templates = session
            .handle_request(
                json!({"jsonrpc": "2.0", "id": 20, "method": "resources/templates/list"}),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            templates["result"]["resourceTemplates"][0]["uriTemplate"],
            "polaris://concept/{id}/diagnosis"
        );
    }

    #[test]
    fn mcp_reads_status_resource() {
        let mut session = test_session();

        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "resources/read",
                "params": {"uri": "polaris://status"}
            }))
            .unwrap()
            .unwrap();
        let text = response["result"]["contents"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(text).unwrap();

        assert_eq!(payload["due_today"], 0);
        assert_eq!(payload["concepts"][0]["concept_id"], "ownership");
        assert_eq!(payload["concepts"][0]["phase"], "undetermined");
    }

    #[test]
    fn mcp_submit_evidence_records_attempt() {
        let mut session = test_session();

        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {"name": "submit_evidence", "arguments": {
                    "session": "mcp-test",
                    "concept": "ownership",
                    "response": "Ownership controls which binding can drop a value.",
                    "confidence": 4
                }}
            }))
            .unwrap()
            .unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(text).unwrap();

        assert!(
            (payload["provisional_score"].as_f64().unwrap() - 0.70).abs() < 1e-9,
            "unexpected provisional score: {}",
            payload["provisional_score"]
        );
        assert!(payload["attempt_id"].as_str().unwrap().len() > 10);

        let attempt_count: i64 = session
            .engine()
            .conn()
            .query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(attempt_count, 1);
    }

    #[test]
    fn mcp_next_task_records_event_and_submit_accepts_returned_concept_id() {
        let mut session = test_session();

        let next_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 30,
                "method": "tools/call",
                "params": {"name": "get_next_task", "arguments": {
                    "session": "mcp-flow"
                }}
            }))
            .unwrap()
            .unwrap();
        let next_text = next_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let next_payload: Value = serde_json::from_str(next_text).unwrap();
        let concept_id = next_payload["task"]["concept_id"].as_str().unwrap();
        assert_eq!(next_payload["teaching_instruction"]["move"], "recall");
        assert_eq!(
            next_payload["teaching_instruction"]["target_depth"],
            "recall"
        );

        let next_events: i64 = session
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events
                 WHERE session_id='mcp-flow' AND concept_id=?1 AND type='next'",
                [concept_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_events, 1);

        let submit_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 31,
                "method": "tools/call",
                "params": {"name": "submit_evidence", "arguments": {
                    "session": "mcp-flow",
                    "concept_id": concept_id,
                    "response": "Ownership controls which binding can drop a value.",
                    "confidence": 4
                }}
            }))
            .unwrap()
            .unwrap();
        let submit_text = submit_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let submit_payload: Value = serde_json::from_str(submit_text).unwrap();

        assert!(
            submit_payload["attempt_id"].as_str().is_some(),
            "submit response should accept concept_id and return attempt_id, got {submit_payload}"
        );
    }

    #[test]
    fn mcp_interleaved_batch_returns_assignment_fields() {
        let mut session = test_session();

        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 32,
                "method": "tools/call",
                "params": {"name": "get_interleaved_batch", "arguments": {
                    "batch_size": 3
                }}
            }))
            .unwrap()
            .unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(text).unwrap();
        let batch = payload.as_array().expect("batch array");

        assert_eq!(batch.len(), 3);
        for item in batch {
            assert!(item["concept_id"].as_str().is_some());
            assert!(item["concept_name"].as_str().is_some());
            assert!(item["move"].as_str().is_some());
            assert!(item["task_type"].as_str().is_some());
            assert!(item["template"].as_str().is_some());
            assert!(item["phase"].as_str().is_some());
            assert!(item["p_known"].as_f64().is_some());
            assert!(item["expected_success"].as_f64().is_some());
        }
    }

    #[test]
    fn mcp_teaching_instruction_contains_tier2_guardrails() {
        let mut session = test_session();

        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {"name": "get_teaching_instruction", "arguments": {
                    "concept": "ownership"
                }}
            }))
            .unwrap()
            .unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(text).unwrap();

        assert_eq!(payload["target"], "ownership");
        assert!(payload["dont"]
            .as_str()
            .unwrap()
            .contains("不要直接改掌握度"));
        assert!(payload["do"].as_str().unwrap().contains("先让学习者作答"));
    }

    #[test]
    fn mcp_stdio_frame_round_trips_json_rpc() {
        let message = json!({"jsonrpc": "2.0", "id": 6, "method": "tools/list"});
        let mut bytes = Vec::new();

        write_message(&mut bytes, &message).unwrap();
        let mut cursor = Cursor::new(bytes);
        let decoded = read_message(&mut cursor).unwrap().unwrap();

        assert_eq!(decoded, message);
    }

    fn test_session() -> McpSession {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
        McpSession::new(engine)
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }
}
