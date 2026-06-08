# Lattice 开发指南

本文档为 Lattice 项目的开发者提供详细的开发指南。

## 目录

- [开发环境设置](#开发环境设置)
- [项目架构](#项目架构)
- [核心概念](#核心概念)
- [开发工作流](#开发工作流)
- [测试指南](#测试指南)
- [调试技巧](#调试技巧)
- [性能分析](#性能分析)
- [常见任务](#常见任务)

## 开发环境设置

### 1. 安装 Rust

```bash
# 使用 rustup 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装稳定版
rustup install stable
rustup default stable

# 验证安装
rustc --version
cargo --version
```

### 2. 安装开发工具

```bash
# 代码格式化工具
rustup component add rustfmt

# 代码检查工具
rustup component add clippy

# 文档生成工具
cargo install cargo-doc

# 测试覆盖率工具
cargo install cargo-tarpaulin

# 性能分析工具
cargo install cargo-flamegraph

# Android 交叉编译工具
cargo install cargo-ndk
```

### 3. IDE 配置

**VS Code 推荐插件：**
- rust-analyzer
- CodeLLDB (调试)
- Better TOML
- Error Lens
- GitLens

**VS Code 配置 (settings.json)：**
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### 4. 克隆项目

```bash
git clone https://github.com/yourusername/Lattice.git
cd Lattice

# 构建项目
cargo build

# 运行测试
cargo test --workspace
```

## 项目架构

### Workspace 结构

```
lattice/
├── Cargo.toml                    # Workspace 根配置
├── proto/                        # Protobuf 定义
├── crates/
│   ├── lattice-proto/             # Protobuf 生成代码
│   ├── lattice-crypto/            # 加密层
│   ├── lattice-core/              # 核心逻辑
│   ├── lattice-transport/         # 传输层
│   ├── lattice-storage/           # 存储层
│   ├── lattice-discovery/         # 节点发现
│   ├── lattice-store/             # Store 节点
│   ├── lattice-peer/              # Peer 节点
│   ├── lattice-ffi/               # FFI 绑定
│   └── lattice-tests/             # 集成测试
└── docs/                         # 文档
```

### 依赖关系

```
lattice-store
    ├── lattice-core
    │   ├── lattice-proto
    │   └── lattice-crypto
    ├── lattice-transport
    │   └── lattice-proto
    ├── lattice-storage
    │   └── lattice-proto
    └── lattice-discovery

lattice-peer
    ├── lattice-proto
    └── lattice-transport

lattice-ffi
    ├── lattice-core
    ├── lattice-crypto
    ├── lattice-storage
    └── lattice-proto
```

### 模块职责

| Crate | 职责 | 关键类型 |
|-------|------|---------|
| lattice-proto | Protobuf 定义和生成代码 | Message, Frame, Identity |
| lattice-crypto | 加密、签名、信任模型 | MasterKeyPair, OlmAccount, TrustLevel |
| lattice-core | 核心业务逻辑和 Trait 定义 | Transport, Storage, SearchIndex |
| lattice-transport | WebSocket 传输实现 | WebSocketTransport |
| lattice-storage | SQLite 存储和 Tantivy 搜索 | SqliteStorage, TantivySearch |
| lattice-discovery | DHT 节点发现 | DhtNode, NodeId |
| lattice-store | Store 节点二进制 | main, server, api |
| lattice-peer | Peer 节点二进制 | main, relay, cache |
| lattice-ffi | Android FFI 绑定 | LatticeClient |
| lattice-tests | 集成测试 | store_api, store_communication |

## 核心概念

### 1. Trait 抽象

Lattice 使用 Trait 实现可插拔的组件：

```rust
// Lattice 使用 Rust 1.75+ 原生 async fn in trait（不依赖 async-trait crate）

// 传输层 Trait
pub trait Transport: Send + Sync {
    fn connect(&mut self, addr: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    fn send(&self, data: &[u8]) -> impl std::future::Future<Output = Result<()>> + Send;
    fn recv(&mut self) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send;
    fn close(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn is_connected(&self) -> bool;
}

// 存储层 Trait
pub trait Storage: Send + Sync {
    fn save_message(&self, msg: &Message) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_messages(&self, room_id: &str, range: &TimeRange, page: &Pagination)
        -> impl std::future::Future<Output = Result<Vec<Message>>> + Send;
    // ...
}

// 搜索层 Trait
pub trait SearchIndex: Send + Sync {
    fn index_message(&self, msg: &Message) -> impl std::future::Future<Output = Result<()>> + Send;
    fn search(&self, query: &str, limit: usize)
        -> impl std::future::Future<Output = Result<Vec<SearchResult>>> + Send;
    // ...
}
```

### 2. 消息流程

```
发送方                Store A              Peer              Store B              接收方
  │                     │                   │                   │                     │
  ├─ 1. 创建消息 ──────►│                   │                   │                     │
  │                     ├─ 2. 签名验证      │                   │                     │
  │                     ├─ 3. 存储消息      │                   │                     │
  │                     ├─ 4. 转发 ────────►│                   │                     │
  │                     │                   ├─ 5. 缓存          │                     │
  │                     │                   ├─ 6. 转发 ────────►│                     │
  │                     │                   │                   ├─ 7. 验证签名        │
  │                     │                   │                   ├─ 8. 存储消息        │
  │                     │                   │                   ├─ 9. 推送 ──────────►│
  │◄─ ACK ──────────────┤                   │                   │                     │
```

### 3. 加密流程

**1v1 加密 (Olm):**
```
Alice                                                Bob
  │                                                   │
  ├─ 1. 生成 One-time key                            │
  ├─ 2. 发布到 Store                                 │
  │                                                   │
  │                                    3. 获取 OTK ◄─┤
  │                                    4. 建立会话 ◄─┤
  │                                                   │
  │◄─ 5. 加密消息 ─────────────────────────────────┤
  ├─ 6. 解密消息                                     │
  ├─ 7. 加密回复 ────────────────────────────────►│
  │                                    8. 解密回复 ◄─┤
```

**群聊加密 (Megolm):**
```
创建者                                              成员
  │                                                   │
  ├─ 1. 创建 Megolm 会话                            │
  ├─ 2. 生成会话密钥                                 │
  ├─ 3. 通过 Olm 分发密钥 ──────────────────────►│
  │                                                   │
  ├─ 4. 加密群消息 ──────────────────────────────►│
  │                                    5. 解密消息 ◄─┤
```

### 4. 信任模型

```rust
pub enum TrustLevel {
    Public,    // 接受所有消息
    TOFU,      // 首次接受，后续验证指纹
    Verified,  // 仅接受已验证指纹的消息
}
```

## 开发工作流

### 1. 创建新功能

```bash
# 创建特性分支
git checkout -b feature/new-feature

# 开发功能
# 1. 编写代码
# 2. 添加测试
# 3. 更新文档

# 运行测试
cargo test --workspace

# 代码检查
cargo clippy --workspace

# 格式化代码
cargo fmt --all

# 提交更改
git add .
git commit -m "feat: add new feature"

# 推送到远程
git push origin feature/new-feature
```

### 2. 修复 Bug

```bash
# 创建修复分支
git checkout -b fix/bug-description

# 1. 编写失败的测试（重现 Bug）
# 2. 修复代码
# 3. 验证测试通过

# 运行测试
cargo test --workspace

# 提交更改
git commit -m "fix: resolve bug description"
```

### 3. 重构代码

```bash
# 创建重构分支
git checkout -b refactor/component-name

# 1. 确保测试覆盖
# 2. 重构代码
# 3. 验证测试仍然通过

# 运行测试
cargo test --workspace

# 提交更改
git commit -m "refactor: improve component structure"
```

## 测试指南

### 1. 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = setup_test_data();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_function() {
        // 异步测试
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### 2. 集成测试

```rust
// crates/lattice-tests/src/store_api.rs

#[tokio::test]
async fn test_send_and_get_messages() {
    let url = start_test_store().await;
    let client = reqwest::Client::new();

    // 发送消息
    let resp = client.post(format!("{url}/messages"))
        .json(&serde_json::json!({
            "room_id": "test-room",
            "text": "hello"
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // 查询消息
    let resp = client.get(format!("{url}/messages/test-room"))
        .send().await.unwrap();
    let msgs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(msgs.len(), 1);
}
```

### 3. 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定 crate 的测试
cargo test -p lattice-crypto

# 运行特定测试
cargo test test_name

# 显示测试输出
cargo test -- --nocapture

# 运行忽略的测试
cargo test -- --ignored

# 并行运行测试
cargo test -- --test-threads=4
```

### 4. 测试覆盖率

```bash
# 安装 tarpaulin
cargo install cargo-tarpaulin

# 生成覆盖率报告
cargo tarpaulin --workspace --out Html

# 查看报告
open tarpaulin-report.html
```

## 调试技巧

### 1. 日志调试

```rust
use tracing::{debug, info, warn, error};

pub async fn process_message(msg: &Message) -> Result<()> {
    debug!("Processing message: {:?}", msg.msg_id);

    if msg.content.is_none() {
        warn!("Message has no content: {}", msg.msg_id);
        return Err(LatticeError::InvalidMessage);
    }

    info!("Message processed successfully: {}", msg.msg_id);
    Ok(())
}
```

启用日志：
```bash
# 设置日志级别
RUST_LOG=debug cargo run --bin lattice-store

# 只显示特定模块的日志
RUST_LOG=lattice_store::server=debug cargo run --bin lattice-store
```

### 2. 使用 dbg! 宏

```rust
fn calculate_distance(a: &NodeId, b: &NodeId) -> [u8; 32] {
    let dist = a.distance(b);
    dbg!(&dist);  // 打印调试信息
    dist
}
```

### 3. 使用调试器

**VS Code 配置 (.vscode/launch.json):**
```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug Store",
      "cargo": {
        "args": [
          "build",
          "--bin=lattice-store",
          "--package=lattice-store"
        ],
        "filter": {
          "name": "lattice-store",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}"
    }
  ]
}
```

### 4. 断点调试

```rust
// 在代码中设置断点
fn process_frame(frame: Frame) -> Result<()> {
    // 在这里设置断点
    let payload = frame.payload;
    // ...
}
```

## 性能分析

### 1. 基准测试

```rust
// benches/storage_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_save_message(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let storage = SqliteStorage::in_memory().unwrap();

    c.bench_function("save_message", |b| {
        b.iter(|| {
            rt.block_on(async {
                let msg = create_test_message();
                storage.save_message(&msg).await.unwrap();
            })
        })
    });
}

criterion_group!(benches, bench_save_message);
criterion_main!(benches);
```

运行基准测试：
```bash
cargo bench
```

### 2. 火焰图

```bash
# 安装工具
cargo install cargo-flamegraph

# 生成火焰图
cargo flamegraph --bin lattice-store

# 查看火焰图
open flamegraph.svg
```

### 3. 性能分析

```bash
# 使用 perf (Linux)
perf record -g cargo run --release --bin lattice-store
perf report

# 使用 Instruments (macOS)
instruments -t "Time Profiler" target/release/lattice-store
```

## 常见任务

### 1. 添加新的 Protobuf 消息

1. 编辑 `proto/*.proto` 文件
2. 重新构建 `lattice-proto`
3. 更新使用该消息的代码

```bash
cargo build -p lattice-proto
```

### 2. 添加新的 REST API 端点

在 `crates/lattice-store/src/api.rs` 中：

```rust
// 添加新的路由
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/new-endpoint", post(handle_new_endpoint))
        .with_state(state)
}

// 实现处理函数
async fn handle_new_endpoint(
    State(state): State<Arc<AppState>>,
    Json(req): Json<NewRequest>,
) -> Result<Json<NewResponse>, StatusCode> {
    // 实现逻辑
    Ok(Json(NewResponse { /* ... */ }))
}
```

### 3. 添加新的存储方法

在 `crates/lattice-core/src/traits/storage.rs` 中（原生 async fn in trait）：

```rust
pub trait Storage: Send + Sync {
    // 添加新方法
    fn new_method(&self, param: &str)
        -> impl std::future::Future<Output = Result<Data>> + Send;
}
```

在 `crates/lattice-storage/src/sqlite.rs` 中实现（直接用 `async fn`，无需宏）：

```rust
impl Storage for SqliteStorage {
    async fn new_method(&self, param: &str) -> Result<Data> {
        // 实现逻辑
    }
}
```

### 4. 添加新的测试

```rust
#[tokio::test]
async fn test_new_feature() {
    // 设置测试环境
    let storage = SqliteStorage::in_memory().unwrap();

    // 执行操作
    let result = storage.new_method("test").await;

    // 验证结果
    assert!(result.is_ok());
}
```

### 5. 更新依赖

```bash
# 检查过期的依赖
cargo outdated

# 更新依赖
cargo update

# 更新特定依赖
cargo update -p tokio
```

### 6. 生成文档

```bash
# 生成文档
cargo doc --workspace --no-deps

# 生成并打开文档
cargo doc --workspace --no-deps --open
```

## 代码审查检查清单

提交 PR 前检查：

- [ ] 代码通过所有测试
- [ ] 代码通过 clippy 检查（无警告）
- [ ] 代码已格式化
- [ ] 添加了必要的测试
- [ ] 更新了相关文档
- [ ] Commit 信息符合规范
- [ ] 没有调试代码（println!, dbg!）
- [ ] 没有未使用的导入
- [ ] 错误处理完善
- [ ] 性能影响已评估

## 常见问题

### Q: 如何添加新的 crate？

A: 在 `Cargo.toml` 的 `[workspace]` 中添加：
```toml
[workspace]
members = [
    "crates/new-crate",
    # ...
]
```

### Q: 如何处理异步代码？

A: 使用 `tokio` 运行时和 `async/await`：
```rust
#[tokio::main]
async fn main() {
    // 异步代码
}
```

### Q: 如何处理错误？

A: 使用 `Result<T, E>` 和 `?` 操作符：
```rust
pub async fn process() -> Result<()> {
    let data = fetch_data().await?;
    save_data(&data).await?;
    Ok(())
}
```

## 资源链接

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Tokio 文档](https://tokio.rs/)
- [Protobuf 文档](https://developers.google.com/protocol-buffers)
- [Matrix 协议规范](https://spec.matrix.org/)
- [Vodozemac 文档](https://docs.rs/vodozemac/)

---

**持续更新中，欢迎贡献！**
