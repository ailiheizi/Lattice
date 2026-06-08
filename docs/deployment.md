# Lattice 部署指南

本文档介绍如何在生产环境中部署 Lattice 系统。

## 目录

- [系统要求](#系统要求)
- [部署架构](#部署架构)
- [Store 节点部署](#store-节点部署)
- [Peer 节点部署](#peer-节点部署)
- [安全配置](#安全配置)
- [性能优化](#性能优化)
- [监控和日志](#监控和日志)
- [备份和恢复](#备份和恢复)
- [故障排查](#故障排查)

## 系统要求

### 硬件要求

**Store 节点（最低配置）：**
- CPU: 1 核
- 内存: 512 MB
- 存储: 10 GB SSD
- 网络: 10 Mbps

**Store 节点（推荐配置）：**
- CPU: 2 核
- 内存: 2 GB
- 存储: 50 GB SSD
- 网络: 100 Mbps

**Peer 节点（最低配置）：**
- CPU: 1 核
- 内存: 256 MB
- 存储: 1 GB
- 网络: 100 Mbps

**Peer 节点（推荐配置）：**
- CPU: 2 核
- 内存: 1 GB
- 存储: 5 GB
- 网络: 1 Gbps

### 软件要求

- 操作系统: Linux (Ubuntu 20.04+, Debian 11+, CentOS 8+) / Windows Server 2019+ / macOS 11+
- Rust: 1.75+ (仅编译时需要)
- 可选: Docker / systemd

## 部署架构

### 单节点部署

适用于个人用户或小型团队：

```
┌─────────────────┐
│   Store Node    │
│  (127.0.0.1)    │
│                 │
│  - WebSocket    │
│  - REST API     │
│  - SQLite       │
│  - Tantivy      │
└─────────────────┘
```

### 多节点部署

适用于多用户场景：

```
┌──────────┐         ┌──────────┐         ┌──────────┐
│  Store   │◄───────►│   Peer   │◄───────►│  Store   │
│  Node A  │         │  Relay   │         │  Node B  │
│          │         │          │         │          │
│  User A  │         │  Public  │         │  User B  │
└──────────┘         └──────────┘         └──────────┘
```

### 高可用部署

适用于企业级应用：

```
                    ┌──────────┐
                    │   Peer   │
                    │  Relay 1 │
                    └────┬─────┘
                         │
┌──────────┐       ┌────▼─────┐       ┌──────────┐
│  Store   │◄─────►│   Peer   │◄─────►│  Store   │
│  Node A  │       │  Relay 2 │       │  Node B  │
└──────────┘       └────┬─────┘       └──────────┘
                         │
                    ┌────▼─────┐
                    │   Peer   │
                    │  Relay 3 │
                    └──────────┘
```

## Store 节点部署

### 1. 编译二进制

```bash
# 克隆仓库
git clone https://github.com/yourusername/Lattice.git
cd Lattice

# 编译 release 版本
cargo build --release --bin lattice-store

# 二进制文件位于
ls -lh target/release/lattice-store
```

### 2. 创建配置文件

```bash
# 创建配置目录
mkdir -p /etc/lattice

# 创建配置文件
cat > /etc/lattice/store.toml <<EOF
[server]
listen_addr = "0.0.0.0:9100"
data_dir = "/var/lib/lattice/store"

[identity]
display_name = "My Store Node"

[proxy]
proxy_store_address = ""

[log]
level = "info"
EOF
```

### 3. 创建数据目录

```bash
# 创建数据目录
sudo mkdir -p /var/lib/lattice/store
sudo chown -R lattice:lattice /var/lib/lattice

# 创建日志目录
sudo mkdir -p /var/log/lattice
sudo chown -R lattice:lattice /var/log/lattice
```

### 4. 使用 systemd 管理服务

创建 systemd 服务文件：

```bash
sudo cat > /etc/systemd/system/lattice-store.service <<EOF
[Unit]
Description=Lattice Store Node
After=network.target

[Service]
Type=simple
User=lattice
Group=lattice
WorkingDirectory=/var/lib/lattice
ExecStart=/usr/local/bin/lattice-store --config /etc/lattice/store.toml
Restart=on-failure
RestartSec=5s

# 安全配置
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/lattice /var/log/lattice

# 资源限制
LimitNOFILE=65536
LimitNPROC=512

# 日志
StandardOutput=journal
StandardError=journal
SyslogIdentifier=lattice-store

[Install]
WantedBy=multi-user.target
EOF
```

启动服务：

```bash
# 重载 systemd
sudo systemctl daemon-reload

# 启动服务
sudo systemctl start lattice-store

# 设置开机自启
sudo systemctl enable lattice-store

# 查看状态
sudo systemctl status lattice-store

# 查看日志
sudo journalctl -u lattice-store -f
```

### 5. 使用 Docker 部署

创建 Dockerfile：

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin lattice-store

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/lattice-store /usr/local/bin/

RUN useradd -m -u 1000 lattice && \
    mkdir -p /var/lib/lattice/store && \
    chown -R lattice:lattice /var/lib/lattice

USER lattice
WORKDIR /var/lib/lattice

EXPOSE 9100

CMD ["lattice-store", "--config", "/etc/lattice/store.toml"]
```

构建和运行：

```bash
# 构建镜像
docker build -t lattice-store:latest .

# 运行容器
docker run -d \
  --name lattice-store \
  -p 9100:9100 \
  -v /etc/lattice:/etc/lattice:ro \
  -v /var/lib/lattice:/var/lib/lattice \
  --restart unless-stopped \
  lattice-store:latest
```

使用 docker-compose：

```yaml
version: '3.8'

services:
  store:
    image: lattice-store:latest
    container_name: lattice-store
    ports:
      - "9100:9100"
    volumes:
      - ./config:/etc/lattice:ro
      - ./data:/var/lib/lattice
    restart: unless-stopped
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9100/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

## Peer 节点部署

### 1. 编译二进制

```bash
cargo build --release --bin lattice-peer
```

### 2. 创建配置文件

```bash
cat > /etc/lattice/peer.toml <<EOF
[server]
listen_addr = "0.0.0.0:9200"

[cache]
max_size = 10000
ttl_seconds = 300

[proxy]
proxy_stores = []

[log]
level = "info"
EOF
```

### 3. 使用 systemd 管理服务

```bash
sudo cat > /etc/systemd/system/lattice-peer.service <<EOF
[Unit]
Description=Lattice Peer Relay Node
After=network.target

[Service]
Type=simple
User=lattice
Group=lattice
WorkingDirectory=/var/lib/lattice
ExecStart=/usr/local/bin/lattice-peer --config /etc/lattice/peer.toml
Restart=on-failure
RestartSec=5s

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true

LimitNOFILE=65536
LimitNPROC=512

StandardOutput=journal
StandardError=journal
SyslogIdentifier=lattice-peer

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl start lattice-peer
sudo systemctl enable lattice-peer
```

## 安全配置

### 1. 防火墙配置

```bash
# Ubuntu/Debian (ufw)
sudo ufw allow 9100/tcp  # Store WebSocket
sudo ufw allow 9200/tcp  # Peer relay
sudo ufw enable

# CentOS/RHEL (firewalld)
sudo firewall-cmd --permanent --add-port=9100/tcp
sudo firewall-cmd --permanent --add-port=9200/tcp
sudo firewall-cmd --reload
```

### 2. TLS/SSL 配置

使用 nginx 作为反向代理：

```nginx
upstream lattice_store {
    server 127.0.0.1:9100;
}

server {
    listen 443 ssl http2;
    server_name store.example.com;

    ssl_certificate /etc/letsencrypt/live/store.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/store.example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://lattice_store;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket 超时
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }
}
```

### 3. 速率限制

在 nginx 中配置速率限制：

```nginx
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=10r/s;

location /api/ {
    limit_req zone=api_limit burst=20 nodelay;
    proxy_pass http://lattice_store;
}
```

## 性能优化

### 1. SQLite 优化

在配置文件中添加：

```toml
[storage]
# SQLite 优化参数
journal_mode = "WAL"
synchronous = "NORMAL"
cache_size = 10000
mmap_size = 268435456  # 256 MB
```

### 2. 系统参数优化

```bash
# 增加文件描述符限制
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# 优化网络参数
cat >> /etc/sysctl.conf <<EOF
net.core.somaxconn = 1024
net.ipv4.tcp_max_syn_backlog = 2048
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 30
EOF

sysctl -p
```

### 3. 资源监控

使用 prometheus 监控（待实现）：

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'lattice-store'
    static_configs:
      - targets: ['localhost:9100']
```

## 监控和日志

### 1. 日志配置

设置日志级别：

```bash
# 环境变量
export RUST_LOG=info

# 或在配置文件中
[log]
level = "info"  # trace, debug, info, warn, error
```

### 2. 日志轮转

配置 logrotate：

```bash
cat > /etc/logrotate.d/lattice <<EOF
/var/log/lattice/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 lattice lattice
    sharedscripts
    postrotate
        systemctl reload lattice-store
    endscript
}
EOF
```

### 3. 健康检查

```bash
# 检查 Store 节点状态
curl http://localhost:9100/health

# 检查 WebSocket 连接
wscat -c ws://localhost:9100
```

## 备份和恢复

### 1. 数据备份

```bash
#!/bin/bash
# backup.sh

BACKUP_DIR="/backup/lattice"
DATA_DIR="/var/lib/lattice/store"
DATE=$(date +%Y%m%d_%H%M%S)

# 创建备份目录
mkdir -p $BACKUP_DIR

# 备份数据库
sqlite3 $DATA_DIR/store.db ".backup $BACKUP_DIR/store_$DATE.db"

# 备份搜索索引
tar -czf $BACKUP_DIR/search_$DATE.tar.gz -C $DATA_DIR search_index/

# 删除 7 天前的备份
find $BACKUP_DIR -name "*.db" -mtime +7 -delete
find $BACKUP_DIR -name "*.tar.gz" -mtime +7 -delete

echo "Backup completed: $DATE"
```

设置定时备份：

```bash
# 添加到 crontab
crontab -e

# 每天凌晨 2 点备份
0 2 * * * /usr/local/bin/backup.sh >> /var/log/lattice/backup.log 2>&1
```

### 2. 数据恢复

```bash
#!/bin/bash
# restore.sh

BACKUP_FILE=$1
DATA_DIR="/var/lib/lattice/store"

# 停止服务
systemctl stop lattice-store

# 恢复数据库
cp $BACKUP_FILE $DATA_DIR/store.db

# 恢复搜索索引
tar -xzf ${BACKUP_FILE%.db}.tar.gz -C $DATA_DIR

# 启动服务
systemctl start lattice-store

echo "Restore completed"
```

## 故障排查

### 常见问题

#### 1. 无法连接到 Store 节点

**症状：** WebSocket 连接失败

**排查步骤：**
```bash
# 检查服务状态
systemctl status lattice-store

# 检查端口监听
netstat -tlnp | grep 9100

# 检查防火墙
sudo ufw status

# 查看日志
journalctl -u lattice-store -n 100
```

#### 2. 消息发送失败

**症状：** 消息无法发送或接收

**排查步骤：**
```bash
# 检查数据库
sqlite3 /var/lib/lattice/store/store.db "SELECT COUNT(*) FROM messages;"

# 检查磁盘空间
df -h /var/lib/lattice

# 检查日志
tail -f /var/log/lattice/store.log
```

#### 3. 性能问题

**症状：** 响应缓慢

**排查步骤：**
```bash
# 检查 CPU 使用率
top -p $(pgrep lattice-store)

# 检查内存使用
ps aux | grep lattice-store

# 检查数据库大小
du -sh /var/lib/lattice/store/store.db

# 优化数据库
sqlite3 /var/lib/lattice/store/store.db "VACUUM;"
```

### 调试模式

启用详细日志：

```bash
# 临时启用
RUST_LOG=debug systemctl restart lattice-store

# 永久启用
# 修改 /etc/lattice/store.toml
[log]
level = "debug"
```

## 升级指南

### 1. 备份数据

```bash
./backup.sh
```

### 2. 停止服务

```bash
systemctl stop lattice-store
```

### 3. 更新二进制

```bash
# 下载新版本
wget https://github.com/yourusername/Lattice/releases/download/v0.2.0/lattice-store

# 替换二进制
sudo mv lattice-store /usr/local/bin/
sudo chmod +x /usr/local/bin/lattice-store
```

### 4. 检查配置

```bash
# 检查配置文件兼容性
lattice-store --config /etc/lattice/store.toml --check-config
```

### 5. 启动服务

```bash
systemctl start lattice-store
systemctl status lattice-store
```

## 生产环境检查清单

部署前检查：

- [ ] 硬件资源充足
- [ ] 防火墙规则配置正确
- [ ] TLS/SSL 证书有效
- [ ] 数据目录权限正确
- [ ] 备份策略已配置
- [ ] 监控已启用
- [ ] 日志轮转已配置
- [ ] 系统参数已优化
- [ ] 健康检查正常
- [ ] 文档已更新

## 支持

如有问题，请：

1. 查看日志文件
2. 搜索 GitHub Issues
3. 创建新的 Issue
4. 联系技术支持

---

**注意：** 本指南持续更新中，请关注最新版本。
