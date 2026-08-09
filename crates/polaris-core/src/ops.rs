use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::db::{schema_migration_count, schema_version};
use crate::error::Result;
use crate::mastery::{fold_all, AttemptObservation, MasteryParams};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub schema_version: i64,
    pub migration_count: i64,
    pub integrity_ok: bool,
    pub integrity_messages: Vec<String>,
    pub replay_checked: usize,
    pub replay_mismatches: Vec<ReplayMismatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivitySummary {
    pub count_7d: i64,
    pub last_at: Option<String>,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorDiagnostics {
    pub window_days: i64,
    pub param_tuning_runs: ActivitySummary,
    pub breeding_evaluated_7d: ActivitySummary,
    pub breeding_admitted_7d: ActivitySummary,
    pub breeding_retired_7d: ActivitySummary,
    pub mental_fit_hazard: ActivitySummary,
    pub mental_fit_state_gate: ActivitySummary,
    pub gu_inductions: ActivitySummary,
    pub consolidation_runs: ActivitySummary,
    pub mirror_reports: ActivitySummary,
}

impl DoctorDiagnostics {
    pub fn empty(window_days: i64) -> Self {
        let empty = ActivitySummary::empty();
        Self {
            window_days,
            param_tuning_runs: empty.clone(),
            breeding_evaluated_7d: empty.clone(),
            breeding_admitted_7d: empty.clone(),
            breeding_retired_7d: empty.clone(),
            mental_fit_hazard: empty.clone(),
            mental_fit_state_gate: empty.clone(),
            gu_inductions: empty.clone(),
            consolidation_runs: empty.clone(),
            mirror_reports: empty,
        }
    }
}

impl ActivitySummary {
    pub fn empty() -> Self {
        Self {
            count_7d: 0,
            last_at: None,
            last_status: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayMismatch {
    pub concept_id: String,
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug)]
struct StoredMasteryFold {
    p_known: f64,
    fsrs_json: Option<String>,
    calib_gap: f64,
    brier_ewma: f64,
    last_depth: Option<String>,
    max_depth: Option<String>,
    attempt_count: i64,
    lapses: i64,
}

pub fn doctor_report(conn: &Connection) -> Result<DoctorReport> {
    let schema_version = schema_version(conn)?;
    let migration_count = schema_migration_count(conn)?;
    let integrity_messages = integrity_check(conn)?;
    let integrity_ok = integrity_messages.len() == 1 && integrity_messages[0] == "ok";
    let (replay_checked, replay_mismatches) = replay_self_check(conn)?;
    Ok(DoctorReport {
        ok: integrity_ok && replay_mismatches.is_empty(),
        schema_version,
        migration_count,
        integrity_ok,
        integrity_messages,
        replay_checked,
        replay_mismatches,
    })
}

pub fn doctor_diagnostics(conn: &Connection, window_days: i64) -> Result<DoctorDiagnostics> {
    let window_days = window_days.max(1);
    Ok(DoctorDiagnostics {
        window_days,
        param_tuning_runs: activity_summary(
            conn,
            "param_tuning_runs",
            "ran_at",
            "status",
            window_days,
        )?,
        breeding_evaluated_7d: activity_summary(
            conn,
            "bred_moves",
            "updated_at",
            "status",
            window_days,
        )?,
        breeding_admitted_7d: activity_summary(
            conn,
            "bred_moves",
            "admitted_at",
            "'admitted'",
            window_days,
        )?,
        breeding_retired_7d: activity_summary(
            conn,
            "bred_moves",
            "retired_at",
            "'retired'",
            window_days,
        )?,
        mental_fit_hazard: hazard_model_summary(conn, window_days)?,
        mental_fit_state_gate: state_gate_summary(conn, window_days)?,
        gu_inductions: activity_summary(conn, "gu_rules", "updated_at", "status", window_days)?,
        consolidation_runs: activity_summary(
            conn,
            "consolidation_runs",
            "ran_at",
            "status",
            window_days,
        )?,
        mirror_reports: activity_summary(
            conn,
            "mirror_reports",
            "generated_at",
            "CASE WHEN json_valid(report_json) THEN 'generated' ELSE 'parse_error' END",
            window_days,
        )?,
    })
}

fn activity_summary(
    conn: &Connection,
    table: &str,
    at_column: &str,
    status_expr: &str,
    window_days: i64,
) -> Result<ActivitySummary> {
    let count_sql = format!(
        "SELECT COUNT(*) FROM {table}
         WHERE {at_column} IS NOT NULL
           AND julianday({at_column}) >= julianday('now') - ?1"
    );
    let count_7d = conn.query_row(&count_sql, [window_days as f64], |row| row.get(0))?;
    let latest_sql = format!(
        "SELECT {at_column}, {status_expr} FROM {table}
         WHERE {at_column} IS NOT NULL
           AND julianday({at_column}) >= julianday('now') - ?1
         ORDER BY julianday({at_column}) DESC, id DESC
         LIMIT 1"
    );
    let latest: Option<(String, String)> = conn
        .query_row(&latest_sql, [window_days as f64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    Ok(ActivitySummary {
        count_7d,
        last_at: latest.as_ref().map(|(at, _)| at.clone()),
        last_status: latest.map(|(_, status)| status),
    })
}

fn hazard_model_summary(conn: &Connection, window_days: i64) -> Result<ActivitySummary> {
    let count_7d = conn.query_row(
        "SELECT COUNT(*) FROM hazard_models
         WHERE fitted_at IS NOT NULL
           AND julianday(fitted_at) >= julianday('now') - ?1",
        [window_days as f64],
        |row| row.get(0),
    )?;
    let latest: Option<(String, f64)> = conn
        .query_row(
            "SELECT fitted_at, validation_auc FROM hazard_models
             WHERE fitted_at IS NOT NULL
               AND julianday(fitted_at) >= julianday('now') - ?1
             ORDER BY julianday(fitted_at) DESC, id DESC
             LIMIT 1",
            [window_days as f64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(ActivitySummary {
        count_7d,
        last_at: latest.as_ref().map(|(at, _)| at.clone()),
        last_status: latest.map(|(_, auc)| format!("fitted (auc={auc:.2})")),
    })
}

fn state_gate_summary(conn: &Connection, window_days: i64) -> Result<ActivitySummary> {
    let count_7d = conn.query_row(
        "SELECT COUNT(*) FROM state_gate_evals
         WHERE evaluated_at IS NOT NULL
           AND julianday(evaluated_at) >= julianday('now') - ?1",
        [window_days as f64],
        |row| row.get(0),
    )?;
    let latest: Option<(String, f64, i64)> = conn
        .query_row(
            "SELECT evaluated_at, margin, passes FROM state_gate_evals
             WHERE evaluated_at IS NOT NULL
               AND julianday(evaluated_at) >= julianday('now') - ?1
             ORDER BY julianday(evaluated_at) DESC, id DESC
             LIMIT 1",
            [window_days as f64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    Ok(ActivitySummary {
        count_7d,
        last_at: latest.as_ref().map(|(at, _, _)| at.clone()),
        last_status: latest.map(|(_, margin, passes)| {
            let status = if passes != 0 { "passed" } else { "failed_gate" };
            format!("{status} (margin={margin:+.2})")
        }),
    })
}

fn integrity_check(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn replay_self_check(conn: &Connection) -> Result<(usize, Vec<ReplayMismatch>)> {
    let params = MasteryParams::from_conn(conn)?;
    let mut stmt = conn.prepare(
        "SELECT c.id,
                COALESCE(c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL))
         FROM concepts c
         WHERE EXISTS (
             SELECT 1 FROM attempts a
             WHERE a.concept_id=c.id
               AND COALESCE(a.final_score, a.provisional_score) IS NOT NULL
         )
         ORDER BY c.id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let concepts = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut mismatches = Vec::new();
    for (concept_id, p_init) in &concepts {
        let attempts = load_attempts(conn, concept_id)?;
        let expected = fold_all(*p_init, &attempts, &params);
        let Some(stored) = load_stored_mastery(conn, concept_id)? else {
            mismatches.push(ReplayMismatch {
                concept_id: concept_id.clone(),
                field: "mastery_state".to_owned(),
                expected: "present".to_owned(),
                actual: "missing".to_owned(),
            });
            continue;
        };
        push_float_mismatch(
            &mut mismatches,
            concept_id,
            "p_known",
            expected.p_known,
            stored.p_known,
        );
        push_float_mismatch(
            &mut mismatches,
            concept_id,
            "calib_gap",
            expected.calib_gap,
            stored.calib_gap,
        );
        push_float_mismatch(
            &mut mismatches,
            concept_id,
            "brier_ewma",
            expected.brier_ewma,
            stored.brier_ewma,
        );
        push_string_mismatch(
            &mut mismatches,
            concept_id,
            "fsrs_json",
            Some(serde_json::to_string(&expected.fsrs)?),
            stored.fsrs_json,
        );
        push_string_mismatch(
            &mut mismatches,
            concept_id,
            "last_depth",
            expected.last_depth,
            stored.last_depth,
        );
        push_string_mismatch(
            &mut mismatches,
            concept_id,
            "max_depth",
            expected.max_depth,
            stored.max_depth,
        );
        push_int_mismatch(
            &mut mismatches,
            concept_id,
            "attempt_count",
            i64::from(expected.attempt_count),
            stored.attempt_count,
        );
        push_int_mismatch(
            &mut mismatches,
            concept_id,
            "lapses",
            i64::from(expected.lapses),
            stored.lapses,
        );
    }
    Ok((concepts.len(), mismatches))
}

fn load_attempts(conn: &Connection, concept_id: &str) -> Result<Vec<AttemptObservation>> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(task_type, 'recall'), COALESCE(final_score, provisional_score),
                self_confidence, COALESCE(depth, 'recall'), COALESCE(created_at, '1970-01-01T00:00:00Z'),
                COALESCE(julianday(created_at), julianday('1970-01-01T00:00:00Z'))
         FROM attempts
         WHERE concept_id=?1 AND COALESCE(final_score, provisional_score) IS NOT NULL
         ORDER BY created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([concept_id], |row| {
        let mut attempt = AttemptObservation::new(
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, i32>(3)?,
            0.0,
        )
        .with_created_at(row.get::<_, String>(5)?)
        .with_occurred_day(row.get::<_, f64>(6)?);
        attempt.depth = Some(row.get(4)?);
        Ok(attempt)
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn load_stored_mastery(conn: &Connection, concept_id: &str) -> Result<Option<StoredMasteryFold>> {
    conn.query_row(
        "SELECT p_known, fsrs_json, calib_gap, brier_ewma, last_depth, max_depth,
                attempt_count, lapses
         FROM mastery_states WHERE concept_id=?1",
        [concept_id],
        |row| {
            Ok(StoredMasteryFold {
                p_known: row.get(0)?,
                fsrs_json: row.get(1)?,
                calib_gap: row.get(2)?,
                brier_ewma: row.get(3)?,
                last_depth: row.get(4)?,
                max_depth: row.get(5)?,
                attempt_count: row.get(6)?,
                lapses: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn push_float_mismatch(
    mismatches: &mut Vec<ReplayMismatch>,
    concept_id: &str,
    field: &str,
    expected: f64,
    actual: f64,
) {
    if (expected - actual).abs() > 1e-9 {
        mismatches.push(ReplayMismatch {
            concept_id: concept_id.to_owned(),
            field: field.to_owned(),
            expected: format!("{expected:.12}"),
            actual: format!("{actual:.12}"),
        });
    }
}

fn push_int_mismatch(
    mismatches: &mut Vec<ReplayMismatch>,
    concept_id: &str,
    field: &str,
    expected: i64,
    actual: i64,
) {
    if expected != actual {
        mismatches.push(ReplayMismatch {
            concept_id: concept_id.to_owned(),
            field: field.to_owned(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
}

fn push_string_mismatch(
    mismatches: &mut Vec<ReplayMismatch>,
    concept_id: &str,
    field: &str,
    expected: Option<String>,
    actual: Option<String>,
) {
    if expected != actual {
        mismatches.push(ReplayMismatch {
            concept_id: concept_id.to_owned(),
            field: field.to_owned(),
            expected: expected.unwrap_or_else(|| "<null>".to_owned()),
            actual: actual.unwrap_or_else(|| "<null>".to_owned()),
        });
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::migrate;
    use crate::engine::{Engine, SubmitInput};

    #[test]
    fn ops_doctor_detects_replay_mismatches_without_repairing_state() {
        let _env = EnvGuard::remove(&[
            "POLARIS_LLM_FAST_BASE_URL",
            "POLARIS_LLM_FAST_MODEL",
            "POLARIS_LLM_FAST_API_KEY",
            "POLARIS_LLM_STRONG_BASE_URL",
            "POLARIS_LLM_STRONG_MODEL",
            "POLARIS_LLM_STRONG_API_KEY",
        ]);
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine
            .init_pack(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("packs/rust"),
            )
            .unwrap();
        engine
            .submit(SubmitInput {
                session_id: "doctor-test".to_owned(),
                concept_id: "ownership".to_owned(),
                task_type: "recall".to_owned(),
                prompt_text: "Explain ownership.".to_owned(),
                response_text: "Ownership moves values.".to_owned(),
                self_confidence: 4,
                latency_ms: 0,
                hint_count: 0,
            })
            .unwrap();

        let clean = super::doctor_report(engine.conn()).unwrap();
        assert!(clean.ok);
        assert_eq!(clean.schema_version, crate::db::CURRENT_SCHEMA_VERSION);
        assert_eq!(clean.migration_count, 7);
        assert_eq!(clean.replay_checked, 1);
        assert!(clean.replay_mismatches.is_empty());

        engine
            .conn()
            .execute(
                "UPDATE mastery_states SET p_known=0.99 WHERE concept_id='ownership'",
                [],
            )
            .unwrap();

        let corrupted = super::doctor_report(engine.conn()).unwrap();
        assert!(!corrupted.ok);
        assert_eq!(corrupted.replay_checked, 1);
        assert_eq!(corrupted.replay_mismatches[0].concept_id, "ownership");
        assert_eq!(corrupted.replay_mismatches[0].field, "p_known");

        let stored_p_known: f64 = engine
            .conn()
            .query_row(
                "SELECT p_known FROM mastery_states WHERE concept_id='ownership'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_p_known, 0.99);
    }

    #[test]
    fn ops_doctor_diagnostics_summarizes_recent_activity() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_diagnostic_activity(&conn);

        let diagnostics = super::doctor_diagnostics(&conn, 7).unwrap();

        assert_eq!(diagnostics.window_days, 7);
        assert_eq!(diagnostics.param_tuning_runs.count_7d, 1);
        assert_eq!(
            diagnostics.param_tuning_runs.last_status.as_deref(),
            Some("accepted")
        );
        assert_eq!(diagnostics.breeding_evaluated_7d.count_7d, 1);
        assert_eq!(
            diagnostics.breeding_evaluated_7d.last_status.as_deref(),
            Some("admitted")
        );
        assert_eq!(diagnostics.breeding_admitted_7d.count_7d, 1);
        assert_eq!(diagnostics.breeding_retired_7d.count_7d, 1);
        assert_eq!(diagnostics.mental_fit_hazard.count_7d, 1);
        assert_eq!(
            diagnostics.mental_fit_hazard.last_status.as_deref(),
            Some("fitted (auc=0.82)")
        );
        assert_eq!(diagnostics.mental_fit_state_gate.count_7d, 1);
        assert_eq!(
            diagnostics.mental_fit_state_gate.last_status.as_deref(),
            Some("passed (margin=+0.05)")
        );
        assert_eq!(diagnostics.gu_inductions.count_7d, 1);
        assert_eq!(diagnostics.consolidation_runs.count_7d, 1);
        assert_eq!(diagnostics.mirror_reports.count_7d, 1);
        assert_eq!(
            diagnostics.mirror_reports.last_status.as_deref(),
            Some("generated")
        );
    }

    fn seed_diagnostic_activity(conn: &Connection) {
        conn.execute(
            "INSERT INTO param_tuning_runs(id, ran_at, param, old_value, new_value, metric, delta, status)
             VALUES ('tune-1', strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'), 'bkt.p_init', '0.20', '0.21', 'logloss', 0.01, 'accepted')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bred_moves(id, candidate_move, incumbent_move, context_hash, task_type, template,
                                    mechanisms_json, main_effect_hypothesis, prereg_json, status,
                                    created_at, updated_at, admitted_at, retired_at)
             VALUES ('breed-1', 'candidate', 'incumbent', 'ctx', 'recall', 'template',
                     '[]', 'hypothesis', '{}', 'admitted',
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-3 days'),
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 days'),
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'),
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hazard_models(id, fitted_at, beta_json, validation_auc, n_train, n_validation)
             VALUES ('hazard-1', strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'), '[]', 0.82, 80, 20)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO state_gate_evals(id, evaluated_at, baseline_auc, state_auc, margin, passes, n)
             VALUES ('state-1', strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'), 0.70, 0.75, 0.05, 1, 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO gu_rules(id, pattern, concept_ids_json, attempt_ids_json, first_seen, last_seen,
                                  count, status, alpha, beta, updated_at)
             VALUES ('gu-1', 'boundary-blindness', '[]', '[]',
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 days'),
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'),
                     1, 'active', 2.0, 1.0,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO consolidation_runs(id, ran_at, proposals_json, holdout_delta, status)
             VALUES ('consol-1', strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'), '[]', 0.0, 'rejected')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mirror_reports(id, week, generated_at, report_json, assertion_count, skipped_count)
             VALUES ('report-1', '2026-W24', strftime('%Y-%m-%dT%H:%M:%SZ','now','-1 day'), '{}', 0, 0)",
            [],
        )
        .unwrap();
    }

    struct EnvGuard {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn remove(keys: &[&'static str]) -> Self {
            let values = keys
                .iter()
                .map(|key| {
                    let value = std::env::var(key).ok();
                    std::env::remove_var(key);
                    (*key, value)
                })
                .collect();
            Self { values }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}
