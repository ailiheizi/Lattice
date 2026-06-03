---
date: 2026-03-17
topic: nextim-decentralized-im-architecture
---

# NextIM 去中心化 IM 架构设计

## 概述

NextIM 是一款去中心化即时通讯系统，用户完全掌控自己的数据。核心理念：自托管、公钥身份、可选加密、灵活信任模型。

## 架构分层

### 1. 发现层

- **手动交换（基础）** — 用户分享"身份卡片"（公钥 + Store 地址），支持二维码、链接、文件
- **DHT 目录（可选增强）** — 公钥 SHA 作为 key，Store 地址作为 value，发布到 DHT 网络（Kademlia）

优先实现手动交换，DHT 后续迭代。

### 2. 身份层

- 主密钥对：Ed25519（签名）+ Curve25519（加密）
- 设备密钥对：每个设备独立生成，用主私钥签名
- 用户唯一标识：主公钥的 SHA-256 指纹
- 所有消息**强制签名 + SHA 校验**，无论是否加密

### 3. 信任层（三档模式）

| 模式 | 描述 | 适用场景 |
|------|------|----------|
| 严格模式 | 只接收已互相验证公钥 + 签名的消息 | 高安全需求 |
| 宽松模式(TOFU) | 接收有签名但未互相验证的消息，首次信任 | 日常使用 |
| 公开模式 | 接收任何消息，包括无签名的 | 公开频道/广播 |

用户通过"信息交换端"管理接收策略。

### 4. 加密层（可选）

- **默认不加密**，用户自主选择是否开启 E2EE
- 私聊、群聊、频道均可独立配置加密开关
- 1v1 加密：Olm 协议（Double Ratchet），前向保密 + 后妥协安全
- 群聊加密：Megolm 协议，发送方创建会话密钥，通过 Olm 分发给群成员

### 5. 群组层

**权限模型：**

| 角色 | 权限 |
|------|------|
| owner（创建者） | 全部权限，不可被降级 |
| admin | 踢人、禁言、邀请、改群设置 |
| member | 发消息、邀请（可配置） |

**密钥轮换策略（E2EE 群聊）：**
- 成员退出 → 强制轮换 Megolm 会话密钥
- 每 100 条消息或每 7 天 → 定期轮换
- 退出者保留旧密钥可解密历史，无法解密新消息

**历史可见性：**
- `full` — 新成员可看全部历史
- `join_only` — 只能看加入之后的消息

### 6. 传输层

**技术选型：**
- 开发语言：Rust
- 消息序列化：Protobuf
- 通信协议：WebSocket + Protobuf（统一协议，所有场景复用）
- FFI 嵌入模式：直接函数调用，无需网络协议

**协议抽象（Trait 模式）：**

先定义传输 Trait，首期只实现 WebSocket，未来按需扩展：

```rust
trait Transport {
    async fn send(&self, msg: &[u8]) -> Result<()>;
    async fn recv(&mut self) -> Result<Vec<u8>>;
    async fn connect(&mut self, addr: &str) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
}

// 首期实现
struct WsTransport { ... }
impl Transport for WsTransport { ... }

// 未来可选扩展
// struct QuicTransport { ... }
// struct GrpcTransport { ... }
```

**选择 WebSocket 的理由：**
- 浏览器原生支持，未来做 Web 端零成本
- 双向通信，帧开销仅 2 字节
- 一套协议覆盖 Store 间、手机到 Store、Peer 中转所有场景
- 配合 Protobuf 二进制帧，性能足够
- Trait 抽象保留未来切换到 QUIC/gRPC 的能力，上层代码无需修改

**移动端省电策略：**
- 前台：WebSocket 持久连接，实时收发
- 后台：依赖 FCM(Android)/APNs(iOS) 推送通知，唤醒后拉取消息
- FFI 嵌入模式：同进程调用，零网络开销

**Store 节点（用户自托管）：**
- 持久化存储消息
- 接收/转发消息
- 提供搜索功能
- 存储用户公钥、设备密钥、one-time keys
- 部署形态：独立运行（PC/VPS）或 FFI 嵌入 Android 应用
- 手机端通过 RESTful API 与本地 Store 交互

**Peer 节点（中转）：**
- STUN/NAT 穿透
- 实时消息中转
- 短时缓存（几分钟到几小时，严格容量上限）
- 超时未送达 → 转投代收 Store

**代收 Store（离线邮局）：**
- 用户指定的信任第三方 Store
- 接收方离线时暂存消息
- 上线后拉取并删除

**消息投递流程：**
```
发送方 Store → 尝试直连接收方 Store（WebSocket）
           ↓ 失败
       Peer 中转 → 短时缓存
           ↓ 超时
       代收 Store → 接收方上线后拉取
```

### 7. 多设备同步

- Store 为单一数据源，所有设备从 Store 拉取消息
- 设备注册时生成设备密钥对，用主私钥签名
- E2EE 密钥加密后存储在 Store，新设备用主私钥解密获取
- 其他用户验证设备：检查主密钥签名链即可

## 融合模式

轻量用户可使用 Store + 手机程序融合版本：
- Store 核心通过 FFI 嵌入 Android 应用
- 无需额外部署服务器
- 功能完整但依赖手机在线

## 公开频道

- 利用 Peer 节点的临时存储能力
- 频道消息明文 + 签名（公开模式）
- 任何人可订阅和接收
- 频道创建者签名保证消息来源可信

