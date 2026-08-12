use polaris_core::pack_state::PackSwitchReceipt;
use polaris_core::status::StatusSnapshot;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

use crate::contracts::{
    AiInteractionProfileUpdate, AttemptGradeStatus, CaptureWorkspaceInput, CaptureWorkspaceReceipt,
    CommandError, FullDeleteInput, FullDeleteReceiptView, FullDeleteScopePreview, GoalEditorInput,
    GoalMutationReceipt, GoalWorkspaceSnapshot, GradeQueueReceipt, InboxActionInput,
    InboxActionReceipt, InboxPracticeDraft, InboxPracticeSubmitInput, InboxPracticeSubmitReceipt,
    InboxWorkspaceItem, InboxWorkspaceQuery, MapWorkspaceQuery, MapWorkspaceSnapshot,
    NotificationReceipt, PracticeSubmitInput, PracticeSubmitReceipt, PracticeWorkspaceSnapshot,
    ProfileExportInput, ProfileMeasurementSubmitInput, ProfileSettingsUpdateInput,
    ProfileWorkspaceSnapshot, ReportFeedbackInput, ReportMutationReceipt, ReportsWorkspaceSnapshot,
    SettingsMutationReceipt, SettingsWorkspaceSnapshot, TodaySnapshot, TrustWorkspaceSnapshot,
    WindowModeReceipt,
};
use crate::shell::apply_window_mode;
use crate::state::{notification_receipt, DesktopState};
use crate::DATA_CHANGED_EVENT;

#[tauri::command(async)]
pub fn status(state: State<'_, DesktopState>) -> Result<StatusSnapshot, CommandError> {
    state.status()
}

#[tauri::command(async)]
pub fn today(state: State<'_, DesktopState>) -> Result<TodaySnapshot, CommandError> {
    state.today()
}

#[tauri::command(async)]
pub fn map_workspace(
    state: State<'_, DesktopState>,
    query: MapWorkspaceQuery,
) -> Result<MapWorkspaceSnapshot, CommandError> {
    state.map_workspace(query)
}

#[tauri::command(async)]
pub fn practice_workspace(
    state: State<'_, DesktopState>,
    session_id: String,
) -> Result<PracticeWorkspaceSnapshot, CommandError> {
    state.practice_workspace(&session_id)
}

#[tauri::command(async)]
pub fn profile_workspace(
    state: State<'_, DesktopState>,
) -> Result<ProfileWorkspaceSnapshot, CommandError> {
    state.profile_workspace()
}

#[tauri::command(async)]
pub fn goals_workspace(
    state: State<'_, DesktopState>,
    selected_goal_id: Option<String>,
) -> Result<GoalWorkspaceSnapshot, CommandError> {
    state.goals_workspace(selected_goal_id.as_deref())
}

