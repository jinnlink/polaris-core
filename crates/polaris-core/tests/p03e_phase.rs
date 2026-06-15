use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::phase::{determine_phase, Depth, Phase, PhaseInput, PhaseParams};
use polaris_core::scheduler::{
    rank_candidates, rank_candidates_with_params, ScheduleCandidate, SchedulerParams,
};
use polaris_core::status::status_snapshot;
use rusqlite::Connection;

#[test]
fn determine_phase_uses_frozen_priority_order() {
    let mut input = base_phase_input();
    input.attempt_count = 1;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );

    let mut input = base_phase_input();
    input.p_known = 0.65;
    input.max_depth = Some(Depth::Explain);
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Fluctuation);

    let mut input = base_phase_input();
    input.p_known = 0.65;
    input.max_depth = Some(Depth::Apply);
    input.original_context_success = 2;
    input.novel_context_fail = 2;
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Settling);

    let mut input = base_phase_input();
    input.p_known = 0.65;
    input.max_depth = Some(Depth::Apply);
    input.transfer_fail_count = 2;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Solidification
    );

    let mut input = base_phase_input();
    input.p_known = 0.75;
    input.max_depth = Some(Depth::Transfer);
    input.has_transfer_success = true;
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Transfer);

    let mut input = base_phase_input();
    input.p_known = 0.75;
    input.relevant_task_attempt_count = 3;
    input.max_depth = Some(Depth::Transfer);
    input.has_transfer_success = true;
    input.median_latency_ratio = Some(0.80);
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Generation);

    let mut input = base_phase_input();
    input.p_known = 0.55;
    input.calib_gap = 0.30;
    input.max_depth = Some(Depth::Transfer);
    input.has_transfer_success = true;
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Phantom);

    let mut input = base_phase_input();
    input.p_known = 0.49;
    input.attempt_count = 5;
    input.recent_lapses = 2;
    input.max_depth = Some(Depth::Transfer);
    input.ever_reached_transfer_or_generation = true;
    input.median_latency_ratio = Some(0.80);
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Regression);
}

#[test]
fn determine_phase_uses_configured_phantom_thresholds() {
    let mut input = base_phase_input();
    input.p_known = 0.55;
    input.calib_gap = 0.30;
    input.attempt_count = 2;
    input.calibration_sample_count = 3;
    input.calibration_overestimates = 3;
    input.calibration_probability_over_half = 0.875;

    let mut params = phase_params();
    params.phantom_n = 3;

    assert_eq!(determine_phase(&input, &params), Phase::Undetermined);
    input.attempt_count = 3;
    assert_eq!(determine_phase(&input, &params), Phase::Phantom);
}

#[test]
fn phantom_requires_posterior_overestimate_gate() {
    let mut input = base_phase_input();
    input.p_known = 0.55;
    input.calib_gap = 0.30;
    input.attempt_count = 4;
    input.calibration_sample_count = 4;
    input.calibration_overestimates = 2;
    input.calibration_probability_over_half = 0.50;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );

    input.calibration_overestimates = 4;
    input.calibration_probability_over_half = 0.9375;
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Phantom);
}

#[test]
fn settling_requires_novel_context_evidence_and_no_transfer_success() {
    let mut input = base_phase_input();
    input.p_known = 0.65;
    input.max_depth = Some(Depth::Apply);
    input.original_context_success = 2;

    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );

    input.novel_context_fail = 2;
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Settling);

    input.has_transfer_success = true;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );
}

#[test]
fn solidification_requires_no_transfer_success() {
    let mut input = base_phase_input();
    input.p_known = 0.65;
    input.max_depth = Some(Depth::Apply);
    input.transfer_fail_count = 2;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Solidification
    );

    input.has_transfer_success = true;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );
}

