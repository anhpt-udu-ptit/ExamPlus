//! # Student Exam Handlers
//!
//! Xử lý tải trạng thái phòng chờ, đề thi bảo mật, lưu nháp và nộp bài chấm điểm trực tiếp.

use crate::handlers::auth::{ApiErrorDetail, ApiErrorResponse};
use crate::state::{AppState, ServerEvent};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use examplus_core::db::VariantQuestionMapping;
use examplus_core::engine::scoring::{
    calculate_part2_score, calculate_total_score, normalize_number_string, StudentSubmission,
};
use examplus_core::models::{
    ExamStatus, ExamineeStatus, PartType, Question, Submission, SubmissionAnswer,
};
use examplus_core::parser::answer_key::TrueFalseAnswer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct LobbyStatusResponse {
    pub exam_status: String,
    pub server_time: String,
    pub remaining_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct ExamPaperResponse {
    pub exam_id: String,
    pub title: String,
    pub duration_minutes: u32,
    pub remaining_seconds: u64,
    pub started_at: Option<String>,
    pub end_at: Option<String>,
    pub variant_code: String,
    pub questions: Vec<SanitizedQuestionItem>,
}

#[derive(Debug, Serialize)]
pub struct SanitizedQuestionItem {
    pub id: String,
    pub display_number: u32,
    pub part_type: String,
    pub content_html: String,
    pub image_url: Option<String>,
    pub options: Vec<SanitizedOption>,
}

#[derive(Debug, Serialize)]
pub struct SanitizedOption {
    pub label: String,
    pub content_html: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubmissionDraft {
    pub answers: Vec<DraftAnswerItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DraftAnswerItem {
    pub question_id: String,
    pub selected_option: Option<String>,
    pub sub_answers: Option<HashMap<String, bool>>,
    pub short_answer: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitResultResponse {
    pub success: bool,
    pub submitted_at: String,
    pub total_score: f64,
    pub part1_score: f64,
    pub part2_score: f64,
    pub part3_score: f64,
}

pub fn extract_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            if s.starts_with("Bearer ") {
                Some(s[7..].trim().to_string())
            } else {
                Some(s.trim().to_string())
            }
        })
}

pub async fn get_lobby_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "UNAUTHORIZED".to_string(),
                        message: "Thiếu Session Token xác thực".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let examinee = match repo.get_examinee_by_token(&token) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_TOKEN".to_string(),
                        message: "Phiên làm việc không hợp lệ hoặc đã hết hạn".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let exam = match repo.get_exam(examinee.exam_id) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Không tìm thấy kỳ thi".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let status_str = match exam.status {
        ExamStatus::InProgress => "IN_PROGRESS",
        ExamStatus::Finished => "FINISHED",
        _ => "WAITING",
    };

    let remaining_seconds = match exam.end_at {
        Some(end_at) => (end_at - Utc::now()).num_seconds().max(0) as u64,
        None => (exam.duration_minutes as u64) * 60,
    };

    (
        StatusCode::OK,
        Json(LobbyStatusResponse {
            exam_status: status_str.to_string(),
            server_time: Utc::now().to_rfc3339(),
            remaining_seconds,
        }),
    )
        .into_response()
}

