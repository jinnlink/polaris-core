use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::mirt::{decode_vector, encode_vector, LatentPrediction};
use polaris_core::pack_state::ThetaMode;
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
fn init_pack_initializes_theta_adagrad_accumulator() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let (theta_blob, g2_blob): (Vec<u8>, Vec<u8>) = engine
        .conn()
        .query_row("SELECT vec, g2 FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let theta = decode_vector(&theta_blob).unwrap();
    let g2 = decode_vector(&g2_blob).unwrap();

    assert_eq!(g2.len(), theta.len());
    assert!(g2.iter().all(|value| *value == 0.0));
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
fn repeated_theta_updates_use_adagrad_accumulator_to_reduce_step() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let first_attempt = submit_ownership_attempt(&mut engine);
    engine.apply_final_score(&first_attempt, 0.90).unwrap();
    let theta_after_first = theta_vec(engine.conn());
    let g2_after_first = theta_g2(engine.conn());
    let first_delta = theta_after_first[0];

    let second_attempt = submit_ownership_attempt(&mut engine);
    engine.apply_final_score(&second_attempt, 0.90).unwrap();
    let theta_after_second = theta_vec(engine.conn());
    let g2_after_second = theta_g2(engine.conn());
    let second_delta = theta_after_second[0] - theta_after_first[0];

    assert!(
        first_delta > 0.045,
        "first AdaGrad step should approach the configured cap, got {first_delta}"
    );
    assert!(second_delta > 0.0);
    assert!(
        second_delta < first_delta,
        "g2 accumulation should reduce repeated same-dimension step: first={first_delta}, second={second_delta}"
    );
    assert!(g2_after_first[0] > 0.0);
    assert!(g2_after_second[0] > g2_after_first[0]);
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
    assert!((evidenced.p_known - legacy_lambda_fusion(&evidenced)).abs() < 1e-12);
    assert!(evidenced.shadow_uses_inverse_variance);
    assert!(evidenced.shadow_p_known >= evidenced.mirt_p_hat);
    assert!(evidenced.shadow_p_known <= evidenced.bkt_p_known);
    assert!(evidenced.shadow_variance > 0.0);
    assert!(evidenced.shadow_variance <= evidenced.bkt_variance.max(evidenced.mirt_variance));
}

#[test]
fn shadow_fusion_falls_back_to_legacy_without_evidence() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let fused = engine.fused_p_known("ownership", "recall").unwrap();

    assert_eq!(fused.lambda, 0.0);
    assert!(!fused.shadow_uses_inverse_variance);
    assert!((fused.shadow_p_known - fused.p_known).abs() < 1e-12);
    assert!((fused.shadow_bkt_weight - fused.lambda).abs() < 1e-12);
}

#[test]
fn shadow_bkt_variance_shrinks_with_more_evidence() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    upsert_mastery(engine.conn(), 0.60, 2);
    let low_n = engine.fused_p_known("ownership", "recall").unwrap();
    upsert_mastery(engine.conn(), 0.60, 20);
    let high_n = engine.fused_p_known("ownership", "recall").unwrap();

    assert!(low_n.shadow_uses_inverse_variance);
    assert!(high_n.shadow_uses_inverse_variance);
    assert!(high_n.bkt_variance < low_n.bkt_variance);
}

#[test]
fn shadow_mirt_variance_shrinks_with_adagrad_information() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    upsert_mastery(engine.conn(), 0.60, 10);

    set_theta_g2(engine.conn(), 0.0);
    let low_info = engine.fused_p_known("ownership", "recall").unwrap();
    set_theta_g2(engine.conn(), 100.0);
    let high_info = engine.fused_p_known("ownership", "recall").unwrap();

    assert!(low_info.shadow_uses_inverse_variance);
    assert!(high_info.shadow_uses_inverse_variance);
    assert!(high_info.mirt_variance < low_info.mirt_variance);
}

