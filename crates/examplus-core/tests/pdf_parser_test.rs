use std::path::Path;

use examplus_core::models::ExamType;
use examplus_core::parser::docx::ParseError;
use examplus_core::parser::pdf::{parse_pdf, parse_pdf_answer_key, parse_pdf_bytes};

#[test]
fn test_parse_real_pdf_toan_12_with_fractions_and_figures() {
    let path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\025850b73fb9_de-thi-giua-ki-1-toan-12-de-so-2-1729579568.pdf",
    );
    let parsed = parse_pdf(path).expect("Phải parse được đề Toán 12 PDF");

    assert_eq!(parsed.exam_type, ExamType::Gdpt2018);
    assert_eq!(parsed.part1_questions.len(), 12, "Phần I phải có 12 câu");
    assert_eq!(parsed.part2_questions.len(), 4, "Phần II phải có 4 câu");
    assert_eq!(parsed.part3_questions.len(), 6, "Phần III phải có 6 câu");

    // Kiểm tra Câu 1 Phần I: có ảnh bảng biến thiên và phương án A..D không bị xáo trộn (-∞;3) thành (;3)-∞
    let q1 = &parsed.part1_questions[0];
    assert!(
        q1.html_content.contains("data:image/"),
        "Câu 1 phải chứa ảnh bảng biến thiên"
    );
    assert_eq!(q1.options.len(), 4);
    let opt_a = &q1.options[0];
    assert!(
        opt_a.raw_content.contains("(-∞") || opt_a.raw_content.contains("−∞") || opt_a.html_content.contains("∞;3)"),
        "Phương án A phải tái dựng đúng (-∞;3) theo trục x, nhận được: raw={:?}, html={:?}",
        opt_a.raw_content,
        opt_a.html_content
    );
    assert!(
        !opt_a.raw_content.contains("(;3)"),
        "Không được để lộn thứ tự thành (;3)-∞: {:?}",
        opt_a.raw_content
    );
    let opt_d = &q1.options[3];
    assert!(
        opt_d.html_content.contains("data:image/") || !opt_d.raw_content.trim().is_empty(),
        "Phương án D câu 1 (ℝ dạng SMask) không được rỗng: raw={:?}, html={:?}",
        opt_d.raw_content,
        opt_d.html_content
    );

    // Kiểm tra Câu 2 Phần I: đồ thị hàm số + 4 phương án phân số đúng thứ tự x+1 / x-1
    let q2 = &parsed.part1_questions[1];
    assert!(q2.html_content.contains("exam-figure"), "Câu 2 phải gắn ảnh đồ thị hàm số");
    assert_eq!(q2.options.len(), 4);
    assert!(
        q2.options.iter().all(|o| o.html_content.contains("<mfrac") || o.raw_content.contains("/")),
        "Cả 4 phương án Câu 2 phải chứa phân số dựng từ nét gạch ngang"
    );
    for o in &q2.options {
        assert!(
            !o.html_content.contains("x1</mi>") && !o.raw_content.contains("x1+") && !o.raw_content.contains("x1-"),
            "Mẫu số/tử số phương án {} không được đảo thứ tự x+1/x-1 thành x1: raw={:?}, html={:?}",
            o.label,
            o.raw_content,
            o.html_content
        );
    }

    // Tuyệt đối không chèn ảnh nền watermark toàn trang (~1768x2500), ký tự PUA hoặc thẻ đóng/mở lẻ ở biên HTML
    for q in parsed
        .part1_questions
        .iter()
        .chain(&parsed.part2_questions)
        .chain(&parsed.part3_questions)
    {
        for o in &q.options {
            let trimmed = o.html_content.trim();
            assert!(
                !trimmed.starts_with("</b>") && !trimmed.starts_with("</i>") && !trimmed.ends_with("<b>"),
                "Phương án {} câu {} không được rò thẻ HTML mồ côi ở biên: {:?}",
                o.label,
                q.number,
                o.html_content
            );
        }
        for pua in '\u{E000}'..='\u{F8FF}' {
            assert!(
                !q.raw_prompt.contains(pua),
                "Không được còn ký tự PUA U+{:04X} chưa giải mã trong câu {}",
                pua as u32,
                q.number
            );
        }
    }
}

