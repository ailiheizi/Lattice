//! Lattice 会话编排层(可插拔加密实现):基于 vodozemac 的 Olm(1v1)/ Megolm(群组)
//! 会话管理与 `EncryptedPayload` 编解码。基础密码学(身份/签名/信任)在 `lattice-crypto`。

pub mod group_session;
pub mod key_distribution;
pub mod megolm;
pub mod olm;
pub mod session;

use lattice_proto::message::CryptoSuite;

/// 本二进制当前使用的加密套件标识,写入所有新产出的 `EncryptedPayload`。
/// crypto agility:将来引入新套件时改这里,旧消息仍按其自带 `crypto_suite` 解密。
pub const CURRENT_CRYPTO_SUITE: i32 = CryptoSuite::SuiteOlmV1MegolmV1 as i32;

/// 解密前校验密文声明的套件本端是否支持。未知套件明确拒绝,而非误用当前算法解错。
pub fn is_supported_suite(suite: i32) -> bool {
    suite == CryptoSuite::SuiteOlmV1MegolmV1 as i32
}
