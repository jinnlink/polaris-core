use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::report::MirrorReport;
use rusqlite::Connection;

#[test]
fn empty_database_produces_report_with_no_unevidenced_items() {
    let engine = seeded_engine();

    let report = engine.run_mirror_report().unwrap();

    assert!(report.assertions.is_empty());
    assert!(report.hypotheses.is_empty());
    assert!(report.suggestions.is_empty());
    assert!(!report.hazard_gate.participates);
    assert_eq!(report.hazard_gate.reason, "no_mental_state_data");
    assert_eq!(report.reflection_prompts.len(), 3);

    let stored: i64 = engine
        .conn()
        .query_row("SELECT COUNT(*) FROM mirror_reports", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, 1);
}

#[test]
fn calibration_phantom_assertion_carries_attempt_evidence_and_confidence() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);

    let report = engine.run_mirror_report().unwrap();

    let assertion = find_item(&report.assertions, "calibration_phantom:ownership")
        .expect("phantom assertion present");
    assert!(assertion.confidence >= 0.6, "got {}", assertion.confidence);
    assert_eq!(assertion.evidence_ids.len(), 4);
    assert_eq!(
        assertion.stats["probability_over_half"].as_f64().unwrap(),
        assertion.confidence
    );
    for evidence in &assertion.evidence_ids {
        let attempt_id = evidence.strip_prefix("attempt:").expect("attempt prefix");
        let exists: i64 = engine
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM attempts WHERE id=?1",
                [attempt_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "evidence {evidence} must resolve");
    }
    assert!(assertion.claim.contains("幻影"));
}

#[test]
fn calibration_phantom_below_min_evidence_is_skipped_with_audit_trail() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 2);

    let report = engine.run_mirror_report().unwrap();

    assert!(find_item(&report.assertions, "calibration_phantom:ownership").is_none());
    let skipped = report
        .skipped
        .iter()
        .find(|skip| skip.id == "calibration_phantom:ownership")
        .expect("skip recorded");
    assert_eq!(skipped.reason, "insufficient_evidence");
}

#[test]
fn hint_streak_followed_by_abandons_yields_conditional_assertion() {
    let engine = seeded_engine();
    for idx in 0..3 {
        let session = format!("hint-session-{idx}");
        insert_behavior_event(
            &engine,
            &format!("hint-a-{idx}"),
            &session,
            "hint",
            "-9 minutes",
        );
        insert_behavior_event(
            &engine,
            &format!("hint-b-{idx}"),
            &session,
            "hint",
            "-8 minutes",
        );
        insert_behavior_event(
            &engine,
            &format!("abandon-{idx}"),
            &session,
            "abandon",
            "-6 minutes",
        );
    }
    for idx in 0..6 {
        insert_attempt_at(
            &engine,
            &format!("baseline-{idx}"),
            "traits",
            &format!("calm-session-{idx}"),
            "-30 minutes",
        );
    }

    let report = engine.run_mirror_report().unwrap();

    let assertion = find_item(&report.assertions, "hint_abandon_conditional:hint_streak_2")
        .expect("conditional assertion present");
    assert!(
        assertion.claim.contains("3/3"),
        "claim: {}",
        assertion.claim
    );
    assert!(assertion.confidence >= 0.6, "got {}", assertion.confidence);
    assert!(assertion
        .evidence_ids
        .iter()
        .any(|id| id.starts_with("behavior:hint-b-")));
    assert!(assertion
        .evidence_ids
        .iter()
        .any(|id| id.starts_with("behavior:abandon-")));
}

#[test]
fn single_hint_episode_is_skipped_for_insufficient_evidence() {
    let engine = seeded_engine();
    insert_behavior_event(&engine, "hint-a", "s1", "hint", "-9 minutes");
    insert_behavior_event(&engine, "hint-b", "s1", "hint", "-8 minutes");
    insert_behavior_event(&engine, "abandon-1", "s1", "abandon", "-6 minutes");
    insert_attempt_at(&engine, "baseline-0", "traits", "calm", "-30 minutes");

    let report = engine.run_mirror_report().unwrap();

    assert!(find_item(&report.assertions, "hint_abandon_conditional:hint_streak_2").is_none());
    let skipped = report
        .skipped
        .iter()
        .find(|skip| skip.id == "hint_abandon_conditional:hint_streak_2")
        .expect("skip recorded");
    assert_eq!(skipped.reason, "insufficient_evidence");
}

