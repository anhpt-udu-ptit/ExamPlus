//! # Google OAuth 2.0 (External Browser) & Real Google Drive Sync Handler
//!
//! Cung cấp luồng xác thực OAuth 2.0 an toàn trên trình duyệt mặc định của hệ điều hành (RFC 8252, RFC 7636 PKCE S256)
//! và giao thức Google Picker chính thức trên desktop (`trigger_onepick=true`, `scope=drive.file`).
//! Đồng bộ hóa thực tế với Google Drive (`ExamPlus/DeThi_Word`, `ExamPlus/BangDiem_Excel`, `ExamPlus/SaoLuu_CauHinh`).

use crate::handlers::auth::{ApiErrorDetail, ApiErrorResponse};
use crate::handlers::teacher::{upload_docx, UploadDocxQuery};
use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserSession {
    pub provider: String,
    pub name: String,
    pub email: String,
    pub avatar: String,
    pub drive_connected: bool,
    #[serde(skip_serializing, default)]
    pub access_token: Option<String>,
    pub connected_at: String,
    pub last_sync: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFileItem {
    pub name: String,
    pub category: String, // "docx" | "xlsx" | "config"
    pub folder: String,
    pub size_bytes: u64,
    #[serde(rename = "size_Formatted")]
    pub size_formatted: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickedFileMetadata {
    pub file_id: String,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: Option<u64>,
    pub size_formatted: String,
    pub modified_time: Option<String>,
    pub picked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredOAuthTokens {
    pub email: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub scopes: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudDriveItem {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub is_folder: bool,
    pub file_kind: String, // "folder" | "docx" | "pdf" | "gdoc" | "other"
    pub size_bytes: Option<u64>,
    pub size_formatted: String,
    pub modified_time: Option<String>,
    pub starred: bool,
    pub shared: bool,
    pub owner_name: String,
}

#[derive(Debug, Deserialize)]
pub struct CloudFilesQuery {
    pub tab: Option<String>,        // "recent" | "mydrive" | "shared" | "starred"
    pub folder_id: Option<String>,  // Drive folder ID or "root"
    pub q: Option<String>,          // search substring
    pub file_type: Option<String>,  // "all" | "docx" | "pdf"
    pub page_token: Option<String>, // pagination token
}

#[derive(Debug, Serialize)]
pub struct GoogleAuthStatusResponse {
    pub authenticated: bool,
    pub user: Option<GoogleUserSession>,
    pub drive_folder_path: String,
    pub storage_mode: String,
    pub files_count: usize,
    pub total_size_bytes: u64,
    pub files: Vec<DriveFileItem>,
    pub latest_picked_file: Option<PickedFileMetadata>,
    pub persistent_session: bool,
    pub cloud_browse_ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub name: String,
    pub email: String,
    pub avatar: Option<String>,
    pub access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriveImportRequest {
    pub filename: String,
    pub file_id: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriveDeleteRequest {
    pub filename: String,
    pub folder: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriveUploadQuery {
    pub filename: String,
    pub folder: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleStartQuery {
    pub flow: Option<String>, // "login" | "picker" | "connect"
}

#[derive(Debug, Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub picked_file_ids: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OAuthTransaction {
    pub flow: String,
    pub code_verifier: String,
    pub created_at: chrono::DateTime<Utc>,
}

pub static OAUTH_TRANSACTIONS: LazyLock<Mutex<HashMap<String, OAuthTransaction>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub static LATEST_PICKED_FILE: LazyLock<Mutex<Option<PickedFileMetadata>>> =
    LazyLock::new(|| Mutex::new(None));

pub static BACKEND_TOKEN_VAULT: LazyLock<Mutex<HashMap<String, StoredOAuthTokens>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Phát hiện thư mục Google Drive for Desktop trên Windows hoặc khởi tạo kho đồng bộ ExamPlus_Drive/ExamPlus
pub fn resolve_drive_root() -> (PathBuf, String) {
    if let Ok(override_root) = std::env::var("EXAMPLUS_DRIVE_ROOT") {
        let trimmed = override_root.trim();
        if !trimmed.is_empty() {
            let custom_root = PathBuf::from(trimmed);
            let _ = ensure_drive_subdirs(&custom_root);
            return (custom_root, "Google Drive Sync Vault".to_string());
        }
    }

    let mut candidates: Vec<(PathBuf, &str)> = Vec::new();

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let up = PathBuf::from(&user_profile);
        candidates.push((up.join("Google Drive").join("My Drive"), "Google Drive Desktop"));
        candidates.push((up.join("Google Drive"), "Google Drive Desktop"));
        candidates.push((up.join("My Drive"), "Google Drive Desktop"));
    }
    candidates.push((PathBuf::from(r"G:\My Drive"), "Google Drive (G:)"));
    candidates.push((PathBuf::from(r"H:\My Drive"), "Google Drive (H:)"));

    for (base, label) in candidates {
        if base.exists() && base.is_dir() {
            let root = base.join("ExamPlus");
            let _ = ensure_drive_subdirs(&root);
            return (root, label.to_string());
        }
    }

    let local_root = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("ExamPlus_Drive")
        .join("ExamPlus");
    let _ = ensure_drive_subdirs(&local_root);
    (local_root, "Google Drive Sync Vault".to_string())
}

fn ensure_drive_subdirs(root: &Path) -> std::io::Result<()> {
    let word_dir = root.join("DeThi_Word");
    let pdf_dir = root.join("DeThi_PDF");
    let excel_dir = root.join("BangDiem_Excel");
    let config_dir = root.join("SaoLuu_CauHinh");

    fs::create_dir_all(&word_dir)?;
    fs::create_dir_all(&pdf_dir)?;
    fs::create_dir_all(&excel_dir)?;
    fs::create_dir_all(&config_dir)?;

    Ok(())
}

fn session_file_path(root: &Path) -> PathBuf {
    root.join("SaoLuu_CauHinh").join("google_session.json")
}

fn profile_config_path(root: &Path) -> PathBuf {
    root.join("SaoLuu_CauHinh").join("profile.epconfig")
}

fn private_vault_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("EXAMPLUS_VAULT_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_appdata).join("ExamPlus").join(".vault")
    } else {
        std::env::temp_dir().join("ExamPlus_Private_Vault")
    }
}

fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn vault_cache_key(dir: &Path, clean_email: &str) -> String {
    format!("{}::{}", dir.display(), clean_email)
}

fn vault_token_file(dir: &Path, clean_email: &str) -> PathBuf {
    dir.join(format!("{}_token.json", sanitize_for_filename(clean_email)))
}

fn vault_session_backup_file(dir: &Path) -> PathBuf {
    dir.join("active_session.json")
}

fn should_use_vault_session_fallback() -> bool {
    std::env::var("EXAMPLUS_VAULT_DIR")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("EXAMPLUS_DISABLE_DRIVE_MIRROR").as_deref() != Ok("1")
}

fn hidden_curl_cmd() -> std::process::Command {
    let mut cmd = std::process::Command::new("curl.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Debug)]
struct HttpRawResponse {
    status: u16,
    body: Vec<u8>,
}

pub fn parse_curl_http_output(raw: &[u8]) -> Result<(u16, Vec<u8>), String> {
    if let Some(pos) = raw.iter().rposition(|&b| b == b'\n') {
        let code_str = String::from_utf8_lossy(&raw[pos + 1..]).trim().to_string();
        if let Ok(code) = code_str.parse::<u16>() {
            return Ok((code, raw[..pos].to_vec()));
        }
    }
    let code_str = String::from_utf8_lossy(raw).trim().to_string();
    if raw.len() <= 5 {
        if let Ok(code) = code_str.parse::<u16>() {
            return Ok((code, Vec::new()));
        }
    }
    Err("Phản hồi HTTP từ Google không hợp lệ".to_string())
}

fn curl_http_request(args: &[&str]) -> Result<HttpRawResponse, String> {
    let mut cmd = hidden_curl_cmd();
    cmd.args(args);
    cmd.args(["--connect-timeout", "10", "--max-time", "45", "-w", "\n%{http_code}"]);
    let output = cmd
        .output()
        .map_err(|e| format!("Lỗi thực thi curl: {}", e))?;
    if !output.status.success() && output.stdout.is_empty() {
        return Err("Lỗi mạng khi kết nối tới Google API".to_string());
    }
    let (status, body) = parse_curl_http_output(&output.stdout)?;
    Ok(HttpRawResponse { status, body })
}

pub fn has_drive_readonly_scope(scopes: Option<&str>) -> bool {
    scopes
        .unwrap_or("")
        .split_whitespace()
        .any(|s| {
            s == "https://www.googleapis.com/auth/drive.readonly"
                || s == "https://www.googleapis.com/auth/drive"
        })
}

pub fn is_valid_drive_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn escape_drive_query_literal(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_control() {
            continue;
        }
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn build_cloud_files_query(
    tab: &str,
    folder_id: Option<&str>,
    search: Option<&str>,
    file_type: &str,
) -> Result<String, String> {
    let clean_tab = match tab.trim().to_lowercase().as_str() {
        "mydrive" | "shared" | "starred" => tab.trim().to_lowercase(),
        _ => "recent".to_string(),
    };

    let clean_folder = folder_id.map(|s| s.trim()).filter(|s| !s.is_empty());
    if let Some(fid) = clean_folder {
        if !is_valid_drive_id(fid) {
            return Err("Mã thư mục Google Drive không hợp lệ".to_string());
        }
    }

    let search_trimmed = search.map(|s| s.trim()).unwrap_or("");
    let include_folders = clean_tab != "recent" || !search_trimmed.is_empty();

    let type_clause = match file_type.trim().to_lowercase().as_str() {
        "docx" => {
            if include_folders {
                "(mimeType = 'application/vnd.google-apps.folder' or mimeType = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' or mimeType = 'application/vnd.google-apps.document' or name contains '.docx')"
            } else {
                "(mimeType = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' or mimeType = 'application/vnd.google-apps.document' or name contains '.docx')"
            }
        }
        "pdf" => {
            if include_folders {
                "(mimeType = 'application/vnd.google-apps.folder' or mimeType = 'application/pdf' or name contains '.pdf')"
            } else {
                "(mimeType = 'application/pdf' or name contains '.pdf')"
            }
        }
        _ => {
            if include_folders {
                "(mimeType = 'application/vnd.google-apps.folder' or mimeType = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' or mimeType = 'application/pdf' or mimeType = 'application/vnd.google-apps.document' or name contains '.docx' or name contains '.pdf')"
            } else {
                "(mimeType = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' or mimeType = 'application/pdf' or mimeType = 'application/vnd.google-apps.document' or name contains '.docx' or name contains '.pdf')"
            }
        }
    };

    let mut parts = vec!["trashed = false".to_string(), type_clause.to_string()];

    match clean_tab.as_str() {
        "mydrive" => {
            let parent = clean_folder.unwrap_or("root");
            parts.push(format!("'{}' in parents", parent));
        }
        "shared" => {
            if let Some(fid) = clean_folder.filter(|f| *f != "root") {
                parts.push(format!("'{}' in parents", fid));
            } else {
                parts.push("sharedWithMe = true".to_string());
            }
        }
        "starred" => {
            if let Some(fid) = clean_folder.filter(|f| *f != "root") {
                parts.push(format!("'{}' in parents", fid));
            } else {
                parts.push("starred = true".to_string());
            }
        }
        _ => {
            if !search_trimmed.is_empty() {
                // Keep folders discoverable during search too
            } else {
                parts.push("mimeType != 'application/vnd.google-apps.folder'".to_string());
            }
        }
    }

    if !search_trimmed.is_empty() {
        let escaped = escape_drive_query_literal(search_trimmed);
        parts.push(format!("name contains '{}'", escaped));
    }

    Ok(parts.join(" and "))
}

pub fn get_stored_token_bundle(email: &str) -> Option<StoredOAuthTokens> {
    let clean = email.trim().to_lowercase();
    if clean.is_empty() {
        return None;
    }
    let dir = private_vault_dir();
    let cache_key = vault_cache_key(&dir, &clean);
    let file = vault_token_file(&dir, &clean);
    if let Ok(data) = fs::read_to_string(&file) {
        if let Ok(mut bundle) = serde_json::from_str::<StoredOAuthTokens>(&data) {
            if bundle.email.trim().is_empty() {
                bundle.email = clean.clone();
            }
            if !bundle.access_token.trim().is_empty()
                || bundle
                    .refresh_token
                    .as_deref()
                    .map(|r| !r.trim().is_empty())
                    .unwrap_or(false)
            {
                if let Ok(mut guard) = BACKEND_TOKEN_VAULT.lock() {
                    guard.insert(cache_key, bundle.clone());
                }
                return Some(bundle);
            }
        }
    }
    if let Ok(guard) = BACKEND_TOKEN_VAULT.lock() {
        if let Some(bundle) = guard.get(&cache_key) {
            if !bundle.access_token.trim().is_empty()
                || bundle
                    .refresh_token
                    .as_deref()
                    .map(|r| !r.trim().is_empty())
                    .unwrap_or(false)
            {
                return Some(bundle.clone());
            }
        }
    }
    None
}

pub fn save_backend_token_bundle(
    email: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_in: Option<i64>,
    scopes: Option<&str>,
) -> StoredOAuthTokens {
    let clean = email.trim().to_lowercase();
    let dir = private_vault_dir();
    let cache_key = vault_cache_key(&dir, &clean);

    let existing = get_stored_token_bundle(&clean);
    let merged_refresh = refresh_token
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|e| e.refresh_token.clone())
                .filter(|s| !s.trim().is_empty())
        });
    let merged_scopes = scopes
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|e| e.scopes.clone())
                .filter(|s| !s.trim().is_empty())
        });
    let expires_at = expires_in.map(|secs| Utc::now().timestamp() + secs.max(60));

    let bundle = StoredOAuthTokens {
        email: clean.clone(),
        access_token: access_token.to_string(),
        refresh_token: merged_refresh,
        expires_at,
        scopes: merged_scopes,
        updated_at: Utc::now().to_rfc3339(),
    };

    if let Ok(mut guard) = BACKEND_TOKEN_VAULT.lock() {
        guard.insert(cache_key, bundle.clone());
    }
    if fs::create_dir_all(&dir).is_ok() {
        let file = vault_token_file(&dir, &clean);
        if let Ok(json_str) = serde_json::to_string_pretty(&bundle) {
            let _ = fs::write(file, json_str);
        }
    }
    bundle
}

pub fn save_backend_token(email: &str, token: &str) {
    let _ = save_backend_token_bundle(email, token, None, None, None);
}

pub fn get_backend_token(email: &str) -> Option<String> {
    let bundle = get_stored_token_bundle(email)?;
    if !bundle.access_token.trim().is_empty() {
        Some(bundle.access_token)
    } else {
        None
    }
}

pub fn clear_backend_tokens(email: &str) {
    let clean = email.trim().to_lowercase();
    let dir = private_vault_dir();
    let cache_key = vault_cache_key(&dir, &clean);
    if let Ok(mut guard) = BACKEND_TOKEN_VAULT.lock() {
        guard.remove(&cache_key);
    }
    let _ = fs::remove_file(vault_token_file(&dir, &clean));
}

pub fn load_session(root: &Path) -> Option<GoogleUserSession> {
    let p = session_file_path(root);
    if let Ok(data) = fs::read_to_string(p) {
        if let Ok(sess) = serde_json::from_str(&data) {
            return Some(sess);
        }
    }
    if should_use_vault_session_fallback() {
        let backup = vault_session_backup_file(&private_vault_dir());
        if let Ok(data) = fs::read_to_string(backup) {
            if let Ok(sess) = serde_json::from_str::<GoogleUserSession>(&data) {
                let _ = fs::create_dir_all(root.join("SaoLuu_CauHinh"));
                if let Ok(json) = serde_json::to_string_pretty(&sess) {
                    let _ = fs::write(session_file_path(root), json);
                }
                return Some(sess);
            }
        }
    }
    None
}

pub fn save_session(root: &Path, session: &GoogleUserSession) -> std::io::Result<()> {
    let p = session_file_path(root);
    // Nhờ #[serde(skip_serializing)] trên access_token, tệp json này tuyệt đối không lưu access_token
    let json = serde_json::to_string_pretty(session).unwrap_or_default();
    fs::write(p, &json)?;

    if should_use_vault_session_fallback() {
        let vault_dir = private_vault_dir();
        if fs::create_dir_all(&vault_dir).is_ok() {
            let _ = fs::write(vault_session_backup_file(&vault_dir), &json);
        }
    }

    // Đồng thời lưu vào profile.epconfig để đồng bộ cấu hình
    let cfg_path = profile_config_path(root);
    let cfg_payload = serde_json::json!({
        "app": "ExamPlus Teacher Studio",
        "version": "1.0.0",
        "updated_at": Utc::now().format("%d/%m/%Y %H:%M:%S").to_string(),
        "teacher_profile": {
            "name": session.name,
            "email": session.email,
            "email_locked": true,
            "avatar": session.avatar,
            "provider": session.provider,
        },
        "drive_sync": {
            "enabled": true,
            "auto_backup_docx": true,
            "auto_backup_xlsx": true,
            "last_sync": session.last_sync,
        }
    });
    fs::write(
        cfg_path,
        serde_json::to_string_pretty(&cfg_payload).unwrap_or_default(),
    )?;
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn list_all_drive_files(root: &Path) -> (Vec<DriveFileItem>, u64) {
    let mut items = Vec::new();
    let mut total_bytes = 0u64;

    let subdirs = [
        ("DeThi_Word", "docx"),
        ("DeThi_PDF", "pdf"),
        ("BangDiem_Excel", "xlsx"),
        ("SaoLuu_CauHinh", "config"),
    ];

    for (folder, default_cat) in subdirs {
        let dir_path = root.join(folder);
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let name = match path.file_name().and_then(|s| s.to_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    if name == "google_session.json" {
                        continue;
                    }
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let sz = meta.len();
                    total_bytes += sz;

                    let modified_at = meta
                        .modified()
                        .ok()
                        .map(|t| {
                            let dt: chrono::DateTime<Utc> = t.into();
                            dt.format("%d/%m/%Y %H:%M").to_string()
                        })
                        .unwrap_or_else(|| Utc::now().format("%d/%m/%Y %H:%M").to_string());

                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let cat = if ext == "docx" {
                        "docx"
                    } else if ext == "pdf" {
                        "pdf"
                    } else if ext == "xlsx" {
                        "xlsx"
                    } else {
                        default_cat
                    };

                    items.push(DriveFileItem {
                        name,
                        category: cat.to_string(),
                        folder: folder.to_string(),
                        size_bytes: sz,
                        size_formatted: format_bytes(sz),
                        modified_at,
                    });
                }
            }
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    (items, total_bytes)
}

/// Lưu bản sao tệp .docx vừa tải lên vào thư mục Google Drive (DeThi_Word)
pub fn mirror_docx_to_drive(filename_hint: &str, bytes: &[u8]) {
    mirror_exam_to_drive(filename_hint, bytes, examplus_core::parser::ExamFileFormat::Docx);
}

/// Lưu bản sao tệp đề thi (.docx hoặc .pdf) vừa tải lên vào thư mục Google Drive (DeThi_Word / DeThi_PDF)
pub fn mirror_exam_to_drive(
    filename_hint: &str,
    bytes: &[u8],
    format: examplus_core::parser::ExamFileFormat,
) {
    if std::env::var("EXAMPLUS_DISABLE_DRIVE_MIRROR").as_deref() == Ok("1") {
        return;
    }
    let (root, _) = resolve_drive_root();
    let (subdir, ext) = match format {
        examplus_core::parser::ExamFileFormat::Pdf => ("DeThi_PDF", ".pdf"),
        examplus_core::parser::ExamFileFormat::Docx => ("DeThi_Word", ".docx"),
    };
    let safe_name: String = filename_hint
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = safe_name.trim();
    let final_name = if trimmed.to_lowercase().ends_with(ext) {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        format!("De_Thi_{}{}", Utc::now().format("%Y%m%d_%H%M%S"), ext)
    } else {
        let stem = trimmed
            .trim_end_matches(".docx")
            .trim_end_matches(".DOCX")
            .trim_end_matches(".pdf")
            .trim_end_matches(".PDF");
        format!("{}{}", stem, ext)
    };
    let folder_path = root.join(subdir);
    let _ = fs::create_dir_all(&folder_path);
    let dest = folder_path.join(final_name);
    let _ = fs::write(dest, bytes);
}

/// Lưu bản sao bảng điểm .xlsx vừa xuất vào thư mục Google Drive (BangDiem_Excel)
pub fn mirror_xlsx_to_drive(exam_title: &str, bytes: &[u8]) {
    if std::env::var("EXAMPLUS_DISABLE_DRIVE_MIRROR").as_deref() == Ok("1") {
        return;
    }
    let (root, _) = resolve_drive_root();
    let slug: String = exam_title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(40)
        .collect();
    let name = format!(
        "BangDiem_{}_{}.xlsx",
        if slug.is_empty() { "ExamPlus" } else { &slug },
        Utc::now().format("%Y%m%d_%H%M")
    );
    let dest = root.join("BangDiem_Excel").join(name);
    let _ = fs::write(dest, bytes);
}

fn default_oauth_client_credentials() -> (String, String) {
    if let (Ok(cid), Ok(csec)) = (
        std::env::var("GOOGLE_CLIENT_ID"),
        std::env::var("GOOGLE_CLIENT_SECRET"),
    ) {
        if !cid.trim().is_empty() && !csec.trim().is_empty() {
            return (cid.trim().to_string(), csec.trim().to_string());
        }
    }

    let candidate_files = [
        PathBuf::from("google_oauth.json"),
        PathBuf::from("client_secret.json"),
    ];
    for p in candidate_files {
        if p.exists() {
            if let Ok(raw) = fs::read_to_string(&p) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                    let node = val
                        .get("installed")
                        .or_else(|| val.get("web"))
                        .unwrap_or(&val);
                    let cid = node
                        .get("client_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let csec = node
                        .get("client_secret")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    if !cid.is_empty() && !csec.is_empty() && !cid.starts_with("THAY_BANG_") {
                        return (cid.to_string(), csec.to_string());
                    }
                }
            }
        }
    }

    // Mặc định trả về placeholder an toàn cho client_id để hỗ trợ luồng PKCE S256 mà không lộ secret
    let cid = std::env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| "examplus-public-client.apps.googleusercontent.com".to_string());
    (cid, String::new())
}

pub fn generate_code_verifier() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn compute_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

pub fn urlencoding(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// GET /api/v1/auth/google/start
/// Khởi tạo giao dịch OAuth 2.0 PKCE S256 và chuyển hướng 302 tới Google:
/// - flow=picker: mở Google Picker chính thức trên desktop (`trigger_onepick=true`, scope `drive.file`)
/// - flow=login: đăng nhập tài khoản Google (`openid email profile drive.file`)
pub async fn google_auth_start_page(
    State(state): State<AppState>,
    Query(query): Query<GoogleStartQuery>,
) -> impl IntoResponse {
    let (client_id, _) = default_oauth_client_credentials();
    let redirect_uri = format!(
        "http://127.0.0.1:{}/api/v1/auth/google/callback",
        state.server_port
    );

    let verifier = generate_code_verifier();
    let challenge = compute_code_challenge(&verifier);
    let state_token = Uuid::new_v4().to_string();

    let flow = query.flow.as_deref().unwrap_or("login").to_string();

    {
        let mut map = OAUTH_TRANSACTIONS.lock().unwrap();
        let now = Utc::now();
        map.retain(|_, v| (now - v.created_at).num_seconds() < 300);
        map.insert(
            state_token.clone(),
            OAuthTransaction {
                flow: flow.clone(),
                code_verifier: verifier,
                created_at: now,
            },
        );
    }

    let url = if flow == "picker" {
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdrive.file&access_type=offline&prompt=consent&trigger_onepick=true&code_challenge={}&code_challenge_method=S256&state={}&hl=vi",
            urlencoding(&client_id),
            urlencoding(&redirect_uri),
            urlencoding(&challenge),
            urlencoding(&state_token),
        )
    } else {
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdrive.file%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdrive.readonly&access_type=offline&include_granted_scopes=true&prompt=select_account%20consent&code_challenge={}&code_challenge_method=S256&state={}&hl=vi",
            urlencoding(&client_id),
            urlencoding(&redirect_uri),
            urlencoding(&challenge),
            urlencoding(&state_token),
        )
    };

    Redirect::temporary(&url).into_response()
}

