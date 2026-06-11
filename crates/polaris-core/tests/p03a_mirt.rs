use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::mirt::{decode_vector, LatentPrediction};
use rusqlite::Connection;

#[test]
fn init_pack_initializes_q_and_theta() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let q_blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT q FROM concepts WHERE id='ownership'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let q = decode_vector(&q_blob).unwrap();
    assert_eq!(q.len(), 32);
    assert_eq!(q[0], 1.0);

    let (theta_blob, version): (Vec<u8>, i64) = engine
        .conn()
        .query_row("SELECT vec, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let theta = decode_vector(&theta_blob).unwrap();
    assert_eq!(theta.len(), 32);
    assert!(theta.iter().all(|value| *value == 0.0));
    assert_eq!(version, 1);
}

#[test]
fn final_score_updates_theta_and_attempt_version() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    let receipt = engine
        .submit(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 4,
            latency_ms: 1000,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.90).unwrap();

    let theta_blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    let theta = decode_vector(&theta_blob).unwrap();
    assert!(
        theta[0] > 0.0,
        "theta should move along q[0], got {theta:?}"
    );

    let theta_version: i64 = engine
        .conn()
        .query_row(
            "SELECT theta_version FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(theta_version, 1);
}

#[test]
fn degraded_provisional_submit_does_not_update_theta_or_attempt_version() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let receipt = engine
        .submit(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 4,
            latency_ms: 1000,
            hint_count: 0,
        })
        .unwrap();

    assert!(receipt.degraded);
    let theta_blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    let theta = decode_vector(&theta_blob).unwrap();
    assert!(
        theta.iter().all(|value| *value == 0.0),
        "degraded provisional submit must not update theta, got {theta:?}"
    );

    let theta_version: Option<i64> = engine
        .conn()
        .query_row(
            "SELECT theta_version FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(theta_version, None);
}

#[test]
fn final_score_accepts_legacy_free_explain_task_type() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    let receipt = engine
        .submit(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "free_explain".to_owned(),
            prompt_text: "Explain ownership freely.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 4,
            latency_ms: 1000,
            hint_count: 0,
        })
        .unwrap();

    engine.apply_final_score(&receipt.attempt_id, 0.90).unwrap();

    let theta_version: i64 = engine
        .conn()
        .query_row(
            "SELECT theta_version FROM attempts WHERE id=?1",
            [&receipt.attempt_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(theta_version, 1);
}

#[test]
fn fused_p_known_moves_from_mirt_prior_toward_bkt_with_evidence() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let prior_prediction = engine.latent_prediction("ownership", "recall").unwrap();
    assert_prediction_shape(&prior_prediction);
    let prior_fused = engine.fused_p_known("ownership", "recall").unwrap();
    assert!((prior_fused.p_known - prior_prediction.p_hat).abs() < 1e-9);

    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, attempt_count, updated_at)
             VALUES ('ownership', 0.90, 20, '2026-06-11T00:00:00Z')
             ON CONFLICT(concept_id) DO UPDATE SET p_known=excluded.p_known, attempt_count=excluded.attempt_count",
            [],
        )
        .unwrap();

    let evidenced = engine.fused_p_known("ownership", "recall").unwrap();
    assert!(evidenced.p_known > prior_prediction.p_hat);
    assert!(evidenced.p_known < 0.90);
    assert!(evidenced.lambda > 0.75);
}

fn assert_prediction_shape(prediction: &LatentPrediction) {
    assert_eq!(prediction.concept_id, "ownership");
    assert_eq!(prediction.task_type, "recall");
    assert!(prediction.p_hat > 0.0);
    assert!(prediction.p_hat < 1.0);
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
