//! Lattice 会话编排层(可插拔加密实现):基于 vodozemac 的 Olm(1v1)/ Megolm(群组)
//! 会话管理与 `EncryptedPayload` 编解码。基础密码学(身份/签名/信任)在 `lattice-crypto`。

pub mod group_session;
pub mod key_distribution;
pub mod megolm;
pub mod olm;
pub mod session;