#[derive(Debug, Clone)]
pub struct ParsedTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    pub scope: Option<String>,
}

pub fn parse_token_exchange_response(body: &[u8]) -> Result<ParsedTokenResponse, String> {
    let json_val: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| format!("Lỗi phân tích JSON phản hồi từ Google: {}", e))?;

    if let Some(err) = json_val.get("error").and_then(|v| v.as_str()) {
        let desc = json_val
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        tracing::warn!("OAuth token exchange error: {}", err);
        return Err(format!("Google từ chối đổi mã: {} ({})", err, desc));
    }

    let access_token = json_val
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Không tìm thấy access_token trong phản hồi".to_string())?;

    let refresh_token = json_val
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let expires_in = json_val
        .get("expires_in")
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)));

    let scope = json_val
        .get("scope")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(ParsedTokenResponse {
        access_token,
        refresh_token,
        expires_in,
        scope,
    })
}

fn exchange_google_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<ParsedTokenResponse, String> {
    let p_code = format!("code={}", urlencoding(code));
    let p_cid = format!("client_id={}", urlencoding(client_id));
    let p_csec = format!("client_secret={}", urlencoding(client_secret));
    let p_redir = format!("redirect_uri={}", urlencoding(redirect_uri));
    let p_ver = format!("code_verifier={}", urlencoding(code_verifier));

    let resp = curl_http_request(&[
        "-s",
        "-X",
        "POST",
        "https://oauth2.googleapis.com/token",
        "-d",
        &p_code,
        "-d",
        &p_cid,
        "-d",
        &p_csec,
        "-d",
        &p_redir,
        "-d",
        "grant_type=authorization_code",
        "-d",
        &p_ver,
    ])?;

    if resp.status >= 400 && resp.body.is_empty() {
        return Err(format!("Google từ chối yêu cầu (HTTP {})", resp.status));
    }

    parse_token_exchange_response(&resp.body)
}

