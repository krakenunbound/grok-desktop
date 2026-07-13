//! Application configuration, projects, and chat history persistence.
//!
//! Data lives under the platform app-data directory:
//! Windows: `%APPDATA%\com.the-kraken.grok-desktop\`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Top-level app settings persisted as `settings.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub default_model: String,
    pub reasoning_effort: String,
    pub yolo_default: bool,
    pub theme: String,
    pub sidebar_collapsed: bool,
    pub right_panel_open: bool,
    /// Optional override for Grok binary path (empty = resolve from PATH).
    pub grok_binary: String,
    /// Optional override for image temp directory (empty = app data temp_images).
    pub temp_images_dir: String,
    pub last_project_id: Option<String>,
    /// Agent Transparency: when false (default), UI is black-box (high-level status only).
    /// When true, stream raw CLI/tool output (Verbose Mode).
    pub verbose_mode: bool,
    /// Keep Grok's planning behavior available by default; false passes --no-plan.
    pub plan_mode: bool,
    pub disable_web_search: bool,
    pub subagents_enabled: bool,
    pub memory_enabled: bool,
    pub permission_mode: String,
    pub tools: String,
    pub disallowed_tools: String,
    pub allow_rules: String,
    pub deny_rules: String,
    pub extra_rules: String,
    pub max_turns: String,
    /// Disable optional telemetry/trace sinks and stop a turn if repo-state upload is detected.
    pub privacy_guard_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_model: "grok-4.5".to_string(),
            reasoning_effort: "high".to_string(),
            yolo_default: false,
            theme: "dark".to_string(),
            sidebar_collapsed: false,
            right_panel_open: false,
            grok_binary: String::new(),
            temp_images_dir: String::new(),
            last_project_id: None,
            // Core design: Agent Transparency Mode default = Hidden (black box).
            verbose_mode: false,
            plan_mode: true,
            disable_web_search: false,
            subagents_enabled: true,
            memory_enabled: false,
            permission_mode: "default".to_string(),
            tools: String::new(),
            disallowed_tools: String::new(),
            allow_rules: String::new(),
            deny_rules: String::new(),
            extra_rules: String::new(),
            max_turns: String::new(),
            privacy_guard_enabled: true,
        }
    }
}

/// A pinned or recent project folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub pinned: bool,
    pub archived: bool,
    pub notes: String,
    pub project_type: String,
    pub last_modified: Option<DateTime<Utc>>,
    pub last_opened: DateTime<Utc>,
    pub last_chat_id: Option<String>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            path: String::new(),
            pinned: false,
            archived: false,
            notes: String::new(),
            project_type: "Folder".to_string(),
            last_modified: None,
            last_opened: Utc::now(),
            last_chat_id: None,
        }
    }
}

/// One chat session within a project (or "global" workspace).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<ChatMessage>,
}

/// A single chat message stored for history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub images: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub status: Option<String>,
}

/// Collection of known projects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectStore {
    pub projects: Vec<Project>,
}

/// Resolve the app data root directory, creating it if needed.
pub fn app_data_dir() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve data directory".to_string())?;
    let dir = base.join("com.the-kraken.grok-desktop");
    fs::create_dir_all(&dir).map_err(|e| format!("create app data dir: {e}"))?;
    Ok(dir)
}

/// Path for temporary uploaded images.
pub fn temp_images_dir(settings: &AppSettings) -> Result<PathBuf, String> {
    if !settings.temp_images_dir.trim().is_empty() {
        let p = PathBuf::from(settings.temp_images_dir.trim());
        // Refuse path components that escape (relative ..) before create.
        if p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("temp_images_dir must not contain '..'".into());
        }
        fs::create_dir_all(&p).map_err(|e| format!("create temp images dir: {e}"))?;
        return Ok(p);
    }
    let dir = app_data_dir()?.join("temp_images");
    fs::create_dir_all(&dir).map_err(|e| format!("create temp images dir: {e}"))?;
    Ok(dir)
}

/// Safe id for filesystem names (UUID-shaped: hex + hyphens only).
pub fn validate_id(id: &str, kind: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        return Err(format!("Invalid {kind} id"));
    }
    if !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(format!(
            "Invalid {kind} id (expected UUID-like characters only)"
        ));
    }
    if id.contains("..") {
        return Err(format!("Invalid {kind} id"));
    }
    Ok(())
}

/// Cap message content size when persisting (DoS / disk abuse).
pub const MAX_MESSAGE_CHARS: usize = 500_000;

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("settings.json"))
}

fn projects_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("projects.json"))
}

fn chats_dir() -> Result<PathBuf, String> {
    let dir = app_data_dir()?.join("chats");
    fs::create_dir_all(&dir).map_err(|e| format!("create chats dir: {e}"))?;
    Ok(dir)
}

