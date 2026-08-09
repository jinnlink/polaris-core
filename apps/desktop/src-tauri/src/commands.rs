use polaris_core::status::StatusSnapshot;
use tauri::State;

use crate::contracts::CommandError;
use crate::state::DesktopState;

#[tauri::command(async)]
pub fn status(state: State<'_, DesktopState>) -> Result<StatusSnapshot, CommandError> {
    state.status()
}