#[test]
fn test_parse_real_pdf_hoahoc_strips_solutions_decodes_subscripts_and_extracts_answer_key() {
    let path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\23cbc867ba2e_de-thi-giua-ki-1-hoa-12-ket-noi-tri-thuc-de-so-2-1726200814.pdf",
    );
    let parsed = parse_pdf(path).expect("Phải parse được đề Hóa học 12 PDF");

    assert_eq!(parsed.exam_type, ExamType::Gdpt2018);
    assert_eq!(parsed.part1_questions.len(), 18, "Hóa học Phần I phải có 18 câu");
    assert_eq!(parsed.part2_questions.len(), 4, "Hóa học Phần II phải có 4 câu");
    assert_eq!(parsed.part3_questions.len(), 4, "Hóa học Phần III phải có 4 câu");

    let q1 = &parsed.part1_questions[0];
    assert!(
        q1.html_content.contains("<sub>4</sub>") || q1.html_content.contains("C<sub>4</sub>H<sub>8</sub>O<sub>2</sub>"),
        "Công thức hóa học phải giữ chỉ số dưới <sub>: {}",
        q1.html_content
    );
    assert!(
        !q1.raw_prompt.contains("Phương pháp") && !q1.raw_prompt.contains("Lời giải"),
        "Lời giải không được lọt vào đề thí sinh: {}",
        q1.raw_prompt
    );
    for opt in &q1.options {
        assert!(
            !opt.raw_content.contains("Phương pháp")
                && !opt.raw_content.contains("Lời giải")
                && !opt.raw_content.contains("Đáp án D"),
            "Lời giải/đáp án câu 1 không được lọt vào phương án {}: {}",
            opt.label,
            opt.raw_content
        );
    }

    // Kiểm tra tách đáp án chuẩn từ bảng đầu tài liệu & từng câu
    let keys = parse_pdf_answer_key(path).expect("Phải bóc tách được đáp án Hóa học PDF");
    let k = keys.get("default").expect("Phải có mã đề mặc định default");
    assert_eq!(k.part1_answers.get(&1).map(String::as_str), Some("D"));
    assert_eq!(k.part1_answers.get(&2).map(String::as_str), Some("B"));
    assert_eq!(k.part1_answers.get(&18).map(String::as_str), Some("A"));
    let p2_1 = k.part2_answers.get(&1).expect("Phải có đáp án Phần II câu 1");
    assert!(!p2_1.a && p2_1.b && p2_1.c && p2_1.d);
    let p2_3 = k.part2_answers.get(&3).expect("Phải có đáp án Phần II câu 3");
    assert!(p2_3.a && p2_3.b && !p2_3.c && !p2_3.d);
    assert_eq!(k.part3_answers.get(&1).map(String::as_str), Some("500"));
    assert_eq!(k.part3_answers.get(&3).map(String::as_str), Some("17320"));
}

#[test]
fn test_parse_real_pdf_vatli_and_english_traditional() {
    let physics = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\04f682cbbdd5_de-thi-giua-hoc-ki-1-vat-li-12-ket-noi-tri-thuc-de-so-2-1727798184.pdf",
    );
    let p_exam = parse_pdf(physics).expect("Phải parse được đề Vật Lí 12 PDF");
    assert_eq!(p_exam.exam_type, ExamType::Gdpt2018);
    assert_eq!(p_exam.part1_questions.len(), 18);
    assert_eq!(p_exam.part2_questions.len(), 4);
    assert_eq!(p_exam.part3_questions.len(), 6);

    let p_keys = parse_pdf_answer_key(physics).expect("Phải bóc tách được đáp án Vật Lí PDF");
    let pk = p_keys.get("default").expect("Phải có bảng đáp án Vật Lí");
    assert_eq!(pk.part1_answers.get(&1).map(String::as_str), Some("C"));
    assert_eq!(pk.part1_answers.get(&18).map(String::as_str), Some("A"));

    let english = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\87b4d7b87603_de-thi-giua-ki-1-tieng-anh-12-global-success-de-so-4-1727943077.pdf",
    );
    let e_exam = parse_pdf(english).expect("Phải parse được đề Tiếng Anh PDF");
    assert_eq!(e_exam.exam_type, ExamType::Traditional);
    let total_e = e_exam.part1_questions.len()
        + e_exam.part2_questions.len()
        + e_exam.part3_questions.len();
    assert_eq!(total_e, 40, "Đề Tiếng Anh số 4 phải nhận diện đủ 40 câu");
    let q11 = e_exam
        .part1_questions
        .iter()
        .find(|q| q.number == 11)
        .expect("Phải có câu 11 PART 2 PRONUNCIATION trong part1_questions");
    assert_eq!(q11.options.len(), 4);
}