#[test]
fn abandon_time_contrast_reports_direction_with_confidence() {
    let engine = seeded_engine();
    for idx in 0..5 {
        insert_behavior_event_at_hour(
            &engine,
            &format!("evening-abandon-{idx}"),
            &format!("evening-{idx}"),
            "abandon",
            19,
        );
        insert_attempt_at_hour(
            &engine,
            &format!("evening-attempt-{idx}"),
            "ownership",
            &format!("evening-{idx}"),
            19,
        );
    }
    for idx in 0..10 {
        insert_attempt_at_hour(
            &engine,
            &format!("morning-attempt-{idx}"),
            "traits",
            &format!("morning-{idx}"),
            8,
        );
    }

    let report = engine.run_mirror_report().unwrap();

    let assertion = find_item(
        &report.assertions,
        "abandon_time_contrast:bucket3_vs_bucket1",
    )
    .expect("contrast assertion present");
    assert!(
        assertion.claim.contains("晚上"),
        "claim: {}",
        assertion.claim
    );
    assert!(
        assertion.claim.contains("上午"),
        "claim: {}",
        assertion.claim
    );
    assert!(assertion.confidence >= 0.6, "got {}", assertion.confidence);
    assert!(assertion
        .evidence_ids
        .iter()
        .all(|id| id.starts_with("behavior:evening-abandon-")));
}

#[test]
fn hazard_prediction_assertions_are_gated_out_while_model_unfit() {
    let engine = seeded_engine();
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES ('ms-1', 's1', strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'mental_state', 'ownership',
                     '{\"hazard\":{\"validation_auc\":null,\"probability\":0.9}}')",
            [],
        )
        .unwrap();
    seed_phantom_concept(&engine, "ownership", 4);

    let report = engine.run_mirror_report().unwrap();

    assert!(!report.hazard_gate.participates);
    assert_eq!(report.hazard_gate.reason, "model_unfit");
    assert!(report
        .assertions
        .iter()
        .all(|item| item.kind != "hazard_prediction"));
}

#[test]
fn consolidation_proposals_surface_as_gated_hypotheses() {
    let engine = seeded_engine();
    engine
        .conn()
        .execute(
            "INSERT INTO consolidation_runs(id, ran_at, proposals_json, holdout_delta, status)
             VALUES ('run-1', strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                     '[{\"kind\":\"candidate_latent_dimension\",\"concepts\":[\"ownership\",\"borrowing\",\"lifetimes\"]}]',
                     0.0, 'rejected')",
            [],
        )
        .unwrap();

    let report = engine.run_mirror_report().unwrap();

    let hypothesis = find_item(&report.hypotheses, "consolidation_hypothesis:run-1:0")
        .expect("hypothesis present");
    assert_eq!(hypothesis.evidence_ids, vec!["consolidation:run-1"]);
    assert!(hypothesis.claim.contains("未过留出验证门"));
    assert!(report.assertions.is_empty());
}

#[test]
fn active_gu_rule_appears_with_attempt_evidence() {
    let engine = seeded_engine();
    for idx in 0..3 {
        insert_attempt_at(
            &engine,
            &format!("gu-attempt-{idx}"),
            "ownership",
            "s1",
            "-2 days",
        );
    }
    engine
        .conn()
        .execute(
            "INSERT INTO gu_rules(id, pattern, concept_ids_json, attempt_ids_json, first_seen, last_seen,
                                  count, status, alpha, beta, updated_at)
             VALUES ('rule-1', 'boundary-blindness',
                     '[\"ownership\",\"borrowing\"]',
                     '[\"gu-attempt-0\",\"gu-attempt-1\",\"gu-attempt-2\"]',
                     '2026-06-01T00:00:00Z', '2026-06-10T00:00:00Z',
                     3, 'active', 4.0, 1.0, '2026-06-10T00:00:00Z')",
            [],
        )
        .unwrap();

    let report = engine.run_mirror_report().unwrap();

    let assertion =
        find_item(&report.assertions, "gu_pattern:rule-1").expect("gu assertion present");
    assert_eq!(assertion.subject, "boundary-blindness");
    assert_eq!(assertion.evidence_ids.len(), 3);
    assert!(assertion
        .evidence_ids
        .iter()
        .all(|id| id.starts_with("attempt:gu-attempt-")));
    assert!(assertion.confidence > 0.9, "got {}", assertion.confidence);
    assert!(assertion.claim.contains("不是个人特质"));
}

