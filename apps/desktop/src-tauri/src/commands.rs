use polaris_core::pack_state::PackSwitchReceipt;
use polaris_core::status::StatusSnapshot;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

use crate::contracts::{CommandError, NotificationReceipt, TodaySnapshot, WindowModeReceipt};
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
