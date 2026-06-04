//! STUN 客户端 — NAT 类型检测和外部地址发现
//!
//! 实现 RFC 5389 STUN Binding Request，用于：
//! 1. 发现自己的公网 IP 和端口
//! 2. 检测 NAT 类型（用于判断是否可以直连）

use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// STUN 发现结果
#[derive(Debug, Clone)]
pub struct StunResult {
    /// 本地绑定地址
    pub local_addr: SocketAddr,
    /// STUN 服务器返回的外部地址（公网 IP + 端口）
    pub external_addr: SocketAddr,
    /// 是否在 NAT 后面
    pub behind_nat: bool,
}

/// STUN Binding Request（RFC 5389 最小实现）
///
/// STUN 消息格式：
/// - 2 bytes: Message Type (0x0001 = Binding Request)
/// - 2 bytes: Message Length
/// - 4 bytes: Magic Cookie (0x2112A442)
/// - 12 bytes: Transaction ID
fn build_binding_request() -> Vec<u8> {
    let mut buf = Vec::with_capacity(20);

    // Message Type: Binding Request (0x0001)
    buf.extend_from_slice(&0x0001u16.to_be_bytes());
    // Message Length: 0 (no attributes)
    buf.extend_from_slice(&0x0000u16.to_be_bytes());
    // Magic Cookie
    buf.extend_from_slice(&0x2112A442u32.to_be_bytes());
    // Transaction ID (12 random bytes)
    let tx_id: [u8; 12] = rand::random();
    buf.extend_from_slice(&tx_id);

    buf
}

/// 解析 STUN Binding Response，提取 XOR-MAPPED-ADDRESS
fn parse_binding_response(data: &[u8]) -> Option<SocketAddr> {
    if data.len() < 20 {
        return None;
    }

    // 验证 Message Type: Binding Success Response (0x0101)
    let msg_type = u16::from_be_bytes([data[0], data[1]]);
    if msg_type != 0x0101 {
        return None;
    }

    // 验证 Magic Cookie
    let cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if cookie != 0x2112A442 {
        return None;
    }

    let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    let attrs = &data[20..20 + msg_len.min(data.len() - 20)];

    // 遍历属性，查找 XOR-MAPPED-ADDRESS (0x0020) 或 MAPPED-ADDRESS (0x0001)
    let mut offset = 0;
    while offset + 4 <= attrs.len() {
        let attr_type = u16::from_be_bytes([attrs[offset], attrs[offset + 1]]);
        let attr_len = u16::from_be_bytes([attrs[offset + 2], attrs[offset + 3]]) as usize;
        let attr_data = &attrs[offset + 4..offset + 4 + attr_len.min(attrs.len() - offset - 4)];

        match attr_type {
            0x0020 => {
                // XOR-MAPPED-ADDRESS
                return parse_xor_mapped_address(attr_data, &data[4..8]);
            }
            0x0001 => {
                // MAPPED-ADDRESS (fallback)
                return parse_mapped_address(attr_data);
            }
            _ => {}
        }

        // 属性按 4 字节对齐
        offset += 4 + ((attr_len + 3) & !3);
    }

    None
}

fn parse_xor_mapped_address(data: &[u8], magic: &[u8]) -> Option<SocketAddr> {
    if data.len() < 8 {
        return None;
    }

    let family = data[1];
    let xor_port = u16::from_be_bytes([data[2], data[3]]);
    let port = xor_port ^ 0x2112; // XOR with first 2 bytes of magic cookie

    match family {
        0x01 => {
            // IPv4
            let ip = [
                data[4] ^ magic[0],
                data[5] ^ magic[1],
                data[6] ^ magic[2],
                data[7] ^ magic[3],
            ];
            let addr = std::net::Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]);
            Some(SocketAddr::new(addr.into(), port))
        }
        _ => None, // IPv6 暂不支持
    }
}

fn parse_mapped_address(data: &[u8]) -> Option<SocketAddr> {
    if data.len() < 8 {
        return None;
    }

    let family = data[1];
    let port = u16::from_be_bytes([data[2], data[3]]);

    match family {
        0x01 => {
            let addr = std::net::Ipv4Addr::new(data[4], data[5], data[6], data[7]);
            Some(SocketAddr::new(addr.into(), port))
        }
        _ => None,
    }
}