pub async fn get_exam_paper(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "UNAUTHORIZED".to_string(),
                        message: "Thiếu Session Token".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let examinee = match repo.get_examinee_by_token(&token) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_TOKEN".to_string(),
                        message: "Token không hợp lệ".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let exam = match repo.get_exam(examinee.exam_id) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Không tìm thấy đề thi".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    // 1. Kiểm tra trạng thái ca thi: Thí sinh chỉ được lấy đề khi ca thi đã bắt đầu
    let is_started = exam.started_at.is_some()
        || exam.status == ExamStatus::InProgress
        || exam.status == ExamStatus::Finished;

    if !is_started {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EXAM_NOT_STARTED".to_string(),
                    message: "Ca thi chưa bắt đầu. Vui lòng đợi trong phòng chờ cho đến khi giám thị bấm bắt đầu!".to_string(),
                },
            }),
        )
            .into_response();
    }

    // 2. Chặn thí sinh đã nộp bài tải lại đề
    if examinee.status == ExamineeStatus::Submitted {
        return (
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "ALREADY_SUBMITTED".to_string(),
                    message: "Thí sinh đã nộp bài thi rồi và không thể tải lại đề thi.".to_string(),
                },
            }),
        )
            .into_response();
    }

    let raw_questions = repo.get_questions_by_exam(exam.id).unwrap_or_default();
    let mappings = examinee
        .assigned_variant_id
        .and_then(|vid| repo.get_variant_mappings(vid).ok())
        .unwrap_or_default();
    let questions = apply_variant_mapping(raw_questions, &mappings);

    // Ẩn đáp án đúng hoàn toàn trước khi trả về trình duyệt học sinh
    let sanitized_questions: Vec<SanitizedQuestionItem> = questions
        .into_iter()
        .map(|q| {
            let part_str = match q.part_type {
                PartType::Part1 => "PART_1",
                PartType::Part2 => "PART_2",
                PartType::Part3 => "PART_3",
            };

            let options = q
                .options
                .into_iter()
                .map(|opt| SanitizedOption {
                    label: opt.label,
                    content_html: opt.content_html,
                })
                .collect();

            SanitizedQuestionItem {
                id: q.id.to_string(),
                display_number: q.order_index,
                part_type: part_str.to_string(),
                content_html: q.content_html,
                image_url: q.image_url,
                options,
            }
        })
        .collect();

    let remaining_seconds = match exam.end_at {
        Some(end_at) => (end_at - Utc::now()).num_seconds().max(0) as u64,
        None => (exam.duration_minutes as u64) * 60,
    };
    let started_at = exam.started_at.map(|dt| dt.to_rfc3339());
    let end_at = exam.end_at.map(|dt| dt.to_rfc3339());

    let assigned_variant_code = if let Some(var_id) = examinee.assigned_variant_id {
        repo.get_variants_by_exam(exam.id)
            .ok()
            .and_then(|vars| vars.into_iter().find(|v| v.id == var_id).map(|v| v.code))
            .unwrap_or_else(|| "101".to_string())
    } else {
        "101".to_string()
    };

    (
        StatusCode::OK,
        Json(ExamPaperResponse {
            exam_id: exam.id.to_string(),
            title: exam.title,
            duration_minutes: exam.duration_minutes,
            remaining_seconds,
            started_at,
            end_at,
            variant_code: assigned_variant_code,
            questions: sanitized_questions,
        }),
    )
        .into_response()
}

pub async fn save_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_draft): Json<SubmissionDraft>,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "UNAUTHORIZED".to_string(),
                        message: "Thiếu token xác thực".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let examinee = match repo.get_examinee_by_token(&token) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_TOKEN".to_string(),
                        message: "Token không hợp lệ".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    if examinee.status == ExamineeStatus::Submitted {
        return (
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "ALREADY_SUBMITTED".to_string(),
                    message: "Thí sinh đã nộp bài thi rồi và không thể lưu nháp.".to_string(),
                },
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "saved_at": Utc::now().to_rfc3339()
        })),
    )
        .into_response()
}

