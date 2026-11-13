//! # QR Code Generator Handler
//!
//! Sinh mã QR định dạng SVG 100% offline từ chuỗi URL bằng crate `qrcode`.

use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct QrQuery {
    pub url: Option<String>,
}

pub async fn get_qr_svg(Query(query): Query<QrQuery>) -> impl IntoResponse {
    let target_url = query
        .url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());

    let code = match QrCode::new(target_url.as_bytes()) {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "Invalid QR content").into_response();
        }
    };

    let svg_xml = code
        .render::<svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .build();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(axum::body::Body::from(svg_xml))
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error creating response",
            )
                .into_response()
        })
}
