# Lattice Store 管理面板

现代化的 Lattice Store 管理界面，提供实时数据监控和管理功能。

## 功能特性

- **仪表板** - 实时统计和数据可视化
- **消息管理** - 查看、搜索和删除消息
- **联系人管理** - 管理所有联系人
- **房间管理** - 查看和管理聊天房间
- **系统日志** - 实时日志查看

## 技术栈

- 纯 JavaScript (ES6+)
- Chart.js - 数据可视化
- REST API 集成
- 响应式设计

## 快速开始

1. 确保 Lattice Store 服务运行在 `http://localhost:9100`

2. 使用任意 HTTP 服务器启动管理面板：

```bash
# 使用 Python
python -m http.server 8080

# 使用 Node.js
npx http-server -p 8080

# 使用 PHP
php -S localhost:8080
```

3. 访问 `http://localhost:8080`

## API 配置

默认 API 地址：`http://localhost:9100`

修改配置请编辑 `js/config.js`：

```javascript
const CONFIG = {
    API_BASE_URL: 'http://localhost:9100',
    REFRESH_INTERVAL: 5000, // 5秒自动刷新
    // ...
};
```

## 文件结构

```
store-admin/
├── index.html          # 主页面
├── css/
│   └── style.css       # 完整样式表
├── js/
│   ├── config.js       # 配置文件
│   ├── api.js          # API 客户端
│   ├── app.js          # 主应用逻辑
│   ├── dashboard.js    # 仪表板模块
│   ├── messages.js     # 消息管理模块
│   ├── contacts.js     # 联系人管理模块
│   ├── rooms.js        # 房间管理模块
│   ├── logs.js         # 日志查看模块
│   └── charts.js       # 图表管理
└── README.md
```

## API 端点

管理面板使用以下 Store REST API 端点：

- `GET /stats` - 获取统计信息
- `GET /messages` - 获取消息列表
- `GET /messages/:id` - 获取特定消息
- `DELETE /messages/:id` - 删除消息
- `GET /contacts` - 获取联系人列表
- `DELETE /contacts/:id` - 删除联系人
- `GET /rooms` - 获取房间列表
- `DELETE /rooms/:id` - 删除房间
- `GET /logs` - 获取日志（可选）

## 功能说明

### 仪表板
- 实时统计卡片（消息、联系人、房间、存储）
- 消息趋势图表
- 存储使用分布
- 活动统计

### 消息管理
- 消息列表展示
- 实时搜索过滤
- 按联系人筛选
- 删除消息功能
- 自动刷新（5秒）

### 联系人管理
- 联系人卡片展示
- 在线状态显示
- 查看联系人消息
- 删除联系人
- 搜索功能

### 房间管理
- 房间信息展示
- 成员列表
- 消息统计
- 删除房间
- 搜索功能

### 系统日志
- 实时日志流
- 日志级别过滤
- 自动滚动
- 清空日志

## 浏览器支持

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

## 开发说明

所有模块采用类封装，支持：
- 独立初始化和销毁
- 自动刷新机制
- 错误处理
- 加载状态管理

## 许可证

与 Lattice 项目相同
