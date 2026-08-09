use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, Subcommand, ValueEnum};
use polaris_core::ai_profile::{AiInteractionProfile, AiInteractionProfileInput};
use polaris_core::capture_queue::{CaptureInput, CaptureRecord, CaptureStatus, LearnerCaptureKind};
use polaris_core::config::{
    parameter_specs, parameters_markdown, ParameterClass, ParameterSpec, TuningRoute,
};
use polaris_core::db::{
    open_database, open_database_read_only, schema_version, CURRENT_SCHEMA_VERSION,
};
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::error::PolarisError;
use polaris_core::inbox_practice::{
    InboxPracticeDraft, InboxPracticeSubmissionInput, InboxPracticeSubmissionReceipt,
};
use polaris_core::learner_feedback::{LearnerFeedbackInput, LearnerFeedbackReceipt};
use polaris_core::learner_inbox::{
    LearnerInboxAction, LearnerInboxActionReceipt, LearnerInboxItem,
};
use polaris_core::learner_mirror::LearnerMirrorSnapshot;
use polaris_core::ops::{
    doctor_diagnostics, doctor_report, ActivitySummary, DoctorDiagnostics, DoctorReport,
};
use polaris_core::pack::validate_pack_path;
use polaris_core::pack_state::{PackSummary, PackSwitchReceipt, ThetaMode};
use polaris_core::privacy::PrivacyCallInventory;
use polaris_core::profile::{
    delete_all_learning_data, profile_instruments, FullDataDeletionRequest,
    ProfileMeasurementInput, ProfileSettingsUpdate,
};
use polaris_core::project_manifest::{
    discover_learning_projects, discover_project_manifest, DiscoveredProjectManifest,
};
use polaris_core::sandbox::{run_pack_sandbox, SandboxLearner, SandboxOptions, SandboxReport};
use polaris_core::session::SessionCloseSummary;
use polaris_core::status::StatusSnapshot;
use polaris_core::trust::{TrustPanel, TrustParameter};
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
    Capture {
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "paste")]
        source: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, default_value = "text/plain")]
        content_type: String,
        #[arg(long = "learner-kind", default_value = "reference")]
        learner_kind: String,
        #[arg(long = "candidate-concept")]
        candidate_concepts: Vec<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Inbox {
        #[command(subcommand)]
        command: InboxCommands,
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
        #[arg(long)]
        no_attempt_reason: Option<String>,
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
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
    LearnerMirror {
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
        #[arg(long)]
        diagnose: bool,
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
    FsrsFit {
        #[arg(long)]
        json: bool,
    },
    MentalFit,
    ReportFeedback {
        #[arg(long)]
        assertion: String,
        #[arg(long)]
        report: Option<String>,
        #[arg(long, default_value = "inaccurate")]
        verdict: String,
    },
    Diagnose {
        #[arg(long)]
        concept: String,
    },
    Mcp,
    Feedback {
        #[command(subcommand)]
        command: FeedbackCommands,
    },
    Pack {
        #[command(subcommand)]
        command: PackCommands,
    },
    Privacy {
        #[command(subcommand)]
        command: PrivacyCommands,
    },
    Trust {
        #[command(subcommand)]
        command: TrustCommands,
    },
    AiProfile {
        #[command(subcommand)]
        command: AiProfileCommands,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommands {
    Close {
        #[arg(long)]
        session: String,
        #[arg(long)]
        json: bool,
    },
    Show {
        #[arg(long)]
        session: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum FeedbackCommands {
    State {
        #[arg(long)]
        state: String,
        #[arg(long, default_value = "cli")]
        session: String,
        #[arg(long)]
        concept: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Pause {
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        session: String,
        #[arg(long)]
        concept: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum InboxCommands {
    List {
        #[arg(long = "status")]
        statuses: Vec<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Act {
        #[arg(long)]
        capture: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Practice {
        #[arg(long)]
        capture: String,
        #[arg(long)]
        json: bool,
    },
    Submit {
        #[arg(long)]
        capture: String,
        #[arg(long)]
        response: String,
        #[arg(long)]
        confidence: i32,
        #[arg(long, default_value = "cli")]
        session: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PackCommands {
    Validate {
        path: PathBuf,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Switch {
        pack: String,
        #[arg(long = "theta-mode", value_enum)]
        theta_mode: Option<ThetaModeArg>,
    },
    Sandbox {
        path: PathBuf,
        #[arg(long, value_enum, default_value = "mixed")]
        profile: SandboxProfileArg,
        #[arg(long, default_value_t = 7)]
        days: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SandboxProfileArg {
    Strong,
    Weak,
    Mixed,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ThetaModeArg {
    Shared,
    Isolated,
}

impl From<ThetaModeArg> for ThetaMode {
    fn from(value: ThetaModeArg) -> Self {
        match value {
            ThetaModeArg::Shared => Self::Shared,
            ThetaModeArg::Isolated => Self::Isolated,
        }
    }
}

#[derive(Debug, Subcommand)]
enum PrivacyCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum TrustCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AiProfileCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
    Set {
        #[arg(long)]
        persona: Option<String>,
        #[arg(long)]
        verbosity: Option<String>,
        #[arg(long = "explanation-depth")]
        explanation_depth: Option<String>,
        #[arg(long)]
        proactivity: Option<String>,
        #[arg(long = "intervention-frequency")]
        intervention_frequency: Option<String>,
        #[arg(long = "correction-style")]
        correction_style: Option<String>,
        #[arg(long = "custom-notes")]
        custom_notes: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ProfileCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
    Instruments {
        #[arg(long)]
        json: bool,
    },
    Set {
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long = "acknowledge-disclosure")]
        acknowledge_disclosure: bool,
        #[arg(long = "summary-sharing")]
        summary_sharing_enabled: Option<bool>,
        #[arg(long = "pause-until")]
        paused_until: Option<String>,
        #[arg(long = "clear-pause")]
        clear_pause: bool,
        #[arg(long)]
        json: bool,
    },
    Record {
        #[arg(long)]
        instrument: String,
        #[arg(long, default_value = "1.0")]
        version: String,
        #[arg(long)]
        item: String,
        #[arg(long)]
        response: i64,
        #[arg(long, default_value = "en")]
        locale: String,
        #[arg(long = "admin-mode", default_value = "ema_single_item")]
        admin_mode: String,
        #[arg(long, default_value = "cli")]
        session: String,
        #[arg(long)]
        json: bool,
    },
    Export {
        #[arg(long)]
        output: PathBuf,
    },
    Reset {
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    DeleteAll {
        #[arg(long)]
        confirm: String,
        #[arg(long)]
        backup: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    List {
        #[arg(long)]
        class: Option<String>,
        #[arg(long = "tuning-route")]
        tuning_route: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        md: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommands {
    Detect {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Scan {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long = "max-depth", default_value_t = 3)]
        max_depth: usize,
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
        Commands::Pack {
            command:
                PackCommands::Sandbox {
                    path,
                    profile,
                    days,
                    json,
                },
        } => {
            if cli.db.is_some() {
                return Err(adapter_error(
                    "pack sandbox uses an in-memory database and does not accept --db",
                ));
            }
            let reports = run_pack_sandbox_profiles(&path, profile, days)?;
            if json {
                println!("{}", sandbox_reports_json(&reports)?);
            } else {
                print!("{}", sandbox_reports_text(&reports));
            }
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
        Commands::Profile {
            command: ProfileCommands::Instruments { json },
        } => {
            let instruments = profile_instruments()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&instruments)?);
            } else {
                for instrument in instruments {
                    println!(
                        "{}\tversion={}\titems={}\tlicense={}",
                        instrument.id,
                        instrument.version,
                        instrument.items.len(),
                        instrument.license.name
                    );
                }
            }
        }
        Commands::Profile {
            command:
                ProfileCommands::DeleteAll {
                    confirm,
                    backup,
                    json,
                },
        } => {
            let db_path = cli.db.unwrap_or_else(default_db_path);
            let receipt = delete_all_learning_data(
                FullDataDeletionRequest {
                    database_path: db_path,
                    backup_path: backup,
                    confirmation: confirm,
                },
                || Ok(0),
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            } else {
                println!(
                    "全部学习数据已清空并建立空库：{}（删除旧文件 {} 个，本机密钥 {} 项）",
                    receipt.database_path, receipt.files_deleted, receipt.local_secrets_deleted
                );
                if let Some(path) = receipt.backup_path {
                    println!("备份保留：{path}");
                }
            }
        }
        Commands::Trust {
            command: TrustCommands::Show { json },
        } => {
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            let panel = engine.trust_panel()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&panel)?);
            } else {
                print!("{}", trust_show_text(&panel));
            }
        }
        Commands::AiProfile {
            command: AiProfileCommands::Show { json },
        } => {
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            let profile = engine.ai_interaction_profile()?;
            if json {
                println!("{}", ai_profile_json(&profile)?);
            } else {
                print!("{}", ai_profile_text(&profile));
            }
        }
        Commands::AiProfile {
            command:
                AiProfileCommands::Set {
                    persona,
                    verbosity,
                    explanation_depth,
                    proactivity,
                    intervention_frequency,
                    correction_style,
                    custom_notes,
                    json,
                },
        } => {
            let conn = open_existing_database(&cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            let profile = engine.update_ai_interaction_profile(AiInteractionProfileInput {
                persona,
                verbosity,
                explanation_depth,
                proactivity,
                intervention_frequency,
                correction_style,
                custom_notes,
            })?;
            if json {
                println!("{}", ai_profile_json(&profile)?);
            } else {
                print!("{}", ai_profile_text(&profile));
            }
        }
        Commands::Config {
            command:
                ConfigCommands::List {
                    class,
                    tuning_route,
                    json,
                    md,
                },
        } => {
            if json && md {
                return Err(adapter_error(
                    "config list accepts either --json or --md, not both",
                ));
            }
            let class = parse_parameter_class(class.as_deref())?;
            let tuning_route = parse_tuning_route(tuning_route.as_deref())?;
            let specs = parameter_specs(class, tuning_route);
            if json {
                println!("{}", config_list_json(&specs)?);
            } else if md {
                print!("{}", config_list_markdown(&specs));
            } else {
                print!("{}", config_list_text(&specs));
            }
        }
        Commands::Project { command } => match command {
            ProjectCommands::Detect { path, json } => {
                let discovered = discover_project_manifest(path)?.ok_or_else(|| {
                    adapter_error("no p-os.toml found from the given path or its parents")
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&discovered)?);
                } else {
                    print!("{}", project_detect_text(&discovered));
                }
            }
            ProjectCommands::Scan {
                root,
                max_depth,
                json,
            } => {
                let projects = discover_learning_projects(&root, max_depth)?;
                if json {
                    println!("{}", project_scan_json(&root, &projects)?);
                } else {
                    print!("{}", project_scan_text(&root, &projects));
                }
            }
        },
        Commands::Diagnose { concept } => {
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            print_diagnosis(engine.diagnose_concept(&concept)?);
        }
        Commands::LearnerMirror { json } => {
            if !json {
                return Err(adapter_error("learner-mirror currently requires --json"));
            }
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let engine = Engine::new(conn);
            println!(
                "{}",
                learner_mirror_json(&engine.learner_mirror_snapshot()?)?
            );
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
        Commands::Doctor { json, diagnose } => {
            let conn = open_database_read_only(cli.db.unwrap_or_else(default_db_path))?;
            let report = doctor_report(&conn)?;
            if diagnose {
                let diagnostics = doctor_diagnostics(&conn, 7)?;
                if json {
                    println!("{}", doctor_diagnose_json(&report, &diagnostics)?);
                } else {
                    print!("{}", doctor_report_text(&report));
                    print!("{}", doctor_diagnostics_text(&diagnostics));
                }
            } else if json {
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
                Commands::Capture {
                    text,
                    source,
                    session,
                    content_type,
                    learner_kind,
                    candidate_concepts,
                    note,
                    json,
                } => {
                    let record = engine.capture_learning_evidence(CaptureInput {
                        session_id: session,
                        source,
                        content_type,
                        text,
                        learner_kind: parse_learner_capture_kind(&learner_kind)?,
                        candidate_concept_ids: candidate_concepts,
                        note,
                    })?;
                    if json {
                        println!("{}", capture_record_json(&record)?);
                    } else {
                        print!("{}", capture_record_text(&record));
                    }
                }
                Commands::Inbox { command } => match command {
                    InboxCommands::List {
                        statuses,
                        limit,
                        json,
                    } => {
                        let statuses = parse_capture_statuses(&statuses)?;
                        let items = engine.learner_inbox(&statuses, limit)?;
                        if json {
                            println!("{}", learner_inbox_json(&items)?);
                        } else {
                            print!("{}", learner_inbox_text(&items));
                        }
                    }
                    InboxCommands::Act {
                        capture,
                        action,
                        note,
                        json,
                    } => {
                        let receipt = engine.act_on_learner_inbox_item(
                            &capture,
                            parse_learner_inbox_action(&action)?,
                            note,
                        )?;
                        if json {
                            println!("{}", learner_inbox_receipt_json(&receipt)?);
                        } else {
                            print!("{}", learner_inbox_receipt_text(&receipt));
                        }
                    }
                    InboxCommands::Practice { capture, json } => {
                        let draft = engine.draft_inbox_practice(&capture)?;
                        if json {
                            println!("{}", inbox_practice_draft_json(&draft)?);
                        } else {
                            print!("{}", inbox_practice_draft_text(&draft));
                        }
                    }
                    InboxCommands::Submit {
                        capture,
                        response,
                        confidence,
                        session,
                        json,
                    } => {
                        let draft = engine.draft_inbox_practice(&capture)?;
                        let observation = read_behavior_observation_now(
                            engine.conn(),
                            session.as_str(),
                            draft.concept_id.as_str(),
                        )?;
                        let receipt =
                            engine.submit_inbox_practice(InboxPracticeSubmissionInput {
                                capture_id: capture,
                                session_id: session,
                                response_text: response,
                                self_confidence: confidence,
                                latency_ms: observation.latency_ms,
                                hint_count: observation.hint_count,
                            })?;
                        if json {
                            println!("{}", inbox_practice_receipt_json(&receipt)?);
                        } else {
                            print!("{}", inbox_practice_receipt_text(&receipt));
                        }
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
                    no_attempt_reason,
                } => {
                    let observation =
                        read_behavior_observation_now(engine.conn(), &session, &concept)?;
                    let input = SubmitInput {
                        session_id: session,
                        concept_id: concept,
                        task_type,
                        prompt_text: prompt,
                        response_text: response,
                        self_confidence: confidence,
                        latency_ms: observation.latency_ms,
                        hint_count: observation.hint_count,
                    };
                    if let Some(reason) = no_attempt_reason {
                        let receipt = engine.submit_no_attempt(input, &reason)?;
                        println!(
                            "attempt: {} no_attempt_reason={}",
                            receipt.attempt_id,
                            receipt.no_attempt_reason.as_str()
                        );
                    } else {
                        let receipt = engine.submit(input)?;
                        println!(
                            "attempt: {} provisional_score={:.3} degraded={}",
                            receipt.attempt_id, receipt.provisional_score, receipt.degraded
                        );
                    }
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
                Commands::Session { command } => {
                    let summary = match &command {
                        SessionCommands::Close { session, .. } => engine.close_session(session)?,
                        SessionCommands::Show { session, .. } => {
                            engine.session_close_summary(session)?.ok_or_else(|| {
                                adapter_error(format!("session is not closed: {session}"))
                            })?
                        }
                    };
                    let json = match &command {
                        SessionCommands::Close { json, .. }
                        | SessionCommands::Show { json, .. } => *json,
                    };
                    if json {
                        println!("{}", serde_json::to_string_pretty(&summary)?);
                    } else {
                        print!("{}", session_close_summary_text(&summary));
                    }
                }
                Commands::Status { json } => {
                    let snapshot = engine.status_snapshot()?;
                    if json {
                        println!("{}", status_snapshot_json(&snapshot)?);
                    } else {
                        print_status_snapshot(&snapshot);
                    }
                }
                Commands::Profile { command } => match command {
                    ProfileCommands::Show { json } => {
                        let overview = engine.global_profile_overview()?;
                        if json {
                            println!("{}", serde_json::to_string_pretty(&overview)?);
                        } else {
                            print!("{}", profile_overview_text(&overview));
                        }
                    }
                    ProfileCommands::Set {
                        enabled,
                        acknowledge_disclosure,
                        summary_sharing_enabled,
                        paused_until,
                        clear_pause,
                        json,
                    } => {
                        let settings =
                            engine.update_global_profile_settings(ProfileSettingsUpdate {
                                enabled,
                                acknowledge_disclosure,
                                summary_sharing_enabled,
                                paused_until,
                                clear_pause,
                            })?;
                        if json {
                            println!("{}", serde_json::to_string_pretty(&settings)?);
                        } else {
                            print!("{}", profile_settings_text(&settings));
                        }
                    }
                    ProfileCommands::Record {
                        instrument,
                        version,
                        item,
                        response,
                        locale,
                        admin_mode,
                        session,
                        json,
                    } => {
                        let receipt =
                            engine.record_profile_measurement(ProfileMeasurementInput {
                                session_id: session,
                                instrument_id: instrument,
                                instrument_version: version,
                                item_id: item,
                                locale,
                                admin_mode,
                                response,
                            })?;
                        if json {
                            println!("{}", serde_json::to_string_pretty(&receipt)?);
                        } else {
                            println!("{}", receipt.message);
                        }
                    }
                    ProfileCommands::Export { output } => {
                        if output.exists() {
                            return Err(adapter_error(format!(
                                "profile export output already exists: {}",
                                output.display()
                            )));
                        }
                        if let Some(parent) = output.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        let export = engine.export_global_profile()?;
                        std::fs::write(&output, serde_json::to_vec_pretty(&export)?)?;
                        println!("画像数据已导出：{}", output.display());
                    }
                    ProfileCommands::Reset { yes, json } => {
                        if !yes {
                            return Err(adapter_error(
                                "profile reset deletes answers and derived profile data; pass --yes to continue",
                            ));
                        }
                        let receipt = engine.reset_global_profile()?;
                        if json {
                            println!("{}", serde_json::to_string_pretty(&receipt)?);
                        } else {
                            println!(
                                "画像已重置：回答 {}、维度 {}、验证 {}；保留学习尝试 {}。",
                                receipt.measurements_deleted,
                                receipt.dimensions_deleted,
                                receipt.validation_runs_deleted,
                                receipt.learning_attempts_preserved
                            );
                        }
                    }
                    ProfileCommands::Instruments { .. } => {
                        unreachable!("handled before database open")
                    }
                    ProfileCommands::DeleteAll { .. } => {
                        unreachable!("handled before database open")
                    }
                },
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
                Commands::FsrsFit { json } => {
                    let summary = engine.fit_fsrs_personal_params()?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&summary)?);
                    } else {
                        print!("{}", fsrs_fit_text(&summary));
                    }
                }
                Commands::ReportFeedback {
                    assertion,
                    report,
                    verdict,
                } => {
                    let verdict = verdict.trim().to_ascii_lowercase();
                    let report_id = engine.record_report_feedback_with_verdict(
                        report.as_deref(),
                        &assertion,
                        &verdict,
                    )?;
                    println!(
                        "recorded report feedback: report={report_id} assertion={assertion} verdict={verdict}"
                    );
                    if verdict == "inaccurate" {
                        println!(
                            "effect=recorded_only; inaccurate assertions are suppressed in the report window."
                        );
                    } else {
                        println!(
                            "effect=recorded_only; accurate feedback does not directly change mastery or scheduling."
                        );
                    }
                }
                Commands::Feedback { command } => {
                    let (input, json) = match command {
                        FeedbackCommands::State {
                            state,
                            session,
                            concept,
                            note,
                            json,
                        } => (
                            LearnerFeedbackInput {
                                session_id: session,
                                source: "cli".to_owned(),
                                kind: "state".to_owned(),
                                concept_id: concept,
                                state: Some(state),
                                reason: None,
                                note,
                            },
                            json,
                        ),
                        FeedbackCommands::Pause {
                            reason,
                            session,
                            concept,
                            note,
                            json,
                        } => (
                            LearnerFeedbackInput {
                                session_id: session,
                                source: "cli".to_owned(),
                                kind: "pause".to_owned(),
                                concept_id: concept,
                                state: None,
                                reason: Some(reason),
                                note,
                            },
                            json,
                        ),
                    };
                    let receipt = engine.record_learner_feedback(input)?;
                    if json {
                        println!("{}", learner_feedback_json(&receipt)?);
                    } else {
                        print!("{}", learner_feedback_text(&receipt));
                    }
                }
                Commands::Pack { command } => match command {
                    PackCommands::List { json } => {
                        let packs = engine.list_packs()?;
                        if json {
                            println!("{}", pack_list_json(&packs)?);
                        } else {
                            print!("{}", pack_list_text(&packs));
                        }
                    }
                    PackCommands::Switch { pack, theta_mode } => {
                        let receipt = engine.switch_pack(&pack, theta_mode.map(Into::into))?;
                        print!("{}", pack_switch_text(&receipt));
                    }
                    PackCommands::Validate { .. } => {
                        unreachable!("handled before database open")
                    }
                    PackCommands::Sandbox { .. } => {
                        unreachable!("handled before database open")
                    }
                },
                Commands::Diagnose { .. } => unreachable!("handled before writable database open"),
                Commands::LearnerMirror { .. } => {
                    unreachable!("handled before writable database open")
                }
                Commands::Mcp => unreachable!("handled before command dispatch"),
                Commands::Backup { .. } => unreachable!("handled before writable database open"),
                Commands::Doctor { .. } => unreachable!("handled before writable database open"),
                Commands::Privacy { .. } => unreachable!("handled before database open"),
                Commands::Trust { .. } => unreachable!("handled before writable database open"),
                Commands::AiProfile { .. } => unreachable!("handled before writable database open"),
                Commands::Config { .. } => unreachable!("handled before database open"),
                Commands::Project { .. } => unreachable!("handled before database open"),
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

fn parse_learner_capture_kind(
    value: &str,
) -> Result<LearnerCaptureKind, Box<dyn std::error::Error>> {
    LearnerCaptureKind::parse(value).ok_or_else(|| {
        adapter_error(
            "capture --learner-kind must be one of reference, own_answer, error_log, code_change, chat_excerpt, unknown",
        )
    })
}

fn parse_capture_statuses(
    values: &[String],
) -> Result<Vec<CaptureStatus>, Box<dyn std::error::Error>> {
    values
        .iter()
        .map(|value| {
            CaptureStatus::parse(value).ok_or_else(|| {
                adapter_error(
                    "inbox --status must be one of pending, mapped, practice_ready, practiced, ignored, archived",
                )
            })
        })
        .collect()
}

fn parse_learner_inbox_action(
    value: &str,
) -> Result<LearnerInboxAction, Box<dyn std::error::Error>> {
    LearnerInboxAction::parse(value).ok_or_else(|| {
        adapter_error("inbox --action must be one of accept, defer, ignore, archive")
    })
}

fn open_existing_database(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(adapter_error(format!(
            "database does not exist: {}",
            path.display()
        )));
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )?;
    let user_version = schema_version(&conn)?;
    if user_version > CURRENT_SCHEMA_VERSION {
        return Err(Box::new(PolarisError::UnsupportedSchemaVersion {
            found: user_version,
            current: CURRENT_SCHEMA_VERSION,
        }));
    }
    Ok(conn)
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

fn profile_settings_text(settings: &polaris_core::profile::ProfileSettings) -> String {
    format!(
        "本地画像={}\n首次说明={}\n本地集成摘要分享={}\n暂停至={}\n",
        if settings.enabled { "启用" } else { "关闭" },
        if settings.disclosure_required {
            "待确认"
        } else {
            "已确认"
        },
        if settings.summary_sharing_enabled {
            "启用"
        } else {
            "关闭"
        },
        settings.paused_until.as_deref().unwrap_or("未暂停")
    )
}

fn profile_overview_text(overview: &polaris_core::profile::GlobalProfileOverview) -> String {
    let mut text = profile_settings_text(&overview.settings);
    text.push_str(&format!(
        "量表={}\n已记录回答={}\n派生维度={}\n验证运行={}\n",
        overview.instrument_count,
        overview.measurement_count,
        overview.dimensions.len(),
        overview.validation_runs.len(),
    ));
    text
}

fn doctor_report_text(report: &DoctorReport) -> String {
    let mut text = String::new();
    text.push_str(&format!("ok={}\n", report.ok));
    text.push_str(&format!("schema_version={}\n", report.schema_version));
    text.push_str(&format!("migration_count={}\n", report.migration_count));
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

fn doctor_diagnose_json(
    report: &DoctorReport,
    diagnostics: &DoctorDiagnostics,
) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "doctor": report,
        "diagnostics": diagnostics,
    }))
}

fn doctor_diagnostics_text(diagnostics: &DoctorDiagnostics) -> String {
    let mut text = format!("\ndiagnostics_window_days={}\n", diagnostics.window_days);
    for (label, summary) in [
        ("param_tuning_runs", &diagnostics.param_tuning_runs),
        ("breeding.evaluated_7d", &diagnostics.breeding_evaluated_7d),
        ("breeding.admitted_7d", &diagnostics.breeding_admitted_7d),
        ("breeding.retired_7d", &diagnostics.breeding_retired_7d),
        ("mental_fit.hazard", &diagnostics.mental_fit_hazard),
        ("mental_fit.state_gate", &diagnostics.mental_fit_state_gate),
        ("gu_inductions", &diagnostics.gu_inductions),
        ("consolidation_runs", &diagnostics.consolidation_runs),
        ("mirror_reports", &diagnostics.mirror_reports),
    ] {
        text.push_str(&activity_summary_text(label, summary));
    }
    text
}

fn activity_summary_text(label: &str, summary: &ActivitySummary) -> String {
    format!(
        "{label}\tcount_7d={}\tlast_at={}\tlast_status={}\n",
        summary.count_7d,
        summary.last_at.as_deref().unwrap_or("-"),
        summary.last_status.as_deref().unwrap_or("-")
    )
}

fn trust_show_text(panel: &TrustPanel) -> String {
    let mut text = String::new();
    text.push_str(&format!("generated_at={}\n", panel.generated_at));
    text.push_str(&format!("window_days={}\n", panel.window_days));
    text.push_str(&format!(
        "current_pack={}\n",
        panel.governance.current_pack_id.as_deref().unwrap_or("-")
    ));

    text.push_str("\ngates\n");
    for gate in &panel.gates {
        text.push_str(&format!(
            "{}\t{}\tstatus={}\tgate={}\tmetric={}\treason={}\n",
            gate.framework,
            gate.name,
            gate.status,
            gate.gate,
            gate.metric.as_deref().unwrap_or("-"),
            gate.reason
        ));
    }

    text.push_str("\nactive_breeding_experiments\n");
    if panel.active_breeding_experiments.is_empty() {
        text.push_str("-\n");
    } else {
        for experiment in &panel.active_breeding_experiments {
            text.push_str(&format!(
                "{}\t{}>{}\tstatus={}\tposterior={:.3}\tn={}/{}\tmin_n={}\tadmit_p={:.2}\tretire_p={:.2}\tcontext={}\thypothesis={}\n",
                experiment.id,
                experiment.candidate_move,
                experiment.incumbent_move,
                experiment.status,
                experiment.posterior_win_prob,
                experiment.n_candidate,
                experiment.n_incumbent,
                experiment.min_n,
                experiment.admit_p,
                experiment.retire_p,
                experiment.context_hash,
                experiment.main_effect_hypothesis
            ));
        }
    }

    text.push_str("\nactive_mrt_experiments\n");
    if panel.active_mrt_experiments.is_empty() {
        text.push_str("-\n");
    } else {
        for experiment in &panel.active_mrt_experiments {
            text.push_str(&format!(
                "{}\tmove={}\trandomized={}\tprereg_id={}\tcontext={}\twindow={}\thypothesis={}\tat={}\n",
                experiment.id,
                experiment.move_id,
                experiment.randomized,
                experiment.prereg_id,
                experiment.context_hash.as_deref().unwrap_or("-"),
                experiment.window.as_deref().unwrap_or("-"),
                experiment.main_effect_hypothesis.as_deref().unwrap_or("-"),
                experiment.at
            ));
        }
    }

    text.push_str("\nrecent_activity\n");
    text.push_str(&activity_summary_text(
        "param_tuning_runs",
        &panel.recent_activity.param_tuning_runs,
    ));
    text.push_str(&activity_summary_text(
        "breeding.evaluated_7d",
        &panel.recent_activity.breeding_evaluated_7d,
    ));
    text.push_str(&activity_summary_text(
        "breeding.admitted_7d",
        &panel.recent_activity.breeding_admitted_7d,
    ));
    text.push_str(&activity_summary_text(
        "breeding.retired_7d",
        &panel.recent_activity.breeding_retired_7d,
    ));
    text.push_str(&activity_summary_text(
        "mental_fit.hazard",
        &panel.recent_activity.mental_fit_hazard,
    ));
    text.push_str(&activity_summary_text(
        "mental_fit.state_gate",
        &panel.recent_activity.mental_fit_state_gate,
    ));
    text.push_str(&activity_summary_text(
        "gu_inductions",
        &panel.recent_activity.gu_inductions,
    ));
    text.push_str(&activity_summary_text(
        "nightly_consolidation",
        &panel.recent_activity.nightly_consolidation,
    ));
    text.push_str(&activity_summary_text(
        "mirror_reports",
        &panel.recent_activity.mirror_reports,
    ));

    text.push_str("\ngovernance\n");
    for parameter in [
        &panel.governance.breeding_admit_p,
        &panel.governance.breeding_retire_p,
        &panel.governance.breeding_min_n,
    ] {
        text.push_str(&trust_parameter_text(parameter));
    }
    text
}

fn trust_parameter_text(parameter: &TrustParameter) -> String {
    format!(
        "{}\tcurrent={}\tdefault={}\tclass={}\tbounds={}\ttuning_route={}\tgovernance_gate={}\n",
        parameter.key,
        parameter.current_value,
        parameter.default_value,
        parameter.class,
        parameter.bounds.as_deref().unwrap_or("-"),
        parameter.tuning_route,
        parameter.is_governance_gate
    )
}

fn fsrs_fit_text(summary: &polaris_core::fsrs_fit::FsrsFitSummary) -> String {
    if summary.status == polaris_core::fsrs_fit::FsrsFitStatus::Skipped {
        return format!(
            "skipped {}\treason={}\tfinal_attempts={}\n",
            summary.param,
            summary.reason.as_deref().unwrap_or("-"),
            summary.total_final_attempts
        );
    }
    if summary.accepted {
        return format!(
            "accepted {}: {} -> {} ({} improvement {:+.4}, train_predictions={}, holdout_predictions={}, replayed_concepts={})\n",
            summary.param,
            summary.old_value,
            summary.new_value,
            summary.metric,
            summary.delta,
            summary.train_predictions,
            summary.holdout_predictions,
            summary.replayed_concepts,
        );
    }
    format!(
        "rejected {}: kept={} candidate={} ({} improvement {:+.4}, train_predictions={}, holdout_predictions={}, replayed_concepts={})\n",
        summary.param,
        summary.old_value,
        summary.new_value,
        summary.metric,
        summary.delta,
        summary.train_predictions,
        summary.holdout_predictions,
        summary.replayed_concepts,
    )
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

fn ai_profile_json(profile: &AiInteractionProfile) -> serde_json::Result<String> {
    serde_json::to_string_pretty(profile)
}

fn ai_profile_text(profile: &AiInteractionProfile) -> String {
    format!(
        "AI 交互偏好\npersona: {}\nverbosity: {}\nexplanation_depth: {}\nproactivity: {}\nintervention_frequency: {}\ncorrection_style: {}\ncustom_notes: {}\nguidance: {}\n",
        profile.persona,
        profile.verbosity,
        profile.explanation_depth,
        profile.proactivity,
        profile.intervention_frequency,
        profile.correction_style,
        profile.custom_notes.as_deref().unwrap_or("-"),
        profile.guidance
    )
}

fn parse_parameter_class(
    value: Option<&str>,
) -> Result<Option<ParameterClass>, Box<dyn std::error::Error>> {
    value
        .map(|value| {
            ParameterClass::parse(value)
                .ok_or_else(|| adapter_error("config --class must be one of A, B, C"))
        })
        .transpose()
}

fn parse_tuning_route(
    value: Option<&str>,
) -> Result<Option<TuningRoute>, Box<dyn std::error::Error>> {
    value
        .map(|value| {
            TuningRoute::parse(value).ok_or_else(|| {
                adapter_error("config --tuning-route must be one of Replay, Mrt, Manual, Fit")
            })
        })
        .transpose()
}

fn config_list_text(specs: &[ParameterSpec]) -> String {
    let mut text = String::from("key\tdefault\tclass\tbounds\ttuning_route\n");
    for spec in specs {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            spec.key,
            spec.default_value,
            spec.class.as_str(),
            spec.bounds.unwrap_or("-"),
            spec.tuning_route.as_str()
        ));
    }
    text
}

fn config_list_json(specs: &[ParameterSpec]) -> serde_json::Result<String> {
    serde_json::to_string_pretty(specs)
}

fn config_list_markdown(specs: &[ParameterSpec]) -> String {
    parameters_markdown(specs)
}

fn status_snapshot_json(snapshot: &StatusSnapshot) -> serde_json::Result<String> {
    serde_json::to_string_pretty(snapshot)
}

fn session_close_summary_text(summary: &SessionCloseSummary) -> String {
    let mut text = format!(
        "会话 {} 已收口：{} 次作答，触及 {} 个概念。\n",
        summary.session_id,
        summary.attempts_count,
        summary.concepts_touched.len()
    );
    if let Some(concept) = &summary.top_stuck_concept_id {
        text.push_str(&format!("最需要补缺：{concept}\n"));
    }
    if let Some(concept) = &summary.next_entry_concept_id {
        text.push_str(&format!("下次从这里接：{concept}\n"));
    }
    for assertion in &summary.assertions {
        text.push_str(&format!("- {}\n", assertion.text));
    }
    text
}

fn pack_list_json(packs: &[PackSummary]) -> serde_json::Result<String> {
    serde_json::to_string_pretty(packs)
}

fn pack_list_text(packs: &[PackSummary]) -> String {
    if packs.is_empty() {
        return "no packs installed\n".to_owned();
    }

    let mut text = String::new();
    for pack in packs {
        let marker = if pack.active { "*" } else { " " };
        text.push_str(&format!(
            "{marker} {}\t{}\tconcepts={}\ttheta_mode={}\n",
            pack.id, pack.title, pack.concept_count, pack.theta_mode
        ));
    }
    text
}

fn pack_switch_text(receipt: &PackSwitchReceipt) -> String {
    format!(
        "active_pack={}\ntheta_mode={}\n",
        receipt.active_pack, receipt.theta_mode
    )
}

fn run_pack_sandbox_profiles(
    path: &Path,
    profile: SandboxProfileArg,
    days: usize,
) -> Result<Vec<SandboxReport>, Box<dyn std::error::Error>> {
    let learners = sandbox_profile_learners(profile);
    let mut reports = Vec::with_capacity(learners.len());
    for learner in learners {
        reports.push(run_pack_sandbox(
            SandboxOptions::new(path)
                .with_learner(learner)
                .with_days(days),
        )?);
    }
    Ok(reports)
}

fn sandbox_profile_learners(profile: SandboxProfileArg) -> Vec<SandboxLearner> {
    match profile {
        SandboxProfileArg::Strong => vec![SandboxLearner::Strong],
        SandboxProfileArg::Weak => vec![SandboxLearner::Weak],
        SandboxProfileArg::Mixed => vec![SandboxLearner::Mixed],
        SandboxProfileArg::All => vec![
            SandboxLearner::Strong,
            SandboxLearner::Weak,
            SandboxLearner::Mixed,
        ],
    }
}

fn sandbox_reports_json(reports: &[SandboxReport]) -> serde_json::Result<String> {
    if let [report] = reports {
        serde_json::to_string_pretty(report)
    } else {
        serde_json::to_string_pretty(reports)
    }
}

fn sandbox_reports_text(reports: &[SandboxReport]) -> String {
    let mut text = String::new();
    for report in reports {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&format!(
            "sandbox status={}\n\
             pack={} title={}\n\
             profile={} days={} theta_mode={}\n\
             mode={} writes_user_db={} tier0_only={} llm_used={} score_source={}\n\
             validation: concepts={} prerequisites={} misconceptions={}\n\
             mean_p_known: {:.3} -> {:.3} slope={:.3}\n\
             calibration_gap: {:.3} -> {:.3} theta_cosine={:.3}\n\
             deadlock_days={:?} hmm_state_lock={} early_transfer_violations={}\n\
             final_phase_counts={}\n\
             note=virtual learner simulation; not a real learner mastery estimate\n\
             summary={}\n",
            report.status.as_str(),
            report.pack_id,
            report.pack_title,
            report.learner.as_str(),
            report.days,
            report.theta_mode,
            report.mode,
            report.writes_user_db,
            report.tier0_only,
            report.llm_used,
            report.score_source,
            report.validation.concept_count,
            report.validation.prerequisite_count,
            report.validation.misconception_count,
            report.initial_mean_p_known,
            report.final_mean_p_known,
            report.mean_p_known_slope,
            report.initial_abs_calib_gap,
            report.final_abs_calib_gap,
            report.final_theta_cosine,
            report.deadlock_days,
            report.hmm_state_lock,
            report.early_transfer_violations.len(),
            phase_counts_inline(report),
            report.summary
        ));
        for violation in &report.early_transfer_violations {
            text.push_str(&format!(
                "early_transfer: concept={} attempts={} phase={}\n",
                violation.concept_id, violation.attempt_count, violation.phase
            ));
        }
    }
    text
}

fn phase_counts_inline(report: &SandboxReport) -> String {
    serde_json::to_string(&report.final_phase_counts).unwrap_or_else(|_| "{}".to_owned())
}

fn learner_mirror_json(snapshot: &LearnerMirrorSnapshot) -> serde_json::Result<String> {
    serde_json::to_string_pretty(snapshot)
}

fn learner_feedback_json(receipt: &LearnerFeedbackReceipt) -> serde_json::Result<String> {
    serde_json::to_string_pretty(receipt)
}

fn learner_feedback_text(receipt: &LearnerFeedbackReceipt) -> String {
    let concept = receipt.concept_id.as_deref().unwrap_or("-");
    let state = receipt.state.as_deref().unwrap_or("-");
    let reason = receipt.reason.as_deref().unwrap_or("-");
    format!(
        "learner_feedback recorded: event_id={} kind={} session={} concept={} state={} reason={} effect={}\n\
         effect=recorded_only does not directly change mastery or scheduling.\n",
        receipt.event_id,
        receipt.kind,
        receipt.session_id,
        concept,
        state,
        reason,
        receipt.effect
    )
}

fn print_status_snapshot(snapshot: &StatusSnapshot) {
    print!("{}", status_snapshot_text(snapshot));
}

fn status_snapshot_text(snapshot: &StatusSnapshot) -> String {
    let current_pack = snapshot.current_pack.as_deref().unwrap_or("-");
    let theta_mode = snapshot.theta_mode.as_deref().unwrap_or("-");
    let mut text = format!("current_pack={current_pack}\ntheta_mode={theta_mode}\n");
    if snapshot.packs.is_empty() {
        text.push_str("packs=-\n");
    } else {
        let packs = snapshot
            .packs
            .iter()
            .map(|pack| {
                let marker = if pack.active { "*" } else { "" };
                format!(
                    "{}{}:{}:concepts={}",
                    marker, pack.id, pack.theta_mode, pack.concept_count
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        text.push_str(&format!("packs={packs}\n"));
    }
    text.push_str(&format!("due_today={}\n", snapshot.due_today));
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

fn capture_record_json(record: &CaptureRecord) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "capture_id": record.capture_id,
        "evidence_id": record.evidence_id,
        "status": record.status.as_str(),
        "learner_kind": record.learner_kind.as_str(),
        "recorded_only": record.effect == polaris_core::capture_queue::CaptureEffect::RecordedOnly,
        "message": record.message,
    }))
}

fn capture_record_text(record: &CaptureRecord) -> String {
    format!(
        "capture_id: {}\nevidence_id: {}\nstatus: {}\nlearner_kind: {}\nrecorded_only: {}\nmessage: {}\n",
        record.capture_id,
        record.evidence_id,
        record.status.as_str(),
        record.learner_kind.as_str(),
        record.effect == polaris_core::capture_queue::CaptureEffect::RecordedOnly,
        record.message
    )
}

fn learner_inbox_json(items: &[LearnerInboxItem]) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({ "items": items }))
}

fn learner_inbox_text(items: &[LearnerInboxItem]) -> String {
    if items.is_empty() {
        return "学习收件箱为空。\n".to_owned();
    }
    let mut text = format!("学习收件箱：{} 条\n", items.len());
    for (index, item) in items.iter().enumerate() {
        text.push_str(&format!(
            "{}. {} [{}]\n",
            index + 1,
            item.message,
            item.status.as_str()
        ));
        text.push_str(&format!("   capture_id: {}\n", item.capture_id));
        text.push_str(&format!("   摘要: {}\n", item.text_preview));
        if let Some(concept_hint) = &item.concept_hint {
            text.push_str(&format!("   可能相关: {}\n", concept_hint));
        }
        let actions = item
            .actions
            .iter()
            .map(|action| format!("{}({})", action.label, action.action.as_str()))
            .collect::<Vec<_>>()
            .join(" / ");
        if !actions.is_empty() {
            text.push_str(&format!("   可选: {actions}\n"));
        }
    }
    text
}

fn learner_inbox_receipt_json(receipt: &LearnerInboxActionReceipt) -> serde_json::Result<String> {
    serde_json::to_string_pretty(receipt)
}

fn learner_inbox_receipt_text(receipt: &LearnerInboxActionReceipt) -> String {
    format!(
        "capture_id: {}\nstatus: {}\neffect: {}\nmessage: {}\n",
        receipt.capture_id,
        receipt.status.as_str(),
        receipt.effect,
        receipt.message
    )
}

fn inbox_practice_draft_json(draft: &InboxPracticeDraft) -> serde_json::Result<String> {
    serde_json::to_string_pretty(draft)
}

fn inbox_practice_draft_text(draft: &InboxPracticeDraft) -> String {
    let concept_hint = draft.concept_hint.as_deref().unwrap_or("-");
    format!(
        "capture_id: {}\nevidence_id: {}\nstatus: {}\nconcept_hint: {}\ntask_type: {}\nprompt: {}\n",
        draft.capture_id,
        draft.evidence_id,
        draft.status.as_str(),
        concept_hint,
        draft.task_type,
        draft.prompt
    )
}

fn inbox_practice_receipt_json(
    receipt: &InboxPracticeSubmissionReceipt,
) -> serde_json::Result<String> {
    serde_json::to_string_pretty(receipt)
}

fn inbox_practice_receipt_text(receipt: &InboxPracticeSubmissionReceipt) -> String {
    format!(
        "capture_id: {}\nattempt_id: {}\nstatus: {}\neffect: {}\nprovisional_score: {:.3}\ndegraded: {}\nmessage: {}\n",
        receipt.capture_id,
        receipt.attempt_id,
        receipt.status.as_str(),
        receipt.effect,
        receipt.provisional_score,
        receipt.degraded,
        receipt.message
    )
}

fn project_detect_text(discovered: &DiscoveredProjectManifest) -> String {
    format!(
        "project_id: {}\n\
         title: {}\n\
         kind: {}\n\
         default_pack: {}\n\
         entry: {}\n\
         today_command: {}\n\
         manifest: {}\n\
         root: {}\n",
        discovered.manifest.project_id,
        discovered.manifest.title,
        discovered.manifest.kind,
        discovered.manifest.default_pack,
        discovered.manifest.default_entry,
        discovered.manifest.entry.today_command,
        discovered.manifest_path.display(),
        discovered.project_root.display()
    )
}

fn project_scan_json(
    root: &Path,
    projects: &[DiscoveredProjectManifest],
) -> serde_json::Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "root": root.display().to_string(),
        "projects": projects,
    }))
}

fn project_scan_text(root: &Path, projects: &[DiscoveredProjectManifest]) -> String {
    let mut text = format!(
        "root: {}\nprojects_found: {}\n",
        root.display(),
        projects.len()
    );
    for project in projects {
        text.push_str(&format!(
            "- project_id: {}\n  title: {}\n  kind: {}\n  root: {}\n  manifest: {}\n  today_command: {}\n",
            project.manifest.project_id,
            project.manifest.title,
            project.manifest.kind,
            project.project_root.display(),
            project.manifest_path.display(),
            project.manifest.entry.today_command
        ));
    }
    text
}

fn print_diagnosis(diagnosis: polaris_core::diagnosis::GraphDiagnosis) {
    println!("concept: {}", diagnosis.concept_id);
    println!("latest_failed: {}", diagnosis.latest_failed);
    if let Some(score) = diagnosis.latest_score {
        println!("latest_score: {score:.3}");
    }
    if let Some(reason) = diagnosis.latest_no_attempt_reason {
        println!("latest_no_attempt_reason: {reason}");
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
    print!("{}", mirror_report_text(report));
}

fn mirror_report_text(report: &polaris_core::report::MirrorReport) -> String {
    let mut text = String::new();
    text.push_str(&format!("镜像报告 {} （周 {}）\n", report.id, report.week));
    text.push_str(&format!(
        "窗口={}天 断言={} 假设={} 建议={} 被过滤={}\n",
        report.window_days,
        report.assertions.len(),
        report.hypotheses.len(),
        report.suggestions.len(),
        report.skipped.len()
    ));
    text.push_str(&format!(
        "hazard 门：participates={} reason={}\n",
        report.hazard_gate.participates, report.hazard_gate.reason
    ));
    if let Some(top_signal) = &report.top_signal {
        text.push_str(&format!("top_signal: {}\n", top_signal.claim));
        text.push_str(&format!(
            "top_action: {}\n\n",
            top_signal.suggested_action.as_deref().unwrap_or("-")
        ));
    }
    if let Some(narrative) = &report.narrative {
        text.push_str("--- Tier 1 叙事 ---\n");
        text.push_str(&format!("{}\n", narrative.text));
        if !narrative.citations.is_empty() {
            let cited = narrative
                .citations
                .iter()
                .map(|citation| citation.evidence_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            text.push_str(&format!("引用断言：{cited}\n"));
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
        text.push_str(&format!("--- {label} ---\n"));
        for item in items.iter() {
            text.push_str(&format!(
                "[{}] 置信度={:.0}% 证据={}条\n",
                item.id,
                item.confidence * 100.0,
                item.evidence_ids.len()
            ));
            text.push_str(&format!("  {}\n", item.claim));
        }
    }
    if !report.skipped.is_empty() {
        text.push_str("--- 被过滤候选 ---\n");
        for skip in &report.skipped {
            text.push_str(&format!("[{}] reason={}\n", skip.id, skip.reason));
        }
    }
    text.push_str("--- 三问反思 ---\n");
    for prompt in &report.reflection_prompts {
        text.push_str(&format!("· {prompt}\n"));
    }
    text
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
            vec!["polaris", "capture", "--text", "hello", "--source", "paste"],
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
            vec!["polaris", "session", "close", "--session", "cli", "--json"],
            vec!["polaris", "session", "show", "--session", "cli"],
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
            vec!["polaris", "fsrs-fit"],
            vec!["polaris", "fsrs-fit", "--json"],
            vec!["polaris", "diagnose", "--concept", "ownership"],
            vec!["polaris", "mcp"],
            vec![
                "polaris",
                "feedback",
                "state",
                "--state",
                "flow",
                "--concept",
                "ownership",
            ],
            vec!["polaris", "feedback", "pause", "--reason", "done_for_now"],
            vec!["polaris", "pack", "validate", "packs/rust"],
            vec!["polaris", "pack", "list"],
            vec!["polaris", "pack", "list", "--json"],
            vec![
                "polaris",
                "pack",
                "switch",
                "algorithms",
                "--theta-mode",
                "isolated",
            ],
            vec!["polaris", "privacy", "show"],
            vec!["polaris", "privacy", "show", "--json"],
            vec!["polaris", "trust", "show"],
            vec!["polaris", "trust", "show", "--json"],
            vec!["polaris", "ai-profile", "show"],
            vec![
                "polaris",
                "ai-profile",
                "set",
                "--persona",
                "balanced_mentor",
            ],
            vec!["polaris", "project", "detect"],
            vec!["polaris", "project", "detect", "--path", ".", "--json"],
            vec![
                "polaris",
                "project",
                "scan",
                "--root",
                "examples/project-manifests",
                "--max-depth",
                "2",
            ],
            vec!["polaris", "project", "scan", "--root", ".", "--json"],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
    }

    #[test]
    fn p16i_submit_parses_explicit_no_attempt_reason() {
        let cli = Cli::try_parse_from([
            "polaris",
            "submit",
            "--concept",
            "ownership",
            "--response",
            "",
            "--confidence",
            "1",
            "--no-attempt-reason",
            "not_understood_prompt",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Submit {
                no_attempt_reason: Some(ref reason),
                ..
            } if reason == "not_understood_prompt"
        ));
    }

    #[test]
    fn pack_list_flags_parse() {
        let cli = Cli::try_parse_from(vec!["polaris", "pack", "list", "--json"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::Pack {
                command: PackCommands::List { json: true }
            }
        ));
    }

    #[test]
    fn pack_switch_flags_parse() {
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "pack",
            "switch",
            "algorithms",
            "--theta-mode",
            "isolated",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Commands::Pack {
                command: PackCommands::Switch {
                    ref pack,
                    theta_mode: Some(ThetaModeArg::Isolated)
                }
            } if pack == "algorithms"
        ));
    }

    #[test]
    fn report_narrative_flag_parses_explicit_tier1_request() {
        let cli = Cli::try_parse_from(vec!["polaris", "report", "--narrative"]).unwrap();

        assert!(matches!(cli.command, Commands::Report { narrative: true }));
    }

    #[test]
    fn fsrs_fit_json_flag_parses() {
        let cli = Cli::try_parse_from(vec!["polaris", "fsrs-fit", "--json"]).unwrap();

        assert!(matches!(cli.command, Commands::FsrsFit { json: true }));
    }

    #[test]
    fn fsrs_fit_text_marks_rejected_value_as_candidate() {
        let summary = polaris_core::fsrs_fit::FsrsFitSummary {
            param: "fsrs.w".to_owned(),
            status: polaris_core::fsrs_fit::FsrsFitStatus::Rejected,
            old_value: "[1.0;17]".to_owned(),
            new_value: "[2.0;17]".to_owned(),
            old_weights: vec![1.0; 17],
            candidate_weights: vec![2.0; 17],
            metric: "fsrs_holdout_logloss".to_owned(),
            current_metric: Some(1.0),
            candidate_metric: Some(0.9),
            delta: 0.1,
            accepted: false,
            reason: None,
            total_final_attempts: 100,
            train_predictions: 80,
            holdout_predictions: 20,
            candidates_evaluated: 2,
            replayed_concepts: 0,
        };

        let text = fsrs_fit_text(&summary);

        assert!(text.starts_with("rejected fsrs.w: kept=[1.0;17] candidate=[2.0;17]"));
        assert!(!text.contains(" -> "));
    }

    #[test]
    fn report_feedback_verdict_flag_parses() {
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "report-feedback",
            "--assertion",
            "calibration_phantom:ownership",
            "--verdict",
            "accurate",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Commands::ReportFeedback {
                ref assertion,
                report: None,
                ref verdict,
            } if assertion == "calibration_phantom:ownership" && verdict == "accurate"
        ));
    }

    #[test]
    fn learner_mirror_json_flag_parses() {
        let cli = Cli::try_parse_from(vec!["polaris", "learner-mirror", "--json"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::LearnerMirror { json: true }
        ));
    }

    #[test]
    fn trust_show_json_flag_parses() {
        let cli = Cli::try_parse_from(vec!["polaris", "trust", "show", "--json"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::Trust {
                command: TrustCommands::Show { json: true }
            }
        ));
    }

    #[test]
    fn pack_sandbox_flags_parse() {
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "pack",
            "sandbox",
            "packs/template",
            "--profile",
            "strong",
            "--days",
            "7",
            "--json",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Commands::Pack {
                command: PackCommands::Sandbox {
                    profile: SandboxProfileArg::Strong,
                    days: 7,
                    json: true,
                    ..
                }
            }
        ));
    }

    #[test]
    fn pack_sandbox_rejects_user_database_path() {
        let db_path = temp_db_path("sandbox-ignored-db");
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "pack",
            "sandbox",
            workspace_pack_path("packs/template").to_str().unwrap(),
        ])
        .unwrap();

        let err = run(cli).unwrap_err().to_string();

        assert!(
            err.contains("in-memory") || err.contains("sandbox"),
            "error should explain that sandbox does not use --db: {err}"
        );
        assert!(
            !db_path.exists(),
            "sandbox must not create or write the user-provided database path"
        );
    }

    #[test]
    fn pack_sandbox_json_uses_profile_contract_field() {
        let reports = run_pack_sandbox_profiles(
            &workspace_pack_path("packs/template"),
            SandboxProfileArg::Strong,
            1,
        )
        .unwrap();
        let value: Value = serde_json::from_str(&sandbox_reports_json(&reports).unwrap()).unwrap();

        assert_eq!(value["mode"], "sandbox");
        assert_eq!(value["profile"], "strong");
        assert!(value.get("learner").is_none(), "{value:#}");
    }

    #[test]
    fn learner_feedback_flags_parse() {
        let state = Cli::try_parse_from(vec![
            "polaris",
            "feedback",
            "state",
            "--state",
            "frustrated",
            "--session",
            "cli",
            "--concept",
            "ownership",
            "--note",
            "transfer feels stuck",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            state.command,
            Commands::Feedback {
                command: FeedbackCommands::State {
                    ref state,
                    ref session,
                    ref concept,
                    ref note,
                    json: true
                }
            } if state == "frustrated"
                && session == "cli"
                && concept.as_deref() == Some("ownership")
                && note.as_deref() == Some("transfer feels stuck")
        ));

        let pause = Cli::try_parse_from(vec![
            "polaris",
            "feedback",
            "pause",
            "--reason",
            "done_for_now",
        ])
        .unwrap();
        assert!(matches!(
            pause.command,
            Commands::Feedback {
                command: FeedbackCommands::Pause {
                    ref reason,
                    ref session,
                    concept: None,
                    note: None,
                    json: false
                }
            } if reason == "done_for_now" && session == "cli"
        ));
    }

    #[test]
    fn learner_feedback_text_reports_recorded_receipt() {
        let receipt = polaris_core::learner_feedback::LearnerFeedbackReceipt {
            event_id: "event-1".to_owned(),
            kind: "state".to_owned(),
            session_id: "cli".to_owned(),
            concept_id: Some("ownership".to_owned()),
            state: Some("frustrated".to_owned()),
            reason: None,
            effect: "recorded_only".to_owned(),
        };

        let text = learner_feedback_text(&receipt);

        assert!(text.contains("event_id=event-1"));
        assert!(text.contains("kind=state"));
        assert!(text.contains("state=frustrated"));
        assert!(text.contains("effect=recorded_only"));
        assert!(text.contains("does not directly change mastery or scheduling"));
    }

    #[test]
    fn capture_record_text_reports_recorded_only_message() {
        let record = polaris_core::capture_queue::CaptureRecord {
            capture_id: "capture-1".to_owned(),
            evidence_id: "evidence-1".to_owned(),
            status: polaris_core::capture_queue::CaptureStatus::Pending,
            learner_kind: polaris_core::capture_queue::LearnerCaptureKind::Reference,
            effect: polaris_core::capture_queue::CaptureEffect::RecordedOnly,
            message: "已保存为学习资料，不会直接算作掌握。".to_owned(),
        };

        let text = capture_record_text(&record);

        assert!(text.contains("capture_id: capture-1"));
        assert!(text.contains("evidence_id: evidence-1"));
        assert!(text.contains("status: pending"));
        assert!(text.contains("learner_kind: reference"));
        assert!(text.contains("recorded_only: true"));
        assert!(text.contains("不会直接算作掌握"));
    }

    #[test]
    fn capture_command_records_pending_item_without_attempt_or_mastery() {
        let db_path = temp_db_path("capture-command");
        cleanup_db_path(&db_path);
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "capture",
            "--text",
            "我刚看了 Rust 所有权的一段解释",
            "--source",
            "paste",
            "--learner-kind",
            "reference",
            "--candidate-concept",
            "ownership",
        ])
        .unwrap();

        run(cli).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let (queued_count, evidence_count, attempt_count, mastery_count): (i64, i64, i64, i64) =
            conn.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM capture_queue WHERE status='pending'),
                    (SELECT COUNT(*) FROM evidence_items WHERE source='paste'),
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM mastery_states)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        drop(conn);
        cleanup_db_path(&db_path);

        assert_eq!(queued_count, 1);
        assert_eq!(evidence_count, 1);
        assert_eq!(attempt_count, 0);
        assert_eq!(mastery_count, 0);
    }

    #[test]
    fn p12d_inbox_commands_parse_list_and_act() {
        let list = Cli::try_parse_from(vec![
            "polaris", "inbox", "list", "--status", "pending", "--limit", "5", "--json",
        ])
        .unwrap();
        assert!(matches!(
            list.command,
            Commands::Inbox {
                command: InboxCommands::List {
                    ref statuses,
                    limit: 5,
                    json: true,
                }
            } if statuses == &vec!["pending".to_owned()]
        ));

        let act = Cli::try_parse_from(vec![
            "polaris",
            "inbox",
            "act",
            "--capture",
            "capture-1",
            "--action",
            "accept",
            "--note",
            "turn it into practice",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            act.command,
            Commands::Inbox {
                command: InboxCommands::Act {
                    ref capture,
                    ref action,
                    ref note,
                    json: true,
                }
            } if capture == "capture-1"
                && action == "accept"
                && note.as_deref() == Some("turn it into practice")
        ));
    }

    #[test]
    fn p12d_inbox_accept_command_marks_practice_ready_without_attempt_or_mastery() {
        let db_path = temp_db_path("inbox-accept-command");
        cleanup_db_path(&db_path);
        let capture = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "capture",
            "--text",
            "Borrow checker error I want to practice",
            "--source",
            "paste",
        ])
        .unwrap();
        run(capture).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let capture_id: String = conn
            .query_row("SELECT id FROM capture_queue", [], |row| row.get(0))
            .unwrap();
        drop(conn);

        let accept = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "inbox",
            "act",
            "--capture",
            capture_id.as_str(),
            "--action",
            "accept",
        ])
        .unwrap();
        run(accept).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let (status, attempt_count, mastery_count, grade_queue_count): (String, i64, i64, i64) =
            conn.query_row(
                "SELECT
                    (SELECT status FROM capture_queue WHERE id=?1),
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM mastery_states),
                    (SELECT COUNT(*) FROM grade_queue)",
                [capture_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        drop(conn);
        cleanup_db_path(&db_path);

        assert_eq!(status, "practice_ready");
        assert_eq!(attempt_count, 0);
        assert_eq!(mastery_count, 0);
        assert_eq!(grade_queue_count, 0);
    }

    #[test]
    fn p12e_inbox_commands_parse_practice_and_submit() {
        let practice = Cli::try_parse_from(vec![
            "polaris",
            "inbox",
            "practice",
            "--capture",
            "capture-1",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            practice.command,
            Commands::Inbox {
                command: InboxCommands::Practice {
                    ref capture,
                    json: true,
                }
            } if capture == "capture-1"
        ));

        let submit = Cli::try_parse_from(vec![
            "polaris",
            "inbox",
            "submit",
            "--capture",
            "capture-1",
            "--response",
            "Ownership prevents double free.",
            "--confidence",
            "4",
            "--session",
            "cli-practice",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            submit.command,
            Commands::Inbox {
                command: InboxCommands::Submit {
                    ref capture,
                    ref response,
                    confidence: 4,
                    ref session,
                    json: true,
                }
            } if capture == "capture-1"
                && response == "Ownership prevents double free."
                && session == "cli-practice"
        ));
    }

    #[test]
    fn p12e_inbox_submit_command_records_attempt_and_practiced_status() {
        let db_path = temp_db_path("inbox-practice-submit-command");
        cleanup_db_path(&db_path);

        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "init",
            "--pack",
            workspace_pack_path("packs/rust").to_str().unwrap(),
        ])
        .unwrap())
        .unwrap();
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "capture",
            "--text",
            "Ownership means one value has one owner.",
            "--candidate-concept",
            "ownership",
        ])
        .unwrap())
        .unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let capture_id: String = conn
            .query_row("SELECT id FROM capture_queue", [], |row| row.get(0))
            .unwrap();
        drop(conn);

        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "inbox",
            "act",
            "--capture",
            capture_id.as_str(),
            "--action",
            "accept",
        ])
        .unwrap())
        .unwrap();
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "inbox",
            "submit",
            "--capture",
            capture_id.as_str(),
            "--response",
            "Ownership controls which binding can drop the value.",
            "--confidence",
            "4",
        ])
        .unwrap())
        .unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let (status, attempt_count, mastery_count, grade_queue_count): (String, i64, i64, i64) =
            conn.query_row(
                "SELECT
                    (SELECT status FROM capture_queue WHERE id=?1),
                    (SELECT COUNT(*) FROM attempts),
                    (SELECT COUNT(*) FROM mastery_states),
                    (SELECT COUNT(*) FROM grade_queue)",
                [capture_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        drop(conn);
        cleanup_db_path(&db_path);

        assert_eq!(status, "practiced");
        assert_eq!(attempt_count, 1);
        assert_eq!(mastery_count, 1);
        assert_eq!(grade_queue_count, 1);
    }

    #[test]
    fn learner_feedback_commands_record_state_and_pause_events() {
        let db_path = temp_db_path("learner-feedback");
        cleanup_db_path(&db_path);

        let state = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "feedback",
            "state",
            "--state",
            "frustrated",
            "--session",
            "cli-flow",
            "--concept",
            "ownership",
            "--note",
            "transfer feels stuck",
            "--json",
        ])
        .unwrap();
        let pause = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "feedback",
            "pause",
            "--reason",
            "today is enough",
            "--session",
            "cli-flow",
        ])
        .unwrap();

        run(state).unwrap();
        run(pause).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let state_payload: String = conn
            .query_row(
                "SELECT payload_json FROM behavior_events
                 WHERE type='learner_feedback'
                   AND session_id='cli-flow'
                   AND concept_id='ownership'
                   AND json_extract(payload_json, '$.kind')='state'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let state_payload: serde_json::Value = serde_json::from_str(&state_payload).unwrap();
        assert_eq!(state_payload["state"], "frustrated");
        assert_eq!(state_payload["source"], "cli");
        assert_eq!(state_payload["effect"], "recorded_only");

        let pause_reason: String = conn
            .query_row(
                "SELECT json_extract(payload_json, '$.reason') FROM behavior_events
                 WHERE type='learner_feedback'
                   AND session_id='cli-flow'
                   AND json_extract(payload_json, '$.kind')='pause'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let abandon_events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM behavior_events WHERE type='abandon'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pause_reason, "today is enough");
        assert_eq!(abandon_events, 0);

        drop(conn);
        cleanup_db_path(&db_path);
    }

    #[test]
    fn learner_feedback_command_rejects_invalid_state_without_event() {
        let db_path = temp_db_path("learner-feedback-invalid");
        cleanup_db_path(&db_path);
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "feedback",
            "state",
            "--state",
            "sleepy",
        ])
        .unwrap();

        let error = run(cli).unwrap_err().to_string();

        assert!(error.contains("learner_feedback.state"));
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let events: i64 = conn
            .query_row("SELECT COUNT(*) FROM behavior_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(events, 0);

        drop(conn);
        cleanup_db_path(&db_path);
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
    fn p13c_ai_profile_commands_parse_show_and_set() {
        let show = Cli::try_parse_from(vec!["polaris", "ai-profile", "show", "--json"]).unwrap();
        assert!(matches!(
            show.command,
            Commands::AiProfile {
                command: AiProfileCommands::Show { json: true }
            }
        ));

        let set = Cli::try_parse_from(vec![
            "polaris",
            "ai-profile",
            "set",
            "--persona",
            "socratic_tutor",
            "--verbosity",
            "detailed",
            "--explanation-depth",
            "examples_first",
            "--proactivity",
            "stuck_only",
            "--intervention-frequency",
            "normal",
            "--correction-style",
            "guided",
            "--custom-notes",
            "先问再讲。",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            set.command,
            Commands::AiProfile {
                command: AiProfileCommands::Set {
                    persona: Some(ref persona),
                    verbosity: Some(ref verbosity),
                    explanation_depth: Some(ref explanation_depth),
                    proactivity: Some(ref proactivity),
                    intervention_frequency: Some(ref intervention_frequency),
                    correction_style: Some(ref correction_style),
                    custom_notes: Some(ref custom_notes),
                    json: true,
                }
            } if persona == "socratic_tutor"
                && verbosity == "detailed"
                && explanation_depth == "examples_first"
                && proactivity == "stuck_only"
                && intervention_frequency == "normal"
                && correction_style == "guided"
                && custom_notes == "先问再讲。"
        ));
    }

    #[test]
    fn p13c_ai_profile_set_command_persists_preferences() {
        let db_path = temp_db_path("ai-profile-set-command");
        cleanup_db_path(&db_path);

        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "init",
            "--pack",
            workspace_pack_path("packs/rust").to_str().unwrap(),
        ])
        .unwrap())
        .unwrap();
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "ai-profile",
            "set",
            "--persona",
            "strict_coach",
            "--verbosity",
            "brief",
            "--explanation-depth",
            "key_steps",
            "--proactivity",
            "on_request",
            "--intervention-frequency",
            "low",
            "--correction-style",
            "direct",
        ])
        .unwrap())
        .unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let profile_json: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='ai.interaction_profile'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let profile: serde_json::Value = serde_json::from_str(&profile_json).unwrap();
        drop(conn);
        cleanup_db_path(&db_path);

        assert_eq!(profile["persona"], "strict_coach");
        assert_eq!(profile["verbosity"], "brief");
        assert_eq!(profile["proactivity"], "on_request");
    }

    #[test]
    fn p16d_profile_commands_parse_governance_actions() {
        let set = Cli::try_parse_from(vec![
            "polaris",
            "profile",
            "set",
            "--enabled",
            "true",
            "--acknowledge-disclosure",
            "--summary-sharing",
            "false",
            "--json",
        ])
        .unwrap();
        assert!(matches!(
            set.command,
            Commands::Profile {
                command: ProfileCommands::Set {
                    enabled: Some(true),
                    acknowledge_disclosure: true,
                    summary_sharing_enabled: Some(false),
                    json: true,
                    ..
                }
            }
        ));

        let delete = Cli::try_parse_from(vec![
            "polaris",
            "profile",
            "delete-all",
            "--confirm",
            polaris_core::profile::DELETE_ALL_CONFIRMATION,
            "--backup",
            "backup.sqlite",
        ])
        .unwrap();
        assert!(matches!(
            delete.command,
            Commands::Profile {
                command: ProfileCommands::DeleteAll {
                    backup: Some(_),
                    json: false,
                    ..
                }
            }
        ));
    }

    #[test]
    fn p16d_profile_cli_records_exports_disables_and_resets_locally() {
        let db_path = temp_db_path("profile-cli");
        let export_path = db_path.with_extension("profile-export.json");
        cleanup_db_path(&db_path);
        let _ = std::fs::remove_file(&export_path);

        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "set",
            "--acknowledge-disclosure",
        ])
        .unwrap())
        .unwrap();
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "record",
            "--instrument",
            "gse",
            "--item",
            "gse_01",
            "--response",
            "4",
        ])
        .unwrap())
        .unwrap();
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "export",
            "--output",
            export_path.to_str().unwrap(),
        ])
        .unwrap())
        .unwrap();

        let export: Value = serde_json::from_slice(&std::fs::read(&export_path).unwrap()).unwrap();
        assert_eq!(export["measurements"][0]["payload"]["response"], 4);

        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "set",
            "--enabled",
            "false",
        ])
        .unwrap())
        .unwrap();
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "record",
            "--instrument",
            "gse",
            "--item",
            "gse_02",
            "--response",
            "3",
        ])
        .unwrap())
        .unwrap();

        let missing_confirmation = run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "reset",
        ])
        .unwrap())
        .unwrap_err();
        assert!(missing_confirmation.to_string().contains("--yes"));
        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "reset",
            "--yes",
        ])
        .unwrap())
        .unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM behavior_events WHERE type='profile_measurement'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 0);
        drop(conn);
        cleanup_db_path(&db_path);
        let _ = std::fs::remove_file(export_path);
    }

    #[test]
    fn p16d_profile_cli_delete_all_never_opens_engine_and_keeps_optional_backup() {
        let db_path = temp_db_path("profile-delete-all");
        let backup_path = db_path.with_extension("profile-backup.sqlite");
        cleanup_db_path(&db_path);
        let _ = std::fs::remove_file(&backup_path);
        drop(polaris_core::db::open_database(&db_path).unwrap());

        let wrong = run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "delete-all",
            "--confirm",
            "yes",
        ])
        .unwrap())
        .unwrap_err();
        assert!(wrong
            .to_string()
            .contains(polaris_core::profile::DELETE_ALL_CONFIRMATION));
        assert!(db_path.exists());

        run(Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "profile",
            "delete-all",
            "--confirm",
            polaris_core::profile::DELETE_ALL_CONFIRMATION,
            "--backup",
            backup_path.to_str().unwrap(),
        ])
        .unwrap())
        .unwrap();

        assert!(db_path.exists());
        assert!(backup_path.exists());
        let empty = Connection::open(&db_path).unwrap();
        assert_eq!(
            empty
                .query_row("SELECT COUNT(*) FROM attempts", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(empty);
        let backup = Connection::open(&backup_path).unwrap();
        assert_eq!(
            backup
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        drop(backup);
        cleanup_db_path(&db_path);
        cleanup_db_path(&backup_path);
    }

    #[test]
    fn config_list_flags_parse() {
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "config",
            "list",
            "--class",
            "A",
            "--tuning-route",
            "Replay",
            "--md",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Commands::Config {
                command: ConfigCommands::List {
                    class: Some(ref class),
                    tuning_route: Some(ref tuning_route),
                    md: true,
                    json: false,
                }
            } if class == "A" && tuning_route == "Replay"
        ));
    }

    #[test]
    fn config_list_text_and_json_use_stable_fields() {
        let specs = polaris_core::config::parameter_specs(
            Some(polaris_core::config::ParameterClass::B),
            Some(polaris_core::config::TuningRoute::Replay),
        );

        let text = config_list_text(&specs);
        assert!(text.contains("key\tdefault\tclass\tbounds\ttuning_route\n"));
        assert!(text.contains("bkt.p_init\t0.20\tB\t[0.05,0.50]\tReplay\n"));

        let json = config_list_json(&specs).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(payload
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["key"] == "bkt.p_init" && item["tuning_route"] == "Replay"));
    }

    #[test]
    fn config_list_markdown_matches_parameters_doc_shape() {
        let specs = polaris_core::config::parameter_specs(None, None);
        let markdown = config_list_markdown(&specs);

        assert!(markdown.starts_with("# Polaris Parameter Registry\n\n"));
        assert!(markdown.contains("| `bkt.p_init` | `0.20` | B | `[0.05,0.50]` | Replay |"));
    }

    #[test]
    fn mirror_report_text_surfaces_top_signal_before_sections() {
        let item = polaris_core::report::ReportItem {
            id: "calibration_phantom:ownership".to_owned(),
            kind: "calibration_phantom".to_owned(),
            subject: "ownership".to_owned(),
            claim: "你的自信持续高于实际表现。".to_owned(),
            confidence: 0.8,
            evidence_ids: vec!["attempt:a1".to_owned()],
            stats: serde_json::json!({}),
            suggested_action: Some(
                "可以为该概念挑一道更高深度的验证题（迁移 / 自由解释）。".to_owned(),
            ),
        };
        let report = polaris_core::report::MirrorReport {
            schema_version: 1,
            id: "report-1".to_owned(),
            week: "2026-W24".to_owned(),
            generated_at: "2026-06-15T00:00:00Z".to_owned(),
            window_days: 7,
            assertions: vec![item.clone()],
            hypotheses: Vec::new(),
            suggestions: Vec::new(),
            skipped: Vec::new(),
            hazard_gate: polaris_core::report::HazardGateStatus {
                participates: false,
                reason: "fixture".to_owned(),
                validation_auc: None,
                auc_gate: 0.7,
            },
            reflection_prompts: Vec::new(),
            top_signal: Some(item),
            narrative: None,
        };

        let text = mirror_report_text(&report);

        assert!(text.contains("top_signal: 你的自信持续高于实际表现。\n"));
        assert!(text.contains(
            "top_action: 可以为该概念挑一道更高深度的验证题（迁移 / 自由解释）。\n\n--- 断言 ---"
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
    fn doctor_diagnose_json_flags_parse() {
        let cli = Cli::try_parse_from(vec!["polaris", "doctor", "--diagnose", "--json"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::Doctor {
                json: true,
                diagnose: true,
            }
        ));
    }

    #[test]
    fn doctor_diagnose_json_keeps_doctor_and_diagnostics_separate() {
        let doctor = polaris_core::ops::DoctorReport {
            ok: true,
            schema_version: 1,
            migration_count: 1,
            integrity_ok: true,
            integrity_messages: vec!["ok".to_owned()],
            replay_checked: 0,
            replay_mismatches: Vec::new(),
        };
        let diagnostics = polaris_core::ops::DoctorDiagnostics::empty(7);

        let json = doctor_diagnose_json(&doctor, &diagnostics).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(payload["doctor"]["ok"], true);
        assert_eq!(payload["doctor"]["schema_version"], 1);
        assert_eq!(payload["doctor"]["migration_count"], 1);
        assert_eq!(payload["diagnostics"]["window_days"], 7);
        assert!(
            payload.get("ok").is_none(),
            "top-level doctor fields must not be merged"
        );
    }

    #[test]
    fn init_creates_schema_version_and_migration_ledger() {
        let db_path = temp_db_path("init-schema");
        cleanup_db_path(&db_path);
        let cli = Cli::try_parse_from(vec![
            "polaris",
            "--db",
            db_path.to_str().unwrap(),
            "init",
            "--pack",
            workspace_pack_path("packs/rust").to_str().unwrap(),
        ])
        .unwrap();

        run(cli).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let user_version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let migration_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let active_pack: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key='active_pack'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        drop(conn);
        cleanup_db_path(&db_path);

        assert_eq!(user_version, polaris_core::db::CURRENT_SCHEMA_VERSION);
        assert_eq!(migration_count, 7);
        assert_eq!(active_pack, "rust");
    }

    #[test]
    fn status_json_serializes_current_pack() {
        let snapshot = polaris_core::status::StatusSnapshot {
            generated_at: "2026-06-13T00:00:00Z".to_owned(),
            current_pack: Some("algorithms".to_owned()),
            theta_mode: Some("isolated".to_owned()),
            packs: vec![pack_summary(
                "algorithms",
                "Algorithms",
                17,
                true,
                "isolated",
            )],
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
                phase_label: "看起来懂".to_owned(),
                phase_summary: "自信高但实际表现不稳，需要用更硬的题确认。".to_owned(),
            }],
        };

        let text = status_snapshot_json(&snapshot).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert_eq!(payload["generated_at"], "2026-06-13T00:00:00Z");
        assert_eq!(payload["current_pack"], "algorithms");
        assert_eq!(payload["theta_mode"], "isolated");
        assert_eq!(payload["packs"][0]["id"], "algorithms");
        assert_eq!(payload["packs"][0]["active"], true);
        assert_eq!(payload["due_today"], 2);
        assert_eq!(payload["phase_counts"][0]["phase"], "phantom");
        assert_eq!(payload["phase_counts"][0]["count"], 1);
        assert_eq!(payload["concepts"][0]["concept_id"], "ownership");
        assert_eq!(payload["concepts"][0]["phase"], "phantom");
        assert_eq!(payload["concepts"][0]["phase_label"], "看起来懂");
        assert_eq!(
            payload["concepts"][0]["phase_summary"],
            "自信高但实际表现不稳，需要用更硬的题确认。"
        );
    }

    #[test]
    fn learner_mirror_json_serializes_static_panel_fields() {
        let snapshot = polaris_core::learner_mirror::LearnerMirrorSnapshot {
            generated_at: "2026-06-16T00:00:00Z".to_owned(),
            confidence_curve: vec![polaris_core::learner_mirror::ConfidenceCurvePoint {
                attempt_id: "a1".to_owned(),
                concept_id: "ownership".to_owned(),
                created_at: "2026-06-15T00:00:00Z".to_owned(),
                confidence: 1.0,
                actual_score: 0.2,
                is_final: true,
            }],
            phase_distribution: vec![polaris_core::learner_mirror::PhaseDistributionItem {
                phase: "phantom".to_owned(),
                label: "看起来懂".to_owned(),
                summary: "自信高但实际表现不稳，需要用更硬的题确认。".to_owned(),
                count: 1,
            }],
            recent_assertions: vec![polaris_core::learner_mirror::RecentAssertion {
                id: "calibration_phantom:ownership".to_owned(),
                kind: "calibration_phantom".to_owned(),
                claim: "Confidence is ahead of actual score.".to_owned(),
                confidence: 0.82,
                suggested_action: None,
            }],
        };

        let json = learner_mirror_json(&snapshot).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(payload["generated_at"], "2026-06-16T00:00:00Z");
        assert_eq!(payload["confidence_curve"][0]["attempt_id"], "a1");
        assert_eq!(payload["confidence_curve"][0]["is_final"], true);
        assert_eq!(payload["phase_distribution"][0]["phase"], "phantom");
        let assertion = payload["recent_assertions"][0].as_object().unwrap();
        assert!(assertion.contains_key("suggested_action"));
        assert!(assertion["suggested_action"].is_null());
    }

    #[test]
    fn status_text_surfaces_current_pack() {
        let snapshot = polaris_core::status::StatusSnapshot {
            generated_at: "2026-06-13T00:00:00Z".to_owned(),
            current_pack: Some("algorithms".to_owned()),
            theta_mode: Some("isolated".to_owned()),
            packs: vec![
                pack_summary("algorithms", "Algorithms", 17, true, "isolated"),
                pack_summary("rust", "Rust", 24, false, "shared"),
            ],
            due_today: 2,
            phase_counts: Vec::new(),
            concepts: vec![polaris_core::status::ConceptStatus {
                concept_id: "ownership".to_owned(),
                name: "Ownership".to_owned(),
                retrieval: None,
                p_known: 0.42,
                calib_gap: 0.31,
                phase: "phantom".to_owned(),
                phase_label: "看起来懂".to_owned(),
                phase_summary: "自信高但实际表现不稳，需要用更硬的题确认。".to_owned(),
            }],
        };

        let text = status_snapshot_text(&snapshot);

        assert_eq!(
            text,
            "current_pack=algorithms\ntheta_mode=isolated\npacks=*algorithms:isolated:concepts=17; rust:shared:concepts=24\ndue_today=2\nownership\tOwnership\tR=-\tp_known=0.420\tcalib_gap=0.310\tphase=phantom\n"
        );
    }

    #[test]
    fn pack_list_text_surfaces_active_pack_and_theta_mode() {
        let text = pack_list_text(&[
            pack_summary("algorithms", "Algorithms", 17, true, "isolated"),
            pack_summary("rust", "Rust", 24, false, "shared"),
        ]);

        assert_eq!(
            text,
            "* algorithms\tAlgorithms\tconcepts=17\ttheta_mode=isolated\n  rust\tRust\tconcepts=24\ttheta_mode=shared\n"
        );
    }

    #[test]
    fn pack_switch_text_surfaces_resulting_context() {
        let text = pack_switch_text(&polaris_core::pack_state::PackSwitchReceipt {
            active_pack: "algorithms".to_owned(),
            theta_mode: "isolated".to_owned(),
        });

        assert_eq!(text, "active_pack=algorithms\ntheta_mode=isolated\n");
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
        let backup_schema_version: i64 = backup_conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(evidence_count, 1);
        assert_eq!(
            backup_schema_version,
            polaris_core::db::CURRENT_SCHEMA_VERSION
        );

        let report = polaris_core::ops::doctor_report(&conn).unwrap();
        assert!(report.ok);
        let text = doctor_report_text(&report);
        assert!(text.contains("schema_version="));
        assert!(text.contains("migration_count=7"));
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
    fn backup_rejects_newer_schema_source_without_enabling_wal() {
        let path = temp_db_path("backup-newer-schema");
        cleanup_db_path(&path);
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
            conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
                .unwrap();
        }

        let error = open_existing_database(&path).unwrap_err().to_string();
        let conn = rusqlite::Connection::open(&path).unwrap();
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        drop(conn);
        let wal_exists = path.with_extension("db-wal").exists();
        let shm_exists = path.with_extension("db-shm").exists();
        cleanup_db_path(&path);

        assert!(error.contains("unsupported database schema version"));
        assert_eq!(journal_mode.to_lowercase(), "delete");
        assert!(!wal_exists);
        assert!(!shm_exists);
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

    fn pack_summary(
        id: &str,
        title: &str,
        concept_count: i64,
        active: bool,
        theta_mode: &str,
    ) -> PackSummary {
        PackSummary {
            id: id.to_owned(),
            title: title.to_owned(),
            concept_count,
            active,
            theta_mode: theta_mode.to_owned(),
        }
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }

    fn temp_db_path(prefix: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("polaris-core-{prefix}-{suffix}.db"))
    }

    fn cleanup_db_path(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
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
