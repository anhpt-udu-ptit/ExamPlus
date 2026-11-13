//! # Security & Anti-Cheat Audit Tests
//!
//! Kiểm thử an ninh bảo mật và cơ chế chống gian lận (OWASP Checklist):
//! - Chống SQL Injection trên các trường đầu vào
//! - Chống lộ lọt đáp án (Answer Leakage Prevention)
//! - Kiểm soát truy cập Session Token (401 Unauthorized khi thiếu/sai token)
//! - Chặn thi đúp / nộp lại bài thi (409 Conflict khi đã nộp bài)

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use examplus_core::db::DbRepository;
use examplus_core::models::{
    Exam, ExamStatus, ExamType, ExamVariant, NetworkMode, PartType, Question, QuestionOption,
};
use examplus_server::{build_router, AppState};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_security_test_app() -> (axum::Router, Uuid) {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");

    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Kiểm tra Bảo Mật".to_string(),
        subject: "Toán".to_string(),
        duration_minutes: 45,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::InProgress,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: None,
    };
    repo.create_exam(&exam).expect("Tạo exam");

    let var_id = Uuid::new_v4();
    let variant = ExamVariant {
        id: var_id,
        exam_id,
        code: "101".to_string(),
        is_original: true,
    };
    repo.create_variant(&variant).expect("Tạo variant");

    let q_id = Uuid::new_v4();
    let q = Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>Câu hỏi bảo mật</p>".to_string(),
        raw_text: Some("Câu hỏi bảo mật".to_string()),
        image_url: None,
        short_answer_solution: Some("SECRET_ANSWER".to_string()),
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "A".to_string(),
                content_html: "Đáp án đúng".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "B".to_string(),
                content_html: "Đáp án sai".to_string(),
                is_correct: false,
            },
        ],
    };
    repo.create_question(&q).expect("Tạo question");

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state);
    (app, q_id)
}

