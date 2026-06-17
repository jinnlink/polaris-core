use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::fsrs::FsrsState;
use polaris_core::fsrs_fit::{evaluate_fsrs_personal_params, FsrsFitStatus};
use polaris_core::ops::doctor_report;
use rusqlite::{params, Connection};

#[test]
fn insufficient_final_history_skips_without_audit_or_meta_change() {
    let engine = test_engine();
    insert_concept(&engine, "ownership");
    for idx in 0..5 {
        insert_final_attempt(&engine, &format!("a-{idx:02}"), "ownership", idx, 0.8);
    }
    let before = meta(&engine, "fsrs.w");

    let summary = engine.fit_fsrs_personal_params().unwrap();

    assert_eq!(summary.status, FsrsFitStatus::Skipped);
    assert_eq!(summary.param, "fsrs.w");
    assert!(summary
        .reason
        .as_deref()
        .unwrap()
        .starts_with("insufficient_history"));
    assert_eq!(meta(&engine, "fsrs.w"), before);
    assert_eq!(audit_count(&engine, "fsrs.w"), 0);
    assert_eq!(count(&engine, "mastery_states"), 0);
}

#[test]
fn provisional_scores_are_not_used_for_personal_fsrs_fit() {
    let engine = test_engine();
    set_meta(&engine, "fsrs_fit.min_attempts", "5");
    insert_concept(&engine, "ownership");
    for idx in 0..20 {
        insert_provisional_only_attempt(&engine, &format!("p-{idx:02}"), "ownership", idx, 0.9);
    }
    let before = meta(&engine, "fsrs.w");

    let summary = engine.fit_fsrs_personal_params().unwrap();

    assert_eq!(summary.status, FsrsFitStatus::Skipped);
    assert!(summary
        .reason
        .as_deref()
        .unwrap()
        .starts_with("insufficient_history"));
    assert_eq!(meta(&engine, "fsrs.w"), before);
    assert_eq!(audit_count(&engine, "fsrs.w"), 0);
}

#[test]
fn first_reviews_do_not_count_as_holdout_predictions() {
    let engine = test_engine();
    set_meta(&engine, "fsrs_fit.min_attempts", "5");
    set_meta(&engine, "fsrs_fit.min_holdout_predictions", "1");
    for idx in 0..5 {
        let concept_id = format!("concept-{idx}");
        insert_concept(&engine, &concept_id);
        insert_final_attempt(&engine, &format!("first-{idx}"), &concept_id, idx, 0.8);
    }

    let summary = engine.fit_fsrs_personal_params().unwrap();

    assert_eq!(summary.status, FsrsFitStatus::Skipped);
    assert!(summary
        .reason
        .as_deref()
        .unwrap()
        .starts_with("insufficient_holdout_predictions"));
    assert_eq!(summary.holdout_predictions, 0);
    assert_eq!(audit_count(&engine, "fsrs.w"), 0);
}

