use super::*;

impl Engine {
    pub(crate) fn pre_attempt_p_hat(&self, concept_id: &str, task_type: &str) -> Result<f64> {
        self.fused_p_known(concept_id, task_type)
            .map(|prediction| prediction.p_known)
    }

    pub(crate) fn mental_state_observation(
        &self,
        input: &SubmitInput,
        score: f64,
        p_hat: f64,
    ) -> Result<HmmObservation> {
        let confidence = confidence_unit(input.self_confidence);
        let mean_confidence = self.mean_confidence()?.unwrap_or(confidence);
        Ok(HmmObservation {
            z_latency: self.latency_z(&input.task_type, input.latency_ms)?,
            hints: input.hint_count.clamp(0, 3) as f64,
            residual: score.clamp(0.0, 1.0) - p_hat,
            consec_fail: self.consecutive_failures(&input.session_id)? as f64,
            conf_delta: confidence - mean_confidence,
            interval_bucket: self.interval_bucket(&input.session_id)?,
            session_min: self.session_minutes(&input.session_id)?,
        })
    }

    fn final_mental_state_observation(
        &self,
        attempt_id: &str,
        final_score: f64,
    ) -> Result<(HmmObservation, f64, StatePosterior)> {
        if let Some(payload) = self.mental_state_payload_for_attempt(attempt_id, "provisional")? {
            if let Some((mut observation, p_hat, prior)) = mental_state_payload_features(&payload)?
            {
                observation.residual = final_score.clamp(0.0, 1.0) - p_hat;
                return Ok((observation, p_hat, prior));
            }
        }

        let (session_id, concept_id, task_type, self_confidence, latency_ms, hint_count): (
            String,
            String,
            String,
            i32,
            i64,
            i64,
        ) = self
            .conn
            .query_row(
                "SELECT COALESCE(session_id, 'default'), concept_id, COALESCE(task_type, 'recall'),
                        COALESCE(self_confidence, 3), COALESCE(latency_ms, 0), COALESCE(hint_count, 0)
                 FROM attempts WHERE id=?1",
                [attempt_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))?;
        let p_hat = self.pre_attempt_p_hat(&concept_id, &task_type)?;
        let input = SubmitInput {
            session_id,
            concept_id,
            task_type,
            prompt_text: String::new(),
            response_text: String::new(),
            self_confidence,
            latency_ms,
            hint_count,
        };
        Ok((
            self.mental_state_observation(&input, final_score, p_hat)?,
            p_hat,
            StatePosterior::uniform(),
        ))
    }

