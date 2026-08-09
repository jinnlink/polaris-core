use std::fs;
use std::path::{Path, PathBuf};

use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::pack::{load_pack, validate_pack_path, PackError};
use rusqlite::{params, Connection};

#[test]
fn schema_v8_adds_material_layer_and_rolls_back_an_interrupted_unit() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    assert_eq!(CURRENT_SCHEMA_VERSION, 9);
    assert!(table_exists(&conn, "materials"));
    assert!(column_exists(&conn, "attempts", "material_id"));

    let broken = Connection::open_in_memory().unwrap();
    broken
        .execute_batch(
            r#"
            CREATE TABLE schema_migrations(
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );
            CREATE TABLE materials(id TEXT PRIMARY KEY);
            PRAGMA user_version=7;
            "#,
        )
        .unwrap();
    let error = migrate(&broken).unwrap_err().to_string();
    assert!(error.contains("pack"), "{error}");
    assert_eq!(pragma_version(&broken), 7);
    assert_eq!(migration_row_count(&broken, 8), 0);
    assert!(!column_exists(&broken, "attempts", "material_id"));
}

#[test]
fn packs_without_materials_are_unchanged_and_undeclared_levels_are_rejected() {
    for relative in ["packs/rust", "packs/algorithms", "examples/packs/english"] {
        let pack = load_pack(workspace_path(relative)).unwrap();
        assert!(pack.material_levels.is_empty(), "{relative}");
        assert!(pack.materials.is_empty(), "{relative}");
    }

    let root = temp_pack_dir("invalid-level");
    fs::create_dir_all(&root).unwrap();
    for file in [
        "pack.toml",
        "concepts.toml",
        "misconceptions.toml",
        "rubric.md",
        "moves.toml",
    ] {
        fs::copy(workspace_path("packs/template").join(file), root.join(file)).unwrap();
    }
    fs::write(
        root.join("materials.toml"),
        r#"
[levels]
order = ["starter"]

[[material]]
id = "bad_material"
kind = "lesson"
level = "advanced"
title = "Bad"
source_ref = "course://bad"
"#,
    )
    .unwrap();
    let error = validate_pack_path(&root).unwrap_err();
    fs::remove_dir_all(root).unwrap();
    assert!(matches!(error, PackError::MissingMaterialLevel { .. }));
}

#[test]
fn unknown_material_is_rejected_without_partial_writes() {
    let mut engine = template_engine();
    let before = learning_row_counts(&engine);
    let error = engine
        .submit_with_material(input("unknown-material"), Some("missing-material"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("material_id"), "{error}");
    assert_eq!(learning_row_counts(&engine), before);
}

#[test]
fn summaries_follow_declared_level_order_and_compute_first_success() {
    let engine = template_engine();
    for (material, concept, score, at) in [
        (
            "template_intro",
            "template_core_terms",
            0.80,
            "2026-01-01T00:00:00Z",
        ),
        (
            "template_intro",
            "template_core_terms",
            0.60,
            "2026-01-02T00:00:00Z",
        ),
        (
            "template_intro",
            "template_worked_example",
            0.50,
            "2026-01-03T00:00:00Z",
        ),
        (
            "template_exercises",
            "template_core_terms",
            0.90,
            "2026-01-04T00:00:00Z",
        ),
    ] {
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(
                     id, concept_id, task_type, final_score, material_id, created_at
                 ) VALUES (?1, ?2, 'recall', ?3, ?4, ?5)",
                params![format!("{material}-{at}"), concept, score, material, at],
            )
            .unwrap();
    }

    let report = engine.material_performance(Some("template")).unwrap();
    assert_eq!(
        report
            .by_level
            .iter()
            .map(|item| item.level.as_str())
            .collect::<Vec<_>>(),
        ["starter", "practice", "reference"]
    );
    let intro = report
        .by_material
        .iter()
        .find(|item| item.material_id == "template_intro")
        .unwrap();
    assert_eq!(intro.attempt_count, 3);
    assert_close(intro.average_final_score.unwrap(), 1.9 / 3.0);
    assert_close(intro.first_success_rate.unwrap(), 0.5);
    assert_eq!(report.by_level[2].attempt_count, 0);
    assert_eq!(report.by_level[2].average_final_score, None);
}

#[test]
fn material_id_is_record_only_and_null_preserves_the_previous_path() {
    let mut baseline = template_engine();
    let mut material = template_engine();

    let baseline_receipt = baseline.submit_provisional(input("baseline")).unwrap();
    let material_receipt = material
        .submit_provisional_with_material(input("material"), Some("template_intro"))
        .unwrap();
    baseline
        .apply_final_score(&baseline_receipt.attempt_id, 0.82)
        .unwrap();
    material
        .apply_final_score(&material_receipt.attempt_id, 0.82)
        .unwrap();

    let stored_material: Option<String> = material
        .conn()
        .query_row(
            "SELECT material_id FROM attempts WHERE id=?1",
            [&material_receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    let stored_null: Option<String> = baseline
        .conn()
        .query_row(
            "SELECT material_id FROM attempts WHERE id=?1",
            [&baseline_receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_material.as_deref(), Some("template_intro"));
    assert_eq!(stored_null, None);

    assert_eq!(mastery_signature(&baseline), mastery_signature(&material));
    assert_eq!(theta_signature(&baseline), theta_signature(&material));
    assert_eq!(
        baseline
            .latent_prediction("template_core_terms", "recall")
            .unwrap(),
        material
            .latent_prediction("template_core_terms", "recall")
            .unwrap()
    );
    for engine in [&baseline, &material] {
        engine
            .conn()
            .execute("UPDATE meta SET value='0' WHERE key='mrt.epsilon'", [])
            .unwrap();
    }
    let baseline_next = baseline.next_task().unwrap().unwrap();
    let material_next = material.next_task().unwrap().unwrap();
    assert_eq!(baseline_next.concept_id, material_next.concept_id);
    assert_eq!(baseline_next.move_id, material_next.move_id);
    assert_eq!(baseline_next.task_type, material_next.task_type);
    assert_eq!(baseline_next.prompt_text, material_next.prompt_text);
    assert_eq!(baseline_next.reason, material_next.reason);
}

fn template_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path("packs/template")).unwrap();
    engine
}

fn input(session: &str) -> SubmitInput {
    SubmitInput {
        session_id: session.to_owned(),
        concept_id: "template_core_terms".to_owned(),
        task_type: "recall".to_owned(),
        prompt_text: "解释核心术语。".to_owned(),
        response_text: "核心术语定义了边界。".to_owned(),
        self_confidence: 4,
        latency_ms: 1200,
        hint_count: 0,
    }
}

fn learning_row_counts(engine: &Engine) -> (i64, i64, i64, i64) {
    let counts = ["sessions", "evidence_items", "attempts", "mastery_states"].map(|table| {
        engine
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    });
    (counts[0], counts[1], counts[2], counts[3])
}

fn mastery_signature(engine: &Engine) -> (f64, i64, String) {
    engine
        .conn()
        .query_row(
            "SELECT p_known, attempt_count, phase FROM mastery_states WHERE concept_id='template_core_terms'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

fn theta_signature(engine: &Engine) -> (Vec<u8>, Vec<u8>, i64) {
    engine
        .conn()
        .query_row("SELECT vec, g2, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
    .unwrap()
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .any(|name| name.unwrap() == column)
}

fn migration_row_count(conn: &Connection, version: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=?1",
        [version],
        |row| row.get(0),
    )
    .unwrap()
}

fn pragma_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn temp_pack_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("polaris-p16l-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}
