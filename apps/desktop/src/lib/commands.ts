import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import type {
  AttemptGradeStatus,
  CaptureWorkspaceInput,
  CaptureWorkspaceReceipt,
  CommandError,
  CredentialInput,
  DiagnosticExportInput,
  DatabasePathInput,
  BackgroundEventView,
  GradeQueueReceipt,
  GoalEditorInput,
  GoalMutationReceipt,
  GoalWorkspaceSnapshot,
  InboxActionInput,
  InboxActionReceipt,
  InboxPracticeDraft,
  InboxPracticeSubmitInput,
  InboxPracticeSubmitReceipt,
  InboxWorkspaceItem,
  InboxWorkspaceQuery,
  MapWorkspaceQuery,
  MapWorkspaceSnapshot,
  PackSwitchReceipt,
  PracticeSubmitInput,
  PracticeSubmitReceipt,
  PracticeWorkspaceSnapshot,
  ProfileWorkspaceSnapshot,
  ReportFeedbackInput,
  ReportMutationReceipt,
  ReportsWorkspaceSnapshot,
  AiInteractionProfileUpdate,
  ProfileExportInput,
  ProfileMeasurementSubmitInput,
  ProfileSettingsUpdateInput,
  SettingsMutationReceipt,
  SettingsWorkspaceSnapshot,
  FullDeleteInput,
  FullDeleteReceiptView,
  FullDeleteScopePreview,
  LifecycleSnapshot,
  StatusSnapshot,
  TodaySnapshot,
  TrustWorkspaceSnapshot,
  WindowModeReceipt,
} from "../contracts/core";
import { previewMapWorkspace } from "./mapPreview";
import { previewGoalMutation, previewGoalsWorkspace, previewSaveGoal } from "./goalsPreview";
import { previewReportFeedback, previewReportsWorkspace, previewRunReport, previewTrustWorkspace } from "./governancePreview";
import { previewResetProfile, previewSettingsWorkspace, previewSubmitMeasurement, previewUpdateAiProfile, previewUpdateProfileSettings } from "./settingsPreview";
import {
  previewActOnInbox,
  previewAttemptGradeStatus,
  previewCaptureWorkspace,
  previewDraftInboxPractice,
  previewInboxWorkspace,
  previewPracticeWorkspace,
  previewProcessGradeQueue,
  previewSubmitInboxPractice,
  previewSubmitPractice,
} from "./workbenchPreview";

export const commandKeys = {
  status: ["core", "status"] as const,
  today: ["core", "today"] as const,
  map: (query: MapWorkspaceQuery) => ["core", "map", query] as const,
  practice: (sessionId: string) => ["core", "practice", sessionId] as const,
  grade: (attemptId: string) => ["core", "grade", attemptId] as const,
  inbox: (query: InboxWorkspaceQuery) => ["core", "inbox", query] as const,
  profile: ["core", "profile"] as const,
  goals: (selectedGoalId: string | null) => ["core", "goals", selectedGoalId] as const,
  reports: ["core", "reports"] as const,
  trust: ["core", "trust"] as const,
  lifecycle: ["desktop", "lifecycle"] as const,
  settings: ["core", "settings"] as const,
};

function isBrowserPreview() {
  return (import.meta.env.DEV || import.meta.env.MODE === "release-test") && !("__TAURI_INTERNALS__" in window);
}

function previewPacks() {
  return [
    { id: "algorithms", title: "算法基础 · 核心包", concept_count: 10_000, active: true, theta_mode: "shared" },
    { id: "rust", title: "Rust 掌握实验室", concept_count: 2_840, active: false, theta_mode: "isolated" },
  ];
}

export async function getStatus(): Promise<StatusSnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve({
      generated_at: new Date().toISOString(), current_pack: "algorithms", theta_mode: "shared",
      packs: previewPacks(), due_today: 3, phase_counts: [{ phase: "acquisition", count: 4 }, { phase: "transfer", count: 2 }], concepts: [],
    });
  }
  return invoke<StatusSnapshot>("status");
}

