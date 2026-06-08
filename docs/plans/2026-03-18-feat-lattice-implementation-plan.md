---
title: "feat: Lattice 去中心化 IM 技术实现计划"
type: feat
date: 2026-03-18
---

# Lattice 去中心化 IM 技术实现计划

## Overview

基于 brainstorm 阶段确定的架构设计，本文档定义 Lattice 的 Rust 项目结构、核心 Trait、Protobuf schema、开发里程碑。

参考了 matrix-rust-sdk 的生产级 workspace 组织模式（37 个 crate、sans-I/O 核心、Trait 驱动存储）。

## 项目结构

```
lattice/
├── Cargo.toml                    # workspace 根配置
├── proto/                        # Protobuf 定义文件
│   ├── message.proto
│   ├── identity.proto
│   ├── group.proto
│   └── transport.proto
├── crates/
│   ├── lattice-proto/             # Protobuf 生成代码 (prost)
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   └── src/lib.rs
│   ├── lattice-crypto/            # 加密层 (vodozemac 封装)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── identity.rs       # 身份密钥管理
│   │       ├── olm.rs            # 1v1 加密会话
│   │       ├── megolm.rs         # 群聊加密会话
│   │       ├── sign.rs           # 签名 + SHA 校验
│   │       └── trust.rs          # 三档信任模型
│   ├── lattice-core/              # 核心业务逻辑 (sans-I/O)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits/           # 所有 Trait 定义
│   │       │   ├── mod.rs
│   │       │   ├── transport.rs
│   │       │   ├── storage.rs
│   │       │   └── search.rs
│   │       ├── message.rs        # 消息处理逻辑
│   │       ├── room.rs           # 房间/群组管理
│   │       ├── contact.rs        # 联系人管理
│   │       ├── device.rs         # 多设备管理
│   │       └── exchange.rs       # 信息交换端逻辑
│   ├── lattice-transport/         # 传输层实现
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── websocket.rs      # WebSocket 实现
│   ├── lattice-storage/           # 存储层实现
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite.rs         # SQLite 存储实现
│   │       └── tantivy.rs        # Tantivy 搜索实现
│   ├── lattice-store/             # Store 节点 (二进制)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── server.rs         # WebSocket 服务端
│   │       ├── relay.rs          # 消息转发逻辑
│   │       └── api.rs            # RESTful API (手机端)
│   ├── lattice-peer/              # Peer 中转节点 (二进制)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── relay.rs          # 中转逻辑
│   │       ├── stun.rs           # STUN/NAT 穿透
│   │       └── cache.rs          # 短时缓存
│   └── lattice-ffi/               # Android FFI (UniFFI)
│       ├── Cargo.toml
│       ├── src/lib.rs
│       └── uniffi.toml
├── tests/                        # 集成测试
│   └── integration/
├── examples/                     # 示例代码
└── docs/
    ├── brainstorms/
    └── plans/
```

## Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/lattice-proto",
    "crates/lattice-crypto",
    "crates/lattice-core",
    "crates/lattice-transport",
    "crates/lattice-storage",
    "crates/lattice-store",
    "crates/lattice-peer",
    "crates/lattice-ffi",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# 序列化
prost = "0.13"
prost-types = "0.13"
serde = { version = "1", features = ["derive"] }

# 异步运行时
tokio = { version = "1", features = ["full"] }

# 加密
vodozemac = { version = "0.9", default-features = false }
ed25519-dalek = "2"
x25519-dalek = "2"
sha2 = "0.10"

# 传输
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-native-roots"] }

# 存储
rusqlite = { version = "0.32", features = ["bundled"] }
tantivy = "0.22"
cang-jie = "0.7"

# 工具
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
bytes = "1"
uuid = { version = "1", features = ["v4"] }
base64 = "0.22"
zeroize = "1"
rand = "0.8"
```

## 依赖关系图

```
lattice-ffi
    ↓
lattice-store / lattice-peer        (二进制目标)
    ↓              ↓
lattice-core                       (核心业务逻辑, sans-I/O)
    ↓       ↓          ↓
lattice-   lattice-     lattice-     (Trait 实现层)
transport  storage     crypto
    ↓       ↓          ↓
