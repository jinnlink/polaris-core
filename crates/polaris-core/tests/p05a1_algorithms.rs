use std::path::{Path, PathBuf};

use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::pack::validate_pack_path;
use rusqlite::{params, Connection};

const ALGORITHM_CONCEPTS: [&str; 17] = [
    "complexity_basics",
    "arrays_lists",
    "stacks_queues",
    "hash_tables",
    "trees_basics",
    "bst",
    "heaps",
    "graphs_repr",
    "comparison_sorts",
    "merge_sort",
    "quicksort",
    "bfs_dfs",
    "shortest_path",
    "dynamic_programming",
    "greedy",
    "divide_conquer",
    "backtracking",
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
}

#[test]
fn algorithms_pack_validates_expected_shape() {
    let report = validate_pack_path(workspace_path("packs/algorithms")).unwrap();

    assert_eq!(report.concept_count, 17);
    assert!(report.prerequisite_count >= 16);
    assert!(report.misconception_count >= 8);
}

#[test]
fn algorithms_pack_initializes_and_schedules_domain_concepts() {
    let engine = engine_for_pack("packs/algorithms");

    let concept_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE pack='algorithms'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(concept_count, 17);

    let task = engine
        .next_task()
        .unwrap()
        .expect("algorithms pack should produce a task");
    assert!(
        ALGORITHM_CONCEPTS.contains(&task.concept_id.as_str()),
        "next_task returned non-algorithms concept {}",
        task.concept_id
    );
}

#[test]
fn prerequisite_gate_keeps_advanced_concepts_out_until_ready() {
    let engine = engine_for_pack("packs/algorithms");

    for concept_id in [
        "complexity_basics",
        "arrays_lists",
        "trees_basics",
        "graphs_repr",
        "comparison_sorts",
        "merge_sort",
        "quicksort",
        "dynamic_programming",
        "greedy",
        "divide_conquer",
        "backtracking",
    ] {
        seed_mastery(&engine, concept_id, 0.91);
    }
    seed_mastery(&engine, "bfs_dfs", 0.52);
    seed_mastery(&engine, "heaps", 0.55);

    let task = engine
        .next_task()
        .unwrap()
        .expect("scheduler should still have legal tasks");

    assert_ne!(
        task.concept_id, "shortest_path",
        "shortest_path must wait until both bfs_dfs and heaps are mastered"
    );
}

#[test]
fn failed_attempt_with_misconception_raises_repair_priority() {
    let engine = engine_for_pack("packs/algorithms");

    seed_mastery(&engine, "complexity_basics", 0.95);
    seed_mastery(&engine, "arrays_lists", 0.95);
    seed_mastery(&engine, "graphs_repr", 0.95);
    seed_mastery(&engine, "greedy", 0.55);
    seed_mastery(&engine, "bfs_dfs", 0.55);

    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, task_type, final_score, misconception_id, depth, created_at)
             VALUES ('greedy-misconception-attempt', 'greedy', 'apply', 0.2, 'greedy_always_optimal', 'apply', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();

    let task = engine
        .next_task()
        .unwrap()
        .expect("scheduler should prioritize an active misconception");

    assert_eq!(task.concept_id, "greedy");
}

#[test]
fn algorithms_and_rust_packs_share_submit_grade_mastery_shape() {
    let mut algorithms = engine_for_pack("packs/algorithms");
    let mut rust = engine_for_pack("packs/rust");

    let algorithms_receipt = algorithms
        .submit(SubmitInput {
            session_id: "algorithms-session".to_owned(),
            concept_id: "complexity_basics".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain Big-O notation.".to_owned(),
            response_text: "Big-O gives an upper bound on growth as input size changes.".to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();
    algorithms
        .apply_final_score(&algorithms_receipt.attempt_id, 0.82)
        .unwrap();

    let rust_receipt = rust
        .submit(SubmitInput {
            session_id: "rust-session".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership determines which binding is responsible for a value."
                .to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();
    rust.apply_final_score(&rust_receipt.attempt_id, 0.82)
        .unwrap();

    let algorithms_state = algorithms
        .mastery_state("complexity_basics")
        .unwrap()
        .expect("algorithms attempt should create mastery state");
    let rust_state = rust
        .mastery_state("ownership")
        .unwrap()
        .expect("rust attempt should create mastery state");

    assert_eq!(algorithms_state.attempt_count, 1);
    assert_eq!(rust_state.attempt_count, 1);
    assert!(algorithms_state.p_known > 0.2);
    assert!(rust_state.p_known > 0.2);
}
