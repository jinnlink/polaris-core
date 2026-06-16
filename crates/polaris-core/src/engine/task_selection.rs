use super::*;

impl Engine {
    pub fn next_task(&self) -> Result<Option<NextTask>> {
        let ranked = self.ranked_task_candidates()?;
        let Some(best) = ranked.first() else {
            return Ok(None);
        };

        let hmm_strategy = self.batch_strategy()?;
        let base_move = select_next_move_for_concept(&self.conn, &best.id)?;
        let (planned_move, phase_strategy) = phase_adjusted_move(
            base_move,
            best.phase,
            phase_adjustment_protected(hmm_strategy),
        );
        let selection = select_move_for_concept(
            &self.conn,
            &best.id,
            &best.name,
            planned_move,
            best.p_known,
            best.phase,
            phase_strategy,
        )?;
        let selected_move = selection.selected_move;
        let template = move_template_for_concept(&self.conn, &best.id, selected_move.id)?
            .map(|item| item.template)
            .unwrap_or_else(|| fallback_template(selected_move.id).to_owned());
        let prompt_text = render_move_prompt(&template, &best.name);
        Ok(Some(NextTask {
            concept_id: best.id.clone(),
            move_id: selected_move.id.to_owned(),
            task_type: selected_move.task_type.to_owned(),
            prompt_text,
            reason: next_task_reason(best, selected_move, phase_strategy),
            mrt_context_hash: selection.context_hash,
            mrt_prereg_id: selection.prereg_id,
            mrt_randomized: selection.randomized,
        }))
    }

