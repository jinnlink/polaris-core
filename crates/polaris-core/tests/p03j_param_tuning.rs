use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::Connection;

#[test]
fn insufficient_history_skips_without_audit_rows() {
    let engine = seeded_engine();
    for idx in 0..5 {
        insert_graded(&engine, &format!("a-{idx}"), "ownership", 0.2, 2, 50 - idx);
    }

    let summary = engine.run_param_tuning().unwrap();

    assert!(summary.outcomes.is_empty());
    assert_eq!(summary.skipped.len(), 1);
    assert!(summary.skipped[0].starts_with("all:insufficient_history"));
    assert_eq!(audit_row_count(&engine), 0);
    assert_eq!(meta(&engine, "tuning.rotation_cursor"), "0");
    assert_eq!(meta(&engine, "bkt.guess"), "0.20");
}

#[test]
fn failing_history_tunes_guess_downward_and_audits() {
    let engine = seeded_engine();
    seed_failing_history(&engine);
    set_meta(&engine, "tuning.rotation_cursor", "2");
    set_meta(&engine, "tuning.max_params_per_run", "1");

    let summary = engine.run_param_tuning().unwrap();

    assert_eq!(summary.outcomes.len(), 1);
    let outcome = &summary.outcomes[0];
    assert_eq!(outcome.param, "bkt.guess");
    assert!(outcome.accepted);
    assert!(outcome.delta > 0.0);
    let new_value: f64 = outcome.new_value.parse().unwrap();
    assert!(new_value < 0.20, "guess should move down, got {new_value}");
    assert_eq!(meta(&engine, "bkt.guess"), outcome.new_value);

    let (param, status): (String, String) = engine
        .conn()
        .query_row("SELECT param, status FROM param_tuning_runs", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(param, "bkt.guess");
    assert_eq!(status, "accepted");
    assert_eq!(meta(&engine, "tuning.rotation_cursor"), "3");
}

#[test]
fn high_margin_rejects_change_and_keeps_meta() {
    let engine = seeded_engine();
    seed_failing_history(&engine);
    set_meta(&engine, "tuning.rotation_cursor", "2");
    set_meta(&engine, "tuning.max_params_per_run", "1");
    set_meta(&engine, "tuning.accept_margin", "0.5");

    let summary = engine.run_param_tuning().unwrap();

    assert_eq!(summary.outcomes.len(), 1);
    assert!(!summary.outcomes[0].accepted);
    assert_eq!(meta(&engine, "bkt.guess"), "0.20");
    let status: String = engine
        .conn()
        .query_row("SELECT status FROM param_tuning_runs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(status, "rejected");
}

#[test]
fn provisional_pair_regression_accepted_when_biased() {
    let engine = seeded_engine();
    for idx in 0..36 {
        let (confidence, final_score) = match idx % 3 {
            0 => (2, 0.2),
            1 => (3, 0.5),
            _ => (4, 0.8),
        };
        insert_graded(
            &engine,
            &format!("pair-{idx}"),
            "ownership",
            final_score,
            confidence,
            120 - idx,
        );
    }
    set_meta(&engine, "tuning.rotation_cursor", "5");

    let summary = engine.run_param_tuning().unwrap();

    assert_eq!(summary.outcomes.len(), 2);
    assert!(summary.outcomes.iter().all(|outcome| outcome.accepted));
    assert!(summary.outcomes.iter().all(|outcome| outcome.delta > 0.0));
    let base: f64 = meta(&engine, "grade.provisional_base").parse().unwrap();
    let slope: f64 = meta(&engine, "grade.provisional_slope").parse().unwrap();
    assert!((base - 0.0).abs() < 1e-9, "base clamped to 0, got {base}");
    assert!(
        (slope - 1.0).abs() < 1e-9,
        "slope clamped to 1, got {slope}"
    );
    assert_eq!(audit_row_count(&engine), 2);
    assert_eq!(meta(&engine, "tuning.rotation_cursor"), "0");
}

#[test]
fn pair_skipped_when_budget_insufficient() {
    let engine = seeded_engine();
    seed_failing_history(&engine);
    set_meta(&engine, "tuning.rotation_cursor", "5");
    set_meta(&engine, "tuning.max_params_per_run", "1");

    let summary = engine.run_param_tuning().unwrap();

    assert!(summary.outcomes.is_empty());
    assert_eq!(
        summary.skipped,
        vec!["grade.provisional:budget_exhausted".to_owned()]
    );
    assert_eq!(audit_row_count(&engine), 0);
    assert_eq!(meta(&engine, "tuning.rotation_cursor"), "5");
}

#[test]
fn gate_manual_and_mrt_params_never_touched() {
    let engine = seeded_engine();
    seed_failing_history(&engine);
    set_meta(&engine, "tuning.max_params_per_run", "12");

    let summary = engine.run_param_tuning().unwrap();
    assert!(!summary.outcomes.is_empty());

    for key in [
        ("bkt.cut_hi", "0.75"),
        ("bkt.cut_lo", "0.40"),
        ("sched.w_r", "0.40"),
        ("friction.w1", "0.40"),
        ("hazard.auc_gate", "0.70"),
        ("tuning.accept_margin", "0.005"),
    ] {
        assert_eq!(meta(&engine, key.0), key.1, "{} must stay untouched", key.0);
    }

    let whitelist = [
        "bkt.p_init",
        "bkt.slip",
        "bkt.guess",
        "bkt.guess_explain",
        "bkt.learn",
        "grade.provisional_base",
        "grade.provisional_slope",
    ];
    let mut stmt = engine
        .conn()
        .prepare("SELECT DISTINCT param FROM param_tuning_runs")
        .unwrap();
    let audited = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!audited.is_empty());
    for param in &audited {
        assert!(
            whitelist.contains(&param.as_str()),
            "audited param {param} outside whitelist"
        );
    }
}

#[test]
fn cursor_rotation_advances_and_wraps() {
    let engine = seeded_engine();
    seed_failing_history(&engine);
    set_meta(&engine, "tuning.rotation_cursor", "4");
    set_meta(&engine, "tuning.max_params_per_run", "2");

    let first = engine.run_param_tuning().unwrap();
    assert_eq!(first.outcomes.len(), 1, "bkt.learn evaluated");
    assert_eq!(first.outcomes[0].param, "bkt.learn");
    assert!(first
        .skipped
        .contains(&"grade.provisional:budget_exhausted".to_owned()));
    assert_eq!(meta(&engine, "tuning.rotation_cursor"), "5");

    let second = engine.run_param_tuning().unwrap();
    assert_eq!(second.outcomes.len(), 2, "provisional pair evaluated");
    assert_eq!(meta(&engine, "tuning.rotation_cursor"), "0");
}

#[test]
fn tuning_is_deterministic_for_same_state() {
    let engine = seeded_engine();
    seed_failing_history(&engine);
    set_meta(&engine, "tuning.rotation_cursor", "2");
    set_meta(&engine, "tuning.max_params_per_run", "1");

    let first = engine.run_param_tuning().unwrap();

    set_meta(&engine, "bkt.guess", "0.20");
    set_meta(&engine, "tuning.rotation_cursor", "2");
    engine
        .conn()
        .execute("DELETE FROM param_tuning_runs", [])
        .unwrap();

    let second = engine.run_param_tuning().unwrap();

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

fn seed_failing_history(engine: &Engine) {
    let concepts = ["ownership", "borrowing", "lifetimes", "traits"];
    for idx in 0..40 {
        insert_graded(
            engine,
            &format!("fail-{idx:02}"),
            concepts[idx % concepts.len()],
            0.2,
            2,
            200 - idx,
        );
    }
}

fn insert_graded(
    engine: &Engine,
    id: &str,
    concept_id: &str,
    final_score: f64,
    confidence: i32,
    minutes_ago: usize,
) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                  provisional_score, final_score, created_at, graded_at)
             VALUES (?1, 's1', ?2, 'recall', ?3, 0.5, ?4,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now',?5),
                     strftime('%Y-%m-%dT%H:%M:%SZ','now',?5))",
            (
                id,
                concept_id,
                confidence,
                final_score,
                format!("-{minutes_ago} minutes"),
            ),
        )
        .unwrap();
}

fn set_meta(engine: &Engine, key: &str, value: &str) {
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            (key, value),
        )
        .unwrap();
}

fn meta(engine: &Engine, key: &str) -> String {
    engine
        .conn()
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .unwrap()
}

fn audit_row_count(engine: &Engine) -> i64 {
    engine
        .conn()
        .query_row("SELECT COUNT(*) FROM param_tuning_runs", [], |row| {
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