/// Load settings, returning defaults if the file is missing or corrupt.
pub fn load_settings() -> AppSettings {
    // Corrupt / missing settings.json → safe defaults.
    settings_path()
        .and_then(|p| {
            if !p.exists() {
                return Ok(AppSettings::default());
            }
            let raw = fs::read_to_string(&p).map_err(|e| e.to_string())?;
            serde_json::from_str(&raw).map_err(|e| e.to_string())
        })
        .unwrap_or_default()
}

/// Persist settings atomically (write temp then rename).
pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    write_json_atomic(&path, settings)
}

pub fn load_projects() -> ProjectStore {
    projects_path()
        .and_then(|p| {
            if !p.exists() {
                return Ok(ProjectStore::default());
            }
            let raw = fs::read_to_string(&p).map_err(|e| e.to_string())?;
            serde_json::from_str(&raw).map_err(|e| e.to_string())
        })
        .unwrap_or_default()
}

pub fn save_projects(store: &ProjectStore) -> Result<(), String> {
    let path = projects_path()?;
    write_json_atomic(&path, store)
}

pub fn add_or_update_project(path: &str, name: Option<String>) -> Result<Project, String> {
    let requested_path = PathBuf::from(path);
    if !requested_path.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    let path_buf = requested_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve project directory: {e}"))?;
    let path = path_buf.to_string_lossy().to_string();
    let display_name = name.unwrap_or_else(|| {
        path_buf
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone())
    });

    let mut store = load_projects();
    let project_type = detect_project_type(&path_buf);
    let last_modified = folder_modified_at(&path_buf);
    if let Some(existing) = store
        .projects
        .iter_mut()
        .find(|p| paths_equal(&p.path, &path))
    {
        existing.last_opened = Utc::now();
        existing.name = display_name;
        existing.path = path.clone();
        existing.archived = false;
        existing.project_type = project_type;
        existing.last_modified = last_modified;
        let project = existing.clone();
        save_projects(&store)?;
        return Ok(project);
    }

    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: display_name,
        path,
        pinned: false,
        archived: false,
        notes: String::new(),
        project_type,
        last_modified,
        last_opened: Utc::now(),
        last_chat_id: None,
    };
    store.projects.push(project.clone());
    save_projects(&store)?;
    Ok(project)
}

pub fn create_project_folder(parent: &str, name: &str) -> Result<String, String> {
    let parent = PathBuf::from(parent.trim())
        .canonicalize()
        .map_err(|e| format!("Project location does not exist: {e}"))?;
    if !parent.is_dir() {
        return Err("Project location must be a folder".into());
    }

    let name = name.trim();
    let device_stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved_device = matches!(
        device_stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if name.is_empty()
        || name.len() > 128
        || matches!(name, "." | "..")
        || name.ends_with(['.', ' '])
        || reserved_device
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err("Use a valid folder name without Windows-reserved characters".into());
    }

    let path = parent.join(name);
    if path.exists() {
        return Err("A file or folder with that name already exists".into());
    }
    fs::create_dir(&path).map_err(|e| format!("Create project folder: {e}"))?;
    path.canonicalize()
        .map(|value| value.to_string_lossy().to_string())
        .map_err(|e| format!("Resolve project folder: {e}"))
}

