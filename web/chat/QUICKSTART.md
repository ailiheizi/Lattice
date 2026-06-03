# NextIM Web Chat - Quick Start Guide

## 快速开始

### 1. 启动后端服务

在项目根目录运行：

```bash
# 启动 Store 服务（默认端口 9100）
cargo run --bin nextim-store

# 或使用配置文件
cargo run --bin nextim-store -- --config store.toml
```

### 2. 访问 Web 界面

有两种方式访问：

#### 方式 A：直接打开文件（推荐用于开发）

```
file:///D:/windows/code/project/NextIM/web/chat/index.html
```

#### 方式 B：使用 HTTP 服务器（推荐用于测试）

```bash
# 使用 Python
cd web/chat
python -m http.server 8080

# 或使用 Node.js
npx http-server -p 8080

# 然后访问
http://localhost:8080
```

### 3. 连接配置

1. 在左侧面板输入：
   - **Store API URL**: `http://127.0.0.1:9100`
   - **Username**: 任意用户名（如 `alice`）

2. 点击 **Connect** 按钮

3. 等待连接成功（顶部显示 "API OK" 和 "WS Live"）

### 4. 开始使用

#### 创建房间
1. 在 "Rooms" 标签下
2. 输入房间名称（如 "General"）
3. 点击 "Create Room"

#### 发送消息
1. 选择一个房间
2. 在底部输入框输入消息
3. 按 Enter 发送（Shift+Enter 换行）

#### 添加联系人
1. 切换到 "Contacts" 标签
2. 输入联系人指纹、名称和地址
3. 点击 "Add Contact"

#### 搜索消息
1. 在顶部搜索框输入关键词
2. 点击搜索按钮或按 Enter
3. 查看搜索结果

## 功能说明

### 主界面布局

```
┌─────────────────────────────────────────────────────────┐
│  Header: NextIM | Status | User Info | Actions          │
├──────────┬──────────────────────────────┬───────────────┤
│          │  Chat Header                 │               │
│  Sidebar │  ┌────────────────────────┐  │  Settings     │
│          │  │                        │  │  (Collapsible)│
│  - Rooms │  │  Messages Area         │  │               │
│  - Contacts  │                        │  │  - Prefs      │
│          │  │                        │  │  - Theme      │
│  [New]   │  └────────────────────────┘  │  - Data       │
│          │  Input Area                  │               │
└──────────┴──────────────────────────────┴───────────────┘
```

### 快捷操作

- **Enter**: 发送消息
- **Shift + Enter**: 换行
- **点击房间**: 切换聊天
- **点击 🌓**: 切换主题
- **点击 ☰**: 折叠侧边栏
- **点击 ⚙**: 打开设置

### 主题切换

支持亮色和暗色两种主题：
- 点击顶部 🌓 按钮切换
- 设置会自动保存到浏览器

### 通知设置

在右侧设置面板中可以配置：
- 桌面通知（需要浏览器权限）
- 消息提示音
- Enter 键发送
- 时间戳显示
- 紧凑模式

## 多用户测试

### 在同一台机器上测试多用户

1. **启动 Store 服务**（只需一个）
   ```bash
   cargo run --bin nextim-store
   ```

2. **打开多个浏览器窗口**
   - 窗口 1: 用户名 `alice`
   - 窗口 2: 用户名 `bob`
   - 窗口 3: 用户名 `charlie`

3. **创建共同房间**
   - 在 alice 窗口创建房间 "Team Chat"
   - 记下房间 ID（在 URL 或控制台）

4. **其他用户加入**
   - 在 bob 和 charlie 窗口点击 "Join Room"
   - 输入房间 ID

5. **开始聊天**
   - 所有用户都能实时看到消息
   - WebSocket 自动推送更新

### 跨机器测试

1. **确保 Store 服务可访问**
   ```bash
   # 绑定到所有接口
   cargo run --bin nextim-store -- --bind 0.0.0.0:9100
   ```

2. **在其他机器上访问**
   - Store API URL: `http://<服务器IP>:9100`
   - 其他步骤相同

## 故障排查

### 连接失败

**问题**: 点击 Connect 后显示 "Failed"

**解决**:
1. 检查 Store 服务是否运行
   ```bash
   curl http://127.0.0.1:9100/health
   # 应返回: ok
   ```

2. 检查浏览器控制台错误
   - 按 F12 打开开发者工具
   - 查看 Console 和 Network 标签

3. 检查 CORS 设置
   - Store 服务已配置允许所有来源
   - 如果使用文件协议，某些浏览器可能限制

### WebSocket 连接失败

**问题**: API OK 但 WS Off

**解决**:
1. 检查 WebSocket 端口（默认与 API 相同）
2. 查看浏览器控制台 WebSocket 错误
3. 确认防火墙未阻止连接

### 消息不显示

**问题**: 发送消息后看不到

**解决**:
1. 检查是否选择了房间
2. 刷新页面重新加载
3. 查看浏览器控制台错误
4. 检查 Store 日志

### 搜索无结果

**问题**: 搜索消息返回空

**解决**:
1. 确认消息已发送成功
2. 等待几秒让索引更新
3. 尝试不同的搜索词
4. 检查 Store 搜索索引是否正常

## 开发调试

### 查看网络请求

1. 打开浏览器开发者工具（F12）
2. 切换到 Network 标签
3. 筛选 WS（WebSocket）和 Fetch/XHR
4. 查看请求和响应

### 查看本地存储

1. 开发者工具 → Application 标签
2. 左侧 Storage → Local Storage
3. 查看保存的设置：
   - `nextim-api-url`
   - `nextim-username`
   - `nextim-theme`
   - `nextim-settings`

### 清除缓存

在设置面板中点击 "Clear Cache" 或：
```javascript
// 在浏览器控制台执行
localStorage.clear();
location.reload();
```

### 导出数据

1. 打开右侧设置面板
2. 点击 "Export Data"
3. 保存 JSON 文件
4. 包含所有消息、联系人、设置

## 性能优化

### 大量消息

- 消息自动分页加载
- 虚拟滚动（计划中）
- 定期清理旧消息缓存

### 多个房间

- 只加载当前房间消息
- 后台轮询间隔 5 秒
- WebSocket 实时推送

### 网络优化

- WebSocket 自动重连
- 消息去重
- 离线消息队列（计划中）

## 浏览器兼容性

### 完全支持
- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

### 部分支持
- 旧版浏览器可能缺少某些功能
- 建议使用最新版本

### 必需特性
- WebSocket
- LocalStorage
- Fetch API
- ES6+ (Promise, async/await, class)
- CSS Grid & Flexbox

## 下一步

- 查看 [README.md](README.md) 了解完整功能
- 查看 [API 文档](../../docs/api.md) 了解后端接口
- 修改 CSS 自定义样式
- 扩展 JS 模块添加功能

## 获取帮助

- 查看浏览器控制台错误
- 查看 Store 服务日志
- 检查 GitHub Issues
- 阅读源代码注释
