//! # Teacher & Admin Handlers
//!
//! Các endpoint quản trị phòng thi, điều khiển bắt đầu và xuất báo cáo điểm Excel.

use crate::handlers::auth::{ApiErrorDetail, ApiErrorResponse};
use crate::network::{get_local_ip, CloudflareTunnel};
use crate::state::{AppState, ServerEvent};
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use examplus_core::db::DbRepository;
use examplus_core::engine::scoring::{
    calculate_part2_score, calculate_total_score, normalize_number_string, StudentSubmission,
};
use examplus_core::export::{
    export_exam_results, StudentGradeRecord, ViolationExportRecord,
};
use examplus_core::models::{
    Exam, ExamStatus, ExamVariant, ExamineeStatus, NetworkMode, PartType, Question,
    QuestionOption, ViolationType,
};
use examplus_core::parser::answer_key::TrueFalseAnswer;
use examplus_core::parser::docx::ParseError;
use examplus_core::parser::{
    parse_answer_key_bytes, parse_exam_bytes, ExamFileFormat,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct TeacherStatusResponse {
    pub exam_id: Option<String>,
    pub network_mode: String,
    pub local_ip: String,
    pub public_url: Option<String>,
    pub total_examinees: usize,
    pub submitted_count: usize,
    pub violation_count: usize,
    pub exam_title: Option<String>,
    pub exam_status: Option<String>,
    pub duration_minutes: Option<u32>,
    pub started_at: Option<String>,
    pub end_at: Option<String>,
    pub remaining_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TeacherStatusQuery {
    pub exam_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadDocxQuery {
    pub title: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadDocxResponse {
    pub success: bool,
    pub exam_id: String,
    pub title: String,
    pub subject: String,
    pub duration_minutes: u32,
    #[serde(default)]
    pub source_format: String,
    pub total_questions: usize,
    pub part1_count: usize,
    pub part2_count: usize,
    pub part3_count: usize,
    pub questions: Vec<PreviewQuestion>,
}

#[derive(Debug, Serialize)]
pub struct PreviewQuestion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub number: u32,
    pub part: String,
    pub content_html: String,
    pub options: Vec<PreviewOption>,
    pub short_answer: Option<String>,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_json: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PreviewOption {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub label: String,
    pub content_html: String,
    pub is_correct: bool,
}

#[derive(Debug, Deserialize)]
pub struct GenerateVariantsRequest {
    pub count: Option<usize>,
    pub variant_count: Option<usize>,
    pub exam_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateVariantsResponse {
    pub success: bool,
    pub exam_id: String,
    pub exam_title: String,
    pub total_questions: usize,
    pub count: usize,
    pub variants: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleNetworkRequest {
    pub mode: Option<String>,
    pub network_mode: Option<String>,
    pub force_refresh: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RoomActionRequest {
    pub exam_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExamPreviewQuery {
    pub exam_id: Option<Uuid>,
    pub variant_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenExamRoomRequest {
    pub exam_id: Option<String>,
    pub room_name: Option<String>,
    pub network_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteExamRequest {
    pub exam_id: Option<String>,
    pub variant_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ToggleNetworkResponse {
    pub success: bool,
    pub mode: String,
    pub network_mode: String,
    pub local_ip: String,
    pub public_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnswerKeyRequest {
    pub exam_id: Option<Uuid>,
    pub updates: Vec<QuestionAnswerUpdateItem>,
}

#[derive(Debug, Deserialize)]
pub struct QuestionAnswerUpdateItem {
    pub question_id: Uuid,
    #[serde(default)]
    pub correct_options: Vec<String>, // ["A"] cho Phần 1; ["a", "c"] cho các ý Đúng ở Phần 2
    pub short_answer: Option<String>, // Chuỗi cho Phần 3
    pub content_html: Option<String>,
    pub raw_text: Option<String>,
    pub option_updates: Option<Vec<OptionUpdateItem>>,
    pub clear_needs_review: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct OptionUpdateItem {
    pub option_id: Option<Uuid>,
    pub label: Option<String>,
    pub content_html: String,
    pub is_correct: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ImportAnswerKeyQuery {
    pub exam_id: Option<Uuid>,
    pub filename: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateAnswerKeyResponse {
    pub success: bool,
    pub updated_questions: usize,
    pub regraded_submissions: usize,
}

#[derive(Debug, Serialize)]
pub struct StartExamResponse {
    pub success: bool,
    pub started_at: String,
    pub end_at: String,
    pub duration_minutes: u32,
    pub remaining_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct AddTimeRequest {
    pub minutes: i64,
    pub exam_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExamineeAuditResponse {
    pub success: bool,
    pub student: ExamineeAuditStudentInfo,
    pub submission: Option<ExamineeAuditSubmissionInfo>,
    pub violations: Vec<ExamineeAuditViolationItem>,
    pub answers: Vec<ExamineeAuditAnswerItem>,
}

#[derive(Debug, Serialize)]
pub struct ExamineeAuditStudentInfo {
    pub id: Uuid,
    pub student_code: String,
    pub full_name: String,
    pub class_name: String,
    pub status: String,
    pub variant_code: Option<String>,
    pub ip_address: Option<String>,
    pub joined_at: String,
}

#[derive(Debug, Serialize)]
pub struct ExamineeAuditSubmissionInfo {
    pub started_at: Option<String>,
    pub submitted_at: Option<String>,
    pub total_score: f64,
    pub part1_score: f64,
    pub part2_score: f64,
    pub part3_score: f64,
    pub is_graded: bool,
}

#[derive(Debug, Serialize)]
pub struct ExamineeAuditViolationItem {
    pub id: Uuid,
    pub violation_type: String,
    pub duration_seconds: u32,
    pub details: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ExamineeAuditAnswerItem {
    pub question_id: Uuid,
    pub question_number: u32,
    pub part_type: String,
    pub content_html: String,
    pub selected_option: Option<String>,
    pub sub_answers_json: Option<String>,
    pub short_answer_text: Option<String>,
    pub awarded_score: f64,
    pub is_correct: bool,
}

pub async fn get_teacher_status(
    State(state): State<AppState>,
    Query(query): Query<TeacherStatusQuery>,
) -> impl IntoResponse {
    let local_ip = get_local_ip();
    let exam_id = if let Some(id_str) = query.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Uuid::parse_str(id_str).ok()
    } else {
        let guard = state.current_exam_id.read().await;
        *guard
    };

    let exam_id = match exam_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::OK,
                Json(TeacherStatusResponse {
                    exam_id: None,
                    network_mode: "OFFLINE_LAN".to_string(),
                    local_ip,
                    public_url: None,
                    total_examinees: 0,
                    submitted_count: 0,
                    violation_count: 0,
                    exam_title: None,
                    exam_status: None,
                    duration_minutes: None,
                    started_at: None,
                    end_at: None,
                    remaining_seconds: None,
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let exam = repo.get_exam(exam_id).ok().flatten();
    let examinees = repo.list_examinees_by_exam(exam_id).unwrap_or_default();
    let violations = repo.list_violations_by_exam(exam_id).unwrap_or_default();

    let total_examinees = examinees.len();
    let submitted_count = examinees
        .iter()
        .filter(|e| e.status == ExamineeStatus::Submitted)
        .count();
    let violation_count = violations.len();

    let network_mode_str = match exam.as_ref().map(|e| e.network_mode) {
        Some(NetworkMode::OnlineTunnel) => "ONLINE_TUNNEL",
        _ => "OFFLINE_LAN",
    };
    let public_url = exam.as_ref().and_then(|e| e.public_url.clone());
    let exam_title = exam.as_ref().map(|e| e.title.clone());
    let exam_status = exam.as_ref().map(|e| match e.status {
        ExamStatus::Ready => "READY",
        ExamStatus::InProgress => "IN_PROGRESS",
        ExamStatus::Finished => "FINISHED",
        _ => "DRAFT",
    }.to_string());
    let duration_minutes = exam.as_ref().map(|e| e.duration_minutes);
    let started_at = exam.as_ref().and_then(|e| e.started_at.map(|dt| dt.to_rfc3339()));
    let end_at = exam.as_ref().and_then(|e| e.end_at.map(|dt| dt.to_rfc3339()));
    let remaining_seconds = exam.as_ref().and_then(|e| {
        e.end_at.map(|end| {
            (end - Utc::now()).num_seconds().max(0) as u64
        })
    });

    (
        StatusCode::OK,
        Json(TeacherStatusResponse {
            exam_id: Some(exam_id.to_string()),
            network_mode: network_mode_str.to_string(),
            local_ip,
            public_url,
            total_examinees,
            submitted_count,
            violation_count,
            exam_title,
            exam_status,
            duration_minutes,
            started_at,
            end_at,
            remaining_seconds,
        }),
    )
        .into_response()
}

pub async fn start_exam(
    State(state): State<AppState>,
    payload: Option<Json<RoomActionRequest>>,
) -> impl IntoResponse {
    let exam_id = if let Some(Json(p)) = payload {
        p.exam_id
            .as_deref()
            .and_then(|id_str| Uuid::parse_str(id_str).ok())
    } else {
        None
    };

    let target_id = if let Some(id) = exam_id {
        id
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "NO_ACTIVE_EXAM".to_string(),
                            message: "Không có kỳ thi nào đang mở".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    let repo = state.repo.lock().await;
    let exam = repo.get_exam(target_id).ok().flatten();

    // Bảo vệ idempotent: nếu kỳ thi đã đang diễn ra và THỰC SỰ chưa hết giờ, giữ nguyên thời gian đếm ngược
    if let Some(ref ex) = exam {
        if ex.status == ExamStatus::InProgress && ex.started_at.is_some() {
            let started_dt = ex.started_at.unwrap();
            let end_dt = ex
                .end_at
                .unwrap_or_else(|| started_dt + chrono::Duration::minutes(ex.duration_minutes as i64));
            if Utc::now() < end_dt {
                let remaining = (end_dt - Utc::now()).num_seconds().max(0) as u64;
                return (
                    StatusCode::OK,
                    Json(StartExamResponse {
                        success: true,
                        started_at: started_dt.to_rfc3339(),
                        end_at: end_dt.to_rfc3339(),
                        duration_minutes: ex.duration_minutes,
                        remaining_seconds: remaining,
                    }),
                )
                    .into_response();
            }
        }
    }

    if let Err(e) = repo.update_exam_status(target_id, ExamStatus::InProgress) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "DB_ERROR".to_string(),
                    message: format!("Không thể cập nhật trạng thái đề thi: {}", e),
                },
            }),
        )
            .into_response();
    }

    let exam = repo.get_exam(target_id).ok().flatten();
    let duration = exam.as_ref().map(|e| e.duration_minutes).unwrap_or(45);
    let now = Utc::now();
    let end_at = now + chrono::Duration::minutes(duration as i64);

    if let Err(e) = repo.update_exam_start_time(target_id, now, end_at) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "DB_ERROR".to_string(),
                    message: format!("Không thể cập nhật thời gian bắt đầu đề thi: {}", e),
                },
            }),
        )
            .into_response();
    }

    // Phát lệnh bắt đầu tới toàn bộ thí sinh của phòng thi
    let _ = state.event_sender.send(ServerEvent::ExamStarted {
        started_at: now.to_rfc3339(),
        end_at: end_at.to_rfc3339(),
        duration_minutes: duration,
        exam_id: Some(target_id.to_string()),
    });

    (
        StatusCode::OK,
        Json(StartExamResponse {
            success: true,
            started_at: now.to_rfc3339(),
            end_at: end_at.to_rfc3339(),
            duration_minutes: duration,
            remaining_seconds: (duration as u64) * 60,
        }),
    )
        .into_response()
}

pub async fn add_exam_time(
    State(state): State<AppState>,
    Json(payload): Json<AddTimeRequest>,
) -> impl IntoResponse {
    if payload.minutes <= 0 || payload.minutes > 180 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "INVALID_MINUTES".to_string(),
                    message: "Số phút gia hạn phải trong khoảng 1 đến 180.".to_string(),
                },
            }),
        )
            .into_response();
    }

    let exam_id = if let Some(id_str) = payload.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Uuid::parse_str(id_str).ok()
    } else {
        let guard = state.current_exam_id.read().await;
        *guard
    };

    let target_id = match exam_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "NO_ACTIVE_EXAM".to_string(),
                        message: "Không có phòng thi nào đang mở".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let exam = match repo.get_exam(target_id).ok().flatten() {
        Some(ex) => ex,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Không tìm thấy đề thi trong CSDL".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    if exam.status != ExamStatus::InProgress {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EXAM_NOT_STARTED".to_string(),
                    message: "Vui lòng nhấn 'Bắt Đầu Tính Giờ' trước khi cộng thêm giờ.".to_string(),
                },
            }),
        )
            .into_response();
    }

    match repo.extend_exam_time(target_id, payload.minutes) {
        Ok(Some((new_duration, started_at, new_end))) => {
            let remaining = (new_end - Utc::now()).num_seconds().max(0) as u64;
            let _ = state.event_sender.send(ServerEvent::ExamStarted {
                started_at: started_at.to_rfc3339(),
                end_at: new_end.to_rfc3339(),
                duration_minutes: new_duration,
                exam_id: Some(target_id.to_string()),
            });
            (
                StatusCode::OK,
                Json(StartExamResponse {
                    success: true,
                    started_at: started_at.to_rfc3339(),
                    end_at: new_end.to_rfc3339(),
                    duration_minutes: new_duration,
                    remaining_seconds: remaining,
                }),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EXAM_NOT_FOUND".to_string(),
                    message: "Không tìm thấy đề thi".to_string(),
                },
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "DB_ERROR".to_string(),
                    message: format!("Không thể gia hạn thời gian thi: {}", e),
                },
            }),
        )
            .into_response(),
    }
}

