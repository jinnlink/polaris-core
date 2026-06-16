use std::path::{Path, PathBuf};

use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::mirt::decode_vector;
use polaris_core::pack_state::ThetaMode;
use rusqlite::Connection;

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

#[test]
fn pack_switch_filters_next_batch_and_status_to_active_pack() {
    let engine = engine_with_rust_and_algorithms();

    let packs = engine.list_packs().unwrap();
    assert_eq!(packs.len(), 2);
    assert_eq!(packs[0].id, "algorithms");
    assert_eq!(packs[1].id, "rust");
    assert!(packs.iter().find(|pack| pack.id == "rust").unwrap().active);

    let receipt = engine
        .switch_pack("algorithms", Some(ThetaMode::Isolated))
        .unwrap();
    assert_eq!(receipt.active_pack, "algorithms");
    assert_eq!(receipt.theta_mode, "isolated");

    let next = engine.next_task().unwrap().expect("active pack task");
    assert!(
        ALGORITHM_CONCEPTS.contains(&next.concept_id.as_str()),
        "next_task must stay inside active pack, got {}",
        next.concept_id
    );

    let batch = engine.get_interleaved_batch(3).unwrap();
    assert!(!batch.is_empty());
    assert!(
        batch
            .iter()
            .all(|item| ALGORITHM_CONCEPTS.contains(&item.concept_id.as_str())),
        "batch must stay inside active pack: {batch:?}"
    );

    let status = engine.status_snapshot().unwrap();
    assert_eq!(status.current_pack.as_deref(), Some("algorithms"));
    assert_eq!(status.theta_mode.as_deref(), Some("isolated"));
    assert_eq!(status.concepts.len(), ALGORITHM_CONCEPTS.len());
    assert!(status
        .concepts
        .iter()
        .all(|item| ALGORITHM_CONCEPTS.contains(&item.concept_id.as_str())));
}

#[test]
fn isolated_theta_updates_pack_theta_without_touching_shared_theta() {
    let mut engine = engine_with_rust_and_algorithms();
    engine
        .switch_pack("algorithms", Some(ThetaMode::Isolated))
        .unwrap();

    let shared_before = shared_theta(&engine);
    let isolated_before = pack_theta(&engine, "algorithms");
    let receipt = submit_and_grade(&mut engine, "complexity_basics", "algorithms-session", 0.92);

    let shared_after = shared_theta(&engine);
    let isolated_after = pack_theta(&engine, "algorithms");
    assert_eq!(
        shared_after, shared_before,
        "isolated mode must not update global shared theta"
    );
    assert_ne!(
        isolated_after, isolated_before,
        "isolated mode must update pack-specific theta"
    );
    assert_eq!(theta_scope(&engine, &receipt.attempt_id), "pack:algorithms");
}

#[test]
fn isolated_theta_attempts_replay_into_nightly_residual_stats() {
    let mut engine = engine_with_rust_and_algorithms();
    engine
        .switch_pack("algorithms", Some(ThetaMode::Isolated))
        .unwrap();
    submit_and_grade(&mut engine, "complexity_basics", "algorithms-session", 0.92);

    let summary = engine.run_nightly_consolidation().unwrap();

    assert!(summary.residual_rows > 0);
    let residual_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COALESCE(SUM(n), 0) FROM residual_stats WHERE concept_id='complexity_basics'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(residual_count, 1);
}

#[test]
fn shared_theta_mode_preserves_global_theta_updates() {
    let mut engine = engine_with_rust_and_algorithms();
    engine.switch_pack("rust", Some(ThetaMode::Shared)).unwrap();

    let shared_before = shared_theta(&engine);
    let receipt = submit_and_grade(&mut engine, "ownership", "rust-session", 0.86);
    let shared_after = shared_theta(&engine);

    assert_ne!(
        shared_after, shared_before,
        "shared mode must keep using global theta"
    );
    assert_eq!(theta_scope(&engine, &receipt.attempt_id), "shared");
}

#[test]
fn invalid_active_pack_meta_is_not_silent() {
    let engine = engine_with_rust_and_algorithms();
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('active_pack', 'missing-pack')",
            [],
        )
        .unwrap();

    let error = engine.status_snapshot().unwrap_err().to_string();

    assert!(error.contains("active_pack"));
    assert!(error.contains("pack not installed: missing-pack"));
}

#[test]
fn init_pack_rejects_concept_id_collision_across_packs() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path("packs/rust")).unwrap();

    let root = temp_pack_dir("collision");
    write_collision_pack(&root);

    let error = engine.init_pack(&root).unwrap_err().to_string();

    assert!(error.contains("concept id collision"));
    let collision_rows: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE pack='collision'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(collision_rows, 0);
}

fn engine_with_rust_and_algorithms() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path("packs/rust")).unwrap();
    engine
        .init_pack(workspace_path("packs/algorithms"))
        .unwrap();
    engine
}

fn submit_and_grade(
    engine: &mut Engine,
    concept_id: &str,
    session_id: &str,
    final_score: f64,
) -> polaris_core::engine::SubmitReceipt {
    let receipt = engine
        .submit(SubmitInput {
            session_id: session_id.to_owned(),
            concept_id: concept_id.to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: format!("Explain {concept_id}."),
            response_text: format!("{concept_id} answer with enough evidence."),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();
    engine
        .apply_final_score(&receipt.attempt_id, final_score)
        .unwrap();
    receipt
}

fn shared_theta(engine: &Engine) -> Vec<f64> {
    let blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    decode_vector(&blob).unwrap()
}

fn pack_theta(engine: &Engine, pack: &str) -> Vec<f64> {
    let blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT vec FROM pack_theta WHERE pack=?1", [pack], |row| {
            row.get(0)
        })
        .unwrap();
    decode_vector(&blob).unwrap()
}

fn theta_scope(engine: &Engine, attempt_id: &str) -> String {
    engine
        .conn()
        .query_row(
            "SELECT theta_scope FROM attempts WHERE id=?1",
            [attempt_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn write_collision_pack(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("pack.toml"),
        "id = \"collision\"\ntitle = \"Collision\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("concepts.toml"),
        r#"
[[concept]]
id = "ownership"
name = "Collision Ownership"
kind = "concept"
seed_order = 1
"#,
    )
    .unwrap();
    std::fs::write(root.join("misconceptions.toml"), "misconception = []\n").unwrap();
    std::fs::write(
        root.join("moves.toml"),
        r#"
[[move]]
id = "recall"
task_type = "recall"
template = "Explain {concept}."
"#,
    )
    .unwrap();
    std::fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_pack_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "polaris-core-p08a-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
