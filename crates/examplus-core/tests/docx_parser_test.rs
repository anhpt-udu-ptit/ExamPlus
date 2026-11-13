use examplus_core::models::PartType;
use examplus_core::parser::docx::parse_docx;
use std::path::Path;

#[test]
fn test_parse_thuvienhoclieu_sample() {
    let path = Path::new("../../sample_exams/thuvienhoclieu_sample.docx");
    // Also support running test from root or crate dir
    let test_path = if path.exists() {
        path.to_path_buf()
    } else {
        let p = Path::new("sample_exams/thuvienhoclieu_sample.docx").to_path_buf();
        if !p.exists() {
            return;
        }
        p
    };

    let parsed = parse_docx(&test_path).expect("Should parse docx successfully");

    // Must detect GDPT 2018 format
    assert_eq!(parsed.exam_type, examplus_core::models::ExamType::Gdpt2018);

    // Phần I: 12 câu hỏi
    assert_eq!(
        parsed.part1_questions.len(),
        12,
        "Phần I phải có đúng 12 câu trắc nghiệm"
    );
    for q in &parsed.part1_questions {
        assert_eq!(q.part, PartType::Part1);
        assert!(
            q.options.len() >= 4,
            "Câu {} Phần I phải có ít nhất 4 phương án A, B, C, D (hiện có {})",
            q.number,
            q.options.len()
        );
    }

    // Phần II: 4 câu hỏi Đúng / Sai
    assert_eq!(
        parsed.part2_questions.len(),
        4,
        "Phần II phải có đúng 4 câu hỏi Đúng/Sai"
    );
    for q in &parsed.part2_questions {
        assert_eq!(q.part, PartType::Part2);
        assert_eq!(
            q.options.len(),
            4,
            "Câu {} Phần II phải có đủ 4 lệnh hỏi a, b, c, d (hiện có {})",
            q.number,
            q.options.len()
        );
    }

    // Phần III: 6 câu hỏi trả lời ngắn
    assert_eq!(
        parsed.part3_questions.len(),
        6,
        "Phần III phải có đúng 6 câu hỏi trả lời ngắn"
    );
    for q in &parsed.part3_questions {
        assert_eq!(q.part, PartType::Part3);
    }

    // Media: Phải trích xuất được các ảnh/công thức MathType
    assert!(
        !parsed.media_files.is_empty(),
        "Phải trích xuất được media files từ file Word"
    );
}

#[test]
fn test_parse_toanmath_made_132() {
    let path = Path::new("../../sample_exams/toanmath_extracted/Toan11_Made 132_CK1_2025.docx");
    let test_path = if path.exists() {
        path.to_path_buf()
    } else {
        let p = Path::new("sample_exams/toanmath_extracted/Toan11_Made 132_CK1_2025.docx").to_path_buf();
        if !p.exists() {
            return;
        }
        p
    };

    let parsed = parse_docx(&test_path).expect("Should parse docx successfully");

    // Nhận diện mã đề 132
    assert_eq!(parsed.variant_code.as_deref(), Some("132"));

    // Phần I: 12 câu
    assert_eq!(parsed.part1_questions.len(), 12);

    // Phần II: có câu hỏi đúng sai
    assert!(!parsed.part2_questions.is_empty());

    // Phần III: có câu hỏi trả lời ngắn
    assert!(!parsed.part3_questions.is_empty());

    // Media trích xuất
    assert!(!parsed.media_files.is_empty());
}

