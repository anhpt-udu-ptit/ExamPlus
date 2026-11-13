//! # Local IP Detection
//!
//! Tự động nhận diện địa chỉ IPv4 cục bộ của máy giáo viên trong mạng LAN/Wi-Fi phòng thi.

use std::net::UdpSocket;

/// Phát hiện địa chỉ IPv4 cục bộ của máy giáo viên (hoạt động kể cả khi mạng LAN hoàn toàn offline)
pub fn get_local_ip() -> String {
    // Thử kết nối UDP socket giả định tới các dải mạng LAN phổ biến để hệ điều hành chọn Card mạng chuẩn
    let probe_targets = [
        "192.168.1.1:80",
        "192.168.0.1:80",
        "10.0.0.1:80",
        "172.16.0.1:80",
        "8.8.8.8:80",
    ];

    for target in probe_targets {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(target).is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    let ip = addr.ip();
                    if !ip.is_loopback() && ip.is_ipv4() {
                        return ip.to_string();
                    }
                }
            }
        }
    }

    "127.0.0.1".to_string()
}
