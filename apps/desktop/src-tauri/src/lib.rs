pub mod background;
pub mod contracts;
pub mod lifecycle;
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
            app.manage(state::DesktopState::bootstrap(&data_dir)?);
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
            commands::lifecycle_status,
            commands::acknowledge_database_path,
            commands::select_database_path,
            commands::set_startup_enabled,
            commands::save_api_key,
            commands::delete_api_key,
            commands::export_diagnostics,
            commands::enqueue_background_job,
            commands::poll_background_events,
            commands::today,
            commands::map_workspace,
            commands::practice_workspace,
            commands::profile_workspace,
            commands::goals_workspace,
            commands::save_goal,
            commands::refresh_goal,
            commands::archive_goal,
            commands::delete_goal,
            commands::reports_workspace,
            commands::run_report,
            commands::report_feedback,
            commands::trust_workspace,
            commands::settings_workspace,
            commands::update_profile_settings,
            commands::update_ai_profile,
            commands::submit_profile_measurement,
            commands::reset_profile,
            commands::export_profile,
            commands::full_delete_scope,
            commands::delete_all_data,
            commands::submit_practice,
            commands::attempt_grade_status,
            commands::process_grade_queue,
            commands::capture_workspace,
            commands::inbox_workspace,
            commands::act_on_inbox,
            commands::draft_inbox_practice,
            commands::submit_inbox_practice,
            commands::switch_pack,
            commands::set_window_mode,
            commands::hide_to_tray,
            commands::show_notification
        ])
        .build(tauri::generate_context!())
        .expect("Polaris desktop failed to build")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                if let Some(state) = app.try_state::<state::DesktopState>() {
                    let _ = state.shutdown(false);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_changed_event_name_is_stable() {
        assert_eq!(super::DATA_CHANGED_EVENT, "polaris://data-changed");
    }
}
