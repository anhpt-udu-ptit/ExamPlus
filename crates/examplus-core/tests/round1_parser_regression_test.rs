use examplus_core::parser::answer_key::parse_answer_key;
use examplus_core::parser::docx::parse_docx;
use examplus_core::parser::wmf::wmf_to_svg;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn sample_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("sample_exams")
        .join(name)
}

/// Xây dựng một tệp WMF placeable (APM + Header + Records + EOF) tối thiểu trong bộ nhớ.
fn build_synthetic_wmf(width: i16, height: i16, records: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut rec_bytes = Vec::new();
    let mut max_record_words: u32 = 3;
    for (func, params) in records {
        let mut padded = params.clone();
        if padded.len() % 2 != 0 {
            padded.push(0);
        }
        let words = (6 + padded.len()) / 2;
        max_record_words = max_record_words.max(words as u32);
        rec_bytes.extend_from_slice(&(words as u32).to_le_bytes());
        rec_bytes.extend_from_slice(&func.to_le_bytes());
        rec_bytes.extend_from_slice(&padded);
    }
    // META_EOF (size=3 words, func=0)
    rec_bytes.extend_from_slice(&3u32.to_le_bytes());
    rec_bytes.extend_from_slice(&0u16.to_le_bytes());

    let mt_size_words = (18 + rec_bytes.len()) / 2;

    // APM Placeable Header (22 bytes)
    let mut apm = Vec::with_capacity(22);
    apm.extend_from_slice(&0x9AC6CDD7u32.to_le_bytes());
    apm.extend_from_slice(&0u16.to_le_bytes()); // handle
    apm.extend_from_slice(&0i16.to_le_bytes()); // left
    apm.extend_from_slice(&0i16.to_le_bytes()); // top
    apm.extend_from_slice(&width.to_le_bytes());
    apm.extend_from_slice(&height.to_le_bytes());
    apm.extend_from_slice(&1440u16.to_le_bytes()); // inch
    apm.extend_from_slice(&0u32.to_le_bytes()); // reserved
    let mut checksum: u16 = 0;
    for chunk in apm.chunks_exact(2) {
        checksum ^= u16::from_le_bytes([chunk[0], chunk[1]]);
    }
    apm.extend_from_slice(&checksum.to_le_bytes());

    // Standard WMF Header (18 bytes)
    let mut hdr = Vec::with_capacity(18);
    hdr.extend_from_slice(&1u16.to_le_bytes()); // mtType
    hdr.extend_from_slice(&9u16.to_le_bytes()); // mtHeaderSize
    hdr.extend_from_slice(&0x0300u16.to_le_bytes()); // mtVersion
    hdr.extend_from_slice(&(mt_size_words as u32).to_le_bytes());
    hdr.extend_from_slice(&8u16.to_le_bytes()); // mtNoObjects
    hdr.extend_from_slice(&max_record_words.to_le_bytes());
    hdr.extend_from_slice(&0u16.to_le_bytes()); // mtNoParameters

    let mut out = Vec::new();
    out.extend_from_slice(&apm);
    out.extend_from_slice(&hdr);
    out.extend_from_slice(&rec_bytes);
    out
}

fn wmf_create_font(height: i16, charset: u8, italic: u8, face: &str) -> (u16, Vec<u8>) {
    let mut d = vec![0u8; 50];
    d[0..2].copy_from_slice(&height.to_le_bytes());
    d[10] = italic;
    d[13] = charset;
    let fb = face.as_bytes();
    let copy_len = fb.len().min(31);
    d[18..18 + copy_len].copy_from_slice(&fb[..copy_len]);
    (0x02FB, d)
}

fn wmf_ext_text_out(x: i16, y: i16, bytes: &[u8]) -> (u16, Vec<u8>) {
    let mut d = Vec::new();
    d.extend_from_slice(&y.to_le_bytes());
    d.extend_from_slice(&x.to_le_bytes());
    d.extend_from_slice(&(bytes.len() as i16).to_le_bytes());
    d.extend_from_slice(&0u16.to_le_bytes()); // options
    d.extend_from_slice(bytes);
    (0x0A32, d)
}