lattice-proto                      (Protobuf 类型定义)
```

规则：
- `lattice-core` 只定义 Trait，不依赖任何具体实现
- `lattice-transport`、`lattice-storage`、`lattice-crypto` 实现 Trait
- `lattice-store` 和 `lattice-peer` 是二进制入口，组装所有依赖
- `lattice-ffi` 封装 `lattice-core` + 实现层，暴露给 Android/iOS

## 核心 Trait 定义

### Transport Trait

```rust
// crates/lattice-core/src/traits/transport.rs

use crate::error::Result;

/// 传输层抽象 — 所有通信协议实现此 Trait
/// 首期实现：WebSocket，未来可扩展 QUIC/gRPC
pub trait Transport: Send + Sync {
    /// 连接到远程地址
    async fn connect(&mut self, addr: &str) -> Result<()>;

    /// 发送二进制数据（Protobuf 序列化后的字节）
    async fn send(&self, data: &[u8]) -> Result<()>;

    /// 接收二进制数据
    async fn recv(&mut self) -> Result<Vec<u8>>;

    /// 关闭连接
    async fn close(&mut self) -> Result<()>;

    /// 连接是否存活
    fn is_connected(&self) -> bool;
}

/// 传输层服务端 — Store/Peer 节点监听连接
pub trait TransportListener: Send + Sync {
    type Conn: Transport;

    /// 绑定并监听地址
    async fn bind(addr: &str) -> Result<Self> where Self: Sized;

    /// 接受一个新连接
    async fn accept(&mut self) -> Result<Self::Conn>;
}
```

### Storage Trait

```rust
// crates/lattice-core/src/traits/storage.rs

use crate::error::Result;
use lattice_proto::{Message, Room, Contact, DeviceInfo, KeyBundle};

/// 时间范围查询
pub struct TimeRange {
    pub start: u64,  // Unix 时间戳 ms
    pub end: u64,
}

/// 分页参数
pub struct Pagination {
    pub offset: u64,
    pub limit: u32,
}

/// 存储层抽象 — 数据持久化
pub trait Storage: Send + Sync {
    // === 消息 ===
    async fn save_message(&self, msg: &Message) -> Result<()>;
    async fn get_messages(&self, room_id: &str, range: &TimeRange, page: &Pagination) -> Result<Vec<Message>>;
    async fn get_message(&self, msg_id: &str) -> Result<Option<Message>>;
    async fn delete_message(&self, msg_id: &str) -> Result<()>;

    // === 房间/群组 ===
    async fn save_room(&self, room: &Room) -> Result<()>;
    async fn get_room(&self, room_id: &str) -> Result<Option<Room>>;
    async fn get_rooms(&self) -> Result<Vec<Room>>;
    async fn delete_room(&self, room_id: &str) -> Result<()>;

    // === 联系人 ===
    async fn save_contact(&self, contact: &Contact) -> Result<()>;
    async fn get_contact(&self, public_key_fingerprint: &str) -> Result<Option<Contact>>;
    async fn get_contacts(&self) -> Result<Vec<Contact>>;
    async fn delete_contact(&self, public_key_fingerprint: &str) -> Result<()>;

    // === 设备 ===
    async fn save_device(&self, device: &DeviceInfo) -> Result<()>;
    async fn get_devices(&self, user_fingerprint: &str) -> Result<Vec<DeviceInfo>>;

    // === 密钥 ===
    async fn save_key_bundle(&self, bundle: &KeyBundle) -> Result<()>;
    async fn get_key_bundle(&self, fingerprint: &str) -> Result<Option<KeyBundle>>;
}
```

### SearchIndex Trait

```rust
// crates/lattice-core/src/traits/search.rs

use crate::error::Result;
use lattice_proto::Message;

/// 搜索结果
pub struct SearchResult {
    pub msg_id: String,
    pub room_id: String,
    pub snippet: String,    // 高亮摘要
    pub score: f32,         // 相关度评分
    pub timestamp: u64,
}

/// 搜索层抽象 — 全文检索
/// 首期实现：Tantivy，未来可扩展向量搜索
pub trait SearchIndex: Send + Sync {
    /// 索引一条消息
    async fn index_message(&self, msg: &Message) -> Result<()>;

    /// 批量索引
    async fn index_messages(&self, msgs: &[Message]) -> Result<()>;

    /// 全文搜索
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;

