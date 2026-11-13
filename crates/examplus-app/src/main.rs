#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! # ExamPlus Desktop Application
//!
//! Ứng dụng Desktop chạy độc lập dành cho Giáo viên tổ chức thi trắc nghiệm GDPT 2018:
//! - Khởi chạy cửa sổ Native Win32 bằng Tao + Wry (Microsoft Edge WebView2).
//! - Ẩn hoàn toàn cửa sổ đen Terminal (Windows Subsystem: Windows).
//! - Chạy máy chủ Axum 0.7 ngầm trên cổng 8080 phục vụ thí sinh quét mã QR / vào link làm bài.

use examplus_core::db::DbRepository;
use examplus_server::{get_local_ip, run_server, AppState};
use std::net::SocketAddr;
use std::path::PathBuf;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::{WindowBuilderExtWindows, WindowExtWindows},
    window::{Icon, WindowBuilder},
};
use wry::WebViewBuilder;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 0. Thiết lập Application User Model ID trên Windows để Taskbar hiển thị đúng Icon nhận diện
    #[cfg(target_os = "windows")]
    unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let app_id: Vec<u16> = OsStr::new("ExamPlus.TeacherStudio.1.0")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(AppID: *const u16) -> i32;
        }
        let _ = SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }

    // Khởi tạo Tracing Logger (trong debug mode)
    #[cfg(debug_assertions)]
    {
        let _ = tracing_subscriber::fmt::try_init();
    }

    // 1. Khởi tạo CSDL SQLite cục bộ (WAL mode)
    let db_path = PathBuf::from("examplus.db");
    let repo = DbRepository::open(&db_path)?;
    // Khi khởi động app, không tự động mở phòng thi mặc định; giáo viên chủ động mở phòng khi cần
    let initial_exam_id = None;
    let port: u16 = 8080;
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let local_ip = get_local_ip();

    let state = AppState::with_port(repo, initial_exam_id, port);

    if std::env::args().any(|a| a == "--headless-server") {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async move {
            let _ = run_server(state, addr).await;
        });
        return Ok(());
    }

    // 2. Khởi chạy máy chủ Axum trên luồng ngầm (Multi-Thread Tokio Runtime)
    let server_state = state.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Không thể tạo Tokio Runtime cho máy chủ Axum");

        rt.block_on(async move {
            tracing::info!("Máy chủ ngầm đang phục vụ tại: http://{}:{}", local_ip, port);
            if let Err(e) = run_server(server_state, addr).await {
                eprintln!("Lỗi máy chủ Axum: {}", e);
            }
        });
    });

    // 3. Khởi tạo Cửa sổ Native Desktop bằng Tao với Custom Titlebar
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let icon_32_bytes = include_bytes!("../resources/icon_32.rgba");
    let window_icon = Icon::from_rgba(icon_32_bytes.to_vec(), 32, 32).ok();

    let icon_256_bytes = include_bytes!("../resources/icon_256.rgba");
    let taskbar_icon = Icon::from_rgba(icon_256_bytes.to_vec(), 256, 256).ok();

    let mut window_builder = WindowBuilder::new()
        .with_title("ExamPlus")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(1024.0, 700.0))
        .with_decorations(false)
        .with_undecorated_shadow(true);

    if let Some(ref icon) = window_icon {
        window_builder = window_builder.with_window_icon(Some(icon.clone()));
    }
    #[cfg(target_os = "windows")]
    if let Some(ref icon) = taskbar_icon {
        window_builder = window_builder.with_taskbar_icon(Some(icon.clone()));
    }

    let window = window_builder.build(&event_loop)?;

    #[cfg(target_os = "windows")]
    {
        if let Some(ref icon) = taskbar_icon {
            window.set_taskbar_icon(Some(icon.clone()));
        }
        if let Some(ref icon) = window_icon {
            window.set_window_icon(Some(icon.clone()));
        }
    }

    // 4. Khởi tạo WebView2 nạp giao diện Giáo viên bên trong cửa sổ App
    let teacher_url = format!("http://127.0.0.1:{}/teacher", port);
    let _webview = WebViewBuilder::new()
        .with_devtools(false)
        .with_initialization_script(
            r#"(function () {
                document.addEventListener('contextmenu', function (e) {
                    e.preventDefault();
                }, true);
                document.addEventListener('keydown', function (e) {
                    const key = (e.key || '').toUpperCase();
                    if (
                        key === 'F12' ||
                        (e.ctrlKey && e.shiftKey && (key === 'I' || key === 'J' || key === 'C')) ||
                        (e.ctrlKey && !e.shiftKey && key === 'U')
                    ) {
                        e.preventDefault();
                        e.stopPropagation();
                        return false;
                    }
                }, true);
            })();"#,
        )
        .with_url(&teacher_url)
        .with_ipc_handler(move |req| {
            let _ = proxy.send_event(req.body().clone());
        })
        .build(&window)?;

    // 5. Vòng lặp sự kiện Native Windows Message Loop
    event_loop.run(move |event, _event_loop_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(cmd) => {
                if let Some(url) = cmd.strip_prefix("open_external:") {
                    let url = url.trim();
                    if url.starts_with("http://") || url.starts_with("https://") {
                        #[cfg(target_os = "windows")]
                        {
                            use std::os::windows::process::CommandExt;
                            const CREATE_NO_WINDOW: u32 = 0x08000000;
                            let mut cmd = std::process::Command::new("rundll32");
                            cmd.creation_flags(CREATE_NO_WINDOW);
                            let _ = cmd
                                .args(["url.dll,FileProtocolHandler", url])
                                .spawn();
                        }
                    }
                } else {
                    match cmd.as_str() {
                        "minimize" => window.set_minimized(true),
                        "maximize" => window.set_maximized(!window.is_maximized()),
                        "close" => *control_flow = ControlFlow::Exit,
                        "drag_window" => {
                            let _ = window.drag_window();
                        }
                        _ => (),
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_app_version() {
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
    }
}