#[test]
fn test_parse_invalid_and_empty_scanned_pdf_returns_clear_error() {
    let err_bad = parse_pdf_bytes(b"NOT A VALID PDF HEADER").unwrap_err();
    assert!(matches!(err_bad, ParseError::Pdf(_)));

    // Minimal valid PDF with 1 blank page (no text layer, 0 questions)
    let minimal_empty_pdf = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 595 842]>>endobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000056 00000 n \n0000000111 00000 n \ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n180\n%%EOF\n";
    let err_empty = parse_pdf_bytes(minimal_empty_pdf).unwrap_err();
    match err_empty {
        ParseError::Pdf(msg) => {
            assert!(
                msg.contains("văn bản") || msg.contains("câu hỏi") || msg.contains("scan"),
                "Thông báo lỗi phải giải thích rõ nguyên nhân: {}",
                msg
            );
        }
        other => panic!("Kỳ vọng ParseError::Pdf, nhận được {:?}", other),
    }
}

fn check_balanced_inline_html(html: &str) -> bool {
    for tag in ["b", "i", "u", "strong", "em", "sub", "sup", "span", "math"] {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        let mut depth: i32 = 0;
        let mut idx = 0;
        let bytes = html.as_bytes();
        while idx < html.len() {
            if html[idx..].starts_with(&close) {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                idx += close.len();
            } else if html[idx..].starts_with(&open) {
                let next_ch = bytes.get(idx + open.len()).copied();
                if matches!(next_ch, Some(b'>') | Some(b' ') | Some(b'/')) {
                    // Skip self-closing tags like <span ... />
                    if let Some(gt) = html[idx..].find('>') {
                        let is_self_closing = html[idx..idx + gt].trim_end().ends_with('/');
                        if !is_self_closing {
                            depth += 1;
                        }
                        idx += gt + 1;
                        continue;
                    }
                }
                idx += 1;
            } else {
                let ch_len = html[idx..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                idx += ch_len;
            }
        }
        if depth != 0 {
            return false;
        }
    }
    true
}

fn has_unmapped_pua(s: &str) -> bool {
    s.chars().any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c))
}

#[test]
fn test_stress_scanned_skewed_and_mixed_pdf_ocr() {
    let scan_path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\stress_synthetic\stress_scan_200dpi_hoa11.pdf",
    );
    let scan_exam = parse_pdf(scan_path).expect("Phải OCR được PDF ảnh scan 200 DPI");
    assert_eq!(
        scan_exam.part1_questions.len(),
        15,
        "Stress scan 200 DPI phải nhận diện đủ 15 câu Phần I (hiện có {})",
        scan_exam.part1_questions.len()
    );
    assert!(
        scan_exam.provenance_summary.ocr_blocks > 0,
        "PDF scan phải ghi nhận provenance ocr_blocks > 0"
    );
    let scan_keys = parse_pdf_answer_key(scan_path).expect("Phải OCR được bảng đáp án PDF scan");
    let sk = scan_keys.get("default").expect("Phải có đáp án default từ PDF scan");
    assert!(
        sk.part1_answers.len() >= 14,
        "OCR đáp án scan phải tách được >= 14/15 câu (hiện có {})",
        sk.part1_answers.len()
    );

    let skew_path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\stress_synthetic\stress_skewed_scan_hoa11.pdf",
    );
    let skew_exam = parse_pdf(skew_path).expect("Phải OCR được PDF scan bị nghiêng");
    assert_eq!(skew_exam.part1_questions.len(), 15);

    let mixed_path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\stress_synthetic\stress_mixed_scan_text_hoa11.pdf",
    );
    let mixed_exam = parse_pdf(mixed_path).expect("Phải xử lý được PDF hỗn hợp text + ảnh scan");
    assert_eq!(
        mixed_exam.part1_questions.len(),
        15,
        "PDF hỗn hợp không được làm rớt câu trên trang scan"
    );
    assert!(mixed_exam.provenance_summary.native_text_blocks > 0);
    assert!(mixed_exam.provenance_summary.ocr_blocks > 0);
}