pub async fn export_excel(
    State(state): State<AppState>,
    Query(query): Query<TeacherStatusQuery>,
) -> impl IntoResponse {
    let exam_id = if let Some(id_str) = query.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Uuid::parse_str(id_str).ok()
    } else {
        let guard = state.current_exam_id.read().await;
        *guard
    };

    let exam_id = match exam_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "NO_ACTIVE_EXAM".to_string(),
                        message: "Không tìm thấy kỳ thi để xuất điểm".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    let exam = match repo.get_exam(exam_id) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Kỳ thi không tồn tại".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let examinee_submissions = repo.list_submissions_by_exam(exam_id).unwrap_or_default();
    let violations_raw = repo.list_violations_by_exam(exam_id).unwrap_or_default();

    let variants = repo.get_variants_by_exam(exam_id).unwrap_or_default();
    let var_map: std::collections::HashMap<uuid::Uuid, String> =
        variants.into_iter().map(|v| (v.id, v.code)).collect();

    // Đếm số lần vi phạm cho từng thí sinh
    let mut viol_count_map: std::collections::HashMap<uuid::Uuid, usize> = std::collections::HashMap::new();
    for (_ex, v) in &violations_raw {
        *viol_count_map.entry(v.examinee_id).or_insert(0) += 1;
    }

    let grades: Vec<StudentGradeRecord> = examinee_submissions
        .into_iter()
        .map(|(ex, sub)| {
            let vcount = viol_count_map.get(&ex.id).copied().unwrap_or(0);
            let var_code = ex
                .assigned_variant_id
                .and_then(|vid| var_map.get(&vid).cloned())
                .unwrap_or_else(|| "101".to_string());
            if let Some(s) = sub {
                StudentGradeRecord {
                    student_code: ex.student_code,
                    full_name: ex.full_name,
                    class_name: ex.class_name,
                    variant_code: var_code,
                    part1_score: s.part1_score,
                    part2_score: s.part2_score,
                    part3_score: s.part3_score,
                    total_score: s.total_score,
                    submitted_at: s.submitted_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
                    violation_count: vcount,
                }
            } else {
                StudentGradeRecord {
                    student_code: ex.student_code,
                    full_name: ex.full_name,
                    class_name: ex.class_name,
                    variant_code: var_code,
                    part1_score: 0.0,
                    part2_score: 0.0,
                    part3_score: 0.0,
                    total_score: 0.0,
                    submitted_at: None,
                    violation_count: vcount,
                }
            }
        })
        .collect();

    let violations: Vec<ViolationExportRecord> = violations_raw
        .into_iter()
        .map(|(ex, v)| {
            let vtype_str = match v.violation_type {
                ViolationType::TabSwitch => "Chuyển Tab",
                ViolationType::Blur => "Mất Tiêu Điểm",
                ViolationType::FullscreenExit => "Thoát Toàn Màn Hình",
                ViolationType::CopyAttempt => "Cố Ý Sao Chép",
                ViolationType::PrintScreen => "Chụp Màn Hình",
            };
            ViolationExportRecord {
                student_code: ex.student_code,
                full_name: ex.full_name,
                class_name: ex.class_name,
                violation_type: vtype_str.to_string(),
                duration_seconds: v.duration_seconds,
                details: v.details.unwrap_or_default(),
                timestamp: v.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            }
        })
        .collect();

    drop(repo);

    // Tạo file tạm để ghi Excel
    let tmp = match NamedTempFile::new() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "IO_ERROR".to_string(),
                        message: format!("Không thể tạo tệp tạm: {}", e),
                    },
                }),
            )
                .into_response();
        }
    };

    let tmp_path = tmp.path().with_extension("xlsx");
    if let Err(e) = export_exam_results(&tmp_path, &exam.title, &exam.subject, &grades, &violations) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EXCEL_ERROR".to_string(),
                    message: format!("Lỗi xuất bảng điểm: {}", e),
                },
            }),
        )
            .into_response();
    }

    let bytes = match std::fs::read(&tmp_path) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "IO_ERROR".to_string(),
                        message: format!("Không thể đọc tệp Excel đã xuất: {}", e),
                    },
                }),
            )
                .into_response();
        }
    };
    let _ = std::fs::remove_file(tmp_path);

    // Tự động sao lưu bảng điểm Excel vào thư mục Google Drive (BangDiem_Excel)
    crate::handlers::drive::mirror_xlsx_to_drive(&exam.title, &bytes);

    let mut res = Response::new(Body::from(bytes));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    res.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"bang_diem_examplus.xlsx\""),
    );

    res
}