pub fn paths_equal(left: &str, right: &str) -> bool {
    #[cfg(windows)]
    {
        left.replace('/', "\\")
            .eq_ignore_ascii_case(&right.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

pub fn set_project_pinned(id: &str, pinned: bool) -> Result<ProjectStore, String> {
    let mut store = load_projects();
    let project = store
        .projects
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Project not found: {id}"))?;
    project.pinned = pinned;
    save_projects(&store)?;
    Ok(store)
}

pub fn set_project_archived(id: &str, archived: bool) -> Result<ProjectStore, String> {
    let mut store = load_projects();
    let project = store
        .projects
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Project not found: {id}"))?;
    project.archived = archived;
    if archived {
        project.pinned = false;
    }
    save_projects(&store)?;
    Ok(store)
}

pub fn update_project_notes(id: &str, notes: String) -> Result<ProjectStore, String> {
    if notes.len() > 2000 {
        return Err("Project notes are too long".into());
    }
    let mut store = load_projects();
    let project = store
        .projects
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Project not found: {id}"))?;
    project.notes = notes.trim().to_string();
    save_projects(&store)?;
    Ok(store)
}

pub fn remove_project(id: &str) -> Result<ProjectStore, String> {
    let mut store = load_projects();
    store.projects.retain(|p| p.id != id);
    save_projects(&store)?;
    Ok(store)
}

pub fn touch_project(id: &str, last_chat_id: Option<String>) -> Result<(), String> {
    let mut store = load_projects();
    if let Some(p) = store.projects.iter_mut().find(|p| p.id == id) {
        p.last_opened = Utc::now();
        let path = PathBuf::from(&p.path);
        p.last_modified = folder_modified_at(&path);
        p.project_type = detect_project_type(&path);
        if last_chat_id.is_some() {
            p.last_chat_id = last_chat_id;
        }
        save_projects(&store)?;
    }
    Ok(())
}

fn folder_modified_at(path: &Path) -> Option<DateTime<Utc>> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(DateTime::<Utc>::from)
}

fn detect_project_type(path: &Path) -> String {
    let has = |file: &str| path.join(file).exists();
    if has("tauri.conf.json") || path.join("src-tauri").exists() {
        return "Tauri".to_string();
    }
    if has("package.json") {
        return "Node".to_string();
    }
    if has("Cargo.toml") {
        return "Rust".to_string();
    }
    if has("pyproject.toml") || has("requirements.txt") {
        return "Python".to_string();
    }
    if has("go.mod") {
        return "Go".to_string();
    }
    if has("deno.json") || has("deno.jsonc") {
        return "Deno".to_string();
    }
    if path.join(".git").exists() {
        return "Git".to_string();
    }
    "Folder".to_string()
}

fn chat_path(chat_id: &str) -> Result<PathBuf, String> {
    validate_id(chat_id, "chat")?;
    let dir = chats_dir()?;
    let path = dir.join(format!("{chat_id}.json"));
    // Ensure resolved path stays under chats dir (defense in depth).
    let canon_dir = dir.canonicalize().unwrap_or(dir.clone());
    if let Ok(canon) = path.canonicalize() {
        if !canon.starts_with(&canon_dir) {
            return Err("Chat path escapes chats directory".into());
        }
    } else {
        // File may not exist yet — join must still be a single segment.
        if path.parent() != Some(dir.as_path()) {
            return Err("Invalid chat path".into());
        }
    }
    Ok(path)
}

pub fn load_chat(chat_id: &str) -> Result<ChatSession, String> {
    let path = chat_path(chat_id)?;
    if !path.exists() {
        return Err(format!("Chat not found: {chat_id}"));
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save_chat(session: &ChatSession) -> Result<(), String> {
    validate_id(&session.id, "chat")?;
    if let Some(ref pid) = session.project_id {
        validate_id(pid, "project")?;
    }
    let path = chat_path(&session.id)?;
    write_json_atomic(&path, session)
}

pub fn delete_chat(chat_id: &str) -> Result<(), String> {
    let path = chat_path(chat_id)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn export_chat_markdown(chat_id: &str, destination: &str) -> Result<(), String> {
    let session = load_chat(chat_id)?;
    if destination.trim().is_empty() || destination.len() > 4096 || destination.contains('\0') {
        return Err("Invalid export path".into());
    }
    let path = PathBuf::from(destination);
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
        return Err("Chat exports must use a .md extension".into());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "Export path has no parent directory".to_string())?;
    if !parent.is_dir() {
        return Err("Export destination directory does not exist".into());
    }

    write_text_atomic(&path, &chat_markdown(&session))
}

fn chat_markdown(session: &ChatSession) -> String {
    let mut markdown = format!(
        "# {}\n\n- Created: {}\n- Updated: {}\n\n",
        session.title.replace(['\r', '\n'], " "),
        session.created_at.to_rfc3339(),
        session.updated_at.to_rfc3339()
    );
    for message in &session.messages {
        let role = match message.role.as_str() {
            "user" => "You",
            "assistant" => "Grok",
            _ => "System",
        };
        markdown.push_str(&format!("## {role}\n\n"));
        if !message.content.is_empty() {
            markdown.push_str(&message.content);
            markdown.push_str("\n\n");
        }
        if !message.images.is_empty() {
            markdown.push_str("Attachments:\n\n");
            for attachment in &message.images {
                let name = Path::new(attachment)
                    .file_name()
                    .map(|value| value.to_string_lossy())
                    .unwrap_or_else(|| "attachment".into());
                markdown.push_str(&format!("- `{name}`\n"));
            }
            markdown.push('\n');
        }
    }
    markdown
}

/// Lightweight list row — avoids deserializing full message bodies.
#[derive(Debug, Deserialize)]
struct ChatListMeta {
    id: String,
    project_id: Option<String>,
    title: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// List chat metadata (messages stripped for listing performance).
pub fn list_chats(project_id: Option<&str>) -> Result<Vec<ChatSession>, String> {
    let dir = chats_dir()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Cap per-file read for list to avoid loading multi-MB histories into memory repeatedly.
        let raw = match fs::read(&path) {
            Ok(bytes) if bytes.len() > 8 * 1024 * 1024 => continue,
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(_) => continue,
        };
        // Prefer partial parse: serde ignores unknown fields; we only need metadata.
        // Full ChatSession deserialize still works if messages present — clear after.
        let mut session: ChatSession = match serde_json::from_str::<ChatListMeta>(&raw) {
            Ok(meta) => ChatSession {
                id: meta.id,
                project_id: meta.project_id,
                title: meta.title,
                created_at: meta.created_at,
                updated_at: meta.updated_at,
                messages: Vec::new(),
            },
            Err(_) => match serde_json::from_str::<ChatSession>(&raw) {
                Ok(mut s) => {
                    s.messages.clear();
                    s
                }
                Err(_) => continue,
            },
        };
        // None => only workspace chats (project_id is None). Some(id) => that project.
        match project_id {
            Some(pid) if session.project_id.as_deref() != Some(pid) => continue,
            None if session.project_id.is_some() => continue,
            _ => {}
        }
        let _ = &mut session;
        out.push(session);
    }
    out.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
    Ok(out)
}

pub fn new_chat(project_id: Option<String>, title: Option<String>) -> Result<ChatSession, String> {
    let now = Utc::now();
    let session = ChatSession {
        id: Uuid::new_v4().to_string(),
        project_id,
        title: title.unwrap_or_else(|| "New Chat".to_string()),
        created_at: now,
        updated_at: now,
        messages: Vec::new(),
    };
    save_chat(&session)?;
    Ok(session)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid path (no parent)".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("data"),
        Uuid::new_v4()
    ));
    let raw = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(&tmp, raw).map_err(|e| e.to_string())?;

    replace_file(&tmp, path).map_err(|e| {
        // Best-effort cleanup of temp on failure.
        let _ = fs::remove_file(&tmp);
        format!("replace target with temp: {e}")
    })?;
    Ok(())
}

pub(crate) fn write_text_atomic(path: &Path, value: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid path (no parent)".to_string())?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("export"),
        Uuid::new_v4()
    ));
    fs::write(&tmp, value).map_err(|error| format!("write export: {error}"))?;
    replace_file(&tmp, path).map_err(|error| {
        let _ = fs::remove_file(&tmp);
        format!("replace export: {error}")
    })
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Known Grok model IDs for the UI selector (backend may accept any string).
pub fn default_models() -> Vec<&'static str> {
    vec!["grok-4.5", "grok-composer-2.5-fast"]
}

