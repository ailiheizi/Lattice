# Lattice Store 管理面板

[English](README.md) | 简体中文

现代化的 Lattice Store 管理界面，提供实时数据监控和管理功能。

## 特性

- 🎯 **实时监控** - 5秒自动刷新，实时掌握系统状态
- 📊 **数据可视化** - Chart.js 驱动的精美图表
- 🔍 **智能搜索** - 快速查找消息、联系人、房间
- 📱 **响应式设计** - 完美支持桌面、平板、手机
- 🎨 **现代 UI** - 简洁美观的用户界面
- ⚡ **轻量高效** - 纯原生 JavaScript，无框架依赖

## 快速开始

### 1. 启动 Store 服务

```bash
cd Lattice
cargo run --bin lattice-store
```

### 2. 启动管理面板

```bash
cd web/store-admin
python -m http.server 8080
```

### 3. 访问管理面板

打开浏览器访问 http://localhost:8080

## 功能模块

### 📊 仪表板
- 实时统计卡片（消息、联系人、房间、存储）
- 消息趋势折线图
- 存储使用环形图
- 活动统计柱状图

### 💬 消息管理
- 消息列表展示
- 实时搜索过滤
- 按联系人筛选
- 删除消息

### 👥 联系人管理
- 联系人卡片展示
- 在线状态显示（在线/离开/离线）
- 查看联系人消息
- 删除联系人

### 🏠 房间管理
- 房间信息展示
- 成员列表
- 消息统计
- 删除房间

### 📝 系统日志
- 实时日志流
- 日志级别过滤
- 自动滚动
- 清空日志

## 技术栈

- **前端**: 纯 JavaScript (ES6+)
- **图表**: Chart.js 4.4.0
- **样式**: CSS3 (Grid, Flexbox, Variables)
- **API**: REST API

## 浏览器支持

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

## 项目结构

```
store-admin/
├── index.html          # 主页面
├── test.html           # API 测试工具
├── css/
│   └── style.css       # 样式表
├── js/
│   ├── config.js       # 配置
│   ├── api.js          # API 客户端
│   ├── app.js          # 主应用
│   ├── dashboard.js    # 仪表板
│   ├── messages.js     # 消息管理
│   ├── contacts.js     # 联系人管理
│   ├── rooms.js        # 房间管理
│   ├── logs.js         # 日志查看
│   └── charts.js       # 图表管理
└── docs/               # 文档
```

## 配置

编辑 `js/config.js` 修改配置：

```javascript
const CONFIG = {
    API_BASE_URL: 'http://localhost:9100',  // Store API 地址
    REFRESH_INTERVAL: 5000,                  // 刷新间隔（毫秒）
    CHART_COLORS: {
        primary: '#3b82f6',
        success: '#10b981',
        // ...
    }
};
```

## 文档

- [快速开始](QUICKSTART.md) - 5分钟上手指南
- [安装指南](INSTALL.md) - 详细安装说明
- [功能详解](FEATURES.md) - 完整功能介绍
- [API 文档](API.md) - REST API 说明
- [贡献指南](CONTRIBUTING.md) - 如何贡献代码
- [更新日志](CHANGELOG.md) - 版本历史

## 测试

访问 `test.html` 测试 API 连接：

```bash
open http://localhost:8080/test.html
```

## 故障排除

### 连接失败

1. 确认 Store 服务运行在 9100 端口
2. 检查防火墙设置
3. 查看浏览器控制台错误

### CORS 错误

配置 Store 服务 CORS 头或使用反向代理。

### 数据不显示

1. 检查 Store 数据库是否有数据
2. 查看网络请求响应
3. 确认 API 端点返回正确格式

## 开发

### 安装依赖

```bash
npm install
```

### 启动开发服务器

```bash
npm run dev
```

### 代码规范

- 使用 ES6+ 语法
- 遵循模块化设计
- 添加必要注释
- 保持代码简洁

## 贡献

欢迎贡献代码、报告问题或提出建议！

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

详见 [贡献指南](CONTRIBUTING.md)。

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件

## 致谢

- [Chart.js](https://www.chartjs.org/) - 数据可视化
- [Lattice](https://github.com/lattice) - 去中心化即时通讯

## 联系方式

- 问题反馈: [GitHub Issues](https://github.com/lattice/Lattice/issues)
- 代码贡献: [Pull Requests](https://github.com/lattice/Lattice/pulls)

---

**由 Lattice Team 用 ❤️ 制作**