#[test]
fn param_suggestion_fires_without_mutating_meta() {
    let engine = seeded_engine();
    for idx in 0..12 {
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, provisional_score,
                                      final_score, created_at, graded_at)
                 VALUES (?1, 's1', 'ownership', 'recall', 0.9, 0.5,
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'),
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'))",
                [format!("biased-{idx}")],
            )
            .unwrap();
    }

    let report = engine.run_mirror_report().unwrap();

    let suggestion = find_item(&report.suggestions, "param_suggestion:grade.provisional")
        .expect("suggestion present");
    assert!(suggestion.claim.contains("仅建议"));
    assert!(!suggestion.evidence_ids.is_empty());

    let base: String = engine
        .conn()
        .query_row(
            "SELECT value FROM meta WHERE key='grade.provisional_base'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let slope: String = engine
        .conn()
        .query_row(
            "SELECT value FROM meta WHERE key='grade.provisional_slope'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(base, "0.10");
    assert_eq!(slope, "0.80");
}

#[test]
fn inaccurate_feedback_suppresses_assertion_in_next_report() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);

    let first = engine.run_mirror_report().unwrap();
    assert!(find_item(&first.assertions, "calibration_phantom:ownership").is_some());

    let report_id = engine
        .record_report_feedback(None, "calibration_phantom:ownership")
        .unwrap();
    assert_eq!(report_id, first.id);

    let feedback_events: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events WHERE type='report_feedback'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(feedback_events, 1);

    let second = engine.run_mirror_report().unwrap();
    assert!(find_item(&second.assertions, "calibration_phantom:ownership").is_none());
    let skipped = second
        .skipped
        .iter()
        .find(|skip| skip.id == "calibration_phantom:ownership")
        .expect("suppression recorded");
    assert_eq!(skipped.reason, "user_marked_inaccurate");
}

#[test]
fn feedback_for_unknown_assertion_is_rejected() {
    let engine = seeded_engine();
    engine.run_mirror_report().unwrap();

    let result = engine.record_report_feedback(None, "calibration_phantom:nonexistent");

    assert!(result.is_err());
}

#[test]
fn report_generation_is_deterministic_on_stable_fields() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);
    seed_phantom_concept(&engine, "borrowing", 5);

    let first = engine.run_mirror_report().unwrap();
    let second = engine.run_mirror_report().unwrap();

    assert_eq!(stable_fields(&first), stable_fields(&second));
    assert_ne!(first.id, second.id);
}

#[test]
fn latest_mirror_report_returns_most_recent() {
    let engine = seeded_engine();

    let generated = engine.run_mirror_report().unwrap();
    let latest = engine.latest_mirror_report().unwrap().expect("report row");

    assert_eq!(generated.id, latest.id);
    assert_eq!(generated.week, latest.week);
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

fn seed_phantom_concept(engine: &Engine, concept_id: &str, attempt_count: usize) {
    for idx in 0..attempt_count {
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                      final_score, created_at, graded_at)
                 VALUES (?1, 's1', ?2, 'recall', 5, 0.2,
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'),
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'))",
                (format!("{concept_id}-phantom-{idx}"), concept_id),
            )
            .unwrap();
    }
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO mastery_states(concept_id, p_known, calib_gap, attempt_count, updated_at)
             VALUES (?1, 0.30, 0.40, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            (concept_id, attempt_count as i64),
        )
        .unwrap();
}

fn insert_behavior_event(engine: &Engine, id: &str, session: &str, event_type: &str, offset: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now',?3), ?4, 'ownership', '{}')",
            (id, session, offset, event_type),
        )
        .unwrap();
}

fn insert_behavior_event_at_hour(
    engine: &Engine,
    id: &str,
    session: &str,
    event_type: &str,
    hour: u32,
) {
    engine
        .conn()
        .execute(
            &format!(
                "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
                 VALUES (?1, ?2, strftime('%Y-%m-%d','now','-1 day') || 'T{hour:02}:00:00Z', ?3, 'ownership', '{{}}')"
            ),
            (id, session, event_type),
        )
        .unwrap();
}

fn insert_attempt_at(engine: &Engine, id: &str, concept_id: &str, session: &str, offset: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, final_score, created_at, graded_at)
             VALUES (?1, ?2, ?3, 'recall', 0.8,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now',?4),
                     strftime('%Y-%m-%dT%H:%M:%SZ','now',?4))",
            (id, session, concept_id, offset),
        )
        .unwrap();
}

fn insert_attempt_at_hour(engine: &Engine, id: &str, concept_id: &str, session: &str, hour: u32) {
    engine
        .conn()
        .execute(
            &format!(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, final_score, created_at, graded_at)
                 VALUES (?1, ?2, ?3, 'recall', 0.8,
                         strftime('%Y-%m-%d','now','-1 day') || 'T{hour:02}:00:00Z',
                         strftime('%Y-%m-%d','now','-1 day') || 'T{hour:02}:00:00Z')"
            ),
            (id, session, concept_id),
        )
        .unwrap();
}

fn find_item<'report>(
    items: &'report [polaris_core::report::ReportItem],
    id: &str,
) -> Option<&'report polaris_core::report::ReportItem> {
    items.iter().find(|item| item.id == id)
}

fn stable_fields(report: &MirrorReport) -> Vec<(String, String, String, Vec<String>)> {
    report
        .assertions
        .iter()
        .chain(report.hypotheses.iter())
        .chain(report.suggestions.iter())
        .map(|item| {
            (
                item.id.clone(),
                item.claim.clone(),
                format!("{:.9}", item.confidence),
                item.evidence_ids.clone(),
            )
        })
        .collect()
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
