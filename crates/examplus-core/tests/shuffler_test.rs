use examplus_core::engine::shuffler::shuffle_exam;
use examplus_core::models::PartType;
use examplus_core::parser::answer_key::{ExamAnswerKey, TrueFalseAnswer};
use examplus_core::parser::docx::{ParsedExam, ParsedOption, ParsedQuestion};
use std::collections::HashMap;

fn create_sample_parsed_exam() -> (ParsedExam, ExamAnswerKey) {
    let mut part1 = Vec::new();
    let mut key_p1 = HashMap::new();

    for i in 1..=4 {
        part1.push(ParsedQuestion {
            number: i,
            part: PartType::Part1,
            raw_prompt: format!("Câu hỏi {}", i),
            html_content: format!("<p>Câu hỏi {}</p>", i),
            options: vec![
                ParsedOption {
                    label: "A".to_string(),
                    raw_content: "Lựa chọn 1".to_string(),
                    html_content: "Lựa chọn 1".to_string(),
                    is_correct: Some(true),
                },
                ParsedOption {
                    label: "B".to_string(),
                    raw_content: "Lựa chọn 2".to_string(),
                    html_content: "Lựa chọn 2".to_string(),
                    is_correct: Some(false),
                },
                ParsedOption {
                    label: "C".to_string(),
                    raw_content: "Lựa chọn 3".to_string(),
                    html_content: "Lựa chọn 3".to_string(),
                    is_correct: Some(false),
                },
                ParsedOption {
                    label: "D".to_string(),
                    raw_content: "Lựa chọn 4".to_string(),
                    html_content: "Lựa chọn 4".to_string(),
                    is_correct: Some(false),
                },
            ],
            short_answer: None,
            ..Default::default()
        });
        key_p1.insert(i, "A".to_string());
    }

    let mut part2 = Vec::new();
    let mut key_p2 = HashMap::new();
    for i in 1..=2 {
        part2.push(ParsedQuestion {
            number: i,
            part: PartType::Part2,
            raw_prompt: format!("Câu hỏi đúng sai {}", i),
            html_content: format!("<p>Câu hỏi đúng sai {}</p>", i),
            options: vec![
                ParsedOption {
                    label: "a".to_string(),
                    raw_content: "Lệnh a".to_string(),
                    html_content: "Lệnh a".to_string(),
                    is_correct: Some(true),
                },
                ParsedOption {
                    label: "b".to_string(),
                    raw_content: "Lệnh b".to_string(),
                    html_content: "Lệnh b".to_string(),
                    is_correct: Some(false),
                },
                ParsedOption {
                    label: "c".to_string(),
                    raw_content: "Lệnh c".to_string(),
                    html_content: "Lệnh c".to_string(),
                    is_correct: Some(true),
                },
                ParsedOption {
                    label: "d".to_string(),
                    raw_content: "Lệnh d".to_string(),
                    html_content: "Lệnh d".to_string(),
                    is_correct: Some(false),
                },
            ],
            short_answer: None,
            ..Default::default()
        });
        key_p2.insert(
            i,
            TrueFalseAnswer {
                a: true,
                b: false,
                c: true,
                d: false,
            },
        );
    }

    let mut part3 = Vec::new();
    let mut key_p3 = HashMap::new();
    for i in 1..=3 {
        part3.push(ParsedQuestion {
            number: i,
            part: PartType::Part3,
            raw_prompt: format!("Câu trả lời ngắn {}", i),
            html_content: format!("<p>Câu trả lời ngắn {}</p>", i),
            options: Vec::new(),
            short_answer: Some(format!("{}", i * 10)),
            ..Default::default()
        });
        key_p3.insert(i, format!("{}", i * 10));
    }

    let exam = ParsedExam {
        title: "Đề gốc Toán".to_string(),
        subject: Some("Toán".to_string()),
        exam_type: examplus_core::models::ExamType::Gdpt2018,
        variant_code: Some("101".to_string()),
        duration_minutes: Some(45),
        part1_questions: part1,
        part2_questions: part2,
        part3_questions: part3,
        media_files: HashMap::new(),
        ..Default::default()
    };

    let key = ExamAnswerKey {
        variant_code: "101".to_string(),
        part1_answers: key_p1,
        part2_answers: key_p2,
        part3_answers: key_p3,
        ..Default::default()
    };

    (exam, key)
}

#[test]
fn test_intra_part_shuffling_preserves_count() {
    let (exam, key) = create_sample_parsed_exam();

    let (shuffled_exam, new_key) = shuffle_exam(&exam, &key, "202", 42);

    assert_eq!(shuffled_exam.variant_code.as_deref(), Some("202"));
    assert_eq!(new_key.variant_code, "202");

    // Số lượng câu hỏi trong từng phần không bao giờ thay đổi
    assert_eq!(shuffled_exam.part1_questions.len(), 4);
    assert_eq!(shuffled_exam.part2_questions.len(), 2);
    assert_eq!(shuffled_exam.part3_questions.len(), 3);

    // Không xáo trộn giữa các phần
    for q in &shuffled_exam.part1_questions {
        assert_eq!(q.part, PartType::Part1);
    }
    for q in &shuffled_exam.part2_questions {
        assert_eq!(q.part, PartType::Part2);
    }
    for q in &shuffled_exam.part3_questions {
        assert_eq!(q.part, PartType::Part3);
    }
}

#[test]
fn test_shuffled_answer_key_accuracy() {
    let (exam, key) = create_sample_parsed_exam();

    let (shuffled_exam, new_key) = shuffle_exam(&exam, &key, "303", 9999);

    // Đối chiếu với từng câu trong đề xáo
    for q in &shuffled_exam.part1_questions {
        let correct_label = new_key
            .part1_answers
            .get(&q.number)
            .expect("Mỗi câu phải có đáp án mới");

        let opt = q
            .options
            .iter()
            .find(|o| &o.label == correct_label)
            .expect("Phương án đúng phải tồn tại trong danh sách options");

        // Nội dung của phương án đúng ban đầu là "Lựa chọn 1"
        assert_eq!(
            opt.raw_content, "Lựa chọn 1",
            "Phương án đúng sau khi xáo vẫn phải trỏ về nội dung chuẩn ban đầu"
        );
    }
}
