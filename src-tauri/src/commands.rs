//! Tauri commands exposed to the Svelte frontend.

use crate::agent_runs::{
    self, AgentDefinition, AgentRunManager, AgentRunStarted, StartAgentRunRequest,
};
use crate::capabilities::{self, GrokInventory};
use crate::config::{self, AppSettings, ChatMessage, ChatSession, Project, ProjectStore};
use crate::grok_cli::{self, GrokCliOverview};
use crate::grok_process::{self, GrokManager, GrokStatus, SessionConfig};
use crate::image_handler::{self, SavedImage};
use crate::launch_status::{self, LaunchStatus, LaunchStatusSnapshot};
use crate::usage::{self, UsageSnapshot};
use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub models: Vec<String>,
}

#[tauri::command]
pub fn get_settings() -> AppSettings {
    config::load_settings()
}

#[tauri::command]
pub fn save_settings(mut settings: AppSettings) -> Result<AppSettings, String> {
    // Sanitize free-form path overrides (local desktop app; still avoid path tricks).
    settings.grok_binary = settings.grok_binary.trim().to_string();
    settings.temp_images_dir = settings.temp_images_dir.trim().to_string();
    settings.permission_mode = settings.permission_mode.trim().to_string();
    settings.reasoning_effort = settings.reasoning_effort.trim().to_string();
    settings.tools = settings.tools.trim().to_string();
    settings.disallowed_tools = settings.disallowed_tools.trim().to_string();
    settings.allow_rules = settings.allow_rules.trim().to_string();
    settings.deny_rules = settings.deny_rules.trim().to_string();
    settings.extra_rules = settings.extra_rules.trim().to_string();
    settings.max_turns = settings.max_turns.trim().to_string();
    if settings.default_model.trim().is_empty() || settings.default_model.len() > 64 {
        return Err("Invalid default model".into());
    }
    if !matches!(
        settings.reasoning_effort.as_str(),
        "low" | "medium" | "high"
    ) {
        return Err("Invalid reasoning effort".into());
    }
    if !matches!(
        settings.permission_mode.as_str(),
        "default" | "acceptEdits" | "auto" | "dontAsk" | "bypassPermissions" | "plan"
    ) {
        return Err("Invalid permission mode".into());
    }
    if !settings.max_turns.is_empty() {
        let n = settings
            .max_turns
            .parse::<u32>()
            .map_err(|_| "Max turns must be a number".to_string())?;
        if n == 0 || n > 200 {
            return Err("Max turns must be between 1 and 200".into());
        }
    }
    for (label, value) in [
        ("tools", &settings.tools),
        ("disallowed tools", &settings.disallowed_tools),
        ("allow rules", &settings.allow_rules),
        ("deny rules", &settings.deny_rules),
    ] {
        if value.len() > 2000
            || value.contains('\0')
            || value.contains('\n')
            || value.contains('\r')
        {
            return Err(format!("Invalid {label} value"));
        }
    }
    if settings.extra_rules.len() > 4000 || settings.extra_rules.contains('\0') {
        return Err("Invalid extra rules value".into());
    }
    if !settings.grok_binary.is_empty() {
        let _ = grok_process::resolve_grok_binary(&settings.grok_binary)?;
    }
    if !settings.temp_images_dir.is_empty() {
        let _ = config::temp_images_dir(&settings)?;
    }
    config::save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn get_models() -> ModelsResponse {
    ModelsResponse {
        models: config::default_models()
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

#[tauri::command]
pub fn get_app_data_dir() -> Result<String, String> {
    Ok(config::app_data_dir()?.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_projects() -> ProjectStore {
    config::load_projects()
}

#[tauri::command]
pub fn add_project(path: String, name: Option<String>) -> Result<Project, String> {
    config::add_or_update_project(&path, name)
}

#[tauri::command]
pub fn create_project_folder(parent: String, name: String) -> Result<String, String> {
    config::create_project_folder(&parent, &name)
}

#[tauri::command]
pub fn remove_project(id: String) -> Result<ProjectStore, String> {
    config::validate_id(&id, "project")?;
    config::remove_project(&id)
}

#[tauri::command]
pub fn set_project_pinned(id: String, pinned: bool) -> Result<ProjectStore, String> {
    config::validate_id(&id, "project")?;
    config::set_project_pinned(&id, pinned)
}

#[tauri::command]
pub fn set_project_archived(id: String, archived: bool) -> Result<ProjectStore, String> {
    config::validate_id(&id, "project")?;
    config::set_project_archived(&id, archived)
}

#[tauri::command]
pub fn update_project_notes(id: String, notes: String) -> Result<ProjectStore, String> {
    config::validate_id(&id, "project")?;
    config::update_project_notes(&id, notes)
}

#[tauri::command]
pub fn touch_project(id: String, last_chat_id: Option<String>) -> Result<(), String> {
    config::validate_id(&id, "project")?;
    if let Some(ref cid) = last_chat_id {
        config::validate_id(cid, "chat")?;
    }
    config::touch_project(&id, last_chat_id)
}

#[tauri::command]
pub fn get_grok_inventory() -> Result<GrokInventory, String> {
    let settings = config::load_settings();
    capabilities::inventory(&settings.grok_binary)
}

#[tauri::command]
pub fn set_mcp_server_enabled(name: String, enabled: bool) -> Result<GrokInventory, String> {
    capabilities::set_mcp_enabled(&name, enabled)
}

#[tauri::command]
pub fn set_plugin_enabled(name: String, enabled: bool) -> Result<GrokInventory, String> {
    let settings = config::load_settings();
    capabilities::set_plugin_enabled(&settings.grok_binary, &name, enabled)
}

#[tauri::command]
pub fn get_grok_cli_overview(cwd: Option<String>) -> Result<GrokCliOverview, String> {
    let settings = config::load_settings();
    grok_cli::overview(&settings.grok_binary, cwd.as_deref())
}

#[tauri::command]
pub fn list_agent_definitions(cwd: String) -> Result<Vec<AgentDefinition>, String> {
    let settings = config::load_settings();
    agent_runs::list_definitions(&settings.grok_binary, &cwd)
}

#[tauri::command]
pub fn create_agent_definition(
    cwd: String,
    scope: String,
    name: String,
    description: String,
    instructions: String,
) -> Result<String, String> {
    agent_runs::create_definition(&cwd, &scope, &name, &description, &instructions)
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn start_agent_run(
    app: AppHandle,
    manager: State<'_, AgentRunManager>,
    cwd: String,
    agent: String,
    prompt: String,
    model: String,
    yolo: bool,
) -> Result<AgentRunStarted, String> {
    let settings = config::load_settings();
    agent_runs::start_run(
        app,
        manager.inner(),
        &settings.grok_binary,
        StartAgentRunRequest {
            cwd,
            agent,
            prompt,
            model,
            yolo,
        },
    )
    .await
}

#[tauri::command]
pub async fn stop_agent_run(
    manager: State<'_, AgentRunManager>,
    run_id: String,
) -> Result<(), String> {
    config::validate_id(&run_id, "agent run")?;
    agent_runs::stop_run(manager.inner(), &run_id).await
}

#[tauri::command]
pub fn list_chats(project_id: Option<String>) -> Result<Vec<ChatSession>, String> {
    config::list_chats(project_id.as_deref())
}

#[tauri::command]
pub fn load_chat(chat_id: String) -> Result<ChatSession, String> {
    config::load_chat(&chat_id)
}

#[tauri::command]
pub fn save_chat(session: ChatSession) -> Result<(), String> {
    if session.messages.len() > 2000 {
        return Err("Chat history limit exceeded".into());
    }
    let mut total = 0usize;
    let settings = config::load_settings();
    for m in &session.messages {
        total = total.saturating_add(m.content.len());
        if total > config::MAX_MESSAGE_CHARS * 4 {
            return Err("Chat payload too large".into());
        }
        if m.content.len() > config::MAX_MESSAGE_CHARS {
            return Err("Message too large".into());
        }
        if !matches!(m.role.as_str(), "user" | "assistant" | "system") {
            return Err("Invalid message role".into());
        }
        if m.images.len() > 16 {
            return Err("Too many attachments in a message".into());
        }
        for path in &m.images {
            let _ = image_handler::validate_managed_attachment(&settings, path)?;
        }
    }
    config::save_chat(&session)
}

#[tauri::command]
pub fn delete_chat(chat_id: String) -> Result<(), String> {
    config::delete_chat(&chat_id)
}

#[tauri::command]
pub fn export_chat_markdown(chat_id: String, destination: String) -> Result<(), String> {
    config::validate_id(&chat_id, "chat")?;
    config::export_chat_markdown(&chat_id, &destination)
}

#[tauri::command]
pub fn new_chat(project_id: Option<String>, title: Option<String>) -> Result<ChatSession, String> {
    config::new_chat(project_id, title)
}

/// Append a message to a chat and persist.
#[tauri::command]
pub fn append_chat_message(
    chat_id: String,
    message_id: String,
    role: String,
    content: String,
    images: Vec<String>,
    status: Option<String>,
) -> Result<ChatSession, String> {
    config::validate_id(&chat_id, "chat")?;
    config::validate_id(&message_id, "message")?;
    Uuid::parse_str(&message_id).map_err(|_| "Invalid message id (expected UUID)".to_string())?;
    let role = role.to_lowercase();
    if !matches!(role.as_str(), "user" | "assistant" | "system") {
        return Err("Invalid message role".into());
    }
    if content.len() > config::MAX_MESSAGE_CHARS {
        return Err("Message too large to persist".into());
    }
    if images.len() > 16 {
        return Err("Too many attachments".into());
    }
    let settings = config::load_settings();
    for path in &images {
        let _ = image_handler::validate_managed_attachment(&settings, path)?;
    }
    let mut session = config::load_chat(&chat_id)?;
    // IPC retries are safe: return the already-persisted result instead of
    // duplicating a message whose success response was interrupted.
    if session
        .messages
        .iter()
        .any(|message| message.id == message_id)
    {
        return Ok(session);
    }
    // Soft cap history growth to keep load times reasonable.
    if session.messages.len() > 2000 {
        return Err("Chat history limit reached; start a new chat".into());
    }
    session.messages.push(ChatMessage {
        id: message_id,
        role,
        content: content.clone(),
        images,
        timestamp: Utc::now(),
        status,
    });
    // Auto-title from first user message.
    if session.title == "New Chat" {
        if let Some(first_user) = session.messages.iter().find(|m| m.role == "user") {
            let t: String = first_user.content.chars().take(48).collect();
            if !t.trim().is_empty() {
                session.title = t;
            }
        }
    }
    session.updated_at = Utc::now();
    config::save_chat(&session)?;
    Ok(session)
}

#[tauri::command]
pub async fn start_grok_session(
    app: AppHandle,
    manager: State<'_, GrokManager>,
    model: String,
    yolo: bool,
    cwd: String,
    chat_id: Option<String>,
) -> Result<SessionConfig, String> {
    let settings = config::load_settings();
    let cfg = grok_process::start_session(
        app,
        manager.inner(),
        model,
        yolo,
        cwd,
        chat_id,
        &settings.grok_binary,
    )
    .await?;

    let mut settings = settings;
    let projects = config::load_projects();
    if let Some(p) = projects
        .projects
        .iter()
        .find(|p| config::paths_equal(&p.path, &cfg.cwd))
    {
        settings.last_project_id = Some(p.id.clone());
        let _ = config::save_settings(&settings);
        let _ = config::touch_project(&p.id, cfg.chat_id.clone());
    }

    Ok(cfg)
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    manager: State<'_, GrokManager>,
    prompt: String,
    attachment_paths: Vec<String>,
) -> Result<(), String> {
    let settings = config::load_settings();
    grok_process::send_message(app, manager, prompt, attachment_paths, settings.grok_binary).await
}

#[tauri::command]
pub async fn stop_session(manager: State<'_, GrokManager>) -> Result<(), String> {
    grok_process::stop_session(manager.inner()).await
}

#[tauri::command]
pub async fn set_session_yolo(
    manager: State<'_, GrokManager>,
    yolo: bool,
) -> Result<SessionConfig, String> {
    grok_process::set_yolo(manager.inner(), yolo).await
}

#[tauri::command]
pub async fn set_session_model(
    manager: State<'_, GrokManager>,
    model: String,
) -> Result<SessionConfig, String> {
    grok_process::set_model(manager.inner(), model).await
}

#[tauri::command]
pub async fn get_session(manager: State<'_, GrokManager>) -> Result<Option<SessionConfig>, String> {
    Ok(grok_process::current_session(manager.inner()).await)
}

#[tauri::command]
pub async fn is_session_running(manager: State<'_, GrokManager>) -> Result<bool, String> {
    Ok(grok_process::is_running(manager.inner()).await)
}

#[tauri::command]
pub fn save_image_base64(
    data_base64: String,
    mime_hint: Option<String>,
    filename_hint: Option<String>,
) -> Result<SavedImage, String> {
    let settings = config::load_settings();
    image_handler::save_image_base64(&settings, &data_base64, mime_hint, filename_hint)
}

#[tauri::command]
pub fn import_image_path(path: String) -> Result<SavedImage, String> {
    let settings = config::load_settings();
    image_handler::import_image_path(&settings, &path)
}

#[tauri::command]
pub fn import_attachment_path(path: String) -> Result<SavedImage, String> {
    let settings = config::load_settings();
    image_handler::import_attachment_path(&settings, &path)
}

#[tauri::command]
pub fn discard_temp_image(path: String) -> Result<(), String> {
    let settings = config::load_settings();
    image_handler::discard_temp_image(&settings, &path)
}

#[tauri::command]
pub async fn get_status(manager: State<'_, GrokManager>) -> Result<GrokStatus, String> {
    let cfg = grok_process::current_session(manager.inner()).await;
    let running = grok_process::is_running(manager.inner()).await;
    Ok(GrokStatus {
        state: if running {
            "running".into()
        } else if cfg.is_some() {
            "ready".into()
        } else {
            "idle".into()
        },
        detail: if running {
            "Thinking…".into()
        } else {
            "Idle".into()
        },
        yolo: cfg.as_ref().map(|c| c.yolo).unwrap_or(false),
        model: cfg.map(|c| c.model).unwrap_or_default(),
        running,
    })
}

#[tauri::command]
pub fn resolve_grok_binary() -> Result<String, String> {
    let settings = config::load_settings();
    Ok(grok_process::resolve_grok_binary(&settings.grok_binary)?
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub fn get_usage() -> UsageSnapshot {
    usage::load_usage()
}

// --- Launch / WebView load reporting ---

/// Called by the Svelte app once it has successfully mounted.
/// This is the ONLY signal we treat as real UI success.
#[tauri::command]
pub fn report_ui_ready(app: AppHandle, status: State<'_, LaunchStatus>) -> LaunchStatusSnapshot {
    status.mark_ready(&app);
    status.snapshot()
}

/// Called when the WebView error page probe detects a connection failure.
#[tauri::command]
pub fn report_ui_failed(
    app: AppHandle,
    status: State<'_, LaunchStatus>,
    reason: String,
) -> LaunchStatusSnapshot {
    status.mark_failed(&app, &reason);
    if let Some(window) = app.get_webview_window("main") {
        launch_status::inject_load_error_page(&window, &reason);
    }
    status.snapshot()
}

#[tauri::command]
pub fn get_launch_status(status: State<'_, LaunchStatus>) -> LaunchStatusSnapshot {
    status.snapshot()
}

/// Reload the main window (user-facing "Retry").
#[tauri::command]
pub fn retry_ui_load(app: AppHandle, status: State<'_, LaunchStatus>) -> Result<(), String> {
    status
        .ui_ready
        .store(false, std::sync::atomic::Ordering::SeqCst);
    status
        .load_failed
        .store(false, std::sync::atomic::Ordering::SeqCst);
    status.set_phase(&app, "Retrying load…");
    if let Some(window) = app.get_webview_window("main") {
        window
            .eval("location.reload()")
            .map_err(|e| format!("reload failed: {e}"))?;
    }
    Ok(())
}
