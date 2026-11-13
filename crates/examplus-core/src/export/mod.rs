//! # Export Module
//!
//! Mô-đun xuất dữ liệu bảng điểm và báo cáo sang định dạng Excel (.xlsx).

pub mod excel;

pub use excel::{export_exam_results, StudentGradeRecord, ViolationExportRecord};
