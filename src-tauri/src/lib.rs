mod actions;
// `backends` and `forwards` carry the multi-host plumbing - SSH tunnel
// management, BackendRouter, ForwardManager - which is only consumed by the
// GUI lane (run() + commands.rs). The headless Linux server does not route
// to remote backends nor open local forwards, so on `--no-default-features`
// these modules would compile into dead code; gate them at the boundary.
#[cfg(feature = "gui")]
mod backends;
#[cfg(feature = "gui")]
mod commands;
mod db;
#[cfg(feature = "gui")]
mod forwards;
mod paths;
mod scanner;
mod socket;

use db::Database;
use std::path::PathBuf;
use std::sync::Arc;

/// Returns true when the process should run as a headless backend (socket
/// server only, no Tauri GUI). Exposed so `main.rs` can dispatch before
/// constructing any Tauri state.
pub fn is_headless_argv<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter()
        .skip(1)
        .any(|a| matches!(a.as_ref(), "--headless" | "-H"))
}

/// Parse `--socket <path>` (or `--socket=<path>`) from argv. Returns the
/// override path if present. Used by `run_headless()` so the systemd unit can
/// place the socket somewhere other than the per-user XDG default (e.g.
/// `/run/portsage/portsage.sock` for the system-wide service).
pub fn parse_socket_argv<S: AsRef<str>>(args: &[S]) -> Option<PathBuf> {
    let mut iter = args.iter().skip(1);
    while let Some(a) = iter.next() {
        let a = a.as_ref();
        if a == "--socket" {
            if let Some(next) = iter.next() {
                return Some(PathBuf::from(next.as_ref()));
            }
        } else if let Some(val) = a.strip_prefix("--socket=") {
            return Some(PathBuf::from(val));
        }
    }
    None
}

/// Cheap liveness probe: another Portsage process is already serving the
/// socket if a plain Unix-domain connect to `path` succeeds.
fn another_instance_alive_at(path: &std::path::Path) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

/// Headless mode: spin up the socket server only, then block on SIGINT / SIGTERM.
/// Used by the CLI's autospawn flow and by anyone who wants the backend in CI
/// or scripted contexts without the menubar UI.
pub fn run_headless() {
    let args: Vec<String> = std::env::args().collect();
    let socket_override = parse_socket_argv(&args);
    let socket_path = paths::resolve_socket_path(socket_override.as_deref());

    if another_instance_alive_at(&socket_path) {
        eprintln!(
            "portsage: another instance is already serving {}; exiting cleanly",
            socket_path.display()
        );
        return;
    }

    let database = match Database::new() {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("portsage: failed to initialize database: {e}");
            std::process::exit(1);
        }
    };

    socket::start_socket_server_at(database, socket_path.clone());
    eprintln!(
        "portsage: headless backend ready ({})",
        socket_path.display()
    );

    // Block on either SIGINT (Ctrl-C) or SIGTERM (`kill <pid>`, brew upgrade
    // shutdowns). Without the SIGTERM handler the process would die hard on
    // upgrade scripts that signal it to stop, leaving a stale socket file.
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("portsage: failed to create tokio runtime for signals: {e}");
            std::process::exit(1);
        }
    };
    rt.block_on(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("portsage: cannot install SIGTERM handler: {e}");
                    return;
                }
            };
            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("portsage: cannot install SIGINT handler: {e}");
                    return;
                }
            };
            tokio::select! {
                _ = sigterm.recv() => {}
                _ = sigint.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    });
    eprintln!("portsage: shutting down");
}
#[cfg(feature = "gui")]
use tauri::{
    tray::MouseButton, tray::MouseButtonState, tray::TrayIconEvent, Manager, PhysicalPosition,
    RunEvent, WindowEvent,
};

#[cfg(all(feature = "gui", target_os = "macos"))]
use tauri::ActivationPolicy;

