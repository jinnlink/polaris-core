use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use polaris_core::capture_queue::{CaptureInput, CaptureStatus, LearnerCaptureKind};
use polaris_core::db::open_database;
use polaris_core::engine::{Engine, TaskAssignment};
use polaris_core::inbox_practice::InboxPracticeSubmissionInput;
use polaris_core::knowledge_map::{
    KnowledgeMapDueStatus, KnowledgeMapGateStatus, KnowledgeMapQuery, KnowledgeMapScope,
    KnowledgeMapSnapshot, KnowledgeMapStateSource,
};
use polaris_core::learner_inbox::LearnerInboxAction;
use polaris_core::notification::NotificationPolicy;
use polaris_core::pack_state::{PackSwitchReceipt, ThetaMode};
use polaris_core::prediction_map::{PredictionEstimate, PredictionMapSnapshot};
use polaris_core::status::StatusSnapshot;

use crate::contracts::{
    AttemptGradeStatus, CaptureWorkspaceInput, CaptureWorkspaceReceipt, CommandError,
    GradeQueueReceipt, InboxActionInput, InboxActionOption, InboxActionReceipt, InboxPracticeDraft,
    InboxPracticeSubmitInput, InboxPracticeSubmitReceipt, InboxWorkspaceItem, InboxWorkspaceQuery,
    MapWorkspaceAggregate, MapWorkspaceAnchor, MapWorkspaceEdge, MapWorkspaceLayer,
    MapWorkspaceNode, MapWorkspacePath, MapWorkspaceQuery, MapWorkspaceSnapshot,
    NotificationReceipt, PracticeSubmitInput, PracticeSubmitReceipt, PracticeTask,
    PracticeWorkspaceSnapshot, TodayAction, TodaySignal, TodaySnapshot, WorkbenchAction,
};

pub struct DesktopState {
    engine: Mutex<Engine>,
}

impl DesktopState {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CommandError> {
        let connection = open_database(path).map_err(CommandError::core)?;
        Ok(Self {
            engine: Mutex::new(Engine::new(connection)),
        })
    }

