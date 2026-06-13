use std::fs;
use std::path::{Path, PathBuf};

use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::pack::validate_pack_path;
use rusqlite::{params, Connection};

const ENGLISH_CONCEPTS: [&str; 24] = [
    "cefr_a1",
    "a1_vocabulary",
    "a1_grammar",
    "a1_expression",
    "cefr_a2",
    "a2_vocabulary",
    "a2_grammar",
    "a2_expression",
    "cefr_b1",
    "b1_vocabulary",
    "b1_grammar",
    "b1_expression",
    "cefr_b2",
    "b2_vocabulary",
    "b2_grammar",
    "b2_expression",
    "cefr_c1",
    "c1_vocabulary",
    "c1_grammar",
    "c1_expression",
    "cefr_c2",
    "c2_vocabulary",
    "c2_grammar",
    "c2_expression",
];

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn engine_for_pack(relative: &str) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path(relative)).unwrap();
    engine
}

fn seed_mastery(engine: &Engine, concept_id: &str, p_known: f64) {
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, attempt_count, max_depth, updated_at)
             VALUES (?1, ?2, 1, 'apply', '2026-06-13T00:00:00Z')
             ON CONFLICT(concept_id) DO UPDATE SET
                p_known=excluded.p_known,
                attempt_count=excluded.attempt_count,
                max_depth=excluded.max_depth",
            params![concept_id, p_known],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT OR IGNORE INTO attempts(id, concept_id, task_type, final_score, depth, created_at)
             VALUES (?1, ?2, 'apply', ?3, 'apply', '2026-06-13T00:00:00Z')",
            params![format!("seed-{concept_id}"), concept_id, p_known],
        )
        .unwrap();
}

#[test]
fn english_pack_validates_expected_cefr_shape() {
    let report = validate_pack_path(workspace_path("examples/packs/english")).unwrap();
    let pack_toml = fs::read_to_string(workspace_path("examples/packs/english/pack.toml")).unwrap();
    let rubric = fs::read_to_string(workspace_path("examples/packs/english/rubric.md")).unwrap();

    assert_eq!(report.concept_count, 24);
    assert!(report.prerequisite_count >= 18);
    assert!(report.misconception_count >= 8);
    assert!(pack_toml.contains("CEFR-J Vocabulary Profile"));
    assert!(pack_toml.contains("cefrj-grammar-profile-20180315.csv"));
    assert!(rubric.contains("strict-citation"));
}

#[test]
fn english_pack_initializes_and_schedules_domain_concepts() {
    let engine = engine_for_pack("examples/packs/english");

    let concept_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE pack='english'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let schema_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE pack='english' AND kind='schema'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(concept_count, 24);
    assert_eq!(schema_count, 6);

    let task = engine
        .next_task()
        .unwrap()
        .expect("english pack should produce a task");
    assert!(
        ENGLISH_CONCEPTS.contains(&task.concept_id.as_str()),
        "next_task returned non-English concept {}",
        task.concept_id
    );
    assert!(
        task.prompt_text.contains("CEFR") || task.prompt_text.contains("English"),
        "language move prompt should carry English/CEFR context: {}",
        task.prompt_text
    );
}

#[test]
fn cefr_prerequisite_gate_keeps_c1_c2_out_until_intermediate_ready() {
    let engine = engine_for_pack("examples/packs/english");

    for concept_id in [
        "cefr_a1",
        "a1_vocabulary",
        "a1_grammar",
        "a1_expression",
        "cefr_a2",
        "a2_vocabulary",
        "a2_grammar",
        "a2_expression",
    ] {
        seed_mastery(&engine, concept_id, 0.91);
    }

    let task = engine
        .next_task()
        .unwrap()
        .expect("scheduler should offer a post-A2 English task");

    assert!(
        task.concept_id == "cefr_b1" || task.concept_id.starts_with("b1_"),
        "cold-start map should enter B1 after A2, got {}",
        task.concept_id
    );
    assert!(
        !task.concept_id.starts_with("c1_")
            && !task.concept_id.starts_with("c2_")
            && task.concept_id != "cefr_c1"
            && task.concept_id != "cefr_c2",
        "C1/C2 must wait for intermediate prerequisites"
    );
}

#[test]
fn failed_english_attempt_with_misconception_raises_repair_priority() {
    let engine = engine_for_pack("examples/packs/english");

    for concept_id in [
        "cefr_a1",
        "a1_vocabulary",
        "a1_grammar",
        "a1_expression",
        "cefr_a2",
        "a2_vocabulary",
        "a2_grammar",
        "a2_expression",
        "cefr_b1",
        "b1_vocabulary",
        "b1_grammar",
        "b1_expression",
        "cefr_b2",
        "b2_vocabulary",
    ] {
        seed_mastery(&engine, concept_id, 0.88);
    }
    seed_mastery(&engine, "b2_grammar", 0.55);

    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, task_type, final_score, misconception_id, depth, created_at)
             VALUES ('english-tense-misconception-attempt', 'b2_grammar', 'apply', 0.2, 'tense_as_time_only', 'apply', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();

    let task = engine
        .next_task()
        .unwrap()
        .expect("scheduler should prioritize an active English misconception");

    assert_eq!(task.concept_id, "b2_grammar");
}

#[test]
fn english_and_rust_packs_share_submit_grade_mastery_shape() {
    let mut english = engine_for_pack("examples/packs/english");
    let mut rust = engine_for_pack("packs/rust");

    let english_receipt = english
        .submit(SubmitInput {
            session_id: "english-session".to_owned(),
            concept_id: "b1_grammar".to_owned(),
            task_type: "apply".to_owned(),
            prompt_text: "Rewrite the sentence using the present perfect.".to_owned(),
            response_text: "I have lived here for three years.".to_owned(),
            self_confidence: 4,
            latency_ms: 1300,
            hint_count: 0,
        })
        .unwrap();
    english
        .apply_final_score(&english_receipt.attempt_id, 0.82)
        .unwrap();

    let rust_receipt = rust
        .submit(SubmitInput {
            session_id: "rust-session".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "apply".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership determines which binding is responsible for a value."
                .to_owned(),
            self_confidence: 4,
            latency_ms: 1300,
            hint_count: 0,
        })
        .unwrap();
    rust.apply_final_score(&rust_receipt.attempt_id, 0.82)
        .unwrap();

    let english_state = english
        .mastery_state("b1_grammar")
        .unwrap()
        .expect("english attempt should create mastery state");
    let rust_state = rust
        .mastery_state("ownership")
        .unwrap()
        .expect("rust attempt should create mastery state");

    assert_eq!(english_state.attempt_count, 1);
    assert_eq!(rust_state.attempt_count, 1);
    assert!(english_state.p_known > 0.2);
    assert!(rust_state.p_known > 0.2);
}
