//! # Healthcheck Endpoint
//!
//! Kiểm tra trạng thái hoạt động của máy chủ và kết nối CSDL SQLite.

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: &'static str,
    pub db_status: String,
}

pub async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    let repo = state.repo.lock().await;
    let db_ok = repo.conn().query_row("SELECT 1", (), |_| Ok(())).is_ok();

    let db_status = if db_ok {
        "connected".to_string()
    } else {
        "error".to_string()
    };

    let status = if db_ok { "ok" } else { "degraded" };
    let status_code = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(HealthResponse {
            status: status.to_string(),
            version: env!("CARGO_PKG_VERSION"),
            db_status,
        }),
    )
        .into_response()
}
