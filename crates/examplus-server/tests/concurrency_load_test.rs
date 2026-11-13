//! # Concurrency & Load Stress Test
//!
//! Kiểm thử mô phỏng tải đồng thời (Concurrent Load Testing):
//! Mô phỏng 50 thí sinh kết nối đồng thời, điểm danh, tải đề và nộp bài cùng lúc trong thời gian ngắn.

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
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_concurrent_examinees_join_and_submit() {
    let repo = DbRepository::in_memory().expect("Tạo in-memory DB");

    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Thi Thử Đại Trà Toán 11".to_string(),
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

    // Tạo câu hỏi mẫu
    let q_id = Uuid::new_v4();
    let q = Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>Hàm số y = sin x tuần hoàn với chu kì?</p>".to_string(),
        raw_text: Some("Hàm số y = sin x tuần hoàn với chu kì?".to_string()),
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
                content_html: "2pi".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "B".to_string(),
                content_html: "pi".to_string(),
                is_correct: false,
            },
        ],
    };
    repo.create_question(&q).expect("Tạo question");

    let state = AppState::new(repo, Some(exam_id));
    let app = build_router(state);

    let concurrent_students = 50;
    let start_time = Instant::now();

    // 1. Giáo viên bấm bắt đầu làm bài
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

    // 2. Chạy 50 tác vụ đồng thời: Đăng nhập -> Tải đề thi -> Nộp bài
    let mut handles = Vec::new();
    let app_shared = Arc::new(app.clone());

    for i in 1..=concurrent_students {
        let app_clone = app_shared.clone();
        let q_id_clone = q_id;

        let handle = tokio::spawn(async move {
            let student_code = format!("HS{:03}", i);
            let join_payload = serde_json::json!({
                "student_code": student_code,
                "full_name": format!("Thí sinh số {}", i),
                "class_name": "11A1"
            });

            // Join
            let res = (*app_clone)
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
                .expect("Gửi request join");

            assert_eq!(res.status(), StatusCode::OK);
            let body = res.into_body().collect().await.unwrap().to_bytes();
            let join_json: Value = serde_json::from_slice(&body).unwrap();
            let token = join_json["session_token"].as_str().unwrap().to_string();

            // Lấy đề thi
            let res = (*app_clone)
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
                .expect("Gửi request paper");

            assert_eq!(res.status(), StatusCode::OK);

            // Nộp bài thi đồng loạt
            let submit_payload = serde_json::json!({
                "answers": [
                    {
                        "question_id": q_id_clone.to_string(),
                        "selected_option": "A"
                    }
                ]
            });

            let res = (*app_clone)
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
                .expect("Gửi request submit");

            assert_eq!(res.status(), StatusCode::OK);
            let body = res.into_body().collect().await.unwrap().to_bytes();
            let submit_json: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(submit_json["total_score"].as_f64().unwrap(), 0.25);
        });

        handles.push(handle);
    }

    // Đợi toàn bộ 50 thí sinh hoàn thành
    for h in handles {
        h.await.expect("Task thí sinh hoàn thành an toàn");
    }

    let elapsed = start_time.elapsed();
    println!("-> 50 Thí sinh hoàn thành toàn bộ chu trình trong: {:?}", elapsed);

    // 3. Giáo viên kiểm tra tổng kết phòng thi
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
    assert_eq!(
        teacher_json["total_examinees"].as_u64().unwrap(),
        concurrent_students as u64
    );
    assert_eq!(
        teacher_json["submitted_count"].as_u64().unwrap(),
        concurrent_students as u64
    );
}
