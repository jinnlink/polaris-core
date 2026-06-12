use polaris_core::engine::{Engine, SubmitInput};
use polaris_core::mental_state::{
    estimate_hazard, fit_hazard_model, forward_filter, HazardInputs, HazardTrainingExample,
    HmmObservation, MentalState, STATE_COUNT,
};

#[test]
fn hmm_prior_emission_distinguishes_flow_and_frustration() {
    let flow = forward_filter(
        None,
        HmmObservation {
            z_latency: -0.5,
            hints: 0.0,
            residual: 0.10,
            consec_fail: 0.0,
            conf_delta: 0.2,
            interval_bucket: 0.0,
            session_min: 5.0,
        },
    );
    assert_eq!(flow.dominant_state(), MentalState::Flow);
    assert!((flow.probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-9);

    let frustrated = forward_filter(
        None,
        HmmObservation {
            z_latency: 1.0,
            hints: 1.5,
            residual: -0.30,
            consec_fail: 2.5,
            conf_delta: -0.5,
            interval_bucket: 1.0,
            session_min: 25.0,
        },
    );
    assert_eq!(frustrated.dominant_state(), MentalState::Frustrated);
}

#[test]
fn hmm_transition_smooths_toward_previous_posterior() {
    let previous = forward_filter(
        None,
        HmmObservation {
            z_latency: -0.5,
            hints: 0.0,
            residual: 0.10,
            consec_fail: 0.0,
            conf_delta: 0.2,
            interval_bucket: 0.0,
            session_min: 5.0,
        },
    );

    let smoothed = forward_filter(
        Some(&previous),
        HmmObservation {
            z_latency: 0.5,
            hints: 0.8,
            residual: -0.20,
            consec_fail: 1.0,
            conf_delta: 0.0,
            interval_bucket: 1.0,
            session_min: 10.0,
        },
    );

    assert!(smoothed.probability(MentalState::Flow) > 0.10);
    assert!((smoothed.probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-9);
}

#[test]
fn hmm_temporal_features_distinguish_boredom_and_fatigue() {
    let bored = forward_filter(
        None,
        HmmObservation {
            z_latency: 0.0,
            hints: 0.3,
            residual: 0.0,
            consec_fail: 0.3,
            conf_delta: 0.0,
            interval_bucket: 2.0,
            session_min: 5.0,
        },
    );
    assert_eq!(bored.dominant_state(), MentalState::Bored);

    let fatigued = forward_filter(
        None,
        HmmObservation {
            z_latency: 0.0,
            hints: 0.3,
            residual: 0.0,
            consec_fail: 0.3,
            conf_delta: 0.0,
            interval_bucket: 0.0,
            session_min: 80.0,
        },
    );
    assert_eq!(fatigued.dominant_state(), MentalState::Fatigued);
}

#[test]
fn hazard_requires_auc_gate_before_participating() {
    let posterior = forward_filter(
        None,
        HmmObservation {
            z_latency: 1.0,
            hints: 1.5,
            residual: -0.30,
            consec_fail: 2.5,
            conf_delta: -0.5,
            interval_bucket: 1.0,
            session_min: 25.0,
        },
    );
    let beta = [2.0; 12];
    let low_auc = estimate_hazard(
        HazardInputs::new(&posterior, 0.2, 2.0, 0.6, 0.0, 1.0, 25.0),
        &beta,
        Some(0.69),
        0.70,
    );
    assert!(!low_auc.participates);

    let passing_auc = estimate_hazard(
        HazardInputs::new(&posterior, 0.2, 2.0, 0.6, 0.0, 1.0, 25.0),
        &beta,
        Some(0.70),
        0.70,
    );
    assert!(passing_auc.participates);
    assert!(passing_auc.probability > 0.5);
}

#[test]
fn hazard_logistic_fit_gates_only_when_auc_passes() {
    let mut examples = Vec::new();
    for _ in 0..12 {
        let mut frustrated = [0.0; 12];
        frustrated[MentalState::Frustrated.index()] = 1.0;
        frustrated[7] = 2.0;
        examples.push(HazardTrainingExample {
            inputs: HazardInputs {
                features: frustrated,
            },
            abandoned_within_10m: true,
        });

        let mut flow = [0.0; 12];
        flow[MentalState::Flow.index()] = 1.0;
        flow[7] = 0.0;
        examples.push(HazardTrainingExample {
            inputs: HazardInputs { features: flow },
            abandoned_within_10m: false,
        });
    }

    let validation = examples.clone();
    let model = fit_hazard_model(&examples, &validation, 0.01, 300, 0.5, 0.70);

    assert!(model.validation_auc.unwrap() >= 0.99);
    let high_risk = model.estimate(examples[0].inputs);
    let low_risk = model.estimate(examples[1].inputs);
    assert!(high_risk.participates);
    assert!(high_risk.probability > low_risk.probability);
}

#[test]
fn submit_records_mental_state_snapshot_without_enabling_strategy_or_hazard() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    polaris_core::db::migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 2,
            latency_ms: 2500,
            hint_count: 5,
        })
        .unwrap();

    let payload: String = engine
        .conn()
        .query_row(
            "SELECT payload_json FROM behavior_events
             WHERE type='mental_state' AND concept_id='ownership'
             ORDER BY at DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(json["attempt_id"], receipt.attempt_id);
    assert_eq!(json["features"]["hints"], 3.0);
    assert_eq!(json["strategy_enabled"], false);
    assert_eq!(json["hazard"]["participates"], false);
    assert_eq!(json["posterior"].as_array().unwrap().len(), STATE_COUNT);

    let posterior_sum = json["posterior"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .sum::<f64>();
    assert!((posterior_sum - 1.0).abs() < 1e-9);

    let event_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type='mental_state'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 1);
}

