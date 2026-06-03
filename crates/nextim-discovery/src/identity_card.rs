//! 身份卡片 — 手动交换用的序列化/反序列化
//!
//! 支持导出为 JSON 字符串（可生成二维码）和从 JSON 导入。

use serde::{Deserialize, Serialize};

/// 可序列化的身份卡片（用于二维码/链接分享）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCardExport {
    pub fingerprint: String,
    pub display_name: String,
    pub ed25519_public_key: String,    // base64
    pub curve25519_public_key: String, // base64
    pub store_address: String,
    #[serde(default)]
    pub proxy_store_address: String,
}

impl IdentityCardExport {
    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 序列化为紧凑 JSON（适合二维码）
    pub fn to_compact_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 反序列化
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 生成可分享的 URI（nextim:// 协议）
    pub fn to_uri(&self) -> String {
        format!(
            "nextim://add?fp={}&name={}&store={}",
            self.fingerprint,
            urlencoded(&self.display_name),
            urlencoded(&self.store_address),
        )
    }
}

fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card() -> IdentityCardExport {
        IdentityCardExport {
            fingerprint: "abcdef1234567890".to_string(),
            display_name: "Alice".to_string(),
            ed25519_public_key: "AAAA".to_string(),
            curve25519_public_key: "BBBB".to_string(),
            store_address: "ws://127.0.0.1:9100".to_string(),
            proxy_store_address: String::new(),
        }
    }

    #[test]
    fn test_json_roundtrip() {
        let card = make_card();
        let json = card.to_json().unwrap();
        let restored = IdentityCardExport::from_json(&json).unwrap();
        assert_eq!(restored.fingerprint, card.fingerprint);
        assert_eq!(restored.display_name, card.display_name);
        assert_eq!(restored.store_address, card.store_address);
    }

    #[test]
    fn test_to_uri() {
        let card = make_card();
        let uri = card.to_uri();
        assert!(uri.starts_with("nextim://add?"));
        assert!(uri.contains("fp=abcdef1234567890"));
        assert!(uri.contains("name=Alice"));
    }

    #[test]
    fn test_from_json_with_missing_proxy() {
        let json = r#"{"fingerprint":"abc","display_name":"Bob","ed25519_public_key":"X","curve25519_public_key":"Y","store_address":"ws://localhost"}"#;
        let card = IdentityCardExport::from_json(json).unwrap();
        assert_eq!(card.proxy_store_address, "");
    }
}
