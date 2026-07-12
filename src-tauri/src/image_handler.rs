//! Managed attachment handling for pasted images and local files.

use crate::config::{self, AppSettings};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES: u64 = 200 * 1024 * 1024;

/// Result returned to the frontend after saving an image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedImage {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub mime: String,
    pub size_bytes: u64,
    pub kind: String,
}

/// Decode a data-URL or raw base64 payload and write it under temp_images.
pub fn save_image_base64(
    settings: &AppSettings,
    data_base64: &str,
    mime_hint: Option<String>,
    filename_hint: Option<String>,
) -> Result<SavedImage, String> {
    // Reject absurd payloads before base64 decode (base64 ~4/3 overhead).
    if data_base64.len() > (MAX_IMAGE_BYTES as usize) * 2 {
        return Err("Image payload too large".into());
    }

    let (mime, b64_payload) = parse_payload(data_base64, mime_hint)?;
    if !is_allowed_image_mime(&mime) {
        return Err(format!("Unsupported image type: {mime}"));
    }

    let bytes = B64
        .decode(b64_payload.as_bytes())
        .map_err(|e| format!("Invalid base64 image data: {e}"))?;

    if bytes.is_empty() {
        return Err("Empty image payload".into());
    }
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("Image exceeds 20 MB limit".into());
    }
    if !looks_like_image(&bytes) {
        return Err("Payload does not look like a supported image".into());
    }

    let ext = extension_for_mime(&mime);
    let id = Uuid::new_v4().to_string();
    let filename = filename_hint
        .filter(|f| !f.trim().is_empty())
        .map(|f| sanitize_filename(&f, ext))
        .unwrap_or_else(|| {
            let stamp = Utc::now().format("%Y%m%d_%H%M%S");
            format!("img_{stamp}_{}.{ext}", &id[..8])
        });

    let dir = config::temp_images_dir(settings)?;
    let path = unique_path(&dir, &filename)?;
    fs::write(&path, &bytes).map_err(|e| format!("Failed to write image: {e}"))?;

    Ok(SavedImage {
        id,
        path: path.to_string_lossy().to_string(),
        filename: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(filename),
        mime,
        size_bytes: bytes.len() as u64,
        kind: "image".into(),
    })
}

/// Copy an existing local file into temp_images (for drag-drop of file paths).
pub fn import_image_path(settings: &AppSettings, source_path: &str) -> Result<SavedImage, String> {
    if source_path.trim().is_empty() || source_path.len() > 4096 {
        return Err("Invalid image path".into());
    }

    let source = PathBuf::from(source_path);
    let source = source
        .canonicalize()
        .map_err(|e| format!("Cannot resolve image path: {e}"))?;

    if !source.is_file() {
        return Err(format!("File not found: {source_path}"));
    }

    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !is_allowed_image_ext(&ext) {
        return Err(format!(
            "Unsupported image extension '.{ext}' (allowed: png, jpg, jpeg, gif, webp, bmp)"
        ));
    }

    let meta = fs::metadata(&source).map_err(|e| format!("Stat image failed: {e}"))?;
    if meta.len() > MAX_IMAGE_BYTES {
        return Err("Image exceeds 20 MB limit".into());
    }

    let bytes = fs::read(&source).map_err(|e| format!("Read image failed: {e}"))?;
    if !looks_like_image(&bytes) {
        return Err("File does not look like a supported image".into());
    }

    let mime = mime_for_extension(&ext);
    let id = Uuid::new_v4().to_string();
    let stamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("img_{stamp}_{}.{ext}", &id[..8]);

    let dir = config::temp_images_dir(settings)?;
    let dest = unique_path(&dir, &filename)?;
    fs::write(&dest, &bytes).map_err(|e| format!("Failed to copy image: {e}"))?;

    Ok(SavedImage {
        id,
        path: dest.to_string_lossy().to_string(),
        filename: dest
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(filename),
        mime,
        size_bytes: bytes.len() as u64,
        kind: "image".into(),
    })
}