#[test]
fn regression_requires_recent_lapses_after_reaching_transfer_or_generation() {
    let mut input = base_phase_input();
    input.p_known = 0.49;
    input.attempt_count = 5;
    input.max_depth = Some(Depth::Transfer);
    input.has_transfer_success = true;

    input.recent_lapses = 2;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );

    input.ever_reached_transfer_or_generation = true;
    input.recent_lapses = 1;
    assert_eq!(
        determine_phase(&input, &phase_params()),
        Phase::Undetermined
    );

    input.recent_lapses = 2;
    assert_eq!(determine_phase(&input, &phase_params()), Phase::Regression);
}

proptest::proptest! {
    #[test]
    fn determine_phase_is_deterministic(input in arbitrary_phase_input()) {
        let params = phase_params();
        proptest::prop_assert_eq!(
            determine_phase(&input, &params),
            determine_phase(&input, &params)
        );
    }

    #[test]
    fn correct_depth_progression_does_not_move_backward(attempt_count in 2_u32..20) {
        let phases = [
            phase_for_depth(Depth::Explain, false, 0, None, attempt_count),
            phase_for_depth(Depth::Apply, false, 2, None, attempt_count),
            phase_for_depth(Depth::Transfer, true, 0, None, attempt_count),
            phase_for_depth(Depth::Transfer, true, 0, Some(0.80), attempt_count.max(3)),
        ];

        for window in phases.windows(2) {
            proptest::prop_assert!(
                window[0].progress_rank() <= window[1].progress_rank(),
                "phase order moved backward: {:?} -> {:?}",
                window[0],
                window[1]
            );
        }
    }
}

#[test]
fn engine_persists_phase_and_emits_transition_event() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first = submit_high_confidence(&mut engine);
    engine.apply_final_score(&first.attempt_id, 0.20).unwrap();
    assert_eq!(
        engine.concept_phase("ownership").unwrap(),
        Phase::Undetermined
    );

    let second = submit_high_confidence(&mut engine);
    engine.apply_final_score(&second.attempt_id, 0.20).unwrap();

    assert_eq!(engine.concept_phase("ownership").unwrap(), Phase::Phantom);
    let stored_phase: String = engine
        .conn()
        .query_row(
            "SELECT phase FROM mastery_states WHERE concept_id='ownership'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_phase, Phase::Phantom.as_str());

    let payload: String = engine
        .conn()
        .query_row(
            "SELECT payload_json FROM behavior_events
             WHERE type='phase_transition' AND concept_id='ownership'
             ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(json["attempt_id"], second.attempt_id);
    assert_eq!(json["from"], Phase::Undetermined.as_str());
    assert_eq!(json["to"], Phase::Phantom.as_str());
}

#[test]
fn status_snapshot_exposes_stored_phase() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first = submit_high_confidence(&mut engine);
    engine.apply_final_score(&first.attempt_id, 0.20).unwrap();
    let second = submit_high_confidence(&mut engine);
    engine.apply_final_score(&second.attempt_id, 0.20).unwrap();

    let snapshot = status_snapshot(engine.conn()).unwrap();
    let ownership = snapshot
        .concepts
        .iter()
        .find(|concept| concept.concept_id == "ownership")
        .expect("ownership status");
    assert_eq!(ownership.phase, Phase::Phantom.as_str());
}

#[test]
fn engine_uses_meta_phantom_thresholds() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute("UPDATE meta SET value='3' WHERE key='calib.phantom_n'", [])
        .unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first = submit_high_confidence(&mut engine);
    engine.apply_final_score(&first.attempt_id, 0.20).unwrap();
    let second = submit_high_confidence(&mut engine);
    engine.apply_final_score(&second.attempt_id, 0.20).unwrap();

    assert_eq!(
        engine.concept_phase("ownership").unwrap(),
        Phase::Undetermined
    );

    let third = submit_high_confidence(&mut engine);
    engine.apply_final_score(&third.attempt_id, 0.20).unwrap();
    assert_eq!(engine.concept_phase("ownership").unwrap(), Phase::Phantom);
}

