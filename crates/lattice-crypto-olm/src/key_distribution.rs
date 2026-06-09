//! E4b:群密钥分发编排 —— 把 Megolm `session_key` 经 Olm 1v1 加密分发给成员。
//!
//! 组合 [`crate::session::OlmSessionManager`](1v1 Olm)与
//! [`crate::group_session::MegolmSessionManager`](群组 Megolm):
//! - 群主:`MegolmSessionManager::create_outbound` 得 `(session_id, session_key)`,
//!   再对每个成员设备用 Olm 加密 `RoomKey{room_id, session_key}`,产出 `EncryptedPayload(OLM)`。
//! - 成员:Olm 解密得到 `RoomKey` 字节 → `decode` → `MegolmSessionManager::accept_inbound`。
//!
//! session_key 经 Olm 加密后就是普通的 `EncryptedPayload(OLM)`,可走现有消息通道转发,
//! 无需新增 proto/frame 类型;Store 仍只转发密文、看不到明文 session_key。
//!
//! ## RoomKey 线格式(长度前缀,无外部依赖)
//! `u32_be(room_id.len()) ‖ room_id ‖ u32_be(session_key.len()) ‖ session_key`

use crate::session::{OlmSessionManager, SessionError};
use lattice_proto::message::EncryptedPayload;

/// 分发给成员的房间密钥载荷:房间 id + Megolm session_key 字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomKey {
    pub room_id: String,
    pub session_key: Vec<u8>,
}

/// 密钥分发错误。
#[derive(Debug, thiserror::Error)]
pub enum KeyDistributionError {
    #[error("olm session error: {0}")]
    Session(#[from] SessionError),
    #[error("malformed RoomKey payload: {0}")]
    Malformed(String),
}

impl RoomKey {
    /// 序列化为长度前缀字节(作为 Olm 加密的明文输入)。
    pub fn encode(&self) -> Vec<u8> {
        let rid = self.room_id.as_bytes();
        let mut out = Vec::with_capacity(8 + rid.len() + self.session_key.len());
        out.extend_from_slice(&(rid.len() as u32).to_be_bytes());
        out.extend_from_slice(rid);
        out.extend_from_slice(&(self.session_key.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.session_key);
        out
    }

    /// 从长度前缀字节反序列化(Olm 解密后的明文)。
    pub fn decode(bytes: &[u8]) -> Result<Self, KeyDistributionError> {
        let malformed = |m: &str| KeyDistributionError::Malformed(m.to_string());
        let read_u32 = |b: &[u8], at: usize| -> Result<usize, KeyDistributionError> {
            let end = at
                .checked_add(4)
                .filter(|&e| e <= b.len())
                .ok_or_else(|| malformed("truncated length prefix"))?;
            let mut arr = [0u8; 4];
            arr.copy_from_slice(&b[at..end]);
            Ok(u32::from_be_bytes(arr) as usize)
        };

        let rid_len = read_u32(bytes, 0)?;
        let rid_start = 4usize;
        let rid_end = rid_start
            .checked_add(rid_len)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| malformed("room_id out of bounds"))?;
        let room_id = String::from_utf8(bytes[rid_start..rid_end].to_vec())
            .map_err(|_| malformed("room_id not utf-8"))?;

        let key_len = read_u32(bytes, rid_end)?;
        let key_start = rid_end + 4;
        let key_end = key_start
            .checked_add(key_len)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| malformed("session_key out of bounds"))?;
        let session_key = bytes[key_start..key_end].to_vec();

        Ok(RoomKey {
            room_id,
            session_key,
        })
    }
}

/// 群主侧:对单个成员设备加密房间密钥,产出可经普通通道转发的 `EncryptedPayload(OLM)`。
///
/// `olm` 为群主与该成员设备的 Olm 会话管理器(需已 `establish_outbound`)。
/// `member_identity_key` 是成员设备的 curve25519 公钥字节。
pub fn encrypt_room_key(
    olm: &mut OlmSessionManager,
    member_identity_key: &[u8],
    room_key: &RoomKey,
) -> Result<EncryptedPayload, KeyDistributionError> {
    let plaintext = room_key.encode();
    Ok(olm.encrypt(member_identity_key, &plaintext)?)
}