pub fn refresh_google_access_token(email: &str) -> Result<StoredOAuthTokens, String> {
    let existing = get_stored_token_bundle(email)
        .ok_or_else(|| "MISSING_CREDENTIALS".to_string())?;
    let refresh_tok = existing
        .refresh_token
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "MISSING_REFRESH_TOKEN".to_string())?
        .to_string();

    let (client_id, client_secret) = default_oauth_client_credentials();
    let p_cid = format!("client_id={}", urlencoding(&client_id));
    let p_csec = format!("client_secret={}", urlencoding(&client_secret));
    let p_rt = format!("refresh_token={}", urlencoding(&refresh_tok));

    let resp = curl_http_request(&[
        "-s",
        "-X",
        "POST",
        "https://oauth2.googleapis.com/token",
        "-d",
        &p_cid,
        "-d",
        &p_csec,
        "-d",
        &p_rt,
        "-d",
        "grant_type=refresh_token",
    ])?;

    if resp.status == 400 || resp.status == 401 {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&resp.body) {
            if let Some(err) = json_val.get("error").and_then(|v| v.as_str()) {
                if err == "invalid_grant" {
                    return Err("REFRESH_REVOKED".to_string());
                }
            }
        }
    }

    if resp.status >= 400 {
        return Err(format!("Không thể làm mới phiên Google (HTTP {})", resp.status));
    }

    let parsed = parse_token_exchange_response(&resp.body)?;
    Ok(save_backend_token_bundle(
        email,
        &parsed.access_token,
        parsed.refresh_token.as_deref(),
        parsed.expires_in,
        parsed.scope.as_deref(),
    ))
}

