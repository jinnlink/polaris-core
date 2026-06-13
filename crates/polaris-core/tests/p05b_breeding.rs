use polaris_core::breeding::{BredMoveInput, BredMoveStatus};
use polaris_core::config::{default_registry, ParameterClass, TuningRoute};
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::Connection;
use serde_json::Value;

#[test]
fn preregistration_writes_audit_and_keeps_candidate_out_of_admitted_library() {
    let engine = empty_engine();

    let status = engine
        .preregister_bred_move(sample_input("breed-delayed-contrast"))
        .unwrap();

    assert_eq!(status.status, BredMoveStatus::Preregistered);
    assert_eq!(
        engine
            .admitted_bred_moves("state:flow|phase:active")
            .unwrap()
            .len(),
        0
    );

    let audit_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM mrt_log WHERE prereg_id=?1 AND randomized=0",
            ["breed-delayed-contrast"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(audit_count, 1);

    let prereg_json: String = engine
        .conn()
        .query_row(
            "SELECT prereg_json FROM bred_moves WHERE id=?1",
            ["breed-delayed-contrast"],
            |row| row.get(0),
        )
        .unwrap();
    let prereg: Value = serde_json::from_str(&prereg_json).unwrap();
    assert_eq!(prereg["candidate_set"][0], "delayed_contrast_retrieval");
    assert_eq!(prereg["incumbent"], "retrieval");
    assert_eq!(
        prereg["main_effect_hypothesis"],
        "candidate beats incumbent on 7d success"
    );
}

#[test]
fn candidate_admits_only_after_posterior_beats_incumbent_with_minimum_n() {
    let engine = empty_engine();
    engine
        .preregister_bred_move(sample_input("breed-admit"))
        .unwrap();

    for _ in 0..5 {
        engine
            .record_bred_move_outcome("breed-admit", "delayed_contrast_retrieval", true)
            .unwrap();
        engine
            .record_bred_move_outcome("breed-admit", "retrieval", false)
            .unwrap();
    }
    let summary = engine.evaluate_bred_moves().unwrap();
    assert_eq!(summary.admitted, 0);
    assert_eq!(bred_status(&engine, "breed-admit"), "preregistered");

    for _ in 0..3 {
        engine
            .record_bred_move_outcome("breed-admit", "delayed_contrast_retrieval", true)
            .unwrap();
        engine
            .record_bred_move_outcome("breed-admit", "retrieval", false)
            .unwrap();
    }
    let summary = engine.evaluate_bred_moves().unwrap();

    assert_eq!(summary.admitted, 1);
    assert_eq!(bred_status(&engine, "breed-admit"), "admitted");
    let admitted = engine
        .admitted_bred_moves("state:flow|phase:active")
        .unwrap();
    assert_eq!(admitted.len(), 1);
    assert_eq!(admitted[0].candidate_move, "delayed_contrast_retrieval");
    assert!(admitted[0].posterior_win_prob > 0.8);

    let (alpha, beta, n): (f64, f64, i64) = engine
        .conn()
        .query_row(
            "SELECT alpha, beta, n FROM moves_effects WHERE move=?1 AND context_hash=?2",
            ("delayed_contrast_retrieval", "state:flow|phase:active"),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((alpha, beta, n), (9.0, 1.0, 8));
}

#[test]
fn admission_uses_frozen_preregistration_gates_not_current_meta() {
    let engine = empty_engine();
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES
             ('breeding.min_n', '12'),
             ('breeding.admit_p', '0.95')",
            [],
        )
        .unwrap();
    engine
        .preregister_bred_move(sample_input("breed-frozen-gate"))
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES
             ('breeding.min_n', '2'),
             ('breeding.admit_p', '0.50')",
            [],
        )
        .unwrap();

    for _ in 0..8 {
        engine
            .record_bred_move_outcome("breed-frozen-gate", "delayed_contrast_retrieval", true)
            .unwrap();
        engine
            .record_bred_move_outcome("breed-frozen-gate", "retrieval", false)
            .unwrap();
    }

    let summary = engine.evaluate_bred_moves().unwrap();

    assert_eq!(summary.admitted, 0);
    assert_eq!(bred_status(&engine, "breed-frozen-gate"), "preregistered");
}

#[test]
fn admitted_move_retires_when_effect_decays_below_incumbent() {
    let engine = empty_engine();
    admit_candidate(&engine, "breed-retire");

    for _ in 0..12 {
        engine
            .record_bred_move_outcome("breed-retire", "delayed_contrast_retrieval", false)
            .unwrap();
        engine
            .record_bred_move_outcome("breed-retire", "retrieval", true)
            .unwrap();
    }

    let summary = engine.evaluate_bred_moves().unwrap();

    assert_eq!(summary.retired, 1);
    assert_eq!(bred_status(&engine, "breed-retire"), "retired");
    assert!(engine
        .admitted_bred_moves("state:flow|phase:active")
        .unwrap()
        .is_empty());
}

#[test]
fn breeding_parameters_are_governance_gates() {
    let registry = default_registry();

    for key in ["breeding.admit_p", "breeding.retire_p", "breeding.min_n"] {
        let spec = registry.get(key).unwrap_or_else(|| panic!("missing {key}"));
        assert_eq!(
            spec.class,
            ParameterClass::A,
            "{key} must be an A-class validation gate"
        );
        assert_eq!(spec.tuning_route, TuningRoute::Manual);
    }
    assert_eq!(registry["breeding.admit_p"].default_value, "0.80");
}

fn empty_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn sample_input(id: &str) -> BredMoveInput {
    BredMoveInput {
        id: id.to_owned(),
        candidate_move: "delayed_contrast_retrieval".to_owned(),
        incumbent_move: "retrieval".to_owned(),
        context_hash: "state:flow|phase:active".to_owned(),
        task_type: "free_explain".to_owned(),
        template: "Retrieve {concept}, wait, then contrast with a near miss.".to_owned(),
        mechanisms: vec![
            "retrieval".to_owned(),
            "contrast".to_owned(),
            "feedback_timing".to_owned(),
        ],
        main_effect_hypothesis: "candidate beats incumbent on 7d success".to_owned(),
    }
}

fn admit_candidate(engine: &Engine, id: &str) {
    engine.preregister_bred_move(sample_input(id)).unwrap();
    for _ in 0..8 {
        engine
            .record_bred_move_outcome(id, "delayed_contrast_retrieval", true)
            .unwrap();
        engine
            .record_bred_move_outcome(id, "retrieval", false)
            .unwrap();
    }
    let summary = engine.evaluate_bred_moves().unwrap();
    assert_eq!(summary.admitted, 1);
}

fn bred_status(engine: &Engine, id: &str) -> String {
    engine
        .conn()
        .query_row("SELECT status FROM bred_moves WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}
