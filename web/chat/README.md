# Lattice Web Chat

现代化的 Web 聊天界面，用于 Lattice 去中心化即时通讯系统。

## 功能特性

- **实时消息** - WebSocket 连接实现实时消息推送
- **房间管理** - 创建、加入、切换聊天房间
- **联系人管理** - 添加、查看联系人列表
- **消息搜索** - 全文搜索历史消息
- **主题切换** - 亮色/暗色主题
- **响应式设计** - 支持桌面和移动设备
- **离线缓存** - 本地缓存消息和设置
- **通知支持** - 浏览器桌面通知

## 文件结构

```
web/chat/
├── index.html          # 主页面
├── css/
│   └── style.css       # 样式表
└── js/
    ├── app.js          # 主应用逻辑
    ├── chat.js         # 聊天功能模块
    ├── rooms.js        # 房间管理模块
    ├── contacts.js     # 联系人管理模块
    └── settings.js     # 设置模块
```

## 使用方法

### 1. 启动 Store 服务

```bash
# 在项目根目录
cargo run --bin lattice-store
```

Store 服务默认运行在 `http://localhost:9100`

### 2. 打开 Web 界面

在浏览器中打开：
```
file:///D:/windows/code/project/Lattice/web/chat/index.html
```

或使用本地 HTTP 服务器：
```bash
# 使用 Python
cd web/chat
python -m http.server 8080

# 然后访问 http://localhost:8080
```

### 3. 连接到 Store

1. 在左侧面板输入 Store API 地址（默认 `http://127.0.0.1:9100`）
2. 输入用户名
3. 点击 "Connect" 按钮

### 4. 开始聊天

- **创建房间**：在 Rooms 标签下输入房间名称，点击 "Create Room"
- **加入房间**：点击 "Join Room" 输入房间 ID
- **发送消息**：选择房间后在底部输入框输入消息，按 Enter 发送
- **搜索消息**：使用顶部搜索框搜索历史消息

## 界面布局

### 左侧边栏
- **连接设置** - Store API 地址和用户名配置
- **Rooms 标签** - 显示所有房间列表
- **Contacts 标签** - 显示联系人列表
- **新建表单** - 创建房间或添加联系人

### 中间区域
- **聊天头部** - 显示当前房间信息和搜索框
- **消息列表** - 显示聊天消息
- **输入框** - 发送新消息

### 右侧边栏（可折叠）
- **偏好设置** - 通知、声音、主题等
- **数据管理** - 导出数据、清除缓存
- **关于信息** - 版本信息

## 快捷键

- `Enter` - 发送消息
- `Shift + Enter` - 换行
- `Ctrl/Cmd + K` - 聚焦搜索框（计划中）

## 主题

支持亮色和暗色两种主题，点击顶部 🌓 按钮切换。主题设置会自动保存到本地存储。

## 浏览器兼容性

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

需要支持：
- WebSocket
- LocalStorage
- Fetch API
- ES6+

## 配置

所有设置保存在浏览器 LocalStorage 中：

- `lattice-api-url` - Store API 地址
- `lattice-username` - 用户名
- `lattice-theme` - 主题设置
- `lattice-settings` - 其他偏好设置

## API 端点

Web 界面使用以下 Store API 端点：

- `GET /health` - 健康检查
- `GET /identity` - 获取 Store 身份信息
- `GET /rooms` - 获取房间列表
- `POST /rooms` - 创建新房间
- `GET /messages/{room_id}` - 获取房间消息
- `POST /messages` - 发送消息
- `GET /search` - 搜索消息
- `GET /contacts` - 获取联系人列表
- `POST /contacts` - 添加联系人
- `WS /` - WebSocket 连接

## 开发

### 修改样式

编辑 `css/style.css` 文件，使用 CSS 变量定义的颜色主题。

### 添加功能

各模块职责：
- `app.js` - 应用初始化、连接管理、通用工具
- `chat.js` - 消息发送、接收、渲染
- `rooms.js` - 房间列表、创建、加入
- `contacts.js` - 联系人列表、添加、删除
- `settings.js` - 用户设置、数据导出

### 调试

打开浏览器开发者工具（F12）查看控制台日志和网络请求。

## 已知问题

- WebSocket 消息为 Protobuf 二进制格式，浏览器端暂时只触发刷新
- 文件上传功能尚未实现
- 群组成员管理功能待完善
- 消息加密状态显示需要改进

## 未来计划

- [ ] 支持图片、文件发送
- [ ] 消息已读状态
- [ ] 输入状态提示
- [ ] 表情符号选择器
- [ ] 消息引用回复
- [ ] 用户在线状态
- [ ] 语音/视频通话
- [ ] PWA 支持
- [ ] 多语言支持

## 许可证

与 Lattice 项目相同
