//! 1v1 Olm 会话编排(运行时核心,sans-I/O)。
//!
//! 把 `olm` 模块的原语接成"对端 → 会话"的编排层:建立出站会话(从 claim 到的预密钥)、
//! 加密明文为 `EncryptedPayload`、解密 `EncryptedPayload` 回明文、会话/账户持久化。
//!
//! 纯逻辑:不碰网络/磁盘(取预密钥、传密文、存 pickle 都由调用方做),便于单测。
//! Store 永远只转发 `EncryptedPayload.ciphertext`,不解密 —— 编排只在客户端侧用。
//!
//! ## EncryptedPayload 字段映射(OLM)
//! - `ciphertext`:Olm 密文字节(`OlmMessage::to_parts` 的密文部分)。
//! - `message_index`:复用承载 Olm 消息类型(0=PreKey,1=Normal),解密方据此区分。
//! - `session_id`:Olm 会话 id(便于关联/调试)。
//! - `encryption_type`:固定 `EncryptionType::Olm`。

use std::collections::HashMap;

use vodozemac::olm::OlmMessage;
use vodozemac::Curve25519PublicKey;

use lattice_proto::message::{EncryptedPayload, EncryptionType};

use crate::olm::{OlmAccount, OlmSession};

/// Olm 编排错误。
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("invalid curve25519 key: {0}")]
    InvalidKey(String),
    #[error("no established session for peer")]
    NoSession,
    #[error("expected OLM payload, got encryption_type={0}")]
    WrongEncryptionType(i32),
    #[error("malformed olm message: {0}")]
    MalformedMessage(String),
    #[error("decrypt failed: {0}")]
    Decrypt(String),
    #[error("inbound session creation failed: {0}")]
    InboundSession(String),
    #[error("pickle error: {0}")]
    Pickle(String),
}

/// Olm 消息类型在 `message_index` 中的编码。
const OLM_TYPE_PREKEY: u32 = 0;
const OLM_TYPE_NORMAL: u32 = 1;

/// 用对端 curve25519 公钥(base64)作为会话索引键。
fn peer_key_id(identity_key: &Curve25519PublicKey) -> String {
    identity_key.to_base64()
}

fn parse_curve_key(bytes: &[u8]) -> Result<Curve25519PublicKey, SessionError> {
    Curve25519PublicKey::from_slice(bytes).map_err(|e| SessionError::InvalidKey(e.to_string()))
}

/// 1v1 Olm 会话管理器:一个本地账户 + 多个对端会话。
///
/// 会话按"对端 identity key"索引(每个对端设备一个会话)。多设备场景下,
/// 上层对每个设备的 identity key 各调用一次本管理器即可。
pub struct OlmSessionManager {
    account: OlmAccount,
    /// 对端 identity key(base64)→ 已建立的 Olm 会话。
    sessions: HashMap<String, OlmSession>,
}

impl OlmSessionManager {
    /// 用已有账户创建管理器(账户负责本设备的 Olm 身份/预密钥)。
    pub fn new(account: OlmAccount) -> Self {
        Self {
            account,
            sessions: HashMap::new(),
        }
    }

    /// 本设备的 curve25519 identity key 字节(分发给对端用)。
    pub fn identity_key_bytes(&self) -> [u8; 32] {
        self.account.curve25519_key().to_bytes()
    }

    /// 是否已与该对端(identity key 字节)建立会话。
    pub fn has_session(&self, peer_identity_key: &[u8]) -> Result<bool, SessionError> {
        let key = peer_key_id(&parse_curve_key(peer_identity_key)?);
        Ok(self.sessions.contains_key(&key))
    }

    /// 从 claim 到的对端预密钥建立出站会话(发起方)。
    ///
    /// `peer_identity_key` / `peer_one_time_key` 是对端 curve25519 公钥字节
    /// (来自 `GET /keys/claim` 的 base64 解码)。
    pub fn establish_outbound(
        &mut self,
        peer_identity_key: &[u8],
        peer_one_time_key: &[u8],
    ) -> Result<(), SessionError> {
        let identity = parse_curve_key(peer_identity_key)?;
        let otk = parse_curve_key(peer_one_time_key)?;
        let session = self.account.create_outbound_session(identity, otk);
        self.sessions.insert(peer_key_id(&identity), session);
        Ok(())
    }

