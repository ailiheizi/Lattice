//! Megolm 群会话编排(运行时核心,sans-I/O)。
//!
//! 把 `megolm` 原语接成"房间 → 群会话"的编排层:
//! - 发送方:每房间一个出站会话,加密群消息为 `EncryptedPayload(MEGOLM)`,
//!   并导出 `session_key` 字节供上层经 Olm 1v1 分发给成员设备。
//! - 接收方:收到(经 Olm 解密出的)session_key 后建入站会话,按 session_id 解密群消息。
//!
//! session_key 如何经 Olm 加密分发(KeyDistribution)由上层(E4b)编排,本模块只管群密钥与群消息。
//! 防重放由 vodozemac 入站会话内建(拒绝重复 message_index)。
//!
//! ## EncryptedPayload 字段映射(MEGOLM)
//! - `ciphertext`:Megolm 密文字节(`MegolmMessage::to_bytes`)。
//! - `session_id`:Megolm 会话 id(接收方据此选入站会话)。
//! - `message_index`:Megolm 消息序号(发送方当前 index)。
//! - `encryption_type`:固定 `EncryptionType::Megolm`。

use std::collections::HashMap;

use vodozemac::megolm::{MegolmMessage, SessionKey};

use lattice_proto::message::{EncryptedPayload, EncryptionType};

use crate::megolm::{MegolmInboundSession, MegolmOutboundSession};

/// Megolm 编排错误。
#[derive(Debug, thiserror::Error)]
pub enum GroupSessionError {
    #[error("no outbound session for room")]
    NoOutboundSession,
    #[error("no inbound session for session_id")]
    NoInboundSession,
    #[error("expected MEGOLM payload, got encryption_type={0}")]
    WrongEncryptionType(i32),
    #[error("malformed megolm message: {0}")]
    MalformedMessage(String),
    #[error("invalid session key: {0}")]
    InvalidSessionKey(String),
    #[error("decrypt failed: {0}")]
    Decrypt(String),
    #[error("pickle error: {0}")]
    Pickle(String),
}

/// Megolm 群会话管理器:发送方按 room 持出站会话,接收方按 session_id 持入站会话。
///
/// 同一个管理器可同时充当某些房间的发送方与另一些房间的接收方。
#[derive(Default)]
pub struct MegolmSessionManager {
    /// room_id → 出站会话(本设备作为发送方)。
    outbound: HashMap<String, MegolmOutboundSession>,
    /// session_id → 入站会话(本设备作为接收方)。
    inbound: HashMap<String, MegolmInboundSession>,
}

