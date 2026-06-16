use std::collections::BTreeMap;

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::phase::Phase;
use rusqlite::{params, Connection};
use serde_json::json;

#[test]
fn learner_mirror_snapshot_derives_curve_phases_and_latest_assertions() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_concept(&conn, "ownership", "Ownership", 1);
    seed_concept(&conn, "borrowing", "Borrowing", 2);
    insert_mastery_phase(&conn, "ownership", 0.35, Phase::Phantom);
    insert_attempt(
        &conn,
        "a-old",
        "ownership",
        5,
        0.90,
        Some(0.20),
        "2026-06-10T00:00:00Z",
    );
    insert_attempt(
        &conn,
        "a-new",
        "borrowing",
        3,
        0.50,
        None,
        "2026-06-11T00:00:00Z",
    );
    insert_report(&conn, "old-report", "2026-06-11T00:00:00Z", "old-assertion");
    insert_report(&conn, "new-report", "2026-06-12T00:00:00Z", "new-assertion");
    let engine = Engine::new(conn);

    let snapshot = engine.learner_mirror_snapshot().unwrap();
    let snapshot_again = engine.learner_mirror_snapshot().unwrap();

    assert_eq!(snapshot, snapshot_again);
    assert_eq!(snapshot.generated_at, "2026-06-12T00:00:00Z");
    assert_eq!(snapshot.confidence_curve.len(), 2);
    assert_eq!(snapshot.confidence_curve[0].attempt_id, "a-old");
    assert_eq!(snapshot.confidence_curve[0].confidence, 1.0);
    assert_eq!(snapshot.confidence_curve[0].actual_score, 0.20);
    assert!(snapshot.confidence_curve[0].is_final);
    assert_eq!(snapshot.confidence_curve[1].attempt_id, "a-new");
    assert_eq!(snapshot.confidence_curve[1].confidence, 0.5);
    assert_eq!(snapshot.confidence_curve[1].actual_score, 0.50);
    assert!(!snapshot.confidence_curve[1].is_final);

    let phases = snapshot
        .phase_distribution
        .iter()
        .map(|item| (item.phase.as_str(), item.count))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(snapshot.phase_distribution.len(), Phase::ALL.len());
    assert_eq!(phases.get(Phase::Phantom.as_str()), Some(&1));
    assert_eq!(phases.get(Phase::Undetermined.as_str()), Some(&1));
    assert!(snapshot
        .phase_distribution
        .iter()
        .all(|item| !item.label.is_empty() && !item.summary.is_empty()));

    assert_eq!(snapshot.recent_assertions.len(), 1);
    assert_eq!(snapshot.recent_assertions[0].id, "new-assertion");
    assert_eq!(
        snapshot.recent_assertions[0].suggested_action.as_deref(),
        Some("try transfer")
    );

    let report_count: i64 = engine
        .conn()
        .query_row("SELECT COUNT(*) FROM mirror_reports", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        report_count, 2,
        "snapshot must not run or persist a new report"
    );
}

#[test]
fn learner_mirror_snapshot_is_empty_without_attempts_or_report() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let engine = Engine::new(conn);

    let snapshot = engine.learner_mirror_snapshot().unwrap();

    assert_eq!(snapshot.generated_at, "1970-01-01T00:00:00Z");
    assert!(snapshot.confidence_curve.is_empty());
    assert_eq!(snapshot.phase_distribution.len(), Phase::ALL.len());
    assert!(snapshot.recent_assertions.is_empty());
}

fn seed_concept(conn: &Connection, id: &str, name: &str, seed_order: i64) {
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
         VALUES (?1, 'test', ?2, 'concept', ?3, 0.20, 'pack-seed', '[]', '2026-06-10T00:00:00Z')",
        (id, name, seed_order),
    )
    .unwrap();
}

fn insert_mastery_phase(conn: &Connection, concept_id: &str, p_known: f64, phase: Phase) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, calib_gap, brier_ewma, attempt_count, phase, updated_at)
         VALUES (?1, ?2, 0.0, 0.0, 2, ?3, '2026-06-10T00:00:00Z')",
        (concept_id, p_known, phase.as_str()),
    )
    .unwrap();
}

fn insert_attempt(
    conn: &Connection,
    id: &str,
    concept_id: &str,
    confidence: i64,
    provisional_score: f64,
    final_score: Option<f64>,
    created_at: &str,
) {
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                              provisional_score, final_score, created_at)
         VALUES (?1, 's1', ?2, 'recall', ?3, ?4, ?5, ?6)",
        params![
            id,
            concept_id,
            confidence,
            provisional_score,
            final_score,
            created_at
        ],
    )
    .unwrap();
}

fn insert_report(conn: &Connection, report_id: &str, generated_at: &str, assertion_id: &str) {
    let report = json!({
        "schema_version": 1,
        "id": report_id,
        "week": "2026-W24",
        "generated_at": generated_at,
        "window_days": 7,
        "assertions": [{
            "id": assertion_id,
            "kind": "calibration_phantom",
            "subject": "ownership",
            "claim": "Confidence is running ahead of actual transfer evidence.",
            "confidence": 0.82,
            "evidence_ids": ["attempt:a-old"],
            "stats": {},
            "suggested_action": "try transfer"
        }],
        "hypotheses": [],
        "suggestions": [],
        "top_signal": null,
        "skipped": [],
        "hazard_gate": {
            "participates": false,
            "reason": "fixture",
            "validation_auc": null,
            "auc_gate": 0.7
        },
        "reflection_prompts": [],
        "narrative": null
    });
    conn.execute(
        "INSERT INTO mirror_reports(id, week, generated_at, report_json, assertion_count, skipped_count)
         VALUES (?1, '2026-W24', ?2, ?3, 1, 0)",
        params![report_id, generated_at, report.to_string()],
    )
    .unwrap();
}
