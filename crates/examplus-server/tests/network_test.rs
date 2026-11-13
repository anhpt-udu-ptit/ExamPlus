use examplus_server::network::{get_local_ip, CloudflareTunnel};

#[test]
fn test_get_local_ip_returns_valid_ip() {
    let ip = get_local_ip();
    assert!(!ip.is_empty(), "Địa chỉ IP không được rỗng");
    // Phải là địa chỉ IPv4 hợp lệ (chứa 3 dấu chấm)
    let parts: Vec<&str> = ip.split('.').collect();
    assert_eq!(parts.len(), 4, "Phải là định dạng IPv4 chuẩn");
    for p in parts {
        let _octet: u8 = p.parse().expect("Mỗi octet phải là số 0-255");
    }
}

#[test]
fn test_cloudflare_tunnel_lifecycle() {
    let mut tunnel = CloudflareTunnel::new();
    assert!(tunnel.public_url().is_none());
    tunnel.stop();
    assert!(tunnel.public_url().is_none());
}
