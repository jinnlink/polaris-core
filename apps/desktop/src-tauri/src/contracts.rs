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
        InboxWorkspaceItem::decl(&config),
        InboxActionInput::decl(&config),
        InboxActionReceipt::decl(&config),
        InboxPracticeDraft::decl(&config),
        InboxPracticeSubmitInput::decl(&config),
        InboxPracticeSubmitReceipt::decl(&config),
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
