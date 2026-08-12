use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalSize, WebviewWindow};

use crate::contracts::{CommandError, WindowModeReceipt};

pub const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellIntent {
    CloseWindow,
    Show,
    Compact,
    Workspace,
    Quit,
    SecondInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEffect {
    Hide,
    ShowAndFocus,
    CompactAndFocus,
    WorkspaceAndFocus,
    Exit,
}

pub fn shell_effect(intent: ShellIntent) -> ShellEffect {
    match intent {
        ShellIntent::CloseWindow => ShellEffect::Hide,
        ShellIntent::Show | ShellIntent::SecondInstance => ShellEffect::ShowAndFocus,
        ShellIntent::Compact => ShellEffect::CompactAndFocus,
        ShellIntent::Workspace => ShellEffect::WorkspaceAndFocus,
        ShellIntent::Quit => ShellEffect::Exit,
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn apply_shell_effect(app: &AppHandle, effect: ShellEffect) {
    match effect {
        ShellEffect::Hide => {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.hide();
            }
        }
        ShellEffect::ShowAndFocus => show_main_window(app),
        ShellEffect::CompactAndFocus => {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = apply_window_mode(&window, "compact");
            }
        }
        ShellEffect::WorkspaceAndFocus => {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = apply_window_mode(&window, "workspace");
            }
        }
        ShellEffect::Exit => {
            if let Some(state) = app.try_state::<crate::state::DesktopState>() {
                let _ = state.shutdown(true);
            }
            app.exit(0);
        }
    }
}

pub fn apply_window_mode(
    window: &WebviewWindow,
    mode: &str,
) -> Result<WindowModeReceipt, CommandError> {
    match mode {
        "compact" => {
            window.set_always_on_top(true).map_err(CommandError::core)?;
            window.set_resizable(false).map_err(CommandError::core)?;
            window
                .set_size(PhysicalSize::new(420, 640))
                .map_err(CommandError::core)?;
        }
        "workspace" => {
            window
                .set_always_on_top(false)
                .map_err(CommandError::core)?;
            window.set_resizable(true).map_err(CommandError::core)?;
            window
                .set_size(PhysicalSize::new(1180, 760))
                .map_err(CommandError::core)?;
            let _ = window.center();
        }
        other => {
            return Err(CommandError::state(format!("unknown window mode: {other}")));
        }
    }
    window.show().map_err(CommandError::core)?;
    window.set_focus().map_err(CommandError::core)?;
    Ok(WindowModeReceipt {
        mode: mode.to_owned(),
    })
}

pub fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "打开 Polaris", true, None::<&str>)?;
    let compact = MenuItem::with_id(app, "compact", "常驻小窗", true, None::<&str>)?;
    let workspace = MenuItem::with_id(app, "workspace", "展开工作区", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &compact, &workspace, &quit])?;
    let icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::new()
        .tooltip("Polaris · 本地学习伙伴")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let intent = match event.id.as_ref() {
                "show" => Some(ShellIntent::Show),
                "compact" => Some(ShellIntent::Compact),
                "workspace" => Some(ShellIntent::Workspace),
                "quit" => Some(ShellIntent::Quit),
                _ => None,
            };
            if let Some(intent) = intent {
                apply_shell_effect(app, shell_effect(intent));
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                apply_shell_effect(tray.app_handle(), shell_effect(ShellIntent::Show));
            }
        });
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{shell_effect, ShellEffect, ShellIntent};

    #[test]
    fn shell_intents_have_stable_non_destructive_effects() {
        assert_eq!(shell_effect(ShellIntent::CloseWindow), ShellEffect::Hide);
        assert_eq!(shell_effect(ShellIntent::Show), ShellEffect::ShowAndFocus);
        assert_eq!(
            shell_effect(ShellIntent::SecondInstance),
            ShellEffect::ShowAndFocus
        );
        assert_eq!(
            shell_effect(ShellIntent::Compact),
            ShellEffect::CompactAndFocus
        );
        assert_eq!(
            shell_effect(ShellIntent::Workspace),
            ShellEffect::WorkspaceAndFocus
        );
        assert_eq!(shell_effect(ShellIntent::Quit), ShellEffect::Exit);
    }
}