pub fn ensure_valid_access_token(
    email: &str,
    force_refresh: bool,
) -> Result<StoredOAuthTokens, String> {
    let bundle = get_stored_token_bundle(email)
        .ok_or_else(|| "MISSING_CREDENTIALS".to_string())?;

    let now = Utc::now().timestamp();
    let is_expiring = bundle
        .expires_at
        .map(|exp| exp <= now + 90)
        .unwrap_or(false);
    let has_access = !bundle.access_token.trim().is_empty();
    let has_refresh = bundle
        .refresh_token
        .as_deref()
        .map(|r| !r.trim().is_empty())
        .unwrap_or(false);

    if force_refresh || !has_access || is_expiring {
        if has_refresh {
            return refresh_google_access_token(email);
        }
        if force_refresh || !has_access || is_expiring {
            return Err("MISSING_REFRESH_TOKEN".to_string());
        }
    }

    Ok(bundle)
}

fn fetch_google_userinfo(access_token: &str) -> Result<(String, String, String), String> {
    let auth_header = format!("Authorization: Bearer {}", access_token);
    let resp = curl_http_request(&[
        "-s",
        "https://www.googleapis.com/oauth2/v3/userinfo",
        "-H",
        &auth_header,
    ])?;

    if resp.status >= 400 {
        return Err(format!("Google UserInfo trả về HTTP {}", resp.status));
    }

    let u_json: serde_json::Value = serde_json::from_slice(&resp.body)
        .map_err(|e| format!("Lỗi phân tích JSON hồ sơ: {}", e))?;

    let email = u_json
        .get("email")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Google không trả về email người dùng".to_string())?
        .trim()
        .to_string();

    let name = u_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| email.split('@').next().unwrap_or("Giáo Viên"))
        .to_string();

    let picture = u_json
        .get("picture")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "https://ui-avatars.com/api/?name={}&background=2563eb&color=fff&bold=true&size=128",
                name.replace(' ', "+")
            )
        });

    Ok((email, name, picture))
}

fn fetch_drive_file_metadata(access_token: &str, file_id: &str) -> Option<PickedFileMetadata> {
    if !is_valid_drive_id(file_id) {
        return None;
    }
    let auth_header = format!("Authorization: Bearer {}", access_token);
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?fields=id,name,mimeType,size,modifiedTime",
        file_id
    );
    let resp = curl_http_request(&["-s", &url, "-H", &auth_header]).ok()?;

    if resp.status >= 400 {
        return None;
    }

    let val: serde_json::Value = serde_json::from_slice(&resp.body).ok()?;
    let name = val
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Tệp không tên")
        .to_string();
    let mime_type = val
        .get("mimeType")
        .and_then(|v| v.as_str())
        .unwrap_or("application/octet-stream")
        .to_string();
    let size_bytes = val
        .get("size")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| val.get("size").and_then(|v| v.as_u64()));
    let modified_time = val
        .get("modifiedTime")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let size_formatted = format_bytes(size_bytes.unwrap_or(0));

    Some(PickedFileMetadata {
        file_id: file_id.to_string(),
        name,
        mime_type,
        size_bytes,
        size_formatted,
        modified_time,
        picked_at: Utc::now().format("%d/%m/%Y %H:%M:%S").to_string(),
    })
}

fn render_error_page(title: &str, message_html: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="vi">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{} — ExamPlus</title>
<style>
  body {{
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, Roboto, sans-serif;
    background: #f0f4f9;
    color: #1f1f1f;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    margin: 0;
    padding: 20px;
    box-sizing: border-box;
  }}
  .card {{
    background: #fff;
    padding: 36px;
    border-radius: 24px;
    max-width: 460px;
    width: 100%;
    text-align: center;
    box-shadow: 0 10px 30px rgba(0,0,0,0.08);
  }}
  .icon {{
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: #fce8e6;
    color: #d93025;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 16px;
    font-size: 26px;
  }}
  h2 {{
    font-size: 20px;
    margin: 0 0 10px;
    color: #b3261e;
  }}
  .desc {{
    font-size: 13.5px;
    color: #444746;
    line-height: 1.5;
    margin-bottom: 24px;
  }}
  .btn {{
    display: inline-block;
    background: #1a73e8;
    color: #fff;
    padding: 10px 20px;
    border-radius: 10px;
    text-decoration: none;
    font-size: 13.5px;
    font-weight: 600;
    border: none;
    cursor: pointer;
  }}
  .btn-subtle {{
    background: #f1f3f4;
    color: #3c4043;
    margin-left: 8px;
  }}
</style>
</head>
<body>
  <div class="card">
    <div class="icon">⚠️</div>
    <h2>{}</h2>
    <div class="desc">{}</div>
    <a href="/api/v1/auth/google/start" class="btn">Thử lại</a>
    <button onclick="window.close()" class="btn btn-subtle">Đóng Tab</button>
  </div>
</body>
</html>"#,
        html_escape(title),
        html_escape(title),
        message_html
    )
}

fn render_success_page(title: &str, body_html: &str, auto_close: bool) -> String {
    let script = if auto_close {
        r#"<script>setTimeout(() => { try { window.close(); } catch(e) {} }, 1800);</script>"#
    } else {
        ""
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="vi">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{}</title>
<style>
  body {{
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, Roboto, sans-serif;
    background: #f0f4f9;
    color: #1f1f1f;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    margin: 0;
    padding: 20px;
    box-sizing: border-box;
  }}
  .card {{
    background: #fff;
    padding: 40px;
    border-radius: 28px;
    max-width: 460px;
    width: 100%;
    text-align: center;
    box-shadow: 0 12px 36px rgba(0,0,0,0.08);
  }}
  .avatar {{
    width: 72px;
    height: 72px;
    border-radius: 50%;
    object-fit: cover;
    margin-bottom: 14px;
    border: 3px solid #e8f0fe;
  }}
  .check-icon {{
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: #e6f4ea;
    color: #137333;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 16px;
    font-size: 26px;
  }}
</style>
</head>
<body>
  <div class="card">
    {}
    <div style="margin-top: 20px;">
      <button onclick="window.close()" style="background:#f1f3f4;color:#3c4043;border:none;padding:8px 18px;border-radius:8px;font-size:13px;font-weight:600;cursor:pointer;">Đóng Tab Này</button>
    </div>
  </div>
  {}
</body>
</html>"#,
        html_escape(title),
        body_html,
        script
    )
}

