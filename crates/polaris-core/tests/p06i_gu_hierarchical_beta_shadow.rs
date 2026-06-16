use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::gu_prior::{GuPriorShadowStatus, GuPriorValidationStatus};
use rusqlite::{params, Connection};

#[test]
fn empty_database_returns_no_data_without_writes() {
    let engine = test_engine();
    let before = table_counts(&engine);

    let summary = engine.gu_prior_shadow().unwrap();

    assert_eq!(summary.status, GuPriorShadowStatus::NoData);
    assert_eq!(summary.rules_evaluated, 0);
    assert_eq!(summary.holdout_attempt_count, 0);
    assert!(summary.rows.is_empty());
    assert_eq!(summary.validation.status, GuPriorValidationStatus::Skipped);
    assert_eq!(table_counts(&engine), before);
}

#[test]
fn no_source_evidence_degenerates_to_flat_beta_prior() {
    let engine = test_engine();
    set_meta(&engine, "gu_prior.min_shadow_rules", "1");
    set_meta(&engine, "gu_prior.min_holdout_attempts", "1");
    insert_concept(&engine, "ownership");
    insert_rule(
        &engine,
        "rule-target",
        "boundary-blindness",
        &["ownership"],
        "2026-01-01T00:00:00Z",
    );
    insert_attempt(
        &engine,
        "holdout-hit",
        "ownership",
        "2026-01-02T00:00:00Z",
        0.2,
        Some("boundary-blindness"),
    );
    insert_attempt(
        &engine,
        "holdout-miss",
        "ownership",
        "2026-01-03T00:00:00Z",
        0.8,
        None,
    );

    let summary = engine.gu_prior_shadow().unwrap();

    assert_eq!(summary.status, GuPriorShadowStatus::ShadowReady);
    assert_eq!(summary.rows.len(), 1);
    let row = &summary.rows[0];
    assert_eq!(row.rule_id, "rule-target");
    assert_close(row.flat_prior_alpha, 1.0);
    assert_close(row.flat_prior_beta, 1.0);
    assert_close(row.hierarchical_prior_alpha, 1.0);
    assert_close(row.hierarchical_prior_beta, 1.0);
    assert_eq!(row.source_attempt_count, 0);
    assert_close(row.flat_logloss, row.hierarchical_logloss);
    assert_close(row.flat_brier, row.hierarchical_brier);
    assert_eq!(summary.validation.status, GuPriorValidationStatus::Computed);
    assert_eq!(summary.validation.passed, Some(true));

    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["status"], "shadow_ready");
    assert_eq!(json["validation"]["status"], "computed");
}

#[test]
fn source_evidence_builds_bounded_hierarchical_prior_without_mutating_gu_rules() {
    let engine = test_engine();
    set_meta(&engine, "gu_prior.min_shadow_rules", "1");
    set_meta(&engine, "gu_prior.min_holdout_attempts", "1");
    set_meta(&engine, "gu_prior.max_prior_strength", "4");
    insert_concept(&engine, "ownership");
    insert_concept(&engine, "borrowing");
    insert_edge(&engine, "edge-ob", "ownership", "borrowing");
    insert_rule(
        &engine,
        "rule-source",
        "boundary-blindness",
        &["borrowing"],
        "2025-12-20T00:00:00Z",
    );
    insert_rule(
        &engine,
        "rule-target",
        "boundary-blindness",
        &["ownership"],
        "2026-01-01T00:00:00Z",
    );
    for idx in 0..30 {
        insert_attempt(
            &engine,
            &format!("source-hit-{idx:02}"),
            "borrowing",
            &format!("2025-12-{day:02}T00:00:00Z", day = 1 + idx % 20),
            0.2,
            Some("boundary-blindness"),
        );
    }
    for idx in 0..10 {
        insert_attempt(
            &engine,
            &format!("source-miss-{idx:02}"),
            "borrowing",
            &format!("2025-12-{day:02}T12:00:00Z", day = 1 + idx),
            0.8,
            None,
        );
    }
    for idx in 0..4 {
        insert_attempt(
            &engine,
            &format!("holdout-hit-{idx:02}"),
            "ownership",
            &format!("2026-01-{day:02}T00:00:00Z", day = 2 + idx),
            0.2,
            Some("boundary-blindness"),
        );
    }

    let before_changes = engine.conn().total_changes();
    let before_gu = gu_snapshot(&engine);
    let before_counts = table_counts(&engine);
    let summary = engine.gu_prior_shadow().unwrap();
    let after_gu = gu_snapshot(&engine);
    let after_counts = table_counts(&engine);
    let after_changes = engine.conn().total_changes();

    assert_eq!(
        before_gu, after_gu,
        "shadow summary must not mutate G_u rules"
    );
    assert_eq!(
        before_changes, after_changes,
        "shadow summary must not write any rows"
    );
    assert_eq!(
        before_counts, after_counts,
        "shadow summary must not insert/delete business rows"
    );
    assert_eq!(summary.status, GuPriorShadowStatus::ShadowReady);
    let row = summary
        .rows
        .iter()
        .find(|row| row.rule_id == "rule-target")
        .unwrap();
    assert_eq!(row.source_attempt_count, 40);
    assert_close(row.hierarchical_prior_alpha, 4.0);
    assert_close(row.hierarchical_prior_beta, 2.0);
    assert!(row.hierarchical_logloss < row.flat_logloss);
    assert_eq!(summary.validation.passed, Some(true));
}

