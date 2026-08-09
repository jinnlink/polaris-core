use std::fs;
use std::path::{Path, PathBuf};

use polaris_core::config::{default_registry, ParameterClass, TuningRoute};
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::{params, Connection};

#[test]
fn sustained_underconfidence_changes_next_and_batch_to_explain() {
    let root = temp_pack_dir("action");
    write_pack(&root, &["weak", "underconfident", "mastered", "review"]);
    let engine = engine_for_pack(&root);
    seed_state(&engine, "weak", 0.30, 2, 0.0, None);
    seed_state(&engine, "underconfident", 0.90, 2, -0.30, None);
    seed_state(&engine, "mastered", 0.90, 2, 0.0, None);
    seed_state(&engine, "review", 0.90, 2, 0.0, None);

    let batch = engine.get_interleaved_batch(4).unwrap();
    let calibrated = batch
        .iter()
        .find(|item| item.concept_id == "underconfident")
        .expect("underconfident concept in batch");
    let ordinary = batch
        .iter()
        .find(|item| item.concept_id == "mastered")
        .expect("ordinary mastered concept in batch");
    assert_eq!(calibrated.move_name, "explain");
    assert_eq!(ordinary.move_name, "recall");

    let solo_root = temp_pack_dir("next");
    write_pack(&solo_root, &["underconfident"]);
    let solo = engine_for_pack(&solo_root);
    seed_state(&solo, "underconfident", 0.90, 2, -0.30, None);
    let next = solo.next_task().unwrap().expect("next task");
    assert_eq!(next.move_id, "explain");
    assert!(next.reason.contains("低自信校准"));
    let strategy: String = solo
        .conn()
        .query_row(
            "SELECT json_extract(context_json, '$.phase_strategy')
             FROM mrt_log ORDER BY at DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(strategy, "underconfidence_calibration");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(solo_root);
}

#[test]
fn underconfidence_gate_requires_mastery_evidence_and_gap() {
    for (name, p_known, attempts, gap) in [
        ("not-mastered", 0.70, 2, -0.30),
        ("too-few", 0.90, 1, -0.30),
        ("within-gap", 0.90, 2, -0.20),
    ] {
        let root = temp_pack_dir(name);
        write_pack(&root, &[name]);
        let engine = engine_for_pack(&root);
        seed_state(&engine, name, p_known, attempts, gap, None);

        let next = engine.next_task().unwrap().expect("next task");
        assert_eq!(next.move_id, "recall", "unexpected action for {name}");
        assert!(!next.reason.contains("低自信校准"));
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn underconfidence_action_advances_one_step_without_exceeding_transfer() {
    let root = temp_pack_dir("deeper");
    write_pack(&root, &["underconfident"]);
    let engine = engine_for_pack(&root);
    seed_state(&engine, "underconfident", 0.90, 2, -0.30, Some("explain"));

    let next = engine.next_task().unwrap().expect("next task");
    assert_eq!(next.move_id, "analyze");
    assert!(next.reason.contains("低自信校准"));

    seed_state(&engine, "underconfident", 0.90, 2, -0.30, Some("transfer"));
    let capped = engine.next_task().unwrap().expect("next task");
    assert_eq!(capped.move_id, "transfer");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn underconfidence_threshold_is_a_manual_governance_parameter() {
    let registry = default_registry();
    let spec = registry
        .get("calib.underconfidence_gap")
        .expect("underconfidence parameter");
    assert_eq!(spec.default_value, "0.25");
    assert_eq!(spec.class, ParameterClass::A);
    assert_eq!(spec.bounds, Some("[0.15,0.40]"));
    assert_eq!(spec.tuning_route, TuningRoute::Manual);
}

fn engine_for_pack(root: &Path) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(root).unwrap();
    engine
        .conn()
        .execute("UPDATE meta SET value='0' WHERE key='mrt.epsilon'", [])
        .unwrap();
    engine
}

fn seed_state(
    engine: &Engine,
    concept_id: &str,
    p_known: f64,
    attempt_count: i64,
    calib_gap: f64,
    max_depth: Option<&str>,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(
                 concept_id, p_known, calib_gap, attempt_count, max_depth, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, '2026-08-09T00:00:00Z')
             ON CONFLICT(concept_id) DO UPDATE SET
                 p_known=excluded.p_known,
                 calib_gap=excluded.calib_gap,
                 attempt_count=excluded.attempt_count,
                 max_depth=excluded.max_depth",
            params![concept_id, p_known, calib_gap, attempt_count, max_depth],
        )
        .unwrap();
    for index in 0..attempt_count {
        engine
            .conn()
            .execute(
                "INSERT OR IGNORE INTO attempts(
                     id, concept_id, task_type, self_confidence, final_score, created_at
                 ) VALUES (?1, ?2, 'recall', 1, 0.95, '2026-08-09T00:00:00Z')",
                params![format!("{concept_id}-{index}"), concept_id],
            )
            .unwrap();
    }
}

fn write_pack(root: &Path, concept_ids: &[&str]) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("pack.toml"),
        "id = \"p16g1\"\ntitle = \"P16G1\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let concepts = concept_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            format!(
                "[[concept]]\nid = \"{id}\"\nname = \"{id}\"\nseed_order = {}\n",
                index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(root.join("concepts.toml"), concepts).unwrap();
    fs::write(root.join("misconceptions.toml"), "misconception = []\n").unwrap();
    fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
    fs::write(
        root.join("moves.toml"),
        r#"[[move]]
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
"#,
    )
    .unwrap();
}

fn temp_pack_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "polaris-core-p16g1-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