/// Nạp đề thi (.docx hoặc .pdf), bóc tách công thức/hình ảnh & đáp án tự động,
/// lưu trữ vào CSDL SQLite và trả về danh sách câu hỏi xem trước trực tiếp trên giao diện.
pub async fn upload_docx(
    State(state): State<AppState>,
    Query(query): Query<UploadDocxQuery>,
    body: Bytes,
) -> impl IntoResponse {
    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EMPTY_FILE".to_string(),
                    message: "Tệp tải lên rỗng".to_string(),
                },
            }),
        )
            .into_response();
    }

    let filename_hint = query.filename.as_deref();
    let (detected_format, parsed) = match parse_exam_bytes(&body, filename_hint) {
        Ok(res) => res,
        Err(ParseError::UnsupportedFormat(msg)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "UNSUPPORTED_FORMAT".to_string(),
                        message: msg,
                    },
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "PARSE_ERROR".to_string(),
                        message: format!("Lỗi phân tích cú pháp tệp đề thi: {}", e),
                    },
                }),
            )
                .into_response();
        }
    };

    let answer_keys_map = parse_answer_key_bytes(&body, filename_hint, None).ok();
    let primary_key = answer_keys_map.as_ref().and_then(|keys| {
        keys.get("101")
            .or_else(|| parsed.variant_code.as_deref().and_then(|c| keys.get(c)))
            .or_else(|| keys.values().next())
    });

    let exam_id = Uuid::new_v4();
    let title = query
        .title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            if parsed.title.is_empty() {
                "Đề Thi Khảo Sát GDPT 2018".to_string()
            } else {
                parsed.title.clone()
            }
        });
    let subject = parsed.subject.clone().unwrap_or_else(|| "Toán Học".to_string());
    let duration = parsed.duration_minutes.unwrap_or(50);

    // Tự động sao lưu tệp đề thi (.docx -> DeThi_Word, .pdf -> DeThi_PDF)
    crate::handlers::drive::mirror_exam_to_drive(
        query.filename.as_deref().unwrap_or(&title),
        &body,
        detected_format,
    );
    let source_format = match detected_format {
        ExamFileFormat::Pdf => "pdf".to_string(),
        ExamFileFormat::Docx => "docx".to_string(),
    };

    let exam = Exam {
        id: exam_id,
        title: title.clone(),
        subject: subject.clone(),
        duration_minutes: duration,
        exam_type: parsed.exam_type,
        status: ExamStatus::Ready,
        network_mode: NetworkMode::OfflineLan,
        public_url: None,
        created_at: Utc::now(),
        started_at: None,
        end_at: None,
        source_format: Some(source_format.clone()),
    };

    let repo = state.repo.lock().await;
    if let Err(e) = repo.create_exam(&exam) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "DB_ERROR".to_string(),
                    message: format!("Lỗi lưu kỳ thi vào CSDL: {}", e),
                },
            }),
        )
            .into_response();
    }

    // Tạo mã đề gốc 101
    let original_variant = ExamVariant {
        id: Uuid::new_v4(),
        exam_id,
        code: "101".to_string(),
        is_original: true,
    };
    let _ = repo.create_variant(&original_variant);

    let mut preview_questions = Vec::new();

    // 1. Lưu câu hỏi Phần I
    for q in &parsed.part1_questions {
        let q_id = Uuid::new_v4();
        let correct_label = primary_key.and_then(|k| k.part1_answers.get(&q.number)).cloned();
        let options: Vec<QuestionOption> = q
            .options
            .iter()
            .map(|opt| {
                let is_correct = if let Some(ref cl) = correct_label {
                    opt.label.trim().eq_ignore_ascii_case(cl.trim()) || opt.is_correct.unwrap_or(false)
                } else {
                    opt.is_correct.unwrap_or(false)
                };
                QuestionOption {
                    id: Uuid::new_v4(),
                    question_id: q_id,
                    label: opt.label.clone(),
                    content_html: opt.html_content.clone(),
                    is_correct,
                }
            })
            .collect();

        let prov_json = serde_json::to_string(&q.provenance).ok();
        let question = Question {
            id: q_id,
            exam_id,
            part_type: PartType::Part1,
            order_index: q.number,
            content_html: q.html_content.clone(),
            raw_text: Some(q.raw_prompt.clone()),
            image_url: None,
            short_answer_solution: None,
            provenance_json: prov_json.clone(),
            answer_meta_json: None,
            needs_review: q.needs_review,
            options: options.clone(),
        };
        let _ = repo.create_question(&question);

        preview_questions.push(PreviewQuestion {
            id: Some(q_id),
            number: q.number,
            part: "PART_1".to_string(),
            content_html: q.html_content.clone(),
            options: options
                .into_iter()
                .map(|o| PreviewOption {
                    id: Some(o.id),
                    label: o.label,
                    content_html: o.content_html,
                    is_correct: o.is_correct,
                })
                .collect(),
            short_answer: None,
            needs_review: q.needs_review,
            provenance_json: prov_json,
        });
    }

    // 2. Lưu câu hỏi Phần II
    for q in &parsed.part2_questions {
        let q_id = Uuid::new_v4();
        let tf_ans = primary_key.and_then(|k| k.part2_answers.get(&q.number));
        let options: Vec<QuestionOption> = q
            .options
            .iter()
            .map(|opt| {
                let is_correct = if let Some(tf) = tf_ans {
                    match opt.label.trim().to_lowercase().as_str() {
                        "a" => tf.a,
                        "b" => tf.b,
                        "c" => tf.c,
                        "d" => tf.d,
                        _ => opt.is_correct.unwrap_or(false),
                    }
                } else {
                    opt.is_correct.unwrap_or(false)
                };
                QuestionOption {
                    id: Uuid::new_v4(),
                    question_id: q_id,
                    label: opt.label.clone(),
                    content_html: opt.html_content.clone(),
                    is_correct,
                }
            })
            .collect();

        let prov_json = serde_json::to_string(&q.provenance).ok();
        let question = Question {
            id: q_id,
            exam_id,
            part_type: PartType::Part2,
            order_index: q.number,
            content_html: q.html_content.clone(),
            raw_text: Some(q.raw_prompt.clone()),
            image_url: None,
            short_answer_solution: None,
            provenance_json: prov_json.clone(),
            answer_meta_json: None,
            needs_review: q.needs_review,
            options: options.clone(),
        };
        let _ = repo.create_question(&question);

        preview_questions.push(PreviewQuestion {
            id: Some(q_id),
            number: q.number,
            part: "PART_2".to_string(),
            content_html: q.html_content.clone(),
            options: options
                .into_iter()
                .map(|o| PreviewOption {
                    id: Some(o.id),
                    label: o.label,
                    content_html: o.content_html,
                    is_correct: o.is_correct,
                })
                .collect(),
            short_answer: None,
            needs_review: q.needs_review,
            provenance_json: prov_json,
        });
    }

    // 3. Lưu câu hỏi Phần III
    for q in &parsed.part3_questions {
        let q_id = Uuid::new_v4();
        let short_ans = primary_key.and_then(|k| k.part3_answers.get(&q.number)).cloned();
        let short_answer_solution = short_ans.or_else(|| q.short_answer.clone());

        let prov_json = serde_json::to_string(&q.provenance).ok();
        let question = Question {
            id: q_id,
            exam_id,
            part_type: PartType::Part3,
            order_index: q.number,
            content_html: q.html_content.clone(),
            raw_text: Some(q.raw_prompt.clone()),
            image_url: None,
            short_answer_solution: short_answer_solution.clone(),
            provenance_json: prov_json.clone(),
            answer_meta_json: None,
            needs_review: q.needs_review,
            options: Vec::new(),
        };
        let _ = repo.create_question(&question);

        preview_questions.push(PreviewQuestion {
            id: Some(q_id),
            number: q.number,
            part: "PART_3".to_string(),
            content_html: q.html_content.clone(),
            options: Vec::new(),
            short_answer: short_answer_solution,
            needs_review: q.needs_review,
            provenance_json: prov_json,
        });
    }

    drop(repo);

    // Cập nhật exam_id hiện tại của hệ thống
    {
        let mut guard = state.current_exam_id.write().await;
        *guard = Some(exam_id);
    }

    (
        StatusCode::OK,
        Json(UploadDocxResponse {
            success: true,
            exam_id: exam_id.to_string(),
            title,
            subject,
            duration_minutes: duration,
            source_format,
            total_questions: preview_questions.len(),
            part1_count: parsed.part1_questions.len(),
            part2_count: parsed.part2_questions.len(),
            part3_count: parsed.part3_questions.len(),
            questions: preview_questions,
        }),
    )
        .into_response()
}

