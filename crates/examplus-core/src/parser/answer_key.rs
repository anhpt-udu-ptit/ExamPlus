use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use super::docx::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrueFalseAnswer {
    pub a: bool,
    pub b: bool,
    pub c: bool,
    pub d: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerSourceKind {
    Extracted,
    Inferred,
    NeedsReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerKeyMeta {
    pub source_kind: AnswerSourceKind,
    #[serde(default)]
    pub source_detail: Option<String>,
    #[serde(default)]
    pub review_reason: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
}

impl AnswerKeyMeta {
    pub fn extracted(detail: &str, page: Option<usize>) -> Self {
        Self {
            source_kind: AnswerSourceKind::Extracted,
            source_detail: Some(detail.to_string()),
            review_reason: None,
            page,
        }
    }

    pub fn inferred(detail: &str, reason: Option<&str>, page: Option<usize>) -> Self {
        Self {
            source_kind: AnswerSourceKind::Inferred,
            source_detail: Some(detail.to_string()),
            review_reason: reason.map(str::to_string),
            page,
        }
    }

    pub fn needs_review(detail: &str, reason: &str, page: Option<usize>) -> Self {
        Self {
            source_kind: AnswerSourceKind::NeedsReview,
            source_detail: Some(detail.to_string()),
            review_reason: Some(reason.to_string()),
            page,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExamAnswerKey {
    pub variant_code: String,
    pub part1_answers: HashMap<u32, String>,
    pub part2_answers: HashMap<u32, TrueFalseAnswer>,
    pub part3_answers: HashMap<u32, String>,
    #[serde(default)]
    pub part1_meta: HashMap<u32, AnswerKeyMeta>,
    #[serde(default)]
    pub part2_meta: HashMap<u32, AnswerKeyMeta>,
    #[serde(default)]
    pub part3_meta: HashMap<u32, AnswerKeyMeta>,
}

impl ExamAnswerKey {
    pub fn new(code: &str) -> Self {
        Self {
            variant_code: code.to_string(),
            part1_answers: HashMap::new(),
            part2_answers: HashMap::new(),
            part3_answers: HashMap::new(),
            part1_meta: HashMap::new(),
            part2_meta: HashMap::new(),
            part3_meta: HashMap::new(),
        }
    }

    /// Điền metadata mặc định `Extracted` cho mọi đáp án đã tách nhưng chưa gắn meta.
    pub fn backfill_extracted_meta(&mut self, detail: &str, page: Option<usize>) {
        for qn in self.part1_answers.keys() {
            self.part1_meta
                .entry(*qn)
                .or_insert_with(|| AnswerKeyMeta::extracted(detail, page));
        }
        for qn in self.part2_answers.keys() {
            self.part2_meta
                .entry(*qn)
                .or_insert_with(|| AnswerKeyMeta::extracted(detail, page));
        }
        for qn in self.part3_answers.keys() {
            self.part3_meta
                .entry(*qn)
                .or_insert_with(|| AnswerKeyMeta::extracted(detail, page));
        }
    }
}

impl Default for ExamAnswerKey {
    fn default() -> Self {
        Self::new("default")
    }
}

pub fn parse_answer_key(path: &Path) -> Result<HashMap<String, ExamAnswerKey>, ParseError> {
    let file = File::open(path)?;
    parse_answer_key_reader(file)
}

pub fn parse_answer_key_bytes(bytes: &[u8]) -> Result<HashMap<String, ExamAnswerKey>, ParseError> {
    parse_answer_key_reader(std::io::Cursor::new(bytes))
}

fn parse_answer_key_reader<R: Read + Seek>(reader: R) -> Result<HashMap<String, ExamAnswerKey>, ParseError> {
    let mut archive = zip::ZipArchive::new(reader)?;

    let xml = {
        let mut doc_entry = archive
            .by_name("word/document.xml")
            .map_err(|_| ParseError::MissingDocumentXml)?;
        let mut s = String::new();
        doc_entry.read_to_string(&mut s)?;
        s
    };

    let mut rel_targets: Vec<(String, String)> = Vec::new();
    if let Ok(mut rels_entry) = archive.by_name("word/_rels/document.xml.rels") {
        let mut rels_xml = String::new();
        if rels_entry.read_to_string(&mut rels_xml).is_ok() {
            let re_rel = Regex::new(r#"<Relationship\b[^>]*>"#).unwrap();
            let re_id = Regex::new(r#"Id="([^"]+)""#).unwrap();
            let re_target = Regex::new(r#"Target="([^"]+)""#).unwrap();
            for m in re_rel.find_iter(&rels_xml) {
                let tag = m.as_str();
                if let (Some(id_cap), Some(t_cap)) = (re_id.captures(tag), re_target.captures(tag)) {
                    let target_raw = t_cap[1].trim();
                    if target_raw.to_ascii_lowercase().ends_with(".wmf") {
                        let norm = if target_raw.starts_with("/word/") {
                            target_raw.trim_start_matches('/').to_string()
                        } else if target_raw.starts_with("word/") {
                            target_raw.to_string()
                        } else {
                            format!("word/{}", target_raw.trim_start_matches("./"))
                        };
                        rel_targets.push((id_cap[1].to_string(), norm));
                    }
                }
            }
        }
    }

    let mut wmf_plain_by_rid: HashMap<String, String> = HashMap::new();
    for (rid, zip_path) in rel_targets {
        if let Ok(mut wmf_entry) = archive.by_name(&zip_path) {
            let mut buf = Vec::new();
            if wmf_entry.read_to_end(&mut buf).is_ok() {
                if let Some(plain) = super::wmf::wmf_to_plain_text(&buf) {
                    wmf_plain_by_rid.insert(rid, plain);
                }
            }
        }
    }

    let tables = extract_tables_with_wmf(&xml, &wmf_plain_by_rid);
    let mut keys: HashMap<String, ExamAnswerKey> = HashMap::new();

    let re_tf_combo = Regex::new(r#"^[ĐDSFđdsf]{4}$"#).unwrap();
    let re_num_val = Regex::new(r#"^-?\d+([,\.]\d+)?$"#).unwrap();

    for tbl in &tables {
        if tbl.is_empty() {
            continue;
        }

        // --- KỊCH BẢN 1: Bảng Ma Trận Đa Mã Đề (Vertical Multi-Variant Table) ---
        let mut is_multi_variant = false;
        let mut variant_cols: Vec<(usize, String)> = Vec::new();

        let re_variant_cell = Regex::new(r#"(?i)Mã\s*đề(?:\s*thi)?\s*[:\.]?\s*(\d{3,4})"#).unwrap();

        // 1. Kiểm tra hàng có chứa trực tiếp "Mã đề 132", "Mã đề 209", ...
        for row in tbl.iter().take(2) {
            let mut detected = Vec::new();
            for (c_idx, cell) in row.iter().enumerate() {
                if let Some(cap) = re_variant_cell.captures(cell) {
                    detected.push((c_idx, cap[1].to_string()));
                }
            }
            if detected.len() >= 2 {
                variant_cols = detected;
                is_multi_variant = true;
                break;
            }
        }

        // 2. Kiểm tra hàng 0 có "Mã đề thi" và hàng 1 là danh sách các số mã đề ["", "132", "209", ...]
        if !is_multi_variant && tbl.len() >= 2 {
            let row0_text = tbl[0].join(" ");
            if row0_text.contains("Mã đề") || row0_text.contains("mã đề") {
                let row1 = &tbl[1];
                let mut detected = Vec::new();
                for (c_idx, cell) in row1.iter().enumerate().skip(1) {
                    let trimmed = cell.trim();
                    if trimmed.len() >= 3
                        && trimmed.len() <= 4
                        && trimmed.chars().all(|c| c.is_ascii_digit())
                    {
                        detected.push((c_idx, trimmed.to_string()));
                    }
                }
                if detected.len() >= 2 {
                    variant_cols = detected;
                    is_multi_variant = true;
                }
            }
        }

        if is_multi_variant {
            // Khởi tạo các mã đề nếu chưa có trong map
            for (_, code) in &variant_cols {
                keys.entry(code.clone())
                    .or_insert_with(|| ExamAnswerKey::new(code));
            }

            // Duyệt các hàng dữ liệu bên dưới
            for row in tbl.iter().skip(1) {
                if row.is_empty() {
                    continue;
                }
                let q_num = match row[0].parse::<u32>() {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                for (col_idx, code) in &variant_cols {
                    if *col_idx >= row.len() {
                        continue;
                    }
                    let cell_val = clean_text(&row[*col_idx]);
                    if cell_val.is_empty() {
                        continue;
                    }

                    // 1. Kiểm tra nếu là 1 chữ cái đơn A, B, C, D (Phần I)
                    if cell_val.len() == 1
                        && "ABCDabcd".contains(&cell_val)
                    {
                        if let Some(key) = keys.get_mut(code) {
                            key.part1_answers
                                .insert(q_num, cell_val.to_uppercase());
                        }
                    }
                    // 2. Kiểm tra nếu là chuỗi 4 ký tự Đ/S (Phần II, vd "ĐSĐĐ")
                    else if re_tf_combo.is_match(&cell_val) {
                        if let Some(tf) = parse_tf_4chars(&cell_val) {
                            if let Some(key) = keys.get_mut(code) {
                                key.part2_answers.insert(q_num, tf);
                            }
                        }
                    }
                    // 3. Kiểm tra nếu là số trả lời ngắn (Phần III, vd "162", "10,4")
                    else if re_num_val.is_match(&cell_val) || is_numeric_answer(&cell_val) {
                        let normalized = cell_val.replace(',', ".");
                        if let Some(key) = keys.get_mut(code) {
                            key.part3_answers.insert(q_num, normalized);
                        }
                    }
                }
            }
            continue;
        }

        // --- KỊCH BẢN 2: Bảng Đúng/Sai dạng hàng câu hỏi (Row-based True/False Table) ---
        // Hàng 0: ["Câu", "a)", "b)", "c)", "d)"]
        // Hàng 1: ["1", "Sai", "Đúng", "Đúng", "Đúng"]
        // Hàng 2: ["2", "Sai", "Đúng", "Đúng", "Sai"]
        if tbl.len() >= 2 && tbl[0].len() >= 5 {
            let c0 = tbl[0][0].to_lowercase();
            let c1 = tbl[0][1].to_lowercase();
            let c2 = tbl[0][2].to_lowercase();
            let c3 = tbl[0][3].to_lowercase();
            let c4 = tbl[0][4].to_lowercase();

            let is_tf_by_row = (c0.contains("câu") || c0.contains("tt") || c0 == "stt")
                && (c1.contains('a') || c1 == "a)")
                && (c2.contains('b') || c2 == "b)")
                && (c3.contains('c') || c3 == "c)")
                && (c4.contains('d') || c4 == "d)");

            if is_tf_by_row {
                let default_key = keys
                    .entry("default".to_string())
                    .or_insert_with(|| ExamAnswerKey::new("default"));

                for row in tbl.iter().skip(1) {
                    if row.is_empty() {
                        continue;
                    }
                    if let Some(q_num) = parse_question_number(&row[0]) {
                        let a_val = parse_single_tf_cell(row.get(1));
                        let b_val = parse_single_tf_cell(row.get(2));
                        let c_val = parse_single_tf_cell(row.get(3));
                        let d_val = parse_single_tf_cell(row.get(4));

                        if let (Some(a), Some(b), Some(c), Some(d)) = (a_val, b_val, c_val, d_val) {
                            default_key.part2_answers.insert(
                                q_num,
                                TrueFalseAnswer { a, b, c, d },
                            );
                        }
                    }
                }
                continue;
            }
        }

        // --- KỊCH BẢN 3: Bảng Dọc Đơn/Đa Cặp (2/3/4/6/8 cột: Câu | Đáp án | Câu | Đáp án ...) ---
        if tbl.len() >= 2 && tbl[0].len() >= 2 {
            let c0 = tbl[0][0].to_lowercase();
            let c1 = tbl[0][1].to_lowercase();

            let is_vertical_key = (c0.contains("câu") || c0.contains("tt") || c0 == "stt")
                && (c1.contains("kết quả") || c1.contains("đáp án") || c1.contains("chọn") || c1.contains("trả lời"));

            if is_vertical_key {
                let is_multi_pair = tbl[0].len() >= 4
                    && tbl[0].len() % 2 == 0
                    && tbl[0][2].to_lowercase().contains("câu");

                let default_key = keys
                    .entry("default".to_string())
                    .or_insert_with(|| ExamAnswerKey::new("default"));

                for row in tbl.iter().skip(1) {
                    if row.len() < 2 {
                        continue;
                    }
                    if is_multi_pair {
                        for ci in (0..row.len().saturating_sub(1)).step_by(2) {
                            if let Some(q_num) = parse_question_number(&row[ci]) {
                                assign_answer_cell(default_key, q_num, &row[ci + 1], &re_tf_combo);
                            }
                        }
                    } else if let Some(q_num) = parse_question_number(&row[0]) {
                        assign_answer_cell(default_key, q_num, &row[1], &re_tf_combo);
                    }
                }
                continue;
            }
        }

        // --- KỊCH BẢN 4: Bảng Ngang Đa Hàng (bao gồm dạng "Câu 1 | Câu 2 | ... | Câu 6" ở cột 0) ---
        let mut r = 0;
        let mut matched_horizontal = false;
        while r + 1 < tbl.len() {
            let header_cell = tbl[r][0].to_lowercase();
            let explicit_cau_cells = tbl[r]
                .iter()
                .filter(|c| c.to_lowercase().contains("câu") && parse_question_number(c).is_some())
                .count();
            if header_cell.contains("câu") || header_cell.contains("stt") || header_cell == "tt" || explicit_cau_cells >= 2 {
                matched_horizontal = true;
                let row_q = &tbl[r];
                let row_ans = &tbl[r + 1];

                let default_key = keys
                    .entry("default".to_string())
                    .or_insert_with(|| ExamAnswerKey::new("default"));

                for (col_idx, q_str) in row_q.iter().enumerate() {
                    if let Some(q_num) = parse_question_number(q_str) {
                        if col_idx < row_ans.len() {
                            assign_answer_cell(default_key, q_num, &row_ans[col_idx], &re_tf_combo);
                        }
                    }
                }
                r += 2;
            } else {
                r += 1;
            }
        }
        if matched_horizontal {
            continue;
        }

        // --- KỊCH BẢN 5: Bảng Đúng/Sai 4 hàng tương ứng 4 ý a), b), c), d) dạng cột ---
        // Hàng 1: ["a) S", "a) Đ", "a) S", "a) Đ"]
        // Hàng 2: ["b) Đ", "b) Đ", "b) S", "b) S"]
        // Hàng 3: ["c) Đ", "c) S", "c) S", "c) S"]
        // Hàng 4: ["d) S", "d) Đ", "d) Đ", "d) Đ"]
        if tbl.len() >= 5 {
            let is_tf_matrix = tbl[1..5].iter().all(|r| {
                !r.is_empty() && r.iter().any(|c| c.contains('Đ') || c.contains('S') || c.contains('đ') || c.contains('s'))
            });

            if is_tf_matrix {
                let default_key = keys
                    .entry("default".to_string())
                    .or_insert_with(|| ExamAnswerKey::new("default"));

                let num_cols = tbl[1].len();
                for c_idx in 0..num_cols {
                    let q_num = (c_idx + 1) as u32;

                    let a_val = parse_single_tf_cell(tbl.get(1).and_then(|r| r.get(c_idx)));
                    let b_val = parse_single_tf_cell(tbl.get(2).and_then(|r| r.get(c_idx)));
                    let c_val = parse_single_tf_cell(tbl.get(3).and_then(|r| r.get(c_idx)));
                    let d_val = parse_single_tf_cell(tbl.get(4).and_then(|r| r.get(c_idx)));

                    if let (Some(a), Some(b), Some(c), Some(d)) = (a_val, b_val, c_val, d_val) {
                        default_key.part2_answers.insert(
                            q_num,
                            TrueFalseAnswer { a, b, c, d },
                        );
                    }
                }
            }
        }
    }

    for k in keys.values_mut() {
        k.backfill_extracted_meta("docx_answer_table", None);
    }

    Ok(keys)
}

pub fn extract_tables(xml: &str) -> Vec<Vec<Vec<String>>> {
    extract_tables_with_wmf(xml, &HashMap::new())
}

fn extract_tables_with_wmf(
    xml: &str,
    wmf_plain_by_rid: &HashMap<String, String>,
) -> Vec<Vec<Vec<String>>> {
    let tbl_regex = Regex::new(r#"(?s)<w:tbl(?:\s|>).*?</w:tbl>"#).unwrap();
    let tr_regex = Regex::new(r#"(?s)<w:tr(?:\s|>).*?</w:tr>"#).unwrap();
    let tc_regex = Regex::new(r#"(?s)<w:tc(?:\s|>).*?</w:tc>"#).unwrap();
    let t_regex = Regex::new(r#"<w:t(?:\s+[^>]*)?>([\s\S]*?)</w:t>"#).unwrap();
    let rid_regex = Regex::new(r#"r:(?:id|embed)="([^"]+)""#).unwrap();

    let mut tables = Vec::new();

    for tbl_cap in tbl_regex.find_iter(xml) {
        let tbl_xml = tbl_cap.as_str();
        let mut rows = Vec::new();

        for tr_cap in tr_regex.find_iter(tbl_xml) {
            let tr_xml = tr_cap.as_str();
            let mut cells = Vec::new();

            for tc_cap in tc_regex.find_iter(tr_xml) {
                let tc_xml = tc_cap.as_str();
                let mut cell_text = String::new();

                for t_cap in t_regex.captures_iter(tc_xml) {
                    cell_text.push_str(&t_cap[1]);
                }

                let mut cleaned = clean_text(&cell_text);
                if cleaned.is_empty() && !wmf_plain_by_rid.is_empty() {
                    let mut wmf_parts = Vec::new();
                    for r_cap in rid_regex.captures_iter(tc_xml) {
                        if let Some(txt) = wmf_plain_by_rid.get(&r_cap[1]) {
                            wmf_parts.push(txt.as_str());
                        }
                    }
                    if !wmf_parts.is_empty() {
                        cleaned = clean_text(&wmf_parts.join(" "));
                    }
                }

                cells.push(cleaned);
            }

            if !cells.is_empty() {
                rows.push(cells);
            }
        }

        if !rows.is_empty() {
            tables.push(rows);
        }
    }

    tables
}

pub(crate) fn clean_text(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .trim()
        .to_string()
}

pub(crate) fn parse_tf_4chars(s: &str) -> Option<TrueFalseAnswer> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 4 {
        return None;
    }

    let is_true = |c: char| c == 'Đ' || c == 'đ' || c == 'T' || c == 't' || c == '1';

    Some(TrueFalseAnswer {
        a: is_true(chars[0]),
        b: is_true(chars[1]),
        c: is_true(chars[2]),
        d: is_true(chars[3]),
    })
}

pub(crate) fn parse_single_tf_cell(opt_cell: Option<&String>) -> Option<bool> {
    let raw = opt_cell?.trim().to_lowercase();
    if raw == "đ" || raw == "đúng" || raw == "d" || raw == "true" || raw == "t" || raw == "1" {
        Some(true)
    } else if raw == "s" || raw == "sai" || raw == "false" || raw == "f" || raw == "0" {
        Some(false)
    } else if raw.starts_with('đ') || raw.starts_with("dung") {
        Some(true)
    } else if raw.starts_with('s') || raw.starts_with("sai") {
        Some(false)
    } else if raw.contains('đ') {
        Some(true)
    } else if raw.contains('s') {
        Some(false)
    } else {
        None
    }
}

pub(crate) fn parse_question_number(s: &str) -> Option<u32> {
    let re = Regex::new(r#"(?i)^(?:c[âa]u|b[àa]i|stt|c)?\s*(\d{1,3})[\.:\)]?$"#).ok()?;
    let caps = re.captures(s.trim())?;
    let n = caps.get(1)?.as_str().parse::<u32>().ok()?;
    if (1..=60).contains(&n) {
        Some(n)
    } else {
        None
    }
}

pub(crate) fn assign_answer_cell(
    key: &mut ExamAnswerKey,
    q_num: u32,
    raw_ans: &str,
    re_tf_combo: &Regex,
) {
    let ans = clean_text(raw_ans);
    if ans.is_empty() || ans.chars().count() > 40 {
        return;
    }
    let no_space = ans.replace(' ', "");
    if no_space.len() == 1 && "ABCDabcd".contains(&no_space) {
        key.part1_answers.insert(q_num, no_space.to_uppercase());
    } else if re_tf_combo.is_match(&no_space) {
        if let Some(tf) = parse_tf_4chars(&no_space) {
            key.part2_answers.insert(q_num, tf);
        }
    } else if let Some(num) = extract_number(&ans) {
        key.part3_answers.insert(q_num, num);
    }
}

pub(crate) fn extract_number(s: &str) -> Option<String> {
    let cleaned = s.trim().replace(',', ".").replace("- ", "-").replace('\u{2212}', "-");
    let re = Regex::new(r#"^-?\d+(\.\d+)?"#).ok()?;
    re.find(&cleaned).map(|m| m.as_str().to_string())
}

fn is_numeric_answer(s: &str) -> bool {
    let cleaned = s.replace(',', ".");
    cleaned.parse::<f64>().is_ok()
}
