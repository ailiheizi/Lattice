# Lattice Web Chat - 开发指南

## 开发环境设置

### 前置要求

- **Rust**: 1.70+ (用于运行 Store 服务)
- **浏览器**: Chrome 90+, Firefox 88+, Safari 14+
- **编辑器**: VS Code, WebStorm, 或任何文本编辑器
- **可选**: Node.js (用于 HTTP 服务器)

### 快速开始

```bash
# 1. 克隆项目
git clone https://github.com/yourusername/Lattice.git
cd Lattice

# 2. 启动 Store 服务
cargo run --bin lattice-store

# 3. 在新终端启动 Web 服务器
cd web/chat
python -m http.server 8080

# 4. 打开浏览器
# http://localhost:8080
```

## 项目结构详解

```
web/chat/
├── index.html              # 主应用入口
├── demo.html               # 演示页面
├── test.html               # 测试套件
│
├── css/
│   ├── style.css           # 核心样式
│   └── extensions.css      # 扩展样式（未来功能）
│
├── js/
│   ├── app.js              # 应用主类
│   ├── chat.js             # 聊天模块
│   ├── rooms.js            # 房间模块
│   ├── contacts.js         # 联系人模块
│   ├── settings.js         # 设置模块
│   ├── shortcuts.js        # 快捷键模块
│   └── utils.js            # 工具函数
│
└── docs/
    ├── README.md           # 项目文档
    ├── QUICKSTART.md       # 快速开始
    ├── DEPLOYMENT.md       # 部署指南
    ├── FILE_MANIFEST.md    # 文件清单
    └── PROJECT_SUMMARY.md  # 项目总结
```

## 代码架构

### 模块化设计

```
LatticeApp (app.js)
├── ChatModule (chat.js)
│   ├── sendMessage()
│   ├── loadMessages()
│   ├── renderMessages()
│   └── searchMessages()
│
├── RoomsModule (rooms.js)
│   ├── loadRooms()
│   ├── createRoom()
│   ├── selectRoom()
│   └── renderRooms()
│
├── ContactsModule (contacts.js)
│   ├── loadContacts()
│   ├── addContact()
│   └── renderContacts()
│
├── SettingsModule (settings.js)
│   ├── loadSettings()
│   ├── saveSettings()
│   └── exportData()
│
└── KeyboardShortcuts (shortcuts.js)
    ├── register()
    └── showHelp()
```

### 数据流

```
User Action
    ↓
Event Handler
    ↓
Module Method
    ↓
API Call (fetch/WebSocket)
    ↓
Store Service
    ↓
Response
    ↓
Update State
    ↓
Render UI
```

## 开发工作流

### 1. 添加新功能

#### 示例：添加消息编辑功能

**步骤 1**: 在 `chat.js` 中添加方法

```javascript
// chat.js
async editMessage(msgId, newText) {
  try {
    const resp = await fetch(`${this.app.apiUrl}/messages/id/${msgId}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text: newText })
    });

    if (!resp.ok) throw new Error('Edit failed');

    // 更新本地缓存
    const messages = this.messages.get(this.currentRoom);
    const msg = messages.find(m => m.msg_id === msgId);
    if (msg) {
      msg.text = newText;
      msg.edited = true;
      this.renderMessages();
    }

    this.app.showNotification('Message edited', 'success');
  } catch (e) {
    this.app.showNotification('Failed to edit message', 'error');
  }
}
```

**步骤 2**: 添加 UI 元素

```javascript
// 在 renderMessage() 中添加编辑按钮
renderMessage(msg) {
  // ... 现有代码 ...

  if (msg.sent) {
    html += `<button class="msg-action-btn" onclick="app.chat.editMessage('${msg.msg_id}')">Edit</button>`;
  }

  return html;
}
```

**步骤 3**: 添加样式

```css
/* style.css */
.msg-action-btn {
  background: rgba(0,0,0,0.3);
  border: none;
  color: #fff;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
}
```

**步骤 4**: 测试功能

```javascript
// 在浏览器控制台测试
app.chat.editMessage('msg-id-123', 'New text');
```

### 2. 修改样式

#### 示例：更改主题颜色

```css
/* style.css */
:root {
  /* 修改主色调 */
  --accent-blue: #4a90e2;  /* 原来是 #58a6ff */
  --accent-green: #27ae60; /* 原来是 #238636 */
}
```

### 3. 调试技巧

#### 使用浏览器开发者工具

```javascript
// 在代码中添加断点
debugger;

// 打印调试信息
console.log('Current room:', this.currentRoom);
console.table(this.messages.get(this.currentRoom));

