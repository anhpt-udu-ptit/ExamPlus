//! # Cloudflare Quick Tunnel Manager
//!
//! Quản lý tiến trình kết nối xuyên mạng thông qua Cloudflare Quick Tunnel:
//! - Không cần mở cổng modem (Port Forwarding), hoạt động mượt mà sau CGNAT & tường lửa trường học.
//! - Bắt giữ URL công khai dạng `https://*.trycloudflare.com` và tự động dọn dẹp tiến trình khi dừng.

use regex::Regex;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("Không tìm thấy tệp thực thi cloudflared")]
    BinaryNotFound,
    #[error("Lỗi khởi chạy tiến trình: {0}")]
    ProcessFailed(String),
    #[error("Hết thời gian chờ nhận URL đường hầm")]
    Timeout,
}

pub struct CloudflareTunnel {
    child: Option<Child>,
    public_url: Option<String>,
}

impl CloudflareTunnel {
    pub fn new() -> Self {
        Self {
            child: None,
            public_url: None,
        }
    }

    /// Tìm kiếm tệp nhị phân `cloudflared.exe` trong các đường dẫn tiêu chuẩn và thư mục chứa app
    pub fn find_binary() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(override_path) = std::env::var("EXAMPLUS_CLOUDFLARED_PATH") {
            let trimmed = override_path.trim();
            if !trimmed.is_empty() {
                candidates.push(PathBuf::from(trimmed));
            }
        }

        // Ưu tiên kiểm tra cùng thư mục với file thực thi ExamPlus.exe và các cấp cha (Portable / dist / cargo target)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidates.push(exe_dir.join("cloudflared.exe"));
                candidates.push(exe_dir.join("tools").join("cloudflared.exe"));
                candidates.push(exe_dir.join("..").join("tools").join("cloudflared.exe"));
                candidates.push(exe_dir.join("..").join("..").join("tools").join("cloudflared.exe"));
                candidates.push(
                    exe_dir
                        .join("..")
                        .join("..")
                        .join("..")
                        .join("tools")
                        .join("cloudflared.exe"),
                );
                candidates.push(
                    exe_dir
                        .join("..")
                        .join("..")
                        .join("dist")
                        .join("ExamPlus")
                        .join("cloudflared.exe"),
                );
            }
        }

        candidates.extend([
            PathBuf::from("cloudflared.exe"),
            PathBuf::from("tools/cloudflared.exe"),
            PathBuf::from("../tools/cloudflared.exe"),
            PathBuf::from("../../tools/cloudflared.exe"),
            PathBuf::from("dist/ExamPlus/cloudflared.exe"),
            PathBuf::from("../dist/ExamPlus/cloudflared.exe"),
            PathBuf::from(r"E:\ExamPlus\dist\ExamPlus\cloudflared.exe"),
            PathBuf::from(r"E:\ExamPlus\tools\cloudflared.exe"),
            PathBuf::from(r"E:\ExamPlus\target\release\cloudflared.exe"),
            PathBuf::from(r"E:\ExamPlus\target\debug\cloudflared.exe"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/cloudflared.exe"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/ExamPlus/cloudflared.exe"),
        ]);

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            candidates.push(
                Path::new(&local_app_data)
                    .join("ExamPlus")
                    .join("cloudflared.exe"),
            );
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            candidates.push(
                Path::new(&program_files)
                    .join("cloudflared")
                    .join("cloudflared.exe"),
            );
        }
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            candidates.push(
                Path::new(&program_files_x86)
                    .join("cloudflared")
                    .join("cloudflared.exe"),
            );
        }

        for c in candidates {
            if c.is_file() {
                return Some(c);
            }
        }

        // Kiểm tra PATH hệ thống
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let bin = dir.join("cloudflared.exe");
                if bin.is_file() {
                    return Some(bin);
                }
            }
        }

        None
    }

    /// Khởi động Quick Tunnel trỏ về cổng cục bộ
    pub fn start(&mut self, local_port: u16, binary_path: Option<&Path>) -> Result<String, TunnelError> {
        self.stop();

        let bin = binary_path
            .map(|p| p.to_path_buf())
            .or_else(Self::find_binary)
            .ok_or(TunnelError::BinaryNotFound)?;

        let mut cmd = Command::new(&bin);
        cmd.args([
            "tunnel",
            "--url",
            &format!("http://127.0.0.1:{}", local_port),
            "--no-autoupdate",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| TunnelError::ProcessFailed(e.to_string()))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TunnelError::ProcessFailed("Không thể mở stderr của cloudflared".into()))?;

        let url_shared = Arc::new(Mutex::new(None));
        let url_clone = url_shared.clone();
        let stderr_tail = Arc::new(Mutex::new(Vec::<String>::new()));
        let stderr_tail_clone = stderr_tail.clone();

        // Luồng đọc stderr để trích xuất URL Quick Tunnel và giữ log lỗi gần nhất
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let re = Regex::new(r"https://[a-zA-Z0-9-]+\.trycloudflare\.com").unwrap();
            let mut found = false;

            for line in reader.lines().map_while(Result::ok) {
                if !found {
                    if let Some(mat) = re.find(&line) {
                        if let Ok(mut guard) = url_clone.lock() {
                            *guard = Some(mat.as_str().to_string());
                        }
                        found = true;
                    }
                    if let Ok(mut tail) = stderr_tail_clone.lock() {
                        if tail.len() >= 15 {
                            tail.remove(0);
                        }
                        tail.push(line);
                    }
                }
            }
        });

        // Chờ nhận URL tối đa 15 giây (thoát sớm nếu tiến trình dừng bất thường)
        let start_time = std::time::Instant::now();
        while start_time.elapsed() < Duration::from_secs(15) {
            {
                let guard = url_shared.lock().unwrap();
                if let Some(ref url) = *guard {
                    self.public_url = Some(url.clone());
                    self.child = Some(child);
                    return Ok(url.clone());
                }
            }

            if let Ok(Some(exit_status)) = child.try_wait() {
                std::thread::sleep(Duration::from_millis(50));
                if let Ok(guard) = url_shared.lock() {
                    if let Some(ref url) = *guard {
                        self.public_url = Some(url.clone());
                        self.child = Some(child);
                        return Ok(url.clone());
                    }
                }
                let detail = stderr_tail
                    .lock()
                    .ok()
                    .and_then(|lines| lines.last().cloned())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| format!("exit status {}", exit_status));
                return Err(TunnelError::ProcessFailed(detail));
            }

            std::thread::sleep(Duration::from_millis(200));
        }

        // Nếu hết thời gian chờ mà chưa nhận được URL, tiêu diệt tiến trình
        let _ = child.kill();
        let _ = child.wait();
        Err(TunnelError::Timeout)
    }

    /// Lấy URL công khai hiện tại
    pub fn public_url(&self) -> Option<&str> {
        self.public_url.as_deref()
    }

    /// Kiểm tra tiến trình tunnel có đang thực sự chạy hay không
    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => self.public_url.is_some(),
                _ => {
                    self.public_url = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Dừng tiến trình Quick Tunnel
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.public_url = None;
    }
}

impl Drop for CloudflareTunnel {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_binary_resolves_workspace_cloudflared() {
        let bin = CloudflareTunnel::find_binary();
        assert!(
            bin.is_some(),
            "CloudflareTunnel::find_binary() phải tìm thấy cloudflared.exe trong tools/ hoặc dist/ExamPlus/"
        );
        let path = bin.unwrap();
        assert!(path.is_file(), "Đường dẫn tìm thấy phải là tệp hợp lệ: {:?}", path);
    }
}