    fn engine(&self) -> Result<MutexGuard<'_, Engine>, CommandError> {
        self.engine
            .lock()
            .map_err(|_| CommandError::state("Polaris 引擎暂时不可用，请重试"))
    }

    pub fn status(&self) -> Result<StatusSnapshot, CommandError> {
        self.engine()?.status_snapshot().map_err(CommandError::core)
    }

    pub fn today(&self) -> Result<TodaySnapshot, CommandError> {
        let engine = self.engine()?;
        let status = engine.status_snapshot().map_err(CommandError::core)?;
        let top_signal = engine
            .latest_mirror_report()
            .map_err(CommandError::core)?
            .and_then(|report| report.top_signal)
            .map(|signal| TodaySignal {
                claim: signal.claim,
                confidence: signal.confidence,
                suggested_action: signal.suggested_action,
            });
        let assignments = engine
            .get_interleaved_batch(3)
            .map_err(CommandError::core)?;
        let notification_policy = engine.notification_policy().map_err(CommandError::core)?;

        Ok(TodaySnapshot {
            generated_at: status.generated_at,
            current_pack: status.current_pack,
            theta_mode: status.theta_mode,
            packs: status.packs,
            top_signal,
            actions: build_today_actions(assignments),
            notification_policy,
        })
    }

    pub fn switch_pack(
        &self,
        pack_id: &str,
        theta_mode: Option<&str>,
    ) -> Result<PackSwitchReceipt, CommandError> {
        let theta_mode = theta_mode
            .map(ThetaMode::parse)
            .transpose()
            .map_err(CommandError::core)?;
        self.engine()?
            .switch_pack(pack_id, theta_mode)
            .map_err(CommandError::core)
    }

    pub fn notification_policy(&self) -> Result<NotificationPolicy, CommandError> {
        self.engine()?
            .notification_policy()
            .map_err(CommandError::core)
    }

    pub fn map_workspace(
        &self,
        request: MapWorkspaceQuery,
    ) -> Result<MapWorkspaceSnapshot, CommandError> {
        let query = core_map_query(&request)?;
        let engine = self.engine()?;
        match request.view.as_str() {
            "current" | "global" => engine
                .knowledge_map(query)
                .map(|snapshot| map_from_knowledge(&request.view, snapshot))
                .map_err(CommandError::core),
            "prediction" => engine
                .knowledge_and_prediction_map(query)
                .map(|(knowledge, prediction)| map_from_prediction(knowledge, prediction))
                .map_err(CommandError::core),
            other => Err(CommandError::state(format!("unknown map view: {other}"))),
        }
    }

    pub fn practice_workspace(
        &self,
        session_id: &str,
    ) -> Result<PracticeWorkspaceSnapshot, CommandError> {
        let task = self
            .engine()?
            .issue_or_resume_task(session_id)
            .map_err(CommandError::core)?
            .map(|task| PracticeTask {
                task_event_id: task.task_event_id,
                session_id: task.session_id,
                concept_id: task.concept_id,
                concept_name: task.concept_name,
                move_id: task.move_id,
                task_type: task.task_type,
                prompt_text: task.prompt_text,
                reason: task.reason,
                issued_at: task.issued_at,
            });
        let actions = if task.is_some() {
            workbench_actions(&[
                ("submit", "提交回答", "primary"),
                ("capture", "保存为资料", "secondary"),
                ("today", "返回 Today", "quiet"),
            ])
        } else {
            workbench_actions(&[
                ("capture", "记录新资料", "primary"),
                ("inbox", "处理收件箱", "secondary"),
                ("today", "返回 Today", "quiet"),
            ])
        };
        Ok(PracticeWorkspaceSnapshot { task, actions })
    }

    pub fn submit_practice(
        &self,
        input: PracticeSubmitInput,
    ) -> Result<PracticeSubmitReceipt, CommandError> {
        let receipt = self
            .engine()?
            .submit_task_response_provisional(
                &input.session_id,
                &input.task_event_id,
                input.response_text,
                input.self_confidence,
            )
            .map_err(CommandError::core)?;
        Ok(PracticeSubmitReceipt {
            attempt_id: receipt.attempt_id,
            provisional_score: receipt.provisional_score,
            degraded: receipt.degraded,
            message: "回答已本地落账；后台评分回来后会自动修正。".to_owned(),
        })
    }

    pub fn attempt_grade_status(
        &self,
        attempt_id: &str,
    ) -> Result<AttemptGradeStatus, CommandError> {
        let status = self
            .engine()?
            .attempt_grade_status(attempt_id)
            .map_err(CommandError::core)?;
        Ok(AttemptGradeStatus {
            attempt_id: status.attempt_id,
            evidence_id: status.evidence_id,
            provisional_score: status.provisional_score,
            final_score: status.final_score,
            graded_at: status.graded_at,
            queued: status.queued,
        })
    }

    pub fn process_grade_queue(&self) -> Result<GradeQueueReceipt, CommandError> {
        let summary = self.engine()?.grade_pending().map_err(CommandError::core)?;
        Ok(GradeQueueReceipt {
            processed: i32::try_from(summary.processed).unwrap_or(i32::MAX),
            pending: i32::try_from(summary.pending).unwrap_or(i32::MAX),
        })
    }

    pub fn capture_workspace(
        &self,
        input: CaptureWorkspaceInput,
    ) -> Result<CaptureWorkspaceReceipt, CommandError> {
        let learner_kind = LearnerCaptureKind::parse(&input.learner_kind)
            .ok_or_else(|| CommandError::core("unknown learner capture kind"))?;
        let receipt = self
            .engine()?
            .capture_learning_evidence(CaptureInput {
                session_id: input.session_id,
                source: input.source,
                content_type: input.content_type,
                text: input.text,
                learner_kind,
                candidate_concept_ids: input.candidate_concept_ids,
                note: input.note,
            })
            .map_err(CommandError::core)?;
        Ok(CaptureWorkspaceReceipt {
            capture_id: receipt.capture_id,
            evidence_id: receipt.evidence_id,
            status: receipt.status.as_str().to_owned(),
            learner_kind: receipt.learner_kind.as_str().to_owned(),
            effect: receipt.effect.as_str().to_owned(),
            message: receipt.message,
        })
    }

    pub fn inbox_workspace(
        &self,
        query: InboxWorkspaceQuery,
    ) -> Result<Vec<InboxWorkspaceItem>, CommandError> {
        let statuses = query
            .statuses
            .iter()
            .map(|status| {
                CaptureStatus::parse(status)
                    .ok_or_else(|| CommandError::core(format!("unknown inbox status: {status}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.engine()?
            .learner_inbox(&statuses, query.limit)
            .map_err(CommandError::core)
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| InboxWorkspaceItem {
                        capture_id: item.capture_id,
                        evidence_id: item.evidence_id,
                        status: item.status.as_str().to_owned(),
                        learner_kind: item.learner_kind.as_str().to_owned(),
                        source: item.source,
                        content_type: item.content_type,
                        text_preview: item.text_preview,
                        concept_hint: item.concept_hint,
                        note: item.note,
                        created_at: item.created_at,
                        updated_at: item.updated_at,
                        message: item.message,
                        actions: item
                            .actions
                            .into_iter()
                            .map(|action| InboxActionOption {
                                action: action.action.as_str().to_owned(),
                                label: action.label,
                            })
                            .collect(),
                    })
                    .collect()
            })
    }

    pub fn act_on_inbox(
        &self,
        input: InboxActionInput,
    ) -> Result<InboxActionReceipt, CommandError> {
        let action = LearnerInboxAction::parse(&input.action)
            .ok_or_else(|| CommandError::core("unknown learner inbox action"))?;
        let receipt = self
            .engine()?
            .act_on_learner_inbox_item(&input.capture_id, action, input.note)
            .map_err(CommandError::core)?;
        Ok(InboxActionReceipt {
            capture_id: receipt.capture_id,
            status: receipt.status.as_str().to_owned(),
            effect: receipt.effect,
            message: receipt.message,
        })
    }

    pub fn draft_inbox_practice(
        &self,
        capture_id: &str,
    ) -> Result<InboxPracticeDraft, CommandError> {
        let draft = self
            .engine()?
            .draft_inbox_practice(capture_id)
            .map_err(CommandError::core)?;
        Ok(InboxPracticeDraft {
            capture_id: draft.capture_id,
            evidence_id: draft.evidence_id,
            status: draft.status.as_str().to_owned(),
            concept_hint: draft.concept_hint,
            task_type: draft.task_type,
            prompt: draft.prompt,
            source_excerpt: draft.source_excerpt,
            message: draft.message,
        })
    }

    pub fn submit_inbox_practice(
        &self,
        input: InboxPracticeSubmitInput,
    ) -> Result<InboxPracticeSubmitReceipt, CommandError> {
        let receipt = self
            .engine()?
            .submit_inbox_practice_provisional(InboxPracticeSubmissionInput {
                capture_id: input.capture_id,
                session_id: input.session_id,
                response_text: input.response_text,
                self_confidence: input.self_confidence,
                latency_ms: i64::from(input.latency_ms),
                hint_count: i64::from(input.hint_count),
            })
            .map_err(CommandError::core)?;
        Ok(InboxPracticeSubmitReceipt {
            capture_id: receipt.capture_id,
            attempt_id: receipt.attempt_id,
            status: receipt.status.as_str().to_owned(),
            effect: receipt.effect,
            message: receipt.message,
            provisional_score: receipt.provisional_score,
            degraded: receipt.degraded,
        })
    }
}

fn workbench_actions(items: &[(&str, &str, &str)]) -> Vec<WorkbenchAction> {
    items
        .iter()
        .map(|(id, label, kind)| WorkbenchAction {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
            kind: (*kind).to_owned(),
        })
        .collect()
}

fn core_map_query(request: &MapWorkspaceQuery) -> Result<KnowledgeMapQuery, CommandError> {
    let scope = match request.view.as_str() {
        "current" | "prediction" => KnowledgeMapScope::Pack,
        "global" => KnowledgeMapScope::Global,
        other => return Err(CommandError::state(format!("unknown map view: {other}"))),
    };
    let due = request
        .due
        .as_deref()
        .map(|value| match value {
            "new" => Ok(KnowledgeMapDueStatus::New),
            "due" => Ok(KnowledgeMapDueStatus::Due),
            "scheduled" => Ok(KnowledgeMapDueStatus::Scheduled),
            "unscheduled" => Ok(KnowledgeMapDueStatus::Unscheduled),
            other => Err(CommandError::state(format!("unknown due status: {other}"))),
        })
        .transpose()?;
    Ok(KnowledgeMapQuery {
        scope,
        pack: request.pack.clone(),
        root: request.root.clone(),
        depth: request.depth,
        phase: request.phase.clone(),
        due,
        min_confidence: request.min_confidence,
        limit: request.limit,
        cursor: request.cursor.clone(),
    })
}

fn map_from_knowledge(view: &str, snapshot: KnowledgeMapSnapshot) -> MapWorkspaceSnapshot {
    let aggregates = map_aggregates(&snapshot);
    let model_version = snapshot.model_version.clone();
    let nodes = snapshot
        .nodes
        .into_iter()
        .map(|node| MapWorkspaceNode {
            id: node.id,
            name: node.name,
            kind: node.kind,
            pack: node.pack,
            phase: node.phase,
            phase_label: node.phase_label,
            phase_summary: node.phase_summary,
            due_status: due_status(node.due_status),
            attempt_count: node.attempt_count,
            evidence_count: node.evidence_count,
            layers: vec![MapWorkspaceLayer {
                source: state_source(node.provenance.source),
                cross_domain: false,
                value: node.p_known,
                confidence: Some(node.uncertainty.confidence),
                lower: None,
                upper: None,
                gate_status: gate_status(node.provenance.gate_status),
                model_version: model_version.clone(),
                origin: node.provenance.origin,
                evidence_ids: node.provenance.evidence_ids,
                provenance_complete: node.provenance.complete,
            }],
        })
        .collect();
    MapWorkspaceSnapshot {
        generated_at: snapshot.generated_at,
        view: view.to_owned(),
        resolved_pack: snapshot.summary.resolved_pack,
        theta_mode: None,
        total_nodes: snapshot.summary.total_nodes,
        returned_nodes: snapshot.summary.returned_nodes,
        next_cursor: snapshot.next_cursor,
        nodes,
        edges: map_edges(snapshot.edges),
        aggregates,
        anchors: Vec::new(),
        paths: Vec::new(),
    }
}

fn map_from_prediction(
    knowledge: KnowledgeMapSnapshot,
    prediction: PredictionMapSnapshot,
) -> MapWorkspaceSnapshot {
    let aggregates = map_aggregates(&knowledge);
    let metadata = knowledge
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let nodes = prediction
        .nodes
        .into_iter()
        .map(|node| {
            let current = metadata.get(&node.id);
            let mut layers = Vec::new();
            for estimate in [node.observed, node.latent_prediction, node.inherited_prior]
                .into_iter()
                .flatten()
            {
                layers.push(map_prediction_layer(estimate));
            }
            MapWorkspaceNode {
                id: node.id,
                name: node.name,
                kind: node.kind,
                pack: node.pack,
                phase: node.phase,
                phase_label: current
                    .map(|item| item.phase_label.clone())
                    .unwrap_or_default(),
                phase_summary: current
                    .map(|item| item.phase_summary.clone())
                    .unwrap_or_default(),
                due_status: due_status(node.due_status),
                attempt_count: node.attempt_count,
                evidence_count: node.evidence_count,
                layers,
            }
        })
        .collect::<Vec<_>>();
    MapWorkspaceSnapshot {
        generated_at: prediction.generated_at,
        view: "prediction".to_owned(),
        resolved_pack: prediction.summary.resolved_pack,
        theta_mode: prediction.summary.theta_mode,
        total_nodes: prediction.summary.total_nodes,
        returned_nodes: prediction.summary.returned_nodes,
        next_cursor: prediction.next_cursor,
        nodes,
        edges: map_edges(knowledge.edges),
        aggregates,
        anchors: prediction
            .anchors
            .into_iter()
            .map(|anchor| MapWorkspaceAnchor {
                id: anchor.id,
                source_concept_id: anchor.source_concept_id,
                source_name: anchor.source_name,
                source_pack: anchor.source_pack,
                target_id: anchor.target_id,
                target_name: anchor.target_name,
                target_pack: anchor.target_pack,
                structural_score: anchor.structural_score,
                difference: anchor.difference,
                origin: anchor.provenance.origin,
                evidence_ids: anchor.provenance.evidence_ids,
            })
            .collect(),
        paths: prediction
            .initial_paths
            .into_iter()
            .map(|path| MapWorkspacePath {
                rank: path.rank,
                concept_id: path.concept_id,
                concept_name: path.concept_name,
                move_name: path.move_name,
                expected_success: path.expected_success,
            })
            .collect(),
    }
}

fn map_prediction_layer(estimate: PredictionEstimate) -> MapWorkspaceLayer {
    MapWorkspaceLayer {
        source: state_source(estimate.source),
        cross_domain: estimate.cross_domain,
        value: estimate.value,
        confidence: None,
        lower: Some(estimate.interval.lower),
        upper: Some(estimate.interval.upper),
        gate_status: gate_status(estimate.gate_status),
        model_version: estimate.model_version,
        origin: estimate.provenance.origin,
        evidence_ids: estimate.provenance.evidence_ids,
        provenance_complete: estimate.provenance.complete,
    }
}

fn map_edges(edges: Vec<polaris_core::knowledge_map::KnowledgeMapEdge>) -> Vec<MapWorkspaceEdge> {
    edges
        .into_iter()
        .map(|edge| MapWorkspaceEdge {
            id: edge.id,
            source_id: edge.source_id,
            target_id: edge.target_id,
            kind: edge.kind,
            weight: edge.weight,
            origin: edge.provenance.origin,
            evidence_ids: edge.provenance.evidence_ids,
        })
        .collect()
}

fn map_aggregates(snapshot: &KnowledgeMapSnapshot) -> Vec<MapWorkspaceAggregate> {
    let packs = snapshot
        .summary
        .packs
        .iter()
        .map(|pack| MapWorkspaceAggregate {
            id: pack.id.clone().unwrap_or_else(|| "unassigned".to_owned()),
            label: pack
                .title
                .clone()
                .or_else(|| pack.id.clone())
                .unwrap_or_else(|| "未分组".to_owned()),
            kind: "pack".to_owned(),
            concept_count: pack.concept_count,
            due_count: Some(pack.due_count),
            observed_count: Some(pack.observed_count),
            mean_value: pack.mean_p_known,
            mean_confidence: pack.mean_confidence,
        });
    let dimensions = snapshot
        .summary
        .dimensions
        .iter()
        .map(|dimension| MapWorkspaceAggregate {
            id: dimension.id.clone(),
            label: dimension.id.clone(),
            kind: "dimension".to_owned(),
            concept_count: dimension.concept_count,
            due_count: None,
            observed_count: None,
            mean_value: dimension.mean_p_known,
            mean_confidence: dimension.mean_confidence,
        });
    packs.chain(dimensions).collect()
}

fn due_status(status: KnowledgeMapDueStatus) -> String {
    match status {
        KnowledgeMapDueStatus::New => "new",
        KnowledgeMapDueStatus::Due => "due",
        KnowledgeMapDueStatus::Scheduled => "scheduled",
        KnowledgeMapDueStatus::Unscheduled => "unscheduled",
    }
    .to_owned()
}

fn state_source(source: KnowledgeMapStateSource) -> String {
    match source {
        KnowledgeMapStateSource::Observed => "observed",
        KnowledgeMapStateSource::LatentPrediction => "latent_prediction",
        KnowledgeMapStateSource::InheritedPrior => "inherited_prior",
    }
    .to_owned()
}

fn gate_status(status: KnowledgeMapGateStatus) -> String {
    match status {
        KnowledgeMapGateStatus::Active => "active",
        KnowledgeMapGateStatus::Shadow => "shadow",
        KnowledgeMapGateStatus::Unfit => "unfit",
        KnowledgeMapGateStatus::PriorOnly => "prior_only",
    }
    .to_owned()
}

pub fn build_today_actions(assignments: Vec<TaskAssignment>) -> Vec<TodayAction> {
    let mut actions = assignments
        .into_iter()
        .take(3)
        .map(|assignment| TodayAction {
            id: format!("practice:{}", assignment.concept_id),
            kind: "practice".to_owned(),
            title: assignment.concept_name,
            detail: format!("{} · {}", assignment.move_name, assignment.task_type),
            route: Some("/practice".to_owned()),
            concept_id: Some(assignment.concept_id),
            expected_success: Some(assignment.expected_success),
        })
        .collect::<Vec<_>>();
    let fallbacks = [
        TodayAction {
            id: "fallback:evidence".to_owned(),
            kind: "evidence".to_owned(),
            title: "补一条学习证据".to_owned(),
            detail: "记下刚遇到的难点，不会直接算作掌握。".to_owned(),
            route: Some("/inbox".to_owned()),
            concept_id: None,
            expected_success: None,
        },
        TodayAction {
            id: "fallback:inbox".to_owned(),
            kind: "inbox".to_owned(),
            title: "处理学习收件箱".to_owned(),
            detail: "查看已记录的线索，选一条准备练习。".to_owned(),
            route: Some("/inbox".to_owned()),
            concept_id: None,
            expected_success: None,
        },
        TodayAction {
            id: "fallback:rest".to_owned(),
            kind: "rest".to_owned(),
            title: "休息一下".to_owned(),
            detail: "现在不练也可以，Polaris 会保留当前状态。".to_owned(),
            route: None,
            concept_id: None,
            expected_success: None,
        },
    ];
    for fallback in fallbacks {
        if actions.len() == 3 {
            break;
        }
        actions.push(fallback);
    }
    actions
}

pub fn notification_receipt(
    level: &str,
    policy: &NotificationPolicy,
) -> Result<NotificationReceipt, CommandError> {
    if !matches!(level, "info" | "error") {
        return Err(CommandError::state(format!(
            "unknown notification level: {level}"
        )));
    }
    let suppressed_by_flow = level != "error" && policy.suppress_non_error;
    Ok(NotificationReceipt {
        emitted: !suppressed_by_flow,
        suppressed_by_flow,
    })
}