    pub fn record_next_task_event(&self, session_id: &str, task: &NextTask) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions(id, started_at, context_json)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), '{}')
             ON CONFLICT(id) DO NOTHING",
            [session_id],
        )?;
        let payload = serde_json::json!({
            "task_type": &task.task_type,
            "prompt": &task.prompt_text,
            "reason": &task.reason,
            "move": &task.move_id,
            "mrt_context_hash": &task.mrt_context_hash,
            "mrt_prereg_id": &task.mrt_prereg_id,
            "mrt_randomized": task.mrt_randomized,
        })
        .to_string();
        self.conn.execute(
            "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
             VALUES (lower(hex(randomblob(16))), ?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'next', ?2, ?3)",
            (session_id, task.concept_id.as_str(), payload.as_str()),
        )?;
        Ok(())
    }

    pub fn get_interleaved_batch(&self, batch_size: usize) -> Result<Vec<TaskAssignment>> {
        let batch_size = batch_size.max(1);
        let ranked = self.ranked_task_candidates()?;
        if ranked.len() < 3 || batch_size < 3 {
            return self.single_next_task_assignment();
        }

        let hmm_strategy = self.batch_strategy()?;
        let strategy = if matches!(hmm_strategy, BatchStrategy::Default) {
            phase_batch_strategy(&ranked).unwrap_or(hmm_strategy)
        } else {
            hmm_strategy
        };
        let mut selected = Vec::new();
        match strategy {
            BatchStrategy::Default => {
                self.push_first_matching(&mut selected, &ranked, is_new_or_weak);
                self.push_review_pair(&mut selected, &ranked, |candidate| {
                    candidate.p_known >= 0.6
                })?;
            }
            BatchStrategy::EasyReviews => {
                self.push_review_pair(&mut selected, &ranked, |candidate| {
                    candidate.p_known >= 0.8
                })?;
                while selected.len() < batch_size {
                    if !self.push_first_matching(&mut selected, &ranked, |candidate| {
                        candidate.p_known >= 0.8
                    }) {
                        break;
                    }
                    if selected.len() >= ranked.len() {
                        break;
                    }
                }
            }
            BatchStrategy::Flow => {
                self.push_first_matching(&mut selected, &ranked, is_new_or_weak);
                self.push_first_matching(&mut selected, &ranked, is_new_or_weak);
                self.push_first_matching(&mut selected, &ranked, |candidate| {
                    candidate.p_known >= 0.6
                });
            }
            BatchStrategy::PhantomChallenge
            | BatchStrategy::SettlingProbe
            | BatchStrategy::RegressionRecovery => {
                if let Some(phase) = strategy_target_phase(strategy) {
                    self.push_first_matching(&mut selected, &ranked, |candidate| {
                        candidate.phase == phase
                    });
                }
                self.push_review_pair(&mut selected, &ranked, |candidate| {
                    candidate.p_known >= 0.6
                })?;
            }
        }

        if !matches!(strategy, BatchStrategy::EasyReviews) {
            while selected.len() < batch_size.min(ranked.len()) {
                if !self.push_first_matching(&mut selected, &ranked, |_| true) {
                    break;
                }
            }
        }

        selected.truncate(batch_size);
        self.adjust_expected_success(&mut selected, &ranked, strategy)?;
        if is_phase_action_strategy(strategy) {
            self.shrink_out_of_band_batch(&mut selected, strategy)?;
        }
        selected
            .iter()
            .map(|candidate| self.assignment_for_candidate(candidate, strategy))
            .collect()
    }

    fn adjust_expected_success(
        &self,
        selected: &mut [RankedTaskCandidate],
        ranked: &[RankedTaskCandidate],
        strategy: BatchStrategy,
    ) -> Result<()> {
        if selected.len() < 3 {
            return Ok(());
        }
        let mut current = self.assignment_mean(selected, strategy)?;
        if target_distance(current) == 0.0 {
            return Ok(());
        }

        for _ in 0..selected.len() {
            let mut best: Option<(usize, RankedTaskCandidate, f64)> = None;
            for index in 0..selected.len() {
                for candidate in ranked {
                    if selected
                        .iter()
                        .enumerate()
                        .any(|(idx, item)| idx != index && item.id == candidate.id)
                    {
                        continue;
                    }
                    if !role_allows_candidate(strategy, index, candidate) {
                        continue;
                    }
                    let mut proposal = selected.to_vec();
                    proposal[index] = candidate.clone();
                    if !self.review_slots_are_diverse(strategy, &proposal)? {
                        continue;
                    }
                    let mean = self.assignment_mean(&proposal, strategy)?;
                    if target_distance(mean) < target_distance(current) {
                        match &best {
                            Some((_, _, best_mean))
                                if target_distance(*best_mean) <= target_distance(mean) => {}
                            _ => best = Some((index, candidate.clone(), mean)),
                        }
                    }
                }
            }

            let Some((index, candidate, mean)) = best else {
                return Ok(());
            };
            selected[index] = candidate;
            current = mean;
            if target_distance(current) == 0.0 {
                return Ok(());
            }
        }
        Ok(())
    }

    fn shrink_out_of_band_batch(
        &self,
        selected: &mut Vec<RankedTaskCandidate>,
        strategy: BatchStrategy,
    ) -> Result<()> {
        while selected.len() >= 3 {
            let mean = self.assignment_mean(selected, strategy)?;
            if target_distance(mean) == 0.0 {
                return Ok(());
            }
            selected.pop();
        }
        Ok(())
    }

    fn assignment_mean(
        &self,
        candidates: &[RankedTaskCandidate],
        strategy: BatchStrategy,
    ) -> Result<f64> {
        let mut total = 0.0;
        for candidate in candidates {
            total += self.expected_success_for_candidate(candidate, strategy)?;
        }
        Ok(total / candidates.len() as f64)
    }

    fn expected_success_for_candidate(
        &self,
        candidate: &RankedTaskCandidate,
        strategy: BatchStrategy,
    ) -> Result<f64> {
        let base_move = select_next_move_for_concept(&self.conn, &candidate.id)?;
        let mut selected_move = phase_adjusted_move(
            base_move,
            candidate.phase,
            phase_adjustment_protected(strategy),
        )
        .0;
        let mut expected =
            self.expected_success(&candidate.id, selected_move.task_type, candidate.p_known)?;
        if is_new_or_weak(candidate) && expected < 0.50 {
            selected_move = easier_move(selected_move);
            selected_move = phase_adjusted_move(
                selected_move,
                candidate.phase,
                phase_adjustment_protected(strategy),
            )
            .0;
            expected =
                self.expected_success(&candidate.id, selected_move.task_type, candidate.p_known)?;
        }
        Ok(expected)
    }

    fn review_slots_are_diverse(
        &self,
        strategy: BatchStrategy,
        selected: &[RankedTaskCandidate],
    ) -> Result<bool> {
        let review_indexes: &[usize] = match strategy {
            BatchStrategy::Default => &[1, 2],
            BatchStrategy::EasyReviews => &[0, 1],
            BatchStrategy::Flow => &[],
            BatchStrategy::PhantomChallenge
            | BatchStrategy::SettlingProbe
            | BatchStrategy::RegressionRecovery => &[1, 2],
        };
        if review_indexes
            .iter()
            .any(|index| selected.get(*index).is_none())
        {
            return Ok(true);
        }
        if let [left_index, right_index] = review_indexes {
            let left = self.neighborhood_fingerprint(&selected[*left_index].id)?;
            let right = self.neighborhood_fingerprint(&selected[*right_index].id)?;
            return Ok(neighborhood_overlap(&left, &right) <= 0.50);
        }
        Ok(true)
    }

    fn ranked_task_candidates(&self) -> Result<Vec<RankedTaskCandidate>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.seed_order,
                    COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)) AS p_known,
                    ms.fsrs_json, COALESCE(ms.calib_gap, 0.0),
                    COALESCE((SELECT COUNT(*) FROM attempts a WHERE a.concept_id=c.id), 0),
                    CASE
                        WHEN ms.last_review_at IS NULL THEN NULL
                        ELSE julianday('now') - julianday(ms.last_review_at)
                    END,
                    COALESCE(ms.phase, 'undetermined')
             FROM concepts c
             LEFT JOIN mastery_states ms ON ms.concept_id=c.id
             ORDER BY c.seed_order ASC, c.id ASC",
        )?;

        let mut rows = stmt.query([])?;
        let mut schedule_candidates = Vec::new();
        let mut details = BTreeMap::new();
        while let Some(row) = rows.next()? {
            let concept_id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let seed_order: i64 = row.get(2)?;
            let p_known: f64 = row.get(3)?;
            let fsrs_json: Option<String> = row.get(4)?;
            let calib_gap: f64 = row.get(5)?;
            let attempt_count: i64 = row.get(6)?;
            let elapsed_days: Option<f64> = row.get(7)?;
            let phase = Phase::parse(&row.get::<_, String>(8)?).unwrap_or(Phase::Undetermined);
            let retrieval = fsrs_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<FsrsState>(json).ok())
                .map(|state| retrievability(state.stability, elapsed_days.unwrap_or(0.0).max(0.0)));

            schedule_candidates.push(ScheduleCandidate {
                id: concept_id.clone(),
                seed_order,
                retrieval,
                calib_gap,
                misconception_active: self.misconception_active(&concept_id)?,
                has_attempts: attempt_count > 0,
                prerequisites_met: self.prerequisites_met(&concept_id)?,
                phase,
            });
            details.insert(concept_id, (name, p_known, attempt_count > 0, phase));
        }

        let scheduler_params = SchedulerParams::from_conn(&self.conn)?;
        let ranked = rank_candidates_with_params(schedule_candidates, &scheduler_params);
        Ok(ranked
            .into_iter()
            .filter_map(|item| match details.get(&item.id) {
                Some((name, p_known, has_attempts, phase)) => Some(RankedTaskCandidate {
                    id: item.id,
                    name: name.clone(),
                    utility: item.utility,
                    p_known: *p_known,
                    has_attempts: *has_attempts,
                    phase: *phase,
                }),
                None => None,
            })
            .collect())
    }

    fn single_next_task_assignment(&self) -> Result<Vec<TaskAssignment>> {
        let Some(next) = self.next_task()? else {
            return Ok(Vec::new());
        };
        let (concept_name, p_known, phase): (String, f64, String) = self.conn.query_row(
            "SELECT c.name,
                    COALESCE(ms.p_known, c.p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                    COALESCE(ms.phase, 'undetermined')
             FROM concepts c
             LEFT JOIN mastery_states ms ON ms.concept_id=c.id
             WHERE c.id=?1",
            [&next.concept_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let move_name = task_type_target_depth(&next.task_type).unwrap_or("recall");
        Ok(vec![TaskAssignment {
            expected_success: self.expected_success(&next.concept_id, &next.task_type, p_known)?,
            concept_id: next.concept_id,
            concept_name,
            move_name: move_name.to_owned(),
            task_type: next.task_type,
            template: next.prompt_text,
            phase: Phase::parse(&phase).unwrap_or(Phase::Undetermined),
            p_known,
        }])
    }

    fn assignment_for_candidate(
        &self,
        candidate: &RankedTaskCandidate,
        strategy: BatchStrategy,
    ) -> Result<TaskAssignment> {
        let base_move = select_next_move_for_concept(&self.conn, &candidate.id)?;
        let mut selected_move = phase_adjusted_move(
            base_move,
            candidate.phase,
            phase_adjustment_protected(strategy),
        )
        .0;
        let mut expected =
            self.expected_success(&candidate.id, selected_move.task_type, candidate.p_known)?;
        if is_new_or_weak(candidate) && expected < 0.50 {
            selected_move = easier_move(selected_move);
            selected_move = phase_adjusted_move(
                selected_move,
                candidate.phase,
                phase_adjustment_protected(strategy),
            )
            .0;
            expected =
                self.expected_success(&candidate.id, selected_move.task_type, candidate.p_known)?;
        }

        let template = move_template_for_concept(&self.conn, &candidate.id, selected_move.id)?
            .map(|item| item.template)
            .unwrap_or_else(|| fallback_template(selected_move.id).to_owned());
        Ok(TaskAssignment {
            concept_id: candidate.id.clone(),
            concept_name: candidate.name.clone(),
            move_name: selected_move.id.to_owned(),
            task_type: selected_move.task_type.to_owned(),
            template: render_move_prompt(&template, &candidate.name),
            phase: candidate.phase,
            p_known: candidate.p_known,
            expected_success: expected,
        })
    }

    fn expected_success(&self, concept_id: &str, task_type: &str, fallback: f64) -> Result<f64> {
        match self.fused_p_known(concept_id, task_type) {
            Ok(prediction) => Ok(prediction.p_known.clamp(0.0, 1.0)),
            Err(_) => Ok(fallback.clamp(0.0, 1.0)),
        }
    }

    fn batch_strategy(&self) -> Result<BatchStrategy> {
        let payload: Option<String> = self
            .conn
            .query_row(
                "SELECT payload_json
                 FROM behavior_events
                 WHERE type='mental_state'
                 ORDER BY at DESC, id DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(payload) = payload else {
            return Ok(BatchStrategy::Default);
        };
        let value: serde_json::Value = serde_json::from_str(&payload)?;
        if value
            .get("strategy_enabled")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Ok(BatchStrategy::Default);
        }
        let dominant = value
            .get("dominant_state")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| dominant_state_from_posterior(&value["posterior"]));
        Ok(match dominant.as_deref() {
            Some("flow") => BatchStrategy::Flow,
            Some("fatigued" | "fatigue" | "bored" | "disengagement" | "disengaged") => {
                BatchStrategy::EasyReviews
            }
            _ => BatchStrategy::Default,
        })
    }

    fn push_review_pair<P>(
        &self,
        selected: &mut Vec<RankedTaskCandidate>,
        ranked: &[RankedTaskCandidate],
        predicate: P,
    ) -> Result<()>
    where
        P: Fn(&RankedTaskCandidate) -> bool,
    {
        if !self.push_first_matching(selected, ranked, &predicate) {
            return Ok(());
        }
        let Some(first_review) = selected.last() else {
            return Ok(());
        };
        let first_fingerprint = self.neighborhood_fingerprint(&first_review.id)?;
        let diverse = ranked.iter().find(|candidate| {
            predicate(candidate)
                && !already_selected(selected, &candidate.id)
                && self
                    .neighborhood_fingerprint(&candidate.id)
                    .map(|fingerprint| {
                        neighborhood_overlap(&first_fingerprint, &fingerprint) <= 0.50
                    })
                    .unwrap_or(false)
        });
        if let Some(candidate) = diverse {
            selected.push(candidate.clone());
            return Ok(());
        }

        self.push_first_matching(selected, ranked, predicate);
        Ok(())
    }

    fn push_first_matching<P>(
        &self,
        selected: &mut Vec<RankedTaskCandidate>,
        ranked: &[RankedTaskCandidate],
        predicate: P,
    ) -> bool
    where
        P: Fn(&RankedTaskCandidate) -> bool,
    {
        let Some(candidate) = ranked
            .iter()
            .find(|candidate| predicate(candidate) && !already_selected(selected, &candidate.id))
        else {
            return false;
        };
        selected.push(candidate.clone());
        true
    }

    fn neighborhood_fingerprint(&self, concept_id: &str) -> Result<BTreeSet<String>> {
        let mut seen = BTreeSet::from([concept_id.to_owned()]);
        let mut frontier = BTreeSet::from([concept_id.to_owned()]);
        for _ in 0..2 {
            let mut next = BTreeSet::new();
            for id in &frontier {
                let mut stmt = self.conn.prepare(
                    "SELECT CASE WHEN src=?1 THEN dst ELSE src END
                     FROM edges
                     WHERE type <> 'maps_to' AND (src=?1 OR dst=?1)",
                )?;
                let neighbors = stmt
                    .query_map([id], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for neighbor in neighbors {
                    if seen.insert(neighbor.clone()) {
                        next.insert(neighbor);
                    }
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
        }
        Ok(seen)
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
        Ok(active > 0 || concept_has_active_gu_rule(&self.conn, concept_id)?)
    }
}

fn is_new_or_weak(candidate: &RankedTaskCandidate) -> bool {
    !candidate.has_attempts || candidate.p_known < 0.6
}

fn already_selected(selected: &[RankedTaskCandidate], concept_id: &str) -> bool {
    selected.iter().any(|candidate| candidate.id == concept_id)
}

fn neighborhood_overlap(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    let denominator = left.len().max(right.len());
    if denominator == 0 {
        return 0.0;
    }
    let intersection = left.intersection(right).count();
    intersection as f64 / denominator as f64
}

fn target_distance(mean: f64) -> f64 {
    if mean < 0.75 {
        0.75 - mean
    } else if mean > 0.90 {
        mean - 0.90
    } else {
        0.0
    }
}

fn role_allows_candidate(
    strategy: BatchStrategy,
    index: usize,
    candidate: &RankedTaskCandidate,
) -> bool {
    match strategy {
        BatchStrategy::Default => {
            if index == 0 {
                is_new_or_weak(candidate)
            } else {
                candidate.p_known >= 0.6
            }
        }
        BatchStrategy::EasyReviews => candidate.p_known >= 0.8,
        BatchStrategy::Flow => {
            if index < 2 {
                is_new_or_weak(candidate)
            } else {
                candidate.p_known >= 0.6
            }
        }
        BatchStrategy::PhantomChallenge
        | BatchStrategy::SettlingProbe
        | BatchStrategy::RegressionRecovery => {
            if index == 0 {
                strategy_target_phase(strategy) == Some(candidate.phase)
            } else {
                candidate.p_known >= 0.6
            }
        }
    }
}

fn phase_batch_strategy(ranked: &[RankedTaskCandidate]) -> Option<BatchStrategy> {
    if ranked
        .iter()
        .any(|candidate| candidate.phase == Phase::Phantom)
    {
        return Some(BatchStrategy::PhantomChallenge);
    }
    if ranked
        .iter()
        .any(|candidate| candidate.phase == Phase::Settling)
    {
        return Some(BatchStrategy::SettlingProbe);
    }
    if ranked
        .iter()
        .any(|candidate| candidate.phase == Phase::Regression)
    {
        return Some(BatchStrategy::RegressionRecovery);
    }
    None
}

fn strategy_target_phase(strategy: BatchStrategy) -> Option<Phase> {
    match strategy {
        BatchStrategy::PhantomChallenge => Some(Phase::Phantom),
        BatchStrategy::SettlingProbe => Some(Phase::Settling),
        BatchStrategy::RegressionRecovery => Some(Phase::Regression),
        _ => None,
    }
}

fn phase_adjustment_protected(strategy: BatchStrategy) -> bool {
    matches!(strategy, BatchStrategy::EasyReviews | BatchStrategy::Flow)
}

fn is_phase_action_strategy(strategy: BatchStrategy) -> bool {
    matches!(
        strategy,
        BatchStrategy::PhantomChallenge
            | BatchStrategy::SettlingProbe
            | BatchStrategy::RegressionRecovery
    )
}

fn phase_adjusted_move(
    selected_move: SelectedMove,
    phase: Phase,
    protect_easy_review: bool,
) -> (SelectedMove, Option<&'static str>) {
    if protect_easy_review {
        return (selected_move, None);
    }
    match phase {
        Phase::Phantom => (bloom_move("transfer"), Some("phantom_challenge")),
        Phase::Settling => (bloom_move("transfer"), Some("settling_probe")),
        Phase::Regression => {
            let move_id = if selected_move.id == "recall" {
                "recall"
            } else {
                "explain"
            };
            (bloom_move(move_id), Some("regression_recovery"))
        }
        _ => (selected_move, None),
    }
}

fn next_task_reason(
    candidate: &RankedTaskCandidate,
    selected_move: SelectedMove,
    phase_strategy: Option<&str>,
) -> String {
    let phase_reason = match phase_strategy {
        Some("phantom_challenge") => "\n相响应：用迁移题确认是否真懂。",
        Some("settling_probe") => "\n相响应：用迁移题补新情境证据。",
        Some("regression_recovery") => "\n相响应：先降到回忆或解释任务恢复证据。",
        _ => "",
    };
    format!(
        "选它因为：当前效用最高。\n证据是：U={:.3}，并结合 p_known 与 max_depth 选择 {}。{}\n现在做什么：完成一个 {} 任务。",
        candidate.utility, selected_move.target_depth, phase_reason, selected_move.id
    )
}

fn dominant_state_from_posterior(value: &serde_json::Value) -> Option<String> {
    let values = value.as_array()?;
    let (idx, _) = values
        .iter()
        .enumerate()
        .filter_map(|(idx, value)| value.as_f64().map(|probability| (idx, probability)))
        .max_by(|left, right| left.1.total_cmp(&right.1))?;
    let state = match idx {
        0 => "flow",
        1 => "productive_confusion",
        2 => "frustrated",
        3 => "bored",
        4 => "anxious",
        5 => "fatigued",
        _ => return None,
    };
    Some(state.to_owned())
}

fn easier_move(selected_move: SelectedMove) -> SelectedMove {
    match selected_move.id {
        "recall" | "explain" => selected_move,
        _ => bloom_move("explain"),
    }
}
