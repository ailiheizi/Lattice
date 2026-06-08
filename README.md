# Lattice

> 基于 Rust 的去中心化即时通讯系统 —— 用户自托管 Store 节点，消息端到端签名，支持中文全文搜索。

Lattice 是一个 Rust workspace，包含 13 个 crate，覆盖协议、加密、存储、传输、节点二进制与 FFI。设计参考了 [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) 的 sans-I/O 核心 + Trait 驱动分层模式。

**当前状态**：核心链路（消息收发、存储、转发、房间事件同步、全文搜索、DHT 地址发现 fallback）已实现并通过集成测试；E2EE 的运行时集成仍在收口中（见 [完成度](#完成度)）。本文所述能力均与代码实际状态一致，未闭环的能力会明确标注。

- 蓝图主锚点：[`docs/plans/2026-03-18-feat-lattice-implementation-plan.md`](docs/plans/2026-03-18-feat-lattice-implementation-plan.md)
- 契约事实源：[`.plans/lattice-dev/docs/api-contracts.md`](.plans/lattice-dev/docs/api-contracts.md)
- 剩余缺口清单：[`.plans/lattice-dev/docs/gap-remediation.md`](.plans/lattice-dev/docs/gap-remediation.md)

---

## 目录

- [设计理念](#设计理念)
- [架构](#架构)
- [完成度](#完成度)
- [快速开始](#快速开始)
- [配置](#配置)
- [API 概览](#api-概览)
- [项目结构](#项目结构)
- [Web 前端](#web-前端)
- [测试](#测试)
- [文档导航](#文档导航)
- [安全模型](#安全模型)
- [贡献](#贡献)
- [许可证](#许可证)

---

## 设计理念

Lattice 不依赖中心化服务器。每个用户运行（或信任）一个 **Store 节点** 来存储和转发自己的消息：

- **去中心化**：没有全局服务器，节点之间通过 WebSocket + Protobuf 直连转发。
- **自托管优先**：联系人地址（`store_address`）是消息路由的主事实源，节点身份由密钥指纹标识。
- **签名先行**：所有消息使用 Ed25519 签名；信任分 Public / TOFU / Verified 三档。
- **可选 E2EE**：基于 vodozemac 的 Olm（1v1）/ Megolm（群聊）加密原语已就绪，运行时联调仍在收口。
- **可插拔实现**：`lattice-core` 只定义 Trait（Transport / Storage / SearchIndex），具体实现可替换。

---

## 架构

```
┌──────────┐        WebSocket         ┌──────────┐        WebSocket         ┌──────────┐
│  Store   │◄────────(转发)──────────►│   Peer   │◄────────(转发)──────────►│  Store   │
│  Node A  │                          │  Relay   │                          │  Node B  │
│          │                          │          │                          │          │
│ WS+REST  │                          │ 缓存+中转 │                          │ WS+REST  │
│ SQLite   │                          │ 管理 API  │                          │ SQLite   │
│ Tantivy  │                          └──────────┘                          │ Tantivy  │
└────┬─────┘                                                                └─────┬────┘
     │ REST                                                                       │ REST
┌────▼─────┐                                                                ┌─────▼────┐
│  Client  │                                                                │  Client  │
│ Web/FFI  │                                                                │ Web/FFI  │
└──────────┘                                                                └──────────┘
```

- **Store 节点**（`lattice-store`）：同时启动 WebSocket 服务（节点间通信）与 REST API（客户端调用）。负责消息持久化、签名校验、向接收方 Store 转发、全文搜索、房间事件存储与同步。
- **Peer 节点**（`lattice-peer`）：relay 中转节点，提供短时缓存（TTL + 容量上限）、超时转投代收 Store、以及统计/连接/缓存的管理 API。
- **Client**：Web 前端（聊天、Store 管理台、Peer 监控）与 Android FFI 绑定。

### 分层依赖

```
lattice-store / lattice-peer / lattice-ffi   ← 二进制 + 绑定层（装配）
            │
        lattice-core                       ← Trait 定义（sans-I/O，不依赖具体实现）
       ╱     │      ╲
lattice-    lattice-   lattice-               ← Trait 实现层
transport  storage   crypto
       ╲     │      ╱
        lattice-proto                       ← Protobuf 生成类型
```

规则：`lattice-core` 只定义 Trait 不依赖实现；各实现 crate(`transport-ws` / `storage-sqlite` / `search-tantivy` / `crypto-olm`)实现对应 Trait；二进制 crate 通过 Cargo feature + 类型别名(`ActiveStorage`/`ActiveSearch`)配置式装配选定的后端。详见 [`.plans/lattice-dev/docs/architecture.md`](.plans/lattice-dev/docs/architecture.md)。

---

## 完成度

以下分级以**代码实际状态 + 集成测试是否覆盖**为准，不以模块是否存在为准。

| 能力 | 状态 | 说明 |
|------|------|------|
| Cargo workspace / Protobuf 生成 | ✅ 已落地 | 13 crate 编译通过，proto 类型生成正常 |
| 身份与签名（Ed25519 / Curve25519 / SHA-256 指纹） | ✅ 已落地 | `lattice-crypto`，24 单元测试 |
| 三档信任模型（Public / TOFU / Verified） | ✅ 已落地 | `lattice-crypto/trust.rs` |
| SQLite 存储（消息/房间/联系人/设备/密钥/房间事件） | ✅ 已落地 | `lattice-storage`，19 测试 |
| Tantivy 全文搜索（含 CJK 分词） | ✅ 已落地 | `lattice-search-tantivy` |
| WebSocket 传输（Frame 编解码 / 心跳） | ✅ 已落地 | `lattice-transport`，3 测试 |
| Store REST + WebSocket 服务 | ✅ 已落地 | `lattice-store`，REST 路由见下文 |
| Store→Store / Store→Peer 消息转发（含 ACK 超时、proxy fallback） | ✅ 已落地 | 集成测试覆盖转发与超时重试 |
| **房间事件运行时**（ROOM_EVENT 落库 + sync 回放） | ✅ 已落地 | 端到端测试 `real_ws_server_stores_and_syncs_room_events` |
| Peer relay + 缓存 + 管理 API | ✅ 已落地 | `lattice-peer`，14 测试 |
| Peer 可观测性（relayed/delivered/error/latency/connections） | ✅ 已落地 | `observability.rs` + `/stats` 等接口 |
| 真实集成测试（起 WS/REST 服务的端到端） | ✅ 已落地 | `lattice-tests`，7 集成测试 |
| Android FFI（UniFFI 绑定） | 🟡 部分 | 接口已暴露，8 测试；未做真机 demo 验证 |
| E2EE 加密原语（Olm / Megolm） | 🟡 部分 | 加密/解密/序列化已实现；1v1 Olm 运行时已闭环（见下行），群组 Megolm 运行时未做 |
| **1v1 E2EE 运行时**（预密钥 claim + Olm 会话编排 + 端到端） | ✅ 已落地 | `/keys/bundle`+`/keys/claim`、`lattice_crypto::session::OlmSessionManager`、端到端测试 `e2ee_1v1_roundtrip_through_real_store`（密文经真实 Store 转发后对端解密，Store 只见密文） |
| 多设备注册与发现 | 🟡 部分 | 设备注册/列表 REST 端点已落地（`/devices`），同账号设备发现可用；密钥跨设备分发未做 |
| 多设备密钥同步（重加密/收敛） | 🟠 待收口 | `DeviceManager` + Storage device 接口就绪，缺跨设备密钥分发与冲突收敛 |
| DHT 节点发现 | 🟡 部分 | WebSocket DHT 服务已接入 store 运行时:节点 publish 签名身份卡片、转发缺地址时 lookup 作 fallback(`enable_dht`)。验签防伪造。未做完整 Kademlia 迭代查询 |
| STUN / NAT 穿透 | 🔴 未实现 | 蓝图规划中,无运行时实现 |

图例：✅ 已落地并验证 · 🟡 部分实现 · 🟠 待收口 · 🔴 未实现

---

## 快速开始

### 前置要求

- Rust 1.75+（推荐 [rustup](https://rustup.rs) 安装）
- 操作系统：Windows / Linux / macOS

### 构建与测试

```bash
git clone <repo-url> Lattice
cd Lattice

# 编译所有组件
cargo build --release

# 运行全部测试（193 个）
cargo test --workspace
```

### 运行 Store 节点

```bash
# 1. 复制配置模板
cp lattice-store.example.toml lattice-store.toml

# 2. 按需编辑 lattice-store.toml（见下方配置说明）

# 3. 启动
cargo run --release --bin lattice-store
```

启动后默认监听：WebSocket `0.0.0.0:9100`、REST API `0.0.0.0:9101`。

```bash
# 健康检查
curl http://localhost:9101/health
```

### 运行 Peer 节点

```bash
cp lattice-peer.example.toml lattice-peer.toml
cargo run --release --bin lattice-peer
```

默认监听：relay `0.0.0.0:9200`、管理 API `0.0.0.0:9201`。

> ⚠️ **安全提示**：Store 的 REST **写接口**（POST/DELETE）已要求 `Authorization: Bearer <api_token>`，只读接口（health/identity/消息读取/搜索）公开。Peer 管理 API 与节点间 WebSocket 目前仍**无内置鉴权**，且默认无 TLS。生产部署务必置于反向代理之后并启用 TLS + 访问控制，详见 [`docs/deployment.md`](docs/deployment.md)。

---

## 配置

配置字段以代码实际读取为准（`crates/lattice-store/src/main.rs`、`crates/lattice-peer/src/main.rs`）。

### Store（`lattice-store.example.toml`）

| 字段 | 说明 | 示例 |
|------|------|------|
| `ws_addr` | WebSocket 服务监听地址（节点间通信） | `0.0.0.0:9100` |
| `api_addr` | REST API 监听地址（客户端调用） | `0.0.0.0:9101` |
| `data_dir` | 数据目录（SQLite + Tantivy 索引） | `./data` |
| `display_name` | 节点显示名称 | `My Store` |
| `proxy_store_address` | 代收 Store 地址（离线消息暂存，留空禁用） | `""` |
| `api_token` | REST 写接口的 Bearer token（留空则启动自动生成并打印） | `""` |

### Peer（`lattice-peer.example.toml`）

| 字段 | 说明 | 示例 |
|------|------|------|
| `listen_addr` | relay 监听地址 | `0.0.0.0:9200` |
| `api_addr` | 管理 API 监听地址 | `0.0.0.0:9201` |
| `max_cache_entries` | 缓存最大条目数 | `10000` |
| `cache_ttl_ms` | 缓存 TTL（毫秒），超时转投代收 Store | `3600000` |
| `eviction_interval_ms` | 超时检查间隔（毫秒） | `60000` |
| `proxy_stores` | 代收 Store 地址列表（超时消息转投目标） | `[]` |

---

## API 概览

> 这是概览。精确请求/响应字段以代码入口 `crates/lattice-store/src/api.rs`、`crates/lattice-peer/src/api.rs` 与契约文档 [`.plans/lattice-dev/docs/api-contracts.md`](.plans/lattice-dev/docs/api-contracts.md) 为准。完整 REST/WS 说明见 [`docs/api.md`](docs/api.md)。

### Store REST API（默认 `:9101`）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/identity` | 获取节点身份信息 |
| POST | `/messages` | 发送消息 |
| GET | `/messages/:room_id` | 获取房间消息 |
| GET | `/messages/id/:msg_id` | 按 ID 获取单条消息 |
| DELETE | `/messages/id/:msg_id` | 删除消息 |
| GET | `/search?q=<query>` | 全文搜索（支持中文） |
| GET / POST | `/contacts` | 列出 / 添加联系人 |
| GET / DELETE | `/contacts/:fingerprint` | 获取 / 删除联系人 |
| GET / POST | `/rooms` | 列出 / 创建房间 |
| GET | `/rooms/:room_id` | 房间详情 |
| POST | `/rooms/:room_id/members` | 添加成员 |
| POST | `/rooms/:room_id/leave` | 离开房间 |
| GET | `/keys/one-time` | 获取一次性预密钥 |
| POST | `/keys/generate` | 生成一次性预密钥 |
| POST | `/keys/bundle` | 上传自己的预密钥包（E2EE，写接口需 token） |
| GET | `/keys/claim/:fingerprint` | claim 目标用户预密钥（消费一个 OTK，耗尽回退 fallback） |
| POST | `/devices` | 注册设备到当前用户（同 ID 重复返回 409） |
| GET | `/devices/:user_fingerprint` | 列出该用户已注册设备（多设备发现） |

### Peer 管理 API（默认 `:9201`）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/stats` | 统计指标（relayed / delivered / latency / connections / error / uptime） |
| GET | `/connections` | 活跃连接列表 |
| GET | `/cache` | 缓存状态 |
| GET | `/config` | 当前运行配置 |

### WebSocket 协议

Store 与 Peer 节点间使用 WebSocket + Protobuf 通信。帧结构以 [`proto/transport.proto`](proto/transport.proto) 中的 `Frame` 为准（`lattice_proto::transport::Frame`），支持的 `FrameType`：`MESSAGE` / `ACK` / `KEY_BUNDLE` / `KEY_REQUEST` / `ROOM_EVENT` / `SYNC_REQUEST` / `SYNC_RESPONSE` / `PING` / `PONG`。

> 旧文档中的简化版 `Frame { type, payload }` 已废弃，不再代表实际协议。

---

## 节点变体（配置式编译）

节点由 Cargo feature + 类型别名在编译期组装,无需运行时插件/动态加载。每个后端维度(storage / search)必须恰好选一个实现,选零个或多个在编译期 `compile_error!` 报错。

| 变体 | 组成 | 编译命令 |
|------|------|----------|
| **store-full**（默认） | sqlite + tantivy 搜索 + Olm/Megolm + DHT(运行时开关) | `cargo build --release -p lattice-store` |
| **store-light** | sqlite + 空搜索(`NoopSearch`,搜索恒空、零依赖) | `cargo build --release -p lattice-store --no-default-features --features "storage-sqlite,search-noop"` |
| **relay-light**（lattice-peer） | 内存中转缓存 + STUN + 签名校验,无持久化/搜索/E2EE | `cargo build --release -p lattice-peer` |

加新后端 = 新实现(crate 或模块)+ 新 feature + 一条 `#[cfg]` 类型别名,上层(`AppState`/工厂/测试)引用的 `ActiveStorage`/`ActiveSearch` 别名不变。

> **部署者无需自行编译**:打 `v*` tag 会触发 [`.github/workflows/release.yml`](.github/workflows/release.yml),自动为 linux / windows / macos 各编出 `lattice-store-full`、`lattice-store-light`、`lattice-peer` 预编译二进制并发布到 GitHub Release。直接下载对应平台与变体的产物即可运行,行为由 `*.toml` 配置文件的运行时开关(DHT / 限流 / 准入 / token)控制。

---

## 项目结构

```
Lattice/
├── Cargo.toml                    # Workspace 根配置
├── proto/                        # Protobuf 定义
│   ├── identity.proto            # 身份、设备、密钥包
│   ├── message.proto             # 信封、消息内容、加密载荷
│   ├── group.proto               # 房间、成员、房间事件
│   └── transport.proto           # WebSocket Frame
├── crates/
│   ├── lattice-proto/             # Protobuf 生成代码（prost）
│   ├── lattice-core/              # 核心逻辑 + Trait 定义（sans-I/O，最小内核）
│   ├── lattice-crypto/            # 基础密码学：身份、签名、信任
│   ├── lattice-crypto-olm/        # 会话编排实现：Olm(1v1) / Megolm(群组)
│   ├── lattice-transport-ws/      # Transport 实现：WebSocket
│   ├── lattice-storage-sqlite/    # Storage 实现：SQLite
│   ├── lattice-search-tantivy/    # SearchIndex 实现：Tantivy（含 CJK 分词）
│   ├── lattice-discovery/         # Kademlia DHT + WebSocket 发现服务(已接入 store fallback)
│   ├── lattice-store/             # Store 节点（二进制：server + relay + api，feature 装配后端）
│   ├── lattice-peer/              # Peer 节点（二进制：relay + cache + observability）
│   ├── lattice-ffi/               # Android FFI（UniFFI）
│   └── lattice-tests/             # 跨节点集成测试
├── web/                          # Web 前端（见下文）
├── docs/                         # 公开文档
└── .plans/lattice-dev/            # 开发控制面（契约/架构/进度/缺口）
```

---

## Web 前端

`web/` 下有三个独立前端，均通过真实 REST/WebSocket 接口连接节点（非 mock）：

| 目录 | 用途 | 默认连接 |
|------|------|----------|
| [`web/chat/`](web/chat/) | 聊天客户端 | Store REST + WS |
| [`web/store-admin/`](web/store-admin/) | Store 管理台（消息/联系人/房间/日志） | Store REST `:9101` |
| [`web/peer-monitor/`](web/peer-monitor/) | Peer 监控面板（统计/连接/缓存图表） | Peer API `:9201` |

各前端为纯静态页面，用浏览器打开对应 `index.html` 并在界面中配置节点地址即可。

---

## 测试

```bash
# 全部测试（193 个，全绿）
cargo test --workspace

# 单个 crate
cargo test -p lattice-crypto

# 集成测试（起真实 WS/REST 服务）
cargo test -p lattice-tests
```

各 crate 测试分布（`cargo test --workspace` 实测）：

| Crate | 测试数 | 覆盖 |
|-------|-------|------|
| lattice-crypto | 37 | 基础密码学:密钥生成、签名验证、信任 |
| lattice-core | 37 | 消息/房间/联系人/设备/DAG/限流核心逻辑 |
| lattice-crypto-olm | 22 | 会话编排:Olm/Megolm、1v1 Olm + 群组 Megolm |
| lattice-storage-sqlite | 12 | SQLite CRUD、房间事件、密钥包 |
| lattice-search-tantivy | 10 | Tantivy 全文搜索、CJK 分词 |
| lattice-peer | 18 | relay、缓存、可观测性、转投重试 |
| lattice-store | 18 | frame 处理、转发、房间事件、REST 路由 |
| lattice-tests | 15 | 跨节点 WS/REST 端到端 + 多设备 + 1v1 E2EE |
| lattice-discovery | 13 | Kademlia 路由表、身份卡片签名 |
| lattice-ffi | 8 | FFI 绑定 |
| lattice-transport-ws | 3 | WebSocket 编解码 |
| **总计** | **193** | 单元 + 集成 |

> 集成测试（`lattice-tests`）会真实启动 WebSocket 服务器和 REST 路由，验证消息存储/同步、房间事件回放、加密载荷透传、跨 Store 转发等链路。

---

## 文档导航

### 公开文档（`docs/`）

- [实现计划（蓝图主锚点）](docs/plans/2026-03-18-feat-lattice-implementation-plan.md) — 项目结构、核心 Trait、Protobuf schema、开发里程碑
- [消息完整性 / 哈希 DAG 设计](docs/plans/2026-06-04-design-message-integrity-dag.md) — 签名链、DAG 全序、并发/缺失处理
- [Matrix 能力差距与路线图](docs/plans/2026-06-06-matrix-gap-and-roadmap.md) — 对比 Matrix 的能力清单、防骚扰准入设计、推进顺序
- [E2EE 运行时设计](docs/plans/2026-06-07-design-e2ee-runtime.md) — Olm/Megolm 运行时编排、密钥协商/分发/轮换、分阶段路线
- [架构脑暴](docs/brainstorms/2026-03-17-lattice-architecture-brainstorm.md) — 早期架构设计讨论
- [API 文档](docs/api.md) — REST / WebSocket 接口概览
- [部署指南](docs/deployment.md) — 单/多节点部署、systemd、Docker、TLS、备份、排障
- [开发指南](docs/development.md) — 环境搭建、调试、性能分析、常见任务

### 开发控制面（`.plans/lattice-dev/docs/`）

> 这是开发期间维护的「真实状态事实源」，比公开文档更精确：

- [架构边界](.plans/lattice-dev/docs/architecture.md)
- [API / 配置契约](.plans/lattice-dev/docs/api-contracts.md)
- [剩余缺口与修复顺序](.plans/lattice-dev/docs/gap-remediation.md)
- [不可破坏约束](.plans/lattice-dev/docs/invariants.md)
- [文档索引](.plans/lattice-dev/docs/index.md)

---

## 安全模型

### 加密与签名

- **身份密钥**：Ed25519（签名）+ Curve25519（加密）
- **消息签名**：Ed25519 签名 + SHA-256 完整性校验
- **E2EE**（原语就绪，运行时收口中）：Olm（1v1，Double Ratchet）/ Megolm（群聊）

### 信任模型

| 等级 | 行为 |
|------|------|
| `Public` | 接受所有消息（不推荐） |
| `TOFU` | 首次信任，后续校验指纹（Trust On First Use） |
| `Verified` | 仅接受已验证指纹的消息 |

### 部署安全注意

- Store REST 写接口需 Bearer token（`api_token`）；Peer 管理 API 与节点间 WebSocket 暂无鉴权——**生产环境必须**置于带 TLS 与访问控制的反向代理后。
- 消息路由依赖联系人中显式保存的 `store_address`，请确认地址来源可信。
- 详见 [`docs/deployment.md`](docs/deployment.md) 的安全配置章节。

---

## 贡献

1. Fork 并创建特性分支（`git checkout -b feature/xxx`）
2. 开发 + 补测试 + 同步文档
3. 本地验证：
   ```bash
   cargo fmt --all
   cargo clippy --workspace
   cargo test --workspace
   ```
4. 提交 PR

代码规范遵循 Rust 官方风格；新功能需附测试；改动 API/配置/架构时，代码与 [`.plans/lattice-dev/docs/`](.plans/lattice-dev/docs/) 须同步更新。

---

## 许可证

双许可，任选其一：

- Apache License 2.0（[`LICENSE-APACHE`](LICENSE-APACHE)）
- MIT License（[`LICENSE-MIT`](LICENSE-MIT)）

## 致谢

- [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) — workspace 组织与 sans-I/O 分层参考
- [vodozemac](https://github.com/matrix-org/vodozemac) — Olm/Megolm 加密库
- [Tantivy](https://github.com/quickwit-oss/tantivy) — 全文搜索引擎
