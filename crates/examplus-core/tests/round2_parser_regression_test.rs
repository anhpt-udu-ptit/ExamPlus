use examplus_core::parser::answer_key::parse_answer_key;
use examplus_core::parser::docx::parse_docx;
use std::io::Write;
use tempfile::NamedTempFile;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn build_temp_docx(document_xml: &str) -> NamedTempFile {
    let tmp = NamedTempFile::new().expect("temp docx");
    {
        let mut zip = ZipWriter::new(tmp.reopen().unwrap());
        let opts = SimpleFileOptions::default();

        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        )
        .unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        )
        .unwrap();

        zip.start_file("word/_rels/document.xml.rels", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#,
        )
        .unwrap();

        zip.start_file("word/document.xml", opts).unwrap();
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    tmp
}

#[test]
fn test_stem_experiment_tables_do_not_trigger_answer_key_cutoff() {
    // Reproduces Ninh Binh (Nhiệt độ/Hiệu suất), Bình Minh (t phút / 0,10,20), Sơn Động (Lần chuẩn độ / 1,2,3)
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>PHẦN I. TRẮC NGHIỆM</w:t></w:r></w:p>
    <w:p><w:r><w:t>Câu 1: Đánh giá hiệu suất theo bảng sau:</w:t></w:r></w:p>
    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>Nhiệt độ (°C)</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Hiệu suất (%)</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>60</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>64,35</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>65</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>68,65</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
    <w:p><w:r><w:t>Nhận định đúng là</w:t></w:r></w:p>
    <w:p><w:r><w:t>A. Đúng   B. Sai   C. Tăng   D. Giảm</w:t></w:r></w:p>

    <w:p><w:r><w:t>PHẦN II. TRẮC NGHIỆM ĐÚNG SAI</w:t></w:r></w:p>
    <w:p><w:r><w:t>Câu 1: Kết quả chuẩn độ theo thời gian:</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>t (phút)</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>0</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>20</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>V(NaOH) (mL)</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>2,00</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>6,20</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>9,70</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:p><w:r><w:t>a) Phản ứng xảy ra.   b) Tốc độ giảm dần.   c) Tại 10 phút hết chất.   d) Hiệu suất tăng.</w:t></w:r></w:p>

    <w:p><w:r><w:t>PHẦN III. TRẢ LỜI NGẮN</w:t></w:r></w:p>
    <w:p><w:r><w:t>Câu 1: Thể tích dung dịch sau 3 lần chuẩn độ:</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Lần chuẩn độ</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>V (mL)</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>27,0</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>27,2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>27,1</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:p><w:r><w:t>Câu 2: Tính hàm lượng phần trăm.</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let tmp = build_temp_docx(doc_xml);
    let parsed = parse_docx(tmp.path()).expect("parse docx");
    assert_eq!(parsed.part1_questions.len(), 1);
    assert_eq!(parsed.part1_questions[0].options.len(), 4);
    assert_eq!(parsed.part2_questions.len(), 1);
    assert_eq!(parsed.part2_questions[0].options.len(), 4);
    assert_eq!(parsed.part3_questions.len(), 2);
}

#[test]
fn test_answer_key_ignores_trailing_math_solution_number_tables() {
    // Reproduces Nguyễn Viết Xuân: bảng P3 chính xác theo sau bởi bảng biến thiên / bảng thống kê lời giải
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>ĐÁP ÁN PHẦN III</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Câu</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Kết quả</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Lưu ý</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>0,5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>mét</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>0,84</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>xác suất</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>125</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>mét</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
    <w:p><w:r><w:t>LỜI GIẢI CHI TIẾT</w:t></w:r></w:p>
    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>-3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>f(x)</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>-16</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>0</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>Chu kỳ</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Nhóm không hợp lệ</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Tổng nhóm</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>84</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>680</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>70</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2380</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

    let tmp = build_temp_docx(doc_xml);
    let keys = parse_answer_key(tmp.path()).expect("parse answer key");
    let first = keys.values().next().expect("default key");
    assert_eq!(first.part3_answers.get(&1).map(String::as_str), Some("0.5"));
    assert_eq!(first.part3_answers.get(&2).map(String::as_str), Some("0.84"));
    assert_eq!(first.part3_answers.get(&3).map(String::as_str), Some("125"));
    assert!(
        !first.part3_answers.contains_key(&84),
        "Không được coi bảng thống kê lời giải là đáp án: {:?}",
        first.part3_answers
    );
}

#[test]
fn test_ma_de_1210_q2_symbols_and_proportional_formula_sizing() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let m1210 = manifest_dir
        .join("..")
        .join("..")
        .join("sample_exams")
        .join("ma_de_1210.docx");
    if !m1210.exists() {
        return;
    }
    let parsed = parse_docx(&m1210).expect("parse ma_de_1210.docx");
    let q2 = parsed
        .part1_questions
        .iter()
        .find(|q| q.number == 2)
        .expect("Câu 2 phần I");
    assert_eq!(q2.options.len(), 4);

    let all_q2_html: String = std::iter::once(q2.html_content.as_str())
        .chain(q2.options.iter().map(|o| o.html_content.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    // Công thức VML phải dùng tỷ lệ co giãn theo width: min(100%, ...) + height: auto để không méo khung hẹp
    assert!(
        all_q2_html.contains("width: min(100%, ") && all_q2_html.contains("height: auto;"),
        "Công thức câu 2 phải giữ tỷ lệ co giãn width: min(100%, ...) + height: auto: {}",
        all_q2_html
    );

    // Giải mã SVG data URI (base64 hoặc utf8) trong các đáp án để xác minh π và ℤ
    let decode_base64 = |b64: &str| -> Vec<u8> {
        let mut out = Vec::with_capacity(b64.len() * 3 / 4);
        let mut buf: u32 = 0;
        let mut bits: u8 = 0;
        for b in b64.bytes() {
            let val = match b {
                b'A'..=b'Z' => b - b'A',
                b'a'..=b'z' => b - b'a' + 26,
                b'0'..=b'9' => b - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => break,
                _ => continue,
            };
            buf = (buf << 6) | (val as u32);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((buf >> bits) & 0xFF) as u8);
            }
        }
        out
    };

    let decode_svgs = |html: &str| -> String {
        let mut out = String::new();
        let marker = "data:image/svg+xml;base64,";
        let mut rest = html;
        while let Some(idx) = rest.find(marker) {
            let after = &rest[idx + marker.len()..];
            let end = after.find('"').unwrap_or(after.len());
            let decoded = decode_base64(&after[..end]);
            out.push_str(&String::from_utf8_lossy(&decoded));
            rest = &after[end..];
        }
        out
    };

    let opt_b_svg = decode_svgs(&q2.options[1].html_content);
    assert!(
        opt_b_svg.contains("&#x03C0;") || opt_b_svg.contains('π'),
        "Đáp án B câu 2 phải chứa ký tự π chứ không phải p: {}",
        opt_b_svg
    );
    assert!(
        opt_b_svg.contains("&#x2124;") || opt_b_svg.contains('ℤ'),
        "Đáp án B câu 2 phải chứa ký tự ℤ: {}",
        opt_b_svg
    );
    assert!(
        !opt_b_svg.contains("stroke-dasharray=\"4,4\""),
        "Không được xuất hiện khung nét đứt trống thay công thức"
    );
}
