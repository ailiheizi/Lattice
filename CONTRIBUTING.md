# Contributing to Lattice

感谢你对 Lattice 项目的关注！我们欢迎各种形式的贡献，包括但不限于：

- 报告 Bug
- 提出新功能建议
- 提交代码改进
- 完善文档
- 分享使用经验

## 行为准则

参与本项目即表示你同意遵守我们的行为准则：

- 尊重所有贡献者
- 接受建设性批评
- 关注对社区最有利的事情
- 对其他社区成员表现出同理心

## 如何贡献

### 报告 Bug

如果你发现了 Bug，请通过 GitHub Issues 报告，并提供以下信息：

1. **Bug 描述** - 清晰简洁地描述问题
2. **复现步骤** - 详细的复现步骤
3. **期望行为** - 你期望发生什么
4. **实际行为** - 实际发生了什么
5. **环境信息** - 操作系统、Rust 版本等
6. **日志输出** - 相关的错误日志或堆栈跟踪

**Bug 报告模板：**

```markdown
## Bug 描述
[简要描述问题]

## 复现步骤
1. 启动 Store 节点
2. 发送消息到 room-1
3. 观察到错误

## 期望行为
消息应该被成功存储并转发

## 实际行为
收到错误: "connection refused"

## 环境信息
- OS: Windows 11
- Rust: 1.75.0
- Lattice: 0.1.0

## 日志输出
```
[ERROR] Failed to connect to peer: connection refused
```
```

### 提出新功能

如果你有新功能的想法，请先通过 GitHub Issues 讨论：

1. **功能描述** - 清晰描述你想要的功能
2. **使用场景** - 为什么需要这个功能
3. **实现建议** - 如果有的话，提供实现思路
4. **替代方案** - 是否考虑过其他方案

### 提交代码

#### 开发流程

1. **Fork 仓库**
   ```bash
   # 在 GitHub 上 Fork 本仓库
   git clone https://github.com/your-username/Lattice.git
   cd Lattice
   ```

2. **创建分支**
   ```bash
   git checkout -b feature/your-feature-name
   # 或
   git checkout -b fix/your-bug-fix
   ```

3. **开发和测试**
   ```bash
   # 编写代码
   # 运行测试
   cargo test --workspace

   # 代码检查
   cargo clippy --workspace

   # 格式化代码
   cargo fmt --all
   ```

4. **提交更改**
   ```bash
   git add .
   git commit -m "feat: add amazing feature"
   ```

5. **推送到 GitHub**
   ```bash
   git push origin feature/your-feature-name
   ```

6. **创建 Pull Request**
   - 在 GitHub 上创建 Pull Request
   - 填写 PR 描述模板
   - 等待代码审查

#### Commit 规范

我们使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Type 类型：**
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式（不影响功能）
- `refactor`: 重构（既不是新功能也不是 Bug 修复）
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建过程或辅助工具的变动

**Scope 范围：**
- `crypto`: 加密模块
- `storage`: 存储模块
- `transport`: 传输模块
- `store`: Store 节点
- `peer`: Peer 节点
- `ffi`: FFI 绑定
- `tests`: 测试

**示例：**
```bash
feat(crypto): add Megolm session key rotation

Implement automatic key rotation for Megolm group sessions:
- Rotate after 100 messages
- Rotate after 7 days
- Rotate when member leaves

Closes #123
```

#### 代码规范

1. **Rust 代码风格**
   - 遵循 Rust 官方代码风格
   - 运行 `cargo fmt` 格式化代码
   - 运行 `cargo clippy` 检查代码质量
   - 所有 clippy 警告必须修复

2. **命名规范**
   - 类型名：`PascalCase`
   - 函数名：`snake_case`
   - 常量名：`SCREAMING_SNAKE_CASE`
   - 模块名：`snake_case`

3. **文档注释**
   - 公开 API 必须有文档注释
   - 使用 `///` 为函数和类型添加文档
   - 使用 `//!` 为模块添加文档
   - 提供使用示例

   ```rust
   /// 创建新的身份密钥对
   ///
   /// # Examples
   ///
   /// ```
   /// use lattice_crypto::identity::MasterKeyPair;
   ///
   /// let keypair = MasterKeyPair::generate();
   /// let fingerprint = keypair.fingerprint();
   /// ```
   pub fn generate() -> Self {
       // ...
   }
   ```

4. **错误处理**
   - 使用 `Result<T, E>` 返回可能失败的操作
   - 使用 `thiserror` 定义错误类型
   - 提供有意义的错误信息

5. **测试要求**
   - 新功能必须有单元测试
   - Bug 修复必须有回归测试
   - 测试覆盖率不应降低
   - 集成测试覆盖关键流程

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_feature_name() {
           // Arrange
           let input = setup_test_data();

           // Act
           let result = function_under_test(input);

           // Assert
           assert_eq!(result, expected_value);
       }
   }
   ```

