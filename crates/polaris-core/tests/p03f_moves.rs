use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use polaris_core::config::{default_registry, ParameterClass, TuningRoute};
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::grader::{grade_with_static_response, GradeRequest};
use polaris_core::pack::validate_pack_path;
use rusqlite::Connection;

#[test]
fn rust_pack_declares_seven_bloom_moves_and_validates() {
    let pack_path = workspace_pack_path("packs/rust");
    let report = validate_pack_path(&pack_path).unwrap();
    assert!(report.concept_count >= 24);

    let moves = read_moves(&pack_path);
    assert_eq!(
        moves.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["analyze", "apply", "create", "evaluate", "explain", "recall", "transfer"]
    );
    assert_move(&moves, "recall", "recall");
    assert_move(&moves, "explain", "free_explain");
    assert_move(&moves, "apply", "apply");
    assert_move(&moves, "analyze", "analyze");
    assert_move(&moves, "evaluate", "evaluate");
    assert_move(&moves, "create", "create");
    assert_move(&moves, "transfer", "transfer");
}

#[test]
fn legacy_three_move_pack_still_validates() {
    let root = temp_pack_dir("legacy-three-move");
    write_pack(&root, legacy_moves_toml());

    let result = validate_pack_path(&root);

    let _ = fs::remove_dir_all(&root);
    assert!(result.is_ok(), "legacy pack should validate: {result:?}");
}

#[test]
fn new_move_task_types_have_registered_mirt_difficulties() {
    let registry = default_registry();
    for (key, expected) in [
        ("mirt.d.analyze", "0.40"),
        ("mirt.d.evaluate", "0.45"),
        ("mirt.d.create", "0.50"),
        ("mirt.d.transfer", "0.50"),
    ] {
        let spec = registry.get(key).unwrap_or_else(|| panic!("missing {key}"));
        assert_eq!(spec.default_value, expected);
        assert_eq!(spec.class, ParameterClass::B);
        assert_eq!(spec.tuning_route, TuningRoute::Replay);
    }

    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let apply = engine.latent_prediction("ownership", "apply").unwrap();
    let analyze = engine.latent_prediction("ownership", "analyze").unwrap();
    let evaluate = engine.latent_prediction("ownership", "evaluate").unwrap();
    let create = engine.latent_prediction("ownership", "create").unwrap();
    let transfer = engine.latent_prediction("ownership", "transfer").unwrap();

    assert!(analyze.p_hat < apply.p_hat);
    assert!(evaluate.p_hat < analyze.p_hat);
    assert!((create.p_hat - transfer.p_hat).abs() < 1e-9);
}