pub async fn submit_exam(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(draft): Json<SubmissionDraft>,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "UNAUTHORIZED".to_string(),
                        message: "Thiếu token xác thực".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let examinee = match repo.get_examinee_by_token(&token) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_TOKEN".to_string(),
                        message: "Token không hợp lệ".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    // Kiểm tra chặn nộp đúp (Double Submission Prevention)
    if examinee.status == ExamineeStatus::Submitted {
        return (
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "ALREADY_SUBMITTED".to_string(),
                    message: "Thí sinh đã nộp bài thi rồi và không thể nộp lại.".to_string(),
                },
            }),
        )
            .into_response();
    }

    let exam = match repo.get_exam(examinee.exam_id) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Không tìm thấy kỳ thi".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let is_started = exam.started_at.is_some()
        || exam.status == ExamStatus::InProgress
        || exam.status == ExamStatus::Finished;

    if !is_started {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EXAM_NOT_ACTIVE".to_string(),
                    message: "Ca thi chưa bắt đầu làm bài.".to_string(),
                },
            }),
        )
            .into_response();
    }

    // Kiểm tra thời hạn làm bài (kèm ân hạn 30 giây chống mất bài do độ trễ mạng)
    if let Some(end_at) = exam.end_at {
        if Utc::now() > end_at + chrono::Duration::seconds(30) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_EXPIRED".to_string(),
                        message: "Ca thi đã kết thúc và hết thời gian ân hạn nộp bài (30 giây)".to_string(),
                    },
                }),
            )
                .into_response();
        }
    }

    let questions = repo.get_questions_by_exam(examinee.exam_id).unwrap_or_default();

    // Xây dựng bảng đáp án chuẩn từ các câu hỏi trong CSDL
    let mut part1_key: HashMap<u32, String> = HashMap::new();
    let mut part2_key: HashMap<u32, TrueFalseAnswer> = HashMap::new();
    let mut part3_key: HashMap<u32, String> = HashMap::new();

    for q in &questions {
        match q.part_type {
            PartType::Part1 => {
                if let Some(correct) = q.options.iter().find(|o| o.is_correct) {
                    part1_key.insert(q.order_index, correct.label.clone());
                }
            }
            PartType::Part2 => {
                let mut tf = TrueFalseAnswer {
                    a: false,
                    b: false,
                    c: false,
                    d: false,
                };
                for opt in &q.options {
                    match opt.label.to_lowercase().as_str() {
                        "a" => tf.a = opt.is_correct,
                        "b" => tf.b = opt.is_correct,
                        "c" => tf.c = opt.is_correct,
                        "d" => tf.d = opt.is_correct,
                        _ => {}
                    }
                }
                part2_key.insert(q.order_index, tf);
            }
            PartType::Part3 => {
                if let Some(sol) = &q.short_answer_solution {
                    part3_key.insert(q.order_index, sol.clone());
                }
            }
        }
    }

    // Ánh xạ ngược lựa chọn từ mã đề đã xáo về đáp án gốc
    let variant_options_map: HashMap<String, String> = examinee
        .assigned_variant_id
        .and_then(|vid| repo.get_variant_mappings(vid).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| m.options_order_json.map(|j| (m.question_id.to_string(), j)))
        .collect();

    // Bóc tách bài làm của thí sinh
    let mut submission_data = StudentSubmission::default();
    let q_map: HashMap<String, (u32, PartType)> = questions
        .iter()
        .map(|q| (q.id.to_string(), (q.order_index, q.part_type)))
        .collect();

    let draft_map: HashMap<String, &DraftAnswerItem> = draft
        .answers
        .iter()
        .map(|a| (a.question_id.clone(), a))
        .collect();

    for ans in &draft.answers {
        if let Some((order_idx, part_type)) = q_map.get(&ans.question_id) {
            match part_type {
                PartType::Part1 => {
                    if let Some(opt) = &ans.selected_option {
                        let canonical_opt = unmap_variant_option(
                            opt,
                            variant_options_map.get(&ans.question_id).map(|s| s.as_str()),
                        );
                        submission_data.part1.insert(*order_idx, canonical_opt);
                    }
                }
                PartType::Part2 => {
                    if let Some(sub) = &ans.sub_answers {
                        let tf = TrueFalseAnswer {
                            a: sub.get("a").or_else(|| sub.get("A")).copied().unwrap_or(false),
                            b: sub.get("b").or_else(|| sub.get("B")).copied().unwrap_or(false),
                            c: sub.get("c").or_else(|| sub.get("C")).copied().unwrap_or(false),
                            d: sub.get("d").or_else(|| sub.get("D")).copied().unwrap_or(false),
                        };
                        submission_data.part2.insert(*order_idx, tf);
                    }
                }
                PartType::Part3 => {
                    if let Some(sa) = &ans.short_answer {
                        submission_data.part3.insert(*order_idx, sa.clone());
                    }
                }
            }
        }
    }

    // Tính điểm tổng hợp chuẩn GDPT 2018
    let score_res = calculate_total_score(
        &submission_data,
        &part1_key,
        &part2_key,
        &part3_key,
        0.25,
        0.5,
    );

    let submitted_at = Utc::now();
    let submission_record = Submission {
        id: Uuid::new_v4(),
        examinee_id: examinee.id,
        started_at: Some(examinee.joined_at),
        submitted_at: Some(submitted_at),
        total_score: score_res.total_score,
        part1_score: score_res.part1_score,
        part2_score: score_res.part2_score,
        part3_score: score_res.part3_score,
        is_graded: true,
    };

    let _ = repo.create_submission(&submission_record);

    // Lưu chi tiết từng câu trả lời vào bảng submission_answers
    let mut submission_answers = Vec::new();
    for q in &questions {
        let q_id_str = q.id.to_string();
        let draft_ans = draft_map.get(&q_id_str);

        let selected_option = draft_ans
            .and_then(|a| a.selected_option.as_ref())
            .map(|opt| {
                unmap_variant_option(
                    opt,
                    variant_options_map.get(&q_id_str).map(|s| s.as_str()),
                )
            });
        let sub_answers_json = draft_ans
            .and_then(|a| a.sub_answers.as_ref())
            .and_then(|m| serde_json::to_string(m).ok());
        let short_answer_text = draft_ans.and_then(|a| a.short_answer.clone());

        let (awarded_score, is_correct) = match q.part_type {
            PartType::Part1 => {
                let is_corr = if let Some(ref opt) = selected_option {
                    part1_key
                        .get(&q.order_index)
                        .map(|k| k.trim().eq_ignore_ascii_case(opt.trim()))
                        .unwrap_or(false)
                } else {
                    false
                };
                (if is_corr { 0.25 } else { 0.0 }, is_corr)
            }
            PartType::Part2 => {
                let score = if let Some(student_tf) = submission_data.part2.get(&q.order_index) {
                    if let Some(std_tf) = part2_key.get(&q.order_index) {
                        calculate_part2_score(student_tf, std_tf)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                (score, score == 1.0)
            }
            PartType::Part3 => {
                let is_corr = if let Some(ref sa) = short_answer_text {
                    if let Some(std) = part3_key.get(&q.order_index) {
                        let ns = normalize_number_string(sa);
                        let nk = normalize_number_string(std);
                        if !ns.is_empty() {
                            if ns.eq_ignore_ascii_case(&nk) {
                                true
                            } else if let (Ok(s_val), Ok(k_val)) = (ns.parse::<f64>(), nk.parse::<f64>()) {
                                (s_val - k_val).abs() < 1e-6
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };
                (if is_corr { 0.5 } else { 0.0 }, is_corr)
            }
        };

        submission_answers.push(SubmissionAnswer {
            id: Uuid::new_v4(),
            submission_id: submission_record.id,
            question_id: q.id,
            selected_option,
            sub_answers_json,
            short_answer_text,
            awarded_score,
            is_correct,
        });
    }

    let _ = repo.save_submission_answers(submission_record.id, &submission_answers);
    let _ = repo.update_examinee_status(examinee.id, ExamineeStatus::Submitted);

    // Bắn thông báo nộp bài tới giáo viên
    let _ = state.event_sender.send(ServerEvent::StudentSubmitted {
        student_code: examinee.student_code,
        total_score: score_res.total_score,
        exam_id: Some(examinee.exam_id.to_string()),
    });

    (
        StatusCode::OK,
        Json(SubmitResultResponse {
            success: true,
            submitted_at: submitted_at.to_rfc3339(),
            total_score: score_res.total_score,
            part1_score: score_res.part1_score,
            part2_score: score_res.part2_score,
            part3_score: score_res.part3_score,
        }),
    )
        .into_response()
}

pub(crate) fn apply_variant_mapping(
    questions: Vec<Question>,
    mappings: &[VariantQuestionMapping],
) -> Vec<Question> {
    if mappings.is_empty() {
        return questions;
    }

    let mut q_by_id: HashMap<Uuid, Question> = questions.into_iter().map(|q| (q.id, q)).collect();
    let mut result = Vec::with_capacity(q_by_id.len());
    let mut p1_idx = 0u32;
    let mut p2_idx = 0u32;
    let mut p3_idx = 0u32;
    let std_labels = ["A", "B", "C", "D", "E", "F"];

    for m in mappings {
        if let Some(mut q) = q_by_id.remove(&m.question_id) {
            q.order_index = match q.part_type {
                PartType::Part1 => {
                    p1_idx += 1;
                    p1_idx
                }
                PartType::Part2 => {
                    p2_idx += 1;
                    p2_idx
                }
                PartType::Part3 => {
                    p3_idx += 1;
                    p3_idx
                }
            };

            if q.part_type == PartType::Part1 {
                if let Some(ref json) = m.options_order_json {
                    if let Ok(order) = serde_json::from_str::<Vec<String>>(json) {
                        let orig_opts = std::mem::take(&mut q.options);
                        let mut reordered = Vec::with_capacity(orig_opts.len());
                        let mut used = vec![false; orig_opts.len()];

                        for (idx, lbl) in order.iter().enumerate() {
                            if let Some((pos, found)) = orig_opts
                                .iter()
                                .enumerate()
                                .find(|(p, o)| !used[*p] && o.label.eq_ignore_ascii_case(lbl))
                            {
                                used[pos] = true;
                                let mut opt_clone = found.clone();
                                opt_clone.label = if idx < std_labels.len() {
                                    std_labels[idx].to_string()
                                } else {
                                    format!("{}", (b'A' + idx as u8) as char)
                                };
                                reordered.push(opt_clone);
                            }
                        }
                        for (pos, left) in orig_opts.into_iter().enumerate() {
                            if !used[pos] {
                                reordered.push(left);
                            }
                        }
                        q.options = reordered;
                    }
                }
            }

            result.push(q);
        }
    }

    // Gắn bất kỳ câu hỏi mới/dư nào nếu chưa có trong mapping
    let mut leftovers: Vec<Question> = q_by_id.into_values().collect();
    leftovers.sort_by_key(|q| (
        match q.part_type {
            PartType::Part1 => 1,
            PartType::Part2 => 2,
            PartType::Part3 => 3,
        },
        q.order_index,
    ));
    for mut q in leftovers {
        q.order_index = match q.part_type {
            PartType::Part1 => {
                p1_idx += 1;
                p1_idx
            }
            PartType::Part2 => {
                p2_idx += 1;
                p2_idx
            }
            PartType::Part3 => {
                p3_idx += 1;
                p3_idx
            }
        };
        result.push(q);
    }

    result
}

pub(crate) fn unmap_variant_option(selected_label: &str, options_order_json: Option<&str>) -> String {
    let trimmed = selected_label.trim();
    if let Some(json) = options_order_json {
        if let Ok(order) = serde_json::from_str::<Vec<String>>(json) {
            if let Some(ch) = trimmed.chars().next() {
                let upper = ch.to_ascii_uppercase();
                if upper.is_ascii_uppercase() {
                    let idx = (upper as u8 - b'A') as usize;
                    if idx < order.len() {
                        return order[idx].clone();
                    }
                }
            }
        }
    }
    trimmed.to_string()
}