// 性能分析
console.time('loadMessages');
await this.loadMessages();
console.timeEnd('loadMessages');
```

#### 查看网络请求

1. 打开开发者工具 (F12)
2. 切换到 Network 标签
3. 筛选 XHR/Fetch 查看 API 请求
4. 筛选 WS 查看 WebSocket 连接

#### 查看本地存储

```javascript
// 查看所有存储的数据
console.log('API URL:', localStorage.getItem('lattice-api-url'));
console.log('Username:', localStorage.getItem('lattice-username'));
console.log('Theme:', localStorage.getItem('lattice-theme'));
console.log('Settings:', JSON.parse(localStorage.getItem('lattice-settings')));

// 清除所有数据
localStorage.clear();
```

## 代码规范

### JavaScript 规范

```javascript
// 1. 使用 ES6+ 语法
class MyModule {
  constructor(app) {
    this.app = app;
  }

  async myMethod() {
    // 使用 async/await
    const data = await fetch(url).then(r => r.json());
    return data;
  }
}

// 2. 使用箭头函数
const handler = (e) => {
  console.log(e);
};

// 3. 使用模板字符串
const message = `Hello, ${username}!`;

// 4. 使用解构赋值
const { apiUrl, username } = this.app;

// 5. 使用 const/let，避免 var
const API_URL = 'http://localhost:9100';
let counter = 0;

// 6. 错误处理
try {
  await riskyOperation();
} catch (e) {
  console.error('Operation failed:', e);
  this.app.showNotification('Error occurred', 'error');
}
```

### CSS 规范

```css
/* 1. 使用 CSS 变量 */
:root {
  --primary-color: #667eea;
}

.button {
  background: var(--primary-color);
}

/* 2. 使用 BEM 命名 */
.message { }
.message__text { }
.message__meta { }
.message--sent { }
.message--received { }

/* 3. 移动优先 */
.container {
  width: 100%;
}

@media (min-width: 768px) {
  .container {
    width: 750px;
  }
}

/* 4. 使用 Flexbox/Grid */
.layout {
  display: flex;
  gap: 16px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
}
```

### HTML 规范

```html
<!-- 1. 语义化标签 -->
<header>
  <nav>
    <ul>
      <li><a href="#">Home</a></li>
    </ul>
  </nav>
</header>

<main>
  <article>
    <h1>Title</h1>
    <p>Content</p>
  </article>
</main>

<footer>
  <p>&copy; 2026 Lattice</p>
</footer>

<!-- 2. 可访问性 -->
<button aria-label="Close" onclick="close()">×</button>
<input type="text" id="username" aria-required="true">

<!-- 3. 数据属性 -->
<div class="room-item" data-room-id="123" data-room-name="General">
  Room Name
</div>
```

## 测试

### 单元测试（计划中）

```javascript
// tests/chat.test.js
describe('ChatModule', () => {
  let app, chat;

  beforeEach(() => {
    app = new LatticeApp();
    chat = new ChatModule(app);
  });

  test('should send message', async () => {
    const result = await chat.sendMessage();
    expect(result).toBeTruthy();
  });

  test('should load messages', async () => {
    await chat.loadMessages();
    expect(chat.messages.size).toBeGreaterThan(0);
  });
});
```

### 集成测试

使用 `test.html` 进行集成测试：

```bash
# 1. 启动 Store 服务
cargo run --bin lattice-store

# 2. 打开测试页面
open http://localhost:8080/test.html

# 3. 点击 "Run All Tests"
# 4. 查看测试结果
```

### 手动测试清单

- [ ] 连接到 Store
- [ ] 创建房间
- [ ] 发送消息
- [ ] 接收消息
- [ ] 搜索消息
- [ ] 添加联系人
- [ ] 切换主题
- [ ] 测试快捷键
- [ ] 测试响应式布局
- [ ] 测试 WebSocket 重连

## 性能优化

### 1. 减少重绘

```javascript
// 不好：多次操作 DOM
for (const msg of messages) {
  container.appendChild(createMessageElement(msg));
}

// 好：批量更新
const fragment = document.createDocumentFragment();
for (const msg of messages) {
  fragment.appendChild(createMessageElement(msg));
}
container.appendChild(fragment);

// 更好：使用 innerHTML
container.innerHTML = messages.map(renderMessage).join('');
```

### 2. 防抖和节流

```javascript
// 搜索输入防抖
const debouncedSearch = Utils.debounce((query) => {
  this.searchMessages(query);
}, 300);

searchInput.addEventListener('input', (e) => {
  debouncedSearch(e.target.value);
});

// 滚动事件节流
const throttledScroll = Utils.throttle(() => {
  this.checkScrollPosition();
}, 100);

container.addEventListener('scroll', throttledScroll);
```

### 3. 懒加载

```javascript
// 消息分页加载
async loadMoreMessages() {
  const messages = this.messages.get(this.currentRoom) || [];
  const oldestTimestamp = messages[0]?.timestamp || Date.now();

  const resp = await fetch(
    `${this.app.apiUrl}/messages/${this.currentRoom}?before=${oldestTimestamp}&limit=50`
  );
  const newMessages = await resp.json();

  messages.unshift(...newMessages);
  this.renderMessages();
}
```

### 4. 缓存策略

```javascript
// 缓存 API 响应
class CacheManager {
  constructor() {
    this.cache = new Map();
    this.ttl = 60000; // 1 分钟
  }