/// Copy a user-selected file into managed storage without loading it into IPC memory.
pub fn import_attachment_path(
    settings: &AppSettings,
    source_path: &str,
) -> Result<SavedImage, String> {
    if source_path.trim().is_empty() || source_path.len() > 4096 || source_path.contains('\0') {
        return Err("Invalid attachment path".into());
    }
    let source = PathBuf::from(source_path)
        .canonicalize()
        .map_err(|e| format!("Cannot resolve attachment: {e}"))?;
    if !source.is_file() {
        return Err("Attachment is not a regular file".into());
    }

    let metadata = fs::metadata(&source).map_err(|e| format!("Stat attachment failed: {e}"))?;
    if metadata.len() == 0 {
        return Err("Attachment is empty".into());
    }
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Err("Attachment exceeds the 200 MB limit".into());
    }

    let ext = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if is_dangerous_attachment_ext(&ext) {
        return Err(format!("Executable attachment '.{ext}' is not allowed"));
    }

    let kind = attachment_kind(&ext);
    if kind == "image" {
        let header = read_header(&source, 16)?;
        if !looks_like_image(&header) {
            return Err("Attachment extension says image, but its contents do not match".into());
        }
    }

    let original = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment");
    let safe_name = sanitize_attachment_filename(original);
    let id = Uuid::new_v4().to_string();
    let stamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("file_{stamp}_{}_{}", &id[..8], safe_name);
    let dir = config::temp_images_dir(settings)?;
    let destination = unique_path(&dir, &filename)?;
    fs::copy(&source, &destination).map_err(|e| format!("Copy attachment failed: {e}"))?;

    Ok(SavedImage {
        id,
        path: destination.to_string_lossy().to_string(),
        filename: safe_name,
        mime: mime_for_attachment(&ext).into(),
        size_bytes: metadata.len(),
        kind: kind.into(),
    })
}

fn read_header(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| format!("Read attachment failed: {e}"))?;
    let mut bytes = vec![0; max];
    let count = file
        .read(&mut bytes)
        .map_err(|e| format!("Read attachment failed: {e}"))?;
    bytes.truncate(count);
    Ok(bytes)
}

fn is_dangerous_attachment_ext(ext: &str) -> bool {
    matches!(
        ext,
        "exe"
            | "dll"
            | "msi"
            | "msp"
            | "com"
            | "scr"
            | "pif"
            | "cpl"
            | "sys"
            | "drv"
            | "lnk"
            | "url"
            | "reg"
    )
}

fn attachment_kind(ext: &str) -> &'static str {
    if is_allowed_image_ext(ext) {
        "image"
    } else if matches!(ext, "mp4" | "webm" | "mov" | "m4v" | "avi" | "mkv") {
        "video"
    } else if matches!(ext, "mp3" | "wav" | "m4a" | "aac" | "ogg" | "flac") {
        "audio"
    } else {
        "file"
    }
}

fn mime_for_attachment(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "csv" => "text/csv",
        "txt" | "md" | "log" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn sanitize_attachment_filename(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    let safe = safe.trim_matches([' ', '.']).trim();
    if safe.is_empty() {
        "attachment".into()
    } else {
        safe.into()
    }
}

/// Delete an unsent attachment, restricted to the managed temp image directory.
pub fn discard_temp_image(settings: &AppSettings, image_path: &str) -> Result<(), String> {
    let path = validate_managed_attachment(settings, image_path)?;
    fs::remove_file(path).map_err(|e| format!("Failed to discard attachment: {e}"))
}

/// Resolve an attachment and prove it is a regular file inside managed storage.
pub fn validate_managed_attachment(
    settings: &AppSettings,
    attachment_path: &str,
) -> Result<PathBuf, String> {
    if attachment_path.trim().is_empty()
        || attachment_path.len() > 4096
        || attachment_path.contains('\0')
    {
        return Err("Invalid attachment path".into());
    }
    let root = config::temp_images_dir(settings)?
        .canonicalize()
        .map_err(|e| format!("Cannot resolve temp image directory: {e}"))?;
    let path = PathBuf::from(attachment_path)
        .canonicalize()
        .map_err(|e| format!("Cannot resolve image path: {e}"))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err("Attachment is not a managed file".into());
    }
    Ok(path)
}

