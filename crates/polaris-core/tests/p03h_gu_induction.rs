use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::Connection;
use serde_json::json;

#[test]
fn three_cross_concept_failed_pattern_tags_generate_candidate() {
    let engine = seeded_engine();
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("fail-{idx}"),
            concept,
            0.20,
            &["boundary-blindness"],
            "2026-06-01T00:00:00Z",
        );
    }

    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.candidates_created, 1);
    let rule = gu_rule(&engine, "boundary-blindness").unwrap();
    assert_eq!(rule.status, "candidate");
    assert_eq!(rule.count, 3);
    assert_eq!(
        rule.concept_ids,
        vec!["borrowing", "lifetimes", "ownership"]
    );
    assert_eq!(rule.attempt_ids, vec!["fail-0", "fail-1", "fail-2"]);
    assert_lifecycle_event(&engine, "candidate");
}

#[test]
fn candidate_without_holdout_gate_remains_candidate() {
    let engine = seeded_engine();
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("fail-{idx}"),
            concept,
            0.20,
            &["causal-inversion"],
            "2026-06-01T00:00:00Z",
        );
    }

    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.validated, 0);
    let rule = gu_rule(&engine, "causal-inversion").unwrap();
    assert_eq!(rule.status, "candidate");
    assert_eq!(confusion_edge_count(&engine, &rule.id), 0);
}

#[test]
fn candidate_passing_holdout_gate_creates_misconception_node_and_confusion_edges() {
    let engine = seeded_engine();
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("fail-{idx}"),
            concept,
            0.20,
            &["interference-confusion"],
            "2026-06-01T00:00:00Z",
        );
    }
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("holdout-hit-{idx}"),
            concept,
            0.30,
            &["interference-confusion"],
            "2026-06-02T00:00:00Z",
        );
    }
    for (idx, concept) in ["traits", "modules", "closures"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("baseline-{idx}"),
            concept,
            0.85,
            &[],
            "2026-06-02T00:00:00Z",
        );
    }

    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.validated, 1);
    let rule = gu_rule(&engine, "interference-confusion").unwrap();
    assert_eq!(rule.status, "validated");
    assert_eq!(
        misconception_node_kind(&engine, &rule.id),
        "misconception_induced"
    );
    assert_eq!(confusion_edge_count(&engine, &rule.id), 3);
    assert_lifecycle_event(&engine, "validated");
}

#[test]
fn first_consumption_marks_validated_rule_active_and_correct_streak_resolves_it() {
    let engine = seeded_engine();
    create_validated_rule(&engine, "procedural-conceptual-gap");

    let active = engine.active_gu_rules_for_concept("ownership").unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].status, "active");

    for idx in 0..3 {
        insert_graded_attempt(
            &engine,
            &format!("correct-{idx}"),
            "ownership",
            0.90,
            &[],
            &format!("2026-06-0{}T00:00:00Z", idx + 3),
        );
    }
    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.resolved, 1);
    let rule = gu_rule(&engine, "procedural-conceptual-gap").unwrap();
    assert_eq!(rule.status, "resolved");
}

#[test]
fn scheduler_query_does_not_activate_validated_rule() {
    let engine = seeded_engine();
    create_validated_rule(&engine, "symbol-referent-confusion");

    let _ = engine.next_task().unwrap();

    let rule = gu_rule(&engine, "symbol-referent-confusion").unwrap();
    assert_eq!(rule.status, "validated");
}

#[test]
fn low_precision_active_rule_is_retired() {
    let engine = seeded_engine();
    create_validated_rule(&engine, "fluency-illusion");
    let _ = engine.active_gu_rules_for_concept("ownership").unwrap();
    for idx in 0..6 {
        insert_graded_attempt(
            &engine,
            &format!("miss-{idx}"),
            "ownership",
            0.20,
            &[],
            &format!("2026-06-0{}T00:00:00Z", idx + 3),
        );
    }

    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.retired, 1);
    let rule = gu_rule(&engine, "fluency-illusion").unwrap();
    assert_eq!(rule.status, "retired");
}