export async function getToday(): Promise<TodaySnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve({
      generated_at: new Date().toISOString(), current_pack: "algorithms", theta_mode: "shared", packs: previewPacks(),
      top_signal: { claim: "Dijkstra 能写出实现，但负权边与迁移推理仍不稳定。", confidence: 0.68, suggested_action: "先在地图看证据，再做一道边界变式。" },
      actions: [
        { id: "map-dijkstra", kind: "定位模糊", title: "看清 Dijkstra 的证据地形", detail: "从当前图谱确认低置信来源与两跳依赖。", route: "/map?concept=dijkstra", concept_id: "dijkstra", expected_success: 0.68 },
        { id: "practice-dijkstra", kind: "验证真懂", title: "完成负权边边界变式", detail: "用一道短练习验证迁移稳定性。", route: "/practice?concept=dijkstra", concept_id: "dijkstra", expected_success: 0.63 },
        { id: "rest", kind: "恢复", title: "今天先到这里", detail: "保留现场并隐藏到托盘。", route: null, concept_id: null, expected_success: null },
      ],
      notification_policy: { state_gate_passed: true, dominant_state: null, suppress_non_error: false },
    });
  }
  return invoke<TodaySnapshot>("today");
}

export async function getMapWorkspace(
  query: MapWorkspaceQuery,
): Promise<MapWorkspaceSnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve(previewMapWorkspace(query));
  }
  return invoke<MapWorkspaceSnapshot>("map_workspace", { query });
}

export async function getPracticeWorkspace(sessionId: string): Promise<PracticeWorkspaceSnapshot> {
  if (isBrowserPreview()) return Promise.resolve(previewPracticeWorkspace(sessionId));
  return invoke<PracticeWorkspaceSnapshot>("practice_workspace", { sessionId });
}

export async function getProfileWorkspace(): Promise<ProfileWorkspaceSnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve({
      generated_at: new Date().toISOString(),
      settings: { enabled: true, disclosure_required: true, disclosure_acknowledged: true, summary_sharing_enabled: false, paused_until: null },
      facts: [
        { id: "sessions", label: "有效会话", value: "18", detail: "只统计已完成且可用于行为估计的会话。" },
        { id: "calibration", label: "校准差", value: "+0.08", detail: "比较反馈前把握度与实际表现，不解释人格。" },
        { id: "move-effects", label: "教法观测", value: "42", detail: "用于检验哪种教学行动在什么情境下有效。" },
        { id: "abandons", label: "中断记录", value: "3", detail: "只描述行为事实，不推断意志力或性格。" },
      ],
      dimensions: [
        { key: "self_efficacy", label: "学习自我效能", mean: 0.64, lower: 0.39, upper: 0.89, evidence_count: 21, gate_status: "shadow", gate_label: "尚未通过验证", purpose: "当前只在影子模式检验预测增益。", will_not_affect: "不会参与调度、评分、掌握度或确定性解释。", evidence_ids: ["profile-evidence-21"] },
        { key: "self_discipline", label: "自我调节", mean: 0.58, lower: 0.47, upper: 0.69, evidence_count: 168, gate_status: "active", gate_label: "已通过前瞻验证", purpose: "仅作为策略与节律的慢先验，实际干预仍需通过 MRT。", will_not_affect: "不会直接改写掌握度、评分或知识图谱。", evidence_ids: ["profile-evidence-168"] },
      ],
      notice: "画像是带不确定度的慢先验，不是人格类型，也不是对你的最终判断。",
      actions: [{ id: "settings", label: "管理画像", kind: "primary" }, { id: "goals", label: "查看目标", kind: "secondary" }, { id: "today", label: "返回 Today", kind: "quiet" }],
    });
  }
  return invoke<ProfileWorkspaceSnapshot>("profile_workspace");
}

export async function getGoalsWorkspace(selectedGoalId: string | null): Promise<GoalWorkspaceSnapshot> {
  if (isBrowserPreview()) return Promise.resolve(previewGoalsWorkspace(selectedGoalId));
  return invoke<GoalWorkspaceSnapshot>("goals_workspace", { selectedGoalId });
}

export async function saveGoal(input: GoalEditorInput): Promise<GoalMutationReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewSaveGoal(input));
  return invoke<GoalMutationReceipt>("save_goal", { input });
}

export async function refreshGoal(goalId: string): Promise<GoalMutationReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewGoalMutation(goalId, "refreshed"));
  return invoke<GoalMutationReceipt>("refresh_goal", { goalId });
}

export async function archiveGoal(goalId: string): Promise<GoalMutationReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewGoalMutation(goalId, "archived"));
  return invoke<GoalMutationReceipt>("archive_goal", { goalId });
}

