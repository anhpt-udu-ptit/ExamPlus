use base64::Engine;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{content::Content, Document, Object, ObjectId, Stream};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::answer_key::{
    assign_answer_cell, clean_text, extract_number, parse_question_number, parse_single_tf_cell,
    ExamAnswerKey, TrueFalseAnswer,
};
use super::docx::{
    assemble_exam, attach_source_file, clean_edge_html, html_escape, render_drawing_figure_img,
    BlockProvenance, BoundingBox, DocBlock, ExtractionMethod, ParagraphData, ParseError,
    ParsedExam, TokenView,
};

const MAX_PDF_PAGES: usize = 120;
const MAX_FORM_RECURSION: usize = 8;
static TEMP_PDF_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Phân tích file `.pdf` thành cấu trúc đề thi chuẩn GDPT 2018 hoặc truyền thống.
pub fn parse_pdf(path: &Path) -> Result<ParsedExam, ParseError> {
    let bytes = std::fs::read(path)?;
    let mut exam = parse_pdf_bytes(&bytes)?;
    attach_source_file(&mut exam, path);
    Ok(exam)
}

/// Phân tích trực tiếp bytes của tệp `.pdf` thành đề thi có cấu trúc.
pub fn parse_pdf_bytes(bytes: &[u8]) -> Result<ParsedExam, ParseError> {
    let extracted = extract_pdf_document(bytes)?;
    let (exam_blocks, _answers, total_visible_chars) = split_pdf_document(&extracted);

    if total_visible_chars < 30 {
        return Err(ParseError::Pdf(
            "Tệp PDF chưa có lớp văn bản (có thể là PDF ảnh scan) hoặc không chứa nội dung chữ hợp lệ."
                .to_string(),
        ));
    }

    let mut exam = assemble_exam(exam_blocks, extracted.media_files)?;
    exam.source_format = Some("pdf".to_string());
    let total_q =
        exam.part1_questions.len() + exam.part2_questions.len() + exam.part3_questions.len();
    if total_q == 0 {
        return Err(ParseError::Pdf(
            "Không nhận diện được câu hỏi nào trong tệp PDF. Nếu đây là tệp PDF dạng ảnh quét (scan) hoặc thiếu ký tự văn bản, vui lòng kiểm tra lại bản gốc."
                .to_string(),
        ));
    }
    Ok(exam)
}

/// Bóc tách đáp án tự động từ file `.pdf` (bảng đáp án đầu/cuối hoặc đáp án sau mỗi câu).
pub fn parse_pdf_answer_key(path: &Path) -> Result<HashMap<String, ExamAnswerKey>, ParseError> {
    let bytes = std::fs::read(path)?;
    parse_pdf_answer_key_bytes(&bytes)
}

/// Bóc tách đáp án tự động từ bytes của tệp `.pdf`.
pub fn parse_pdf_answer_key_bytes(bytes: &[u8]) -> Result<HashMap<String, ExamAnswerKey>, ParseError> {
    let extracted = extract_pdf_document(bytes)?;
    let (_blocks, answers, total_visible_chars) = split_pdf_document(&extracted);
    if total_visible_chars < 20 {
        return Err(ParseError::Pdf(
            "Tệp PDF chưa có lớp văn bản hoặc không chứa bảng đáp án hợp lệ.".to_string(),
        ));
    }
    Ok(answers)
}

// ============================================================================
// 1. Trích xuất hình học trang PDF (Text spans, Paths, Images)
// ============================================================================

#[derive(Debug, Clone)]
struct PdfSpan {
    text: String,
    x0: f64,
    x1: f64,
    y_base: f64, // Top-down visual coordinate after /CropBox & /Rotate
    font_size: f64,
    is_bold: bool,
    is_italic: bool,
    has_unmapped_pua: bool,
    ocr_conf: Option<f64>,
}

#[derive(Debug, Clone)]
struct PdfHLine {
    x0: f64,
    x1: f64,
    y: f64,
}

#[derive(Debug, Clone)]
struct PdfVLine {
    x: f64,
    y0: f64,
    y1: f64,
}

#[derive(Debug, Clone)]
struct PdfCroppedRegion {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    is_inline: bool,
    needs_review: bool,
    review_reason: Option<String>,
    method: ExtractionMethod,
    data_uri: String,
}

#[derive(Debug, Clone)]
struct PdfPlacedImage {
    obj_id: Option<ObjectId>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    px_w: u32,
    px_h: u32,
    is_swatch_smask: bool,
    in_text_form: bool,
    data_uri: String,
}

#[derive(Clone)]
struct DecodedPdfImage {
    px_w: u32,
    px_h: u32,
    uv_rect: [f64; 4],
    is_swatch_smask: bool,
    data_uri: String,
}

#[derive(Debug, Clone)]
struct PdfPageData {
    page_num_1based: usize,
    width: f64,
    height: f64,
    extra_cw_rot: i64,
    spans: Vec<PdfSpan>,
    h_lines: Vec<PdfHLine>,
    v_lines: Vec<PdfVLine>,
    vector_boxes: Vec<[f64; 4]>,
    images: Vec<PdfPlacedImage>,
    cropped_regions: Vec<PdfCroppedRegion>,
}

struct PdfDocExtracted {
    pages: Vec<PdfPageData>,
    media_files: HashMap<String, String>,
}

#[derive(Clone)]
struct FontDecodeTable {
    to_unicode: HashMap<(u8, u32), String>,
    min_code_len: u8,
    max_code_len: u8,
    single_byte: [Option<char>; 256],
    widths: HashMap<u32, f64>,
    default_width: f64,
    is_symbol_font: bool,
    is_pua_custom_font: bool,
    is_cid_mtextra_subset: bool,
    is_cid: bool,
    is_bold: bool,
    is_italic: bool,
}

impl Default for FontDecodeTable {
    fn default() -> Self {
        Self {
            to_unicode: HashMap::new(),
            min_code_len: 1,
            max_code_len: 1,
            single_byte: [None; 256],
            widths: HashMap::new(),
            default_width: 500.0,
            is_symbol_font: false,
            is_pua_custom_font: false,
            is_cid_mtextra_subset: false,
            is_cid: false,
            is_bold: false,
            is_italic: false,
        }
    }
}

impl FontDecodeTable {
    fn decode_next_unit(&self, bytes: &[u8], i: usize) -> (String, f64, usize, bool, bool) {
        if !self.to_unicode.is_empty() {
            for len in (self.min_code_len..=self.max_code_len).rev() {
                let l = len as usize;
                if l > 0 && i + l <= bytes.len() {
                    let mut code = 0u32;
                    for &b in &bytes[i..i + l] {
                        code = (code << 8) | (b as u32);
                    }
                    if let Some(s) = self.to_unicode.get(&(len, code)) {
                        let mut glyph_out = String::new();
                        let mut had_pua = false;
                        for ch in s.chars() {
                            let (mapped, is_pua) = normalize_pdf_char(
                                ch,
                                self.is_symbol_font,
                                self.is_pua_custom_font,
                                self.is_cid_mtextra_subset,
                            );
                            if is_pua {
                                had_pua = true;
                            }
                            if let Some(mc) = mapped {
                                glyph_out.push(mc);
                            }
                        }
                        let w = self.widths.get(&code).copied().unwrap_or_else(|| {
                            if s.chars().count() == 1 && s.chars().next() == Some(' ') {
                                278.0
                            } else {
                                self.default_width
                            }
                        });
                        let is_space = len == 1 && bytes[i] == b' ';
                        return (glyph_out, w, l, is_space, had_pua);
                    }
                }
            }
        }

        if self.is_cid && i + 2 <= bytes.len() {
            let code = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
            let w = self.widths.get(&code).copied().unwrap_or(self.default_width);
            let mut glyph_out = String::new();
            let mut had_pua = false;
            if let Some(ch) = char::from_u32(code) {
                let (mapped, is_pua) = normalize_pdf_char(
                    ch,
                    self.is_symbol_font,
                    self.is_pua_custom_font,
                    self.is_cid_mtextra_subset,
                );
                if is_pua {
                    had_pua = true;
                }
                if let Some(mc) = mapped {
                    if !ch.is_control() {
                        glyph_out.push(mc);
                    }
                }
            }
            (glyph_out, w, 2, false, had_pua)
        } else {
            let b = bytes[i];
            let w = self
                .widths
                .get(&(b as u32))
                .copied()
                .unwrap_or_else(|| if b == b' ' { 278.0 } else { self.default_width });
            let mut glyph_out = String::new();
            let mut had_pua = false;
            if self.is_pua_custom_font && b > 0x20 {
                had_pua = true;
                glyph_out.push('\u{FFFD}');
            } else if let Some(ch) = self.single_byte[b as usize] {
                let (mapped, is_pua) = normalize_pdf_char(
                    ch,
                    self.is_symbol_font,
                    self.is_pua_custom_font,
                    self.is_cid_mtextra_subset,
                );
                if is_pua {
                    had_pua = true;
                }
                if let Some(mc) = mapped {
                    glyph_out.push(mc);
                }
            } else if b >= 0x20 && b < 0x7f {
                glyph_out.push(b as char);
            }
            (glyph_out, w, 1, b == b' ', had_pua)
        }
    }

    fn decode_bytes(&self, bytes: &[u8]) -> (String, f64, bool) {
        let mut out = String::new();
        let mut total_width_1000 = 0.0_f64;
        let mut had_pua = false;

        let mut i = 0usize;
        while i < bytes.len() {
            let (unit_str, w, step, _, unit_pua) = self.decode_next_unit(bytes, i);
            out.push_str(&unit_str);
            total_width_1000 += w;
            if unit_pua {
                had_pua = true;
            }
            i += step.max(1);
        }

        if self.is_symbol_font && out == "[[" {
            out = "[".to_string();
        } else if self.is_symbol_font && out == "]]" {
            out = "]".to_string();
        }

        (out, total_width_1000, had_pua)
    }
}

fn normalize_pdf_char(
    ch: char,
    is_symbol_font: bool,
    is_pua_custom_font: bool,
    is_cid_mtextra_subset: bool,
) -> (Option<char>, bool) {
    let code = ch as u32;
    if code == 0x00A0 {
        return (Some(' '), false);
    }
    if is_cid_mtextra_subset {
        return match code {
            0xF0A1 => (Some('ℝ'), false),
            0xF0A2 => (Some('ℤ'), false),
            0xF0A5 => (Some('ℕ'), false),
            0xF050 => (Some('∥'), false),
            0xF056 => (Some('△'), false),
            0xF072 | 0xF075 => (None, false),
            0x20 => (Some(' '), false),
            0xF020..=0xF0FF => {
                let low = (code & 0xFF) as u8;
                if let Some(sym) = decode_symbol_byte(low) {
                    (Some(sym), false)
                } else if (0x20..=0x7E).contains(&low) {
                    (char::from_u32(low as u32), false)
                } else {
                    (Some('\u{FFFD}'), true)
                }
            }
            0xE000..=0xF8FF => (Some('\u{FFFD}'), true),
            _ => (Some(ch), false),
        };
    }
    if is_pua_custom_font {
        if ch == ' ' || code == 0x20 {
            return (Some(' '), false);
        }
        if (0xF020..=0xF0FF).contains(&code) {
            let low = (code & 0xFF) as u8;
            match code {
                0xF0A1 => return (Some('ℝ'), false),
                0xF0A2 => return (Some('ℤ'), false),
                0xF0A5 => return (Some('ℕ'), false),
                0xF050 => return (Some('∥'), false),
                0xF056 => return (Some('△'), false),
                0xF028 => return (Some('('), false),
                0xF029 => return (Some(')'), false),
                0xF0E9 | 0xF0EB => return (Some('['), false),
                0xF0F9 | 0xF0FB => return (Some(']'), false),
                _ => {}
            }
            if let Some(sym) = decode_symbol_byte(low) {
                return (Some(sym), false);
            }
            if (0x20..=0x7E).contains(&low) {
                return (char::from_u32(low as u32), false);
            }
        }
        return (Some('\u{FFFD}'), true);
    }
    // Các mảnh dấu ngoặc nhọn hệ phương trình xếp dọc nhiều dòng (\uf8f1..\uf8f4) cần giữ ảnh vùng gốc
    if (0xF8F1..=0xF8F4).contains(&code) {
        return (Some('{'), true);
    }
    // Phần trên/giữa/dưới của dấu ngoặc tròn/vuông ghép mảnh từ SymbolMT / Adobe PUA
    if matches!(code, 0xF8EB..=0xF8F0 | 0xF8F6..=0xF8FE) {
        let ch_mapped = match code {
            0xF8EB => Some('('),
            0xF8EE => Some('['),
            0xF8F6 => Some(')'),
            0xF8F9 => Some(']'),
            0xF8FC => Some('}'),
            0xF8EC | 0xF8ED | 0xF8EF | 0xF8F0 | 0xF8F5 | 0xF8F7 | 0xF8F8 | 0xF8FA | 0xF8FB
            | 0xF8FD | 0xF8FE => None,
            _ => None,
        };
        return (ch_mapped, false);
    }
    if code == 0xF8FF {
        return (None, false);
    }
    if (0xE000..=0xF8FF).contains(&code) {
        if is_symbol_font && (0xF020..=0xF0FF).contains(&code) {
            let low = (code & 0xFF) as u8;
            if let Some(sym) = decode_symbol_byte(low) {
                return (Some(sym), false);
            }
            if (0x20..=0x7E).contains(&low) {
                return (char::from_u32(low as u32), false);
            }
        }
        return (Some('\u{FFFD}'), true);
    }
    (Some(ch), false)
}

fn decode_symbol_byte(b: u8) -> Option<char> {
    let ch = match b {
        0x20 => ' ',
        0x21 => '!',
        0x22 => '∀',
        0x23 => '#',
        0x24 => '∃',
        0x25 => '%',
        0x26 => '&',
        0x27 => '∋',
        0x28 => '(',
        0x29 => ')',
        0x2A => '*',
        0x2B => '+',
        0x2C => ',',
        0x2D => '−',
        0x2E => '.',
        0x2F => '/',
        0x30..=0x39 => b as char,
        0x3A => ':',
        0x3B => ';',
        0x3C => '<',
        0x3D => '=',
        0x3E => '>',
        0x3F => '?',
        0x40 => '≅',
        0x41 => 'Α',
        0x42 => 'Β',
        0x43 => 'Χ',
        0x44 => 'Δ',
        0x45 => 'Ε',
        0x46 => 'Φ',
        0x47 => 'Γ',
        0x48 => 'Η',
        0x49 => 'Ι',
        0x4A => 'ϑ',
        0x4B => 'Κ',
        0x4C => 'Λ',
        0x4D => 'Μ',
        0x4E => 'Ν',
        0x4F => 'Ο',
        0x50 => 'Π',
        0x51 => 'Θ',
        0x52 => 'Ρ',
        0x53 => 'Σ',
        0x54 => 'Τ',
        0x55 => 'Υ',
        0x56 => 'ς',
        0x57 => 'Ω',
        0x58 => 'Ξ',
        0x59 => 'Ψ',
        0x5A => 'Ζ',
        0x5B => '[',
        0x5C => '∴',
        0x5D => ']',
        0x5E => '⊥',
        0x5F => '_',
        0x61 => 'α',
        0x62 => 'β',
        0x63 => 'χ',
        0x64 => 'δ',
        0x65 => 'ε',
        0x66 => 'φ',
        0x67 => 'γ',
        0x68 => 'η',
        0x69 => 'ι',
        0x6A => 'ϕ',
        0x6B => 'κ',
        0x6C => 'λ',
        0x6D => 'μ',
        0x6E => 'ν',
        0x6F => 'ο',
        0x70 => 'π',
        0x71 => 'θ',
        0x72 => 'ρ',
        0x73 => 'σ',
        0x74 => 'τ',
        0x75 => 'υ',
        0x76 => 'ϖ',
        0x77 => 'ω',
        0x78 => 'ξ',
        0x79 => 'ψ',
        0x7A => 'ζ',
        0x7B => '{',
        0x7C => '|',
        0x7D => '}',
        0x7E => '~',
        0xA2 => '′',
        0xA3 => '≤',
        0xA4 => '/',
        0xA5 => '∞',
        0xA6 => 'ƒ',
        0xAB => '↔',
        0xAC => '←',
        0xAD => '↑',
        0xAE => '→',
        0xAF => '↓',
        0xB0 => '°',
        0xB1 => '±',
        0xB2 => '″',
        0xB3 => '≥',
        0xB4 => '×',
        0xB5 => '∝',
        0xB6 => '∂',
        0xB7 => '•',
        0xB8 => '÷',
        0xB9 => '≠',
        0xBA => '≡',
        0xBB => '≈',
        0xBC => '…',
        0xBD => '|',
        0xBE => '→',
        0xBF => '↵',
        0xC5 => '⊕',
        0xC6 => '∅',
        0xC7 => '∩',
        0xC8 => '∪',
        0xC9 => '⊃',
        0xCA => '⊇',
        0xCB => '⊄',
        0xCC => '⊂',
        0xCD => '⊆',
        0xCE => '∈',
        0xCF => '∉',
        0xD0 => '∠',
        0xD1 => '∇',
        0xD5 => '∏',
        0xD6 => '√',
        0xD7 => '·',
        0xD8 => '¬',
        0xD9 => '∧',
        0xDA => '∨',
        0xDB => '⇔',
        0xDC => '⇐',
        0xDD => '⇑',
        0xDE => '⇒',
        0xDF => '⇓',
        0xE1 => '⟨',
        0xE5 => '∑',
        0xE6 => '(',
        0xE7 => '|',
        0xE8 => '(',
        0xE9 => '[',
        0xEA => '[',
        0xEB => '[',
        0xEC => '{',
        0xED => '{',
        0xEE => '{',
        0xEF => '|',
        0xF0 => '}',
        0xF1 => '⟩',
        0xF2 => '∫',
        0xF6 => ')',
        0xF7 => '|',
        0xF8 => ')',
        0xF9 => ']',
        0xFA => ']',
        0xFB => ']',
        0xFC => '}',
        0xFD => '}',
        0xFE => '}',
        _ => return None,
    };
    Some(ch)
}

fn extract_pdf_document(bytes: &[u8]) -> Result<PdfDocExtracted, ParseError> {
    if bytes.is_empty()
        || (!bytes.starts_with(b"%PDF-")
            && !bytes
                .get(..1024)
                .is_some_and(|head| head.windows(5).any(|w| w == b"%PDF-")))
    {
        return Err(ParseError::Pdf(
            "Tệp không phải định dạng PDF hợp lệ (thiếu chữ ký %PDF-).".to_string(),
        ));
    }

    let mut doc = Document::load_mem(bytes)
        .map_err(|e| ParseError::Pdf(format!("Không thể giải mã cấu trúc PDF: {}", e)))?;

    if doc.is_encrypted() {
        if doc.decrypt("").is_err() {
            return Err(ParseError::Pdf(
                "Tệp PDF đang được đặt mật khẩu bảo vệ, không thể bóc tách nội dung.".to_string(),
            ));
        }
    }

    let page_map = doc.get_pages();
    if page_map.is_empty() {
        return Err(ParseError::Pdf("Tệp PDF không có trang nào.".to_string()));
    }

    let mut font_cache: HashMap<ObjectId, FontDecodeTable> = HashMap::new();
    let mut image_cache: HashMap<ObjectId, Option<DecodedPdfImage>> = HashMap::new();
    let mut media_files: HashMap<String, String> = HashMap::new();
    let mut pages_raw: Vec<PdfPageData> = Vec::new();

    for (page_num, page_id) in page_map.into_iter().take(MAX_PDF_PAGES) {
        let crop_box = resolve_page_box(&doc, page_id);
        let rotate_deg = resolve_page_rotate(&doc, page_id);
        let raw_w = (crop_box[2] - crop_box[0]).abs().max(100.0);
        let raw_h = (crop_box[3] - crop_box[1]).abs().max(100.0);
        let (page_w, page_h) = if rotate_deg == 90 || rotate_deg == 270 {
            (raw_h, raw_w)
        } else {
            (raw_w, raw_h)
        };
        let content_bytes = doc.get_page_content(page_id);

        let page_res_dicts = collect_page_resource_dicts(&doc, page_id);
        let mut state = ExtractState {
            doc: &doc,
            font_cache: &mut font_cache,
            image_cache: &mut image_cache,
            crop_box,
            raw_w,
            raw_h,
            rotate_deg,
            page_w,
            page_h,
            text_dir_vx: 0.0,
            text_dir_vy: 0.0,
            text_abs_vx: 0.0,
            text_abs_vy: 0.0,
            spans: Vec::new(),
            h_lines: Vec::new(),
            v_lines: Vec::new(),
            vector_boxes: Vec::new(),
            images: Vec::new(),
        };

        state.walk_stream_bytes(&content_bytes, &page_res_dicts, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], None, 0);

        // Nếu hướng chữ thực tế trong visual coords bị xoay 90/180/270° (ví dụ PDF bị gắn /Rotate xoay ngang đề thẳng),
        // tự động xoay bù để khôi phục hướng đọc trái->phải, trên->dưới.
        let extra_cw = if state.text_abs_vy > state.text_abs_vx * 2.0 && state.text_abs_vy > 10.0 {
            if state.text_dir_vy >= 0.0 { 270 } else { 90 }
        } else if state.text_abs_vx > state.text_abs_vy * 2.0
            && state.text_abs_vx > 10.0
            && state.text_dir_vx < 0.0
        {
            180
        } else {
            0
        };

        let (final_page_w, final_page_h, final_state) = if extra_cw != 0 {
            let eff_rot = (rotate_deg + extra_cw) % 360;
            let (ew, eh) = if eff_rot == 90 || eff_rot == 270 {
                (raw_h, raw_w)
            } else {
                (raw_w, raw_h)
            };
            let mut st2 = ExtractState {
                doc: &doc,
                font_cache: state.font_cache,
                image_cache: state.image_cache,
                crop_box,
                raw_w,
                raw_h,
                rotate_deg: eff_rot,
                page_w: ew,
                page_h: eh,
                text_dir_vx: 0.0,
                text_dir_vy: 0.0,
                text_abs_vx: 0.0,
                text_abs_vy: 0.0,
                spans: Vec::new(),
                h_lines: Vec::new(),
                v_lines: Vec::new(),
                vector_boxes: Vec::new(),
                images: Vec::new(),
            };
            st2.walk_stream_bytes(&content_bytes, &page_res_dicts, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], None, 0);
            (ew, eh, st2)
        } else {
            (page_w, page_h, state)
        };

        pages_raw.push(PdfPageData {
            page_num_1based: page_num as usize,
            width: final_page_w,
            height: final_page_h,
            extra_cw_rot: extra_cw,
            spans: final_state.spans,
            h_lines: final_state.h_lines,
            v_lines: final_state.v_lines,
            vector_boxes: final_state.vector_boxes,
            images: final_state.images,
            cropped_regions: Vec::new(),
        });
    }

    // Lọc bỏ ảnh rác, watermark lặp ở lề; giữ lại ảnh toàn trang nếu trang đó chưa có lớp chữ (PDF scan)
    let mut obj_margin_pages: HashMap<ObjectId, HashSet<usize>> = HashMap::new();
    for (p_idx, pg) in pages_raw.iter().enumerate() {
        for img in &pg.images {
            if let Some(oid) = img.obj_id {
                if img.y1 <= 85.0 || img.y0 >= pg.height - 85.0 {
                    obj_margin_pages.entry(oid).or_default().insert(p_idx);
                }
            }
        }
    }

    let mut temp_pdf_guard: Option<TempPdfGuard> = None;
    let mut media_seq = 0usize;

    for pg in &mut pages_raw {
        let pw = pg.width;
        let ph = pg.height;
        let native_char_count: usize = pg
            .spans
            .iter()
            .map(|s| s.text.chars().filter(|c| !c.is_whitespace()).count())
            .sum();

        pg.images.retain(|img| {
            if img.is_swatch_smask && img.in_text_form {
                return false;
            }
            if !img.is_swatch_smask && (img.px_w <= 4 || img.px_h <= 4) {
                return false;
            }
            if img.px_w <= 1 || img.px_h <= 1 {
                return false;
            }
            let dw = (img.x1 - img.x0).abs();
            let dh = (img.y1 - img.y0).abs();
            if !img.is_swatch_smask && dw < 3.0 && dh < 3.0 {
                return false;
            }
            // Nếu trang đã có lớp văn bản rõ ràng (>=40 ký tự) và ảnh lớn phủ nền/chồng chữ -> watermark
            if dw >= pw * 0.62 && dh >= ph * 0.62 && native_char_count >= 40 {
                let text_inside_img = pg.spans.iter().any(|s| {
                    let cx = 0.5 * (s.x0 + s.x1);
                    cx >= img.x0 + 12.0
                        && cx <= img.x1 - 12.0
                        && s.y_base >= img.y0 + 12.0
                        && s.y_base <= img.y1 - 12.0
                });
                if text_inside_img {
                    return false;
                }
            }
            if let Some(oid) = img.obj_id {
                if obj_margin_pages
                    .get(&oid)
                    .is_some_and(|pset| pset.len() >= 2)
                {
                    return false;
                }
            }
            true
        });

        // Kiểm tra nhu cầu OCR offline (trang scan hoàn toàn hoặc vùng ảnh lớn không có text nằm đè trong trang hỗn hợp)
        let has_large_unparsed_img = pg.images.iter().any(|img| {
            let dw = (img.x1 - img.x0).abs();
            let dh = (img.y1 - img.y0).abs();
            dw >= pw * 0.45 && dh >= ph * 0.28
        });

        let needs_full_page_ocr = native_char_count < 40 && (!pg.images.is_empty() || !pg.vector_boxes.is_empty());
        let pua_regions = collect_pua_fallback_regions(pg);
        let vector_regions = if native_char_count >= 25 {
            cluster_vector_figures(pg)
        } else {
            Vec::new()
        };

        let mut cached_ppm: Option<PpmImage> = None;
        let need_raster =
            needs_full_page_ocr || has_large_unparsed_img || !pua_regions.is_empty() || !vector_regions.is_empty();

        if need_raster {
            let dpi = if needs_full_page_ocr || has_large_unparsed_img {
                260u32
            } else {
                200u32
            };
            if temp_pdf_guard.is_none() {
                temp_pdf_guard = TempPdfGuard::new(bytes).ok();
            }
            if let Some(ref guard) = temp_pdf_guard {
                cached_ppm = render_pdf_page_to_ppm(&guard.path, pg.page_num_1based, dpi)
                    .map(|p| if pg.extra_cw_rot != 0 { rotate_ppm_cw(&p, pg.extra_cw_rot) } else { p });
            }
        }

        if let Some(ref ppm) = cached_ppm {
            if needs_full_page_ocr {
                if let Some((ocr_spans, _mean_conf)) = run_tesseract_ocr_on_ppm(ppm, pg.width, pg.height) {
                    if !ocr_spans.is_empty() {
                        pg.spans = ocr_spans;
                        // Ảnh scan toàn trang đã được OCR thành văn bản có tọa độ -> bỏ ảnh nền lớn khỏi danh sách hình minh họa
                        pg.images.retain(|img| {
                            let dw = (img.x1 - img.x0).abs();
                            let dh = (img.y1 - img.y0).abs();
                            !(dw >= pw * 0.55 && dh >= ph * 0.40)
                        });
                    }
                }
            } else if has_large_unparsed_img {
                // Trang hỗn hợp (Mixed scan + text): kiểm tra từng ảnh lớn chưa có text bên trong xem có phải đoạn đề scan
                let mut remove_img_idxs: HashSet<usize> = HashSet::new();
                let mut extra_ocr_spans: Vec<PdfSpan> = Vec::new();
                for (i_idx, img) in pg.images.iter().enumerate() {
                    let dw = (img.x1 - img.x0).abs();
                    let dh = (img.y1 - img.y0).abs();
                    if dw < pw * 0.45 || dh < ph * 0.25 {
                        continue;
                    }
                    let has_native_inside = pg.spans.iter().any(|s| {
                        let cx = 0.5 * (s.x0 + s.x1);
                        cx >= img.x0 + 8.0
                            && cx <= img.x1 - 8.0
                            && s.y_base >= img.y0 + 8.0
                            && s.y_base <= img.y1 - 8.0
                    });
                    if has_native_inside {
                        continue;
                    }
                    if let Some(sub_ppm) = crop_ppm_subimage(ppm, [img.x0, img.y0, img.x1, img.y1], pg.width, pg.height, 2.0) {
                        if let Some((mut ocr_spans, _conf)) = run_tesseract_ocr_on_ppm(&sub_ppm, dw + 4.0, dh + 4.0) {
                            let alpha_chars: usize = ocr_spans
                                .iter()
                                .map(|s| s.text.chars().filter(|c| c.is_alphabetic()).count())
                                .sum();
                            let joined: String = ocr_spans
                                .iter()
                                .map(|s| s.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            let looks_like_text_block = alpha_chars >= 28
                                && (joined.contains("Câu ")
                                    || joined.contains("A.")
                                    || joined.contains("B.")
                                    || ocr_spans.len() >= 6);
                            if looks_like_text_block {
                                for s in &mut ocr_spans {
                                    s.x0 += (img.x0 - 2.0).max(0.0);
                                    s.x1 += (img.x0 - 2.0).max(0.0);
                                    s.y_base += (img.y0 - 2.0).max(0.0);
                                }
                                extra_ocr_spans.extend(ocr_spans);
                                remove_img_idxs.insert(i_idx);
                            }
                        }
                    }
                }
                if !remove_img_idxs.is_empty() {
                    pg.spans.extend(extra_ocr_spans);
                    let mut keep_idx = 0usize;
                    pg.images.retain(|_| {
                        let keep = !remove_img_idxs.contains(&keep_idx);
                        keep_idx += 1;
                        keep
                    });
                }
            }

            // Cắt ảnh độ phân giải cao cho công thức PUA/hệ ngoặc nhiều dòng (`ImageCropFallback`)
            for reg in pua_regions {
                if let Some(png_uri) = crop_ppm_bbox_to_png_uri(ppm, reg.bbox, pg.width, pg.height, 2.5, true) {
                    pg.cropped_regions.push(PdfCroppedRegion {
                        x0: reg.bbox[0],
                        y0: reg.bbox[1],
                        x1: reg.bbox[2],
                        y1: reg.bbox[3],
                        is_inline: reg.is_inline,
                        needs_review: true,
                        review_reason: Some(reg.reason),
                        method: ExtractionMethod::ImageCropFallback,
                        data_uri: png_uri,
                    });
                }
            }

            // Cắt ảnh hình vẽ vector / đồ thị / bảng biến thiên (`PdfVectorCrop`)
            for vbbox in vector_regions {
                if let Some(png_uri) = crop_ppm_bbox_to_png_uri(ppm, vbbox, pg.width, pg.height, 4.0, true) {
                    pg.cropped_regions.push(PdfCroppedRegion {
                        x0: vbbox[0],
                        y0: vbbox[1],
                        x1: vbbox[2],
                        y1: vbbox[3],
                        is_inline: false,
                        needs_review: false,
                        review_reason: None,
                        method: ExtractionMethod::PdfVectorCrop,
                        data_uri: png_uri,
                    });
                }
            }
        }

        for img in &pg.images {
            media_seq += 1;
            let key = format!("pdf_img_{}", media_seq);
            media_files.insert(key, img.data_uri.clone());
        }
        for reg in &pg.cropped_regions {
            media_seq += 1;
            let key = format!("pdf_crop_{}", media_seq);
            media_files.insert(key, reg.data_uri.clone());
        }
    }

    Ok(PdfDocExtracted {
        pages: pages_raw,
        media_files,
    })
}

fn resolve_page_rotate(doc: &Document, page_id: ObjectId) -> i64 {
    let mut cur = Some(page_id);
    let mut seen = HashSet::new();
    while let Some(id) = cur {
        if !seen.insert(id) {
            break;
        }
        if let Ok(dict) = doc.get_dictionary(id) {
            if let Ok(obj) = dict.get(b"Rotate") {
                if let Ok(deg) = obj.as_i64() {
                    let norm = ((deg % 360) + 360) % 360;
                    if matches!(norm, 0 | 90 | 180 | 270) {
                        return norm;
                    }
                }
            }
            cur = dict.get(b"Parent").and_then(Object::as_reference).ok();
        } else {
            break;
        }
    }
    0
}

struct TempPdfGuard {
    path: PathBuf,
}

impl TempPdfGuard {
    fn new(bytes: &[u8]) -> std::io::Result<Self> {
        let seq = TEMP_PDF_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("examplus_pdf_{}_{}.pdf", std::process::id(), seq));
        std::fs::write(&path, bytes)?;
        Ok(Self { path })
    }
}

