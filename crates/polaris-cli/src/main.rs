use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, Subcommand};
use polaris_core::db::{open_database, open_database_read_only};
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::ops::{doctor_report, DoctorReport};
use polaris_core::pack::validate_pack_path;
use polaris_core::privacy::PrivacyCallInventory;
use polaris_core::status::StatusSnapshot;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

mod http;
mod mcp;

#[derive(Debug, Parser)]
#[command(name = "polaris")]
struct Cli {
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        #[arg(long, default_value = "packs/rust")]
        pack: PathBuf,
    },
    Ingest {
        #[arg(long)]
        text: Option<String>,
        #[arg(long, default_value = "cli")]
        session: String,
        #[arg(long, default_value = "cli")]
        source: String,
        #[arg(long)]
        adapter_command: Option<String>,
        #[arg(long = "adapter-arg", allow_hyphen_values = true)]
        adapter_args: Vec<String>,
    },
    Next {
        #[arg(long, default_value = "cli")]
        session: String,
    },
    Submit {
        #[arg(long)]
        concept: String,
        #[arg(long)]
        response: String,
        #[arg(long)]
        confidence: i32,
        #[arg(long, default_value = "recall")]
        task_type: String,
        #[arg(long, default_value = "")]
        prompt: String,
        #[arg(long, default_value = "cli")]
        session: String,
    },
    Hint {
        #[arg(long)]
        concept: String,
        #[arg(long, default_value = "cli")]
        session: String,
    },
    Abandon {
        #[arg(long)]
        concept: String,
        #[arg(long, default_value = "cli")]
        session: String,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
    Backup {
        #[arg(long)]
        output: PathBuf,
    },
    Doctor {
        #[arg(long)]
        json: bool,
    },
    ServeHttp {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
    GradePending,
    Report {
        #[arg(long)]
        narrative: bool,
    },
    Tune,
    MentalFit,
    ReportFeedback {
        #[arg(long)]
        assertion: String,
        #[arg(long)]
        report: Option<String>,
    },
    Diagnose {
        #[arg(long)]
        concept: String,
    },
    Mcp,
    Pack {
        #[command(subcommand)]
        command: PackCommands,
    },
    Privacy {
        #[command(subcommand)]
        command: PrivacyCommands,
    },
}

#[derive(Debug, Subcommand)]
enum PackCommands {
    Validate { path: PathBuf },
}

#[derive(Debug, Subcommand)]
enum PrivacyCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Pack {
            command: PackCommands::Validate { path },
        } => {
            let report = validate_pack_path(path)?;
            println!(
                "pack ok: concepts={} prerequisites={} misconceptions={}",
                report.concept_count, report.prerequisite_count, report.misconception_count
            );
        }
        Commands::Privacy {
            command: PrivacyCommands::Show { json },
        } => {
            let inventory = PrivacyCallInventory::all();
            if json {
                println!("{}", serde_json::to_string_pretty(&inventory)?);
            } else {
                print!("{}", privacy_show_text(&inventory, inventory.tier0_only));
            }
        }
        Commands::Diagnose { concept } => {
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            print_diagnosis(engine.diagnose_concept(&concept)?);
        }
        Commands::Mcp => {
            let conn = open_database(cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            mcp::serve_stdio(engine)?;
        }
        Commands::Backup { output } => {
            let db_path = cli.db.unwrap_or_else(default_db_path);
            let conn = open_existing_database(&db_path)?;
            backup_database(&conn, &output)?;
            println!("backup written: {}", output.display());
        }
        Commands::Doctor { json } => {
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let report = doctor_report(&conn)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", doctor_report_text(&report));
            }
            if !report.ok {
                return Err(adapter_error("doctor found data integrity problems"));
            }
        }
        command => {
            let conn = open_database(cli.db.unwrap_or_else(default_db_path))?;
            let mut engine = Engine::new(conn);
            match command {
                Commands::Init { pack } => {
                    engine.init_pack(pack)?;
                    println!("initialized");
                }
                Commands::Ingest {
                    text,
                    session,
                    source,
                    adapter_command,
                    adapter_args,
                } => match (text, adapter_command) {
                    (Some(text), None) => {
                        ingest_text(&engine, &session, &source, &text)?;
                        println!("ingested");
                    }
                    (None, Some(command)) => {
                        let summary = ingest_adapter_command(&mut engine, &command, &adapter_args)?;
                        println!(
                            "ingested evidence={} attempts={}",
                            summary.evidence, summary.attempts
                        );
                    }
                    (Some(_), Some(_)) => {
                        return Err(adapter_error(
                            "ingest accepts either --text or --adapter-command, not both",
                        ));
                    }
                    (None, None) => {
                        return Err(adapter_error("ingest requires --text or --adapter-command"));
                    }
                },
                Commands::Next { session } => {
                    if let Some(task) = engine.next_task()? {
                        engine.record_next_task_event(&session, &task)?;
                        println!("concept: {}", task.concept_id);
                        println!("task_type: {}", task.task_type);
                        println!("prompt: {}", task.prompt_text);
                        println!("{}", task.reason);
                    } else {
                        println!("no task");
                    }
                }
                Commands::Submit {
                    concept,
                    response,
                    confidence,
                    task_type,
                    prompt,
                    session,
                } => {
                    let observation =
                        read_behavior_observation_now(engine.conn(), &session, &concept)?;
                    let receipt = engine.submit(SubmitInput {
                        session_id: session,
                        concept_id: concept,
                        task_type,
                        prompt_text: prompt,
                        response_text: response,
                        self_confidence: confidence,
                        latency_ms: observation.latency_ms,
                        hint_count: observation.hint_count,
                    })?;
                    println!(
                        "attempt: {} provisional_score={:.3} degraded={}",
                        receipt.attempt_id, receipt.provisional_score, receipt.degraded
                    );
                }
                Commands::Hint { concept, session } => {
                    engine.conn().execute(
                        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
                         VALUES (lower(hex(randomblob(16))), ?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'hint', ?2, '{\"template\":\"回到概念边界，先说它限制什么。\"}')",
                        (&session, &concept),
                    )?;
                    println!("提示：回到概念边界，先说它限制什么。");
                }
                Commands::Abandon { concept, session } => {
                    engine.conn().execute(
                        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
                         VALUES (lower(hex(randomblob(16))), ?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'abandon', ?2, '{}')",
                        (&session, &concept),
                    )?;
                    println!("abandoned");
                }
                Commands::Status { json } => {
                    let snapshot = engine.status_snapshot()?;
                    if json {
                        println!("{}", status_snapshot_json(&snapshot)?);
                    } else {
                        print_status_snapshot(&snapshot);
                    }
                }
                Commands::ServeHttp { host, port } => {
                    http::serve_http(engine, &host, port)?;
                }
                Commands::GradePending => {
                    let summary = engine.grade_pending()?;
                    println!(
                        "processed={} pending={}",
                        summary.processed, summary.pending
                    );
                }
                Commands::Report { narrative } => {
                    let report = if narrative {
                        engine.run_mirror_report_with_narrative()?
                    } else {
                        engine.run_mirror_report()?
                    };
                    print_mirror_report(&report);
                }
                Commands::MentalFit => {
                    let summary = engine.run_mental_dynamics_fit()?;
                    println!(
                        "hazard: {} {}",
                        summary.hazard.status, summary.hazard.detail
                    );
                    println!(
                        "state_gate: {} {}",
                        summary.state_gate.status, summary.state_gate.detail
                    );
                    println!("em: {} {}", summary.em.status, summary.em.detail);
                }
                Commands::Tune => {
                    let summary = engine.run_param_tuning()?;
                    if summary.outcomes.is_empty() && summary.skipped.is_empty() {
                        println!("无可评估槽位");
                    }
                    for outcome in &summary.outcomes {
                        println!(
                            "{} {}: {} -> {} （{} 改善 {:+.4}）",
                            if outcome.accepted {
                                "accepted"
                            } else {
                                "rejected"
                            },
                            outcome.param,
                            outcome.old_value,
                            outcome.new_value,
                            outcome.metric,
                            outcome.delta,
                        );
                    }
                    for skip in &summary.skipped {
                        println!("skipped {skip}");
                    }
                }
                Commands::ReportFeedback { assertion, report } => {
                    let report_id = engine.record_report_feedback(report.as_deref(), &assertion)?;
                    println!("已记录「不准」反馈：report={report_id} assertion={assertion}");
                    println!("该断言将在抑制窗口内不再进入报告；这条反馈本身已成为校正数据。");
                }
                Commands::Diagnose { .. } => unreachable!("handled before writable database open"),
                Commands::Mcp => unreachable!("handled before command dispatch"),
                Commands::Backup { .. } => unreachable!("handled before writable database open"),
                Commands::Doctor { .. } => unreachable!("handled before writable database open"),
                Commands::Pack { .. } => unreachable!("handled before database open"),
                Commands::Privacy { .. } => unreachable!("handled before database open"),
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AdapterIngestSummary {
    evidence: usize,
    attempts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AdapterEvent {
    Evidence {
        session: String,
        source: String,
        content_type: String,
        text: String,
        concept_ids: Vec<String>,
    },
    Attempt {
        input: SubmitInput,
    },
}

fn ingest_text(
    engine: &Engine,
    session: &str,
    source: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    engine.conn().execute(
        "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
         VALUES (lower(hex(randomblob(16))), ?1, ?2, 'text/plain', ?3, '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        (session, source, text),
    )?;
    Ok(())
}

fn ingest_adapter_command(
    engine: &mut Engine,
    command: &str,
    args: &[String],
) -> Result<AdapterIngestSummary, Box<dyn std::error::Error>> {
    let output = Command::new(command).args(args).output()?;
    if !output.status.success() {
        return Err(adapter_error(format!(
            "adapter command exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let stdout = String::from_utf8(output.stdout)?;
    ingest_adapter_jsonl(engine, &stdout)
}

fn ingest_adapter_jsonl(
    engine: &mut Engine,
    jsonl: &str,
) -> Result<AdapterIngestSummary, Box<dyn std::error::Error>> {
    let events = parse_adapter_jsonl(jsonl)?;
    let mut summary = AdapterIngestSummary::default();
    for event in events {
        match event {
            AdapterEvent::Evidence {
                session,
                source,
                content_type,
                text,
                concept_ids,
            } => {
                ingest_adapter_evidence(
                    engine,
                    &session,
                    &source,
                    &content_type,
                    &text,
                    &concept_ids,
                )?;
                summary.evidence += 1;
            }
            AdapterEvent::Attempt { input } => {
                let _ = engine.submit(input)?;
                summary.attempts += 1;
            }
        }
    }
    Ok(summary)
}

fn parse_adapter_jsonl(jsonl: &str) -> Result<Vec<AdapterEvent>, Box<dyn std::error::Error>> {
    let mut events = Vec::new();
    for (line_index, line) in jsonl.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(line).map_err(|error| {
            adapter_error(format!("adapter JSONL line {}: {error}", line_index + 1))
        })?;
        let event_type = required_event_str(&event, "type", line_index)?;
        match event_type {
            "evidence" => {
                events.push(parse_adapter_evidence(&event, line_index)?);
            }
            "attempt" => {
                events.push(parse_adapter_attempt(&event, line_index)?);
            }
            other => {
                return Err(adapter_error(format!(
                    "adapter JSONL line {}: unsupported adapter event type `{other}`",
                    line_index + 1
                )));
            }
        }
    }
    Ok(events)
}

fn ingest_adapter_evidence(
    engine: &Engine,
    session: &str,
    source: &str,
    content_type: &str,
    text: &str,
    concept_ids: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    engine.conn().execute(
        "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
         VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        (
            session,
            source,
            content_type,
            text,
            serde_json::to_string(&concept_ids)?,
        ),
    )?;
    Ok(())
}

fn parse_adapter_evidence(
    event: &Value,
    line_index: usize,
) -> Result<AdapterEvent, Box<dyn std::error::Error>> {
    Ok(AdapterEvent::Evidence {
        session: optional_event_str(event, "session")
            .unwrap_or("adapter")
            .to_owned(),
        source: optional_event_str(event, "source")
            .unwrap_or("adapter")
            .to_owned(),
        content_type: optional_event_str(event, "content_type")
            .unwrap_or("text/plain")
            .to_owned(),
        text: required_event_str(event, "text", line_index)?.to_owned(),
        concept_ids: optional_event_string_array(event, "concept_ids", line_index)?,
    })
}

fn parse_adapter_attempt(
    event: &Value,
    line_index: usize,
) -> Result<AdapterEvent, Box<dyn std::error::Error>> {
    let confidence = required_event_i64(event, "confidence", line_index)?;
    if !(1..=5).contains(&confidence) {
        return Err(adapter_error(format!(
            "adapter JSONL line {}: confidence must be in 1..=5",
            line_index + 1
        )));
    }
    let concept_id = required_event_str(event, "concept_id", line_index)?;
    let response = required_event_str(event, "response", line_index)?;
    Ok(AdapterEvent::Attempt {
        input: SubmitInput {
            session_id: optional_event_str(event, "session")
                .unwrap_or("adapter")
                .to_owned(),
            concept_id: concept_id.to_owned(),
            task_type: optional_event_str(event, "task_type")
                .unwrap_or("recall")
                .to_owned(),
            prompt_text: optional_event_str(event, "prompt").unwrap_or("").to_owned(),
            response_text: response.to_owned(),
            self_confidence: confidence as i32,
            latency_ms: optional_event_i64(event, "latency_ms").unwrap_or(0).max(0),
            hint_count: optional_event_i64(event, "hint_count").unwrap_or(0).max(0),
        },
    })
}

fn required_event_str<'a>(
    event: &'a Value,
    key: &str,
    line_index: usize,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    optional_event_str(event, key).ok_or_else(|| {
        adapter_error(format!(
            "adapter JSONL line {}: missing string field `{key}`",
            line_index + 1
        ))
    })
}

fn optional_event_str<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event.get(key).and_then(Value::as_str)
}

fn required_event_i64(
    event: &Value,
    key: &str,
    line_index: usize,
) -> Result<i64, Box<dyn std::error::Error>> {
    optional_event_i64(event, key).ok_or_else(|| {
        adapter_error(format!(
            "adapter JSONL line {}: missing integer field `{key}`",
            line_index + 1
        ))
    })
}

fn optional_event_i64(event: &Value, key: &str) -> Option<i64> {
    event.get(key).and_then(Value::as_i64)
}

fn optional_event_string_array(
    event: &Value,
    key: &str,
    line_index: usize,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let Some(value) = event.get(key) else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(adapter_error(format!(
            "adapter JSONL line {}: `{key}` must be an array of strings",
            line_index + 1
        )));
    };
    let mut strings = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(adapter_error(format!(
                "adapter JSONL line {}: `{key}` must be an array of strings",
                line_index + 1
            )));
        };
        strings.push(text.to_owned());
    }
    Ok(strings)
}

fn adapter_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.into(),
    ))
}