/// 向 STUN 服务器发送 Binding Request，获取外部地址
pub async fn discover_external_address(stun_server: &str) -> Result<StunResult, StunError> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| StunError::Network(e.to_string()))?;

    let local_addr = socket
        .local_addr()
        .map_err(|e| StunError::Network(e.to_string()))?;

    let server_addr: SocketAddr = stun_server
        .parse()
        .map_err(|e: std::net::AddrParseError| StunError::InvalidServer(e.to_string()))?;

    let request = build_binding_request();
    socket
        .send_to(&request, server_addr)
        .await
        .map_err(|e| StunError::Network(e.to_string()))?;

    let mut buf = [0u8; 512];
    let timeout = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        socket.recv_from(&mut buf),
    )
    .await
    .map_err(|_| StunError::Timeout)?
    .map_err(|e| StunError::Network(e.to_string()))?;

    let (len, _from) = timeout;
    let external_addr = parse_binding_response(&buf[..len]).ok_or(StunError::ParseError)?;

    let behind_nat =
        external_addr.ip() != local_addr.ip() || external_addr.port() != local_addr.port();

    Ok(StunResult {
        local_addr,
        external_addr,
        behind_nat,
    })
}

/// 常用的公共 STUN 服务器
pub const STUN_SERVERS: &[&str] = &[
    "74.125.250.129:19302", // Google stun1
    "64.233.163.127:19302", // Google stun2
    "159.89.location:3478", // 备用
];

#[derive(Debug, thiserror::Error)]
pub enum StunError {
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid STUN server address: {0}")]
    InvalidServer(String),
    #[error("STUN request timed out")]
    Timeout,
    #[error("failed to parse STUN response")]
    ParseError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_binding_request() {
        let req = build_binding_request();
        assert_eq!(req.len(), 20);
        // Message Type: Binding Request
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0x01);
        // Message Length: 0
        assert_eq!(req[2], 0x00);
        assert_eq!(req[3], 0x00);
        // Magic Cookie
        assert_eq!(&req[4..8], &[0x21, 0x12, 0xA4, 0x42]);
    }

    #[test]
    fn test_parse_xor_mapped_address() {
        // 构造一个模拟的 STUN Binding Success Response
        // 外部地址: 203.0.113.1:54321
        let magic = [0x21, 0x12, 0xA4, 0x42];

        let ip = [203u8, 0, 113, 1];
        let port: u16 = 54321;

        let xor_port = port ^ 0x2112;
        let xor_ip = [
            ip[0] ^ magic[0],
            ip[1] ^ magic[1],
            ip[2] ^ magic[2],
            ip[3] ^ magic[3],
        ];

        let mut response = Vec::new();
        // Header
        response.extend_from_slice(&0x0101u16.to_be_bytes()); // Binding Success
        response.extend_from_slice(&12u16.to_be_bytes()); // Attr length
        response.extend_from_slice(&magic); // Magic Cookie
        response.extend_from_slice(&[0u8; 12]); // Transaction ID

        // XOR-MAPPED-ADDRESS attribute
        response.extend_from_slice(&0x0020u16.to_be_bytes()); // Type
        response.extend_from_slice(&8u16.to_be_bytes()); // Length
        response.push(0x00); // Reserved
        response.push(0x01); // Family: IPv4
        response.extend_from_slice(&xor_port.to_be_bytes());
        response.extend_from_slice(&xor_ip);

        let addr = parse_binding_response(&response).unwrap();
        assert_eq!(addr.ip(), std::net::Ipv4Addr::new(203, 0, 113, 1));
        assert_eq!(addr.port(), 54321);
    }

    #[test]
    fn test_parse_invalid_response() {
        assert!(parse_binding_response(&[]).is_none());
        assert!(parse_binding_response(&[0; 10]).is_none());

        // Wrong message type
        let mut bad = vec![0x01, 0x00]; // Binding Request, not Response
        bad.extend_from_slice(&[0, 0]);
        bad.extend_from_slice(&[0x21, 0x12, 0xA4, 0x42]);
        bad.extend_from_slice(&[0; 12]);
        assert!(parse_binding_response(&bad).is_none());
    }
}
