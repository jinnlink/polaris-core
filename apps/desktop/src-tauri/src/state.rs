use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use polaris_core::db::open_database;
use polaris_core::engine::{Engine, TaskAssignment};
use polaris_core::notification::NotificationPolicy;
use polaris_core::pack_state::{PackSwitchReceipt, ThetaMode};
use polaris_core::status::StatusSnapshot;

use crate::contracts::{
    CommandError, NotificationReceipt, TodayAction, TodaySignal, TodaySnapshot,
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
