//! # Excel Grade Exporter
//!
//! Xuất báo cáo điểm thi và nhật ký giám sát vi phạm ra định dạng Excel (.xlsx)
//! sử dụng `rust_xlsxwriter` với định dạng bảng biểu trực quan, chuyên nghiệp.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, XlsxError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentGradeRecord {
    pub student_code: String,
    pub full_name: String,
    pub class_name: String,
    pub variant_code: String,
    pub part1_score: f64,
    pub part2_score: f64,
    pub part3_score: f64,
    pub total_score: f64,
    pub submitted_at: Option<String>,
    pub violation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationExportRecord {
    pub student_code: String,
    pub full_name: String,
    pub class_name: String,
    pub violation_type: String,
    pub duration_seconds: u32,
    pub details: String,
    pub timestamp: String,
}

pub fn export_exam_results(
    file_path: impl AsRef<Path>,
    exam_title: &str,
    subject: &str,
    grades: &[StudentGradeRecord],
    violations: &[ViolationExportRecord],
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();

    // Định dạng tiêu đề lớn
    let title_format = Format::new()
        .set_bold()
        .set_font_size(16)
        .set_font_name("Segoe UI")
        .set_font_color(Color::RGB(0x1E3A8A)); // Xanh Navy

    // Định dạng phụ đề
    let subtitle_format = Format::new()
        .set_italic()
        .set_font_size(11)
        .set_font_name("Segoe UI")
        .set_font_color(Color::RGB(0x4B5563));

    // Định dạng header cột
    let header_format = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_font_name("Segoe UI")
        .set_background_color(Color::RGB(0x2563EB)) // Xanh dương
        .set_font_color(Color::RGB(0xFFFFFF))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    // Định dạng ô dữ liệu thông thường
    let cell_format = Format::new()
        .set_font_size(10)
        .set_font_name("Segoe UI")
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::VerticalCenter);

    let cell_center = Format::new()
        .set_font_size(10)
        .set_font_name("Segoe UI")
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    let score_format = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_font_name("Segoe UI")
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_font_color(Color::RGB(0xDC2626)); // Đỏ nổi bật

    // =========================================================================
    // SHEET 1: BẢNG ĐIỂM TỔNG HỢP
    // =========================================================================
    let worksheet1 = workbook.add_worksheet();
    worksheet1.set_name("Bảng Điểm")?;

    // Tiêu đề
    worksheet1.write_string_with_format(0, 0, format!("BẢNG ĐIỂM KỲ THI: {}", exam_title.to_uppercase()), &title_format)?;
    worksheet1.write_string_with_format(1, 0, format!("Môn thi: {} | Tổng số thí sinh: {}", subject, grades.len()), &subtitle_format)?;

    // Headers
    let headers1 = [
        "STT",
        "Số báo danh (SBD)",
        "Họ và tên thí sinh",
        "Lớp",
        "Mã đề",
        "Điểm Phần I",
        "Điểm Phần II",
        "Điểm Phần III",
        "TỔNG ĐIỂM",
        "Thời điểm nộp",
        "Số vi phạm",
    ];

    for (col_idx, h) in headers1.iter().enumerate() {
        worksheet1.write_string_with_format(3, col_idx as u16, *h, &header_format)?;
    }

    // Rows
    for (row_idx, g) in grades.iter().enumerate() {
        let r = (row_idx + 4) as u32;
        worksheet1.write_number_with_format(r, 0, (row_idx + 1) as f64, &cell_center)?;
        worksheet1.write_string_with_format(r, 1, &g.student_code, &cell_center)?;
        worksheet1.write_string_with_format(r, 2, &g.full_name, &cell_format)?;
        worksheet1.write_string_with_format(r, 3, &g.class_name, &cell_center)?;
        worksheet1.write_string_with_format(r, 4, &g.variant_code, &cell_center)?;
        worksheet1.write_number_with_format(r, 5, g.part1_score, &cell_center)?;
        worksheet1.write_number_with_format(r, 6, g.part2_score, &cell_center)?;
        worksheet1.write_number_with_format(r, 7, g.part3_score, &cell_center)?;
        worksheet1.write_number_with_format(r, 8, g.total_score, &score_format)?;
        worksheet1.write_string_with_format(r, 9, g.submitted_at.as_deref().unwrap_or("Chưa nộp"), &cell_center)?;
        worksheet1.write_number_with_format(r, 10, g.violation_count as f64, &cell_center)?;
    }

    // Tự động căn chỉnh độ rộng cột
    worksheet1.set_column_width(0, 6)?;
    worksheet1.set_column_width(1, 18)?;
    worksheet1.set_column_width(2, 26)?;
    worksheet1.set_column_width(3, 12)?;
    worksheet1.set_column_width(4, 10)?;
    worksheet1.set_column_width(5, 14)?;
    worksheet1.set_column_width(6, 14)?;
    worksheet1.set_column_width(7, 14)?;
    worksheet1.set_column_width(8, 14)?;
    worksheet1.set_column_width(9, 22)?;
    worksheet1.set_column_width(10, 12)?;

    // =========================================================================
    // SHEET 2: NHẬT KÝ VI PHẠM
    // =========================================================================
    let worksheet2 = workbook.add_worksheet();
    worksheet2.set_name("Giám Sát Vi Phạm")?;

    let header2_format = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_font_name("Segoe UI")
        .set_background_color(Color::RGB(0xBE123C)) // Đỏ sẫm cảnh báo
        .set_font_color(Color::RGB(0xFFFFFF))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);

    worksheet2.write_string_with_format(0, 0, "NHẬT KÝ GIÁM SÁT GIAN LẬN & VI PHẠM PHÒNG THI", &title_format)?;

    let headers2 = [
        "STT",
        "SBD",
        "Họ và tên",
        "Lớp",
        "Hành vi vi phạm",
        "Thời lượng (giây)",
        "Chi tiết sự kiện",
        "Thời điểm ghi nhận",
    ];

    for (col_idx, h) in headers2.iter().enumerate() {
        worksheet2.write_string_with_format(2, col_idx as u16, *h, &header2_format)?;
    }

    for (row_idx, v) in violations.iter().enumerate() {
        let r = (row_idx + 3) as u32;
        worksheet2.write_number_with_format(r, 0, (row_idx + 1) as f64, &cell_center)?;
        worksheet2.write_string_with_format(r, 1, &v.student_code, &cell_center)?;
        worksheet2.write_string_with_format(r, 2, &v.full_name, &cell_format)?;
        worksheet2.write_string_with_format(r, 3, &v.class_name, &cell_center)?;
        worksheet2.write_string_with_format(r, 4, &v.violation_type, &cell_center)?;
        worksheet2.write_number_with_format(r, 5, v.duration_seconds as f64, &cell_center)?;
        worksheet2.write_string_with_format(r, 6, &v.details, &cell_format)?;
        worksheet2.write_string_with_format(r, 7, &v.timestamp, &cell_center)?;
    }

    worksheet2.set_column_width(0, 6)?;
    worksheet2.set_column_width(1, 18)?;
    worksheet2.set_column_width(2, 26)?;
    worksheet2.set_column_width(3, 12)?;
    worksheet2.set_column_width(4, 20)?;
    worksheet2.set_column_width(5, 18)?;
    worksheet2.set_column_width(6, 35)?;
    worksheet2.set_column_width(7, 24)?;

    workbook.save(file_path)?;
    Ok(())
}