    pub(crate) fn record_mental_state_snapshot(&self, record: MentalStateRecord<'_>) -> Result<()> {
        let observation = record.observation;
        let prior = match record.prior_posterior {
            Some(prior) => prior,
            None => self
                .latest_mental_state_posterior(record.session_id)?
                .unwrap_or_else(StatePosterior::uniform),
        };
        let transitions = transitions_from_meta(&self.conn)?.unwrap_or_else(prior_transitions);
        let posterior = forward_filter_with_transitions(Some(&prior), observation, &transitions);
        let (time_sin, time_cos) = self.time_of_day_features()?;
        let hazard_gate = meta_f64(&self.conn, "hazard.auc_gate")?;
        let hmm_gate_margin = meta_f64(&self.conn, "hmm.gate_auc_margin")?;
        let calib_gap = self.current_calib_gap(record.concept_id)?;
        let hint_rate = observation.hints / observation.session_min.max(1.0);
        let fitted_model = self.latest_hazard_model()?;
        let (beta, validation_auc, model_status) = match &fitted_model {
            Some((beta, auc)) => (*beta, Some(*auc), "fitted"),
            None => ([0.0; HAZARD_FEATURE_COUNT], None, "unfit"),
        };
        let gate_eval = self.latest_state_gate_eval()?;
        let (observed_auc_margin, strategy_enabled) = match gate_eval {
            Some((margin, passes)) => (Some(margin), passes),
            None => (None, false),
        };
        let hazard_inputs = HazardInputs::new(
            &posterior,
            calib_gap,
            observation.consec_fail,
            hint_rate,
            time_sin,
            time_cos,
            observation.session_min,
        );
        let hazard = estimate_hazard(hazard_inputs, &beta, validation_auc, hazard_gate);
        let payload = serde_json::json!({
            "schema_version": 1,
            "attempt_id": record.attempt_id,
            "score_source": record.score_source,
            "features": {
                "z_latency": observation.z_latency,
                "hints": observation.hints,
                "residual": observation.residual,
                "p_hat": record.p_hat,
                "consec_fail": observation.consec_fail,
                "conf_delta": observation.conf_delta,
                "interval_bucket": observation.interval_bucket,
                "session_min": observation.session_min
            },
            "prior_posterior": prior.probabilities,
            "posterior": posterior.probabilities,
            "dominant_state": posterior.dominant_state().as_str(),
            "strategy_enabled": strategy_enabled,
            "state_gate": {
                "required_auc_margin": hmm_gate_margin,
                "observed_auc_margin": observed_auc_margin,
                "participates": strategy_enabled
            },
            "hazard": {
                "probability": hazard.probability,
                "participates": hazard.participates,
                "validation_auc": hazard.validation_auc,
                "auc_gate": hazard.auc_gate,
                "model_status": model_status,
                "inputs": hazard_inputs.features,
                "calib_gap": calib_gap,
                "hint_rate": hint_rate,
                "time_sin": time_sin,
                "time_cos": time_cos
            }
        })
        .to_string();

        self.conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'mental_state', ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                record.session_id,
                record.concept_id,
                payload
            ],
        )?;
        Ok(())
    }

    fn latest_hazard_model(&self) -> Result<Option<([f64; HAZARD_FEATURE_COUNT], f64)>> {
        let row: Option<(String, f64)> = self
            .conn
            .query_row(
                "SELECT beta_json, validation_auc FROM hazard_models
                 ORDER BY julianday(fitted_at) DESC, id DESC
                 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((beta_json, auc)) = row else {
            return Ok(None);
        };
        let values: Vec<f64> = serde_json::from_str(&beta_json)?;
        if values.len() != HAZARD_FEATURE_COUNT {
            return Ok(None);
        }
        let mut beta = [0.0; HAZARD_FEATURE_COUNT];
        beta.copy_from_slice(&values);
        Ok(Some((beta, auc)))
    }

    fn latest_state_gate_eval(&self) -> Result<Option<(f64, bool)>> {
        let row: Option<(f64, i64)> = self
            .conn
            .query_row(
                "SELECT margin, passes FROM state_gate_evals
                 ORDER BY julianday(evaluated_at) DESC, id DESC
                 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(margin, passes)| (margin, passes != 0)))
    }

    fn latest_mental_state_posterior(&self, session_id: &str) -> Result<Option<StatePosterior>> {
        let payload: Option<String> = self
            .conn
            .query_row(
                "SELECT payload_json FROM behavior_events
                 WHERE session_id=?1 AND type='mental_state'
                   AND json_extract(payload_json, '$.score_source')='provisional'
                 ORDER BY julianday(at) DESC, rowid DESC
                 LIMIT 1",
                [session_id],
                |row| row.get(0),
            )
            .optional()?;
        payload.as_deref().map(posterior_from_payload).transpose()
    }

    fn mental_state_payload_for_attempt(
        &self,
        attempt_id: &str,
        score_source: &str,
    ) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT payload_json FROM behavior_events
                 WHERE type='mental_state'
                   AND json_extract(payload_json, '$.attempt_id')=?1
                   AND json_extract(payload_json, '$.score_source')=?2
                 ORDER BY julianday(at) DESC, rowid DESC
                 LIMIT 1",
                (attempt_id, score_source),
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn current_calib_gap(&self, concept_id: &str) -> Result<f64> {
        Ok(self
            .conn
            .query_row(
                "SELECT COALESCE(calib_gap, 0.0) FROM mastery_states WHERE concept_id=?1",
                [concept_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0.0))
    }

    fn mean_confidence(&self) -> Result<Option<f64>> {
        self.conn
            .query_row(
                "SELECT AVG((self_confidence - 1.0) / 4.0)
                 FROM attempts WHERE self_confidence IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn latency_z(&self, task_type: &str, latency_ms: i64) -> Result<f64> {
        let (count, mean, mean_square): (i64, Option<f64>, Option<f64>) = self.conn.query_row(
            "SELECT COUNT(*), AVG(CAST(latency_ms AS REAL)), AVG(CAST(latency_ms AS REAL) * CAST(latency_ms AS REAL))
             FROM attempts
             WHERE task_type=?1 AND latency_ms IS NOT NULL",
            [task_type],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if count < 2 {
            return Ok(0.0);
        }
        let mean = mean.unwrap_or(0.0);
        let variance = (mean_square.unwrap_or(0.0) - mean * mean).max(0.0);
        let stddev = variance.sqrt();
        if stddev < 1.0 {
            Ok(0.0)
        } else {
            Ok((latency_ms as f64 - mean) / stddev)
        }
    }

    pub(crate) fn record_final_mental_state_snapshot_for_attempt(
        &self,
        attempt_id: &str,
        final_score: f64,
    ) -> Result<String> {
        let (concept_id, session_id): (String, String) = self
            .conn
            .query_row(
                "SELECT concept_id, COALESCE(session_id, 'default') FROM attempts WHERE id=?1",
                [attempt_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| PolarisError::MissingAttempt(attempt_id.to_owned()))?;
        let (observation, pre_attempt_p_hat, prior_posterior) =
            self.final_mental_state_observation(attempt_id, final_score)?;
        self.record_mental_state_snapshot(MentalStateRecord {
            attempt_id,
            score_source: "final",
            session_id: &session_id,
            concept_id: &concept_id,
            observation,
            p_hat: pre_attempt_p_hat,
            prior_posterior: Some(prior_posterior),
        })?;
        Ok(concept_id)
    }

    fn consecutive_failures(&self, session_id: &str) -> Result<i64> {
        let cut_lo = meta_f64(&self.conn, "bkt.cut_lo")?;
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(final_score, provisional_score)
             FROM attempts
             WHERE session_id=?1 AND COALESCE(final_score, provisional_score) IS NOT NULL
             ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) DESC, id DESC
             LIMIT 50",
        )?;
        let scores = stmt
            .query_map([session_id], |row| row.get::<_, f64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut count = 0;
        for score in scores {
            if score <= cut_lo {
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    fn interval_bucket(&self, session_id: &str) -> Result<f64> {
        let minutes: Option<f64> = self
            .conn
            .query_row(
                "SELECT MAX(0.0, (julianday('now') - julianday(created_at)) * 1440.0)
                 FROM attempts
                 WHERE session_id=?1 AND created_at IS NOT NULL
                 ORDER BY julianday(created_at) DESC, id DESC
                 LIMIT 1",
                [session_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let Some(minutes) = minutes else {
            return Ok(0.0);
        };
        Ok(if minutes < 1.0 {
            0.0
        } else if minutes < 10.0 {
            1.0
        } else {
            2.0
        })
    }

    fn session_minutes(&self, session_id: &str) -> Result<f64> {
        Ok(self
            .conn
            .query_row(
                "SELECT MAX(0.0, (julianday('now') - julianday(started_at)) * 1440.0)
                 FROM sessions WHERE id=?1",
                [session_id],
                |row| row.get::<_, Option<f64>>(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0.0))
    }

    fn time_of_day_features(&self) -> Result<(f64, f64)> {
        let minute_of_day: f64 = self.conn.query_row(
            "SELECT CAST(strftime('%H','now') AS REAL) * 60.0 + CAST(strftime('%M','now') AS REAL)",
            [],
            |row| row.get(0),
        )?;
        let angle = minute_of_day / 1440.0 * std::f64::consts::TAU;
        Ok((angle.sin(), angle.cos()))
    }
}

fn confidence_unit(confidence: i32) -> f64 {
    ((confidence as f64 - 1.0) / 4.0).clamp(0.0, 1.0)
}

fn posterior_from_payload(payload: &str) -> Result<StatePosterior> {
    let value: serde_json::Value = serde_json::from_str(payload)?;
    posterior_from_value(&value["posterior"]).ok_or_else(|| PolarisError::InvalidParameter {
        key: "mental_state.posterior".to_owned(),
        value: payload.to_owned(),
    })
}

fn mental_state_payload_features(
    payload: &str,
) -> Result<Option<(HmmObservation, f64, StatePosterior)>> {
    let value: serde_json::Value = serde_json::from_str(payload)?;
    let features = &value["features"];
    let Some(p_hat) = json_f64(features, "p_hat") else {
        return Ok(None);
    };
    let Some(prior) = posterior_from_value(&value["prior_posterior"]) else {
        return Ok(None);
    };
    let observation = HmmObservation {
        z_latency: json_f64(features, "z_latency").unwrap_or(0.0),
        hints: json_f64(features, "hints").unwrap_or(0.0),
        residual: json_f64(features, "residual").unwrap_or(0.0),
        consec_fail: json_f64(features, "consec_fail").unwrap_or(0.0),
        conf_delta: json_f64(features, "conf_delta").unwrap_or(0.0),
        interval_bucket: json_f64(features, "interval_bucket").unwrap_or(0.0),
        session_min: json_f64(features, "session_min").unwrap_or(0.0),
    };
    Ok(Some((observation, p_hat, prior)))
}

fn posterior_from_value(value: &serde_json::Value) -> Option<StatePosterior> {
    let values = value.as_array()?;
    if values.len() != STATE_COUNT {
        return None;
    }
    let mut probabilities = [0.0; STATE_COUNT];
    for (idx, value) in values.iter().enumerate() {
        let probability = value.as_f64()?;
        if !probability.is_finite() || probability < 0.0 {
            return None;
        }
        probabilities[idx] = probability;
    }
    let total = probabilities.iter().sum::<f64>();
    if total <= 0.0 {
        return None;
    }
    for probability in &mut probabilities {
        *probability /= total;
    }
    Some(StatePosterior { probabilities })
}

fn json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(serde_json::Value::as_f64)
}
