//! 轻量级 Kademlia DHT 实现
//!
//! 简化版：不实现完整 Kademlia 协议，只实现核心的 k-bucket 路由表和 key-value 存储。
//! 足够用于节点发现场景。

use std::collections::HashMap;
use sha2::{Sha256, Digest};

/// DHT 节点 ID — 256 位（SHA-256 哈希）
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// 从公钥指纹（hex 字符串）创建
    pub fn from_fingerprint(fingerprint: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(fingerprint)?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[..32]);
        Ok(Self(arr))
    }

    /// 从任意数据计算 SHA-256 作为 NodeId
    pub fn from_data(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        Self(arr)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// 计算两个 NodeId 的 XOR 距离
    pub fn distance(&self, other: &NodeId) -> [u8; 32] {
        let mut dist = [0u8; 32];
        for (i, item) in dist.iter_mut().enumerate() {
            *item = self.0[i] ^ other.0[i];
        }
        dist
    }

    /// 计算 XOR 距离的前导零位数（用于确定 k-bucket 索引）
    pub fn bucket_index(&self, other: &NodeId) -> usize {
        let dist = self.distance(other);
        for (i, &byte) in dist.iter().enumerate() {
            if byte != 0 {
                return i * 8 + byte.leading_zeros() as usize;
            }
        }
        255 // 相同节点
    }
}

/// DHT 中存储的节点信息
#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: String,       // WebSocket 地址
    pub last_seen: u64,        // 最后活跃时间戳
}

/// K-Bucket — 存储距离相近的节点
struct KBucket {
    nodes: Vec<NodeInfo>,
    k: usize,
}

impl KBucket {
    fn new(k: usize) -> Self {
        Self { nodes: Vec::new(), k }
    }

    fn insert(&mut self, node: NodeInfo) {
        // 如果已存在，更新 last_seen 并移到末尾
        if let Some(pos) = self.nodes.iter().position(|n| n.id == node.id) {
            self.nodes.remove(pos);
            self.nodes.push(node);
            return;
        }
        // 未满直接插入
        if self.nodes.len() < self.k {
            self.nodes.push(node);
        } else {
            // 满了，替换最旧的（简化版，完整 Kademlia 会先 ping 最旧节点）
            self.nodes.remove(0);
            self.nodes.push(node);
        }
    }

    #[allow(dead_code)]
    fn find_closest(&self, target: &NodeId, count: usize) -> Vec<NodeInfo> {
        let mut sorted = self.nodes.clone();
        sorted.sort_by_key(|n| n.id.distance(target));
        sorted.truncate(count);
        sorted
    }

    fn remove(&mut self, id: &NodeId) {
        self.nodes.retain(|n| n.id != *id);
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }
}

/// DHT 路由表
pub struct RoutingTable {
    local_id: NodeId,
    buckets: Vec<KBucket>,
    #[allow(dead_code)]
    k: usize,
}

impl RoutingTable {
    pub fn new(local_id: NodeId, k: usize) -> Self {
        let mut buckets = Vec::with_capacity(256);
        for _ in 0..256 {
            buckets.push(KBucket::new(k));
        }
        Self { local_id, buckets, k }
    }

    /// 插入节点到路由表
    pub fn insert(&mut self, node: NodeInfo) {
        if node.id == self.local_id {
            return; // 不存自己
        }
        let idx = self.local_id.bucket_index(&node.id);
        self.buckets[idx].insert(node);
    }

    /// 移除节点
    pub fn remove(&mut self, id: &NodeId) {
        let idx = self.local_id.bucket_index(id);
        self.buckets[idx].remove(id);
    }

    /// 查找最接近目标的 k 个节点
    pub fn find_closest(&self, target: &NodeId, count: usize) -> Vec<NodeInfo> {
        let mut all: Vec<NodeInfo> = Vec::new();
        let idx = self.local_id.bucket_index(target);

        // 从目标 bucket 开始，向两侧扩展
        let mut left = idx as isize;
        let mut right = idx + 1;

        while all.len() < count && (left >= 0 || right < 256) {
            if left >= 0 {
                all.extend(self.buckets[left as usize].nodes.clone());
                left -= 1;
            }
            if right < 256 {
                all.extend(self.buckets[right].nodes.clone());
                right += 1;
            }
        }

        all.sort_by_key(|n| n.id.distance(target));
        all.truncate(count);
        all
    }

    /// 路由表中的总节点数
    pub fn len(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// DHT 存储 — key-value 存储（公钥指纹 → Store 地址）
pub struct DhtStore {
    routing_table: RoutingTable,
    store: HashMap<String, String>, // fingerprint → ws address
}

impl DhtStore {
    pub fn new(local_id: NodeId, k: usize) -> Self {
        Self {
            routing_table: RoutingTable::new(local_id, k),
            store: HashMap::new(),
        }
    }

    /// 发布自己的地址
    pub fn publish(&mut self, fingerprint: &str, address: &str) {
        self.store.insert(fingerprint.to_string(), address.to_string());
    }

    /// 查找地址
    pub fn lookup(&self, fingerprint: &str) -> Option<&String> {
        self.store.get(fingerprint)
    }

    /// 移除发布
    pub fn unpublish(&mut self, fingerprint: &str) {
        self.store.remove(fingerprint);
    }

    /// 添加已知节点
    pub fn add_node(&mut self, node: NodeInfo) {
        self.routing_table.insert(node);
    }

    /// 查找最接近目标的节点（用于路由查询）
    pub fn find_closest_nodes(&self, fingerprint: &str, count: usize) -> Vec<NodeInfo> {
        let target = NodeId::from_data(fingerprint.as_bytes());
        self.routing_table.find_closest(&target, count)
    }

    /// 已知节点数
    pub fn node_count(&self) -> usize {
        self.routing_table.len()
    }

    /// 已发布记录数
    pub fn record_count(&self) -> usize {
        self.store.len()
    }
}

#[allow(dead_code)]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id_byte: u8, addr: &str) -> NodeInfo {
        let mut id = [0u8; 32];
        id[0] = id_byte;
        NodeInfo {
            id: NodeId::from_bytes(id),
            address: addr.to_string(),
            last_seen: now_ms(),
        }
    }

