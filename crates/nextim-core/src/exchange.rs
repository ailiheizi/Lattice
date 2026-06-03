//! 信息交换端 — 管理消息接收策略

use std::collections::HashMap;

use nextim_proto::identity::TrustLevel;

/// 接收策略配置
pub struct ExchangePolicy {
    /// 全局默认信任等级
    pub default_trust: TrustLevel,
    /// 按联系人覆盖的信任等级
    overrides: HashMap<String, TrustLevel>,
    /// 黑名单（无论什么信任等级都拒绝）
    blocked: Vec<String>,
}

impl ExchangePolicy {
    pub fn new(default_trust: TrustLevel) -> Self {
        Self {
            default_trust,
            overrides: HashMap::new(),
            blocked: Vec::new(),
        }
    }

    /// 为指定联系人设置信任等级
    pub fn set_trust(&mut self, fingerprint: &str, level: TrustLevel) {
        self.overrides.insert(fingerprint.to_string(), level);
    }

    /// 移除联系人的信任覆盖（回退到默认）
    pub fn remove_trust(&mut self, fingerprint: &str) {
        self.overrides.remove(fingerprint);
    }

    /// 拉黑
    pub fn block(&mut self, fingerprint: &str) {
        if !self.blocked.contains(&fingerprint.to_string()) {
            self.blocked.push(fingerprint.to_string());
        }
    }

    /// 取消拉黑
    pub fn unblock(&mut self, fingerprint: &str) {
        self.blocked.retain(|f| f != fingerprint);
    }

    /// 获取指定联系人的有效信任等级
    pub fn effective_trust(&self, fingerprint: &str) -> TrustLevel {
        self.overrides
            .get(fingerprint)
            .copied()
            .unwrap_or(self.default_trust)
    }

    /// 判断是否接受来自指定发送者的消息
    pub fn should_accept(
        &self,
        sender_fingerprint: &str,
        signature_valid: bool,
        key_verified: bool,
    ) -> AcceptResult {
        // 黑名单直接拒绝
        if self.blocked.contains(&sender_fingerprint.to_string()) {
            return AcceptResult::Blocked;
        }

        let required = self.effective_trust(sender_fingerprint);
        let sender_level = evaluate_sender_trust(signature_valid, key_verified);

        if trust_value(sender_level) >= trust_value(required) {
            AcceptResult::Accept
        } else {
            AcceptResult::Rejected(required)
        }
    }
}

fn evaluate_sender_trust(signature_valid: bool, key_verified: bool) -> TrustLevel {
    if key_verified && signature_valid {
        TrustLevel::Verified
    } else if signature_valid {
        TrustLevel::Tofu
    } else {
        TrustLevel::Public
    }
}

fn trust_value(level: TrustLevel) -> i32 {
    match level {
        TrustLevel::Public => 0,
        TrustLevel::Tofu => 1,
        TrustLevel::Verified => 2,
    }
}

/// 消息接受结果
#[derive(Debug, PartialEq)]
pub enum AcceptResult {
    Accept,
    Blocked,
    Rejected(TrustLevel),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_public_accepts_all() {
        let policy = ExchangePolicy::new(TrustLevel::Public);
        assert_eq!(
            policy.should_accept("anyone", false, false),
            AcceptResult::Accept
        );
    }

    #[test]
    fn test_tofu_requires_signature() {
        let policy = ExchangePolicy::new(TrustLevel::Tofu);
        assert_eq!(
            policy.should_accept("sender", false, false),
            AcceptResult::Rejected(TrustLevel::Tofu)
        );
        assert_eq!(
            policy.should_accept("sender", true, false),
            AcceptResult::Accept
        );
    }

    #[test]
    fn test_verified_requires_both() {
        let policy = ExchangePolicy::new(TrustLevel::Verified);
        assert_eq!(
            policy.should_accept("sender", true, false),
            AcceptResult::Rejected(TrustLevel::Verified)
        );
        assert_eq!(
            policy.should_accept("sender", true, true),
            AcceptResult::Accept
        );
    }

    #[test]
    fn test_per_contact_override() {
        let mut policy = ExchangePolicy::new(TrustLevel::Verified);
        policy.set_trust("friend", TrustLevel::Tofu);

        // friend 只需要签名
        assert_eq!(
            policy.should_accept("friend", true, false),
            AcceptResult::Accept
        );
        // 其他人仍需要 verified
        assert_eq!(
            policy.should_accept("stranger", true, false),
            AcceptResult::Rejected(TrustLevel::Verified)
        );
    }

    #[test]
    fn test_block_overrides_everything() {
        let mut policy = ExchangePolicy::new(TrustLevel::Public);
        policy.block("spammer");

        assert_eq!(
            policy.should_accept("spammer", true, true),
            AcceptResult::Blocked
        );
    }

    #[test]
    fn test_unblock() {
        let mut policy = ExchangePolicy::new(TrustLevel::Public);
        policy.block("user");
        assert_eq!(policy.should_accept("user", true, true), AcceptResult::Blocked);

        policy.unblock("user");
        assert_eq!(policy.should_accept("user", true, true), AcceptResult::Accept);
    }

    #[test]
    fn test_remove_override() {
        let mut policy = ExchangePolicy::new(TrustLevel::Verified);
        policy.set_trust("friend", TrustLevel::Public);
        assert_eq!(
            policy.should_accept("friend", false, false),
            AcceptResult::Accept
        );

        policy.remove_trust("friend");
        assert_eq!(
            policy.should_accept("friend", false, false),
            AcceptResult::Rejected(TrustLevel::Verified)
        );
    }
}
