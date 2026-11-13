use chrono::Utc;
use examplus_core::db::DbRepository;
use examplus_core::models::{
    AuditViolation, Exam, ExamStatus, ExamType, ExamVariant, Examinee, ExamineeStatus, NetworkMode,
    PartType, Question, QuestionOption, Submission, SubmissionAnswer, ViolationType,
};
use uuid::Uuid;

#[test]
fn test_database_lifecycle_and_crud() {
    let repo = DbRepository::in_memory().expect("Khởi tạo SQLite in-memory thành công");

    // 1. Tạo Exam
    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Kiểm tra Cuối kỳ 1 Toán 11".to_string(),
        subject: "Toán".to_string(),
        duration_minutes: 90,
        exam_type: ExamType::Gdpt2018,
        status: ExamStatus::Ready,
        network_mode: NetworkMode::OfflineLan,
        public_url: Some("http://192.168.1.100:8080".to_string()),
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: Some("docx".to_string()),
    };
    repo.create_exam(&exam).expect("Tạo exam thành công");

    let loaded_exam = repo.get_exam(exam_id).expect("Query exam").expect("Phải tìm thấy exam");
    assert_eq!(loaded_exam.title, "Kiểm tra Cuối kỳ 1 Toán 11");
    assert_eq!(loaded_exam.duration_minutes, 90);
    assert_eq!(loaded_exam.status, ExamStatus::Ready);
    assert!(loaded_exam.started_at.is_none());
    assert!(loaded_exam.end_at.is_none());

    // 2. Cập nhật trạng thái và thời gian Exam
    repo.update_exam_status(exam_id, ExamStatus::InProgress).expect("Cập nhật trạng thái");
    let now = Utc::now();
    let end = now + chrono::Duration::minutes(90);
    repo.update_exam_start_time(exam_id, now, end).expect("Cập nhật start time");

    let updated_exam = repo.get_exam(exam_id).unwrap().unwrap();
    assert_eq!(updated_exam.status, ExamStatus::InProgress);
    assert!(updated_exam.started_at.is_some());
    assert!(updated_exam.end_at.is_some());

    // 3. Tạo Variant
    let variant_id = Uuid::new_v4();
    let variant = ExamVariant {
        id: variant_id,
        exam_id,
        code: "132".to_string(),
        is_original: true,
    };
    repo.create_variant(&variant).expect("Tạo variant");
    let variants = repo.get_variants_by_exam(exam_id).expect("Lấy variants");
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].code, "132");

    // 4. Tạo Câu hỏi và Phương án
    let q_id = Uuid::new_v4();
    let question = Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part1,
        order_index: 1,
        content_html: "<p>Phương trình sin x = 1 có nghiệm là?</p>".to_string(),
        raw_text: Some("Phương trình sin x = 1 có nghiệm là?".to_string()),
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
                content_html: "x = pi/2 + k2pi".to_string(),
                is_correct: true,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "B".to_string(),
                content_html: "x = kpi".to_string(),
                is_correct: false,
            },
        ],
    };
    repo.create_question(&question).expect("Tạo question");
    let questions = repo.get_questions_by_exam(exam_id).expect("Lấy questions");
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].options.len(), 2);
    assert!(questions[0].options[0].is_correct);

    // 5. Đăng ký Thí sinh
    let examinee_id = Uuid::new_v4();
    let examinee = Examinee {
        id: examinee_id,
        exam_id,
        assigned_variant_id: Some(variant_id),
        student_code: "SBD001".to_string(),
        full_name: "Nguyễn Văn An".to_string(),
        class_name: "11A1".to_string(),
        session_token: "token-secret-123".to_string(),
        status: ExamineeStatus::Waiting,
        ip_address: Some("192.168.1.50".to_string()),
        user_agent: Some("Mozilla/5.0 Chrome".to_string()),
        joined_at: Utc::now(),
        last_heartbeat: Utc::now(),
    };
    repo.create_examinee(&examinee).expect("Tạo thí sinh");

    let found_by_token = repo
        .get_examinee_by_token("token-secret-123")
        .expect("Query examinee by token")
        .expect("Phải tìm thấy thí sinh");
    assert_eq!(found_by_token.full_name, "Nguyễn Văn An");

    // 6. Ghi nhận Bài nộp (Submission)
    let sub_id = Uuid::new_v4();
    let submission = Submission {
        id: sub_id,
        examinee_id,
        started_at: Some(Utc::now()),
        submitted_at: Some(Utc::now()),
        total_score: 9.25,
        part1_score: 2.75,
        part2_score: 3.50,
        part3_score: 3.00,
        is_graded: true,
    };
    repo.create_submission(&submission).expect("Ghi submission");
    let loaded_sub = repo
        .get_submission_by_examinee(examinee_id)
        .expect("Query submission")
        .expect("Phải có submission");
    assert_eq!(loaded_sub.total_score, 9.25);
    assert!(loaded_sub.is_graded);

    // 7. Ghi nhận Vi phạm (Audit Violation)
    let viol_id = Uuid::new_v4();
    let violation = AuditViolation {
        id: viol_id,
        examinee_id,
        violation_type: ViolationType::TabSwitch,
        duration_seconds: 5,
        details: Some("Rời khỏi màn hình thi sang Chrome tab khác".to_string()),
        timestamp: Utc::now(),
    };
    repo.create_violation(&violation).expect("Ghi violation");

    let violations = repo.list_violations_by_exam(exam_id).expect("Lấy violations của exam");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].0.student_code, "SBD001");
    assert_eq!(violations[0].1.duration_seconds, 5);

    // 8. Lưu và đọc chi tiết câu trả lời (Submission Answers)
    let ans_id = Uuid::new_v4();
    let sub_answer = SubmissionAnswer {
        id: ans_id,
        submission_id: sub_id,
        question_id: q_id,
        selected_option: Some("A".to_string()),
        sub_answers_json: None,
        short_answer_text: None,
        awarded_score: 0.25,
        is_correct: true,
    };
    repo.save_submission_answers(sub_id, &[sub_answer]).expect("Lưu submission answers");
    let loaded_answers = repo.get_submission_answers(sub_id).expect("Đọc submission answers");
    assert_eq!(loaded_answers.len(), 1);
    assert_eq!(loaded_answers[0].selected_option.as_deref(), Some("A"));
    assert!(loaded_answers[0].is_correct);

    // 9. Cập nhật tính đúng đắn của phương án & chấm lại điểm
    repo.update_question_options_correctness(q_id, &["B".to_string()]).expect("Đổi đáp án đúng sang B");
    let updated_questions = repo.get_questions_by_exam(exam_id).expect("Lấy lại questions");
    let opt_a = updated_questions[0].options.iter().find(|o| o.label == "A").unwrap();
    let opt_b = updated_questions[0].options.iter().find(|o| o.label == "B").unwrap();
    assert!(!opt_a.is_correct);
    assert!(opt_b.is_correct);

    repo.update_submission_scores(sub_id, 0.0, 3.50, 3.00, 6.50).expect("Cập nhật lại điểm");
    let re_sub = repo.get_submission_by_examinee(examinee_id).unwrap().unwrap();
    assert_eq!(re_sub.total_score, 6.50);

    // 10. Cập nhật câu trả lời ngắn & truy vấn exam_id theo question_id
    let q3_id = Uuid::new_v4();
    let q3 = Question {
        id: q3_id,
        exam_id,
        part_type: PartType::Part3,
        order_index: 1,
        content_html: "<p>Tìm m để hàm số liên tục?</p>".to_string(),
        raw_text: Some("Tìm m để hàm số liên tục?".to_string()),
        image_url: None,
        short_answer_solution: Some("5".to_string()),
        provenance_json: None,
        answer_meta_json: None,
        needs_review: false,
        options: Vec::new(),
    };
    repo.create_question(&q3).expect("Tạo question part 3");
    repo.update_question_short_answer(q3_id, Some("10")).expect("Cập nhật đáp án ngắn");
    let loaded_q3 = repo.get_questions_by_exam(exam_id).unwrap();
    let q3_found = loaded_q3.iter().find(|q| q.id == q3_id).unwrap();
    assert_eq!(q3_found.short_answer_solution.as_deref(), Some("10"));

    let resolved_exam_id = repo.get_exam_id_by_question(q3_id).unwrap();
    assert_eq!(resolved_exam_id, Some(exam_id));
}