#[test]
fn test_parse_ma_de_1210() {
    let path = Path::new("../../sample_exams/ma_de_1210.docx");
    let test_path = if path.exists() {
        path.to_path_buf()
    } else {
        let p = Path::new("sample_exams/ma_de_1210.docx").to_path_buf();
        if !p.exists() {
            return;
        }
        p
    };

    let parsed = parse_docx(&test_path).expect("Should parse docx 1210 successfully");
    println!("Part 1 count: {}", parsed.part1_questions.len());
    println!("Part 2 count: {}", parsed.part2_questions.len());
    println!("Part 3 count: {}", parsed.part3_questions.len());
    let total = parsed.part1_questions.len() + parsed.part2_questions.len() + parsed.part3_questions.len();
    println!("Total questions: {}", total);
    
    assert_eq!(parsed.part1_questions.len(), 12, "Phần I phải có 12 câu");
    assert_eq!(parsed.part2_questions.len(), 4, "Phần II phải có 4 câu");
    assert_eq!(parsed.part3_questions.len(), 6, "Phần III phải có 6 câu");
    assert_eq!(total, 22, "Tổng số câu phải là 22");

    // Kiểm tra công thức MathType SVG:
    // 1. Phải có media_files và là SVG data url
    assert!(!parsed.media_files.is_empty(), "Phải có media files");
    let mut found_prime = false;
    let mut found_real_set = false;
    let mut found_dashed_box = false;

    for (k, url) in &parsed.media_files {
        if url.starts_with("data:image/svg+xml;base64,") {
            let b64 = &url["data:image/svg+xml;base64,".len()..];
            if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                let svg_text = String::from_utf8_lossy(&bytes);
                if svg_text.contains("&#x2032;") || svg_text.contains('′') {
                    found_prime = true;
                }
                if svg_text.contains("&#x211D;") || svg_text.contains('ℝ') {
                    found_real_set = true;
                }
                if k.contains("image23") && svg_text.contains("stroke-dasharray=\"4,4\"") {
                    found_dashed_box = true;
                }
            }
        }
    }

    assert!(found_prime, "Phải nhận diện được ký tự prime (A'B'C') trong công thức");
    assert!(found_real_set, "Phải nhận diện được ký tự tập số thực ℝ trong công thức Câu 4");
    assert!(!found_dashed_box, "Công thức Câu 4 không được sinh khung nét đứt rỗng");
}

#[test]
fn test_parse_large_toan_docx_under_five_seconds() {
    use examplus_core::parser::answer_key::parse_answer_key;
    use std::time::Instant;

    let path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\docx\88f089a5a007_thuvienhoclieu.com-De-thi-thu-tot-nghiep-THPT-2027-mon-Toan-THPT-NGUYEN-VIET-XUAN-PHU-THO-Lan-1.docx",
    );
    assert!(path.exists(), "Fixture Toán DOCX phải tồn tại");

    let t0 = Instant::now();
    let parsed = parse_docx(path).expect("Phải parse được đề Toán 88f089a5");
    let parse_elapsed = t0.elapsed();

    assert_eq!(parsed.part1_questions.len(), 12, "Phần I phải có 12 câu");
    assert_eq!(parsed.part2_questions.len(), 4, "Phần II phải có 4 câu");
    assert_eq!(parsed.part3_questions.len(), 6, "Phần III phải có 6 câu");
    assert!(!parsed.part1_questions[0].provenance.is_empty());
    assert!(
        parse_elapsed.as_secs_f64() < 8.0,
        "parse_docx đề Toán lớn quá chậm: {:.2}s (ngưỡng < 8s)",
        parse_elapsed.as_secs_f64()
    );

    let t1 = Instant::now();
    let keys = parse_answer_key(path).expect("Phải tách được đáp án DOCX 88f089a5");
    let key_elapsed = t1.elapsed();
    let dk = keys.get("default").expect("Phải có bảng đáp án default");
    assert_eq!(dk.part1_answers.len(), 12);
    assert_eq!(dk.part2_answers.len(), 4);
    assert_eq!(dk.part3_answers.len(), 6);
    assert!(
        key_elapsed.as_secs_f64() < 3.0,
        "parse_answer_key đề Toán lớn quá chậm: {:.2}s (ngưỡng < 3s)",
        key_elapsed.as_secs_f64()
    );
}

#[test]
fn test_inspect_user_docx() {
    let mut candidate_paths = Vec::new();
    if let Ok(p) = std::env::var("EXAMPLUS_TEST_DOCX") {
        candidate_paths.push(std::path::PathBuf::from(p));
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        candidate_paths.push(
            std::path::PathBuf::from(user_profile)
                .join("Downloads")
                .join("De kiem tra giua hoc ky 1.docx"),
        );
    }
    let Some(path) = candidate_paths.into_iter().find(|p| p.exists()) else {
        return;
    };
    let parsed = parse_docx(&path).expect("parse user docx");
    assert_eq!(parsed.part1_questions.len(), 12);
    assert_eq!(parsed.part2_questions.len(), 2);
    assert_eq!(parsed.part3_questions.len(), 4);

    // Verify Question 1 Option b does NOT have an image, and Question 2 stem HAS the image
    let q1 = &parsed.part2_questions[0];
    let opt_1b = q1.options.iter().find(|o| o.label == "b").expect("Option 1b");
    assert!(!opt_1b.html_content.contains("<img"), "Option 1b should not contain floating figure");

    let q2 = &parsed.part2_questions[1];
    assert!(q2.html_content.contains("<img"), "Question 2 stem should contain the graph figure");
}