#[test]
fn shadow_fusion_falls_back_to_legacy_when_mirt_variance_is_invalid() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    upsert_mastery(engine.conn(), 0.80, 20);
    engine
        .conn()
        .execute("UPDATE theta SET g2=x'0000' WHERE id=1", [])
        .unwrap();

    let fused = engine.fused_p_known("ownership", "recall").unwrap();

    assert!(!fused.shadow_uses_inverse_variance);
    assert!((fused.shadow_p_known - fused.p_known).abs() < 1e-12);
    assert!((fused.shadow_bkt_weight - fused.lambda).abs() < 1e-12);
}

#[test]
fn shadow_mirt_variance_uses_isolated_pack_adagrad_information() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .switch_pack("rust", Some(ThetaMode::Isolated))
        .unwrap();
    upsert_mastery(engine.conn(), 0.60, 10);

    set_pack_theta_g2(engine.conn(), "rust", 0.0);
    let low_info = engine.fused_p_known("ownership", "recall").unwrap();
    set_pack_theta_g2(engine.conn(), "rust", 100.0);
    let high_info = engine.fused_p_known("ownership", "recall").unwrap();

    assert!(low_info.shadow_uses_inverse_variance);
    assert!(high_info.shadow_uses_inverse_variance);
    assert!(high_info.mirt_variance < low_info.mirt_variance);
}

#[test]
fn shadow_fusion_falls_back_to_legacy_when_isolated_pack_mirt_variance_is_invalid() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .switch_pack("rust", Some(ThetaMode::Isolated))
        .unwrap();
    upsert_mastery(engine.conn(), 0.80, 20);
    engine
        .conn()
        .execute("UPDATE pack_theta SET g2=x'0000' WHERE pack='rust'", [])
        .unwrap();

    let fused = engine.fused_p_known("ownership", "recall").unwrap();

    assert!(!fused.shadow_uses_inverse_variance);
    assert!((fused.shadow_p_known - fused.p_known).abs() < 1e-12);
    assert!((fused.shadow_bkt_weight - fused.lambda).abs() < 1e-12);
}

fn assert_prediction_shape(prediction: &LatentPrediction) {
    assert_eq!(prediction.concept_id, "ownership");
    assert_eq!(prediction.task_type, "recall");
    assert!(prediction.p_hat > 0.0);
    assert!(prediction.p_hat < 1.0);
}

fn legacy_lambda_fusion(fused: &polaris_core::mirt::FusedPKnown) -> f64 {
    fused.lambda * fused.bkt_p_known + (1.0 - fused.lambda) * fused.mirt_p_hat
}

fn upsert_mastery(conn: &Connection, p_known: f64, attempt_count: i64) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, attempt_count, updated_at)
         VALUES ('ownership', ?1, ?2, '2026-06-17T00:00:00Z')
         ON CONFLICT(concept_id) DO UPDATE SET
            p_known=excluded.p_known,
            attempt_count=excluded.attempt_count",
        (p_known, attempt_count),
    )
    .unwrap();
}

fn set_theta_g2(conn: &Connection, value: f64) {
    let g2 = vec![value; 32];
    conn.execute("UPDATE theta SET g2=?1 WHERE id=1", [encode_vector(&g2)])
        .unwrap();
}

fn set_pack_theta_g2(conn: &Connection, pack_id: &str, value: f64) {
    let g2 = vec![value; 32];
    conn.execute(
        "UPDATE pack_theta SET g2=?1 WHERE pack=?2",
        (encode_vector(&g2), pack_id),
    )
    .unwrap();
}

fn submit_ownership_attempt(engine: &mut Engine) -> String {
    engine
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
        .unwrap()
        .attempt_id
}

fn theta_vec(conn: &Connection) -> Vec<f64> {
    let blob: Vec<u8> = conn
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    decode_vector(&blob).unwrap()
}

fn theta_g2(conn: &Connection) -> Vec<f64> {
    let blob: Vec<u8> = conn
        .query_row("SELECT g2 FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    decode_vector(&blob).unwrap()
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
