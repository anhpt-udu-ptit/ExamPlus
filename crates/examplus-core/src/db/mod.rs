//! # Database Module
//!
//! Lược đồ và Repository thao tác cơ sở dữ liệu SQLite cục bộ.

pub mod repository;
pub mod schema;

pub use repository::{DbRepository, VariantQuestionMapping};