export async function deleteGoal(goalId: string): Promise<GoalMutationReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewGoalMutation(goalId, "deleted"));
  return invoke<GoalMutationReceipt>("delete_goal", { goalId });
}

export async function getReportsWorkspace(): Promise<ReportsWorkspaceSnapshot> {
  if (isBrowserPreview()) return Promise.resolve(previewReportsWorkspace());
  return invoke<ReportsWorkspaceSnapshot>("reports_workspace");
}

export async function runReport(): Promise<ReportMutationReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewRunReport());
  return invoke<ReportMutationReceipt>("run_report");
}

export async function submitReportFeedback(input: ReportFeedbackInput): Promise<ReportMutationReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewReportFeedback(input));
  return invoke<ReportMutationReceipt>("report_feedback", { input });
}

export async function getTrustWorkspace(): Promise<TrustWorkspaceSnapshot> {
  if (isBrowserPreview()) return Promise.resolve(previewTrustWorkspace());
  return invoke<TrustWorkspaceSnapshot>("trust_workspace");
}

export async function getSettingsWorkspace(): Promise<SettingsWorkspaceSnapshot> { if (isBrowserPreview()) return Promise.resolve(previewSettingsWorkspace()); return invoke<SettingsWorkspaceSnapshot>("settings_workspace"); }
export async function getLifecycleStatus(): Promise<LifecycleSnapshot> { if (isBrowserPreview()) return Promise.resolve({ database_path: "C:\\Users\\you\\AppData\\Local\\Polaris\\polaris.sqlite", database_source: "local_app_data", database_path_acknowledged: false, startup_status: "ready", startup_message: "数据库完整且版本受支持。", schema_version: 9, upgrade_required: false, pre_upgrade_backup: null, previous_run_incomplete: false, recovered_background_jobs: [], pending_background_jobs: [], config_warning: null, startup_enabled: false, fast_api_key_configured: false, strong_api_key_configured: false, embed_api_key_configured: false }); return invoke<LifecycleSnapshot>("lifecycle_status"); }
export async function acknowledgeDatabasePath(): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "database_path_acknowledged", message: "当前数据库路径已确认。" }); return invoke<SettingsMutationReceipt>("acknowledge_database_path"); }
export async function selectDatabasePath(input: DatabasePathInput): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "database_path_changed", message: `预览切换到 ${input.path}` }); return invoke<SettingsMutationReceipt>("select_database_path", { input }); }
export async function setStartupEnabled(enabled: boolean): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "startup_updated", message: enabled ? "已启用开机启动。" : "开机启动已关闭。" }); return invoke<SettingsMutationReceipt>("set_startup_enabled", { enabled }); }
export async function saveApiKey(input: CredentialInput): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "credential_saved", message: `${input.slot} 凭据已保存。` }); return invoke<SettingsMutationReceipt>("save_api_key", { input }); }
export async function deleteApiKey(slot: string): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "credential_deleted", message: `${slot} 凭据已删除。` }); return invoke<SettingsMutationReceipt>("delete_api_key", { slot }); }
export async function exportDiagnostics(input: DiagnosticExportInput): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "diagnostics_exported", message: `预览模式不会写文件：${input.output_path}` }); return invoke<SettingsMutationReceipt>("export_diagnostics", { input }); }
export async function enqueueBackgroundJob(job: string): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "background_job_enqueued", message: `${job} 已排队。` }); return invoke<SettingsMutationReceipt>("enqueue_background_job", { job }); }
export async function pollBackgroundEvents(): Promise<BackgroundEventView[]> { if (isBrowserPreview()) return Promise.resolve([]); return invoke<BackgroundEventView[]>("poll_background_events"); }
export async function updateProfileSettings(input: ProfileSettingsUpdateInput): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve(previewUpdateProfileSettings(input)); return invoke<SettingsMutationReceipt>("update_profile_settings", { input }); }
export async function updateAiProfile(input: AiInteractionProfileUpdate): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve(previewUpdateAiProfile(input)); return invoke<SettingsMutationReceipt>("update_ai_profile", { input }); }
export async function submitProfileMeasurement(input: ProfileMeasurementSubmitInput): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve(previewSubmitMeasurement(input)); return invoke<SettingsMutationReceipt>("submit_profile_measurement", { input }); }
export async function resetProfile(): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve(previewResetProfile()); return invoke<SettingsMutationReceipt>("reset_profile"); }
export async function exportProfile(input: ProfileExportInput): Promise<SettingsMutationReceipt> { if (isBrowserPreview()) return Promise.resolve({ effect: "profile_exported", message: `预览模式不会写文件：${input.output_path}` }); return invoke<SettingsMutationReceipt>("export_profile", { input }); }
export async function getFullDeleteScope(): Promise<FullDeleteScopePreview> { if (isBrowserPreview()) return Promise.resolve({ database_path: "C:\\Users\\you\\polaris.sqlite", learning_attempts: 42, evidence_records: 96, goals: 2, profile_measurements: 6, reports: 4, behavior_events: 130, sqlite_files: ["C:\\Users\\you\\polaris.sqlite"], confirmation_phrase: "DELETE ALL POLARIS LEARNING DATA", backup_supported: true }); return invoke<FullDeleteScopePreview>("full_delete_scope"); }
export async function deleteAllData(input: FullDeleteInput): Promise<FullDeleteReceiptView> { if (isBrowserPreview()) return Promise.resolve({ deleted_at: new Date().toISOString(), database_path: "preview", backup_path: input.backup_path, files_deleted: 1, local_secrets_deleted: 0, empty_database_created: true, message: "预览数据已清除。" }); return invoke<FullDeleteReceiptView>("delete_all_data", { input }); }

