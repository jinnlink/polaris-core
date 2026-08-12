use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use polaris_core::ai_profile::{AiInteractionProfile, AiInteractionProfileInput};
use polaris_core::capture_queue::{CaptureInput, CaptureStatus, LearnerCaptureKind};
use polaris_core::db::open_database;
use polaris_core::engine::{Engine, TaskAssignment};
use polaris_core::goals::{
    GoalDimensionInput as CoreGoalDimensionInput, GoalInput as CoreGoalInput,
    GoalMilestoneInput as CoreGoalMilestoneInput, GoalProgressReport, GoalRecord,
    GoalScope as CoreGoalScope,
};
use polaris_core::inbox_practice::InboxPracticeSubmissionInput;
use polaris_core::knowledge_map::{
    KnowledgeMapDueStatus, KnowledgeMapGateStatus, KnowledgeMapQuery, KnowledgeMapScope,
    KnowledgeMapSnapshot, KnowledgeMapStateSource,
};
use polaris_core::learner_inbox::LearnerInboxAction;
use polaris_core::notification::NotificationPolicy;
use polaris_core::ops::ActivitySummary;
use polaris_core::pack_state::{PackSwitchReceipt, ThetaMode};
use polaris_core::prediction_map::{PredictionEstimate, PredictionMapSnapshot};
use polaris_core::privacy::PrivacyCallInventory;
use polaris_core::profile::{
    FullDataDeletionRequest, ProfileGateStatus, ProfileMeasurementInput, ProfileSettings,
    ProfileSettingsUpdate, DELETE_ALL_CONFIRMATION,
};
use polaris_core::report::{MirrorReport, ReportItem};
use polaris_core::status::StatusSnapshot;
use polaris_core::trust::{TrustPanel, TrustParameter};

use crate::background::{BackgroundEvent, BackgroundJob, BackgroundJobResult, SerialWorker};
use crate::contracts::{
    AiInteractionProfileUpdate, AiInteractionProfileView, AttemptGradeStatus, BackgroundEventView,
    CaptureWorkspaceInput, CaptureWorkspaceReceipt, CommandError, FullDeleteInput,
    FullDeleteReceiptView, FullDeleteScopePreview, GoalDimensionView, GoalEditorInput,
    GoalMilestoneView, GoalMutationReceipt, GoalScopeInput, GoalView, GoalWorkspaceSnapshot,
    GradeQueueReceipt, InboxActionInput, InboxActionOption, InboxActionReceipt, InboxPracticeDraft,
    InboxPracticeSubmitInput, InboxPracticeSubmitReceipt, InboxWorkspaceItem, InboxWorkspaceQuery,
    LifecycleSnapshot, MapWorkspaceAggregate, MapWorkspaceAnchor, MapWorkspaceEdge,
    MapWorkspaceLayer, MapWorkspaceNode, MapWorkspacePath, MapWorkspaceQuery, MapWorkspaceSnapshot,
    MirrorCurvePoint, MirrorPhaseItem, MirrorReportView, NotificationReceipt, PracticeSubmitInput,
    PracticeSubmitReceipt, PracticeTask, PracticeWorkspaceSnapshot, PrivacyCallView,
    ProfileBehaviorFact, ProfileDimensionView, ProfileExportInput, ProfileInstrumentItemView,
    ProfileInstrumentView, ProfileMeasurementSubmitInput, ProfileSettingsUpdateInput,
    ProfileSettingsView, ProfileWorkspaceSnapshot, ReportCitationView, ReportFeedbackInput,
    ReportItemView, ReportMutationReceipt, ReportNarrativeView, ReportSkippedView,
    ReportsWorkspaceSnapshot, SettingsMutationReceipt, SettingsWorkspaceSnapshot, TodayAction,
    TodaySignal, TodaySnapshot, TrustActivityView, TrustExperimentView, TrustGateView,
    TrustParameterView, TrustWorkspaceSnapshot, WorkbenchAction,
};
use crate::lifecycle::{
    append_redacted_log, backup_database_to, begin_run, export_diagnostic_bundle, finish_run,
    load_config_recovering, load_pending_jobs, prepare_database_for_open, resolve_database_path,
    save_config, save_pending_jobs, CrashMarkerReceipt, DatabasePathSource, DatabasePreparation,
    DatabaseResolution, DesktopConfig, StartupDatabaseState,
};

pub struct DesktopState {
    engine: Arc<Mutex<Option<Engine>>>,
    database_path: Arc<Mutex<PathBuf>>,
    worker: SerialWorker,
    app_data_dir: PathBuf,
    config: Mutex<DesktopConfig>,
    resolution: Mutex<DatabaseResolution>,
    startup_state: Mutex<StartupDatabaseState>,
    pre_upgrade_backup: Mutex<Option<PathBuf>>,
    crash_marker: Option<CrashMarkerReceipt>,
    pending_jobs: Arc<Mutex<Vec<String>>>,
    recovered_background_jobs: Vec<String>,
    config_warning: Option<String>,
    manage_platform_credentials: bool,
}

fn start_worker(
    engine: Arc<Mutex<Option<Engine>>>,
    database_path: Arc<Mutex<PathBuf>>,
    app_data_dir: PathBuf,
    pending_jobs: Arc<Mutex<Vec<String>>>,
) -> SerialWorker {
    SerialWorker::start(move |job| {
        let result = (|| {
            if job == BackgroundJob::Backup {
                let path = database_path
                    .lock()
                    .map_err(|_| "数据库路径状态不可用".to_owned())?
                    .clone();
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| error.to_string())?
                    .as_secs();
                let backup = path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("backups")
                    .join(format!("polaris-{timestamp}.sqlite"));
                backup_database_to(&path, &backup)?;
                return Ok(BackgroundJobResult {
                    invalidates: vec!["settings".to_owned()],
                    message: format!("一致性备份已写入 {}", backup.display()),
                });
            }
            let mut slot = engine
                .lock()
                .map_err(|_| "Polaris 引擎状态锁不可用".to_owned())?;
            let engine = slot
                .as_mut()
                .ok_or_else(|| "数据库尚未就绪，后台任务已跳过".to_owned())?;
            let (invalidates, message) = match job {
                BackgroundJob::GradeQueue => {
                    let summary = engine.grade_pending().map_err(|error| error.to_string())?;
                    (
                        vec!["practice", "today", "map", "reports"],
                        format!(
                            "后台评分已处理 {} 条，剩余 {} 条",
                            summary.processed, summary.pending
                        ),
                    )
                }
                BackgroundJob::MirrorReport => {
                    engine
                        .run_mirror_report()
                        .map_err(|error| error.to_string())?;
                    (
                        vec!["reports", "today", "trust"],
                        "镜像报告已更新".to_owned(),
                    )
                }
                BackgroundJob::NightlyConsolidation => {
                    engine
                        .run_nightly_consolidation()
                        .map_err(|error| error.to_string())?;
                    (vec!["map", "trust"], "夜间巩固已完成".to_owned())
                }
                BackgroundJob::MentalDynamicsFit => {
                    engine
                        .run_mental_dynamics_fit()
                        .map_err(|error| error.to_string())?;
                    (vec!["trust", "reports"], "心智动力学拟合已完成".to_owned())
                }
                BackgroundJob::ParameterTuning => {
                    engine
                        .run_param_tuning()
                        .map_err(|error| error.to_string())?;
                    (vec!["trust", "today"], "参数重放调优已完成".to_owned())
                }
                BackgroundJob::FsrsFit => {
                    engine
                        .fit_fsrs_personal_params()
                        .map_err(|error| error.to_string())?;
                    (
                        vec!["today", "map", "trust"],
                        "FSRS 个人参数拟合已完成".to_owned(),
                    )
                }
                BackgroundJob::Backup => unreachable!(),
            };
            Ok(BackgroundJobResult {
                invalidates: invalidates.into_iter().map(str::to_owned).collect(),
                message,
            })
        })();
        if !app_data_dir.as_os_str().is_empty() {
            if let Ok(mut pending) = pending_jobs.lock() {
                if let Some(index) = pending.iter().position(|id| id == job.id()) {
                    pending.remove(index);
                }
                let _ = save_pending_jobs(&app_data_dir, &pending);
            }
            let outcome = if result.is_ok() { "finished" } else { "failed" };
            let _ = append_redacted_log(
                &app_data_dir,
                &format!("background {} {outcome}", job.id()),
                &[],
                1_048_576,
            );
        }
        result
    })
}