#[test]
fn same_pattern_source_uses_only_rules_known_before_target_holdout() {
    let engine = test_engine();
    set_meta(&engine, "gu_prior.min_shadow_rules", "1");
    set_meta(&engine, "gu_prior.min_holdout_attempts", "1");
    set_meta(&engine, "gu_prior.max_prior_strength", "4");
    insert_concept(&engine, "ownership");
    insert_concept(&engine, "past-source");
    insert_concept(&engine, "future-source");
    insert_rule(
        &engine,
        "rule-past-source",
        "interference-confusion",
        &["past-source"],
        "2025-12-20T00:00:00Z",
    );
    insert_rule(
        &engine,
        "rule-target",
        "interference-confusion",
        &["ownership"],
        "2026-01-10T00:00:00Z",
    );
    insert_rule(
        &engine,
        "rule-future-source",
        "interference-confusion",
        &["future-source"],
        "2026-01-20T00:00:00Z",
    );
    insert_attempt(
        &engine,
        "past-hit",
        "past-source",
        "2026-01-05T00:00:00Z",
        0.2,
        Some("interference-confusion"),
    );
    insert_attempt(
        &engine,
        "future-hit-before-target",
        "future-source",
        "2026-01-05T00:00:00Z",
        0.2,
        Some("interference-confusion"),
    );
    insert_attempt(
        &engine,
        "target-hit",
        "ownership",
        "2026-01-11T00:00:00Z",
        0.2,
        Some("interference-confusion"),
    );

    let summary = engine.gu_prior_shadow().unwrap();
    let target = summary
        .rows
        .iter()
        .find(|row| row.rule_id == "rule-target")
        .unwrap();

    assert_eq!(
        target.source_attempt_count, 1,
        "future same-pattern rules must not leak into earlier priors"
    );
    assert_close(target.hierarchical_prior_alpha, 2.0);
    assert_close(target.hierarchical_prior_beta, 1.0);
}

