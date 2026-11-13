//! # Axum Router Builder
//!
//! Khởi tạo cây định tuyến HTTP REST, WebSockets và Static Assets cho toàn bộ ứng dụng.

use crate::handlers::{auth, drive, exam, health, qr, teacher, ws};
use crate::state::AppState;
use crate::static_files::{self, static_handler};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

/// Middleware kiểm soát phân quyền Giáo viên (Teacher Guard)
/// Ngăn chặn thí sinh qua LAN hoặc Cloudflare Tunnel truy cập trái quyền vào Dashboard,
/// xem trước đề thi (lộ đáp án) hoặc thao tác quản trị.
async fn teacher_guard_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let connect_info = req.extensions().get::<ConnectInfo<SocketAddr>>().copied();
    let peer_addr = connect_info.map(|c| c.0);
    let headers = req.headers();

    let query_token = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "token" {
                parts.next()
            } else {
                None
            }
        })
    });

    if !state.is_teacher_authorized(peer_addr, headers, query_token) {
        let is_json = req.uri().path().starts_with("/api/")
            || req
                .headers()
                .get(header::ACCEPT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.contains("json"))
                .unwrap_or(false);

        if is_json {
            let err_json = serde_json::json!({
                "success": false,
                "error": {
                    "code": "TEACHER_ACCESS_DENIED",
                    "message": "Truy cập bị từ chối: Bảng điều khiển và API Giáo viên chỉ dành cho máy chủ cục bộ hoặc yêu cầu token xác thực quản trị."
                }
            });
            return Ok((
                StatusCode::FORBIDDEN,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                Json(err_json).into_response(),
            )
                .into_response());
        } else {
            return Ok((
                StatusCode::FORBIDDEN,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                axum::response::Html(
                    r#"<!DOCTYPE html><html lang="vi"><head><meta charset="UTF-8"><title>403 Forbidden</title></head>
                    <body style="font-family:'Segoe UI',sans-serif;background:#0f172a;color:#f8fafc;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;">
                      <div style="background:#1e293b;padding:36px;border-radius:20px;max-width:480px;text-align:center;box-shadow:0 10px 25px rgba(0,0,0,0.5);">
                        <h2 style="color:#ef4444;margin-bottom:12px;">403 Forbidden: Truy Cập Bị Từ Chối</h2>
                        <p style="color:#94a3b8;font-size:14px;line-height:1.6;">Giao diện và API Giáo viên chỉ cho phép truy cập trực tiếp từ máy tính của giám thị (Localhost) hoặc yêu cầu mã xác thực quản trị.</p>
                      </div>
                    </body></html>"#,
                )
                .into_response(),
            )
                .into_response());
        }
    }

    Ok(next.run(req).await)
}

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Tuyến đường dành riêng cho Giáo viên (có bảo vệ Teacher Guard Middleware)
    let teacher_router = Router::new()
        .route("/teacher", get(static_files::teacher_static_handler))
        .route("/teacher/", get(static_files::teacher_static_handler))
        .route("/teacher/*path", get(static_files::teacher_static_handler))
        .route("/api/v1/teacher/status", get(teacher::get_teacher_status))
        .route("/api/v1/teacher/exams", get(teacher::list_saved_exams))
        .route("/api/v1/teacher/save-exam", post(teacher::save_exam_to_bank))
        .route("/api/v1/teacher/open-room", post(teacher::open_exam_room))
        .route("/api/v1/teacher/delete-exam", post(teacher::delete_saved_exam))
        .route(
            "/api/v1/teacher/upload-docx",
            post(teacher::upload_docx)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/api/v1/teacher/upload-exam",
            post(teacher::upload_docx)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route("/api/v1/teacher/generate-variants", post(teacher::generate_variants))
        .route("/api/v1/teacher/toggle-network", post(teacher::toggle_network))
        .route("/api/v1/teacher/start-exam", post(teacher::start_exam))
        .route("/api/v1/teacher/stop-exam", post(teacher::stop_exam))
        .route("/api/v1/teacher/close-room", post(teacher::close_exam_room))
        .route("/api/v1/teacher/delete-room", post(teacher::delete_exam_room))
        .route("/api/v1/teacher/add-time", post(teacher::add_exam_time))
        .route("/api/v1/teacher/export-excel", get(teacher::export_excel))
        .route("/api/v1/teacher/exam-preview", get(teacher::get_exam_preview))
        .route("/api/v1/teacher/grades", get(teacher::get_grades))
        .route("/api/v1/teacher/examinee-audit/:code", get(teacher::get_examinee_audit))
        .route("/api/v1/teacher/update-answer-key", post(teacher::update_answer_key))
        .route(
            "/api/v1/teacher/import-answer-key",
            post(teacher::import_answer_key)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route("/api/v1/teacher/drive/files", get(drive::list_drive_files))
        .route("/api/v1/teacher/drive/cloud-files", get(drive::list_cloud_drive_files))
        .route("/api/v1/teacher/drive/import", post(drive::import_drive_docx))
        .route("/api/v1/teacher/drive/upload", post(drive::upload_file_to_drive))
        .route("/api/v1/teacher/drive/sync", post(drive::sync_drive_now))
        .route("/api/v1/teacher/drive/delete", post(drive::delete_drive_file))
        .route("/api/v1/teacher/drive/open-folder", post(drive::open_drive_folder_in_explorer))
        .route("/api/v1/teacher/drive/download", get(drive::download_drive_file))
        .route("/ws/teacher", get(ws::ws_teacher_handler))
        .route_layer(middleware::from_fn_with_state(state.clone(), teacher_guard_middleware));

    Router::new()
        // Healthcheck & Metrics
        .route("/healthz", get(health::healthz))
        // API Tiện ích (QR SVG)
        .route("/api/v1/qr", get(qr::get_qr_svg))
        // OAuth Google Callback, Start, Status & Profile/Logout (Công khai để đăng nhập & nhận redirect từ accounts.google.com)
        .route("/api/v1/auth/google/start", get(drive::google_auth_start_page))
        .route("/api/v1/auth/google/callback", get(drive::google_auth_callback))
        .route("/api/v1/auth/google/status", get(drive::google_auth_status))
        .route("/api/v1/auth/google/authorize", post(drive::google_authorize))
        .route("/api/v1/auth/google/profile", post(drive::google_update_profile))
        .route("/api/v1/auth/google/logout", post(drive::google_logout))
        // API Xác thực thí sinh
        .route("/api/v1/auth/join", post(auth::join_exam))
        // API Phòng thi & Bài làm của thí sinh
        .route("/api/v1/exam/lobby-status", get(exam::get_lobby_status))
        .route("/api/v1/exam/paper", get(exam::get_exam_paper))
        .route("/api/v1/exam/save-draft", post(exam::save_draft))
        .route("/api/v1/exam/submit", post(exam::submit_exam))
        // Kênh truyền WebSocket Thí sinh
        .route("/ws/student", get(ws::ws_student_handler))
        .merge(teacher_router)
        // Phục vụ giao diện Web Thí sinh (SPA Fallback)
        .fallback(static_handler)
        .layer(cors)
        .with_state(state)
}