impl MegolmSessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 为房间创建(或重置)出站会话,返回 (session_id, session_key 字节)。
    ///
    /// session_key 字节供上层经 Olm 1v1 加密分发给每个成员设备
    /// (成员设备解密后调用 [`accept_inbound`])。重置等同轮换(E5)。
    pub fn create_outbound(&mut self, room_id: &str) -> (String, Vec<u8>) {
        let session = MegolmOutboundSession::new();
        let session_id = session.session_id();
        let key_bytes = session.session_key().to_bytes();
        self.outbound.insert(room_id.to_string(), session);
        (session_id, key_bytes)
    }

    /// 房间是否已有出站会话。
    pub fn has_outbound(&self, room_id: &str) -> bool {
        self.outbound.contains_key(room_id)
    }

    /// 用房间出站会话加密群消息为 `EncryptedPayload(MEGOLM)`。
    pub fn encrypt(
        &mut self,
        room_id: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedPayload, GroupSessionError> {
        let session = self
            .outbound
            .get_mut(room_id)
            .ok_or(GroupSessionError::NoOutboundSession)?;
        let message_index = session.message_index();
        let msg = session.encrypt(plaintext);
        Ok(EncryptedPayload {
            ciphertext: msg.to_bytes(),
            session_id: session.session_id(),
            message_index,
            encryption_type: EncryptionType::Megolm as i32,
        })
    }

    /// 接收(经 Olm 解密出的)session_key 字节,建立入站会话。
    ///
    /// 重复接收同一 session 是幂等的(覆盖),便于多设备/重发场景。
    pub fn accept_inbound(
        &mut self,
        session_key_bytes: &[u8],
    ) -> Result<String, GroupSessionError> {
        let key = SessionKey::from_bytes(session_key_bytes)
            .map_err(|e| GroupSessionError::InvalidSessionKey(e.to_string()))?;
        let session = MegolmInboundSession::new(&key);
        let session_id = session.session_id();
        self.inbound.insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// 是否已有该 session_id 的入站会话。
    pub fn has_inbound(&self, session_id: &str) -> bool {
        self.inbound.contains_key(session_id)
    }

    /// 解密 `EncryptedPayload(MEGOLM)` 回明文(需先 [`accept_inbound`])。
    ///
    /// 防重放由 vodozemac 内建:重复 message_index 会被入站会话拒绝。
    pub fn decrypt(&mut self, payload: &EncryptedPayload) -> Result<Vec<u8>, GroupSessionError> {
        if payload.encryption_type != EncryptionType::Megolm as i32 {
            return Err(GroupSessionError::WrongEncryptionType(
                payload.encryption_type,
            ));
        }
        let session = self
            .inbound
            .get_mut(&payload.session_id)
            .ok_or(GroupSessionError::NoInboundSession)?;
        let msg = MegolmMessage::from_bytes(&payload.ciphertext)
            .map_err(|e| GroupSessionError::MalformedMessage(e.to_string()))?;
        let decrypted = session
            .decrypt(&msg)
            .map_err(|e| GroupSessionError::Decrypt(e.to_string()))?;
        Ok(decrypted.plaintext)
    }

    /// 持久化:出站(room_id → pickle)+ 入站(session_id → pickle)。
    pub fn to_pickles(&self) -> (HashMap<String, String>, HashMap<String, String>) {
        let outbound = self
            .outbound
            .iter()
            .map(|(k, s)| (k.clone(), s.pickle()))
            .collect();
        let inbound = self
            .inbound
            .iter()
            .map(|(k, s)| (k.clone(), s.pickle()))
            .collect();
        (outbound, inbound)
    }

    /// 从 pickle 恢复管理器。
    pub fn from_pickles(
        outbound_pickles: &HashMap<String, String>,
        inbound_pickles: &HashMap<String, String>,
    ) -> Result<Self, GroupSessionError> {
        let mut outbound = HashMap::with_capacity(outbound_pickles.len());
        for (k, v) in outbound_pickles {
            outbound.insert(
                k.clone(),
                MegolmOutboundSession::from_pickle(v).map_err(GroupSessionError::Pickle)?,
            );
        }
        let mut inbound = HashMap::with_capacity(inbound_pickles.len());
        for (k, v) in inbound_pickles {
            inbound.insert(
                k.clone(),
                MegolmInboundSession::from_pickle(v).map_err(GroupSessionError::Pickle)?,
            );
        }
        Ok(Self { outbound, inbound })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 端到端:发送方建群会话 → 分发 key → 接收方建入站 → 加密/解密往返。
    #[test]
    fn megolm_group_roundtrip() {
        let room = "group-room";
        let mut sender = MegolmSessionManager::new();
        let mut receiver = MegolmSessionManager::new();

        let (session_id, key_bytes) = sender.create_outbound(room);
        assert!(sender.has_outbound(room));

        // 接收方收到(经 Olm 分发的)key_bytes → 建入站会话。
        let accepted_id = receiver.accept_inbound(&key_bytes).unwrap();
        assert_eq!(accepted_id, session_id);
        assert!(receiver.has_inbound(&session_id));

        // 发送方加密两条,接收方按序解密。
        let p1 = sender.encrypt(room, b"group hello").unwrap();
        assert_eq!(p1.encryption_type, EncryptionType::Megolm as i32);
        assert_eq!(p1.session_id, session_id);
        assert_eq!(receiver.decrypt(&p1).unwrap(), b"group hello");

        let p2 = sender.encrypt(room, b"second message").unwrap();
        assert_eq!(receiver.decrypt(&p2).unwrap(), b"second message");
    }

    #[test]
    fn encrypt_without_outbound_fails() {
        let mut mgr = MegolmSessionManager::new();
        let err = mgr.encrypt("nope", b"x").unwrap_err();
        assert!(matches!(err, GroupSessionError::NoOutboundSession));
    }

    #[test]
    fn decrypt_without_inbound_fails() {
        let mut sender = MegolmSessionManager::new();
        let mut other = MegolmSessionManager::new();
        let (_id, _key) = sender.create_outbound("r");
        let payload = sender.encrypt("r", b"hi").unwrap();
        // other 没有该 session 的入站会话。
        let err = other.decrypt(&payload).unwrap_err();
        assert!(matches!(err, GroupSessionError::NoInboundSession));
    }

    #[test]
    fn wrong_encryption_type_rejected() {
        let mut mgr = MegolmSessionManager::new();
        let payload = EncryptedPayload {
            ciphertext: vec![],
            session_id: "x".into(),
            message_index: 0,
            encryption_type: EncryptionType::Olm as i32,
        };
        let err = mgr.decrypt(&payload).unwrap_err();
        assert!(matches!(err, GroupSessionError::WrongEncryptionType(_)));
    }

    #[test]
    fn invalid_session_key_rejected() {
        let mut mgr = MegolmSessionManager::new();
        let err = mgr.accept_inbound(&[0u8; 4]).unwrap_err();
        assert!(matches!(err, GroupSessionError::InvalidSessionKey(_)));
    }

    #[test]
    fn pickle_roundtrip_preserves_sessions() {
        let room = "persist-room";
        let mut sender = MegolmSessionManager::new();
        let (session_id, key_bytes) = sender.create_outbound(room);
        let mut receiver = MegolmSessionManager::new();
        receiver.accept_inbound(&key_bytes).unwrap();

        // 持久化并恢复接收方。
        let (out, inb) = receiver.to_pickles();
        let mut restored = MegolmSessionManager::from_pickles(&out, &inb).unwrap();
        assert!(restored.has_inbound(&session_id));

        // 恢复后仍能解密发送方后续消息。
        let payload = sender.encrypt(room, b"after restore").unwrap();
        assert_eq!(restored.decrypt(&payload).unwrap(), b"after restore");
    }
}
