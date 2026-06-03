# NextIM Store 管理面板 - 快速开始

5 分钟快速上手指南。

## 第一步：启动 Store 服务

确保 NextIM Store 服务正在运行：

```bash
cd NextIM
cargo run --bin nextim-store
```

默认监听端口：`9100`

## 第二步：启动管理面板

选择以下任一方式：

### 方式 A: Python（推荐）

```bash
cd web/store-admin
python -m http.server 8080
```

### 方式 B: Node.js

```bash
cd web/store-admin
npx http-server -p 8080 -c-1
```

### 方式 C: PHP

```bash
cd web/store-admin
php -S localhost:8080
```

## 第三步：访问管理面板

打开浏览器访问：

```
http://localhost:8080
```

## 第四步：验证连接

1. 查看右下角连接状态指示器
   - 🟢 绿色 = 已连接
   - 🔴 红色 = 未连接

2. 如果显示红色，检查：
   - Store 服务是否运行
   - 端口 9100 是否可访问
   - 浏览器控制台是否有错误

## 第五步：测试 API

访问测试页面验证 API 连接：

```
http://localhost:8080/test.html
```

点击"运行所有测试"按钮，确保所有测试通过。

## 功能导览

### 仪表板
- 查看实时统计数据
- 查看消息趋势图表
- 监控存储使用情况

### 消息管理
- 浏览所有消息
- 搜索特定消息
- 删除不需要的消息

### 联系人管理
- 查看所有联系人
- 查看在线状态
- 管理联系人信息

### 房间管理
- 查看所有聊天房间
- 查看房间成员
- 管理房间设置

### 系统日志
- 查看实时日志
- 按级别过滤
- 搜索日志内容

## 常见问题

### Q: 连接失败怎么办？

**A**: 检查以下几点：
1. Store 服务是否运行：`curl http://localhost:9100/stats`
2. 防火墙是否阻止连接
3. 浏览器控制台是否有 CORS 错误

### Q: 数据不显示？

**A**: 可能原因：
1. Store 数据库为空（正常情况）
2. API 返回格式不匹配
3. 查看浏览器控制台错误信息

### Q: 图表不显示？

**A**: 检查：
1. Chart.js CDN 是否可访问
2. 浏览器控制台是否有错误
3. 数据格式是否正确

### Q: 如何修改 API 地址？

**A**: 编辑 `js/config.js`：

```javascript
const CONFIG = {
    API_BASE_URL: 'http://your-host:9100',
    // ...
};
```

### Q: 如何修改刷新间隔？

**A**: 编辑 `js/config.js`：

```javascript
const CONFIG = {
    REFRESH_INTERVAL: 10000, // 10 秒
    // ...
};
```

## 下一步

- 阅读 [功能详解](FEATURES.md) 了解所有功能
- 查看 [API 文档](API.md) 了解 API 详情
- 阅读 [安装指南](INSTALL.md) 了解部署方式

## 获取帮助

遇到问题？

1. 查看浏览器控制台错误
2. 检查 Store 服务日志
3. 运行 test.html 诊断
4. 提交 Issue 并附上错误信息

## 快捷键

- `Ctrl+F5` / `Cmd+Shift+R` - 强制刷新
- `F12` - 打开开发者工具
- `Ctrl+Shift+I` - 打开控制台

## 性能提示

- 调整刷新间隔以减少服务器负载
- 使用搜索功能而非加载全部数据
- 定期清理不需要的数据

## 安全提示

- 不要在公网暴露管理面板
- 使用 VPN 或 SSH 隧道访问
- 定期备份数据
- 监控异常访问

---

**祝使用愉快！**

如有问题或建议，欢迎提交 Issue。