    /// 加密明文为 `EncryptedPayload`(需先 `establish_outbound` 或已有入站会话)。
    pub fn encrypt(
        &mut self,
        peer_identity_key: &[u8],
        plaintext: &[u8],
    ) -> Result<EncryptedPayload, SessionError> {
        let key = peer_key_id(&parse_curve_key(peer_identity_key)?);
        let session = self.sessions.get_mut(&key).ok_or(SessionError::NoSession)?;
        let msg = session.encrypt(plaintext);
        let (msg_type, ciphertext) = msg.to_parts();
        let message_index = if msg_type == 0 {
            OLM_TYPE_PREKEY
        } else {
            OLM_TYPE_NORMAL
        };
        Ok(EncryptedPayload {
            ciphertext,
            session_id: session.session_id(),
            message_index,
            encryption_type: EncryptionType::Olm as i32,
        })
    }

    /// 解密 `EncryptedPayload` 回明文。
    ///
    /// 首条 PreKey 消息且本地无会话时,自动 `create_inbound_session` 建立入站会话。
    /// `peer_identity_key` 是发送方 curve25519 公钥字节(签名层已验证发送方身份)。
    pub fn decrypt(
        &mut self,
        peer_identity_key: &[u8],
        payload: &EncryptedPayload,
    ) -> Result<Vec<u8>, SessionError> {
        if payload.encryption_type != EncryptionType::Olm as i32 {
            return Err(SessionError::WrongEncryptionType(payload.encryption_type));
        }
        let identity = parse_curve_key(peer_identity_key)?;
        let key = peer_key_id(&identity);
        let msg_type = if payload.message_index == OLM_TYPE_PREKEY {
            0usize
        } else {
            1usize
        };
        let olm_msg = OlmMessage::from_parts(msg_type, &payload.ciphertext)
            .map_err(|e| SessionError::MalformedMessage(e.to_string()))?;

        // 已有会话:直接解密。
        if let Some(session) = self.sessions.get_mut(&key) {
            return session
                .decrypt(&olm_msg)
                .map_err(|e| SessionError::Decrypt(e.to_string()));
        }

        // 无会话:仅 PreKey 消息可建立入站会话。
        match olm_msg {
            OlmMessage::PreKey(pre_key) => {
                let (session, plaintext) = self
                    .account
                    .create_inbound_session(identity, &pre_key)
                    .map_err(|e| SessionError::InboundSession(e.to_string()))?;
                self.sessions.insert(key, session);
                Ok(plaintext)
            }
            OlmMessage::Normal(_) => Err(SessionError::NoSession),
        }
    }

    /// 持久化:导出账户 pickle + 每个会话 pickle(对端 key → 会话 pickle)。
    ///
    /// 调用方把结果存到本地安全存储,重启后用 [`from_pickles`] 恢复。
    pub fn to_pickles(&self, pickle_key: &[u8; 32]) -> (String, HashMap<String, String>) {
        let account = self.account.pickle(pickle_key);
        let sessions = self
            .sessions
            .iter()
            .map(|(k, s)| (k.clone(), s.pickle()))
            .collect();
        (account, sessions)
    }

    /// 从 pickle 恢复管理器。
    pub fn from_pickles(
        account_pickle: &str,
        session_pickles: &HashMap<String, String>,
    ) -> Result<Self, SessionError> {
        let account = OlmAccount::from_pickle(account_pickle).map_err(SessionError::Pickle)?;
        let mut sessions = HashMap::with_capacity(session_pickles.len());
        for (k, v) in session_pickles {
            let session = OlmSession::from_pickle(v).map_err(SessionError::Pickle)?;
            sessions.insert(k.clone(), session);
        }
        Ok(Self { account, sessions })
    }