#[test]
fn accepted_fit_updates_fsrs_w_audits_and_replays_existing_concepts() {
    let engine = test_engine();
    configure_small_fit_gate(&engine);
    set_meta(&engine, "fsrs_fit.accept_margin", "0.0001");
    insert_concept(&engine, "ownership");
    set_low_good_stability(&engine);
    let before = meta(&engine, "fsrs.w");
    for idx in 0..32 {
        insert_final_attempt(
            &engine,
            &format!("good-{idx:02}"),
            "ownership",
            idx * 3,
            0.8,
        );
    }

    let summary = engine.fit_fsrs_personal_params().unwrap();

    assert_eq!(summary.status, FsrsFitStatus::Accepted);
    assert!(summary.delta > 0.0001);
    assert_eq!(summary.replayed_concepts, 1);
    assert_ne!(summary.new_value, before);
    assert_eq!(meta(&engine, "fsrs.w"), summary.new_value);
    assert_eq!(audit_status(&engine, "fsrs.w"), "accepted");

    let (fsrs_json, next_due_at): (String, Option<String>) = engine
        .conn()
        .query_row(
            "SELECT fsrs_json, next_due_at FROM mastery_states WHERE concept_id='ownership'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let state: FsrsState = serde_json::from_str(&fsrs_json).unwrap();
    assert!(state.reps > 0);
    assert!(next_due_at.is_some(), "accepted fit must refresh due dates");
}

#[test]
fn accepted_fit_leaves_doctor_clean_and_does_not_touch_shadow_tables() {
    let engine = test_engine();
    configure_small_fit_gate(&engine);
    set_meta(&engine, "fsrs_fit.accept_margin", "0.0001");
    insert_concept(&engine, "ownership");
    set_low_good_stability(&engine);
    for idx in 0..32 {
        insert_final_attempt(
            &engine,
            &format!("good-{idx:02}"),
            "ownership",
            idx * 3,
            0.8,
        );
    }

    let summary = engine.fit_fsrs_personal_params().unwrap();

    assert_eq!(summary.status, FsrsFitStatus::Accepted);
    let report = doctor_report(engine.conn()).unwrap();
    assert!(
        report.ok,
        "accepted FSRS fit must replay to a doctor-clean state"
    );
    assert_eq!(
        count(&engine, "gu_rules"),
        0,
        "P06J must not write P06I state"
    );
    assert_eq!(
        count_where(&engine, "behavior_events", "type='phase_transition'"),
        0,
        "P06J must not write P06H phase-transition events"
    );
}

#[test]
fn rejected_fit_keeps_fsrs_w_and_does_not_replay_mastery_states() {
    let engine = test_engine();
    configure_small_fit_gate(&engine);
    set_meta(&engine, "fsrs_fit.accept_margin", "10.0");
    insert_concept(&engine, "ownership");
    set_low_good_stability(&engine);
    let before = meta(&engine, "fsrs.w");
    for idx in 0..32 {
        insert_final_attempt(
            &engine,
            &format!("good-{idx:02}"),
            "ownership",
            idx * 3,
            0.8,
        );
    }

    let summary = engine.fit_fsrs_personal_params().unwrap();

    assert_eq!(summary.status, FsrsFitStatus::Rejected);
    assert_eq!(summary.old_value, before);
    assert_eq!(meta(&engine, "fsrs.w"), before);
    assert_eq!(summary.replayed_concepts, 0);
    assert_eq!(count(&engine, "mastery_states"), 0);
    assert_eq!(audit_status(&engine, "fsrs.w"), "rejected");
}

#[test]
fn fsrs_fit_evaluation_is_deterministic_for_same_database_state() {
    let engine = test_engine();
    configure_small_fit_gate(&engine);
    set_meta(&engine, "fsrs_fit.accept_margin", "0.0001");
    insert_concept(&engine, "ownership");
    set_low_good_stability(&engine);
    for idx in 0..32 {
        insert_final_attempt(
            &engine,
            &format!("good-{idx:02}"),
            "ownership",
            idx * 3,
            0.8,
        );
    }

    let first = evaluate_fsrs_personal_params(engine.conn()).unwrap();
    let second = evaluate_fsrs_personal_params(engine.conn()).unwrap();

    assert_eq!(first, second);
    assert_eq!(audit_count(&engine, "fsrs.w"), 0);
}

#[test]
fn holdout_outcomes_do_not_influence_candidate_search() {
    let left = test_engine();
    let right = test_engine();
    for engine in [&left, &right] {
        configure_small_fit_gate(engine);
        insert_concept(engine, "ownership");
        set_low_good_stability(engine);
    }

    for idx in 0..32 {
        let train_score = 0.8;
        let left_score = if idx < 24 { train_score } else { 0.8 };
        let right_score = if idx < 24 { train_score } else { 0.2 };
        insert_final_attempt(
            &left,
            &format!("attempt-{idx:02}"),
            "ownership",
            idx * 3,
            left_score,
        );
        insert_final_attempt(
            &right,
            &format!("attempt-{idx:02}"),
            "ownership",
            idx * 3,
            right_score,
        );
    }

    let left_summary = evaluate_fsrs_personal_params(left.conn()).unwrap();
    let right_summary = evaluate_fsrs_personal_params(right.conn()).unwrap();

    assert_eq!(
        left_summary.train_predictions,
        right_summary.train_predictions
    );
    assert_eq!(
        left_summary.holdout_predictions,
        right_summary.holdout_predictions
    );
    assert_eq!(
        left_summary.candidate_weights, right_summary.candidate_weights,
        "candidate search must be driven only by the train split"
    );
    assert_ne!(
        left_summary.candidate_metric, right_summary.candidate_metric,
        "the fixture should differ only once holdout is scored"
    );
}

#[test]
fn p03j_param_tuning_still_never_touches_fsrs_w() {
    let engine = test_engine();
    set_meta(&engine, "tuning.max_params_per_run", "12");
    insert_concept(&engine, "ownership");
    let before = meta(&engine, "fsrs.w");
    for idx in 0..40 {
        insert_final_attempt(&engine, &format!("fail-{idx:02}"), "ownership", idx, 0.2);
    }

    let summary = engine.run_param_tuning().unwrap();

    assert!(!summary.outcomes.is_empty());
    assert_eq!(meta(&engine, "fsrs.w"), before);
    assert_eq!(audit_count(&engine, "fsrs.w"), 0);
}

fn test_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn configure_small_fit_gate(engine: &Engine) {
    set_meta(engine, "fsrs_fit.min_attempts", "12");
    set_meta(engine, "fsrs_fit.min_holdout_predictions", "3");
    set_meta(engine, "fsrs_fit.holdout_frac", "0.25");
}

fn set_low_good_stability(engine: &Engine) {
    let mut w: Vec<f64> = serde_json::from_str(&meta(engine, "fsrs.w")).unwrap();
    w[2] = 0.2;
    set_meta(engine, "fsrs.w", &serde_json::to_string(&w).unwrap());
}

fn insert_concept(engine: &Engine, id: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO concepts(id, pack, name, kind, seed_order, provenance, evidence_ids_json, created_at)
             VALUES (?1, 'test', ?1, 'concept', 0, 'test', '[]', '2026-01-01T00:00:00Z')",
            [id],
        )
        .unwrap();
}

