//! 每发件人滑动窗口限流(防轰炸)。
//!
//! 纯逻辑、sans-I/O:时间由调用方传入(便于测试,也避免在算法里读时钟)。
//! 准入控制(require_contact)挡的是陌生人;限流挡的是已加好友后的刷屏。

use std::collections::HashMap;

/// 滑动窗口限流器:记录每个发送者在窗口内的请求时间戳。
pub struct RateLimiter {
    /// 窗口长度(毫秒)。
    window_ms: u64,
    /// 窗口内允许的最大请求数。0 表示不限流。
    max_in_window: u32,
    /// 每个发送者的请求时间戳(毫秒),已按时间升序。
    hits: HashMap<String, Vec<u64>>,
}

impl RateLimiter {
    /// max_in_window=0 表示不限流(check 永远放行)。
    pub fn new(window_ms: u64, max_in_window: u32) -> Self {
        Self {
            window_ms,
            max_in_window,
            hits: HashMap::new(),
        }
    }

    /// 记录一次来自 sender 的请求并判断是否允许。
    /// 返回 true=允许,false=超限应拒绝。now_ms 由调用方传入当前时间。
    pub fn check_and_record(&mut self, sender: &str, now_ms: u64) -> bool {
        if self.max_in_window == 0 {
            return true; // 不限流
        }
        let window_start = now_ms.saturating_sub(self.window_ms);
        let entry = self.hits.entry(sender.to_string()).or_default();
        // 移除窗口外的旧时间戳
        entry.retain(|&t| t >= window_start);
        if entry.len() as u32 >= self.max_in_window {
            return false; // 超限,不记录本次
        }
        entry.push(now_ms);
        true
    }

    /// 清理所有发送者中已完全过期的条目(可周期调用以释放内存)。
    pub fn evict_expired(&mut self, now_ms: u64) {
        let window_start = now_ms.saturating_sub(self.window_ms);
        for v in self.hits.values_mut() {
            v.retain(|&t| t >= window_start);
        }
        self.hits.retain(|_, v| !v.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_max_never_limits() {
        let mut rl = RateLimiter::new(60_000, 0);
        for i in 0..1000 {
            assert!(rl.check_and_record("a", i));
        }
    }

    #[test]
    fn rejects_after_max_in_window() {
        let mut rl = RateLimiter::new(1000, 3);
        assert!(rl.check_and_record("a", 0));
        assert!(rl.check_and_record("a", 100));
        assert!(rl.check_and_record("a", 200));
        // 第 4 条在窗口内 → 拒绝
        assert!(!rl.check_and_record("a", 300));
    }

    #[test]
    fn window_slides_allows_after_expiry() {
        let mut rl = RateLimiter::new(1000, 2);
        assert!(rl.check_and_record("a", 0));
        assert!(rl.check_and_record("a", 500));
        assert!(!rl.check_and_record("a", 900)); // 窗口内第3条被拒
                                                 // 时间前进到窗口外,旧的两条过期 → 放行
        assert!(rl.check_and_record("a", 1600));
    }

    #[test]
    fn per_sender_isolated() {
        let mut rl = RateLimiter::new(1000, 1);
        assert!(rl.check_and_record("a", 0));
        assert!(!rl.check_and_record("a", 100)); // a 超限
        assert!(rl.check_and_record("b", 100)); // b 独立计数,放行
    }

    #[test]
    fn evict_expired_frees_entries() {
        let mut rl = RateLimiter::new(1000, 5);
        rl.check_and_record("a", 0);
        rl.evict_expired(2000); // a 的记录已过期
                                // 过期后重新计数,a 可立即再发
        assert!(rl.check_and_record("a", 2000));
    }
}
