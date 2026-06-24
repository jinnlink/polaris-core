use std::io::{self, BufRead, Write};

use polaris_core::capture_queue::{CaptureEffect, CaptureInput, CaptureStatus, LearnerCaptureKind};
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::inbox_practice::InboxPracticeSubmissionInput;
use polaris_core::learner_feedback::LearnerFeedbackInput;
use polaris_core::learner_inbox::LearnerInboxAction;
use polaris_core::project_manifest::discover_project_manifest;
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
                "get_phase_snapshot" => self.get_phase_snapshot(),
                "get_active_gu_rules" => self.get_active_gu_rules(arguments),
                "run_mirror_report" => self.run_mirror_report(arguments),
                "get_latest_mirror_report" => self.get_latest_mirror_report(),
                "mark_report_assertion_inaccurate" => {
                    self.mark_report_assertion_inaccurate(arguments)
                }
                "mark_report_assertion_accurate" => self.mark_report_assertion_accurate(arguments),
                "record_learner_feedback" => self.record_learner_feedback(arguments),
                "get_trust_panel" => self.get_trust_panel(),
                "detect_project_manifest" => self.detect_project_manifest(arguments),
                "capture_evidence" => self.capture_evidence(arguments),
                "list_learner_inbox" => self.list_learner_inbox(arguments),
                "act_on_learner_inbox_item" => self.act_on_learner_inbox_item(arguments),
                "draft_inbox_practice" => self.draft_inbox_practice(arguments),
                "submit_inbox_practice" => self.submit_inbox_practice(arguments),
                "get_learner_mirror" => self.get_learner_mirror(),
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
            if uri == "polaris://trust" {
                let panel = self
                    .engine
                    .trust_panel()
                    .map_err(|error| error.to_string())?;
                return serde_json::to_string_pretty(&panel)
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

    fn get_phase_snapshot(&self) -> Result<Value, String> {
        serde_json::to_value(
            self.engine
                .status_snapshot()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn get_active_gu_rules(&self, arguments: &Value) -> Result<Value, String> {
        let concept = concept_id_argument(arguments)?;
        serde_json::to_value(
            self.engine
                .active_gu_rules_for_concept(concept)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn run_mirror_report(&self, arguments: &Value) -> Result<Value, String> {
        let narrative = optional_bool(arguments, "narrative").unwrap_or(false);
        let report = if narrative {
            self.engine
                .run_mirror_report_with_narrative()
                .map_err(|error| error.to_string())?
        } else {
            self.engine
                .run_mirror_report()
                .map_err(|error| error.to_string())?
        };
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn get_latest_mirror_report(&self) -> Result<Value, String> {
        Ok(json!({
            "report": self.engine.latest_mirror_report().map_err(|error| error.to_string())?
        }))
    }

    fn get_trust_panel(&self) -> Result<Value, String> {
        let panel = self
            .engine
            .trust_panel()
            .map_err(|error| error.to_string())?;
        serde_json::to_value(panel).map_err(|error| error.to_string())
    }

    fn detect_project_manifest(&self, arguments: &Value) -> Result<Value, String> {
        let path = optional_str(arguments, "path").unwrap_or(".");
        let Some(discovered) =
            discover_project_manifest(path).map_err(|error| error.to_string())?
        else {
            return Ok(json!({
                "found": false,
                "path": path,
            }));
        };

        Ok(json!({
            "found": true,
            "project_root": discovered.project_root.display().to_string(),
            "manifest_path": discovered.manifest_path.display().to_string(),
            "manifest": discovered.manifest,
        }))
    }

    fn capture_evidence(&self, arguments: &Value) -> Result<Value, String> {
        let text = required_str(arguments, "text")?;
        let source = optional_str(arguments, "source").unwrap_or("paste");
        let content_type = optional_str(arguments, "content_type").unwrap_or("text/plain");
        let learner_kind_text = optional_str(arguments, "learner_kind").unwrap_or("reference");
        let learner_kind = LearnerCaptureKind::parse(learner_kind_text).ok_or_else(|| {
            "learner_kind must be one of reference, own_answer, error_log, code_change, chat_excerpt, unknown".to_owned()
        })?;
        let candidate_concept_ids = optional_string_array(arguments, "candidate_concept_ids")?;

        let record = self
            .engine
            .capture_learning_evidence(CaptureInput {
                session_id: optional_str(arguments, "session").map(str::to_owned),
                source: source.to_owned(),
                content_type: content_type.to_owned(),
                text: text.to_owned(),
                learner_kind,
                candidate_concept_ids,
                note: optional_str(arguments, "note").map(str::to_owned),
            })
            .map_err(|error| error.to_string())?;

        Ok(json!({
            "capture_id": record.capture_id,
            "evidence_id": record.evidence_id,
            "status": record.status.as_str(),
            "learner_kind": record.learner_kind.as_str(),
            "recorded_only": record.effect == CaptureEffect::RecordedOnly,
            "message": record.message,
        }))
    }

    fn list_learner_inbox(&self, arguments: &Value) -> Result<Value, String> {
        let statuses = parse_capture_statuses(optional_string_array(arguments, "statuses")?)?;
        let limit = optional_i64(arguments, "limit").unwrap_or(20).max(1) as usize;
        let items = self
            .engine
            .learner_inbox(&statuses, limit)
            .map_err(|error| error.to_string())?;
        Ok(json!({ "items": items }))
    }

    fn act_on_learner_inbox_item(&self, arguments: &Value) -> Result<Value, String> {
        let capture_id = required_str(arguments, "capture_id")?;
        let action_text = required_str(arguments, "action")?;
        let action = LearnerInboxAction::parse(action_text)
            .ok_or_else(|| "action must be one of accept, defer, ignore, archive".to_owned())?;
        serde_json::to_value(
            self.engine
                .act_on_learner_inbox_item(
                    capture_id,
                    action,
                    optional_str(arguments, "note").map(str::to_owned),
                )
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn draft_inbox_practice(&self, arguments: &Value) -> Result<Value, String> {
        let capture_id = required_str(arguments, "capture_id")?;
        serde_json::to_value(
            self.engine
                .draft_inbox_practice(capture_id)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn submit_inbox_practice(&mut self, arguments: &Value) -> Result<Value, String> {
        let capture_id = required_str(arguments, "capture_id")?;
        let session = optional_str(arguments, "session").unwrap_or("mcp");
        let response = required_str(arguments, "response")?;
        let confidence = required_i64(arguments, "confidence")?;
        if !(1..=5).contains(&confidence) {
            return Err("confidence must be in 1..=5".to_owned());
        }
        let draft = self
            .engine
            .draft_inbox_practice(capture_id)
            .map_err(|error| error.to_string())?;
        let observation = crate::read_behavior_observation_now(
            self.engine.conn(),
            session,
            draft.concept_id.as_str(),
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(
            self.engine
                .submit_inbox_practice(InboxPracticeSubmissionInput {
                    capture_id: capture_id.to_owned(),
                    session_id: session.to_owned(),
                    response_text: response.to_owned(),
                    self_confidence: confidence as i32,
                    latency_ms: observation.latency_ms,
                    hint_count: observation.hint_count,
                })
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn get_learner_mirror(&self) -> Result<Value, String> {
        serde_json::to_value(
            self.engine
                .learner_mirror_snapshot()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn mark_report_assertion_inaccurate(&self, arguments: &Value) -> Result<Value, String> {
        self.mark_report_assertion(arguments, "inaccurate")
    }

    fn mark_report_assertion_accurate(&self, arguments: &Value) -> Result<Value, String> {
        self.mark_report_assertion(arguments, "accurate")
    }

    fn mark_report_assertion(&self, arguments: &Value, verdict: &str) -> Result<Value, String> {
        let assertion_id = required_str(arguments, "assertion_id")?;
        let report_id = optional_str(arguments, "report_id");
        let marked_report_id = self
            .engine
            .record_report_feedback_with_verdict(report_id, assertion_id, verdict)
            .map_err(|error| error.to_string())?;
        Ok(json!({
            "report_id": marked_report_id,
            "assertion_id": assertion_id,
            "verdict": verdict,
            "effect": "recorded_only",
        }))
    }

    fn record_learner_feedback(&self, arguments: &Value) -> Result<Value, String> {
        let kind = required_str(arguments, "kind")?;
        let session = optional_str(arguments, "session").unwrap_or("mcp");
        let receipt = self
            .engine
            .record_learner_feedback(LearnerFeedbackInput {
                session_id: session.to_owned(),
                source: "mcp".to_owned(),
                kind: kind.to_owned(),
                concept_id: arguments
                    .get("concept_id")
                    .and_then(Value::as_str)
                    .or_else(|| arguments.get("concept").and_then(Value::as_str))
                    .map(str::to_owned),
                state: optional_str(arguments, "state").map(str::to_owned),
                reason: optional_str(arguments, "reason").map(str::to_owned),
                note: optional_str(arguments, "note").map(str::to_owned),
            })
            .map_err(|error| error.to_string())?;
        serde_json::to_value(receipt).map_err(|error| error.to_string())
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
            "name": "get_phase_snapshot",
            "description": "Return the current knowledge phase snapshot with phase_counts and concept phase states for Tier 2 diagnosis.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "get_active_gu_rules",
            "description": "Return validated or active G_u rules for a concept so the external tutor can check recurring error patterns without inventing traits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "concept_id": {"type": "string", "description": "Target concept id."},
                    "concept": {"type": "string", "description": "Deprecated alias for concept_id."}
                },
                "anyOf": [
                    {"required": ["concept_id"]},
                    {"required": ["concept"]}
                ]
            }
        },
        {
            "name": "run_mirror_report",
            "description": "Generate and persist an evidence-bound mirror report using existing engine rules. Set narrative=true to request optional Tier 1 prose polishing; failure degrades to the raw assertion list.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "narrative": {"type": "boolean", "description": "When true, request Tier 1 narrative polish with strict citations to report item claims. Defaults to false."}
                }
            }
        },
        {
            "name": "get_latest_mirror_report",
            "description": "Return the latest persisted mirror report, or null when no report has been generated.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "mark_report_assertion_inaccurate",
            "description": "Record learner feedback that a mirror report assertion is inaccurate. The feedback becomes calibration data; it does not rewrite mastery state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report_id": {"type": "string", "description": "Optional report id. Defaults to the latest report."},
                    "assertion_id": {"type": "string", "description": "Assertion, hypothesis, or suggestion id to mark inaccurate."}
                },
                "required": ["assertion_id"]
            }
        },
        {
            "name": "mark_report_assertion_accurate",
            "description": "Record learner feedback that a mirror report assertion is accurate. The feedback is recorded only; it does not rewrite mastery state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report_id": {"type": "string", "description": "Optional report id. Defaults to the latest report."},
                    "assertion_id": {"type": "string", "description": "Assertion, hypothesis, or suggestion id to mark accurate."}
                },
                "required": ["assertion_id"]
            }
        },
        {
            "name": "record_learner_feedback",
            "description": "Record learner state or pause feedback as an auditable behavior event. This is recorded only and does not directly change mastery, HMM state, or scheduling.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": {"type": "string", "description": "Session id. Defaults to mcp."},
                    "kind": {"type": "string", "enum": ["state", "pause"], "description": "Feedback kind."},
                    "concept_id": {"type": "string", "description": "Optional related concept id."},
                    "concept": {"type": "string", "description": "Deprecated alias for concept_id."},
                    "state": {"type": "string", "enum": ["flow", "productive_confusion", "frustrated", "bored", "anxious", "fatigued"], "description": "Required when kind=state."},
                    "reason": {"type": "string", "description": "Required when kind=pause. Free-text learner reason, trimmed before storage."},
                    "note": {"type": "string", "description": "Optional learner note."}
                },
                "required": ["kind"]
            }
        },
        {
            "name": "get_trust_panel",
            "description": "Return the read-only trust panel with F1-F5 gate status, active breeding and MRT experiments, recent activity, and governance thresholds.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "detect_project_manifest",
            "description": "Discover a p-os.toml learning project manifest by walking upward from a path. Returns found=false when no manifest exists.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File or directory path to start discovery from. Defaults to the MCP server current directory."}
                }
            }
        },
        {
            "name": "capture_evidence",
            "description": "Record external learning material into evidence_items and capture_queue as pending raw capture. This is recorded only and never changes mastery or creates attempts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Learning material, note, answer, error log, code excerpt, or chat excerpt to store."},
                    "session": {"type": "string", "description": "Optional session id to attach to the evidence."},
                    "source": {"type": "string", "description": "Evidence source. Defaults to paste."},
                    "content_type": {"type": "string", "description": "Content type. Defaults to text/plain."},
                    "learner_kind": {"type": "string", "enum": ["reference", "own_answer", "error_log", "code_change", "chat_excerpt", "unknown"], "description": "Kind of captured material. Defaults to reference."},
                    "candidate_concept_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional candidate concept ids. Stored as hints only; no mastery fold is triggered."
                    },
                    "note": {"type": "string", "description": "Optional learner or AI IDE note."}
                },
                "required": ["text"]
            }
        },
        {
            "name": "list_learner_inbox",
            "description": "List learner-facing capture inbox items in pending, mapped, or practice_ready states. Returns student-readable messages and up to three actions per item; does not expose mastery parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "statuses": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["pending", "mapped", "practice_ready", "practiced", "ignored", "archived"]},
                        "description": "Optional status filter. Defaults to pending, mapped, and practice_ready."
                    },
                    "limit": {"type": "integer", "minimum": 1, "description": "Maximum number of items. Defaults to 20."}
                }
            }
        },
        {
            "name": "act_on_learner_inbox_item",
            "description": "Apply a light learner inbox action: accept marks an item practice_ready, defer keeps it for later, ignore hides it, archive stores it without reminders. Never creates attempts or mastery state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "capture_id": {"type": "string", "description": "Capture queue item id returned by capture_evidence or list_learner_inbox."},
                    "action": {"type": "string", "enum": ["accept", "defer", "ignore", "archive"], "description": "Inbox action to apply."},
                    "note": {"type": "string", "description": "Optional note to store with the item."}
                },
                "required": ["capture_id", "action"]
            }
        },
        {
            "name": "draft_inbox_practice",
            "description": "Create a deterministic learner practice prompt from a practice_ready inbox capture and its existing candidate concept. Returns student-facing prompt text only; does not create attempts or expose mastery parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "capture_id": {"type": "string", "description": "Practice-ready capture queue item id returned by list_learner_inbox."}
                },
                "required": ["capture_id"]
            }
        },
        {
            "name": "submit_inbox_practice",
            "description": "Submit the learner's answer to an inbox practice prompt through Polaris-owned scoring. Requires learner self-confidence; ignores external AI score fields and then marks the capture practiced.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "capture_id": {"type": "string", "description": "Practice-ready capture queue item id."},
                    "session": {"type": "string", "description": "Optional session id. Defaults to mcp."},
                    "response": {"type": "string", "description": "Learner's own answer to the drafted practice prompt."},
                    "confidence": {"type": "integer", "minimum": 1, "maximum": 5, "description": "Learner self-confidence collected before feedback."}
                },
                "required": ["capture_id", "response", "confidence"]
            }
        },
        {
            "name": "get_learner_mirror",
            "description": "Return the read-only learner mirror static panel with confidence curve, phase distribution, and recent assertions.",
            "inputSchema": {
                "type": "object",
                "properties": {}
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
            "description": "Read-only status snapshot with current_pack, theta_mode, due_today, and concept states.",
            "mimeType": "application/json"
        },
        {
            "uri": "polaris://trust",
            "name": "Polaris trust panel",
            "description": "Read-only F1-F5 gate status, active experiments, recent activity, and governance thresholds.",
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

fn optional_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
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

fn parse_capture_statuses(values: Vec<String>) -> Result<Vec<CaptureStatus>, String> {
    values
        .into_iter()
        .map(|value| {
            CaptureStatus::parse(&value).ok_or_else(|| {
                "statuses must contain only pending, mapped, practice_ready, practiced, ignored, archived".to_owned()
            })
        })
        .collect()
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
    use rusqlite::{params, Connection};
    use serde_json::{json, Value};
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const API_CONTRACT_DOC: &str = include_str!("../../../docs/API_CONTRACT.md");

    #[test]
    fn mcp_lists_polaris_tools_and_status_resource() {
        let mut session = test_session();

        let tools = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
            .unwrap()
            .unwrap();
        let tools = tools["result"]["tools"].as_array().unwrap();
        assert_contains_names(
            tools,
            &[
                "get_next_task",
                "get_interleaved_batch",
                "get_phase_snapshot",
                "get_active_gu_rules",
                "run_mirror_report",
                "get_latest_mirror_report",
                "mark_report_assertion_inaccurate",
                "mark_report_assertion_accurate",
                "record_learner_feedback",
                "get_trust_panel",
                "detect_project_manifest",
                "capture_evidence",
                "list_learner_inbox",
                "act_on_learner_inbox_item",
                "draft_inbox_practice",
                "submit_inbox_practice",
                "get_learner_mirror",
                "submit_evidence",
                "get_teaching_instruction",
            ],
        );

        let resources = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list"}))
            .unwrap()
            .unwrap();
        let resources = resources["result"]["resources"].as_array().unwrap();
        assert_contains_uris(resources, &["polaris://status", "polaris://trust"]);

        let templates = session
            .handle_request(
                json!({"jsonrpc": "2.0", "id": 20, "method": "resources/templates/list"}),
            )
            .unwrap()
            .unwrap();
        let templates = templates["result"]["resourceTemplates"].as_array().unwrap();
        assert!(
            find_by_field(templates, "uriTemplate", "polaris://concept/{id}/diagnosis").is_some()
        );
    }

    #[test]
    fn mcp_contract_document_names_stable_surface_and_policy() {
        for required in [
            "## MCP v1",
            "initialize",
            "tools/list",
            "resources/list",
            "resources/read",
            "polaris://status",
            "polaris://trust",
            "get_next_task",
            "submit_evidence",
            "record_learner_feedback",
            "get_trust_panel",
            "detect_project_manifest",
            "capture_evidence",
            "list_learner_inbox",
            "act_on_learner_inbox_item",
            "draft_inbox_practice",
            "submit_inbox_practice",
            "get_learner_mirror",
        ] {
            assert!(
                API_CONTRACT_DOC.contains(required),
                "API contract missing MCP item: {required}"
            );
        }
    }

    #[test]
    fn mcp_contract_initialize_keeps_stable_handshake_fields() {
        let mut session = test_session();

        let response = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 100, "method": "initialize"}))
            .unwrap()
            .unwrap();

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 100);
        assert_string_field(&response["result"], "protocolVersion");
        assert!(response["result"]["capabilities"].as_object().is_some());
        assert_eq!(response["result"]["serverInfo"]["name"], "polaris-core");
        assert_eq!(
            response["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn mcp_contract_lists_stable_tools_resources_and_templates() {
        let mut session = test_session();

        let tools_response = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 101, "method": "tools/list"}))
            .unwrap()
            .unwrap();
        let tools = tools_response["result"]["tools"].as_array().unwrap();
        assert_contains_names(
            tools,
            &[
                "get_next_task",
                "get_interleaved_batch",
                "get_phase_snapshot",
                "get_active_gu_rules",
                "run_mirror_report",
                "get_latest_mirror_report",
                "mark_report_assertion_inaccurate",
                "mark_report_assertion_accurate",
                "record_learner_feedback",
                "get_trust_panel",
                "detect_project_manifest",
                "capture_evidence",
                "list_learner_inbox",
                "act_on_learner_inbox_item",
                "draft_inbox_practice",
                "submit_inbox_practice",
                "get_learner_mirror",
                "submit_evidence",
                "get_teaching_instruction",
            ],
        );
        for tool in tools {
            assert_string_field(tool, "name");
            assert_string_field(tool, "description");
            assert_eq!(tool["inputSchema"]["type"], "object");
        }

        let resources_response = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 102, "method": "resources/list"}))
            .unwrap()
            .unwrap();
        let resources = resources_response["result"]["resources"]
            .as_array()
            .unwrap();
        assert_contains_uris(resources, &["polaris://status", "polaris://trust"]);
        for resource in resources {
            assert_string_field(resource, "name");
            assert_eq!(resource["mimeType"], "application/json");
        }

        let templates_response = session
            .handle_request(
                json!({"jsonrpc": "2.0", "id": 103, "method": "resources/templates/list"}),
            )
            .unwrap()
            .unwrap();
        let templates = templates_response["result"]["resourceTemplates"]
            .as_array()
            .unwrap();
        let diagnosis_template =
            find_by_field(templates, "uriTemplate", "polaris://concept/{id}/diagnosis")
                .expect("missing concept diagnosis template");
        assert_eq!(
            diagnosis_template["uriTemplate"],
            "polaris://concept/{id}/diagnosis"
        );
        assert_string_field(diagnosis_template, "name");
        assert_eq!(diagnosis_template["mimeType"], "application/json");
    }

    #[test]
    fn mcp_contract_resource_reads_keep_stable_top_level_fields() {
        let mut session = test_session();

        let status = read_json_resource(&mut session, "polaris://status");
        assert_fields(
            &status,
            &[
                "current_pack",
                "theta_mode",
                "packs",
                "due_today",
                "phase_counts",
                "concepts",
            ],
        );

        let trust = read_json_resource(&mut session, "polaris://trust");
        assert_fields(
            &trust,
            &[
                "gates",
                "active_breeding_experiments",
                "active_mrt_experiments",
                "recent_activity",
                "governance",
            ],
        );
    }

    #[test]
    fn mcp_contract_errors_keep_stable_shape() {
        let mut session = test_session();

        let unknown_method = session
            .handle_request(json!({"jsonrpc": "2.0", "id": 201, "method": "missing/method"}))
            .unwrap()
            .unwrap();
        assert_eq!(unknown_method["jsonrpc"], "2.0");
        assert_eq!(unknown_method["id"], 201);
        assert_eq!(unknown_method["error"]["code"], -32601);
        assert_eq!(unknown_method["error"]["message"], "method not found");

        let unknown_resource = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 202,
                "method": "resources/read",
                "params": {"uri": "polaris://missing"}
            }))
            .unwrap()
            .unwrap();
        assert_eq!(unknown_resource["result"]["isError"], true);
        assert_eq!(
            unknown_resource["result"]["contents"][0]["uri"],
            "polaris://error"
        );
        assert_eq!(
            unknown_resource["result"]["contents"][0]["mimeType"],
            "text/plain"
        );
        assert!(unknown_resource["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown resource"));
    }

    #[test]
    fn mcp_reads_status_resource_includes_current_pack() {
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

        assert_eq!(payload["current_pack"], "rust");
        assert_eq!(payload["theta_mode"], "shared");
        assert_eq!(payload["packs"][0]["id"], "rust");
        assert_eq!(payload["packs"][0]["active"], true);
        assert_eq!(payload["due_today"], 0);
        assert_eq!(payload["concepts"][0]["concept_id"], "ownership");
        assert_eq!(payload["concepts"][0]["phase"], "undetermined");
    }

    #[test]
    fn mcp_get_trust_panel_tool_and_resource_share_shape() {
        let mut session = test_session();

        let tool_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {"name": "get_trust_panel", "arguments": {}}
            }))
            .unwrap()
            .unwrap();
        let tool_text = tool_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let tool_payload: Value = serde_json::from_str(tool_text).unwrap();

        let resource_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "resources/read",
                "params": {"uri": "polaris://trust"}
            }))
            .unwrap()
            .unwrap();
        let resource_text = resource_response["result"]["contents"][0]["text"]
            .as_str()
            .unwrap();
        let resource_payload: Value = serde_json::from_str(resource_text).unwrap();

        for payload in [&tool_payload, &resource_payload] {
            assert!(payload["gates"].as_array().is_some());
            assert!(payload["active_breeding_experiments"].as_array().is_some());
            assert!(payload["active_mrt_experiments"].as_array().is_some());
            assert!(payload["recent_activity"].as_object().is_some());
            assert!(payload["governance"].as_object().is_some());
        }
    }

    #[test]
    fn mcp_detect_project_manifest_returns_project_contract() {
        let mut session = test_session();
        let root = TestDir::new("mcp-project");
        write_test_manifest(root.path());
        let nested = root.path().join("course/day01/src");
        fs::create_dir_all(&nested).unwrap();

        let payload = call_tool_json(
            &mut session,
            "detect_project_manifest",
            json!({"path": nested.display().to_string()}),
        );

        assert_eq!(payload["found"], true);
        assert_eq!(payload["project_root"], root.path().display().to_string());
        assert_eq!(
            payload["manifest_path"],
            root.path().join("p-os.toml").display().to_string()
        );
        assert_eq!(payload["manifest"]["project_id"], "rust-mastery-lab");
        assert_eq!(payload["manifest"]["default_pack"], "rust");
        assert_eq!(
            payload["manifest"]["entry"]["today_command"],
            "cargo run -p labctl -- today --date {today}"
        );
        assert_eq!(payload["manifest"]["evidence"]["include"][0], "course/**");
        assert_eq!(payload["manifest"]["ui"]["preferred_shell"], "aura");
    }

    #[test]
    fn mcp_capture_evidence_records_pending_without_mastery() {
        let mut session = test_session();
        let before_attempts = count_rows(session.engine().conn(), "attempts");
        let before_mastery = count_rows(session.engine().conn(), "mastery_states");
        let before_grade_queue = count_rows(session.engine().conn(), "grade_queue");

        let payload = call_tool_json(
            &mut session,
            "capture_evidence",
            json!({
                "session": "mcp-capture",
                "source": "ai-ide",
                "content_type": "text/plain",
                "text": "我刚看完 Rust 所有权解释，先保存下来，之后再练。",
                "learner_kind": "reference",
                "candidate_concept_ids": ["ownership"],
                "note": "from AI IDE"
            }),
        );

        assert!(payload["capture_id"].as_str().unwrap().len() > 10);
        assert!(payload["evidence_id"].as_str().unwrap().len() > 10);
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["learner_kind"], "reference");
        assert_eq!(payload["recorded_only"], true);
        assert!(payload["message"]
            .as_str()
            .unwrap()
            .contains("不会直接算作掌握"));
        assert_eq!(
            count_rows(session.engine().conn(), "attempts"),
            before_attempts
        );
        assert_eq!(
            count_rows(session.engine().conn(), "mastery_states"),
            before_mastery
        );
        assert_eq!(
            count_rows(session.engine().conn(), "grade_queue"),
            before_grade_queue
        );

        let (status, learner_kind, note, candidates): (String, String, Option<String>, String) =
            session
                .engine()
                .conn()
                .query_row(
                    "SELECT status, learner_kind, note, candidate_concept_ids_json
                     FROM capture_queue
                     WHERE id=?1 AND evidence_id=?2",
                    [
                        payload["capture_id"].as_str().unwrap(),
                        payload["evidence_id"].as_str().unwrap(),
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(learner_kind, "reference");
        assert_eq!(note.as_deref(), Some("from AI IDE"));
        assert_eq!(candidates, "[\"ownership\"]");
    }

    #[test]
    fn mcp_get_learner_mirror_returns_static_panel_fields() {
        let mut session = test_session();

        let payload = call_tool_json(&mut session, "get_learner_mirror", json!({}));

        assert_fields(
            &payload,
            &[
                "generated_at",
                "confidence_curve",
                "phase_distribution",
                "recent_assertions",
            ],
        );
    }

    #[test]
    fn p12d_mcp_lists_and_updates_learner_inbox_without_mastery_facts() {
        let mut session = test_session();
        let before = (
            count_rows(session.engine().conn(), "attempts"),
            count_rows(session.engine().conn(), "mastery_states"),
            count_rows(session.engine().conn(), "grade_queue"),
        );

        let capture = call_tool_json(
            &mut session,
            "capture_evidence",
            json!({
                "text": "Borrow checker note from an AI IDE.",
                "source": "ai-ide",
                "candidate_concept_ids": ["ownership"]
            }),
        );
        let capture_id = capture["capture_id"].as_str().unwrap().to_owned();

        let inbox = call_tool_json(&mut session, "list_learner_inbox", json!({}));

        let items = inbox["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["capture_id"], capture_id.as_str());
        assert_eq!(items[0]["status"], "pending");
        assert_eq!(items[0]["message"], "已保存，稍后帮你整理");
        assert_eq!(items[0]["actions"][0]["action"], "accept");
        assert_eq!(items[0]["actions"][0]["label"], "转成一道小题");
        assert!(items[0].get("p_known").is_none());
        assert!(items[0].get("theta").is_none());

        let receipt = call_tool_json(
            &mut session,
            "act_on_learner_inbox_item",
            json!({
                "capture_id": capture_id.as_str(),
                "action": "accept"
            }),
        );

        assert_eq!(receipt["status"], "practice_ready");
        assert_eq!(receipt["effect"], "recorded_only");
        assert!(
            receipt["message"].as_str().unwrap().contains("小题"),
            "{receipt}"
        );
        assert_eq!(
            (
                count_rows(session.engine().conn(), "attempts"),
                count_rows(session.engine().conn(), "mastery_states"),
                count_rows(session.engine().conn(), "grade_queue"),
            ),
            before
        );
    }

    #[test]
    fn p12e_mcp_drafts_and_submits_inbox_practice_without_external_score() {
        let mut session = test_session();

        let capture = call_tool_json(
            &mut session,
            "capture_evidence",
            json!({
                "text": "Ownership means one value has one owner at a time.",
                "source": "ai-ide",
                "candidate_concept_ids": ["ownership"]
            }),
        );
        let capture_id = capture["capture_id"].as_str().unwrap().to_owned();
        call_tool_json(
            &mut session,
            "act_on_learner_inbox_item",
            json!({
                "capture_id": capture_id,
                "action": "accept"
            }),
        );

        let draft = call_tool_json(
            &mut session,
            "draft_inbox_practice",
            json!({"capture_id": capture_id.as_str()}),
        );

        assert_eq!(draft["capture_id"], capture_id.as_str());
        assert_eq!(draft["status"], "practice_ready");
        assert_eq!(draft["concept_hint"], "Ownership");
        assert_eq!(draft["task_type"], "explain");
        assert!(draft["prompt"].as_str().unwrap().contains("用自己的话"));
        assert!(draft.get("p_known").is_none());
        assert!(draft.get("theta").is_none());

        let receipt = call_tool_json(
            &mut session,
            "submit_inbox_practice",
            json!({
                "capture_id": capture_id.as_str(),
                "session": "mcp-practice",
                "response": "Ownership controls which binding can drop the value.",
                "confidence": 4,
                "external_score": 1.0,
                "final_score": 1.0
            }),
        );

        assert_eq!(receipt["capture_id"], capture_id.as_str());
        assert_eq!(receipt["status"], "practiced");
        assert_eq!(receipt["effect"], "submitted");
        assert!((receipt["provisional_score"].as_f64().unwrap() - 0.70).abs() < 1e-9);
        assert_eq!(count_rows(session.engine().conn(), "attempts"), 1);
        assert_eq!(count_rows(session.engine().conn(), "mastery_states"), 1);
        assert_eq!(count_rows(session.engine().conn(), "grade_queue"), 1);
        let final_score_count: i64 = session
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM attempts WHERE final_score IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(final_score_count, 0);
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
    fn mcp_learner_feedback_records_state_and_pause() {
        let mut session = test_session();

        let state_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 33,
                "method": "tools/call",
                "params": {"name": "record_learner_feedback", "arguments": {
                    "session": "mcp-flow",
                    "kind": "state",
                    "concept": "ownership",
                    "state": "frustrated",
                    "note": "transfer feels stuck"
                }}
            }))
            .unwrap()
            .unwrap();
        let state_text = state_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let state_payload: Value = serde_json::from_str(state_text).unwrap();
        assert_eq!(state_payload["kind"], "state");
        assert_eq!(state_payload["state"], "frustrated");
        assert_eq!(state_payload["effect"], "recorded_only");

        let pause_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 34,
                "method": "tools/call",
                "params": {"name": "record_learner_feedback", "arguments": {
                    "kind": "pause",
                    "reason": "done_for_now"
                }}
            }))
            .unwrap()
            .unwrap();
        let pause_text = pause_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let pause_payload: Value = serde_json::from_str(pause_text).unwrap();
        assert_eq!(pause_payload["session_id"], "mcp");
        assert_eq!(pause_payload["kind"], "pause");
        assert_eq!(pause_payload["reason"], "done_for_now");

        let feedback_events: i64 = session
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events WHERE type='learner_feedback'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(feedback_events, 2);
    }

    #[test]
    fn mcp_learner_feedback_returns_tool_error_for_invalid_kind() {
        let mut session = test_session();

        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 35,
                "method": "tools/call",
                "params": {"name": "record_learner_feedback", "arguments": {
                    "kind": "mood",
                    "state": "flow"
                }}
            }))
            .unwrap()
            .unwrap();

        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("learner_feedback.kind"));
    }

    #[test]
    fn mcp_phase_gu_and_mirror_report_tools() {
        let mut session = test_session();

        let phase_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 40,
                "method": "tools/call",
                "params": {"name": "get_phase_snapshot", "arguments": {}}
            }))
            .unwrap()
            .unwrap();
        let phase_text = phase_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let phase_payload: Value = serde_json::from_str(phase_text).unwrap();
        assert!(phase_payload["phase_counts"].as_array().unwrap().len() >= 7);
        assert_eq!(phase_payload["concepts"][0]["concept_id"], "ownership");
        assert_eq!(phase_payload["concepts"][0]["phase"], "undetermined");

        let concept_ids_json = serde_json::to_string(&vec!["ownership"]).unwrap();
        session
            .engine()
            .conn()
            .execute(
                "INSERT INTO gu_rules(id, pattern, concept_ids_json, attempt_ids_json,
                                      first_seen, last_seen, count, status, alpha, beta, updated_at)
                 VALUES ('gu-mcp-test', 'fluency-illusion', ?1, '[]',
                         '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z',
                         3, 'validated', 4.0, 1.0, '2026-01-02T00:00:00Z')",
                [concept_ids_json],
            )
            .unwrap();

        let gu_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 41,
                "method": "tools/call",
                "params": {"name": "get_active_gu_rules", "arguments": {
                    "concept": "ownership"
                }}
            }))
            .unwrap()
            .unwrap();
        let gu_text = gu_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let gu_payload: Value = serde_json::from_str(gu_text).unwrap();
        assert_eq!(gu_payload[0]["id"], "gu-mcp-test");
        assert_eq!(gu_payload[0]["pattern"], "fluency-illusion");

        let latest_empty = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 42,
                "method": "tools/call",
                "params": {"name": "get_latest_mirror_report", "arguments": {}}
            }))
            .unwrap()
            .unwrap();
        let latest_empty_text = latest_empty["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let latest_empty_payload: Value = serde_json::from_str(latest_empty_text).unwrap();
        assert!(latest_empty_payload["report"].is_null());

        let missing_gu_concept = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 421,
                "method": "tools/call",
                "params": {"name": "get_active_gu_rules", "arguments": {}}
            }))
            .unwrap()
            .unwrap();
        assert_eq!(missing_gu_concept["result"]["isError"], true);
        assert!(missing_gu_concept["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("missing string argument: concept_id"));

        let missing_report_feedback = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 422,
                "method": "tools/call",
                "params": {"name": "mark_report_assertion_inaccurate", "arguments": {
                    "report_id": "missing-report",
                    "assertion_id": "assertion:missing"
                }}
            }))
            .unwrap()
            .unwrap();
        assert_eq!(missing_report_feedback["result"]["isError"], true);
        assert!(missing_report_feedback["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("report.feedback_report_id"));
        let feedback_events_before_success: i64 = session
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events WHERE type='report_feedback'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(feedback_events_before_success, 0);

        let report_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 43,
                "method": "tools/call",
                "params": {"name": "run_mirror_report", "arguments": {}}
            }))
            .unwrap()
            .unwrap();
        let report_text = report_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let report_payload: Value = serde_json::from_str(report_text).unwrap();
        let generated_report_id = report_payload["id"].as_str().unwrap();
        assert_eq!(report_payload["schema_version"], 1);

        let latest_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 44,
                "method": "tools/call",
                "params": {"name": "get_latest_mirror_report", "arguments": {}}
            }))
            .unwrap()
            .unwrap();
        let latest_text = latest_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let latest_payload: Value = serde_json::from_str(latest_text).unwrap();
        assert_eq!(latest_payload["report"]["id"], generated_report_id);

        let feedback_report = json!({
            "schema_version": 1,
            "id": "mcp-feedback-report",
            "week": "2026-W01",
            "generated_at": "2026-01-02T00:00:00Z",
            "window_days": 7,
            "assertions": [{
                "id": "assertion:calibration",
                "kind": "calibration",
                "subject": "ownership",
                "claim": "校准断言",
                "confidence": 0.8,
                "evidence_ids": ["evidence-1"],
                "stats": {}
            }],
            "hypotheses": [],
            "suggestions": [],
            "skipped": [],
            "hazard_gate": {
                "participates": false,
                "reason": "unfit",
                "validation_auc": null,
                "auc_gate": 0.70
            },
            "reflection_prompts": []
        });
        session
            .engine()
            .conn()
            .execute(
                "INSERT INTO mirror_reports(id, week, generated_at, report_json, assertion_count, skipped_count)
                 VALUES ('mcp-feedback-report', '2026-W01', '2026-01-02T00:00:00Z', ?1, 1, 0)",
                params![feedback_report.to_string()],
            )
            .unwrap();

        let accurate_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 451,
                "method": "tools/call",
                "params": {"name": "mark_report_assertion_accurate", "arguments": {
                    "report_id": "mcp-feedback-report",
                    "assertion_id": "assertion:calibration"
                }}
            }))
            .unwrap()
            .unwrap();
        let accurate_text = accurate_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let accurate_payload: Value = serde_json::from_str(accurate_text).unwrap();
        assert_eq!(accurate_payload["report_id"], "mcp-feedback-report");
        assert_eq!(accurate_payload["assertion_id"], "assertion:calibration");
        assert_eq!(accurate_payload["verdict"], "accurate");
        assert_eq!(accurate_payload["effect"], "recorded_only");

        let feedback_response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 45,
                "method": "tools/call",
                "params": {"name": "mark_report_assertion_inaccurate", "arguments": {
                    "report_id": "mcp-feedback-report",
                    "assertion_id": "assertion:calibration"
                }}
            }))
            .unwrap()
            .unwrap();
        let feedback_text = feedback_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let feedback_payload: Value = serde_json::from_str(feedback_text).unwrap();
        assert_eq!(feedback_payload["report_id"], "mcp-feedback-report");
        assert_eq!(feedback_payload["assertion_id"], "assertion:calibration");
        assert_eq!(feedback_payload["verdict"], "inaccurate");

        let feedback_events: i64 = session
            .engine()
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM behavior_events WHERE type='report_feedback'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(feedback_events, 2);
    }

    #[test]
    fn mcp_report_narrative_argument_degrades_without_llm_config() {
        let _guard = EnvGuard::remove(llm_env_keys());
        let mut session = test_session();
        seed_phantom_concept(session.engine().conn(), "ownership", 4);

        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 450,
                "method": "tools/call",
                "params": {"name": "run_mirror_report", "arguments": {
                    "narrative": true
                }}
            }))
            .unwrap()
            .unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(text).unwrap();

        assert!(!payload["assertions"].as_array().unwrap().is_empty());
        assert!(payload["narrative"].is_null());
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

    fn seed_phantom_concept(conn: &Connection, concept_id: &str, attempt_count: usize) {
        for idx in 0..attempt_count {
            conn.execute(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                      final_score, created_at, graded_at)
                 VALUES (?1, 's1', ?2, 'recall', 5, 0.2,
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'),
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'))",
                (format!("{concept_id}-mcp-narrative-{idx}"), concept_id),
            )
            .unwrap();
        }
        conn.execute(
            "INSERT OR REPLACE INTO mastery_states(concept_id, p_known, calib_gap, attempt_count, updated_at)
             VALUES (?1, 0.30, 0.40, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            (concept_id, attempt_count as i64),
        )
        .unwrap();
    }

    struct EnvGuard {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn remove(keys: &[&'static str]) -> Self {
            let values = keys
                .iter()
                .map(|key| {
                    let value = std::env::var(key).ok();
                    std::env::remove_var(key);
                    (*key, value)
                })
                .collect();
            Self { values }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn llm_env_keys() -> &'static [&'static str] {
        &[
            "POLARIS_LLM_FAST_BASE_URL",
            "POLARIS_LLM_FAST_MODEL",
            "POLARIS_LLM_FAST_API_KEY",
            "POLARIS_LLM_STRONG_BASE_URL",
            "POLARIS_LLM_STRONG_MODEL",
            "POLARIS_LLM_STRONG_API_KEY",
        ]
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }

    fn call_tool_json(session: &mut McpSession, name: &str, arguments: Value) -> Value {
        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 300,
                "method": "tools/call",
                "params": {"name": name, "arguments": arguments}
            }))
            .unwrap()
            .unwrap();
        assert_ne!(
            response["result"]["isError"], true,
            "tool returned error: {}",
            response["result"]["content"][0]["text"]
        );
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    fn read_json_resource(session: &mut McpSession, uri: &str) -> Value {
        let response = session
            .handle_request(json!({
                "jsonrpc": "2.0",
                "id": 301,
                "method": "resources/read",
                "params": {"uri": uri}
            }))
            .unwrap()
            .unwrap();
        let text = response["result"]["contents"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    fn count_rows(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    fn write_test_manifest(root: &Path) {
        fs::write(
            root.join("p-os.toml"),
            r#"
schema_version = 1
project_id = "rust-mastery-lab"
title = "Rust 与软件工程训练"
kind = "course"
default_pack = "rust"
default_entry = "today"

[entry]
start_label = "继续今天"
capture_label = "记录我刚学到的"
stuck_label = "我卡住了"
today_command = "cargo run -p labctl -- today --date {today}"

[evidence]
include = ["course/**", "exercises/**"]
ignore = ["target/**", ".git/**"]

[ui]
preferred_shell = "aura"
"#
            .trim_start(),
        )
        .unwrap();
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "polaris-p13a-{name}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
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

    fn assert_contains_names(items: &[Value], expected: &[&str]) {
        for name in expected {
            assert!(
                find_by_field(items, "name", name).is_some(),
                "missing stable name {name}"
            );
        }
    }

    fn assert_contains_uris(items: &[Value], expected: &[&str]) {
        for uri in expected {
            assert!(
                find_by_field(items, "uri", uri).is_some(),
                "missing stable uri {uri}"
            );
        }
    }

    fn find_by_field<'a>(items: &'a [Value], field: &str, expected: &str) -> Option<&'a Value> {
        items
            .iter()
            .find(|item| item[field].as_str() == Some(expected))
    }
}