fn open_existing_database(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(adapter_error(format!(
            "database does not exist: {}",
            path.display()
        )));
    }
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(Into::into)
}

fn backup_database(conn: &Connection, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() {
        return Err(adapter_error(format!(
            "backup output already exists: {}",
            output.display()
        )));
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    conn.execute("VACUUM INTO ?1", [output.to_string_lossy().to_string()])?;
    Ok(())
}

fn doctor_report_text(report: &DoctorReport) -> String {
    let mut text = String::new();
    text.push_str(&format!("ok={}\n", report.ok));
    text.push_str(&format!(
        "integrity={}\n",
        if report.integrity_ok { "ok" } else { "failed" }
    ));
    for message in &report.integrity_messages {
        text.push_str(&format!("integrity_message={message}\n"));
    }
    text.push_str(&format!("replay_checked={}\n", report.replay_checked));
    text.push_str(&format!(
        "replay_mismatches={}\n",
        report.replay_mismatches.len()
    ));
    for mismatch in &report.replay_mismatches {
        text.push_str(&format!(
            "mismatch\t{}\t{}\texpected={}\tactual={}\n",
            mismatch.concept_id, mismatch.field, mismatch.expected, mismatch.actual
        ));
    }
    text
}

fn privacy_show_text(inventory: &PrivacyCallInventory, tier0_only: bool) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "Tier 0 only 模式：{}\n",
        if tier0_only { "启用" } else { "未启用" }
    ));
    text.push_str("设置 POLARIS_TIER0_ONLY=1 可全禁外部模型调用。\n");
    for call in &inventory.calls {
        text.push_str(&format!("\n{}\n", call.id));
        text.push_str(&format!("tier: {}\n", call.tier));
        text.push_str(&format!("trigger: {}\n", call.trigger));
        text.push_str(&format!("env: {}\n", call.env_keys.join(", ")));
        text.push_str(&format!("data_sent: {}\n", call.data_sent.join("; ")));
        text.push_str(&format!("degradation: {}\n", call.degradation));
        text.push_str(&format!(
            "disabled_when_tier0_only: {}\n",
            call.disabled_when_tier0_only
        ));
    }
    text
}