fn populate_variant_mappings(
    repo: &examplus_core::db::DbRepository,
    exam_id: Uuid,
    variant_id: Uuid,
    variant_num: usize,
    questions: &[Question],
) {
    let mut rng_seed = (exam_id.as_u128() as u64).wrapping_add(variant_num as u64);
    let mut p1_qs: Vec<_> = questions
        .iter()
        .filter(|q| q.part_type == PartType::Part1)
        .cloned()
        .collect();
    let mut p2_qs: Vec<_> = questions
        .iter()
        .filter(|q| q.part_type == PartType::Part2)
        .cloned()
        .collect();
    let mut p3_qs: Vec<_> = questions
        .iter()
        .filter(|q| q.part_type == PartType::Part3)
        .cloned()
        .collect();

    let mut shuffle = |slice: &mut [Question]| {
        for j in (1..slice.len()).rev() {
            rng_seed = rng_seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let k = (rng_seed % ((j + 1) as u64)) as usize;
            slice.swap(j, k);
        }
    };

    shuffle(&mut p1_qs);
    shuffle(&mut p2_qs);
    shuffle(&mut p3_qs);

    let mut disp_idx = 1;
    for q in p1_qs.iter().chain(p2_qs.iter()).chain(p3_qs.iter()) {
        let options_json = if q.part_type == PartType::Part1 {
            let mut opt_labels: Vec<_> = q.options.iter().map(|o| o.label.clone()).collect();
            for j in (1..opt_labels.len()).rev() {
                rng_seed = rng_seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let k = (rng_seed % ((j + 1) as u64)) as usize;
                opt_labels.swap(j, k);
            }
            serde_json::to_string(&opt_labels).ok()
        } else {
            None
        };

        let _ = repo.save_variant_mapping(
            Uuid::new_v4(),
            variant_id,
            q.id,
            disp_idx,
            options_json.as_deref(),
        );
        disp_idx += 1;
    }
}

fn any_other_online_room_active(repo: &DbRepository, exclude_id: Option<Uuid>) -> bool {
    let exams = repo.list_exams().unwrap_or_default();
    exams.iter().any(|e| {
        Some(e.id) != exclude_id
            && (e.status == ExamStatus::Ready || e.status == ExamStatus::InProgress)
            && e.network_mode == NetworkMode::OnlineTunnel
    })
}

async fn apply_network_mode(
    state: &AppState,
    exam_id: Option<Uuid>,
    target_mode: &str,
    force_refresh: bool,
) -> Result<(String, Option<String>), String> {
    let normalized = target_mode.trim().to_uppercase();
    if normalized == "ONLINE_TUNNEL" || normalized == "TUNNEL" {
        let mut tunnel_guard = state.tunnel.lock().await;
        let is_running = tunnel_guard.as_mut().map(|t| t.is_alive()).unwrap_or(false);

        let public_url = if !force_refresh && is_running {
            // Tái sử dụng tunnel đang chạy ổn định để không làm gián đoạn các phòng thi trước đó (tránh lỗi Cloudflare 1033)
            tunnel_guard
                .as_ref()
                .and_then(|t| t.public_url().map(|s| s.to_string()))
        } else {
            // Chỉ dừng tiến trình cũ và khởi động tunnel mới khi chưa có tunnel hoặc khi force_refresh được yêu cầu
            if let Some(mut old_t) = tunnel_guard.take() {
                old_t.stop();
            }
            let mut tunnel = CloudflareTunnel::new();
            match tunnel.start(state.server_port, None) {
                Ok(url) => {
                    *tunnel_guard = Some(tunnel);
                    Some(url)
                }
                Err(e) => {
                    return Err(format!("Không thể khởi động Cloudflare Tunnel: {}", e));
                }
            }
        };

        if let Some(id) = exam_id {
            let repo = state.repo.lock().await;
            let _ = repo.update_exam_network(id, NetworkMode::OnlineTunnel, public_url.as_deref());
        }

        let _ = state.event_sender.send(ServerEvent::NetworkModeChanged {
            mode: "ONLINE_TUNNEL".to_string(),
            public_url: public_url.clone(),
        });

        Ok(("ONLINE_TUNNEL".to_string(), public_url))
    } else {
        if let Some(id) = exam_id {
            let repo = state.repo.lock().await;
            let _ = repo.update_exam_network(id, NetworkMode::OfflineLan, None);
        }

        // Chỉ dừng tunnel nếu KHÔNG CÒN bất kỳ phòng thi nào khác (READY hoặc IN_PROGRESS) đang dùng Online Tunnel!
        let has_other_online = {
            let repo = state.repo.lock().await;
            any_other_online_room_active(&repo, exam_id)
        };

        if !has_other_online {
            let mut tunnel_guard = state.tunnel.lock().await;
            if let Some(mut t) = tunnel_guard.take() {
                t.stop();
            }
            let _ = state.event_sender.send(ServerEvent::NetworkModeChanged {
                mode: "OFFLINE_LAN".to_string(),
                public_url: None,
            });
        }

        Ok(("OFFLINE_LAN".to_string(), None))
    }
}

/// Tạo danh sách mã đề xáo tự động (102, 103, 104...) cho kỳ thi được chọn hoặc hiện tại
pub async fn generate_variants(
    State(state): State<AppState>,
    Json(payload): Json<GenerateVariantsRequest>,
) -> impl IntoResponse {
    let exam_id = if let Some(raw_id) = payload.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        match Uuid::parse_str(raw_id) {
            Ok(id) => id,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "INVALID_EXAM_ID".to_string(),
                            message: "Mã đề thi không hợp lệ hoặc đề chưa được lưu vào CSDL.".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                drop(guard);
                let repo = state.repo.lock().await;
                match repo.list_exams().unwrap_or_default().into_iter().next() {
                    Some(first) => first.id,
                    None => {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(ApiErrorResponse {
                                success: false,
                                error: ApiErrorDetail {
                                    code: "NO_ACTIVE_EXAM".to_string(),
                                    message: "Chưa có kỳ thi nào được nạp hoặc chọn".to_string(),
                                },
                            }),
                        )
                            .into_response();
                    }
                }
            }
        }
    };

    let count = payload.count.or(payload.variant_count).unwrap_or(4).clamp(1, 24);
    let base_code = 101usize;

    let repo = state.repo.lock().await;
    let exam = match repo.get_exam(exam_id) {
        Ok(Some(ex)) => ex,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Không tìm thấy đề thi tương ứng trong CSDL".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let questions = repo.get_questions_by_exam(exam_id).unwrap_or_default();
    if questions.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EMPTY_EXAM".to_string(),
                    message: "Đề thi này chưa có câu hỏi trong CSDL".to_string(),
                },
            }),
        )
            .into_response();
    }

    let existing = repo.get_variants_by_exam(exam_id).unwrap_or_default();
    if !existing.iter().any(|v| v.code == "101") {
        let _ = repo.create_variant(&ExamVariant {
            id: Uuid::new_v4(),
            exam_id,
            code: "101".to_string(),
            is_original: true,
        });
    }

    let mut variant_codes = vec!["101".to_string()];
    for i in 1..count {
        let code_num = base_code + i;
        let code = code_num.to_string();
        if let Some(existing_v) = existing.iter().find(|v| v.code == code) {
            if repo.get_variant_mappings(existing_v.id).unwrap_or_default().is_empty() {
                populate_variant_mappings(&repo, exam_id, existing_v.id, code_num, &questions);
            }
        } else {
            let new_id = Uuid::new_v4();
            let variant = ExamVariant {
                id: new_id,
                exam_id,
                code: code.clone(),
                is_original: false,
            };
            let _ = repo.create_variant(&variant);
            populate_variant_mappings(&repo, exam_id, new_id, code_num, &questions);
        }
        variant_codes.push(code);
    }
    drop(repo);

    {
        let mut guard = state.current_exam_id.write().await;
        if guard.is_none() {
            *guard = Some(exam_id);
        }
    }

    (
        StatusCode::OK,
        Json(GenerateVariantsResponse {
            success: true,
            exam_id: exam_id.to_string(),
            exam_title: exam.title,
            total_questions: questions.len(),
            count: variant_codes.len(),
            variants: variant_codes,
        }),
    )
        .into_response()
}