fn parse_payload(data: &str, mime_hint: Option<String>) -> Result<(String, String), String> {
    let trimmed = data.trim();
    if let Some(rest) = trimmed.strip_prefix("data:") {
        let (meta, payload) = rest
            .split_once(',')
            .ok_or_else(|| "Malformed data URL".to_string())?;
        let mime = meta
            .split(';')
            .next()
            .filter(|m| !m.is_empty())
            .unwrap_or("image/png")
            .to_string();
        if !meta.contains("base64") {
            return Err("Only base64 data URLs are supported".into());
        }
        return Ok((mime, payload.to_string()));
    }
    let mime = mime_hint.unwrap_or_else(|| "image/png".to_string());
    Ok((mime, trimmed.to_string()))
}

fn is_allowed_image_mime(mime: &str) -> bool {
    matches!(
        mime.to_lowercase().as_str(),
        "image/jpeg" | "image/jpg" | "image/png" | "image/gif" | "image/webp" | "image/bmp"
    )
}

fn is_allowed_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp")
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime.to_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        _ => "png",
    }
}

fn mime_for_extension(ext: &str) -> String {
    match ext {
        "jpg" | "jpeg" => "image/jpeg".into(),
        "png" => "image/png".into(),
        "gif" => "image/gif".into(),
        "webp" => "image/webp".into(),
        "bmp" => "image/bmp".into(),
        _ => "application/octet-stream".into(),
    }
}

/// Magic-byte sniff so we do not write arbitrary binaries as "images".
fn looks_like_image(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    // PNG
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return true;
    }
    // JPEG
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true;
    }
    // GIF
    if bytes.starts_with(b"GIF8") {
        return true;
    }
    // BMP
    if bytes.starts_with(b"BM") {
        return true;
    }
    // WEBP: RIFF....WEBP
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return true;
    }
    false
}

fn sanitize_filename(name: &str, default_ext: &str) -> String {
    // Use basename only — strip any path segments from hostile hints.
    let name = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = base.trim_matches('.').to_string();
    if base.is_empty() {
        return format!("image.{default_ext}");
    }
    // Force a single safe extension from mime, ignore attacker-controlled multi-ext.
    let stem = base.split('.').next().unwrap_or("image");
    let stem: String = stem.chars().take(80).collect();
    format!("{stem}.{default_ext}")
}

fn unique_path(dir: &Path, filename: &str) -> Result<PathBuf, String> {
    let mut path = dir.join(filename);
    if !path.exists() {
        return Ok(path);
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "img".into());
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "png".into());
    for i in 1..1000 {
        path = dir.join(format!("{stem}_{i}.{ext}"));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err("Could not allocate unique image filename".into())
}

#[cfg(test)]
mod tests {
    use super::{
        attachment_kind, discard_temp_image, import_attachment_path, is_dangerous_attachment_ext,
        sanitize_attachment_filename,
    };
    use crate::config::AppSettings;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn classifies_common_media_and_blocks_executables() {
        assert_eq!(attachment_kind("png"), "image");
        assert_eq!(attachment_kind("mp4"), "video");
        assert_eq!(attachment_kind("mp3"), "audio");
        assert_eq!(attachment_kind("pdf"), "file");
        assert!(is_dangerous_attachment_ext("exe"));
        assert!(is_dangerous_attachment_ext("lnk"));
        assert!(!is_dangerous_attachment_ext("rs"));
    }

    #[test]
    fn imports_and_discards_managed_document() {
        let root = std::env::temp_dir().join(format!("grok-attachments-{}", Uuid::new_v4()));
        let source_dir = root.join("source");
        let managed_dir = root.join("managed");
        fs::create_dir_all(&source_dir).expect("create source directory");
        let source = source_dir.join("plan (final).md");
        fs::write(&source, "# Test plan\n").expect("write fixture");
        let settings = AppSettings {
            temp_images_dir: managed_dir.to_string_lossy().to_string(),
            ..AppSettings::default()
        };

        let saved = import_attachment_path(&settings, &source.to_string_lossy())
            .expect("import attachment");
        assert_eq!(saved.kind, "file");
        assert!(saved.path.ends_with("plan _final_.md"));
        assert!(std::path::Path::new(&saved.path).is_file());
        discard_temp_image(&settings, &saved.path).expect("discard managed attachment");
        assert!(!std::path::Path::new(&saved.path).exists());

        fs::remove_dir_all(root).expect("remove fixture directory");
    }

    #[test]
    fn sanitizes_hostile_attachment_names() {
        assert_eq!(
            sanitize_attachment_filename("../../hello<>.txt"),
            "_.._hello__.txt"
        );
    }
}
