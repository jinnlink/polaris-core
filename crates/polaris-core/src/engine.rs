use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

use crate::config::meta_f64;
use crate::consolidation::{run_nightly_consolidation, ConsolidationSummary};
use crate::diagnosis::{diagnose_concept, GraphDiagnosis};
use crate::error::{PolarisError, Result};
use crate::fsrs::{retrievability, FsrsParams, FsrsState, Rating};
use crate::geometry::{
    geometry_candidates, refresh_missing_embeddings, refresh_missing_embeddings_with_provider,
    upsert_geometry_maps_to_candidates, EmbeddingProvider, EmbeddingRefreshSummary,
    GeometryCandidate,
};
use crate::grader::{
    grade_request_for_attempt, grade_with_config, grade_with_static_response,
    heuristic_score_with_conn, GradeRequest, LlmConfig,
};
use crate::graph::{structural_mapping_score, upsert_maps_to_candidate, StructuralMapping};
use crate::gu::{
    active_gu_rules_for_concept, concept_has_active_gu_rule, run_gu_induction, ActiveGuRule,
    GuInductionSummary,
};
use crate::mastery::{fold_all, fold_attempt, AttemptObservation, MasteryParams, MasteryState};
use crate::mental_fit::{run_mental_dynamics_fit, transitions_from_meta, MentalFitSummary};
use crate::mental_state::{
    estimate_hazard, forward_filter_with_transitions, prior_transitions, HazardInputs,
    HmmObservation, StatePosterior, HAZARD_FEATURE_COUNT, STATE_COUNT,
};
use crate::mirt::{
    ensure_theta, fused_p_known, initial_track_q_blob, latent_prediction, update_theta_for_attempt,
    FusedPKnown, LatentPrediction,
};
use crate::moves::{
    bloom_move, fallback_template, move_template_for_concept, render_move_prompt,
    select_next_move_for_concept, task_type_target_depth, SelectedMove,
};
use crate::pack::load_pack;
use crate::phase::{determine_phase, Depth, Phase, PhaseInput, PhaseParams};
use crate::report::{
    latest_mirror_report, record_report_feedback, run_mirror_report, MirrorReport,
};
use crate::scheduler::{rank_candidates_with_params, ScheduleCandidate, SchedulerParams};
use crate::status::{status_snapshot, StatusSnapshot};
use crate::teaching::{teaching_instruction, TeachingInstruction};
use crate::tuning::{run_param_tuning, TuningSummary};

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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskAssignment {
    pub concept_id: String,
    pub concept_name: String,
    #[serde(rename = "move")]
    pub move_name: String,
    pub task_type: String,
    pub template: String,
    pub phase: Phase,
    pub p_known: f64,
    pub expected_success: f64,
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
    pub phase: Phase,
}

struct MentalStateRecord<'a> {
    attempt_id: &'a str,
    score_source: &'a str,
    session_id: &'a str,
    concept_id: &'a str,
    observation: HmmObservation,
    p_hat: f64,
    prior_posterior: Option<StatePosterior>,
}

