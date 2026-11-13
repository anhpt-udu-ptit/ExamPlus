use std::collections::HashMap;
use std::path::Path;

pub mod answer_key;
pub mod docx;
pub mod emf;
pub mod pdf;
pub mod wmf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExamFileFormat {
    Docx,
    Pdf,
}

impl ExamFileFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExamFileFormat::Docx => "docx",
            ExamFileFormat::Pdf => "pdf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExamFileFormat::Docx => "Word (.docx)",
            ExamFileFormat::Pdf => "PDF (.pdf)",
        }
    }
}

/// Nhận diện định dạng đề thi qua magic bytes ưu tiên trước đuôi tên tệp.
pub fn detect_exam_format(
    bytes: &[u8],
    filename_hint: Option<&str>,
) -> Result<ExamFileFormat, docx::ParseError> {
    if bytes.starts_with(b"%PDF-") || bytes.get(..1024).is_some_and(|head| head.windows(5).any(|w| w == b"%PDF-")) {
        return Ok(ExamFileFormat::Pdf);
    }
    if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) || bytes.starts_with(&[0x50, 0x4B, 0x05, 0x06]) {
        return Ok(ExamFileFormat::Docx);
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return Err(docx::ParseError::UnsupportedFormat(
            "Tệp Word định dạng cũ (.doc nhị phân) chưa được hỗ trợ. Vui lòng chuyển sang .docx hoặc .pdf."
                .to_string(),
        ));
    }

    if let Some(name) = filename_hint {
        let lower = name.trim().to_ascii_lowercase();
        if lower.ends_with(".pdf") {
            return Err(docx::ParseError::Pdf(
                "Tệp có đuôi .pdf nhưng không mang chữ ký %PDF- hợp lệ.".to_string(),
            ));
        }
        if lower.ends_with(".docx") {
            return Err(docx::ParseError::Zip(zip::result::ZipError::InvalidArchive(
                std::borrow::Cow::Borrowed("Tệp .docx không đúng cấu trúc ZIP OOXML"),
            )));
        }
        if lower.ends_with(".doc") {
            return Err(docx::ParseError::UnsupportedFormat(
                "Định dạng .doc cũ chưa được hỗ trợ. Vui lòng lưu lại thành .docx hoặc xuất .pdf."
                    .to_string(),
            ));
        }
    }

    Err(docx::ParseError::UnsupportedFormat(
        "Chỉ hỗ trợ tệp đề thi Word (.docx) hoặc PDF (.pdf).".to_string(),
    ))
}

/// Phân tích đề thi tự động theo định dạng thực của tệp (`.docx` hoặc `.pdf`).
pub fn parse_exam_file(path: &Path) -> Result<(ExamFileFormat, docx::ParsedExam), docx::ParseError> {
    let bytes = std::fs::read(path)?;
    let hint = path.file_name().and_then(|s| s.to_str());
    let (fmt, parsed) = parse_exam_bytes(&bytes, hint)?;
    Ok((fmt, parsed))
}

/// Phân tích đề thi từ vùng nhớ bytes kèm gợi ý tên tệp (tùy chọn).
pub fn parse_exam_bytes(
    bytes: &[u8],
    filename_hint: Option<&str>,
) -> Result<(ExamFileFormat, docx::ParsedExam), docx::ParseError> {
    let fmt = detect_exam_format(bytes, filename_hint)?;
    let parsed = match fmt {
        ExamFileFormat::Docx => docx::parse_docx_bytes(bytes)?,
        ExamFileFormat::Pdf => pdf::parse_pdf_bytes(bytes)?,
    };
    Ok((fmt, parsed))
}

/// Bóc tách bảng đáp án tự động theo định dạng thực của tệp (`.docx` hoặc `.pdf`).
pub fn parse_answer_key_file(
    path: &Path,
) -> Result<HashMap<String, answer_key::ExamAnswerKey>, docx::ParseError> {
    let bytes = std::fs::read(path)?;
    let hint = path.file_name().and_then(|s| s.to_str());
    parse_answer_key_bytes(&bytes, hint, Some(path))
}

/// Bóc tách bảng đáp án từ bytes (hoặc path cho `.docx`).
pub fn parse_answer_key_bytes(
    bytes: &[u8],
    filename_hint: Option<&str>,
    path_hint: Option<&Path>,
) -> Result<HashMap<String, answer_key::ExamAnswerKey>, docx::ParseError> {
    let is_text_or_csv = filename_hint.map(|n| {
        let l = n.to_ascii_lowercase();
        l.ends_with(".txt") || l.ends_with(".csv")
    }).unwrap_or(false);
    if is_text_or_csv || (!bytes.starts_with(b"%PDF-") && !bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) && !bytes.iter().take(200).any(|&b| b == 0)) {
        if let Ok(text) = std::str::from_utf8(bytes) {
            let mut key = answer_key::ExamAnswerKey::new("default");
            let lines: Vec<String> = text.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            pdf::parse_summary_answer_lines(&lines, &mut key);
            let total = key.part1_answers.len() + key.part2_answers.len() + key.part3_answers.len();
            if total > 0 {
                let mut map = HashMap::new();
                map.insert("default".to_string(), key);
                return Ok(map);
            }
        }
    }

    let fmt = detect_exam_format(bytes, filename_hint)?;
    match fmt {
        ExamFileFormat::Pdf => pdf::parse_pdf_answer_key_bytes(bytes),
        ExamFileFormat::Docx => {
            if let Some(p) = path_hint {
                answer_key::parse_answer_key(p)
            } else {
                answer_key::parse_answer_key_bytes(bytes)
            }
        }
    }
}