    /// 在指定房间内搜索
    async fn search_in_room(&self, room_id: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>>;

    /// 删除消息索引
    async fn delete_index(&self, msg_id: &str) -> Result<()>;

    /// 重建全部索引
    async fn rebuild_index(&self) -> Result<()>;
}
```

## Protobuf Schema

### proto/identity.proto

```protobuf
syntax = "proto3";
package lattice.identity;

// 用户身份
message Identity {
    bytes ed25519_public_key = 1;       // Ed25519 签名公钥
    bytes curve25519_public_key = 2;    // Curve25519 加密公钥
    string fingerprint = 3;             // SHA-256(ed25519_public_key) 十六进制
    string display_name = 4;            // 可选显示名
    uint64 created_at = 5;
}

// 设备信息
message DeviceInfo {
    string device_id = 1;              // UUID
    string user_fingerprint = 2;       // 所属用户指纹
    bytes device_ed25519_key = 3;      // 设备签名公钥
    bytes device_curve25519_key = 4;   // 设备加密公钥
    bytes signature = 5;               // 主私钥对设备公钥的签名
    string device_name = 6;            // "iPhone 15" 等
    uint64 created_at = 7;
}

// 密钥包（用于密钥交换）
message KeyBundle {
    string fingerprint = 1;
    bytes identity_key = 2;            // Curve25519 身份密钥
    repeated bytes one_time_keys = 3;  // 一次性预密钥
    bytes fallback_key = 4;            // 备用密钥
    bytes signature = 5;               // 签名
}

// 身份卡片（用于手动交换/二维码）
message IdentityCard {
    Identity identity = 1;
    string store_address = 2;          // WebSocket 地址
    string proxy_store_address = 3;    // 代收 Store 地址（可选）
}
```

### proto/message.proto

```protobuf
syntax = "proto3";
package lattice.message;

import "identity.proto";

// 信封 — 所有传输的消息都包在信封里
message Envelope {
    string msg_id = 1;                 // UUID
    string sender_fingerprint = 2;     // 发送方公钥指纹
    string recipient_fingerprint = 3;  // 接收方公钥指纹（群聊为 room_id）
    uint64 timestamp = 4;             // 发送时间戳 ms
    bytes signature = 5;              // Ed25519 签名（对 payload 签名）
    bytes payload_hash = 6;           // SHA-256(payload)

    oneof payload {
        PlainPayload plain = 10;       // 明文消息
        EncryptedPayload encrypted = 11; // E2EE 加密消息
    }
}

// 明文载荷
message PlainPayload {
    MessageContent content = 1;
}

// 加密载荷
message EncryptedPayload {
    bytes ciphertext = 1;
    string session_id = 2;            // Olm/Megolm 会话 ID
    uint32 message_index = 3;         // Megolm 消息索引
    EncryptionType encryption_type = 4;
}

enum EncryptionType {
    OLM = 0;       // 1v1 Olm
    MEGOLM = 1;    // 群聊 Megolm
}

// 消息内容
message MessageContent {
    MessageType type = 1;
    string text = 2;                   // 文本内容
    bytes media = 3;                   // 媒体数据（可选）
    string media_type = 4;            // MIME type
    string reply_to = 5;              // 回复的 msg_id（可选）
}

enum MessageType {
    TEXT = 0;
    IMAGE = 1;
    FILE = 2;
    AUDIO = 3;
    VIDEO = 4;
    SYSTEM = 5;    // 系统消息（加入/退出等）
}

// 存储用的完整消息（包含解密后的内容）
message Message {
    string msg_id = 1;
    string room_id = 2;
    string sender_fingerprint = 3;
    uint64 timestamp = 4;
    MessageContent content = 5;
    bool encrypted = 6;               // 是否经过 E2EE
    bool verified = 7;                // 签名是否验证通过
}
```

### proto/group.proto

```protobuf
syntax = "proto3";
package lattice.group;

// 房间/群组
message Room {
    string room_id = 1;               // UUID
    string name = 2;
    RoomType type = 3;
    string creator_fingerprint = 4;
    bool encrypted = 5;               // 是否开启 E2EE
    HistoryVisibility history_visibility = 6;
    uint64 created_at = 7;
    repeated RoomMember members = 8;
}

enum RoomType {
    DIRECT = 0;     // 1v1 私聊
    GROUP = 1;      // 群聊
    CHANNEL = 2;    // 公开频道
}

enum HistoryVisibility {
    FULL = 0;        // 新成员可看全部历史
    JOIN_ONLY = 1;   // 只能看加入之后的
}

// 群成员
message RoomMember {
    string user_fingerprint = 1;
    MemberRole role = 2;
    uint64 joined_at = 3;
}

enum MemberRole {
    MEMBER = 0;
    ADMIN = 1;
    OWNER = 2;
}

// 群操作事件
message RoomEvent {
    string room_id = 1;
    string actor_fingerprint = 2;     // 操作者
    RoomEventType type = 3;
    string target_fingerprint = 4;    // 被操作者（可选）
    uint64 timestamp = 5;
    bytes signature = 6;
}

enum RoomEventType {
    MEMBER_JOIN = 0;
    MEMBER_LEAVE = 1;
    MEMBER_KICK = 2;
    MEMBER_BAN = 3;
    ROLE_CHANGE = 4;
    ROOM_UPDATE = 5;     // 改名/改设置
    KEY_ROTATION = 6;    // Megolm 密钥轮换
}
```

### proto/transport.proto

```protobuf
syntax = "proto3";
package lattice.transport;

import "message.proto";
import "identity.proto";
import "group.proto";

// WebSocket 帧 — 所有 WebSocket 通信的顶层消息
message Frame {
    uint64 seq = 1;                    // 序列号
    FrameType type = 2;

    oneof body {
        // 消息相关
        message.Envelope message = 10;
        MessageAck ack = 11;

        // 身份/密钥交换
        identity.KeyBundle key_bundle = 20;
        KeyRequest key_request = 21;

        // 群组操作
        group.RoomEvent room_event = 30;

        // 同步
        SyncRequest sync_request = 40;
        SyncResponse sync_response = 41;

        // 心跳
        Ping ping = 50;
        Pong pong = 51;
    }
}

enum FrameType {
    MESSAGE = 0;
    ACK = 1;
    KEY_BUNDLE = 2;
    KEY_REQUEST = 3;
    ROOM_EVENT = 4;
    SYNC_REQUEST = 5;
    SYNC_RESPONSE = 6;
    PING = 7;
    PONG = 8;
}

message MessageAck {
    string msg_id = 1;
    AckStatus status = 2;
}

enum AckStatus {
    RECEIVED = 0;
    DELIVERED = 1;
    READ = 2;
    REJECTED = 3;       // 信任层拒绝
}

message KeyRequest {
    string target_fingerprint = 1;
    string requester_fingerprint = 2;
}

message SyncRequest {
    uint64 since_timestamp = 1;       // 从什么时间开始同步
    repeated string room_ids = 2;     // 指定房间（空=全部）
}

message SyncResponse {
    repeated message.Envelope messages = 1;
    repeated group.RoomEvent events = 2;
    uint64 next_batch = 3;            // 下次同步的起点
}

message Ping {
    uint64 timestamp = 1;
}

message Pong {
    uint64 timestamp = 1;
}
```

// 联系人（本地存储用）
message Contact {
    identity.Identity identity = 1;
    string store_address = 2;
    string proxy_store_address = 3;
    TrustLevel trust_level = 4;
    string alias = 5;                  // 本地备注名
    uint64 added_at = 6;
}

enum TrustLevel {
    PUBLIC = 0;       // 公开模式 — 接收任何消息
    TOFU = 1;         // 宽松模式 — 首次信任
    VERIFIED = 2;     // 严格模式 — 已验证公钥
}
```

## 开发里程碑

### Phase 1: 骨架搭建（基础设施）

**目标：** Cargo workspace 跑通，Protobuf 编译通过，核心 Trait 定义完成。

- [ ] 初始化 Cargo workspace，创建所有 crate 目录结构
- [ ] 编写 proto/ 下所有 .proto 文件
- [ ] `lattice-proto`: 配置 prost-build，`cargo build` 生成 Rust 类型
- [ ] `lattice-core`: 定义 Transport、Storage、SearchIndex Trait
- [ ] `lattice-core`: 定义 Error 类型（thiserror）
- [ ] 验证：`cargo check --workspace` 通过

**产出文件：**
- `Cargo.toml` (workspace root)
- `proto/*.proto` (4 个文件)
- `crates/lattice-proto/` (build.rs + lib.rs)
- `crates/lattice-core/src/traits/` (transport.rs, storage.rs, search.rs)
- `crates/lattice-core/src/error.rs`

### Phase 2: 身份与签名（安全基础）

**目标：** 用户可以生成密钥对、创建身份、对消息签名和验证。

- [ ] `lattice-crypto/src/identity.rs`: 生成 Ed25519 + Curve25519 密钥对
- [ ] `lattice-crypto/src/identity.rs`: 导出/导入私钥（加密存储）
- [ ] `lattice-crypto/src/identity.rs`: 生成 SHA-256 指纹
- [ ] `lattice-crypto/src/sign.rs`: 消息签名（Ed25519）
- [ ] `lattice-crypto/src/sign.rs`: 签名验证
- [ ] `lattice-crypto/src/sign.rs`: SHA-256 完整性校验
- [ ] `lattice-crypto/src/trust.rs`: 三档信任模型（Public/TOFU/Verified）
- [ ] `lattice-crypto/src/identity.rs`: 设备密钥生成 + 主密钥签名
- [ ] 单元测试：密钥生成、签名验证、信任判断
- [ ] 身份卡片序列化/反序列化（IdentityCard protobuf）

**产出文件：**
- `crates/lattice-crypto/src/` (identity.rs, sign.rs, trust.rs)

### Phase 3: 存储层实现

**目标：** 消息可以持久化存储和全文搜索。

- [ ] `lattice-storage/src/sqlite.rs`: 实现 Storage Trait
- [ ] SQLite schema 建表（messages, rooms, contacts, devices, keys）
- [ ] SQLite 索引优化（room_id + timestamp 复合索引）
- [ ] `lattice-storage/src/tantivy.rs`: 实现 SearchIndex Trait
- [ ] Tantivy 中文分词配置（cang-jie tokenizer）
- [ ] 消息写入时同步索引到 Tantivy
- [ ] 单元测试：CRUD 操作、全文搜索（中英文）
- [ ] 内存实现 MemoryStorage（用于测试）

**产出文件：**
- `crates/lattice-storage/src/` (sqlite.rs, tantivy.rs, memory.rs)

### Phase 4: 传输层实现

**目标：** 两个 Store 节点可以通过 WebSocket 互发 Protobuf 消息。

- [ ] `lattice-transport/src/websocket.rs`: 实现 Transport Trait（客户端）
- [ ] `lattice-transport/src/websocket.rs`: 实现 TransportListener Trait（服务端）
- [ ] WebSocket 连接管理（自动重连、指数退避）
- [ ] 心跳机制（Ping/Pong）
- [ ] Frame 序列化/反序列化（Protobuf ↔ WebSocket 二进制帧）
- [ ] 集成测试：两个节点互发消息

**产出文件：**
- `crates/lattice-transport/src/websocket.rs`

### Phase 5: Store 节点

**目标：** 可运行的 Store 节点，支持消息收发、存储、搜索。

- [ ] `lattice-store/src/server.rs`: WebSocket 服务端，接受连接
- [ ] `lattice-store/src/relay.rs`: 消息路由（本地存储 + 转发给接收方）
- [ ] `lattice-store/src/api.rs`: RESTful API（手机端查询消息、发送消息、搜索）
- [ ] 消息投递流程：直连 → 失败记录待发
- [ ] 联系人管理（添加、删除、信任等级）
- [ ] 配置文件（监听地址、存储路径、代收 Store 地址）
- [ ] 集成测试：两个 Store 节点完整聊天流程

**产出文件：**
- `crates/lattice-store/src/` (main.rs, server.rs, relay.rs, api.rs)

### Phase 6: E2EE 加密（可选层）

**目标：** 用户可以开启 1v1 和群聊的端到端加密。

- [ ] `lattice-crypto/src/olm.rs`: 封装 vodozemac Olm（1v1 加密会话）
- [ ] One-time key 生成和管理
- [ ] Olm 会话建立（3DH 密钥交换）
- [ ] `lattice-crypto/src/megolm.rs`: 封装 vodozemac Megolm（群聊加密）
- [ ] Megolm 会话密钥分发（通过 Olm 加密发送给每个成员）
- [ ] 密钥轮换（成员退出 / 100 条消息 / 7 天）
- [ ] 加密消息的 Envelope 封装和解封
- [ ] 密钥存储（加密后存入 SQLite）
- [ ] 集成测试：加密私聊、加密群聊、密钥轮换

**产出文件：**
- `crates/lattice-crypto/src/` (olm.rs, megolm.rs)

### Phase 7: 群聊与频道

**目标：** 完整的群聊功能，包括权限管理和公开频道。

- [ ] `lattice-core/src/room.rs`: 群组创建、成员管理
- [ ] 权限模型实现（owner/admin/member）
- [ ] RoomEvent 处理（加入、退出、踢人、禁言）
- [ ] 历史可见性控制（full / join_only）
- [ ] 公开频道：明文 + 签名，任何人可订阅
- [ ] 群消息广播（发送给所有成员的 Store）

**产出文件：**
- `crates/lattice-core/src/room.rs` 完善

### Phase 8: Peer 中转节点

**目标：** Peer 节点可以中转消息，支持短时缓存和代收转投。

- [ ] `lattice-peer/src/relay.rs`: 消息中转逻辑
- [ ] `lattice-peer/src/cache.rs`: 短时缓存（内存，TTL + 容量上限）
- [ ] `lattice-peer/src/stun.rs`: STUN/NAT 穿透基础
- [ ] 超时未送达 → 转投代收 Store
- [ ] Peer 节点配置（缓存大小、TTL、监听地址）

**产出文件：**
- `crates/lattice-peer/src/` (main.rs, relay.rs, cache.rs, stun.rs)

### Phase 9: Android FFI

**目标：** Rust 核心通过 UniFFI 暴露给 Android/Kotlin。

- [ ] `lattice-ffi`: UniFFI 配置，定义暴露的接口
- [ ] 核心功能封装：创建身份、发送消息、搜索、加解密
- [ ] `cargo-ndk` 交叉编译到 aarch64-linux-android
- [ ] 生成 Kotlin 绑定代码
- [ ] Android demo 项目验证 FFI 调用

**产出文件：**
- `crates/lattice-ffi/` (lib.rs, uniffi.toml)

### Phase 10: 信息交换端 + 多设备同步

**目标：** 用户可以管理消息接收策略，多设备同步消息。

- [ ] `lattice-core/src/exchange.rs`: 信息交换端逻辑
- [ ] 接收策略管理（按联系人/群组设置信任等级）
- [ ] 消息过滤（根据信任等级决定是否接收）
- [ ] `lattice-core/src/device.rs`: 多设备同步
- [ ] 设备注册/注销
- [ ] E2EE 密钥跨设备同步（加密存储在 Store）

**产出文件：**
- `crates/lattice-core/src/` (exchange.rs, device.rs)

## 开发优先级排序

```
Phase 1 (骨架)
    ↓
Phase 2 (身份签名) ← 安全基础，一切依赖于此
    ↓
Phase 3 (存储) + Phase 4 (传输) ← 可并行开发
    ↓
Phase 5 (Store 节点) ← 第一个可运行的里程碑
    ↓
Phase 6 (E2EE) ← 可选但重要
    ↓
Phase 7 (群聊) + Phase 8 (Peer) ← 可并行开发
    ↓
Phase 9 (Android FFI)
    ↓
Phase 10 (交换端 + 多设备)
```

**MVP 定义：** Phase 1-5 完成后，两个 Store 节点可以互发签名验证的明文消息，支持消息存储和中文全文搜索。这是最小可用产品。

## 技术决策总结

| 决策 | 选择 | 备选 |
|------|------|------|
| 异步运行时 | tokio | async-std |
| WebSocket 库 | tokio-tungstenite + rustls | async-tungstenite |
| Protobuf 库 | prost + prost-build | protobuf crate |
| async Trait | 原生 async fn in trait (Rust 1.75+) | async-trait crate |
| 错误处理 | thiserror (库) + anyhow (二进制) | — |
| 日志 | tracing + tracing-subscriber | log + env_logger |

## 参考

- `docs/brainstorms/2026-03-17-lattice-architecture-brainstorm.md` — 架构设计
- `reference/vodozemac/` — Olm/Megolm 加密库
- `reference/matrix-rust-sdk/` — 项目结构参考（workspace 组织、FFI 分层、Store Trait 模式）
- `reference/matrix-spec/` — 协议规范参考