/// GET /api/v1/auth/google/callback
/// Nhận mã ủy quyền (`?code=...`), xác thực `state` dùng một lần, đổi mã bằng PKCE `code_verifier`,
/// lấy thông tin người dùng từ Google và hỗ trợ luồng chọn tệp Google Picker (`picked_file_ids`).
pub async fn google_auth_callback(
    State(state): State<AppState>,
    Query(query): Query<GoogleCallbackQuery>,
) -> (StatusCode, Html<String>) {
    // 1. Kiểm tra mã state một lần (one-time state)
    let tx = {
        let mut map = OAUTH_TRANSACTIONS.lock().unwrap();
        let now = Utc::now();
        map.retain(|_, v| (now - v.created_at).num_seconds() < 300);

        let state_key = query.state.as_deref().unwrap_or("").trim();
        if state_key.is_empty() {
            None
        } else {
            map.remove(state_key)
        }
    };

    let tx = match tx {
        Some(t) => t,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Html(render_error_page(
                    "Yêu cầu không hợp lệ hoặc phiên đã hết hạn",
                    "Mã state bảo mật không hợp lệ hoặc đã qua sử dụng. Vui lòng quay lại ExamPlus và thử lại.",
                )),
            );
        }
    };

    // 2. Xử lý khi Google trả về lỗi hoặc người dùng hủy
    if let Some(ref err) = query.error {
        return (
            StatusCode::OK,
            Html(render_error_page(
                "Chưa hoàn tất ủy quyền Google",
                &format!(
                    "Google trả về: <code>{}</code>. Phiên hiện tại của bạn vẫn được giữ nguyên. Bạn có thể đóng tab này và thử lại từ ExamPlus.",
                    html_escape(err)
                ),
            )),
        );
    }

    // 3. Kiểm tra authorization code
    let code = match query.code.as_deref() {
        Some(c) if !c.trim().is_empty() => c.trim(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Html(render_error_page(
                    "Thiếu mã xác thực",
                    "Không tìm thấy mã ủy quyền OAuth 2.0 từ phản hồi của Google.",
                )),
            );
        }
    };

    let (client_id, client_secret) = default_oauth_client_credentials();
    let redirect_uri = format!(
        "http://127.0.0.1:{}/api/v1/auth/google/callback",
        state.server_port
    );

    // 4. Đổi mã ủy quyền + PKCE code_verifier lấy access_token + refresh_token
    let token_resp = match exchange_google_code(
        &client_id,
        &client_secret,
        code,
        &redirect_uri,
        &tx.code_verifier,
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("Google code exchange failed: [REDACTED]");
            return (
                StatusCode::BAD_REQUEST,
                Html(render_error_page(
                    "Đổi mã xác thực không thành công",
                    &format!(
                        "Không thể đổi mã ủy quyền Google lấy token: {}. Phiên hiện tại của bạn vẫn được giữ nguyên. Vui lòng thử lại.",
                        html_escape(&e)
                    ),
                )),
            );
        }
    };
    let access_token = token_resp.access_token.clone();

    let (root, _) = resolve_drive_root();

    // 5. Phân nhánh luồng xử lý: Picker vs Login
    if tx.flow == "picker" {
        let mut picked_metadata: Option<PickedFileMetadata> = None;
        if let Some(ref ids) = query.picked_file_ids {
            let first_id = ids.split(',').next().unwrap_or(ids).trim();
            if !first_id.is_empty() {
                picked_metadata = fetch_drive_file_metadata(&access_token, first_id);
            }
        }

        if let Some(ref meta) = picked_metadata {
            *LATEST_PICKED_FILE.lock().unwrap() = Some(meta.clone());
        }

        if let Some(mut sess) = load_session(&root) {
            sess.drive_connected = true;
            sess.last_sync = Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
            let _ = save_backend_token_bundle(
                &sess.email,
                &access_token,
                token_resp.refresh_token.as_deref(),
                token_resp.expires_in,
                token_resp.scope.as_deref(),
            );
            let _ = save_session(&root, &sess);
        }

        let body_content = if let Some(ref m) = picked_metadata {
            format!(
                r#"<div class="check-icon">✓</div>
                   <h2 style="font-size:20px;margin:0 0 8px;color:#137333;">Đã Chọn Tệp Google Drive Thành Công</h2>
                   <div style="background:#e8f0fe;border-radius:14px;padding:16px;margin:16px 0;text-align:left;">
                    <div style="font-weight:700;color:#1967d2;font-size:14px;margin-bottom:6px;">📄 Tệp Google Drive đã chọn:</div>
                    <div style="font-size:13.5px;color:#1f1f1f;word-break:break-all;"><strong>{}</strong> ({})</div>
                    <div style="font-size:11px;color:#5f6368;margin-top:4px;">ID: <code>{}</code></div>
                   </div>
                   <p style="font-size:13px;color:#444746;">Thông tin tệp đã được chuyển an toàn về ExamPlus. Bạn có thể đóng tab trình duyệt này để tiếp tục.</p>"#,
                html_escape(&m.name),
                m.size_formatted,
                html_escape(&m.file_id)
            )
        } else {
            r#"<div class="check-icon">✓</div>
               <h2 style="font-size:20px;margin:0 0 8px;color:#137333;">Cấp Quyền Google Drive Thành Công</h2>
               <p style="color:#137333;font-size:14px;margin:14px 0;font-weight:600;">Đã hoàn tất kết nối &amp; cấp quyền Google Drive thành công!</p>
               <p style="font-size:13px;color:#444746;">Ứng dụng ExamPlus đã nhận quyền truy cập Google Drive. Bạn có thể đóng tab này.</p>"#.to_string()
        };

        return (
            StatusCode::OK,
            Html(render_success_page(
                "Chọn Tệp Google Drive Thành Công — ExamPlus",
                &body_content,
                true,
            )),
        );
    }

    // 6. Luồng đăng nhập thông thường (flow == "login" | "connect")
    let (email, name, picture) = match fetch_google_userinfo(&access_token) {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!("Failed to fetch userinfo from Google: [REDACTED]");
            return (
                StatusCode::BAD_REQUEST,
                Html(render_error_page(
                    "Không lấy được thông tin hồ sơ Google",
                    &format!(
                        "Lỗi khi lấy thông tin tài khoản: {}. Vui lòng thử lại.",
                        html_escape(&e)
                    ),
                )),
            );
        }
    };

    let saved_bundle = save_backend_token_bundle(
        &email,
        &access_token,
        token_resp.refresh_token.as_deref(),
        token_resp.expires_in,
        token_resp.scope.as_deref(),
    );
    let _ = ensure_cloud_drive_folder(&access_token);

    let now_str = Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
    let session = GoogleUserSession {
        provider: "google".to_string(),
        name: name.clone(),
        email: email.clone(),
        avatar: picture.clone(),
        drive_connected: true,
        access_token: None, // Bảo mật: Không lưu access_token vào session JSON
        connected_at: now_str.clone(),
        last_sync: now_str,
    };
    let _ = save_session(&root, &session);

    let persist_note = if saved_bundle
        .refresh_token
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        "Đã lưu phiên đăng nhập tự động. Từ nay bạn có thể mở trực tiếp cửa sổ chọn Google Drive ngay trong ExamPlus mà không cần xác nhận lại."
    } else {
        "Ứng dụng ExamPlus đã tự động đăng nhập. Bạn có thể đóng tab trình duyệt này và quay lại ExamPlus."
    };

    let success_body = format!(
        r#"<img class="avatar" src="{}" alt="Avatar">
        <h2 style="font-size:20px;margin:0 0 6px;">Xin chào, {}!</h2>
        <p style="font-size:13.5px;color:#444746;margin:0 0 16px;">Đã xác thực tài khoản Google <strong>{}</strong> &amp; liên kết Google Drive thành công.</p>
        <div style="background:#e6f4ea;color:#137333;padding:10px 16px;border-radius:12px;font-size:13px;font-weight:600;">
          {}
        </div>"#,
        html_escape(&picture),
        html_escape(&name),
        html_escape(&email),
        html_escape(persist_note),
    );

    (
        StatusCode::OK,
        Html(render_success_page(
            "Đăng nhập Google thành công — ExamPlus",
            &success_body,
            true,
        )),
    )
}

