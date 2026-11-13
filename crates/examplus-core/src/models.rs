use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExamType {
    Gdpt2018,
    Traditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PartType {
    Part1,
    Part2,
    Part3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExamStatus {
    Draft,
    Ready,
    InProgress,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NetworkMode {
    OfflineLan,
    OnlineTunnel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exam {
    pub id: Uuid,
    pub title: String,
    pub subject: String,
    pub duration_minutes: u32,
    pub exam_type: ExamType,
    pub status: ExamStatus,
    pub network_mode: NetworkMode,
    pub public_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub source_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub id: Uuid,
    pub question_id: Uuid,
    pub label: String,
    pub content_html: String,
    pub is_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: Uuid,
    pub exam_id: Uuid,
    pub part_type: PartType,
    pub order_index: u32,
    pub content_html: String,
    pub raw_text: Option<String>,
    pub image_url: Option<String>,
    pub short_answer_solution: Option<String>,
    pub provenance_json: Option<String>,
    pub answer_meta_json: Option<String>,
    pub needs_review: bool,
    pub options: Vec<QuestionOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamVariant {
    pub id: Uuid,
    pub exam_id: Uuid,
    pub code: String,
    pub is_original: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExamineeStatus {
    Waiting,
    Testing,
    Submitted,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Examinee {
    pub id: Uuid,
    pub exam_id: Uuid,
    pub assigned_variant_id: Option<Uuid>,
    pub student_code: String,
    pub full_name: String,
    pub class_name: String,
    pub session_token: String,
    pub status: ExamineeStatus,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: Uuid,
    pub examinee_id: Uuid,
    pub started_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub total_score: f64,
    pub part1_score: f64,
    pub part2_score: f64,
    pub part3_score: f64,
    pub is_graded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubmissionAnswer {
    pub id: Uuid,
    pub submission_id: Uuid,
    pub question_id: Uuid,
    pub selected_option: Option<String>,
    pub sub_answers_json: Option<String>,
    pub short_answer_text: Option<String>,
    pub awarded_score: f64,
    pub is_correct: bool,
}

pub type SubmissionAnswerRecord = SubmissionAnswer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ViolationType {
    TabSwitch,
    Blur,
    FullscreenExit,
    CopyAttempt,
    PrintScreen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditViolation {
    pub id: Uuid,
    pub examinee_id: Uuid,
    pub violation_type: ViolationType,
    pub duration_seconds: u32,
    pub details: Option<String>,
    pub timestamp: DateTime<Utc>,
}