## 参考实现

- `reference/vodozemac/` — Matrix Olm/Megolm Rust 实现
- `reference/matrix-rust-sdk/` — Matrix 客户端 SDK（Rust）
- `reference/matrix-spec/` — Matrix 协议规范

## 技术栈确认

| 层面 | 选型 | 理由 |
|------|------|------|
| 开发语言 | Rust | 性能、安全、跨平台 |
| 消息序列化 | Protobuf | 紧凑高效，强类型 |
| 通信协议 | WebSocket + Protobuf | 浏览器兼容，Trait 抽象可扩展 |
| 加密库 | vodozemac 0.9（关闭默认 features） | 纯 Rust，安全审计，零 Matrix 耦合 |
| Android FFI | UniFFI | 生产就绪，自动生成 Kotlin 绑定 |
| 本地存储 | SQLite (rusqlite + bundled + SQLCipher) | 加密支持，移动端验证，可靠轻量 |
| 全文搜索 | Tantivy + tantivy-jieba（中文分词） | 15k stars，纯 Rust 可嵌入，多语言支持 |

## 技术选型详解

### 1. 加密库：vodozemac

**选择理由：**
- 纯 Rust 实现，无 C 依赖，交叉编译友好
- Olm/Megolm 模块完全通用，不绑定 Matrix 协议
- 经过 Least Authority 安全审计
- 只有约 7400 行代码，轻量
- 底层用标准原语：Ed25519、Curve25519、AES-256、ChaCha20-Poly1305

**去耦合配置：**
```toml
[dependencies]
vodozemac = { version = "0.9", default-features = false }
```

关闭 `libolm-compat` 和 `insecure-pk-encryption` features 后，零 Matrix 耦合。

### 2. Android FFI：UniFFI

**选择理由：**
- Firefox 和 Element X 生产验证
- 自动生成 Kotlin 绑定，减少手写 unsafe 代码
- matrix-rust-sdk 的成功案例可参考
- 类型安全，维护成本低

**架构模式：**
- 分离 FFI 层（`nextim-ffi`）和核心逻辑（`nextim-core`）
- Protobuf 消息在 Rust 侧序列化，通过 UniFFI 传递字节数组
- 使用 `cargo-ndk` 交叉编译到 Android 目标

**已知问题及解决方案：**
- 32 位 ARM 设备 JNA 校验失败 → 配置 `omit_checksums = true`
- 2026 年 Google 强制 64 位，32 位兼容性不再关键

### 3. 本地存储：SQLite + Tantivy（Trait 抽象）

**核心设计：存储和搜索都通过 Trait 抽象，首期实现可替换，未来可扩展。**

**存储层：SQLite**
- WhatsApp 等主流 IM 验证过的方案
- SQLCipher 提供透明 256 位 AES 加密
- `rusqlite` 的 `bundled` feature 解决交叉编译
- 适合 IM 场景：大量小写入 + 按时间范围查询

**搜索层：Tantivy**
- 纯 Rust 全文搜索引擎库（Rust 版 Lucene），15k GitHub stars
- 可嵌入，不需要独立进程，适合 FFI 嵌入 Android
- 中文支持：tantivy-jieba / cang-jie（基于 jieba 分词）
- 日文：lindera / Vaporetto；韩文：lindera + lindera-ko-dic-builder
- 17 种拉丁语言词干提取

**配置：**
```toml
[dependencies]
rusqlite = { version = "0.32", features = ["bundled", "sqlcipher"] }
tantivy = "0.22"
cang-jie = "0.7"    # 中文分词 tokenizer
```

**Trait 抽象设计：**
```rust
// 存储 Trait — 数据持久化
trait Storage {
    async fn save_message(&self, msg: &Message) -> Result<()>;
    async fn get_messages(&self, room_id: &str, range: TimeRange) -> Result<Vec<Message>>;
    async fn save_key(&self, key_id: &str, key_data: &[u8]) -> Result<()>;
    // ...
}

// 搜索 Trait — 全文检索
trait SearchIndex {
    async fn index_message(&self, msg: &Message) -> Result<()>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    async fn delete_index(&self, msg_id: &str) -> Result<()>;
}

// 传输 Trait — 通信协议
trait Transport {
    async fn send(&self, msg: &[u8]) -> Result<()>;
    async fn recv(&mut self) -> Result<Vec<u8>>;
    async fn connect(&mut self, addr: &str) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
}

// 首期实现
struct SqliteStorage { ... }      // SQLite 存数据
struct TantivySearch { ... }      // Tantivy 做搜索
struct WsTransport { ... }        // WebSocket 通信

// 未来可扩展
// struct VectorSearch { ... }    // 向量语义搜索
// struct QuicTransport { ... }   // QUIC 传输
```

**Schema 设计要点：**
- 消息表：按 `(room_id, timestamp)` 索引
- 密钥表：加密存储 Megolm 会话密钥
- 设备表：存储设备密钥签名链
- Tantivy 索引：消息内容全文索引（独立于 SQLite）

## 待定事项

- DHT 实现选型（libp2p Kademlia? 自己实现?）
- Protobuf schema 具体定义
- WebSocket 库选型（tokio-tungstenite? async-tungstenite?）
- 推送通知集成（FCM/APNs）

## 下一步

→ 进入技术规划阶段，细化各层的具体实现方案
