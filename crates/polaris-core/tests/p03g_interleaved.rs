use std::fs;
use std::path::{Path, PathBuf};

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::pack::validate_pack_path;
use rusqlite::Connection;

#[test]
fn default_batch_interleaves_one_weak_and_two_diverse_reviews() {
    let root = temp_pack_dir("default");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "weak");
    assert!(batch[0].p_known < 0.6);
    assert!(batch[1].p_known >= 0.6);
    assert!(batch[2].p_known >= 0.6);
    assert_eq!(
        unique_count(batch.iter().map(|item| item.concept_id.as_str())),
        3
    );
    assert_eq!(
        batch[1].concept_id, "review_a",
        "highest-U review should occupy slot 1"
    );
    assert_eq!(
        batch[2].concept_id, "review_b",
        "slot 2 should skip the closer overlapping review when a diverse review exists"
    );
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gated_fatigue_batch_uses_only_easy_reviews() {
    let root = temp_pack_dir("fatigue");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_gated_state(&engine, "fatigued", true);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert!(batch.iter().all(|item| item.p_known >= 0.8));
    assert!(!batch.iter().any(|item| item.concept_id == "weak"));
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gated_fatigue_does_not_backfill_with_weak_when_easy_reviews_are_insufficient() {
    let root = temp_pack_dir("fatigue-insufficient");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.70, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.72, 10, "explain", 0.2);
    insert_gated_state(&engine, "fatigued", true);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 1);
    assert!(batch.iter().all(|item| item.p_known >= 0.8));
    assert!(!batch.iter().any(|item| item.p_known < 0.6));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ungated_mental_state_does_not_change_default_batch() {
    let root = temp_pack_dir("ungated");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_gated_state(&engine, "fatigued", false);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "weak");
    assert!(batch.iter().any(|item| item.concept_id == "weak"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn latest_ungated_mental_state_overrides_older_gated_state() {
    let root = temp_pack_dir("latest-ungated");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_mental_state_at(
        &engine,
        "older-gated",
        "fatigued",
        true,
        "2026-06-12T00:00:00Z",
    );
    insert_mental_state_at(
        &engine,
        "newer-ungated",
        "fatigued",
        false,
        "2026-06-12T00:01:00Z",
    );

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "weak");
    assert!(batch.iter().any(|item| item.concept_id == "weak"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn flow_batch_allows_two_weak_concepts() {
    let root = temp_pack_dir("flow");
    write_pack(&root, five_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "weak_two", 0.50, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_gated_state(&engine, "flow", true);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch.iter().filter(|item| item.p_known < 0.6).count(),
        2,
        "flow strategy should admit two weak/new slots"
    );
    assert_eq!(batch.iter().filter(|item| item.p_known >= 0.6).count(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn batch_replaces_out_of_band_review_to_keep_expected_success_in_target() {
    let root = temp_pack_dir("target-band");
    write_pack(&root, five_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "weak_two", 0.84, 10, "explain", 0.0);
    seed_mastery(&engine, "review_a", 0.99, 10, "explain", 0.4);
    seed_mastery(&engine, "review_b", 0.86, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.98, 10, "explain", 0.3);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "weak");
    assert!(batch.iter().any(|item| item.concept_id == "review_b"));
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_phantom_challenge_prefers_phantom_transfer() {
    let root = temp_pack_dir("phase-phantom");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery_with_phase(&engine, "review_a", 0.86, 10, "recall", 0.3, "phantom");
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "review_a");
    assert_eq!(batch[0].phase.as_str(), "phantom");
    assert_eq!(batch[0].move_name, "transfer");
    assert_eq!(batch[0].task_type, "transfer");
    assert!(batch[0].template.contains("different project or domain"));
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_settling_probe_prefers_settling_transfer_without_phantom() {
    let root = temp_pack_dir("phase-settling");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery_with_phase(&engine, "review_a", 0.86, 10, "apply", 0.3, "settling");
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "review_a");
    assert_eq!(batch[0].phase.as_str(), "settling");
    assert_eq!(batch[0].move_name, "transfer");
    assert_eq!(batch[0].task_type, "transfer");
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_regression_recovery_prefers_regression_at_most_explain() {
    let root = temp_pack_dir("phase-regression");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery_with_phase(&engine, "review_a", 0.86, 10, "create", 0.3, "regression");
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch[0].concept_id, "review_a");
    assert_eq!(batch[0].phase.as_str(), "regression");
    assert!(
        matches!(batch[0].move_name.as_str(), "recall" | "explain"),
        "regression recovery must not assign a high-friction move: {batch:?}"
    );
    assert!(
        matches!(
            batch[0].task_type.as_str(),
            "recall" | "free_explain" | "explain"
        ),
        "regression recovery task_type must stay at recall/explain: {batch:?}"
    );
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_easy_reviews_override_phase_challenge() {
    let root = temp_pack_dir("phase-fatigue");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery_with_phase(&engine, "weak", 0.55, 2, "recall", 0.4, "phantom");
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_gated_state(&engine, "fatigued", true);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert!(batch.iter().all(|item| item.p_known >= 0.8));
    assert!(!batch.iter().any(|item| item.concept_id == "weak"));
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_flow_strategy_keeps_existing_slot_shape() {
    let root = temp_pack_dir("phase-flow");
    write_pack(&root, five_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "weak_two", 0.50, 2, "recall", 0.0);
    seed_mastery_with_phase(&engine, "review_a", 0.86, 10, "recall", 0.4, "phantom");
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_gated_state(&engine, "flow", true);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch.iter().filter(|item| item.p_known < 0.6).count(),
        2,
        "flow strategy should keep two weak/new slots even when a review is phantom"
    );
    assert_eq!(batch.iter().filter(|item| item.p_known >= 0.6).count(), 1);
    if let Some(phantom) = batch.iter().find(|item| item.concept_id == "review_a") {
        assert_ne!(phantom.move_name, "transfer");
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_bored_easy_reviews_override_phase_challenge() {
    let root = temp_pack_dir("phase-bored");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery_with_phase(&engine, "weak", 0.55, 2, "recall", 0.4, "phantom");
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.3);
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    insert_gated_state(&engine, "bored", true);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 3);
    assert!(batch.iter().all(|item| item.p_known >= 0.8));
    assert!(!batch.iter().any(|item| item.concept_id == "weak"));
    assert_85_rule(&batch);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn phase_action_loop_returns_shorter_batch_when_target_band_is_unreachable() {
    let root = temp_pack_dir("phase-short");
    write_pack(&root, four_concept_pack());
    let engine = engine_for_pack(&root);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery_with_phase(&engine, "review_a", 0.61, 10, "recall", 0.3, "phantom");
    seed_mastery(&engine, "review_b", 0.84, 10, "explain", 0.1);
    seed_mastery(&engine, "review_close", 0.89, 10, "explain", 0.2);
    set_b_difficulty(&engine, "review_a", 3.0);

    let batch = engine.get_interleaved_batch(3).unwrap();

    assert!(
        batch.len() < 3,
        "unreachable 85% band should degrade to a shorter batch: {batch:?}"
    );
    assert!(batch.iter().any(|item| item.concept_id == "review_a"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fewer_than_three_concepts_degrades_to_existing_next_task() {
    let root = temp_pack_dir("small");
    write_pack(&root, two_concept_pack());
    let engine = engine_for_pack(&root);
    disable_mrt(&engine);
    seed_mastery(&engine, "weak", 0.55, 2, "recall", 0.0);
    seed_mastery(&engine, "review_a", 0.86, 10, "explain", 0.2);

    let next = engine.next_task().unwrap().expect("next task");
    let batch = engine.get_interleaved_batch(3).unwrap();

    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].concept_id, next.concept_id);
    assert_eq!(batch[0].task_type, next.task_type);
    assert_eq!(batch[0].template, next.prompt_text);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn p03g_pack_fixture_still_validates() {
    let root = temp_pack_dir("validate");
    write_pack(&root, four_concept_pack());

    let report = validate_pack_path(&root).unwrap();

    assert_eq!(report.concept_count, 4);
    let _ = fs::remove_dir_all(&root);
}

fn engine_for_pack(root: &Path) -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(root).unwrap();
    engine
}

fn disable_mrt(engine: &Engine) {
    engine
        .conn()
        .execute("UPDATE meta SET value='0.0' WHERE key='mrt.epsilon'", [])
        .unwrap();
}

fn seed_mastery(
    engine: &Engine,
    concept_id: &str,
    p_known: f64,
    attempt_count: i64,
    max_depth: &str,
    calib_gap: f64,
) {
    seed_mastery_with_phase(
        engine,
        concept_id,
        p_known,
        attempt_count,
        max_depth,
        calib_gap,
        "undetermined",
    );
}

fn seed_mastery_with_phase(
    engine: &Engine,
    concept_id: &str,
    p_known: f64,
    attempt_count: i64,
    max_depth: &str,
    calib_gap: f64,
    phase: &str,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, calib_gap, attempt_count, max_depth, phase, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-06-12T00:00:00Z')
             ON CONFLICT(concept_id) DO UPDATE SET
                p_known=excluded.p_known,
                calib_gap=excluded.calib_gap,
                attempt_count=excluded.attempt_count,
                max_depth=excluded.max_depth,
                phase=excluded.phase",
            (concept_id, p_known, calib_gap, attempt_count, max_depth, phase),
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "UPDATE concepts SET b_difficulty=-1.7 WHERE id=?1",
            [concept_id],
        )
        .unwrap();
    if attempt_count > 0 {
        engine
            .conn()
            .execute(
                "INSERT OR IGNORE INTO attempts(id, concept_id, task_type, final_score, depth, created_at)
                 VALUES (?1, ?2, 'recall', ?3, ?4, '2026-06-12T00:00:00Z')",
                (
                    format!("{concept_id}-seed-attempt"),
                    concept_id,
                    p_known,
                    max_depth,
                ),
            )
            .unwrap();
    }
}

fn set_b_difficulty(engine: &Engine, concept_id: &str, b_difficulty: f64) {
    engine
        .conn()
        .execute(
            "UPDATE concepts SET b_difficulty=?2 WHERE id=?1",
            (concept_id, b_difficulty),
        )
        .unwrap();
}

fn insert_gated_state(engine: &Engine, dominant_state: &str, strategy_enabled: bool) {
    insert_mental_state_at(
        engine,
        "mental-state-p03g",
        dominant_state,
        strategy_enabled,
        "2026-06-12T00:00:00Z",
    );
}

fn insert_mental_state_at(
    engine: &Engine,
    id: &str,
    dominant_state: &str,
    strategy_enabled: bool,
    at: &str,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, 's1', ?2, 'mental_state', 'weak', ?3)",
            (
                id,
                at,
                serde_json::json!({
                    "attempt_id": "manual",
                    "dominant_state": dominant_state,
                    "posterior": [0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
                    "strategy_enabled": strategy_enabled,
                    "hazard": {"participates": strategy_enabled, "probability": 0.85}
                })
                .to_string(),
            ),
        )
        .unwrap();
}

fn assert_85_rule(batch: &[polaris_core::engine::TaskAssignment]) {
    let mean = batch.iter().map(|item| item.expected_success).sum::<f64>() / batch.len() as f64;
    assert!(
        (0.75..=0.90).contains(&mean),
        "batch mean expected_success should be in [0.75,0.90], got {mean}: {batch:?}"
    );
}

fn unique_count<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    let mut values = items.collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values.len()
}

fn write_pack(root: &Path, concepts_toml: String) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("pack.toml"),
        "id = \"test\"\ntitle = \"Test Pack\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("concepts.toml"), concepts_toml).unwrap();
    fs::write(root.join("misconceptions.toml"), "misconception = []\n").unwrap();
    fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
    fs::write(
        root.join("moves.toml"),
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
"#,
    )
    .unwrap();
}

fn four_concept_pack() -> String {
    format!(
        "{concepts}{edges}",
        concepts = concepts(&["weak", "review_a", "review_b", "review_close"]),
        edges = edges()
    )
}

fn five_concept_pack() -> String {
    format!(
        "{concepts}{edges}",
        concepts = concepts(&["weak", "weak_two", "review_a", "review_b", "review_close"]),
        edges = edges()
    )
}

fn two_concept_pack() -> String {
    concepts(&["weak", "review_a"])
}

fn concepts(ids: &[&str]) -> String {
    ids.iter()
        .enumerate()
        .map(|(idx, id)| {
            format!(
                r#"
[[concept]]
id = "{id}"
name = "{name}"
seed_order = {seed_order}
"#,
                name = id.replace('_', " "),
                seed_order = idx + 1
            )
        })
        .collect()
}

fn edges() -> &'static str {
    r#"
[[edge]]
id = "a-close-1"
src = "review_a"
dst = "review_close"
type = "confusion"
weight = 1.0

[[edge]]
id = "a-close-2"
src = "weak"
dst = "review_a"
type = "prerequisite"
weight = 1.0
"#
}

fn temp_pack_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "polaris-core-p03g-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