    /// 生成并标记发布 one-time keys(供对端 claim);返回公钥字节列表。
    pub fn publish_one_time_keys(&mut self, count: usize) -> Vec<[u8; 32]> {
        self.account.generate_one_time_keys(count);
        let keys: Vec<[u8; 32]> = self
            .account
            .one_time_keys()
            .iter()
            .map(|k| k.to_bytes())
            .collect();
        self.account.mark_keys_as_published();
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 端到端:Alice 建出站 → 加密 → Bob 解密(自动建入站)→ Bob 回复 → Alice 解密。
    #[test]
    fn olm_1v1_roundtrip_via_payload() {
        let alice = OlmAccount::new();
        let mut bob_mgr = OlmSessionManager::new(OlmAccount::new());
        let bob_otks = bob_mgr.publish_one_time_keys(1);
        let bob_identity = bob_mgr.identity_key_bytes();

        let mut alice_mgr = OlmSessionManager::new(alice);
        alice_mgr
            .establish_outbound(&bob_identity, &bob_otks[0])
            .unwrap();

        // Alice 加密 → EncryptedPayload。
        let payload = alice_mgr.encrypt(&bob_identity, b"hello bob").unwrap();
        assert_eq!(payload.encryption_type, EncryptionType::Olm as i32);
        assert_eq!(payload.message_index, OLM_TYPE_PREKEY); // 首条是 PreKey
        assert!(!payload.session_id.is_empty());

        // Bob 解密(无会话 → 自动建入站)。
        let alice_identity = alice_mgr.identity_key_bytes();
        let decrypted = bob_mgr.decrypt(&alice_identity, &payload).unwrap();
        assert_eq!(decrypted, b"hello bob");
        assert!(bob_mgr.has_session(&alice_identity).unwrap());

        // Bob 回复(已有会话 → Normal 消息)。
        let reply = bob_mgr.encrypt(&alice_identity, b"hi alice").unwrap();
        assert_eq!(reply.message_index, OLM_TYPE_NORMAL);
        let reply_plain = alice_mgr.decrypt(&bob_identity, &reply).unwrap();
        assert_eq!(reply_plain, b"hi alice");
    }

    #[test]
    fn encrypt_without_session_fails() {
        let mut mgr = OlmSessionManager::new(OlmAccount::new());
        let bogus = [7u8; 32];
        let err = mgr.encrypt(&bogus, b"x").unwrap_err();
        assert!(matches!(err, SessionError::NoSession));
    }

    #[test]
    fn decrypt_normal_without_session_fails() {
        // 构造一个 Normal(message_index=1)但无会话 → NoSession。
        let mut mgr = OlmSessionManager::new(OlmAccount::new());
        let peer = OlmAccount::new();
        let peer_identity = peer.curve25519_key().to_bytes();
        let payload = EncryptedPayload {
            ciphertext: vec![1, 2, 3],
            session_id: "x".into(),
            message_index: OLM_TYPE_NORMAL,
            encryption_type: EncryptionType::Olm as i32,
        };
        let err = mgr.decrypt(&peer_identity, &payload).unwrap_err();
        // 无会话的 Normal:可能先在 from_parts 处失败(畸形),也可能 NoSession;
        // 两者都表示"无法解密",断言为这两类之一。
        assert!(matches!(
            err,
            SessionError::NoSession | SessionError::MalformedMessage(_)
        ));
    }

    #[test]
    fn wrong_encryption_type_rejected() {
        let mut mgr = OlmSessionManager::new(OlmAccount::new());
        let peer = [3u8; 32];
        let payload = EncryptedPayload {
            ciphertext: vec![],
            session_id: String::new(),
            message_index: 0,
            encryption_type: EncryptionType::Megolm as i32,
        };
        let err = mgr.decrypt(&peer, &payload).unwrap_err();
        assert!(matches!(err, SessionError::WrongEncryptionType(_)));
    }

    #[test]
    fn pickle_roundtrip_preserves_sessions() {
        let mut bob_mgr = OlmSessionManager::new(OlmAccount::new());
        let bob_otks = bob_mgr.publish_one_time_keys(1);
        let bob_identity = bob_mgr.identity_key_bytes();

        let mut alice_mgr = OlmSessionManager::new(OlmAccount::new());
        alice_mgr
            .establish_outbound(&bob_identity, &bob_otks[0])
            .unwrap();
        let payload = alice_mgr.encrypt(&bob_identity, b"first").unwrap();
        // Bob 先解密,建立入站会话。
        let alice_identity = alice_mgr.identity_key_bytes();
        bob_mgr.decrypt(&alice_identity, &payload).unwrap();

        // 持久化并恢复 Bob。
        let key = [9u8; 32];
        let (acc, sessions) = bob_mgr.to_pickles(&key);
        let mut restored = OlmSessionManager::from_pickles(&acc, &sessions).unwrap();
        assert!(restored.has_session(&alice_identity).unwrap());

        // 恢复后仍能继续解密 Alice 的后续消息。
        let next = alice_mgr.encrypt(&bob_identity, b"second").unwrap();
        let plain = restored.decrypt(&alice_identity, &next).unwrap();
        assert_eq!(plain, b"second");
    }

    #[test]
    fn invalid_peer_key_rejected() {
        let mut mgr = OlmSessionManager::new(OlmAccount::new());
        let too_short = [0u8; 10];
        let err = mgr.establish_outbound(&too_short, &too_short).unwrap_err();
        assert!(matches!(err, SessionError::InvalidKey(_)));
    }
}