    #[test]
    fn test_node_id_distance() {
        let a = NodeId::from_bytes([0xFF; 32]);
        let b = NodeId::from_bytes([0x00; 32]);
        let dist = a.distance(&b);
        assert_eq!(dist, [0xFF; 32]);

        let self_dist = a.distance(&a);
        assert_eq!(self_dist, [0x00; 32]);
    }

    #[test]
    fn test_bucket_index() {
        let a = NodeId::from_bytes([0x00; 32]);
        let mut b_bytes = [0x00u8; 32];
        b_bytes[0] = 0x80; // first bit differs
        let b = NodeId::from_bytes(b_bytes);
        assert_eq!(a.bucket_index(&b), 0);

        b_bytes[0] = 0x01; // 8th bit differs
        let c = NodeId::from_bytes(b_bytes);
        assert_eq!(a.bucket_index(&c), 7);
    }

    #[test]
    fn test_routing_table_insert_and_find() {
        let local = NodeId::from_bytes([0x00; 32]);
        let mut rt = RoutingTable::new(local, 3);

        for i in 1..=5u8 {
            rt.insert(make_node(i, &format!("ws://node-{i}")));
        }

        assert_eq!(rt.len(), 5);

        let target = NodeId::from_bytes([0x03; 32]);
        let closest = rt.find_closest(&target, 3);
        assert_eq!(closest.len(), 3);
        // 最近的应该是 node 3（XOR 距离为 0）
        assert_eq!(closest[0].id.as_bytes()[0], 0x03);
    }

    #[test]
    fn test_routing_table_no_self() {
        let local = NodeId::from_bytes([0x00; 32]);
        let mut rt = RoutingTable::new(local.clone(), 3);

        let self_node = NodeInfo {
            id: local,
            address: "ws://self".to_string(),
            last_seen: now_ms(),
        };
        rt.insert(self_node);
        assert_eq!(rt.len(), 0); // 不存自己
    }

    #[test]
    fn test_kbucket_eviction() {
        let local = NodeId::from_bytes([0x00; 32]);
        let mut rt = RoutingTable::new(local, 2); // k=2

        // 插入 3 个节点到同一个 bucket
        // 0x80, 0xC0, 0xA0 都在 bucket 0（第一位为 1）
        let mut id1 = [0u8; 32]; id1[0] = 0x80;
        let mut id2 = [0u8; 32]; id2[0] = 0xC0;
        let mut id3 = [0u8; 32]; id3[0] = 0xA0;

        rt.insert(NodeInfo { id: NodeId::from_bytes(id1), address: "ws://1".into(), last_seen: 1 });
        rt.insert(NodeInfo { id: NodeId::from_bytes(id2), address: "ws://2".into(), last_seen: 2 });
        rt.insert(NodeInfo { id: NodeId::from_bytes(id3), address: "ws://3".into(), last_seen: 3 });

        // k=2，最旧的 (0x80) 应该被淘汰
        let closest = rt.find_closest(&NodeId::from_bytes([0xFF; 32]), 10);
        assert_eq!(closest.len(), 2);
        assert!(closest.iter().all(|n| n.id.as_bytes()[0] != 0x80));
    }

    #[test]
    fn test_dht_store_publish_lookup() {
        let local = NodeId::from_data(b"my-node");
        let mut dht = DhtStore::new(local, 20);

        dht.publish("abc123", "ws://127.0.0.1:9100");
        assert_eq!(dht.lookup("abc123"), Some(&"ws://127.0.0.1:9100".to_string()));
        assert_eq!(dht.lookup("nonexistent"), None);
        assert_eq!(dht.record_count(), 1);

        dht.unpublish("abc123");
        assert_eq!(dht.lookup("abc123"), None);
    }

    #[test]
    fn test_dht_store_with_nodes() {
        let local = NodeId::from_data(b"local");
        let mut dht = DhtStore::new(local, 20);

        for i in 0..10u8 {
            let node = make_node(i + 1, &format!("ws://peer-{i}"));
            dht.add_node(node);
        }

        assert_eq!(dht.node_count(), 10);

        let closest = dht.find_closest_nodes("target-user", 5);
        assert_eq!(closest.len(), 5);
    }

    #[test]
    fn test_node_id_from_fingerprint() {
        let hex_str = "a".repeat(64); // 32 bytes in hex
        let id = NodeId::from_fingerprint(&hex_str).unwrap();
        assert_eq!(id.as_bytes(), &[0xAA; 32]);
    }
}