fn status_snapshot_json(snapshot: &StatusSnapshot) -> serde_json::Result<String> {
    serde_json::to_string_pretty(snapshot)
}

fn print_status_snapshot(snapshot: &StatusSnapshot) {
    print!("{}", status_snapshot_text(snapshot));
}

fn status_snapshot_text(snapshot: &StatusSnapshot) -> String {
    let mut text = format!("due_today={}\n", snapshot.due_today);
    for concept in &snapshot.concepts {
        let retrieval = concept
            .retrieval
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "-".to_owned());
        text.push_str(&format!(
            "{}\t{}\tR={}\tp_known={:.3}\tcalib_gap={:.3}\tphase={}",
            concept.concept_id,
            concept.name,
            retrieval,
            concept.p_known,
            concept.calib_gap,
            concept.phase
        ));
        text.push('\n');
    }
    text
}

fn print_diagnosis(diagnosis: polaris_core::diagnosis::GraphDiagnosis) {
    println!("concept: {}", diagnosis.concept_id);
    println!("latest_failed: {}", diagnosis.latest_failed);
    if let Some(score) = diagnosis.latest_score {
        println!("latest_score: {score:.3}");
    }
    if let Some(focus) = diagnosis.focus {
        println!(
            "focus: {} {} reason={}",
            focus.kind, focus.concept_id, focus.reason
        );
    } else {
        println!("focus: none");
    }
    for gap in diagnosis.unmet_prerequisites {
        println!(
            "prerequisite_gap: {} p_known={:.3} threshold={:.3}",
            gap.concept_id, gap.p_known, gap.threshold
        );
    }
    for task in diagnosis.confusion_tasks {
        println!(
            "confusion_task: {} vs {} task_type={}",
            task.concept_id, task.contrast_concept_id, task.task_type
        );
        println!("prompt: {}", task.prompt);
    }
}