#[test]
fn engine_uses_posterior_gate_for_phantom() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "UPDATE meta SET value='0.80' WHERE key='calib.phantom_confidence'",
        [],
    )
    .unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first = submit_high_confidence(&mut engine);
    set_attempt_time(&engine, &first.attempt_id, 1);
    engine.apply_final_score(&first.attempt_id, 0.20).unwrap();
    let second = submit_high_confidence(&mut engine);
    set_attempt_time(&engine, &second.attempt_id, 2);
    engine.apply_final_score(&second.attempt_id, 0.20).unwrap();
    let third = engine
        .submit(SubmitInput {
            session_id: "phase-test".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 1,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();
    set_attempt_time(&engine, &third.attempt_id, 3);
    engine.apply_final_score(&third.attempt_id, 0.90).unwrap();

    assert_eq!(
        engine.concept_phase("ownership").unwrap(),
        Phase::Undetermined
    );

    let fourth = submit_high_confidence(&mut engine);
    set_attempt_time(&engine, &fourth.attempt_id, 4);
    engine.apply_final_score(&fourth.attempt_id, 0.20).unwrap();

    assert_eq!(engine.concept_phase("ownership").unwrap(), Phase::Phantom);
}

#[test]
fn generation_latency_uses_relevant_task_type_bucket() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    insert_scored_attempt(
        &engine,
        "slow-1",
        "borrowing",
        ("recall", "recall"),
        (0.90, 1000, 1),
    );
    insert_scored_attempt(
        &engine,
        "slow-2",
        "lifetimes",
        ("recall", "recall"),
        (0.90, 1000, 2),
    );
    insert_scored_attempt(
        &engine,
        "slow-3",
        "borrowing",
        ("recall", "recall"),
        (0.90, 1000, 3),
    );
    insert_scored_attempt(
        &engine,
        "slow-4",
        "lifetimes",
        ("recall", "recall"),
        (0.90, 1000, 4),
    );
    insert_scored_attempt(
        &engine,
        "own-recall-1",
        "ownership",
        ("recall", "recall"),
        (0.90, 50, 5),
    );
    insert_scored_attempt(
        &engine,
        "own-recall-2",
        "ownership",
        ("recall", "recall"),
        (0.90, 60, 6),
    );
    insert_scored_attempt(
        &engine,
        "own-recall-3",
        "ownership",
        ("recall", "recall"),
        (0.90, 70, 7),
    );
    insert_scored_attempt(
        &engine,
        "own-transfer-1",
        "ownership",
        ("transfer", "transfer"),
        (0.90, 2000, 8),
    );

    engine.apply_final_score("own-transfer-1", 0.90).unwrap();

    assert_eq!(engine.concept_phase("ownership").unwrap(), Phase::Transfer);
}

#[test]
fn grade_pending_transfer_depth_replays_phase() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    for idx in 1..=2 {
        let receipt = engine
            .submit(SubmitInput {
                session_id: "phase-grade-pending".to_owned(),
                concept_id: "ownership".to_owned(),
                task_type: "recall".to_owned(),
                prompt_text: "Explain ownership.".to_owned(),
                response_text: "Ownership controls which binding can drop a value.".to_owned(),
                self_confidence: 5,
                latency_ms: 120,
                hint_count: 0,
            })
            .unwrap();
        engine.apply_final_score(&receipt.attempt_id, 0.90).unwrap();
        engine
            .conn()
            .execute(
                "UPDATE attempts SET created_at=?2 WHERE id=?1",
                rusqlite::params![receipt.attempt_id, timestamp(idx)],
            )
            .unwrap();
    }

    let receipt = engine
        .submit(SubmitInput {
            session_id: "phase-grade-pending".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "transfer".to_owned(),
            prompt_text: "Transfer ownership to a new context.".to_owned(),
            response_text: "Ownership still decides who drops the value when APIs move it."
                .to_owned(),
            self_confidence: 5,
            latency_ms: 120,
            hint_count: 0,
        })
        .unwrap();
    engine
        .conn()
        .execute(
            "UPDATE grade_queue SET last_error='retry' WHERE attempt_id=?1",
            [&receipt.attempt_id],
        )
        .unwrap();

    let evidence_id: String = engine
        .conn()
        .query_row(
            "SELECT response_evidence_id FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    let response_json = serde_json::json!({
        "score": 0.90,
        "depth": "transfer",
        "citations": [{"evidence_id": evidence_id, "quote": "Ownership"}],
    })
    .to_string();

    let summary = engine
        .grade_pending_with_static_response(&response_json)
        .unwrap();

    assert_eq!(summary.processed, 1);
    assert_eq!(engine.concept_phase("ownership").unwrap(), Phase::Transfer);
}