#[tauri::command(async)]
pub fn save_goal(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: GoalEditorInput,
) -> Result<GoalMutationReceipt, CommandError> {
    let receipt = state.save_goal(input)?;
    emit_data_changed(&app, &["goals", "today"], "goal_saved")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn refresh_goal(
    app: AppHandle,
    state: State<'_, DesktopState>,
    goal_id: String,
) -> Result<GoalMutationReceipt, CommandError> {
    let receipt = state.refresh_goal(&goal_id)?;
    emit_data_changed(&app, &["goals"], "goal_refreshed")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn archive_goal(
    app: AppHandle,
    state: State<'_, DesktopState>,
    goal_id: String,
) -> Result<GoalMutationReceipt, CommandError> {
    let receipt = state.archive_goal(&goal_id)?;
    emit_data_changed(&app, &["goals", "today"], "goal_archived")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn delete_goal(
    app: AppHandle,
    state: State<'_, DesktopState>,
    goal_id: String,
) -> Result<GoalMutationReceipt, CommandError> {
    let receipt = state.delete_goal(&goal_id)?;
    emit_data_changed(&app, &["goals", "today"], "goal_deleted")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn reports_workspace(
    state: State<'_, DesktopState>,
) -> Result<ReportsWorkspaceSnapshot, CommandError> {
    state.reports_workspace()
}

#[tauri::command(async)]
pub fn run_report(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<ReportMutationReceipt, CommandError> {
    let receipt = state.run_report()?;
    emit_data_changed(&app, &["reports", "today", "trust"], "report_generated")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn report_feedback(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: ReportFeedbackInput,
) -> Result<ReportMutationReceipt, CommandError> {
    let receipt = state.report_feedback(input)?;
    emit_data_changed(&app, &["reports"], "report_feedback")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn trust_workspace(
    state: State<'_, DesktopState>,
) -> Result<TrustWorkspaceSnapshot, CommandError> {
    state.trust_workspace()
}

#[tauri::command(async)]
pub fn settings_workspace(
    state: State<'_, DesktopState>,
) -> Result<SettingsWorkspaceSnapshot, CommandError> {
    state.settings_workspace()
}

#[tauri::command(async)]
pub fn update_profile_settings(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: ProfileSettingsUpdateInput,
) -> Result<SettingsMutationReceipt, CommandError> {
    let receipt = state.update_profile_settings(input)?;
    emit_data_changed(&app, &["settings", "profile"], "profile_settings_updated")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn update_ai_profile(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: AiInteractionProfileUpdate,
) -> Result<SettingsMutationReceipt, CommandError> {
    let receipt = state.update_ai_profile(input)?;
    emit_data_changed(&app, &["settings"], "ai_profile_updated")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn submit_profile_measurement(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: ProfileMeasurementSubmitInput,
) -> Result<SettingsMutationReceipt, CommandError> {
    let receipt = state.submit_profile_measurement(input)?;
    emit_data_changed(&app, &["settings", "profile"], "profile_measurement")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn reset_profile(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<SettingsMutationReceipt, CommandError> {
    let receipt = state.reset_profile()?;
    emit_data_changed(&app, &["settings", "profile"], "profile_reset")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn export_profile(
    state: State<'_, DesktopState>,
    input: ProfileExportInput,
) -> Result<SettingsMutationReceipt, CommandError> {
    state.export_profile(input)
}

#[tauri::command(async)]
pub fn full_delete_scope(
    state: State<'_, DesktopState>,
) -> Result<FullDeleteScopePreview, CommandError> {
    state.full_delete_scope()
}

#[tauri::command(async)]
pub fn delete_all_data(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: FullDeleteInput,
) -> Result<FullDeleteReceiptView, CommandError> {
    let receipt = state.delete_all_data(input)?;
    emit_data_changed(
        &app,
        &[
            "today", "map", "practice", "inbox", "profile", "goals", "reports", "trust", "settings",
        ],
        "all_data_deleted",
    )?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn submit_practice(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: PracticeSubmitInput,
) -> Result<PracticeSubmitReceipt, CommandError> {
    let receipt = state.submit_practice(input)?;
    emit_data_changed(
        &app,
        &["practice", "today", "map", "mirror"],
        "provisional_submission",
    )?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn attempt_grade_status(
    state: State<'_, DesktopState>,
    attempt_id: String,
) -> Result<AttemptGradeStatus, CommandError> {
    state.attempt_grade_status(&attempt_id)
}

#[tauri::command(async)]
pub fn process_grade_queue(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<GradeQueueReceipt, CommandError> {
    let receipt = state.process_grade_queue()?;
    if receipt.processed > 0 {
        emit_data_changed(&app, &["practice", "today", "map", "mirror"], "final_grade")?;
    }
    Ok(receipt)
}

#[tauri::command(async)]
pub fn capture_workspace(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: CaptureWorkspaceInput,
) -> Result<CaptureWorkspaceReceipt, CommandError> {
    let receipt = state.capture_workspace(input)?;
    emit_data_changed(&app, &["inbox"], "capture_recorded")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn inbox_workspace(
    state: State<'_, DesktopState>,
    query: InboxWorkspaceQuery,
) -> Result<Vec<InboxWorkspaceItem>, CommandError> {
    state.inbox_workspace(query)
}

#[tauri::command(async)]
pub fn act_on_inbox(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: InboxActionInput,
) -> Result<InboxActionReceipt, CommandError> {
    let receipt = state.act_on_inbox(input)?;
    emit_data_changed(&app, &["inbox"], "inbox_action")?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn draft_inbox_practice(
    state: State<'_, DesktopState>,
    capture_id: String,
) -> Result<InboxPracticeDraft, CommandError> {
    state.draft_inbox_practice(&capture_id)
}

#[tauri::command(async)]
pub fn submit_inbox_practice(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: InboxPracticeSubmitInput,
) -> Result<InboxPracticeSubmitReceipt, CommandError> {
    let receipt = state.submit_inbox_practice(input)?;
    emit_data_changed(
        &app,
        &["inbox", "practice", "today", "map", "mirror"],
        "inbox_practice_submission",
    )?;
    Ok(receipt)
}

#[tauri::command(async)]
pub fn switch_pack(
    app: AppHandle,
    state: State<'_, DesktopState>,
    pack_id: String,
    theta_mode: Option<String>,
) -> Result<PackSwitchReceipt, CommandError> {
    let receipt = state.switch_pack(&pack_id, theta_mode.as_deref())?;
    app.emit(
        DATA_CHANGED_EVENT,
        serde_json::json!({
            "domains": ["map", "today", "goals", "inbox"],
            "reason": "pack_switched"
        }),
    )
    .map_err(CommandError::core)?;
    Ok(receipt)
}

#[tauri::command]
pub fn set_window_mode(
    window: WebviewWindow,
    mode: String,
) -> Result<WindowModeReceipt, CommandError> {
    apply_window_mode(&window, &mode)
}

#[tauri::command]
pub fn hide_to_tray(window: WebviewWindow) -> Result<(), CommandError> {
    window.hide().map_err(CommandError::core)
}

#[tauri::command(async)]
pub fn show_notification(
    app: AppHandle,
    state: State<'_, DesktopState>,
    level: String,
    title: String,
    body: String,
) -> Result<NotificationReceipt, CommandError> {
    let policy = state.notification_policy()?;
    let receipt = notification_receipt(&level, &policy)?;
    if !receipt.emitted {
        return Ok(receipt);
    }
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(CommandError::core)?;
    Ok(receipt)
}

fn emit_data_changed(app: &AppHandle, domains: &[&str], reason: &str) -> Result<(), CommandError> {
    app.emit(
        DATA_CHANGED_EVENT,
        serde_json::json!({ "domains": domains, "reason": reason }),
    )
    .map_err(CommandError::core)
}
