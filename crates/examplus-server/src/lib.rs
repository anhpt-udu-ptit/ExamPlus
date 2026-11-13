//! # ExamPlus Server Crate
//!
//! Máy chủ Axum 0.7 nhúng, WebSocket Monitor Hub, Embedded Assets cho Student Web App,
//! và Quản lý kết nối Secure Tunnel.

pub mod handlers;
pub mod network;
pub mod router;
pub mod state;
pub mod static_files;

pub use network::{get_local_ip, CloudflareTunnel};
pub use router::build_router;
pub use state::{AppState, ServerEvent};

use std::net::SocketAddr;
use tokio::net::TcpListener;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Khởi chạy máy chủ Axum lắng nghe trên cổng chỉ định
pub async fn run_server(
    state: AppState,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = build_router(state);
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("ExamPlus Axum Server đang lắng nghe tại: http://{}", addr);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_version() {
        assert_eq!(version(), "0.1.0");
    }
}
