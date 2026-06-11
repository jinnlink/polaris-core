use std::path::PathBuf;

use clap::{Parser, Subcommand};
use polaris_core::db::open_database;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::fsrs::{retrievability, FsrsState};
use polaris_core::pack::validate_pack_path;

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
        text: String,
        #[arg(long, default_value = "cli")]
        session: String,
        #[arg(long, default_value = "cli")]
        source: String,
    },
    Next,
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
    Status,
    GradePending,
    Pack {
        #[command(subcommand)]
        command: PackCommands,
    },
}

#[derive(Debug, Subcommand)]
enum PackCommands {
    Validate { path: PathBuf },
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
                } => {
                    engine.conn().execute(
                        "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
                         VALUES (lower(hex(randomblob(16))), ?1, ?2, 'text/plain', ?3, '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                        (&session, &source, &text),
                    )?;
                    println!("ingested");
                }
                Commands::Next => {
                    if let Some(task) = engine.next_task()? {
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
                    let receipt = engine.submit(SubmitInput {
                        session_id: session,
                        concept_id: concept,
                        task_type,
                        prompt_text: prompt,
                        response_text: response,
                        self_confidence: confidence,
                        latency_ms: 0,
                        hint_count: 0,
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
                Commands::Status => {
                    let due_today: i64 = engine.conn().query_row(
                        "SELECT COUNT(*) FROM mastery_states WHERE next_due_at IS NOT NULL AND next_due_at='today'",
                        [],
                        |row| row.get(0),
                    )?;
                    println!("due_today={due_today}");
                    let mut stmt = engine.conn().prepare(
                        "SELECT c.id, c.name, COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                                COALESCE(ms.calib_gap, 0.0), COALESCE(ms.attempt_count, 0), ms.fsrs_json
                         FROM concepts c
                         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
                         ORDER BY c.seed_order ASC, c.id ASC",
                    )?;
                    let rows = stmt.query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, f64>(2)?,
                            row.get::<_, f64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, Option<String>>(5)?,
                        ))
                    })?;
                    for row in rows {
                        let (id, name, p_known, calib_gap, attempts, fsrs_json) = row?;
                        let retrieval = fsrs_json
                            .as_deref()
                            .and_then(|json| serde_json::from_str::<FsrsState>(json).ok())
                            .map(|state| format!("{:.3}", retrievability(state.stability, 0.0)))
                            .unwrap_or_else(|| "-".to_owned());
                        let phase = if attempts >= 2 && calib_gap >= 0.25 && p_known < 0.60 {
                            "幻影!"
                        } else {
                            "正常"
                        };
                        println!(
                            "{id}\t{name}\tR={retrieval}\tp_known={p_known:.3}\tcalib_gap={calib_gap:.3}\tphase={phase}"
                        );
                    }
                }
                Commands::GradePending => {
                    let count: i64 =
                        engine
                            .conn()
                            .query_row("SELECT COUNT(*) FROM grade_queue", [], |row| row.get(0))?;
                    println!("pending={count}");
                }
                Commands::Pack { .. } => unreachable!("handled before database open"),
            }
        }
    }
    Ok(())
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
            vec!["polaris", "next"],
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
            vec!["polaris", "grade-pending"],
            vec!["polaris", "pack", "validate", "packs/rust"],
        ] {
            Cli::try_parse_from(args).unwrap();
        }
    }
}