struct EngineGuard<'a>(MutexGuard<'a, Option<Engine>>);

fn startup_view(state: &StartupDatabaseState) -> (String, String, Option<i32>, bool) {
    match state {
        StartupDatabaseState::Missing => (
            "ready".to_owned(),
            "将于此路径建立新的本地数据库。".to_owned(),
            Some(i32::try_from(polaris_core::db::CURRENT_SCHEMA_VERSION).unwrap_or(i32::MAX)),
            false,
        ),
        StartupDatabaseState::Ready {
            user_version,
            upgrade_required,
        } => (
            "ready".to_owned(),
            if *upgrade_required {
                "已有数据库将在一致性备份后升级。".to_owned()
            } else {
                "数据库完整且版本受支持。".to_owned()
            },
            Some(i32::try_from(*user_version).unwrap_or(i32::MAX)),
            *upgrade_required,
        ),
        StartupDatabaseState::Locked { message } => (
            "locked".to_owned(),
            format!("数据库正被其他进程锁定：{message}"),
            None,
            false,
        ),
        StartupDatabaseState::Corrupt { message } => (
            "corrupt".to_owned(),
            format!("数据库完整性检查未通过：{message}"),
            None,
            false,
        ),
        StartupDatabaseState::Newer { found, supported } => (
            "newer".to_owned(),
            format!("数据库版本 {found} 高于当前程序支持的 {supported}，已保持只读保护。"),
            Some(i32::try_from(*found).unwrap_or(i32::MAX)),
            false,
        ),
        StartupDatabaseState::Unavailable { message } => (
            "unavailable".to_owned(),
            format!("数据库暂时不可用：{message}"),
            None,
            false,
        ),
    }
}

fn parse_background_job(value: &str) -> Result<BackgroundJob, CommandError> {
    match value {
        "grade_queue" => Ok(BackgroundJob::GradeQueue),
        "mirror_report" => Ok(BackgroundJob::MirrorReport),
        "nightly_consolidation" => Ok(BackgroundJob::NightlyConsolidation),
        "mental_dynamics_fit" => Ok(BackgroundJob::MentalDynamicsFit),
        "parameter_tuning" => Ok(BackgroundJob::ParameterTuning),
        "fsrs_fit" => Ok(BackgroundJob::FsrsFit),
        "backup" => Ok(BackgroundJob::Backup),
        other => Err(CommandError::state(format!("未知后台任务：{other}"))),
    }
}

fn background_event_view(event: BackgroundEvent) -> BackgroundEventView {
    match event {
        BackgroundEvent::Started(job) => BackgroundEventView {
            job: Some(job.id().to_owned()),
            status: "started".to_owned(),
            invalidates: Vec::new(),
            message: format!("{} 已开始", job.id()),
        },
        BackgroundEvent::Finished {
            job,
            invalidates,
            message,
        } => BackgroundEventView {
            job: Some(job.id().to_owned()),
            status: "finished".to_owned(),
            invalidates,
            message,
        },
        BackgroundEvent::Failed { job, message } => BackgroundEventView {
            job: Some(job.id().to_owned()),
            status: "failed".to_owned(),
            invalidates: Vec::new(),
            message,
        },
        BackgroundEvent::Cancelled(job) => BackgroundEventView {
            job: Some(job.id().to_owned()),
            status: "cancelled".to_owned(),
            invalidates: Vec::new(),
            message: format!("{} 已在安全边界取消", job.id()),
        },
        BackgroundEvent::Stopped => BackgroundEventView {
            job: None,
            status: "stopped".to_owned(),
            invalidates: Vec::new(),
            message: "后台 worker 已停止".to_owned(),
        },
    }
}

fn credential_target(slot: &str) -> Result<&'static str, CommandError> {
    match slot {
        "fast" => Ok("Polaris/LLM/Fast"),
        "strong" => Ok("Polaris/LLM/Strong"),
        "embed" => Ok("Polaris/Embedding"),
        other => Err(CommandError::state(format!("未知凭据槽位：{other}"))),
    }
}

fn credential_environment(slot: &str) -> Result<&'static str, CommandError> {
    match slot {
        "fast" => Ok("POLARIS_LLM_FAST_API_KEY"),
        "strong" => Ok("POLARIS_LLM_STRONG_API_KEY"),
        "embed" => Ok("POLARIS_EMBED_API_KEY"),
        other => Err(CommandError::state(format!("未知凭据槽位：{other}"))),
    }
}

#[cfg(windows)]
fn credential_configured(slot: &str) -> Result<bool, CommandError> {
    let target = credential_target(slot)?;
    crate::lifecycle::windows::read_credential(target)
        .map(|value| value.is_some())
        .map_err(CommandError::state)
}

#[cfg(not(windows))]
fn credential_configured(slot: &str) -> Result<bool, CommandError> {
    let _ = credential_target(slot)?;
    Ok(false)
}

#[cfg(windows)]
fn platform_write_credential(slot: &str, secret: &str) -> Result<(), CommandError> {
    crate::lifecycle::windows::write_credential(credential_target(slot)?, "Polaris", secret)
        .map_err(CommandError::state)?;
    std::env::set_var(credential_environment(slot)?, secret);
    Ok(())
}

#[cfg(not(windows))]
fn platform_write_credential(slot: &str, _secret: &str) -> Result<(), CommandError> {
    let _ = credential_target(slot)?;
    Err(CommandError::state(
        "当前平台不支持 Windows Credential Manager",
    ))
}

#[cfg(windows)]
fn platform_delete_credential(slot: &str) -> Result<(), CommandError> {
    crate::lifecycle::windows::delete_credential(credential_target(slot)?)
        .map(|_| ())
        .map_err(CommandError::state)?;
    std::env::remove_var(credential_environment(slot)?);
    Ok(())
}

#[cfg(not(windows))]
fn platform_delete_credential(slot: &str) -> Result<(), CommandError> {
    let _ = credential_target(slot)?;
    Ok(())
}

#[cfg(windows)]
fn platform_load_credentials() -> Result<(), CommandError> {
    for slot in ["fast", "strong", "embed"] {
        if let Some(secret) = crate::lifecycle::windows::read_credential(credential_target(slot)?)
            .map_err(CommandError::state)?
        {
            std::env::set_var(credential_environment(slot)?, secret);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn platform_load_credentials() -> Result<(), CommandError> {
    Ok(())
}

fn platform_delete_all_credentials() -> polaris_core::error::Result<usize> {
    let mut deleted = 0;
    for slot in ["fast", "strong", "embed"] {
        #[cfg(windows)]
        {
            if crate::lifecycle::windows::delete_credential(
                credential_target(slot).map_err(|error| std::io::Error::other(error.message))?,
            )
            .map_err(std::io::Error::other)?
            {
                deleted += 1;
            }
            std::env::remove_var(
                credential_environment(slot)
                    .map_err(|error| std::io::Error::other(error.message))?,
            );
        }
        #[cfg(not(windows))]
        {
            let _ = slot;
        }
    }
    Ok(deleted)
}

#[cfg(windows)]
fn platform_startup_enabled() -> Result<bool, CommandError> {
    crate::lifecycle::windows::startup_enabled().map_err(CommandError::state)
}

#[cfg(not(windows))]
fn platform_startup_enabled() -> Result<bool, CommandError> {
    Ok(false)
}

#[cfg(windows)]
fn platform_set_startup(enabled: bool) -> Result<(), CommandError> {
    let executable = std::env::current_exe().map_err(CommandError::core)?;
    crate::lifecycle::windows::set_startup_enabled(enabled, &executable.display().to_string())
        .map_err(CommandError::state)
}

#[cfg(not(windows))]
fn platform_set_startup(enabled: bool) -> Result<(), CommandError> {
    if enabled {
        Err(CommandError::state("当前平台不支持 Windows 开机启动"))
    } else {
        Ok(())
    }
}

impl Deref for EngineGuard<'_> {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("EngineGuard only wraps Some")
    }
}

impl DerefMut for EngineGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("EngineGuard only wraps Some")
    }
}

