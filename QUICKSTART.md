# Lattice 快速开始指南

## 🚀 5 分钟快速上手

### 前置要求

- Rust 1.75+ (使用 rustup 安装)
- Python 3.x (用于启动 Web 服务器)
- 操作系统: Windows / Linux / macOS

### 步骤 1: 编译项目

```bash
cd D:/windows/code/project/Lattice
cargo build --release
```

### 步骤 2: 启动 Store 节点

```bash
# 复制配置文件
cp lattice-store.example.toml lattice-store.toml

# 启动 Store 节点
cargo run --release --bin lattice-store
```

Store 节点将在 `http://127.0.0.1:9100` 启动。

### 步骤 3: 启动 Peer 节点（可选）

```bash
# 在新终端中
cp lattice-peer.example.toml lattice-peer.toml
cargo run --release --bin lattice-peer
```

Peer 节点将在 `http://127.0.0.1:9200` 启动。

### 步骤 4: 启动 Web 服务器

```bash
# 在新终端中
cd web
python -m http.server 8080
```

### 步骤 5: 访问可视化面板

打开浏览器访问：

- **Web 聊天界面**: http://localhost:8080/chat/
- **Store 管理面板**: http://localhost:8080/store-admin/
- **Peer 监控面板**: http://localhost:8080/peer-monitor/

### 步骤 6: 开始聊天

1. 在 Web 聊天界面中输入用户名（如 `alice`）
2. 连接到 Store 节点（默认已配置）
3. 创建房间或添加联系人
4. 开始发送消息！

## 🎯 使用示例

### 发送消息

1. 在左侧边栏选择或创建房间
2. 在底部输入框输入消息
3. 按 Enter 发送

### 搜索消息

1. 点击顶部搜索图标
2. 输入关键词
3. 查看搜索结果

### 管理联系人

1. 点击左侧"联系人"标签
2. 点击"添加联系人"按钮
3. 输入联系人指纹和名称

### 查看 Store 状态

1. 访问 Store 管理面板
2. 查看实时统计数据
3. 管理消息、联系人和房间

## 📖 更多信息

- **完整文档**: 查看 `docs/` 目录
- **API 文档**: 查看 `docs/api.md`
- **部署指南**: 查看 `docs/deployment.md`
- **开发指南**: 查看 `docs/development.md`

## 🆘 常见问题

### Q: Store 节点无法启动？
A: 检查端口 9100 是否被占用，或修改配置文件中的端口。

### Q: Web 界面无法连接？
A: 确保 Store 节点正在运行，并检查浏览器控制台的错误信息。

### Q: 如何启用 E2EE 加密？
A: 在发送消息时勾选"加密"选项，系统会自动处理密钥交换。

## 🎉 开始使用 Lattice！

现在你已经成功启动了 Lattice，可以开始探索去中心化即时通讯的世界了！

**Happy Chatting!** 💬
