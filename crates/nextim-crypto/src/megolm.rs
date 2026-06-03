//! Megolm 群聊加密封装
//!
//! 基于 vodozemac 的 Megolm 实现。
//! - GroupSession (outbound): 发送方持有，用于加密群消息
//! - InboundGroupSession: 接收方持有，用于解密群消息

use vodozemac::megolm::{
    GroupSession, GroupSessionPickle,
    InboundGroupSession, InboundGroupSessionPickle,
    MegolmMessage, SessionKey,
};

/// Megolm 出站群会话（发送方）
pub struct MegolmOutboundSession {
    inner: GroupSession,
}

impl MegolmOutboundSession {
    /// 创建新的出站会话
    pub fn new() -> Self {
        Self { inner: GroupSession::new(Default::default()) }
    }

    /// 加密消息
    pub fn encrypt(&mut self, plaintext: &[u8]) -> MegolmMessage {
        self.inner.encrypt(plaintext)
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> String {
        self.inner.session_id()
    }

    /// 获取当前消息索引
    pub fn message_index(&self) -> u32 {
        self.inner.message_index()
    }

    /// 导出会话密钥（用于通过 Olm 分发给群成员）
    pub fn session_key(&self) -> SessionKey {
        self.inner.session_key()
    }

    /// 序列化
    pub fn pickle(&self) -> String {
        serde_json::to_string(&self.inner.pickle()).unwrap_or_default()
    }

    /// 反序列化
    pub fn from_pickle(data: &str) -> Result<Self, String> {
        let pickle: GroupSessionPickle = serde_json::from_str(data)
            .map_err(|e| e.to_string())?;
        Ok(Self { inner: GroupSession::from(pickle) })
    }
}

impl Default for MegolmOutboundSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Megolm 入站群会话（接收方）
pub struct MegolmInboundSession {
    inner: InboundGroupSession,
}

impl MegolmInboundSession {
    /// 从会话密钥创建（接收方收到发送方分发的密钥后调用）
    pub fn new(session_key: &SessionKey) -> Self {
        Self {
            inner: InboundGroupSession::new(session_key, Default::default()),
        }
    }

    /// 解密消息
    pub fn decrypt(
        &mut self,
        message: &MegolmMessage,
    ) -> Result<vodozemac::megolm::DecryptedMessage, vodozemac::megolm::DecryptionError> {
        self.inner.decrypt(message)
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> String {
        self.inner.session_id()
    }

    /// 序列化
    pub fn pickle(&self) -> String {
        serde_json::to_string(&self.inner.pickle()).unwrap_or_default()
    }

    /// 反序列化
    pub fn from_pickle(data: &str) -> Result<Self, String> {
        let pickle: InboundGroupSessionPickle = serde_json::from_str(data)
            .map_err(|e| e.to_string())?;
        Ok(Self { inner: InboundGroupSession::from(pickle) })
    }
}

/// 群聊密钥轮换策略
pub struct KeyRotationPolicy {
    /// 最大消息数（超过后轮换）
    pub max_messages: u32,
    /// 最大存活时间（毫秒）
    pub max_age_ms: u64,
}

impl Default for KeyRotationPolicy {
    fn default() -> Self {
        Self {
            max_messages: 100,
            max_age_ms: 7 * 24 * 60 * 60 * 1000, // 7 天
        }
    }
}

impl KeyRotationPolicy {
    /// 判断是否需要轮换
    pub fn should_rotate(&self, message_index: u32, session_created_at: u64, now: u64) -> bool {
        message_index >= self.max_messages || (now - session_created_at) >= self.max_age_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_megolm_encrypt_decrypt() {
        // 发送方创建出站会话
        let mut outbound = MegolmOutboundSession::new();
        let session_key = outbound.session_key();

        // 接收方从会话密钥创建入站会话
        let mut inbound = MegolmInboundSession::new(&session_key);

        assert_eq!(outbound.session_id(), inbound.session_id());

        // 加密
        let plaintext = b"hello group!";
        let ciphertext = outbound.encrypt(plaintext);

        // 解密
        let decrypted = inbound.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted.plaintext, plaintext);
        assert_eq!(decrypted.message_index, 0);
    }

    #[test]
    fn test_megolm_multiple_messages() {
        let mut outbound = MegolmOutboundSession::new();
        let session_key = outbound.session_key();
        let mut inbound = MegolmInboundSession::new(&session_key);

        for i in 0..5u32 {
            let msg = format!("message {i}");
            let cipher = outbound.encrypt(msg.as_bytes());
            let decrypted = inbound.decrypt(&cipher).unwrap();
            assert_eq!(decrypted.plaintext, msg.as_bytes());
            assert_eq!(decrypted.message_index, i);
        }

        assert_eq!(outbound.message_index(), 5);
    }

    #[test]
    fn test_megolm_multiple_receivers() {
        let mut outbound = MegolmOutboundSession::new();
        let session_key = outbound.session_key();

        // 多个接收方
        let mut inbound_a = MegolmInboundSession::new(&session_key);
        let mut inbound_b = MegolmInboundSession::new(&session_key);

        let cipher = outbound.encrypt(b"broadcast");

        let dec_a = inbound_a.decrypt(&cipher).unwrap();
        let dec_b = inbound_b.decrypt(&cipher).unwrap();

        assert_eq!(dec_a.plaintext, b"broadcast");
        assert_eq!(dec_b.plaintext, b"broadcast");
    }

    #[test]
    fn test_megolm_pickle_roundtrip() {
        let outbound = MegolmOutboundSession::new();
        let session_id = outbound.session_id();

        let pickled = outbound.pickle();
        let restored = MegolmOutboundSession::from_pickle(&pickled).unwrap();
        assert_eq!(restored.session_id(), session_id);
    }

    #[test]
    fn test_key_rotation_policy() {
        let policy = KeyRotationPolicy::default();

        // 未超过限制
        assert!(!policy.should_rotate(50, 0, 1000));

        // 消息数超过
        assert!(policy.should_rotate(100, 0, 1000));

        // 时间超过 7 天
        let seven_days = 7 * 24 * 60 * 60 * 1000;
        assert!(policy.should_rotate(0, 0, seven_days));
    }

    #[test]
    fn test_key_rotation_new_session() {
        // 模拟密钥轮换：旧会话加密的消息，新会话无法解密
        let mut old_outbound = MegolmOutboundSession::new();
        let old_key = old_outbound.session_key();
        let mut old_inbound = MegolmInboundSession::new(&old_key);

        let old_cipher = old_outbound.encrypt(b"old message");
        let _ = old_inbound.decrypt(&old_cipher).unwrap();

        // 轮换：创建新会话
        let mut new_outbound = MegolmOutboundSession::new();
        let new_key = new_outbound.session_key();
        let mut new_inbound = MegolmInboundSession::new(&new_key);

        let new_cipher = new_outbound.encrypt(b"new message");
        let decrypted = new_inbound.decrypt(&new_cipher).unwrap();
        assert_eq!(decrypted.plaintext, b"new message");

        // 旧入站会话无法解密新会话的消息
        assert!(old_inbound.decrypt(&new_cipher).is_err());
    }
}
