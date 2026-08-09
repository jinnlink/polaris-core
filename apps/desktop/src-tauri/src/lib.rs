pub mod contracts;
pub mod state;

mod commands;

use std::fs;

use tauri::Manager;

pub const DATA_CHANGED_EVENT: &str = "polaris://data-changed";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            app.manage(state::DesktopState::open(data_dir.join("polaris.sqlite"))?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::status])
        .run(tauri::generate_context!())
        .expect("Polaris desktop failed to start");
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_changed_event_name_is_stable() {
        assert_eq!(super::DATA_CHANGED_EVENT, "polaris://data-changed");
    }
}
