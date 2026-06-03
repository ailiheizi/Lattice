# NextIM Web Client - 功能详解

## 1. 用户界面组件

### 1.1 顶部导航栏
```
[NextIM] [API Status] [WS Status] ................ [User] [☰] [👤] [⚙️]
```

**组件说明：**
- **应用标题** - NextIM 品牌标识
- **API Status** - REST API 连接状态指示器
  - 🟢 API OK - 已连接
  - 🔴 Failed - 连接失败
- **WS Status** - WebSocket 连接状态
  - 🔵 WS Live - 实时连接
  - 🔴 WS Off - 断开连接
- **用户信息** - 显示当前用户名
- **侧边栏切换** - 移动端显示/隐藏侧边栏
- **用户面板** - 打开个人信息设置
- **设置按钮** - 快速访问设置

### 1.2 侧边栏（280px 宽）

#### 配置面板
- Store API 地址输入框
- 用户名输入框
- Connect 按钮
- Store 信息显示（连接后）

#### 标签页切换
- **Rooms** - 房间列表
- **Contacts** - 联系人列表
- **Settings** - 应用设置

#### Rooms 标签页
```
┌─────────────────────────────┐
│ 🏠 General Chat        [3]  │  ← 未读消息数
│ Last message preview...     │
├─────────────────────────────┤
│ 🔒 Private Team             │  ← 加密房间
│ No messages                 │
└─────────────────────────────┘
[Room name input]
[Create Room]
```

**功能：**
- 显示所有房间
- 未读消息徽章
- 加密状态图标（🔒）
- 最后一条消息预览
- 创建新房间

#### Contacts 标签页
```
┌─────────────────────────────┐
│ 🟢 Alice [verified]         │  ← 在线状态
│ ws://store.example.com      │
├─────────────────────────────┤
│ ⚫ Bob [tofu]                │  ← 离线
│ ws://bob.store.net          │
└─────────────────────────────┘
[Fingerprint input]
[Display name input]
[Store address input]
[Add Contact]
```

**功能：**
- 在线状态指示器
- 信任级别标签
  - verified - 已验证
  - tofu - 首次使用信任
  - public - 公开
- Store 地址显示
- 添加新联系人

#### Settings 标签页
```
Appearance
  ☑ Dark Theme

Notifications
  ☑ Desktop Notifications
  ☑ Sound Alerts

Privacy
  ☑ Read Receipts
```

**功能：**
- 主题切换（暗色/亮色）
- 通知设置
- 隐私选项

### 1.3 主聊天区域

#### 聊天头部
```
[Room Name]                    [Search] [🔍] [ℹ️]
2 members · 🔒 Encrypted
```

**功能：**
- 房间名称显示
- 成员数量
- 加密状态
- 消息搜索
- 房间信息按钮

#### 消息列表
```
┌─────────────────────────────────────┐
│         System Message              │  ← 系统消息
│                                     │
│  ┌─────────────────────┐           │
│  │ Alice                │           │  ← 接收消息
│  │ Hello everyone!      │           │
│  │ 10:30 AM        ✓   │           │
│  └─────────────────────┘           │
│                                     │
│           ┌─────────────────────┐  │
│           │ Hi Alice!            │  │  ← 发送消息
│           │ 10:31 AM        ✓   │  │
│           └─────────────────────┘  │
└─────────────────────────────────────┘
```

**功能：**
- 消息气泡样式
- 发送/接收消息区分
- 发送者名称（接收消息）
- 时间戳
- 验证标记（✓）
- 自动滚动到底部
- 平滑动画效果

#### 输入区域
```
[😊] [Type a message.....................] [📎] [Send]
```

**功能：**
- 表情符号选择器
- 消息输入框
- 附件按钮（待实现）
- 发送按钮
- Enter 键发送

## 2. 模态框

### 2.1 用户信息面板
```
┌─────────────────────────────────┐
│ User Profile              [×]   │
├─────────────────────────────────┤
│ Display Name:                   │
│ [alice                    ]     │
│                                 │
│ Fingerprint:                    │
│ [abc123...xyz (readonly)  ]     │
│                                 │
│ Store Address:                  │
│ [ws://127.0.0.1:9100 (ro) ]     │
│                                 │
│ Status Message:                 │
│ [What's on your mind?     ]     │
│ [                         ]     │
│                                 │
│        [Cancel]  [Save]         │
└─────────────────────────────────┘
```

### 2.2 房间信息面板
```
┌─────────────────────────────────┐
│ Room Information          [×]   │
├─────────────────────────────────┤
│ Room Name:                      │
│ [General Chat             ]     │
│                                 │
│ Room Type:                      │
│ [Group (readonly)         ]     │
│                                 │
│ Members (3):                    │
│ ┌─────────────────────────┐    │
│ │ alice                   │    │
│ │ bob                     │    │
│ │ charlie                 │    │
│ └─────────────────────────┘    │
│                                 │
│ Add Member:                     │
│ [fingerprint...    ] [Add]      │
│                                 │
│    [Leave Room]  [Close]        │
└─────────────────────────────────┘
```

## 3. 通知系统

### 3.1 应用内通知
```
┌─────────────────────────────────┐
│ ✓ Success                  [×] │
│ Room created successfully       │
└─────────────────────────────────┘
```

**类型：**
- success - 成功操作（绿色边框）
- error - 错误提示（红色边框）
- info - 信息提示（蓝色边框）

