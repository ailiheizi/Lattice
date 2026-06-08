# Lattice Web Chat - 部署指南

## 部署方式

### 方式 1: 本地文件访问（开发）

**优点**: 简单快速，无需服务器
**缺点**: 某些浏览器可能限制功能

```bash
# 直接在浏览器中打开
file:///D:/windows/code/project/Lattice/web/chat/index.html
```

### 方式 2: Python HTTP 服务器（推荐）

**优点**: 简单，跨平台，适合开发和测试
**缺点**: 不适合生产环境

```bash
cd D:/windows/code/project/Lattice/web/chat
python -m http.server 8080

# 访问
http://localhost:8080
```

### 方式 3: Node.js HTTP 服务器

**优点**: 快速，支持热重载
**缺点**: 需要 Node.js 环境

```bash
# 安装 http-server
npm install -g http-server

# 启动服务器
cd D:/windows/code/project/Lattice/web/chat
http-server -p 8080 -c-1

# 访问
http://localhost:8080
```

### 方式 4: Nginx（生产环境）

**优点**: 高性能，稳定，适合生产
**缺点**: 配置相对复杂

#### Nginx 配置示例

```nginx
server {
    listen 80;
    server_name chat.lattice.example.com;

    # 重定向到 HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name chat.lattice.example.com;

    # SSL 证书
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # 安全头
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "no-referrer-when-downgrade" always;

    # CSP 策略
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' ws://localhost:9100 http://localhost:9100; img-src 'self' data:;" always;

    # 根目录
    root /var/www/lattice/web/chat;
    index index.html;

    # 静态文件缓存
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # HTML 文件不缓存
    location ~* \.html$ {
        expires -1;
        add_header Cache-Control "no-store, no-cache, must-revalidate";
    }

    # Gzip 压缩
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types text/plain text/css text/xml text/javascript application/javascript application/json;

    # 主页面
    location / {
        try_files $uri $uri/ /index.html;
    }

    # 反向代理到 Store API
    location /api/ {
        proxy_pass http://localhost:9100/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket 支持
        proxy_read_timeout 86400;
    }
}
```

### 方式 5: Apache（生产环境）

#### Apache 配置示例

```apache
<VirtualHost *:80>
    ServerName chat.lattice.example.com
    Redirect permanent / https://chat.lattice.example.com/
</VirtualHost>

<VirtualHost *:443>
    ServerName chat.lattice.example.com

    # SSL 配置
    SSLEngine on
    SSLCertificateFile /path/to/cert.pem
    SSLCertificateKeyFile /path/to/key.pem

    # 文档根目录
    DocumentRoot /var/www/lattice/web/chat

    <Directory /var/www/lattice/web/chat>
        Options -Indexes +FollowSymLinks
        AllowOverride All
        Require all granted

        # 启用 Rewrite
        RewriteEngine On
        RewriteBase /
        RewriteRule ^index\.html$ - [L]
        RewriteCond %{REQUEST_FILENAME} !-f
        RewriteCond %{REQUEST_FILENAME} !-d
        RewriteRule . /index.html [L]
    </Directory>

    # 安全头
    Header always set X-Frame-Options "SAMEORIGIN"
    Header always set X-Content-Type-Options "nosniff"
    Header always set X-XSS-Protection "1; mode=block"

    # 压缩
    <IfModule mod_deflate.c>
        AddOutputFilterByType DEFLATE text/html text/plain text/xml text/css text/javascript application/javascript application/json
    </IfModule>

    # 缓存
    <IfModule mod_expires.c>
        ExpiresActive On
        ExpiresByType text/html "access plus 0 seconds"
        ExpiresByType text/css "access plus 1 year"
        ExpiresByType application/javascript "access plus 1 year"
        ExpiresByType image/* "access plus 1 year"
    </IfModule>

    # 反向代理到 Store API
    ProxyPreserveHost On
    ProxyPass /api/ http://localhost:9100/
    ProxyPassReverse /api/ http://localhost:9100/

    # WebSocket 支持
    RewriteEngine On
    RewriteCond %{HTTP:Upgrade} websocket [NC]
    RewriteCond %{HTTP:Connection} upgrade [NC]
    RewriteRule ^/api/(.*)$ ws://localhost:9100/$1 [P,L]
</VirtualHost>
```

### 方式 6: Docker 容器

#### Dockerfile

