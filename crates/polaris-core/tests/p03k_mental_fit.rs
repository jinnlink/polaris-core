use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn insufficient_data_skips_all_three_tasks() {
    let engine = seeded_engine();

    let summary = engine.run_mental_dynamics_fit().unwrap();

    assert_eq!(summary.hazard.status, "skipped");
    assert_eq!(summary.state_gate.status, "skipped");
    assert_eq!(summary.em.status, "skipped");
    assert_eq!(table_count(&engine, "hazard_models"), 0);
    assert_eq!(table_count(&engine, "state_gate_evals"), 0);
    assert_eq!(meta(&engine, "hmm.transitions"), "[]");
}

#[test]
fn separable_abandon_history_fits_hazard_model_and_snapshot_consumes_it() {
    let mut engine = seeded_engine();
    seed_consec_fail_abandon_history(&engine, 60);

    let summary = engine.run_mental_dynamics_fit().unwrap();

    assert_eq!(summary.hazard.status, "done", "{}", summary.hazard.detail);
    assert_eq!(table_count(&engine, "hazard_models"), 1);
    let auc: f64 = engine
        .conn()
        .query_row("SELECT validation_auc FROM hazard_models", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(auc > 0.9, "separable data should give high AUC, got {auc}");

    submit_once(&mut engine, "post-fit");
    let payload = latest_mental_state_payload(&engine);
    assert_eq!(payload["hazard"]["model_status"], "fitted");
    assert!((payload["hazard"]["validation_auc"].as_f64().unwrap() - auc).abs() < 1e-12);
    assert_eq!(payload["hazard"]["participates"], json!(true));
}

#[test]
fn state_gate_passes_when_posterior_carries_information() {
    let mut engine = seeded_engine();
    for idx in 0..60 {
        let leaver = idx % 2 == 0;
        let posterior = if leaver {
            onehot(2) // frustrated
        } else {
            onehot(0) // flow
        };
        let session = format!("s{idx}");
        let minutes_ago = 400 - idx * 3;
        insert_snapshot(&engine, &session, minutes_ago, posterior, 0.0);
        if leaver {
            insert_action(&engine, &session, "hint", minutes_ago - 1);
        }
    }

    let summary = engine.run_mental_dynamics_fit().unwrap();

    assert_eq!(
        summary.state_gate.status, "done",
        "{}",
        summary.state_gate.detail
    );
    let (margin, passes): (f64, i64) = engine
        .conn()
        .query_row("SELECT margin, passes FROM state_gate_evals", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert!(margin >= 0.03, "expected margin >= gate, got {margin}");
    assert_eq!(passes, 1);

    submit_once(&mut engine, "post-gate");
    let payload = latest_mental_state_payload(&engine);
    assert_eq!(payload["strategy_enabled"], json!(true));
    assert!(
        (payload["state_gate"]["observed_auc_margin"]
            .as_f64()
            .unwrap()
            - margin)
            .abs()
            < 1e-12
    );
}

#[test]
fn state_gate_fails_when_baseline_already_separates() {
    let mut engine = seeded_engine();
    for idx in 0..60 {
        let leaver = idx % 2 == 0;
        let session = format!("s{idx}");
        let minutes_ago = 400 - idx * 3;
        let uniform = [1.0 / 6.0; 6];
        insert_snapshot(
            &engine,
            &session,
            minutes_ago,
            uniform,
            if leaver { 3.0 } else { 0.0 },
        );
        if leaver {
            insert_action(&engine, &session, "hint", minutes_ago - 1);
        }
    }

    let summary = engine.run_mental_dynamics_fit().unwrap();

    assert_eq!(summary.state_gate.status, "done");
    let passes: i64 = engine
        .conn()
        .query_row("SELECT passes FROM state_gate_evals", [], |row| row.get(0))
        .unwrap();
    assert_eq!(passes, 0, "state posterior adds nothing over baseline");

    submit_once(&mut engine, "post-gate-fail");
    let payload = latest_mental_state_payload(&engine);
    assert_eq!(payload["strategy_enabled"], json!(false));
}

#[test]
fn em_reestimates_transitions_with_enough_graded_attempts() {
    let mut engine = seeded_engine();
    for idx in 0..200 {
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, concept_id, task_type, self_confidence, final_score, created_at, graded_at)
                 VALUES (?1, 'ownership', 'recall', 3, 0.8,
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'),
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'))",
                [format!("graded-{idx:03}")],
            )
            .unwrap();
    }
    for session_idx in 0..4 {
        let session = format!("em-{session_idx}");
        for step in 0..24 {
            let frustrated = step >= 12;
            insert_em_observation(&engine, &session, 500 - session_idx * 60 - step, frustrated);
        }
    }

    let summary = engine.run_mental_dynamics_fit().unwrap();

    assert_eq!(summary.em.status, "done", "{}", summary.em.detail);
    let rows: Vec<Vec<f64>> = serde_json::from_str(&meta(&engine, "hmm.transitions")).unwrap();
    assert_eq!(rows.len(), 6);
    for row in &rows {
        assert_eq!(row.len(), 6);
        let total = row.iter().sum::<f64>();
        assert!((total - 1.0).abs() < 1e-9, "row sums to {total}");
        assert!(row.iter().all(|value| *value > 0.0 && value.is_finite()));
    }

    // 滤波路径在重估矩阵下仍可用
    submit_once(&mut engine, "post-em");
    let payload = latest_mental_state_payload(&engine);
    assert_eq!(payload["posterior"].as_array().unwrap().len(), 6);
}

#[test]
fn mirror_report_includes_hazard_summary_only_after_gate_passes() {
    let mut engine = seeded_engine();

    let before = engine.run_mirror_report().unwrap();
    assert!(before
        .assertions
        .iter()
        .all(|item| item.kind != "hazard_risk_summary"));

    seed_consec_fail_abandon_history(&engine, 60);
    engine.run_mental_dynamics_fit().unwrap();
    for idx in 0..3 {
        submit_once(&mut engine, &format!("fitted-{idx}"));
    }

    let after = engine.run_mirror_report().unwrap();
    assert!(after.hazard_gate.participates, "{:?}", after.hazard_gate);
    let assertion = after
        .assertions
        .iter()
        .find(|item| item.id == "hazard_risk_summary:window")
        .expect("hazard summary present after gate passes");
    assert!(assertion.confidence >= 0.70);
    assert!(!assertion.evidence_ids.is_empty());
    assert!(assertion
        .evidence_ids
        .iter()
        .all(|id| id.starts_with("behavior:")));
}

#[test]
fn fit_is_deterministic_for_same_state() {
    let engine = seeded_engine();
    seed_consec_fail_abandon_history(&engine, 60);

    let first = engine.run_mental_dynamics_fit().unwrap();

    engine
        .conn()
        .execute("DELETE FROM hazard_models", [])
        .unwrap();
    engine
        .conn()
        .execute("DELETE FROM state_gate_evals", [])
        .unwrap();
    engine
        .conn()
        .execute("UPDATE meta SET value='[]' WHERE key='hmm.transitions'", [])
        .unwrap();

    let second = engine.run_mental_dynamics_fit().unwrap();

    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// 测试工具
// ---------------------------------------------------------------------------

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn onehot(index: usize) -> [f64; 6] {
    let mut posterior = [0.0; 6];
    posterior[index] = 1.0;
    posterior
}

/// 一半高 consec_fail 且随后放弃、一半平静；时间交错保证留出段两类齐备。
fn seed_consec_fail_abandon_history(engine: &Engine, total: usize) {
    for idx in 0..total {
        let risky = idx % 2 == 0;
        let session = format!("hz-{idx}");
        let minutes_ago = 600 - idx * 4;
        insert_snapshot(
            engine,
            &session,
            minutes_ago,
            [1.0 / 6.0; 6],
            if risky { 3.0 } else { 0.0 },
        );
        if risky {
            insert_action(engine, &session, "abandon", minutes_ago - 1);
        }
    }
}

fn insert_snapshot(
    engine: &Engine,
    session: &str,
    minutes_ago: usize,
    posterior: [f64; 6],
    consec_fail: f64,
) {
    let mut inputs = vec![0.0; 12];
    inputs[..6].copy_from_slice(&posterior);
    inputs[7] = consec_fail;
    inputs[11] = 5.0;
    let payload = json!({
        "schema_version": 1,
        "attempt_id": format!("seed-{session}"),
        "score_source": "provisional",
        "posterior": posterior,
        "hazard": { "inputs": inputs },
    });
    insert_mental_event(engine, session, minutes_ago, &payload.to_string());
}

fn insert_em_observation(engine: &Engine, session: &str, minutes_ago: usize, frustrated: bool) {
    let features = if frustrated {
        json!({"z_latency": 1.0, "hints": 1.5, "residual": -0.30, "consec_fail": 2.5,
               "conf_delta": -0.5, "interval_bucket": 1.0, "session_min": 32.0})
    } else {
        json!({"z_latency": -0.5, "hints": 0.2, "residual": 0.10, "consec_fail": 0.2,
               "conf_delta": 0.2, "interval_bucket": 0.0, "session_min": 8.0})
    };
    let payload = json!({
        "schema_version": 1,
        "attempt_id": format!("em-{session}-{minutes_ago}"),
        "score_source": "provisional",
        "features": features,
    });
    insert_mental_event(engine, session, minutes_ago, &payload.to_string());
}

fn insert_mental_event(engine: &Engine, session: &str, minutes_ago: usize, payload: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (lower(hex(randomblob(16))), ?1,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now',?2), 'mental_state', 'ownership', ?3)",
            (session, format!("-{minutes_ago} minutes"), payload),
        )
        .unwrap();
}

fn insert_action(engine: &Engine, session: &str, event_type: &str, minutes_ago: usize) {
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (lower(hex(randomblob(16))), ?1,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now',?2), ?3, 'ownership', '{}')",
            (session, format!("-{minutes_ago} minutes"), event_type),
        )
        .unwrap();
}

fn submit_once(engine: &mut Engine, session: &str) {
    engine
        .submit(SubmitInput {
            session_id: session.to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "测试".to_owned(),
            response_text: "所有权决定值的生命周期与释放时机。".to_owned(),
            self_confidence: 3,
            latency_ms: 1200,
            hint_count: 0,
        })
        .unwrap();
}

fn latest_mental_state_payload(engine: &Engine) -> serde_json::Value {
    let payload: String = engine
        .conn()
        .query_row(
            "SELECT payload_json FROM behavior_events
             WHERE type='mental_state'
             ORDER BY julianday(at) DESC, rowid DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&payload).unwrap()
}

fn table_count(engine: &Engine, table: &str) -> i64 {
    engine
        .conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn meta(engine: &Engine, key: &str) -> String {
    engine
        .conn()
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .unwrap()
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
