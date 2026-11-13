use examplus_core::parser::answer_key::parse_answer_key;
use std::path::Path;

#[test]
fn test_parse_multi_variant_answer_key() {
    let path = Path::new("../../sample_exams/toanmath_extracted/HUONG_DAN_CHAM_CUOI_KY_1_MON_TOAN_11.docx");
    let test_path = if path.exists() {
        path.to_path_buf()
    } else {
        let p = Path::new("sample_exams/toanmath_extracted/HUONG_DAN_CHAM_CUOI_KY_1_MON_TOAN_11.docx").to_path_buf();
        if !p.exists() {
            return;
        }
        p
    };

    let keys = parse_answer_key(&test_path).expect("Phải parse bảng đáp án thành công");

    // Phải đọc đủ 4 mã đề: 132, 209, 357, 485
    assert!(keys.contains_key("132"), "Phải có mã đề 132");
    assert!(keys.contains_key("209"), "Phải có mã đề 209");
    assert!(keys.contains_key("357"), "Phải có mã đề 357");
    assert!(keys.contains_key("485"), "Phải có mã đề 485");

    // Kiểm tra chi tiết mã đề 132
    let key132 = &keys["132"];

    // Phần I: 12 câu
    assert_eq!(key132.part1_answers.len(), 12);
    assert_eq!(key132.part1_answers.get(&1).map(String::as_str), Some("C"));
    assert_eq!(key132.part1_answers.get(&2).map(String::as_str), Some("B"));
    assert_eq!(key132.part1_answers.get(&3).map(String::as_str), Some("D"));
    assert_eq!(key132.part1_answers.get(&4).map(String::as_str), Some("C"));

    // Phần II: Câu 1 là ĐSĐĐ, Câu 2 là ĐSSĐ
    assert!(key132.part2_answers.len() >= 2);
    let q1_tf = key132.part2_answers.get(&1).expect("Phải có đáp án câu 1 Phần II");
    assert_eq!(q1_tf.a, true);
    assert_eq!(q1_tf.b, false);
    assert_eq!(q1_tf.c, true);
    assert_eq!(q1_tf.d, true);

    let q2_tf = key132.part2_answers.get(&2).expect("Phải có đáp án câu 2 Phần II");
    assert_eq!(q2_tf.a, true);
    assert_eq!(q2_tf.b, false);
    assert_eq!(q2_tf.c, false);
    assert_eq!(q2_tf.d, true);

    // Phần III: Trả lời ngắn
    assert!(key132.part3_answers.len() >= 3);
    assert_eq!(key132.part3_answers.get(&1).map(String::as_str), Some("162"));
    assert_eq!(key132.part3_answers.get(&2).map(String::as_str), Some("7"));
    assert_eq!(key132.part3_answers.get(&3).map(String::as_str), Some("1"));
}

#[test]
fn test_parse_inline_answer_key_thuvienhoclieu() {
    let path = Path::new("../../sample_exams/thuvienhoclieu_sample.docx");
    let test_path = if path.exists() {
        path.to_path_buf()
    } else {
        let p = Path::new("sample_exams/thuvienhoclieu_sample.docx").to_path_buf();
        if !p.exists() {
            return;
        }
        p
    };

    let keys = parse_answer_key(&test_path).expect("Phải parse được đáp án thuvienhoclieu");
    assert!(!keys.is_empty(), "Phải đọc được ít nhất 1 bảng đáp án");

    // Lấy bảng đáp án đầu tiên
    let key = keys.values().next().expect("Có ít nhất 1 đáp án");

    // Phần I có 12 câu
    assert_eq!(key.part1_answers.len(), 12);
    assert_eq!(key.part1_answers.get(&1).map(String::as_str), Some("B"));
    assert_eq!(key.part1_answers.get(&2).map(String::as_str), Some("A"));
    assert_eq!(key.part1_answers.get(&3).map(String::as_str), Some("D"));

    // Phần II có 4 câu
    assert_eq!(key.part2_answers.len(), 4);
    // Câu 1: a) S, b) Đ, c) Đ, d) S
    let q1 = key.part2_answers.get(&1).expect("Câu 1 Phần II");
    assert_eq!(q1.a, false);
    assert_eq!(q1.b, true);
    assert_eq!(q1.c, true);
    assert_eq!(q1.d, false);

    // Phần III có đáp án số: Câu 1: 419, Câu 2: 2.6
    assert!(key.part3_answers.len() >= 2);
    assert_eq!(key.part3_answers.get(&1).map(String::as_str), Some("419"));
    assert_eq!(key.part3_answers.get(&2).map(String::as_str), Some("2.6"));
}

#[test]
fn test_parse_tamdao_full_answers() {
    let p1 = Path::new("../../sample_exams/tam_dao_vat_li.docx");
    let p2 = Path::new("sample_exams/tam_dao_vat_li.docx");
    let mut candidate_paths = vec![p1.to_path_buf(), p2.to_path_buf()];
    if let Ok(up) = std::env::var("USERPROFILE") {
        candidate_paths.push(
            std::path::PathBuf::from(up)
                .join("Downloads")
                .join("thuvienhoclieu.com-De-thi-thu-TN-2027-Vat-Li-THPT-Tam-Dao-II-Phu-Tho.docx"),
        );
    }

    let Some(test_path) = candidate_paths.into_iter().find(|p| p.exists()) else {
        return;
    };

    let keys = parse_answer_key(&test_path).expect("Phải parse được đáp án Tam Đảo");
    assert!(!keys.is_empty(), "Phải có bảng đáp án");
    let key = keys.values().next().unwrap();

    assert_eq!(key.part1_answers.len(), 18, "Phần I phải có đủ 18 đáp án");
    assert_eq!(key.part1_answers.get(&1).map(String::as_str), Some("A"));
    assert_eq!(key.part1_answers.get(&10).map(String::as_str), Some("D"));
    assert_eq!(key.part1_answers.get(&18).map(String::as_str), Some("B"));

    assert_eq!(key.part2_answers.len(), 4, "Phần II phải có đủ 4 câu Đúng/Sai");
    let q1 = key.part2_answers.get(&1).expect("Câu 1 Phần II");
    assert_eq!(q1.a, false);
    assert_eq!(q1.b, true);
    assert_eq!(q1.c, true);
    assert_eq!(q1.d, true);

    let q3 = key.part2_answers.get(&3).expect("Câu 3 Phần II");
    assert_eq!(q3.a, true);
    assert_eq!(q3.b, false);
    assert_eq!(q3.c, true);
    assert_eq!(q3.d, false);

    assert_eq!(key.part3_answers.len(), 6, "Phần III phải có đủ 6 đáp án ngắn");
    assert_eq!(key.part3_answers.get(&1).map(String::as_str), Some("2380"));
    assert_eq!(key.part3_answers.get(&2).map(String::as_str), Some("4"));
    assert_eq!(key.part3_answers.get(&3).map(String::as_str), Some("88.1"));
    assert_eq!(key.part3_answers.get(&6).map(String::as_str), Some("30.3"));
}
