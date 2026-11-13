//! # SQLite Database Repository
//!
//! Tầng truy xuất dữ liệu an toàn, hiệu năng cao trên SQLite 3 chế độ WAL.

use crate::db::schema::init_database;
use crate::models::{
    AuditViolation, Exam, ExamStatus, ExamType, ExamVariant, Examinee, ExamineeStatus, NetworkMode,
    PartType, Question, QuestionOption, Submission, SubmissionAnswer, SubmissionAnswerRecord,
    ViolationType,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct VariantQuestionMapping {
    pub id: Uuid,
    pub variant_id: Uuid,
    pub question_id: Uuid,
    pub display_order: i32,
    pub options_order_json: Option<String>,
}

pub struct DbRepository {
    conn: Connection,
}

impl DbRepository {
    /// Mở hoặc tạo mới tệp CSDL SQLite tại đường dẫn chỉ định
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        init_database(&conn)?;
        Ok(Self { conn })
    }

    /// Khởi tạo CSDL SQLite trong bộ nhớ RAM (In-Memory), phục vụ kiểm thử đơn vị
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        init_database(&conn)?;
        Ok(Self { conn })
    }

    /// Lấy tham chiếu kết nối SQLite
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // =========================================================================
    // EXAMS
    // =========================================================================

    pub fn create_exam(&self, exam: &Exam) -> Result<()> {
        let exam_type_str = match exam.exam_type {
            ExamType::Gdpt2018 => "GDPT_2018",
            ExamType::Traditional => "TRADITIONAL",
        };
        let status_str = match exam.status {
            ExamStatus::Draft => "DRAFT",
            ExamStatus::Ready => "READY",
            ExamStatus::InProgress => "IN_PROGRESS",
            ExamStatus::Finished => "FINISHED",
        };
        let mode_str = match exam.network_mode {
            NetworkMode::OfflineLan => "OFFLINE_LAN",
            NetworkMode::OnlineTunnel => "ONLINE_TUNNEL",
        };
        let started_at_str = exam.started_at.map(|dt| dt.to_rfc3339());
        let end_at_str = exam.end_at.map(|dt| dt.to_rfc3339());

        self.conn.execute(
            r#"
            INSERT INTO exams (id, title, subject, duration_minutes, exam_type, status, network_mode, public_url, created_at, started_at, end_at, source_format)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                exam.id.to_string(),
                exam.title,
                exam.subject,
                exam.duration_minutes,
                exam_type_str,
                status_str,
                mode_str,
                exam.public_url,
                exam.created_at.to_rfc3339(),
                started_at_str,
                end_at_str,
                exam.source_format,
            ],
        )?;
        Ok(())
    }

    pub fn get_exam(&self, id: Uuid) -> Result<Option<Exam>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, subject, duration_minutes, exam_type, status, network_mode, public_url, created_at, started_at, end_at, source_format
            FROM exams WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let exam_type_str: String = row.get(4)?;
            let status_str: String = row.get(5)?;
            let mode_str: String = row.get(6)?;
            let created_at_str: String = row.get(8)?;
            let started_at_opt: Option<String> = row.get(9)?;
            let end_at_opt: Option<String> = row.get(10)?;
            let source_format: Option<String> = row.get(11).ok();

            let exam_type = match exam_type_str.as_str() {
                "TRADITIONAL" => ExamType::Traditional,
                _ => ExamType::Gdpt2018,
            };
            let status = match status_str.as_str() {
                "READY" => ExamStatus::Ready,
                "IN_PROGRESS" => ExamStatus::InProgress,
                "FINISHED" => ExamStatus::Finished,
                _ => ExamStatus::Draft,
            };
            let network_mode = match mode_str.as_str() {
                "ONLINE_TUNNEL" => NetworkMode::OnlineTunnel,
                _ => NetworkMode::OfflineLan,
            };
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let started_at = started_at_opt.and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            });
            let end_at = end_at_opt.and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            });

            Ok(Some(Exam {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                title: row.get(1)?,
                subject: row.get(2)?,
                duration_minutes: row.get(3)?,
                exam_type,
                status,
                network_mode,
                public_url: row.get(7)?,
                created_at,
                started_at,
                end_at,
                source_format,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_exams(&self) -> Result<Vec<Exam>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, subject, duration_minutes, exam_type, status, network_mode, public_url, created_at, started_at, end_at, source_format
            FROM exams ORDER BY created_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let exam_type_str: String = row.get(4)?;
            let status_str: String = row.get(5)?;
            let mode_str: String = row.get(6)?;
            let created_at_str: String = row.get(8)?;
            let started_at_opt: Option<String> = row.get(9)?;
            let end_at_opt: Option<String> = row.get(10)?;
            let source_format: Option<String> = row.get(11).ok();

            let exam_type = match exam_type_str.as_str() {
                "TRADITIONAL" => ExamType::Traditional,
                _ => ExamType::Gdpt2018,
            };
            let status = match status_str.as_str() {
                "READY" => ExamStatus::Ready,
                "IN_PROGRESS" => ExamStatus::InProgress,
                "FINISHED" => ExamStatus::Finished,
                _ => ExamStatus::Draft,
            };
            let network_mode = match mode_str.as_str() {
                "ONLINE_TUNNEL" => NetworkMode::OnlineTunnel,
                _ => NetworkMode::OfflineLan,
            };

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let started_at = started_at_opt.and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            });
            let end_at = end_at_opt.and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            });

            Ok(Exam {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                title: row.get(1)?,
                subject: row.get(2)?,
                duration_minutes: row.get(3)?,
                exam_type,
                status,
                network_mode,
                public_url: row.get(7)?,
                created_at,
                started_at,
                end_at,
                source_format,
            })
        })?;

        let mut list = Vec::new();
        for e in rows {
            list.push(e?);
        }
        Ok(list)
    }

    pub fn save_variant_mapping(
        &self,
        id: Uuid,
        variant_id: Uuid,
        question_id: Uuid,
        display_order: i32,
        options_order_json: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO variant_question_mappings (id, variant_id, question_id, display_order, options_order_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                id.to_string(),
                variant_id.to_string(),
                question_id.to_string(),
                display_order,
                options_order_json,
            ],
        )?;
        Ok(())
    }

    pub fn get_variant_mappings(&self, variant_id: Uuid) -> Result<Vec<VariantQuestionMapping>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, variant_id, question_id, display_order, options_order_json
            FROM variant_question_mappings
            WHERE variant_id = ?1
            ORDER BY display_order ASC
            "#,
        )?;
        let rows = stmt.query_map(params![variant_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let var_id_str: String = row.get(1)?;
            let q_id_str: String = row.get(2)?;
            Ok(VariantQuestionMapping {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                variant_id: Uuid::parse_str(&var_id_str).unwrap_or_default(),
                question_id: Uuid::parse_str(&q_id_str).unwrap_or_default(),
                display_order: row.get(3)?,
                options_order_json: row.get(4)?,
            })
        })?;
        let mut mappings = Vec::new();
        for m in rows {
            mappings.push(m?);
        }
        Ok(mappings)
    }

    pub fn delete_exam(&self, exam_id: Uuid) -> Result<usize> {
        let affected = self.conn.execute(
            "DELETE FROM exams WHERE id = ?1",
            params![exam_id.to_string()],
        )?;
        Ok(affected)
    }

    pub fn delete_variant_by_code(&self, exam_id: Uuid, code: &str) -> Result<bool> {
        // Không cho phép xóa mã gốc 101 hoặc mã đang có thí sinh sử dụng
        let assigned_count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*) FROM examinees e
            JOIN exam_variants v ON e.assigned_variant_id = v.id
            WHERE v.exam_id = ?1 AND v.code = ?2
            "#,
            params![exam_id.to_string(), code],
            |row| row.get(0),
        )?;
        if assigned_count > 0 || code == "101" {
            return Ok(false);
        }
        let affected = self.conn.execute(
            "DELETE FROM exam_variants WHERE exam_id = ?1 AND code = ?2",
            params![exam_id.to_string(), code],
        )?;
        Ok(affected > 0)
    }

    pub fn update_exam_start_time(
        &self,
        exam_id: Uuid,
        started_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE exams SET started_at = ?1, end_at = ?2 WHERE id = ?3",
            params![
                started_at.to_rfc3339(),
                end_at.to_rfc3339(),
                exam_id.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn extend_exam_time(
        &self,
        exam_id: Uuid,
        add_minutes: i64,
    ) -> Result<Option<(u32, DateTime<Utc>, DateTime<Utc>)>> {
        let Some(exam) = self.get_exam(exam_id)? else {
            return Ok(None);
        };
        let new_duration = ((exam.duration_minutes as i64) + add_minutes).clamp(1, 600) as u32;
        let now = Utc::now();
        let started_at = exam.started_at.unwrap_or(now);
        let base_end = exam
            .end_at
            .map(|e| if e > now { e } else { now })
            .unwrap_or_else(|| started_at + chrono::Duration::minutes(exam.duration_minutes as i64));
        let new_end = base_end + chrono::Duration::minutes(add_minutes);
        self.conn.execute(
            "UPDATE exams SET duration_minutes = ?1, started_at = ?2, end_at = ?3 WHERE id = ?4",
            params![
                new_duration as i64,
                started_at.to_rfc3339(),
                new_end.to_rfc3339(),
                exam_id.to_string()
            ],
        )?;
        Ok(Some((new_duration, started_at, new_end)))
    }

    pub fn reset_exam_to_ready(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "UPDATE exams SET status = 'READY', started_at = NULL, end_at = NULL WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Làm sạch toàn bộ dữ liệu phòng thi của một đề thi:
    /// Xóa toàn bộ vi phạm, câu trả lời, bài nộp, và danh sách thí sinh đã tham gia,
    /// đồng thời đặt lại trạng thái đề thi về READY và xóa thời gian bắt đầu/kết thúc.
    pub fn clear_exam_room_data(&self, exam_id: Uuid) -> Result<()> {
        let exam_id_str = exam_id.to_string();

        // 1. Xóa audit_violations của các thí sinh trong kỳ thi
        self.conn.execute(
            r#"
            DELETE FROM audit_violations 
            WHERE examinee_id IN (SELECT id FROM examinees WHERE exam_id = ?1)
            "#,
            params![exam_id_str],
        )?;

        // 2. Xóa submission_answers của các bài nộp trong kỳ thi
        self.conn.execute(
            r#"
            DELETE FROM submission_answers 
            WHERE submission_id IN (
                SELECT s.id FROM submissions s 
                JOIN examinees e ON s.examinee_id = e.id 
                WHERE e.exam_id = ?1
            )
            "#,
            params![exam_id_str],
        )?;

        // 3. Xóa submissions của kỳ thi
        self.conn.execute(
            r#"
            DELETE FROM submissions 
            WHERE examinee_id IN (SELECT id FROM examinees WHERE exam_id = ?1)
            "#,
            params![exam_id_str],
        )?;

        // 4. Xóa examinees của kỳ thi
        self.conn.execute(
            "DELETE FROM examinees WHERE exam_id = ?1",
            params![exam_id_str],
        )?;

        // 5. Đặt lại trạng thái phòng thi về READY, xóa started_at & end_at
        self.conn.execute(
            "UPDATE exams SET status = 'READY', started_at = NULL, end_at = NULL, public_url = NULL, network_mode = 'OFFLINE_LAN' WHERE id = ?1",
            params![exam_id_str],
        )?;

        Ok(())
    }

    /// Tạo một bản sao độc lập của đề thi phục vụ mở phòng thi mới:
    /// Bản sao nhận UUID mới, tiêu đề phòng thi mới, trạng thái READY,
    /// sao chép toàn bộ câu hỏi (với UUID câu hỏi & đáp án mới),
    /// sao chép toàn bộ mã đề hoán vị và mapping câu hỏi.
    /// Bản sao hoàn toàn trắng thông tin thí sinh (0 thí sinh, 0 bài nộp).
    pub fn clone_exam_for_room(&self, source_exam_id: Uuid, room_title: &str) -> Result<Exam> {
        let source_exam = self
            .get_exam(source_exam_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;

        let new_exam_id = Uuid::new_v4();
        let new_exam = Exam {
            id: new_exam_id,
            title: room_title.to_string(),
            subject: source_exam.subject.clone(),
            duration_minutes: source_exam.duration_minutes,
            exam_type: source_exam.exam_type,
            status: ExamStatus::Ready,
            network_mode: source_exam.network_mode,
            public_url: source_exam.public_url.clone(),
            created_at: Utc::now(),
            started_at: None,
            end_at: None,
            source_format: source_exam.source_format.clone(),
        };
        self.create_exam(&new_exam)?;

        // Sao chép toàn bộ câu hỏi và đáp án
        let source_questions = self.get_questions_by_exam(source_exam_id)?;
        let mut q_id_map: std::collections::HashMap<Uuid, Uuid> = std::collections::HashMap::new();

        for q in source_questions {
            let old_qid = q.id;
            let new_qid = Uuid::new_v4();
            q_id_map.insert(old_qid, new_qid);

            let new_options: Vec<QuestionOption> = q
                .options
                .into_iter()
                .map(|opt| QuestionOption {
                    id: Uuid::new_v4(),
                    question_id: new_qid,
                    label: opt.label,
                    content_html: opt.content_html,
                    is_correct: opt.is_correct,
                })
                .collect();

            let new_q = Question {
                id: new_qid,
                exam_id: new_exam_id,
                part_type: q.part_type,
                order_index: q.order_index,
                content_html: q.content_html,
                raw_text: q.raw_text,
                image_url: q.image_url,
                short_answer_solution: q.short_answer_solution,
                options: new_options,
                provenance_json: q.provenance_json,
                answer_meta_json: q.answer_meta_json,
                needs_review: q.needs_review,
            };
            self.create_question(&new_q)?;
        }

        // Sao chép toàn bộ mã đề (variants) và variant_question_mappings
        let source_variants = self.get_variants_by_exam(source_exam_id)?;
        for v in source_variants {
            let old_vid = v.id;
            let new_vid = Uuid::new_v4();
            let new_v = ExamVariant {
                id: new_vid,
                exam_id: new_exam_id,
                code: v.code.clone(),
                is_original: v.is_original,
            };
            self.create_variant(&new_v)?;

            let mappings = self.get_variant_mappings(old_vid)?;
            for m in mappings {
                if let Some(&new_qid) = q_id_map.get(&m.question_id) {
                    self.save_variant_mapping(
                        Uuid::new_v4(),
                        new_vid,
                        new_qid,
                        m.display_order,
                        m.options_order_json.as_deref(),
                    )?;
                }
            }
        }

        Ok(new_exam)
    }

    pub fn update_exam_status(&self, id: Uuid, status: ExamStatus) -> Result<()> {
        let status_str = match status {
            ExamStatus::Draft => "DRAFT",
            ExamStatus::Ready => "READY",
            ExamStatus::InProgress => "IN_PROGRESS",
            ExamStatus::Finished => "FINISHED",
        };
        self.conn.execute(
            "UPDATE exams SET status = ?1 WHERE id = ?2",
            params![status_str, id.to_string()],
        )?;
        Ok(())
    }

    pub fn update_exam_network(
        &self,
        id: Uuid,
        mode: NetworkMode,
        public_url: Option<&str>,
    ) -> Result<()> {
        let mode_str = match mode {
            NetworkMode::OfflineLan => "OFFLINE_LAN",
            NetworkMode::OnlineTunnel => "ONLINE_TUNNEL",
        };
        self.conn.execute(
            "UPDATE exams SET network_mode = ?1, public_url = ?2 WHERE id = ?3",
            params![mode_str, public_url, id.to_string()],
        )?;
        Ok(())
    }

    // =========================================================================
    // VARIANTS
    // =========================================================================

    pub fn create_variant(&self, variant: &ExamVariant) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO exam_variants (id, exam_id, code, is_original)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                variant.id.to_string(),
                variant.exam_id.to_string(),
                variant.code,
                if variant.is_original { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn get_variants_by_exam(&self, exam_id: Uuid) -> Result<Vec<ExamVariant>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, exam_id, code, is_original FROM exam_variants WHERE exam_id = ?1 ORDER BY code ASC",
        )?;

        let rows = stmt.query_map(params![exam_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let exam_id_str: String = row.get(1)?;
            let is_orig: i32 = row.get(3)?;
            Ok(ExamVariant {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                exam_id: Uuid::parse_str(&exam_id_str).unwrap_or_default(),
                code: row.get(2)?,
                is_original: is_orig == 1,
            })
        })?;

        let mut variants = Vec::new();
        for v in rows {
            variants.push(v?);
        }
        Ok(variants)
    }

    // =========================================================================
    // QUESTIONS & OPTIONS
    // =========================================================================

    pub fn create_question(&self, q: &Question) -> Result<()> {
        let part_str = match q.part_type {
            PartType::Part1 => "PART_1",
            PartType::Part2 => "PART_2",
            PartType::Part3 => "PART_3",
        };

        self.conn.execute(
            r#"
            INSERT INTO questions (id, exam_id, part_type, order_index, content_html, raw_text, image_url, short_answer_solution, provenance_json, answer_meta_json, needs_review)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                q.id.to_string(),
                q.exam_id.to_string(),
                part_str,
                q.order_index,
                q.content_html,
                q.raw_text,
                q.image_url,
                q.short_answer_solution,
                q.provenance_json,
                q.answer_meta_json,
                if q.needs_review { 1 } else { 0 },
            ],
        )?;

        for opt in &q.options {
            self.conn.execute(
                r#"
                INSERT INTO question_options (id, question_id, label, content_html, is_correct)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    opt.id.to_string(),
                    q.id.to_string(),
                    opt.label,
                    opt.content_html,
                    if opt.is_correct { 1 } else { 0 },
                ],
            )?;
        }

        Ok(())
    }

    pub fn get_questions_by_exam(&self, exam_id: Uuid) -> Result<Vec<Question>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, exam_id, part_type, order_index, content_html, raw_text, image_url, short_answer_solution, provenance_json, answer_meta_json, needs_review
            FROM questions
            WHERE exam_id = ?1
            ORDER BY part_type ASC, order_index ASC
            "#,
        )?;

        let q_rows = stmt.query_map(params![exam_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let exam_id_str: String = row.get(1)?;
            let part_str: String = row.get(2)?;
            let part_type = match part_str.as_str() {
                "PART_2" => PartType::Part2,
                "PART_3" => PartType::Part3,
                _ => PartType::Part1,
            };
            let needs_rev_int: i32 = row.get(10).unwrap_or(0);

            Ok(Question {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                exam_id: Uuid::parse_str(&exam_id_str).unwrap_or_default(),
                part_type,
                order_index: row.get(3)?,
                content_html: row.get(4)?,
                raw_text: row.get(5)?,
                image_url: row.get(6)?,
                short_answer_solution: row.get(7)?,
                provenance_json: row.get(8).ok(),
                answer_meta_json: row.get(9).ok(),
                needs_review: needs_rev_int != 0,
                options: Vec::new(),
            })
        })?;

        let mut questions = Vec::new();
        for q in q_rows {
            let mut question = q?;
            // Lấy options
            let mut opt_stmt = self.conn.prepare(
                "SELECT id, question_id, label, content_html, is_correct FROM question_options WHERE question_id = ?1 ORDER BY label ASC",
            )?;
            let opt_rows = opt_stmt.query_map(params![question.id.to_string()], |orow| {
                let opt_id_str: String = orow.get(0)?;
                let q_id_str: String = orow.get(1)?;
                let is_corr: i32 = orow.get(4)?;
                Ok(QuestionOption {
                    id: Uuid::parse_str(&opt_id_str).unwrap_or_default(),
                    question_id: Uuid::parse_str(&q_id_str).unwrap_or_default(),
                    label: orow.get(2)?,
                    content_html: orow.get(3)?,
                    is_correct: is_corr == 1,
                })
            })?;

            for opt in opt_rows {
                question.options.push(opt?);
            }
            questions.push(question);
        }

        Ok(questions)
    }

    // =========================================================================
    // EXAMINEES
    // =========================================================================

    pub fn create_examinee(&self, e: &Examinee) -> Result<()> {
        let status_str = match e.status {
            ExamineeStatus::Waiting => "WAITING",
            ExamineeStatus::Testing => "TESTING",
            ExamineeStatus::Submitted => "SUBMITTED",
            ExamineeStatus::Disconnected => "DISCONNECTED",
        };

        self.conn.execute(
            r#"
            INSERT INTO examinees (id, exam_id, assigned_variant_id, student_code, full_name, class_name, session_token, status, ip_address, user_agent, joined_at, last_heartbeat)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                e.id.to_string(),
                e.exam_id.to_string(),
                e.assigned_variant_id.map(|v| v.to_string()),
                e.student_code,
                e.full_name,
                e.class_name,
                e.session_token,
                status_str,
                e.ip_address,
                e.user_agent,
                e.joined_at.to_rfc3339(),
                e.last_heartbeat.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_examinee_by_token(&self, token: &str) -> Result<Option<Examinee>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, exam_id, assigned_variant_id, student_code, full_name, class_name, session_token, status, ip_address, user_agent, joined_at, last_heartbeat
            FROM examinees WHERE session_token = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![token])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let exam_id_str: String = row.get(1)?;
            let var_id_str: Option<String> = row.get(2)?;
            let status_str: String = row.get(7)?;
            let joined_at_str: String = row.get(10)?;
            let last_hb_str: String = row.get(11)?;

            let status = match status_str.as_str() {
                "TESTING" => ExamineeStatus::Testing,
                "SUBMITTED" => ExamineeStatus::Submitted,
                "DISCONNECTED" => ExamineeStatus::Disconnected,
                _ => ExamineeStatus::Waiting,
            };

            Ok(Some(Examinee {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                exam_id: Uuid::parse_str(&exam_id_str).unwrap_or_default(),
                assigned_variant_id: var_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                student_code: row.get(3)?,
                full_name: row.get(4)?,
                class_name: row.get(5)?,
                session_token: row.get(6)?,
                status,
                ip_address: row.get(8)?,
                user_agent: row.get(9)?,
                joined_at: DateTime::parse_from_rfc3339(&joined_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_heartbeat: DateTime::parse_from_rfc3339(&last_hb_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_examinee_by_code_and_exam(&self, exam_id: Uuid, code: &str) -> Result<Option<Examinee>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, exam_id, assigned_variant_id, student_code, full_name, class_name, session_token, status, ip_address, user_agent, joined_at, last_heartbeat
            FROM examinees WHERE exam_id = ?1 AND student_code = ?2
            "#,
        )?;

        let mut rows = stmt.query(params![exam_id.to_string(), code])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let exam_id_str: String = row.get(1)?;
            let var_id_str: Option<String> = row.get(2)?;
            let status_str: String = row.get(7)?;
            let joined_at_str: String = row.get(10)?;
            let last_hb_str: String = row.get(11)?;

            let status = match status_str.as_str() {
                "TESTING" => ExamineeStatus::Testing,
                "SUBMITTED" => ExamineeStatus::Submitted,
                "DISCONNECTED" => ExamineeStatus::Disconnected,
                _ => ExamineeStatus::Waiting,
            };

            Ok(Some(Examinee {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                exam_id: Uuid::parse_str(&exam_id_str).unwrap_or_default(),
                assigned_variant_id: var_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                student_code: row.get(3)?,
                full_name: row.get(4)?,
                class_name: row.get(5)?,
                session_token: row.get(6)?,
                status,
                ip_address: row.get(8)?,
                user_agent: row.get(9)?,
                joined_at: DateTime::parse_from_rfc3339(&joined_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_heartbeat: DateTime::parse_from_rfc3339(&last_hb_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_examinee_by_code(&self, code: &str) -> Result<Option<Examinee>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, exam_id, assigned_variant_id, student_code, full_name, class_name, session_token, status, ip_address, user_agent, joined_at, last_heartbeat
            FROM examinees WHERE student_code = ?1 ORDER BY joined_at DESC LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![code])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let exam_id_str: String = row.get(1)?;
            let var_id_str: Option<String> = row.get(2)?;
            let status_str: String = row.get(7)?;
            let joined_at_str: String = row.get(10)?;
            let last_hb_str: String = row.get(11)?;

            let status = match status_str.as_str() {
                "TESTING" => ExamineeStatus::Testing,
                "SUBMITTED" => ExamineeStatus::Submitted,
                "DISCONNECTED" => ExamineeStatus::Disconnected,
                _ => ExamineeStatus::Waiting,
            };

            Ok(Some(Examinee {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                exam_id: Uuid::parse_str(&exam_id_str).unwrap_or_default(),
                assigned_variant_id: var_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                student_code: row.get(3)?,
                full_name: row.get(4)?,
                class_name: row.get(5)?,
                session_token: row.get(6)?,
                status,
                ip_address: row.get(8)?,
                user_agent: row.get(9)?,
                joined_at: DateTime::parse_from_rfc3339(&joined_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_heartbeat: DateTime::parse_from_rfc3339(&last_hb_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_examinees_by_exam(&self, exam_id: Uuid) -> Result<Vec<Examinee>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, exam_id, assigned_variant_id, student_code, full_name, class_name, session_token, status, ip_address, user_agent, joined_at, last_heartbeat
            FROM examinees WHERE exam_id = ?1 ORDER BY class_name ASC, student_code ASC
            "#,
        )?;

        let rows = stmt.query_map(params![exam_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let exam_id_str: String = row.get(1)?;
            let var_id_str: Option<String> = row.get(2)?;
            let status_str: String = row.get(7)?;
            let joined_at_str: String = row.get(10)?;
            let last_hb_str: String = row.get(11)?;

            let status = match status_str.as_str() {
                "TESTING" => ExamineeStatus::Testing,
                "SUBMITTED" => ExamineeStatus::Submitted,
                "DISCONNECTED" => ExamineeStatus::Disconnected,
                _ => ExamineeStatus::Waiting,
            };

            Ok(Examinee {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                exam_id: Uuid::parse_str(&exam_id_str).unwrap_or_default(),
                assigned_variant_id: var_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                student_code: row.get(3)?,
                full_name: row.get(4)?,
                class_name: row.get(5)?,
                session_token: row.get(6)?,
                status,
                ip_address: row.get(8)?,
                user_agent: row.get(9)?,
                joined_at: DateTime::parse_from_rfc3339(&joined_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_heartbeat: DateTime::parse_from_rfc3339(&last_hb_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut examinees = Vec::new();
        for e in rows {
            examinees.push(e?);
        }
        Ok(examinees)
    }

    pub fn update_examinee_status(&self, id: Uuid, status: ExamineeStatus) -> Result<()> {
        let status_str = match status {
            ExamineeStatus::Waiting => "WAITING",
            ExamineeStatus::Testing => "TESTING",
            ExamineeStatus::Submitted => "SUBMITTED",
            ExamineeStatus::Disconnected => "DISCONNECTED",
        };
        self.conn.execute(
            "UPDATE examinees SET status = ?1 WHERE id = ?2",
            params![status_str, id.to_string()],
        )?;
        Ok(())
    }

    pub fn update_examinee_session(
        &self,
        id: Uuid,
        session_token: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE examinees SET session_token = ?1, ip_address = ?2, user_agent = ?3, last_heartbeat = ?4 WHERE id = ?5",
            params![
                session_token,
                ip_address,
                user_agent,
                Utc::now().to_rfc3339(),
                id.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn update_examinee_heartbeat(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "UPDATE examinees SET last_heartbeat = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id.to_string()],
        )?;
        Ok(())
    }

    // =========================================================================
    // SUBMISSIONS
    // =========================================================================

    pub fn create_submission(&self, s: &Submission) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO submissions (id, examinee_id, started_at, submitted_at, total_score, part1_score, part2_score, part3_score, is_graded)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                s.id.to_string(),
                s.examinee_id.to_string(),
                s.started_at.map(|d| d.to_rfc3339()),
                s.submitted_at.map(|d| d.to_rfc3339()),
                s.total_score,
                s.part1_score,
                s.part2_score,
                s.part3_score,
                if s.is_graded { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn get_submission_by_examinee(&self, examinee_id: Uuid) -> Result<Option<Submission>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, examinee_id, started_at, submitted_at, total_score, part1_score, part2_score, part3_score, is_graded
            FROM submissions WHERE examinee_id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![examinee_id.to_string()])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let ex_id_str: String = row.get(1)?;
            let started_str: Option<String> = row.get(2)?;
            let sub_str: Option<String> = row.get(3)?;
            let is_graded_val: i32 = row.get(8)?;

            Ok(Some(Submission {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                examinee_id: Uuid::parse_str(&ex_id_str).unwrap_or_default(),
                started_at: started_str.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                submitted_at: sub_str.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                total_score: row.get(4)?,
                part1_score: row.get(5)?,
                part2_score: row.get(6)?,
                part3_score: row.get(7)?,
                is_graded: is_graded_val == 1,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_submissions_by_exam(&self, exam_id: Uuid) -> Result<Vec<(Examinee, Option<Submission>)>> {
        let examinees = self.list_examinees_by_exam(exam_id)?;
        let mut results = Vec::new();

        for ex in examinees {
            let sub = self.get_submission_by_examinee(ex.id)?;
            results.push((ex, sub));
        }

        Ok(results)
    }

    // =========================================================================
    // SUBMISSION ANSWERS & RE-GRADING
    // =========================================================================

    pub fn save_submission_answers(&self, submission_id: Uuid, answers: &[SubmissionAnswerRecord]) -> Result<()> {
        let mut stmt = self.conn.prepare(
            r#"
            INSERT INTO submission_answers (
                id, submission_id, question_id, selected_option, sub_answers_json, short_answer_text, awarded_score, is_correct
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(submission_id, question_id) DO UPDATE SET
                selected_option = excluded.selected_option,
                sub_answers_json = excluded.sub_answers_json,
                short_answer_text = excluded.short_answer_text,
                awarded_score = excluded.awarded_score,
                is_correct = excluded.is_correct
            "#,
        )?;

        for ans in answers {
            stmt.execute(params![
                ans.id.to_string(),
                submission_id.to_string(),
                ans.question_id.to_string(),
                ans.selected_option,
                ans.sub_answers_json,
                ans.short_answer_text,
                ans.awarded_score,
                if ans.is_correct { 1 } else { 0 },
            ])?;
        }

        Ok(())
    }

    pub fn get_submission_answers(&self, submission_id: Uuid) -> Result<Vec<SubmissionAnswerRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, submission_id, question_id, selected_option, sub_answers_json, short_answer_text, awarded_score, is_correct
            FROM submission_answers
            WHERE submission_id = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![submission_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let sub_id_str: String = row.get(1)?;
            let q_id_str: String = row.get(2)?;
            let is_correct_val: i32 = row.get(7)?;

            Ok(SubmissionAnswer {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                submission_id: Uuid::parse_str(&sub_id_str).unwrap_or_default(),
                question_id: Uuid::parse_str(&q_id_str).unwrap_or_default(),
                selected_option: row.get(3)?,
                sub_answers_json: row.get(4)?,
                short_answer_text: row.get(5)?,
                awarded_score: row.get(6)?,
                is_correct: is_correct_val == 1,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn update_question_options_correctness(&self, question_id: Uuid, correct_labels: &[String]) -> Result<()> {
        self.conn.execute(
            "UPDATE question_options SET is_correct = 0 WHERE question_id = ?1",
            params![question_id.to_string()],
        )?;

        let mut stmt = self.conn.prepare(
            "UPDATE question_options SET is_correct = 1 WHERE question_id = ?1 AND (
                LOWER(TRIM(label)) = LOWER(TRIM(?2))
                OR LOWER(TRIM(REPLACE(REPLACE(REPLACE(label, ')', ''), '.', ''), ':', ''))) = LOWER(TRIM(REPLACE(REPLACE(REPLACE(?2, ')', ''), '.', ''), ':', '')))
            )",
        )?;

        for label in correct_labels {
            stmt.execute(params![question_id.to_string(), label.trim()])?;
        }

        Ok(())
    }

    pub fn update_question_short_answer(&self, question_id: Uuid, short_answer: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE questions SET short_answer_solution = ?1 WHERE id = ?2",
            params![short_answer, question_id.to_string()],
        )?;
        Ok(())
    }

    pub fn update_question_content(
        &self,
        question_id: Uuid,
        content_html: &str,
        raw_text: Option<&str>,
        clear_needs_review: bool,
    ) -> Result<()> {
        if clear_needs_review {
            self.conn.execute(
                "UPDATE questions SET content_html = ?1, raw_text = COALESCE(?2, raw_text), needs_review = 0 WHERE id = ?3",
                params![content_html, raw_text, question_id.to_string()],
            )?;
        } else {
            self.conn.execute(
                "UPDATE questions SET content_html = ?1, raw_text = COALESCE(?2, raw_text) WHERE id = ?3",
                params![content_html, raw_text, question_id.to_string()],
            )?;
        }
        Ok(())
    }

    pub fn update_question_option_content(
        &self,
        option_id: Uuid,
        content_html: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE question_options SET content_html = ?1 WHERE id = ?2",
            params![content_html, option_id.to_string()],
        )?;
        Ok(())
    }

    pub fn update_question_needs_review(&self, question_id: Uuid, needs_review: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE questions SET needs_review = ?1 WHERE id = ?2",
            params![if needs_review { 1 } else { 0 }, question_id.to_string()],
        )?;
        Ok(())
    }

    pub fn update_submission_scores(&self, submission_id: Uuid, p1: f64, p2: f64, p3: f64, total: f64) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE submissions
            SET part1_score = ?1, part2_score = ?2, part3_score = ?3, total_score = ?4, is_graded = 1
            WHERE id = ?5
            "#,
            params![p1, p2, p3, total, submission_id.to_string()],
        )?;
        Ok(())
    }

    pub fn get_exam_id_by_question(&self, question_id: Uuid) -> Result<Option<Uuid>> {
        let mut stmt = self.conn.prepare("SELECT exam_id FROM questions WHERE id = ?1")?;
        let mut rows = stmt.query(params![question_id.to_string()])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(Uuid::parse_str(&s).ok())
        } else {
            Ok(None)
        }
    }

    // =========================================================================
    // AUDIT VIOLATIONS
    // =========================================================================

    pub fn create_violation(&self, v: &AuditViolation) -> Result<()> {
        let vtype_str = match v.violation_type {
            ViolationType::TabSwitch => "TAB_SWITCH",
            ViolationType::Blur => "BLUR",
            ViolationType::FullscreenExit => "FULLSCREEN_EXIT",
            ViolationType::CopyAttempt => "COPY_ATTEMPT",
            ViolationType::PrintScreen => "PRINT_SCREEN",
        };

        self.conn.execute(
            r#"
            INSERT INTO audit_violations (id, examinee_id, violation_type, duration_seconds, details, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                v.id.to_string(),
                v.examinee_id.to_string(),
                vtype_str,
                v.duration_seconds,
                v.details,
                v.timestamp.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_violations_by_examinee(&self, examinee_id: Uuid) -> Result<Vec<AuditViolation>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, examinee_id, violation_type, duration_seconds, details, timestamp
            FROM audit_violations
            WHERE examinee_id = ?1
            ORDER BY timestamp ASC
            "#,
        )?;

        let rows = stmt.query_map(params![examinee_id.to_string()], |row| {
            let v_id_str: String = row.get(0)?;
            let ex_id_str: String = row.get(1)?;
            let vtype_str: String = row.get(2)?;
            let ts_str: String = row.get(5)?;

            let violation_type = match vtype_str.as_str() {
                "BLUR" => ViolationType::Blur,
                "FULLSCREEN_EXIT" => ViolationType::FullscreenExit,
                "COPY_ATTEMPT" => ViolationType::CopyAttempt,
                "PRINT_SCREEN" => ViolationType::PrintScreen,
                _ => ViolationType::TabSwitch,
            };

            Ok(AuditViolation {
                id: Uuid::parse_str(&v_id_str).unwrap_or_default(),
                examinee_id: Uuid::parse_str(&ex_id_str).unwrap_or_default(),
                violation_type,
                duration_seconds: row.get(3)?,
                details: row.get(4)?,
                timestamp: DateTime::parse_from_rfc3339(&ts_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn list_violations_by_exam(&self, exam_id: Uuid) -> Result<Vec<(Examinee, AuditViolation)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT v.id, v.examinee_id, v.violation_type, v.duration_seconds, v.details, v.timestamp,
                   e.student_code, e.full_name, e.class_name
            FROM audit_violations v
            JOIN examinees e ON v.examinee_id = e.id
            WHERE e.exam_id = ?1
            ORDER BY v.timestamp DESC
            "#,
        )?;

        let rows = stmt.query_map(params![exam_id.to_string()], |row| {
            let v_id_str: String = row.get(0)?;
            let ex_id_str: String = row.get(1)?;
            let vtype_str: String = row.get(2)?;
            let ts_str: String = row.get(5)?;

            let violation_type = match vtype_str.as_str() {
                "BLUR" => ViolationType::Blur,
                "FULLSCREEN_EXIT" => ViolationType::FullscreenExit,
                "COPY_ATTEMPT" => ViolationType::CopyAttempt,
                "PRINT_SCREEN" => ViolationType::PrintScreen,
                _ => ViolationType::TabSwitch,
            };

            let violation = AuditViolation {
                id: Uuid::parse_str(&v_id_str).unwrap_or_default(),
                examinee_id: Uuid::parse_str(&ex_id_str).unwrap_or_default(),
                violation_type,
                duration_seconds: row.get(3)?,
                details: row.get(4)?,
                timestamp: DateTime::parse_from_rfc3339(&ts_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            };

            let examinee = Examinee {
                id: Uuid::parse_str(&ex_id_str).unwrap_or_default(),
                exam_id,
                assigned_variant_id: None,
                student_code: row.get(6)?,
                full_name: row.get(7)?,
                class_name: row.get(8)?,
                session_token: String::new(),
                status: ExamineeStatus::Waiting,
                ip_address: None,
                user_agent: None,
                joined_at: Utc::now(),
                last_heartbeat: Utc::now(),
            };

            Ok((examinee, violation))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }
}
