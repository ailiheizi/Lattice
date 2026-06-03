# NextIM Store 管理面板 - 安装指南

## 前置要求

1. NextIM Store 服务运行中（默认端口 9100）
2. 现代浏览器（Chrome/Firefox/Safari/Edge）
3. HTTP 服务器（用于提供静态文件）

## 快速安装

### 方法 1: Python HTTP 服务器（推荐）

```bash
cd web/store-admin
python -m http.server 8080
```

访问: http://localhost:8080

### 方法 2: Node.js HTTP 服务器

```bash
cd web/store-admin
npx http-server -p 8080 -c-1
```

访问: http://localhost:8080

### 方法 3: PHP 内置服务器

```bash
cd web/store-admin
php -S localhost:8080
```

访问: http://localhost:8080

### 方法 4: Nginx 配置

```nginx
server {
    listen 8080;
    server_name localhost;
    
    root /path/to/NextIM/web/store-admin;
    index index.html;
    
    location / {
        try_files $uri $uri/ /index.html;
    }
    
    # CORS 配置（如果需要）
    location /api/ {
        proxy_pass http://localhost:9100/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## 配置 Store API 地址

如果 Store 服务不在 `localhost:9100`，需要修改配置：

编辑 `js/config.js`:

```javascript
const CONFIG = {
    API_BASE_URL: 'http://your-store-host:9100',
    REFRESH_INTERVAL: 5000,
    // ...
};
```

## CORS 配置

如果遇到 CORS 错误，需要在 Store 服务端配置 CORS 头：

在 Store 服务的 HTTP 响应中添加：

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type
```

或者使用反向代理（推荐生产环境）。

## 验证安装

1. 打开浏览器访问管理面板
2. 检查右下角连接状态指示器
3. 访问 `test.html` 运行 API 测试
4. 查看浏览器控制台确认无错误

## 故障排除

### 连接失败

- 确认 Store 服务正在运行
- 检查防火墙设置
- 验证 API 地址配置正确
- 查看浏览器控制台错误信息

### CORS 错误

- 配置 Store 服务 CORS 头
- 或使用反向代理
- 或使用浏览器 CORS 插件（仅开发环境）

### 数据不显示

- 检查 Store 数据库是否有数据
- 查看网络请求响应
- 确认 API 端点返回正确格式

### 图表不显示

- 确认 Chart.js CDN 可访问
- 检查浏览器控制台错误
- 验证数据格式正确

## 生产部署建议

1. 使用 HTTPS
2. 配置反向代理
3. 启用 Gzip 压缩
4. 设置适当的缓存策略
5. 限制管理面板访问（IP 白名单/认证）

## 性能优化

1. 调整自动刷新间隔（`CONFIG.REFRESH_INTERVAL`）
2. 限制数据加载量（API 查询参数）
3. 使用 CDN 加速静态资源
4. 启用浏览器缓存

## 安全建议

1. 不要在公网直接暴露管理面板
2. 使用 VPN 或 SSH 隧道访问
3. 配置 HTTP 基本认证
4. 定期更新依赖库
5. 监控异常访问

## 更新

拉取最新代码后，清除浏览器缓存：

```bash
# 强制刷新: Ctrl+F5 (Windows/Linux) 或 Cmd+Shift+R (Mac)
```

## 技术支持

遇到问题请查看：
- 浏览器控制台错误
- Store 服务日志
- test.html 测试结果

提交 Issue 时请包含：
- 浏览器版本
- Store 版本
- 错误信息截图
- 网络请求详情