fn print_mirror_report(report: &polaris_core::report::MirrorReport) {
    println!("镜像报告 {} （周 {}）", report.id, report.week);
    println!(
        "窗口={}天 断言={} 假设={} 建议={} 被过滤={}",
        report.window_days,
        report.assertions.len(),
        report.hypotheses.len(),
        report.suggestions.len(),
        report.skipped.len()
    );
    println!(
        "hazard 门：participates={} reason={}",
        report.hazard_gate.participates, report.hazard_gate.reason
    );
    if let Some(narrative) = &report.narrative {
        println!("--- Tier 1 叙事 ---");
        println!("{}", narrative.text);
        if !narrative.citations.is_empty() {
            let cited = narrative
                .citations
                .iter()
                .map(|citation| citation.evidence_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            println!("引用断言：{cited}");
        }
    }
    for section in [
        ("断言", &report.assertions),
        ("假设（未过验证门）", &report.hypotheses),
        ("参数建议（只建议不执行）", &report.suggestions),
    ] {
        let (label, items) = section;
        if items.is_empty() {
            continue;
        }
        println!("--- {label} ---");
        for item in items.iter() {
            println!(
                "[{}] 置信度={:.0}% 证据={}条",
                item.id,
                item.confidence * 100.0,
                item.evidence_ids.len()
            );
            println!("  {}", item.claim);
        }
    }
    if !report.skipped.is_empty() {
        println!("--- 被过滤候选 ---");
        for skip in &report.skipped {
            println!("[{}] reason={}", skip.id, skip.reason);
        }
    }
    println!("--- 三问反思 ---");
    for prompt in &report.reflection_prompts {
        println!("· {prompt}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BehaviorObservation {
    latency_ms: i64,
    hint_count: i64,
}

fn read_behavior_observation_now(
    conn: &rusqlite::Connection,
    session: &str,
    concept: &str,
) -> Result<BehaviorObservation, Box<dyn std::error::Error>> {
    let now: String = conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
        row.get(0)
    })?;
    Ok(read_behavior_observation_at(conn, session, concept, &now)?)
}

fn read_behavior_observation_at(
    conn: &rusqlite::Connection,
    session: &str,
    concept: &str,
    now: &str,
) -> rusqlite::Result<BehaviorObservation> {
    let last_next_at: Option<String> = conn
        .query_row(
            "SELECT at
             FROM behavior_events
             WHERE session_id=?1 AND concept_id=?2 AND type='next'
             ORDER BY julianday(at) DESC, id DESC
             LIMIT 1",
            (session, concept),
            |row| row.get(0),
        )
        .optional()?;

    let Some(last_next_at) = last_next_at else {
        return Ok(BehaviorObservation {
            latency_ms: 0,
            hint_count: 0,
        });
    };

    let latency_ms = conn.query_row(
        "SELECT CAST(MAX(0, ROUND((julianday(?1)-julianday(?2))*86400000.0)) AS INTEGER)",
        (now, last_next_at.as_str()),
        |row| row.get(0),
    )?;
    let hint_count = conn.query_row(
        "SELECT COUNT(*)
         FROM behavior_events
         WHERE session_id=?1 AND concept_id=?2 AND type='hint'
           AND julianday(at) >= julianday(?3)
           AND julianday(at) <= julianday(?4)",
        (session, concept, last_next_at.as_str(), now),
        |row| row.get(0),
    )?;

    Ok(BehaviorObservation {
        latency_ms,
        hint_count,
    })
}

fn default_db_path() -> PathBuf {
    if let Some(path) = std::env::var_os("POLARIS_CORE_DB") {
        return PathBuf::from(path);
    }

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".polaris-core").join("core.db")
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_required_command_set() {
        for args in [
            vec!["polaris", "init", "--pack", "packs/rust"],
            vec!["polaris", "ingest", "--text", "hello"],
            vec![
                "polaris",
                "ingest",
                "--adapter-command",
                "adapter.exe",
                "--adapter-arg",
                "--jsonl",
            ],
            vec!["polaris", "next", "--session", "cli"],
            vec![
                "polaris",
                "submit",
                "--concept",
                "ownership",
                "--response",
                "Ownership moves values.",
                "--confidence",
                "4",
            ],
            vec!["polaris", "hint", "--concept", "ownership"],
            vec!["polaris", "abandon", "--concept", "ownership"],
            vec!["polaris", "status"],
            vec!["polaris", "status", "--json"],
            vec!["polaris", "backup", "--output", "backup.db"],
            vec!["polaris", "doctor"],
            vec!["polaris", "doctor", "--json"],
            vec![
                "polaris",
                "serve-http",
                "--host",
                "127.0.0.1",
                "--port",
                "0",
            ],
            vec!["polaris", "grade-pending"],
            vec!["polaris", "diagnose", "--concept", "ownership"],
            vec!["polaris", "mcp"],
            vec!["polaris", "pack", "validate", "packs/rust"],
            vec!["polaris", "privacy", "show"],
            vec!["polaris", "privacy", "show", "--json"],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
    }

    #[test]
    fn report_narrative_flag_parses_explicit_tier1_request() {
        let cli = Cli::try_parse_from(vec!["polaris", "report", "--narrative"]).unwrap();

        assert!(matches!(cli.command, Commands::Report { narrative: true }));
    }

    #[test]
    fn privacy_show_parses_and_reports_tier0_state() {
        let cli = Cli::try_parse_from(vec!["polaris", "privacy", "show"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::Privacy {
                command: PrivacyCommands::Show { json: false }
            }
        ));

        let text = privacy_show_text(&polaris_core::privacy::PrivacyCallInventory::all(), true);
        assert!(text.contains("Tier 0 only 模式：启用"));
        assert!(text.contains("POLARIS_TIER0_ONLY=1"));
    }

    #[test]
    fn privacy_show_json_flag_parses() {
        let cli = Cli::try_parse_from(vec!["polaris", "privacy", "show", "--json"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::Privacy {
                command: PrivacyCommands::Show { json: true }
            }
        ));
    }

    #[test]
    fn behavior_observation_reads_latency_and_hint_count_since_last_next() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        polaris_core::db::migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES ('next1', 's1', '2026-06-11T00:00:00Z', 'next', 'ownership', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES ('hint1', 's1', '2026-06-11T00:00:02Z', 'hint', 'ownership', '{}')",
            [],
        )
        .unwrap();

        let observation =
            read_behavior_observation_at(&conn, "s1", "ownership", "2026-06-11T00:00:05Z").unwrap();

        assert_eq!(observation.latency_ms, 5000);
        assert_eq!(observation.hint_count, 1);
    }

    #[test]
    fn status_json_serializes_desktop_mirror_fields() {
        let snapshot = polaris_core::status::StatusSnapshot {
            generated_at: "2026-06-13T00:00:00Z".to_owned(),
            due_today: 2,
            phase_counts: vec![polaris_core::status::PhaseCount {
                phase: "phantom".to_owned(),
                count: 1,
            }],
            concepts: vec![polaris_core::status::ConceptStatus {
                concept_id: "ownership".to_owned(),
                name: "Ownership".to_owned(),
                retrieval: Some(0.87),
                p_known: 0.42,
                calib_gap: 0.31,
                phase: "phantom".to_owned(),
            }],
        };

        let text = status_snapshot_json(&snapshot).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert_eq!(payload["generated_at"], "2026-06-13T00:00:00Z");
        assert_eq!(payload["due_today"], 2);
        assert_eq!(payload["phase_counts"][0]["phase"], "phantom");
        assert_eq!(payload["phase_counts"][0]["count"], 1);
        assert_eq!(payload["concepts"][0]["concept_id"], "ownership");
        assert_eq!(payload["concepts"][0]["phase"], "phantom");
    }

    #[test]
    fn status_text_keeps_existing_cli_shape() {
        let snapshot = polaris_core::status::StatusSnapshot {
            generated_at: "2026-06-13T00:00:00Z".to_owned(),
            due_today: 2,
            phase_counts: Vec::new(),
            concepts: vec![polaris_core::status::ConceptStatus {
                concept_id: "ownership".to_owned(),
                name: "Ownership".to_owned(),
                retrieval: None,
                p_known: 0.42,
                calib_gap: 0.31,
                phase: "phantom".to_owned(),
            }],
        };

        let text = status_snapshot_text(&snapshot);

        assert_eq!(
            text,
            "due_today=2\nownership\tOwnership\tR=-\tp_known=0.420\tcalib_gap=0.310\tphase=phantom\n"
        );
    }

    #[test]
    fn adapter_jsonl_ingests_evidence_and_attempt_without_trusting_external_score() {
        let _guard = EnvGuard::remove(llm_env_keys());
        let mut engine = in_memory_engine_with_rust_pack();
        let jsonl = r#"
{"type":"evidence","session":"s-adapter","source":"browser-fixture","content_type":"text/plain","text":"Ownership moves values unless borrowed.","concept_ids":["ownership"],"external_score":1.0}
{"type":"attempt","session":"s-adapter","concept_id":"ownership","task_type":"recall","prompt":"Explain ownership.","response":"Ownership moves values.","confidence":4,"latency_ms":1200,"hint_count":1,"final_score":1.0,"external_score":1.0}
"#;

        let summary = ingest_adapter_jsonl(&mut engine, jsonl).unwrap();

        assert_eq!(summary.evidence, 1);
        assert_eq!(summary.attempts, 1);
        let evidence_count: i64 = engine
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM evidence_items WHERE source='browser-fixture'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(evidence_count, 1);
        let (attempts, final_score): (i64, Option<f64>) = engine
            .conn()
            .query_row(
                "SELECT COUNT(*), MAX(final_score) FROM attempts WHERE concept_id='ownership'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(attempts, 1);
        assert_eq!(final_score, None);
    }

    #[test]
    fn adapter_jsonl_rejects_unknown_event_types() {
        let mut engine = in_memory_engine_with_rust_pack();
        let error = ingest_adapter_jsonl(
            &mut engine,
            r#"
{"type":"evidence","session":"s-adapter","source":"browser-fixture","content_type":"text/plain","text":"This line must roll back."}
{"type":"mastery","concept_id":"ownership","p_known":1.0}
"#,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unsupported adapter event type"));
        let evidence_count: i64 = engine
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM evidence_items WHERE source='browser-fixture'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(evidence_count, 0);
        let mastery_count: i64 = engine
            .conn()
            .query_row("SELECT COUNT(*) FROM mastery_states", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mastery_count, 0);
    }

    #[test]
    fn diagnose_does_not_create_missing_database() {
        let path = std::env::temp_dir().join(format!(
            "polaris-core-diagnose-readonly-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            path.to_str().unwrap(),
            "diagnose",
            "--concept",
            "ownership",
        ])
        .unwrap();

        let result = run(cli);

        assert!(result.is_err());
        assert!(!path.exists(), "diagnose must not create a missing db");
    }

    #[test]
    fn backup_and_doctor_helpers_create_backup_and_report() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("polaris-core-backup-src-{suffix}.db"));
        let backup_path =
            std::env::temp_dir().join(format!("polaris-core-backup-copy-{suffix}.db"));
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(&backup_path);

        let conn = polaris_core::db::open_database(&db_path).unwrap();
        conn.execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
             VALUES ('e1', 's1', 'cli-test', 'text/plain', 'backup smoke', '[]', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        backup_database(&conn, &backup_path).unwrap();
        let backup_conn = rusqlite::Connection::open(&backup_path).unwrap();
        let evidence_count: i64 = backup_conn
            .query_row("SELECT COUNT(*) FROM evidence_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(evidence_count, 1);

        let report = polaris_core::ops::doctor_report(&conn).unwrap();
        assert!(report.ok);
        let text = doctor_report_text(&report);
        assert!(text.contains("integrity=ok"));
        assert!(text.contains("replay_checked=0"));

        drop(backup_conn);
        drop(conn);
        let _ = std::fs::remove_file(&backup_path);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn backup_rejects_missing_source_without_creating_it() {
        let path = std::env::temp_dir().join(format!(
            "polaris-core-missing-source-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);

        let error = open_existing_database(&path).unwrap_err().to_string();

        assert!(error.contains("database does not exist"));
        assert!(!path.exists(), "missing backup source must not be created");
    }

    #[test]
    fn backup_rejects_existing_output_without_overwriting() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("polaris-core-existing-src-{suffix}.db"));
        let backup_path =
            std::env::temp_dir().join(format!("polaris-core-existing-output-{suffix}.db"));
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(&backup_path);
        std::fs::write(&backup_path, b"keep me").unwrap();
        let conn = polaris_core::db::open_database(&db_path).unwrap();

        let error = backup_database(&conn, &backup_path)
            .unwrap_err()
            .to_string();

        assert!(error.contains("backup output already exists"));
        assert_eq!(std::fs::read(&backup_path).unwrap(), b"keep me");

        drop(conn);
        let _ = std::fs::remove_file(&backup_path);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    fn in_memory_engine_with_rust_pack() -> Engine {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        polaris_core::db::migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
        engine
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
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
}
