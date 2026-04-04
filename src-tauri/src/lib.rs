mod commands;
mod db;
mod scanner;
mod socket;

use db::Database;
use std::sync::Arc;
use tauri::{
    Manager,
    tray::TrayIconEvent,
    tray::MouseButton,
    tray::MouseButtonState,
    PhysicalPosition,
    RunEvent, WindowEvent,
};

#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = Arc::new(Database::new().expect("failed to initialize database"));

    // Start Unix socket server for MCP
    socket::start_socket_server(database.clone());

    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(database)
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::create_project,
            commands::delete_project,
            commands::add_port,
            commands::remove_port,
            commands::scan_ports,
            commands::get_next_range,
            commands::list_unmanaged_ports,
            commands::open_in_finder,
            commands::open_in_terminal,
            commands::get_config,
            commands::set_config,
            commands::export_data,
            commands::import_data,
            commands::show_main_window,
            commands::quit_app,
            commands::check_mcp_installed,
            commands::install_mcp,
            commands::uninstall_mcp,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            if let Some(popover) = app_handle.get_webview_window("popover") {
                let popover_clone = popover.clone();
                popover.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = popover_clone.hide();
                    }
                });
            }
            Ok(())
        })
        .on_tray_icon_event(|tray_icon, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray_icon.app_handle();
                if let Some(popover) = app.get_webview_window("popover") {
                    let is_visible = popover.is_visible().unwrap_or(false);
                    if is_visible {
                        let _ = popover.hide();
                    } else {
                        let (px, py) = match rect.position {
                            tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                            tauri::Position::Logical(p) => (p.x, p.y),
                        };
                        let (sw, sh) = match rect.size {
                            tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
                            tauri::Size::Logical(s) => (s.width, s.height),
                        };
                        let x = px - 175.0 + (sw / 2.0);
                        let y = py + sh;
                        let _ = popover.set_position(
                            PhysicalPosition::new(x as i32, y as i32),
                        );
                        let _ = popover.show();
                        let _ = popover.set_focus();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Start as accessory (no dock icon, just status bar)
    #[cfg(target_os = "macos")]
    app.set_activation_policy(ActivationPolicy::Accessory);

    app.run(|app_handle: &tauri::AppHandle, event: RunEvent| {
        if let RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } = &event
        {
            if label == "main" {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
                #[cfg(target_os = "macos")]
                let _ = app_handle.set_activation_policy(ActivationPolicy::Accessory);
            }
        }
    });
}
