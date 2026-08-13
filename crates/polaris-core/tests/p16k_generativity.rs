mod common;

use common::workspace_pack_path;
use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_core::pack::{load_pack, validate_pack_path};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn legacy_packs_default_every_concept_to_unknown() {
    for path in ["packs/rust", "packs/algorithms", "examples/packs/english"] {
        let pack = load_pack(workspace_pack_path(path)).unwrap();
        assert!(pack
            .concepts
            .iter()
            .all(|concept| concept.generativity == "unknown"));
    }
}

#[test]
fn validator_rejects_invalid_generativity_enum() {
    let root = temp_dir("invalid-generativity");
    fs::create_dir_all(&root).unwrap();
    for file in [
        "pack.toml",
        "concepts.toml",
        "misconceptions.toml",
        "rubric.md",
        "moves.toml",
    ] {
        fs::copy(
            workspace_pack_path("packs/template").join(file),
            root.join(file),
        )
        .unwrap();
    }
    let concepts_path = root.join("concepts.toml");
    let concepts = fs::read_to_string(&concepts_path).unwrap().replacen(
        "generativity = \"generative\"",
        "generativity = \"magic\"",
        1,
    );
    fs::write(&concepts_path, concepts).unwrap();

    let error = validate_pack_path(&root).unwrap_err().to_string();

    let _ = fs::remove_dir_all(root);
    assert!(error.contains("invalid generativity magic"));
}

#[test]
fn generative_changes_only_teaching_prescription_while_item_matches_unknown() {
    let engine = seeded_engine();
    let unknown = engine.teaching_instruction("ownership").unwrap();
    let next_before = task_signature(&engine);

    set_generativity(&engine, "generative");
    let generative = engine.teaching_instruction("ownership").unwrap();
    let next_generative = task_signature(&engine);

    set_generativity(&engine, "item");
    let item = engine.teaching_instruction("ownership").unwrap();
    let next_item = task_signature(&engine);

    assert_eq!(generative.move_name, "transfer");
    assert_eq!(generative.target_depth, "transfer");
    assert!(generative.do_text.contains("没教过的同族实例"));
    assert_eq!(item, unknown);
    assert_ne!(generative, item);
    assert_eq!(next_before, next_generative);
    assert_eq!(next_before, next_item);
}

#[test]
fn schema_v7_defaults_existing_and_direct_concepts_to_unknown() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    assert_eq!(CURRENT_SCHEMA_VERSION, 10);
    conn.execute(
        "INSERT INTO concepts(id, name, seed_order) VALUES ('legacy', 'Legacy', 1)",
        [],
    )
    .unwrap();
    let value: String = conn
        .query_row(
            "SELECT generativity FROM concepts WHERE id='legacy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(value, "unknown");
    assert!(conn
        .execute(
            "UPDATE concepts SET generativity='invalid' WHERE id='legacy'",
            [],
        )
        .is_err());
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .conn()
        .execute("UPDATE meta SET value='0.0' WHERE key='mrt.epsilon'", [])
        .unwrap();
    engine
}

fn set_generativity(engine: &Engine, value: &str) {
    engine
        .conn()
        .execute(
            "UPDATE concepts SET generativity=?1 WHERE id='ownership'",
            [value],
        )
        .unwrap();
}

fn task_signature(engine: &Engine) -> (String, String, String, String, String) {
    let task = engine.next_task().unwrap().unwrap();
    (
        task.concept_id,
        task.move_id,
        task.task_type,
        task.prompt_text,
        task.reason,
    )
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("polaris-{label}-{nonce}"))
}