/// 成员侧:Olm 解密收到的密钥分发载荷,还原 `RoomKey`。
///
/// `sender_identity_key` 是群主设备的 curve25519 公钥字节(首条 PreKey 会自动建入站会话)。
pub fn decrypt_room_key(
    olm: &mut OlmSessionManager,
    sender_identity_key: &[u8],
    payload: &EncryptedPayload,
) -> Result<RoomKey, KeyDistributionError> {
    let plaintext = olm.decrypt(sender_identity_key, payload)?;
    RoomKey::decode(&plaintext)
}

/// 群主侧:把房间密钥分发给多个设备(多设备 / 多成员通用)。
///
/// `device_identity_keys` 是当前所有目标设备的 curve25519 公钥字节(每个用户的每台设备一项);
/// 每项都必须已 `establish_outbound`。返回 `(设备 identity_key, 该设备的 OLM 密文)` 列表,
/// 由调用方分别投递。轮换后只对"当前成员设备"调用本函数,被移除者自然收不到新 key。
pub fn distribute_to_devices(
    olm: &mut OlmSessionManager,
    device_identity_keys: &[Vec<u8>],
    room_key: &RoomKey,
) -> Result<Vec<(Vec<u8>, EncryptedPayload)>, KeyDistributionError> {
    let mut out = Vec::with_capacity(device_identity_keys.len());
    for dev in device_identity_keys {
        let payload = encrypt_room_key(olm, dev, room_key)?;
        out.push((dev.clone(), payload));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_session::MegolmSessionManager;
    use crate::olm::OlmAccount;

    #[test]
    fn room_key_encode_decode_roundtrip() {
        let rk = RoomKey {
            room_id: "room-中文-42".to_string(),
            session_key: vec![1, 2, 3, 250, 0, 99],
        };
        let decoded = RoomKey::decode(&rk.encode()).unwrap();
        assert_eq!(decoded, rk);
    }

    #[test]
    fn decode_rejects_truncated() {
        assert!(matches!(
            RoomKey::decode(&[0, 0, 0, 5, 1, 2]),
            Err(KeyDistributionError::Malformed(_))
        ));
        assert!(matches!(
            RoomKey::decode(&[]),
            Err(KeyDistributionError::Malformed(_))
        ));
    }

    /// 完整群组 E2EE 闭环:群主建群密钥 → Olm 分发 → 成员收 key → Megolm 群消息收发。
    #[test]
    fn group_e2ee_full_loop_via_key_distribution() {
        let room = "group-room";

        // 群主与成员各有 Olm 账户;成员发布预密钥供群主建立出站 Olm 会话。
        let mut owner_olm = OlmSessionManager::new(OlmAccount::new());
        let mut member_olm = OlmSessionManager::new(OlmAccount::new());
        let member_otks = member_olm.publish_one_time_keys(1);
        let member_id = member_olm.identity_key_bytes();
        owner_olm
            .establish_outbound(&member_id, &member_otks[0])
            .unwrap();

        // 群主建 Megolm 出站会话,拿到 session_key。
        let mut owner_group = MegolmSessionManager::new();
        let (session_id, session_key) = owner_group.create_outbound(room);

        // 群主把 session_key 经 Olm 加密分发(产出 OLM 密文,可走普通通道)。
        let rk = RoomKey {
            room_id: room.to_string(),
            session_key,
        };
        let dist_payload = encrypt_room_key(&mut owner_olm, &member_id, &rk).unwrap();
        assert_ne!(dist_payload.ciphertext, rk.encode()); // 确为密文

        // 成员 Olm 解密还原 RoomKey,建立 Megolm 入站会话。
        let owner_id = owner_olm.identity_key_bytes();
        let mut member_group = MegolmSessionManager::new();
        let recovered = decrypt_room_key(&mut member_olm, &owner_id, &dist_payload).unwrap();
        assert_eq!(recovered.room_id, room);
        let accepted_id = member_group.accept_inbound(&recovered.session_key).unwrap();
        assert_eq!(accepted_id, session_id);

        // 群主用 Megolm 加密群消息,成员解密还原 —— 完整闭环。
        let ct = owner_group.encrypt(room, b"hello group").unwrap();
        assert_eq!(member_group.decrypt(&ct).unwrap(), b"hello group");
        let ct2 = owner_group.encrypt(room, b"second").unwrap();
        assert_eq!(member_group.decrypt(&ct2).unwrap(), b"second");
    }

    /// 多成员:同一 session_key 分发给两个成员设备,两者都能解群消息。
    #[test]
    fn distributes_to_multiple_members() {
        let room = "multi";
        let mut owner_group = MegolmSessionManager::new();
        let (_sid, session_key) = owner_group.create_outbound(room);
        let rk = RoomKey {
            room_id: room.to_string(),
            session_key,
        };

        let mut owner_olm = OlmSessionManager::new(OlmAccount::new());
        let owner_id = owner_olm.identity_key_bytes();

        // 两个成员各自建会话、收 key。
        let mut members = Vec::new();
        for _ in 0..2 {
            let mut m_olm = OlmSessionManager::new(OlmAccount::new());
            let otks = m_olm.publish_one_time_keys(1);
            let m_id = m_olm.identity_key_bytes();
            owner_olm.establish_outbound(&m_id, &otks[0]).unwrap();
            let payload = encrypt_room_key(&mut owner_olm, &m_id, &rk).unwrap();

            let mut m_group = MegolmSessionManager::new();
            let got = decrypt_room_key(&mut m_olm, &owner_id, &payload).unwrap();
            m_group.accept_inbound(&got.session_key).unwrap();
            members.push(m_group);
        }

        let ct = owner_group.encrypt(room, b"broadcast").unwrap();
        for m in members.iter_mut() {
            assert_eq!(m.decrypt(&ct).unwrap(), b"broadcast");
        }
    }

    /// E5 前向保密:轮换后,持旧 session 的成员无法解密新 session 的消息。
    #[test]
    fn rotation_locks_out_old_session() {
        let room = "rot";
        let mut owner_group = MegolmSessionManager::new();
        let (_sid, key_v1) = owner_group.create_outbound(room);

        // 成员用 v1 session_key 建入站会话,能解 v1 消息。
        let mut member = MegolmSessionManager::new();
        member.accept_inbound(&key_v1).unwrap();
        let c1 = owner_group.encrypt(room, b"before rotation").unwrap();
        assert_eq!(member.decrypt(&c1).unwrap(), b"before rotation");

        // 群主轮换(踢人场景):新 session_key,且不再分发给该成员。
        let (_sid2, _key_v2) = owner_group.rotate(room);
        let c2 = owner_group.encrypt(room, b"after rotation").unwrap();

        // 旧成员持 v1,解不了 v2 消息(新 session_id 无对应入站会话)。
        assert!(member.decrypt(&c2).is_err());
    }

    /// E5 多设备:distribute_to_devices 批量分发,所有设备都能解群消息。
    #[test]
    fn distribute_to_devices_reaches_all() {
        let room = "multidev";
        let mut owner_group = MegolmSessionManager::new();
        let (_sid, session_key) = owner_group.create_outbound(room);
        let rk = RoomKey {
            room_id: room.to_string(),
            session_key,
        };

        let mut owner_olm = OlmSessionManager::new(OlmAccount::new());
        let owner_id = owner_olm.identity_key_bytes();

        // 三台设备:各发布预密钥,群主对每台建出站 Olm 会话。
        let mut devices = Vec::new();
        let mut dev_ids = Vec::new();
        for _ in 0..3 {
            let mut d_olm = OlmSessionManager::new(OlmAccount::new());
            let otks = d_olm.publish_one_time_keys(1);
            let d_id = d_olm.identity_key_bytes();
            owner_olm.establish_outbound(&d_id, &otks[0]).unwrap();
            dev_ids.push(d_id.to_vec());
            devices.push(d_olm);
        }

        // 一次批量分发到三台设备。
        let payloads = distribute_to_devices(&mut owner_olm, &dev_ids, &rk).unwrap();
        assert_eq!(payloads.len(), 3);

        // 每台设备 Olm 解密 → 建 Megolm 入站 → 解群消息。
        let ct = owner_group.encrypt(room, b"to all devices").unwrap();
        for (i, d_olm) in devices.iter_mut().enumerate() {
            let (_dev_id, payload) = &payloads[i];
            let got = decrypt_room_key(d_olm, &owner_id, payload).unwrap();
            let mut d_group = MegolmSessionManager::new();
            d_group.accept_inbound(&got.session_key).unwrap();
            assert_eq!(d_group.decrypt(&ct).unwrap(), b"to all devices");
        }
    }
}