fn ensure_cloud_drive_folder(access_token: &str) -> Option<String> {
    let auth_header = format!("Authorization: Bearer {}", access_token);
    let check_resp = curl_http_request(&[
        "-s",
        "https://www.googleapis.com/drive/v3/files?q=mimeType%3D'application%2Fvnd.google-apps.folder'+and+name%3D'ExamPlus'+and+trashed%3Dfalse&fields=files(id,name)",
        "-H",
        &auth_header,
    ])
    .ok()?;

    if check_resp.status < 400 {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&check_resp.body) {
            if let Some(files) = json_val.get("files").and_then(|v| v.as_array()) {
                if let Some(first_id) = files
                    .first()
                    .and_then(|f| f.get("id"))
                    .and_then(|v| v.as_str())
                {
                    return Some(first_id.to_string());
                }
            }
        }
    }

    let create_resp = curl_http_request(&[
        "-s",
        "-X",
        "POST",
        "https://www.googleapis.com/drive/v3/files",
        "-H",
        &auth_header,
        "-H",
        "Content-Type: application/json",
        "-d",
        r#"{"name":"ExamPlus","mimeType":"application/vnd.google-apps.folder"}"#,
    ])
    .ok()?;

    if create_resp.status < 400 {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&create_resp.body) {
            if let Some(id) = json_val.get("id").and_then(|v| v.as_str()) {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn upload_file_to_cloud_drive(access_token: &str, filename: &str, file_path: &Path) {
    if access_token.trim().is_empty() || !file_path.exists() {
        return;
    }
    let folder_id = match ensure_cloud_drive_folder(access_token) {
        Some(id) => id,
        None => return,
    };
    let auth_header = format!("Authorization: Bearer {}", access_token);

    let create_meta = serde_json::json!({
        "name": filename,
        "parents": [folder_id]
    });
    if let Ok(meta_resp) = curl_http_request(&[
        "-s",
        "-X",
        "POST",
        "https://www.googleapis.com/drive/v3/files",
        "-H",
        &auth_header,
        "-H",
        "Content-Type: application/json",
        "-d",
        &create_meta.to_string(),
    ]) {
        if meta_resp.status < 400 {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&meta_resp.body) {
                if let Some(file_id) = val.get("id").and_then(|v| v.as_str()) {
                    let upload_url = format!(
                        "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
                        file_id
                    );
                    let _ = curl_http_request(&[
                        "-s",
                        "-X",
                        "PATCH",
                        &upload_url,
                        "-H",
                        &auth_header,
                        "-H",
                        "Content-Type: application/octet-stream",
                        "--data-binary",
                        &format!("@{}", file_path.display()),
                    ]);
                }
            }
        }
    }
}

fn compute_session_capabilities(user: Option<&GoogleUserSession>) -> (bool, bool) {
    let Some(u) = user else {
        return (false, false);
    };
    let Some(bundle) = get_stored_token_bundle(&u.email) else {
        return (false, false);
    };
    let has_refresh = bundle
        .refresh_token
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let now = Utc::now().timestamp();
    let has_active_access = !bundle.access_token.trim().is_empty()
        && bundle.expires_at.map(|exp| exp > now + 30).unwrap_or(true);
    let cloud_browse_ready =
        (has_refresh || has_active_access) && has_drive_readonly_scope(bundle.scopes.as_deref());
    (has_refresh, cloud_browse_ready)
}

fn build_auth_status_response(
    authenticated: bool,
    user: Option<GoogleUserSession>,
    root: &Path,
    storage_mode: String,
    files: Vec<DriveFileItem>,
    total_size_bytes: u64,
    latest_picked_file: Option<PickedFileMetadata>,
) -> GoogleAuthStatusResponse {
    let (persistent_session, cloud_browse_ready) = compute_session_capabilities(user.as_ref());
    GoogleAuthStatusResponse {
        authenticated,
        user,
        drive_folder_path: root.display().to_string(),
        storage_mode,
        files_count: files.len(),
        total_size_bytes,
        files,
        latest_picked_file,
        persistent_session,
        cloud_browse_ready,
    }
}

/// POST /api/v1/auth/google/authorize
/// Nhận thông tin ủy quyền từ client, lưu phiên an toàn và khởi tạo thư mục Google Drive.
pub async fn google_authorize(Json(req): Json<AuthorizeRequest>) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INVALID_EMAIL".to_string(),
                    message: "Địa chỉ Email Google không hợp lệ".to_string(),
                },
            }),
        )
            .into_response();
    }

    let name = if req.name.trim().is_empty() {
        email.split('@').next().unwrap_or("Giáo Viên").to_string()
    } else {
        req.name.trim().to_string()
    };

    let avatar = req.avatar.filter(|a| !a.trim().is_empty()).unwrap_or_else(|| {
        format!(
            "https://ui-avatars.com/api/?name={}&background=2563eb&color=fff&bold=true&size=128",
            name.replace(' ', "+")
        )
    });

    if let Some(ref tok) = req.access_token {
        if !tok.trim().is_empty() {
            save_backend_token(&email, tok);
        }
    }

    let now_str = Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
    let (root, storage_mode) = resolve_drive_root();

    let session = GoogleUserSession {
        provider: "google".to_string(),
        name,
        email,
        avatar,
        drive_connected: true,
        access_token: None, // Không serialize access_token
        connected_at: now_str.clone(),
        last_sync: now_str,
    };

    let _ = save_session(&root, &session);
    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            true,
            Some(session),
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

/// GET /api/v1/auth/google/status
/// Trả về trạng thái phiên Google và danh sách tệp thực tế trong thư mục Google Drive.
/// Tuyệt đối không để lộ token và có chứa latest_picked_file khi người dùng vừa chọn tệp từ Picker.
pub async fn google_auth_status() -> impl IntoResponse {
    let (root, storage_mode) = resolve_drive_root();
    let session = load_session(&root);
    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            session.is_some(),
            session,
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

/// POST /api/v1/auth/google/profile
/// Cập nhật Tên hiển thị của người dùng (Email được khóa cố định không cho phép sửa đổi).
pub async fn google_update_profile(
    Json(req): Json<UpdateProfileRequest>,
) -> impl IntoResponse {
    let new_name = req.name.trim();
    if new_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EMPTY_NAME".to_string(),
                    message: "Tên hiển thị không được để trống".to_string(),
                },
            }),
        )
            .into_response();
    }

    let (root, storage_mode) = resolve_drive_root();
    let mut session = match load_session(&root) {
        Some(s) => s,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "NOT_AUTHENTICATED".to_string(),
                        message: "Chưa đăng nhập tài khoản Google".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    session.name = new_name.to_string();
    if let Some(new_avatar) = req.avatar {
        let trimmed_avatar = new_avatar.trim();
        if !trimmed_avatar.is_empty() {
            session.avatar = trimmed_avatar.to_string();
        }
    } else if session.avatar.contains("ui-avatars.com") {
        session.avatar = format!(
            "https://ui-avatars.com/api/?name={}&background=2563eb&color=fff&bold=true&size=128",
            new_name.replace(' ', "+")
        );
    }
    session.last_sync = Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
    let _ = save_session(&root, &session);

    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            true,
            Some(session),
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

/// POST /api/v1/auth/google/logout
/// Đăng xuất tài khoản Google khỏi máy chủ.
pub async fn google_logout() -> impl IntoResponse {
    let (root, _) = resolve_drive_root();
    if let Some(s) = load_session(&root) {
        clear_backend_tokens(&s.email);
    }
    let p = session_file_path(&root);
    let _ = fs::remove_file(p);
    let _ = fs::remove_file(vault_session_backup_file(&private_vault_dir()));
    *LATEST_PICKED_FILE.lock().unwrap() = None;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "authenticated": false
        })),
    )
}

/// GET /api/v1/teacher/drive/files
/// Liệt kê toàn bộ tệp thực tế trong thư mục ExamPlus/ trên Google Drive.
pub async fn list_drive_files() -> impl IntoResponse {
    let (root, storage_mode) = resolve_drive_root();
    let session = load_session(&root);
    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            session.is_some(),
            session,
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

fn parse_drive_cloud_items(val: &serde_json::Value) -> Vec<CloudDriveItem> {
    let Some(arr) = val.get("files").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Tệp không tên")
                .to_string();
            let mime_type = item
                .get("mimeType")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string();
            let is_folder = mime_type == "application/vnd.google-apps.folder";
            let lower_name = name.to_lowercase();
            let file_kind = if is_folder {
                "folder"
            } else if mime_type == "application/pdf" || lower_name.ends_with(".pdf") {
                "pdf"
            } else if mime_type == "application/vnd.google-apps.document" {
                "gdoc"
            } else if mime_type
                == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                || lower_name.ends_with(".docx")
            {
                "docx"
            } else {
                "other"
            }
            .to_string();

            let size_bytes = item
                .get("size")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .or_else(|| item.get("size").and_then(|v| v.as_u64()));
            let size_formatted = if is_folder {
                "Thư mục".to_string()
            } else if mime_type == "application/vnd.google-apps.document" && size_bytes.is_none() {
                "Google Docs".to_string()
            } else {
                format_bytes(size_bytes.unwrap_or(0))
            };
            let modified_time = item
                .get("modifiedTime")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let starred = item.get("starred").and_then(|v| v.as_bool()).unwrap_or(false);
            let shared = item.get("shared").and_then(|v| v.as_bool()).unwrap_or(false);
            let owner_name = item
                .get("owners")
                .and_then(|v| v.as_array())
                .and_then(|owners| owners.first())
                .and_then(|o| o.get("displayName").or_else(|| o.get("emailAddress")))
                .and_then(|v| v.as_str())
                .unwrap_or("Google Drive")
                .to_string();

            Some(CloudDriveItem {
                id,
                name,
                mime_type,
                is_folder,
                file_kind,
                size_bytes,
                size_formatted,
                modified_time,
                starred,
                shared,
                owner_name,
            })
        })
        .collect()
}

fn fetch_cloud_drive_page(
    access_token: &str,
    drive_query: &str,
    order_by: &str,
    page_token: Option<&str>,
) -> Result<HttpRawResponse, String> {
    let mut url = format!(
        "https://www.googleapis.com/drive/v3/files?q={}&orderBy={}&pageSize=48&supportsAllDrives=true&includeItemsFromAllDrives=true&fields={}",
        urlencoding(drive_query),
        urlencoding(order_by),
        urlencoding("nextPageToken,files(id,name,mimeType,size,modifiedTime,starred,shared,owners(displayName,emailAddress))"),
    );
    if let Some(pt) = page_token.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        url.push_str("&pageToken=");
        url.push_str(&urlencoding(pt));
    }
    let auth_header = format!("Authorization: Bearer {}", access_token);
    curl_http_request(&["-s", &url, "-H", &auth_header])
}