#[test]
fn novel_context_failures_trigger_settling_without_transfer_success() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    for sequence in 1..=6 {
        insert_scored_attempt(
            &engine,
            &format!("orig-{sequence}"),
            "ownership",
            ("apply", "apply"),
            (0.90, 100, sequence),
        );
    }
    insert_novel_attempt(&engine, "novel-1", 0.49, 7);
    insert_novel_attempt(&engine, "novel-2", 0.49, 8);

    engine.apply_final_score("novel-2", 0.20).unwrap();

    assert_eq!(engine.concept_phase("ownership").unwrap(), Phase::Settling);
}

#[test]
fn legacy_mastery_states_migration_adds_phase_without_losing_rows() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE mastery_states(
            concept_id TEXT PRIMARY KEY,
            p_known REAL,
            attempt_count INTEGER DEFAULT 0,
            updated_at TEXT
        );
        INSERT INTO mastery_states(concept_id, p_known, attempt_count, updated_at)
        VALUES ('ownership', 0.42, 7, '2026-01-01T00:00:00Z');",
    )
    .unwrap();

    migrate(&conn).unwrap();

    let row: (f64, i64, String) = conn
        .query_row(
            "SELECT p_known, attempt_count, phase FROM mastery_states WHERE concept_id='ownership'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, (0.42, 7, Phase::Undetermined.as_str().to_owned()));
}

#[test]
fn scheduler_phase_factor_defaults_to_zero_and_does_not_change_order() {
    let ranked = rank_candidates(vec![
        schedule_candidate("plain", 1, Phase::Undetermined),
        schedule_candidate("regression", 2, Phase::Regression),
    ]);
    assert_eq!(ranked[0].id, "plain");

    let mut params = SchedulerParams::defaults();
    assert_eq!(params.w_phase, 0.0);
    params.w_phase = 1.0;
    let boosted = rank_candidates_with_params(
        vec![
            schedule_candidate("plain", 1, Phase::Undetermined),
            schedule_candidate("regression", 2, Phase::Regression),
        ],
        &params,
    );
    assert_eq!(boosted[0].id, "regression");
}

fn base_phase_input() -> PhaseInput {
    PhaseInput {
        p_known: 0.20,
        retrievability: None,
        theta_prediction: None,
        calib_gap: 0.0,
        attempt_count: 2,
        lapses: 0,
        recent_lapses: 0,
        max_depth: None,
        has_transfer_success: false,
        ever_reached_transfer_or_generation: false,
        relevant_task_attempt_count: 0,
        original_context_success: 0,
        transfer_fail_count: 0,
        novel_context_success: 0,
        novel_context_fail: 0,
        calibration_overestimates: 2,
        calibration_sample_count: 2,
        calibration_probability_over_half: 0.75,
        median_latency_ratio: None,
    }
}

fn arbitrary_phase_input() -> impl proptest::strategy::Strategy<Value = PhaseInput> {
    use proptest::prelude::*;

    (
        0.0_f64..1.0,
        -1.0_f64..1.0,
        0_u32..20,
        0_u32..5,
        proptest::option::of(0_u8..4),
        proptest::bool::ANY,
        proptest::bool::ANY,
        0_u32..20,
        0_u32..5,
        0_u32..5,
        0_u32..5,
        proptest::option::of(0.1_f64..2.0),
    )
        .prop_map(
            |(
                p_known,
                calib_gap,
                attempt_count,
                recent_lapses,
                depth_rank,
                has_transfer_success,
                ever_reached_transfer_or_generation,
                relevant_task_attempt_count,
                original_context_success,
                transfer_fail_count,
                novel_context_fail,
                median_latency_ratio,
            )| PhaseInput {
                p_known,
                retrievability: None,
                theta_prediction: None,
                calib_gap,
                attempt_count,
                lapses: recent_lapses,
                recent_lapses,
                max_depth: depth_rank.map(depth_from_rank),
                has_transfer_success,
                ever_reached_transfer_or_generation,
                relevant_task_attempt_count,
                original_context_success,
                transfer_fail_count,
                novel_context_success: 0,
                novel_context_fail,
                calibration_overestimates: attempt_count as usize,
                calibration_sample_count: attempt_count as usize,
                calibration_probability_over_half: 0.99,
                median_latency_ratio,
            },
        )
}

