use polaris_core::notification::NotificationPolicy;
use polaris_core::pack_state::PackSummary;
use polaris_core::pack_state::PackSwitchReceipt;
use polaris_core::status::{ConceptStatus, PhaseCount, StatusSnapshot};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct TodayAction {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub route: Option<String>,
    pub concept_id: Option<String>,
    pub expected_success: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct TodaySignal {
    pub claim: String,
    pub confidence: f64,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct TodaySnapshot {
    pub generated_at: String,
    pub current_pack: Option<String>,
    pub theta_mode: Option<String>,
    pub packs: Vec<PackSummary>,
    pub top_signal: Option<TodaySignal>,
    pub actions: Vec<TodayAction>,
    pub notification_policy: NotificationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct WindowModeReceipt {
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct NotificationReceipt {
    pub emitted: bool,
    pub suppressed_by_flow: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct MapWorkspaceQuery {
    pub view: String,
    pub pack: Option<String>,
    pub root: Option<String>,
    pub depth: Option<u32>,
    pub phase: Option<String>,
    pub due: Option<String>,
    pub min_confidence: Option<f64>,
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspaceSnapshot {
    pub generated_at: String,
    pub view: String,
    pub resolved_pack: Option<String>,
    pub theta_mode: Option<String>,
    pub total_nodes: usize,
    pub returned_nodes: usize,
    pub next_cursor: Option<String>,
    pub nodes: Vec<MapWorkspaceNode>,
    pub edges: Vec<MapWorkspaceEdge>,
    pub aggregates: Vec<MapWorkspaceAggregate>,
    pub anchors: Vec<MapWorkspaceAnchor>,
    pub paths: Vec<MapWorkspacePath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspaceNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub pack: Option<String>,
    pub phase: String,
    pub phase_label: String,
    pub phase_summary: String,
    pub due_status: String,
    pub attempt_count: usize,
    pub evidence_count: usize,
    pub layers: Vec<MapWorkspaceLayer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspaceLayer {
    pub source: String,
    pub cross_domain: bool,
    pub value: f64,
    pub confidence: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub gate_status: String,
    pub model_version: String,
    pub origin: Option<String>,
    pub evidence_ids: Vec<String>,
    pub provenance_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspaceEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub weight: f64,
    pub origin: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspaceAggregate {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub concept_count: usize,
    pub due_count: Option<usize>,
    pub observed_count: Option<usize>,
    pub mean_value: f64,
    pub mean_confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspacePath {
    pub rank: usize,
    pub concept_id: String,
    pub concept_name: String,
    pub move_name: String,
    pub expected_success: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MapWorkspaceAnchor {
    pub id: String,
    pub source_concept_id: String,
    pub source_name: String,
    pub source_pack: Option<String>,
    pub target_id: String,
    pub target_name: String,
    pub target_pack: Option<String>,
    pub structural_score: f64,
    pub difference: String,
    pub origin: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct WorkbenchAction {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct PracticeTask {
    pub task_event_id: String,
    pub session_id: String,
    pub concept_id: String,
    pub concept_name: String,
    pub move_id: String,
    pub task_type: String,
    pub prompt_text: String,
    pub reason: String,
    pub issued_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct PracticeWorkspaceSnapshot {
    pub task: Option<PracticeTask>,
    pub actions: Vec<WorkbenchAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct PracticeSubmitInput {
    pub session_id: String,
    pub task_event_id: String,
    pub response_text: String,
    pub self_confidence: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PracticeSubmitReceipt {
    pub attempt_id: String,
    pub provisional_score: f64,
    pub degraded: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct AttemptGradeStatus {
    pub attempt_id: String,
    pub evidence_id: String,
    pub provisional_score: f64,
    pub final_score: Option<f64>,
    pub graded_at: Option<String>,
    pub queued: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GradeQueueReceipt {
    pub processed: i32,
    pub pending: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct CaptureWorkspaceInput {
    pub session_id: Option<String>,
    pub source: String,
    pub content_type: String,
    pub text: String,
    pub learner_kind: String,
    pub candidate_concept_ids: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CaptureWorkspaceReceipt {
    pub capture_id: String,
    pub evidence_id: String,
    pub status: String,
    pub learner_kind: String,
    pub effect: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct InboxWorkspaceQuery {
    pub statuses: Vec<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InboxActionOption {
    pub action: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InboxSuggestionView {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub reason: String,
    pub model_version: String,
    pub evidence_id: String,
    pub quote: String,
    pub base_pack_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct OverlayDecisionInput {
    pub base_pack_id: String,
    pub suggestion_ids: Vec<String>,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct OverlayDecisionReceipt {
    pub status: String,
    pub message: String,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct GenerateSuggestionsInput {
    pub capture_id: String,
    pub base_pack_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GenerateSuggestionsReceipt {
    pub status: String,
    pub count: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InboxWorkspaceItem {
    pub capture_id: String,
    pub evidence_id: String,
    pub status: String,
    pub learner_kind: String,
    pub source: String,
    pub content_type: String,
    pub text_preview: String,
    pub concept_hint: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message: String,
    pub actions: Vec<InboxActionOption>,
    pub suggestions: Vec<InboxSuggestionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct InboxActionInput {
    pub capture_id: String,
    pub action: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InboxActionReceipt {
    pub capture_id: String,
    pub status: String,
    pub effect: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct InboxPracticeDraft {
    pub capture_id: String,
    pub evidence_id: String,
    pub status: String,
    pub concept_hint: Option<String>,
    pub task_type: String,
    pub prompt: String,
    pub source_excerpt: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct InboxPracticeSubmitInput {
    pub capture_id: String,
    pub session_id: String,
    pub response_text: String,
    pub self_confidence: i32,
    pub latency_ms: i32,
    pub hint_count: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct InboxPracticeSubmitReceipt {
    pub capture_id: String,
    pub attempt_id: String,
    pub status: String,
    pub effect: String,
    pub message: String,
    pub provisional_score: f64,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ProfileSettingsView {
    pub enabled: bool,
    pub disclosure_required: bool,
    pub disclosure_acknowledged: bool,
    pub summary_sharing_enabled: bool,
    pub paused_until: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProfileBehaviorFact {
    pub id: String,
    pub label: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProfileDimensionView {
    pub key: String,
    pub label: String,
    pub mean: f64,
    pub lower: f64,
    pub upper: f64,
    pub evidence_count: i32,
    pub gate_status: String,
    pub gate_label: String,
    pub purpose: String,
    pub will_not_affect: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProfileWorkspaceSnapshot {
    pub generated_at: String,
    pub settings: ProfileSettingsView,
    pub facts: Vec<ProfileBehaviorFact>,
    pub dimensions: Vec<ProfileDimensionView>,
    pub notice: String,
    pub actions: Vec<WorkbenchAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct GoalScopeInput {
    pub pack_ids: Vec<String>,
    pub dimension_keys: Vec<String>,
    pub concept_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct GoalDimensionInput {
    pub id: String,
    pub dimension_key: String,
    pub display_name: String,
    pub metric_type: String,
    pub target_value: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct GoalMilestoneInput {
    pub id: String,
    pub title: String,
    pub dimension_key: Option<String>,
    pub threshold: Option<f64>,
    pub manual: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct GoalEditorInput {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub deadline: Option<String>,
    pub pace: Option<String>,
    pub priority: i32,
    pub scope: GoalScopeInput,
    pub dimensions: Vec<GoalDimensionInput>,
    pub milestones: Vec<GoalMilestoneInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GoalDimensionView {
    pub id: String,
    pub dimension_key: String,
    pub display_name: String,
    pub metric_type: String,
    pub current_value: f64,
    pub target_value: f64,
    pub weight: f64,
    pub progress: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GoalMilestoneView {
    pub id: String,
    pub title: String,
    pub status: String,
    pub reached_at: Option<String>,
    pub dimension_key: Option<String>,
    pub threshold: Option<f64>,
    pub manual: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GoalView {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub deadline: Option<String>,
    pub pace: Option<String>,
    pub priority: i32,
    pub scope: GoalScopeInput,
    pub overall_progress: f64,
    pub dimensions: Vec<GoalDimensionView>,
    pub milestones: Vec<GoalMilestoneView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GoalWorkspaceSnapshot {
    pub generated_at: String,
    pub goals: Vec<GoalView>,
    pub selected_goal_id: Option<String>,
    pub actions: Vec<TodayAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct GoalMutationReceipt {
    pub goal_id: String,
    pub effect: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MirrorCurvePoint {
    pub attempt_id: String,
    pub concept_id: String,
    pub created_at: String,
    pub confidence: f64,
    pub actual_score: f64,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct MirrorPhaseItem {
    pub phase: String,
    pub label: String,
    pub summary: String,
    pub count: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReportItemView {
    pub id: String,
    pub category: String,
    pub kind: String,
    pub subject: String,
    pub claim: String,
    pub confidence: f64,
    pub evidence_ids: Vec<String>,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ReportSkippedView {
    pub id: String,
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ReportCitationView {
    pub evidence_id: String,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ReportNarrativeView {
    pub text: String,
    pub citations: Vec<ReportCitationView>,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MirrorReportView {
    pub id: String,
    pub week: String,
    pub generated_at: String,
    pub window_days: i32,
    pub items: Vec<ReportItemView>,
    pub top_signal: Option<ReportItemView>,
    pub skipped: Vec<ReportSkippedView>,
    pub hazard_participates: bool,
    pub hazard_reason: String,
    pub hazard_validation_auc: Option<f64>,
    pub reflection_prompts: Vec<String>,
    pub narrative: Option<ReportNarrativeView>,
    pub citation_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReportsWorkspaceSnapshot {
    pub generated_at: String,
    pub confidence_curve: Vec<MirrorCurvePoint>,
    pub phase_distribution: Vec<MirrorPhaseItem>,
    pub report: Option<MirrorReportView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct ReportFeedbackInput {
    pub report_id: String,
    pub assertion_id: String,
    pub verdict: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ReportMutationReceipt {
    pub report_id: String,
    pub effect: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TrustGateView {
    pub framework: String,
    pub name: String,
    pub status: String,
    pub gate: String,
    pub metric: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct TrustExperimentView {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub metric: Option<f64>,
    pub sample_summary: String,
    pub hypothesis: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TrustActivityView {
    pub id: String,
    pub label: String,
    pub count_7d: i32,
    pub last_at: Option<String>,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TrustParameterView {
    pub key: String,
    pub current_value: String,
    pub default_value: String,
    pub class: String,
    pub bounds: Option<String>,
    pub tuning_route: String,
    pub is_governance_gate: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct TrustWorkspaceSnapshot {
    pub generated_at: String,
    pub window_days: i32,
    pub gates: Vec<TrustGateView>,
    pub breeding_experiments: Vec<TrustExperimentView>,
    pub mrt_experiments: Vec<TrustExperimentView>,
    pub recent_activity: Vec<TrustActivityView>,
    pub current_pack_id: Option<String>,
    pub governance: Vec<TrustParameterView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AiInteractionProfileView {
    pub persona: String,
    pub verbosity: String,
    pub explanation_depth: String,
    pub proactivity: String,
    pub intervention_frequency: String,
    pub correction_style: String,
    pub custom_notes: Option<String>,
    pub guidance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct AiInteractionProfileUpdate {
    pub persona: Option<String>,
    pub verbosity: Option<String>,
    pub explanation_depth: Option<String>,
    pub proactivity: Option<String>,
    pub intervention_frequency: Option<String>,
    pub correction_style: Option<String>,
    pub custom_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct PrivacyCallView {
    pub id: String,
    pub tier: String,
    pub trigger: String,
    pub data_sent: Vec<String>,
    pub degradation: String,
    pub disabled_when_tier0_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ProfileInstrumentItemView {
    pub id: String,
    pub dimension: String,
    pub prompt: String,
    pub keyed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ProfileInstrumentView {
    pub id: String,
    pub title: String,
    pub version: String,
    pub citation: String,
    pub source_url: String,
    pub response_min: i32,
    pub response_max: i32,
    pub admin_modes: Vec<String>,
    pub interpretation_notice: String,
    pub items: Vec<ProfileInstrumentItemView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SettingsWorkspaceSnapshot {
    pub generated_at: String,
    pub profile: ProfileSettingsView,
    pub ai_profile: AiInteractionProfileView,
    pub tier0_only: bool,
    pub privacy_calls: Vec<PrivacyCallView>,
    pub instruments: Vec<ProfileInstrumentView>,
    pub profile_measurement_count: i32,
    pub profile_dimension_count: i32,
    pub valid_session_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct ProfileSettingsUpdateInput {
    pub enabled: Option<bool>,
    pub acknowledge_disclosure: bool,
    pub summary_sharing_enabled: Option<bool>,
    pub paused_until: Option<String>,
    pub clear_pause: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct ProfileMeasurementSubmitInput {
    pub session_id: String,
    pub instrument_id: String,
    pub instrument_version: String,
    pub item_id: String,
    pub locale: String,
    pub admin_mode: String,
    pub response: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SettingsMutationReceipt {
    pub effect: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LifecycleSnapshot {
    pub database_path: String,
    pub database_source: String,
    pub database_path_acknowledged: bool,
    pub startup_status: String,
    pub startup_message: String,
    pub schema_version: Option<i32>,
    pub upgrade_required: bool,
    pub pre_upgrade_backup: Option<String>,
    pub previous_run_incomplete: bool,
    pub recovered_background_jobs: Vec<String>,
    pub pending_background_jobs: Vec<String>,
    pub config_warning: Option<String>,
    pub startup_enabled: bool,
    pub fast_api_key_configured: bool,
    pub strong_api_key_configured: bool,
    pub embed_api_key_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct DatabasePathInput {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct CredentialInput {
    pub slot: String,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct DiagnosticExportInput {
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct BackgroundEventView {
    pub job: Option<String>,
    pub status: String,
    pub invalidates: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct ProfileExportInput {
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FullDeleteScopePreview {
    pub database_path: String,
    pub learning_attempts: i32,
    pub evidence_records: i32,
    pub goals: i32,
    pub profile_measurements: i32,
    pub reports: i32,
    pub behavior_events: i32,
    pub sqlite_files: Vec<String>,
    pub confirmation_phrase: String,
    pub backup_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
pub struct FullDeleteInput {
    pub confirmation: String,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FullDeleteReceiptView {
    pub deleted_at: String,
    pub database_path: String,
    pub backup_path: Option<String>,
    pub files_deleted: i32,
    pub local_secrets_deleted: i32,
    pub empty_database_created: bool,
    pub message: String,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl CommandError {
    pub fn core(error: impl std::fmt::Display) -> Self {
        Self {
            code: "core_error".to_owned(),
            message: error.to_string(),
            retryable: false,
        }
    }

    pub fn state(message: impl Into<String>) -> Self {
        Self {
            code: "state_unavailable".to_owned(),
            message: message.into(),
            retryable: true,
        }
    }
}

pub fn generated_typescript_contracts() -> String {
    let config = ts_rs::Config::default();
    let declarations = [
        PackSummary::decl(&config),
        PhaseCount::decl(&config),
        ConceptStatus::decl(&config),
        StatusSnapshot::decl(&config),
        PackSwitchReceipt::decl(&config),
        NotificationPolicy::decl(&config),
        TodayAction::decl(&config),
        TodaySignal::decl(&config),
        TodaySnapshot::decl(&config),
        WindowModeReceipt::decl(&config),
        NotificationReceipt::decl(&config),
        MapWorkspaceQuery::decl(&config),
        MapWorkspaceLayer::decl(&config),
        MapWorkspaceNode::decl(&config),
        MapWorkspaceEdge::decl(&config),
        MapWorkspaceAggregate::decl(&config),
        MapWorkspaceAnchor::decl(&config),
        MapWorkspacePath::decl(&config),
        MapWorkspaceSnapshot::decl(&config),
        WorkbenchAction::decl(&config),
        PracticeTask::decl(&config),
        PracticeWorkspaceSnapshot::decl(&config),
        PracticeSubmitInput::decl(&config),
        PracticeSubmitReceipt::decl(&config),
        AttemptGradeStatus::decl(&config),
        GradeQueueReceipt::decl(&config),
        CaptureWorkspaceInput::decl(&config),
        CaptureWorkspaceReceipt::decl(&config),
        InboxWorkspaceQuery::decl(&config),
        InboxActionOption::decl(&config),
        InboxSuggestionView::decl(&config),
        OverlayDecisionInput::decl(&config),
        OverlayDecisionReceipt::decl(&config),
        GenerateSuggestionsInput::decl(&config),
        GenerateSuggestionsReceipt::decl(&config),
        InboxWorkspaceItem::decl(&config),
        InboxActionInput::decl(&config),
        InboxActionReceipt::decl(&config),
        InboxPracticeDraft::decl(&config),
        InboxPracticeSubmitInput::decl(&config),
        InboxPracticeSubmitReceipt::decl(&config),
        ProfileSettingsView::decl(&config),
        ProfileBehaviorFact::decl(&config),
        ProfileDimensionView::decl(&config),
        ProfileWorkspaceSnapshot::decl(&config),
        GoalScopeInput::decl(&config),
        GoalDimensionInput::decl(&config),
        GoalMilestoneInput::decl(&config),
        GoalEditorInput::decl(&config),
        GoalDimensionView::decl(&config),
        GoalMilestoneView::decl(&config),
        GoalView::decl(&config),
        GoalWorkspaceSnapshot::decl(&config),
        GoalMutationReceipt::decl(&config),
        MirrorCurvePoint::decl(&config),
        MirrorPhaseItem::decl(&config),
        ReportItemView::decl(&config),
        ReportSkippedView::decl(&config),
        ReportCitationView::decl(&config),
        ReportNarrativeView::decl(&config),
        MirrorReportView::decl(&config),
        ReportsWorkspaceSnapshot::decl(&config),
        ReportFeedbackInput::decl(&config),
        ReportMutationReceipt::decl(&config),
        TrustGateView::decl(&config),
        TrustExperimentView::decl(&config),
        TrustActivityView::decl(&config),
        TrustParameterView::decl(&config),
        TrustWorkspaceSnapshot::decl(&config),
        AiInteractionProfileView::decl(&config),
        AiInteractionProfileUpdate::decl(&config),
        PrivacyCallView::decl(&config),
        ProfileInstrumentItemView::decl(&config),
        ProfileInstrumentView::decl(&config),
        SettingsWorkspaceSnapshot::decl(&config),
        ProfileSettingsUpdateInput::decl(&config),
        ProfileMeasurementSubmitInput::decl(&config),
        SettingsMutationReceipt::decl(&config),
        LifecycleSnapshot::decl(&config),
        DatabasePathInput::decl(&config),
        CredentialInput::decl(&config),
        DiagnosticExportInput::decl(&config),
        BackgroundEventView::decl(&config),
        ProfileExportInput::decl(&config),
        FullDeleteScopePreview::decl(&config),
        FullDeleteInput::decl(&config),
        FullDeleteReceiptView::decl(&config),
        CommandError::decl(&config),
    ];
    format!(
        "// @generated by polaris-desktop Rust DTOs. Do not edit by hand.\n{}\n",
        declarations
            .into_iter()
            .map(|declaration| format!("export {declaration}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