#[test]
fn stale_candidate_expires_after_window_without_new_evidence() {
    let engine = seeded_engine();
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("old-fail-{idx}"),
            concept,
            0.20,
            &["granularity-mismatch"],
            "2026-04-01T00:00:00Z",
        );
    }
    engine.run_gu_induction().unwrap();
    insert_graded_attempt(
        &engine,
        "today",
        "traits",
        0.90,
        &[],
        "2026-06-12T00:00:00Z",
    );

    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.expired, 1);
    let rule = gu_rule(&engine, "granularity-mismatch").unwrap();
    assert_eq!(rule.status, "expired");
}

#[test]
fn attempts_without_pattern_tags_do_not_create_gu_candidates() {
    let engine = seeded_engine();
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            &engine,
            &format!("plain-fail-{idx}"),
            concept,
            0.20,
            &[],
            "2026-06-01T00:00:00Z",
        );
    }

    let summary = engine.run_gu_induction().unwrap();

    assert_eq!(summary.candidates_created, 0);
    assert_eq!(gu_rule_count(&engine), 0);
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn create_validated_rule(engine: &Engine, pattern: &str) {
    for (idx, concept) in ["ownership", "borrowing", "lifetimes"].iter().enumerate() {
        insert_graded_attempt(
            engine,
            &format!("{pattern}-fail-{idx}"),
            concept,
            0.20,
            &[pattern],
            "2026-06-01T00:00:00Z",
        );
        insert_graded_attempt(
            engine,
            &format!("{pattern}-hit-{idx}"),
            concept,
            0.30,
            &[pattern],
            "2026-06-02T00:00:00Z",
        );
    }
    engine.run_gu_induction().unwrap();
}

fn insert_graded_attempt(
    engine: &Engine,
    id: &str,
    concept_id: &str,
    score: f64,
    pattern_tags: &[&str],
    created_at: &str,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence, provisional_score,
                                  final_score, depth, grader_json, rating, created_at, graded_at)
             VALUES (?1, 's1', ?2, 'recall', 2, 0.30, ?3, 'recall', ?4, 'again', ?5, ?5)",
            (
                id,
                concept_id,
                score,
                json!({
                    "score": score,
                    "depth": "recall",
                    "pattern_tags": pattern_tags,
                    "citations": [],
                })
                .to_string(),
                created_at,
            ),
        )
        .unwrap();
}

#[derive(Debug)]
struct RuleSnapshot {
    id: String,
    status: String,
    count: i64,
    concept_ids: Vec<String>,
    attempt_ids: Vec<String>,
}

fn gu_rule(engine: &Engine, pattern: &str) -> Option<RuleSnapshot> {
    engine
        .conn()
        .query_row(
            "SELECT id, status, count, concept_ids_json, attempt_ids_json
             FROM gu_rules WHERE pattern=?1 ORDER BY id LIMIT 1",
            [pattern],
            |row| {
                let concept_ids_json: String = row.get(3)?;
                let attempt_ids_json: String = row.get(4)?;
                Ok(RuleSnapshot {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    count: row.get(2)?,
                    concept_ids: serde_json::from_str(&concept_ids_json).unwrap(),
                    attempt_ids: serde_json::from_str(&attempt_ids_json).unwrap(),
                })
            },
        )
        .ok()
}

fn gu_rule_count(engine: &Engine) -> i64 {
    engine
        .conn()
        .query_row("SELECT COUNT(*) FROM gu_rules", [], |row| row.get(0))
        .unwrap()
}

fn misconception_node_kind(engine: &Engine, rule_id: &str) -> String {
    engine
        .conn()
        .query_row(
            "SELECT kind FROM concepts WHERE id=?1",
            [format!("gu:{rule_id}")],
            |row| row.get(0),
        )
        .unwrap()
}

fn confusion_edge_count(engine: &Engine, rule_id: &str) -> i64 {
    engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM edges WHERE src=?1 AND type='confusion' AND provenance='engine'",
            [format!("gu:{rule_id}")],
            |row| row.get(0),
        )
        .unwrap()
}

fn assert_lifecycle_event(engine: &Engine, status: &str) {
    let count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM behavior_events
             WHERE type='gu_lifecycle'
               AND json_extract(payload_json, '$.status')=?1",
            [status],
            |row| row.get(0),
        )
        .unwrap();
    assert!(count > 0, "missing lifecycle event for {status}");
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