#[test]
fn scheduler_ignores_mental_state_events_until_gate_passes() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    polaris_core::db::migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let before = engine.next_task().unwrap().unwrap().concept_id;
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES ('mental-state-test', 's1', '2026-06-12T00:00:00Z', 'mental_state', 'ownership', ?1)",
            [serde_json::json!({
                "attempt_id": "manual",
                "score_source": "provisional",
                "posterior": [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                "strategy_enabled": false,
                "hazard": {"participates": false, "probability": 0.99}
            })
            .to_string()],
        )
        .unwrap();

    let after = engine.next_task().unwrap().unwrap().concept_id;
    assert_eq!(after, before);
}

#[test]
fn final_score_appends_corrected_mental_state_snapshot() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    polaris_core::db::migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 5,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.20).unwrap();

    let snapshots: Vec<String> = {
        let mut stmt = engine
            .conn()
            .prepare(
                "SELECT payload_json FROM behavior_events
                 WHERE type='mental_state' AND concept_id='ownership'
                 ORDER BY at ASC, id ASC",
            )
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };

    assert_eq!(snapshots.len(), 2);
    let values = snapshots
        .iter()
        .map(|snapshot| serde_json::from_str::<serde_json::Value>(snapshot).unwrap())
        .collect::<Vec<_>>();
    let provisional = values
        .iter()
        .find(|value| value["score_source"] == "provisional")
        .expect("provisional snapshot");
    let final_snapshot = values
        .iter()
        .find(|value| value["score_source"] == "final")
        .expect("final snapshot");
    assert_eq!(provisional["score_source"], "provisional");
    assert_eq!(final_snapshot["score_source"], "final");
    assert_eq!(final_snapshot["attempt_id"], receipt.attempt_id);
    assert_ne!(
        provisional["features"]["residual"],
        final_snapshot["features"]["residual"]
    );
    assert_eq!(
        provisional["features"]["p_hat"], final_snapshot["features"]["p_hat"],
        "final correction must reuse the pre-attempt p_hat"
    );
}

#[test]
fn grade_pending_success_appends_final_mental_state_snapshot() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    polaris_core::db::migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls which binding can drop a value.".to_owned(),
            self_confidence: 4,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();

    let evidence_id: String = engine
        .conn()
        .query_row(
            "SELECT response_evidence_id FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    let response = format!(
        r#"{{"score":0.83,"depth":"explain","citations":[{{"evidence_id":"{evidence_id}","quote":"controls which binding"}}]}}"#
    );
    let summary = engine
        .grade_pending_with_static_response(&response)
        .unwrap();
    assert_eq!(summary.processed, 1);

    let final_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events
             WHERE type='mental_state'
               AND json_extract(payload_json, '$.attempt_id')=?1
               AND json_extract(payload_json, '$.score_source')='final'",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(final_count, 1);
}

#[test]
fn delayed_final_snapshot_does_not_become_next_prior() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    polaris_core::db::migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 5,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();
    let second = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "references".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain references.".to_owned(),
            response_text: "References borrow values.".to_owned(),
            self_confidence: 3,
            latency_ms: 1500,
            hint_count: 1,
        })
        .unwrap();
    let second_posterior =
        mental_state_json(engine.conn(), &second.attempt_id, "provisional")["posterior"].clone();

    engine.apply_final_score(&first.attempt_id, 0.20).unwrap();

    let third = engine
        .submit(SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "pattern_matching".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain pattern matching.".to_owned(),
            response_text: "Patterns match shapes.".to_owned(),
            self_confidence: 3,
            latency_ms: 1600,
            hint_count: 0,
        })
        .unwrap();

    let third_prior = mental_state_json(engine.conn(), &third.attempt_id, "provisional")
        ["prior_posterior"]
        .clone();
    assert_json_float_arrays_close(&third_prior, &second_posterior);
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn mental_state_json(
    conn: &rusqlite::Connection,
    attempt_id: &str,
    score_source: &str,
) -> serde_json::Value {
    let payload: String = conn
        .query_row(
            "SELECT payload_json FROM behavior_events
             WHERE type='mental_state'
               AND json_extract(payload_json, '$.attempt_id')=?1
               AND json_extract(payload_json, '$.score_source')=?2
             ORDER BY rowid DESC LIMIT 1",
            (attempt_id, score_source),
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&payload).unwrap()
}

fn assert_json_float_arrays_close(actual: &serde_json::Value, expected: &serde_json::Value) {
    let actual = actual.as_array().expect("actual array");
    let expected = expected.as_array().expect("expected array");
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        let actual = actual.as_f64().unwrap();
        let expected = expected.as_f64().unwrap();
        assert!(
            (actual - expected).abs() < 1e-12,
            "actual {actual} expected {expected}"
        );
    }
}
