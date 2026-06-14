use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::Result;
use crate::mastery::{fold_all, AttemptObservation, MasteryParams};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub integrity_ok: bool,
    pub integrity_messages: Vec<String>,
    pub replay_checked: usize,
    pub replay_mismatches: Vec<ReplayMismatch>,
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
    let integrity_messages = integrity_check(conn)?;
    let integrity_ok = integrity_messages.len() == 1 && integrity_messages[0] == "ok";
    let (replay_checked, replay_mismatches) = replay_self_check(conn)?;
    Ok(DoctorReport {
        ok: integrity_ok && replay_mismatches.is_empty(),
        integrity_ok,
        integrity_messages,
        replay_checked,
        replay_mismatches,
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