export async function submitPractice(input: PracticeSubmitInput): Promise<PracticeSubmitReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewSubmitPractice(input));
  return invoke<PracticeSubmitReceipt>("submit_practice", { input });
}

export async function getAttemptGradeStatus(attemptId: string): Promise<AttemptGradeStatus> {
  if (isBrowserPreview()) return Promise.resolve(previewAttemptGradeStatus(attemptId));
  return invoke<AttemptGradeStatus>("attempt_grade_status", { attemptId });
}

export async function processGradeQueue(): Promise<GradeQueueReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewProcessGradeQueue());
  return invoke<GradeQueueReceipt>("process_grade_queue");
}

export async function captureWorkspace(input: CaptureWorkspaceInput): Promise<CaptureWorkspaceReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewCaptureWorkspace(input));
  return invoke<CaptureWorkspaceReceipt>("capture_workspace", { input });
}

export async function getInboxWorkspace(query: InboxWorkspaceQuery): Promise<InboxWorkspaceItem[]> {
  if (isBrowserPreview()) return Promise.resolve(previewInboxWorkspace(query));
  return invoke<InboxWorkspaceItem[]>("inbox_workspace", { query });
}

export async function actOnInbox(input: InboxActionInput): Promise<InboxActionReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewActOnInbox(input));
  return invoke<InboxActionReceipt>("act_on_inbox", { input });
}

export async function draftInboxPractice(captureId: string): Promise<InboxPracticeDraft> {
  if (isBrowserPreview()) return Promise.resolve(previewDraftInboxPractice(captureId));
  return invoke<InboxPracticeDraft>("draft_inbox_practice", { captureId });
}

export async function submitInboxPractice(input: InboxPracticeSubmitInput): Promise<InboxPracticeSubmitReceipt> {
  if (isBrowserPreview()) return Promise.resolve(previewSubmitInboxPractice(input));
  return invoke<InboxPracticeSubmitReceipt>("submit_inbox_practice", { input });
}

export async function switchPack(packId: string): Promise<PackSwitchReceipt> {
  if (isBrowserPreview()) return Promise.resolve({ active_pack: packId, theta_mode: packId === "rust" ? "isolated" : "shared" });
  return invoke<PackSwitchReceipt>("switch_pack", { packId });
}

export async function setWindowMode(
  mode: "compact" | "workspace",
): Promise<WindowModeReceipt> {
  if (isBrowserPreview()) return Promise.resolve({ mode });
  return invoke<WindowModeReceipt>("set_window_mode", { mode });
}

export async function hideToTray(): Promise<void> {
  if (isBrowserPreview()) return Promise.resolve();
  return invoke("hide_to_tray");
}

export function normalizeCommandError(error: unknown): CommandError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    "retryable" in error
  ) {
    return error as CommandError;
  }
  return {
    code: "desktop_error",
    message: error instanceof Error ? error.message : String(error),
    retryable: false,
  };
}

export async function openExternal(url: string): Promise<void> {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:") {
    throw new Error("只允许打开 HTTPS 外链");
  }
  await openUrl(parsed.toString());
}