#[cfg(test)]
mod tests {
    use super::{
        chat_markdown, create_project_folder, write_json_atomic, ChatMessage, ChatSession,
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn atomic_json_write_creates_and_replaces() {
        let dir = std::env::temp_dir().join(format!("grok-desktop-test-{}", Uuid::new_v4()));
        let path = dir.join("state.json");

        write_json_atomic(&path, &json!({ "version": 1 })).expect("create JSON");
        write_json_atomic(&path, &json!({ "version": 2 })).expect("replace JSON");

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read JSON"))
                .expect("parse JSON");
        assert_eq!(value, json!({ "version": 2 }));
        assert_eq!(fs::read_dir(&dir).expect("read temp dir").count(), 1);

        fs::remove_dir_all(dir).expect("remove temp dir");
    }

    #[test]
    fn markdown_export_preserves_messages_and_safe_attachment_names() {
        let now = Utc.with_ymd_and_hms(2026, 7, 12, 10, 30, 0).unwrap();
        let session = ChatSession {
            id: Uuid::new_v4().to_string(),
            project_id: None,
            title: "Export\nDemo".into(),
            created_at: now,
            updated_at: now,
            messages: vec![ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".into(),
                content: "Please review this.".into(),
                images: vec![r"C:\managed\report.md".into()],
                timestamp: now,
                status: None,
            }],
        };

        let markdown = chat_markdown(&session);
        assert!(markdown.starts_with("# Export Demo\n"));
        assert!(markdown.contains("## You\n\nPlease review this."));
        assert!(markdown.contains("- `report.md`"));
        assert!(!markdown.contains(r"C:\managed"));
    }

    #[test]
    fn project_folder_creation_rejects_unsafe_names() {
        let parent = std::env::temp_dir().join(format!("grok-project-test-{}", Uuid::new_v4()));
        fs::create_dir(&parent).expect("create parent");

        let created = create_project_folder(&parent.to_string_lossy(), "Demo Project")
            .expect("create project folder");
        assert!(std::path::Path::new(&created).is_dir());
        assert!(create_project_folder(&parent.to_string_lossy(), "../escape").is_err());
        assert!(create_project_folder(&parent.to_string_lossy(), "CON").is_err());
        assert!(create_project_folder(&parent.to_string_lossy(), "Demo Project").is_err());

        fs::remove_dir_all(parent).expect("remove project test");
    }
}
