//! # Embedded Static Files
//!
//! Nhúng toàn bộ mã nguồn giao diện thí sinh (Single Page Application)
//! vào thẳng tệp nhị phân .exe thông qua `rust-embed`.

use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "assets/student"]
struct StudentAsset;

#[derive(Embed)]
#[folder = "assets/teacher"]
struct TeacherAsset;

/// Phục vụ tài nguyên giao diện Giáo viên (được bảo vệ bởi Teacher Guard)
pub async fn teacher_static_handler(uri: Uri) -> impl IntoResponse {
    let raw_path = uri.path();
    let mut sub_path = raw_path
        .trim_start_matches("/teacher")
        .trim_start_matches('/')
        .to_string();

    if sub_path.is_empty() {
        sub_path = "index.html".to_string();
    }

    match TeacherAsset::get(&sub_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&sub_path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
                .body(Body::from(content.data))
                .unwrap()
        }
        None => match TeacherAsset::get("index.html") {
            Some(content) => Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
                .body(Body::from(content.data))
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Teacher Not Found"))
                .unwrap(),
        },
    }
}

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let raw_path = uri.path();

    if raw_path.starts_with("/teacher") {
        return teacher_static_handler(uri).await.into_response();
    }

    let mut path = raw_path.trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match StudentAsset::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
                .body(Body::from(content.data))
                .unwrap()
        }
        None => match StudentAsset::get("index.html") {
            Some(content) => Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
                .body(Body::from(content.data))
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap(),
        },
    }
}
