//! # Dual-Network Subsystem
//!
//! Mô-đun quản lý mạng kép: Phát hiện IP LAN cục bộ và Điều khiển Cloudflare Quick Tunnel.

pub mod local_ip;
pub mod tunnel;

pub use local_ip::get_local_ip;
pub use tunnel::{CloudflareTunnel, TunnelError};