#[test]
fn source_evidence_excludes_future_grades_and_future_edges() {
    let engine = test_engine();
    set_meta(&engine, "gu_prior.min_shadow_rules", "1");
    set_meta(&engine, "gu_prior.min_holdout_attempts", "1");
    set_meta(&engine, "gu_prior.max_prior_strength", "4");
    insert_concept(&engine, "ownership");
    insert_concept(&engine, "borrowing");
    insert_concept(&engine, "traits");
    insert_concept_at(&engine, "future-concept", "2026-01-20T00:00:00Z");
    insert_edge_at(
        &engine,
        "edge-past",
        "ownership",
        "borrowing",
        "2026-01-01T00:00:00Z",
    );
    insert_edge_at(
        &engine,
        "edge-future",
        "ownership",
        "traits",
        "2026-01-20T00:00:00Z",
    );
    insert_edge_at(
        &engine,
        "edge-cutoff",
        "ownership",
        "future-concept",
        "2026-01-10T00:00:00Z",
    );
    insert_rule(
        &engine,
        "rule-target",
        "boundary-blindness",
        &["ownership"],
        "2026-01-10T00:00:00Z",
    );
    insert_rule(
        &engine,
        "rule-future-concept-source",
        "boundary-blindness",
        &["future-concept"],
        "2026-01-01T00:00:00Z",
    );
    insert_attempt_with_graded_at(
        &engine,
        "past-grade-hit",
        "borrowing",
        "2026-01-05T00:00:00Z",
        Some("2026-01-05T01:00:00Z"),
        0.2,
        Some("boundary-blindness"),
    );
    insert_attempt_with_graded_at(
        &engine,
        "future-grade-hit",
        "borrowing",
        "2026-01-05T00:00:00Z",
        Some("2026-01-15T00:00:00Z"),
        0.2,
        Some("boundary-blindness"),
    );
    insert_attempt(
        &engine,
        "future-created-hit",
        "borrowing",
        "2026-01-10T00:00:00Z",
        0.2,
        Some("boundary-blindness"),
    );
    insert_attempt(
        &engine,
        "future-edge-hit",
        "traits",
        "2026-01-05T00:00:00Z",
        0.2,
        Some("boundary-blindness"),
    );
    insert_attempt(
        &engine,
        "future-concept-hit",
        "future-concept",
        "2026-01-05T00:00:00Z",
        0.2,
        Some("boundary-blindness"),
    );
    insert_attempt(
        &engine,
        "target-hit",
        "ownership",
        "2026-01-11T00:00:00Z",
        0.2,
        Some("boundary-blindness"),
    );

    let summary = engine.gu_prior_shadow().unwrap();
    let row = summary
        .rows
        .iter()
        .find(|row| row.rule_id == "rule-target")
        .unwrap();

    assert_eq!(row.source_attempt_count, 1);
    assert_close(row.hierarchical_prior_alpha, 2.0);
    assert_close(row.hierarchical_prior_beta, 1.0);
}

#[test]
fn holdout_respects_gu_window_days_inclusive_upper_bound() {
    let engine = test_engine();
    set_meta(&engine, "gu_prior.min_shadow_rules", "1");
    set_meta(&engine, "gu_prior.min_holdout_attempts", "1");
    set_meta(&engine, "gu.window_days", "2");
    insert_concept(&engine, "ownership");
    insert_rule(
        &engine,
        "rule-target",
        "fluency-illusion",
        &["ownership"],
        "2026-01-01T00:00:00Z",
    );
    insert_attempt(
        &engine,
        "at-lower-bound",
        "ownership",
        "2026-01-01T00:00:00Z",
        0.2,
        Some("fluency-illusion"),
    );
    insert_attempt(
        &engine,
        "at-upper-bound",
        "ownership",
        "2026-01-03T00:00:00Z",
        0.2,
        Some("fluency-illusion"),
    );
    insert_attempt(
        &engine,
        "outside-window",
        "ownership",
        "2026-01-03T00:00:01Z",
        0.2,
        Some("fluency-illusion"),
    );

    let summary = engine.gu_prior_shadow().unwrap();
    let row = &summary.rows[0];

    assert_eq!(row.holdout_attempt_count, 1);
    assert_eq!(summary.holdout_attempt_count, 1);
}