/// GET /api/v1/teacher/drive/cloud-files
/// Duyệt trực tiếp danh sách tệp & thư mục trên Google Drive thật ngay trong ứng dụng.
/// Tự động làm mới access_token qua refresh_token khi sắp hết hạn hoặc khi gặp HTTP 401.
pub async fn list_cloud_drive_files(Query(query): Query<CloudFilesQuery>) -> impl IntoResponse {
    let (root, _) = resolve_drive_root();
    let Some(session) = load_session(&root) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "success": false,
                "needs_reauth": true,
                "persistent_session": false,
                "cloud_browse_ready": false,
                "code": "NOT_AUTHENTICATED",
                "message": "Bạn chưa đăng nhập Google Drive. Bấm Kết Nối Google để đăng nhập 1 lần."
            })),
        )
            .into_response();
    };

    let tab = query.tab.as_deref().unwrap_or("recent");
    let file_type = query.file_type.as_deref().unwrap_or("all");
    let drive_q = match build_cloud_files_query(
        tab,
        query.folder_id.as_deref(),
        query.q.as_deref(),
        file_type,
    ) {
        Ok(q) => q,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "needs_reauth": false,
                    "code": "INVALID_DRIVE_QUERY",
                    "message": msg
                })),
            )
                .into_response();
        }
    };

    let order_by = if tab.eq_ignore_ascii_case("recent")
        && query.q.as_deref().unwrap_or("").trim().is_empty()
    {
        "modifiedTime desc"
    } else {
        "folder,modifiedTime desc"
    };

    let mut bundle = match ensure_valid_access_token(&session.email, false) {
        Ok(b) => b,
        Err(reason) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "success": false,
                    "needs_reauth": true,
                    "persistent_session": false,
                    "cloud_browse_ready": false,
                    "code": reason,
                    "message": "Phiên Google cũ đã hết hạn hoặc thiếu quyền tự động làm mới. Vui lòng xác nhận 1 lần để lưu phiên lâu dài."
                })),
            )
                .into_response();
        }
    };

    let mut resp = match fetch_cloud_drive_page(
        &bundle.access_token,
        &drive_q,
        order_by,
        query.page_token.as_deref(),
    ) {
        Ok(r) => r,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "success": false,
                    "needs_reauth": false,
                    "code": "DRIVE_NETWORK_ERROR",
                    "message": err
                })),
            )
                .into_response();
        }
    };

    // Nếu HTTP 401 -> thử làm mới access_token tự động bằng refresh_token và thử lại 1 lần
    if resp.status == 401 {
        match ensure_valid_access_token(&session.email, true) {
            Ok(refreshed) => {
                bundle = refreshed;
                resp = match fetch_cloud_drive_page(
                    &bundle.access_token,
                    &drive_q,
                    order_by,
                    query.page_token.as_deref(),
                ) {
                    Ok(r) => r,
                    Err(err) => {
                        return (
                            StatusCode::BAD_GATEWAY,
                            Json(serde_json::json!({
                                "success": false,
                                "needs_reauth": false,
                                "code": "DRIVE_NETWORK_ERROR",
                                "message": err
                            })),
                        )
                            .into_response();
                    }
                };
            }
            Err(reason) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "success": false,
                        "needs_reauth": true,
                        "persistent_session": false,
                        "cloud_browse_ready": false,
                        "code": reason,
                        "message": "Phiên Google Drive đã hết hạn. Vui lòng đăng nhập lại 1 lần để lưu phiên tự động."
                    })),
                )
                    .into_response();
            }
        }
    }

    if resp.status == 401 || resp.status == 403 {
        let json_err: serde_json::Value =
            serde_json::from_slice(&resp.body).unwrap_or_default();
        let api_msg = json_err
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("Không đủ quyền truy cập Google Drive.");
        let is_scope_issue = resp.status == 403
            && (api_msg.to_lowercase().contains("insufficient")
                || !has_drive_readonly_scope(bundle.scopes.as_deref()));
        return (
            StatusCode::from_u16(resp.status).unwrap_or(StatusCode::UNAUTHORIZED),
            Json(serde_json::json!({
                "success": false,
                "needs_reauth": true,
                "limited_scope": is_scope_issue,
                "persistent_session": bundle.refresh_token.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false),
                "cloud_browse_ready": false,
                "code": if is_scope_issue { "INSUFFICIENT_DRIVE_SCOPE" } else { "GOOGLE_AUTH_REQUIRED" },
                "message": if is_scope_issue {
                    "Tài khoản chưa cấp quyền duyệt toàn bộ Google Drive. Bấm 'Cấp quyền đầy đủ (1 lần)' để xem mọi thư mục & đề thi."
                } else {
                    "Phiên Google Drive cần được xác thực lại 1 lần."
                }
            })),
        )
            .into_response();
    }

    if resp.status >= 400 {
        let json_err: serde_json::Value =
            serde_json::from_slice(&resp.body).unwrap_or_default();
        let api_msg = json_err
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("Google Drive API trả về lỗi");
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "success": false,
                "needs_reauth": false,
                "code": "DRIVE_API_ERROR",
                "message": format!("Google Drive API ({}): {}", resp.status, api_msg)
            })),
        )
            .into_response();
    }

    let val: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or_default();
    let files = parse_drive_cloud_items(&val);
    let next_page_token = val
        .get("nextPageToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let has_refresh = bundle
        .refresh_token
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_readonly = has_drive_readonly_scope(bundle.scopes.as_deref());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "needs_reauth": false,
            "limited_scope": !has_readonly,
            "persistent_session": has_refresh,
            "cloud_browse_ready": (has_refresh || !bundle.access_token.is_empty()) && has_readonly,
            "tab": tab,
            "folder_id": query.folder_id.unwrap_or_else(|| "root".to_string()),
            "files": files,
            "next_page_token": next_page_token,
            "user_email": session.email,
        })),
    )
        .into_response()
}

/// Tải nội dung tệp nhị phân (.docx / .pdf) trực tiếp từ Google Drive API
pub fn fetch_drive_file_content(access_token: &str, file_id: &str) -> Result<Vec<u8>, String> {
    if !is_valid_drive_id(file_id) {
        return Err("Mã tệp Google Drive không hợp lệ".to_string());
    }
    let auth_header = format!("Authorization: Bearer {}", access_token);
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?alt=media&supportsAllDrives=true",
        file_id
    );

    let resp = curl_http_request(&["-s", "-L", &url, "-H", &auth_header])?;
    if resp.status == 401 {
        return Err("UNAUTHORIZED_TOKEN".to_string());
    }
    if resp.status >= 400 {
        return Err(format!("Google Drive trả về HTTP {}", resp.status));
    }
    if resp.body.is_empty() {
        return Err("Tệp trên Google Drive rỗng".to_string());
    }

    Ok(resp.body)
}

fn export_google_doc_as_docx(access_token: &str, file_id: &str) -> Result<Vec<u8>, String> {
    if !is_valid_drive_id(file_id) {
        return Err("Mã tệp Google Docs không hợp lệ".to_string());
    }
    let auth_header = format!("Authorization: Bearer {}", access_token);
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}/export?mimeType=application%2Fvnd.openxmlformats-officedocument.wordprocessingml.document",
        file_id
    );
    let resp = curl_http_request(&["-s", "-L", &url, "-H", &auth_header])?;
    if resp.status == 401 {
        return Err("UNAUTHORIZED_TOKEN".to_string());
    }
    if resp.status >= 400 {
        return Err(format!("Không thể xuất Google Docs sang DOCX (HTTP {})", resp.status));
    }
    if resp.body.is_empty() {
        return Err("Tệp Google Docs rỗng".to_string());
    }
    Ok(resp.body)
}

fn fetch_drive_file_with_auto_refresh(
    session: Option<&GoogleUserSession>,
    file_id: &str,
    mime_hint: Option<&str>,
) -> Option<(Vec<u8>, bool)> {
    if !is_valid_drive_id(file_id) {
        return None;
    }
    let email = session.map(|s| s.email.as_str())?;
    let mut bundle = ensure_valid_access_token(email, false).ok()?;
    let is_gdoc = mime_hint == Some("application/vnd.google-apps.document")
        || fetch_drive_file_metadata(&bundle.access_token, file_id)
            .map(|m| m.mime_type == "application/vnd.google-apps.document")
            .unwrap_or(false);

    let first_attempt = if is_gdoc {
        export_google_doc_as_docx(&bundle.access_token, file_id)
    } else {
        fetch_drive_file_content(&bundle.access_token, file_id)
    };

    match first_attempt {
        Ok(bytes) => Some((bytes, is_gdoc)),
        Err(e) if e == "UNAUTHORIZED_TOKEN" => {
            bundle = ensure_valid_access_token(email, true).ok()?;
            let retry = if is_gdoc {
                export_google_doc_as_docx(&bundle.access_token, file_id)
            } else {
                fetch_drive_file_content(&bundle.access_token, file_id)
            };
            retry.ok().map(|b| (b, is_gdoc))
        }
        Err(_) => None,
    }
}

