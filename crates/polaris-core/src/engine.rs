use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{PolarisError, Result};
use crate::fsrs::{retrievability, FsrsParams, FsrsState, Rating};
use crate::grader::{
    grade_request_for_attempt, grade_with_config, grade_with_static_response,
    heuristic_score_with_conn, GradeRequest, LlmConfig,
};
use crate::mastery::{fold_all, AttemptObservation, MasteryParams};
use crate::pack::load_pack;
use crate::scheduler::{rank_candidates_with_params, ScheduleCandidate, SchedulerParams};

pub struct Engine {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextTask {
    pub concept_id: String,
    pub task_type: String,
    pub prompt_text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitInput {
    pub session_id: String,
    pub concept_id: String,
    pub task_type: String,
    pub prompt_text: String,
    pub response_text: String,
    pub self_confidence: i32,
    pub latency_ms: i64,
    pub hint_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubmitReceipt {
    pub attempt_id: String,
    pub provisional_score: f64,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradePendingSummary {
    pub processed: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredMasteryState {
    pub concept_id: String,
    pub p_known: f64,
    pub calib_gap: f64,
    pub brier_ewma: f64,
    pub attempt_count: i64,
}

impl Engine {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn init_pack(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let pack = load_pack(path)?;
        let tx = self.conn.transaction()?;

        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![format!("pack.{}.rubric", pack.id), pack.rubric],
        )?;

        for concept in &pack.concepts {
            tx.execute(
                "INSERT OR REPLACE INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pack-seed', '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![
                    concept.id,
                    pack.id,
                    concept.name,
                    concept.kind,
                    concept.seed_order,
                    concept.p_init
                ],
            )?;
        }

        for edge in &pack.edges {
            tx.execute(
                "INSERT OR REPLACE INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pack-seed', '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![edge.id, edge.src, edge.dst, edge.edge_type, edge.weight],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn next_task(&self) -> Result<Option<NextTask>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.seed_order,
                    COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)) AS p_known,
                    ms.fsrs_json, COALESCE(ms.calib_gap, 0.0),
                    COALESCE((SELECT COUNT(*) FROM attempts a WHERE a.concept_id=c.id), 0),
                    CASE
                        WHEN ms.last_review_at IS NULL THEN NULL
                        ELSE julianday('now') - julianday(ms.last_review_at)
                    END
             FROM concepts c
             LEFT JOIN mastery_states ms ON ms.concept_id=c.id
             ORDER BY c.seed_order ASC, c.id ASC",
        )?;

        let mut rows = stmt.query([])?;
        let mut candidates = Vec::new();
        while let Some(row) = rows.next()? {
            let concept_id: String = row.get(0)?;
            let seed_order: i64 = row.get(1)?;
            let _p_known: f64 = row.get(2)?;
            let fsrs_json: Option<String> = row.get(3)?;
            let calib_gap: f64 = row.get(4)?;
            let attempt_count: i64 = row.get(5)?;
            let elapsed_days: Option<f64> = row.get(6)?;
            let retrieval = fsrs_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<FsrsState>(json).ok())
                .map(|state| retrievability(state.stability, elapsed_days.unwrap_or(0.0).max(0.0)));

            candidates.push(ScheduleCandidate {
                id: concept_id.clone(),
                seed_order,
                retrieval,
                calib_gap,
                misconception_active: self.misconception_active(&concept_id)?,
                has_attempts: attempt_count > 0,
                prerequisites_met: self.prerequisites_met(&concept_id)?,
            });
        }

        let scheduler_params = SchedulerParams::from_conn(&self.conn)?;
        let ranked = rank_candidates_with_params(candidates, &scheduler_params);
        let Some(best) = ranked.first() else {
            return Ok(None);
        };

        let concept_name: String =
            self.conn
                .query_row("SELECT name FROM concepts WHERE id=?1", [&best.id], |row| {
                    row.get(0)
                })?;
        Ok(Some(NextTask {
            concept_id: best.id.clone(),
            task_type: "recall".to_owned(),
            prompt_text: format!("用自己的话说明 {concept_name} 的核心约束。"),
            reason: format!(
                "选它因为：当前效用最高。\n证据是：U={:.3}，按 FSRS/校准/误解/新概念门槛计算。\n现在做什么：完成一个 recall 任务。",
                best.utility
            ),
        }))
    }

