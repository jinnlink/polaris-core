mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::workspace_pack_path;
use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use rusqlite::{params, Connection};

const ARM_ORDER: [Arm; 7] = [
    Arm::Cold,
    Arm::Mastery,
    Arm::Fail,
    Arm::Underconfident,
    Arm::Phantom,
    Arm::Misconception,
    Arm::Behavioral,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Arm {
    Cold,
    Mastery,
    Fail,
    Underconfident,
    Phantom,
    Misconception,
    Behavioral,
}

impl Arm {
    fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Mastery => "mastery",
            Self::Fail => "fail",
            Self::Underconfident => "underconfident",
            Self::Phantom => "phantom",
            Self::Misconception => "misconception",
            Self::Behavioral => "behavioral",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TaskSignature {
    concept_id: String,
    task_type: String,
    move_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BatchItemSignature {
    concept_id: String,
    task_type: String,
    move_id: String,
    phase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OutputSignature {
    next: TaskSignature,
    batch_strategy: Vec<BatchItemSignature>,
}

#[test]
fn stability_same_evidence_sequence_replays_identically() {
    for arm in ARM_ORDER {
        let first = observe(arm);
        let second = observe(arm);
        assert_eq!(first, second, "{} arm was not deterministic", arm.name());
    }
}

#[test]
fn responsiveness_different_evidence_arms_diverge() {
    let outputs = collect_outputs();
    print_matrix(&outputs);

    let unique = outputs.values().collect::<BTreeSet<_>>();
    assert!(
        unique.len() >= 5,
        "at least five distinct targeted outputs are required, got {}",
        unique.len()
    );
    assert_ne!(
        outputs[&Arm::Mastery],
        outputs[&Arm::Fail],
        "success and failure must not lead to the same action"
    );
    assert_ne!(
        outputs[&Arm::Mastery],
        outputs[&Arm::Underconfident],
        "underconfidence must not collapse into ordinary mastery"
    );
    assert_ne!(
        outputs[&Arm::Mastery],
        outputs[&Arm::Behavioral],
        "behavior-only evidence must reach the scheduling output"
    );
}

#[test]
fn directionality_each_divergence_hits_its_expected_target() {
    let outputs = collect_outputs();
    print_matrix(&outputs);

    let cold = &outputs[&Arm::Cold];
    let mastery = &outputs[&Arm::Mastery];
    let fail = &outputs[&Arm::Fail];
    let underconfident = &outputs[&Arm::Underconfident];
    let phantom = &outputs[&Arm::Phantom];
    let misconception = &outputs[&Arm::Misconception];
    let behavioral = &outputs[&Arm::Behavioral];

    let mut failures = Vec::new();
    if mastery.next == cold.next {
        failures.push("mastery did not advance from cold start");
    }
    if max_batch_depth(fail) >= max_batch_depth(mastery) {
        failures.push("failure did not lower the batch depth below mastery");
    }
    if underconfident == mastery {
        failures.push("underconfidence collapsed into ordinary mastery");
    }
    if phantom.next.move_id != "transfer"
        && !phantom
            .batch_strategy
            .iter()
            .any(|item| item.phase == "phantom" && item.move_id == "transfer")
    {
        failures.push("phantom phase did not trigger the phantom challenge direction");
    }
    if misconception.next.concept_id != "borrowing" {
        failures.push("active misconception did not raise borrowing to repair priority");
    }
    if behavioral.batch_strategy == mastery.batch_strategy {
        failures.push("latency, hints and abandon evidence did not modulate the batch");
    }
    assert!(
        failures.is_empty(),
        "directionality failures:\n- {}",
        failures.join("\n- ")
    );
}

fn collect_outputs() -> BTreeMap<Arm, OutputSignature> {
    let mut outputs = ARM_ORDER
        .into_iter()
        .map(|arm| (arm, observe(arm)))
        .collect::<BTreeMap<_, _>>();
    if std::env::var_os("POLARIS_P16G_FORCE_RIGID").is_some() {
        let rigid = outputs[&Arm::Cold].clone();
        for output in outputs.values_mut() {
            *output = rigid.clone();
        }
    }
    outputs
}

fn observe(arm: Arm) -> OutputSignature {
    let engine = seeded_engine(arm);
    if std::env::var_os("POLARIS_P16G_DIAGNOSTIC").is_some() {
        if let Some(state) = engine.mastery_state("ownership").unwrap() {
            let mental_state = engine
                .conn()
                .query_row(
                    "SELECT json_extract(payload_json, '$.dominant_state')
                     FROM behavior_events WHERE type='mental_state'
                     ORDER BY at DESC, id DESC LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok();
            println!(
                "P16G {} ownership: p_known={:.3} calib_gap={:.3} attempts={} phase={} mental_state={mental_state:?}",
                arm.name(),
                state.p_known,
                state.calib_gap,
                state.attempt_count,
                state.phase.as_str()
            );
        }
    }
    let next = engine.next_task().unwrap().expect("next task");
    let batch = engine.get_interleaved_batch(3).unwrap();
    OutputSignature {
        next: TaskSignature {
            concept_id: next.concept_id,
            task_type: next.task_type,
            move_id: next.move_id,
        },
        batch_strategy: batch
            .into_iter()
            .map(|item| BatchItemSignature {
                concept_id: item.concept_id,
                task_type: item.task_type,
                move_id: item.move_name,
                phase: item.phase.as_str().to_owned(),
            })
            .collect(),
    }
}

fn seeded_engine(arm: Arm) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .conn()
        .execute("UPDATE meta SET value='0' WHERE key='mrt.epsilon'", [])
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO state_gate_evals(id, evaluated_at, baseline_auc, state_auc, margin, passes, n)
             VALUES ('p16g-state-gate', '2026-08-09T00:00:00Z', 0.60, 0.80, 0.20, 1, 60)",
            [],
        )
        .unwrap();

    match arm {
        Arm::Cold => {}
        Arm::Mastery => seed_attempts(&mut engine, arm, "ownership", 0.92, 5, 500, 0),
        Arm::Fail => seed_attempts(&mut engine, arm, "ownership", 0.10, 1, 500, 0),
        Arm::Underconfident => seed_attempts(&mut engine, arm, "ownership", 0.92, 1, 500, 0),
        Arm::Phantom => seed_attempts(&mut engine, arm, "ownership", 0.10, 5, 500, 0),
        Arm::Misconception => {
            seed_attempts(&mut engine, arm, "borrowing", 0.10, 2, 500, 0);
            engine
                .conn()
                .execute(
                    "UPDATE attempts SET misconception_id='borrow_checker_runtime'
                     WHERE session_id LIKE 'p16g-misconception-%'",
                    [],
                )
                .unwrap();
        }
        Arm::Behavioral => {
            seed_attempts(&mut engine, arm, "ownership", 0.92, 5, 30_000, 5);
            for idx in 0..4 {
                engine
                    .conn()
                    .execute(
                        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
                         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                                 'abandon', 'ownership', '{}')",
                        params![
                            format!("p16g-behavioral-abandon-{idx}"),
                            format!("p16g-behavioral-{idx}")
                        ],
                    )
                    .unwrap();
            }
        }
    }
    engine
}

fn seed_attempts(
    engine: &mut Engine,
    arm: Arm,
    concept_id: &str,
    final_score: f64,
    confidence: i32,
    latency_ms: i64,
    hint_count: i64,
) {
    for idx in 0..4 {
        let receipt = engine
            .submit(SubmitInput {
                session_id: format!("p16g-{}-{idx}", arm.name()),
                concept_id: concept_id.to_owned(),
                task_type: "apply".to_owned(),
                prompt_text: format!("Apply {concept_id} to a new case."),
                response_text: format!(
                    "Evidence sequence {idx} for {concept_id} explains the rule and its boundary."
                ),
                self_confidence: confidence,
                latency_ms,
                hint_count,
            })
            .unwrap();
        engine
            .apply_final_score(&receipt.attempt_id, final_score)
            .unwrap();
    }
}

fn depth_rank(task_type: &str) -> usize {
    match task_type {
        "recall" | "recognition" | "mcq" | "cloze" => 0,
        "explain" | "free_explain" | "rewrite" => 1,
        "apply" => 2,
        "analyze" => 3,
        "evaluate" => 4,
        "create" => 5,
        "transfer" => 6,
        _ => 0,
    }
}

fn max_batch_depth(output: &OutputSignature) -> usize {
    output
        .batch_strategy
        .iter()
        .map(|item| depth_rank(&item.task_type))
        .max()
        .unwrap_or_else(|| depth_rank(&output.next.task_type))
}

fn print_matrix(outputs: &BTreeMap<Arm, OutputSignature>) {
    println!("P16G arm outputs:");
    for arm in ARM_ORDER {
        println!("  {:<14} {:?}", arm.name(), outputs[&arm]);
    }
    println!("P16G divergence matrix (1=different, 0=same):");
    println!(
        "  {:<14} {}",
        "arm",
        ARM_ORDER
            .iter()
            .map(|arm| format!("{:>3}", &arm.name()[..3]))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for row in ARM_ORDER {
        let cells = ARM_ORDER
            .iter()
            .map(|column| usize::from(outputs[&row] != outputs[column]).to_string())
            .collect::<Vec<_>>()
            .join("   ");
        println!("  {:<14} {cells}", row.name());
    }
}
