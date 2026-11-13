//! # ExamPlus Core Crate
//!
//! Mô-đun cốt lõi chứa Data Models, Docx & MathType Parser,
//! Bộ xáo đề thi, Công cụ chấm điểm GDPT 2018, SQLite Repository và Excel Exporter.

pub mod db;
pub mod engine;
pub mod export;
pub mod models;
pub mod parser;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_version() {
        assert_eq!(version(), "0.1.0");
    }
}
