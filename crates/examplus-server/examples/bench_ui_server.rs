//! Isolated in-memory ExamPlus HTTP server for Playwright UI evaluation.
//! Never touches live `examplus.db` or `ExamPlus_Drive`.

use examplus_core::db::DbRepository;
use examplus_server::{build_router, AppState};
use std::io::{self, Write};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::env::set_var("EXAMPLUS_DISABLE_DRIVE_MIRROR", "1");
    let repo = DbRepository::in_memory()?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let state = AppState::with_port(repo, None, addr.port());
    let secret = state.teacher_secret.as_ref().clone();

    println!("READY port={} secret={}", addr.port(), secret);
    let _ = io::stdout().flush();

    let app = build_router(state);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}