#[test]
fn next_task_advances_from_apply_to_analyze_evaluate_and_falls_back_when_weak() {
    let root = temp_pack_dir("scheduler-seven-move");
    write_pack(&root, seven_moves_toml());
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(&root).unwrap();
    disable_mrt(&engine);

    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, max_depth, attempt_count, updated_at)
             VALUES ('core', 0.65, 'apply', 3, '2026-06-12T00:00:00Z')",
            [],
        )
        .unwrap();

    let first = engine.next_task().unwrap().expect("next task");
    assert_eq!(first.concept_id, "core");
    assert_eq!(first.task_type, "analyze");
    assert!(first.prompt_text.contains("trade-off"));

    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, task_type, final_score, depth, created_at)
             VALUES ('a-analyze', 'core', 'analyze', 0.80, 'analyze', '2026-06-12T00:00:00Z')",
            [],
        )
        .unwrap();
    let second = engine.next_task().unwrap().expect("next task");
    assert_eq!(second.task_type, "evaluate");

    engine
        .conn()
        .execute(
            "UPDATE mastery_states SET p_known=0.49, max_depth='create' WHERE concept_id='core'",
            [],
        )
        .unwrap();
    let fallback = engine.next_task().unwrap().expect("next task");
    assert_eq!(fallback.task_type, "recall");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn next_task_follows_full_bloom_depth_ladder() {
    let root = temp_pack_dir("scheduler-full-ladder");
    write_pack(&root, seven_moves_toml());
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(&root).unwrap();
    disable_mrt(&engine);

    assert_eq!(
        engine.next_task().unwrap().expect("initial task").task_type,
        "recall"
    );

    upsert_depth(engine.conn(), "recall", 0.65);
    assert_eq!(
        engine.next_task().unwrap().expect("recall next").task_type,
        "free_explain"
    );

    upsert_depth(engine.conn(), "explain", 0.65);
    assert_eq!(
        engine.next_task().unwrap().expect("explain next").task_type,
        "apply"
    );

    upsert_depth(engine.conn(), "analyze", 0.65);
    assert_eq!(
        engine.next_task().unwrap().expect("analyze next").task_type,
        "create"
    );

    upsert_depth(engine.conn(), "evaluate", 0.65);
    assert_eq!(
        engine
            .next_task()
            .unwrap()
            .expect("evaluate next")
            .task_type,
        "create"
    );

    upsert_depth(engine.conn(), "create", 0.65);
    assert_eq!(
        engine.next_task().unwrap().expect("create next").task_type,
        "transfer"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn teaching_instruction_exposes_target_depth_for_bloom_moves() {
    let root = temp_pack_dir("teaching-seven-move");
    write_pack(&root, seven_moves_toml());
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(&root).unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, max_depth, attempt_count, updated_at)
             VALUES ('core', 0.65, 'apply', 3, '2026-06-12T00:00:00Z')",
            [],
        )
        .unwrap();

    let instruction = engine.teaching_instruction("core").unwrap();

    assert_eq!(instruction.move_name, "analyze");
    assert_eq!(instruction.target_depth, "analyze");
    assert_eq!(instruction.target, "core");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn grader_accepts_new_bloom_depths() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
         VALUES ('core', 'test', 'Core', 'concept', 1, 0.20, 'pack-seed', '[]', '2026-06-12T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO meta(key, value) VALUES ('pack.test.rubric', '# Rubric\n## create\nRequire design rationale.')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
         VALUES ('ev1', 's1', 'cli-submit', 'text/plain', 'I propose a small API and explain the ownership trade-off.', '[\"core\"]', '2026-06-12T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, response_evidence_id, self_confidence, provisional_score, created_at)
         VALUES ('attempt-create', 's1', 'core', 'create', 'ev1', 4, 0.70, '2026-06-12T00:00:00Z')",
        [],
    )
    .unwrap();

    let result = grade_with_static_response(
        &conn,
        GradeRequest {
            attempt_id: "attempt-create".to_owned(),
            self_confidence: 4,
            response_text: "I propose a small API and explain the ownership trade-off.".to_owned(),
        },
        r#"{"score":0.86,"depth":"create","citations":[{"evidence_id":"ev1","quote":"small API"}]}"#,
    )
    .unwrap();

    assert!(!result.degraded);
    assert_eq!(result.depth, "create");
}

fn assert_move(moves: &BTreeMap<String, String>, id: &str, task_type: &str) {
    assert_eq!(
        moves.get(id).map(String::as_str),
        Some(task_type),
        "move {id} should map to {task_type}"
    );
}

fn read_moves(root: &Path) -> BTreeMap<String, String> {
    let text = fs::read_to_string(root.join("moves.toml")).unwrap();
    let value: toml::Value = toml::from_str(&text).unwrap();
    value["move"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            let id = item["id"].as_str().unwrap().to_owned();
            let task_type = item["task_type"].as_str().unwrap().to_owned();
            assert!(!item["template"].as_str().unwrap().trim().is_empty());
            (id, task_type)
        })
        .collect()
}

fn write_pack(root: &Path, moves_toml: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("pack.toml"),
        "id = \"test\"\ntitle = \"Test Pack\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("concepts.toml"),
        r#"
[[concept]]
id = "core"
name = "Core"
seed_order = 1
"#,
    )
    .unwrap();
    fs::write(root.join("misconceptions.toml"), "misconception = []\n").unwrap();
    fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
    fs::write(root.join("moves.toml"), moves_toml).unwrap();
}

fn legacy_moves_toml() -> &'static str {
    r#"
[[move]]
id = "recall"
template = "Recall {concept}."

[[move]]
id = "explain"
template = "Explain {concept}."

[[move]]
id = "apply"
template = "Apply {concept}."
"#
}

fn seven_moves_toml() -> &'static str {
    r#"
[[move]]
id = "recall"
task_type = "recall"
template = "Recall {concept}."

[[move]]
id = "explain"
task_type = "free_explain"
template = "Explain {concept}."

[[move]]
id = "apply"
task_type = "apply"
template = "Apply {concept}."

[[move]]
id = "analyze"
task_type = "analyze"
template = "Analyze two {concept} designs and name one trade-off."

[[move]]
id = "evaluate"
task_type = "evaluate"
template = "Evaluate {concept} code and identify one bug."

[[move]]
id = "create"
task_type = "create"
template = "Create a small {concept} design."

[[move]]
id = "transfer"
task_type = "transfer"
template = "Transfer {concept} into a different context."
"#
}

fn temp_pack_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "polaris-core-p03f-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn upsert_depth(conn: &Connection, max_depth: &str, p_known: f64) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, max_depth, attempt_count, updated_at)
         VALUES ('core', ?1, ?2, 3, '2026-06-12T00:00:00Z')
         ON CONFLICT(concept_id) DO UPDATE SET p_known=excluded.p_known, max_depth=excluded.max_depth",
        (p_known, max_depth),
    )
    .unwrap();
}

fn workspace_pack_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn disable_mrt(engine: &Engine) {
    engine
        .conn()
        .execute("UPDATE meta SET value='0.0' WHERE key='mrt.epsilon'", [])
        .unwrap();
}