fn insert_final_attempt(
    engine: &Engine,
    id: &str,
    concept_id: &str,
    days_after_start: usize,
    final_score: f64,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                  provisional_score, final_score, created_at, graded_at)
             VALUES (?1, 's1', ?2, 'recall', 3, 0.5, ?3,
                     strftime('%Y-%m-%dT%H:%M:%SZ', '2026-01-01T00:00:00Z', ?4),
                     strftime('%Y-%m-%dT%H:%M:%SZ', '2026-01-01T00:00:00Z', ?4))",
            params![
                id,
                concept_id,
                final_score,
                format!("+{days_after_start} days")
            ],
        )
        .unwrap();
}

fn insert_provisional_only_attempt(
    engine: &Engine,
    id: &str,
    concept_id: &str,
    days_after_start: usize,
    provisional_score: f64,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                  provisional_score, created_at)
             VALUES (?1, 's1', ?2, 'recall', 3, ?3,
                     strftime('%Y-%m-%dT%H:%M:%SZ', '2026-01-01T00:00:00Z', ?4))",
            params![
                id,
                concept_id,
                provisional_score,
                format!("+{days_after_start} days")
            ],
        )
        .unwrap();
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

fn meta(engine: &Engine, key: &str) -> String {
    engine
        .conn()
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .unwrap()
}

fn audit_status(engine: &Engine, param: &str) -> String {
    engine
        .conn()
        .query_row(
            "SELECT status FROM param_tuning_runs WHERE param=?1 ORDER BY ran_at DESC, id DESC LIMIT 1",
            [param],
            |row| row.get(0),
        )
        .unwrap()
}

fn audit_count(engine: &Engine, param: &str) -> i64 {
    engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM param_tuning_runs WHERE param=?1",
            [param],
            |row| row.get(0),
        )
        .unwrap()
}

fn count(engine: &Engine, table: &str) -> i64 {
    engine
        .conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn count_where(engine: &Engine, table: &str, where_sql: &str) -> i64 {
    engine
        .conn()
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {where_sql}"),
            [],
            |row| row.get(0),
        )
        .unwrap()
}