#### Pull Request 检查清单

在提交 PR 之前，请确保：

- [ ] 代码通过 `cargo test --workspace`
- [ ] 代码通过 `cargo clippy --workspace`（无警告）
- [ ] 代码已格式化 `cargo fmt --all`
- [ ] 添加了必要的测试
- [ ] 更新了相关文档
- [ ] Commit 信息符合规范
- [ ] PR 描述清晰完整

#### Pull Request 模板

```markdown
## 变更类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 重构
- [ ] 文档更新
- [ ] 性能优化

## 变更描述
[清晰描述你的变更]

## 相关 Issue
Closes #[issue number]

## 测试
- [ ] 添加了单元测试
- [ ] 添加了集成测试
- [ ] 手动测试通过

## 检查清单
- [ ] 代码通过所有测试
- [ ] 代码通过 clippy 检查
- [ ] 代码已格式化
- [ ] 文档已更新
- [ ] CHANGELOG 已更新

## 截图（如适用）
[添加截图]

## 额外说明
[其他需要说明的内容]
```

### 代码审查

所有 PR 都需要经过代码审查：

1. **审查重点**
   - 代码正确性
   - 代码可读性
   - 性能影响
   - 安全性
   - 测试覆盖

2. **审查流程**
   - 提交 PR 后，维护者会进行审查
   - 根据反馈修改代码
   - 所有讨论解决后，PR 会被合并

3. **审查礼仪**
   - 保持友好和建设性
   - 解释你的建议原因
   - 接受不同的观点

## 开发环境设置

### 前置要求

- Rust 1.75+ (使用 rustup 安装)
- Git
- 文本编辑器或 IDE (推荐 VS Code + rust-analyzer)

### 克隆仓库

```bash
git clone https://github.com/yourusername/Lattice.git
cd Lattice
```

### 构建项目

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release
```

### 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定 crate 的测试
cargo test -p lattice-crypto

# 运行特定测试
cargo test test_name

# 显示测试输出
cargo test -- --nocapture
```

### 代码检查

```bash
# 运行 clippy
cargo clippy --workspace

# 自动修复 clippy 建议
cargo clippy --workspace --fix

# 格式化代码
cargo fmt --all

# 检查格式
cargo fmt --all -- --check
```

### 调试

```bash
# 启用日志
RUST_LOG=debug cargo run --bin lattice-store

# 使用 rust-gdb 调试
rust-gdb target/debug/lattice-store
```

## 项目结构

```
lattice/
├── crates/              # Rust crates
│   ├── lattice-proto/    # Protobuf 定义
│   ├── lattice-crypto/   # 加密层
│   ├── lattice-core/     # 核心逻辑
│   ├── lattice-transport/# 传输层
│   ├── lattice-storage/  # 存储层
│   ├── lattice-discovery/# 节点发现
│   ├── lattice-store/    # Store 节点
│   ├── lattice-peer/     # Peer 节点
│   ├── lattice-ffi/      # FFI 绑定
│   └── lattice-tests/    # 集成测试
├── proto/               # Protobuf 源文件
├── web/                 # Web 前端
├── android/             # Android demo
├── docs/                # 文档
└── tests/               # 额外测试
```

## 发布流程

（仅限维护者）

1. 更新版本号（Cargo.toml）
2. 更新 CHANGELOG.md
3. 创建 Git tag
4. 推送到 GitHub
5. 创建 GitHub Release
6. 发布到 crates.io（如适用）

## 获取帮助

如果你有任何问题：

- 查看 [文档](./docs/)
- 搜索 [已有 Issues](https://github.com/yourusername/Lattice/issues)
- 创建新的 Issue
- 加入讨论区

## 许可证

通过贡献代码，你同意你的贡献将在 MIT OR Apache-2.0 双许可证下发布。

---

再次感谢你的贡献！🎉
