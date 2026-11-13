//! # Application State & Event Broadcasting
//!
//! Trạng thái chia sẻ của máy chủ Axum và hệ thống kênh truyền thời gian thực (Broadcast).

use crate::network::CloudflareTunnel;
use examplus_core::db::DbRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerEvent {
    StudentJoined {
        student_code: String,
        full_name: String,
        class_name: String,
        #[serde(default)]
        exam_id: Option<String>,
    },
    StudentHeartbeat {
        student_code: String,
        timestamp: String,
        #[serde(default)]
        exam_id: Option<String>,
    },
    ViolationAlert {
        student_code: String,
        full_name: String,
        violation_type: String,
        details: String,
        #[serde(default)]
        exam_id: Option<String>,
    },
    StudentSubmitted {
        student_code: String,
        total_score: f64,
        #[serde(default)]
        exam_id: Option<String>,
    },
    ExamStarted {
        started_at: String,
        end_at: String,
        duration_minutes: u32,
        #[serde(default)]
        exam_id: Option<String>,
    },
    NetworkModeChanged {
        mode: String,
        public_url: Option<String>,
    },
    GradesUpdated {
        exam_id: String,
    },
    ExamFinished {
        exam_id: String,
    },
    RoomClosed {
        exam_id: String,
    },
}

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Mutex<DbRepository>>,
    pub current_exam_id: Arc<RwLock<Option<Uuid>>>,
    pub event_sender: broadcast::Sender<ServerEvent>,
    pub tunnel: Arc<Mutex<Option<CloudflareTunnel>>>,
    pub server_port: u16,
    pub teacher_secret: Arc<String>,
}

impl AppState {
    pub fn new(repo: DbRepository, current_exam_id: Option<Uuid>) -> Self {
        Self::with_port(repo, current_exam_id, 8080)
    }

    pub fn with_port(repo: DbRepository, current_exam_id: Option<Uuid>, port: u16) -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        let teacher_secret = std::env::var("EXAMPLUS_TEACHER_SECRET")
            .unwrap_or_else(|_| Uuid::new_v4().to_string());
        Self {
            repo: Arc::new(Mutex::new(repo)),
            current_exam_id: Arc::new(RwLock::new(current_exam_id)),
            event_sender: tx,
            tunnel: Arc::new(Mutex::new(None)),
            server_port: port,
            teacher_secret: Arc::new(teacher_secret),
        }
    }

    /// Xác thực quyền truy cập dành riêng cho Giáo viên (Teacher Guard)
    pub fn is_teacher_authorized(
        &self,
        peer_addr: Option<std::net::SocketAddr>,
        headers: &axum::http::HeaderMap,
        query_token: Option<&str>,
    ) -> bool {
        // 1. Kiểm tra nếu có token quản trị hợp lệ trong Header hoặc Query
        if let Some(token) = query_token {
            if !token.is_empty() && token == *self.teacher_secret {
                return true;
            }
        }
        if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
            let tok = auth.strip_prefix("Bearer ").map(|s| s.trim()).unwrap_or(auth.trim());
            if !tok.is_empty() && tok == *self.teacher_secret {
                return true;
            }
        }
        if let Some(tok) = headers.get("x-teacher-token").and_then(|v| v.to_str().ok()) {
            if !tok.trim().is_empty() && tok.trim() == *self.teacher_secret {
                return true;
            }
        }

        // 2. Kiểm tra Origin để phòng thủ Cross-Site Port Attack (Drive-by Localhost Exploitation)
        // Nếu trình duyệt gửi request mang header Origin, bắt buộc phải là localhost hoặc WebView2
        if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
            let origin_clean = origin.trim().to_lowercase();
            let is_allowed_origin = origin_clean == "null"
                || origin_clean == "tauri://localhost"
                || origin_clean == "http://tauri.localhost"
                || origin_clean == "https://tauri.localhost"
                || origin_clean == format!("http://127.0.0.1:{}", self.server_port)
                || origin_clean == format!("http://localhost:{}", self.server_port)
                || origin_clean.starts_with("http://127.0.0.1:")
                || origin_clean.starts_with("http://localhost:");

            if !is_allowed_origin {
                return false;
            }
        }

        // 3. Kiểm tra Sec-Fetch-Site: Chặn triệt để các request cross-site từ website bên ngoài
        // (Ngoại trừ trường hợp điều hướng trình duyệt cấp cao nhất như redirect từ Google OAuth hoặc mở từ ứng dụng ngoài)
        if let Some(sec_site) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
            if sec_site.trim().eq_ignore_ascii_case("cross-site") {
                let is_navigate = headers
                    .get("sec-fetch-mode")
                    .and_then(|v| v.to_str().ok())
                    .map(|m| m.eq_ignore_ascii_case("navigate"))
                    .unwrap_or(false);
                if !is_navigate {
                    return false;
                }
            }
        }

        // 4. Kết nối trực tiếp từ Loopback (127.0.0.1 / ::1) và không qua Cloudflare Tunnel / External Proxy
        let is_loopback = match peer_addr {
            Some(addr) => addr.ip().is_loopback(),
            None => true, // Môi trường test nội bộ tower oneshot
        };

        let is_external_proxy = headers.contains_key("cf-connecting-ip")
            || headers.contains_key("cf-ray")
            || headers.contains_key("x-forwarded-for")
            || headers.contains_key("x-real-ip")
            || headers.contains_key("forwarded");

        is_loopback && !is_external_proxy
    }
}