impl DesktopState {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CommandError> {
        let database_path = std::path::absolute(path).map_err(CommandError::core)?;
        let connection = open_database(&database_path).map_err(CommandError::core)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(CommandError::core)?;
        let engine = Arc::new(Mutex::new(Some(Engine::new(connection))));
        let database_path = Arc::new(Mutex::new(database_path));
        let pending_jobs = Arc::new(Mutex::new(Vec::new()));
        let worker = start_worker(
            Arc::clone(&engine),
            Arc::clone(&database_path),
            PathBuf::new(),
            Arc::clone(&pending_jobs),
        );
        let resolution = DatabaseResolution {
            path: database_path
                .lock()
                .map_err(|_| CommandError::state("数据库路径状态不可用"))?
                .clone(),
            source: DatabasePathSource::Saved,
            needs_acknowledgement: false,
        };
        Ok(Self {
            engine,
            database_path,
            worker,
            app_data_dir: PathBuf::new(),
            config: Mutex::new(DesktopConfig {
                database_path: Some(resolution.path.clone()),
                database_path_acknowledged: true,
                startup_enabled: false,
            }),
            resolution: Mutex::new(resolution),
            startup_state: Mutex::new(StartupDatabaseState::Ready {
                user_version: polaris_core::db::CURRENT_SCHEMA_VERSION,
                upgrade_required: false,
            }),
            pre_upgrade_backup: Mutex::new(None),
            crash_marker: None,
            pending_jobs,
            recovered_background_jobs: Vec::new(),
            config_warning: None,
            manage_platform_credentials: false,
        })
    }

    pub fn bootstrap(app_data_dir: impl AsRef<Path>) -> Result<Self, CommandError> {
        let app_data_dir = std::path::absolute(app_data_dir).map_err(CommandError::core)?;
        std::fs::create_dir_all(&app_data_dir).map_err(CommandError::core)?;
        let (config, quarantined_config) =
            load_config_recovering(&app_data_dir).map_err(CommandError::state)?;
        platform_load_credentials()?;
        let environment_path = std::env::var_os("POLARIS_CORE_DB").map(PathBuf::from);
        let resolution = resolve_database_path(&config, environment_path.as_deref(), &app_data_dir)
            .map_err(CommandError::state)?;
        let preparation =
            prepare_database_for_open(&resolution.path, &app_data_dir.join("backups"))
                .map_err(CommandError::state)?;
        let crash_marker = begin_run(&app_data_dir).map_err(CommandError::state)?;
        Self::from_bootstrap(
            app_data_dir,
            config,
            resolution,
            preparation,
            crash_marker,
            quarantined_config.map(|path| {
                format!(
                    "损坏的桌面配置已隔离到 {}，已使用安全默认值启动。",
                    path.display()
                )
            }),
        )
    }

    fn from_bootstrap(
        app_data_dir: PathBuf,
        config: DesktopConfig,
        resolution: DatabaseResolution,
        preparation: DatabasePreparation,
        crash_marker: CrashMarkerReceipt,
        config_warning: Option<String>,
    ) -> Result<Self, CommandError> {
        let engine_ready = matches!(
            preparation.state,
            StartupDatabaseState::Missing | StartupDatabaseState::Ready { .. }
        );
        let engine = if engine_ready {
            let connection = open_database(&resolution.path).map_err(CommandError::core)?;
            connection
                .busy_timeout(std::time::Duration::from_secs(5))
                .map_err(CommandError::core)?;
            Some(Engine::new(connection))
        } else {
            None
        };
        let engine = Arc::new(Mutex::new(engine));
        let database_path = Arc::new(Mutex::new(resolution.path.clone()));
        let recovered_background_jobs = load_pending_jobs(&app_data_dir)
            .map_err(CommandError::state)?
            .into_iter()
            .filter(|id| parse_background_job(id).is_ok())
            .collect::<Vec<_>>();
        save_pending_jobs(&app_data_dir, &recovered_background_jobs)
            .map_err(CommandError::state)?;
        let pending_jobs = Arc::new(Mutex::new(recovered_background_jobs.clone()));
        let worker = start_worker(
            Arc::clone(&engine),
            Arc::clone(&database_path),
            app_data_dir.clone(),
            Arc::clone(&pending_jobs),
        );
        let state = Self {
            engine,
            database_path,
            worker,
            app_data_dir,
            config: Mutex::new(config),
            resolution: Mutex::new(resolution),
            startup_state: Mutex::new(preparation.state),
            pre_upgrade_backup: Mutex::new(preparation.pre_upgrade_backup),
            crash_marker: Some(crash_marker),
            pending_jobs,
            recovered_background_jobs: recovered_background_jobs.clone(),
            config_warning,
            manage_platform_credentials: true,
        };
        if engine_ready {
            for id in recovered_background_jobs {
                state
                    .worker
                    .enqueue(parse_background_job(&id)?)
                    .map_err(CommandError::state)?;
            }
        }
        Ok(state)
    }

