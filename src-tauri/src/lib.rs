//! Grok Desktop — Tauri application entry (library target).

mod capabilities;
mod commands;
mod config;
mod grok_cli;
mod grok_process;
mod image_handler;
mod launch_status;
mod tray;
mod usage;

use grok_process::GrokManager;
use launch_status::LaunchStatus;
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::webview::PageLoadEvent;
use tauri::{Emitter, Manager};

/// True when something accepts TCP on 127.0.0.1:1420 (Vite).
fn dev_server_reachable() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:1420".parse().unwrap(),
        Duration::from_millis(400),
    )
    .is_ok()
}

/// Production asset origin used by Tauri 2 custom protocol (Windows WebView2).
fn production_ui_url() -> tauri::Url {
    // Prefer tauri.localhost (Windows); fall back if parse ever fails.
    tauri::Url::parse("http://tauri.localhost/")
        .or_else(|_| tauri::Url::parse("https://tauri.localhost/"))
        .or_else(|_| tauri::Url::parse("tauri://localhost/"))
        .expect("static production UI URL")
}

fn is_dev_server_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.contains("127.0.0.1:1420") || u.contains("localhost:1420")
}

/// If the WebView is pointed at a dead Vite URL, switch to offline assets.
fn ensure_offline_ui_if_needed(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let current = window.url().map(|u| u.to_string()).unwrap_or_default();
    if !is_dev_server_url(&current) {
        // Already on asset protocol / production URL.
        if let Some(status) = app.try_state::<LaunchStatus>() {
            status.set_phase(app, "Loading offline UI…");
        }
        return;
    }

    // On dev URL — only stay there if Vite is actually up.
    if dev_server_reachable() {
        if let Some(status) = app.try_state::<LaunchStatus>() {
            status.set_phase(app, "Connected to dev server on 127.0.0.1:1420");
        }
        return;
    }

    if let Some(status) = app.try_state::<LaunchStatus>() {
        status.set_phase(
            app,
            "Dev server not running — switching to offline UI (build\\)…",
        );
    }

    let prod = production_ui_url();
    if let Err(e) = window.navigate(prod.clone()) {
        if let Some(status) = app.try_state::<LaunchStatus>() {
            status.mark_failed(
                app,
                format!(
                    "Dev server down on 127.0.0.1:1420 and offline navigate failed: {e}. \
                     Run Grok-Desktop.bat / npm start (release build)."
                ),
            );
        }
        launch_status::inject_load_error_page(
            &window,
            &format!("127.0.0.1:1420 refused connection, and offline UI navigate failed: {e}"),
        );
        return;
    }

    if let Some(status) = app.try_state::<LaunchStatus>() {
        status.set_phase(app, format!("Navigated to offline UI ({prod})"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GrokManager::new())
        .manage(LaunchStatus::default())
        .on_page_load(|webview, payload| {
            let app = webview.app_handle().clone();
            let url = payload.url().to_string();

            match payload.event() {
                PageLoadEvent::Started => {
                    if let Some(status) = app.try_state::<LaunchStatus>() {
                        status.ui_ready.store(false, Ordering::SeqCst);
                        if is_dev_server_url(&url) {
                            status.set_phase(&app, format!("Loading from dev server… ({url})"));
                        } else {
                            status.set_phase(&app, format!("Loading UI… ({url})"));
                        }
                    }
                }
                PageLoadEvent::Finished => {
                    if let Some(status) = app.try_state::<LaunchStatus>() {
                        // Dead Vite → connection-error page
                        if is_dev_server_url(&url) && !dev_server_reachable() {
                            status.set_phase(
                                &app,
                                "Dev server refused connection — switching to offline UI…",
                            );
                            ensure_offline_ui_if_needed(&app);
                            return;
                        }

                        if launch_status::looks_like_connection_error(&url, "", "") {
                            // Error page for 1420 — try offline fallback first
                            if !dev_server_reachable() {
                                ensure_offline_ui_if_needed(&app);
                                return;
                            }
                            status.mark_failed(&app, format!("WebView could not open {url}"));
                            if let Some(window) = app.get_webview_window(webview.label()) {
                                launch_status::inject_load_error_page(
                                    &window,
                                    &format!("Failed URL: {url}"),
                                );
                            }
                        } else {
                            status.set_phase(&app, "Page loaded — starting app…");
                            let _ = webview.eval(launch_status::connection_probe_script());
                        }
                    }
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            if let Some(status) = app.try_state::<LaunchStatus>() {
                status.set_phase(&handle, "Starting window…");
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                let _ = window.center();

                let hide_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        tray::hide_main_window(&hide_handle);
                    }
                });
            }

            // Immediately leave a dead 1420 URL if present.
            ensure_offline_ui_if_needed(app.handle());

            // Retry once shortly after start (first navigate can race window init).
            let retry_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(800)).await;
                ensure_offline_ui_if_needed(&retry_handle);
                tokio::time::sleep(Duration::from_millis(1500)).await;
                ensure_offline_ui_if_needed(&retry_handle);
            });

            if let Err(_e) = tray::setup_tray(app.handle()) {
                // Non-fatal
            }

            // Watchdog only if UI never becomes ready after offline attempts.
            let watchdog = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(20)).await;

                let Some(status) = watchdog.try_state::<LaunchStatus>() else {
                    return;
                };
                if status.ui_ready.load(Ordering::SeqCst) {
                    return;
                }
                if status.load_failed.load(Ordering::SeqCst) {
                    return;
                }

                // Last chance offline navigate
                ensure_offline_ui_if_needed(&watchdog);
                tokio::time::sleep(Duration::from_secs(8)).await;

                if status.ui_ready.load(Ordering::SeqCst) {
                    return;
                }

                let reason = "Timed out waiting for the frontend to boot. \
                    Close this app and run Grok-Desktop.bat (builds RELEASE offline UI). \
                    Debug binaries open http://127.0.0.1:1420 unless Vite is running.";
                status.mark_failed(&watchdog, reason);
                if let Some(window) = watchdog.get_webview_window("main") {
                    launch_status::inject_load_error_page(&window, reason);
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = watchdog.emit("launch-status", status.snapshot());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_models,
            commands::get_app_data_dir,
            commands::list_projects,
            commands::add_project,
            commands::create_project_folder,
            commands::remove_project,
            commands::set_project_pinned,
            commands::set_project_archived,
            commands::update_project_notes,
            commands::touch_project,
            commands::get_grok_inventory,
            commands::set_mcp_server_enabled,
            commands::set_plugin_enabled,
            commands::get_grok_cli_overview,
            commands::list_chats,
            commands::load_chat,
            commands::save_chat,
            commands::delete_chat,
            commands::export_chat_markdown,
            commands::new_chat,
            commands::append_chat_message,
            commands::start_grok_session,
            commands::send_message,
            commands::stop_session,
            commands::set_session_yolo,
            commands::set_session_model,
            commands::get_session,
            commands::is_session_running,
            commands::save_image_base64,
            commands::import_image_path,
            commands::import_attachment_path,
            commands::discard_temp_image,
            commands::get_status,
            commands::get_usage,
            commands::resolve_grok_binary,
            commands::report_ui_ready,
            commands::report_ui_failed,
            commands::get_launch_status,
            commands::retry_ui_load,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Grok Desktop");
}