/// Tạo file DOCX tạm thời trong bộ nhớ từ document.xml, rels, và media nhị phân tùy chọn.
fn build_temp_docx(document_xml: &str, media: &[(&str, &str, &[u8])]) -> NamedTempFile {
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
  <Default Extension="wmf" ContentType="image/x-wmf"/>
  <Default Extension="emf" ContentType="image/x-emf"/>
  <Default Extension="png" ContentType="image/png"/>
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

        let mut rels_xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for (rid, target, bytes) in media {
            rels_xml.push_str(&format!(
                r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{}"/>"#,
                rid, target
            ));
            zip.start_file(format!("word/{}", target.trim_start_matches('/')), opts)
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        rels_xml.push_str("</Relationships>");

        zip.start_file("word/_rels/document.xml.rels", opts).unwrap();
        zip.write_all(rels_xml.as_bytes()).unwrap();

        zip.start_file("word/document.xml", opts).unwrap();
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    tmp
}

#[test]
fn test_wmf_font_offset_18_symbol_reaction_arrow_no_dashed_placeholder() {
    // Reproduces Le Xoay image4.wmf: Symbol font at offset 18 with bytes 0xBE 0xBE 0xAE (⎯⎯→)
    let wmf = build_synthetic_wmf(
        800,
        300,
        &[
            wmf_create_font(-240, 2, 0, "Symbol"),
            (0x012D, 0u16.to_le_bytes().to_vec()),
            wmf_ext_text_out(100, 200, &[0xBE, 0xBE, 0xAE]),
        ],
    );
    let svg = wmf_to_svg(&wmf).expect("wmf svg");
    assert!(
        !svg.contains("stroke-dasharray=\"4,4\""),
        "Không được fallback khung nét đứt khi có ký tự Symbol 0xBE/0xAE: {}",
        svg
    );
    assert!(
        svg.contains("&#x2192;") || svg.contains('→'),
        "Phải giải mã Symbol 0xAE thành mũi tên →: {}",
        svg
    );
}

#[test]
fn test_wmf_font_aware_mt_extra_cp1252_and_gb2312_charset134() {
    // 1. MT Extra: 0x6C=ℓ, 0x50=∥, 0x56=△, 0xA5=ℕ, 0xB7=∠
    let wmf_mte = build_synthetic_wmf(
        1000,
        300,
        &[
            wmf_create_font(-240, 2, 0, "MT Extra"),
            (0x012D, 0u16.to_le_bytes().to_vec()),
            wmf_ext_text_out(50, 200, &[0x6C, 0x50, 0x56, 0xA5, 0xB7]),
        ],
    );
    let svg_mte = wmf_to_svg(&wmf_mte).expect("mt extra svg");
    for expected in ["&#x2113;", "&#x2225;", "&#x25B3;", "&#x2115;", "&#x2220;"] {
        assert!(
            svg_mte.contains(expected),
            "MT Extra thiếu ký hiệu {} trong {}",
            expected,
            svg_mte
        );
    }

    // 2. Times New Roman (CP1252) Vietnamese: 0xE1='á', 0xED='í', 0xB2='²' (NOT ∉ or ∨!)
    let wmf_tnr = build_synthetic_wmf(
        1000,
        300,
        &[
            wmf_create_font(-240, 0, 0, "Times New Roman"),
            (0x012D, 0u16.to_le_bytes().to_vec()),
            wmf_ext_text_out(50, 200, &[0xE1, 0xED, 0xB2]),
        ],
    );
    let svg_tnr = wmf_to_svg(&wmf_tnr).expect("tnr svg");
    assert!(
        !svg_tnr.contains("&#x2209;") && !svg_tnr.contains("&#x2228;"),
        "Times New Roman không được giải mã nhầm chữ Việt 0xE1/0xED thành ∉/∨: {}",
        svg_tnr
    );
    assert!(
        (svg_tnr.contains('á') || svg_tnr.contains("&#225;"))
            && (svg_tnr.contains('²') || svg_tnr.contains("&#178;")),
        "Times New Roman phải giữ nguyên á và ²: {}",
        svg_tnr
    );

    // 3. Malgun Gothic lfCharSet=134 (GB2312) 0xA8 0x4F -> '∣' (&#x2223;)
    let wmf_gbk = build_synthetic_wmf(
        500,
        300,
        &[
            wmf_create_font(-240, 134, 0, "Malgun Gothic"),
            (0x012D, 0u16.to_le_bytes().to_vec()),
            wmf_ext_text_out(50, 200, &[0xA8, 0x4F]),
        ],
    );
    let svg_gbk = wmf_to_svg(&wmf_gbk).expect("gb2312 svg");
    assert!(
        svg_gbk.contains("&#x2223;") || svg_gbk.contains('∣'),
        "GB2312 (cs=134) [0xA8,0x4F] phải ra ký hiệu chia hết/ước ∣: {}",
        svg_gbk
    );
}

#[test]
fn test_wmf_gdi_handle_slot_reuse_and_select_object() {
    // slot 0: Brush, slot 1: Times New Roman, DeleteObject(0), create Symbol (reuses slot 0!), SelectObject(0)
    let wmf = build_synthetic_wmf(
        600,
        300,
        &[
            (0x02FC, vec![0; 8]),                   // slot 0: Brush
            wmf_create_font(-240, 0, 0, "Arial"),   // slot 1: Arial
            (0x01F0, 0u16.to_le_bytes().to_vec()),  // DeleteObject(0)
            wmf_create_font(-240, 2, 0, "Symbol"),  // reuses slot 0!
            (0x012D, 0u16.to_le_bytes().to_vec()),  // SelectObject(0) -> Symbol
            wmf_ext_text_out(50, 200, b"a"),        // 'a' in Symbol -> 'α' (&#x03B1;)
        ],
    );
    let svg = wmf_to_svg(&wmf).expect("gdi reuse svg");
    assert!(
        svg.contains("&#x03B1;") || svg.contains('α'),
        "Tái sử dụng GDI handle slot 0 phải chọn đúng Symbol α thay vì Arial 'a': {}",
        svg
    );
}

#[test]
fn test_real_sample_thuvienhoclieu_emf_and_options_and_1210_proportional_size() {
    // 1. Real file: thuvienhoclieu_sample.docx (EMF + P1#Q1 A.B C. D.)
    let tv_path = sample_path("thuvienhoclieu_sample.docx");
    if tv_path.exists() {
        let parsed = parse_docx(&tv_path).expect("parse thuvienhoclieu_sample");
        let p2_q4 = parsed
            .part2_questions
            .iter()
            .find(|q| q.number == 4)
            .expect("P2 Q4");
        assert!(
            !p2_q4.html_content.contains("application/octet-stream"),
            "Hình pyramid image84.emf không được xuất dạng application/octet-stream"
        );
        assert!(
            p2_q4.html_content.contains("data:image/svg+xml;base64,"),
            "Hình image84.emf phải được chuyển sang SVG data URL"
        );

        let p1_q1 = parsed
            .part1_questions
            .iter()
            .find(|q| q.number == 1)
            .expect("P1 Q1");
        assert_eq!(p1_q1.options.len(), 4);
        for opt in &p1_q1.options {
            assert!(
                opt.html_content.contains("<img "),
                "Phương án {} của P1 Câu 1 (A.B C. D.) không được mất công thức WMF: {:?}",
                opt.label,
                opt.html_content
            );
        }
    }

    // 2. Real file: ma_de_1210.docx (no hardcoded 1.35em squish on tall formulas or lăng trụ)
    let m1210_path = sample_path("ma_de_1210.docx");
    if m1210_path.exists() {
        let parsed = parse_docx(&m1210_path).expect("parse ma_de_1210");
        assert_eq!(parsed.part1_questions.len(), 12);
        assert_eq!(parsed.part2_questions.len(), 4);
        assert_eq!(parsed.part3_questions.len(), 6);
        let all_html: String = parsed
            .part1_questions
            .iter()
            .chain(parsed.part2_questions.iter())
            .chain(parsed.part3_questions.iter())
            .flat_map(|q| {
                std::iter::once(q.html_content.clone())
                    .chain(q.options.iter().map(|o| o.html_content.clone()))
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !all_html.contains("height: 1.35em; max-height: 2.4em;"),
            "Không được hardcode style 'height: 1.35em; max-height: 2.4em;' lên toàn bộ công thức"
        );
    }

    // 3. Real file: Toan11_Made 132_CK1_2025.docx (tab-separated P2#Q2 a) .. b) .. / c) .. d) ..)
    let m132_path = sample_path("Toan11_Made 132_CK1_2025.docx");
    if m132_path.exists() {
        let parsed = parse_docx(&m132_path).expect("parse Toan11_Made 132");
        let p2_q2 = parsed
            .part2_questions
            .iter()
            .find(|q| q.number == 2)
            .expect("P2 Q2");
        assert_eq!(
            p2_q2.options.len(),
            4,
            "P2 Câu 2 dạng a)... b)... trên cùng dòng phải tách đủ 4 ý a,b,c,d"
        );
    }
}

#[test]
fn test_docx_inline_omml_position_shared_context_merged_q_and_proportional_vshape() {
    let wmf = build_synthetic_wmf(
        1000,
        500,
        &[
            wmf_create_font(-240, 0, 0, "Times New Roman"),
            (0x012D, 0u16.to_le_bytes().to_vec()),
            wmf_ext_text_out(50, 200, b"x=1"),
        ],
    );
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:body>
    <w:p><w:r><w:t>PHẦN I. TRẮC NGHIỆM</w:t></w:r></w:p>
    <w:p>
      <w:r><w:t xml:space="preserve">Câu 1: Giá trị của </w:t></w:r>
      <m:oMath><m:f><m:num><m:r><m:t>1</m:t></m:r></m:num><m:den><m:r><m:t>2</m:t></m:r></m:den></m:f></m:oMath>
      <w:r><w:t xml:space="preserve"> cộng 1 bằng bao nhiêu?</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>A. 1</w:t></w:r></w:p>
    <w:p><w:r><w:t>C. 2</w:t></w:r></w:p>
    <w:p><w:r><w:t>B. 3</w:t></w:r></w:p>
    <w:p><w:r><w:t xml:space="preserve">D. </w:t></w:r><w:r><w:object><v:shape style="width:62pt;height:36pt"><v:imagedata r:id="rIdImg1"/></v:shape></w:object></w:r><w:r><w:t>Câu 2: Methyl acrylate là chất có tên là</w:t></w:r></w:p>
    <w:p><w:r><w:t>A. X</w:t></w:r></w:p>
    <w:p><w:r><w:t>B. Y</w:t></w:r></w:p>
    <w:p><w:r><w:t>C. Z</w:t></w:r></w:p>
    <w:p><w:r><w:t>D.Heptane</w:t></w:r></w:p>
    <w:p><w:r><w:t>Sử dụng đoạn thông tin dưới đây để trả lời câu 3 và câu 4: Đoạn văn chung.</w:t></w:r></w:p>
    <w:p><w:r><w:t>Câu 3: Ý thứ nhất?</w:t></w:r></w:p>
    <w:p><w:r><w:t>A. 1   B. 2   C. 3   D. 4</w:t></w:r></w:p>
    <w:p><w:r><w:t>Câu 4: Ý thứ hai?</w:t></w:r></w:p>
    <w:p><w:r><w:t>A. 5   B. 6   C. 7   D. 8</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let tmp = build_temp_docx(doc_xml, &[("rIdImg1", "media/image1.wmf", &wmf)]);
    let parsed = parse_docx(tmp.path()).expect("parse synthetic docx");

    assert_eq!(
        parsed.part1_questions.len(),
        4,
        "Phải có đúng 4 câu (không tạo câu giả từ 'câu 4:' trong ngữ cảnh chung, tách đúng 'Câu 2:' sau 'D.')",
    );

    let q1 = &parsed.part1_questions[0];
    // OMML inline position preserved BEFORE "cộng 1"
    let idx_frac = q1.html_content.find("1").unwrap_or(usize::MAX);
    let idx_cong = q1.html_content.find("cộng 1").unwrap_or(0);
    assert!(
        idx_frac < idx_cong && q1.html_content.contains("2"),
        "OMML phải nằm đúng vị trí giữa 'Giá trị của' và 'cộng 1': {}",
        q1.html_content
    );
    // D option of Q1 has proportional height from height:36pt (~3em, NOT 1.35em!)
    assert_eq!(q1.options.len(), 4);
    assert!(
        !q1.options[3].html_content.contains("1.35em"),
        "Công thức cao 36pt không được ép height: 1.35em: {}",
        q1.options[3].html_content
    );
    assert!(
        !q1.options[3].html_content.contains("Câu 2"),
        "Option D câu 1 không được dính sang Câu 2: {}",
        q1.options[3].html_content
    );

    let q2 = &parsed.part1_questions[1];
    assert_eq!(q2.number, 2);
    assert_eq!(q2.options.len(), 4, "Phải tách được 'D.Heptane' không dấu cách");

    let q3 = &parsed.part1_questions[2];
    let q4 = &parsed.part1_questions[3];
    assert_eq!(q3.number, 3);
    assert_eq!(q4.number, 4);
    assert!(
        q3.html_content.contains("Đoạn văn chung") && q4.html_content.contains("Đoạn văn chung"),
        "Cả Câu 3 và Câu 4 phải nhận được đoạn thông tin dùng chung"
    );
}

#[test]
fn test_answer_key_six_col_horizontal_units_and_wmf_cells() {
    // WMF cell containing "0,71"
    let wmf_071 = build_synthetic_wmf(
        500,
        300,
        &[
            wmf_create_font(-240, 0, 0, "Times New Roman"),
            (0x012D, 0u16.to_le_bytes().to_vec()),
            wmf_ext_text_out(10, 200, b"0,71"),
        ],
    );
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:v="urn:schemas-microsoft-com:vml">
  <w:body>
    <w:p><w:r><w:t>ĐÁP ÁN THAM KHẢO</w:t></w:r></w:p>
    <w:p><w:r><w:t>PHẦN I</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Câu</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Đáp án</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Câu</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Đáp án</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Câu</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Đáp án</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>C</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:p><w:r><w:t>PHẦN III</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Câu 1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Câu 2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Câu 3</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>724 kg</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>3,3 tấn/ngày</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:object><v:shape style="width:20pt;height:14pt"><v:imagedata r:id="rIdAk1"/></v:shape></w:object></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

    let tmp = build_temp_docx(doc_xml, &[("rIdAk1", "media/ak1.wmf", &wmf_071)]);
    let keys = parse_answer_key(tmp.path()).expect("parse answer key");
    let first = keys.values().next().expect("at least one variant key");

    assert_eq!(first.part1_answers.get(&1).map(String::as_str), Some("A"));
    assert_eq!(first.part1_answers.get(&2).map(String::as_str), Some("B"));
    assert_eq!(first.part1_answers.get(&3).map(String::as_str), Some("C"));
    assert_eq!(first.part3_answers.get(&1).map(String::as_str), Some("724"));
    assert_eq!(first.part3_answers.get(&2).map(String::as_str), Some("3.3"));
    assert_eq!(first.part3_answers.get(&3).map(String::as_str), Some("0.71"));
}
