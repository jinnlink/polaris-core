pub mod contracts;
pub mod state;

mod commands;
mod shell;

use std::fs;

use tauri::Manager;

pub const DATA_CHANGED_EVENT: &str = "polaris://data-changed";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            shell::apply_shell_effect(app, shell::shell_effect(shell::ShellIntent::SecondInstance));
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            app.manage(state::DesktopState::open(data_dir.join("polaris.sqlite"))?);
            shell::install_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == shell::MAIN_WINDOW_LABEL {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    shell::apply_shell_effect(
                        window.app_handle(),
                        shell::shell_effect(shell::ShellIntent::CloseWindow),
                    );
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::today,
            commands::switch_pack,
            commands::set_window_mode,
            commands::hide_to_tray,
            commands::show_notification
        ])
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
