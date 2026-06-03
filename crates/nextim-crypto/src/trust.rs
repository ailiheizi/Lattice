use nextim_proto::identity::TrustLevel;

/// 信任策略 — 决定是否接收消息
pub struct TrustPolicy {
    /// 默认信任等级
    pub default_level: TrustLevel,
}

impl TrustPolicy {
    pub fn strict() -> Self {
        Self { default_level: TrustLevel::Verified }
    }

    pub fn relaxed() -> Self {
        Self { default_level: TrustLevel::Tofu }
    }

    pub fn open() -> Self {
        Self { default_level: TrustLevel::Public }
    }

    /// 判断是否接受来自指定信任等级的消息
    ///
    /// 规则：消息的信任等级 >= 策略要求的等级才接受
    /// Verified(2) > TOFU(1) > Public(0)
    pub fn should_accept(&self, sender_trust: TrustLevel) -> bool {
        trust_level_value(sender_trust) >= trust_level_value(self.default_level)
    }

    /// 判断消息是否需要签名验证
    ///
    /// Public 模式不要求签名，其他模式都要求
    pub fn requires_signature(&self) -> bool {
        self.default_level != TrustLevel::Public
    }

    /// 判断是否需要已验证的公钥
    pub fn requires_verified_key(&self) -> bool {
        self.default_level == TrustLevel::Verified
    }
}

fn trust_level_value(level: TrustLevel) -> i32 {
    match level {
        TrustLevel::Public => 0,
        TrustLevel::Tofu => 1,
        TrustLevel::Verified => 2,
    }
}

/// 评估发送者的信任等级
pub fn evaluate_trust(
    _sender_fingerprint: &str,
    signature_valid: bool,
    key_verified: bool,
) -> TrustLevel {
    if key_verified && signature_valid {
        TrustLevel::Verified
    } else if signature_valid {
        TrustLevel::Tofu
    } else {
        TrustLevel::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_policy() {
        let policy = TrustPolicy::strict();
        assert!(!policy.should_accept(TrustLevel::Public));
        assert!(!policy.should_accept(TrustLevel::Tofu));
        assert!(policy.should_accept(TrustLevel::Verified));
        assert!(policy.requires_signature());
        assert!(policy.requires_verified_key());
    }

    #[test]
    fn test_relaxed_policy() {
        let policy = TrustPolicy::relaxed();
        assert!(!policy.should_accept(TrustLevel::Public));
        assert!(policy.should_accept(TrustLevel::Tofu));
        assert!(policy.should_accept(TrustLevel::Verified));
        assert!(policy.requires_signature());
        assert!(!policy.requires_verified_key());
    }

    #[test]
    fn test_open_policy() {
        let policy = TrustPolicy::open();
        assert!(policy.should_accept(TrustLevel::Public));
        assert!(policy.should_accept(TrustLevel::Tofu));
        assert!(policy.should_accept(TrustLevel::Verified));
        assert!(!policy.requires_signature());
    }

    #[test]
    fn test_evaluate_trust() {
        assert_eq!(
            evaluate_trust("abc", true, true) as i32,
            TrustLevel::Verified as i32
        );
        assert_eq!(
            evaluate_trust("abc", true, false) as i32,
            TrustLevel::Tofu as i32
        );
        assert_eq!(
            evaluate_trust("abc", false, false) as i32,
            TrustLevel::Public as i32
        );
    }
}