**特性：**
- 右上角显示
- 5秒自动消失
- 可手动关闭
- 滑入动画

### 3.2 桌面通知
```
┌─────────────────────────────┐
│ NextIM                      │
│ New Message                 │
│ You have a new message      │
└─────────────────────────────┘
```

**要求：**
- 浏览器通知权限
- 设置中启用
- 标签页不在前台时显示

### 3.3 声音提醒
- 使用 Web Audio API
- 800Hz 正弦波
- 0.5秒持续时间
- 可在设置中关闭

## 4. 表情符号选择器

```
┌─────────────────────────────────┐
│ 😊 😂 ❤️ 👍 👎 🎉 🔥 ✨        │
│ 💯 🚀 👀 🤔 😎 🙏 💪 🎯        │
└─────────────────────────────────┘
```

**功能：**
- 16个常用表情
- 点击插入到输入框
- 点击外部自动关闭
- 网格布局

## 5. 主题系统

### 5.1 暗色主题（默认）
```css
--bg-primary: #0d1117
--bg-secondary: #161b22
--bg-tertiary: #21262d
--text-primary: #c9d1d9
--accent-blue: #58a6ff
--accent-green: #238636
```

### 5.2 亮色主题
```css
--bg-primary: #ffffff
--bg-secondary: #f6f8fa
--bg-tertiary: #eaeef2
--text-primary: #24292f
--accent-blue: #0969da
--accent-green: #1a7f37
```

**切换方式：**
- Settings 标签页中的 Dark Theme 开关
- 使用 CSS Variables 实现
- 设置保存到 LocalStorage
- 页面刷新后保持

## 6. 响应式设计

### 6.1 桌面端（> 768px）
- 侧边栏固定显示（280px）
- 聊天区域占据剩余空间
- 消息最大宽度 65%

### 6.2 移动端（≤ 768px）
- 侧边栏全屏显示
- 可通过 ☰ 按钮切换
- 侧边栏使用 transform 动画
- 消息最大宽度 85%
- 搜索框宽度自适应

## 7. 数据流

### 7.1 连接流程
```
用户输入 API 地址和用户名
    ↓
检查 /health 端点
    ↓
获取 /identity 信息
    ↓
建立 WebSocket 连接
    ↓
加载房间列表 /rooms
    ↓
开始轮询消息（5秒间隔）
```

### 7.2 消息发送流程
```
用户输入消息
    ↓
POST /messages
    ↓
本地添加到消息列表
    ↓
渲染消息
    ↓
更新房间预览
```

### 7.3 消息接收流程
```
WebSocket 收到通知
    ↓
触发消息加载
    ↓
GET /messages/:room_id?since=timestamp
    ↓
合并新消息
    ↓
更新未读计数
    ↓
显示通知
    ↓
播放声音
```

## 8. 本地存储

### 8.1 LocalStorage 数据
```json
{
  "nextim-settings": {
    "theme": "dark",
    "notifications": true,
    "sound": true,
    "readReceipts": true
  }
}
```

### 8.2 内存数据结构
```javascript
rooms = Map {
  "room-id-1": {
    name: "General Chat",
    messages: [...],
    lastPoll: 1234567890,
    memberCount: 3,
    encrypted: false,
    unreadCount: 2
  }
}

contacts = [
  {
    fingerprint: "abc123...",
    display_name: "Alice",
    store_address: "ws://...",
    trust_level: "verified"
  }
]
```

## 9. 性能优化

### 9.1 消息渲染
- 使用 innerHTML 批量渲染
- 避免频繁 DOM 操作
- 自动滚动使用 scrollTop

### 9.2 网络请求
- WebSocket 实时推送
- 轮询作为备用（5秒间隔）
- 增量加载消息（since 参数）

### 9.3 动画
- CSS transitions（0.2-0.3秒）
- transform 动画（GPU 加速）
- 消息淡入动画

## 10. 错误处理

### 10.1 连接错误
- API 连接失败 → 显示错误通知
- WebSocket 断开 → 自动重连（3秒后）
- 网络请求失败 → 静默失败或通知

### 10.2 用户输入验证
- 空消息不发送
- 必填字段检查
- 确认对话框（离开房间）

## 11. 安全考虑

### 11.1 XSS 防护
- 使用 escapeHtml() 转义所有用户输入
- innerHTML 仅用于已转义内容

### 11.2 CORS
- 后端配置 CORS 允许所有来源
- 适用于开发环境

### 11.3 数据传输
- WebSocket 使用 ws:// 协议
- 生产环境应使用 wss://
- REST API 使用 http://
- 生产环境应使用 https://

## 12. 浏览器 API 使用

- **Fetch API** - HTTP 请求
- **WebSocket API** - 实时通信
- **LocalStorage API** - 设置持久化
- **Notification API** - 桌面通知
- **Web Audio API** - 声音提醒
- **DOM API** - 界面操作

## 13. 未来增强

### 13.1 短期
- [ ] 消息编辑/删除
- [ ] 消息回复（引用）
- [ ] @提及功能
- [ ] Markdown 渲染
- [ ] 代码高亮

### 13.2 中期
- [ ] 文件上传/下载
- [ ] 图片预览
- [ ] 视频/音频播放
- [ ] 拖拽上传
- [ ] 剪贴板粘贴

### 13.3 长期
- [ ] 端到端加密支持
- [ ] 离线消息缓存
- [ ] PWA 支持
- [ ] Service Worker
- [ ] 推送通知
- [ ] 语音/视频通话