    pub fn submit(&mut self, input: SubmitInput) -> Result<SubmitReceipt> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sessions(id, started_at, context_json)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), '{}')",
            [&input.session_id],
        )?;

        let evidence_id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
             VALUES (?1, ?2, 'cli-submit', 'text/plain', ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                evidence_id,
                input.session_id,
                input.response_text,
                serde_json::to_string(&vec![input.concept_id.clone()])?
            ],
        )?;

        let attempt_id = Uuid::new_v4().to_string();
        let provisional_score = heuristic_score_with_conn(&self.conn, input.self_confidence)?;
        let fsrs_params = FsrsParams::from_conn(&self.conn)?;
        self.conn.execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, prompt_text, response_evidence_id,
                                  self_confidence, latency_ms, hint_count, provisional_score, depth, rating, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'recall', ?11, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                attempt_id,
                input.session_id,
                input.concept_id,
                input.task_type,
                input.prompt_text,
                evidence_id,
                input.self_confidence,
                input.latency_ms,
                input.hint_count,
                provisional_score,
                format!("{:?}", Rating::from_score_with_params(provisional_score, &fsrs_params)).to_lowercase()
            ],
        )?;

        self.conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'latency', ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                input.session_id,
                input.concept_id,
                serde_json::json!({"latency_ms": input.latency_ms, "hint_count": input.hint_count})
                    .to_string()
            ],
        )?;

        self.replay_concept(&input.concept_id)?;
        let grade = grade_with_config(
            &self.conn,
            GradeRequest {
                attempt_id: attempt_id.clone(),
                self_confidence: input.self_confidence,
                response_text: input.response_text,
            },
            LlmConfig::from_env(),
        )?;
        if !grade.degraded {
            self.replay_concept(&input.concept_id)?;
        }

        Ok(SubmitReceipt {
            attempt_id,
            provisional_score,
            degraded: grade.degraded,
        })
    }

    pub fn apply_final_score(&mut self, attempt_id: &str, final_score: f64) -> Result<()> {
        let concept_id: String = self
            .conn
            .query_row(
                "SELECT concept_id FROM attempts WHERE id=?1",
                [attempt_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))?;

        self.conn.execute(
            "UPDATE attempts
             SET final_score=?1, graded_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'), rating=?2
             WHERE id=?3",
            params![
                final_score,
                format!(
                    "{:?}",
                    Rating::from_score_with_params(
                        final_score,
                        &FsrsParams::from_conn(&self.conn)?
                    )
                )
                .to_lowercase(),
                attempt_id
            ],
        )?;
        self.replay_concept(&concept_id)
    }

    pub fn grade_pending(&mut self) -> Result<GradePendingSummary> {
        let config = LlmConfig::from_env();
        if matches!(config, LlmConfig::Unavailable) {
            return Ok(GradePendingSummary {
                processed: 0,
                pending: self.pending_grade_count()?,
            });
        }
        self.process_queued_attempts(|conn, request| {
            grade_with_config(conn, request, config.clone())
        })
    }

    pub fn grade_pending_with_static_response(
        &mut self,
        response_json: &str,
    ) -> Result<GradePendingSummary> {
        self.process_queued_attempts(|conn, request| {
            grade_with_static_response(conn, request, response_json)
        })
    }

    pub fn mastery_state(&self, concept_id: &str) -> Result<Option<StoredMasteryState>> {
        self.conn
            .query_row(
                "SELECT concept_id, p_known, calib_gap, brier_ewma, attempt_count
                 FROM mastery_states WHERE concept_id=?1",
                [concept_id],
                |row| {
                    Ok(StoredMasteryState {
                        concept_id: row.get(0)?,
                        p_known: row.get(1)?,
                        calib_gap: row.get(2)?,
                        brier_ewma: row.get(3)?,
                        attempt_count: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn replay_concept(&self, concept_id: &str) -> Result<()> {
        let p_init: f64 = self.conn.query_row(
            "SELECT COALESCE(p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL))
             FROM concepts WHERE id=?1",
            [concept_id],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT id, COALESCE(task_type, 'recall'), COALESCE(final_score, provisional_score),
                    self_confidence, COALESCE(depth, 'recall'), COALESCE(created_at, '1970-01-01T00:00:00Z'),
                    COALESCE(julianday(created_at), julianday('1970-01-01T00:00:00Z'))
             FROM attempts
             WHERE concept_id=?1 AND COALESCE(final_score, provisional_score) IS NOT NULL
             ORDER BY created_at ASC, id ASC",
        )?;
        let attempts = stmt
            .query_map([concept_id], |row| {
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
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let params = MasteryParams::from_conn(&self.conn)?;
        let state = fold_all(p_init, &attempts, &params);
        let fsrs_json = serde_json::to_string(&state.fsrs)?;
        let last_review_at = attempts.last().map(|attempt| attempt.created_at.as_str());
        let next_due_at = match (last_review_at, state.fsrs.scheduled_days) {
            (Some(last_review_at), Some(days)) => Some(self.next_due_at(last_review_at, days)?),
            _ => None,
        };
        self.conn.execute(
            "INSERT INTO mastery_states(concept_id, p_known, fsrs_json, next_due_at, last_review_at,
                                        calib_gap, brier_ewma, last_depth, max_depth,
                                        attempt_count, lapses, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
             ON CONFLICT(concept_id) DO UPDATE SET
                p_known=excluded.p_known,
                fsrs_json=excluded.fsrs_json,
                next_due_at=excluded.next_due_at,
                last_review_at=excluded.last_review_at,
                calib_gap=excluded.calib_gap,
                brier_ewma=excluded.brier_ewma,
                last_depth=excluded.last_depth,
                max_depth=excluded.max_depth,
                attempt_count=excluded.attempt_count,
                lapses=excluded.lapses,
                updated_at=excluded.updated_at",
            params![
                concept_id,
                state.p_known,
                fsrs_json,
                next_due_at,
                last_review_at,
                state.calib_gap,
                state.brier_ewma,
                state.last_depth,
                state.max_depth,
                i64::from(state.attempt_count),
                i64::from(state.lapses),
            ],
        )?;
        Ok(())
    }

    fn process_queued_attempts<F>(&mut self, mut grade: F) -> Result<GradePendingSummary>
    where
        F: FnMut(&Connection, GradeRequest) -> Result<crate::grader::GradeResult>,
    {
        let attempt_ids = {
            let mut stmt = self.conn.prepare(
                "SELECT attempt_id FROM grade_queue ORDER BY enqueued_at ASC, attempt_id ASC",
            )?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };

        let mut processed = 0;
        for attempt_id in attempt_ids {
            let request = grade_request_for_attempt(&self.conn, &attempt_id)?;
            let result = grade(&self.conn, request)?;
            if result.degraded {
                self.increment_grade_retry(&attempt_id)?;
                continue;
            }
            let concept_id = self.concept_id_for_attempt(&attempt_id)?;
            self.replay_concept(&concept_id)?;
            processed += 1;
        }

        Ok(GradePendingSummary {
            processed,
            pending: self.pending_grade_count()?,
        })
    }

    fn pending_grade_count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM grade_queue", [], |row| row.get(0))
            .map_err(Into::into)
    }

    fn increment_grade_retry(&self, attempt_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE grade_queue
             SET retry_count=COALESCE(retry_count, 0)+1
             WHERE attempt_id=?1",
            [attempt_id],
        )?;
        Ok(())
    }

    fn concept_id_for_attempt(&self, attempt_id: &str) -> Result<String> {
        self.conn
            .query_row(
                "SELECT concept_id FROM attempts WHERE id=?1",
                [attempt_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))
    }

    fn next_due_at(&self, last_review_at: &str, days: i64) -> Result<String> {
        let modifier = format!("+{days} days");
        self.conn
            .query_row(
                "SELECT strftime('%Y-%m-%dT04:00:00Z', date(?1, ?2))",
                params![last_review_at, modifier],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn prerequisites_met(&self, concept_id: &str) -> Result<bool> {
        let prereq_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM edges WHERE dst=?1 AND type='prerequisite'",
            [concept_id],
            |row| row.get(0),
        )?;
        if prereq_count == 0 {
            return Ok(true);
        }

        let unmet: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM edges e
             LEFT JOIN mastery_states ms ON ms.concept_id=e.src
             WHERE e.dst=?1 AND e.type='prerequisite' AND COALESCE(ms.p_known, 0.0) < CAST((SELECT value FROM meta WHERE key='sched.prereq_p') AS REAL)",
            [concept_id],
            |row| row.get(0),
        )?;
        Ok(unmet == 0)
    }

    fn misconception_active(&self, concept_id: &str) -> Result<bool> {
        let active: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM attempts a
             WHERE a.concept_id=?1 AND a.misconception_id IS NOT NULL
               AND julianday(COALESCE(a.created_at, '1970-01-01T00:00:00Z')) >=
                   julianday('now') - CAST((SELECT value FROM meta WHERE key='sched.mis_window_days') AS INTEGER)
               AND NOT EXISTS (
                 SELECT 1 FROM attempts later
                 WHERE later.concept_id=a.concept_id
                   AND (
                       julianday(COALESCE(later.created_at, '1970-01-01T00:00:00Z')) >
                       julianday(COALESCE(a.created_at, '1970-01-01T00:00:00Z'))
                       OR (
                           later.created_at = a.created_at AND later.id > a.id
                       )
                   )
                   AND later.final_score >= CAST((SELECT value FROM meta WHERE key='bkt.cut_hi') AS REAL)
               )",
            [concept_id],
            |row| row.get(0),
        )?;
        Ok(active > 0)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrate;

    #[test]
    fn init_pack_installs_concepts_and_next_returns_first_open_concept() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);

        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
        let next = engine.next_task().unwrap().expect("next task");

        assert_eq!(next.concept_id, "ownership");
        let rubric: String = engine
            .conn()
            .query_row(
                "SELECT value FROM meta WHERE key='pack.rust.rubric'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(rubric.contains("Rubric"));
        assert!(next.reason.contains("选它因为"));
    }

    #[test]
    fn submit_without_llm_records_provisional_mastery_and_retry_queue() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

        let receipt = engine
            .submit(SubmitInput {
                session_id: "s1".to_owned(),
                concept_id: "ownership".to_owned(),
                task_type: "recall".to_owned(),
                prompt_text: "Explain ownership.".to_owned(),
                response_text: "Ownership controls which binding can drop a value.".to_owned(),
                self_confidence: 4,
                latency_ms: 1200,
                hint_count: 0,
            })
            .unwrap();

        assert!((receipt.provisional_score - 0.70).abs() < 1e-9);
        assert!(receipt.degraded);

        let state = engine.mastery_state("ownership").unwrap().expect("mastery");
        assert_eq!(state.attempt_count, 1);

        let queued: i64 = engine
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM grade_queue WHERE attempt_id=?1",
                [&receipt.attempt_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(queued, 1);
    }

    #[test]
    fn final_score_replay_preserves_provisional_history() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

        let receipt = engine
            .submit(SubmitInput {
                session_id: "s1".to_owned(),
                concept_id: "ownership".to_owned(),
                task_type: "recall".to_owned(),
                prompt_text: "Explain ownership.".to_owned(),
                response_text: "Ownership controls drops.".to_owned(),
                self_confidence: 5,
                latency_ms: 800,
                hint_count: 0,
            })
            .unwrap();
        let before = engine.mastery_state("ownership").unwrap().unwrap();

        engine.apply_final_score(&receipt.attempt_id, 0.20).unwrap();
        let after = engine.mastery_state("ownership").unwrap().unwrap();

        let scores: (f64, f64) = engine
            .conn()
            .query_row(
                "SELECT provisional_score, final_score FROM attempts WHERE id=?1",
                [&receipt.attempt_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(scores.0, receipt.provisional_score);
        assert_eq!(scores.1, 0.20);
        assert_ne!(before.p_known, after.p_known);
    }

    #[test]
    fn integration_seed_flow_prioritizes_high_confidence_low_final_score() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

        let concepts = [
            "ownership",
            "ownership",
            "ownership",
            "references",
            "references",
            "pattern_matching",
            "pattern_matching",
            "traits",
            "traits",
            "modules",
            "closures",
        ];
        let mut receipts = Vec::new();
        for (idx, concept) in concepts.iter().enumerate() {
            let receipt = engine
                .submit(SubmitInput {
                    session_id: "integration".to_owned(),
                    concept_id: (*concept).to_owned(),
                    task_type: "recall".to_owned(),
                    prompt_text: format!("Explain {concept}."),
                    response_text: format!("{concept} answer {idx}"),
                    self_confidence: if *concept == "ownership" { 5 } else { 3 },
                    latency_ms: 1000 + idx as i64,
                    hint_count: 0,
                })
                .unwrap();
            receipts.push(((*concept).to_owned(), receipt));
        }

        for (concept, receipt) in &receipts {
            let final_score = if concept == "ownership" { 0.20 } else { 0.82 };
            engine
                .apply_final_score(&receipt.attempt_id, final_score)
                .unwrap();
        }

        let next = engine.next_task().unwrap().expect("next task");
        assert_eq!(next.concept_id, "ownership");

        let state = engine.mastery_state("ownership").unwrap().unwrap();
        assert!(state.calib_gap > 0.25);

        let queued: i64 = engine
            .conn()
            .query_row("SELECT COUNT(*) FROM grade_queue", [], |row| row.get(0))
            .unwrap();
        assert_eq!(queued, 11);

        let due: String = engine
            .conn()
            .query_row(
                "SELECT next_due_at FROM mastery_states WHERE concept_id='ownership'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(due.ends_with("T04:00:00Z"), "due was {due}");

        let due_order: i64 = engine
            .conn()
            .query_row(
                "SELECT julianday((SELECT next_due_at FROM mastery_states WHERE concept_id='ownership')) <=
                        julianday((SELECT next_due_at FROM mastery_states WHERE concept_id='traits'))",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(due_order, 1);
    }

    #[test]
    fn replay_uses_attempt_created_at_for_fsrs_elapsed_days() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

        for (id, at) in [
            ("a1", "2026-06-01T00:00:00Z"),
            ("a2", "2026-06-04T00:00:00Z"),
        ] {
            engine
                .conn()
                .execute(
                    "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence, provisional_score, final_score, created_at)
                     VALUES (?1, 's1', 'ownership', 'recall', 4, 0.80, 0.80, ?2)",
                    (id, at),
                )
                .unwrap();
        }

        engine.replay_concept("ownership").unwrap();

        let fsrs_json: String = engine
            .conn()
            .query_row(
                "SELECT fsrs_json FROM mastery_states WHERE concept_id='ownership'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let state: FsrsState = serde_json::from_str(&fsrs_json).unwrap();
        assert!(
            state.stability > 2.4,
            "second review should use three elapsed days, got stability {}",
            state.stability
        );
    }

    #[test]
    fn grade_pending_processes_queued_attempts() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

        engine
            .conn()
            .execute(
                "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
                 VALUES ('ev1', 's1', 'cli-submit', 'text/plain', 'Ownership controls which binding can drop a value.', '[\"ownership\"]', '2026-06-11T00:00:00Z')",
                [],
            )
            .unwrap();
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, response_evidence_id, self_confidence, provisional_score, created_at)
                 VALUES ('attempt-queued', 's1', 'ownership', 'recall', 'ev1', 4, 0.70, '2026-06-11T00:00:00Z')",
                [],
            )
            .unwrap();
        engine
            .conn()
            .execute(
                "INSERT INTO grade_queue(attempt_id, enqueued_at, retry_count, last_error)
                 VALUES ('attempt-queued', '2026-06-11T00:00:00Z', 0, 'llm config missing')",
                [],
            )
            .unwrap();

        let summary = engine
            .grade_pending_with_static_response(
                r#"{"score":0.83,"depth":"explain","citations":[{"evidence_id":"ev1","quote":"controls which binding"}]}"#,
            )
            .unwrap();

        assert_eq!(summary.processed, 1);
        assert_eq!(summary.pending, 0);
        let final_score: f64 = engine
            .conn()
            .query_row(
                "SELECT final_score FROM attempts WHERE id='attempt-queued'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!((final_score - 0.83).abs() < 1e-9);
    }

    #[test]
    fn next_task_uses_engine_misconception_window_semantics() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let mut engine = Engine::new(conn);
        engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, concept_id, task_type, self_confidence, provisional_score,
                                      final_score, misconception_id, created_at)
                 VALUES ('mis1', 'references', 'recall', 5, 0.20, 0.20, 'm1', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                [],
            )
            .unwrap();
        engine
            .conn()
            .execute(
                "INSERT INTO mastery_states(concept_id, p_known, fsrs_json, calib_gap, brier_ewma, attempt_count, updated_at)
                 VALUES ('references', 0.20, NULL, 0.0, 0.0, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                [],
            )
            .unwrap();

        let next = engine.next_task().unwrap().unwrap();
        assert_eq!(next.concept_id, "references");

        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, concept_id, task_type, self_confidence, provisional_score,
                                      final_score, created_at)
                 VALUES ('pass1', 'references', 'recall', 3, 0.90, 0.90, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                [],
            )
            .unwrap();

        let next_after_success = engine.next_task().unwrap().unwrap();
        assert_ne!(next_after_success.concept_id, "references");
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }
}
