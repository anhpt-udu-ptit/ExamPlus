use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use examplus_core::db::DbRepository;
use examplus_core::models::{
    Exam, ExamStatus, ExamType, ExamVariant, NetworkMode, PartType, Question, QuestionOption,
};
use examplus_server::{build_router, AppState, ServerEvent};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_app() -> (axum::Router, Uuid) {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");

    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Khảo sát chất lượng Toán 11".to_string(),
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

    let var_id = Uuid::new_v4();
    let variant = ExamVariant {
        id: var_id,
        exam_id,
        code: "101".to_string(),
        is_original: true,
    };
    repo.create_variant(&variant).expect("Tạo variant");

    // Tạo 1 câu hỏi Phần 1 (Đáp án A đúng)
    let q_id = Uuid::new_v4();
    let q = Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>Phương trình sin x = 0 có nghiệm?</p>".to_string(),
        raw_text: Some("Phương trình sin x = 0".to_string()),
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "A".to_string(),
                content_html: "x = kpi".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "B".to_string(),
                content_html: "x = pi/2 + kpi".to_string(),
                is_correct: false,
            },
        ],
    };
    repo.create_question(&q).expect("Tạo question");

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state);
    (app, q_id)
}

async fn setup_test_app_with_state() -> (axum::Router, AppState, Uuid, Uuid) {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");

    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Khảo sát chất lượng Toán 11".to_string(),
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
        content_html: "<p>Phương trình sin x = 0 có nghiệm?</p>".to_string(),
        raw_text: Some("Phương trình sin x = 0".to_string()),
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "A".to_string(),
                content_html: "x = kpi".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "B".to_string(),
                content_html: "x = pi/2 + kpi".to_string(),
                is_correct: false,
            },
        ],
    };
    repo.create_question(&q).expect("Tạo question");

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state.clone());
    (app, state, exam_id, q_id)
}

#[tokio::test]
async fn test_full_exam_lifecycle_api() {
    let (app, q_id) = setup_test_app().await;

    // 1. Kiểm tra tải trang tĩnh HTML
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let html_str = String::from_utf8_lossy(&body);
    assert!(html_str.contains("ExamPlus"));

    // 2. Thí sinh đăng nhập vào phòng thi
    let join_payload = serde_json::json!({
        "student_code": "11001",
        "full_name": "Lê Văn An",
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

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(join_json["success"].as_bool().unwrap());
    let token = join_json["session_token"].as_str().unwrap().to_string();

    // 3. Kiểm tra trạng thái phòng chờ (Lobby Status)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/lobby-status")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Giáo viên bấm bắt đầu làm bài
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/start-exam")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Thí sinh tải đề thi bảo mật
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
    let paper_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(paper_json["started_at"].is_string());
    assert!(paper_json["end_at"].is_string());
    assert!(paper_json["remaining_seconds"].as_u64().unwrap() <= 2700);
    let questions = paper_json["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 1);
    // Đảm bảo đáp án đúng đã được ẩn (không lộ is_correct)
    let options = questions[0]["options"].as_array().unwrap();
    assert!(options[0].get("is_correct").is_none());

    // 6. Thí sinh nộp bài thi
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
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let submit_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(submit_json["success"].as_bool().unwrap());
    assert_eq!(submit_json["total_score"].as_f64().unwrap(), 0.25);

    // 7. Giáo viên xem tổng quan phòng thi
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let teacher_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(teacher_json["total_examinees"].as_u64().unwrap(), 1);
    assert_eq!(teacher_json["submitted_count"].as_u64().unwrap(), 1);
    assert!(teacher_json["started_at"].is_string());
    assert!(teacher_json["end_at"].is_string());
    assert!(teacher_json["remaining_seconds"].as_u64().unwrap() <= 2700);

    // 8. Giáo viên xuất bảng điểm Excel
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/export-excel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(body.len() > 1000, "File Excel xuất ra phải có dữ liệu hợp lệ");
}

#[tokio::test]
async fn test_generate_qr_code_svg() {
    let (app, _) = setup_test_app().await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/qr?url=http://192.168.1.5:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("image/svg+xml")
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("<svg"));

    // Kiểm tra không truyền url -> mặc định vẫn sinh SVG thành công
    let res_default = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/qr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_default.status(), StatusCode::OK);
    let body_default = res_default.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body_default).contains("<svg"));
}