```dockerfile
FROM nginx:alpine

# 复制文件
COPY web/chat /usr/share/nginx/html

# 复制 Nginx 配置
COPY nginx.conf /etc/nginx/conf.d/default.conf

# 暴露端口
EXPOSE 80

# 启动 Nginx
CMD ["nginx", "-g", "daemon off;"]
```

#### docker-compose.yml

```yaml
version: '3.8'

services:
  # Store 服务
  store:
    build:
      context: .
      dockerfile: Dockerfile.store
    ports:
      - "9100:9100"
    volumes:
      - store-data:/data
    environment:
      - RUST_LOG=info

  # Web 前端
  web:
    build:
      context: .
      dockerfile: Dockerfile.web
    ports:
      - "8080:80"
    depends_on:
      - store
    environment:
      - STORE_API_URL=http://store:9100

volumes:
  store-data:
```

#### 启动命令

```bash
# 构建镜像
docker-compose build

# 启动服务
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止服务
docker-compose down
```

## 环境配置

### 开发环境

```bash
# 1. 启动 Store 服务
cd D:/windows/code/project/Lattice
cargo run --bin lattice-store

# 2. 启动 Web 服务器
cd web/chat
python -m http.server 8080

# 3. 访问
# http://localhost:8080
```

### 测试环境

```bash
# 使用 Docker Compose
docker-compose -f docker-compose.test.yml up

# 或使用 Nginx
sudo systemctl start nginx
```

### 生产环境

```bash
# 1. 构建优化版本（如果使用构建工具）
npm run build

# 2. 部署到服务器
rsync -avz web/chat/ user@server:/var/www/lattice/

# 3. 配置 Nginx/Apache
sudo systemctl reload nginx

# 4. 启动 Store 服务
systemctl start lattice-store
```

## 性能优化

### 1. 启用 Gzip 压缩

**Nginx**:
```nginx
gzip on;
gzip_vary on;
gzip_min_length 1024;
gzip_types text/plain text/css text/xml text/javascript application/javascript application/json;
```

**Apache**:
```apache
<IfModule mod_deflate.c>
    AddOutputFilterByType DEFLATE text/html text/plain text/xml text/css text/javascript application/javascript application/json
</IfModule>
```

### 2. 配置缓存策略

**静态资源**: 长期缓存（1 年）
```nginx
location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg)$ {
    expires 1y;
    add_header Cache-Control "public, immutable";
}
```

**HTML 文件**: 不缓存
```nginx
location ~* \.html$ {
    expires -1;
    add_header Cache-Control "no-store, no-cache, must-revalidate";
}
```

### 3. 启用 HTTP/2

```nginx
listen 443 ssl http2;
```

### 4. 使用 CDN

将静态资源部署到 CDN：
- CSS 文件
- JavaScript 文件
- 图片资源

### 5. 代码压缩（可选）

如果需要进一步优化，可以使用构建工具：

```bash
# 安装工具
npm install -g terser clean-css-cli html-minifier

# 压缩 JS
terser js/app.js -o js/app.min.js -c -m

# 压缩 CSS
cleancss -o css/style.min.css css/style.css

# 压缩 HTML
html-minifier --collapse-whitespace --remove-comments index.html -o index.min.html
```

## 安全配置

### 1. HTTPS 强制

```nginx
# 重定向 HTTP 到 HTTPS
server {
    listen 80;
    server_name chat.lattice.example.com;
    return 301 https://$server_name$request_uri;
}
```

### 2. 安全头配置

```nginx
# X-Frame-Options
add_header X-Frame-Options "SAMEORIGIN" always;

# X-Content-Type-Options
add_header X-Content-Type-Options "nosniff" always;

# X-XSS-Protection
add_header X-XSS-Protection "1; mode=block" always;

# Referrer-Policy
add_header Referrer-Policy "no-referrer-when-downgrade" always;

# Content-Security-Policy
add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' ws://localhost:9100 http://localhost:9100;" always;
```

### 3. 速率限制

```nginx
# 限制请求速率
limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;

location /api/ {
    limit_req zone=api burst=20 nodelay;
    proxy_pass http://localhost:9100/;
}
```

### 4. 防火墙配置

```bash
# UFW (Ubuntu)
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 9100/tcp  # Store API (仅内部访问)
sudo ufw enable

# firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --permanent --add-port=9100/tcp
sudo firewall-cmd --reload
```

## 监控和日志

### 1. Nginx 访问日志

```nginx
access_log /var/log/nginx/lattice-access.log combined;
error_log /var/log/nginx/lattice-error.log warn;
```

