use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::config::meta_i64;
use crate::db::migrate;
use crate::engine::Engine;
use crate::error::{PolarisError, Result};
use crate::pack::{load_pack, validate_pack_path};
use crate::pack_state::ThetaMode;
use crate::simulation::{
    simulate_learning_quiet_under_env_guard, EarlyTransferViolation, ExternalModelEnvGuard,
    PhaseCounts, SimulationReport, VirtualLearner,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxOptions {
    pub pack_path: PathBuf,
    pub learner: SandboxLearner,
    pub days: usize,
}

impl SandboxOptions {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            pack_path: path.as_ref().to_path_buf(),
            learner: SandboxLearner::Mixed,
            days: 7,
        }
    }

    pub fn with_learner(mut self, learner: SandboxLearner) -> Self {
        self.learner = learner;
        self
    }

    pub fn with_days(mut self, days: usize) -> Self {
        self.days = days;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxLearner {
    Strong,
    Weak,
    Mixed,
}

impl SandboxLearner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Weak => "weak",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    Pass,
    Warn,
    Fail,
}

impl SandboxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SandboxReport {
    pub mode: &'static str,
    pub writes_user_db: bool,
    pub tier0_only: bool,
    pub llm_used: bool,
    pub score_source: &'static str,
    pub pack_id: String,
    pub pack_title: String,
    #[serde(rename = "profile")]
    pub learner: SandboxLearner,
    pub days: usize,
    pub status: SandboxStatus,
    pub theta_mode: String,
    pub validation: SandboxValidationSummary,
    pub deadlock_days: Vec<usize>,
    pub initial_mean_p_known: f64,
    pub final_mean_p_known: f64,
    pub mean_p_known_slope: f64,
    pub initial_abs_calib_gap: f64,
    pub final_abs_calib_gap: f64,
    pub final_theta_cosine: f64,
    pub final_phase_counts: PhaseCounts,
    pub early_transfer_violations: Vec<SandboxEarlyTransferViolation>,
    pub hmm_state_lock: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SandboxValidationSummary {
    pub concept_count: usize,
    pub prerequisite_count: usize,
    pub misconception_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SandboxEarlyTransferViolation {
    pub concept_id: String,
    pub attempt_count: i64,
    pub phase: String,
}

pub fn run_pack_sandbox(options: SandboxOptions) -> Result<SandboxReport> {
    if options.days == 0 {
        return Err(PolarisError::InvalidParameter {
            key: "sandbox.days".to_owned(),
            value: "must be >= 1".to_owned(),
        });
    }

    let validation = validate_pack_path(&options.pack_path)?;
    let pack = load_pack(&options.pack_path)?;
    let _env_guard = ExternalModelEnvGuard::sandbox();

    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    let mut engine = Engine::new(conn);
    engine.init_pack(&options.pack_path)?;
    let receipt = engine.switch_pack(&pack.id, Some(ThetaMode::Isolated))?;

    let latent_k = meta_i64(engine.conn(), "latent.k")? as usize;
    let learner = match options.learner {
        SandboxLearner::Strong => VirtualLearner::strong(latent_k),
        SandboxLearner::Weak => VirtualLearner::weak(latent_k),
        SandboxLearner::Mixed => VirtualLearner::mixed(latent_k),
    };
    let report = simulate_learning_quiet_under_env_guard(&learner, options.days, &mut engine)?;
    let status = sandbox_status(&report);
    let summary = sandbox_summary(status, &report);
    let hmm_state_lock = report.has_hmm_state_lock();
    let early_transfer_violations =
        sandbox_early_transfer_violations(&report.early_transfer_violations);

    Ok(SandboxReport {
        mode: "sandbox",
        writes_user_db: false,
        tier0_only: true,
        llm_used: false,
        score_source: "virtual_learner",
        pack_id: pack.id,
        pack_title: pack.title,
        learner: options.learner,
        days: options.days,
        status,
        theta_mode: receipt.theta_mode,
        validation: SandboxValidationSummary {
            concept_count: validation.concept_count,
            prerequisite_count: validation.prerequisite_count,
            misconception_count: validation.misconception_count,
        },
        deadlock_days: report.deadlock_days,
        initial_mean_p_known: report.initial_mean_p_known,
        final_mean_p_known: report.final_mean_p_known,
        mean_p_known_slope: report.mean_p_known_slope,
        initial_abs_calib_gap: report.initial_abs_calib_gap,
        final_abs_calib_gap: report.final_abs_calib_gap,
        final_theta_cosine: report.final_theta_cosine,
        final_phase_counts: report.final_phase_counts,
        early_transfer_violations,
        hmm_state_lock,
        summary,
    })
}

fn sandbox_status(report: &SimulationReport) -> SandboxStatus {
    if !report.deadlock_days.is_empty()
        || report.has_hmm_state_lock()
        || !report.early_transfer_violations.is_empty()
    {
        return SandboxStatus::Fail;
    }
    if report.final_mean_p_known < report.initial_mean_p_known || report.mean_p_known_slope <= 0.0 {
        return SandboxStatus::Warn;
    }
    SandboxStatus::Pass
}

fn sandbox_summary(status: SandboxStatus, report: &SimulationReport) -> String {
    match status {
        SandboxStatus::Pass => "sandbox closed loop improved without deadlock".to_owned(),
        SandboxStatus::Warn => format!(
            "sandbox completed but improvement was weak: initial_mean_p_known={:.3}, final_mean_p_known={:.3}",
            report.initial_mean_p_known, report.final_mean_p_known
        ),
        SandboxStatus::Fail => "sandbox found deadlock, HMM lock, or early transfer violation".to_owned(),
    }
}

fn sandbox_early_transfer_violations(
    violations: &[EarlyTransferViolation],
) -> Vec<SandboxEarlyTransferViolation> {
    violations
        .iter()
        .map(|violation| SandboxEarlyTransferViolation {
            concept_id: violation.concept_id.clone(),
            attempt_count: violation.attempt_count,
            phase: violation.phase.as_str().to_owned(),
        })
        .collect()
}
