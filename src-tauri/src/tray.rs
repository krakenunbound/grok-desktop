//! System tray icon and menu actions for Grok Desktop.
//!
//! The tray is the primary way to restore a window after close-to-tray.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

/// Build and attach the system tray. Call once during setup.
///
/// Returns `Ok(())` when the tray is registered with the app runtime.
/// Failures are non-fatal at the call site so the main window still opens.
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let show_i = MenuItem::with_id(app, "show", "Show / Hide Window", true, None::<&str>)
        .map_err(|e| format!("tray menu item 'show': {e}"))?;
    let new_chat_i = MenuItem::with_id(app, "new_chat", "New Chat", true, None::<&str>)
        .map_err(|e| format!("tray menu item 'new_chat': {e}"))?;
    let yolo_i = MenuItem::with_id(app, "toggle_yolo", "Toggle YOLO", true, None::<&str>)
        .map_err(|e| format!("tray menu item 'toggle_yolo': {e}"))?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Grok Desktop", true, None::<&str>)
        .map_err(|e| format!("tray menu item 'quit': {e}"))?;

    let menu = Menu::with_items(app, &[&show_i, &new_chat_i, &yolo_i, &quit_i])
        .map_err(|e| format!("tray menu: {e}"))?;

    let icon = app
        .default_window_icon()
        .ok_or_else(|| {
            "No window icon available for the system tray. Rebuild icons or use the default Tauri icon set.".to_string()
        })?
        .clone();

    // Built tray is held by Tauri's resource table; local binding is intentional.
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Grok Desktop — left-click to show/hide")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_main_window(app),
            "new_chat" => {
                let _ = app.emit("tray-new-chat", ());
                show_main_window(app);
            }
            "toggle_yolo" => {
                let _ = app.emit("tray-toggle-yolo", ());
                show_main_window(app);
            }
            "quit" => {
                // True quit — does not just hide.
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|e| format!("Failed to create system tray icon: {e}"))?;

    Ok(())
}

/// Show if hidden, hide if visible. No-op if the main window is missing.
pub fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            Ok(false) | Err(_) => {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    }
}

/// Ensure the main window is visible and focused.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hide the main window (close-to-tray behavior).
pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
