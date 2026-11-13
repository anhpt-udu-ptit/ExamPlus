//! # WebSocket Telemetry Hub
//!
//! Kênh truyền thông tin hai chiều thời gian thực giữa Thí sinh và Giáo viên:
//! - Nhịp tim (Heartbeat) phát hiện rớt mạng
//! - Cảnh báo gian lận trực tiếp (Chuyển tab, thoát toàn màn hình, mất tiêu điểm)
//! - Cập nhật tiến độ phòng thi theo thời gian thực

use crate::state::{AppState, ServerEvent};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use chrono::Utc;
use examplus_core::models::{AuditViolation, ExamineeStatus, ViolationType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientWsMessage {
    #[serde(rename = "heartbeat")]
    Heartbeat,
    #[serde(rename = "violation")]
    Violation {
        violation_type: String,
        duration_seconds: u32,
        details: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct WsAckResponse {
    pub success: bool,
    pub timestamp: String,
}

pub async fn ws_student_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = query.token.unwrap_or_default();
    ws.on_upgrade(move |socket| handle_student_socket(socket, token, state))
}

async fn handle_student_socket(mut socket: WebSocket, token: String, state: AppState) {
    let examinee_opt = {
        let repo = state.repo.lock().await;
        repo.get_examinee_by_token(&token).ok().flatten()
    };

    let examinee = match examinee_opt {
        Some(e) => e,
        None => {
            let _ = socket.send(Message::Text("{\"error\":\"INVALID_TOKEN\"}".to_string())).await;
            return;
        }
    };

    let mut event_rx = state.event_sender.subscribe();

    loop {
        tokio::select! {
            server_event = event_rx.recv() => {
                match server_event {
                    Ok(ServerEvent::ExamStarted { started_at, end_at, duration_minutes, exam_id }) => {
                        if let Some(ref eid) = exam_id {
                            if eid != &examinee.exam_id.to_string() {
                                continue;
                            }
                        }
                        let msg = serde_json::json!({
                            "type": "ExamStarted",
                            "started_at": started_at,
                            "end_at": end_at,
                            "duration_minutes": duration_minutes,
                            "exam_id": exam_id,
                        });
                        if socket.send(Message::Text(msg.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Ok(ServerEvent::ExamFinished { exam_id }) => {
                        if !exam_id.is_empty() && exam_id != examinee.exam_id.to_string() {
                            continue;
                        }
                        let msg = serde_json::json!({
                            "type": "ExamFinished",
                            "exam_id": exam_id,
                        });
                        let _ = socket.send(Message::Text(msg.to_string())).await;
                    }
                    Ok(ServerEvent::RoomClosed { exam_id }) => {
                        if !exam_id.is_empty() && exam_id != examinee.exam_id.to_string() {
                            continue;
                        }
                        let msg = serde_json::json!({
                            "type": "RoomClosed",
                            "exam_id": exam_id,
                        });
                        let _ = socket.send(Message::Text(msg.to_string())).await;
                    }
                    _ => {}
                }
            }
            msg = socket.recv() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    _ => break,
                };

                if let Message::Text(text) = msg {
                    if let Ok(client_msg) = serde_json::from_str::<ClientWsMessage>(&text) {
                        match client_msg {
                            ClientWsMessage::Heartbeat => {
                                let repo = state.repo.lock().await;
                                let _ = repo.update_examinee_heartbeat(examinee.id);
                                let _ = state.event_sender.send(ServerEvent::StudentHeartbeat {
                                    student_code: examinee.student_code.clone(),
                                    timestamp: Utc::now().to_rfc3339(),
                                    exam_id: Some(examinee.exam_id.to_string()),
                                });
                            }
                            ClientWsMessage::Violation {
                                violation_type,
                                duration_seconds,
                                details,
                            } => {
                                // 1. Kiểm tra trạng thái hiện tại của thí sinh trong CSDL:
                                // Tuyệt đối không ghi nhận vi phạm nếu thí sinh đã nộp bài (SUBMITTED)
                                let examinee_check = {
                                    let repo = state.repo.lock().await;
                                    repo.get_examinee_by_token(&token).ok().flatten()
                                };

                                if let Some(current_ex) = examinee_check {
                                    if current_ex.status == ExamineeStatus::Submitted {
                                        // Thí sinh đã nộp bài: loại bỏ hoàn toàn vi phạm sau nộp
                                        continue;
                                    }
                                } else {
                                    continue;
                                }

                                // 2. Kiểm tra nếu kỳ thi đã kết thúc (quá ân hạn 30s)
                                let is_exam_active = {
                                    let repo = state.repo.lock().await;
                                    if let Ok(Some(exam)) = repo.get_exam(examinee.exam_id) {
                                        if let Some(end_at) = exam.end_at {
                                            Utc::now() <= end_at + chrono::Duration::seconds(30)
                                        } else {
                                            true
                                        }
                                    } else {
                                        false
                                    }
                                };

                                if !is_exam_active {
                                    continue;
                                }

                                let vtype = match violation_type.as_str() {
                                    "BLUR" => ViolationType::Blur,
                                    "FULLSCREEN_EXIT" => ViolationType::FullscreenExit,
                                    "COPY_ATTEMPT" => ViolationType::CopyAttempt,
                                    "PRINT_SCREEN" => ViolationType::PrintScreen,
                                    _ => ViolationType::TabSwitch,
                                };

                                let violation = AuditViolation {
                                    id: Uuid::new_v4(),
                                    examinee_id: examinee.id,
                                    violation_type: vtype,
                                    duration_seconds,
                                    details: details.clone(),
                                    timestamp: Utc::now(),
                                };

                                {
                                    let repo = state.repo.lock().await;
                                    let _ = repo.create_violation(&violation);
                                }

                                let _ = state.event_sender.send(ServerEvent::ViolationAlert {
                                    student_code: examinee.student_code.clone(),
                                    full_name: examinee.full_name.clone(),
                                    violation_type,
                                    details: details.unwrap_or_default(),
                                    exam_id: Some(examinee.exam_id.to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

pub async fn ws_teacher_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_teacher_socket(socket, state))
}

async fn handle_teacher_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.event_sender.subscribe();

    while let Ok(event) = rx.recv().await {
        if let Ok(json_text) = serde_json::to_string(&event) {
            if socket.send(Message::Text(json_text)).await.is_err() {
                break;
            }
        }
    }
}