### 2. 日志分析

```bash
# 查看访问量
tail -f /var/log/nginx/lattice-access.log

# 统计 IP 访问次数
awk '{print $1}' /var/log/nginx/lattice-access.log | sort | uniq -c | sort -rn | head -10

# 统计状态码
awk '{print $9}' /var/log/nginx/lattice-access.log | sort | uniq -c | sort -rn
```

### 3. 性能监控

使用工具：
- **Prometheus + Grafana**: 系统监控
- **New Relic**: APM 监控
- **Google Analytics**: 用户行为分析

## 备份策略

### 1. 代码备份

```bash
# Git 仓库
git push origin main

# 定期备份
rsync -avz /var/www/lattice/ /backup/lattice-$(date +%Y%m%d)/
```

### 2. 数据备份

```bash
# Store 数据库备份
cp /data/lattice/store.db /backup/store-$(date +%Y%m%d).db

# 自动备份脚本
#!/bin/bash
BACKUP_DIR="/backup/lattice"
DATE=$(date +%Y%m%d)
mkdir -p $BACKUP_DIR
cp /data/lattice/store.db $BACKUP_DIR/store-$DATE.db
find $BACKUP_DIR -name "store-*.db" -mtime +30 -delete
```

## 故障排查

### 1. 无法连接到 Store

**检查**:
```bash
# Store 服务是否运行
ps aux | grep lattice-store

# 端口是否监听
netstat -tlnp | grep 9100

# 防火墙是否阻止
sudo ufw status
```

**解决**:
```bash
# 启动 Store 服务
cargo run --bin lattice-store

# 开放端口
sudo ufw allow 9100/tcp
```

### 2. WebSocket 连接失败

**检查**:
- Nginx/Apache 是否配置 WebSocket 支持
- 防火墙是否允许 WebSocket 连接
- 浏览器控制台错误信息

**解决**:
```nginx
# Nginx WebSocket 配置
proxy_http_version 1.1;
proxy_set_header Upgrade $http_upgrade;
proxy_set_header Connection "upgrade";
```

### 3. 静态资源 404

**检查**:
- 文件路径是否正确
- Nginx/Apache 根目录配置
- 文件权限

**解决**:
```bash
# 检查文件
ls -la /var/www/lattice/web/chat/

# 修复权限
sudo chown -R www-data:www-data /var/www/lattice/
sudo chmod -R 755 /var/www/lattice/
```

## 更新部署

### 1. 零停机更新

```bash
# 1. 备份当前版本
cp -r /var/www/lattice /var/www/lattice.backup

# 2. 部署新版本
rsync -avz web/chat/ /var/www/lattice/web/chat/

# 3. 重载 Nginx（不中断连接）
sudo nginx -t && sudo nginx -s reload

# 4. 验证
curl -I https://chat.lattice.example.com
```

### 2. 回滚

```bash
# 恢复备份
rm -rf /var/www/lattice
mv /var/www/lattice.backup /var/www/lattice

# 重载 Nginx
sudo nginx -s reload
```

## 扩展部署

### 1. 负载均衡

```nginx
upstream lattice_backend {
    server 127.0.0.1:9100;
    server 127.0.0.1:9101;
    server 127.0.0.1:9102;
}

server {
    location /api/ {
        proxy_pass http://lattice_backend/;
    }
}
```

### 2. 多实例部署

```bash
# 启动多个 Store 实例
cargo run --bin lattice-store -- --port 9100 &
cargo run --bin lattice-store -- --port 9101 &
cargo run --bin lattice-store -- --port 9102 &
```

### 3. 容器编排（Kubernetes）

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: lattice-web
spec:
  replicas: 3
  selector:
    matchLabels:
      app: lattice-web
  template:
    metadata:
      labels:
        app: lattice-web
    spec:
      containers:
      - name: nginx
        image: lattice-web:latest
        ports:
        - containerPort: 80
---
apiVersion: v1
kind: Service
metadata:
  name: lattice-web
spec:
  selector:
    app: lattice-web
  ports:
  - port: 80
    targetPort: 80
  type: LoadBalancer
```

## 总结

Lattice Web Chat 支持多种部署方式，从简单的本地开发到复杂的生产环境。选择合适的部署方式取决于你的需求：

- **开发**: Python HTTP 服务器
- **测试**: Docker Compose
- **生产**: Nginx + HTTPS + 监控

确保遵循安全最佳实践，定期备份数据，并监控系统性能。
