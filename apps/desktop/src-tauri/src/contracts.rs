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