fn phase_for_depth(
    depth: Depth,
    has_transfer_success: bool,
    transfer_fail_count: u32,
    median_latency_ratio: Option<f64>,
    attempt_count: u32,
) -> Phase {
    let mut input = base_phase_input();
    input.p_known = if has_transfer_success { 0.75 } else { 0.65 };
    input.attempt_count = attempt_count;
    input.relevant_task_attempt_count = attempt_count;
    input.max_depth = Some(depth);
    input.has_transfer_success = has_transfer_success;
    input.transfer_fail_count = transfer_fail_count;
    input.median_latency_ratio = median_latency_ratio;
    determine_phase(&input, &phase_params())
}

fn depth_from_rank(rank: u8) -> Depth {
    match rank {
        0 => Depth::Recall,
        1 => Depth::Explain,
        2 => Depth::Apply,
        _ => Depth::Transfer,
    }
}

fn submit_high_confidence(engine: &mut Engine) -> polaris_core::engine::SubmitReceipt {
    engine
        .submit(SubmitInput {
            session_id: "phase-test".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 5,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap()
}

fn insert_scored_attempt(
    engine: &Engine,
    attempt_id: &str,
    concept_id: &str,
    task_and_depth: (&str, &str),
    outcome: (f64, i64, u32),
) {
    let (task_type, depth) = task_and_depth;
    let (score, latency_ms, sequence) = outcome;
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, prompt_text,
                                  self_confidence, latency_ms, provisional_score, final_score,
                                  depth, rating, created_at, graded_at)
             VALUES (?1, 'phase-fixture', ?2, ?3, 'fixture', 5, ?4, ?5, ?5, ?6, 'good', ?7, ?7)",
            rusqlite::params![
                attempt_id,
                concept_id,
                task_type,
                latency_ms,
                score,
                depth,
                timestamp(sequence)
            ],
        )
        .unwrap();
}

fn insert_novel_attempt(engine: &Engine, attempt_id: &str, score: f64, sequence: u32) {
    insert_scored_attempt(
        engine,
        attempt_id,
        "ownership",
        ("apply", "apply"),
        (score, 100, sequence),
    );
    engine
        .conn()
        .execute(
            "UPDATE attempts SET grader_json=?2 WHERE id=?1",
            rusqlite::params![
                attempt_id,
                serde_json::json!({"context_novel": true}).to_string()
            ],
        )
        .unwrap();
}

fn set_attempt_time(engine: &Engine, attempt_id: &str, sequence: u32) {
    engine
        .conn()
        .execute(
            "UPDATE attempts SET created_at=?2 WHERE id=?1",
            rusqlite::params![attempt_id, timestamp(sequence)],
        )
        .unwrap();
}

fn timestamp(sequence: u32) -> String {
    format!("2026-01-01T00:{sequence:02}:00Z")
}

fn phase_params() -> PhaseParams {
    PhaseParams {
        phantom_gap: 0.25,
        phantom_p: 0.60,
        phantom_n: 2,
        phantom_confidence: 0.60,
    }
}

fn schedule_candidate(id: &str, seed_order: i64, phase: Phase) -> ScheduleCandidate {
    ScheduleCandidate {
        id: id.to_owned(),
        seed_order,
        retrieval: Some(1.0),
        calib_gap: 0.0,
        misconception_active: false,
        has_attempts: true,
        prerequisites_met: false,
        phase,
    }
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
