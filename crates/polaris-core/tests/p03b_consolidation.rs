use polaris_core::consolidation::run_nightly_consolidation;
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::mirt::decode_vector;
use rusqlite::{params, Connection};
use serde_json::Value;

#[test]
fn nightly_consolidation_refreshes_residual_stats() {
    let engine = initialized_engine();
    let (monday, tuesday) = recent_same_iso_week(engine.conn());
    seed_final_attempt_at(engine.conn(), "ownership", "a1", 0.90, &monday);
    seed_final_attempt_at(engine.conn(), "ownership", "a2", 0.70, &tuesday);

    let summary = run_nightly_consolidation(engine.conn()).unwrap();

    assert!(summary.residual_rows >= 1);
    let (mean_resid, n): (f64, i64) = engine
        .conn()
        .query_row(
            "SELECT mean_resid, n FROM residual_stats WHERE concept_id='ownership' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(n, 2);
    assert!(
        mean_resid > 0.20,
        "residual should use MIRT p_hat, got {mean_resid}"
    );
}

#[test]
fn nightly_consolidation_snapshots_and_shrinks_theta() {
    let engine = initialized_engine();
    engine
        .conn()
        .execute(
            "UPDATE theta SET vec=?1, version=1 WHERE id=1",
            [polaris_core::mirt::encode_vector(&vec![1.0; 32])],
        )
        .unwrap();

    run_nightly_consolidation(engine.conn()).unwrap();

    let history_count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM theta_history WHERE version=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(history_count, 1);

    let (theta_blob, version): (Vec<u8>, i64) = engine
        .conn()
        .query_row("SELECT vec, version FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let theta = decode_vector(&theta_blob).unwrap();
    assert_eq!(version, 2);
    assert!(theta[0] < 1.0);
    assert!(theta[0] > 0.99);
}

#[test]
fn candidate_cluster_is_audited_and_rejected_without_mutating_q() {
    let engine = initialized_engine();
    for (days_ago, score) in [(28, 0.40), (21, 0.80), (14, 0.60), (7, 0.90)] {
        for concept in ["ownership", "moves", "borrowing"] {
            seed_final_attempt(
                engine.conn(),
                concept,
                &format!("{concept}-{days_ago}"),
                score,
                days_ago,
            );
        }
    }
    engine
        .conn()
        .execute(
            "UPDATE meta SET value='0.0' WHERE key='consol.accept_margin'",
            [],
        )
        .unwrap();
    let q_len_before = concept_q_len(engine.conn(), "ownership");

    let summary = run_nightly_consolidation(engine.conn()).unwrap();

    assert!(!summary.accepted);
    assert!(summary.proposal_count >= 1);
    assert_eq!(q_len_before, concept_q_len(engine.conn(), "ownership"));

    let (status, holdout_delta, proposals_json): (String, f64, String) = engine
        .conn()
        .query_row(
            "SELECT status, holdout_delta, proposals_json
             FROM consolidation_runs
             ORDER BY ran_at DESC, id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(status, "rejected");
    assert_eq!(holdout_delta, 0.0);
    let proposals: Value = serde_json::from_str(&proposals_json).unwrap();
    assert_eq!(proposals[0]["kind"], "candidate_latent_dimension");
    assert!(proposals[0]["concepts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "ownership"));
}

fn initialized_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    let theta_blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT vec FROM theta WHERE id=1", [], |row| row.get(0))
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT OR IGNORE INTO theta_history(version, vec, at)
         VALUES (1, ?1, '2026-06-11T00:00:00Z')",
            [theta_blob],
        )
        .unwrap();
    engine
}

fn seed_final_attempt(
    conn: &Connection,
    concept_id: &str,
    attempt_id: &str,
    final_score: f64,
    days_ago: i64,
) {
    let created_at: String = conn
        .query_row(
            "SELECT strftime('%Y-%m-%dT00:00:00Z', 'now', ?1)",
            [format!("-{days_ago} days")],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                              provisional_score, final_score, theta_version, created_at)
         VALUES (?1, 's1', ?2, 'recall', 3, ?3, ?3, 1, ?4)",
        params![attempt_id, concept_id, final_score, created_at],
    )
    .unwrap();
}

fn seed_final_attempt_at(
    conn: &Connection,
    concept_id: &str,
    attempt_id: &str,
    final_score: f64,
    created_at: &str,
) {
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                              provisional_score, final_score, theta_version, created_at)
         VALUES (?1, 's1', ?2, 'recall', 3, ?3, ?3, 1, ?4)",
        params![attempt_id, concept_id, final_score, created_at],
    )
    .unwrap();
}

fn recent_same_iso_week(conn: &Connection) -> (String, String) {
    conn.query_row(
        "SELECT
            strftime('%Y-%m-%dT00:00:00Z', 'now', 'weekday 1', '-14 days'),
            strftime('%Y-%m-%dT00:00:00Z', 'now', 'weekday 1', '-13 days')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap()
}

fn concept_q_len(conn: &Connection, concept_id: &str) -> usize {
    let q_blob: Vec<u8> = conn
        .query_row("SELECT q FROM concepts WHERE id=?1", [concept_id], |row| {
            row.get(0)
        })
        .unwrap();
    decode_vector(&q_blob).unwrap().len()
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