#[test]
fn test_stress_rotated_and_twocol_crosspage_pdf() {
    let rot_path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\stress_synthetic\stress_rotated_90_180_270_hoa11.pdf",
    );
    let rot_exam = parse_pdf(rot_path).expect("Phải parse được PDF có các trang /Rotate 90/180/270");
    assert_eq!(rot_exam.part1_questions.len(), 15);
    let rot_keys = parse_pdf_answer_key(rot_path).expect("Phải lấy được đáp án PDF xoay trang");
    let rk = rot_keys.get("default").expect("Phải có default key trên PDF xoay");
    assert_eq!(rk.part1_answers.len(), 15);

    let two_col_path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\stress_synthetic\stress_2col_crosspage_hoa11.pdf",
    );
    let tc_exam = parse_pdf(two_col_path).expect("Phải parse được PDF 2 cột & câu qua trang");
    assert_eq!(
        tc_exam.part1_questions.len(),
        10,
        "PDF 2 cột không được rớt câu (hiện có {})",
        tc_exam.part1_questions.len()
    );
    let tc_keys = parse_pdf_answer_key(two_col_path).expect("Phải lấy được đáp án PDF 2 cột");
    let tk = tc_keys.get("default").expect("Phải có default key trên PDF 2 cột");
    assert_eq!(tk.part1_answers.len(), 10);
}

#[test]
fn test_toanmath_sample_multi_variant_and_clean_text() {
    let p1 = Path::new("sample_exams/toanmath_sample.pdf");
    let p2 = Path::new("../../sample_exams/toanmath_sample.pdf");
    let path = if p1.exists() {
        p1
    } else if p2.exists() {
        p2
    } else {
        return;
    };
    let exam = parse_pdf(path).expect("Phải parse được toanmath_sample.pdf");
    assert_eq!(exam.part1_questions.len(), 12, "toanmath_sample Phần I phải có 12 câu");
    assert_eq!(exam.part2_questions.len(), 2, "toanmath_sample Phần II phải có 2 câu");
    assert_eq!(exam.part3_questions.len(), 4, "toanmath_sample Phần III phải có 4 câu");

    for q in exam
        .part1_questions
        .iter()
        .chain(exam.part2_questions.iter())
        .chain(exam.part3_questions.iter())
    {
        assert!(
            !has_unmapped_pua(&q.raw_prompt) && !has_unmapped_pua(&q.html_content),
            "Câu {} {:?} còn ký tự PUA chưa ánh xạ",
            q.number,
            q.part
        );
        assert!(
            check_balanced_inline_html(&q.html_content),
            "Câu {} {:?} bị lỗi thẻ HTML mất cân bằng: {}",
            q.number,
            q.part,
            q.html_content
        );
    }

    let keys = parse_pdf_answer_key(path).expect("Phải bóc tách được 4 mã đề từ bảng đáp án P7");
    for code in ["132", "209", "357", "485"] {
        let vk = keys
            .get(code)
            .unwrap_or_else(|| panic!("Thiếu đáp án mã đề {}", code));
        assert_eq!(vk.part1_answers.len(), 12, "Mã đề {} thiếu Phần I", code);
        assert_eq!(vk.part2_answers.len(), 2, "Mã đề {} thiếu Phần II", code);
        assert_eq!(vk.part3_answers.len(), 4, "Mã đề {} thiếu Phần III", code);
    }
}

#[test]
fn test_pdf_balanced_html_english_keys_and_hoa_part2_part3() {
    let diali_path = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\09fc6ee91957_de-thi-hoc-ki-1-dia-li-11-kntt-de-so-1-1734077838.pdf",
    );
    let diali = parse_pdf(diali_path).expect("Phải parse được đề Địa Lí 11");
    for q in diali
        .part1_questions
        .iter()
        .chain(diali.part2_questions.iter())
        .chain(diali.part3_questions.iter())
    {
        assert!(
            check_balanced_inline_html(&q.html_content),
            "DiaLi Câu {} còn lỗi thẻ HTML: {}",
            q.number,
            q.html_content
        );
        for o in &q.options {
            assert!(
                check_balanced_inline_html(&o.html_content),
                "DiaLi Câu {} phương án {} lỗi HTML: {}",
                q.number,
                o.label,
                o.html_content
            );
        }
    }

    let hoa_ha_tinh = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\685e71688d12_de-thi-thu-hoa-lan-1-thpt-nam-2025-so-gd-ha-tinh-1737596887.pdf",
    );
    let hoa_keys = parse_pdf_answer_key(hoa_ha_tinh).expect("Phải tách được đáp án Hóa Hà Tĩnh");
    let hk = hoa_keys.get("default").expect("Phải có default key Hóa Hà Tĩnh");
    assert_eq!(hk.part1_answers.len(), 18, "Hóa Hà Tĩnh Phần I = 18");
    assert_eq!(hk.part2_answers.len(), 4, "Hóa Hà Tĩnh Phần II = 4");
    assert_eq!(hk.part3_answers.len(), 6, "Hóa Hà Tĩnh Phần III = 6");

    let en_sol = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\c6c19bda622f_de-thi-giua-ki-1-tieng-anh-11-global-success-de-so-1-1697008939.pdf",
    );
    let en_keys = parse_pdf_answer_key(en_sol).expect("Phải tách được file đáp án Tiếng Anh");
    let ek = en_keys.get("default").expect("Phải có default key Tiếng Anh");
    let total_en_keys = ek.part1_answers.len() + ek.part2_answers.len() + ek.part3_answers.len();
    assert!(
        total_en_keys >= 30,
        "File đáp án Tiếng Anh phải tách được >= 30 đáp án (hiện có {})",
        total_en_keys
    );

    let en_50 = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\7742c261e2f1_de-thi-giua-ki-1-tieng-anh-12-global-success-de-so-2-1727759402.pdf",
    );
    let en_exam = parse_pdf(en_50).expect("Phải parse được đề Tiếng Anh 50 câu");
    let total_q = en_exam.part1_questions.len()
        + en_exam.part2_questions.len()
        + en_exam.part3_questions.len();
    assert_eq!(total_q, 50, "Đề Tiếng Anh 12 số 2 phải đủ 50 câu (hiện có {})", total_q);
}