/// Bật / Tắt chế độ mạng (Mạng LAN Cục bộ hoặc Cloudflare Quick Tunnel Online)
pub async fn toggle_network(
    State(state): State<AppState>,
    Json(payload): Json<ToggleNetworkRequest>,
) -> impl IntoResponse {
    let local_ip = get_local_ip();
    let mut exam_id = {
        let guard = state.current_exam_id.read().await;
        *guard
    };

    if exam_id.is_none() {
        let repo = state.repo.lock().await;
        if let Some(first_exam) = repo.list_exams().unwrap_or_default().into_iter().next() {
            exam_id = Some(first_exam.id);
        }
        drop(repo);
        if let Some(id) = exam_id {
            let mut guard = state.current_exam_id.write().await;
            *guard = Some(id);
        }
    }

    if let Some(id) = exam_id {
        let repo = state.repo.lock().await;
        if let Ok(Some(ex)) = repo.get_exam(id) {
            if ex.status == ExamStatus::Draft {
                let _ = repo.update_exam_status(id, ExamStatus::Ready);
            }
        }
    }

    let target_mode = payload
        .mode
        .as_deref()
        .or(payload.network_mode.as_deref())
        .unwrap_or("OFFLINE_LAN");

    let force_refresh = payload.force_refresh.unwrap_or(false);
    match apply_network_mode(&state, exam_id, target_mode, force_refresh).await {
        Ok((mode_str, public_url)) => (
            StatusCode::OK,
            Json(ToggleNetworkResponse {
                success: true,
                mode: mode_str.clone(),
                network_mode: mode_str,
                local_ip,
                public_url,
            }),
        )
            .into_response(),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "TUNNEL_ERROR".to_string(),
                    message: msg,
                },
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize)]
pub struct SavedExamItem {
    pub id: String,
    pub title: String,
    pub subject: String,
    pub duration_minutes: u32,
    pub total_questions: usize,
    #[serde(rename = "questionCount")]
    pub question_count: usize,
    pub variants_count: usize,
    pub variants: Vec<String>,
    pub created_at: String,
    #[serde(rename = "savedAt")]
    pub saved_at: String,
    pub timestamp: i64,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ListSavedExamsResponse {
    pub success: bool,
    pub exams: Vec<SavedExamItem>,
}

pub async fn list_saved_exams(State(state): State<AppState>) -> impl IntoResponse {
    let repo = state.repo.lock().await;
    let exams = repo.list_exams().unwrap_or_default();
    let mut list = Vec::new();
    for ex in exams {
        let qs = repo.get_questions_by_exam(ex.id).unwrap_or_default();
        let vars = repo.get_variants_by_exam(ex.id).unwrap_or_default();
        let var_codes: Vec<String> = vars.into_iter().map(|v| v.code).collect();
        let formatted_time = ex.created_at.format("%d/%m/%Y %H:%M").to_string();
        let ts = ex.created_at.timestamp_millis();
        list.push(SavedExamItem {
            id: ex.id.to_string(),
            title: ex.title,
            subject: ex.subject,
            duration_minutes: ex.duration_minutes,
            total_questions: qs.len(),
            question_count: qs.len(),
            variants_count: var_codes.len(),
            variants: var_codes,
            created_at: formatted_time.clone(),
            saved_at: formatted_time,
            timestamp: ts,
            status: match ex.status {
                ExamStatus::Ready => "READY".to_string(),
                ExamStatus::InProgress => "IN_PROGRESS".to_string(),
                ExamStatus::Finished => "FINISHED".to_string(),
                _ => "DRAFT".to_string(),
            },
        });
    }
    (
        StatusCode::OK,
        Json(ListSavedExamsResponse {
            success: true,
            exams: list,
        }),
    )
        .into_response()
}

pub async fn open_exam_room(
    State(state): State<AppState>,
    Json(payload): Json<OpenExamRoomRequest>,
) -> impl IntoResponse {
    let local_ip = get_local_ip();
    let exam_id = if let Some(raw_id) = payload.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        match Uuid::parse_str(raw_id) {
            Ok(id) => id,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "INVALID_EXAM_ID".to_string(),
                            message: "Mã đề thi không hợp lệ hoặc đề chưa được lưu vào CSDL.".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                drop(guard);
                let repo = state.repo.lock().await;
                match repo.list_exams().unwrap_or_default().into_iter().next() {
                    Some(first) => first.id,
                    None => {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(ApiErrorResponse {
                                success: false,
                                error: ApiErrorDetail {
                                    code: "NO_ACTIVE_EXAM".to_string(),
                                    message: "Chưa có đề thi nào để mở phòng thi.".to_string(),
                                },
                            }),
                        )
                            .into_response();
                    }
                }
            }
        }
    };

    let source_exam_id = exam_id;
    let (room_exam, room_title) = {
        let repo = state.repo.lock().await;
        let source_exam = match repo.get_exam(source_exam_id) {
            Ok(Some(ex)) => ex,
            _ => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "EXAM_NOT_FOUND".to_string(),
                            message: "Không tìm thấy đề thi trong CSDL.".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        };

        let variants = repo.get_variants_by_exam(source_exam_id).unwrap_or_default();
        if variants.is_empty() {
            let _ = repo.create_variant(&ExamVariant {
                id: Uuid::new_v4(),
                exam_id: source_exam_id,
                code: "101".to_string(),
                is_original: true,
            });
        }

        let full_title = if let Some(rn) = payload.room_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            format!("{} - {}", rn, source_exam.title)
        } else {
            source_exam.title.clone()
        };

        match repo.clone_exam_for_room(source_exam_id, &full_title) {
            Ok(cloned) => (cloned, full_title),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "CLONE_ERROR".to_string(),
                            message: format!("Không thể nhân bản đề thi cho phòng mới: {}", e),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    let room_exam_id = room_exam.id;
    let requested_mode = payload.network_mode.as_deref().unwrap_or("OFFLINE_LAN");
    // Khi mở phòng thi mới: Tái sử dụng tunnel đang chạy nếu còn sống (tránh gây lỗi Cloudflare Error 1033 cho phòng trước)
    match apply_network_mode(&state, Some(room_exam_id), requested_mode, false).await {
        Ok((mode_str, public_url)) => {
            {
                let mut guard = state.current_exam_id.write().await;
                *guard = Some(room_exam_id);
            }
            let room_local_url = format!("http://{}:{}/?room={}", local_ip, state.server_port, room_exam_id);
            let room_public_url = public_url.as_ref().map(|p| format!("{}/?room={}", p.trim_end_matches('/'), room_exam_id));
            let effective_url = room_public_url.clone().unwrap_or_else(|| room_local_url.clone());
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "room_id": room_exam_id.to_string(),
                    "exam_id": room_exam_id.to_string(),
                    "exam_title": room_title,
                    "room_name": payload.room_name,
                    "network_mode": mode_str,
                    "local_ip": local_ip,
                    "public_url": public_url,
                    "room_local_url": room_local_url,
                    "room_public_url": room_public_url,
                    "effective_url": effective_url,
                })),
            )
                .into_response()
        }
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "TUNNEL_ERROR".to_string(),
                    message: msg,
                },
            }),
        )
            .into_response(),
    }
}

pub async fn delete_saved_exam(
    State(state): State<AppState>,
    Json(payload): Json<DeleteExamRequest>,
) -> impl IntoResponse {
    let raw_id = match payload.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "MISSING_EXAM_ID".to_string(),
                        message: "Thiếu mã đề thi cần xóa.".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let exam_id = match Uuid::parse_str(raw_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_EXAM_ID".to_string(),
                        message: "Mã đề thi không hợp lệ.".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;
    if let Some(vcode) = payload.variant_code.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        match repo.delete_variant_by_code(exam_id, vcode) {
            Ok(true) => {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": true,
                        "deleted": "variant",
                        "exam_id": exam_id.to_string(),
                        "variant_code": vcode,
                    })),
                )
                    .into_response();
            }
            Ok(false) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "CANNOT_DELETE_VARIANT".to_string(),
                            message: "Không thể xóa mã đề gốc 101 hoặc mã đề đã phát cho thí sinh.".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "DB_ERROR".to_string(),
                            message: format!("Lỗi khi xóa mã đề: {}", e),
                        },
                    }),
                )
                    .into_response();
            }
        }
    }

    match repo.delete_exam(exam_id) {
        Ok(_) => {
            let next_id = repo.list_exams().unwrap_or_default().into_iter().next().map(|e| e.id);
            drop(repo);
            let mut guard = state.current_exam_id.write().await;
            if *guard == Some(exam_id) {
                *guard = next_id;
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "deleted": "exam",
                    "exam_id": exam_id.to_string(),
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "DB_ERROR".to_string(),
                    message: format!("Không thể xóa đề thi: {}", e),
                },
            }),
        )
            .into_response(),
    }
}

pub async fn save_exam_to_bank(State(state): State<AppState>) -> impl IntoResponse {
    let exam_id = {
        let guard = state.current_exam_id.read().await;
        *guard
    };
    if let Some(id) = exam_id {
        let repo = state.repo.lock().await;
        if let Ok(Some(exam)) = repo.get_exam(id) {
            let qs = repo.get_questions_by_exam(id).unwrap_or_default();
            let vars = repo.get_variants_by_exam(id).unwrap_or_default();
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": "Đã lưu đề thi thành công vào CSDL",
                    "exam_id": id.to_string(),
                    "title": exam.title,
                    "questions_count": qs.len(),
                    "variants_count": vars.len(),
                })),
            )
                .into_response();
        }
    }
    (
        StatusCode::BAD_REQUEST,
        Json(ApiErrorResponse {
            success: false,
            error: ApiErrorDetail {
                code: "NO_ACTIVE_EXAM".to_string(),
                message: "Chưa có đề thi nào đang hoạt động để lưu".to_string(),
            },
        }),
    )
        .into_response()
}