#[cfg(feature = "gui")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = match Database::new() {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("portsage: failed to initialize database: {e}");
            std::process::exit(1);
        }
    };

    // Start Unix socket server for MCP
    socket::start_socket_server(database.clone());

    // Per-process singleton that routes Tauri commands to the active backend
    // (Local or one of the configured Remote backends). Holds the SSH tunnel
    // map and the persisted current-target selection.
    let router: Arc<backends::BackendRouter> =
        Arc::new(backends::BackendRouter::new(database.clone()));

    // Phase 3: per-(backend, port) SSH local-forward lifecycle. Shares the
    // BackendManager so it reuses the same ControlMaster the protocol
    // tunnel is using. Clone the Arc before handing one copy to Tauri's
    // managed-state container; we keep the other to fire `shutdown()` from
    // the RunEvent::Exit handler.
    let forward_manager: Arc<forwards::ForwardManager> = Arc::new(forwards::ForwardManager::new(
        database.clone(),
        router.manager().clone(),
    ));
    let forwards_for_shutdown = forward_manager.clone();

    // Background daemon: startup auto-sync over every backend that has
    // `auto_forward_enabled = true`, then loop every PERIODIC_SYNC_INTERVAL.
    // Covers the case where an MCP client on the remote registers a port
    // without going through the Mac UI - the next tick picks it up.
    forwards::start_auto_sync(database.clone(), forward_manager.clone());

    let mut app = tauri::Builder::default()
        // Single-instance must be registered first: if another Portsage process is
        // already running, this callback fires in the existing process and the new
        // one exits immediately. Without this we'd get duplicate tray icons (one per
        // process) when the user accidentally launches twice (e.g. dev build + the
        // installed .app, or relaunch from Spotlight while it's already running).
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(database)
        .manage(router)
        .manage(forward_manager)
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::add_port,
            commands::remove_port,
            commands::scan_ports,
            commands::get_next_range,
            commands::list_unmanaged_ports,
            commands::open_in_finder,
            commands::open_in_terminal,
            commands::open_in_browser,
            commands::kill_port,
            commands::kill_project,
            commands::get_config,
            commands::set_config,
            commands::export_data,
            commands::import_data,
            commands::show_main_window,
            commands::quit_app,
            commands::get_mcp_dir,
            commands::check_mcp_installed,
            commands::install_mcp,
            commands::uninstall_mcp,
            commands::list_remote_backends,
            commands::add_remote_backend,
            commands::update_remote_backend,
            commands::remove_remote_backend,
            commands::set_remote_backend_auto_forward,
            commands::test_remote_backend,
            commands::get_current_backend,
            commands::set_current_backend,
            commands::get_tunnel_statuses,
            commands::close_tunnel,
            commands::list_forward_statuses,
            commands::enable_forward,
            commands::disable_forward,
            commands::sync_forwards,
            commands::list_forward_exclusions,
            commands::add_forward_exclusion,
            commands::remove_forward_exclusion,
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
                        let _ = popover.set_position(PhysicalPosition::new(x as i32, y as i32));
                        let _ = popover.show();
                        let _ = popover.set_focus();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("portsage: failed to build tauri application: {e}");
            std::process::exit(1);
        });

    // Start as accessory (no dock icon, just status bar)
    #[cfg(target_os = "macos")]
    app.set_activation_policy(ActivationPolicy::Accessory);

    app.run(move |app_handle: &tauri::AppHandle, event: RunEvent| {
        match &event {
            RunEvent::ExitRequested { .. } | RunEvent::Exit => {
                // Close any Portsage-managed ControlMasters (Phase 3).
                // User-owned masters are left alone by `shutdown()`.
                forwards_for_shutdown.shutdown();
            }
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
                #[cfg(target_os = "macos")]
                let _ = app_handle.set_activation_policy(ActivationPolicy::Accessory);
            }
            // macOS: when the user re-launches the app from Spotlight/Finder/Dock
            // while the process is already running, Tauri delivers Reopen instead
            // of a fresh launch. Without this handler the dock icon appears but no
            // window is shown, forcing a force-quit. Re-show the main window.
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                let _ = app_handle.set_activation_policy(ActivationPolicy::Regular);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_headless_argv_detects_long_flag() {
        let args = vec!["portsage", "--headless"];
        assert!(is_headless_argv(&args));
    }

    #[test]
    fn is_headless_argv_detects_short_flag() {
        let args = vec!["portsage", "-H"];
        assert!(is_headless_argv(&args));
    }

    #[test]
    fn is_headless_argv_ignores_program_name_position() {
        // A binary literally named `--headless` (extremely unlikely) shouldn't
        // count: only flags after argv[0] are interpreted.
        let args = vec!["--headless"];
        assert!(!is_headless_argv(&args));
    }

    #[test]
    fn is_headless_argv_returns_false_when_absent() {
        let args = vec!["portsage"];
        assert!(!is_headless_argv(&args));
        let args = vec!["portsage", "--other-flag"];
        assert!(!is_headless_argv(&args));
    }

    #[test]
    fn parse_socket_argv_handles_separate_arg() {
        let args = vec![
            "portsage",
            "--headless",
            "--socket",
            "/run/portsage/portsage.sock",
        ];
        assert_eq!(
            parse_socket_argv(&args),
            Some(PathBuf::from("/run/portsage/portsage.sock"))
        );
    }

    #[test]
    fn parse_socket_argv_handles_equals_form() {
        let args = vec!["portsage", "--headless", "--socket=/tmp/foo.sock"];
        assert_eq!(
            parse_socket_argv(&args),
            Some(PathBuf::from("/tmp/foo.sock"))
        );
    }

    #[test]
    fn parse_socket_argv_returns_none_when_absent() {
        let args = vec!["portsage", "--headless"];
        assert!(parse_socket_argv(&args).is_none());
    }

    #[test]
    fn parse_socket_argv_returns_none_when_missing_value() {
        // Trailing `--socket` with no value: don't crash, just return None and
        // let the resolver fall back to defaults.
        let args = vec!["portsage", "--headless", "--socket"];
        assert!(parse_socket_argv(&args).is_none());
    }

    #[test]
    fn another_instance_alive_at_returns_false_when_no_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.sock");
        assert!(!another_instance_alive_at(&path));
    }

    #[test]
    fn another_instance_alive_at_returns_true_when_listener_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ours.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        // The listener is held by `_listener`; a connect probe should succeed.
        assert!(another_instance_alive_at(&path));
    }
}
