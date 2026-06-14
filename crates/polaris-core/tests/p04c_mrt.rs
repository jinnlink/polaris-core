use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::Connection;
use serde_json::Value;

#[test]
fn next_task_writes_mrt_preregistration_audit() {
    let engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "0.0");
    seed_mastery(&engine, "ownership", 0.72, "recall", "phantom");

    let task = engine.next_task().unwrap().expect("task");

    assert_eq!(task.concept_id, "ownership");
    let audit = latest_mrt_log(&engine);
    assert_eq!(audit.randomized, 0);
    assert_eq!(audit.move_id, "explain");
    assert!(!audit.prereg_id.is_empty());
    assert_eq!(audit.context["window"], "7d");
    assert_eq!(audit.context["epsilon"], 0.0);
    assert_eq!(audit.context["context_hash"], "state:unknown|phase:phantom");
    assert_eq!(audit.context["candidate_set"][0], "explain");
    assert!(audit.context["main_effect_hypothesis"]
        .as_str()
        .unwrap()
        .contains("7d success"));
}

#[test]
fn cold_start_mrt_randomization_can_replace_the_base_move() {
    let engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "1.0");
    seed_mastery(&engine, "ownership", 0.72, "recall", "phantom");

    let task = engine.next_task().unwrap().expect("task");

    assert_ne!(task.task_type, "free_explain");
    let audit = latest_mrt_log(&engine);
    assert_eq!(audit.randomized, 1);
    assert_ne!(audit.move_id, "explain");
    assert_eq!(audit.context["candidate_set"][0], audit.move_id);
}

#[test]
fn signature_posterior_can_select_non_default_move_without_randomization() {
    let engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "0.0");
    set_meta(&engine, "friction.lambda", "0.5");
    seed_mastery(&engine, "ownership", 0.72, "recall", "phantom");
    seed_move_effect(
        &engine,
        "explain",
        "state:unknown|phase:phantom",
        1.0,
        9.0,
        8,
    );
    seed_move_effect(
        &engine,
        "apply",
        "state:unknown|phase:phantom",
        12.0,
        1.0,
        11,
    );

    let task = engine.next_task().unwrap().expect("task");

    assert_eq!(task.task_type, "apply");
    let audit = latest_mrt_log(&engine);
    assert_eq!(audit.randomized, 0);
    assert_eq!(audit.move_id, "apply");
    assert_eq!(audit.context["selected_by"], "signature_friction");
}

#[test]
fn forced_mrt_randomization_replaces_selected_move_and_marks_audit() {
    let engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "1.0");
    set_meta(&engine, "friction.lambda", "0.0");
    seed_mastery(&engine, "ownership", 0.72, "recall", "phantom");
    seed_move_effect(
        &engine,
        "explain",
        "state:unknown|phase:phantom",
        12.0,
        1.0,
        11,
    );
    seed_move_effect(&engine, "apply", "state:unknown|phase:phantom", 1.0, 9.0, 8);

    let task = engine.next_task().unwrap().expect("task");

    assert_ne!(task.task_type, "free_explain");
    let audit = latest_mrt_log(&engine);
    assert_eq!(audit.randomized, 1);
    assert_ne!(audit.move_id, "explain");
    assert_eq!(audit.context["incumbent"], "explain");
}

#[test]
fn seven_day_success_updates_the_preregistered_context() {
    let mut engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "0.0");
    seed_mastery(&engine, "ownership", 0.72, "explain", "phantom");
    let task = engine.next_task().unwrap().expect("task");
    assert_eq!(task.task_type, "apply");
    engine.record_next_task_event("s1", &task).unwrap();
    engine
        .conn()
        .execute(
            "UPDATE mastery_states SET phase='active' WHERE concept_id='ownership'",
            [],
        )
        .unwrap();
    seed_mental_state(&engine, "s1", "ownership", "flow");

    let receipt = engine
        .submit_provisional(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: task.task_type.clone(),
            prompt_text: task.prompt_text.clone(),
            response_text: "Ownership moves values unless borrowed.".to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.82).unwrap();

    let (alpha, beta, n): (f64, f64, i64) = engine
        .conn()
        .query_row(
            "SELECT alpha, beta, n FROM moves_effects WHERE move='apply'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((alpha, beta, n), (2.0, 1.0, 1));

    let outcome = outcome_log_for_prereg(&engine, &task.mrt_prereg_id);
    assert_eq!(outcome.context["kind"], "outcome");
    assert_eq!(outcome.context["outcome"], true);
    assert_eq!(outcome.context["context_hash"], task.mrt_context_hash);
    assert_eq!(outcome.move_id, task.move_id);
    assert_eq!(outcome.prereg_id, task.mrt_prereg_id);
}

