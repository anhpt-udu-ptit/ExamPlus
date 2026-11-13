//! # GDPT 2018 Progressive Scoring Engine
//!
//! Mô-đun chấm điểm theo quy chế thi GDPT 2018:
//! - Phần I: Trắc nghiệm 4 phương án (0.25đ / câu)
//! - Phần II: Trắc nghiệm Đúng/Sai lũy tiến (1 đúng = 0.10đ, 2 đúng = 0.25đ, 3 đúng = 0.50đ, 4 đúng = 1.00đ)
//! - Phần III: Trắc nghiệm trả lời ngắn (Chuẩn hóa số học, so khớp số thực với sai số 1e-6)

use crate::parser::answer_key::TrueFalseAnswer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Dữ liệu bài làm của một thí sinh
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StudentSubmission {
    /// Phần I: Câu hỏi -> Phương án chọn (A, B, C, D)
    pub part1: HashMap<u32, String>,
    /// Phần II: Câu hỏi -> TrueFalseAnswer (a, b, c, d)
    pub part2: HashMap<u32, TrueFalseAnswer>,
    /// Phần III: Câu hỏi -> Chuỗi trả lời ngắn
    pub part3: HashMap<u32, String>,
}

/// Kết quả chấm điểm bài thi chi tiết
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamScoreResult {
    pub part1_score: f64,
    pub part2_score: f64,
    pub part3_score: f64,
    pub total_score: f64,
}

/// Chấm điểm Phần I (Trắc nghiệm nhiều lựa chọn)
pub fn calculate_part1_score(
    student_answers: &HashMap<u32, String>,
    answer_key: &HashMap<u32, String>,
    points_per_question: f64,
) -> f64 {
    let mut score = 0.0;
    for (q_num, correct_opt) in answer_key {
        if let Some(ans) = student_answers.get(q_num) {
            if ans.trim().eq_ignore_ascii_case(correct_opt.trim()) {
                score += points_per_question;
            }
        }
    }
    round_to_2_decimals(score)
}

/// Chấm điểm lũy tiến Phần II (Đúng/Sai) theo chuẩn GDPT 2018:
/// - Đúng 1 ý: 0.10 điểm
/// - Đúng 2 ý: 0.25 điểm
/// - Đúng 3 ý: 0.50 điểm
/// - Đúng 4 ý: 1.00 điểm
pub fn calculate_part2_score(
    student_answer: &TrueFalseAnswer,
    standard: &TrueFalseAnswer,
) -> f64 {
    let mut correct_count = 0;
    if student_answer.a == standard.a {
        correct_count += 1;
    }
    if student_answer.b == standard.b {
        correct_count += 1;
    }
    if student_answer.c == standard.c {
        correct_count += 1;
    }
    if student_answer.d == standard.d {
        correct_count += 1;
    }

    match correct_count {
        1 => 0.10,
        2 => 0.25,
        3 => 0.50,
        4 => 1.00,
        _ => 0.0,
    }
}

/// Chuẩn hóa chuỗi số cho Phần III:
/// - Thay dấu phẩy ',' thành dấu chấm '.'
/// - Cắt tỉa khoảng trắng
/// - Chuyển dấu trừ đặc biệt (−, –) thành dấu trừ chuẩn (-)
/// - Bỏ dấu cộng đầu dòng (+15 -> 15)
/// - Bỏ phần thập phân .0 thừa nếu là số nguyên (ví dụ: -7,0 -> -7)
pub fn normalize_number_string(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Thay thế ký tự dấu trừ unicode và dấu phẩy
    let mut cleaned = trimmed
        .replace('\u{2212}', "-") // Minus sign (−)
        .replace('\u{2013}', "-") // En-dash (–)
        .replace('\u{2014}', "-") // Em-dash (—)
        .replace(',', ".");

    if cleaned.starts_with('+') {
        cleaned.remove(0);
    }
    let cleaned = cleaned.trim();

    // Thử phân tích cú pháp số thực
    if let Ok(val) = cleaned.parse::<f64>() {
        if val.is_finite() {
            if val.fract() == 0.0 {
                return format!("{}", val as i64);
            } else {
                return format!("{}", val);
            }
        }
    }

    cleaned.to_string()
}

/// Chấm điểm Phần III (Trả lời ngắn)
pub fn calculate_part3_score(
    student_answers: &HashMap<u32, String>,
    answer_key: &HashMap<u32, String>,
    points_per_question: f64,
) -> f64 {
    let mut score = 0.0;
    for (q_num, correct_ans) in answer_key {
        if let Some(student_ans) = student_answers.get(q_num) {
            let norm_student = normalize_number_string(student_ans);
            let norm_key = normalize_number_string(correct_ans);

            if !norm_student.is_empty() {
                if norm_student.eq_ignore_ascii_case(&norm_key) {
                    score += points_per_question;
                } else if let (Ok(s_val), Ok(k_val)) = (
                    norm_student.parse::<f64>(),
                    norm_key.parse::<f64>(),
                ) {
                    if (s_val - k_val).abs() < 1e-6 {
                        score += points_per_question;
                    }
                }
            }
        }
    }
    round_to_2_decimals(score)
}

/// Chấm điểm tổng hợp toàn bộ bài thi
pub fn calculate_total_score(
    submission: &StudentSubmission,
    part1_key: &HashMap<u32, String>,
    part2_key: &HashMap<u32, TrueFalseAnswer>,
    part3_key: &HashMap<u32, String>,
    part1_points: f64,
    part3_points: f64,
) -> ExamScoreResult {
    let part1_score = calculate_part1_score(&submission.part1, part1_key, part1_points);

    let mut part2_score = 0.0;
    for (q_num, standard) in part2_key {
        if let Some(ans) = submission.part2.get(q_num) {
            part2_score += calculate_part2_score(ans, standard);
        }
    }
    part2_score = round_to_2_decimals(part2_score);

    let part3_score = calculate_part3_score(&submission.part3, part3_key, part3_points);
    let total_score = round_to_2_decimals(part1_score + part2_score + part3_score);

    ExamScoreResult {
        part1_score,
        part2_score,
        part3_score,
        total_score,
    }
}

fn round_to_2_decimals(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}
