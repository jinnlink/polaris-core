use super::*;

impl Engine {
    pub fn submit_no_attempt(
        &mut self,
        input: SubmitInput,
        reason: &str,
    ) -> Result<NoAttemptReceipt> {
        let reason =
            NoAttemptReason::parse(reason).ok_or_else(|| PolarisError::InvalidParameter {
                key: "no_attempt_reason".to_owned(),
                value: reason.to_owned(),
            })?;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.record_no_attempt_submission(input, reason);
        match result {
            Ok(receipt) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(receipt)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub(crate) fn record_no_attempt_submission(
        &mut self,
        input: SubmitInput,
        reason: NoAttemptReason,
    ) -> Result<NoAttemptReceipt> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sessions(id, started_at, context_json)
                 VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), '{}')",
            [&input.session_id],
        )?;
        let evidence_id = Uuid::new_v4().to_string();
        self.conn.execute(
                "INSERT INTO evidence_items(id, session_id, source, content_type, text, concept_ids_json, created_at)
                 VALUES (?1, ?2, 'no-attempt', 'text/plain', ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![
                    evidence_id,
                    input.session_id,
                    input.response_text,
                    serde_json::to_string(&vec![input.concept_id.clone()])?
                ],
            )?;
        let attempt_id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO attempts(
                     id, session_id, concept_id, task_type, prompt_text, response_evidence_id,
                     self_confidence, latency_ms, hint_count, no_attempt_reason, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                           strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
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
                reason.as_str(),
            ],
        )?;
        self.conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
                 VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'no_attempt', ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                input.session_id,
                input.concept_id,
                serde_json::json!({"reason": reason.as_str()}).to_string(),
            ],
        )?;
        Ok(NoAttemptReceipt {
            attempt_id,
            no_attempt_reason: reason,
        })
    }

    pub fn submit_provisional(&mut self, input: SubmitInput) -> Result<SubmitReceipt> {
        let receipt = self.record_provisional_submission(&input)?;
        let grade = grade_with_config(
            &self.conn,
            GradeRequest {
                attempt_id: receipt.attempt_id.clone(),
                self_confidence: input.self_confidence,
                response_text: input.response_text,
            },
            LlmConfig::Unavailable,
        )?;

        Ok(SubmitReceipt {
            degraded: grade.degraded,
            ..receipt
        })
    }

    pub fn submit(&mut self, input: SubmitInput) -> Result<SubmitReceipt> {
        let receipt = self.record_provisional_submission(&input)?;
        let grade = grade_with_config(
            &self.conn,
            GradeRequest {
                attempt_id: receipt.attempt_id.clone(),
                self_confidence: input.self_confidence,
                response_text: input.response_text,
            },
            LlmConfig::from_env(),
        )?;
        if !grade.degraded {
            self.record_final_mental_state_snapshot_for_attempt(&receipt.attempt_id, grade.score)?;
            update_theta_for_attempt(&self.conn, &receipt.attempt_id)?;
            self.replay_concept_after(&input.concept_id, Some(&receipt.attempt_id))?;
            record_move_effect_for_attempt(&self.conn, &receipt.attempt_id, grade.score)?;
            let _ = self.run_gu_induction()?;
        }

        Ok(SubmitReceipt {
            degraded: grade.degraded,
            ..receipt
        })
    }

    fn record_provisional_submission(&mut self, input: &SubmitInput) -> Result<SubmitReceipt> {
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
                input.session_id.as_str(),
                input.response_text.as_str(),
                serde_json::to_string(&vec![input.concept_id.clone()])?
            ],
        )?;

        let attempt_id = Uuid::new_v4().to_string();
        let provisional_score = heuristic_score_with_conn(&self.conn, input.self_confidence)?;
        let pre_attempt_p_hat = self.pre_attempt_p_hat(&input.concept_id, &input.task_type)?;
        let mental_state_observation =
            self.mental_state_observation(input, provisional_score, pre_attempt_p_hat)?;
        let fsrs_params = FsrsParams::from_conn(&self.conn)?;
        let target_depth = task_type_target_depth(&input.task_type).unwrap_or("recall");
        self.conn.execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, prompt_text, response_evidence_id,
                                  self_confidence, latency_ms, hint_count, provisional_score, depth, rating, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                attempt_id,
                input.session_id.as_str(),
                input.concept_id.as_str(),
                input.task_type.as_str(),
                input.prompt_text.as_str(),
                evidence_id,
                input.self_confidence,
                input.latency_ms,
                input.hint_count,
                provisional_score,
                target_depth,
                format!("{:?}", Rating::from_score_with_params(provisional_score, &fsrs_params)).to_lowercase()
            ],
        )?;

        self.conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'latency', ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                input.session_id.as_str(),
                input.concept_id.as_str(),
                serde_json::json!({"latency_ms": input.latency_ms, "hint_count": input.hint_count})
                    .to_string()
            ],
        )?;

        self.record_mental_state_snapshot(MentalStateRecord {
            attempt_id: &attempt_id,
            score_source: "provisional",
            session_id: &input.session_id,
            concept_id: &input.concept_id,
            observation: mental_state_observation,
            p_hat: pre_attempt_p_hat,
            prior_posterior: None,
        })?;
        self.replay_concept_after(&input.concept_id, Some(&attempt_id))?;
        Ok(SubmitReceipt {
            attempt_id,
            provisional_score,
            degraded: true,
        })
    }

    pub fn apply_final_score(&mut self, attempt_id: &str, final_score: f64) -> Result<()> {
        let no_attempt_reason: Option<String> = self.conn.query_row(
            "SELECT no_attempt_reason FROM attempts WHERE id=?1",
            [attempt_id],
            |row| row.get(0),
        )?;
        if no_attempt_reason.is_some() {
            return Err(PolarisError::InvalidTaskTurn(
                "an explicit no-attempt row cannot be graded".to_owned(),
            ));
        }
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
        let concept_id =
            self.record_final_mental_state_snapshot_for_attempt(attempt_id, final_score)?;
        update_theta_for_attempt(&self.conn, attempt_id)?;
        self.replay_concept_after(&concept_id, Some(attempt_id))?;
        record_move_effect_for_attempt(&self.conn, attempt_id, final_score)?;
        let _ = self.run_gu_induction()?;
        Ok(())
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
                "SELECT concept_id, p_known, calib_gap, brier_ewma, attempt_count,
                        COALESCE(phase, 'undetermined')
                 FROM mastery_states WHERE concept_id=?1",
                [concept_id],
                |row| {
                    Ok(StoredMasteryState {
                        concept_id: row.get(0)?,
                        p_known: row.get(1)?,
                        calib_gap: row.get(2)?,
                        brier_ewma: row.get(3)?,
                        attempt_count: row.get(4)?,
                        phase: Phase::parse(&row.get::<_, String>(5)?)
                            .unwrap_or(Phase::Undetermined),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn concept_phase(&self, concept_id: &str) -> Result<Phase> {
        let phase = self
            .conn
            .query_row(
                "SELECT COALESCE(phase, 'undetermined')
                 FROM mastery_states WHERE concept_id=?1",
                [concept_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|value| Phase::parse(&value))
            .unwrap_or(Phase::Undetermined);
        Ok(phase)
    }

    fn stored_phase(&self, concept_id: &str) -> Result<Option<Phase>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT phase FROM mastery_states WHERE concept_id=?1",
                [concept_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map(|phase| Phase::parse(&phase).unwrap_or(Phase::Undetermined)))
    }

    fn phase_input(
        &self,
        concept_id: &str,
        state: &MasteryState,
        history: PhaseHistorySummary,
    ) -> Result<PhaseInput> {
        let cut_hi = meta_f64(&self.conn, "bkt.cut_hi")?;
        let transfer_success_count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM attempts
             WHERE concept_id=?1
               AND (depth='transfer' OR task_type IN ('transfer', 'free_produce'))
               AND COALESCE(final_score, provisional_score) >= ?2",
            params![concept_id, cut_hi],
            |row| row.get(0),
        )?;
        let transfer_fail_count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM attempts
             WHERE concept_id=?1
               AND (depth='transfer' OR task_type IN ('transfer', 'free_produce'))
               AND COALESCE(final_score, provisional_score) < 0.5",
            [concept_id],
            |row| row.get(0),
        )?;
        let (original_context_success, novel_context_success, novel_context_fail) =
            self.context_counts(concept_id, cut_hi)?;
        let (relevant_task_attempt_count, median_latency_ratio) =
            self.relevant_task_latency_ratio(concept_id)?;
        let calibration_posterior =
            posterior_from_samples(&calibration_samples(&self.conn, concept_id, 12)?);

        Ok(PhaseInput {
            p_known: state.p_known,
            retrievability: None,
            theta_prediction: None,
            calib_gap: state.calib_gap,
            attempt_count: state.attempt_count,
            lapses: state.lapses,
            recent_lapses: history.recent_lapses,
            max_depth: state.max_depth.as_deref().and_then(Depth::parse),
            has_transfer_success: transfer_success_count > 0,
            ever_reached_transfer_or_generation: history.ever_reached_transfer_or_generation,
            relevant_task_attempt_count,
            original_context_success,
            transfer_fail_count: transfer_fail_count.max(0) as u32,
            novel_context_success,
            novel_context_fail,
            calibration_overestimates: calibration_posterior.overestimates,
            calibration_sample_count: calibration_posterior.total,
            calibration_probability_over_half: calibration_posterior.probability_over_half,
            median_latency_ratio,
        })
    }

    fn context_counts(&self, concept_id: &str, cut_hi: f64) -> Result<(u32, u32, u32)> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(final_score, provisional_score), grader_json
             FROM attempts
             WHERE concept_id=?1
               AND COALESCE(final_score, provisional_score) IS NOT NULL",
        )?;
        let rows = stmt.query_map([concept_id], |row| {
            Ok((row.get::<_, f64>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        let mut original_success = 0_u32;
        let mut success = 0_u32;
        let mut fail = 0_u32;
        for row in rows {
            let (score, grader_json) = row?;
            let context_novel = grader_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                .and_then(|value| {
                    value
                        .get("context_novel")
                        .and_then(serde_json::Value::as_bool)
                })
                .unwrap_or(false);
            if !context_novel {
                if score >= cut_hi {
                    original_success += 1;
                }
                continue;
            }
            if score >= cut_hi {
                success += 1;
            } else if score < 0.5 {
                fail += 1;
            }
        }
        Ok((original_success, success, fail))
    }

    fn relevant_task_latency_ratio(&self, concept_id: &str) -> Result<(u32, Option<f64>)> {
        let relevant_latencies = self.relevant_task_latencies(concept_id)?;
        if relevant_latencies.len() < 3 {
            return Ok((relevant_latencies.len() as u32, None));
        }
        let global_latencies = self.all_latencies()?;
        let Some(global_p25) = percentile(&global_latencies, 0.25) else {
            return Ok((relevant_latencies.len() as u32, None));
        };
        if global_p25 <= 0.0 {
            return Ok((relevant_latencies.len() as u32, None));
        }
        Ok((
            relevant_latencies.len() as u32,
            median(&relevant_latencies).map(|relevant_median| relevant_median / global_p25),
        ))
    }

    fn relevant_task_latencies(&self, concept_id: &str) -> Result<Vec<f64>> {
        let mut stmt = self.conn.prepare(
            "SELECT CAST(latency_ms AS REAL)
             FROM attempts
             WHERE concept_id=?1
               AND latency_ms IS NOT NULL
               AND no_attempt_reason IS NULL
               AND (depth='transfer' OR task_type IN ('transfer', 'free_produce'))
             ORDER BY latency_ms ASC",
        )?;
        let latencies = stmt
            .query_map([concept_id], |row| row.get::<_, f64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(latencies)
    }

    fn phase_history_summary(
        &self,
        p_init: f64,
        attempts: &[AttemptObservation],
        params: &MasteryParams,
    ) -> Result<PhaseHistorySummary> {
        let cut_hi = meta_f64(&self.conn, "bkt.cut_hi")?;
        let cut_lo = meta_f64(&self.conn, "bkt.cut_lo")?;
        let mut state = MasteryState::initial_with_params(p_init, params);
        let mut has_transfer_success = false;
        let mut ever_reached_transfer_or_generation = false;
        let mut recent_lapses = 0_u32;

        for attempt in attempts {
            fold_attempt(&mut state, attempt, params);
            if is_transfer_attempt(attempt) && attempt.score >= cut_hi {
                has_transfer_success = true;
            }
            if ever_reached_transfer_or_generation {
                if attempt.score < cut_lo {
                    recent_lapses += 1;
                } else {
                    recent_lapses = 0;
                }
            }
            if state.p_known >= 0.7 && has_transfer_success {
                ever_reached_transfer_or_generation = true;
                if attempt.score >= cut_lo {
                    recent_lapses = 0;
                }
            }
        }

        Ok(PhaseHistorySummary {
            ever_reached_transfer_or_generation,
            recent_lapses,
        })
    }

    fn all_latencies(&self) -> Result<Vec<f64>> {
        let mut stmt = self.conn.prepare(
            "SELECT CAST(latency_ms AS REAL)
             FROM attempts
             WHERE latency_ms IS NOT NULL AND no_attempt_reason IS NULL
             ORDER BY latency_ms ASC",
        )?;
        let latencies = stmt
            .query_map([], |row| row.get::<_, f64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(latencies)
    }

    fn record_phase_transition(
        &self,
        concept_id: &str,
        attempt_id: Option<&str>,
        from: Phase,
        to: Phase,
    ) -> Result<()> {
        let Some(attempt_id) = attempt_id else {
            return Ok(());
        };
        let session_id = self
            .conn
            .query_row(
                "SELECT COALESCE(session_id, 'default') FROM attempts WHERE id=?1",
                [attempt_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| "default".to_owned());
        let payload = serde_json::json!({
            "schema_version": 1,
            "from": from.as_str(),
            "to": to.as_str(),
            "concept_id": concept_id,
            "attempt_id": attempt_id
        })
        .to_string();
        self.conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'phase_transition', ?3, ?4)",
            params![Uuid::new_v4().to_string(), session_id, concept_id, payload],
        )?;
        Ok(())
    }

    pub(crate) fn replay_concept_after(
        &self,
        concept_id: &str,
        trigger_attempt_id: Option<&str>,
    ) -> Result<()> {
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
        drop(stmt);

        let params = MasteryParams::from_conn(&self.conn)?;
        let state = fold_all(p_init, &attempts, &params);
        let previous_phase = self.stored_phase(concept_id)?;
        let phase_params = PhaseParams::from_conn(&self.conn)?;
        let phase_history = self.phase_history_summary(p_init, &attempts, &params)?;
        let phase = determine_phase(
            &self.phase_input(concept_id, &state, phase_history)?,
            &phase_params,
        );
        let fsrs_json = serde_json::to_string(&state.fsrs)?;
        let last_review_at = attempts.last().map(|attempt| attempt.created_at.as_str());
        let next_due_at = match (last_review_at, state.fsrs.scheduled_days) {
            (Some(last_review_at), Some(days)) => Some(self.next_due_at(last_review_at, days)?),
            _ => None,
        };
        self.conn.execute(
            "INSERT INTO mastery_states(concept_id, p_known, fsrs_json, next_due_at, last_review_at,
                                        calib_gap, brier_ewma, last_depth, max_depth,
                                        phase, attempt_count, lapses, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
             ON CONFLICT(concept_id) DO UPDATE SET
                p_known=excluded.p_known,
                fsrs_json=excluded.fsrs_json,
                next_due_at=excluded.next_due_at,
                last_review_at=excluded.last_review_at,
                calib_gap=excluded.calib_gap,
                brier_ewma=excluded.brier_ewma,
                last_depth=excluded.last_depth,
                max_depth=excluded.max_depth,
                phase=excluded.phase,
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
                phase.as_str(),
                i64::from(state.attempt_count),
                i64::from(state.lapses),
            ],
        )?;
        if let Some(previous_phase) = previous_phase {
            if previous_phase != phase {
                self.record_phase_transition(
                    concept_id,
                    trigger_attempt_id
                        .or_else(|| attempts.last().map(|attempt| attempt.id.as_str())),
                    previous_phase,
                    phase,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn scored_concept_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT concept_id
             FROM attempts
             WHERE COALESCE(final_score, provisional_score) IS NOT NULL
             ORDER BY concept_id ASC",
        )?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
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
            let concept_id =
                self.record_final_mental_state_snapshot_for_attempt(&attempt_id, result.score)?;
            update_theta_for_attempt(&self.conn, &attempt_id)?;
            self.replay_concept_after(&concept_id, Some(&attempt_id))?;
            record_move_effect_for_attempt(&self.conn, &attempt_id, result.score)?;
            let _ = self.run_gu_induction()?;
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
}

fn is_transfer_attempt(attempt: &AttemptObservation) -> bool {
    attempt.depth.as_deref() == Some("transfer")
        || matches!(attempt.task_type.as_str(), "transfer" | "free_produce")
}

fn median(sorted: &[f64]) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        Some((sorted[mid - 1] + sorted[mid]) / 2.0)
    } else {
        Some(sorted[mid])
    }
}

fn percentile(sorted: &[f64], quantile: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let clamped = quantile.clamp(0.0, 1.0);
    let index = ((sorted.len() - 1) as f64 * clamped).floor() as usize;
    Some(sorted[index])
}