#[tokio::test]
async fn test_sql_injection_resilience() {
    let (app, _) = setup_security_test_app().await;

    // 1. Thử chèn SQL Injection và XSS vào trường student_code / full_name
    let sqli_payload = serde_json::json!({
        "student_code": "' OR '1'='1' --",
        "full_name": "Hacker <script>alert(1)</script>",
        "class_name": "'; DROP TABLE exams; --"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(sqli_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Hệ thống chặn từ chối ngay lập tức ở tầng kiểm soát đầu vào (400 Bad Request)
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 2. Thử truy vấn trực tiếp CSDL với chuỗi SQLi để chứng minh lớp parameterized queries an toàn
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");
    let harmless_ex = repo.get_examinee_by_code("' OR '1'='1' --");
    assert!(harmless_ex.is_ok());
    assert!(harmless_ex.unwrap().is_none());
}

#[tokio::test]
async fn test_answer_key_never_leaked_in_paper_endpoint() {
    let (app, _) = setup_security_test_app().await;

    // Đăng ký lấy token
    let join_payload = serde_json::json!({
        "student_code": "SEC001",
        "full_name": "Nguyễn Bảo Mật",
        "class_name": "11A1"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    let token = join_json["session_token"].as_str().unwrap().to_string();

    // Thí sinh gọi lấy đề thi
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/paper")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let raw_paper_str = String::from_utf8_lossy(&body);

    // Tuyệt đối không được chứa is_correct hoặc đáp số bí mật trong JSON trả về client
    assert!(!raw_paper_str.contains("is_correct"));
    assert!(!raw_paper_str.contains("SECRET_ANSWER"));
}

#[tokio::test]
async fn test_unauthorized_and_invalid_token_rejected() {
    let (app, _) = setup_security_test_app().await;

    // 1. Gọi khi không có header Authorization
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/paper")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Gọi với token giả mạo không tồn tại
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/paper")
                .header("Authorization", "Bearer forged-fake-token-999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_double_submission_rejected() {
    let (app, q_id) = setup_security_test_app().await;

    // Đăng ký
    let join_payload = serde_json::json!({
        "student_code": "SEC002",
        "full_name": "Trần Văn Thật",
        "class_name": "11A1"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    let token = join_json["session_token"].as_str().unwrap().to_string();

    // Nộp bài lần 1
    let submit_payload = serde_json::json!({
        "answers": [
            {
                "question_id": q_id.to_string(),
                "selected_option": "A"
            }
        ]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/exam/submit")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(submit_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Đăng nhập lại với cùng SBD sau khi đã nộp bài -> Phải bị từ chối với mã 409 Conflict
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"]["code"].as_str().unwrap(), "ALREADY_SUBMITTED");
}


#[tokio::test]
async fn test_client_ip_resolution_trusted_proxy_vs_lan() {
    use axum::http::HeaderMap;
    use examplus_server::handlers::auth::resolve_client_ip;
    use std::net::SocketAddr;

    // 1. Kết nối qua mạng LAN (192.168.1.45)
    let lan_socket: SocketAddr = "192.168.1.45:51234".parse().unwrap();
    let mut spoofed_headers = HeaderMap::new();
    spoofed_headers.insert("cf-connecting-ip", "8.8.8.8".parse().unwrap());
    spoofed_headers.insert("x-forwarded-for", "9.9.9.9".parse().unwrap());

    // Máy chủ PHẢI tin tưởng Socket IP thực tế và bỏ qua hoàn toàn các header giả mạo từ client LAN
    let resolved_lan = resolve_client_ip(Some(lan_socket), &spoofed_headers);
    assert_eq!(resolved_lan, "192.168.1.45");

    // 2. Kết nối qua Localhost Proxy / Cloudflare Tunnel (127.0.0.1)
    let loopback_socket: SocketAddr = "127.0.0.1:44321".parse().unwrap();
    let mut tunnel_headers = HeaderMap::new();
    tunnel_headers.insert("cf-connecting-ip", "113.161.45.22".parse().unwrap());

    // Máy chủ tin tưởng CF-Connecting-IP từ local proxy
    let resolved_tunnel = resolve_client_ip(Some(loopback_socket), &tunnel_headers);
    assert_eq!(resolved_tunnel, "113.161.45.22");
}

#[tokio::test]
async fn test_device_binding_and_reconnection() {
    let (app, _) = setup_security_test_app().await;

    // Thiết bị 1 (Điện thoại) đăng nhập vào phòng thi
    let join_dev1 = serde_json::json!({
        "student_code": "SEC009",
        "full_name": "Lê Văn Thiết Bị",
        "class_name": "11A2",
        "device_id": "dev_phone_uuid_123"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_dev1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Thiết bị 1 F5 tải lại trang với cùng device_id -> Cho phép vào lại mượt mà
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_dev1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Thiết bị 2 (Máy tính khác) cố tình đăng nhập cùng SBD khi thiết bị 1 đang thi -> Bị từ chối xung đột
    let join_dev2 = serde_json::json!({
        "student_code": "SEC009",
        "full_name": "Lê Văn Thiết Bị",
        "class_name": "11A2",
        "device_id": "dev_laptop_uuid_456"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_dev2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"]["code"].as_str().unwrap(), "ACTIVE_SESSION_ON_OTHER_DEVICE");
}

#[tokio::test]
async fn test_external_lan_and_tunnel_cannot_access_teacher_endpoints() {
    use axum::extract::ConnectInfo;
    use std::net::SocketAddr;

    let (app, _) = setup_security_test_app().await;

    // 1. Giả lập client kết nối từ mạng LAN bên ngoài (192.168.1.100)
    let lan_addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();

    let mut req1 = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/exam-preview")
        .body(Body::empty())
        .unwrap();
    req1.extensions_mut().insert(ConnectInfo(lan_addr));

    let res1 = app.clone().oneshot(req1).await.unwrap();
    // Bắt buộc từ chối 403 Forbidden để chống lộ đáp án đề thi qua LAN
    assert_eq!(res1.status(), StatusCode::FORBIDDEN);
    let body = res1.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"]["code"].as_str().unwrap(), "TEACHER_ACCESS_DENIED");

    // 2. Thử truy cập Dashboard giáo viên (/teacher) từ LAN bên ngoài
    let mut req_ui = Request::builder()
        .method("GET")
        .uri("/teacher")
        .body(Body::empty())
        .unwrap();
    req_ui.extensions_mut().insert(ConnectInfo(lan_addr));
    let res_ui = app.clone().oneshot(req_ui).await.unwrap();
    assert_eq!(res_ui.status(), StatusCode::FORBIDDEN);

    // 3. Giả lập client từ ngoài Internet qua Cloudflare Quick Tunnel (có header cf-connecting-ip)
    let loopback_addr: SocketAddr = "127.0.0.1:51234".parse().unwrap();
    let mut req2 = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/grades")
        .header("cf-connecting-ip", "113.161.45.22")
        .body(Body::empty())
        .unwrap();
    req2.extensions_mut().insert(ConnectInfo(loopback_addr));

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::FORBIDDEN);

    // 4. Client cục bộ hợp lệ (Localhost không qua tunnel) truy cập thành công
    let mut req_local = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/status")
        .body(Body::empty())
        .unwrap();
    req_local.extensions_mut().insert(ConnectInfo(loopback_addr));

    let res_local = app.clone().oneshot(req_local).await.unwrap();
    assert_eq!(res_local.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_exam_paper_rejected_before_exam_starts() {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");
    let exam_id = Uuid::new_v4();

    // Ca thi đang ở trạng thái Ready (chưa bắt đầu)
    let exam = Exam {
        id: exam_id,
        title: "Kiểm tra Chưa Mở".to_string(),
        subject: "Toán".to_string(),
        duration_minutes: 45,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::Ready,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: None,
    };
    repo.create_exam(&exam).expect("Tạo exam");

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state);

    // Thí sinh đăng ký vào phòng chờ
    let join_payload = serde_json::json!({
        "student_code": "SEC010",
        "full_name": "Lê Văn Chờ",
        "class_name": "11A1"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    let token = join_json["session_token"].as_str().unwrap().to_string();

    // Thí sinh cố tình gọi GET /api/v1/exam/paper trước khi giám thị bấm bắt đầu
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/paper")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Phải bị từ chối 403 Forbidden với mã lỗi EXAM_NOT_STARTED
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"]["code"].as_str().unwrap(), "EXAM_NOT_STARTED");
}

#[tokio::test]
async fn test_repeated_submit_with_same_token_blocked() {
    let (app, q_id) = setup_security_test_app().await;

    // Đăng ký
    let join_payload = serde_json::json!({
        "student_code": "SEC011",
        "full_name": "Ngô Thị Lặp",
        "class_name": "11A1"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    let token = join_json["session_token"].as_str().unwrap().to_string();

    let submit_payload = serde_json::json!({
        "answers": [
            {
                "question_id": q_id.to_string(),
                "selected_option": "A"
            }
        ]
    });

    // Nộp bài lần 1 -> Thành công (200 OK)
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/exam/submit")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(submit_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);

    // Cố tình gửi submit lần 2 bằng cùng token -> Bắt buộc bị từ chối 409 Conflict
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/exam/submit")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(submit_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let err2: Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(err2["error"]["code"].as_str().unwrap(), "ALREADY_SUBMITTED");

    // Thử tải lại đề thi sau khi đã nộp bài -> 409 Conflict
    let res_paper = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/paper")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_paper.status(), StatusCode::CONFLICT);

    // Thử lưu nháp sau khi đã nộp bài -> 409 Conflict
    let res_draft = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/exam/save-draft")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(submit_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_draft.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_drive_path_traversal_prevention() {
    let (app, _) = setup_security_test_app().await;

    // 1. Thử tải file với path traversal trên tham số folder
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/drive/download?filename=secret.docx&folder=../../Windows")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::BAD_REQUEST);

    // 2. Thử xóa file với path traversal trong body payload
    let delete_payload = serde_json::json!({
        "filename": "important.docx",
        "folder": "../../"
    });
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/drive/delete")
                .header("Content-Type", "application/json")
                .body(Body::from(delete_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cross_origin_and_cross_site_requests_blocked_on_teacher_endpoints() {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");
    let exam_id = Uuid::new_v4();
    let state = AppState::new(repo, Some(exam_id));
    let teacher_token = state.teacher_secret.to_string();
    let app = build_router(state);

    let loopback_addr: std::net::SocketAddr = "127.0.0.1:54321".parse().unwrap();

    // 1. Request từ loopback nhưng có header Origin độc hại (Cross-Site Port Attack / Drive-by)
    let mut req_evil_origin = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/status")
        .header("Origin", "https://malicious-website.com")
        .body(Body::empty())
        .unwrap();
    req_evil_origin.extensions_mut().insert(axum::extract::ConnectInfo(loopback_addr));

    let res1 = app.clone().oneshot(req_evil_origin).await.unwrap();
    assert_eq!(res1.status(), StatusCode::FORBIDDEN);

    // 2. Request mang header Sec-Fetch-Site: cross-site
    let mut req_cross_site = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/status")
        .header("Sec-Fetch-Site", "cross-site")
        .body(Body::empty())
        .unwrap();
    req_cross_site.extensions_mut().insert(axum::extract::ConnectInfo(loopback_addr));

    let res2 = app.clone().oneshot(req_cross_site).await.unwrap();
    assert_eq!(res2.status(), StatusCode::FORBIDDEN);

    // 3. Request mang Origin hợp lệ (Localhost / 127.0.0.1) -> Cho phép (200 OK)
    let mut req_valid_origin = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/status")
        .header("Origin", "http://127.0.0.1:8080")
        .body(Body::empty())
        .unwrap();
    req_valid_origin.extensions_mut().insert(axum::extract::ConnectInfo(loopback_addr));

    let res3 = app.clone().oneshot(req_valid_origin).await.unwrap();
    assert_eq!(res3.status(), StatusCode::OK);

    // 4. Request từ origin ngoài nhưng mang token quản trị hợp trị -> Cho phép (200 OK)
    let mut req_with_token = Request::builder()
        .method("GET")
        .uri("/api/v1/teacher/status")
        .header("Origin", "https://authorized-client.com")
        .header("X-Teacher-Token", &teacher_token)
        .body(Body::empty())
        .unwrap();
    req_with_token.extensions_mut().insert(axum::extract::ConnectInfo(loopback_addr));

    let res4 = app.clone().oneshot(req_with_token).await.unwrap();
    assert_eq!(res4.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_student_registration_xss_and_injection_blocked() {
    let (app, _) = setup_security_test_app().await;

    // 1. Thí sinh đăng ký với student_code chứa ký tự HTML script
    let xss_code_payload = serde_json::json!({
        "student_code": "<script>alert(1)</script>",
        "full_name": "Nguyễn Văn Hợp Lệ",
        "class_name": "12A1"
    });
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(xss_code_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::BAD_REQUEST);

    // 2. Thí sinh đăng ký với full_name chứa HTML tag
    let xss_name_payload = serde_json::json!({
        "student_code": "SEC099",
        "full_name": "Hacker <img src=x onerror=alert(1)>",
        "class_name": "12A1"
    });
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(xss_name_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::BAD_REQUEST);

    // 3. Thí sinh đăng ký với student_code vượt quá 30 ký tự
    let long_code = "A".repeat(35);
    let long_payload = serde_json::json!({
        "student_code": long_code,
        "full_name": "Nguyễn Văn Hợp Lệ",
        "class_name": "12A1"
    });
    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(long_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res3.status(), StatusCode::BAD_REQUEST);

    // 4. Thí sinh đăng ký hợp lệ với SBD chữ và số -> 200 OK
    let valid_payload = serde_json::json!({
        "student_code": "SEC-100",
        "full_name": "Trần Thị An Toàn",
        "class_name": "12A2"
    });
    let res4 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(valid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res4.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_google_oauth_start_generates_pkce_and_state() {
    let (app, _) = setup_security_test_app().await;

    // 1. Khởi tạo luồng Google Picker flow
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/start?flow=picker")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
    let loc = res
        .headers()
        .get("location")
        .expect("Có location header")
        .to_str()
        .unwrap();

    assert!(loc.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));
    assert!(loc.contains("trigger_onepick=true"));
    assert!(loc.contains("code_challenge_method=S256"));
    assert!(loc.contains("scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdrive.file"));
    assert!(loc.contains("state="));

    // 2. Callback với state không hợp lệ -> 400 Bad Request
    let res_invalid_state = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?state=tampered_or_invalid_state&code=test_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_invalid_state.status(), StatusCode::BAD_REQUEST);

    // 3. Callback hoàn toàn không có state -> 400 Bad Request
    let res_no_state = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test_code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_no_state.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_google_auth_status_does_not_leak_access_token() {
    let (app, _) = setup_security_test_app().await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);

    // Tuyệt đối không để lộ access_token hoặc refresh_token trong JSON response
    assert!(
        !body_str.contains("access_token"),
        "Endpoint /api/v1/auth/google/status không được serialize access_token!"
    );
    assert!(
        !body_str.contains("refresh_token"),
        "Endpoint /api/v1/auth/google/status không được serialize refresh_token!"
    );
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.get("persistent_session").and_then(|v| v.as_bool()).is_some());
    assert!(parsed.get("cloud_browse_ready").and_then(|v| v.as_bool()).is_some());
}

#[tokio::test]
async fn test_google_login_flow_requests_persistent_and_readonly_scopes() {
    let (app, _) = setup_security_test_app().await;

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/start?flow=login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("access_type=offline"));
    assert!(loc.contains("prompt=select_account%20consent"));
    assert!(loc.contains("drive.readonly"));
    assert!(loc.contains("drive.file"));
    assert!(loc.contains("code_challenge_method=S256"));
    assert!(!loc.contains("trigger_onepick=true"));
}

#[tokio::test]
async fn test_cloud_drive_endpoint_security_and_unauth_state() {
    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref v) = self.prev {
                std::env::set_var(self.key, v);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    let tmp_drive = tempfile::tempdir().unwrap();
    let tmp_vault = tempfile::tempdir().unwrap();
    let _g_drive = EnvGuard {
        key: "EXAMPLUS_DRIVE_ROOT",
        prev: std::env::var("EXAMPLUS_DRIVE_ROOT").ok(),
    };
    let _g_vault = EnvGuard {
        key: "EXAMPLUS_VAULT_DIR",
        prev: std::env::var("EXAMPLUS_VAULT_DIR").ok(),
    };
    std::env::set_var("EXAMPLUS_DRIVE_ROOT", tmp_drive.path());
    std::env::set_var("EXAMPLUS_VAULT_DIR", tmp_vault.path());

    let (app, _) = setup_security_test_app().await;

    // 1. Tunnel / LAN bị chặn bởi TeacherGuard -> 403 FORBIDDEN
    let proxy_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/drive/cloud-files")
                .header("cf-connecting-ip", "203.0.113.195")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(proxy_res.status(), StatusCode::FORBIDDEN);

    // 2. Localhost hợp lệ nhưng chưa đăng nhập Google -> 401 + needs_reauth = true
    let unauth_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/drive/cloud-files")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth_res.status(), StatusCode::UNAUTHORIZED);
    let bytes = unauth_res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["success"].as_bool(), Some(false));
    assert_eq!(json["needs_reauth"].as_bool(), Some(true));
}

#[tokio::test]
async fn test_oauth_callback_error_preserves_session() {
    let (app, _) = setup_security_test_app().await;

    // 1. Bắt đầu luồng để có một state hợp lệ trong bộ nhớ
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/start?flow=login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    let state_param = loc
        .split("state=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .expect("Có state trong redirect URL");

    // 2. Google trả về lỗi người dùng hủy (error=access_denied)
    let callback_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?state={}&error=access_denied",
                    state_param
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Phản hồi 200 OK với trang báo hủy an toàn, không tạo tài khoản giả
    assert_eq!(callback_res.status(), StatusCode::OK);
    let bytes = callback_res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(body_str.contains("Chưa hoàn tất ủy quyền Google"));
    assert!(!body_str.contains("google.teacher@gmail.com"));

    // 3. Sử dụng lại state vừa dùng một lần (replay attack) -> Bị từ chối 400 Bad Request
    let replay_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?state={}&error=access_denied",
                    state_param
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay_res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_teacher_guard_blocks_lan_and_tunnel_access() {
    let (app, _) = setup_security_test_app().await;

    // 1. Thử truy cập API giáo viên qua Cloudflare Tunnel / External Proxy Header
    let proxy_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/export-excel")
                .header("cf-connecting-ip", "203.0.113.195")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(proxy_res.status(), StatusCode::FORBIDDEN);
    let bytes = proxy_res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(body_str.contains("TEACHER_ACCESS_DENIED"));

    // 2. Thử truy cập API giáo viên từ trang web độc hại bên ngoài (Origin check)
    let cross_origin_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/export-excel")
                .header("origin", "https://malicious-attacker.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(cross_origin_res.status(), StatusCode::FORBIDDEN);
    let bytes2 = cross_origin_res.into_body().collect().await.unwrap().to_bytes();
    let body_str2 = String::from_utf8_lossy(&bytes2);
    assert!(body_str2.contains("TEACHER_ACCESS_DENIED"));
}