#[test]
fn later_same_concept_success_settles_pending_preregistration() {
    let mut engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "0.0");
    seed_mastery(&engine, "ownership", 0.72, "explain", "phantom");
    let task = engine.next_task().unwrap().expect("task");
    engine.record_next_task_event("s1", &task).unwrap();

    let receipt = engine
        .submit_provisional(polaris_core::engine::SubmitInput {
            session_id: "s2".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Recall ownership.".to_owned(),
            response_text: "Ownership has one owner for a value.".to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.82).unwrap();

    let (alpha, beta, n): (f64, f64, i64) = engine
        .conn()
        .query_row(
            "SELECT alpha, beta, n FROM moves_effects WHERE move=?1 AND context_hash=?2",
            (task.move_id.as_str(), task.mrt_context_hash.as_str()),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((alpha, beta, n), (2.0, 1.0, 1));
    let outcome = outcome_log_for_prereg(&engine, &task.mrt_prereg_id);
    assert_eq!(outcome.context["source_attempt_id"], receipt.attempt_id);
}

#[test]
fn failing_attempt_waits_for_the_seven_day_window_before_beta_update() {
    let mut engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "0.0");
    seed_mastery(&engine, "ownership", 0.72, "explain", "phantom");
    let task = engine.next_task().unwrap().expect("task");
    engine.record_next_task_event("s1", &task).unwrap();
    let receipt = engine
        .submit_provisional(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: task.task_type.clone(),
            prompt_text: task.prompt_text.clone(),
            response_text: "Ownership still feels confusing.".to_owned(),
            self_confidence: 1,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.20).unwrap();

    let count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM moves_effects WHERE move=?1 AND context_hash=?2",
            (task.move_id.as_str(), task.mrt_context_hash.as_str()),
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn expired_window_without_success_records_failure() {
    let mut engine = rust_engine();
    set_meta(&engine, "mrt.epsilon", "0.0");
    seed_mastery(&engine, "ownership", 0.72, "explain", "phantom");
    let task = engine.next_task().unwrap().expect("task");
    engine.record_next_task_event("s1", &task).unwrap();
    engine
        .conn()
        .execute(
            "UPDATE behavior_events
             SET at=strftime('%Y-%m-%dT%H:%M:%SZ','now','-8 days')
             WHERE json_extract(payload_json, '$.mrt_prereg_id')=?1",
            [task.mrt_prereg_id.as_str()],
        )
        .unwrap();
    let receipt = engine
        .submit_provisional(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: task.task_type.clone(),
            prompt_text: task.prompt_text.clone(),
            response_text: "Ownership still feels confusing.".to_owned(),
            self_confidence: 1,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.20).unwrap();

    let (alpha, beta, n): (f64, f64, i64) = engine
        .conn()
        .query_row(
            "SELECT alpha, beta, n FROM moves_effects WHERE move=?1 AND context_hash=?2",
            (task.move_id.as_str(), task.mrt_context_hash.as_str()),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((alpha, beta, n), (1.0, 2.0, 1));
}

struct MrtAudit {
    context: Value,
    randomized: i64,
    move_id: String,
    prereg_id: String,
}

fn rust_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn set_meta(engine: &Engine, key: &str, value: &str) {
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            (key, value),
        )
        .unwrap();
}

fn seed_mastery(engine: &Engine, concept_id: &str, p_known: f64, max_depth: &str, phase: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(
                concept_id, p_known, fsrs_json, next_due_at, last_review_at,
                calib_gap, brier_ewma, last_depth, max_depth, phase,
                attempt_count, lapses, updated_at
             )
             VALUES (?1, ?2, NULL, NULL, strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                     0.0, 0.0, ?3, ?3, ?4, 4, 0, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            (concept_id, p_known, max_depth, phase),
        )
        .unwrap();
}

fn seed_move_effect(
    engine: &Engine,
    move_id: &str,
    context_hash: &str,
    alpha: f64,
    beta: f64,
    n: i64,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO moves_effects(move, context_hash, alpha, beta, n)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (move_id, context_hash, alpha, beta, n),
        )
        .unwrap();
}

fn seed_mental_state(engine: &Engine, session_id: &str, concept_id: &str, dominant_state: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (lower(hex(randomblob(16))), ?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                     'mental_state', ?2, ?3)",
            (
                session_id,
                concept_id,
                serde_json::json!({
                    "strategy_enabled": true,
                    "dominant_state": dominant_state
                })
                .to_string(),
            ),
        )
        .unwrap();
}

fn latest_mrt_log(engine: &Engine) -> MrtAudit {
    let (context_json, randomized, move_id, prereg_id): (String, i64, String, String) = engine
        .conn()
        .query_row(
            "SELECT context_json, randomized, move, prereg_id
             FROM mrt_log
             ORDER BY at DESC, id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    MrtAudit {
        context: serde_json::from_str(&context_json).unwrap(),
        randomized,
        move_id,
        prereg_id,
    }
}

fn outcome_log_for_prereg(engine: &Engine, prereg_id: &str) -> MrtAudit {
    let (context_json, randomized, move_id, stored_prereg_id): (String, i64, String, String) =
        engine
            .conn()
            .query_row(
                "SELECT context_json, randomized, move, prereg_id
                 FROM mrt_log
                 WHERE prereg_id=?1 AND json_extract(context_json, '$.kind')='outcome'
                 ORDER BY at DESC, id DESC
                 LIMIT 1",
                [prereg_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
    MrtAudit {
        context: serde_json::from_str(&context_json).unwrap(),
        randomized,
        move_id,
        prereg_id: stored_prereg_id,
    }
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