#[test]
fn insufficient_holdout_skips_validation_without_claiming_success() {
    let engine = test_engine();
    set_meta(&engine, "gu_prior.min_shadow_rules", "1");
    set_meta(&engine, "gu_prior.min_holdout_attempts", "3");
    insert_concept(&engine, "ownership");
    insert_rule(
        &engine,
        "rule-target",
        "fluency-illusion",
        &["ownership"],
        "2026-01-01T00:00:00Z",
    );
    insert_attempt(
        &engine,
        "holdout-one",
        "ownership",
        "2026-01-02T00:00:00Z",
        0.2,
        Some("fluency-illusion"),
    );

    let summary = engine.gu_prior_shadow().unwrap();

    assert_eq!(summary.status, GuPriorShadowStatus::InsufficientData);
    assert_eq!(summary.validation.status, GuPriorValidationStatus::Skipped);
    assert_eq!(summary.validation.passed, None);
    assert_eq!(summary.holdout_attempt_count, 1);
}

fn test_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn set_meta(engine: &Engine, key: &str, value: &str) {
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .unwrap();
}

fn insert_concept(engine: &Engine, id: &str) {
    insert_concept_at(engine, id, "2025-12-01T00:00:00Z");
}

fn insert_concept_at(engine: &Engine, id: &str, created_at: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO concepts(id, pack, name, kind, seed_order, provenance, evidence_ids_json, created_at)
             VALUES (?1, 'test', ?1, 'concept', 0, 'pack-seed', '[]', ?2)",
            params![id, created_at],
        )
        .unwrap();
}

fn insert_edge(engine: &Engine, id: &str, src: &str, dst: &str) {
    insert_edge_at(engine, id, src, dst, "2025-12-01T00:00:00Z");
}

fn insert_edge_at(engine: &Engine, id: &str, src: &str, dst: &str, created_at: &str) {
    engine
        .conn()
        .execute(
            "INSERT INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
             VALUES (?1, ?2, ?3, 'confusion', 1.0, 'engine', '[]', ?4)",
            params![id, src, dst, created_at],
        )
        .unwrap();
}

fn insert_rule(engine: &Engine, id: &str, pattern: &str, concepts: &[&str], last_seen: &str) {
    let concept_ids_json = serde_json::to_string(concepts).unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO gu_rules(id, pattern, concept_ids_json, attempt_ids_json, first_seen, last_seen,
                                  count, status, alpha, beta, correct_streak, updated_at)
             VALUES (?1, ?2, ?3, '[]', '2025-12-01T00:00:00Z', ?4, 3, 'validated', 5.0, 2.0, 0,
                     '2026-01-01T00:00:00Z')",
            params![id, pattern, concept_ids_json, last_seen],
        )
        .unwrap();
}

fn insert_attempt(
    engine: &Engine,
    id: &str,
    concept_id: &str,
    created_at: &str,
    final_score: f64,
    pattern: Option<&str>,
) {
    insert_attempt_with_graded_at(
        engine,
        id,
        concept_id,
        created_at,
        Some(created_at),
        final_score,
        pattern,
    );
}

fn insert_attempt_with_graded_at(
    engine: &Engine,
    id: &str,
    concept_id: &str,
    created_at: &str,
    graded_at: Option<&str>,
    final_score: f64,
    pattern: Option<&str>,
) {
    let grader_json = match pattern {
        Some(pattern) => serde_json::json!({ "pattern_tags": [pattern] }).to_string(),
        None => serde_json::json!({ "pattern_tags": [] }).to_string(),
    };
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, task_type, final_score, self_confidence, grader_json, created_at, graded_at)
             VALUES (?1, ?2, 'recall', ?3, 3, ?4, ?5, ?6)",
            params![id, concept_id, final_score, grader_json, created_at, graded_at],
        )
        .unwrap();
}

fn table_counts(engine: &Engine) -> (i64, i64, i64, i64) {
    (
        count(engine, "gu_rules"),
        count(engine, "attempts"),
        count(engine, "edges"),
        count(engine, "mastery_states"),
    )
}

fn count(engine: &Engine, table: &str) -> i64 {
    engine
        .conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn gu_snapshot(engine: &Engine) -> Vec<(String, String, f64, f64, i64, Option<String>)> {
    let mut stmt = engine
        .conn()
        .prepare(
            "SELECT id, status, alpha, beta, correct_streak, consumed_at
             FROM gu_rules
             ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ))
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "actual {actual}, expected {expected}"
    );
}
