//! # SQLite Database Schema
//!
//! Định nghĩa và khởi tạo lược đồ cơ sở dữ liệu SQLite cho ExamPlus.

use rusqlite::{Connection, Result};

pub const SCHEMA_SQL: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

-- 1. Bảng Kỳ thi
CREATE TABLE IF NOT EXISTS exams (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    subject TEXT NOT NULL,
    duration_minutes INTEGER NOT NULL DEFAULT 45,
    exam_type TEXT NOT NULL CHECK(exam_type IN ('GDPT_2018', 'TRADITIONAL')),
    status TEXT NOT NULL DEFAULT 'DRAFT' CHECK(status IN ('DRAFT', 'READY', 'IN_PROGRESS', 'FINISHED')),
    network_mode TEXT NOT NULL DEFAULT 'OFFLINE_LAN' CHECK(network_mode IN ('OFFLINE_LAN', 'ONLINE_TUNNEL')),
    public_url TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    end_at TEXT,
    source_format TEXT
);

-- 2. Bảng Mã đề thi (Đề gốc & Đề xáo)
CREATE TABLE IF NOT EXISTS exam_variants (
    id TEXT PRIMARY KEY,
    exam_id TEXT NOT NULL REFERENCES exams(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    is_original INTEGER NOT NULL DEFAULT 0,
    UNIQUE(exam_id, code)
);

-- 3. Bảng Câu hỏi
CREATE TABLE IF NOT EXISTS questions (
    id TEXT PRIMARY KEY,
    exam_id TEXT NOT NULL REFERENCES exams(id) ON DELETE CASCADE,
    part_type TEXT NOT NULL CHECK(part_type IN ('PART_1', 'PART_2', 'PART_3')),
    order_index INTEGER NOT NULL,
    content_html TEXT NOT NULL,
    raw_text TEXT,
    image_url TEXT,
    short_answer_solution TEXT,
    provenance_json TEXT,
    answer_meta_json TEXT,
    needs_review INTEGER NOT NULL DEFAULT 0
);

-- 4. Bảng Phương án lựa chọn (A, B, C, D hoặc a, b, c, d)
CREATE TABLE IF NOT EXISTS question_options (
    id TEXT PRIMARY KEY,
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    content_html TEXT NOT NULL,
    is_correct INTEGER NOT NULL DEFAULT 0
);

-- 5. Bảng Ánh xạ Xáo đề
CREATE TABLE IF NOT EXISTS variant_question_mappings (
    id TEXT PRIMARY KEY,
    variant_id TEXT NOT NULL REFERENCES exam_variants(id) ON DELETE CASCADE,
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    display_order INTEGER NOT NULL,
    options_order_json TEXT
);

-- 6. Bảng Thí sinh
CREATE TABLE IF NOT EXISTS examinees (
    id TEXT PRIMARY KEY,
    exam_id TEXT NOT NULL REFERENCES exams(id) ON DELETE CASCADE,
    assigned_variant_id TEXT REFERENCES exam_variants(id),
    student_code TEXT NOT NULL,
    full_name TEXT NOT NULL,
    class_name TEXT NOT NULL,
    session_token TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'WAITING' CHECK(status IN ('WAITING', 'TESTING', 'SUBMITTED', 'DISCONNECTED')),
    ip_address TEXT,
    user_agent TEXT,
    joined_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_heartbeat DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(exam_id, student_code)
);

-- 7. Bảng Bài làm / Kết quả nộp
CREATE TABLE IF NOT EXISTS submissions (
    id TEXT PRIMARY KEY,
    examinee_id TEXT NOT NULL UNIQUE REFERENCES examinees(id) ON DELETE CASCADE,
    started_at DATETIME,
    submitted_at DATETIME,
    total_score REAL DEFAULT 0.0,
    part1_score REAL DEFAULT 0.0,
    part2_score REAL DEFAULT 0.0,
    part3_score REAL DEFAULT 0.0,
    is_graded INTEGER NOT NULL DEFAULT 0
);

-- 8. Bảng Chi tiết câu trả lời
CREATE TABLE IF NOT EXISTS submission_answers (
    id TEXT PRIMARY KEY,
    submission_id TEXT NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    selected_option TEXT,
    sub_answers_json TEXT,
    short_answer_text TEXT,
    awarded_score REAL DEFAULT 0.0,
    is_correct INTEGER NOT NULL DEFAULT 0,
    UNIQUE(submission_id, question_id)
);

-- 9. Bảng Nhật ký Vi phạm (Audit Trail)
CREATE TABLE IF NOT EXISTS audit_violations (
    id TEXT PRIMARY KEY,
    examinee_id TEXT NOT NULL REFERENCES examinees(id) ON DELETE CASCADE,
    violation_type TEXT NOT NULL CHECK(violation_type IN ('TAB_SWITCH', 'BLUR', 'FULLSCREEN_EXIT', 'COPY_ATTEMPT', 'PRINT_SCREEN')),
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    details TEXT,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Các chỉ mục tối ưu hóa tốc độ truy vấn
CREATE INDEX IF NOT EXISTS idx_examinees_exam_status ON examinees(exam_id, status);
CREATE INDEX IF NOT EXISTS idx_examinees_token ON examinees(session_token);
CREATE INDEX IF NOT EXISTS idx_submissions_examinee ON submissions(examinee_id);
CREATE INDEX IF NOT EXISTS idx_submission_answers_sub ON submission_answers(submission_id);
CREATE INDEX IF NOT EXISTS idx_violations_examinee ON audit_violations(examinee_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_questions_exam_part ON questions(exam_id, part_type, order_index);
"#;

/// Khởi tạo bảng và chỉ mục trên kết nối SQLite
pub fn init_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    let _ = conn.execute("ALTER TABLE exams ADD COLUMN started_at TEXT", []);
    let _ = conn.execute("ALTER TABLE exams ADD COLUMN end_at TEXT", []);
    let _ = conn.execute("ALTER TABLE exams ADD COLUMN source_format TEXT", []);
    let _ = conn.execute("ALTER TABLE questions ADD COLUMN provenance_json TEXT", []);
    let _ = conn.execute("ALTER TABLE questions ADD COLUMN answer_meta_json TEXT", []);
    let _ = conn.execute("ALTER TABLE questions ADD COLUMN needs_review INTEGER NOT NULL DEFAULT 0", []);
    Ok(())
}
