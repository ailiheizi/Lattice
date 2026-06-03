//! Olm 1v1 加密会话封装
//!
//! 基于 vodozemac 的 Olm 实现，提供简化的 API。


use vodozemac::olm::{
    Account, AccountPickle, OlmMessage, Session, SessionConfig, SessionPickle,
};
use vodozemac::Curve25519PublicKey;

/// Olm 账户 — 管理设备的加密密钥
pub struct OlmAccount {
    inner: Account,
}

impl OlmAccount {
    /// 创建新账户
    pub fn new() -> Self {
        Self { inner: Account::new() }
    }

    /// 获取 Curve25519 身份密钥
    pub fn curve25519_key(&self) -> Curve25519PublicKey {
        self.inner.curve25519_key()
    }

    /// 获取 Ed25519 签名密钥
    pub fn ed25519_key(&self) -> vodozemac::Ed25519PublicKey {
        self.inner.ed25519_key()
    }

    /// 生成 one-time keys
    pub fn generate_one_time_keys(&mut self, count: usize) {
        self.inner.generate_one_time_keys(count);
    }

    /// 获取未发布的 one-time keys（只返回公钥列表）
    pub fn one_time_keys(&self) -> Vec<Curve25519PublicKey> {
        self.inner
            .one_time_keys()
            .into_values()
            .collect()
    }

    /// 标记 one-time keys 为已发布
    pub fn mark_keys_as_published(&mut self) {
        self.inner.mark_keys_as_published();
    }

    /// 生成 fallback key
    pub fn generate_fallback_key(&mut self) {
        self.inner.generate_fallback_key();
    }

    /// 创建出站会话（发起方）
    pub fn create_outbound_session(
        &self,
        their_identity_key: Curve25519PublicKey,
        their_one_time_key: Curve25519PublicKey,
    ) -> OlmSession {
        let session = self.inner.create_outbound_session(
            SessionConfig::version_2(),
            their_identity_key,
            their_one_time_key,
        );
        OlmSession { inner: session }
    }

    /// 创建入站会话（接收方，从 PreKey 消息）
    pub fn create_inbound_session(
        &mut self,
        their_identity_key: Curve25519PublicKey,
        pre_key_message: &vodozemac::olm::PreKeyMessage,
    ) -> Result<(OlmSession, Vec<u8>), vodozemac::olm::SessionCreationError> {
        let result = self.inner.create_inbound_session(
            their_identity_key,
            pre_key_message,
        )?;
        Ok((
            OlmSession { inner: result.session },
            result.plaintext,
        ))
    }

    /// 序列化（pickle）账户
    pub fn pickle(&self, _pickle_key: &[u8; 32]) -> String {
        let pickle = self.inner.pickle();
        serde_json::to_string(&pickle).unwrap_or_default()
    }

    /// 反序列化（unpickle）账户
    pub fn from_pickle(data: &str) -> Result<Self, String> {
        let pickle: AccountPickle = serde_json::from_str(data)
            .map_err(|e| e.to_string())?;
        let inner = Account::from(pickle);
        Ok(Self { inner })
    }
}

impl Default for OlmAccount {
    fn default() -> Self {
        Self::new()
    }
}

/// Olm 加密会话
pub struct OlmSession {
    inner: Session,
}

impl OlmSession {
    /// 加密消息
    pub fn encrypt(&mut self, plaintext: &[u8]) -> OlmMessage {
        self.inner.encrypt(plaintext)
    }

    /// 解密消息
    pub fn decrypt(&mut self, message: &OlmMessage) -> Result<Vec<u8>, vodozemac::olm::DecryptionError> {
        self.inner.decrypt(message)
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> String {
        self.inner.session_id()
    }

    /// 序列化会话
    pub fn pickle(&self) -> String {
        let pickle = self.inner.pickle();
        serde_json::to_string(&pickle).unwrap_or_default()
    }

    /// 反序列化会话
    pub fn from_pickle(data: &str) -> Result<Self, String> {
        let pickle: SessionPickle = serde_json::from_str(data)
            .map_err(|e| e.to_string())?;
        let inner = Session::from(pickle);
        Ok(Self { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_account() {
        let mut account = OlmAccount::new();
        account.generate_one_time_keys(5);
        let keys = account.one_time_keys();
        assert_eq!(keys.len(), 5);
        account.mark_keys_as_published();
    }

    #[test]
    fn test_olm_session_encrypt_decrypt() {
        // Alice 创建账户
        let alice = OlmAccount::new();

        // Bob 创建账户并生成 one-time key
        let mut bob = OlmAccount::new();
        bob.generate_one_time_keys(1);
        let bob_otk = bob.one_time_keys();
        let bob_one_time_key = &bob_otk[0];

        // Alice 创建出站会话
        let mut alice_session = alice.create_outbound_session(
            bob.curve25519_key(),
            *bob_one_time_key,
        );

        // Alice 加密消息
        let plaintext = b"hello bob, this is encrypted!";
        let ciphertext = alice_session.encrypt(plaintext);

        // Bob 从 PreKey 消息创建入站会话
        let pre_key = match &ciphertext {
            OlmMessage::PreKey(m) => m.clone(),
            _ => panic!("first message should be PreKey"),
        };

        let (mut bob_session, decrypted) = bob
            .create_inbound_session(alice.curve25519_key(), &pre_key)
            .unwrap();

        assert_eq!(decrypted, plaintext);

        // Bob 回复
        let reply = b"hi alice!";
        let reply_cipher = bob_session.encrypt(reply);
        let reply_decrypted = alice_session.decrypt(&reply_cipher).unwrap();
        assert_eq!(reply_decrypted, reply);
    }

    #[test]
    fn test_session_pickle_roundtrip() {
        let alice = OlmAccount::new();
        let mut bob = OlmAccount::new();
        bob.generate_one_time_keys(1);
        let bob_otk = bob.one_time_keys();
        let bob_one_time_key = &bob_otk[0];

        let session = alice.create_outbound_session(
            bob.curve25519_key(),
            *bob_one_time_key,
        );

        let session_id = session.session_id();
        let pickled = session.pickle();
        let restored = OlmSession::from_pickle(&pickled).unwrap();
        assert_eq!(restored.session_id(), session_id);
    }

    #[test]
    fn test_account_pickle_roundtrip() {
        let mut account = OlmAccount::new();
        account.generate_one_time_keys(3);
        let key = account.curve25519_key();

        let pickled = account.pickle(&[0u8; 32]);
        let restored = OlmAccount::from_pickle(&pickled).unwrap();
        assert_eq!(restored.curve25519_key(), key);
    }
}