/// Lấy toàn bộ bảng điểm thí sinh của kỳ thi hiện tại
/// Lấy thông tin chi tiết đề thi hiện tại cùng bộ câu hỏi và đáp án để xem trước và chỉnh sửa
pub async fn get_exam_preview(
    State(state): State<AppState>,
    Query(query): Query<ExamPreviewQuery>,
) -> impl IntoResponse {
    let exam_id = if let Some(id) = query.exam_id {
        id
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "NO_ACTIVE_EXAM".to_string(),
                            message: "Chưa có đề thi nào được nạp".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    let repo = state.repo.lock().await;
    let exam = match repo.get_exam(exam_id) {
        Ok(Some(e)) => e,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Kỳ thi không tồn tại".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let mut questions = repo.get_questions_by_exam(exam_id).unwrap_or_default();
    if let Some(ref vcode) = query.variant_code {
        let trimmed = vcode.trim();
        if !trimmed.is_empty() && trimmed != "101" {
            let variants = repo.get_variants_by_exam(exam_id).unwrap_or_default();
            if let Some(variant) = variants.iter().find(|v| v.code == trimmed) {
                let mappings = repo.get_variant_mappings(variant.id).unwrap_or_default();
                questions = crate::handlers::exam::apply_variant_mapping(questions, &mappings);
            }
        }
    }
    let mut part1_count = 0;
    let mut part2_count = 0;
    let mut part3_count = 0;
    let mut preview_questions = Vec::new();

    for q in questions {
        let part_str = match q.part_type {
            PartType::Part1 => {
                part1_count += 1;
                "PART_1"
            }
            PartType::Part2 => {
                part2_count += 1;
                "PART_2"
            }
            PartType::Part3 => {
                part3_count += 1;
                "PART_3"
            }
        };

        let options = q
            .options
            .into_iter()
            .map(|o| PreviewOption {
                id: Some(o.id),
                label: o.label,
                content_html: o.content_html,
                is_correct: o.is_correct,
            })
            .collect();

        preview_questions.push(PreviewQuestion {
            id: Some(q.id),
            number: q.order_index,
            part: part_str.to_string(),
            content_html: q.content_html,
            options,
            short_answer: q.short_answer_solution,
            needs_review: q.needs_review,
            provenance_json: q.provenance_json,
        });
    }

    (
        StatusCode::OK,
        Json(UploadDocxResponse {
            success: true,
            exam_id: exam.id.to_string(),
            title: exam.title,
            subject: exam.subject,
            duration_minutes: exam.duration_minutes,
            source_format: "docx".to_string(),
            total_questions: preview_questions.len(),
            part1_count,
            part2_count,
            part3_count,
            questions: preview_questions,
        }),
    )
        .into_response()
}

pub async fn get_grades(
    State(state): State<AppState>,
    Query(query): Query<TeacherStatusQuery>,
) -> impl IntoResponse {
    let exam_id = if let Some(id_str) = query.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Uuid::parse_str(id_str).ok()
    } else {
        let guard = state.current_exam_id.read().await;
        *guard
    };

    let exam_id = match exam_id {
        Some(id) => id,
        None => {
            return (StatusCode::OK, Json(Vec::<StudentGradeRecord>::new())).into_response();
        }
    };

    let repo = state.repo.lock().await;
    let examinee_submissions = repo.list_submissions_by_exam(exam_id).unwrap_or_default();
    let violations_raw = repo.list_violations_by_exam(exam_id).unwrap_or_default();
    let variants = repo.get_variants_by_exam(exam_id).unwrap_or_default();
    let var_map: std::collections::HashMap<Uuid, String> =
        variants.into_iter().map(|v| (v.id, v.code)).collect();

    let mut viol_count_map: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();
    for (_ex, v) in &violations_raw {
        *viol_count_map.entry(v.examinee_id).or_insert(0) += 1;
    }

    let grades: Vec<StudentGradeRecord> = examinee_submissions
        .into_iter()
        .map(|(ex, sub)| {
            let vcount = viol_count_map.get(&ex.id).copied().unwrap_or(0);
            let var_code = ex
                .assigned_variant_id
                .and_then(|vid| var_map.get(&vid).cloned())
                .unwrap_or_else(|| "101".to_string());
            if let Some(s) = sub {
                StudentGradeRecord {
                    student_code: ex.student_code,
                    full_name: ex.full_name,
                    class_name: ex.class_name,
                    variant_code: var_code,
                    part1_score: s.part1_score,
                    part2_score: s.part2_score,
                    part3_score: s.part3_score,
                    total_score: s.total_score,
                    submitted_at: s.submitted_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
                    violation_count: vcount,
                }
            } else {
                StudentGradeRecord {
                    student_code: ex.student_code,
                    full_name: ex.full_name,
                    class_name: ex.class_name,
                    variant_code: var_code,
                    part1_score: 0.0,
                    part2_score: 0.0,
                    part3_score: 0.0,
                    total_score: 0.0,
                    submitted_at: None,
                    violation_count: vcount,
                }
            }
        })
        .collect();

    (StatusCode::OK, Json(grades)).into_response()
}

/// Lấy thông tin chi tiết nhật ký vi phạm và bài làm của một thí sinh
pub async fn get_examinee_audit(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<TeacherStatusQuery>,
) -> impl IntoResponse {
    let student_code = code.trim();
    let exam_id_opt = if let Some(id_str) = query.exam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Uuid::parse_str(id_str).ok()
    } else {
        let guard = state.current_exam_id.read().await;
        *guard
    };

    let repo = state.repo.lock().await;

    // Tìm thí sinh theo exam_id và student_code, nếu không có exam_id hoặc không thấy thì tìm theo student_code gần nhất
    let examinee = if let Some(exam_id) = exam_id_opt {
        repo.get_examinee_by_code_and_exam(exam_id, student_code).ok().flatten()
    } else {
        None
    };

    let examinee = match examinee.or_else(|| repo.get_examinee_by_code(student_code).ok().flatten()) {
        Some(e) => e,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAMINEE_NOT_FOUND".to_string(),
                        message: format!("Không tìm thấy thí sinh với số báo danh: {}", student_code),
                    },
                }),
            )
                .into_response();
        }
    };

    // Lấy mã đề (variant_code)
    let variant_code = if let Some(vid) = examinee.assigned_variant_id {
        repo.get_variants_by_exam(examinee.exam_id)
            .ok()
            .and_then(|vars| vars.into_iter().find(|v| v.id == vid).map(|v| v.code))
    } else {
        Some("101".to_string())
    };

    // Lấy danh sách vi phạm của thí sinh theo thứ tự thời gian tăng dần
    let violations_raw = repo.get_violations_by_examinee(examinee.id).unwrap_or_default();
    let violations: Vec<ExamineeAuditViolationItem> = violations_raw
        .into_iter()
        .map(|v| {
            let vtype_str = match v.violation_type {
                ViolationType::TabSwitch => "TAB_SWITCH",
                ViolationType::Blur => "BLUR",
                ViolationType::FullscreenExit => "FULLSCREEN_EXIT",
                ViolationType::CopyAttempt => "COPY_ATTEMPT",
                ViolationType::PrintScreen => "PRINT_SCREEN",
            };
            ExamineeAuditViolationItem {
                id: v.id,
                violation_type: vtype_str.to_string(),
                duration_seconds: v.duration_seconds,
                details: v.details,
                timestamp: v.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            }
        })
        .collect();

    // Lấy bài thi và câu trả lời chi tiết nếu đã có submission
    let submission_opt = repo.get_submission_by_examinee(examinee.id).ok().flatten();
    let (submission_info, answers) = if let Some(sub) = submission_opt {
        let sub_info = ExamineeAuditSubmissionInfo {
            started_at: sub.started_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
            submitted_at: sub.submitted_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
            total_score: sub.total_score,
            part1_score: sub.part1_score,
            part2_score: sub.part2_score,
            part3_score: sub.part3_score,
            is_graded: sub.is_graded,
        };

        let sub_answers = repo.get_submission_answers(sub.id).unwrap_or_default();
        let questions = repo.get_questions_by_exam(examinee.exam_id).unwrap_or_default();
        let q_map: std::collections::HashMap<Uuid, &Question> =
            questions.iter().map(|q| (q.id, q)).collect();

        let mut ans_items = Vec::new();
        for ans in sub_answers {
            if let Some(q) = q_map.get(&ans.question_id) {
                let part_str = match q.part_type {
                    PartType::Part1 => "PART_1",
                    PartType::Part2 => "PART_2",
                    PartType::Part3 => "PART_3",
                };

                let display_opt = match q.part_type {
                    PartType::Part1 => ans.selected_option.clone(),
                    PartType::Part2 => {
                        if let Some(ref j) = ans.sub_answers_json {
                            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, bool>>(j) {
                                let mut keys: Vec<String> = map.keys().cloned().collect();
                                keys.sort();
                                let parts: Vec<String> = keys
                                    .iter()
                                    .map(|k| {
                                        let v = map.get(k).copied().unwrap_or(false);
                                        format!("{}:{}", k.to_uppercase(), if v { "Đ" } else { "S" })
                                    })
                                    .collect();
                                if parts.is_empty() {
                                    ans.selected_option.clone()
                                } else {
                                    Some(parts.join(" "))
                                }
                            } else {
                                ans.selected_option.clone()
                            }
                        } else {
                            ans.selected_option.clone()
                        }
                    }
                    PartType::Part3 => ans.short_answer_text.clone().or_else(|| ans.selected_option.clone()),
                };

                ans_items.push(ExamineeAuditAnswerItem {
                    question_id: ans.question_id,
                    question_number: q.order_index,
                    part_type: part_str.to_string(),
                    content_html: q.content_html.clone(),
                    selected_option: display_opt,
                    sub_answers_json: ans.sub_answers_json,
                    short_answer_text: ans.short_answer_text,
                    awarded_score: ans.awarded_score,
                    is_correct: ans.is_correct,
                });
            }
        }
        ans_items.sort_by_key(|a| (
            match a.part_type.as_str() {
                "PART_1" => 1,
                "PART_2" => 2,
                _ => 3,
            },
            a.question_number,
        ));

        (Some(sub_info), ans_items)
    } else {
        let questions = repo.get_questions_by_exam(examinee.exam_id).unwrap_or_default();
        let mut ans_items = Vec::new();
        for q in questions {
            let part_str = match q.part_type {
                PartType::Part1 => "PART_1",
                PartType::Part2 => "PART_2",
                PartType::Part3 => "PART_3",
            };
            ans_items.push(ExamineeAuditAnswerItem {
                question_id: q.id,
                question_number: q.order_index,
                part_type: part_str.to_string(),
                content_html: q.content_html.clone(),
                selected_option: None,
                sub_answers_json: None,
                short_answer_text: None,
                awarded_score: 0.0,
                is_correct: false,
            });
        }
        ans_items.sort_by_key(|a| (
            match a.part_type.as_str() {
                "PART_1" => 1,
                "PART_2" => 2,
                _ => 3,
            },
            a.question_number,
        ));
        (None, ans_items)
    };

    let status_str = match examinee.status {
        ExamineeStatus::Waiting => "WAITING",
        ExamineeStatus::Testing => "TESTING",
        ExamineeStatus::Submitted => "SUBMITTED",
        ExamineeStatus::Disconnected => "DISCONNECTED",
    };

    (
        StatusCode::OK,
        Json(ExamineeAuditResponse {
            success: true,
            student: ExamineeAuditStudentInfo {
                id: examinee.id,
                student_code: examinee.student_code,
                full_name: examinee.full_name,
                class_name: examinee.class_name,
                status: status_str.to_string(),
                variant_code,
                ip_address: examinee.ip_address,
                joined_at: examinee.joined_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            },
            submission: submission_info,
            violations,
            answers,
        }),
    )
        .into_response()
}