#[test]
fn test_pdf_provenance_and_crop_fallback_serialization() {
    use examplus_core::parser::answer_key::{AnswerKeyMeta, AnswerSourceKind, ExamAnswerKey};
    use examplus_core::parser::docx::{
        BlockProvenance, BoundingBox, ExtractionMethod, ParsedQuestion,
    };

    let physics = Path::new(
        r"E:\ExamPlus\data\benchmark_corpus\raw\pdf_reference\04f682cbbdd5_de-thi-giua-hoc-ki-1-vat-li-12-ket-noi-tri-thuc-de-so-2-1727798184.pdf",
    );
    let exam = parse_pdf(physics).expect("Phải parse được Vật Lí 12");
    assert_eq!(exam.source_format.as_deref(), Some("pdf"));
    for q in &exam.part1_questions {
        assert!(!q.provenance.is_empty(), "Câu {} thiếu provenance", q.number);
        assert!(q.provenance[0].page >= 1);
        assert!(q.provenance[0].bbox.is_some());
        assert!(q.provenance[0].source_file.is_some());
    }

    let q_crop = ParsedQuestion {
        number: 1,
        needs_review: true,
        review_reasons: vec!["complex_formula_crop_fallback".to_string()],
        provenance: vec![BlockProvenance {
            source_file: Some("demo.pdf".to_string()),
            page: 2,
            bbox: Some(BoundingBox {
                x0: 10.0,
                y0: 20.0,
                x1: 200.0,
                y1: 80.0,
            }),
            method: ExtractionMethod::ImageCropFallback,
            needs_review: true,
            review_reasons: vec!["complex_formula_crop_fallback".to_string()],
        }],
        ..Default::default()
    };
    let json = serde_json::to_string(&q_crop).unwrap();
    let back: ParsedQuestion = serde_json::from_str(&json).unwrap();
    assert!(back.needs_review);
    assert_eq!(back.provenance[0].method, ExtractionMethod::ImageCropFallback);

    let mut ak = ExamAnswerKey::new("101");
    ak.part1_answers.insert(1, "C".to_string());
    ak.part1_meta.insert(
        1,
        AnswerKeyMeta::needs_review("summary_table", "Cần kiểm tra", Some(4)),
    );
    let ak_json = serde_json::to_string(&ak).unwrap();
    let ak_back: ExamAnswerKey = serde_json::from_str(&ak_json).unwrap();
    assert_eq!(
        ak_back.part1_meta.get(&1).map(|m| m.source_kind),
        Some(AnswerSourceKind::NeedsReview)
    );
}

#[test]
fn test_parse_large_user_pdf_33_de_thi_without_cmd_windows() {
    let mut candidate_paths = Vec::new();
    if let Ok(p) = std::env::var("EXAMPLUS_TEST_LARGE_PDF") {
        candidate_paths.push(std::path::PathBuf::from(p));
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        candidate_paths.push(
            std::path::PathBuf::from(user_profile)
                .join("Downloads")
                .join("33_de_thi_toan_12_giua_hoc_ki_1_kem_dap_an_ket_noi_tri_thuc_D5udTWH1MSdu22_093838.pdf"),
        );
    }
    let Some(path) = candidate_paths.into_iter().find(|p| p.exists()) else {
        return;
    };
    let res = parse_pdf(&path);
    assert!(res.is_ok(), "Phải parse được tệp PDF 218 trang mà không bị lỗi: {:?}", res.err());
}
