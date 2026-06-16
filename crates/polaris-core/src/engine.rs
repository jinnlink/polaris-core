use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

use crate::breeding::{
    admitted_bred_moves, evaluate_bred_moves, preregister_bred_move, record_bred_move_outcome,
    BredMove, BredMoveInput, BreedingEvaluationSummary,
};
use crate::calibration::{calibration_samples, posterior_from_samples};
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
use crate::goals::{
    create_goal, goal_progress, goal_snapshot, refresh_goal_milestones,
    update_goal_dimension_value, GoalInput, GoalMilestoneRecord, GoalProgressReport, GoalRecord,
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
use crate::learner_feedback::{
    record_learner_feedback, LearnerFeedbackInput, LearnerFeedbackReceipt,
};
use crate::learner_mirror::{learner_mirror_snapshot, LearnerMirrorSnapshot};
use crate::mastery::{fold_all, fold_attempt, AttemptObservation, MasteryParams, MasteryState};
use crate::mental_fit::{run_mental_dynamics_fit, transitions_from_meta, MentalFitSummary};
use crate::mental_state::{
    estimate_hazard, forward_filter_with_transitions, prior_transitions, HazardInputs,
    HmmObservation, StatePosterior, HAZARD_FEATURE_COUNT, STATE_COUNT,
};
use crate::mirt::{
    ensure_pack_theta, ensure_theta, fused_p_known, initial_pack_q_blob, latent_prediction,
    update_theta_for_attempt, FusedPKnown, LatentPrediction,
};
use crate::moves::{
    bloom_move, fallback_template, move_template_for_concept, render_move_prompt,
    select_next_move_for_concept, task_type_target_depth, SelectedMove,
};
use crate::pack::load_pack;
use crate::pack_state::{
    list_packs as list_pack_state, switch_pack_metadata, PackSummary, PackSwitchReceipt, ThetaMode,
};
use crate::pedagogy::{record_move_effect_for_attempt, select_move_for_concept};
use crate::phase::{determine_phase, Depth, Phase, PhaseInput, PhaseParams};
use crate::report::{
    latest_mirror_report, record_report_feedback, run_mirror_report, run_mirror_report_with_config,
    run_mirror_report_with_static_narrative, MirrorReport,
};
use crate::scheduler::{rank_candidates_with_params, ScheduleCandidate, SchedulerParams};
use crate::status::{status_snapshot, StatusSnapshot};
use crate::teaching::{teaching_instruction, TeachingInstruction};
use crate::trust::{trust_panel, TrustPanel};
use crate::tuning::{run_param_tuning, TuningSummary};

mod mental_state;
mod submit_pipeline;
mod task_selection;

pub struct Engine {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextTask {
    pub concept_id: String,
    pub move_id: String,
    pub task_type: String,
    pub prompt_text: String,
    pub reason: String,
    pub mrt_context_hash: String,
    pub mrt_prereg_id: String,
    pub mrt_randomized: bool,
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

pub(crate) struct MentalStateRecord<'a> {
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
    PhantomChallenge,
    SettlingProbe,
    RegressionRecovery,
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

    pub fn list_packs(&self) -> Result<Vec<PackSummary>> {
        list_pack_state(&self.conn)
    }

    pub fn switch_pack(
        &self,
        pack_id: &str,
        theta_mode: Option<ThetaMode>,
    ) -> Result<PackSwitchReceipt> {
        let receipt = switch_pack_metadata(&self.conn, pack_id, theta_mode)?;
        if receipt.theta_mode == ThetaMode::Isolated.as_str() {
            ensure_pack_theta(&self.conn, pack_id)?;
        }
        Ok(receipt)
    }

    pub fn learner_mirror_snapshot(&self) -> Result<LearnerMirrorSnapshot> {
        learner_mirror_snapshot(&self.conn)
    }

    pub fn trust_panel(&self) -> Result<TrustPanel> {
        trust_panel(&self.conn)
    }

    pub fn create_goal(&self, input: GoalInput) -> Result<GoalRecord> {
        create_goal(&self.conn, input)
    }

    pub fn goal_snapshot(&self, goal_id: &str) -> Result<Option<GoalRecord>> {
        goal_snapshot(&self.conn, goal_id)
    }

    pub fn update_goal_dimension_value(
        &self,
        goal_id: &str,
        dimension_key: &str,
        current_value: f64,
    ) -> Result<()> {
        update_goal_dimension_value(&self.conn, goal_id, dimension_key, current_value)
    }

    pub fn goal_progress(&self, goal_id: &str) -> Result<GoalProgressReport> {
        goal_progress(&self.conn, goal_id)
    }

    pub fn refresh_goal_milestones(&self, goal_id: &str) -> Result<Vec<GoalMilestoneRecord>> {
        refresh_goal_milestones(&self.conn, goal_id)
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

    pub fn run_mirror_report_with_narrative(&self) -> Result<MirrorReport> {
        run_mirror_report_with_config(&self.conn, crate::grader::LlmConfig::from_env())
    }

    pub fn run_mirror_report_with_static_narrative(
        &self,
        response_json: &str,
    ) -> Result<MirrorReport> {
        run_mirror_report_with_static_narrative(&self.conn, response_json)
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

    pub fn record_report_feedback_with_verdict(
        &self,
        report_id: Option<&str>,
        assertion_id: &str,
        verdict: &str,
    ) -> Result<String> {
        record_report_feedback(&self.conn, report_id, assertion_id, verdict)
    }

    pub fn record_learner_feedback(
        &self,
        input: LearnerFeedbackInput,
    ) -> Result<LearnerFeedbackReceipt> {
        record_learner_feedback(&self.conn, input)
    }

    pub fn run_gu_induction(&self) -> Result<GuInductionSummary> {
        run_gu_induction(&self.conn)
    }

    pub fn active_gu_rules_for_concept(&self, concept_id: &str) -> Result<Vec<ActiveGuRule>> {
        active_gu_rules_for_concept(&self.conn, concept_id)
    }

    pub fn preregister_bred_move(&self, input: BredMoveInput) -> Result<BredMove> {
        preregister_bred_move(&self.conn, input)
    }

    pub fn record_bred_move_outcome(
        &self,
        prereg_id: &str,
        move_id: &str,
        success: bool,
    ) -> Result<()> {
        record_bred_move_outcome(&self.conn, prereg_id, move_id, success)
    }

    pub fn evaluate_bred_moves(&self) -> Result<BreedingEvaluationSummary> {
        evaluate_bred_moves(&self.conn)
    }

    pub fn admitted_bred_moves(&self, context_hash: &str) -> Result<Vec<BredMove>> {
        admitted_bred_moves(&self.conn, context_hash)
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
        let tx = self.conn.transaction()?;
        let initial_q = initial_pack_q_blob(&tx, &pack.id)?;

        for concept in &pack.concepts {
            let existing_pack: Option<String> = tx
                .query_row(
                    "SELECT pack FROM concepts WHERE id=?1",
                    [concept.id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(existing_pack) = existing_pack {
                if existing_pack != pack.id {
                    return Err(PolarisError::InvalidParameter {
                        key: "pack.concept_id".to_owned(),
                        value: format!(
                            "concept id collision: {} already belongs to {}",
                            concept.id, existing_pack
                        ),
                    });
                }
            }
        }

        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![format!("pack.{}.rubric", pack.id), pack.rubric],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![format!("pack.{}.title", pack.id), pack.title],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES (?1, 'shared')",
            [format!("pack.{}.theta_mode", pack.id)],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES ('active_pack', ?1)",
            [pack.id.as_str()],
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
}