#[derive(Debug, Clone)]
struct RankedTaskCandidate {
    id: String,
    name: String,
    utility: f64,
    p_known: f64,
    has_attempts: bool,
    phase: Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchStrategy {
    Default,
    EasyReviews,
    Flow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhaseHistorySummary {
    ever_reached_transfer_or_generation: bool,
    recent_lapses: u32,
}

impl Engine {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn structural_mapping_score(&self, left: &str, right: &str) -> Result<StructuralMapping> {
        structural_mapping_score(&self.conn, left, right)
    }

    pub fn upsert_maps_to_candidate(
        &mut self,
        left: &str,
        right: &str,
    ) -> Result<Option<StructuralMapping>> {
        upsert_maps_to_candidate(&self.conn, left, right)
    }

    pub fn diagnose_concept(&self, concept_id: &str) -> Result<GraphDiagnosis> {
        diagnose_concept(&self.conn, concept_id)
    }

    pub fn status_snapshot(&self) -> Result<StatusSnapshot> {
        status_snapshot(&self.conn)
    }

    pub fn teaching_instruction(&self, concept_id: &str) -> Result<TeachingInstruction> {
        teaching_instruction(&self.conn, concept_id)
    }

    pub fn latent_prediction(&self, concept_id: &str, task_type: &str) -> Result<LatentPrediction> {
        latent_prediction(&self.conn, concept_id, task_type)
    }

    pub fn fused_p_known(&self, concept_id: &str, task_type: &str) -> Result<FusedPKnown> {
        fused_p_known(&self.conn, concept_id, task_type)
    }

    pub fn run_nightly_consolidation(&self) -> Result<ConsolidationSummary> {
        run_nightly_consolidation(&self.conn)
    }

    pub fn run_param_tuning(&self) -> Result<TuningSummary> {
        run_param_tuning(&self.conn)
    }

    pub fn run_mental_dynamics_fit(&self) -> Result<MentalFitSummary> {
        run_mental_dynamics_fit(&self.conn)
    }

    pub fn run_mirror_report(&self) -> Result<MirrorReport> {
        run_mirror_report(&self.conn)
    }

    pub fn latest_mirror_report(&self) -> Result<Option<MirrorReport>> {
        latest_mirror_report(&self.conn)
    }

    pub fn record_report_feedback(
        &self,
        report_id: Option<&str>,
        assertion_id: &str,
    ) -> Result<String> {
        record_report_feedback(&self.conn, report_id, assertion_id, "inaccurate")
    }

    pub fn run_gu_induction(&self) -> Result<GuInductionSummary> {
        run_gu_induction(&self.conn)
    }

    pub fn active_gu_rules_for_concept(&self, concept_id: &str) -> Result<Vec<ActiveGuRule>> {
        active_gu_rules_for_concept(&self.conn, concept_id)
    }

    pub fn refresh_missing_embeddings(&self) -> Result<EmbeddingRefreshSummary> {
        refresh_missing_embeddings(&self.conn)
    }

    pub fn refresh_missing_embeddings_with_provider<P: EmbeddingProvider>(
        &self,
        provider: &P,
    ) -> Result<EmbeddingRefreshSummary> {
        refresh_missing_embeddings_with_provider(&self.conn, provider)
    }

    pub fn geometry_candidates(
        &self,
        source: &str,
        limit: usize,
    ) -> Result<Vec<GeometryCandidate>> {
        geometry_candidates(&self.conn, source, limit)
    }

    pub fn upsert_geometry_maps_to_candidates(
        &mut self,
        source: &str,
        limit: usize,
    ) -> Result<Vec<StructuralMapping>> {
        upsert_geometry_maps_to_candidates(&self.conn, source, limit)
    }

    pub fn init_pack(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let pack = load_pack(path)?;
        let initial_q = initial_track_q_blob(&self.conn)?;
        let tx = self.conn.transaction()?;

        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![format!("pack.{}.rubric", pack.id), pack.rubric],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![
                format!("pack.{}.moves", pack.id),
                serde_json::to_string(&pack.moves)?
            ],
        )?;

        for concept in &pack.concepts {
            tx.execute(
                "INSERT OR REPLACE INTO concepts(id, pack, name, kind, seed_order, p_init, q, provenance, evidence_ids_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT q FROM concepts WHERE id=?1), ?7), 'pack-seed', '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![
                    concept.id,
                    pack.id,
                    concept.name,
                    concept.kind,
                    concept.seed_order,
                    concept.p_init,
                    initial_q
                ],
            )?;
        }

        for edge in &pack.edges {
            tx.execute(
                "INSERT OR REPLACE INTO edges(id, src, dst, type, weight, alignment_json, provenance, evidence_ids_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pack-seed', '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![
                    edge.id,
                    edge.src,
                    edge.dst,
                    edge.edge_type,
                    edge.weight,
                    edge.alignment_json
                ],
            )?;
        }