impl Drop for TempPdfGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct PpmImage {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
}

static PDFTOPPM_BIN_CACHE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
static TESSERACT_BIN_CACHE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Tạo `std::process::Command` được ẩn hoàn toàn 100% cửa sổ console (CREATE_NO_WINDOW) trên Windows.
/// Đảm bảo tuyệt đối không có cửa sổ CMD hay PowerShell nào nhấp nháy trên màn hình người dùng.
pub(crate) fn hidden_command<P: AsRef<std::ffi::OsStr>>(program: P) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn find_external_tool(env_key: &str, exe_names: &[&str], fallback_paths: &[&str]) -> Option<PathBuf> {
    if let Some(val) = std::env::var_os(env_key) {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            for name in exe_names {
                let p = dir.join(name);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    // Tra cứu biến môi trường PATH bằng Rust thuần túy (tuyệt đối không gọi where.exe/which gây nhấp nháy CMD)
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in exe_names {
                let p = dir.join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    for fb in fallback_paths {
        let p = PathBuf::from(fb);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn find_pdftoppm_bin() -> Option<PathBuf> {
    PDFTOPPM_BIN_CACHE
        .get_or_init(|| {
            let mut candidates = vec![
                "tools/poppler/bin/pdftoppm.exe".to_string(),
                "tools/poppler/pdftoppm.exe".to_string(),
            ];
            if let Ok(up) = std::env::var("USERPROFILE") {
                let norm = up.replace('\\', "/");
                candidates.push(format!("{}/.gemini/config/toolchains/skills-runtime/native/poppler/bin/pdftoppm.exe", norm));
            }
            let candidate_refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
            find_external_tool(
                "EXAMPLUS_PDFTOPPM_PATH",
                &["pdftoppm.exe", "pdftoppm"],
                &candidate_refs,
            )
        })
        .clone()
}

fn find_tesseract_bin() -> Option<PathBuf> {
    TESSERACT_BIN_CACHE
        .get_or_init(|| {
            let mut candidates = vec![
                "C:/Program Files/Tesseract-OCR/tesseract.exe".to_string(),
                "tools/tesseract/tesseract.exe".to_string(),
            ];
            if let Ok(up) = std::env::var("USERPROFILE") {
                let norm = up.replace('\\', "/");
                candidates.push(format!("{}/.gemini/config/toolchains/skills-runtime/native/tesseract/tesseract.exe", norm));
            }
            let candidate_refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
            find_external_tool(
                "EXAMPLUS_TESSERACT_PATH",
                &["tesseract.exe", "tesseract"],
                &candidate_refs,
            )
        })
        .clone()
}

fn find_tessdata_prefix(tess_bin: &Path) -> Option<PathBuf> {
    if let Some(val) = std::env::var_os("TESSDATA_PREFIX") {
        let p = PathBuf::from(val);
        if p.join("vie.traineddata").exists() {
            return Some(p);
        }
    }
    if let Ok(up) = std::env::var("USERPROFILE") {
        let norm = up.replace('\\', "/");
        let known = PathBuf::from(format!("{}/.gemini/config/toolchains/skills-runtime/native/tessdata", norm));
        if known.join("vie.traineddata").exists() {
            return Some(known);
        }
    }
    if let Some(parent) = tess_bin.parent() {
        let cand = parent.join("tessdata");
        if cand.join("vie.traineddata").exists() {
            return Some(cand);
        }
    }
    None
}

fn parse_ppm_p6(bytes: &[u8]) -> Option<PpmImage> {
    if !bytes.starts_with(b"P6") {
        return None;
    }
    let mut i = 2usize;
    let mut nums = [0u32; 3];
    let mut n_idx = 0usize;
    while n_idx < 3 && i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if start == i {
            return None;
        }
        nums[n_idx] = std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?;
        n_idx += 1;
    }
    if n_idx < 3 || nums[2] != 255 || nums[0] == 0 || nums[1] == 0 || i >= bytes.len() {
        return None;
    }
    // Theo đặc tả PPM P6, ngay sau maxval là đúng 1 ký tự whitespace trước dữ liệu pixel
    i += 1;
    let need = (nums[0] as usize) * (nums[1] as usize) * 3;
    if bytes.len() < i + need {
        return None;
    }
    Some(PpmImage {
        w: nums[0],
        h: nums[1],
        rgb: bytes[i..i + need].to_vec(),
    })
}

fn render_pdf_page_to_ppm(pdf_path: &Path, page_num_1based: usize, dpi: u32) -> Option<PpmImage> {
    let bin = find_pdftoppm_bin()?;
    let p_str = page_num_1based.to_string();
    let dpi_str = dpi.to_string();
    let out = hidden_command(bin)
        .args([
            "-r",
            &dpi_str,
            "-f",
            &p_str,
            "-l",
            &p_str,
            "-singlefile",
            &pdf_path.to_string_lossy(),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_ppm_p6(&out.stdout)
}

fn crop_ppm_subimage(
    ppm: &PpmImage,
    bbox: [f64; 4],
    page_w: f64,
    page_h: f64,
    pad_pt: f64,
) -> Option<PpmImage> {
    if page_w <= 1.0 || page_h <= 1.0 || ppm.w == 0 || ppm.h == 0 {
        return None;
    }
    let sx = (ppm.w as f64) / page_w;
    let sy = (ppm.h as f64) / page_h;
    let px0 = (((bbox[0] - pad_pt) * sx).floor().max(0.0) as u32).min(ppm.w.saturating_sub(1));
    let py0 = (((bbox[1] - pad_pt) * sy).floor().max(0.0) as u32).min(ppm.h.saturating_sub(1));
    let px1 = (((bbox[2] + pad_pt) * sx).ceil().max((px0 + 1) as f64) as u32).min(ppm.w);
    let py1 = (((bbox[3] + pad_pt) * sy).ceil().max((py0 + 1) as f64) as u32).min(ppm.h);
    let cw = px1.saturating_sub(px0);
    let ch = py1.saturating_sub(py0);
    if cw < 4 || ch < 4 {
        return None;
    }
    let mut rgb = Vec::with_capacity((cw * ch * 3) as usize);
    let src_stride = (ppm.w * 3) as usize;
    let row_bytes = (cw * 3) as usize;
    for y in py0..py1 {
        let off = (y as usize) * src_stride + (px0 as usize) * 3;
        rgb.extend_from_slice(&ppm.rgb[off..off + row_bytes]);
    }
    Some(PpmImage { w: cw, h: ch, rgb })
}

fn crop_ppm_bbox_to_png_uri(
    ppm: &PpmImage,
    bbox: [f64; 4],
    page_w: f64,
    page_h: f64,
    pad_pt: f64,
    trim_white: bool,
) -> Option<String> {
    let sub = crop_ppm_subimage(ppm, bbox, page_w, page_h, pad_pt)?;
    let (final_w, final_h, final_rgb) = if trim_white {
        trim_ppm_white_margins(&sub, 4)
    } else {
        (sub.w, sub.h, sub.rgb)
    };
    let png = encode_png(final_w, final_h, false, &final_rgb)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{}", b64))
}

fn trim_ppm_white_margins(img: &PpmImage, margin: u32) -> (u32, u32, Vec<u8>) {
    let mut min_x = img.w;
    let mut max_x = 0u32;
    let mut min_y = img.h;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..img.h {
        for x in 0..img.w {
            let idx = ((y * img.w + x) * 3) as usize;
            if img.rgb[idx] < 245 || img.rgb[idx + 1] < 245 || img.rgb[idx + 2] < 245 {
                found = true;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }
    if !found {
        return (img.w, img.h, img.rgb.clone());
    }
    let x0 = min_x.saturating_sub(margin);
    let y0 = min_y.saturating_sub(margin);
    let x1 = (max_x + 1 + margin).min(img.w);
    let y1 = (max_y + 1 + margin).min(img.h);
    let cw = x1 - x0;
    let ch = y1 - y0;
    let mut out = Vec::with_capacity((cw * ch * 3) as usize);
    let src_stride = (img.w * 3) as usize;
    let row_bytes = (cw * 3) as usize;
    for y in y0..y1 {
        let off = (y as usize) * src_stride + (x0 as usize) * 3;
        out.extend_from_slice(&img.rgb[off..off + row_bytes]);
    }
    (cw, ch, out)
}

fn rotate_ppm_cw(img: &PpmImage, deg: i64) -> PpmImage {
    let norm = ((deg % 360) + 360) % 360;
    match norm {
        90 => {
            let nw = img.h;
            let nh = img.w;
            let mut rgb = vec![255u8; (nw * nh * 3) as usize];
            for y in 0..img.h {
                for x in 0..img.w {
                    let nx = img.h - 1 - y;
                    let ny = x;
                    let src = ((y * img.w + x) * 3) as usize;
                    let dst = ((ny * nw + nx) * 3) as usize;
                    rgb[dst..dst + 3].copy_from_slice(&img.rgb[src..src + 3]);
                }
            }
            PpmImage { w: nw, h: nh, rgb }
        }
        180 => {
            let nw = img.w;
            let nh = img.h;
            let mut rgb = vec![255u8; (nw * nh * 3) as usize];
            for y in 0..img.h {
                for x in 0..img.w {
                    let nx = img.w - 1 - x;
                    let ny = img.h - 1 - y;
                    let src = ((y * img.w + x) * 3) as usize;
                    let dst = ((ny * nw + nx) * 3) as usize;
                    rgb[dst..dst + 3].copy_from_slice(&img.rgb[src..src + 3]);
                }
            }
            PpmImage { w: nw, h: nh, rgb }
        }
        270 => {
            let nw = img.h;
            let nh = img.w;
            let mut rgb = vec![255u8; (nw * nh * 3) as usize];
            for y in 0..img.h {
                for x in 0..img.w {
                    let nx = y;
                    let ny = img.w - 1 - x;
                    let src = ((y * img.w + x) * 3) as usize;
                    let dst = ((ny * nw + nx) * 3) as usize;
                    rgb[dst..dst + 3].copy_from_slice(&img.rgb[src..src + 3]);
                }
            }
            PpmImage { w: nw, h: nh, rgb }
        }
        _ => PpmImage {
            w: img.w,
            h: img.h,
            rgb: img.rgb.clone(),
        },
    }
}

fn encode_ppm_p6(img: &PpmImage) -> Vec<u8> {
    let header = format!("P6\n{} {}\n255\n", img.w, img.h);
    let mut buf = Vec::with_capacity(header.len() + img.rgb.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(&img.rgb);
    buf
}

fn run_tesseract_tsv_on_ppm(
    tess_bin: &Path,
    tessdata: Option<&PathBuf>,
    ppm: &PpmImage,
    lang: &str,
    psm: &str,
) -> Option<String> {
    let seq = TEMP_PDF_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp_img = std::env::temp_dir();
    tmp_img.push(format!("examplus_ocr_{}_{}.ppm", std::process::id(), seq));
    if std::fs::write(&tmp_img, encode_ppm_p6(ppm)).is_err() {
        return None;
    }

    let mut cmd = hidden_command(tess_bin);
    if let Some(td) = tessdata {
        cmd.env("TESSDATA_PREFIX", td);
        cmd.arg("--tessdata-dir").arg(td);
    }
    let arg_path = tmp_img.to_string_lossy().to_string();
    cmd.args([
        &arg_path,
        "stdout",
        "-l",
        lang,
        "--psm",
        psm,
        "-c",
        "tessedit_create_tsv=1",
    ]);
    let out = cmd.output();
    let _ = std::fs::remove_file(&tmp_img);
    let out = out.ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn detect_osd_rotation(tess_bin: &Path, tessdata: Option<&PathBuf>, ppm: &PpmImage) -> i64 {
    let seq = TEMP_PDF_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp_img = std::env::temp_dir();
    tmp_img.push(format!("examplus_osd_{}_{}.ppm", std::process::id(), seq));
    if std::fs::write(&tmp_img, encode_ppm_p6(ppm)).is_err() {
        return 0;
    }
    let mut cmd = hidden_command(tess_bin);
    if let Some(td) = tessdata {
        cmd.env("TESSDATA_PREFIX", td);
        cmd.arg("--tessdata-dir").arg(td);
    }
    let arg_path = tmp_img.to_string_lossy().to_string();
    cmd.args([&arg_path, "stdout", "--psm", "0"]);
    let out = cmd.output();
    let _ = std::fs::remove_file(&tmp_img);
    if let Ok(o) = out {
        let txt = String::from_utf8_lossy(&o.stdout);
        for line in txt.lines() {
            if let Some(rest) = line.strip_prefix("Rotate:") {
                if let Ok(deg) = rest.trim().parse::<i64>() {
                    return deg;
                }
            }
        }
    }
    0
}

fn parse_tesseract_tsv_spans(
    tsv: &str,
    img_w: u32,
    img_h: u32,
    page_w: f64,
    page_h: f64,
) -> (Vec<PdfSpan>, f64) {
    let sx = page_w / (img_w.max(1) as f64);
    let sy = page_h / (img_h.max(1) as f64);
    let mut spans = Vec::new();
    let mut tsv_line_pts: HashMap<(i32, i32, i32), Vec<(f64, f64)>> = HashMap::new();
    let mut conf_sum = 0.0f64;
    let mut conf_cnt = 0usize;

    for (idx, line) in tsv.lines().enumerate() {
        if idx == 0 {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 12 || cols[0] != "5" {
            continue;
        }
        let text = cols[11].trim();
        if text.is_empty() {
            continue;
        }
        let block_num: i32 = cols[2].parse().unwrap_or(0);
        let par_num: i32 = cols[3].parse().unwrap_or(0);
        let line_num: i32 = cols[4].parse().unwrap_or(0);
        let left: f64 = cols[6].parse().unwrap_or(0.0);
        let top: f64 = cols[7].parse().unwrap_or(0.0);
        let width: f64 = cols[8].parse::<f64>().unwrap_or(10.0).max(2.0);
        let height: f64 = cols[9].parse::<f64>().unwrap_or(12.0).max(4.0);
        let conf: f64 = cols[10].parse().unwrap_or(75.0);
        if conf >= 0.0 {
            conf_sum += conf;
            conf_cnt += 1;
        }
        let fs = (height * sy * 0.9).clamp(8.5, 16.0);
        let x0 = left * sx;
        let x1 = (left + width) * sx;
        let y_base = (top + height * 0.80) * sy;
        tsv_line_pts
            .entry((block_num, par_num, line_num))
            .or_default()
            .push(((x0 + x1) * 0.5, y_base));
        spans.push(PdfSpan {
            text: text.to_string(),
            x0,
            x1,
            y_base,
            font_size: fs,
            is_bold: false,
            is_italic: false,
            has_unmapped_pua: false,
            ocr_conf: Some(conf.clamp(0.0, 100.0)),
        });
    }

    // Estimate page-wide horizontal baseline skew (dy/dx) from clean TSV lines so
    // tilted scans align horizontally before build_page_elements groups y rows.
    let mut line_slopes: Vec<f64> = Vec::new();
    for pts in tsv_line_pts.values() {
        if pts.len() < 4 {
            continue;
        }
        let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        if max_x - min_x < page_w * 0.25 {
            continue;
        }
        let n = pts.len() as f64;
        let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
        let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
        let mut sxx = 0.0;
        let mut sxy = 0.0;
        for &(x, y) in pts {
            let dx = x - mx;
            sxx += dx * dx;
            sxy += dx * (y - my);
        }
        if sxx <= 1.0 {
            continue;
        }
        let slope = sxy / sxx;
        if slope.abs() > 0.18 {
            continue;
        }
        let intercept = my - slope * mx;
        let mean_res = pts
            .iter()
            .map(|&(x, y)| (y - (intercept + slope * x)).abs())
            .sum::<f64>()
            / n;
        if mean_res <= 2.5 {
            line_slopes.push(slope);
        }
    }
    if line_slopes.len() >= 2 {
        line_slopes.sort_by(|a, b| a.total_cmp(b));
        let median_slope = line_slopes[line_slopes.len() / 2];
        let consistent = line_slopes
            .iter()
            .filter(|s| (*s - median_slope).abs() <= 0.025)
            .count();
        if consistent >= 2 && median_slope.abs() > 0.003 {
            let theta = median_slope.atan();
            let (sin_t, cos_t) = theta.sin_cos();
            let cx = page_w * 0.5;
            let cy = page_h * 0.5;
            for sp in &mut spans {
                let hw = (sp.x1 - sp.x0) * 0.5;
                let dx = (sp.x0 + sp.x1) * 0.5 - cx;
                let dy = sp.y_base - cy;
                let nx = cx + dx * cos_t + dy * sin_t;
                let ny = cy - dx * sin_t + dy * cos_t;
                sp.x0 = (nx - hw).clamp(0.0, page_w);
                sp.x1 = (nx + hw).clamp(0.0, page_w);
                sp.y_base = ny.clamp(0.0, page_h);
            }
        }
    }

    let mean_conf = if conf_cnt > 0 {
        conf_sum / (conf_cnt as f64)
    } else {
        0.0
    };
    (spans, mean_conf)
}

fn run_tesseract_ocr_on_ppm(
    ppm: &PpmImage,
    page_w: f64,
    page_h: f64,
) -> Option<(Vec<PdfSpan>, f64)> {
    let tess_bin = find_tesseract_bin()?;
    let tessdata = find_tessdata_prefix(&tess_bin);
    let lang = if tessdata
        .as_ref()
        .is_some_and(|td| td.join("vie.traineddata").exists())
    {
        "vie+eng"
    } else {
        "eng"
    };

    let tsv6 = run_tesseract_tsv_on_ppm(&tess_bin, tessdata.as_ref(), ppm, lang, "6")?;
    let (mut spans, mut mean_conf) =
        parse_tesseract_tsv_spans(&tsv6, ppm.w, ppm.h, page_w, page_h);

    let char_count: usize = spans
        .iter()
        .map(|s| s.text.chars().filter(|c| c.is_alphanumeric()).count())
        .sum();
    if char_count < 15 || mean_conf < 48.0 {
        let osd_rot = detect_osd_rotation(&tess_bin, tessdata.as_ref(), ppm);
        if matches!(osd_rot, 90 | 180 | 270) {
            let rotated = rotate_ppm_cw(ppm, osd_rot);
            let (tw, th) = if osd_rot == 90 || osd_rot == 270 {
                (page_h, page_w)
            } else {
                (page_w, page_h)
            };
            if let Some(tsv_rot) =
                run_tesseract_tsv_on_ppm(&tess_bin, tessdata.as_ref(), &rotated, lang, "6")
            {
                let (rot_spans, rot_conf) =
                    parse_tesseract_tsv_spans(&tsv_rot, rotated.w, rotated.h, tw, th);
                if rot_conf > mean_conf || rot_spans.len() > spans.len() {
                    spans = rot_spans;
                    mean_conf = rot_conf;
                }
            }
        }
    }

    Some((spans, mean_conf))
}

struct PuaFallbackRegion {
    bbox: [f64; 4],
    is_inline: bool,
    reason: String,
}

fn collect_pua_fallback_regions(pg: &PdfPageData) -> Vec<PuaFallbackRegion> {
    let mut pua_idxs: Vec<usize> = pg
        .spans
        .iter()
        .enumerate()
        .filter(|(_, s)| s.has_unmapped_pua)
        .map(|(i, _)| i)
        .collect();
    if pua_idxs.is_empty() {
        return Vec::new();
    }
    pua_idxs.sort_by(|&a, &b| {
        pg.spans[a]
            .y_base
            .partial_cmp(&pg.spans[b].y_base)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                pg.spans[a]
                    .x0
                    .partial_cmp(&pg.spans[b].x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut used = vec![false; pua_idxs.len()];
    let mut regions = Vec::new();

    for idx_pos in 0..pua_idxs.len() {
        if used[idx_pos] {
            continue;
        }
        used[idx_pos] = true;
        let first = &pg.spans[pua_idxs[idx_pos]];
        let mut bx0 = first.x0;
        let mut by0 = first.y_base - first.font_size;
        let mut bx1 = first.x1;
        let mut by1 = first.y_base + first.font_size * 0.28;
        let mut is_brace_cluster = first.text.contains('{');

        // Gom các mảnh PUA lân cận (ví dụ các mảnh `\uf8f1 \uf8f2 \uf8f3` của hệ hàm piecewise)
        loop {
            let mut grew = false;
            for j in 0..pua_idxs.len() {
                if used[j] {
                    continue;
                }
                let sp = &pg.spans[pua_idxs[j]];
                let sy0 = sp.y_base - sp.font_size;
                let sy1 = sp.y_base + sp.font_size * 0.28;
                if sp.x0 <= bx1 + 26.0
                    && sp.x1 >= bx0 - 26.0
                    && sy0 <= by1 + 14.0
                    && sy1 >= by0 - 14.0
                {
                    used[j] = true;
                    bx0 = bx0.min(sp.x0);
                    by0 = by0.min(sy0);
                    bx1 = bx1.max(sp.x1);
                    by1 = by1.max(sy1);
                    if sp.text.contains('{') {
                        is_brace_cluster = true;
                    }
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }

        if is_brace_cluster && (by1 - by0) >= 15.0 {
            // Hệ ngoặc nhọn nhiều dòng (`f(x) = { ... }`): mở rộng sang phải để ôm trọn các nhánh
            let mut max_safe_x = f64::INFINITY;
            for sp in &pg.spans {
                if sp.x0 > bx0 + 4.0 && sp.y_base >= by0 - 8.0 && sp.y_base <= by1 + 10.0 {
                    let t = sp.text.trim();
                    if t.starts_with("Câu ")
                        || t.starts_with("Question ")
                        || t.starts_with("PHẦN")
                        || t.starts_with("A.")
                        || t.starts_with("B.")
                        || t.starts_with("C.")
                        || t.starts_with("D.")
                    {
                        max_safe_x = max_safe_x.min(sp.x0);
                    }
                }
            }
            let mut band_spans: Vec<&PdfSpan> = pg
                .spans
                .iter()
                .filter(|sp| {
                    sp.x0 >= bx0 - 2.0
                        && sp.x0 < max_safe_x - 2.0
                        && sp.x0 <= bx1 + 260.0
                        && sp.y_base >= by0 - 3.0
                        && sp.y_base <= by1 + 6.0
                })
                .collect();
            band_spans.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
            let mut ex1 = bx1;
            for sp in band_spans {
                if sp.x0 <= ex1 + 85.0 && sp.x1 - bx0 <= pg.width * 0.48 {
                    ex1 = ex1.max(sp.x1);
                }
            }
            // Bao gồm nhãn hàm số `f(x) =` ngay sát trái ngoặc nếu đứng gần
            let mut ex0 = bx0;
            for sp in &pg.spans {
                if sp.x1 <= bx0 + 4.0
                    && sp.x0 >= bx0 - 46.0
                    && sp.y_base >= by0
                    && sp.y_base <= by1
                    && !sp.text.contains("số")
                    && !sp.text.contains("Câu")
                    && !sp.text.trim().starts_with("A.")
                    && !sp.text.trim().starts_with("B.")
                    && !sp.text.trim().starts_with("C.")
                    && !sp.text.trim().starts_with("D.")
                {
                    if sp.x0 >= ex0 - 28.0 {
                        ex0 = ex0.min(sp.x0);
                    }
                }
            }
            bx0 = ex0;
            bx1 = ex1;
            regions.push(PuaFallbackRegion {
                bbox: [bx0, by0 - 2.0, bx1, by1 + 2.0],
                is_inline: false,
                reason: "Giữ ảnh vùng hệ công thức nhiều dòng từ PDF để bảo toàn bố cục.".to_string(),
            });
        } else {
            // Cụm PUA nhỏ trên dòng (ví dụ `k ∈ ℤ`, `△SCD`, `MN ‖ CD`): ôm nhẹ vùng ký tự công thức xung quanh
            let mut ex0 = bx0 - 4.0;
            let mut ex1 = bx1 + 2.0;
            for sp in &pg.spans {
                if (sp.y_base - first.y_base).abs() <= 4.5 && sp.x1 >= ex0 - 14.0 && sp.x0 <= ex1 + 10.0 {
                    let t = sp.text.trim();
                    if t.chars().count() <= 6
                        && !t.starts_with("Câu")
                        && !t.starts_with("PHẦN")
                        && !t.starts_with("Question")
                        && !t.starts_with("A.")
                        && !t.starts_with("B.")
                        && !t.starts_with("C.")
                        && !t.starts_with("D.")
                    {
                        ex0 = ex0.min(sp.x0);
                        ex1 = ex1.max(sp.x1);
                    }
                }
            }
            regions.push(PuaFallbackRegion {
                bbox: [ex0, by0 - 1.5, ex1, by1 + 1.5],
                is_inline: true,
                reason: "Giữ ảnh công thức gốc từ PDF do ký tự toán font PUA chưa có bảng ánh xạ chuẩn.".to_string(),
            });
        }
    }
    regions
}

fn cluster_vector_figures(pg: &PdfPageData) -> Vec<[f64; 4]> {
    if pg.vector_boxes.is_empty() {
        return Vec::new();
    }
    let pw = pg.width;
    let ph = pg.height;
    let mut boxes: Vec<[f64; 4]> = pg
        .vector_boxes
        .iter()
        .copied()
        .filter(|b| {
            let bw = (b[2] - b[0]).abs();
            let bh = (b[3] - b[1]).abs();
            !(bw > pw * 0.85 && bh > ph * 0.85) && (bw >= 2.0 || bh >= 2.0)
        })
        .collect();
    if boxes.is_empty() {
        return Vec::new();
    }

    let mut used = vec![false; boxes.len()];
    let mut clusters: Vec<[f64; 4]> = Vec::new();
    for i in 0..boxes.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut cur = boxes[i];
        loop {
            let mut merged = false;
            for j in 0..boxes.len() {
                if used[j] {
                    continue;
                }
                let b = boxes[j];
                if b[0] <= cur[2] + 16.0
                    && b[2] >= cur[0] - 16.0
                    && b[1] <= cur[3] + 16.0
                    && b[3] >= cur[1] - 16.0
                {
                    used[j] = true;
                    cur = [
                        cur[0].min(b[0]),
                        cur[1].min(b[1]),
                        cur[2].max(b[2]),
                        cur[3].max(b[3]),
                    ];
                    merged = true;
                }
            }
            if !merged {
                break;
            }
        }
        boxes[i] = cur;
        clusters.push(cur);
    }

    let mut valid = Vec::new();
    for c in clusters {
        let cw = c[2] - c[0];
        let ch = c[3] - c[1];
        if cw < 38.0 || ch < 28.0 {
            continue;
        }
        if cw > pw * 0.88 && ch > ph * 0.85 {
            continue;
        }
        // Bỏ qua cụm nằm chồng lấn lên ảnh bitmap đã có sẵn
        let overlaps_raster = pg.images.iter().any(|im| {
            let ix0 = im.x0.min(im.x1);
            let ix1 = im.x0.max(im.x1);
            let iy0 = im.y0.min(im.y1);
            let iy1 = im.y0.max(im.y1);
            let ox = (c[2].min(ix1) - c[0].max(ix0)).max(0.0);
            let oy = (c[3].min(iy1) - c[1].max(iy0)).max(0.0);
            ox * oy > (cw * ch) * 0.35
        });
        if overlaps_raster {
            continue;
        }
        // Phân biệt hình vẽ đồ thị/bảng biến thiên vs khung bảng chứa văn bản câu hỏi dài
        let mut has_exam_heading = false;
        let mut opt_markers = 0u8;
        let long_prose_spans = pg
            .spans
            .iter()
            .filter(|s| {
                let cx = 0.5 * (s.x0 + s.x1);
                let inside = cx >= c[0] - 2.0
                    && cx <= c[2] + 2.0
                    && s.y_base >= c[1] - 2.0
                    && s.y_base <= c[3] + 2.0;
                if !inside {
                    return false;
                }
                let t = s.text.trim();
                if t.contains("Câu ") || t.contains("PHẦN") || t.contains("Question ") {
                    has_exam_heading = true;
                }
                if t.starts_with("A.") {
                    opt_markers |= 1;
                } else if t.starts_with("B.") {
                    opt_markers |= 2;
                } else if t.starts_with("C.") {
                    opt_markers |= 4;
                } else if t.starts_with("D.") {
                    opt_markers |= 8;
                }
                s.text.split_whitespace().count() >= 4
                    || t.contains("Câu ")
                    || t.contains("PHẦN")
            })
            .count();
        if has_exam_heading || opt_markers.count_ones() >= 2 || long_prose_spans >= 3 {
            continue;
        }
        valid.push(c);
    }
    valid
}

fn resolve_page_box(doc: &Document, page_id: ObjectId) -> [f64; 4] {
    let mut cur = Some(page_id);
    let mut seen = HashSet::new();
    while let Some(id) = cur {
        if !seen.insert(id) {
            break;
        }
        if let Ok(dict) = doc.get_dictionary(id) {
            for key in [b"CropBox".as_slice(), b"MediaBox".as_slice()] {
                if let Ok(obj) = dict.get(key) {
                    if let Some(arr) = resolve_array(doc, obj) {
                        if arr.len() >= 4 {
                            if let (Some(a), Some(b), Some(c), Some(d)) = (
                                obj_as_f64(&arr[0]),
                                obj_as_f64(&arr[1]),
                                obj_as_f64(&arr[2]),
                                obj_as_f64(&arr[3]),
                            ) {
                                return [a.min(c), b.min(d), a.max(c), b.max(d)];
                            }
                        }
                    }
                }
            }
            cur = dict.get(b"Parent").and_then(Object::as_reference).ok();
        } else {
            break;
        }
    }
    [0.0, 0.0, 595.0, 842.0]
}

fn collect_page_resource_dicts<'a>(doc: &'a Document, page_id: ObjectId) -> Vec<&'a lopdf::Dictionary> {
    let mut out = Vec::new();
    if let Ok((opt_dict, ref_ids)) = doc.get_page_resources(page_id) {
        if let Some(d) = opt_dict {
            out.push(d);
        }
        for rid in ref_ids {
            if let Ok(d) = doc.get_dictionary(rid) {
                out.push(d);
            }
        }
    }
    out
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a lopdf::Dictionary> {
    match obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        _ => None,
    }
}

fn resolve_array<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Vec<Object>> {
    match obj {
        Object::Array(a) => Some(a),
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_array().ok()),
        _ => None,
    }
}

fn obj_as_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

fn mat_mul(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    // Chuẩn PDF row-vector matrix: [x' y' 1] = [x y 1] * M
    // a * b
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn transform_pt(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

#[derive(Clone)]
struct GState {
    ctm: [f64; 6],
    font_name: Vec<u8>,
    font_size: f64,
    char_space: f64,
    word_space: f64,
    horiz_scale: f64,
    leading: f64,
    rise: f64,
    fill_is_white: bool,
    clip_rect: Option<[f64; 4]>,
}

impl Default for GState {
    fn default() -> Self {
        Self {
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            font_name: Vec::new(),
            font_size: 11.0,
            char_space: 0.0,
            word_space: 0.0,
            horiz_scale: 100.0,
            leading: 0.0,
            rise: 0.0,
            fill_is_white: false,
            clip_rect: None,
        }
    }
}

struct ExtractState<'a> {
    doc: &'a Document,
    font_cache: &'a mut HashMap<ObjectId, FontDecodeTable>,
    image_cache: &'a mut HashMap<ObjectId, Option<DecodedPdfImage>>,
    crop_box: [f64; 4],
    raw_w: f64,
    raw_h: f64,
    rotate_deg: i64,
    page_w: f64,
    page_h: f64,
    text_dir_vx: f64,
    text_dir_vy: f64,
    text_abs_vx: f64,
    text_abs_vy: f64,
    spans: Vec<PdfSpan>,
    h_lines: Vec<PdfHLine>,
    v_lines: Vec<PdfVLine>,
    vector_boxes: Vec<[f64; 4]>,
    images: Vec<PdfPlacedImage>,
}

impl<'a> ExtractState<'a> {
    fn to_visual_pt(&self, ux: f64, uy: f64) -> (f64, f64) {
        let rx = ux - self.crop_box[0];
        let ry = self.crop_box[3] - uy;
        match self.rotate_deg {
            90 => (self.raw_h - ry, rx),
            180 => (self.raw_w - rx, self.raw_h - ry),
            270 => (ry, self.raw_w - rx),
            _ => (rx, ry),
        }
    }

    fn walk_stream_bytes(
        &mut self,
        stream_bytes: &[u8],
        res_dicts: &[&lopdf::Dictionary],
        init_ctm: [f64; 6],
        init_clip: Option<[f64; 4]>,
        depth: usize,
    ) {
        if depth > MAX_FORM_RECURSION || stream_bytes.is_empty() {
            return;
        }
        let Ok(content) = Content::decode(stream_bytes) else {
            return;
        };

        let mut gs = GState {
            ctm: init_ctm,
            clip_rect: init_clip,
            ..GState::default()
        };
        let mut gs_stack: Vec<GState> = Vec::new();
        let mut tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut tlm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut in_text = false;

        // Path accumulator for fraction bars, table lines, clipping boxes, and general vector diagrams
        let mut cur_path_pt: Option<(f64, f64)> = None;
        let mut subpath_start: Option<(f64, f64)> = None;
        let mut pending_segs: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut pending_curves: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut pending_rects: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut pending_clip = false;

        for op in &content.operations {
            let args = &op.operands;
            match op.operator.as_str() {
                "q" => gs_stack.push(gs.clone()),
                "Q" => {
                    if let Some(prev) = gs_stack.pop() {
                        gs = prev;
                    }
                }
                "cm" if args.len() >= 6 => {
                    if let Some(m) = read_matrix(args) {
                        gs.ctm = mat_mul(m, gs.ctm);
                    }
                }
                "BT" => {
                    in_text = true;
                    tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                    tlm = tm;
                }
                "ET" => {
                    in_text = false;
                }
                "Tf" if args.len() >= 2 => {
                    if let Ok(name) = args[0].as_name() {
                        gs.font_name = name.to_vec();
                    }
                    if let Some(sz) = obj_as_f64(&args[1]) {
                        gs.font_size = sz;
                    }
                }
                "Tc" if !args.is_empty() => {
                    if let Some(v) = obj_as_f64(&args[0]) {
                        gs.char_space = v;
                    }
                }
                "Tw" if !args.is_empty() => {
                    if let Some(v) = obj_as_f64(&args[0]) {
                        gs.word_space = v;
                    }
                }
                "Tz" if !args.is_empty() => {
                    if let Some(v) = obj_as_f64(&args[0]) {
                        gs.horiz_scale = if v.abs() > 1e-3 { v } else { 100.0 };
                    }
                }
                "TL" if !args.is_empty() => {
                    if let Some(v) = obj_as_f64(&args[0]) {
                        gs.leading = v;
                    }
                }
                "Ts" if !args.is_empty() => {
                    if let Some(v) = obj_as_f64(&args[0]) {
                        gs.rise = v;
                    }
                }
                "Tm" if args.len() >= 6 && in_text => {
                    if let Some(m) = read_matrix(args) {
                        tm = m;
                        tlm = m;
                    }
                }
                "Td" if args.len() >= 2 && in_text => {
                    if let (Some(tx), Some(ty)) = (obj_as_f64(&args[0]), obj_as_f64(&args[1])) {
                        let trans = [1.0, 0.0, 0.0, 1.0, tx, ty];
                        tlm = mat_mul(trans, tlm);
                        tm = tlm;
                    }
                }
                "TD" if args.len() >= 2 && in_text => {
                    if let (Some(tx), Some(ty)) = (obj_as_f64(&args[0]), obj_as_f64(&args[1])) {
                        gs.leading = -ty;
                        let trans = [1.0, 0.0, 0.0, 1.0, tx, ty];
                        tlm = mat_mul(trans, tlm);
                        tm = tlm;
                    }
                }
                "T*" if in_text => {
                    let trans = [1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading];
                    tlm = mat_mul(trans, tlm);
                    tm = tlm;
                }
                "Tj" if !args.is_empty() && in_text => {
                    if let Ok(bytes) = args[0].as_str() {
                        self.emit_text_bytes(&gs, &mut tm, res_dicts, bytes);
                    }
                }
                "'" if !args.is_empty() && in_text => {
                    let trans = [1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading];
                    tlm = mat_mul(trans, tlm);
                    tm = tlm;
                    if let Ok(bytes) = args[0].as_str() {
                        self.emit_text_bytes(&gs, &mut tm, res_dicts, bytes);
                    }
                }
                "\"" if args.len() >= 3 && in_text => {
                    if let Some(aw) = obj_as_f64(&args[0]) {
                        gs.word_space = aw;
                    }
                    if let Some(ac) = obj_as_f64(&args[1]) {
                        gs.char_space = ac;
                    }
                    let trans = [1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading];
                    tlm = mat_mul(trans, tlm);
                    tm = tlm;
                    if let Ok(bytes) = args[2].as_str() {
                        self.emit_text_bytes(&gs, &mut tm, res_dicts, bytes);
                    }
                }
                "TJ" if !args.is_empty() && in_text => {
                    if let Ok(arr) = args[0].as_array() {
                        for item in arr {
                            match item {
                                Object::String(bytes, _) => {
                                    self.emit_text_bytes(&gs, &mut tm, res_dicts, bytes);
                                }
                                Object::Integer(kern) => {
                                    let dx = -(*kern as f64) / 1000.0
                                        * gs.font_size
                                        * (gs.horiz_scale / 100.0);
                                    let tr = [1.0, 0.0, 0.0, 1.0, dx, 0.0];
                                    tm = mat_mul(tr, tm);
                                }
                                Object::Real(kern) => {
                                    let dx = -(*kern as f64) / 1000.0
                                        * gs.font_size
                                        * (gs.horiz_scale / 100.0);
                                    let tr = [1.0, 0.0, 0.0, 1.0, dx, 0.0];
                                    tm = mat_mul(tr, tm);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "m" if args.len() >= 2 => {
                    if let (Some(x), Some(y)) = (obj_as_f64(&args[0]), obj_as_f64(&args[1])) {
                        cur_path_pt = Some((x, y));
                        subpath_start = Some((x, y));
                    }
                }
                "l" if args.len() >= 2 => {
                    if let (Some(x), Some(y)) = (obj_as_f64(&args[0]), obj_as_f64(&args[1])) {
                        if let Some((px, py)) = cur_path_pt {
                            pending_segs.push((px, py, x, y));
                        }
                        cur_path_pt = Some((x, y));
                    }
                }
                "h" => {
                    if let (Some((cx, cy)), Some((sx, sy))) = (cur_path_pt, subpath_start) {
                        pending_segs.push((cx, cy, sx, sy));
                        cur_path_pt = Some((sx, sy));
                    }
                }
                "c" if args.len() >= 6 => {
                    if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x3), Some(y3)) = (
                        obj_as_f64(&args[0]),
                        obj_as_f64(&args[1]),
                        obj_as_f64(&args[2]),
                        obj_as_f64(&args[3]),
                        obj_as_f64(&args[4]),
                        obj_as_f64(&args[5]),
                    ) {
                        let (sx, sy) = cur_path_pt.unwrap_or((x1, y1));
                        let min_x = sx.min(x1).min(x2).min(x3);
                        let min_y = sy.min(y1).min(y2).min(y3);
                        let max_x = sx.max(x1).max(x2).max(x3);
                        let max_y = sy.max(y1).max(y2).max(y3);
                        pending_curves.push((min_x, min_y, max_x, max_y));
                        cur_path_pt = Some((x3, y3));
                    }
                }
                "v" | "y" if args.len() >= 4 => {
                    if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                        obj_as_f64(&args[0]),
                        obj_as_f64(&args[1]),
                        obj_as_f64(&args[2]),
                        obj_as_f64(&args[3]),
                    ) {
                        let (sx, sy) = cur_path_pt.unwrap_or((x1, y1));
                        pending_curves.push((sx.min(x1).min(x2), sy.min(y1).min(y2), sx.max(x1).max(x2), sy.max(y1).max(y2)));
                        cur_path_pt = Some((x2, y2));
                    }
                }
                "re" if args.len() >= 4 => {
                    if let (Some(x), Some(y), Some(w), Some(h)) = (
                        obj_as_f64(&args[0]),
                        obj_as_f64(&args[1]),
                        obj_as_f64(&args[2]),
                        obj_as_f64(&args[3]),
                    ) {
                        pending_rects.push((x, y, w, h));
                    }
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    if pending_clip {
                        let cbox = self.compute_path_visual_bbox(
                            gs.ctm,
                            &pending_rects,
                            &pending_segs,
                            &pending_curves,
                        );
                        Self::apply_clip_bbox(&mut gs, cbox);
                        pending_clip = false;
                    }
                    let is_fill = matches!(op.operator.as_str(), "f" | "F" | "f*" | "B" | "B*" | "b" | "b*");
                    let is_stroke = matches!(op.operator.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*");
                    let skip_white_fill_only = !is_stroke && gs.fill_is_white;
                    let mut poly_min_x = f64::INFINITY;
                    let mut poly_min_y = f64::INFINITY;
                    let mut poly_max_x = f64::NEG_INFINITY;
                    let mut poly_max_y = f64::NEG_INFINITY;
                    let seg_count = pending_segs.len();
                    for (x0, y0, x1, y1) in pending_segs.drain(..) {
                        if skip_white_fill_only {
                            continue;
                        }
                        let (px0, py0, px1, py1) = self.record_segment(gs.ctm, x0, y0, x1, y1);
                        poly_min_x = poly_min_x.min(px0).min(px1);
                        poly_min_y = poly_min_y.min(py0).min(py1);
                        poly_max_x = poly_max_x.max(px0).max(px1);
                        poly_max_y = poly_max_y.max(py0).max(py1);
                    }
                    if !skip_white_fill_only && seg_count >= 3 && poly_max_x.is_finite() {
                        let pw = poly_max_x - poly_min_x;
                        let ph = poly_max_y - poly_min_y;
                        if pw >= 6.0 && ph >= 6.0 && !(pw > self.page_w * 0.85 && ph > self.page_h * 0.85) {
                            self.vector_boxes.push([poly_min_x, poly_min_y, poly_max_x, poly_max_y]);
                        }
                    }
                    for (cx0, cy0, cx1, cy1) in pending_curves.drain(..) {
                        if skip_white_fill_only {
                            continue;
                        }
                        let cpts = [
                            transform_pt(gs.ctm, cx0, cy0),
                            transform_pt(gs.ctm, cx1, cy0),
                            transform_pt(gs.ctm, cx0, cy1),
                            transform_pt(gs.ctm, cx1, cy1),
                        ];
                        let vpts: Vec<(f64, f64)> =
                            cpts.into_iter().map(|(x, y)| self.to_visual_pt(x, y)).collect();
                        let bx0 = vpts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                        let bx1 = vpts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                        let by0 = vpts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
                        let by1 = vpts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
                        if (bx1 - bx0).max(by1 - by0) >= 2.5 {
                            self.vector_boxes.push([bx0, by0, bx1, by1]);
                        }
                    }
                    for (rx, ry, rw, rh) in pending_rects.drain(..) {
                        if skip_white_fill_only {
                            continue;
                        }
                        let corners = [
                            transform_pt(gs.ctm, rx, ry),
                            transform_pt(gs.ctm, rx + rw, ry + rh),
                        ];
                        let (vx0, vy0) = self.to_visual_pt(corners[0].0, corners[0].1);
                        let (vx1, vy1) = self.to_visual_pt(corners[1].0, corners[1].1);
                        let bx0 = vx0.min(vx1);
                        let bx1 = vx0.max(vx1);
                        let by0 = vy0.min(vy1);
                        let by1 = vy0.max(vy1);
                        let bw = (bx1 - bx0).abs();
                        let bh = (by1 - by0).abs();
                        if bh <= 2.6 && bw >= 4.0 {
                            self.h_lines.push(PdfHLine {
                                x0: bx0,
                                x1: bx1,
                                y: 0.5 * (by0 + by1),
                            });
                        } else if bw <= 2.6 && bh >= 4.0 {
                            self.v_lines.push(PdfVLine {
                                x: 0.5 * (bx0 + bx1),
                                y0: by0,
                                y1: by1,
                            });
                        } else if bw > 8.0 && bh > 8.0 && bh < self.page_h * 0.85 {
                            if is_fill && bw < self.page_w * 0.75 {
                                self.vector_boxes.push([bx0, by0, bx1, by1]);
                            }
                            self.h_lines.push(PdfHLine { x0: bx0, x1: bx1, y: by0 });
                            self.h_lines.push(PdfHLine { x0: bx0, x1: bx1, y: by1 });
                            self.v_lines.push(PdfVLine { x: bx0, y0: by0, y1: by1 });
                            self.v_lines.push(PdfVLine { x: bx1, y0: by0, y1: by1 });
                        }
                    }
                    cur_path_pt = None;
                    subpath_start = None;
                }
                "rg" if args.len() >= 3 => {
                    if let (Some(r), Some(g), Some(b)) =
                        (obj_as_f64(&args[0]), obj_as_f64(&args[1]), obj_as_f64(&args[2]))
                    {
                        gs.fill_is_white = r >= 0.94 && g >= 0.94 && b >= 0.94;
                    }
                }
                "g" if !args.is_empty() => {
                    if let Some(v) = obj_as_f64(&args[0]) {
                        gs.fill_is_white = v >= 0.94;
                    }
                }
                "k" if args.len() >= 4 => {
                    if let (Some(c), Some(m), Some(y), Some(k)) = (
                        obj_as_f64(&args[0]),
                        obj_as_f64(&args[1]),
                        obj_as_f64(&args[2]),
                        obj_as_f64(&args[3]),
                    ) {
                        gs.fill_is_white = c <= 0.06 && m <= 0.06 && y <= 0.06 && k <= 0.06;
                    }
                }
                "cs" => {
                    gs.fill_is_white = false;
                }
                "sc" | "scn" => {
                    let nums: Vec<f64> = args.iter().filter_map(obj_as_f64).collect();
                    if nums.len() == args.len() {
                        gs.fill_is_white = match nums.len() {
                            1 => nums[0] >= 0.94,
                            3 => nums[0] >= 0.94 && nums[1] >= 0.94 && nums[2] >= 0.94,
                            4 => nums[0] <= 0.06 && nums[1] <= 0.06 && nums[2] <= 0.06 && nums[3] <= 0.06,
                            _ => false,
                        };
                    }
                }
                "W" | "W*" => {
                    pending_clip = true;
                }
                "n" => {
                    if pending_clip {
                        let cbox = self.compute_path_visual_bbox(
                            gs.ctm,
                            &pending_rects,
                            &pending_segs,
                            &pending_curves,
                        );
                        Self::apply_clip_bbox(&mut gs, cbox);
                        pending_clip = false;
                    }
                    pending_segs.clear();
                    pending_curves.clear();
                    pending_rects.clear();
                    cur_path_pt = None;
                    subpath_start = None;
                }
                "Do" if !args.is_empty() => {
                    if let Ok(xname) = args[0].as_name() {
                        self.handle_xobject(&gs, res_dicts, xname, depth);
                    }
                }
                _ => {}
            }
        }
    }

    fn record_segment(&mut self, ctm: [f64; 6], x0: f64, y0: f64, x1: f64, y1: f64) -> (f64, f64, f64, f64) {
        let (ax, ay) = transform_pt(ctm, x0, y0);
        let (bx, by) = transform_pt(ctm, x1, y1);
        let (px0, py0) = self.to_visual_pt(ax, ay);
        let (px1, py1) = self.to_visual_pt(bx, by);
        let dx = (px1 - px0).abs();
        let dy = (py1 - py0).abs();
        if dy <= 1.8 && dx >= 4.0 {
            self.h_lines.push(PdfHLine {
                x0: px0.min(px1),
                x1: px0.max(px1),
                y: 0.5 * (py0 + py1),
            });
        } else if dx <= 1.8 && dy >= 4.0 {
            self.v_lines.push(PdfVLine {
                x: 0.5 * (px0 + px1),
                y0: py0.min(py1),
                y1: py0.max(py1),
            });
        } else if dx > 2.2 && dy > 2.2 {
            self.vector_boxes.push([
                px0.min(px1),
                py0.min(py1),
                px0.max(px1),
                py0.max(py1),
            ]);
        }
        (px0, py0, px1, py1)
    }

    fn compute_path_visual_bbox(
        &self,
        ctm: [f64; 6],
        rects: &[(f64, f64, f64, f64)],
        segs: &[(f64, f64, f64, f64)],
        curves: &[(f64, f64, f64, f64)],
    ) -> Option<[f64; 4]> {
        let mut bx0 = f64::INFINITY;
        let mut by0 = f64::INFINITY;
        let mut bx1 = f64::NEG_INFINITY;
        let mut by1 = f64::NEG_INFINITY;
        let mut add_pt = |ux: f64, uy: f64| {
            let (tx, ty) = transform_pt(ctm, ux, uy);
            let (vx, vy) = self.to_visual_pt(tx, ty);
            bx0 = bx0.min(vx);
            bx1 = bx1.max(vx);
            by0 = by0.min(vy);
            by1 = by1.max(vy);
        };
        for &(rx, ry, rw, rh) in rects {
            add_pt(rx, ry);
            add_pt(rx + rw, ry);
            add_pt(rx, ry + rh);
            add_pt(rx + rw, ry + rh);
        }
        for &(x0, y0, x1, y1) in segs {
            add_pt(x0, y0);
            add_pt(x1, y1);
        }
        for &(cx0, cy0, cx1, cy1) in curves {
            add_pt(cx0, cy0);
            add_pt(cx1, cy0);
            add_pt(cx0, cy1);
            add_pt(cx1, cy1);
        }
        if bx0.is_finite() && by0.is_finite() && bx1.is_finite() && by1.is_finite() {
            Some([bx0, by0, bx1, by1])
        } else {
            None
        }
    }

    fn apply_clip_bbox(gs: &mut GState, bbox: Option<[f64; 4]>) {
        let Some([cx0, cy0, cx1, cy1]) = bbox else {
            return;
        };
        gs.clip_rect = Some(match gs.clip_rect {
            Some([ox0, oy0, ox1, oy1]) => [ox0.max(cx0), oy0.max(cy0), ox1.min(cx1), oy1.min(cy1)],
            None => [cx0, cy0, cx1, cy1],
        });
    }

    fn span_clipped(clip: Option<[f64; 4]>, x0: f64, x1: f64, y_base: f64, eff_fs: f64) -> bool {
        let Some([cx0, cy0, cx1, cy1]) = clip else {
            return false;
        };
        let g_top = y_base - eff_fs * 0.78;
        let g_bot = y_base + eff_fs * 0.24;
        x1 < cx0 - 0.5 || x0 > cx1 + 0.5 || g_bot < cy0 - 0.5 || g_top > cy1 + 0.5
    }

    fn emit_text_bytes(
        &mut self,
        gs: &GState,
        tm: &mut [f64; 6],
        res_dicts: &[&lopdf::Dictionary],
        bytes: &[u8],
    ) {
        if bytes.is_empty() {
            return;
        }
        let table = self.resolve_font(res_dicts, &gs.font_name);
        let unit_step = if table.is_cid { 2usize } else { 1usize };
        let trm_peek = mat_mul(*tm, gs.ctm);
        let scale_x_peek = (trm_peek[0] * trm_peek[0] + trm_peek[1] * trm_peek[1]).sqrt();
        let scale_y_peek = (trm_peek[2] * trm_peek[2] + trm_peek[3] * trm_peek[3]).sqrt();
        let eff_fs_peek = (gs.font_size * scale_y_peek).abs().max(1.0);
        let page_char_space = (gs.char_space * (gs.horiz_scale / 100.0) * scale_x_peek).abs();

        // Khi `Tc` (tính theo điểm trang) lớn bất thường (ví dụ `/TT0 1 Tf` + `12 Tm` + `1.824 Tc` trong Word Print PDF),
        // tách từng glyph để mỗi ký tự mang đúng tọa độ `x0..x1` thực tế thay vì bị dồn về X của ký tự đầu (`2xx2-+31`, `aP`).
        if page_char_space >= (eff_fs_peek * 0.28).max(2.2) && bytes.len() > unit_step {
            let mut i = 0usize;
            while i < bytes.len() {
                let (decoded, width_1000, step, is_space, had_pua) = table.decode_next_unit(bytes, i);
                let trm = mat_mul(*tm, gs.ctm);
                let scale_x = (trm[0] * trm[0] + trm[1] * trm[1]).sqrt();
                let scale_y = (trm[2] * trm[2] + trm[3] * trm[3]).sqrt();
                let abs_fs_y = (gs.font_size * scale_y).abs();
                let abs_fs_x = (gs.font_size * scale_x).abs();
                let eff_fs = if abs_fs_y > 0.5 { abs_fs_y } else { 11.0 };

                let glyph_w_text =
                    (width_1000 / 1000.0) * gs.font_size * (gs.horiz_scale / 100.0);
                let tx_text = ((width_1000 / 1000.0) * gs.font_size
                    + gs.char_space
                    + if is_space { gs.word_space } else { 0.0 })
                    * (gs.horiz_scale / 100.0);

                let (ux0, uy0) = transform_pt(trm, 0.0, gs.rise);
                let (ux1, uy1) = transform_pt(trm, glyph_w_text, gs.rise);
                let (vx0, vy0) = self.to_visual_pt(ux0, uy0);
                let (vx1, vy1) = self.to_visual_pt(ux1, uy1);
                let x0 = vx0.min(vx1);
                let y_base = vy0;

                let tr = [1.0, 0.0, 0.0, 1.0, tx_text, 0.0];
                *tm = mat_mul(tr, *tm);

                let span_w = (vx1 - vx0).abs();
                let x1 = x0
                    + span_w.max(if decoded.trim().is_empty() {
                        1.5
                    } else {
                        abs_fs_x * 0.35
                    });

                if !decoded.is_empty() && !decoded.chars().all(|c| c.is_control()) {
                    if x1 >= -30.0
                        && x0 <= self.page_w + 30.0
                        && y_base >= -30.0
                        && y_base <= self.page_h + 30.0
                        && !Self::span_clipped(gs.clip_rect, x0, x1, y_base, eff_fs)
                    {
                        if !decoded.trim().is_empty() {
                            let dvx = vx1 - vx0;
                            let dvy = vy1 - vy0;
                            self.text_dir_vx += dvx;
                            self.text_dir_vy += dvy;
                            self.text_abs_vx += dvx.abs();
                            self.text_abs_vy += dvy.abs();
                        }
                        self.spans.push(PdfSpan {
                            text: decoded,
                            x0,
                            x1,
                            y_base,
                            font_size: eff_fs,
                            is_bold: table.is_bold,
                            is_italic: table.is_italic,
                            has_unmapped_pua: had_pua,
                            ocr_conf: None,
                        });
                    }
                }
                i += step.max(1);
            }
            return;
        }

        let (decoded, width_1000, had_pua) = table.decode_bytes(bytes);

        let trm = mat_mul(*tm, gs.ctm);
        let scale_x = (trm[0] * trm[0] + trm[1] * trm[1]).sqrt();
        let scale_y = (trm[2] * trm[2] + trm[3] * trm[3]).sqrt();
        let abs_fs_y = (gs.font_size * scale_y).abs();
        let abs_fs_x = (gs.font_size * scale_x).abs();
        let eff_fs = if abs_fs_y > 0.5 { abs_fs_y } else { 11.0 };

        let n_chars = if table.is_cid { (bytes.len() / 2).max(1) } else { bytes.len().max(1) };
        let spaces = if table.is_cid { decoded.chars().filter(|&c| c == ' ').count() } else { bytes.iter().filter(|&&b| b == b' ').count() };
        let tx_text = ((width_1000 / 1000.0) * gs.font_size
            + gs.char_space * (n_chars as f64)
            + gs.word_space * (spaces as f64))
            * (gs.horiz_scale / 100.0);

        let (ux0, uy0) = transform_pt(trm, 0.0, gs.rise);
        let (ux1, uy1) = transform_pt(trm, tx_text, gs.rise);
        let (vx0, vy0) = self.to_visual_pt(ux0, uy0);
        let (vx1, vy1) = self.to_visual_pt(ux1, uy1);
        let x0 = vx0.min(vx1);
        let y_base = vy0;

        let tr = [1.0, 0.0, 0.0, 1.0, tx_text, 0.0];
        *tm = mat_mul(tr, *tm);

        let span_w = (vx1 - vx0).abs();
        let x1 = x0 + span_w.max(if decoded.trim().is_empty() { 1.5 } else if decoded.trim().chars().count() == 1 { abs_fs_x * 0.18 } else { abs_fs_x * 0.35 });

        if !decoded.is_empty() && !decoded.chars().all(|c| c.is_control()) {
            if x1 >= -30.0
                && x0 <= self.page_w + 30.0
                && y_base >= -30.0
                && y_base <= self.page_h + 30.0
                && !Self::span_clipped(gs.clip_rect, x0, x1, y_base, eff_fs)
            {
                if !decoded.trim().is_empty() {
                    let dvx = vx1 - vx0;
                    let dvy = vy1 - vy0;
                    self.text_dir_vx += dvx;
                    self.text_dir_vy += dvy;
                    self.text_abs_vx += dvx.abs();
                    self.text_abs_vy += dvy.abs();
                }
                self.spans.push(PdfSpan {
                    text: decoded,
                    x0,
                    x1,
                    y_base,
                    font_size: eff_fs,
                    is_bold: table.is_bold,
                    is_italic: table.is_italic,
                    has_unmapped_pua: had_pua,
                    ocr_conf: None,
                });
            }
        }
    }

    fn resolve_font(
        &mut self,
        res_dicts: &[&lopdf::Dictionary],
        font_name: &[u8],
    ) -> FontDecodeTable {
        if font_name.is_empty() {
            return FontDecodeTable::default();
        }
        for res in res_dicts {
            let Ok(font_entry) = res.get(b"Font") else {
                continue;
            };
            let Some(font_map) = resolve_dict(self.doc, font_entry) else {
                continue;
            };
            let Ok(val) = font_map.get(font_name) else {
                continue;
            };
            if let Ok(oid) = val.as_reference() {
                if let Some(cached) = self.font_cache.get(&oid) {
                    return cached.clone();
                }
                if let Ok(fdict) = self.doc.get_dictionary(oid) {
                    let parsed = build_font_decode_table(self.doc, fdict);
                    self.font_cache.insert(oid, parsed.clone());
                    return parsed;
                }
            } else if let Object::Dictionary(fdict) = val {
                return build_font_decode_table(self.doc, fdict);
            }
        }
        FontDecodeTable::default()
    }

    fn handle_xobject(
        &mut self,
        gs: &GState,
        res_dicts: &[&lopdf::Dictionary],
        xname: &[u8],
        depth: usize,
    ) {
        for res in res_dicts {
            let Ok(xo_entry) = res.get(b"XObject") else {
                continue;
            };
            let Some(xo_map) = resolve_dict(self.doc, xo_entry) else {
                continue;
            };
            let Ok(val) = xo_map.get(xname) else {
                continue;
            };
            let oid = val.as_reference().ok();
            let stream = match val {
                Object::Stream(s) => s,
                Object::Reference(id) => match self.doc.get_object(*id) {
                    Ok(Object::Stream(s)) => s,
                    _ => continue,
                },
                _ => continue,
            };

            let subtype = stream.dict.get(b"Subtype").and_then(Object::as_name).ok();
            match subtype {
                Some(b"Form") => {
                    let form_matrix = stream
                        .dict
                        .get(b"Matrix")
                        .ok()
                        .and_then(|o| resolve_array(self.doc, o))
                        .and_then(|arr| read_matrix(arr))
                        .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                    let next_ctm = mat_mul(form_matrix, gs.ctm);
                    let mut child_res: Vec<&lopdf::Dictionary> = Vec::new();
                    if let Ok(r) = stream.dict.get(b"Resources") {
                        if let Some(d) = resolve_dict(self.doc, r) {
                            child_res.push(d);
                        }
                    }
                    child_res.extend_from_slice(res_dicts);
                    let spans_before = self.spans.len();
                    let imgs_before = self.images.len();
                    if let Ok(decompressed) = stream.decompressed_content() {
                        self.walk_stream_bytes(&decompressed, &child_res, next_ctm, gs.clip_rect, depth + 1);
                    } else if stream.dict.get(b"Filter").is_err() {
                        self.walk_stream_bytes(&stream.content, &child_res, next_ctm, gs.clip_rect, depth + 1);
                    }
                    if self.spans.len() > spans_before {
                        for img in &mut self.images[imgs_before..] {
                            img.in_text_form = true;
                        }
                    }
                    return;
                }
                Some(b"Image") => {
                    let decoded = if let Some(id) = oid {
                        if let Some(cached) = self.image_cache.get(&id) {
                            cached.clone()
                        } else {
                            let res = decode_pdf_image_to_data_uri(self.doc, stream);
                            self.image_cache.insert(id, res.clone());
                            res
                        }
                    } else {
                        decode_pdf_image_to_data_uri(self.doc, stream)
                    };

                    if let Some(img_dec) = decoded {
                        // Ảnh trong PDF luôn vẽ trên hình vuông đơn vị [0,1]x[0,1] biến đổi bởi CTM;
                        // nếu đã cắt viền trong suốt (uv_rect) thì co bounding box tương ứng.
                        let [u0, v0, u1, v1] = img_dec.uv_rect;
                        let corners = [
                            transform_pt(gs.ctm, u0, v0),
                            transform_pt(gs.ctm, u1, v0),
                            transform_pt(gs.ctm, u0, v1),
                            transform_pt(gs.ctm, u1, v1),
                        ];
                        let vcorners: Vec<(f64, f64)> = corners
                            .into_iter()
                            .map(|(cx, cy)| self.to_visual_pt(cx, cy))
                            .collect();
                        let x0 = vcorners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
                        let x1 = vcorners.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
                        let y0 = vcorners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
                        let y1 = vcorners.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);

                        self.images.push(PdfPlacedImage {
                            obj_id: oid,
                            x0,
                            y0,
                            x1,
                            y1,
                            px_w: img_dec.px_w,
                            px_h: img_dec.px_h,
                            is_swatch_smask: img_dec.is_swatch_smask,
                            in_text_form: false,
                            data_uri: img_dec.data_uri,
                        });
                    }
                    return;
                }
                _ => return,
            }
        }
    }
}

fn read_matrix(arr: &[Object]) -> Option<[f64; 6]> {
    if arr.len() < 6 {
        return None;
    }
    Some([
        obj_as_f64(&arr[0])?,
        obj_as_f64(&arr[1])?,
        obj_as_f64(&arr[2])?,
        obj_as_f64(&arr[3])?,
        obj_as_f64(&arr[4])?,
        obj_as_f64(&arr[5])?,
    ])
}

// ============================================================================
// 2. Giải mã Font, ToUnicode CMap, Glyph Names & Độ rộng ký tự
// ============================================================================

fn build_font_decode_table(doc: &Document, fdict: &lopdf::Dictionary) -> FontDecodeTable {
    let mut table = FontDecodeTable::default();

    let base_font = fdict
        .get(b"BaseFont")
        .and_then(Object::as_name)
        .map(|n| String::from_utf8_lossy(n).to_string())
        .unwrap_or_default();
    let bf_lower = base_font.to_ascii_lowercase();
    table.is_bold = bf_lower.contains("bold") || bf_lower.contains("-bd") || bf_lower.contains(",bold");
    table.is_italic = bf_lower.contains("italic") || bf_lower.contains("oblique");
    table.is_pua_custom_font = bf_lower.contains("mt extra")
        || bf_lower.contains("mtextra")
        || bf_lower.contains("mt-extra")
        || bf_lower.contains("euclidextra")
        || bf_lower.contains("euclid extra")
        || bf_lower.contains("wingding")
        || bf_lower.contains("webding");
    table.is_symbol_font = !table.is_pua_custom_font && bf_lower.contains("symbol");

    let subtype = fdict.get(b"Subtype").and_then(Object::as_name).ok();
    table.is_cid = matches!(subtype, Some(b"Type0"));

    // 1. Parse /ToUnicode CMap (luôn ưu tiên cao nhất nếu có)
    if let Ok(stream) = fdict.get_deref(b"ToUnicode", doc).and_then(Object::as_stream) {
        if let Ok(cmap_bytes) = stream
            .decompressed_content()
            .or_else(|_| Ok::<_, lopdf::Error>(stream.content.clone()))
        {
            parse_tounicode_cmap(&cmap_bytes, &mut table);
        }
    }

    if !table.to_unicode.is_empty() {
        let mut has_symbol_ops = false;
        let mut has_mtextra_triad = false;
        let mut has_ascii_alnum = false;
        for val in table.to_unicode.values() {
            for ch in val.chars() {
                if matches!(
                    ch,
                    '\u{F02B}'
                        | '\u{F02D}'
                        | '\u{F03C}'
                        | '\u{F03D}'
                        | '\u{F03E}'
                        | '\u{F0A2}'
                        | '\u{F0A3}'
                        | '\u{F0A5}'
                        | '\u{F0AE}'
                        | '\u{F0B1}'
                        | '\u{F0B3}'
                        | '\u{F0B9}'
                        | '\u{F0BB}'
                        | '\u{F0CE}'
                        | '\u{F0D6}'
                        | '\u{F0DB}'
                        | '\u{F0DE}'
                        | '\u{F0E5}'
                        | '\u{F0F2}'
                ) {
                    has_symbol_ops = true;
                }
                if matches!(ch, '\u{F072}' | '\u{F075}' | '\u{F0A1}') {
                    has_mtextra_triad = true;
                }
                if ch.is_ascii_alphanumeric() {
                    has_ascii_alnum = true;
                }
            }
        }
        if !table.is_pua_custom_font && has_symbol_ops {
            table.is_symbol_font = true;
        } else if (bf_lower.starts_with("cidfont+") || table.is_pua_custom_font)
            && !has_symbol_ops
            && !has_ascii_alnum
            && has_mtextra_triad
            && table.to_unicode.len() <= 24
        {
            table.is_cid_mtextra_subset = true;
            table.is_pua_custom_font = false;
        }
    }

    // 2. Nạp bảng ánh xạ 1-byte (/Encoding + /Differences hoặc Symbol)
    if table.is_symbol_font {
        for b in 0..=255u8 {
            table.single_byte[b as usize] = decode_symbol_byte(b);
        }
    } else {
        populate_single_byte_encoding(doc, fdict, &mut table.single_byte);
    }

    // 3. Nạp độ rộng ký tự (/Widths hoặc CIDFont /W)
    if table.is_cid {
        populate_cid_widths(doc, fdict, &mut table);
    } else {
        populate_simple_widths(doc, fdict, &mut table);
    }

    table
}

fn apply_winansi_table(out: &mut [Option<char>; 256]) {
    const CP1252_80_9F: [Option<char>; 32] = [
        Some('€'), None, Some('‚'), Some('ƒ'), Some('„'), Some('…'), Some('†'), Some('‡'),
        Some('ˆ'), Some('‰'), Some('Š'), Some('‹'), Some('Œ'), None, Some('Ž'), None,
        None, Some('‘'), Some('’'), Some('“'), Some('”'), Some('•'), Some('–'), Some('—'),
        Some('˜'), Some('™'), Some('š'), Some('›'), Some('œ'), None, Some('ž'), Some('Ÿ'),
    ];
    for b in 0..=255u8 {
        out[b as usize] = match b {
            0x20..=0x7E | 0xA0..=0xFF => char::from_u32(b as u32),
            0x80..=0x9F => CP1252_80_9F[(b - 0x80) as usize],
            _ => None,
        };
    }
}

fn populate_single_byte_encoding(
    doc: &Document,
    fdict: &lopdf::Dictionary,
    out: &mut [Option<char>; 256],
) {
    // Luôn khởi tạo WinAnsi chuẩn làm nền, sau đó ghi đè bằng lopdf encoding & /Differences nếu có
    apply_winansi_table(out);

    if let Ok(enc) = fdict.get_font_encoding(doc) {
        for b in 0..=255u8 {
            if let Ok(s) = Document::decode_text(&enc, &[b]) {
                if let Some(ch) = s.chars().next() {
                    if s.chars().count() == 1 {
                        out[b as usize] = Some(ch);
                    }
                }
            }
        }
    }

    let enc_obj = match fdict.get(b"Encoding") {
        Ok(o) => match o {
            Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        },
        Err(_) => None,
    };

    if let Some(Object::Dictionary(edict)) = enc_obj {
        if let Ok(diffs) = edict.get(b"Differences") {
            if let Some(arr) = resolve_array(doc, diffs) {
                let mut code: u8 = 0;
                for item in arr {
                    match item {
                        Object::Integer(c) if (0..=255).contains(c) => {
                            code = *c as u8;
                        }
                        Object::Name(gname) => {
                            if let Some(ch) = glyph_name_to_char(gname) {
                                out[code as usize] = Some(ch);
                            }
                            code = code.wrapping_add(1);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn glyph_name_to_char(name: &[u8]) -> Option<char> {
    let s = std::str::from_utf8(name).ok()?;
    if s.len() == 1 {
        return s.chars().next();
    }
    if let Some(hex) = s.strip_prefix("uni") {
        if hex.len() == 4 {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                return char::from_u32(cp);
            }
        }
    }
    if let Some(hex) = s.strip_prefix('u') {
        if (4..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                return char::from_u32(cp);
            }
        }
    }
    match s {
        "dcroat" => Some('đ'),
        "Dcroat" | "Eth" => Some('Đ'),
        "abreve" => Some('ă'),
        "Abreve" => Some('Ă'),
        "ohorn" => Some('ơ'),
        "Ohorn" => Some('Ơ'),
        "uhorn" => Some('ư'),
        "Uhorn" => Some('Ư'),
        "minus" => Some('−'),
        "bullet" => Some('•'),
        "endash" => Some('–'),
        "emdash" => Some('—'),
        _ => None,
    }
}

fn parse_tounicode_cmap(data: &[u8], table: &mut FontDecodeTable) {
    let text = String::from_utf8_lossy(data);
    let mut min_len = 4u8;
    let mut max_len = 1u8;

    let parse_hex_bytes = |token: &str| -> Option<Vec<u8>> {
        let inner = token.trim().strip_prefix('<')?.strip_suffix('>')?;
        let clean: String = inner.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if clean.is_empty() || clean.len() % 2 != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(clean.len() / 2);
        for i in (0..clean.len()).step_by(2) {
            out.push(u8::from_str_radix(&clean[i..i + 2], 16).ok()?);
        }
        Some(out)
    };

    let decode_utf16_be_bytes = |b: &[u8]| -> String {
        let mut units = Vec::with_capacity(b.len() / 2);
        for pair in b.chunks_exact(2) {
            units.push(((pair[0] as u16) << 8) | (pair[1] as u16));
        }
        char::decode_utf16(units).flatten().collect()
    };

    let re_tokens = Regex::new(r#"<[^>]*>|\[|\]|\S+"#).unwrap();
    let tokens: Vec<&str> = re_tokens.find_iter(&text).map(|m| m.as_str()).collect();
    let mut idx = 0usize;

    while idx < tokens.len() {
        match tokens[idx] {
            "begincodespacerange" => {
                idx += 1;
                while idx < tokens.len() && tokens[idx] != "endcodespacerange" {
                    if let (Some(lo), Some(_hi)) = (
                        parse_hex_bytes(tokens[idx]),
                        tokens.get(idx + 1).and_then(|t| parse_hex_bytes(t)),
                    ) {
                        let clen = (lo.len() as u8).clamp(1, 4);
                        min_len = min_len.min(clen);
                        max_len = max_len.max(clen);
                        idx += 2;
                    } else {
                        idx += 1;
                    }
                }
            }
            "beginbfchar" => {
                idx += 1;
                while idx < tokens.len() && tokens[idx] != "endbfchar" {
                    if let (Some(src), Some(dst)) = (
                        parse_hex_bytes(tokens[idx]),
                        tokens.get(idx + 1).and_then(|t| parse_hex_bytes(t)),
                    ) {
                        let clen = (src.len() as u8).clamp(1, 4);
                        min_len = min_len.min(clen);
                        max_len = max_len.max(clen);
                        let code = src.iter().fold(0u32, |acc, &b| (acc << 8) | (b as u32));
                        let s = decode_utf16_be_bytes(&dst);
                        if !s.is_empty() {
                            table.to_unicode.insert((clen, code), s);
                        }
                        idx += 2;
                    } else {
                        idx += 1;
                    }
                }
            }
            "beginbfrange" => {
                idx += 1;
                while idx < tokens.len() && tokens[idx] != "endbfrange" {
                    let Some(src_lo) = parse_hex_bytes(tokens[idx]) else {
                        idx += 1;
                        continue;
                    };
                    let Some(src_hi) = tokens.get(idx + 1).and_then(|t| parse_hex_bytes(t)) else {
                        idx += 1;
                        continue;
                    };
                    let clen = (src_lo.len() as u8).clamp(1, 4);
                    min_len = min_len.min(clen);
                    max_len = max_len.max(clen);
                    let start = src_lo.iter().fold(0u32, |acc, &b| (acc << 8) | (b as u32));
                    let end = src_hi.iter().fold(0u32, |acc, &b| (acc << 8) | (b as u32));
                    idx += 2;

                    if tokens.get(idx) == Some(&"[") {
                        idx += 1;
                        let mut cur = start;
                        while idx < tokens.len() && tokens[idx] != "]" {
                            if let Some(dst) = parse_hex_bytes(tokens[idx]) {
                                if cur <= end {
                                    let s = decode_utf16_be_bytes(&dst);
                                    if !s.is_empty() {
                                        table.to_unicode.insert((clen, cur), s);
                                    }
                                    cur = cur.saturating_add(1);
                                }
                            }
                            idx += 1;
                        }
                        if tokens.get(idx) == Some(&"]") {
                            idx += 1;
                        }
                    } else if let Some(mut dst) = tokens.get(idx).and_then(|t| parse_hex_bytes(t)) {
                        idx += 1;
                        if end >= start && (end - start) <= 4096 {
                            for code in start..=end {
                                let s = decode_utf16_be_bytes(&dst);
                                if !s.is_empty() {
                                    table.to_unicode.insert((clen, code), s);
                                }
                                let dlen = dst.len();
                                if dlen >= 2 && dst[dlen - 1] == 0xFF {
                                    dst[dlen - 1] = 0;
                                    dst[dlen - 2] = dst[dlen - 2].wrapping_add(1);
                                } else if dlen >= 1 {
                                    dst[dlen - 1] = dst[dlen - 1].wrapping_add(1);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    if min_len <= max_len {
        table.min_code_len = min_len;
        table.max_code_len = max_len;
    } else if table.is_cid {
        table.min_code_len = 2;
        table.max_code_len = 2;
    }
}

fn populate_simple_widths(doc: &Document, fdict: &lopdf::Dictionary, table: &mut FontDecodeTable) {
    let first_char = fdict
        .get(b"FirstChar")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0)
        .max(0) as u32;
    if let Ok(w_obj) = fdict.get(b"Widths") {
        if let Some(arr) = resolve_array(doc, w_obj) {
            for (i, item) in arr.iter().enumerate() {
                if let Some(w) = obj_as_f64(item) {
                    table.widths.insert(first_char + i as u32, w);
                }
            }
        }
    }
    if let Ok(fd_obj) = fdict.get(b"FontDescriptor") {
        if let Some(fd) = resolve_dict(doc, fd_obj) {
            if let Ok(mw) = fd.get(b"MissingWidth") {
                if let Some(v) = obj_as_f64(mw) {
                    if v > 0.0 {
                        table.default_width = v;
                    }
                }
            }
        }
    }
}

fn populate_cid_widths(doc: &Document, fdict: &lopdf::Dictionary, table: &mut FontDecodeTable) {
    let Ok(desc_obj) = fdict.get(b"DescendantFonts") else {
        return;
    };
    let Some(desc_arr) = resolve_array(doc, desc_obj) else {
        return;
    };
    let Some(first_desc) = desc_arr.first().and_then(|o| resolve_dict(doc, o)) else {
        return;
    };
    if let Ok(dw) = first_desc.get(b"DW") {
        if let Some(v) = obj_as_f64(dw) {
            if v > 0.0 {
                table.default_width = v;
            }
        }
    }
    if let Ok(w_obj) = first_desc.get(b"W") {
        if let Some(arr) = resolve_array(doc, w_obj) {
            let mut i = 0usize;
            while i < arr.len() {
                let Ok(c_first) = arr[i].as_i64() else {
                    i += 1;
                    continue;
                };
                if let Some(next) = arr.get(i + 1) {
                    if let Some(wlist) = resolve_array(doc, next) {
                        for (off, witem) in wlist.iter().enumerate() {
                            if let Some(w) = obj_as_f64(witem) {
                                table.widths.insert((c_first as u32) + off as u32, w);
                            }
                        }
                        i += 2;
                        continue;
                    } else if let (Ok(c_last), Some(w_val)) =
                        (next.as_i64(), arr.get(i + 2).and_then(obj_as_f64))
                    {
                        if c_last >= c_first && (c_last - c_first) <= 8192 {
                            for cid in c_first..=c_last {
                                table.widths.insert(cid as u32, w_val);
                            }
                        }
                        i += 3;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
}

// ============================================================================
// 3. Giải mã XObject /Image (JPEG DCTDecode & FlateDecode -> PNG)
// ============================================================================

fn decode_pdf_image_to_data_uri(doc: &Document, stream: &Stream) -> Option<DecodedPdfImage> {
    let w = stream.dict.get(b"Width").ok()?.as_i64().ok()? as u32;
    let h = stream.dict.get(b"Height").ok()?.as_i64().ok()? as u32;
    if w == 0 || h == 0 || w > 8000 || h > 8000 {
        return None;
    }

    let filters: Vec<Vec<u8>> = stream
        .filters()
        .map(|v| v.into_iter().map(|s| s.to_vec()).collect())
        .unwrap_or_default();

    // Ảnh JPEG nguyên bản (/DCTDecode)
    if filters.iter().any(|f| f == b"DCTDecode") {
        let jpeg_bytes = if filters.len() == 1 {
            stream.content.clone()
        } else {
            return None;
        };
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
        return Some(DecodedPdfImage {
            px_w: w,
            px_h: h,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            is_swatch_smask: false,
            data_uri: format!("data:image/jpeg;base64,{}", b64),
        });
    }

    // Các bộ lọc khác giải nén bằng lopdf (FlateDecode, LZW, ASCII85, RunLength, DecodeParms Predictor...)
    let raw_pixels = if filters.is_empty() {
        stream.content.clone()
    } else {
        stream.decompressed_content().ok()?
    };

    let bpc = stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(8) as u8;

    let cs_kind = resolve_colorspace(doc, stream);
    let rgb_pixels = convert_samples_to_rgb(&raw_pixels, w, h, bpc, &cs_kind)?;

    // Kiểm tra kênh Alpha rời (/SMask)
    let smask_alpha = stream
        .dict
        .get(b"SMask")
        .ok()
        .and_then(|o| match o {
            Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        })
        .and_then(|o| match o {
            Object::Stream(sm) => decode_smask_alpha(doc, sm),
            _ => None,
        });

    let (out_w, out_h, uv_rect, is_swatch_smask, png_bytes) = if let Some((sw, sh, alpha)) = smask_alpha {
        let is_swatch = w <= 8 && h <= 8 && sw >= 16 && sh >= 16;
        let (cx0, cy0, cx1, cy1, uv) = if is_swatch {
            let mut min_x = sw;
            let mut max_x = 0u32;
            let mut min_y = sh;
            let mut max_y = 0u32;
            let mut found = false;
            for y in 0..sh {
                let row_off = (y * sw) as usize;
                for x in 0..sw {
                    if alpha[row_off + x as usize] > 16 {
                        found = true;
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                        min_y = min_y.min(y);
                        max_y = max_y.max(y);
                    }
                }
            }
            if !found {
                return None;
            }
            let x0 = min_x.saturating_sub(2);
            let y0 = min_y.saturating_sub(2);
            let x1 = (max_x + 2).min(sw - 1);
            let y1 = (max_y + 2).min(sh - 1);
            let uv = [
                x0 as f64 / sw as f64,
                1.0 - (y1 + 1) as f64 / sh as f64,
                (x1 + 1) as f64 / sw as f64,
                1.0 - y0 as f64 / sh as f64,
            ];
            (x0, y0, x1, y1, uv)
        } else {
            (0, 0, sw - 1, sh - 1, [0.0, 0.0, 1.0, 1.0])
        };

        let cw = cx1 - cx0 + 1;
        let ch = cy1 - cy0 + 1;
        let mut rgba = Vec::with_capacity((cw * ch * 4) as usize);
        let same_dims = sw == w && sh == h;
        for y in cy0..=cy1 {
            let sy = if same_dims {
                y as usize
            } else {
                ((y as u64 * h as u64) / sh as u64) as usize
            };
            let a_row = (y * sw) as usize;
            let rgb_row = sy * (w as usize);
            for x in cx0..=cx1 {
                let sx = if same_dims {
                    x as usize
                } else {
                    ((x as u64 * w as u64) / sw as u64) as usize
                };
                let off = (rgb_row + sx) * 3;
                rgba.push(rgb_pixels[off]);
                rgba.push(rgb_pixels[off + 1]);
                rgba.push(rgb_pixels[off + 2]);
                rgba.push(alpha[a_row + x as usize]);
            }
        }
        (cw, ch, uv, is_swatch, encode_png(cw, ch, true, &rgba)?)
    } else {
        (w, h, [0.0, 0.0, 1.0, 1.0], false, encode_png(w, h, false, &rgb_pixels)?)
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Some(DecodedPdfImage {
        px_w: out_w,
        px_h: out_h,
        uv_rect,
        is_swatch_smask,
        data_uri: format!("data:image/png;base64,{}", b64),
    })
}

enum PdfColorSpace {
    Gray,
    Rgb,
    Cmyk,
    Indexed { base_channels: usize, palette: Vec<u8> },
}

fn resolve_colorspace(doc: &Document, stream: &Stream) -> PdfColorSpace {
    let cs_obj = match stream.dict.get(b"ColorSpace") {
        Ok(Object::Reference(id)) => doc.get_object(*id).ok(),
        Ok(other) => Some(other),
        Err(_) => None,
    };

    match cs_obj {
        Some(Object::Name(n)) => match n.as_slice() {
            b"DeviceGray" | b"CalGray" => PdfColorSpace::Gray,
            b"DeviceCMYK" => PdfColorSpace::Cmyk,
            _ => PdfColorSpace::Rgb,
        },
        Some(Object::Array(arr)) => {
            let head = arr.first().and_then(|o| o.as_name().ok()).unwrap_or(b"");
            match head {
                b"ICCBased" => {
                    if let Some(prof_stream) = arr.get(1).and_then(|o| match o {
                        Object::Reference(id) => match doc.get_object(*id) {
                            Ok(Object::Stream(s)) => Some(s),
                            _ => None,
                        },
                        Object::Stream(s) => Some(s),
                        _ => None,
                    }) {
                        let n = prof_stream
                            .dict
                            .get(b"N")
                            .ok()
                            .and_then(|o| o.as_i64().ok())
                            .unwrap_or(3);
                        return match n {
                            1 => PdfColorSpace::Gray,
                            4 => PdfColorSpace::Cmyk,
                            _ => PdfColorSpace::Rgb,
                        };
                    }
                    PdfColorSpace::Rgb
                }
                b"Indexed" => {
                    let base_name = arr.get(1).and_then(|o| match o {
                        Object::Name(n) => Some(n.as_slice()),
                        Object::Array(sub) => sub.first().and_then(|s| s.as_name().ok()),
                        _ => None,
                    });
                    let base_channels = match base_name {
                        Some(b"DeviceGray") | Some(b"CalGray") => 1,
                        Some(b"DeviceCMYK") => 4,
                        _ => 3,
                    };
                    let palette = match arr.get(3) {
                        Some(Object::String(bytes, _)) => bytes.clone(),
                        Some(Object::Stream(s)) => s.decompressed_content().unwrap_or_else(|_| s.content.clone()),
                        Some(Object::Reference(id)) => match doc.get_object(*id) {
                            Ok(Object::Stream(s)) => {
                                s.decompressed_content().unwrap_or_else(|_| s.content.clone())
                            }
                            Ok(Object::String(bytes, _)) => bytes.clone(),
                            _ => Vec::new(),
                        },
                        _ => Vec::new(),
                    };
                    PdfColorSpace::Indexed {
                        base_channels,
                        palette,
                    }
                }
                b"CalGray" => PdfColorSpace::Gray,
                b"CalRGB" => PdfColorSpace::Rgb,
                _ => PdfColorSpace::Rgb,
            }
        }
        _ => PdfColorSpace::Rgb,
    }
}

fn decode_smask_alpha(doc: &Document, sm: &Stream) -> Option<(u32, u32, Vec<u8>)> {
    let sw = sm.dict.get(b"Width").ok()?.as_i64().ok()? as u32;
    let sh = sm.dict.get(b"Height").ok()?.as_i64().ok()? as u32;
    if sw == 0
        || sh == 0
        || sw > 8000
        || sh > 8000
        || (sw as u64) * (sh as u64) > 16_000_000
    {
        return None;
    }
    let bpc = sm
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(8) as u8;
    let invert = sm
        .dict
        .get(b"Decode")
        .ok()
        .and_then(|o| resolve_array(doc, o))
        .and_then(
            |arr| match (arr.first().and_then(obj_as_f64), arr.get(1).and_then(obj_as_f64)) {
                (Some(d0), Some(d1)) => Some(d0 > d1),
                _ => None,
            },
        )
        .unwrap_or(false);
    let data = if sm.filters().is_err() {
        sm.content.clone()
    } else {
        sm.decompressed_content().ok()?
    };
    let mut alpha = Vec::with_capacity((sw * sh) as usize);
    unpack_samples(&data, sw, sh, 1, bpc, |vals| {
        let mut v = if bpc == 1 {
            vals[0] * 255
        } else if bpc == 4 {
            vals[0] * 17
        } else if bpc == 2 {
            vals[0] * 85
        } else {
            vals[0]
        };
        if invert {
            v = 255 - v;
        }
        alpha.push(v);
    });
    if alpha.len() == (sw * sh) as usize {
        Some((sw, sh, alpha))
    } else {
        None
    }
}

fn unpack_samples<F>(
    raw: &[u8],
    w: u32,
    h: u32,
    comps: usize,
    bpc: u8,
    mut on_pixel: F,
) where
    F: FnMut(&[u8]),
{
    let bits_per_row = (w as usize) * comps * (bpc as usize);
    let row_stride = (bits_per_row + 7) / 8;
    let mut vals = [0u8; 4];

    for row in 0..(h as usize) {
        let row_start = row * row_stride;
        if row_start >= raw.len() {
            break;
        }
        let row_slice = &raw[row_start..(row_start + row_stride).min(raw.len())];
        if bpc == 8 {
            for col in 0..(w as usize) {
                let off = col * comps;
                if off + comps <= row_slice.len() {
                    for c in 0..comps {
                        vals[c] = row_slice[off + c];
                    }
                    on_pixel(&vals[..comps]);
                } else {
                    on_pixel(&[0; 4][..comps]);
                }
            }
        } else {
            let mask = (1u32 << bpc) - 1;
            let mut bit_pos = 0usize;
            for _col in 0..(w as usize) {
                for c in 0..comps {
                    let byte_idx = bit_pos / 8;
                    let shift = 8 - (bpc as usize) - (bit_pos % 8);
                    let sample = if byte_idx < row_slice.len() {
                        ((row_slice[byte_idx] as u32) >> shift) & mask
                    } else {
                        0
                    };
                    vals[c] = sample as u8;
                    bit_pos += bpc as usize;
                }
                on_pixel(&vals[..comps]);
            }
        }
    }
}

fn convert_samples_to_rgb(
    raw: &[u8],
    w: u32,
    h: u32,
    bpc: u8,
    cs: &PdfColorSpace,
) -> Option<Vec<u8>> {
    if (w as u64) * (h as u64) > 16_000_000 {
        return None;
    }
    let mut pixels = Vec::with_capacity((w * h) as usize * 3);

    let mut push_rgb = |r: u8, g: u8, b: u8| {
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
    };

    let scale = |v: u8| -> u8 {
        match bpc {
            1 => if v > 0 { 255 } else { 0 },
            2 => v.saturating_mul(85),
            4 => v.saturating_mul(17),
            _ => v,
        }
    };

    match cs {
        PdfColorSpace::Gray => {
            unpack_samples(raw, w, h, 1, bpc, |s| {
                let v = scale(s[0]);
                push_rgb(v, v, v);
            });
        }
        PdfColorSpace::Rgb => {
            unpack_samples(raw, w, h, 3, bpc, |s| {
                push_rgb(scale(s[0]), scale(s[1]), scale(s[2]));
            });
        }
        PdfColorSpace::Cmyk => {
            unpack_samples(raw, w, h, 4, bpc, |s| {
                let c = scale(s[0]) as f64 / 255.0;
                let m = scale(s[1]) as f64 / 255.0;
                let y = scale(s[2]) as f64 / 255.0;
                let k = scale(s[3]) as f64 / 255.0;
                let r = ((1.0 - c) * (1.0 - k) * 255.0).round().clamp(0.0, 255.0) as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0).round().clamp(0.0, 255.0) as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0).round().clamp(0.0, 255.0) as u8;
                push_rgb(r, g, b);
            });
        }
        PdfColorSpace::Indexed {
            base_channels,
            palette,
        } => {
            unpack_samples(raw, w, h, 1, bpc, |s| {
                let idx = (s[0] as usize) * *base_channels;
                if *base_channels == 1 && idx < palette.len() {
                    let v = palette[idx];
                    push_rgb(v, v, v);
                } else if *base_channels >= 3 && idx + 2 < palette.len() {
                    push_rgb(palette[idx], palette[idx + 1], palette[idx + 2]);
                } else {
                    push_rgb(255, 255, 255);
                }
            });
        }
    }

    if pixels.len() < (w * h) as usize * 3 {
        pixels.resize((w * h) as usize * 3, 255);
    }

    Some(pixels)
}

fn encode_png(w: u32, h: u32, rgba: bool, pixels: &[u8]) -> Option<Vec<u8>> {
    let ch = if rgba { 4 } else { 3 };
    let stride = (w as usize) * ch;
    if pixels.len() < stride * (h as usize) {
        return None;
    }

    let mut scanlines = Vec::with_capacity((stride + 1) * (h as usize));
    for y in 0..(h as usize) {
        scanlines.push(0u8); // Filter type 0 (None)
        scanlines.extend_from_slice(&pixels[y * stride..(y + 1) * stride]);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&scanlines).ok()?;
    let idat_data = encoder.finish().ok()?;

    let mut png = Vec::with_capacity(64 + idat_data.len());
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(if rgba { 6 } else { 2 }); // color type 6=RGBA, 2=RGB
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &idat_data);
    write_png_chunk(&mut png, b"IEND", &[]);
    Some(png)
}

fn write_png_chunk(out: &mut Vec<u8>, ctype: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ctype);
    out.extend_from_slice(data);
    let mut crc = 0xFFFF_FFFFu32;
    for &b in ctype.iter().chain(data.iter()) {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

// ============================================================================
// 4. Dựng toán học 2D (Phân số, Chỉ số trên/dưới) & Lắp ráp dòng văn bản
// ============================================================================

#[derive(Debug, Clone)]
enum LineToken {
    Span(PdfSpan),
    Math {
        html: String,
        raw: String,
        x0: f64,
        x1: f64,
        y_base: f64,
        font_size: f64,
    },
    InlineImg {
        html: String,
        x0: f64,
        x1: f64,
        y_base: f64,
        font_size: f64,
        method: ExtractionMethod,
        needs_review: bool,
        review_reason: Option<String>,
    },
}

impl LineToken {
    fn x0(&self) -> f64 {
        match self {
            Self::Span(s) => s.x0,
            Self::Math { x0, .. } | Self::InlineImg { x0, .. } => *x0,
        }
    }
    fn x1(&self) -> f64 {
        match self {
            Self::Span(s) => s.x1,
            Self::Math { x1, .. } | Self::InlineImg { x1, .. } => *x1,
        }
    }
    fn y_base(&self) -> f64 {
        match self {
            Self::Span(s) => s.y_base,
            Self::Math { y_base, .. } | Self::InlineImg { y_base, .. } => *y_base,
        }
    }
    fn font_size(&self) -> f64 {
        match self {
            Self::Span(s) => s.font_size,
            Self::Math { font_size, .. } | Self::InlineImg { font_size, .. } => *font_size,
        }
    }
}

#[derive(Debug, Clone)]
struct PdfPageElement {
    y_top: f64,
    para: ParagraphData,
}

fn is_tall_delimiter(t: &str) -> bool {
    matches!(t.trim(), "(" | ")" | "[" | "]" | "{" | "}" | "|")
}

fn is_protected_marker_span(t: &str) -> bool {
    let tr = t.trim();
    tr.starts_with("Câu")
        || tr.starts_with("PHẦN")
        || tr.starts_with("PART")
        || matches!(tr, "A." | "B." | "C." | "D." | "a)" | "b)" | "c)" | "d)")
}

fn build_page_elements(page: &PdfPageData) -> Vec<PdfPageElement> {
    let mut consumed = vec![false; page.spans.len()];
    let mut tokens: Vec<LineToken> = Vec::new();
    let mut block_elements: Vec<PdfPageElement> = Vec::new();

    // 0. Vùng ảnh cắt từ PUA / đồ thị vector: tiêu thụ spans & nét bên trong để không vỡ công thức hoặc trùng lặp nhãn trục
    for reg in &page.cropped_regions {
        let is_vec = reg.method == ExtractionMethod::PdfVectorCrop;
        let pad_x = if is_vec { 8.0 } else { 1.5 };
        let pad_y = if is_vec { 6.0 } else { 2.0 };
        for (i, sp) in page.spans.iter().enumerate() {
            if consumed[i] {
                continue;
            }
            let sp_cx = 0.5 * (sp.x0 + sp.x1);
            let sp_cy = sp.y_base - sp.font_size * 0.4;
            let strict_inside = sp_cx >= reg.x0 - 1.5
                && sp_cx <= reg.x1 + 1.5
                && sp_cy >= reg.y0 - 2.0
                && sp_cy <= reg.y1 + 2.0;
            let expanded_inside = sp_cx >= reg.x0 - pad_x
                && sp_cx <= reg.x1 + pad_x
                && sp_cy >= reg.y0 - pad_y
                && sp_cy <= reg.y1 + pad_y;
            if is_vec {
                let tr = sp.text.trim();
                let short_label = tr.chars().count() <= 18 && (sp.x1 - sp.x0) <= 95.0;
                if !is_protected_marker_span(tr) && (strict_inside || (expanded_inside && short_label)) {
                    consumed[i] = true;
                }
            } else if strict_inside && !is_protected_marker_span(sp.text.as_str()) {
                consumed[i] = true;
            }
        }

        let dw = (reg.x1 - reg.x0).abs().max(4.0);
        let dh = (reg.y1 - reg.y0).abs().max(4.0);
        let nat_w_px = (dw * 96.0 / 72.0).max(1.0);
        let nat_h_px = (dh * 96.0 / 72.0).max(1.0);
        if reg.is_inline {
            let h_px = nat_h_px.clamp(12.0, 38.0);
            let scale = if nat_h_px > 4.0 {
                (h_px / nat_h_px).min(1.35)
            } else {
                1.0
            };
            let w_px = (nat_w_px * scale).max(6.0);
            let cls = if reg.needs_review {
                "math-formula needs-review"
            } else {
                "math-formula"
            };
            let review_attr = if reg.needs_review {
                r#" data-needs-review="true""#
            } else {
                ""
            };
            let alt = if reg.needs_review {
                "Công thức cần kiểm tra"
            } else {
                "Công thức"
            };
            let html = format!(
                r#"<img src="{}" class="{}"{} style="width: {:.1}px; height: {:.1}px; vertical-align: middle;" alt="{}">"#,
                reg.data_uri, cls, review_attr, w_px, h_px, alt
            );
            tokens.push(LineToken::InlineImg {
                html,
                x0: reg.x0,
                x1: reg.x1,
                y_base: 0.5 * (reg.y0 + reg.y1) + 4.0,
                font_size: 11.0,
                method: reg.method,
                needs_review: reg.needs_review,
                review_reason: reg.review_reason.clone(),
            });
        } else {
            let html = if reg.method == ExtractionMethod::ImageCropFallback {
                let max_w_px = nat_w_px.clamp(48.0, 560.0);
                format!(
                    r#"<img src="{}" class="math-formula math-block needs-review" data-needs-review="true" style="max-width: min(100%, {:.1}px); height: auto;" alt="Hệ công thức cần kiểm tra">"#,
                    reg.data_uri, max_w_px
                )
            } else {
                let cx_emu = dw * 12700.0;
                let cy_emu = dh * 12700.0;
                render_drawing_figure_img(&reg.data_uri, Some(cx_emu), Some(cy_emu))
            };
            let reasons = reg.review_reason.iter().cloned().collect();
            block_elements.push(PdfPageElement {
                y_top: reg.y0,
                para: ParagraphData {
                    html,
                    raw: String::new(),
                    provenance: vec![BlockProvenance {
                        source_file: None,
                        page: page.page_num_1based,
                        bbox: Some(BoundingBox {
                            x0: (reg.x0 * 10.0).round() / 10.0,
                            y0: (reg.y0 * 10.0).round() / 10.0,
                            x1: (reg.x1 * 10.0).round() / 10.0,
                            y1: (reg.y1 * 10.0).round() / 10.0,
                        }),
                        method: reg.method,
                        needs_review: reg.needs_review,
                        review_reasons: reasons,
                    }],
                },
            });
        }
    }

    // 1. Dựng phân số 2D từ các nét gạch ngang + span tử số (trên) & mẫu số (dưới)
    let mut bars: Vec<&PdfHLine> = page
        .h_lines
        .iter()
        .filter(|h| {
            let len = (h.x1 - h.x0).abs();
            if !(4.0..=185.0).contains(&len) {
                return false;
            }
            // Không xử lý nét nằm trong vùng ảnh crop (đồ thị/hệ ngoặc/PUA)
            !page.cropped_regions.iter().any(|r| {
                h.x0 >= r.x0 - 4.0 && h.x1 <= r.x1 + 4.0 && h.y >= r.y0 - 4.0 && h.y <= r.y1 + 4.0
            })
        })
        .collect();
    bars.sort_by(|a, b| (a.x1 - a.x0).partial_cmp(&(b.x1 - b.x0)).unwrap_or(std::cmp::Ordering::Equal));

    for bar in bars.iter().copied() {
        // Kiểm tra nét gạch có phải viền bảng kín nối liền nét dọc hoặc nối tiếp với đoạn kẻ bảng ngang cùng hàng y
        let touches_v_left = page
            .v_lines
            .iter()
            .any(|v| (v.x - bar.x0).abs() <= 3.2 && bar.y >= v.y0 - 3.2 && bar.y <= v.y1 + 3.2);
        let touches_v_right = page
            .v_lines
            .iter()
            .any(|v| (v.x - bar.x1).abs() <= 3.2 && bar.y >= v.y0 - 3.2 && bar.y <= v.y1 + 3.2);
        let collinear_table_rule = page.h_lines.iter().any(|other| {
            !std::ptr::eq(other, bar)
                && (other.y - bar.y).abs() <= 1.2
                && ((other.x1 - bar.x0).abs() <= 2.5 || (other.x0 - bar.x1).abs() <= 2.5)
        });
        if (touches_v_left && touches_v_right)
            || ((touches_v_left || touches_v_right) && collinear_table_rule)
            || collinear_table_rule
        {
            continue;
        }

        let bar_w = (bar.x1 - bar.x0).abs();
        let pad_x = (bar_w * 0.10).clamp(1.0, 3.0);

        let mut num_idxs = Vec::new();
        let mut den_idxs = Vec::new();

        for (i, sp) in page.spans.iter().enumerate() {
            if consumed[i] || sp.has_unmapped_pua || sp.text.trim().is_empty() {
                continue;
            }
            if is_protected_marker_span(sp.text.as_str()) {
                continue;
            }
            let sp_cx = 0.5 * (sp.x0 + sp.x1);
            let sp_w = (sp.x1 - sp.x0).max(0.5);
            let h_overlap = sp.x1.min(bar.x1 + pad_x) - sp.x0.max(bar.x0 - pad_x);
            let inside_x = (sp_cx >= bar.x0 - pad_x && sp_cx <= bar.x1 + pad_x)
                || (h_overlap / sp_w >= 0.45 && sp_w <= bar_w + 8.0);
            let not_much_wider = sp_w <= bar_w + 10.0;
            if !inside_x || !not_much_wider {
                continue;
            }

            // Chặn hút nhầm chữ của dòng văn xuôi liên tục nằm phía trên/dưới nét gạch phân số
            let on_wide_prose_baseline = sp.font_size >= 10.0
                && page.spans.iter().enumerate().any(|(oi, other)| {
                    oi != i
                        && !consumed[oi]
                        && other.font_size >= 10.0
                        && (other.y_base - sp.y_base).abs() <= 1.2
                        && (other.x1 < bar.x0 - 20.0 || other.x0 > bar.x1 + 20.0)
                        && other.text.chars().any(|c| c.is_alphabetic())
                        && !bars.iter().any(|b| {
                            let ocx = 0.5 * (other.x0 + other.x1);
                            ocx >= b.x0 - 4.0
                                && ocx <= b.x1 + 4.0
                                && other.y_base >= b.y - 16.5
                                && other.y_base <= b.y + 19.0
                        })
                });
            if on_wide_prose_baseline {
                continue;
            }

            // Tử số nằm ngay phía trên nét gạch ngang (PDF top-down: y_base nhỏ hơn bar.y)
            let is_valid_num_y = sp.y_base <= bar.y - 0.8
                && ((sp.y_base >= bar.y - 13.0) || (sp.y_base >= bar.y - 16.5 && sp.font_size <= 9.5));
            let is_valid_den_y = sp.y_base > bar.y + 1.0 && sp.y_base <= bar.y + 19.0;
            if is_valid_num_y {
                num_idxs.push(i);
            } else if is_valid_den_y {
                den_idxs.push(i);
            }
        }

        if num_idxs.is_empty() || den_idxs.is_empty() {
            continue;
        }

        let num_plain: String = sorted_spans_text(&page.spans, &num_idxs);
        let den_plain: String = sorted_spans_text(&page.spans, &den_idxs);
        if num_plain.trim().is_empty()
            || den_plain.trim().is_empty()
            || num_plain.chars().count() > 42
            || den_plain.chars().count() > 42
        {
            continue;
        }

        for &i in num_idxs.iter().chain(den_idxs.iter()) {
            consumed[i] = true;
        }

        // Tìm baseline tham chiếu gần nhất trái/phải nét gạch ngang để neo đúng dòng
        let mut anchor_y = bar.y + 3.5;
        let mut best_dist = 24.0_f64;
        for (i, sp) in page.spans.iter().enumerate() {
            if consumed[i] || sp.text.trim().is_empty() {
                continue;
            }
            if (sp.y_base - (bar.y + 3.0)).abs() <= 10.0 {
                let dist = if sp.x1 <= bar.x0 {
                    bar.x0 - sp.x1
                } else if sp.x0 >= bar.x1 {
                    sp.x0 - bar.x1
                } else {
                    continue;
                };
                if dist < best_dist {
                    best_dist = dist;
                    anchor_y = sp.y_base;
                }
            }
        }

        let num_mml = build_mrow_mathml(&page.spans, &num_idxs);
        let den_mml = build_mrow_mathml(&page.spans, &den_idxs);
        let mml = format!(
            r#"<math xmlns="http://www.w3.org/1998/Math/MathML" class="omml-math"><mfrac>{}{}</mfrac></math>"#,
            num_mml, den_mml
        );
        let raw = format!("{}/{}", num_plain.trim(), den_plain.trim());
        tokens.push(LineToken::Math {
            html: mml,
            raw,
            x0: bar.x0,
            x1: bar.x1,
            y_base: anchor_y,
            font_size: 11.0,
        });
    }

    // 1.5. Ghép các toán tử có chỉ số dưới (`lim`, `max`, `min` + `x→1` nằm ngay phía dưới)
    for i in 0..page.spans.len() {
        if consumed[i] {
            continue;
        }
        let op_sp = &page.spans[i];
        let op_t = op_sp.text.trim();
        let op_low = op_t.to_ascii_lowercase();
        if !matches!(op_low.as_str(), "lim" | "max" | "min") || op_sp.font_size < 9.0 {
            continue;
        }
        let mut sub_idxs = Vec::new();
        for (j, sp) in page.spans.iter().enumerate() {
            if j == i || consumed[j] || sp.text.trim().is_empty() {
                continue;
            }
            let dy = sp.y_base - op_sp.y_base;
            if (3.0..=11.5).contains(&dy)
                && sp.font_size <= op_sp.font_size * 0.85
                && sp.x1 >= op_sp.x0 - 4.0
                && sp.x0 <= op_sp.x1 + 14.0
            {
                sub_idxs.push(j);
            }
        }
        if !sub_idxs.is_empty() {
            consumed[i] = true;
            for &j in &sub_idxs {
                consumed[j] = true;
            }
            let sub_plain = sorted_spans_text(&page.spans, &sub_idxs);
            let sub_mml = build_mrow_mathml(&page.spans, &sub_idxs);
            let mut mx0 = op_sp.x0;
            let mut mx1 = op_sp.x1;
            for &j in &sub_idxs {
                mx0 = mx0.min(page.spans[j].x0);
                mx1 = mx1.max(page.spans[j].x1);
            }
            let mml = format!(
                r#"<math xmlns="http://www.w3.org/1998/Math/MathML" class="omml-math"><munder><mo>{}</mo>{}</munder></math>"#,
                html_escape(op_t),
                sub_mml
            );
            let raw = format!("{}({})", op_t, sub_plain.trim());
            tokens.push(LineToken::Math {
                html: mml,
                raw,
                x0: mx0,
                x1: mx1,
                y_base: op_sp.y_base,
                font_size: op_sp.font_size,
            });
        }
    }

    for (i, sp) in page.spans.iter().enumerate() {
        if !consumed[i] {
            tokens.push(LineToken::Span(sp.clone()));
        }
    }

    // 2. Phân loại ảnh: ảnh công thức nhỏ cùng dòng vs hình vẽ/đồ thị độc lập
    for img in &page.images {
        let dw = (img.x1 - img.x0).abs();
        let dh = (img.y1 - img.y0).abs();
        let near_text_line = dw <= 185.0
            && dh <= 30.0
            && tokens.iter().any(|t| {
                let ty = t.y_base();
                ty >= img.y0 - 8.0 && ty <= img.y1 + 10.0 && (t.x1() <= img.x0 + 4.0 || t.x0() >= img.x1 - 4.0)
            });

        if near_text_line {
            let nat_w_px = (dw * 96.0 / 72.0).max(1.0);
            let nat_h_px = (dh * 96.0 / 72.0).max(1.0);
            let h_px = nat_h_px.clamp(12.0, 38.0);
            let scale = if nat_h_px > 4.0 {
                (h_px / nat_h_px).min(1.35)
            } else {
                1.0
            };
            let w_px = (nat_w_px * scale).max(6.0);
            let html = format!(
                r#"<img src="{}" class="math-formula" style="width: {:.1}px; height: {:.1}px; vertical-align: middle;" alt="Công thức">"#,
                img.data_uri, w_px, h_px
            );
            tokens.push(LineToken::InlineImg {
                html,
                x0: img.x0,
                x1: img.x1,
                y_base: img.y1 - 2.0,
                font_size: 11.0,
                method: ExtractionMethod::PdfImageEmbed,
                needs_review: false,
                review_reason: None,
            });
        } else {
            let cx_emu = dw * 12700.0;
            let cy_emu = dh * 12700.0;
            let html = render_drawing_figure_img(&img.data_uri, Some(cx_emu), Some(cy_emu));
            block_elements.push(PdfPageElement {
                y_top: img.y0,
                para: ParagraphData {
                    html,
                    raw: String::new(),
                    provenance: vec![BlockProvenance {
                        source_file: None,
                        page: page.page_num_1based,
                        bbox: Some(BoundingBox {
                            x0: img.x0,
                            y0: img.y0,
                            x1: img.x1,
                            y1: img.y1,
                        }),
                        method: ExtractionMethod::PdfImageEmbed,
                        needs_review: false,
                        review_reasons: Vec::new(),
                    }],
                },
            });
        }
    }

    // 3. Gom nhóm các token theo dòng (giữ an toàn (-∞;3) và C4H8O2)
    // Phát hiện trước trang bố cục 2 cột thật để tránh ghép nhầm cột trái và cột phải cùng độ cao Y.
    let two_col_gutter: Option<f64> = if page.width >= 420.0 && tokens.len() >= 18 {
        let mut best_gx: Option<(f64, usize, usize)> = None;
        for pct in 42..=58 {
            let gx = page.width * (pct as f64) / 100.0;
            let mut bands: std::collections::BTreeMap<i32, (bool, bool, bool, f64, f64)> =
                std::collections::BTreeMap::new();
            for tok in &tokens {
                let yb = tok.y_base();
                if yb < 42.0 || yb > page.height - 42.0 {
                    continue;
                }
                let bk = (yb / 6.0).round() as i32;
                let e = bands
                    .entry(bk)
                    .or_insert((false, false, false, yb, yb));
                e.3 = e.3.min(yb);
                e.4 = e.4.max(yb);
                if tok.x1() <= gx - 4.0 {
                    e.0 = true;
                } else if tok.x0() >= gx + 4.0 {
                    e.1 = true;
                } else {
                    e.2 = true;
                }
            }
            let mut left_cnt = 0usize;
            let mut right_cnt = 0usize;
            let mut cross_cnt = 0usize;
            let mut min_l_y = f64::INFINITY;
            let mut max_l_y = f64::NEG_INFINITY;
            let mut min_r_y = f64::INFINITY;
            let mut max_r_y = f64::NEG_INFINITY;
            for (_bk, (hl, hr, hc, y0, y1)) in bands {
                if hc {
                    cross_cnt += 1;
                    continue;
                }
                if hl {
                    left_cnt += 1;
                    min_l_y = min_l_y.min(y0);
                    max_l_y = max_l_y.max(y1);
                }
                if hr {
                    right_cnt += 1;
                    min_r_y = min_r_y.min(y0);
                    max_r_y = max_r_y.max(y1);
                }
            }
            let overlap_y = max_l_y.min(max_r_y) - min_l_y.max(min_r_y);
            if left_cnt >= 6 && right_cnt >= 5 && cross_cnt <= 2 && overlap_y >= 100.0 {
                let better = match best_gx {
                    None => true,
                    Some((_, bc, bt)) => {
                        cross_cnt < bc || (cross_cnt == bc && (left_cnt + right_cnt) > bt)
                    }
                };
                if better {
                    best_gx = Some((gx, cross_cnt, left_cnt + right_cnt));
                }
            }
        }
        best_gx.map(|(gx, _, _)| gx)
    } else {
        None
    };

    // Xử lý dòng chuẩn trước rồi gắn chỉ số trên/dưới nhỏ để không tách mũ/chỉ số thành dòng riêng
    tokens.sort_by(|a, b| {
        let a_small = a.font_size() < 8.8 && !matches!(a, LineToken::Span(s) if is_tall_delimiter(&s.text));
        let b_small = b.font_size() < 8.8 && !matches!(b, LineToken::Span(s) if is_tall_delimiter(&s.text));
        a_small
            .cmp(&b_small)
            .then_with(|| a.y_base().partial_cmp(&b.y_base()).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.x0().partial_cmp(&b.x0()).unwrap_or(std::cmp::Ordering::Equal))
    });

    struct VisualLine {
        anchor_y: f64,
        dom_fs: f64,
        items: Vec<LineToken>,
    }

    let mut lines: Vec<VisualLine> = Vec::new();
    for tok in tokens {
        let ty = tok.y_base();
        let tfs = tok.font_size();
        let tx0 = tok.x0();
        let tx1 = tok.x1();
        let tok_is_delim = matches!(&tok, LineToken::Span(s) if is_tall_delimiter(&s.text));
        let tok_is_small = tfs < 8.8 && !tok_is_delim;

        let mut best_line: Option<usize> = None;
        let mut best_score = f64::INFINITY;

        for (idx, line) in lines.iter().enumerate() {
            let crosses_gutter = two_col_gutter.is_some_and(|gx| {
                let mut lx0 = f64::INFINITY;
                let mut lx1 = f64::NEG_INFINITY;
                for it in &line.items {
                    lx0 = lx0.min(it.x0());
                    lx1 = lx1.max(it.x1());
                }
                (lx1 <= gx + 4.0 && tx0 >= gx - 4.0 && tx0 - lx1 >= 14.0)
                    || (tx1 <= gx + 4.0 && lx0 >= gx - 4.0 && lx0 - tx1 >= 14.0)
            });
            if crosses_gutter {
                continue;
            }
            let base_fs = line.dom_fs.min(13.5).max(tfs.min(13.5));
            let is_script_candidate = tok_is_small && line.dom_fs >= tfs * 1.18;
            let tol = if is_script_candidate {
                (0.65 * line.dom_fs.min(14.5)).clamp(5.6, 8.2)
            } else {
                (0.48 * base_fs).clamp(4.8, 6.4)
            };
            let dy = (ty - line.anchor_y).abs();
            if dy <= tol {
                let min_x_gap = if is_script_candidate {
                    line.items
                        .iter()
                        .map(|it| (it.x0() - tx1).max(tx0 - it.x1()).max(0.0))
                        .fold(f64::INFINITY, f64::min)
                } else {
                    0.0
                };
                let score = dy + 0.30 * min_x_gap;
                if score < best_score {
                    // Kiểm tra chống gộp 2 dòng kề sát theo trục dọc (bỏ qua dấu ngoặc kéo cao và chỉ số nhỏ)
                    let same_x_overlap = !tok_is_delim
                        && !is_script_candidate
                        && dy > 2.6
                        && line.items.iter().any(|existing| {
                            let ex_delim = matches!(existing, LineToken::Span(s) if is_tall_delimiter(&s.text));
                            if ex_delim {
                                return false;
                            }
                            let ov = tx1.min(existing.x1()) - tx0.max(existing.x0());
                            ov > 4.5
                        });
                    if !same_x_overlap {
                        best_score = score;
                        best_line = Some(idx);
                    }
                }
            }
        }

        if let Some(lidx) = best_line {
            let line = &mut lines[lidx];
            line.items.push(tok);
            let mut fs_buckets: HashMap<i32, usize> = HashMap::new();
            let mut anchor_samples: Vec<(f64, usize)> = Vec::new();
            for it in &line.items {
                if let LineToken::Span(s) = it {
                    let ch_len = s.text.trim().chars().count();
                    if ch_len > 0 && !is_tall_delimiter(&s.text) {
                        let bucket = (s.font_size * 2.0).round() as i32;
                        *fs_buckets.entry(bucket).or_insert(0) += ch_len;
                        anchor_samples.push((s.y_base, ch_len));
                    }
                }
            }
            if let Some((&best_bucket, _)) = fs_buckets.iter().max_by_key(|&(_, w)| w) {
                let dom_est = (best_bucket as f64) * 0.5;
                line.dom_fs = dom_est.clamp(8.0, 20.0);
                let mut normal_ys: Vec<f64> = line
                    .items
                    .iter()
                    .filter_map(|it| match it {
                        LineToken::Span(s)
                            if !s.text.trim().is_empty()
                                && !is_tall_delimiter(&s.text)
                                && (s.font_size - dom_est).abs() <= 1.2 =>
                        {
                            Some(s.y_base)
                        }
                        _ => None,
                    })
                    .collect();
                if !normal_ys.is_empty() {
                    normal_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    line.anchor_y = normal_ys[normal_ys.len() / 2];
                }
            }
        } else {
            lines.push(VisualLine {
                anchor_y: ty,
                dom_fs: if tok_is_delim { tfs.min(12.0) } else { tfs },
                items: vec![tok],
            });
        }
    }

    // 4. Kết xuất từng VisualLine theo thứ tự X tăng dần
    for mut line in lines {
        line.items
            .sort_by(|a, b| a.x0().partial_cmp(&b.x0()).unwrap_or(std::cmp::Ordering::Equal));

        // Tính dominant font_size & baseline theo mode có trọng số ký tự (loại bỏ dấu ngoặc đơn cao SymbolMT)
        let mut fs_weights: HashMap<i32, usize> = HashMap::new();
        let mut normal_spans: Vec<(f64, f64, usize)> = Vec::new();
        for it in &line.items {
            if let LineToken::Span(s) = it {
                let ch_len = s.text.trim().chars().count();
                if ch_len > 0 && !is_tall_delimiter(&s.text) {
                    let bucket = (s.font_size * 2.0).round() as i32;
                    *fs_weights.entry(bucket).or_insert(0) += ch_len;
                    normal_spans.push((s.y_base, s.font_size, ch_len));
                }
            }
        }
        let dom_fs = fs_weights
            .iter()
            .max_by_key(|&(_, w)| w)
            .map(|(&b, _)| (b as f64) * 0.5)
            .unwrap_or(11.0)
            .clamp(8.0, 22.0);
        let mut ys_normal: Vec<f64> = Vec::new();
        for &(yb, fs, w) in &normal_spans {
            if (fs - dom_fs).abs() <= 1.2 {
                for _ in 0..w.min(16) {
                    ys_normal.push(yb);
                }
            }
        }
        if ys_normal.is_empty() {
            for &(yb, _, _) in &normal_spans {
                ys_normal.push(yb);
            }
        }
        if ys_normal.is_empty() {
            ys_normal.push(line.anchor_y);
        }
        ys_normal.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let dom_y = ys_normal[ys_normal.len() / 2];

        let mut html = String::new();
        let mut raw = String::new();
        let mut prev_x1: Option<f64> = None;
        let mut prev_was_sup_or_sub = false;

        let mut ocr_count: usize = 0;
        let mut ocr_min_conf = f64::INFINITY;
        let mut ocr_sum_conf = 0.0_f64;
        let mut native_count: usize = 0;
        let mut fallback_crop_count: usize = 0;
        let mut review_reasons: Vec<String> = Vec::new();

        for item in &line.items {
            if let Some(px1) = prev_x1 {
                let gap = item.x0() - px1;
                if gap > dom_fs * 2.4 && !raw.ends_with('\t') && !raw.ends_with(' ') {
                    html.push(' ');
                    raw.push('\t');
                } else if gap > dom_fs * 0.22
                    && !prev_was_sup_or_sub
                    && !raw.ends_with(' ')
                    && !raw.ends_with('\t')
                {
                    let cur_starts_ws = match item {
                        LineToken::Span(s) => s.text.starts_with(char::is_whitespace),
                        _ => false,
                    };
                    if !cur_starts_ws {
                        html.push(' ');
                        raw.push(' ');
                    }
                }
            }

            match item {
                LineToken::Span(sp) => {
                    if let Some(conf) = sp.ocr_conf {
                        ocr_count += 1;
                        ocr_min_conf = ocr_min_conf.min(conf);
                        ocr_sum_conf += conf;
                    } else {
                        native_count += 1;
                    }
                    let text_clean = sp.text.as_str();
                    let non_ws = !text_clean.trim().is_empty();
                    let esc = html_escape(text_clean);
                    let is_native = sp.ocr_conf.is_none();
                    let smaller = sp.font_size <= dom_fs * 0.86;
                    let is_delim = is_tall_delimiter(text_clean);
                    let is_sup = is_native
                        && non_ws
                        && !is_delim
                        && ((sp.y_base <= dom_y - 1.2 && smaller) || sp.y_base <= dom_y - dom_fs * 0.26);
                    let is_sub = is_native
                        && non_ws
                        && !is_delim
                        && !is_sup
                        && ((sp.y_base >= dom_y + 0.4 && smaller) || sp.y_base >= dom_y + dom_fs * 0.22);

                    if is_sup {
                        html.push_str("<sup>");
                        html.push_str(&esc);
                        html.push_str("</sup>");
                        prev_was_sup_or_sub = true;
                    } else if is_sub {
                        html.push_str("<sub>");
                        html.push_str(&esc);
                        html.push_str("</sub>");
                        prev_was_sup_or_sub = true;
                    } else if !non_ws {
                        html.push_str(&esc);
                        prev_was_sup_or_sub = false;
                    } else {
                        if sp.is_bold {
                            html.push_str("<b>");
                        }
                        if sp.is_italic {
                            html.push_str("<i>");
                        }
                        html.push_str(&esc);
                        if sp.is_italic {
                            html.push_str("</i>");
                        }
                        if sp.is_bold {
                            html.push_str("</b>");
                        }
                        prev_was_sup_or_sub = false;
                    }
                    raw.push_str(text_clean);
                    prev_x1 = Some(sp.x1);
                }
                LineToken::Math {
                    html: mh,
                    raw: mr,
                    x1,
                    ..
                } => {
                    native_count += 1;
                    html.push_str(mh);
                    raw.push_str(mr);
                    prev_x1 = Some(*x1);
                    prev_was_sup_or_sub = false;
                }
                LineToken::InlineImg {
                    html: ih,
                    x1,
                    method,
                    needs_review,
                    review_reason,
                    ..
                } => {
                    if *method == ExtractionMethod::ImageCropFallback {
                        fallback_crop_count += 1;
                    }
                    if *needs_review {
                        if let Some(rr) = review_reason {
                            if !review_reasons.contains(rr) {
                                review_reasons.push(rr.clone());
                            }
                        }
                    }
                    html.push_str(ih);
                    prev_x1 = Some(*x1);
                    prev_was_sup_or_sub = false;
                }
            }
        }

        let trimmed_raw = raw.trim().to_string();
        let trimmed_html = clean_edge_html(&html);
        if !trimmed_raw.is_empty() || trimmed_html.contains("<img") || trimmed_html.contains("<math") {
            let mut min_x0 = f64::INFINITY;
            let mut max_x1 = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            let mut method = if ocr_count > 0 && native_count == 0 {
                ExtractionMethod::OcrTesseract
            } else if fallback_crop_count > 0 && native_count == 0 && ocr_count == 0 {
                ExtractionMethod::ImageCropFallback
            } else {
                ExtractionMethod::PdfNativeText
            };
            for it in &line.items {
                match it {
                    LineToken::Span(s) => {
                        min_x0 = min_x0.min(s.x0);
                        max_x1 = max_x1.max(s.x1);
                        min_y = min_y.min(s.y_base - s.font_size);
                        max_y = max_y.max(s.y_base + s.font_size * 0.25);
                    }
                    LineToken::Math { x0, x1, y_base, font_size, .. } => {
                        min_x0 = min_x0.min(*x0);
                        max_x1 = max_x1.max(*x1);
                        min_y = min_y.min(*y_base - *font_size);
                        max_y = max_y.max(*y_base + *font_size * 0.25);
                    }
                    LineToken::InlineImg { x0, x1, y_base, .. } => {
                        min_x0 = min_x0.min(*x0);
                        max_x1 = max_x1.max(*x1);
                        min_y = min_y.min(*y_base - 12.0);
                        max_y = max_y.max(*y_base + 2.0);
                        if trimmed_raw.is_empty() && method == ExtractionMethod::PdfNativeText {
                            method = ExtractionMethod::PdfImageEmbed;
                        }
                    }
                }
            }
            let mut needs_review = !review_reasons.is_empty() || fallback_crop_count > 0;
            if ocr_count > 0 {
                let mean_conf = ocr_sum_conf / (ocr_count as f64);
                if ocr_min_conf < 65.0 || mean_conf < 75.0 {
                    needs_review = true;
                    let msg = format!(
                        "Đoạn nhận diện OCR có độ tin cậy thấp ({:.0}%), cần đối chiếu bản gốc.",
                        mean_conf
                    );
                    if !review_reasons.contains(&msg) {
                        review_reasons.push(msg);
                    }
                }
            }
            let bbox = if min_x0.is_finite() && max_x1.is_finite() {
                Some(BoundingBox {
                    x0: (min_x0 * 10.0).round() / 10.0,
                    y0: (min_y * 10.0).round() / 10.0,
                    x1: (max_x1 * 10.0).round() / 10.0,
                    y1: (max_y * 10.0).round() / 10.0,
                })
            } else {
                None
            };
            block_elements.push(PdfPageElement {
                y_top: dom_y,
                para: ParagraphData {
                    html: trimmed_html,
                    raw: trimmed_raw,
                    provenance: vec![BlockProvenance {
                        source_file: None,
                        page: page.page_num_1based,
                        bbox,
                        method,
                        needs_review,
                        review_reasons,
                    }],
                },
            });
        }
    }

    block_elements.sort_by(|a, b| {
        a.y_top
            .partial_cmp(&b.y_top)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    reorder_two_column_elements(block_elements, page.width)
}

fn reorder_two_column_elements(mut elements: Vec<PdfPageElement>, page_width: f64) -> Vec<PdfPageElement> {
    if elements.len() < 12 || page_width < 420.0 {
        return elements;
    }

    // Tìm khe dọc trống (gutter) tốt nhất quanh giữa trang (42%..58% chiều rộng trang)
    let mut best_mid: Option<(f64, usize, usize, usize, f64, f64)> = None;
    for pct in 42..=58 {
        let mid_x = page_width * (pct as f64) / 100.0;
        let mut left_cnt = 0usize;
        let mut right_cnt = 0usize;
        let mut cross_cnt = 0usize;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for el in &elements {
            let Some(bb) = el.para.provenance.first().and_then(|p| p.bbox.as_ref()) else {
                continue;
            };
            let w = bb.x1 - bb.x0;
            if w > page_width * 0.56 {
                // Tiêu đề / chân trang toàn chiều rộng không tính vào cột
                continue;
            }
            if bb.x1 <= mid_x + 4.0 {
                left_cnt += 1;
                min_y = min_y.min(el.y_top);
                max_y = max_y.max(el.y_top);
            } else if bb.x0 >= mid_x - 4.0 {
                right_cnt += 1;
                min_y = min_y.min(el.y_top);
                max_y = max_y.max(el.y_top);
            } else {
                cross_cnt += 1;
            }
        }
        if left_cnt >= 6 && right_cnt >= 6 && cross_cnt <= 1 && (max_y - min_y) >= 140.0 {
            let score_better = match best_mid {
                None => true,
                Some((_, bl, br, bc, _, _)) => {
                    cross_cnt < bc || (cross_cnt == bc && (left_cnt + right_cnt) > (bl + br))
                }
            };
            if score_better {
                best_mid = Some((mid_x, left_cnt, right_cnt, cross_cnt, min_y, max_y));
            }
        }
    }

    let Some((mid_x, _, _, _, col_top, col_bot)) = best_mid else {
        return elements;
    };

    // Tìm điểm chia theo chiều dọc: phần tử nằm trên vùng cột giữ nguyên thứ tự, phần cột xếp hết cột trái rồi mới cột phải
    let mut header_els = Vec::new();
    let mut left_els = Vec::new();
    let mut right_els = Vec::new();
    let mut footer_els = Vec::new();

    // Giới hạn y đầu của các phần tử thuộc cột trái/phải hẹp
    let mut first_col_y = col_bot;
    let mut last_col_y = col_top;
    for el in &elements {
        if let Some(bb) = el.para.provenance.first().and_then(|p| p.bbox.as_ref()) {
            let w = bb.x1 - bb.x0;
            if w <= page_width * 0.56 && (bb.x1 <= mid_x + 4.0 || bb.x0 >= mid_x - 4.0) {
                first_col_y = first_col_y.min(el.y_top);
                last_col_y = last_col_y.max(el.y_top);
            }
        }
    }

    for el in elements.drain(..) {
        let bb_opt = el.para.provenance.first().and_then(|p| p.bbox.as_ref()).cloned();
        if let Some(bb) = bb_opt {
            let w = bb.x1 - bb.x0;
            if el.y_top < first_col_y - 4.0 || (w > page_width * 0.56 && el.y_top < first_col_y + 12.0) {
                header_els.push(el);
            } else if el.y_top > last_col_y + 4.0 || (w > page_width * 0.56 && el.y_top > last_col_y - 12.0) {
                footer_els.push(el);
            } else if bb.x1 <= mid_x + 4.0 {
                left_els.push(el);
            } else if bb.x0 >= mid_x - 4.0 {
                right_els.push(el);
            } else {
                // Phần tử toàn chiều rộng nằm giữa vùng 2 cột (ít gặp) giữ vào cột trái theo thứ tự
                left_els.push(el);
            }
        } else {
            left_els.push(el);
        }
    }

    let mut out = Vec::with_capacity(header_els.len() + left_els.len() + right_els.len() + footer_els.len());
    out.extend(header_els);
    out.extend(left_els);
    out.extend(right_els);
    out.extend(footer_els);
    out
}

fn sorted_spans_text(spans: &[PdfSpan], idxs: &[usize]) -> String {
    let mut ordered = idxs.to_vec();
    ordered.sort_by(|&a, &b| {
        spans[a]
            .x0
            .partial_cmp(&spans[b].x0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut s = String::new();
    let mut prev_x1: Option<f64> = None;
    for &idx in &ordered {
        let sp = &spans[idx];
        if let Some(px1) = prev_x1 {
            if sp.x0 - px1 > sp.font_size * 0.25 && !s.ends_with(' ') && !sp.text.starts_with(' ') {
                s.push(' ');
            }
        }
        s.push_str(&sp.text);
        prev_x1 = Some(sp.x1);
    }
    s
}

fn build_mrow_mathml(spans: &[PdfSpan], idxs: &[usize]) -> String {
    let mut ordered = idxs.to_vec();
    ordered.sort_by(|&a, &b| {
        spans[a]
            .x0
            .partial_cmp(&spans[b].x0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let dom_fs = ordered
        .iter()
        .map(|&i| spans[i].font_size)
        .fold(9.0_f64, f64::max);
    let dom_y = ordered
        .iter()
        .filter(|&&i| spans[i].font_size >= dom_fs * 0.85)
        .map(|&i| spans[i].y_base)
        .next()
        .unwrap_or_else(|| ordered.first().map(|&i| spans[i].y_base).unwrap_or(0.0));

    let mut inner = String::new();
    for &i in &ordered {
        let sp = &spans[i];
        let esc = html_escape(sp.text.trim());
        if esc.is_empty() {
            continue;
        }
        let is_sup = sp.y_base < dom_y - 1.5 && sp.font_size <= dom_fs * 0.86;
        let is_sub = !is_sup && sp.y_base > dom_y + 1.5 && sp.font_size <= dom_fs * 0.86;
        if is_sup {
            inner.push_str("<msup><mi></mi><mn>");
            inner.push_str(&esc);
            inner.push_str("</mn></msup>");
        } else if is_sub {
            inner.push_str("<msub><mi></mi><mn>");
            inner.push_str(&esc);
            inner.push_str("</mn></msub>");
        } else if esc.chars().all(|c| c.is_ascii_digit() || c == '.') {
            inner.push_str("<mn>");
            inner.push_str(&esc);
            inner.push_str("</mn>");
        } else if "+−-=*/×÷<>≤≥≠≈()[]{}".contains(&esc) {
            inner.push_str("<mo>");
            inner.push_str(&esc);
            inner.push_str("</mo>");
        } else {
            inner.push_str("<mi>");
            inner.push_str(&esc);
            inner.push_str("</mi>");
        }
    }
    format!("<mrow>{}</mrow>", inner)
}

// ============================================================================
// 5. Phân tách Đề thi vs Bảng đáp án đầu/cuối vs Lời giải từng câu
// ============================================================================

fn split_pdf_document(
    extracted: &PdfDocExtracted,
) -> (Vec<DocBlock>, HashMap<String, ExamAnswerKey>, usize) {
    let re_page_footer = Regex::new(
        r"(?i)^\s*(?:trang\s*\d+(?:\s*/\s*\d+)?|page\s*\d+(?:\s*(?:of|/)\s*\d+)?|-+\s*\d+\s*-+|\d{1,2}/\d{1,2}|(?:toanmath|vietjack|loigiaihay|thuvienhoclieu|tailieuchuan)\.(?:com|vn).*)\s*$"
    ).unwrap();
    let re_watermark_noise = Regex::new(
        r"(?i)^\s*(?:sưu\s*tầm\s*&\s*biên\s*soạn|biên\s*soạn\s*bởi|tài\s*liệu\s*tham\s*khảo\s*miễn\s*phí\b.*|truy\s+c[a-zà-ỹ†ịl]{1,8}\s+h[ft†]{1,3}ps?://\S*lo[i1l]g[i1l]o[i1l]h[aào]y\S*.*|[\}\]\s]*(?:lo[i1l]g[i1l][a-z]{1,6}|olg[a-z]{1,8})\.\s*(?:c[oe]m|vn)\s*|(?:\._\s*\||\|\s*-|_\s*l|l\s*-|nong\s*»|[.,;]\s*„)\s*$)"
    ).unwrap();

    // Thu thập mọi đoạn văn theo thứ tự trang (bỏ chân trang/header lặp)
    let mut all_paras: Vec<(usize, ParagraphData)> = Vec::new();
    let mut line_page_counts: HashMap<String, HashSet<usize>> = HashMap::new();
    let mut has_ocr_pages = false;

    for (p_idx, pg) in extracted.pages.iter().enumerate() {
        let elements = build_page_elements(pg);
        for mut el in elements {
            for pr in &mut el.para.provenance {
                if pr.page == 0 {
                    pr.page = p_idx + 1;
                }
                if matches!(pr.method, ExtractionMethod::OcrTesseract) {
                    has_ocr_pages = true;
                }
            }
            let norm = el.para.raw.trim().to_ascii_lowercase();
            if !norm.is_empty() && (el.y_top <= 60.0 || el.y_top >= pg.height - 60.0) {
                line_page_counts.entry(norm).or_default().insert(p_idx);
            }
            all_paras.push((p_idx, el.para));
        }
    }

    let mut clean_paras: Vec<ParagraphData> = Vec::with_capacity(all_paras.len());
    let mut total_visible_chars = 0usize;

    let re_ocr_cau = Regex::new(
        r#"^\s*[‘'“"`*~•\-–—_.,]*\s*(?-i:Câu|Cau|Caw|Cầu|Question)\s*(\d{1,3})(?:\s*\([^)]{1,12}\))?\s*([:\.])"#
    ).unwrap();

    for (_p_idx, mut para) in all_paras {
        para = normalize_leading_cau_para(para, &re_ocr_cau);
        let raw_trim = para.raw.trim();
        total_visible_chars += raw_trim.chars().filter(|c| !c.is_whitespace()).count();
        if raw_trim.is_empty() && !para.html.contains("<img") && !para.html.contains("<math") {
            continue;
        }
        if re_page_footer.is_match(raw_trim) || re_watermark_noise.is_match(raw_trim) {
            continue;
        }
        let norm = raw_trim.to_ascii_lowercase();
        let protected_exam_line = norm.starts_with("câu ")
            || norm.starts_with("question ")
            || norm.starts_with("phần ")
            || norm.starts_with("đáp án")
            || norm.starts_with("lời giải")
            || norm.starts_with("phương pháp")
            || norm.starts_with("chọn ")
            || norm.starts_with("a.")
            || norm.starts_with("b.")
            || norm.starts_with("c.")
            || norm.starts_with("d.");
        if !protected_exam_line
            && extracted.pages.len() >= 3
            && line_page_counts.get(&norm).is_some_and(|s| s.len() >= 3)
        {
            continue;
        }
        clean_paras.push(para);
    }

    // Nhận diện xem đây là đề chuẩn `Câu N.` hay đề đánh số trơn `1. .. 40.` (Tiếng Anh)
    let count_cau = clean_paras
        .iter()
        .filter(|p| Regex::new(r"^\s*(?:Câu|Question)\s*\d+(?:\s*\([^)]{1,12}\))?\s*[:\.]").unwrap().is_match(&p.raw))
        .count();
    let re_bare_num = Regex::new(r"^\s*(\d{1,2})\s*\.(?:\s+|$)").unwrap();
    let is_traditional_numbered = count_cau < 3
        && clean_paras
            .iter()
            .filter(|p| re_bare_num.is_match(&p.raw))
            .count()
            >= 8;

    let mut default_code = "default".to_string();
    let mut has_explicit_code = false;
    let re_code = Regex::new(r"(?i)m[ãa]\s*[đd][ềe]\s*[:\s]*(\d{3,4})\b").unwrap();
    for p in clean_paras.iter().take(25) {
        if let Some(c) = re_code.captures(&p.raw) {
            default_code = c[1].to_string();
            has_explicit_code = true;
            break;
        }
    }

    let re_top_ans_hdr = Regex::new(
        r"(?i)^\s*(?:(?:B[ẢA]NG\s*)?[ĐD][ÁA]P\s*[ÁA]N(?:\s+(?:THAM\s+KH[ẢA]O|V[ÀA]\s+(?:L[ỜO]I\s*|H[ƯU][ỚO]NG\s*D[ẪA]N\s*)?GI[ẢA]I(?:\s+CHI\s+TI[ẾE]T)?))?|H[ƯU][ỚO]NG\s*D[ẪA]N\s*(?:L[ỜO]I\s*)?GI[ẢA]I(?:\s+CHI\s+TI[ẾE]T)?|L[ỜO]I\s*GI[ẢA]I\s+CHI\s+TI[ẾE]T)\s*[:\.\-]?\s*$"
    ).unwrap();
    let re_body_start = Regex::new(
        r"(?i)^\s*(?:PH[ẦA]N\s*(?:I|1)\b|PART\s*1\b|C[âa]u\s*1\s*[:\.]|1\.\s+\S)"
    ).unwrap();
    let re_opt_prefix = Regex::new(r"^\s*[A-Da-d]\s*[\.\)]\s+").unwrap();
    let re_gdpt_part1 = Regex::new(
        r"(?i)PH[ẦAẢ]N\s*(?:I\b|1(?:\s*[:\.\-–—\(]|\s+(?:c[âa]u|tr[ắa]c)|\s*$))|(?:c[âa]u\s*(?:h[ỏo]i\s*)?)?tr[ắa]c\s*nghi[ệe]m\s*(?:kh[áa]ch\s*quan|nhi[ềe]u\s*(?:l[ựu]a\s*ch[ọo]n|ph[ưươ]ng\s*[áa]n)|4\s*l[ựu]a\s*ch[ọo]n)\b|^\s*(?:[-–•*]\s*)?(?:I\.\s*)?(?:ph[ầa]n\s*)?tr[ắa]c\s*nghi[ệe]m\b"
    ).unwrap();
    let re_gdpt_part2 = Regex::new(
        r"(?i)PH[ẦAẢ]N\s*(?:II\b|2(?:\s*[:\.\-–—\(]|\s+(?:c[âa]u|tr[ắa]c|[đd][úu]ng)|\s*$))|(?:c[âa]u\s*(?:h[ỏo]i\s*)?)?tr[ắa]c\s*nghi[ệe]m\s*[đd][úu]ng\s*[-–/]?\s*sai\b|^\s*(?:[-–•*]\s*)?(?:II\.\s*)?(?:ph[ầa]n\s*)?(?:c[âa]u\s*h[ỏo]i\s*)?[đd][úu]ng\s*[-–/]?\s*sai\b"
    ).unwrap();
    let re_gdpt_part3 = Regex::new(
        r"(?i)PH[ẦAẢ]N\s*(?:III\b|3(?:\s*[:\.\-–—\(]|\s+(?:c[âa]u|tr[ắa]c|tr[ảa])|\s*$))|(?:c[âa]u\s*(?:h[ỏo]i\s*)?)?tr[ắa]c\s*nghi[ệe]m\s*tr[ảa]\s*l[ờo]i\s*ng[ắa]n\b|^\s*(?:[-–•*]\s*)?(?:III\.\s*)?(?:ph[ầa]n\s*)?(?:c[âa]u\s*h[ỏo]i\s*)?tr[ảa]\s*l[ờo]i\s*ng[ắa]n\b"
    ).unwrap();
    let re_gdpt_part4_essay = Regex::new(
        r"(?i)^\s*(?:[-–•*]\s*)?(?:(?:PH[ẦAẢ]N\s*(?:[IVXL]+|[1-4])[\s:\.\-–—]*|(?:[IVX1-4A-D]+|IL|lI|ll|I1|1I)\s*[\.\):]\s*)?T[ỰỤ]\s*LU[ẬA]N\b|PH[ẦAẢ]N\s*(?:IV|4)(?:\s*[:\.\-–—\(]|\s*$))"
    ).unwrap();
    let re_stop_always = Regex::new(
        r"(?i)^\s*(?:[-–=_*]{2,}\s*H[ẾE]T\s*[-–=_*]{2,}|[-–=_*\s]*H[ẾE]T\s*[\.!]*\s*$|[ĐD][ÁA]P\s*[ÁA]N\s*(?:CH[ÍI]NH\s*TH[ỨU]C|THAM\s*KH[ẢA]O|T[ỔO]NG\s*H[ỢO]P)\b|B[ẢA]NG\s*[ĐD][ÁA]P\s*[ÁA]N\b)"
    ).unwrap();
    let re_stop_trailing_guide = Regex::new(
        r"(?i)^\s*(?:PH[ẦA]N\s+)?(?:[ĐD][ÁA]P\s*[ÁA]N\s*(?:V[ÀA]\s*)?)?(?:H[UƯ][OỚƠ]NG\s*D[AẪ]N\s*(?:L[OỜƠ]I\s*)?GI[ẢA]I|L[OỜƠ]I\s*GI[ẢA]I\s*CH[IÌỈĨỊ]\s*TI[ÊẾE]T|[ĐD][ÁA]P\s*[ÁA]N\s*CH[IÌỈĨỊ]\s*TI[ÊẾE]T)\b"
    ).unwrap();
    let re_sol_block_start = Regex::new(
        r"(?i)^\s*(?:Ph[uưươ][ơo]ng\s*ph[áa]p(?:\s*gi[ảa]i)?\s*(?:[:\.\-]|$)|L[oờơ]i\s*gi[ảa]i(?:\s*ch[iìỉ]\s*ti[êếe]t)?\s*[:\.\-]?|H[uưươ][oớơ]ng\s*d[aẫ]n\s*(?:l[oờơ]i\s*)?gi[ảa]i(?:\s*ch[iìỉ]\s*ti[êếe]t)?\s*[:\.\-]?|C[áa]ch\s*gi[ảa]i\s*[:\-]|Gi[ảa]i\s*ch[iìỉ]\s*ti[êếe]t\s*(?:[:\.\-]|$)|Gi[ảa]i\s*th[íi]ch\s*(?:[:\.\-]|$)|T[ạa]m\s*d[ịi]ch\s*[:\-]|D[ịi]ch\s*ngh[ĩi]a\s*[:\-])"
    ).unwrap();
    let re_sol_inline_cut = Regex::new(
        r"(?i)\s+(?:Ph[uưươ][ơo]ng\s*ph[áa]p\s*gi[ảa]i\s*:|Ph[uưươ][ơo]ng\s*ph[áa]p\s*:|L[oờơ]i\s*gi[ảa]i\s*:|H[uưươ][oớơ]ng\s*d[aẫ]n\s*gi[ảa]i\s*:|[ĐD][áa]p\s*[áa]n\s*(?:đúng(?:\s+là)?\s*)?:\s*(?:[A-D]\b|[ĐS]{4}|-?\d)|Ch[ọo]n\s+đáp\s+án\s+[A-D]\b)"
    ).unwrap();
    let re_ans_only_line = Regex::new(
        r"(?i)^\s*(?:[=*_>•\-]+\s*)*(?:[ĐD][áaàảãạ]p\s*[áaàảãạ]n(?:\s+(?:đúng(?:\s+là)?|chọn))?\s*[:\-=\.>]*\s*(?:[A-D]\b|a\)|[ĐSđs\s,;]{4,}|-?\d)|Ch[ọo]n(?:\s+đáp\s+án)?\s*[:\-=\.>]?\s*[A-D]\s*$)"
    ).unwrap();
    let re_en_passage = Regex::new(
        r"(?i)\b(?:Read\s+the\s+following|Questions?\s+\d+\s*(?:to|-|–)\s*\d+|blanks?\s+from\s+\d+\s+to\s+\d+)"
    ).unwrap();
    let re_en_range = Regex::new(r"(?i)(?:from\s+(?:questions?\s+)?|questions?\s+)(\d+)\s*(?:to|-|–)\s*(\d+)").unwrap();
    let _re_reset_after_guide = Regex::new(r"(?i)^\s*(?:C[âa]u\s*1\b|B[àa]i\s*1\b|1[\.\)]\s*[A-D]\b)").unwrap();

    let mut top_answer_lines: Vec<String> = Vec::new();
    let mut bottom_answer_lines: Vec<String> = Vec::new();
    let mut inline_answers: Vec<(u8, u32, String)> = Vec::new(); // (part 1/2/3, q_num, ans_text)

    let mut exam_blocks: Vec<DocBlock> = Vec::new();
    let mut in_top_ans = false;
    let mut in_solution = false;
    let mut in_essay_part4 = false;
    let mut in_extra_variant_booklet = false;
    let mut seen_any_q = false;
    let mut seen_q_count: usize = 0;
    let mut seen_per_q_sol = false;
    let mut current_part: u8 = 1;
    let mut active_q_num: Option<u32> = None;
    let mut expected_next_q: u32 = 1;
    let mut seen_trailing_stop = false;
    let mut skip_next_en_idx: Option<usize> = None;

    let re_top_prose_break = Regex::new(
        r"(?i)^\s*(?:C[âa]u\s*1(?:\s*\([^)]{1,12}\))?\s*[:\.]|PART\s*1\b|PH[ẦA]N\s*(?:I|1)\b.*(?:C[âa]u\s+tr[ắa]c|D[ạa]ng\s+c[âa]u|nhi[ềe]u\s+(?:l[ựự]a\s+ch[ọo]n|ph[ưươ]ng\s+[áa]n))|1\.\s+(?:Read\s+the|Mark\s+the|Choose\s+the|Circle\s+the|Find\s+the|Complete\s+the)\b|Question\s*1(?:\s*\([^)]{1,12}\))?\s*[:\.]?\b|Ki[ếe]n\s*th[ứu]c\s*:|Gi[ảa]i\s*th[íi]ch\s*:|Ph[uưươ][ơo]ng\s*ph[áa]p\s*:|T[ạa]m\s*d[ịi]ch\s*:)"
    ).unwrap();
    let re_cau_prefix_line = Regex::new(r"^\s*(?:Câu|Question)\s*\d+(?:\s*\([^)]{1,12}\))?\s*[:\.]").unwrap();

    for idx in 0..clean_paras.len() {
        if skip_next_en_idx == Some(idx) {
            continue;
        }
        let mut p = clean_paras[idx].clone();
        let raw_trim = p.raw.trim().to_string();

        if seen_trailing_stop {
            bottom_answer_lines.push(raw_trim);
            continue;
        }

        // Bắt đầu vùng bảng đáp án đặt ở đầu trang 1 trước Phần I
        if !seen_any_q && re_top_ans_hdr.is_match(&raw_trim) {
            in_top_ans = true;
            top_answer_lines.push(raw_trim);
            continue;
        }

        if in_top_ans {
            // Giữ trạng thái top_answer nếu gặp tiêu đề `Phần I. Trắc nghiệm...` nhưng ngay sau đó là bảng `Câu / Đáp án` hoặc ô số-đáp án
            let is_top_subhdr_into_table = !Regex::new(r"(?i)^\s*(?:C[âa]u|Question)\s*1\b").unwrap().is_match(&raw_trim)
                && clean_paras[idx + 1..].iter().take(4).any(|np| {
                    let nt = np.raw.trim();
                    let nl = nt.to_lowercase();
                    !nt.is_empty()
                        && (nl == "câu"
                            || nl == "đáp án"
                            || nl.starts_with("câu đáp án")
                            || nt.chars().all(|c| c.is_ascii_digit() || c.is_whitespace()))
                });
            // Kết thúc bảng đáp án đầu đề khi gặp Phần I thật, PART 1, Câu 1., hoặc câu 1 đọc hiểu
            if is_top_subhdr_into_table || !re_top_prose_break.is_match(&raw_trim) {
                top_answer_lines.push(raw_trim);
                continue;
            }
            in_top_ans = false;
        }

        let has_cau_prefix = re_cau_prefix_line.is_match(&raw_trim);

        // Gặp --- HẾT --- / BẢNG ĐÁP ÁN, hoặc gặp phần HƯỚNG DẪN LỜI GIẢI CHI TIẾT ở cuối đề
        let is_bare_sol_hdr = Regex::new(r"(?i)^\s*L[ờo]i\s*gi[ảa]i(?:\s+ch[iìỉ]\s*ti[êếe]t)?\s*[:\.\-]?\s*$")
            .unwrap()
            .is_match(&raw_trim);
        let is_explicit_summary_hdr = Regex::new(
            r"(?i)(?:B[ẢA]NG\s*[ĐD][ÁA]P\s*[ÁA]N|[ĐD][ÁA]P\s*[ÁA]N\s*(?:V[ÀA]\s*)?L[ỜO]I\s*GI[ẢA]I|H[ƯU][ỚO]NG\s*D[ẪA]N\s*(?:L[ỜO]I\s*)?GI[ẢA]I\s*CH[IÌỈ]\s*TI[ÊẾE]T|---\s*H[ẾE]T\s*---)"
        )
        .unwrap()
        .is_match(&raw_trim);
        let re_reset_cau1_strict = Regex::new(r"(?i)^\s*(?:C[âa]u|Question)\s*1(?:\s*\([^)]{1,12}\))?\s*[:\.\-]").unwrap();
        let is_trailing_guide_start = seen_q_count >= 10
            && !has_cau_prefix
            && raw_trim.chars().count() < 120
            && re_stop_trailing_guide.is_match(&raw_trim)
            && (!seen_per_q_sol
                || (!is_bare_sol_hdr
                    && is_explicit_summary_hdr
                    && clean_paras[idx + 1..].iter().take(18).any(|np| {
                        let nt = np.raw.trim();
                        re_gdpt_part1.is_match(nt)
                            || re_top_ans_hdr.is_match(nt)
                            || re_reset_cau1_strict.is_match(nt)
                    })));

        if (seen_any_q && re_stop_always.is_match(&raw_trim)) || is_trailing_guide_start {
            seen_trailing_stop = true;
            bottom_answer_lines.push(raw_trim);
            continue;
        }

        // Nhận diện chuyển sang tập đề của mã đề thứ 2 trong cùng một file (sau khi đã đọc >= 10 câu của mã đầu)
        if !in_extra_variant_booklet && seen_q_count >= 10 {
            if has_explicit_code {
                if let Some(c) = re_code.captures(&raw_trim) {
                    if c[1] != default_code && raw_trim.chars().count() < 120 {
                        in_extra_variant_booklet = true;
                        continue;
                    }
                }
            }
            if current_part >= 2
                && !has_cau_prefix
                && raw_trim.chars().count() < 160
                && re_gdpt_part1.is_match(&raw_trim)
                && !re_gdpt_part2.is_match(&raw_trim)
                && !re_gdpt_part3.is_match(&raw_trim)
            {
                in_extra_variant_booklet = true;
                continue;
            }
        }
        if in_extra_variant_booklet {
            continue;
        }

        // Bỏ qua phần Tự luận (PHẦN IV / Tự luận) để không ghép nhầm vào Part III
        if seen_q_count >= 6
            && !has_cau_prefix
            && raw_trim.chars().count() < 120
            && re_gdpt_part4_essay.is_match(&raw_trim)
        {
            in_essay_part4 = true;
            continue;
        }
        if in_essay_part4 {
            continue;
        }

        // Kiểm tra tiêu đề Phần (chỉ khi dòng ngắn và không phải Câu N hoặc phương án A..D hoặc gạch đầu dòng mục tiêu)
        let is_opt_not_header = re_opt_prefix.is_match(&raw_trim)
            && !raw_trim.contains("PHẦN")
            && !raw_trim.contains("TRẮC NGHIỆM");
        let hdr_low = raw_trim.to_lowercase();
        let hdr_upper = raw_trim.to_uppercase();
        let starts_lowercase = raw_trim.chars().next().map(|c| c.is_lowercase()).unwrap_or(false);
        let is_goal_bullet_not_header = ((raw_trim.starts_with('-')
            || raw_trim.starts_with('•')
            || raw_trim.starts_with('*')
            || raw_trim.starts_with('+'))
            && !hdr_upper.contains("PHẦN")
            && !hdr_upper.contains("PART"))
            || starts_lowercase
            || hdr_low.contains("vận dụng linh hoạt")
            || hdr_low.contains("giải quyết các câu hỏi")
            || hdr_low.contains("mục tiêu")
            || hdr_low.contains("và trắc nghiệm");
        if !has_cau_prefix && !is_opt_not_header && !is_goal_bullet_not_header && raw_trim.chars().count() < 160 {
            if re_gdpt_part3.is_match(&raw_trim) && !re_gdpt_part2.is_match(&raw_trim) {
                current_part = 3;
                in_solution = false;
                expected_next_q = 1;
                exam_blocks.push(DocBlock::Paragraph(p));
                continue;
            } else if re_gdpt_part2.is_match(&raw_trim) {
                current_part = 2;
                in_solution = false;
                expected_next_q = 1;
                exam_blocks.push(DocBlock::Paragraph(p));
                continue;
            } else if re_gdpt_part1.is_match(&raw_trim) {
                current_part = 1;
                in_solution = false;
                expected_next_q = 1;
                exam_blocks.push(DocBlock::Paragraph(p));
                continue;
            }
        }

        // Chuyển đổi câu đánh số trơn `1. ...` (Tiếng Anh) sang `Câu 1. ...` để dùng chung assembler
        let mut is_new_question = None;
        if let Some(cap) = Regex::new(r"^\s*(?:Câu|Question)\s*(\d+)(?:\s*\([^)]{1,12}\))?\s*[:\.]").unwrap().captures(&raw_trim) {
            if let Ok(n) = cap[1].parse::<u32>() {
                is_new_question = Some(n);
            }
        } else if is_traditional_numbered && !raw_trim.starts_with("PART ") {
            if let Some(cap) = re_bare_num.captures(&raw_trim) {
                if let Ok(n) = cap[1].parse::<u32>() {
                    if (1..=60).contains(&n)
                        && (n == expected_next_q || (n + 2 >= expected_next_q && expected_next_q <= n + 4))
                    {
                        is_new_question = Some(n);
                        p = prepend_cau_prefix_to_numbered_para(p);
                    }
                }
            }
        }

        // Hỗ trợ gán ngữ cảnh đoạn văn đọc hiểu Tiếng Anh (`Read the following ... from 1 to 6` hoặc 2-line header hoặc blank `(7) ___`)
        if is_new_question.is_none() && re_en_passage.is_match(&raw_trim) {
            let mut range_pair: Option<(u32, u32)> = None;
            let mut joined_hdr: Option<(String, String)> = None;
            if let Some(cap) = re_en_range.captures(&raw_trim) {
                if let (Ok(s), Ok(e)) = (cap[1].parse::<u32>(), cap[2].parse::<u32>()) {
                    if s <= e && e - s <= 12 {
                        range_pair = Some((s, e));
                    }
                }
            }
            if range_pair.is_none() && idx + 1 < clean_paras.len() {
                let np = &clean_paras[idx + 1];
                let nt = np.raw.trim();
                let comb_raw = format!("{} {}", raw_trim, nt);
                if let Some(cap) = re_en_range.captures(&comb_raw) {
                    if let (Ok(s), Ok(e)) = (cap[1].parse::<u32>(), cap[2].parse::<u32>()) {
                        if s <= e && e - s <= 12 {
                            range_pair = Some((s, e));
                            skip_next_en_idx = Some(idx + 1);
                            joined_hdr = Some((format!("{} {}", p.html.trim(), np.html.trim()), comb_raw));
                        }
                    }
                }
            }
            if range_pair.is_none() {
                let re_blank_num = Regex::new(r"\((\d{1,2})\)\s*(?:_+|\.{3,})").unwrap();
                let mut found_nums: Vec<u32> = Vec::new();
                for cap in re_blank_num.captures_iter(&raw_trim) {
                    if let Ok(n) = cap[1].parse::<u32>() {
                        if (1..=60).contains(&n) {
                            found_nums.push(n);
                        }
                    }
                }
                for np in clean_paras[idx + 1..].iter().take(15) {
                    let nt = np.raw.trim();
                    if re_en_passage.is_match(nt) || re_gdpt_part1.is_match(nt) || re_top_prose_break.is_match(nt) {
                        break;
                    }
                    for cap in re_blank_num.captures_iter(nt) {
                        if let Ok(n) = cap[1].parse::<u32>() {
                            if (1..=60).contains(&n) {
                                found_nums.push(n);
                            }
                        }
                    }
                }
                if let (Some(&s), Some(&e)) = (found_nums.iter().min(), found_nums.iter().max()) {
                    if s <= e && e - s <= 12 && found_nums.len() >= 2 {
                        range_pair = Some((s, e));
                    }
                }
            }
            if let Some((s, e)) = range_pair {
                in_solution = false;
                let (hdr_html, hdr_raw) = joined_hdr.unwrap_or_else(|| (p.html.clone(), raw_trim.clone()));
                let re_en_range_already = Regex::new(r"(?i)\b(?:from\s+\d+\s+to\s+\d+|từ\s+câu\s+\d+\s+đến\s+(?:câu\s+)?\d+)\b").unwrap();
                let (final_html, final_raw) = if re_en_range_already.is_match(&hdr_raw) {
                    (hdr_html, hdr_raw)
                } else {
                    (
                        format!(
                            "<i>Đọc đoạn văn sau và trả lời các câu hỏi từ {} đến {}:</i><br/>{}",
                            s, e, hdr_html
                        ),
                        format!(
                            "Đọc đoạn văn sau và trả lời các câu hỏi từ {} đến {}\n{}",
                            s, e, hdr_raw
                        ),
                    )
                };
                exam_blocks.push(DocBlock::Paragraph(ParagraphData {
                    html: final_html,
                    raw: final_raw,
                    provenance: p.provenance.clone(),
                }));
                continue;
            }
        }

        if let Some(q_num) = is_new_question {
            seen_any_q = true;
            seen_q_count += 1;
            in_solution = false;
            active_q_num = Some(q_num);
            expected_next_q = q_num + 1;
            let (cleaned_para, opt_inline_sol) = strip_inline_solution_from_para(p, &re_sol_inline_cut);
            if let Some(sol_txt) = opt_inline_sol {
                seen_per_q_sol = true;
                inline_answers.push((current_part, q_num, sol_txt));
            }
            exam_blocks.push(DocBlock::Paragraph(cleaned_para));
            continue;
        }

        if in_solution {
            if let Some(qn) = active_q_num {
                inline_answers.push((current_part, qn, raw_trim));
            }
            continue;
        }

        if seen_any_q && (re_sol_block_start.is_match(&raw_trim) || re_ans_only_line.is_match(&raw_trim)) {
            in_solution = true;
            seen_per_q_sol = true;
            if let Some(qn) = active_q_num {
                inline_answers.push((current_part, qn, raw_trim));
            }
            continue;
        }

        if !seen_any_q && re_body_start.is_match(&raw_trim) {
            exam_blocks.push(DocBlock::Paragraph(p));
            continue;
        }

        let (cleaned_para, opt_inline_sol) = strip_inline_solution_from_para(p, &re_sol_inline_cut);
        if let Some(sol_txt) = opt_inline_sol {
            in_solution = true;
            seen_per_q_sol = true;
            if let Some(qn) = active_q_num {
                inline_answers.push((current_part, qn, sol_txt));
            }
        }
        if !cleaned_para.raw.trim().is_empty()
            || cleaned_para.html.contains("<img")
            || cleaned_para.html.contains("<math")
        {
            if let Some(qn) = active_q_num {
                inline_answers.push((current_part, qn, cleaned_para.raw.clone()));
            }
            exam_blocks.push(DocBlock::Paragraph(cleaned_para));
        }
    }

    // 1. Ưu tiên bóc bảng đáp án đa mã đề theo tọa độ hình học (x, y) trên trang (ví dụ Mã đề 132/209/357/485)
    let mut answers_map = extract_geometric_answer_tables(extracted);
    if answers_map.len() == 1 && has_explicit_code && answers_map.contains_key("default") {
        if let Some(entry) = answers_map.remove("default") {
            answers_map.insert(default_code.clone(), entry);
        }
    }

    let mut trailing_sol_items = inline_answers;
    trailing_sol_items.extend(collect_inline_sols_from_summary_lines(&top_answer_lines));
    trailing_sol_items.extend(collect_inline_sols_from_summary_lines(&bottom_answer_lines));

    // 2. Bóc bảng đáp án đơn mã từ vùng đầu/cuối hoặc đáp án inline sau mỗi câu
    if answers_map.is_empty() {
        let mut key = ExamAnswerKey::new(&default_code);
        parse_summary_answer_lines(&top_answer_lines, &mut key);
        parse_summary_answer_lines(&bottom_answer_lines, &mut key);
        let summary_cnt = key.part1_answers.len() + key.part2_answers.len() + key.part3_answers.len();
        if summary_cnt > 0 {
            key.backfill_extracted_meta("pdf_answer_table", None);
        }

        // Bóc bổ sung đáp án tường minh dạng "Câu N: ... Chọn X / Đáp án: X" (chỉ điền các câu chưa có trong bảng tổng hợp)
        let mut explicit_inline = ExamAnswerKey::new(&default_code);
        parse_inline_solution_answers(&trailing_sol_items, &mut explicit_inline, true);
        for (qn, ans) in explicit_inline.part1_answers {
            key.part1_answers.entry(qn).or_insert(ans);
        }
        for (qn, tf) in explicit_inline.part2_answers {
            key.part2_answers.entry(qn).or_insert(tf);
        }
        for (qn, ans) in explicit_inline.part3_answers {
            key.part3_answers.entry(qn).or_insert(ans);
        }

        parse_inline_solution_answers(&trailing_sol_items, &mut key, false);
        key.backfill_extracted_meta("pdf_inline_solution", None);

        let total_k = key.part1_answers.len() + key.part2_answers.len() + key.part3_answers.len();
        let has_key_section = !top_answer_lines.is_empty() || (!bottom_answer_lines.is_empty() && summary_cnt > 0);
        if total_k >= 4 || (total_k > 0 && has_key_section) {
            answers_map.insert(default_code, key);
        }
    } else {
        for key in answers_map.values_mut() {
            key.backfill_extracted_meta("pdf_answer_table", None);
        }
        if answers_map.len() == 1 {
            let only_code = answers_map.keys().next().cloned().unwrap_or_default();
            if let Some(key) = answers_map.get_mut(&only_code) {
                let mut explicit_inline = ExamAnswerKey::new(&only_code);
                parse_inline_solution_answers(&trailing_sol_items, &mut explicit_inline, true);
                for (qn, ans) in explicit_inline.part1_answers {
                    key.part1_answers.entry(qn).or_insert(ans);
                }
                for (qn, tf) in explicit_inline.part2_answers {
                    key.part2_answers.entry(qn).or_insert(tf);
                }
                for (qn, ans) in explicit_inline.part3_answers {
                    key.part3_answers.entry(qn).or_insert(ans);
                }
                parse_inline_solution_answers(&trailing_sol_items, key, false);
                key.backfill_extracted_meta("pdf_inline_solution", None);
            }
        }
    }
    let _ = has_ocr_pages;

    (exam_blocks, answers_map, total_visible_chars)
}

fn collect_inline_sols_from_summary_lines(lines: &[String]) -> Vec<(u8, u32, String)> {
    let re_p1 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:I|1)\b|^\s*(?:I|A)\.\s*(?:C[ÂA]U\s*)?TR[ẮA]C\s*NGHI[ỆE]M(?:\s+NHI[ỀE]U\s*PH[ƯU][ƠO]NG\s*[ÁA]N|\s*4\s*L[ỰU]A\s*CH[ỌO]N)?)").unwrap();
    let re_p2 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:II|2)\b|^\s*(?:II|B)\.\s*(?:C[ÂA]U\s*)?(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?[ĐD][ÚU]NG[\s/\-]*SAI|^\s*TR[ẮA]C\s*NGHI[ỆE]M\s*[ĐD][ÚU]NG[\s/\-]*SAI\b)").unwrap();
    let re_p3 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:III|3)\b|^\s*(?:III|C)\.\s*(?:C[ÂA]U\s*)?(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?TR[ẢA]\s*L[ỜO]I\s*NG[ẮA]N|^\s*(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?TR[ẢA]\s*L[ỜO]I\s*NG[ẮA]N\b)").unwrap();
    let re_q = Regex::new(r"(?i)^\s*(?:C[âa]u|Question)\s*(\d{1,2})(?:\s*\([^)]{1,12}\))?\s*[:\.\-]").unwrap();
    let re_en_hdr_q = Regex::new(r"^\s*(\d{1,2})\.\s*([A-D]?)\s*$").unwrap();
    let re_en_cue_next = Regex::new(r"(?i)^\s*(?:Ki[ếe]n\s*th[ứu]c|Ph[uưươ][ơo]ng\s*ph[áa]p|Gi[ảa]i\s*th[íi]ch|L[ờo]i\s*gi[ảa]i|T[ạa]m\s*d[ịi]ch)\s*:?").unwrap();

    let mut out = Vec::new();
    let mut cur_part = 1u8;
    let mut cur_q: Option<u32> = None;

    for (li, l) in lines.iter().enumerate() {
        let t = clean_text(l);
        if t.is_empty() {
            continue;
        }
        if re_p3.is_match(&t) && !re_p2.is_match(&t) {
            cur_part = 3;
            cur_q = None;
            continue;
        } else if re_p2.is_match(&t) {
            cur_part = 2;
            cur_q = None;
            continue;
        } else if re_p1.is_match(&t) {
            cur_part = 1;
            cur_q = None;
            continue;
        }
        if let Some(cap) = re_q.captures(&t) {
            if let Ok(qn) = cap[1].parse::<u32>() {
                cur_q = Some(qn);
            }
        } else if let Some(cap) = re_en_hdr_q.captures(&t) {
            let nxt = lines[li + 1..]
                .iter()
                .map(|nl| clean_text(nl))
                .find(|s| !s.is_empty())
                .unwrap_or_default();
            if re_en_cue_next.is_match(&nxt) {
                if let Ok(qn) = cap[1].parse::<u32>() {
                    if (1..=60).contains(&qn) {
                        cur_q = Some(qn);
                        let ans_ch = cap[2].trim();
                        if !ans_ch.is_empty() {
                            out.push((cur_part, qn, format!("Đáp án: {}", ans_ch)));
                            continue;
                        }
                    }
                }
            }
        }
        if let Some(qn) = cur_q {
            out.push((cur_part, qn, t));
        }
    }
    out
}

#[derive(Debug, Clone)]
struct GeoCell {
    x0: f64,
    x1: f64,
    cx: f64,
    text: String,
}

fn build_geometric_rows(spans: &[PdfSpan]) -> Vec<(f64, Vec<GeoCell>)> {
    let mut sorted: Vec<&PdfSpan> = spans.iter().filter(|s| !s.text.trim().is_empty()).collect();
    sorted.sort_by(|a, b| {
        a.y_base
            .partial_cmp(&b.y_base)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut row_groups: Vec<(f64, Vec<&PdfSpan>)> = Vec::new();
    for sp in sorted {
        if let Some(last) = row_groups.last_mut() {
            if (sp.y_base - last.0).abs() <= 4.5 {
                last.1.push(sp);
                continue;
            }
        }
        row_groups.push((sp.y_base, vec![sp]));
    }

    let mut rows = Vec::with_capacity(row_groups.len());
    for (ry, mut grp) in row_groups {
        grp.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cells: Vec<GeoCell> = Vec::new();
        for sp in grp {
            let gap_thresh = (sp.font_size * 0.95).max(6.5);
            if let Some(last) = cells.last_mut() {
                if sp.x0 - last.x1 <= gap_thresh {
                    if sp.x0 - last.x1 > sp.font_size * 0.22 && !last.text.ends_with(' ') {
                        last.text.push(' ');
                    }
                    last.text.push_str(sp.text.trim());
                    last.x1 = last.x1.max(sp.x1);
                    last.cx = 0.5 * (last.x0 + last.x1);
                    continue;
                }
            }
            cells.push(GeoCell {
                x0: sp.x0,
                x1: sp.x1,
                cx: 0.5 * (sp.x0 + sp.x1),
                text: sp.text.trim().to_string(),
            });
        }
        if !cells.is_empty() {
            rows.push((ry, cells));
        }
    }
    rows
}

fn extract_geometric_answer_tables(extracted: &PdfDocExtracted) -> HashMap<String, ExamAnswerKey> {
    let re_variant_code = Regex::new(r"(?i)^(?:M[ãa]\s*[đd][ềe]\s*)?(\d{3,4})$").unwrap();
    let re_p1 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:I|1)\b|^\s*(?:I|A)\.\s*(?:C[ÂA]U\s*)?TR[ẮA]C\s*NGHI[ỆE]M(?:\s+NHI[ỀE]U\s*PH[ƯU][ƠO]NG\s*[ÁA]N|\s*4\s*L[ỰU]A\s*CH[ỌO]N)?)").unwrap();
    let re_p2 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:II|2)\b|^\s*(?:II|B)\.\s*(?:C[ÂA]U\s*)?(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?[ĐD][ÚU]NG[\s/\-]*SAI|^\s*TR[ẮA]C\s*NGHI[ỆE]M\s*[ĐD][ÚU]NG[\s/\-]*SAI\b)").unwrap();
    let re_p3 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:III|3)\b|^\s*(?:III|C)\.\s*(?:C[ÂA]U\s*)?(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?TR[ẢA]\s*L[ỜO]I\s*NG[ẮA]N|^\s*(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?TR[ẢA]\s*L[ỜO]I\s*NG[ẮA]N\b)").unwrap();
    let re_stop = Regex::new(
        r"(?i)^\s*(?:H[ƯU][ỚO]NG\s*D[ẪA]N\s*(?:L[ỜO]I\s*)?GI[ẢA]I|L[ỜO]I\s*GI[ẢA]I\s*CHI\s*TI[ẾE]T)"
    ).unwrap();
    let re_tf_combo = Regex::new(r"^(?i)[ĐDđdSsTtFf01]{4}$").unwrap();
    let re_prose_boundary = Regex::new(
        r"(?i)^(?:M\s*[ĐDÐ]\s*\d{3,4}\b|M[ÃA]\s*[ĐD][ỀE]\s*\d{3,4}\s*[:\.]|C[ÂA]U\s*\d{1,3}\s*[:\.]|PH[ẦA]N\s*(?:IV|4)\b|T[ỰU]\s*LU[ẬA]N\b|L[OỜƠ]I\s*GI[ẢA]I\b|CH[ỦU]\s*[ĐD][ỀE]\b)"
    ).unwrap();

    #[derive(Clone)]
    struct VariantCol {
        code: String,
        cx: f64,
        left_x: f64,
        right_x: f64,
    }

    let mut answers_map: HashMap<String, ExamAnswerKey> = HashMap::new();
    let mut active_cols: Vec<VariantCol> = Vec::new();
    let mut cur_part: u8 = 1;
    let mut cur_p2_q: Option<u32> = None;
    let mut p2_acc: HashMap<(String, u32), [Option<bool>; 4]> = HashMap::new();
    let mut active_page: Option<usize> = None;

    for (p_idx, pg) in extracted.pages.iter().enumerate() {
        let rows = build_geometric_rows(&pg.spans);
        let mut r_idx = 0usize;
        while r_idx < rows.len() {
            let (ry, cells) = &rows[r_idx];
            let row_text = cells
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let row_clean = clean_text(&row_text);

            // Kiểm tra hàng tiêu đề cột mã đề: e.g. `Câu | Mã đề 132 | Mã đề 209 | Mã đề 357 | Mã đề 485`
            let explicit_header = cells.iter().any(|c| {
                let tl = clean_text(&c.text).to_lowercase();
                tl.contains("mã đề") || tl.contains("đề thi") || tl.starts_with("đề")
            });
            let mut found_codes: Vec<(String, f64)> = Vec::new();
            for c in cells {
                let tc = clean_text(&c.text);
                if tc.contains("Mã đề") || tc.contains("MÃ ĐỀ") || tc.contains("Đề") {
                    for tok in tc.split_whitespace() {
                        if tok.chars().all(|ch| ch.is_ascii_digit()) && (3..=4).contains(&tok.len()) {
                            found_codes.push((tok.to_string(), c.cx));
                        }
                    }
                } else if let Some(cap) = re_variant_code.captures(&tc) {
                    found_codes.push((cap[1].to_string(), c.cx));
                }
            }
            if found_codes.len() == 1 && cells.iter().any(|c| clean_text(&c.text).to_lowercase().contains("mã đề")) {
                if let Some((ny, ncells)) = rows.get(r_idx + 1) {
                    if ny - ry <= 24.0 {
                        let mut next_codes = Vec::new();
                        for nc in ncells {
                            let ntc = clean_text(&nc.text);
                            if let Some(cap) = re_variant_code.captures(&ntc) {
                                next_codes.push((cap[1].to_string(), nc.cx));
                            }
                        }
                        if next_codes.len() >= 2 {
                            found_codes = next_codes;
                            r_idx += 1;
                        }
                    }
                }
            }

            let uniq_codes: HashSet<&str> = found_codes.iter().map(|(c, _)| c.as_str()).collect();
            let first_found_x = found_codes.iter().map(|(_, x)| *x).fold(f64::INFINITY, f64::min);
            let has_leading_qnum = !active_cols.is_empty()
                && !explicit_header
                && cells.iter().any(|c| {
                    c.cx < first_found_x - 8.0
                        && parse_question_number(&clean_text(&c.text))
                            .is_some_and(|qn| (1..=60).contains(&qn))
                });
            let data_row_false_header = !explicit_header
                && (uniq_codes.len() != found_codes.len() || has_leading_qnum);

            if found_codes.len() >= 2 && !data_row_false_header {
                found_codes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                let spacing_ok = found_codes.windows(2).all(|w| w[1].1 - w[0].1 >= 22.0);
                if spacing_ok {
                    let mut cols = Vec::with_capacity(found_codes.len());
                    for k in 0..found_codes.len() {
                        let cx = found_codes[k].1;
                        let left_x = if k == 0 {
                            let sp0 = (found_codes[1].1 - cx) * 0.5;
                            cx - sp0.min(48.0)
                        } else {
                            0.5 * (found_codes[k - 1].1 + cx)
                        };
                        let right_x = if k + 1 == found_codes.len() {
                            let sp1 = (cx - found_codes[k - 1].1) * 0.5;
                            cx + sp1.min(48.0)
                        } else {
                            0.5 * (cx + found_codes[k + 1].1)
                        };
                        cols.push(VariantCol {
                            code: found_codes[k].0.clone(),
                            cx,
                            left_x,
                            right_x,
                        });
                        answers_map
                            .entry(found_codes[k].0.clone())
                            .or_insert_with(|| ExamAnswerKey::new(&found_codes[k].0));
                    }
                    let preserve_part = !active_cols.is_empty() && cur_part > 1;
                    active_cols = cols;
                    if !preserve_part {
                        cur_part = 1;
                    }
                    cur_p2_q = None;
                    active_page = Some(p_idx + 1);
                    r_idx += 1;
                    continue;
                }
            }

            if active_cols.is_empty() {
                r_idx += 1;
                continue;
            }

            let trim_row = row_clean.trim_start();
            let starts_md_or_q = re_prose_boundary.is_match(trim_row);
            if re_stop.is_match(&row_clean)
                || starts_md_or_q
                || (row_clean.chars().count() > 110
                    && !row_clean.contains("PHẦN")
                    && !row_clean.contains("Phần"))
            {
                active_cols.clear();
                r_idx += 1;
                continue;
            }

            if re_p3.is_match(&row_clean) && !re_p2.is_match(&row_clean) {
                cur_part = 3;
                cur_p2_q = None;
                r_idx += 1;
                continue;
            } else if re_p2.is_match(&row_clean) {
                cur_part = 2;
                cur_p2_q = None;
                r_idx += 1;
                continue;
            } else if re_p1.is_match(&row_clean) {
                cur_part = 1;
                cur_p2_q = None;
                r_idx += 1;
                continue;
            }

            let first_col_left = active_cols[0].left_x;
            let mut row_qn: Option<u32> = None;
            let mut row_sub: Option<usize> = None;
            for c in cells {
                if c.cx < first_col_left {
                    let tc = clean_text(&c.text);
                    if let Some(qn) = parse_question_number(&tc) {
                        row_qn = Some(qn);
                    }
                    let low = tc.trim_end_matches(&['.', ')', ':'][..]).to_ascii_lowercase();
                    row_sub = match low.as_str() {
                        "a" => Some(0),
                        "b" => Some(1),
                        "c" => Some(2),
                        "d" => Some(3),
                        _ => row_sub,
                    };
                    if row_qn.is_none() || row_sub.is_none() {
                        for tok in tc.split_whitespace() {
                            if let Some(qn) = parse_question_number(tok) {
                                row_qn = Some(qn);
                            }
                            let tl = tok.trim_end_matches(&['.', ')', ':'][..]).to_ascii_lowercase();
                            if let Some(si) = match tl.as_str() {
                                "a" => Some(0),
                                "b" => Some(1),
                                "c" => Some(2),
                                "d" => Some(3),
                                _ => None,
                            } {
                                row_sub = Some(si);
                            }
                        }
                    }
                }
            }

            if let Some(qn) = row_qn {
                if cur_part == 2 {
                    cur_p2_q = Some(qn);
                }
            } else if cur_part == 2 && row_sub == Some(0) && cur_p2_q.is_none() {
                cur_p2_q = Some(1);
            }

            let target_qn = if cur_part == 2 && row_sub.is_some() {
                row_qn.or(cur_p2_q)
            } else {
                row_qn
            };

            if let Some(qn) = target_qn {
                for vc in &active_cols {
                    let mut best_cell: Option<&GeoCell> = None;
                    let mut best_dx = f64::INFINITY;
                    for c in cells {
                        if c.cx >= vc.left_x && c.cx <= vc.right_x {
                            let dx = (c.cx - vc.cx).abs();
                            if dx < best_dx {
                                best_dx = dx;
                                best_cell = Some(c);
                            }
                        }
                    }
                    let Some(cell) = best_cell else {
                        continue;
                    };
                    let val = clean_text(&cell.text);
                    if val.is_empty() {
                        continue;
                    }
                    let Some(key) = answers_map.get_mut(&vc.code) else {
                        continue;
                    };

                    if cur_part == 2 {
                        let compact = val.replace(&[' ', ',', ';', '/'][..], "");
                        if re_tf_combo.is_match(&compact) {
                            assign_answer_cell(key, qn, &compact, &re_tf_combo);
                        } else if let (Some(si), Some(tf_val)) = (row_sub, parse_single_tf_cell(Some(&val))) {
                            let entry = p2_acc.entry((vc.code.clone(), qn)).or_insert([None; 4]);
                            entry[si] = Some(tf_val);
                        }
                    } else if cur_part == 3 {
                        if let Some(num) = extract_number(&val) {
                            key.part3_answers.insert(qn, num);
                        }
                    } else {
                        assign_answer_cell(key, qn, &val, &re_tf_combo);
                    }
                }
                if cur_part == 2 && row_sub == Some(3) {
                    cur_p2_q = Some(qn + 1);
                }
            }

            r_idx += 1;
        }
    }

    for ((code, qn), arr) in p2_acc {
        if let (Some(a), Some(b), Some(c), Some(d)) = (arr[0], arr[1], arr[2], arr[3]) {
            if let Some(key) = answers_map.get_mut(&code) {
                key.part2_answers.insert(qn, TrueFalseAnswer { a, b, c, d });
            }
        }
    }

    answers_map.retain(|_, k| {
        !k.part1_answers.is_empty() || !k.part2_answers.is_empty() || !k.part3_answers.is_empty()
    });
    if let Some(pg_num) = active_page {
        for key in answers_map.values_mut() {
            key.backfill_extracted_meta("pdf_answer_table", Some(pg_num));
        }
    }
    answers_map
}

fn normalize_leading_cau_para(p: ParagraphData, re_ocr_cau: &Regex) -> ParagraphData {
    let tv = TokenView::build(&p.html);
    if let Some(cap) = re_ocr_cau.captures(&tv.flat) {
        let matched = cap.get(0).unwrap();
        let norm_match = tv.flat[matched.start()..matched.end()].trim_start();
        if norm_match.starts_with("Câu ") && !norm_match.contains('(') {
            return p;
        }
        let qn = &cap[1];
        let punct = &cap[2];
        let after_idx = matched.end();
        let tail_html = clean_edge_html(&tv.slice_html(&p.html, after_idx, tv.flat.len()));
        let tail_raw = tv.slice_plain(after_idx, tv.flat.len()).trim().to_string();
        return ParagraphData {
            html: format!("<b>Câu {}{}</b> {}", qn, punct, tail_html),
            raw: format!("Câu {}{} {}", qn, punct, tail_raw),
            provenance: p.provenance,
        };
    }
    p
}

fn prepend_cau_prefix_to_numbered_para(p: ParagraphData) -> ParagraphData {
    let tv = TokenView::build(&p.html);
    if let Some(cap) = Regex::new(r"^\s*(\d{1,2})\s*\.(?:\s+|$)").unwrap().captures(&tv.flat) {
        let qn = &cap[1];
        let after_idx = cap.get(0).map(|m| m.end()).unwrap_or(0);
        let tail_html = clean_edge_html(&tv.slice_html(&p.html, after_idx, tv.flat.len()));
        let tail_raw = tv.slice_plain(after_idx, tv.flat.len()).trim().to_string();
        return ParagraphData {
            html: format!("<b>Câu {}.</b> {}", qn, tail_html),
            raw: format!("Câu {}. {}", qn, tail_raw),
            provenance: p.provenance,
        };
    }
    p
}

fn strip_inline_solution_from_para(
    p: ParagraphData,
    re_sol_inline_cut: &Regex,
) -> (ParagraphData, Option<String>) {
    let tv = TokenView::build(&p.html);
    if let Some(m) = re_sol_inline_cut.find(&tv.flat) {
        let sol_txt = tv.slice_plain(m.start(), tv.flat.len()).trim().to_string();
        let before_html = clean_edge_html(&tv.slice_html(&p.html, 0, m.start()));
        let before_raw = tv.slice_plain(0, m.start()).trim().to_string();
        return (
            ParagraphData {
                html: before_html,
                raw: before_raw,
                provenance: p.provenance,
            },
            Some(sol_txt),
        );
    }
    (p, None)
}

// ============================================================================
// 6. Bóc tách Bảng đáp án đầu/cuối & Đáp án inline trong lời giải
// ============================================================================

fn clean_short_text_answer(raw: &str, allow_non_ascii: bool) -> Option<String> {
    let ans = raw.trim();
    if ans.is_empty()
        || ans.contains(':')
        || ans.chars().count() > 120
        || ans.split_whitespace().count() > 18
        || matches!(ans, "A" | "B" | "C" | "D")
    {
        return None;
    }
    let alow = ans.to_lowercase();
    let is_prompt = alow.starts_with("read the")
        || alow.starts_with("mark the")
        || alow.starts_with("choose ")
        || alow.starts_with("circle ")
        || alow.starts_with("find the")
        || alow.starts_with("complete ")
        || alow.starts_with("rewrite ")
        || alow.starts_with("listen ")
        || alow.starts_with("write ")
        || alow.starts_with("giải thích")
        || alow.starts_with("phương pháp")
        || alow.starts_with("hướng dẫn")
        || alow.starts_with("a.")
        || alow.starts_with("b.")
        || alow.starts_with("c.")
        || alow.starts_with("d.");
    if is_prompt {
        return None;
    }
    let is_wh_question = ans.ends_with('?')
        && (alow.starts_with("what ")
            || alow.starts_with("which ")
            || alow.starts_with("who ")
            || alow.starts_with("whom ")
            || alow.starts_with("whose ")
            || alow.starts_with("where ")
            || alow.starts_with("when ")
            || alow.starts_with("why ")
            || alow.starts_with("how ")
            || alow.starts_with("vì sao")
            || alow.starts_with("tại sao")
            || alow.starts_with("như thế nào")
            || alow.starts_with("là gì")
            || alow.starts_with("bao nhiêu"));
    if is_wh_question {
        return None;
    }
    let ascii_or_quote = ans
        .chars()
        .all(|c| c.is_ascii() || "’‘“”–—°%".contains(c));
    if !allow_non_ascii && !ascii_or_quote {
        return None;
    }
    Some(ans.to_string())
}

pub(crate) fn parse_summary_answer_lines(lines: &[String], key: &mut ExamAnswerKey) {
    if lines.is_empty() {
        return;
    }
    let re_p1 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:I|1)\b|^\s*(?:I|A)\.\s*(?:C[ÂA]U\s*)?TR[ẮA]C\s*NGHI[ỆE]M(?:\s+NHI[ỀE]U\s*PH[ƯU][ƠO]NG\s*[ÁA]N|\s*4\s*L[ỰU]A\s*CH[ỌO]N)?)").unwrap();
    let re_p2 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:II|2)\b|^\s*(?:II|B)\.\s*(?:C[ÂA]U\s*)?(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?[ĐD][ÚU]NG[\s/\-]*SAI|^\s*TR[ẮA]C\s*NGHI[ỆE]M\s*[ĐD][ÚU]NG[\s/\-]*SAI\b)").unwrap();
    let re_p3 = Regex::new(r"(?i)(?:PH[ẦA]N\s*(?:III|3)\b|^\s*(?:III|C)\.\s*(?:C[ÂA]U\s*)?(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?TR[ẢA]\s*L[ỜO]I\s*NG[ẮA]N|^\s*(?:TR[ẮA]C\s*NGHI[ỆE]M\s*)?TR[ẢA]\s*L[ỜO]I\s*NG[ẮA]N\b)").unwrap();
    let re_tf_combo = Regex::new(r"^(?i)[ĐDđdSsTtFf01]{4}$").unwrap();
    let re_inline_pair = Regex::new(r"\b(\d{1,2})\s*[:\.\-\t\s]*([A-D]|[ĐSđs]{4})\b").unwrap();
    let re_cau_ans_pair = Regex::new(r"\b[Cc][âaÂA][uU]\s*(\d{1,2})\s*[:\.\-\t\s]+([A-D]|[ĐSđs]{4})\b").unwrap();
    let re_p3_pair = Regex::new(r"(?i)\b(\d{1,2})\s*[:\.\-\t=]+\s*(-?\d+(?:[.,]\d+)?)\b").unwrap();
    let re_short_text_ans = Regex::new(r#"^\s*(\d{1,2})\.\s*([0-9A-Za-zÀ-ỹ"“‘'].*)\s*$"#).unwrap();
    let re_guide_stop = Regex::new(
        r"(?i)^\s*(?:H[UƯ][OỚƠ]NG\s*D[AẪ]N\s*(?:L[OỜƠ]I\s*)?GI[ẢA]I\s*CH[IÌỈĨỊ]\s*TI[ÊẾE]T|L[OỜƠ]I\s*GI[ẢA]I\s*CH[IÌỈĨỊ]\s*TI[ÊẾE]T|[ĐD][ÁA]P\s*[ÁA]N\s*CH[IÌỈĨỊ]\s*TI[ÊẾE]T)\b"
    ).unwrap();
    let re_cau_prose_start = Regex::new(r"^\s*Câu\s*\d").unwrap();

    let mut cur_part: u8 = 1;
    let mut pending_nums: Vec<u32> = Vec::new();
    let mut pending_single_q: Option<u32> = None;
    // Dùng cho bảng Phần II dạng cột: Câu 1..4 ở hàng 1, sau đó 4 hàng a/b/c/d
    let mut p2_col_qs: Vec<u32> = Vec::new();
    let mut p2_col_vals: HashMap<u32, [Option<bool>; 4]> = HashMap::new();
    // Dùng cho bảng Phần II dạng Loigiaihay ghép cặp cột: `1 a S 2 a S` hoặc gộp ô dọc `a Đ a Đ \n b S b Đ \n 1 3 \n c Đ c S \n d S d Đ`
    let mut p2_pair_qs: Vec<u32> = Vec::new();
    let mut p2_pending_block: Vec<(usize, Vec<bool>)> = Vec::new();
    let mut vert_qs: Vec<u32> = Vec::new();
    let mut vert_collecting_qs = false;
    let mut vert_collecting_vals = false;

    for line in lines {
        let t = clean_text(line);
        if t.is_empty() {
            continue;
        }
        if t.to_lowercase() == "câu" {
            vert_qs.clear();
            vert_collecting_qs = true;
            vert_collecting_vals = false;
            continue;
        }
        if vert_collecting_qs {
            if let Ok(qn) = t.parse::<u32>() {
                if (1..=60).contains(&qn) {
                    vert_qs.push(qn);
                    continue;
                }
            } else if t.to_lowercase().replace(' ', "") == "đápán" {
                vert_collecting_qs = false;
                vert_collecting_vals = true;
                continue;
            } else {
                vert_collecting_qs = false;
            }
        }
        if vert_collecting_vals {
            if !vert_qs.is_empty() {
                let qn = vert_qs.remove(0);
                if cur_part == 2 {
                    assign_answer_cell(key, qn, &t, &re_tf_combo);
                } else if cur_part == 3 {
                    if let Some(num) = extract_number(&t) {
                        key.part3_answers.insert(qn, num);
                    }
                } else if cur_part == 1 {
                    let up = t.trim().to_uppercase();
                    if matches!(up.as_str(), "A" | "B" | "C" | "D") {
                        key.part1_answers.insert(qn, up);
                    }
                }
                if vert_qs.is_empty() {
                    vert_collecting_vals = false;
                }
                continue;
            } else {
                vert_collecting_vals = false;
            }
        }
        let summary_keys_so_far =
            key.part1_answers.len() + key.part3_answers.len() + p2_col_vals.len();
        let low_t = t.to_lowercase();
        if summary_keys_so_far >= 4
            && (re_guide_stop.is_match(&t)
                || low_t.starts_with("lời giải chi tiết")
                || low_t.starts_with("hướng dẫn giải chi tiết")
                || low_t.starts_with("cách giải:")
                || low_t.starts_with("phương pháp:"))
        {
            if cur_part < 3 && lines.iter().any(|l| re_p3.is_match(l)) {
                continue;
            }
            break;
        }
        if re_guide_stop.is_match(&t)
            || low_t.starts_with("hướng dẫn giải")
            || low_t.starts_with("lời giải")
        {
            continue;
        }
        if summary_keys_so_far >= 4 {
            if low_t.contains("thí sinh trả lời từ câu")
                || low_t.starts_with("mỗi câu hỏi thí sinh chỉ")
            {
                break;
            }
            let cau_q_cnt = t
                .split_whitespace()
                .filter_map(|tk| parse_question_number(tk))
                .count();
            if re_cau_prose_start.is_match(&t)
                && !low_t.contains("đáp án")
                && !re_cau_ans_pair.is_match(&t)
                && (t.chars().count() >= 45 || (cau_q_cnt < 2 && t.contains(':') && (t.contains('(') || cur_part == 2)))
            {
                if cur_part < 3 && lines.iter().any(|l| re_p3.is_match(l)) {
                    continue;
                }
                break;
            }
        }
        if re_p3.is_match(&t) && !re_p2.is_match(&t) {
            cur_part = 3;
            pending_nums.clear();
            pending_single_q = None;
            p2_pair_qs.clear();
            p2_pending_block.clear();
            continue;
        } else if re_p2.is_match(&t) {
            cur_part = 2;
            pending_nums.clear();
            pending_single_q = None;
            p2_pair_qs.clear();
            p2_pending_block.clear();
            continue;
        } else if re_p1.is_match(&t) {
            cur_part = 1;
            pending_nums.clear();
            pending_single_q = None;
            p2_pair_qs.clear();
            p2_pending_block.clear();
            continue;
        }

        let tokens: Vec<String> = t.split_whitespace().map(|s| s.to_string()).collect();
        if tokens.is_empty() {
            continue;
        }

        if cur_part == 2 {
            // Định dạng Loigiaihay 2 cột: `1 a S 2 a S` followed by `b Đ b Đ`, `c Đ c Đ`, `d Đ d Đ`
            let mut parsed_triplet_row = false;
            if tokens.len() % 3 == 0 && tokens.len() >= 3 {
                let mut row_qs = Vec::new();
                let mut ok = true;
                for chunk in tokens.chunks_exact(3) {
                    let qn = parse_question_number(&chunk[0]);
                    let sub_lbl = chunk[1].trim_end_matches(&['.', ')', ':'][..]).to_ascii_lowercase();
                    let tf = parse_single_tf_cell(Some(&chunk[2]));
                    if let (Some(q), true, Some(v)) = (qn, sub_lbl == "a", tf) {
                        row_qs.push((q, v));
                    } else {
                        ok = false;
                        break;
                    }
                }
                if ok && !row_qs.is_empty() {
                    p2_pair_qs.clear();
                    p2_pending_block.clear();
                    for (q, v) in row_qs {
                        p2_pair_qs.push(q);
                        let entry = p2_col_vals.entry(q).or_insert([None; 4]);
                        entry[0] = Some(v);
                    }
                    parsed_triplet_row = true;
                }
            }
            if parsed_triplet_row {
                continue;
            }

            // Dạng gộp ô dọc Loigiaihay: `a Đ a Đ` / `b S b Đ` / `1 3` / `c Đ c S` / `d S d Đ`
            if tokens.len() >= 2 && tokens.len() % 2 == 0 {
                let mut same_sub: Option<usize> = None;
                let mut row_vals: Vec<bool> = Vec::with_capacity(tokens.len() / 2);
                let mut ok = true;
                for chunk in tokens.chunks_exact(2) {
                    let sub_lbl = chunk[0].trim_end_matches(&['.', ')', ':'][..]).to_ascii_lowercase();
                    let sub_idx = match sub_lbl.as_str() {
                        "a" => Some(0),
                        "b" => Some(1),
                        "c" => Some(2),
                        "d" => Some(3),
                        _ => None,
                    };
                    if let (Some(si), Some(v)) = (sub_idx, parse_single_tf_cell(Some(&chunk[1]))) {
                        if let Some(prev) = same_sub {
                            if prev != si {
                                ok = false;
                                break;
                            }
                        } else {
                            same_sub = Some(si);
                        }
                        row_vals.push(v);
                    } else {
                        ok = false;
                        break;
                    }
                }
                if ok && row_vals.len() >= 2 {
                    if let Some(si) = same_sub {
                        if si == 0 && (tokens.len() >= 4 || p2_pair_qs.is_empty()) {
                            p2_pair_qs.clear();
                            p2_pending_block.clear();
                            p2_pending_block.push((0, row_vals));
                            continue;
                        } else if p2_pair_qs.is_empty()
                            && p2_pending_block
                                .first()
                                .is_some_and(|(_, first_vals)| first_vals.len() == row_vals.len())
                        {
                            p2_pending_block.push((si, row_vals));
                            continue;
                        } else if !p2_pair_qs.is_empty() && row_vals.len() == p2_pair_qs.len() {
                            for (col_idx, v) in row_vals.into_iter().enumerate() {
                                if let Some(&q) = p2_pair_qs.get(col_idx) {
                                    let entry = p2_col_vals.entry(q).or_insert([None; 4]);
                                    entry[si] = Some(v);
                                }
                            }
                            continue;
                        }
                    }
                }
            }

            if !p2_pending_block.is_empty() && p2_pair_qs.is_empty() {
                let q_cols: Vec<u32> = tokens
                    .iter()
                    .filter_map(|tk| parse_question_number(tk))
                    .collect();
                if q_cols.len() == tokens.len() && q_cols.len() == p2_pending_block[0].1.len() {
                    p2_pair_qs = q_cols;
                    for (si, row_vals) in p2_pending_block.drain(..) {
                        for (col_idx, v) in row_vals.into_iter().enumerate() {
                            if let Some(&q) = p2_pair_qs.get(col_idx) {
                                let entry = p2_col_vals.entry(q).or_insert([None; 4]);
                                entry[si] = Some(v);
                            }
                        }
                    }
                    continue;
                }
            }

            // Trường hợp Phần II dạng cột (`Câu 1 2 3 4` hoặc `1 2 3 4`, tiếp theo là `a) Đ S Đ Đ`)
            let sub_key_first = tokens[0].trim_end_matches(&['.', ')', ':'][..]).to_ascii_lowercase();
            if matches!(sub_key_first.as_str(), "a" | "b" | "c" | "d") && tokens.len() >= 2 {
                let sub_idx = match sub_key_first.as_str() {
                    "a" => 0,
                    "b" => 1,
                    "c" => 2,
                    _ => 3,
                };
                if p2_col_qs.is_empty() && pending_nums.len() >= 2 {
                    p2_col_qs = std::mem::take(&mut pending_nums);
                }
                let all_tf_vals = tokens[1..].iter().all(|vt| parse_single_tf_cell(Some(vt)).is_some());
                if !p2_col_qs.is_empty() && all_tf_vals && tokens.len() - 1 <= p2_col_qs.len() {
                    for (col_i, val_tok) in tokens[1..].iter().enumerate() {
                        if let (Some(&qn), Some(tf_val)) =
                            (p2_col_qs.get(col_i), parse_single_tf_cell(Some(val_tok)))
                        {
                            let entry = p2_col_vals.entry(qn).or_insert([None; 4]);
                            entry[sub_idx] = Some(tf_val);
                        }
                    }
                    continue;
                }
            }

            // Hàng header số câu của Phần II (`Câu 1 2 3 4` hoặc `Câu 1 Câu 2`)
            if tokens.iter().any(|tk| tk.to_lowercase() == "câu")
                && tokens.iter().filter_map(|tk| parse_question_number(tk)).count() >= 2
            {
                let qs: Vec<u32> = tokens
                    .iter()
                    .filter(|tk| tk.to_lowercase() != "câu")
                    .filter_map(|tk| parse_question_number(tk))
                    .collect();
                if !qs.is_empty() {
                    p2_col_qs = qs.clone();
                    pending_nums = qs;
                }
                continue;
            }
        }

        // Phần 3 dạng cặp 2 cột từng hàng: `1 500` hoặc `3 17320` hoặc `1  3,3  4  5` hoặc `1: 10,4  2: -5`
        if cur_part == 3 {
            if tokens.len() >= 2 && tokens.len() % 2 == 0 && !t.contains(':') {
                let mut parsed_pairs = Vec::new();
                let mut all_ok = true;
                for pair in tokens.chunks_exact(2) {
                    if let (Some(qn), Some(num)) = (parse_question_number(&pair[0]), extract_number(&pair[1])) {
                        if (1..=30).contains(&qn) {
                            parsed_pairs.push((qn, num));
                        } else {
                            all_ok = false;
                            break;
                        }
                    } else {
                        all_ok = false;
                        break;
                    }
                }
                if all_ok && !parsed_pairs.is_empty() {
                    for (qn, num) in parsed_pairs {
                        key.part3_answers.insert(qn, num);
                    }
                    continue;
                }
            }
            {
                let p3_caps: Vec<_> = re_p3_pair.captures_iter(&t).collect();
                if p3_caps.len() >= 2 || (p3_caps.len() == 1 && tokens.len() <= 3) {
                    let mut inserted = false;
                    for cap in p3_caps {
                        if let (Ok(qn), Some(num)) = (cap[1].parse::<u32>(), extract_number(&cap[2])) {
                            key.part3_answers.insert(qn, num);
                            inserted = true;
                        }
                    }
                    if inserted {
                        continue;
                    }
                }
            }
        }

        // Trường hợp `"Câu 1: D Câu 2: A"` hoặc `"1. D"` trên dòng riêng
        let cau_caps: Vec<_> = re_cau_ans_pair.captures_iter(&t).collect();
        if (cur_part == 1 || cur_part == 2) && (cau_caps.len() >= 2 || (cau_caps.len() == 1 && tokens.len() <= 4)) {
            for cap in cau_caps {
                if let Ok(qn) = cap[1].parse::<u32>() {
                    assign_answer_cell(key, qn, &cap[2], &re_tf_combo);
                }
            }
            continue;
        }

        let has_noisy_ocr_grid = t.contains('|')
            && t.split(|c: char| c.is_whitespace() || c == '|').any(|seg| {
                let w = seg.trim_matches(|ch: char| !ch.is_alphanumeric());
                let ascii_alpha_cnt = w.chars().filter(|c| c.is_ascii_alphabetic()).count();
                let wl = w.to_lowercase();
                ascii_alpha_cnt >= 2
                    && !matches!(
                        wl.as_str(),
                        "câu" | "cau" | "phần" | "phan" | "mã" | "ma" | "đề" | "de"
                            | "đáp" | "dap" | "án" | "an" | "đúng" | "dung" | "sai"
                            | "true" | "false" | "stt" | "i" | "ii" | "iii" | "iv"
                    )
            });

        let re_multi_num = Regex::new(r"(?:^|\s)(\d{1,2})\.\s+").unwrap();
        let multi_num_cnt = if !has_noisy_ocr_grid
            && pending_nums.is_empty()
            && !t.to_lowercase().starts_with("câu ")
            && !t.to_lowercase().starts_with("question ")
        {
            let ms = re_multi_num.find_iter(&t).collect::<Vec<_>>();
            if ms.len() >= 2 && ms[0].start() == 0 {
                ms.len()
            } else {
                0
            }
        } else {
            0
        };
        let has_mixed_text_multi = multi_num_cnt >= 2
            && t.split_whitespace().any(|tok| {
                let st = tok.trim_end_matches(&['.', ',', ';'][..]);
                let up = st.to_uppercase();
                !st.is_empty()
                    && !st.chars().all(|c| c.is_ascii_digit())
                    && !matches!(up.as_str(), "A" | "B" | "C" | "D")
                    && !re_tf_combo.is_match(st)
            });

        // Trường hợp 1 hàng cặp xen kẽ (`1 D 2 B 3 B` hoặc `1D 2B 3C` hoặc `1.D 2.B` hoặc `1. ĐSĐĐ 2. SSĐĐ`)
        if (cur_part == 1 || cur_part == 2) && !has_noisy_ocr_grid && !has_mixed_text_multi {
            let caps: Vec<_> = re_inline_pair.captures_iter(&t).collect();
            if caps.len() >= 2 || (caps.len() == 1 && tokens.len() <= 2) {
                for cap in caps {
                    if let Ok(qn) = cap[1].parse::<u32>() {
                        assign_answer_cell(key, qn, &cap[2], &re_tf_combo);
                    }
                }
                continue;
            }
        }

        // Nhiều đáp án ngắn đánh số trên cùng 1 dòng (`1. true 2. false` hoặc `1. True 6. responsibilities 11. C` hoặc `21. regularly 22. unhealthy`)
        if multi_num_cnt >= 2 {
            let ms: Vec<_> = re_multi_num.find_iter(&t).collect();
            if ms.len() >= 2 && ms[0].start() == 0 {
                let mut parsed_multi: Vec<(u32, String)> = Vec::new();
                for i in 0..ms.len() {
                    if let Some(cap) = re_multi_num.captures(&t[ms[i].start()..ms[i].end()]) {
                        if let Ok(qn) = cap[1].parse::<u32>() {
                            let start = ms[i].end();
                            let end = if i + 1 < ms.len() { ms[i + 1].start() } else { t.len() };
                            let val = t[start..end].trim().trim_end_matches(&[';', ',', '.'][..]).trim();
                            let vlow = val.to_lowercase();
                            let looks_stem = vlow.starts_with("read the")
                                || vlow.starts_with("mark the")
                                || vlow.contains(" a.")
                                || vlow.contains(" b.");
                            if (1..=60).contains(&qn)
                                && !val.is_empty()
                                && !looks_stem
                                && !val.contains(':')
                                && !val.ends_with('?')
                                && val.chars().count() <= 80
                            {
                                parsed_multi.push((qn, val.to_string()));
                            }
                        }
                    }
                }
                if parsed_multi.len() >= 2 {
                    for (qn, val) in parsed_multi {
                        let no_space = val.replace(' ', "");
                        let vu = no_space.to_uppercase();
                        if matches!(vu.as_str(), "A" | "B" | "C" | "D") {
                            key.part1_answers.insert(qn, vu);
                        } else if re_tf_combo.is_match(&no_space) {
                            assign_answer_cell(key, qn, &val, &re_tf_combo);
                        } else {
                            key.part3_answers.insert(qn, val);
                        }
                    }
                    continue;
                }
            }
        }

        // Đáp án trắc nghiệm / điền từ Tiếng Anh ngắn trên bảng tổng hợp (`1. approximately 70 years`, `20. healthy`)
        if (cur_part == 1 || cur_part == 3) && !has_noisy_ocr_grid && pending_nums.is_empty() {
            if let Some(cap) = re_short_text_ans.captures(&t) {
                if let Ok(qn) = cap[1].parse::<u32>() {
                    if (1..=60).contains(&qn) && !key.part1_answers.contains_key(&qn) {
                        if let Some(clean) = clean_short_text_answer(&cap[2], cur_part == 3) {
                            key.part3_answers.insert(qn, clean);
                            continue;
                        }
                    }
                }
            }
        }

        // Trường hợp hàng số câu theo sau bởi hàng đáp án (`1 2 3 4` -> `D B B A` hoặc `500 ... 17320`)
        let nums_only: Vec<u32> = tokens
            .iter()
            .filter(|tk| tk.to_lowercase() != "câu")
            .filter_map(|tk| {
                let stripped = tk.trim_end_matches(&['.', ':'][..]);
                if stripped.chars().all(|c| c.is_ascii_digit()) {
                    stripped.parse::<u32>().ok().filter(|&n| (1..=60).contains(&n))
                } else {
                    None
                }
            })
            .collect();

        let looks_like_qnum_row = !nums_only.is_empty()
            && nums_only.len() == tokens.iter().filter(|tk| tk.to_lowercase() != "câu").count()
            && (nums_only.len() >= 2 && nums_only.windows(2).all(|w| w[1] > w[0] && w[1] - w[0] <= 2));

        if looks_like_qnum_row {
            pending_nums = nums_only;
            pending_single_q = None;
            continue;
        }

        if !pending_nums.is_empty() {
            let clean_vals: Vec<&String> = tokens
                .iter()
                .filter(|tk| {
                    let lw = tk.to_lowercase();
                    lw != "đáp" && lw != "án"
                })
                .collect();
            for (idx, qn) in pending_nums.iter().enumerate() {
                if let Some(val) = clean_vals.get(idx) {
                    if cur_part == 3 {
                        if let Some(num) = extract_number(val) {
                            key.part3_answers.insert(*qn, num);
                        }
                    } else {
                        assign_answer_cell(key, *qn, val, &re_tf_combo);
                    }
                }
            }
            pending_nums.clear();
            continue;
        }

        // Ô dọc 1 giá trị mỗi dòng (`1` -> `A` -> `10` -> `B`)
        if tokens.len() == 1 {
            let tok = tokens[0].trim();
            if let Some(pqn) = pending_single_q.take() {
                let no_sp = tok.replace(' ', "");
                let up = no_sp.to_uppercase();
                if cur_part == 1 && matches!(up.as_str(), "A" | "B" | "C" | "D") {
                    key.part1_answers.insert(pqn, up);
                    continue;
                } else if cur_part == 2 && re_tf_combo.is_match(&no_sp) {
                    assign_answer_cell(key, pqn, tok, &re_tf_combo);
                    continue;
                } else if cur_part == 3 {
                    if let Some(num) = extract_number(tok) {
                        key.part3_answers.insert(pqn, num);
                        continue;
                    }
                }
            }
            let stripped = tok.trim_end_matches(&['.', ':'][..]);
            if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(qn) = stripped.parse::<u32>() {
                    if (1..=60).contains(&qn) {
                        pending_single_q = Some(qn);
                    }
                }
            }
        } else {
            pending_single_q = None;
        }
    }

    for (qn, arr) in p2_col_vals {
        if let (Some(a), Some(b), Some(c), Some(d)) = (arr[0], arr[1], arr[2], arr[3]) {
            key.part2_answers.insert(qn, TrueFalseAnswer { a, b, c, d });
        }
    }
}

fn parse_inline_solution_answers(
    inline: &[(u8, u32, String)],
    key: &mut ExamAnswerKey,
    explicit_only: bool,
) {
    let re_mc = Regex::new(
        r"(?i)(?:[ĐD][áaàảãạ]p\s*[áaàảãạ]n(?:\s*(?:đúng(?:\s*là)?|chọn))?|Ch[ọo]n(?:\s*đáp\s*án)?)\s*[:\-=\.>]*\s*([A-D])\b"
    ).unwrap();
    let re_p2_items = Regex::new(
        r"(?i)([a-d])[\.\)]\s*(?:đúng|đ|true|t|sai|s|false|f)\b"
    ).unwrap();
    let re_p2_seq4 = Regex::new(
        r"(?i)^\s*([ĐDSđds])\s*[-–—,\s]+\s*([ĐDSđds])\s*[-–—,\s]+\s*([ĐDSđds])\s*[-–—,\s]+\s*([ĐDSđds])[\.\s]*$"
    ).unwrap();
    let re_p3_ans = Regex::new(
        r"(?i)(?:[ĐD][áa]p\s*[áa]n|K[ếe]t\s*qu[ảa]|Tr[ảa]\s*l[ờo]i)\s*[:\-]?\s*(-?\d+(?:[.,/]\d+)?%?)"
    ).unwrap();
    let re_p3_explain_num = Regex::new(
        r"(?i)^\s*Gi[ảa]i\s*th[íi]ch\s*[:\-]\s*(-?\d+(?:[.,/]\d+)?%?)(?:\s|\(|$)"
    ).unwrap();
    let re_p3_short_text_sol = Regex::new(
        r#"(?i)^\s*(?:=>|⇨|→|[-*•])?\s*(?:[ĐD][áa]p\s*[áa]n|K[ếe]t\s*qu[ảa]|Tr[ảa]\s*l[ờo]i)\s*[:\-]\s*([0-9A-Za-zÀ-ỹ"“‘'].*)\s*$"#
    ).unwrap();
    let re_opt_chunk = Regex::new(r"(?i)(?:^|\s)([A-D])\.\s*(.+)$").unwrap();
    let re_sol_split = Regex::new(
        r"(?i)^\s*(?:Ph[uưươ][ơo]ng\s*ph[áa]p|L[oờơ]i\s*gi[ảa]i|Gi[ảa]i\s*th[íi]ch)(?:\s*ch[iìỉ]\s*ti[êếe]t)?\s*[:\-]?\s*"
    ).unwrap();
    let re_p3_spec_anchor = Regex::new(
        r"(?i)^\s*(?:L[oờơ]i\s*gi[ảa]i|C[áa]ch\s*gi[ảa]i)(?:\s*ch[iìỉ]\s*ti[êếe]t)?\s*[:\-]?\s*"
    ).unwrap();

    let mut p2_accum: HashMap<u32, [Option<bool>; 4]> = HashMap::new();
    let mut p1_sol_ctx: HashMap<u32, ([Option<String>; 4], String, bool)> = HashMap::new();
    let mut p2_sol_active: HashMap<u32, bool> = HashMap::new();
    let mut p3_sol_active: HashMap<u32, bool> = HashMap::new();
    let mut speculative_p3: HashSet<u32> = HashSet::new();
    let mut p2_cb_opts: HashMap<u32, Vec<String>> = HashMap::new();
    let mut p2_cb_sol_txt: HashMap<u32, String> = HashMap::new();
    let mut p2_cb_in_sol: HashMap<u32, bool> = HashMap::new();

    for (part, qn, txt) in inline {
        let stripped_sol = re_sol_split.find(txt).map(|m| txt[m.end()..].trim());
        match *part {
            1 => {
                if !key.part1_answers.contains_key(qn) {
                    if let Some(cap) = re_mc.captures(txt) {
                        let m_end = cap.get(0).unwrap().end();
                        let rest_after = txt[m_end..].trim_start();
                        let is_negative = rest_after.starts_with("sai")
                            || rest_after.starts_with("không đúng")
                            || rest_after.starts_with("chưa đúng")
                            || rest_after.starts_with("lỗi")
                            || rest_after.starts_with("nhầm")
                            || rest_after.starts_with(',')
                            || rest_after.starts_with(" và ");
                        if !is_negative {
                            key.part1_answers.insert(*qn, cap[1].to_ascii_uppercase());
                        }
                    } else {
                        if let Some(cap) = re_p3_short_text_sol.captures(txt) {
                            let cand_raw = cap[1].trim().trim_end_matches('.');
                            if let Some(cand) = clean_short_text_answer(cand_raw, false) {
                                let existing_has_lower = key
                                    .part3_answers
                                    .get(qn)
                                    .is_some_and(|v| v.chars().any(|c| c.is_ascii_lowercase()));
                                let exam_uses_lower_p3 = !explicit_only
                                    && key.part2_answers.is_empty()
                                    && key
                                        .part3_answers
                                        .values()
                                        .any(|v| v.chars().any(|c| c.is_ascii_lowercase()));
                                if existing_has_lower
                                    || (exam_uses_lower_p3 && !key.part3_answers.contains_key(qn))
                                {
                                    key.part3_answers.insert(*qn, cand);
                                    continue;
                                }
                            }
                        }
                        if !explicit_only {
                            let ctx = p1_sol_ctx
                                .entry(*qn)
                                .or_insert_with(|| ([None, None, None, None], String::new(), false));
                            if let Some(after) = stripped_sol {
                                ctx.2 = true;
                                if !after.is_empty() {
                                    if !ctx.1.is_empty() {
                                        ctx.1.push(' ');
                                    }
                                    ctx.1.push_str(&after.to_lowercase());
                                }
                            } else if ctx.2 {
                                if !ctx.1.is_empty() {
                                    ctx.1.push(' ');
                                }
                                ctx.1.push_str(&txt.to_lowercase());
                            } else {
                                let re_opt_splitter = Regex::new(r"[\t\n]+|\s{2,}").unwrap();
                                for seg in re_opt_splitter.split(txt) {
                                    let seg_t = seg.trim();
                                    if let Some(cap) = re_opt_chunk.captures(seg_t) {
                                        let idx = match cap[1].to_ascii_uppercase().as_str() {
                                            "A" => 0,
                                            "B" => 1,
                                            "C" => 2,
                                            _ => 3,
                                        };
                                        let norm_opt = cap[2]
                                            .trim()
                                            .trim_end_matches(&['.', ';', ','][..])
                                            .to_lowercase();
                                        if !norm_opt.is_empty() {
                                            ctx.0[idx] = Some(norm_opt);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            2 => {
                if !key.part2_answers.contains_key(qn) {
                    let body = stripped_sol.unwrap_or(txt.as_str()).trim();
                    if stripped_sol.is_some() {
                        p2_sol_active.insert(*qn, true);
                    }
                    let mut matched_any = false;
                    let flags = p2_accum.entry(*qn).or_insert([None; 4]);
                    for cap in re_p2_items.captures_iter(body) {
                        let sub = match cap[1].to_ascii_lowercase().as_str() {
                            "a" => 0,
                            "b" => 1,
                            "c" => 2,
                            _ => 3,
                        };
                        let full = cap.get(0).map(|m| m.as_str()).unwrap_or("");
                        let val_part = full.split(&[')', '.'][..]).nth(1).unwrap_or("").trim().to_string();
                        if let Some(b) = parse_single_tf_cell(Some(&val_part)) {
                            flags[sub] = Some(b);
                            matched_any = true;
                        }
                    }
                    let re_p2_multi = Regex::new(
                        r"(?i)([a-d])\s*[\),]\s*(?:và\s*)?([a-d])[\.\)]\s*(đúng|đ|true|t|sai|s|false|f)\b"
                    ).unwrap();
                    for cap in re_p2_multi.captures_iter(body) {
                        let sub1 = match cap[1].to_ascii_lowercase().as_str() {
                            "a" => 0, "b" => 1, "c" => 2, _ => 3,
                        };
                        let sub2 = match cap[2].to_ascii_lowercase().as_str() {
                            "a" => 0, "b" => 1, "c" => 2, _ => 3,
                        };
                        let val_part = cap[3].trim().to_string();
                        if let Some(b) = parse_single_tf_cell(Some(&val_part)) {
                            flags[sub1] = Some(b);
                            flags[sub2] = Some(b);
                            matched_any = true;
                        }
                    }
                    if !matched_any {
                        let check_seq = if stripped_sol.is_some() || *p2_sol_active.get(qn).unwrap_or(&false) {
                            body
                        } else {
                            txt.as_str()
                        };
                        if let Some(cap) = re_p2_seq4.captures(check_seq) {
                            if let (Some(a), Some(b), Some(c), Some(d)) = (
                                parse_single_tf_cell(Some(&cap[1].to_string())),
                                parse_single_tf_cell(Some(&cap[2].to_string())),
                                parse_single_tf_cell(Some(&cap[3].to_string())),
                                parse_single_tf_cell(Some(&cap[4].to_string())),
                            ) {
                                key.part2_answers.insert(*qn, TrueFalseAnswer { a, b, c, d });
                            }
                        } else if !explicit_only {
                            let t_low = txt.trim().to_lowercase();
                            if stripped_sol.is_some()
                                || t_low.starts_with("phương pháp")
                                || t_low.starts_with("lời giải")
                                || t_low.starts_with("giải thích")
                            {
                                p2_cb_in_sol.insert(*qn, true);
                                if let Some(after) = stripped_sol {
                                    if !after.is_empty() {
                                        let entry = p2_cb_sol_txt.entry(*qn).or_default();
                                        entry.push(' ');
                                        entry.push_str(after);
                                    }
                                }
                            } else if *p2_cb_in_sol.get(qn).unwrap_or(&false) {
                                let entry = p2_cb_sol_txt.entry(*qn).or_default();
                                entry.push(' ');
                                entry.push_str(txt.trim());
                            } else if !re_p2_items.is_match(txt) && !re_mc.is_match(txt) {
                                let t_trim = txt.trim();
                                if t_trim.ends_with('?') || t_low.contains("tích vào ô") {
                                    p2_cb_opts.entry(*qn).or_default().clear();
                                } else if !t_low.starts_with("câu ")
                                    && !t_low.starts_with("con hãy")
                                    && !t_trim.is_empty()
                                    && t_trim.chars().count() <= 90
                                {
                                    let vec = p2_cb_opts.entry(*qn).or_default();
                                    if vec.len() < 4 {
                                        vec.push(t_trim.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            3 => {
                let re_p3_bare = Regex::new(r"^\s*(-?\d+(?:[.,/]\d+)?%?)\s*$").unwrap();
                let re_p3_vay_la = Regex::new(r"(?i)\b(?:vậy|do\s*đó|kết\s*quả\s*là|tỉ\s*lệ.*?là)\s*[:\-]?\s*(-?\d+(?:[.,/]\d+)?%?)\)?\s*$").unwrap();
                if let Some(cap) = re_p3_bare.captures(txt.trim()).or_else(|| re_p3_vay_la.captures(txt.trim())) {
                    let raw_v = cap[1].trim_end_matches('.');
                    if !raw_v.is_empty() && (raw_v.contains('/') || raw_v.contains('%') || extract_number(raw_v).is_some()) {
                        key.part3_answers.insert(*qn, raw_v.to_string());
                        speculative_p3.remove(qn);
                        p3_sol_active.insert(*qn, false);
                        continue;
                    }
                }
                if let Some(cap) = re_p3_ans.captures(txt) {
                    let raw_v = cap[1].trim_end_matches('.');
                    if !raw_v.is_empty() {
                        if raw_v.contains('/') || raw_v.contains('%') {
                            key.part3_answers.insert(*qn, raw_v.to_string());
                            speculative_p3.remove(qn);
                            p3_sol_active.insert(*qn, false);
                            continue;
                        } else if let Some(num) = extract_number(raw_v) {
                            key.part3_answers.insert(*qn, num);
                            speculative_p3.remove(qn);
                            p3_sol_active.insert(*qn, false);
                            continue;
                        }
                    }
                }
                if let Some(cap) = re_p3_short_text_sol.captures(txt) {
                    let cand_raw = cap[1].trim().trim_end_matches('.');
                    if let Some(cand) = clean_short_text_answer(cand_raw, false) {
                        let can_replace = match key.part3_answers.get(qn) {
                            None => true,
                            Some(existing) => speculative_p3.contains(qn) || existing.chars().any(|c| c.is_ascii_lowercase()),
                        };
                        if can_replace {
                            key.part3_answers.insert(*qn, cand);
                            speculative_p3.remove(qn);
                            p3_sol_active.insert(*qn, false);
                            continue;
                        }
                    }
                }
                if !explicit_only && (!key.part3_answers.contains_key(qn) || speculative_p3.contains(qn)) {
                    if let Some(cap) = re_p3_explain_num.captures(txt) {
                        let raw_v = cap[1].trim_end_matches('.');
                        if !raw_v.is_empty() {
                            key.part3_answers.insert(*qn, raw_v.to_string());
                            speculative_p3.remove(qn);
                            p3_sol_active.insert(*qn, false);
                            continue;
                        }
                    }
                    let is_safe_p3_spec = |cand: &str| -> bool {
                        let clow = cand.trim().to_lowercase();
                        if clow.is_empty()
                            || cand.chars().count() > 55
                            || cand.contains('\t')
                            || cand.contains('=')
                            || cand.contains('\'')
                            || cand.contains('’')
                            || cand.contains('→')
                            || cand.contains('⇒')
                            || cand.ends_with(':')
                            || clow.starts_with("p:")
                            || clow.starts_with("g:")
                            || clow.starts_with("ta có")
                            || clow.starts_with("vì ")
                            || clow.starts_with("do ")
                            || clow.starts_with("xét ")
                            || clow.starts_with("gọi ")
                            || clow.starts_with("đặt ")
                            || clow.starts_with("khi ")
                            || clow.starts_with("quan sát")
                            || clow.starts_with("tập xác định")
                            || clow.starts_with("theo ")
                            || clow.starts_with("giải thích")
                            || clow.starts_with("phương pháp")
                            || clow.starts_with("câu ")
                            || clow.contains("suy ra")
                        {
                            return false;
                        }
                        extract_number(cand).is_some()
                            || cand.contains('/')
                            || cand.ends_with('%')
                            || !clow.contains(':')
                    };
                    if let Some(m_spec) = re_p3_spec_anchor.find(txt) {
                        let after = txt[m_spec.end()..].trim();
                        p3_sol_active.insert(*qn, true);
                        let cand = after.trim().trim_end_matches('.');
                        if is_safe_p3_spec(cand) {
                            key.part3_answers.insert(*qn, cand.to_string());
                            speculative_p3.insert(*qn);
                            p3_sol_active.insert(*qn, false);
                        }
                    } else if *p3_sol_active.get(qn).unwrap_or(&false) {
                        let cand = txt.trim().trim_end_matches('.');
                        if is_safe_p3_spec(cand) {
                            key.part3_answers.insert(*qn, cand.to_string());
                            speculative_p3.insert(*qn);
                        }
                        p3_sol_active.insert(*qn, false);
                    }
                }
            }
            _ => {}
        }
    }

    if !explicit_only {
        for (qn, cand_opts) in p2_cb_opts {
            if key.part2_answers.contains_key(&qn) || cand_opts.len() != 4 {
                continue;
            }
            let Some(sol_s) = p2_cb_sol_txt.get(&qn) else {
                continue;
            };
            let norm_str = |s: &str| -> String {
                s.to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == ':')
                    .collect()
            };
            let sol_norm = norm_str(sol_s);
            let sol_low = sol_s.to_lowercase();
            let mut hits = [false; 4];
            for (i, opt_s) in cand_opts.iter().enumerate() {
                let onorm = norm_str(opt_s);
                let olow = opt_s.trim().to_lowercase();
                if onorm.len() >= 2 && (sol_low.contains(&olow) || (!opt_s.contains(':') && sol_norm.contains(&onorm)) || (opt_s.contains(':') && sol_norm.contains(&onorm) && !(onorm == "5:1" && sol_norm.contains("15:1")))) {
                    hits[i] = true;
                }
            }
            let cnt = hits.iter().filter(|&&b| b).count();
            if (1..=3).contains(&cnt) {
                key.part2_answers.insert(
                    qn,
                    TrueFalseAnswer {
                        a: hits[0],
                        b: hits[1],
                        c: hits[2],
                        d: hits[3],
                    },
                );
            }
        }
    }

    if !explicit_only {
        for (qn, (opts, sol_txt, has_sol)) in p1_sol_ctx {
            if key.part1_answers.contains_key(&qn) || !has_sol || sol_txt.is_empty() {
                continue;
            }
            let filled_opts = opts.iter().filter(|o| o.is_some()).count();
            if filled_opts < 2 {
                continue;
            }
            let sol_trim = sol_txt.trim();
            let mut matched_idx: Vec<usize> = Vec::new();
            for (idx, opt_opt) in opts.iter().enumerate() {
                let Some(opt) = opt_opt else {
                    continue;
                };
                if opt.chars().count() >= 5 {
                    if sol_trim.contains(opt.as_str()) {
                        matched_idx.push(idx);
                    }
                } else if sol_trim == opt.as_str()
                    || sol_trim.strip_prefix(opt.as_str()).is_some_and(|rest| {
                        rest.is_empty()
                            || rest.starts_with(|c: char| c.is_whitespace() || matches!(c, '.' | ',' | ';'))
                    })
                    || sol_trim.ends_with(&format!("= {}", opt))
                    || sol_trim.ends_with(&format!("= {}.", opt))
                    || sol_trim.ends_with(&format!("là {}", opt))
                    || sol_trim.ends_with(&format!("là {}.", opt))
                {
                    matched_idx.push(idx);
                } else {
                    let esc = regex::escape(opt);
                    if let Ok(re_eq) = Regex::new(&format!(r"(?:=|\blà|\bra)\s*{}(?:\b|[\s\.,;\)\]}}])", esc)) {
                        if re_eq.is_match(sol_trim) {
                            matched_idx.push(idx);
                        }
                    }
                }
            }
            if matched_idx.len() == 1 {
                let letter = match matched_idx[0] {
                    0 => "A",
                    1 => "B",
                    2 => "C",
                    _ => "D",
                };
                key.part1_answers.insert(qn, letter.to_string());
            }
        }
    }

    for (qn, flags) in p2_accum {
        if !key.part2_answers.contains_key(&qn) {
            if let (Some(a), Some(b), Some(c), Some(d)) = (flags[0], flags[1], flags[2], flags[3]) {
                key.part2_answers.insert(qn, TrueFalseAnswer { a, b, c, d });
            }
        }
    }
}