    fn engine(&self) -> Result<EngineGuard<'_>, CommandError> {
        let engine = self
            .engine
            .lock()
            .map_err(|_| CommandError::state("Polaris 引擎暂时不可用，请重试"))?;
        if engine.is_none() {
            return Err(CommandError::state(
                "数据库尚未就绪，请先在恢复面板检查或改选路径",
            ));
        }
        Ok(EngineGuard(engine))
    }

    fn database_path(&self) -> Result<PathBuf, CommandError> {
        self.database_path
            .lock()
            .map(|path| path.clone())
            .map_err(|_| CommandError::state("数据库路径状态暂时不可用"))
    }

    pub fn enqueue_background_job(&self, job: BackgroundJob) -> Result<(), CommandError> {
        if !self.app_data_dir.as_os_str().is_empty() {
            let mut pending = self
                .pending_jobs
                .lock()
                .map_err(|_| CommandError::state("后台任务恢复状态不可用"))?;
            pending.push(job.id().to_owned());
            save_pending_jobs(&self.app_data_dir, &pending).map_err(CommandError::state)?;
            if let Err(error) = self.worker.enqueue(job) {
                pending.pop();
                let _ = save_pending_jobs(&self.app_data_dir, &pending);
                return Err(CommandError::state(error));
            }
            Ok(())
        } else {
            self.worker.enqueue(job).map_err(CommandError::state)
        }
    }

    pub fn take_background_events(&self) -> Vec<BackgroundEvent> {
        self.worker.take_events()
    }

    pub fn shutdown(&self, drain: bool) -> Result<(), CommandError> {
        if drain {
            self.worker.drain_and_stop()
        } else {
            self.worker.cancel_and_stop()
        }
        .map_err(CommandError::state)?;
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| CommandError::state("关闭引擎时状态锁不可用"))?;
        *engine = None;
        if let Some(marker) = &self.crash_marker {
            finish_run(&marker.marker_path).map_err(CommandError::state)?;
        }
        Ok(())
    }

    pub fn lifecycle_snapshot(&self) -> Result<LifecycleSnapshot, CommandError> {
        let resolution = self
            .resolution
            .lock()
            .map_err(|_| CommandError::state("数据库解析状态不可用"))?
            .clone();
        let startup = self
            .startup_state
            .lock()
            .map_err(|_| CommandError::state("数据库启动状态不可用"))?
            .clone();
        let (startup_status, startup_message, schema_version, upgrade_required) =
            startup_view(&startup);
        let startup_enabled = platform_startup_enabled()?;
        Ok(LifecycleSnapshot {
            database_path: resolution.path.display().to_string(),
            database_source: match resolution.source {
                DatabasePathSource::Saved => "saved",
                DatabasePathSource::Environment => "environment",
                DatabasePathSource::LocalAppData => "local_app_data",
            }
            .to_owned(),
            database_path_acknowledged: !resolution.needs_acknowledgement,
            startup_status,
            startup_message,
            schema_version,
            upgrade_required,
            pre_upgrade_backup: self
                .pre_upgrade_backup
                .lock()
                .map_err(|_| CommandError::state("升级备份状态不可用"))?
                .as_ref()
                .map(|path| path.display().to_string()),
            previous_run_incomplete: self
                .crash_marker
                .as_ref()
                .is_some_and(|marker| marker.previous_run_incomplete),
            recovered_background_jobs: self.recovered_background_jobs.clone(),
            pending_background_jobs: self
                .pending_jobs
                .lock()
                .map_err(|_| CommandError::state("后台任务恢复状态不可用"))?
                .clone(),
            config_warning: self.config_warning.clone(),
            startup_enabled,
            fast_api_key_configured: credential_configured("fast")?,
            strong_api_key_configured: credential_configured("strong")?,
            embed_api_key_configured: credential_configured("embed")?,
        })
    }

    pub fn acknowledge_database_path(&self) -> Result<(), CommandError> {
        let path = self.database_path()?;
        let mut config = self
            .config
            .lock()
            .map_err(|_| CommandError::state("桌面配置状态不可用"))?;
        config.database_path = Some(path);
        config.database_path_acknowledged = true;
        save_config(&self.app_data_dir, &config).map_err(CommandError::state)?;
        self.resolution
            .lock()
            .map_err(|_| CommandError::state("数据库解析状态不可用"))?
            .needs_acknowledgement = false;
        Ok(())
    }

    pub fn select_database_path(&self, path: &str) -> Result<(), CommandError> {
        let was_recovery = self
            .engine
            .lock()
            .map_err(|_| CommandError::state("切换数据库时引擎状态不可用"))?
            .is_none();
        let requested = std::path::absolute(path.trim()).map_err(CommandError::core)?;
        let preparation = prepare_database_for_open(&requested, &self.app_data_dir.join("backups"))
            .map_err(CommandError::state)?;
        if !matches!(
            preparation.state,
            StartupDatabaseState::Missing | StartupDatabaseState::Ready { .. }
        ) {
            let (_, message, _, _) = startup_view(&preparation.state);
            return Err(CommandError::state(format!(
                "没有切换数据库；所选文件不可用：{message}"
            )));
        }
        let connection = open_database(&requested).map_err(CommandError::core)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(CommandError::core)?;
        let new_engine = Engine::new(connection);
        let new_config = DesktopConfig {
            database_path: Some(requested.clone()),
            database_path_acknowledged: true,
            startup_enabled: platform_startup_enabled()?,
        };
        save_config(&self.app_data_dir, &new_config).map_err(CommandError::state)?;
        *self
            .engine
            .lock()
            .map_err(|_| CommandError::state("切换数据库时引擎状态不可用"))? = Some(new_engine);
        *self
            .database_path
            .lock()
            .map_err(|_| CommandError::state("切换数据库时路径状态不可用"))? = requested.clone();
        *self
            .config
            .lock()
            .map_err(|_| CommandError::state("切换数据库时配置状态不可用"))? = new_config;
        *self
            .resolution
            .lock()
            .map_err(|_| CommandError::state("切换数据库时解析状态不可用"))? = DatabaseResolution {
            path: requested,
            source: DatabasePathSource::Saved,
            needs_acknowledgement: false,
        };
        *self
            .startup_state
            .lock()
            .map_err(|_| CommandError::state("切换数据库时启动状态不可用"))? =
            StartupDatabaseState::Ready {
                user_version: polaris_core::db::CURRENT_SCHEMA_VERSION,
                upgrade_required: false,
            };
        *self
            .pre_upgrade_backup
            .lock()
            .map_err(|_| CommandError::state("升级备份状态不可用"))? =
            preparation.pre_upgrade_backup;
        if was_recovery {
            let pending = self
                .pending_jobs
                .lock()
                .map_err(|_| CommandError::state("后台任务恢复状态不可用"))?
                .clone();
            for id in pending {
                self.worker
                    .enqueue(parse_background_job(&id)?)
                    .map_err(CommandError::state)?;
            }
        }
        Ok(())
    }

    pub fn set_startup_enabled(&self, enabled: bool) -> Result<(), CommandError> {
        platform_set_startup(enabled)?;
        let mut config = self
            .config
            .lock()
            .map_err(|_| CommandError::state("桌面配置状态不可用"))?;
        config.startup_enabled = enabled;
        save_config(&self.app_data_dir, &config).map_err(CommandError::state)
    }

    pub fn save_api_key(&self, slot: &str, secret: &str) -> Result<(), CommandError> {
        if secret.trim().is_empty() {
            return Err(CommandError::state("API Key 不能为空"));
        }
        platform_write_credential(slot, secret.trim())
    }

    pub fn delete_api_key(&self, slot: &str) -> Result<(), CommandError> {
        platform_delete_credential(slot)
    }

    pub fn export_diagnostics(&self, output_path: &str) -> Result<(), CommandError> {
        let output_path = std::path::absolute(output_path.trim()).map_err(CommandError::core)?;
        let startup = self
            .startup_state
            .lock()
            .map_err(|_| CommandError::state("数据库启动状态不可用"))?
            .clone();
        let pending = self
            .pending_jobs
            .lock()
            .map_err(|_| CommandError::state("后台任务恢复状态不可用"))?
            .clone();
        let owned_secrets = [
            std::env::var("POLARIS_LLM_FAST_API_KEY").ok(),
            std::env::var("POLARIS_LLM_STRONG_API_KEY").ok(),
            std::env::var("POLARIS_EMBED_API_KEY").ok(),
        ];
        let secrets = owned_secrets
            .iter()
            .filter_map(Option::as_deref)
            .collect::<Vec<_>>();
        export_diagnostic_bundle(
            &self.app_data_dir,
            &output_path,
            &self.database_path()?,
            &startup,
            &pending,
            &secrets,
        )
        .map_err(CommandError::state)
    }

    pub fn enqueue_background_job_by_id(&self, job: &str) -> Result<(), CommandError> {
        let job = parse_background_job(job)?;
        self.enqueue_background_job(job)
    }

    pub fn background_events(&self) -> Vec<BackgroundEventView> {
        self.take_background_events()
            .into_iter()
            .map(background_event_view)
            .collect()
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

    pub fn profile_workspace(&self) -> Result<ProfileWorkspaceSnapshot, CommandError> {
        let engine = self.engine()?;
        let overview = engine
            .global_profile_overview()
            .map_err(CommandError::core)?;
        let behavior = engine
            .profile_behavior_snapshot()
            .map_err(CommandError::core)?;
        let facts = vec![
            profile_fact(
                "sessions",
                "有效会话",
                behavior.valid_session_count.to_string(),
                "只统计已完成且可用于行为估计的会话。",
            ),
            profile_fact(
                "calibration",
                "校准差",
                behavior
                    .calibration_mean_gap
                    .map(|value| format!("{value:+.2}"))
                    .unwrap_or_else(|| "暂无".to_owned()),
                "比较反馈前把握度与实际表现，不解释人格。",
            ),
            profile_fact(
                "move-effects",
                "教法观测",
                behavior.move_effect_observations.to_string(),
                "用于检验哪种教学行动在什么情境下有效。",
            ),
            profile_fact(
                "abandons",
                "中断记录",
                behavior.abandon_event_count.to_string(),
                "只描述行为事实，不推断意志力或性格。",
            ),
        ];
        let dimensions = overview
            .dimensions
            .into_iter()
            .map(|dimension| {
                let sigma = dimension.variance.max(0.0).sqrt();
                let gate_status = match dimension.gate_status {
                    ProfileGateStatus::Unfit => "unfit",
                    ProfileGateStatus::Shadow => "shadow",
                    ProfileGateStatus::Active => "active",
                    ProfileGateStatus::Suspended => "suspended",
                };
                let active = matches!(dimension.gate_status, ProfileGateStatus::Active);
                ProfileDimensionView {
                    label: profile_dimension_label(&dimension.dimension_key).to_owned(),
                    key: dimension.dimension_key,
                    mean: dimension.mean,
                    lower: (dimension.mean - 1.96 * sigma).clamp(0.0, 1.0),
                    upper: (dimension.mean + 1.96 * sigma).clamp(0.0, 1.0),
                    evidence_count: i32::try_from(dimension.evidence_count).unwrap_or(i32::MAX),
                    gate_status: gate_status.to_owned(),
                    gate_label: if active {
                        "已通过前瞻验证"
                    } else {
                        "尚未通过验证"
                    }
                    .to_owned(),
                    purpose: if active {
                        "仅作为策略与节律的慢先验，实际干预仍需通过 MRT。"
                    } else {
                        "当前只在影子模式检验预测增益。"
                    }
                    .to_owned(),
                    will_not_affect: if active {
                        "不会直接改写掌握度、评分或知识图谱。"
                    } else {
                        "不会参与调度、评分、掌握度或确定性解释。"
                    }
                    .to_owned(),
                    evidence_ids: dimension.evidence_ids,
                }
            })
            .collect();
        Ok(ProfileWorkspaceSnapshot {
            generated_at: overview.generated_at,
            settings: ProfileSettingsView {
                enabled: overview.settings.enabled,
                disclosure_required: overview.settings.disclosure_required,
                disclosure_acknowledged: overview.settings.disclosure_acknowledged_at.is_some(),
                summary_sharing_enabled: overview.settings.summary_sharing_enabled,
                paused_until: overview.settings.paused_until,
            },
            facts,
            dimensions,
            notice: overview.notice,
            actions: workbench_actions(&[
                ("settings", "管理画像", "primary"),
                ("goals", "查看目标", "secondary"),
                ("today", "返回 Today", "quiet"),
            ]),
        })
    }

    pub fn goals_workspace(
        &self,
        selected_goal_id: Option<&str>,
    ) -> Result<GoalWorkspaceSnapshot, CommandError> {
        let engine = self.engine()?;
        let goals = engine.list_goals(None).map_err(CommandError::core)?;
        let selected = selected_goal_id.map(str::to_owned).or_else(|| {
            goals
                .iter()
                .find(|goal| goal.status == "active")
                .map(|goal| goal.id.clone())
        });
        let selected_is_schedulable = selected.as_deref().is_some_and(|selected_id| {
            goals
                .iter()
                .any(|goal| goal.id == selected_id && goal.status == "active")
        });
        let workspace = engine
            .goal_workspace(if selected_is_schedulable {
                selected.as_deref()
            } else {
                None
            })
            .map_err(CommandError::core)?;
        let goal_views = goals
            .into_iter()
            .map(|goal| {
                let progress = workspace
                    .goal
                    .as_ref()
                    .filter(|item| item.id == goal.id)
                    .and(workspace.progress.as_ref());
                goal_view(goal, progress)
            })
            .collect();
        let actions = if selected.is_some() && !selected_is_schedulable {
            Vec::new()
        } else {
            build_today_actions(workspace.actions)
        };
        Ok(GoalWorkspaceSnapshot {
            generated_at: workspace.generated_at,
            goals: goal_views,
            selected_goal_id: selected,
            actions,
        })
    }

    pub fn save_goal(&self, input: GoalEditorInput) -> Result<GoalMutationReceipt, CommandError> {
        let goal_id = input.id.trim().to_owned();
        let engine = self.engine()?;
        let existing = engine.goal_snapshot(&goal_id).map_err(CommandError::core)?;
        let exists = existing.is_some();
        let core_input = core_goal_input(input, existing.as_ref())?;
        if exists {
            engine.update_goal(core_input).map_err(CommandError::core)?;
        } else {
            engine.create_goal(core_input).map_err(CommandError::core)?;
        }
        Ok(GoalMutationReceipt {
            goal_id,
            effect: if exists { "updated" } else { "created" }.to_owned(),
            message: if exists {
                "目标已更新。"
            } else {
                "目标已创建。"
            }
            .to_owned(),
        })
    }

    pub fn refresh_goal(&self, goal_id: &str) -> Result<GoalMutationReceipt, CommandError> {
        self.engine()?
            .refresh_goal_progress(goal_id)
            .map_err(CommandError::core)?;
        Ok(GoalMutationReceipt {
            goal_id: goal_id.to_owned(),
            effect: "refreshed".to_owned(),
            message: "已从掌握证据刷新目标进度。".to_owned(),
        })
    }

    pub fn archive_goal(&self, goal_id: &str) -> Result<GoalMutationReceipt, CommandError> {
        self.engine()?
            .archive_goal(goal_id)
            .map_err(CommandError::core)?;
        Ok(GoalMutationReceipt {
            goal_id: goal_id.to_owned(),
            effect: "archived".to_owned(),
            message: "目标已归档，历史进度仍保留。".to_owned(),
        })
    }

    pub fn delete_goal(&self, goal_id: &str) -> Result<GoalMutationReceipt, CommandError> {
        self.engine()?
            .delete_goal(goal_id)
            .map_err(CommandError::core)?;
        Ok(GoalMutationReceipt {
            goal_id: goal_id.to_owned(),
            effect: "deleted".to_owned(),
            message: "目标及其维度、里程碑已删除。".to_owned(),
        })
    }

    pub fn reports_workspace(&self) -> Result<ReportsWorkspaceSnapshot, CommandError> {
        let engine = self.engine()?;
        let mirror = engine
            .learner_mirror_snapshot()
            .map_err(CommandError::core)?;
        let report = engine
            .latest_mirror_report()
            .map_err(CommandError::core)?
            .map(report_view);
        Ok(ReportsWorkspaceSnapshot {
            generated_at: mirror.generated_at,
            confidence_curve: mirror
                .confidence_curve
                .into_iter()
                .map(|point| MirrorCurvePoint {
                    attempt_id: point.attempt_id,
                    concept_id: point.concept_id,
                    created_at: point.created_at,
                    confidence: point.confidence,
                    actual_score: point.actual_score,
                    is_final: point.is_final,
                })
                .collect(),
            phase_distribution: mirror
                .phase_distribution
                .into_iter()
                .map(|item| MirrorPhaseItem {
                    phase: item.phase,
                    label: item.label,
                    summary: item.summary,
                    count: bounded_i32(item.count),
                })
                .collect(),
            report,
        })
    }

    pub fn run_report(&self) -> Result<ReportMutationReceipt, CommandError> {
        let report = self
            .engine()?
            .run_mirror_report()
            .map_err(CommandError::core)?;
        Ok(ReportMutationReceipt {
            report_id: report.id,
            effect: "generated".to_owned(),
            message: "本周报告已从本地证据重新生成。".to_owned(),
        })
    }

    pub fn report_feedback(
        &self,
        input: ReportFeedbackInput,
    ) -> Result<ReportMutationReceipt, CommandError> {
        let report_id = self
            .engine()?
            .record_report_feedback_with_verdict(
                Some(input.report_id.as_str()),
                &input.assertion_id,
                &input.verdict,
            )
            .map_err(CommandError::core)?;
        Ok(ReportMutationReceipt {
            report_id,
            effect: format!("feedback_{}", input.verdict),
            message: "反馈已记录；不准会进入后续报告的抑制与校正。".to_owned(),
        })
    }

    pub fn trust_workspace(&self) -> Result<TrustWorkspaceSnapshot, CommandError> {
        let panel = self.engine()?.trust_panel().map_err(CommandError::core)?;
        Ok(trust_view(panel))
    }

    pub fn settings_workspace(&self) -> Result<SettingsWorkspaceSnapshot, CommandError> {
        let engine = self.engine()?;
        let overview = engine
            .global_profile_overview()
            .map_err(CommandError::core)?;
        let behavior = engine
            .profile_behavior_snapshot()
            .map_err(CommandError::core)?;
        let ai_profile = engine
            .ai_interaction_profile()
            .map_err(CommandError::core)?;
        let privacy = PrivacyCallInventory::all();
        Ok(SettingsWorkspaceSnapshot {
            generated_at: overview.generated_at,
            profile: profile_settings_view(overview.settings),
            ai_profile: ai_profile_view(ai_profile),
            tier0_only: privacy.tier0_only,
            privacy_calls: privacy
                .calls
                .into_iter()
                .map(|call| PrivacyCallView {
                    id: call.id.to_owned(),
                    tier: call.tier.to_owned(),
                    trigger: call.trigger.to_owned(),
                    data_sent: call
                        .data_sent
                        .iter()
                        .map(|item| (*item).to_owned())
                        .collect(),
                    degradation: call.degradation.to_owned(),
                    disabled_when_tier0_only: call.disabled_when_tier0_only,
                })
                .collect(),
            instruments: polaris_core::profile::profile_instruments()
                .map_err(CommandError::core)?
                .into_iter()
                .map(|instrument| ProfileInstrumentView {
                    id: instrument.id,
                    title: instrument.title,
                    version: instrument.version,
                    citation: instrument.citation,
                    source_url: instrument.source_url,
                    response_min: bounded_i32(instrument.scoring.response_min),
                    response_max: bounded_i32(instrument.scoring.response_max),
                    admin_modes: instrument.admin_modes,
                    interpretation_notice: instrument.interpretation_notice,
                    items: instrument
                        .items
                        .into_iter()
                        .map(|item| ProfileInstrumentItemView {
                            id: item.id,
                            dimension: item.dimension,
                            prompt: item.prompt,
                            keyed: item.keyed,
                        })
                        .collect(),
                })
                .collect(),
            profile_measurement_count: bounded_i32(overview.measurement_count),
            profile_dimension_count: i32::try_from(overview.dimensions.len()).unwrap_or(i32::MAX),
            valid_session_count: bounded_i32(behavior.valid_session_count),
        })
    }

    pub fn update_profile_settings(
        &self,
        input: ProfileSettingsUpdateInput,
    ) -> Result<SettingsMutationReceipt, CommandError> {
        self.engine()?
            .update_global_profile_settings(ProfileSettingsUpdate {
                enabled: input.enabled,
                acknowledge_disclosure: input.acknowledge_disclosure,
                summary_sharing_enabled: input.summary_sharing_enabled,
                paused_until: input.paused_until,
                clear_pause: input.clear_pause,
            })
            .map_err(CommandError::core)?;
        Ok(SettingsMutationReceipt {
            effect: "profile_settings_updated".to_owned(),
            message: "画像设置已保存；关闭画像会同时关闭摘要分享。".to_owned(),
        })
    }

    pub fn update_ai_profile(
        &self,
        input: AiInteractionProfileUpdate,
    ) -> Result<SettingsMutationReceipt, CommandError> {
        self.engine()?
            .update_ai_interaction_profile(AiInteractionProfileInput {
                persona: input.persona,
                verbosity: input.verbosity,
                explanation_depth: input.explanation_depth,
                proactivity: input.proactivity,
                intervention_frequency: input.intervention_frequency,
                correction_style: input.correction_style,
                custom_notes: input.custom_notes,
            })
            .map_err(CommandError::core)?;
        Ok(SettingsMutationReceipt {
            effect: "ai_profile_updated".to_owned(),
            message: "AI 互动偏好已保存；它只改变表达与介入方式。".to_owned(),
        })
    }

    pub fn submit_profile_measurement(
        &self,
        input: ProfileMeasurementSubmitInput,
    ) -> Result<SettingsMutationReceipt, CommandError> {
        let receipt = self
            .engine()?
            .record_profile_measurement(ProfileMeasurementInput {
                session_id: input.session_id,
                instrument_id: input.instrument_id,
                instrument_version: input.instrument_version,
                item_id: input.item_id,
                locale: input.locale,
                admin_mode: input.admin_mode,
                response: i64::from(input.response),
            })
            .map_err(CommandError::core)?;
        Ok(SettingsMutationReceipt {
            effect: format!("profile_measurement_{:?}", receipt.status).to_ascii_lowercase(),
            message: receipt.message,
        })
    }

    pub fn reset_profile(&self) -> Result<SettingsMutationReceipt, CommandError> {
        let receipt = self
            .engine()?
            .reset_global_profile()
            .map_err(CommandError::core)?;
        Ok(SettingsMutationReceipt {
            effect: "profile_reset".to_owned(),
            message: format!(
                "已删除 {} 条画像测量、{} 个画像维度；{} 条学习尝试保持不变。",
                receipt.measurements_deleted,
                receipt.dimensions_deleted,
                receipt.learning_attempts_preserved
            ),
        })
    }

    pub fn export_profile(
        &self,
        input: ProfileExportInput,
    ) -> Result<SettingsMutationReceipt, CommandError> {
        let output_path =
            std::path::absolute(input.output_path.trim()).map_err(CommandError::core)?;
        if output_path.exists() {
            return Err(CommandError::core("导出目标已存在，请选择新的文件名"));
        }
        let export = self
            .engine()?
            .export_global_profile()
            .map_err(CommandError::core)?;
        let payload = serde_json::to_vec_pretty(&export).map_err(CommandError::core)?;
        std::fs::write(&output_path, payload).map_err(CommandError::core)?;
        Ok(SettingsMutationReceipt {
            effect: "profile_exported".to_owned(),
            message: format!(
                "画像导出已写入 {}。此文件包含原始量表回答。",
                output_path.display()
            ),
        })
    }

    pub fn full_delete_scope(&self) -> Result<FullDeleteScopePreview, CommandError> {
        let database_path = self.database_path()?;
        let connection = open_database(&database_path).map_err(CommandError::core)?;
        let count = |table: &str, condition: &str| -> Result<i32, CommandError> {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE {condition}");
            let value: i64 = connection
                .query_row(&sql, [], |row| row.get(0))
                .map_err(CommandError::core)?;
            Ok(bounded_i32(value))
        };
        let mut sqlite_files = Vec::new();
        for path in sqlite_family_paths(&database_path) {
            if path.is_file() {
                sqlite_files.push(path.display().to_string());
            }
        }
        Ok(FullDeleteScopePreview {
            database_path: database_path.display().to_string(),
            learning_attempts: count("attempts", "1=1")?,
            evidence_records: count("evidence_items", "1=1")?,
            goals: count("goals", "1=1")?,
            profile_measurements: count("behavior_events", "type='profile_measurement'")?,
            reports: count("mirror_reports", "1=1")?,
            behavior_events: count("behavior_events", "1=1")?,
            sqlite_files,
            confirmation_phrase: DELETE_ALL_CONFIRMATION.to_owned(),
            backup_supported: true,
        })
    }

    pub fn delete_all_data(
        &self,
        input: FullDeleteInput,
    ) -> Result<FullDeleteReceiptView, CommandError> {
        let backup_path = input
            .backup_path
            .filter(|value| !value.trim().is_empty())
            .map(|value| std::path::absolute(value.trim()))
            .transpose()
            .map_err(CommandError::core)?;
        let database_path = self.database_path()?;
        let placeholder_path = database_path.with_extension("delete-placeholder.sqlite");
        if sqlite_family_paths(&placeholder_path)
            .iter()
            .any(|path| path.exists())
        {
            return Err(CommandError::state(
                "上次清除留下了占位数据库，请重启应用后重试",
            ));
        }
        let placeholder = open_database(&placeholder_path).map_err(CommandError::core)?;
        let mut engine = self.engine()?;
        let active_engine = std::mem::replace(&mut *engine, Engine::new(placeholder));
        drop(active_engine);
        let delete_credentials = || {
            if self.manage_platform_credentials {
                platform_delete_all_credentials()
            } else {
                Ok(0)
            }
        };
        let result = polaris_core::profile::delete_all_learning_data(
            FullDataDeletionRequest {
                database_path: database_path.clone(),
                backup_path,
                confirmation: input.confirmation,
            },
            delete_credentials,
        );
        let reopened = open_database(&database_path).map(Engine::new);
        let outcome = match (result, reopened) {
            (Ok(receipt), Ok(fresh_engine)) => {
                *engine = fresh_engine;
                Ok(FullDeleteReceiptView {
                    deleted_at: receipt.deleted_at,
                    database_path: receipt.database_path,
                    backup_path: receipt.backup_path,
                    files_deleted: i32::try_from(receipt.files_deleted).unwrap_or(i32::MAX),
                    local_secrets_deleted: i32::try_from(receipt.local_secrets_deleted)
                        .unwrap_or(i32::MAX),
                    empty_database_created: receipt.empty_database_created,
                    message: "全部本地学习数据已清除，空数据库已建立。".to_owned(),
                })
            }
            (Err(error), Ok(restored_engine)) => {
                *engine = restored_engine;
                Err(CommandError::core(error))
            }
            (_, Err(error)) => Err(CommandError::core(format!(
                "数据文件处理后无法重新打开数据库：{error}"
            ))),
        };
        for path in sqlite_family_paths(&placeholder_path) {
            if path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }
        outcome
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

fn profile_fact(id: &str, label: &str, value: String, detail: &str) -> ProfileBehaviorFact {
    ProfileBehaviorFact {
        id: id.to_owned(),
        label: label.to_owned(),
        value,
        detail: detail.to_owned(),
    }
}

fn profile_dimension_label(key: &str) -> &str {
    match key {
        "intellect" => "认知开放",
        "competence" => "胜任感",
        "achievement_striving" => "成就投入",
        "self_discipline" => "自我调节",
        "goal_orientation" => "目标取向",
        "attribution_tendency" => "归因倾向",
        "self_efficacy" => "学习自我效能",
        _ => key,
    }
}

fn core_goal_input(
    input: GoalEditorInput,
    existing: Option<&GoalRecord>,
) -> Result<CoreGoalInput, CommandError> {
    let goal_id = input.id.trim().to_owned();
    let existing_dimensions = existing
        .map(|goal| {
            goal.dimensions
                .iter()
                .map(|dimension| (dimension.id.as_str(), dimension))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let existing_milestones = existing
        .map(|goal| {
            goal.milestones
                .iter()
                .map(|milestone| (milestone.id.as_str(), milestone))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let dimensions = input
        .dimensions
        .into_iter()
        .map(|dimension| {
            let current_value = existing_dimensions
                .get(dimension.id.as_str())
                .map(|item| item.current_value)
                .unwrap_or(0.0);
            CoreGoalDimensionInput {
                id: dimension.id,
                dimension_key: dimension.dimension_key,
                display_name: dimension.display_name,
                metric_type: dimension.metric_type,
                target_value: dimension.target_value,
                target_label: None,
                weight: dimension.weight,
                current_value,
                query_sql: None,
                query_hint: None,
            }
        })
        .collect();
    let milestones = input
        .milestones
        .into_iter()
        .enumerate()
        .map(|(index, milestone)| {
            let (trigger_type, trigger_config) = if milestone.manual {
                ("manual".to_owned(), serde_json::json!({}))
            } else {
                let dimension_key = milestone
                    .dimension_key
                    .ok_or_else(|| CommandError::core("自动里程碑必须选择一个进度维度"))?;
                let threshold = milestone
                    .threshold
                    .ok_or_else(|| CommandError::core("自动里程碑必须填写阈值"))?;
                (
                    "dimension_threshold".to_owned(),
                    serde_json::json!({
                        "dimension_key": dimension_key,
                        "operator": ">=",
                        "value": threshold,
                    }),
                )
            };
            let existing_milestone = existing_milestones.get(milestone.id.as_str());
            Ok(CoreGoalMilestoneInput {
                id: milestone.id,
                title: milestone.title,
                description: None,
                trigger_type,
                trigger_config,
                status: existing_milestone
                    .map(|item| item.status.clone())
                    .unwrap_or_else(|| "pending".to_owned()),
                reached_at: existing_milestone.and_then(|item| item.reached_at.clone()),
                sort_order: i64::try_from(index + 1).unwrap_or(i64::MAX),
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(CoreGoalInput {
        id: goal_id,
        title: input.title,
        description: input.description,
        status: input.status,
        deadline: input.deadline,
        pace: input.pace,
        priority: i64::from(input.priority),
        parent_goal_id: None,
        completion_summary: None,
        scope: CoreGoalScope {
            pack_ids: input.scope.pack_ids,
            dimension_keys: input.scope.dimension_keys,
            concept_ids: input.scope.concept_ids,
        },
        dimensions,
        milestones,
    })
}

fn goal_view(goal: GoalRecord, progress: Option<&GoalProgressReport>) -> GoalView {
    let progress_dimensions = progress
        .map(|item| {
            item.dimensions
                .iter()
                .map(|dimension| (dimension.dimension_key.as_str(), dimension))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let progress_milestones = progress
        .map(|item| {
            item.milestones
                .iter()
                .map(|milestone| (milestone.id.as_str(), milestone))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let dimensions: Vec<GoalDimensionView> = goal
        .dimensions
        .iter()
        .map(|dimension| {
            let derived = progress_dimensions.get(dimension.dimension_key.as_str());
            GoalDimensionView {
                id: dimension.id.clone(),
                dimension_key: dimension.dimension_key.clone(),
                display_name: dimension.display_name.clone(),
                metric_type: dimension.metric_type.clone(),
                current_value: derived
                    .map(|item| item.current_value)
                    .unwrap_or(dimension.current_value),
                target_value: dimension.target_value,
                weight: dimension.weight,
                progress: derived.map(|item| item.progress).unwrap_or_else(|| {
                    if dimension.target_value > 0.0 {
                        (dimension.current_value / dimension.target_value).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                }),
            }
        })
        .collect();
    let milestones = goal
        .milestones
        .iter()
        .map(|milestone| {
            let derived = progress_milestones.get(milestone.id.as_str());
            let dimension_key = milestone
                .trigger_config
                .get("dimension_key")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let threshold = milestone
                .trigger_config
                .get("value")
                .and_then(serde_json::Value::as_f64);
            GoalMilestoneView {
                id: milestone.id.clone(),
                title: milestone.title.clone(),
                status: derived
                    .map(|item| item.status.clone())
                    .unwrap_or_else(|| milestone.status.clone()),
                reached_at: derived
                    .and_then(|item| item.reached_at.clone())
                    .or_else(|| milestone.reached_at.clone()),
                dimension_key,
                threshold,
                manual: milestone.trigger_type == "manual",
            }
        })
        .collect();
    GoalView {
        id: goal.id,
        title: goal.title,
        description: goal.description,
        status: goal.status,
        deadline: goal.deadline,
        pace: goal.pace,
        priority: i32::try_from(goal.priority).unwrap_or(i32::MAX),
        scope: GoalScopeInput {
            pack_ids: goal.scope.pack_ids,
            dimension_keys: goal.scope.dimension_keys,
            concept_ids: goal.scope.concept_ids,
        },
        overall_progress: progress
            .map(|item| item.overall_progress)
            .unwrap_or_else(|| {
                let total_weight = dimensions.iter().map(|item| item.weight).sum::<f64>();
                if total_weight > 0.0 {
                    dimensions
                        .iter()
                        .map(|item| item.progress * item.weight)
                        .sum::<f64>()
                        / total_weight
                } else {
                    0.0
                }
            }),
        dimensions,
        milestones,
    }
}

fn report_view(report: MirrorReport) -> MirrorReportView {
    let mut items = report
        .assertions
        .iter()
        .map(|item| report_item_view(item, "assertion"))
        .collect::<Vec<_>>();
    items.extend(
        report
            .hypotheses
            .iter()
            .map(|item| report_item_view(item, "hypothesis")),
    );
    items.extend(
        report
            .suggestions
            .iter()
            .map(|item| report_item_view(item, "suggestion")),
    );
    let top_signal = report
        .top_signal
        .as_ref()
        .and_then(|top| items.iter().find(|item| item.id == top.id).cloned());
    let citation_status = match report.narrative.as_ref() {
        Some(narrative) if narrative.degraded => "degraded",
        Some(_) => "verified",
        None => "structured_evidence",
    }
    .to_owned();
    MirrorReportView {
        id: report.id,
        week: report.week,
        generated_at: report.generated_at,
        window_days: bounded_i32(report.window_days),
        items,
        top_signal,
        skipped: report
            .skipped
            .into_iter()
            .map(|item| ReportSkippedView {
                id: item.id,
                kind: item.kind,
                reason: item.reason,
            })
            .collect(),
        hazard_participates: report.hazard_gate.participates,
        hazard_reason: report.hazard_gate.reason,
        hazard_validation_auc: report.hazard_gate.validation_auc,
        reflection_prompts: report.reflection_prompts,
        narrative: report.narrative.map(|narrative| ReportNarrativeView {
            text: narrative.text,
            citations: narrative
                .citations
                .into_iter()
                .map(|citation| ReportCitationView {
                    evidence_id: citation.evidence_id,
                    quote: citation.quote,
                })
                .collect(),
            degraded: narrative.degraded,
        }),
        citation_status,
    }
}

fn report_item_view(item: &ReportItem, category: &str) -> ReportItemView {
    ReportItemView {
        id: item.id.clone(),
        category: category.to_owned(),
        kind: item.kind.clone(),
        subject: item.subject.clone(),
        claim: item.claim.clone(),
        confidence: item.confidence,
        evidence_ids: item.evidence_ids.clone(),
        suggested_action: item.suggested_action.clone(),
    }
}

fn trust_view(panel: TrustPanel) -> TrustWorkspaceSnapshot {
    let activity = &panel.recent_activity;
    TrustWorkspaceSnapshot {
        generated_at: panel.generated_at,
        window_days: bounded_i32(panel.window_days),
        gates: panel
            .gates
            .into_iter()
            .map(|gate| TrustGateView {
                framework: gate.framework,
                name: gate.name,
                status: gate.status,
                gate: gate.gate,
                metric: gate.metric,
                reason: gate.reason,
            })
            .collect(),
        breeding_experiments: panel
            .active_breeding_experiments
            .into_iter()
            .map(|experiment| TrustExperimentView {
                id: experiment.id,
                kind: "breeding".to_owned(),
                title: format!(
                    "{} 对照 {}",
                    experiment.candidate_move, experiment.incumbent_move
                ),
                status: experiment.status,
                metric: Some(experiment.posterior_win_prob),
                sample_summary: format!(
                    "候选 n={} · 在位 n={} · 准入 p≥{} · 最少 n={} · 任务 {}",
                    experiment.n_candidate,
                    experiment.n_incumbent,
                    experiment.admit_p,
                    experiment.min_n,
                    experiment.task_type
                ),
                hypothesis: Some(experiment.main_effect_hypothesis),
                at: experiment.updated_at,
            })
            .collect(),
        mrt_experiments: panel
            .active_mrt_experiments
            .into_iter()
            .map(|experiment| TrustExperimentView {
                id: experiment.id,
                kind: "mrt".to_owned(),
                title: experiment.move_id,
                status: if experiment.randomized {
                    "randomized"
                } else {
                    "assigned"
                }
                .to_owned(),
                metric: None,
                sample_summary: format!(
                    "预注册 {}{}{}",
                    experiment.prereg_id,
                    experiment
                        .window
                        .as_deref()
                        .map(|value| format!(" · 窗口 {value}"))
                        .unwrap_or_default(),
                    experiment
                        .context_hash
                        .as_deref()
                        .map(|value| format!(" · context {value}"))
                        .unwrap_or_default()
                ),
                hypothesis: experiment.main_effect_hypothesis,
                at: experiment.at,
            })
            .collect(),
        recent_activity: vec![
            activity_view("param_tuning", "参数调优", &activity.param_tuning_runs),
            activity_view(
                "breeding_evaluated",
                "育种评估",
                &activity.breeding_evaluated_7d,
            ),
            activity_view(
                "breeding_admitted",
                "育种准入",
                &activity.breeding_admitted_7d,
            ),
            activity_view(
                "breeding_retired",
                "育种退役",
                &activity.breeding_retired_7d,
            ),
            activity_view("mental_hazard", "放弃风险拟合", &activity.mental_fit_hazard),
            activity_view(
                "mental_state",
                "状态门拟合",
                &activity.mental_fit_state_gate,
            ),
            activity_view("gu_induction", "Gᵤ 归纳", &activity.gu_inductions),
            activity_view("consolidation", "夜间巩固", &activity.nightly_consolidation),
            activity_view("mirror_reports", "镜像报告", &activity.mirror_reports),
        ],
        current_pack_id: panel.governance.current_pack_id,
        governance: vec![
            trust_parameter_view(panel.governance.breeding_admit_p),
            trust_parameter_view(panel.governance.breeding_retire_p),
            trust_parameter_view(panel.governance.breeding_min_n),
        ],
    }
}

fn activity_view(id: &str, label: &str, activity: &ActivitySummary) -> TrustActivityView {
    TrustActivityView {
        id: id.to_owned(),
        label: label.to_owned(),
        count_7d: bounded_i32(activity.count_7d),
        last_at: activity.last_at.clone(),
        last_status: activity.last_status.clone(),
    }
}

fn trust_parameter_view(parameter: TrustParameter) -> TrustParameterView {
    TrustParameterView {
        key: parameter.key,
        current_value: parameter.current_value,
        default_value: parameter.default_value,
        class: parameter.class,
        bounds: parameter.bounds,
        tuning_route: parameter.tuning_route,
        is_governance_gate: parameter.is_governance_gate,
    }
}

fn profile_settings_view(settings: ProfileSettings) -> ProfileSettingsView {
    ProfileSettingsView {
        enabled: settings.enabled,
        disclosure_required: settings.disclosure_required,
        disclosure_acknowledged: settings.disclosure_acknowledged_at.is_some(),
        summary_sharing_enabled: settings.summary_sharing_enabled,
        paused_until: settings.paused_until,
    }
}

fn ai_profile_view(profile: AiInteractionProfile) -> AiInteractionProfileView {
    AiInteractionProfileView {
        persona: profile.persona,
        verbosity: profile.verbosity,
        explanation_depth: profile.explanation_depth,
        proactivity: profile.proactivity,
        intervention_frequency: profile.intervention_frequency,
        correction_style: profile.correction_style,
        custom_notes: profile.custom_notes,
        guidance: profile.guidance,
    }
}

fn bounded_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}

fn sqlite_family_paths(database_path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![database_path.to_path_buf()];
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut value = database_path.as_os_str().to_owned();
        value.push(suffix);
        paths.push(PathBuf::from(value));
    }
    paths
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