#[tokio::test]
async fn test_master_timer_synchronization() {
    let (app, _state, _exam_id, _q_id) = setup_test_app_with_state().await;

    // 1. Thí sinh tham gia
    let join_payload = serde_json::json!({
        "student_code": "11002",
        "full_name": "Trần Thị Bình",
        "class_name": "11A2"
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
    let token = join_json["session_token"].as_str().unwrap();

    // 2. Trước khi bắt đầu: lobby status trả về 45 * 60 = 2700
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/lobby-status")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let lobby_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(lobby_json["remaining_seconds"].as_u64().unwrap(), 2700);

    // 3. Trước khi bắt đầu: teacher status có started_at / end_at / remaining_seconds là null
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let t_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(t_json["started_at"].is_null());
    assert!(t_json["end_at"].is_null());
    assert!(t_json["remaining_seconds"].is_null());

    // 4. Giáo viên bắt đầu đề thi
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/start-exam")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Sau khi bắt đầu: teacher status có started_at, end_at, remaining_seconds <= 2700
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/teacher/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let t_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(t_json["started_at"].is_string());
    assert!(t_json["end_at"].is_string());
    let t_rem = t_json["remaining_seconds"].as_u64().unwrap();
    assert!(t_rem <= 2700 && t_rem > 2690);

    // 6. Sau khi bắt đầu: lobby status có remaining_seconds <= 2700
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/exam/lobby-status")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let lobby_json: Value = serde_json::from_slice(&body).unwrap();
    let l_rem = lobby_json["remaining_seconds"].as_u64().unwrap();
    assert!(l_rem <= 2700 && l_rem > 2690);

    // 7. Học sinh tải paper: có remaining_seconds, started_at, end_at
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
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let p_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(p_json["started_at"].is_string());
    assert!(p_json["end_at"].is_string());
    let p_rem = p_json["remaining_seconds"].as_u64().unwrap();
    assert!(p_rem <= 2700 && p_rem > 2690);
}

#[tokio::test]
async fn test_grace_period_and_expiration() {
    let (app, state, exam_id, q_id) = setup_test_app_with_state().await;

    // 1. Thí sinh tham gia
    let join_payload = serde_json::json!({
        "student_code": "11003",
        "full_name": "Phạm Quốc Cường",
        "class_name": "11A3"
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
    let token = join_json["session_token"].as_str().unwrap();

    // Giả lập ca thi đã hết giờ 15 giây (vẫn trong khoảng ân hạn 30 giây)
    {
        let repo = state.repo.lock().await;
        let now = Utc::now();
        let started = now - chrono::Duration::minutes(45);
        let end = now - chrono::Duration::seconds(15);
        repo.update_exam_start_time(exam_id, started, end).unwrap();
    }

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
    assert_eq!(res.status(), StatusCode::OK, "Trong ân hạn 30s phải nộp thành công");
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let submit_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(submit_json["success"].as_bool().unwrap());

    // 2. Thí sinh khác nộp bài khi đã quá ân hạn 30 giây (hết giờ 35 giây trước)
    let join_payload2 = serde_json::json!({
        "student_code": "11004",
        "full_name": "Đỗ Hoàng Dũng",
        "class_name": "11A3"
    });
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/join")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let join_json2: Value = serde_json::from_slice(&body2).unwrap();
    let token2 = join_json2["session_token"].as_str().unwrap();

    {
        let repo = state.repo.lock().await;
        let now = Utc::now();
        let started = now - chrono::Duration::minutes(45);
        let end = now - chrono::Duration::seconds(35);
        repo.update_exam_start_time(exam_id, started, end).unwrap();
    }

    let res_expired = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/exam/submit")
                .header("Authorization", format!("Bearer {}", token2))
                .header("Content-Type", "application/json")
                .body(Body::from(submit_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_expired.status(), StatusCode::BAD_REQUEST, "Quá 30s ân hạn phải bị từ chối");
    let body_exp = res_expired.into_body().collect().await.unwrap().to_bytes();
    let exp_json: Value = serde_json::from_slice(&body_exp).unwrap();
    assert_eq!(exp_json["error"]["code"].as_str().unwrap(), "EXAM_EXPIRED");
}

#[tokio::test]
async fn test_upload_docx_auto_extracts_answers() {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");
    let state = AppState::new(repo, None);
    let app = build_router(state.clone());

    let path = std::path::Path::new("../../sample_exams/thuvienhoclieu_sample.docx");
    let test_path = if path.exists() {
        path.to_path_buf()
    } else {
        let p = std::path::Path::new("sample_exams/thuvienhoclieu_sample.docx").to_path_buf();
        if !p.exists() {
            return;
        }
        p
    };

    let docx_bytes = std::fs::read(&test_path).expect("Read sample docx");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/upload-docx")
                .header("Content-Type", "application/vnd.openxmlformats-officedocument.wordprocessingml.document")
                .body(Body::from(docx_bytes))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["part1_count"].as_u64().unwrap(), 12);
    assert_eq!(json["part2_count"].as_u64().unwrap(), 4);
    assert_eq!(json["part3_count"].as_u64().unwrap(), 6);

    let questions = json["questions"].as_array().expect("questions array");
    assert_eq!(questions.len(), 22);

    // Kiểm tra câu 1 Phần I: Đáp án B đúng (theo thuvienhoclieu_sample.docx)
    let q1 = &questions[0];
    assert_eq!(q1["number"].as_u64().unwrap(), 1);
    assert_eq!(q1["part"].as_str().unwrap(), "PART_1");
    let q1_opts = q1["options"].as_array().unwrap();
    let opt_b = q1_opts.iter().find(|o| o["label"] == "B").expect("Option B exists");
    assert_eq!(opt_b["is_correct"].as_bool().unwrap(), true, "Câu 1 phương án B phải đúng");
    let opt_a = q1_opts.iter().find(|o| o["label"] == "A").expect("Option A exists");
    assert_eq!(opt_a["is_correct"].as_bool().unwrap(), false, "Câu 1 phương án A phải sai");

    // Kiểm tra câu 1 Phần II (index 12 trong danh sách câu hỏi):
    // a) S, b) Đ, c) Đ, d) S
    let p2_q1 = &questions[12];
    assert_eq!(p2_q1["number"].as_u64().unwrap(), 1);
    assert_eq!(p2_q1["part"].as_str().unwrap(), "PART_2");
    let p2_opts = p2_q1["options"].as_array().unwrap();
    let opt_p2_a = p2_opts.iter().find(|o| o["label"] == "a").expect("Option a");
    assert_eq!(opt_p2_a["is_correct"].as_bool().unwrap(), false);
    let opt_p2_b = p2_opts.iter().find(|o| o["label"] == "b").expect("Option b");
    assert_eq!(opt_p2_b["is_correct"].as_bool().unwrap(), true);
    let opt_p2_c = p2_opts.iter().find(|o| o["label"] == "c").expect("Option c");
    assert_eq!(opt_p2_c["is_correct"].as_bool().unwrap(), true);
    let opt_p2_d = p2_opts.iter().find(|o| o["label"] == "d").expect("Option d");
    assert_eq!(opt_p2_d["is_correct"].as_bool().unwrap(), false);

    // Kiểm tra câu 1 Phần III (index 16):
    // Đáp án ngắn: "419"
    let p3_q1 = &questions[16];
    assert_eq!(p3_q1["number"].as_u64().unwrap(), 1);
    assert_eq!(p3_q1["part"].as_str().unwrap(), "PART_3");
    assert_eq!(p3_q1["short_answer"].as_str().unwrap(), "419");

    // Đồng thời kiểm tra trong DB SQLite xem is_correct và short_answer_solution đã được ghi chuẩn chưa
    let exam_id: Uuid = json["exam_id"].as_str().unwrap().parse().unwrap();
    let repo_guard = state.repo.lock().await;
    let db_questions = repo_guard.get_questions_by_exam(exam_id).unwrap();
    assert_eq!(db_questions.len(), 22);

    let db_p1_q1 = db_questions.iter().find(|q| q.part_type == PartType::Part1 && q.order_index == 1).unwrap();
    let db_opt_b = db_p1_q1.options.iter().find(|o| o.label == "B").unwrap();
    assert!(db_opt_b.is_correct);

    let db_p3_q1 = db_questions.iter().find(|q| q.part_type == PartType::Part3 && q.order_index == 1).unwrap();
    assert_eq!(db_p3_q1.short_answer_solution.as_deref(), Some("419"));
}


#[tokio::test]
async fn test_update_answer_key_and_regrade() {
    let (app, state, exam_id, q_id) = setup_test_app_with_state().await;

    // 1. Bắt đầu ca thi (giáo viên bấm Start Exam)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/start-exam")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. Thí sinh tham gia phòng thi (Join)
    let join_payload = serde_json::json!({
        "student_code": "HS2026_01",
        "full_name": "Lê Văn C",
        "class_name": "12A1"
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
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    let token = join_json["session_token"].as_str().unwrap();

    // 3. Thí sinh nộp bài: chọn đáp án B (trong khi đáp án đúng ban đầu là A)
    let submit_payload = serde_json::json!({
        "answers": [
            {
                "question_id": q_id.to_string(),
                "selected_option": "B"
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
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let submit_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(submit_json["total_score"].as_f64().unwrap(), 0.0);

    // Kiểm tra trong DB: bài làm đã được lưu vào submission_answers với điểm 0.0 và is_correct = false
    {
        let repo = state.repo.lock().await;
        let examinee = repo.get_examinee_by_token(token).unwrap().unwrap();
        let sub = repo.get_submission_by_examinee(examinee.id).unwrap().unwrap();
        assert_eq!(sub.total_score, 0.0);

        let answers = repo.get_submission_answers(sub.id).unwrap();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].selected_option.as_deref(), Some("B"));
        assert!(!answers[0].is_correct);
        assert_eq!(answers[0].awarded_score, 0.0);
    }

    // Lắng nghe sự kiện WebSocket broadcast
    let mut event_rx = state.event_sender.subscribe();

    // 4. Giáo viên cập nhật đáp án đúng của câu hỏi sang B qua endpoint update-answer-key
    let update_key_payload = serde_json::json!({
        "exam_id": exam_id,
        "updates": [
            {
                "question_id": q_id,
                "correct_options": ["B"],
                "short_answer": null
            }
        ]
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/update-answer-key")
                .header("Content-Type", "application/json")
                .body(Body::from(update_key_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let update_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(update_json["success"].as_bool().unwrap());
    assert_eq!(update_json["updated_questions"].as_u64().unwrap(), 1);
    assert_eq!(update_json["regraded_submissions"].as_u64().unwrap(), 1);

    // 5. Kiểm tra sự kiện WebSocket GradesUpdated đã được phát
    let received_event = event_rx.recv().await.expect("Nhận sự kiện GradesUpdated");
    match received_event {
        ServerEvent::GradesUpdated { exam_id: eid } => {
            assert_eq!(eid, exam_id.to_string());
        }
        other => panic!("Nhận sự kiện không mong đợi: {:?}", other),
    }

    // 6. Kiểm tra lại DB: điểm của học sinh đã được tự động chấm lại thành 0.25
    {
        let repo = state.repo.lock().await;
        let examinee = repo.get_examinee_by_token(token).unwrap().unwrap();
        let sub = repo.get_submission_by_examinee(examinee.id).unwrap().unwrap();
        assert_eq!(sub.total_score, 0.25);
        assert_eq!(sub.part1_score, 0.25);

        let answers = repo.get_submission_answers(sub.id).unwrap();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].selected_option.as_deref(), Some("B"));
        assert!(answers[0].is_correct);
        assert_eq!(answers[0].awarded_score, 0.25);
    }
}

#[tokio::test]
async fn test_teacher_grades_and_examinee_audit_api() {
    let (app, state, _exam_id, _q_id) = setup_test_app_with_state().await;

    // 1. Thí sinh đăng nhập
    let join_payload = serde_json::json!({
        "student_code": "HS007",
        "full_name": "James Bond",
        "class_name": "11A1"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/join")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(join_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let join_json: Value = serde_json::from_slice(&body).unwrap();
    let token = join_json["session_token"].as_str().unwrap();

    // 2. Ghi nhận vi phạm
    {
        let repo = state.repo.lock().await;
        let ex = repo.get_examinee_by_token(token).unwrap().unwrap();
        repo.create_violation(&examplus_core::models::AuditViolation {
            id: Uuid::new_v4(),
            examinee_id: ex.id,
            violation_type: examplus_core::models::ViolationType::TabSwitch,
            duration_seconds: 5,
            details: Some("Rời khỏi bài thi".to_string()),
            timestamp: Utc::now(),
        }).unwrap();
    }

    // 3. Gọi GET /api/v1/teacher/grades
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/grades")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let grades_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(grades_json.is_array());
    let arr = grades_json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["student_code"], "HS007");
    assert_eq!(arr[0]["violation_count"], 1);

    // 4. Gọi GET /api/v1/teacher/examinee-audit/HS007
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/examinee-audit/HS007")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let audit_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(audit_json["success"].as_bool().unwrap());
    assert_eq!(audit_json["student"]["student_code"], "HS007");
    assert_eq!(audit_json["violations"].as_array().unwrap().len(), 1);
    assert_eq!(audit_json["violations"][0]["violation_type"], "TAB_SWITCH");

    // 5. Gọi GET /api/v1/teacher/examinee-audit/NOT_FOUND
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/examinee-audit/NOT_FOUND")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

}

#[tokio::test]
async fn test_healthz_endpoint_returns_ok() {
    let (app, _) = setup_test_app().await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["db_status"], "connected");
}

#[tokio::test]
async fn test_generate_variants_and_save_exam_flow() {
    let (app, _) = setup_test_app().await;

    // 1. Tạo 2 mã đề bằng alias variant_count
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/generate-variants")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"variant_count": 2}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    let variants = json["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0], "101");
    assert_eq!(variants[1], "102");

    // 2. Tạo 6 mã đề bằng trường count
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/generate-variants")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"count": 6}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    let variants = json["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 6);

    // 3. Lưu đề thi vào CSDL SQLite qua POST /api/v1/teacher/save-exam
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/save-exam")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{
                    "title": "Đề Khảo Sát GDPT 2018",
                    "subject": "Vật Lý",
                    "duration_minutes": 50,
                    "total_questions": 22
                }"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let save_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(save_json["success"].as_bool().unwrap());

    // 4. Lấy danh sách đề thi đã lưu qua GET /api/v1/teacher/exams
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/exams")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let list_json: Value = serde_json::from_slice(&body).unwrap();
    assert!(list_json["success"].as_bool().unwrap());
    let exams = list_json["exams"].as_array().unwrap();
    assert!(!exams.is_empty());
}

#[tokio::test]
async fn test_toggle_network_offline_lan() {
    let (app, _) = setup_test_app().await;

    // Chuyển sang mạng LAN
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/toggle-network")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"mode": "LAN"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["mode"], "OFFLINE_LAN");
    assert_eq!(json["network_mode"], "OFFLINE_LAN");
}

#[tokio::test]
async fn test_generate_variants_payloads_and_invalid_exam_id_json() {
    let (app, _state, exam_id, _q_id) = setup_test_app_with_state().await;

    // 1. Payload có cả count và variant_count không được gây lỗi 422 duplicate field
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/generate-variants")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"count": 8, "variant_count": 8, "exam_id": "{}"}}"#,
                    exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["count"], 8);
    let variants = json["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 8);
    assert_eq!(variants[0], "101");
    assert_eq!(variants[7], "108");

    // 2. Payload chỉ dùng variant_count = 3
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/generate-variants")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"variant_count": 3}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["variants"].as_array().unwrap().len(), 3);

    // 3. exam_id không phải UUID hợp lệ phải trả về lỗi JSON 400 (không bao giờ trả text 422)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/generate-variants")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"count": 4, "exam_id": "exam-17943legacy"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: Value = serde_json::from_slice(&body).expect("Phải trả về JSON hợp lệ");
    assert_eq!(err_json["success"], false);
    assert_eq!(err_json["error"]["code"], "INVALID_EXAM_ID");

    // 4. GET /api/v1/teacher/exams trả đủ questionCount, savedAt và timestamp cho UI
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/exams")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let list_json: Value = serde_json::from_slice(&body).unwrap();
    let ex = &list_json["exams"][0];
    assert_eq!(ex["questionCount"], 1);
    assert!(ex["savedAt"].as_str().is_some());
    assert!(ex["timestamp"].as_i64().is_some());
}

#[tokio::test]
async fn test_variant_preview_delivery_and_shuffled_grading() {
    let repo = DbRepository::in_memory().expect("in-memory DB");
    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Đề kiểm tra xáo mã chuẩn".to_string(),
        subject: "Toán Học".to_string(),
        duration_minutes: 45,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::InProgress,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        end_at: Some(Utc::now() + chrono::Duration::minutes(45)),
        source_format: None,
    };
    repo.create_exam(&exam).unwrap();
    repo.create_variant(&ExamVariant {
        id: Uuid::new_v4(),
        exam_id,
        code: "101".to_string(),
        is_original: true,
    })
    .unwrap();

    // Tạo 4 câu Phần 1 (4 lựa chọn A-D), 1 câu Phần 2, 1 câu Phần 3
    for i in 1..=4 {
        let q_id = Uuid::new_v4();
        let q = Question {
            id: q_id,
            exam_id,
            part_type: PartType::Part1,
            order_index: i,
            content_html: format!("<p>Câu P1 số {}</p>", i),
            raw_text: None,
            image_url: None,
            short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
            options: ["A", "B", "C", "D"]
                .iter()
                .enumerate()
                .map(|(idx, lbl)| QuestionOption {
                    id: Uuid::new_v4(),
                    question_id: q_id,
                    label: lbl.to_string(),
                    content_html: format!("Phương án gốc {} của câu {}", lbl, i),
                    is_correct: idx == 0, // Gốc: A đúng
                })
                .collect(),
        };
        repo.create_question(&q).unwrap();
    }

    let p2_id = Uuid::new_v4();
    repo.create_question(&Question {
        id: p2_id,
        exam_id,
        part_type: PartType::Part2,
        order_index: 1,
        content_html: "<p>Câu P2 Đúng/Sai</p>".to_string(),
        raw_text: None,
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: p2_id,
                label: "a".to_string(),
                content_html: "Mệnh đề a".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: p2_id,
                label: "b".to_string(),
                content_html: "Mệnh đề b".to_string(),
                is_correct: false,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: p2_id,
                label: "c".to_string(),
                content_html: "Mệnh đề c".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: p2_id,
                label: "d".to_string(),
                content_html: "Mệnh đề d".to_string(),
                is_correct: false,
            },
        ],
    })
    .unwrap();

    let p3_id = Uuid::new_v4();
    repo.create_question(&Question {
        id: p3_id,
        exam_id,
        part_type: PartType::Part3,
        order_index: 1,
        content_html: "<p>Câu P3 Trả lời ngắn</p>".to_string(),
        raw_text: None,
        image_url: None,
        short_answer_solution: Some("42".to_string()),
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![],
    })
    .unwrap();

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state);

    // Sinh 4 mã đề (101..104)
    let gen_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/generate-variants")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"count": 4}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gen_res.status(), StatusCode::OK);

    // Đăng nhập nhiều SBD để chắc chắn bắt được thí sinh được phát một mã đã xáo (102/103/104)
    let mut chosen_token = String::new();
    let mut chosen_variant = String::new();
    for s_idx in 1..=25 {
        let join_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/join")
                    .method("POST")
                    .header("Content-Type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"student_code":"SHUF{:03}","full_name":"Thí Sinh {}","class_name":"12A1"}}"#,
                        s_idx, s_idx
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(join_res.status(), StatusCode::OK);
        let j_body = join_res.into_body().collect().await.unwrap().to_bytes();
        let j_json: Value = serde_json::from_slice(&j_body).unwrap();
        let v_code = j_json["assigned_variant_code"].as_str().unwrap().to_string();
        if v_code != "101" {
            chosen_token = j_json["session_token"].as_str().unwrap().to_string();
            chosen_variant = v_code;
            break;
        }
    }
    assert!(
        !chosen_token.is_empty() && chosen_variant != "101",
        "Phải có thí sinh nhận mã đề đã xáo"
    );

    // Lấy đề preview của giáo viên cho chính mã đã xáo đó
    let prev_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/teacher/exam-preview?exam_id={}&variant_code={}",
                    exam_id, chosen_variant
                ))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prev_res.status(), StatusCode::OK);
    let p_body = prev_res.into_body().collect().await.unwrap().to_bytes();
    let p_json: Value = serde_json::from_slice(&p_body).unwrap();
    let prev_qs = p_json["questions"].as_array().unwrap();

    // Lấy đề thí sinh qua /api/v1/exam/paper
    let paper_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/exam/paper")
                .method("GET")
                .header("Authorization", format!("Bearer {}", chosen_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paper_res.status(), StatusCode::OK);
    let st_body = paper_res.into_body().collect().await.unwrap().to_bytes();
    let st_json: Value = serde_json::from_slice(&st_body).unwrap();
    assert_eq!(st_json["variant_code"].as_str().unwrap(), chosen_variant);
    let st_qs = st_json["questions"].as_array().unwrap();
    assert_eq!(st_qs.len(), prev_qs.len());

    // Đảm bảo thứ tự câu hỏi và nội dung lựa chọn trên preview & thí sinh khớp nhau 100%,
    // đồng thời chọn đáp án ĐÚNG theo nhãn hiển thị của mã đã xáo
    let mut answers_vec: Vec<Value> = Vec::new();

    for (pq, sq) in prev_qs.iter().zip(st_qs.iter()) {
        assert_eq!(pq["id"], sq["id"]);
        let q_id_str = sq["id"].as_str().unwrap().to_string();
        let part = pq["part"].as_str().unwrap();
        if part == "PART_1" {
            let p_opts = pq["options"].as_array().unwrap();
            let s_opts = sq["options"].as_array().unwrap();
            let mut correct_lbl = None;
            for (po, so) in p_opts.iter().zip(s_opts.iter()) {
                assert_eq!(po["label"], so["label"]);
                assert_eq!(po["content_html"], so["content_html"]);
                if po["is_correct"].as_bool().unwrap_or(false) {
                    assert!(po["content_html"]
                        .as_str()
                        .unwrap()
                        .starts_with("Phương án gốc A"));
                    correct_lbl = Some(so["label"].as_str().unwrap().to_string());
                }
            }
            answers_vec.push(serde_json::json!({
                "question_id": q_id_str,
                "selected_option": correct_lbl.unwrap(),
            }));
        } else if part == "PART_2" {
            let mut tf_map = serde_json::Map::new();
            for opt in pq["options"].as_array().unwrap() {
                tf_map.insert(
                    opt["label"].as_str().unwrap().to_string(),
                    Value::Bool(opt["is_correct"].as_bool().unwrap()),
                );
            }
            answers_vec.push(serde_json::json!({
                "question_id": q_id_str,
                "sub_answers": Value::Object(tf_map),
            }));
        } else if part == "PART_3" {
            answers_vec.push(serde_json::json!({
                "question_id": q_id_str,
                "short_answer": pq["short_answer"].as_str().unwrap(),
            }));
        }
    }

    let submit_payload = serde_json::json!({
        "answers": answers_vec,
    });
    let sub_res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/exam/submit")
                .method("POST")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", chosen_token))
                .body(Body::from(submit_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sub_res.status(), StatusCode::OK);
    let sub_body = sub_res.into_body().collect().await.unwrap().to_bytes();
    let sub_json: Value = serde_json::from_slice(&sub_body).unwrap();
    assert!(sub_json["success"].as_bool().unwrap());
    assert_eq!(sub_json["part1_score"].as_f64().unwrap(), 1.0);
    assert_eq!(sub_json["part2_score"].as_f64().unwrap(), 1.0);
    assert_eq!(sub_json["part3_score"].as_f64().unwrap(), 0.5);
    assert_eq!(sub_json["total_score"].as_f64().unwrap(), 2.5);
}

#[tokio::test]
async fn test_open_room_activates_draft_exam_for_student_join_and_delete_exam() {
    let repo = DbRepository::in_memory().expect("in-memory DB");
    let exam_id = Uuid::new_v4();
    repo.create_exam(&Exam {
        id: exam_id,
        title: "Đề Nháp Từ Kho".to_string(),
        subject: "Toán Học".to_string(),
        duration_minutes: 50,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::Draft,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: None,
    })
    .unwrap();
    let q_id = Uuid::new_v4();
    repo.create_question(&Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>1 + 1 = ?</p>".to_string(),
        raw_text: None,
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![QuestionOption {
            id: Uuid::new_v4(),
            question_id: q_id,
            label: "A".to_string(),
            content_html: "2".to_string(),
            is_correct: true,
        }],
    })
    .unwrap();

    // Khởi động với current_exam_id = None
    let state = AppState::new(repo, None);
    let app = build_router(state);

    // Chưa mở phòng -> thí sinh gặp 404 NO_ACTIVE_EXAM
    let res_before = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/join")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"student_code":"TS01","full_name":"Nguyễn An","class_name":"12A"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_before.status(), StatusCode::NOT_FOUND);

    // Giáo viên bấm mở phòng cho đề từ kho
    let open_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/open-room")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"exam_id":"{}","room_name":"Phòng 1","network_mode":"LAN"}}"#,
                    exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(open_res.status(), StatusCode::OK);
    let open_json: Value =
        serde_json::from_slice(&open_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(open_json["success"], true);

    // Sau khi mở phòng -> thí sinh vào được ngay
    let res_after = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/join")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"student_code":"TS01","full_name":"Nguyễn An","class_name":"12A"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_after.status(), StatusCode::OK);

    // Xóa mã 101 phải bị chặn, xóa cả đề thì thành công
    let del_orig = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/delete-exam")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"exam_id":"{}","variant_code":"101"}}"#,
                    exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_orig.status(), StatusCode::BAD_REQUEST);

    let del_exam = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/delete-exam")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(r#"{{"exam_id":"{}"}}"#, exam_id)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_exam.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_upload_docx_accepts_payload_over_2mb() {
    let (app, _q_id) = setup_test_app().await;
    // Gửi payload > 2.5MB vượt ngưỡng mặc định 2MB của Axum Bytes extractor
    let oversized_dummy = vec![0x50u8; 2_600_000];
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/upload-docx?filename=large_exam.docx")
                .method("POST")
                .header("Content-Type", "application/octet-stream")
                .body(Body::from(oversized_dummy))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        res.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "Route /api/v1/teacher/upload-docx phải cho phép tải đề Word lớn hơn 2MB (tránh HTTP 413)"
    );
}

