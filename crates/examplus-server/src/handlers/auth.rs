//! # Authentication Handlers
//!
//! Xử lý đăng nhập phòng thi và cấp phát Session Token cho thí sinh.

use crate::state::{AppState, ServerEvent};
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use examplus_core::models::{Examinee, ExamineeStatus};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub student_code: String,
    pub full_name: String,
    pub class_name: String,
    pub device_id: Option<String>,
    pub room_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JoinResponse {
    pub success: bool,
    pub session_token: String,
    pub token: String, // Tương thích ngược với client JS
    pub examinee_id: String,
    pub full_name: String,
    pub student_code: String,
    pub assigned_variant_code: String,
    pub exam_title: String,
    pub duration_minutes: u32,
    pub status: String,
    pub exam_status: String, // Tương thích ngược với client JS
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub success: bool,
    pub error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}

pub fn resolve_client_ip(peer_addr: Option<SocketAddr>, headers: &HeaderMap) -> String {
    let peer_ip = peer_addr.map(|a| a.ip());

    // Nếu kết nối đến từ loopback (local proxy / Cloudflare tunnel daemon)
    let is_trusted_proxy = match peer_ip {
        Some(ip) => ip.is_loopback(),
        None => true, // Phục vụ môi trường unit test không có socket connection info
    };

    if is_trusted_proxy {
        if let Some(cf_ip) = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
            let trimmed = cf_ip.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first_ip) = xff.split(',').next() {
                let trimmed = first_ip.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
        if let Some(xrip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            let trimmed = xrip.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Kết nối trực tiếp LAN: tin tưởng hoàn toàn IP socket thực tế
    peer_ip.map(|ip| ip.to_string()).unwrap_or_else(|| "127.0.0.1".to_string())
}

pub async fn join_exam(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(payload): Json<JoinRequest>,
) -> impl IntoResponse {
    let student_code = payload.student_code.trim().to_uppercase();
    let full_name = payload.full_name.trim().to_string();
    let class_name = payload.class_name.trim().to_string();
    let device_id = payload
        .device_id
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let resolved_ip = resolve_client_ip(connect_info.map(|c| c.0), &headers);
    let raw_ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let combined_ua = format!("dev:{}|{}", device_id, raw_ua);

    if student_code.is_empty() || full_name.is_empty() || class_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INVALID_INPUT".to_string(),
                    message: "Vui lòng nhập đầy đủ Số báo danh, Họ tên và Lớp".to_string(),
                },
            }),
        )
            .into_response();
    }

    // Kiểm tra độ dài an toàn chống cạn kiệt tài nguyên (Resource Exhaustion)
    if student_code.len() > 30 || full_name.len() > 100 || class_name.len() > 30 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INPUT_TOO_LONG".to_string(),
                    message: "Số báo danh (tối đa 30), Họ tên (tối đa 100) hoặc Lớp (tối đa 30) vượt quá độ dài cho phép.".to_string(),
                },
            }),
        )
            .into_response();
    }

    // Kiểm tra định dạng số báo danh: chỉ cho phép chữ cái, chữ số, gạch ngang và gạch dưới
    if !student_code.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INVALID_STUDENT_CODE".to_string(),
                    message: "Số báo danh chỉ được chứa chữ cái, chữ số, dấu gạch nối (-) và gạch dưới (_).".to_string(),
                },
            }),
        )
            .into_response();
    }

    // Chặn tiêm mã độc HTML/XSS vào Họ tên và Lớp
    if full_name.contains('<') || full_name.contains('>') || class_name.contains('<') || class_name.contains('>') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INVALID_CHARACTERS".to_string(),
                    message: "Họ tên và Lớp không được chứa các ký tự đặc biệt như < hoặc >.".to_string(),
                },
            }),
        )
            .into_response();
    }

    let exam_id = if let Some(r_id) = payload.room_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        match Uuid::parse_str(r_id) {
            Ok(id) => id,
            Err(_) => {
                let guard = state.current_exam_id.read().await;
                match *guard {
                    Some(id) => id,
                    None => {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(ApiErrorResponse {
                                success: false,
                                error: ApiErrorDetail {
                                    code: "NO_ACTIVE_EXAM".to_string(),
                                    message: "Phòng thi không tồn tại hoặc mã phòng không hợp lệ.".to_string(),
                                },
                            }),
                        )
                            .into_response();
                    }
                }
            }
        }
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "NO_ACTIVE_EXAM".to_string(),
                            message: "Hiện chưa có kỳ thi nào đang mở phòng".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    let repo = state.repo.lock().await;

    let exam = match repo.get_exam(exam_id) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Kỳ thi không tồn tại trong hệ thống".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    // Lấy danh sách mã đề của kỳ thi
    let variants = repo.get_variants_by_exam(exam_id).unwrap_or_default();
    let assigned_variant = if !variants.is_empty() {
        // Gán mã đề cân bằng dựa theo hash của SBD
        let hash: usize = student_code.bytes().map(|b| b as usize).sum();
        Some(variants[hash % variants.len()].clone())
    } else {
        None
    };

    let assigned_variant_id = assigned_variant.as_ref().map(|v| v.id);
    let assigned_variant_code = assigned_variant
        .as_ref()
        .map(|v| v.code.clone())
        .unwrap_or_else(|| "101".to_string());

    // Kiểm tra thí sinh đã tồn tại chưa
    let existing_examinees = repo.list_examinees_by_exam(exam_id).unwrap_or_default();
    let existing = existing_examinees
        .into_iter()
        .find(|e| e.student_code.eq_ignore_ascii_case(&student_code));

    let examinee = if let Some(mut ex) = existing {
        if ex.status == ExamineeStatus::Submitted {
            return (
                StatusCode::CONFLICT,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "ALREADY_SUBMITTED".to_string(),
                        message: "Thí sinh này đã nộp bài thi và không thể vào lại phòng thi".to_string(),
                    },
                }),
            )
                .into_response();
        }

        // Kiểm tra xung đột thiết bị: Nếu đang có nhịp tim trong vòng 15 giây từ một thiết bị khác
        let is_active_recently = ex.last_heartbeat > Utc::now() - chrono::Duration::seconds(15);
        let prev_device_id = ex.user_agent.as_ref().and_then(|ua| {
            if ua.starts_with("dev:") {
                ua.split('|').next().map(|s| s.trim_start_matches("dev:"))
            } else {
                None
            }
        });

        if is_active_recently && prev_device_id.is_some() && prev_device_id != Some(&device_id) {
            return (
                StatusCode::CONFLICT,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "ACTIVE_SESSION_ON_OTHER_DEVICE".to_string(),
                        message: "Thí sinh đang có một phiên làm bài hoạt động trên thiết bị khác. Vui lòng đóng phiên cũ hoặc báo giám thị!".to_string(),
                    },
                }),
            )
                .into_response();
        }

        // Cấp session token mới khi vào lại và cập nhật CSDL
        let new_token = Uuid::new_v4().to_string();
        if let Err(e) = repo.update_examinee_session(
            ex.id,
            &new_token,
            Some(&resolved_ip),
            Some(&combined_ua),
        ) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "DB_ERROR".to_string(),
                        message: format!("Lỗi cập nhật phiên thí sinh: {}", e),
                    },
                }),
            )
                .into_response();
        }

        ex.session_token = new_token;
        ex.ip_address = Some(resolved_ip);
        ex.user_agent = Some(combined_ua);
        ex.last_heartbeat = Utc::now();
        ex
    } else {
        let new_examinee = Examinee {
            id: Uuid::new_v4(),
            exam_id,
            assigned_variant_id,
            student_code: student_code.clone(),
            full_name: full_name.clone(),
            class_name: class_name.clone(),
            session_token: Uuid::new_v4().to_string(),
            status: ExamineeStatus::Waiting,
            ip_address: Some(resolved_ip),
            user_agent: Some(combined_ua),
            joined_at: Utc::now(),
            last_heartbeat: Utc::now(),
        };
        if let Err(e) = repo.create_examinee(&new_examinee) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "DB_ERROR".to_string(),
                        message: format!("Lỗi lưu trữ thí sinh: {}", e),
                    },
                }),
            )
                .into_response();
        }
        new_examinee
    };

    // Phát sự kiện thí sinh tham gia phòng thi tới bảng điều khiển giáo viên
    let _ = state.event_sender.send(ServerEvent::StudentJoined {
        student_code: examinee.student_code.clone(),
        full_name: examinee.full_name.clone(),
        class_name: examinee.class_name.clone(),
        exam_id: Some(exam_id.to_string()),
    });

    let status_str = match examinee.status {
        ExamineeStatus::Testing => "TESTING",
        ExamineeStatus::Submitted => "SUBMITTED",
        ExamineeStatus::Disconnected => "DISCONNECTED",
        _ => "WAITING",
    };

    (
        StatusCode::OK,
        Json(JoinResponse {
            success: true,
            session_token: examinee.session_token.clone(),
            token: examinee.session_token,
            examinee_id: examinee.id.to_string(),
            full_name: examinee.full_name,
            student_code: examinee.student_code,
            assigned_variant_code,
            exam_title: exam.title,
            duration_minutes: exam.duration_minutes,
            status: status_str.to_string(),
            exam_status: status_str.to_string(),
        }),
    )
        .into_response()
}