        tx.commit()?;
        ensure_theta(&self.conn)?;
        Ok(())
    }

    pub fn next_task(&self) -> Result<Option<NextTask>> {
        let ranked = self.ranked_task_candidates()?;
        let Some(best) = ranked.first() else {
            return Ok(None);
        };

        let selected_move = select_next_move_for_concept(&self.conn, &best.id)?;
        let template = move_template_for_concept(&self.conn, &best.id, selected_move.id)?
            .map(|item| item.template)
            .unwrap_or_else(|| fallback_template(selected_move.id).to_owned());
        let prompt_text = render_move_prompt(&template, &best.name);
        Ok(Some(NextTask {
            concept_id: best.id.clone(),
            task_type: selected_move.task_type.to_owned(),
            prompt_text,
            reason: format!(
                "选它因为：当前效用最高。\n证据是：U={:.3}，并结合 p_known 与 max_depth 选择 {}。\n现在做什么：完成一个 {} 任务。",
                best.utility, selected_move.target_depth, selected_move.id
            ),
        }))
    }

    pub fn get_interleaved_batch(&self, batch_size: usize) -> Result<Vec<TaskAssignment>> {
        let batch_size = batch_size.max(1);
        let ranked = self.ranked_task_candidates()?;
        if ranked.len() < 3 || batch_size < 3 {
            return self.single_next_task_assignment();
        }

        let strategy = self.batch_strategy()?;
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
        selected
            .iter()
            .map(|candidate| self.assignment_for_candidate(candidate))
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
        let mut current = self.assignment_mean(selected)?;
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
                    let mean = self.assignment_mean(&proposal)?;
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

    fn assignment_mean(&self, candidates: &[RankedTaskCandidate]) -> Result<f64> {
        let mut total = 0.0;
        for candidate in candidates {
            total += self.expected_success_for_candidate(candidate)?;
        }
        Ok(total / candidates.len() as f64)
    }

    fn expected_success_for_candidate(&self, candidate: &RankedTaskCandidate) -> Result<f64> {
        let mut selected_move = select_next_move_for_concept(&self.conn, &candidate.id)?;
        let mut expected =
            self.expected_success(&candidate.id, selected_move.task_type, candidate.p_known)?;
        if is_new_or_weak(candidate) && expected < 0.50 {
            selected_move = easier_move(selected_move);
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

    fn assignment_for_candidate(&self, candidate: &RankedTaskCandidate) -> Result<TaskAssignment> {
        let mut selected_move = select_next_move_for_concept(&self.conn, &candidate.id)?;
        let mut expected =
            self.expected_success(&candidate.id, selected_move.task_type, candidate.p_known)?;
        if is_new_or_weak(candidate) && expected < 0.50 {
            selected_move = easier_move(selected_move);
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
        let pre_attempt_p_hat = self.pre_attempt_p_hat(&input.concept_id, &input.task_type)?;
        let mental_state_observation =
            self.mental_state_observation(&input, provisional_score, pre_attempt_p_hat)?;
        let fsrs_params = FsrsParams::from_conn(&self.conn)?;
        let target_depth = task_type_target_depth(&input.task_type).unwrap_or("recall");
        self.conn.execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, prompt_text, response_evidence_id,
                                  self_confidence, latency_ms, hint_count, provisional_score, depth, rating, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
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
                target_depth,
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
            self.record_final_mental_state_snapshot_for_attempt(&attempt_id, grade.score)?;
            update_theta_for_attempt(&self.conn, &attempt_id)?;
            self.replay_concept_after(&input.concept_id, Some(&attempt_id))?;
            let _ = self.run_gu_induction()?;
        }

        Ok(SubmitReceipt {
            attempt_id,
            provisional_score,
            degraded: grade.degraded,
        })
    }

    pub fn apply_final_score(&mut self, attempt_id: &str, final_score: f64) -> Result<()> {
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

    fn pre_attempt_p_hat(&self, concept_id: &str, task_type: &str) -> Result<f64> {
        self.fused_p_known(concept_id, task_type)
            .map(|prediction| prediction.p_known)
    }

    fn mental_state_observation(
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

    fn record_mental_state_snapshot(&self, record: MentalStateRecord<'_>) -> Result<()> {
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

    fn record_final_mental_state_snapshot_for_attempt(
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
             WHERE latency_ms IS NOT NULL
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

    fn replay_concept_after(
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

fn confidence_unit(confidence: i32) -> f64 {
    ((confidence as f64 - 1.0) / 4.0).clamp(0.0, 1.0)
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
    }
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

        engine.replay_concept_after("ownership", None).unwrap();

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
