//! # Intra-Part Exam Shuffler
//!
//! Mô-đun xáo trộn đề thi bảo toàn tính toàn vẹn cấu trúc GDPT 2018:
//! - Xáo trộn câu hỏi nội bộ trong từng Phần độc lập (Phần 1, Phần 2, Phần 3 không bị trộn lẫn sang nhau).
//! - Trong Phần 1: Xáo trộn các phương án A, B, C, D và tự động tính toán lại bảng đáp án mới tương ứng.
//! - Trong Phần 2: Xáo trộn thứ tự các câu hỏi đúng/sai, giữ nguyên liên kết các mệnh đề a, b, c, d nội bộ.
//! - Trong Phần 3: Xáo trộn thứ tự các câu hỏi trả lời ngắn và cập nhật lại đáp án tương ứng.
//! - Tái đánh số thứ tự từ 1..N trong từng phần.

use crate::parser::answer_key::ExamAnswerKey;
use crate::parser::docx::ParsedExam;

/// Bộ sinh số giả ngẫu nhiên SplitMix64 độc lập, tái lập 100% theo seed
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() % (upper as u64)) as usize
        }
    }

    /// Thuật toán Fisher-Yates shuffle
    fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.gen_range(i + 1);
            slice.swap(i, j);
        }
    }
}

/// Xáo trộn đề thi và đáp án theo quy tắc bảo toàn cấu trúc GDPT 2018
pub fn shuffle_exam(
    exam: &ParsedExam,
    key: &ExamAnswerKey,
    new_variant_code: &str,
    seed: u64,
) -> (ParsedExam, ExamAnswerKey) {
    let mut rng = SimpleRng::new(seed);

    let mut new_exam = exam.clone();
    new_exam.variant_code = Some(new_variant_code.to_string());

    let mut new_key = ExamAnswerKey::new(new_variant_code);

    // 1. Xáo trộn Phần 1 (Trắc nghiệm 4 phương án)
    let mut p1_questions = exam.part1_questions.clone();
    rng.shuffle(&mut p1_questions);

    let standard_labels = ["A", "B", "C", "D", "E", "F"];

    for (new_idx, q) in p1_questions.iter_mut().enumerate() {
        let new_q_number = (new_idx + 1) as u32;
        let old_q_number = q.number;

        // Xác định phương án đúng ban đầu
        let orig_correct_label = key.part1_answers.get(&old_q_number);

        for opt in &mut q.options {
            if let Some(lbl) = orig_correct_label {
                if opt.label.eq_ignore_ascii_case(lbl) {
                    opt.is_correct = Some(true);
                } else if opt.is_correct.is_none() {
                    opt.is_correct = Some(false);
                }
            }
        }

        // Xáo trộn các phương án A, B, C, D
        rng.shuffle(&mut q.options);

        // Đánh lại nhãn A, B, C, D và cập nhật key mới
        for (opt_idx, opt) in q.options.iter_mut().enumerate() {
            let label = if opt_idx < standard_labels.len() {
                standard_labels[opt_idx].to_string()
            } else {
                format!("{}", (b'A' + opt_idx as u8) as char)
            };

            let was_correct = opt.is_correct == Some(true);
            opt.label = label.clone();

            if was_correct {
                new_key.part1_answers.insert(new_q_number, label);
            }
        }
        if let Some(meta) = key.part1_meta.get(&old_q_number) {
            new_key.part1_meta.insert(new_q_number, meta.clone());
        }

        q.number = new_q_number;
    }
    new_exam.part1_questions = p1_questions;

    // 2. Xáo trộn Phần 2 (Đúng/Sai)
    let mut p2_questions = exam.part2_questions.clone();
    rng.shuffle(&mut p2_questions);

    for (new_idx, q) in p2_questions.iter_mut().enumerate() {
        let new_q_number = (new_idx + 1) as u32;
        let old_q_number = q.number;

        if let Some(ans) = key.part2_answers.get(&old_q_number) {
            new_key.part2_answers.insert(new_q_number, ans.clone());
        }
        if let Some(meta) = key.part2_meta.get(&old_q_number) {
            new_key.part2_meta.insert(new_q_number, meta.clone());
        }

        q.number = new_q_number;
    }
    new_exam.part2_questions = p2_questions;

    // 3. Xáo trộn Phần 3 (Trả lời ngắn)
    let mut p3_questions = exam.part3_questions.clone();
    rng.shuffle(&mut p3_questions);

    for (new_idx, q) in p3_questions.iter_mut().enumerate() {
        let new_q_number = (new_idx + 1) as u32;
        let old_q_number = q.number;

        if let Some(ans) = key.part3_answers.get(&old_q_number) {
            new_key.part3_answers.insert(new_q_number, ans.clone());
        }
        if let Some(meta) = key.part3_meta.get(&old_q_number) {
            new_key.part3_meta.insert(new_q_number, meta.clone());
        }

        q.number = new_q_number;
    }
    new_exam.part3_questions = p3_questions;

    (new_exam, new_key)
}