#[test]
fn test_part2_true_false_and_punctuation_correctness() {
    let repo = DbRepository::in_memory().expect("Khởi tạo SQLite in-memory");
    let exam_id = Uuid::new_v4();
    let exam = Exam {
        id: exam_id,
        title: "Đề kiểm tra giữa học kỳ 1".to_string(),
        subject: "Vật Lí".to_string(),
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
    let q = Question {
        id: q_id,
        exam_id,
        part_type: PartType::Part2,
        order_index: 1,
        content_html: "<p>Bảng dưới đây thể hiện kết quả đo...</p>".to_string(),
        raw_text: Some("Bảng dưới đây...".to_string()),
        image_url: None,
        short_answer_solution: None,
        provenance_json: None,
        answer_meta_json: None,
        needs_review: true,
        options: vec![
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "a)".to_string(),
                content_html: "Đây là phép đo trực tiếp.".to_string(),
                is_correct: false,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "b)".to_string(),
                content_html: "Sai số tỉ đối của phép đo...".to_string(),
                is_correct: false,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "c)".to_string(),
                content_html: "Sai số ngẫu nhiên...".to_string(),
                is_correct: false,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                question_id: q_id,
                label: "d)".to_string(),
                content_html: "Kết quả của phép đo là...".to_string(),
                is_correct: false,
            },
        ],
    };
    repo.create_question(&q).unwrap();

    // Giáo viên chấm ý a và d là ĐÚNG
    // Client truyền ["a", "d"] không kèm dấu ngoặc
    repo.update_question_options_correctness(q_id, &["a".to_string(), "d".to_string()]).unwrap();

    let questions = repo.get_questions_by_exam(exam_id).unwrap();
    let opts = &questions[0].options;
    let opt_a = opts.iter().find(|o| o.label == "a)").unwrap();
    let opt_b = opts.iter().find(|o| o.label == "b)").unwrap();
    let opt_c = opts.iter().find(|o| o.label == "c)").unwrap();
    let opt_d = opts.iter().find(|o| o.label == "d)").unwrap();

    assert!(opt_a.is_correct, "Ý a phải là ĐÚNG");
    assert!(!opt_b.is_correct, "Ý b phải là SAI");
    assert!(!opt_c.is_correct, "Ý c phải là SAI");
    assert!(opt_d.is_correct, "Ý d phải là ĐÚNG");
}