  async get(key, fetcher) {
    const cached = this.cache.get(key);
    if (cached && Date.now() - cached.timestamp < this.ttl) {
      return cached.data;
    }

    const data = await fetcher();
    this.cache.set(key, { data, timestamp: Date.now() });
    return data;
  }
}
```

## 常见问题

### Q1: WebSocket 连接失败

**原因**:
- Store 服务未启动
- 端口被占用
- 防火墙阻止

**解决**:
```bash
# 检查 Store 服务
ps aux | grep lattice-store

# 检查端口
netstat -tlnp | grep 9100

# 重启服务
cargo run --bin lattice-store
```

### Q2: 消息不显示

**原因**:
- 未选择房间
- API 请求失败
- 渲染错误

**解决**:
```javascript
// 检查当前房间
console.log('Current room:', app.chat.currentRoom);

// 检查消息缓存
console.log('Messages:', app.chat.messages.get(app.chat.currentRoom));

// 手动触发渲染
app.chat.renderMessages();
```

### Q3: 样式不生效

**原因**:
- CSS 文件未加载
- 选择器优先级问题
- 浏览器缓存

**解决**:
```bash
# 清除浏览器缓存
Ctrl+Shift+R (Windows/Linux)
Cmd+Shift+R (Mac)

# 检查 CSS 加载
# 开发者工具 -> Network -> CSS
```

### Q4: 本地存储丢失

**原因**:
- 浏览器隐私模式
- 存储空间已满
- 浏览器清理

**解决**:
```javascript
// 检查存储可用性
if (typeof(Storage) !== "undefined") {
  console.log("LocalStorage available");
} else {
  console.log("LocalStorage not available");
}

// 导出数据备份
app.settings.exportData();
```

## 贡献指南

### 提交代码

```bash
# 1. 创建功能分支
git checkout -b feature/my-feature

# 2. 提交更改
git add .
git commit -m "feat: add message editing feature"

# 3. 推送到远程
git push origin feature/my-feature

# 4. 创建 Pull Request
```

### Commit 消息规范

```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型**:
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式（不影响功能）
- `refactor`: 重构
- `perf`: 性能优化
- `test`: 测试
- `chore`: 构建/工具

**示例**:
```
feat(chat): add message editing feature

- Add editMessage() method
- Add edit button to sent messages
- Update UI after editing

Closes #123
```

### 代码审查清单

- [ ] 代码符合规范
- [ ] 添加了必要的注释
- [ ] 更新了文档
- [ ] 通过了所有测试
- [ ] 没有引入新的警告
- [ ] 性能没有明显下降
- [ ] 兼容目标浏览器

## 工具推荐

### 编辑器

**VS Code 扩展**:
- ESLint
- Prettier
- Live Server
- GitLens
- Path Intellisense

**WebStorm**:
- 内置 JavaScript 支持
- 强大的重构功能
- 集成调试器

### 浏览器扩展

- **React DevTools**: 虽然不用 React，但可以学习调试技巧
- **Redux DevTools**: 状态管理调试
- **Lighthouse**: 性能分析
- **WAVE**: 可访问性检查

### 命令行工具

```bash
# HTTP 服务器
npm install -g http-server

# 代码格式化
npm install -g prettier

# 代码检查
npm install -g eslint

# 性能测试
npm install -g lighthouse
```

## 学习资源

### 官方文档

- [MDN Web Docs](https://developer.mozilla.org/)
- [WebSocket API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
- [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API)

### 推荐阅读

- **JavaScript**:
  - "You Don't Know JS" 系列
  - "Eloquent JavaScript"

- **CSS**:
  - "CSS Secrets"
  - "Every Layout"

- **性能**:
  - "High Performance Browser Networking"
  - "Web Performance in Action"

### 在线课程

- [JavaScript.info](https://javascript.info/)
- [CSS-Tricks](https://css-tricks.com/)
- [Web.dev](https://web.dev/)

## 下一步

1. **熟悉代码库**: 阅读现有代码，理解架构
2. **运行测试**: 使用 test.html 验证功能
3. **尝试修改**: 从小改动开始，如修改颜色
4. **添加功能**: 实现一个简单的新功能
5. **优化性能**: 使用开发者工具分析性能
6. **编写文档**: 记录你的更改和决策

## 获取帮助

- **GitHub Issues**: 报告 bug 或请求功能
- **讨论区**: 提问和讨论
- **代码审查**: 请求其他开发者审查你的代码
- **文档**: 查阅项目文档

---

**Happy Coding!** 🚀