/// POST /api/v1/teacher/drive/import
/// Đọc tệp Word (.docx) hoặc PDF (.pdf) thật từ Google Drive API hoặc thư mục Google Drive cục bộ,
/// bóc tách câu hỏi & công thức MathType SVG và lưu vào CSDL SQLite.
pub async fn import_drive_docx(
    State(state): State<AppState>,
    Json(req): Json<DriveImportRequest>,
) -> Response {
    let (root, _) = resolve_drive_root();
    let mut clean_name = Path::new(&req.filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("de_thi.docx")
        .to_string();

    let session = load_session(&root);

    // 1. Nếu có file_id và phiên Google, ưu tiên tải trực tiếp từ Google Drive API (hỗ trợ tự động làm mới token)
    let downloaded_opt = if let Some(ref fid) = req.file_id {
        fetch_drive_file_with_auto_refresh(session.as_ref(), fid.trim(), req.mime_type.as_deref())
    } else {
        None
    };

    // 2. Nếu không tải được qua API, tìm trong thư mục cục bộ ExamPlus/DeThi_Word hoặc ExamPlus/DeThi_PDF
    let bytes = match downloaded_opt {
        Some((b, exported_gdoc)) => {
            if exported_gdoc && !clean_name.to_lowercase().ends_with(".docx") {
                clean_name.push_str(".docx");
            }
            b
        }
        None => {
            let word_path = root.join("DeThi_Word").join(&clean_name);
            let pdf_path = root.join("DeThi_PDF").join(&clean_name);
            let target_path = if word_path.exists() {
                word_path
            } else if pdf_path.exists() {
                pdf_path
            } else if clean_name.to_lowercase().ends_with(".pdf") {
                pdf_path
            } else {
                word_path
            };
            match fs::read(&target_path) {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(ApiErrorResponse {
                            success: false,
                            error: ApiErrorDetail {
                                code: "FILE_NOT_FOUND".to_string(),
                                message: format!(
                                    "Không tìm thấy tệp '{}' trong Google Drive: {}",
                                    clean_name, e
                                ),
                            },
                        }),
                    )
                        .into_response();
                }
            }
        }
    };

    let default_title = clean_name
        .trim_end_matches(".docx")
        .trim_end_matches(".DOCX")
        .trim_end_matches(".pdf")
        .trim_end_matches(".PDF")
        .replace('_', " ");

    upload_docx(
        State(state),
        Query(UploadDocxQuery {
            title: Some(default_title),
            filename: Some(clean_name),
        }),
        Bytes::from(bytes),
    )
    .await
    .into_response()
}

/// POST /api/v1/teacher/drive/upload
/// Tải tệp mới (.docx hoặc .xlsx) trực tiếp lên thư mục Google Drive (`ExamPlus/DeThi_Word` hoặc `BangDiem_Excel`).
pub async fn upload_file_to_drive(
    Query(query): Query<DriveUploadQuery>,
    body: Bytes,
) -> impl IntoResponse {
    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EMPTY_FILE".to_string(),
                    message: "Nội dung tệp rỗng".to_string(),
                },
            }),
        )
            .into_response();
    }

    let (root, storage_mode) = resolve_drive_root();
    let clean_name = Path::new(&query.filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("De_Thi_Moi.docx");

    let target_folder = match query.folder.as_deref() {
        Some("BangDiem_Excel") => "BangDiem_Excel",
        Some("SaoLuu_CauHinh") => "SaoLuu_CauHinh",
        _ => {
            let lower = clean_name.to_lowercase();
            if lower.ends_with(".xlsx") {
                "BangDiem_Excel"
            } else if lower.ends_with(".pdf") {
                "DeThi_PDF"
            } else {
                "DeThi_Word"
            }
        }
    };

    let dest = root.join(target_folder).join(clean_name);
    if let Err(e) = fs::write(&dest, &body) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "WRITE_ERROR".to_string(),
                    message: format!("Không thể ghi tệp vào Google Drive: {}", e),
                },
            }),
        )
            .into_response();
    }

    let mut session = load_session(&root);
    if let Some(ref mut s) = session {
        let valid_tok = ensure_valid_access_token(&s.email, false)
            .map(|b| b.access_token)
            .ok()
            .or_else(|| get_backend_token(&s.email));
        if let Some(tok) = valid_tok {
            upload_file_to_cloud_drive(&tok, clean_name, &dest);
        }
        s.last_sync = Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
        let _ = save_session(&root, s);
    }

    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            session.is_some(),
            session,
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

/// POST /api/v1/teacher/drive/sync
/// Đồng bộ hóa toàn bộ trạng thái ngân hàng đề thi và cấu hình giáo viên vào Google Drive (`ExamPlus/SaoLuu_CauHinh`).
pub async fn sync_drive_now(State(state): State<AppState>) -> impl IntoResponse {
    let (root, storage_mode) = resolve_drive_root();
    let now_str = Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();

    let current_exam_id = { *state.current_exam_id.read().await };
    if let Some(exam_id) = current_exam_id {
        let repo = state.repo.lock().await;
        if let Ok(Some(exam)) = repo.get_exam(exam_id) {
            let questions = repo.get_questions_by_exam(exam_id).unwrap_or_default();
            let snapshot = serde_json::json!({
                "synced_at": now_str,
                "active_exam": exam,
                "questions_count": questions.len(),
            });
            let snap_path = root.join("SaoLuu_CauHinh").join("exam_bank_snapshot.json");
            let _ = fs::write(
                snap_path,
                serde_json::to_string_pretty(&snapshot).unwrap_or_default(),
            );
        }
    }

    let mut session = load_session(&root);
    if let Some(ref mut s) = session {
        s.last_sync = now_str;
        let _ = save_session(&root, s);
        let valid_tok = ensure_valid_access_token(&s.email, false)
            .map(|b| b.access_token)
            .ok()
            .or_else(|| get_backend_token(&s.email));
        if let Some(tok) = valid_tok {
            let cfg_p = profile_config_path(&root);
            upload_file_to_cloud_drive(&tok, "profile.epconfig", &cfg_p);
        }
    }

    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            session.is_some(),
            session,
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

/// POST /api/v1/teacher/drive/delete
/// Xóa một tệp trong thư mục Google Drive (chống Path Traversal).
pub async fn delete_drive_file(Json(req): Json<DriveDeleteRequest>) -> impl IntoResponse {
    let (root, storage_mode) = resolve_drive_root();
    let clean_name = Path::new(&req.filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if clean_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INVALID_FILENAME".to_string(),
                    message: "Tên tệp không hợp lệ".to_string(),
                },
            }),
        )
            .into_response();
    }

    let allowed_folders = ["DeThi_Word", "DeThi_PDF", "BangDiem_Excel", "SaoLuu_CauHinh"];
    let folders: Vec<&str> = if let Some(ref f) = req.folder {
        if !allowed_folders.contains(&f.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_FOLDER".to_string(),
                        message: "Thư mục không hợp lệ. Chỉ cho phép DeThi_Word, DeThi_PDF, BangDiem_Excel, SaoLuu_CauHinh".to_string(),
                    },
                }),
            )
                .into_response();
        }
        vec![f.as_str()]
    } else {
        vec!["DeThi_Word", "DeThi_PDF", "BangDiem_Excel", "SaoLuu_CauHinh"]
    };

    for folder in folders {
        let p = root.join(folder).join(clean_name);
        if p.exists() && p.is_file() {
            let _ = fs::remove_file(p);
        }
    }

    let session = load_session(&root);
    let (files, total_size_bytes) = list_all_drive_files(&root);
    let latest_picked_file = {
        let guard = LATEST_PICKED_FILE.lock().unwrap();
        guard.clone()
    };

    (
        StatusCode::OK,
        Json(build_auth_status_response(
            session.is_some(),
            session,
            &root,
            storage_mode,
            files,
            total_size_bytes,
            latest_picked_file,
        )),
    )
        .into_response()
}

/// POST /api/v1/teacher/drive/open-folder
/// Mở thư mục Google Drive đồng bộ trực tiếp trong Windows Explorer.
pub async fn open_drive_folder_in_explorer() -> impl IntoResponse {
    let (root, _) = resolve_drive_root();
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("explorer.exe");
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.arg(root.as_os_str()).spawn();
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "path": root.display().to_string()
        })),
    )
}

/// GET /api/v1/teacher/drive/download
/// Tải xuống tệp từ thư mục Google Drive (chống Path Traversal).
pub async fn download_drive_file(Query(query): Query<DriveUploadQuery>) -> impl IntoResponse {
    let (root, _) = resolve_drive_root();
    let clean_name = Path::new(&query.filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if clean_name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Tên tệp không hợp lệ").into_response();
    }

    let folder = match query.folder.as_deref() {
        Some("BangDiem_Excel") => "BangDiem_Excel",
        Some("SaoLuu_CauHinh") => "SaoLuu_CauHinh",
        Some("DeThi_PDF") => "DeThi_PDF",
        Some("DeThi_Word") => "DeThi_Word",
        None => {
            if clean_name.to_lowercase().ends_with(".pdf") {
                "DeThi_PDF"
            } else {
                "DeThi_Word"
            }
        }
        _ => return (StatusCode::BAD_REQUEST, "Thư mục không hợp lệ").into_response(),
    };
    let p = root.join(folder).join(clean_name);

    match fs::read(&p) {
        Ok(bytes) => {
            let disp = format!("attachment; filename=\"{}\"", clean_name);
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                    (header::CONTENT_DISPOSITION, disp),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "Không tìm thấy tệp").into_response(),
    }
}