#[tokio::test]
async fn test_upload_pdf_exam_saves_questions_answers_and_generates_variants() {
    let (app, state, _, _) = setup_test_app_with_state().await;

    let rel_path = std::path::Path::new(
        "../../data/benchmark_corpus/raw/pdf_reference/23cbc867ba2e_de-thi-giua-ki-1-hoa-12-ket-noi-tri-thuc-de-so-2-1726200814.pdf",
    );
    let abs_path = std::path::Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\23cbc867ba2e_de-thi-giua-ki-1-hoa-12-ket-noi-tri-thuc-de-so-2-1726200814.pdf",
    );
    let pdf_path = if rel_path.exists() { rel_path } else { abs_path };
    assert!(pdf_path.exists(), "Sample PDF must exist at {:?}", pdf_path);
    let pdf_bytes = std::fs::read(pdf_path).expect("Read sample PDF");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/upload-exam?filename=hoa_12_de_so_2.pdf")
                .header("Content-Type", "application/pdf")
                .body(Body::from(pdf_bytes))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["source_format"].as_str(), Some("pdf"));
    assert_eq!(json["part1_count"].as_u64().unwrap(), 18);
    assert_eq!(json["part2_count"].as_u64().unwrap(), 4);
    assert_eq!(json["part3_count"].as_u64().unwrap(), 4);
    assert_eq!(json["total_questions"].as_u64().unwrap(), 26);

    let questions = json["questions"].as_array().expect("questions array");
    assert_eq!(questions.len(), 26);
    assert!(
        questions[0]["content_html"].as_str().unwrap_or("").contains("<sub>4</sub>"),
        "Công thức hóa học PDF phải giữ <sub>"
    );

    // Câu 1 Phần I -> D đúng
    let p1_q1 = &questions[0];
    let p1_q1_d = p1_q1["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["label"] == "D")
        .unwrap();
    assert_eq!(p1_q1_d["is_correct"].as_bool(), Some(true));

    // Câu 1 Phần II (index 18) -> a=S, b=Đ, c=Đ, d=Đ
    let p2_q1 = &questions[18];
    let p2_opts = p2_q1["options"].as_array().unwrap();
    assert_eq!(p2_opts.iter().find(|o| o["label"] == "a").unwrap()["is_correct"].as_bool(), Some(false));
    assert_eq!(p2_opts.iter().find(|o| o["label"] == "b").unwrap()["is_correct"].as_bool(), Some(true));
    assert_eq!(p2_opts.iter().find(|o| o["label"] == "c").unwrap()["is_correct"].as_bool(), Some(true));
    assert_eq!(p2_opts.iter().find(|o| o["label"] == "d").unwrap()["is_correct"].as_bool(), Some(true));

    // Câu 1 Phần III (index 22) -> "500"
    let p3_q1 = &questions[22];
    assert_eq!(p3_q1["short_answer"].as_str(), Some("500"));

    // Kiểm tra persistence trong SQLite
    let exam_id: Uuid = json["exam_id"].as_str().unwrap().parse().unwrap();
    {
        let repo_guard = state.repo.lock().await;
        let db_qs = repo_guard.get_questions_by_exam(exam_id).unwrap();
        assert_eq!(db_qs.len(), 26);
        let db_p3_1 = db_qs
            .iter()
            .find(|q| q.part_type == PartType::Part3 && q.order_index == 1)
            .unwrap();
        assert_eq!(db_p3_1.short_answer_solution.as_deref(), Some("500"));
    }

    // Xáo 3 mã đề cho đề PDF vừa nạp
    let var_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/generate-variants")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"exam_id":"{}","variant_count":3}}"#,
                    exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(var_res.status(), StatusCode::OK);
    let var_body = var_res.into_body().collect().await.unwrap().to_bytes();
    let var_json: Value = serde_json::from_slice(&var_body).unwrap();
    assert_eq!(var_json["variants"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_upload_invalid_or_blank_pdf_returns_clear_400_error() {
    let (app, _q_id) = setup_test_app().await;

    let res_bad = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/upload-exam?filename=bad.pdf")
                .body(Body::from(b"NOT A PDF".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bad.status(), StatusCode::BAD_REQUEST);
    let body_bad = res_bad.into_body().collect().await.unwrap().to_bytes();
    let j_bad: Value = serde_json::from_slice(&body_bad).unwrap();
    assert_eq!(j_bad["success"].as_bool(), Some(false));
    assert!(!j_bad["error"]["message"].as_str().unwrap_or("").is_empty());

    let res_empty_pdf = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/upload-docx?filename=blank.pdf")
                .body(Body::from(b"%PDF-1.4\n%%EOF".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_empty_pdf.status(), StatusCode::BAD_REQUEST);
    let body_ep = res_empty_pdf.into_body().collect().await.unwrap().to_bytes();
    let j_ep: Value = serde_json::from_slice(&body_ep).unwrap();
    assert_eq!(j_ep["success"].as_bool(), Some(false));
    assert!(!j_ep["error"]["message"].as_str().unwrap_or("").is_empty());
}

#[test]
fn test_drive_lists_both_docx_and_pdf_files() {
    let tmp_dir = tempfile::tempdir().expect("create tempdir");
    let root = tmp_dir.path();
    std::fs::create_dir_all(root.join("DeThi_Word")).unwrap();
    std::fs::create_dir_all(root.join("DeThi_PDF")).unwrap();
    std::fs::write(root.join("DeThi_Word").join("de_word.docx"), b"PK\x03\x04").unwrap();
    std::fs::write(root.join("DeThi_PDF").join("de_pdf.pdf"), b"%PDF-1.4").unwrap();

    let (files, _) = examplus_server::handlers::drive::list_all_drive_files(root);
    let names: Vec<(&str, &str, &str)> = files
        .iter()
        .map(|f| (f.name.as_str(), f.category.as_str(), f.folder.as_str()))
        .collect();
    assert!(names.contains(&("de_word.docx", "docx", "DeThi_Word")));
    assert!(names.contains(&("de_pdf.pdf", "pdf", "DeThi_PDF")));
}

static VAULT_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvRestoreGuard {
    key: &'static str,
    prev: Option<String>,
}
impl Drop for EnvRestoreGuard {
    fn drop(&mut self) {
        if let Some(ref v) = self.prev {
            std::env::set_var(self.key, v);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

#[test]
fn test_parse_token_exchange_response_and_errors() {
    use examplus_server::handlers::drive::parse_token_exchange_response;

    let ok_json = br#"{
        "access_token": "ya29.test_access",
        "refresh_token": "1//0g_test_refresh",
        "expires_in": 3599,
        "scope": "openid email https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/drive.readonly",
        "token_type": "Bearer"
    }"#;
    let parsed = parse_token_exchange_response(ok_json).expect("valid token json");
    assert_eq!(parsed.access_token, "ya29.test_access");
    assert_eq!(parsed.refresh_token.as_deref(), Some("1//0g_test_refresh"));
    assert_eq!(parsed.expires_in, Some(3599));
    assert!(parsed
        .scope
        .as_deref()
        .unwrap_or("")
        .contains("drive.readonly"));

    let err_json = br#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#;
    let err = parse_token_exchange_response(err_json).unwrap_err();
    assert!(err.contains("invalid_grant"));
}

#[test]
fn test_vault_bundle_persists_and_preserves_refresh_token() {
    use examplus_server::handlers::drive::{
        clear_backend_tokens, ensure_valid_access_token, get_stored_token_bundle,
        has_drive_readonly_scope, save_backend_token_bundle,
    };

    let _guard = VAULT_ENV_LOCK.lock().unwrap();
    let prev = std::env::var("EXAMPLUS_VAULT_DIR").ok();
    let tmp_vault = tempfile::tempdir().expect("create vault tempdir");
    std::env::set_var("EXAMPLUS_VAULT_DIR", tmp_vault.path());
    let _restore = EnvRestoreGuard {
        key: "EXAMPLUS_VAULT_DIR",
        prev,
    };

    let email = "Test.Teacher@gmail.com";
    let saved1 = save_backend_token_bundle(
        email,
        "ya29.initial",
        Some("1//0g_persistent_refresh"),
        Some(3600),
        Some("openid email https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/drive.readonly"),
    );
    assert_eq!(
        saved1.refresh_token.as_deref(),
        Some("1//0g_persistent_refresh")
    );
    assert!(has_drive_readonly_scope(saved1.scopes.as_deref()));

    // Giả lập Google refresh response không trả lại refresh_token và scope: bản lưu cũ phải được giữ nguyên!
    let saved2 = save_backend_token_bundle(email, "ya29.refreshed", None, Some(3600), None);
    assert_eq!(saved2.access_token, "ya29.refreshed");
    assert_eq!(
        saved2.refresh_token.as_deref(),
        Some("1//0g_persistent_refresh")
    );
    assert!(has_drive_readonly_scope(saved2.scopes.as_deref()));

    let reloaded = get_stored_token_bundle(email).expect("reload stored bundle");
    assert_eq!(reloaded.access_token, "ya29.refreshed");
    assert_eq!(
        reloaded.refresh_token.as_deref(),
        Some("1//0g_persistent_refresh")
    );

    // Token còn hạn -> ensure_valid_access_token(false) trả ngay không cần gọi mạng
    let valid = ensure_valid_access_token(email, false).expect("valid cached token");
    assert_eq!(valid.access_token, "ya29.refreshed");

    // Hỗ trợ đọc lại định dạng token cũ (không có refresh_token / expires_at / scopes)
    let legacy_email = "legacy.user@gmail.com";
    let legacy_file = tmp_vault.path().join("legacy_user_gmail_com_token.json");
    std::fs::write(
        &legacy_file,
        r#"{"email":"legacy.user@gmail.com","access_token":"ya29.legacy","updated_at":"2026-11-11T13:33:00Z"}"#,
    )
    .unwrap();
    let legacy_bundle = get_stored_token_bundle(legacy_email).expect("read legacy vault file");
    assert_eq!(legacy_bundle.access_token, "ya29.legacy");
    assert!(legacy_bundle.refresh_token.is_none());
    assert!(!has_drive_readonly_scope(legacy_bundle.scopes.as_deref()));
    assert!(ensure_valid_access_token(legacy_email, true).is_err());

    clear_backend_tokens(email);
    clear_backend_tokens(legacy_email);
    assert!(get_stored_token_bundle(email).is_none());
}

#[test]
fn test_drive_id_validation_and_query_builder() {
    use examplus_server::handlers::drive::{
        build_cloud_files_query, escape_drive_query_literal, is_valid_drive_id,
    };

    assert!(is_valid_drive_id("root"));
    assert!(is_valid_drive_id("1AbCdEf_123-xyz"));
    assert!(!is_valid_drive_id(""));
    assert!(!is_valid_drive_id("root' or '1'='1"));
    assert!(!is_valid_drive_id("../etc/passwd"));

    assert_eq!(
        escape_drive_query_literal(r"Đề 'Toán' \ 12"),
        r"Đề \'Toán\' \\ 12"
    );

    let q_recent = build_cloud_files_query("recent", None, None, "all").unwrap();
    assert!(q_recent.contains("trashed = false"));
    assert!(q_recent.contains("mimeType != 'application/vnd.google-apps.folder'"));
    assert!(q_recent.contains("application/pdf"));

    let q_mydrive =
        build_cloud_files_query("mydrive", Some("1Folder_99"), Some("Vật Lý '2026"), "docx")
            .unwrap();
    assert!(q_mydrive.contains("'1Folder_99' in parents"));
    assert!(q_mydrive.contains(r"name contains 'Vật Lý \'2026'"));

    let q_shared = build_cloud_files_query("shared", None, None, "pdf").unwrap();
    assert!(q_shared.contains("sharedWithMe = true"));

    assert!(build_cloud_files_query("mydrive", Some("bad'id"), None, "all").is_err());
}

#[test]
fn test_parse_curl_http_output_preserves_binary_bytes() {
    use examplus_server::handlers::drive::parse_curl_http_output;

    let mut raw = b"PK\x03\x04\x00\x0aBINARY_DOCX_DATA".to_vec();
    raw.extend_from_slice(b"\n200");
    let (status, body) = parse_curl_http_output(&raw).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, b"PK\x03\x04\x00\x0aBINARY_DOCX_DATA");

    let (status401, body401) = parse_curl_http_output(b"{\"error\":\"unauth\"}\n401").unwrap();
    assert_eq!(status401, 401);
    assert_eq!(body401, b"{\"error\":\"unauth\"}");
}

#[tokio::test]
async fn test_start_exam_returns_duration_and_add_time_extends_timer() {
    let (app, _q_id) = setup_test_app().await;

    // 1. Gia hạn trước khi bắt đầu tính giờ -> 400 EXAM_NOT_STARTED
    let add_early = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/add-time")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"minutes": 5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_early.status(), StatusCode::BAD_REQUEST);
    let err_json: Value =
        serde_json::from_slice(&add_early.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(err_json["error"]["code"], "EXAM_NOT_STARTED");

    // 2. Bắt đầu tính giờ -> trả đủ duration_minutes, end_at, remaining_seconds
    let start_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/start-exam")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start_res.status(), StatusCode::OK);
    let start_json: Value =
        serde_json::from_slice(&start_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(start_json["success"], true);
    let base_duration = start_json["duration_minutes"].as_u64().unwrap();
    let base_rem = start_json["remaining_seconds"].as_u64().unwrap();
    assert_eq!(base_duration, 45);
    assert!(start_json["end_at"].is_string());
    assert!((2690..=2705).contains(&base_rem), "remaining_seconds={}", base_rem);

    // 3. Cộng thêm 5 phút -> cập nhật DB và tăng remaining_seconds ~ 300s
    let add_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/teacher/add-time")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"minutes": 5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_res.status(), StatusCode::OK);
    let add_json: Value =
        serde_json::from_slice(&add_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(add_json["success"], true);
    assert_eq!(add_json["duration_minutes"].as_u64(), Some(50));
    let new_rem = add_json["remaining_seconds"].as_u64().unwrap();
    assert!(new_rem >= base_rem + 295, "new_rem={} vs base_rem={}", new_rem, base_rem);
}

#[tokio::test]
async fn test_open_room_resets_finished_exam_to_ready() {
    let repo = DbRepository::in_memory().unwrap();
    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Đề Đã Kết Thúc Trước Đó".to_string(),
        subject: "Toán".to_string(),
        duration_minutes: 45,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::Finished,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        started_at: Some(Utc::now() - chrono::Duration::minutes(50)),
        end_at: Some(Utc::now() - chrono::Duration::minutes(5)),
        created_at: Utc::now(),
        source_format: None,
    };
    repo.create_exam(&exam).unwrap();

    let state = AppState::new(repo, None);
    let app = build_router(state);

    let open_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/open-room")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"exam_id":"{}","room_name":"Phòng Mở Lại","network_mode":"OFFLINE_LAN"}}"#,
                    exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(open_res.status(), StatusCode::OK);

    let status_res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_res.status(), StatusCode::OK);
    let status_json: Value =
        serde_json::from_slice(&status_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(status_json["exam_status"], "READY");
    assert!(status_json["started_at"].is_null());
    assert!(status_json["end_at"].is_null());
}


#[tokio::test]
async fn test_update_answer_key_persists_content_and_import_answer_key() {
    let repo = DbRepository::in_memory().unwrap();
    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Đề Test Persistence".to_string(),
        subject: "Toán".to_string(),
        duration_minutes: 50,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::Ready,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: Some("docx".to_string()),
    };
    repo.create_exam(&exam).unwrap();

    let q_id = Uuid::new_v4();
    let opt_a = Uuid::new_v4();
    let opt_b = Uuid::new_v4();
    repo.create_question(&Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>Nội dung gốc</p>".to_string(),
        raw_text: Some("Nội dung gốc".to_string()),
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: true,
        options: vec![
            QuestionOption {
                id: opt_a,
                question_id: q_id,
                label: "A".to_string(),
                content_html: "Phương án A gốc".to_string(),
                is_correct: false,
            },
            QuestionOption {
                id: opt_b,
                question_id: q_id,
                label: "B".to_string(),
                content_html: "Phương án B gốc".to_string(),
                is_correct: false,
            },
        ],
    })
    .unwrap();

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state.clone());

    // 1. Cập nhật nội dung câu hỏi, đổi đáp án đúng thành B, và xóa needs_review
    let update_payload = serde_json::json!({
        "exam_id": exam_id,
        "updates": [{
            "question_id": q_id,
            "correct_options": ["B"],
            "content_html": "<p>Nội dung đã chỉnh sửa và duyệt</p>",
            "clear_needs_review": true,
            "option_updates": [{
                "option_id": opt_b,
                "content_html": "Phương án B đã sửa"
            }]
        }]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/update-answer-key")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // Kiểm tra trong CSDL SQLite
    let repo_lock = state.repo.lock().await;
    let qs = repo_lock.get_questions_by_exam(exam_id).unwrap();
    let q_updated = &qs[0];
    assert_eq!(q_updated.content_html, "<p>Nội dung đã chỉnh sửa và duyệt</p>");
    assert_eq!(q_updated.needs_review, false);
    let opt_b_updated = q_updated.options.iter().find(|o| o.label == "B").unwrap();
    assert_eq!(opt_b_updated.content_html, "Phương án B đã sửa");
    assert!(opt_b_updated.is_correct);
    drop(repo_lock);

    // 2. Nhập file đáp án rời qua endpoint /api/v1/teacher/import-answer-key
    let fake_answer_txt = b"1. A
";
    let import_res = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/teacher/import-answer-key?exam_id={}&filename=key.txt", exam_id))
                .method("POST")
                .body(Body::from(fake_answer_txt.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(import_res.status(), StatusCode::OK);
    let repo_lock = state.repo.lock().await;
    let qs_after_import = repo_lock.get_questions_by_exam(exam_id).unwrap();
    let opt_a_after = qs_after_import[0].options.iter().find(|o| o.label == "A").unwrap();
    assert!(opt_a_after.is_correct);
}

#[tokio::test]
async fn test_isolated_exam_rooms_and_distinct_links_no_student_leak() {
    let repo = DbRepository::in_memory().expect("in-memory DB");
    let template_exam_id = Uuid::new_v4();
    repo.create_exam(&Exam {
        id: template_exam_id,
        title: "Đề Khảo Sát Vật Lý 12".to_string(),
        subject: "Vật Lý".to_string(),
        duration_minutes: 50,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::Ready,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: None,
    })
    .unwrap();

    let q_id = Uuid::new_v4();
    repo.create_question(&Question {
        id: q_id,
        exam_id: template_exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>Đơn vị của gia tốc là gì?</p>".to_string(),
        raw_text: None,
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: vec![QuestionOption {
            id: Uuid::new_v4(),
            question_id: q_id,
            label: "A".to_string(),
            content_html: "m/s2".to_string(),
            is_correct: true,
        }],
    })
    .unwrap();

    let state = AppState::new(repo, None);
    let app = build_router(state);

    // 1. Mở Phòng Thi 1 từ đề mẫu
    let res_room1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/open-room")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"exam_id":"{}","room_name":"Phòng Thi 1","network_mode":"LAN"}}"#,
                    template_exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_room1.status(), StatusCode::OK);
    let json_room1: Value =
        serde_json::from_slice(&res_room1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json_room1["success"], true);

    let room1_id = json_room1["room_id"].as_str().unwrap().to_string();
    let room1_local_url = json_room1["room_local_url"].as_str().unwrap().to_string();
    // Phòng thi 1 phải có link riêng biệt kèm ?room=<room1_id>
    assert!(room1_local_url.contains(&format!("?room={}", room1_id)));
    assert_ne!(room1_id, template_exam_id.to_string());

    // 2. Thí sinh Phạm Tuấn Anh vào Phòng Thi 1
    let join_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/join")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"student_code":"02","full_name":"Phạm Tuấn Anh","class_name":"12A5","room_id":"{}"}}"#,
                    room1_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(join_res.status(), StatusCode::OK);

    // 3. Kiểm tra status Phòng Thi 1: Có đúng 1 thí sinh
    let status_room1_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/teacher/status?exam_id={}", room1_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_room1_res.status(), StatusCode::OK);
    let status_room1: Value =
        serde_json::from_slice(&status_room1_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(status_room1["total_examinees"], 1);

    // 4. Mở Phòng Thi 2 từ CÙNG đề mẫu đó
    let res_room2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/teacher/open-room")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(format!(
                    r#"{{"exam_id":"{}","room_name":"Phòng Thi 2","network_mode":"LAN"}}"#,
                    template_exam_id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_room2.status(), StatusCode::OK);
    let json_room2: Value =
        serde_json::from_slice(&res_room2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json_room2["success"], true);

    let room2_id = json_room2["room_id"].as_str().unwrap().to_string();
    let room2_local_url = json_room2["room_local_url"].as_str().unwrap().to_string();
    // Phòng Thi 2 phải có link riêng biệt hoàn toàn khác Phòng Thi 1
    assert!(room2_local_url.contains(&format!("?room={}", room2_id)));
    assert_ne!(room2_id, room1_id);
    assert_ne!(room2_id, template_exam_id.to_string());

    // 5. Kiểm tra status Phòng Thi 2: PHẢI HOÀN TOÀN TRẮNG (0 thí sinh, không bị rò rỉ Phạm Tuấn Anh từ Phòng 1)
    let status_room2_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/teacher/status?exam_id={}", room2_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_room2_res.status(), StatusCode::OK);
    let status_room2: Value =
        serde_json::from_slice(&status_room2_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(status_room2["total_examinees"], 0, "Phòng thi mới tạo không được chứa dữ liệu thí sinh của phòng khác");

    // 6. Kiểm tra bảng điểm Phòng Thi 2: Phải rỗng
    let grades_room2_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/teacher/grades?exam_id={}", room2_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grades_room2_res.status(), StatusCode::OK);
    let grades_room2: Value =
        serde_json::from_slice(&grades_room2_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(grades_room2.as_array().unwrap().len(), 0);

    // 7. Bảng điểm Phòng Thi 1: Có đúng thí sinh Phạm Tuấn Anh
    let grades_room1_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/teacher/grades?exam_id={}", room1_id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grades_room1_res.status(), StatusCode::OK);
    let grades_room1: Value =
        serde_json::from_slice(&grades_room1_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(grades_room1.as_array().unwrap().len(), 1);
    assert_eq!(grades_room1[0]["student_code"], "02");
    assert_eq!(grades_room1[0]["full_name"], "Phạm Tuấn Anh");
}
