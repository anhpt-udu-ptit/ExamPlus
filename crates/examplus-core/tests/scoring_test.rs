use examplus_core::engine::scoring::{
    calculate_part1_score, calculate_part2_score, calculate_part3_score, calculate_total_score,
    normalize_number_string, StudentSubmission,
};
use examplus_core::parser::answer_key::TrueFalseAnswer;
use std::collections::HashMap;

#[test]
fn test_part1_scoring() {
    let mut answer_key = HashMap::new();
    answer_key.insert(1, "A".to_string());
    answer_key.insert(2, "B".to_string());
    answer_key.insert(3, "C".to_string());
    answer_key.insert(4, "D".to_string());

    let mut student_answers = HashMap::new();
    student_answers.insert(1, "A".to_string()); // Đúng -> +0.25
    student_answers.insert(2, "B".to_string()); // Đúng -> +0.25
    student_answers.insert(3, "A".to_string()); // Sai -> 0
    // Câu 4 bỏ trống -> 0

    let score = calculate_part1_score(&student_answers, &answer_key, 0.25);
    assert!((score - 0.50).abs() < 1e-6);
}

#[test]
fn test_part2_progressive_scoring() {
    let standard = TrueFalseAnswer {
        a: true,
        b: false,
        c: true,
        d: true,
    };

    // Đúng 0 ý -> 0.00
    let s0 = TrueFalseAnswer {
        a: false,
        b: true,
        c: false,
        d: false,
    };
    assert_eq!(calculate_part2_score(&s0, &standard), 0.0);

    // Đúng 1 ý -> 0.10
    let s1 = TrueFalseAnswer {
        a: true,
        b: true,
        c: false,
        d: false,
    };
    assert!((calculate_part2_score(&s1, &standard) - 0.10).abs() < 1e-6);

    // Đúng 2 ý -> 0.25
    let s2 = TrueFalseAnswer {
        a: true,
        b: false,
        c: false,
        d: false,
    };
    assert!((calculate_part2_score(&s2, &standard) - 0.25).abs() < 1e-6);

    // Đúng 3 ý -> 0.50
    let s3 = TrueFalseAnswer {
        a: true,
        b: false,
        c: true,
        d: false,
    };
    assert!((calculate_part2_score(&s3, &standard) - 0.50).abs() < 1e-6);

    // Đúng cả 4 ý -> 1.00
    let s4 = TrueFalseAnswer {
        a: true,
        b: false,
        c: true,
        d: true,
    };
    assert!((calculate_part2_score(&s4, &standard) - 1.00).abs() < 1e-6);
}

#[test]
fn test_part3_number_normalization() {
    assert_eq!(normalize_number_string("10,4"), "10.4");
    assert_eq!(normalize_number_string(" 10.4 "), "10.4");
    assert_eq!(normalize_number_string("-7"), "-7");
    assert_eq!(normalize_number_string(" -7,0 "), "-7");
    assert_eq!(normalize_number_string("0,5"), "0.5");
    assert_eq!(normalize_number_string("+15"), "15");

    let mut key = HashMap::new();
    key.insert(1, "10.4".to_string());
    key.insert(2, "-7".to_string());
    key.insert(3, "0.5".to_string());

    let mut student = HashMap::new();
    student.insert(1, "10,4".to_string()); // Khớp sau chuẩn hóa -> +0.5
    student.insert(2, " -7 ".to_string()); // Khớp -> +0.5
    student.insert(3, "0,50".to_string()); // Khớp dạng số -> +0.5

    let score = calculate_part3_score(&student, &key, 0.5);
    assert!((score - 1.50).abs() < 1e-6);
}

#[test]
fn test_full_exam_calculation() {
    let submission = StudentSubmission {
        part1: {
            let mut m = HashMap::new();
            m.insert(1, "A".to_string());
            m.insert(2, "B".to_string());
            m
        },
        part2: {
            let mut m = HashMap::new();
            m.insert(
                1,
                TrueFalseAnswer {
                    a: true,
                    b: false,
                    c: true,
                    d: true,
                },
            ); // 1.00đ
            m
        },
        part3: {
            let mut m = HashMap::new();
            m.insert(1, "10,4".to_string()); // 0.5đ
            m
        },
    };

    let result = calculate_total_score(
        &submission,
        &{
            let mut m = HashMap::new();
            m.insert(1, "A".to_string());
            m.insert(2, "B".to_string());
            m
        },
        &{
            let mut m = HashMap::new();
            m.insert(
                1,
                TrueFalseAnswer {
                    a: true,
                    b: false,
                    c: true,
                    d: true,
                },
            );
            m
        },
        &{
            let mut m = HashMap::new();
            m.insert(1, "10.4".to_string());
            m
        },
        0.25,
        0.5,
    );

    assert_eq!(result.part1_score, 0.5);
    assert_eq!(result.part2_score, 1.0);
    assert_eq!(result.part3_score, 0.5);
    assert_eq!(result.total_score, 2.0);
}