pub async fn update_answer_key(
    State(state): State<AppState>,
    Json(payload): Json<UpdateAnswerKeyRequest>,
) -> impl IntoResponse {
    let exam_id = if let Some(id) = payload.exam_id {
        id
    } else {
        let guard = state.current_exam_id.read().await;
        if let Some(id) = *guard {
            id
        } else if let Some(first_update) = payload.updates.first() {
            let repo = state.repo.lock().await;
            match repo.get_exam_id_by_question(first_update.question_id) {
                Ok(Some(id)) => id,
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiErrorResponse {
                            success: false,
                            error: ApiErrorDetail {
                                code: "EXAM_NOT_FOUND".to_string(),
                                message: "Không xác định được kỳ thi để cập nhật đáp án".to_string(),
                            },
                        }),
                    )
                        .into_response();
                }
            }
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "EXAM_NOT_FOUND".to_string(),
                        message: "Không xác định được kỳ thi để cập nhật đáp án".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let repo = state.repo.lock().await;

    // 1. Cập nhật tính đúng đắn & nội dung cho các câu hỏi
    let mut updated_questions_count = 0;
    for item in &payload.updates {
        if let Some(ref sa) = item.short_answer {
            let _ = repo.update_question_short_answer(item.question_id, Some(sa.trim()));
        }
        if !item.correct_options.is_empty() || item.short_answer.is_none() {
            let _ = repo.update_question_options_correctness(item.question_id, &item.correct_options);
        }
        if let Some(ref ch) = item.content_html {
            let _ = repo.update_question_content(
                item.question_id,
                ch,
                item.raw_text.as_deref(),
                item.clear_needs_review.unwrap_or(false),
            );
        } else if item.clear_needs_review.unwrap_or(false) {
            let _ = repo.update_question_needs_review(item.question_id, false);
        }
        if let Some(ref opt_updates) = item.option_updates {
            for opt_u in opt_updates {
                if let Some(opt_id) = opt_u.option_id {
                    let _ = repo.update_question_option_content(opt_id, &opt_u.content_html);
                }
            }
        }
        updated_questions_count += 1;
    }

    // 2. Lấy lại bộ câu hỏi và đáp án mới nhất của kỳ thi
    let updated_questions = repo.get_questions_by_exam(exam_id).unwrap_or_default();
    let q_by_id: std::collections::HashMap<Uuid, &Question> =
        updated_questions.iter().map(|q| (q.id, q)).collect();

    let mut part1_key: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut part2_key: std::collections::HashMap<u32, TrueFalseAnswer> =
        std::collections::HashMap::new();
    let mut part3_key: std::collections::HashMap<u32, String> = std::collections::HashMap::new();

    for q in &updated_questions {
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

    // 3. Chấm lại toàn bộ bài thi đã nộp (submissions) của kỳ thi
    let examinee_subs = repo.list_submissions_by_exam(exam_id).unwrap_or_default();
    let mut regraded_count = 0;

    for (_examinee, submission_opt) in examinee_subs {
        if let Some(sub) = submission_opt {
            let mut sub_answers = repo.get_submission_answers(sub.id).unwrap_or_default();

            let mut student_submission = StudentSubmission::default();
            for ans in &sub_answers {
                if let Some(q) = q_by_id.get(&ans.question_id) {
                    match q.part_type {
                        PartType::Part1 => {
                            if let Some(ref opt) = ans.selected_option {
                                student_submission.part1.insert(q.order_index, opt.clone());
                            }
                        }
                        PartType::Part2 => {
                            if let Some(ref json_str) = ans.sub_answers_json {
                                if let Ok(map) =
                                    serde_json::from_str::<std::collections::HashMap<String, bool>>(
                                        json_str,
                                    )
                                {
                                    let tf = TrueFalseAnswer {
                                        a: map.get("a").or_else(|| map.get("A")).copied().unwrap_or(false),
                                        b: map.get("b").or_else(|| map.get("B")).copied().unwrap_or(false),
                                        c: map.get("c").or_else(|| map.get("C")).copied().unwrap_or(false),
                                        d: map.get("d").or_else(|| map.get("D")).copied().unwrap_or(false),
                                    };
                                    student_submission.part2.insert(q.order_index, tf);
                                }
                            }
                        }
                        PartType::Part3 => {
                            if let Some(ref sa) = ans.short_answer_text {
                                student_submission.part3.insert(q.order_index, sa.clone());
                            }
                        }
                    }
                }
            }

            let score_res = calculate_total_score(
                &student_submission,
                &part1_key,
                &part2_key,
                &part3_key,
                0.25,
                0.5,
            );

            let _ = repo.update_submission_scores(
                sub.id,
                score_res.part1_score,
                score_res.part2_score,
                score_res.part3_score,
                score_res.total_score,
            );

            // Cập nhật lại awarded_score và is_correct cho từng câu trả lời trong submission_answers
            for ans in &mut sub_answers {
                if let Some(q) = q_by_id.get(&ans.question_id) {
                    match q.part_type {
                        PartType::Part1 => {
                            let is_corr = if let Some(ref opt) = ans.selected_option {
                                part1_key
                                    .get(&q.order_index)
                                    .map(|k| k.trim().eq_ignore_ascii_case(opt.trim()))
                                    .unwrap_or(false)
                            } else {
                                false
                            };
                            ans.is_correct = is_corr;
                            ans.awarded_score = if is_corr { 0.25 } else { 0.0 };
                        }
                        PartType::Part2 => {
                            let score = if let Some(student_tf) =
                                student_submission.part2.get(&q.order_index)
                            {
                                if let Some(std_tf) = part2_key.get(&q.order_index) {
                                    calculate_part2_score(student_tf, std_tf)
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            };
                            ans.awarded_score = score;
                            ans.is_correct = score == 1.0;
                        }
                        PartType::Part3 => {
                            let is_corr = if let Some(ref sa) = ans.short_answer_text {
                                if let Some(std) = part3_key.get(&q.order_index) {
                                    let ns = normalize_number_string(sa);
                                    let nk = normalize_number_string(std);
                                    if !ns.is_empty() {
                                        if ns.eq_ignore_ascii_case(&nk) {
                                            true
                                        } else if let (Ok(s_val), Ok(k_val)) =
                                            (ns.parse::<f64>(), nk.parse::<f64>())
                                        {
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
                            ans.is_correct = is_corr;
                            ans.awarded_score = if is_corr { 0.5 } else { 0.0 };
                        }
                    }
                }
            }
            let _ = repo.save_submission_answers(sub.id, &sub_answers);

            regraded_count += 1;
        }
    }

    // 4. Phát sự kiện WebSocket GradesUpdated
    let _ = state.event_sender.send(ServerEvent::GradesUpdated {
        exam_id: exam_id.to_string(),
    });

    (
        StatusCode::OK,
        Json(UpdateAnswerKeyResponse {
            success: true,
            updated_questions: updated_questions_count,
            regraded_submissions: regraded_count,
        }),
    )
        .into_response()
}


pub async fn import_answer_key(
    State(state): State<AppState>,
    Query(query): Query<ImportAnswerKeyQuery>,
    body: Bytes,
) -> impl IntoResponse {
    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                success: false,
                error: ApiErrorDetail {
                    code: "EMPTY_BODY".to_string(),
                    message: "Tệp đáp án tải lên rỗng".to_string(),
                },
            }),
        )
            .into_response();
    }
    let exam_id = if let Some(id) = query.exam_id {
        id
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "NO_ACTIVE_EXAM".to_string(),
                            message: "Chưa có kỳ thi nào để gán đáp án".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    let filename_hint = query.filename.as_deref();
    let keys_map = match parse_answer_key_bytes(&body, filename_hint, None) {
        Ok(km) if !km.is_empty() => km,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "NO_ANSWER_KEY_FOUND".to_string(),
                        message: "Không tìm thấy bảng đáp án hợp lệ trong tệp".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    let primary_key = keys_map
        .get("101")
        .or_else(|| keys_map.get("default"))
        .or_else(|| keys_map.values().next())
        .unwrap();

    let repo = state.repo.lock().await;
    let questions = repo.get_questions_by_exam(exam_id).unwrap_or_default();
    let mut updated_count = 0usize;

    for q in &questions {
        match q.part_type {
            PartType::Part1 => {
                if let Some(ans) = primary_key.part1_answers.get(&q.order_index) {
                    let _ = repo.update_question_options_correctness(q.id, &[ans.clone()]);
                    updated_count += 1;
                }
            }
            PartType::Part2 => {
                if let Some(tf) = primary_key.part2_answers.get(&q.order_index) {
                    let mut correct_labels = Vec::new();
                    if tf.a { correct_labels.push("a".to_string()); }
                    if tf.b { correct_labels.push("b".to_string()); }
                    if tf.c { correct_labels.push("c".to_string()); }
                    if tf.d { correct_labels.push("d".to_string()); }
                    let _ = repo.update_question_options_correctness(q.id, &correct_labels);
                    updated_count += 1;
                }
            }
            PartType::Part3 => {
                if let Some(num) = primary_key.part3_answers.get(&q.order_index) {
                    let _ = repo.update_question_short_answer(q.id, Some(num));
                    updated_count += 1;
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": format!("Đã cập nhật thành công {} câu hỏi từ tệp đáp án", updated_count),
            "updated_questions": updated_count,
            "exam_id": exam_id,
        })),
    )
        .into_response()
}

pub async fn stop_exam(
    State(state): State<AppState>,
    payload: Option<Json<RoomActionRequest>>,
) -> impl IntoResponse {
    let exam_id = if let Some(Json(p)) = payload {
        p.exam_id
            .as_deref()
            .and_then(|id_str| Uuid::parse_str(id_str).ok())
    } else {
        None
    };

    let target_id = if let Some(id) = exam_id {
        id
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "NO_ACTIVE_EXAM".to_string(),
                            message: "Chưa có kỳ thi nào đang mở để dừng".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    let repo = state.repo.lock().await;
    let _ = repo.update_exam_status(target_id, ExamStatus::Finished);

    let _ = state.event_sender.send(ServerEvent::ExamFinished {
        exam_id: target_id.to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Đã dừng ca thi thành công",
            "exam_id": target_id.to_string(),
            "status": "FINISHED"
        })),
    )
        .into_response()
}

pub async fn close_exam_room(
    State(state): State<AppState>,
    payload: Option<Json<RoomActionRequest>>,
) -> impl IntoResponse {
    let exam_id = if let Some(Json(p)) = payload {
        p.exam_id
            .as_deref()
            .and_then(|id_str| Uuid::parse_str(id_str).ok())
    } else {
        None
    };

    let target_id = if let Some(id) = exam_id {
        id
    } else {
        let guard = state.current_exam_id.read().await;
        match *guard {
            Some(id) => id,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse {
                        success: false,
                        error: ApiErrorDetail {
                            code: "NO_ACTIVE_EXAM".to_string(),
                            message: "Chưa có phòng thi nào đang mở".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        }
    };

    // 1. Cập nhật trạng thái đề thi trong CSDL về FINISHED nếu đang IN_PROGRESS
    {
        let repo = state.repo.lock().await;
        if let Ok(Some(exam)) = repo.get_exam(target_id) {
            if exam.status == ExamStatus::InProgress {
                let _ = repo.update_exam_status(target_id, ExamStatus::Finished);
            }
        }
    }

    // 2. Chỉ dừng Cloudflare Tunnel nếu KHÔNG CÒN phòng thi nào khác đang chạy ở chế độ Online Tunnel
    {
        let has_other_online = {
            let repo = state.repo.lock().await;
            any_other_online_room_active(&repo, Some(target_id))
        };
        if !has_other_online {
            let mut tunnel_guard = state.tunnel.lock().await;
            if let Some(mut t) = tunnel_guard.take() {
                t.stop();
            }
            let _ = state.event_sender.send(ServerEvent::NetworkModeChanged {
                mode: "OFFLINE_LAN".to_string(),
                public_url: None,
            });
        }
    }

    // 3. Nếu phòng đang đóng là current_exam_id thì đặt lại None
    {
        let mut curr_guard = state.current_exam_id.write().await;
        if *curr_guard == Some(target_id) {
            *curr_guard = None;
        }
    }

    // 4. Phát sự kiện WebSocket cho thí sinh và các client
    let _ = state.event_sender.send(ServerEvent::RoomClosed {
        exam_id: target_id.to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Đã đóng phòng thi thành công",
            "exam_id": target_id.to_string()
        })),
    )
        .into_response()
}

pub async fn delete_exam_room(
    State(state): State<AppState>,
    Json(payload): Json<RoomActionRequest>,
) -> impl IntoResponse {
    let exam_id_str = match payload.exam_id {
        Some(ref id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            let guard = state.current_exam_id.read().await;
            match *guard {
                Some(id) => id.to_string(),
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiErrorResponse {
                            success: false,
                            error: ApiErrorDetail {
                                code: "INVALID_EXAM_ID".to_string(),
                                message: "Thiếu mã đề/phòng thi để xóa".to_string(),
                            },
                        }),
                    )
                        .into_response();
                }
            }
        }
    };

    let target_id = match Uuid::parse_str(&exam_id_str) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "INVALID_EXAM_ID".to_string(),
                        message: "Mã đề thi không đúng định dạng UUID".to_string(),
                    },
                }),
            )
                .into_response();
        }
    };

    // 1. Dọn sạch toàn bộ dữ liệu ca thi trong CSDL (submissions, answers, examinees, violations)
    // và reset exam status = 'READY', started_at = NULL, end_at = NULL
    {
        let repo = state.repo.lock().await;
        if let Err(e) = repo.clear_exam_room_data(target_id) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse {
                    success: false,
                    error: ApiErrorDetail {
                        code: "DB_ERROR".to_string(),
                        message: format!("Lỗi dọn sạch dữ liệu phòng thi: {}", e),
                    },
                }),
            )
                .into_response();
        }
    }

    // 2. Chỉ dừng Cloudflare Tunnel nếu KHÔNG CÒN phòng thi nào khác đang chạy ở chế độ Online Tunnel
    {
        let has_other_online = {
            let repo = state.repo.lock().await;
            any_other_online_room_active(&repo, Some(target_id))
        };
        if !has_other_online {
            let mut tunnel_guard = state.tunnel.lock().await;
            if let Some(mut t) = tunnel_guard.take() {
                t.stop();
            }
            let _ = state.event_sender.send(ServerEvent::NetworkModeChanged {
                mode: "OFFLINE_LAN".to_string(),
                public_url: None,
            });
        }
    }

    // 3. Clear current_exam_id nếu đang trỏ tới phòng này
    {
        let mut curr_guard = state.current_exam_id.write().await;
        if *curr_guard == Some(target_id) {
            *curr_guard = None;
        }
    }

    // 4. Phát sự kiện RoomClosed
    let _ = state.event_sender.send(ServerEvent::RoomClosed {
        exam_id: target_id.to_string(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Đã xóa phòng thi và làm sạch dữ liệu thành công",
            "exam_id": target_id.to_string()
        })),
    )
        .into_response()
}
