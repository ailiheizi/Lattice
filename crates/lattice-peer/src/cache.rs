//! 短时缓存 — TTL + 容量上限

use std::collections::VecDeque;

/// 缓存条目
struct CacheEntry {
    recipient_fingerprint: String,
    data: Vec<u8>,
    created_at: u64,
}

/// 短时内存缓存
pub struct RelayCache {
    entries: VecDeque<CacheEntry>,
    max_entries: usize,
    ttl_ms: u64,
}

impl RelayCache {
    pub fn new(max_entries: usize, ttl_ms: u64) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            ttl_ms,
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// 清理过期条目
    fn evict_expired(&mut self) {
        let now = Self::now_ms();
        while let Some(front) = self.entries.front() {
            if now - front.created_at > self.ttl_ms {
                self.entries.pop_front();
            } else {
                break;
            }
        }
    }

    /// 存入缓存
    pub fn store(&mut self, recipient_fingerprint: &str, data: Vec<u8>) {
        self.evict_expired();
        // 容量满了，丢弃最旧的
        while self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(CacheEntry {
            recipient_fingerprint: recipient_fingerprint.to_string(),
            data,
            created_at: Self::now_ms(),
        });
    }

    /// 取出指定接收方的所有缓存消息
    pub fn drain_for(&mut self, recipient_fingerprint: &str) -> Vec<Vec<u8>> {
        self.evict_expired();
        let mut result = Vec::new();
        self.entries.retain(|e| {
            if e.recipient_fingerprint == recipient_fingerprint {
                result.push(e.data.clone());
                false
            } else {
                true
            }
        });
        result
    }

    /// 取出所有超时的条目（用于转投代收 Store）
    #[allow(dead_code)]
    pub fn drain_expired(&mut self) -> Vec<(String, Vec<u8>)> {
        let now = Self::now_ms();
        let mut expired = Vec::new();
        while let Some(front) = self.entries.front() {
            if now - front.created_at > self.ttl_ms {
                let entry = self.entries.pop_front().unwrap();
                expired.push((entry.recipient_fingerprint, entry.data));
            } else {
                break;
            }
        }
        expired
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        let now = Self::now_ms();
        let mut expired_count = 0;
        let mut active_count = 0;
        let mut recipients = std::collections::HashSet::new();

        for entry in &self.entries {
            if now - entry.created_at > self.ttl_ms {
                expired_count += 1;
            } else {
                active_count += 1;
            }
            recipients.insert(entry.recipient_fingerprint.clone());
        }

        CacheStats {
            total_entries: self.entries.len(),
            active_entries: active_count,
            expired_entries: expired_count,
            unique_recipients: recipients.len(),
            max_entries: self.max_entries,
            ttl_ms: self.ttl_ms,
        }
    }

    /// 获取所有缓存条目的详细信息
    pub fn get_all_entries(&self) -> Vec<CacheEntryInfo> {
        let now = Self::now_ms();
        self.entries
            .iter()
            .map(|e| CacheEntryInfo {
                recipient_fingerprint: e.recipient_fingerprint.clone(),
                size_bytes: e.data.len(),
                age_ms: now.saturating_sub(e.created_at),
                is_expired: now - e.created_at > self.ttl_ms,
            })
            .collect()
    }
}

/// 缓存统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub expired_entries: usize,
    pub unique_recipients: usize,
    pub max_entries: usize,
    pub ttl_ms: u64,
}

/// 缓存条目信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheEntryInfo {
    pub recipient_fingerprint: String,
    pub size_bytes: usize,
    pub age_ms: u64,
    pub is_expired: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_drain() {
        let mut cache = RelayCache::new(100, 60_000);
        cache.store("alice", b"msg1".to_vec());
        cache.store("alice", b"msg2".to_vec());
        cache.store("bob", b"msg3".to_vec());

        let alice_msgs = cache.drain_for("alice");
        assert_eq!(alice_msgs.len(), 2);
        assert_eq!(cache.len(), 1); // only bob's msg left
    }

    #[test]
    fn test_capacity_limit() {
        let mut cache = RelayCache::new(3, 60_000);
        cache.store("a", b"1".to_vec());
        cache.store("b", b"2".to_vec());
        cache.store("c", b"3".to_vec());
        cache.store("d", b"4".to_vec()); // evicts oldest

        assert_eq!(cache.len(), 3);
        let a_msgs = cache.drain_for("a");
        assert!(a_msgs.is_empty()); // "a" was evicted
    }

    #[test]
    fn test_empty_drain() {
        let mut cache = RelayCache::new(100, 60_000);
        let msgs = cache.drain_for("nobody");
        assert!(msgs.is_empty());
    }
}
