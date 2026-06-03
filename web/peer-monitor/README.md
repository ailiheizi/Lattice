# NextIM Peer 监控面板

完整的 Peer 节点监控系统，提供实时数据监控、性能分析、连接管理和缓存管理功能。

## 功能特性

### 后端 API
- **健康检查** (`GET /health`) - 服务健康状态
- **统计信息** (`GET /stats`) - 缓存数、中转数、延迟等实时统计
- **连接列表** (`GET /connections`) - 活跃连接和历史记录
- **缓存状态** (`GET /cache`) - 缓存详细信息和条目列表
- **配置信息** (`GET /config`) - 当前运行配置

### 前端功能

#### 1. 仪表板
- 实时统计卡片（缓存消息数、中转总数、投递数、延迟、连接数、错误数）
- 消息中转速率图表
- 缓存使用情况饼图
- 系统信息展示（运行时间、监听地址、缓存容量、TTL）

#### 2. 性能指标
- 延迟分布折线图
- 吞吐量趋势图
- 性能统计表格（当前值、平均值、最大值、最小值）

#### 3. 连接管理
- 活跃连接列表（连接ID、远程地址、连接时间、消息数、状态）
- 连接历史记录（持续时长、消息数统计）
- 手动刷新功能

#### 4. 缓存管理
- 缓存统计（当前条目数、使用率、最旧条目）
- 按接收方分组的缓存条目列表
- 过期状态标识
- 清空缓存功能（需后端支持）

#### 5. 配置查看
- 监听地址
- API 地址
- 最大缓存条目
- 缓存 TTL
- 清理间隔
- 代收 Store 列表

#### 6. 日志查看
- 实时日志流
- 日志级别过滤（错误、警告、信息、调试）
- 自动滚动
- 清空显示

## 技术架构

### 后端
- **框架**: Axum (Rust)
- **CORS**: 支持跨域访问
- **静态文件**: 自动服务 web/peer-monitor 目录
- **数据结构**:
  - `ApiState` - API 状态管理
  - `PeerStats` - 统计信息
  - `CacheStats` - 缓存统计
  - `CacheEntryInfo` - 缓存条目详情

### 前端
- **UI 框架**: 原生 JavaScript (无依赖)
- **图表库**: Chart.js 4.4.0
- **架构模式**: 模块化设计
  - `api.js` - API 调用封装
  - `dashboard.js` - 仪表板逻辑
  - `performance.js` - 性能监控
  - `connections.js` - 连接管理
  - `cache.js` - 缓存管理
  - `charts.js` - 图表绘制
  - `app.js` - 主应用逻辑

## 配置说明

在 `nextim-peer.toml` 中添加 API 地址配置：

```toml
# REST API 监听地址
api_addr = "0.0.0.0:9201"
```

## 启动方式

1. 启动 Peer 节点：
```bash
cargo run --package nextim-peer --release
```

2. 访问监控面板：
```
http://localhost:9201/
```

## 自动更新机制

- **仪表板**: 每 2 秒更新
- **性能指标**: 每 2 秒更新
- **连接列表**: 每 5 秒更新
- **缓存状态**: 每 3 秒更新
- **连接状态**: 每 5 秒检查

## 数据流

```
Peer 节点 (Rust)
    ↓
REST API (Axum)
    ↓
HTTP JSON
    ↓
前端 JavaScript
    ↓
Chart.js 可视化
```

## 文件结构

```
crates/nextim-peer/src/
├── api.rs          # REST API 实现 (176 行)
├── main.rs         # 主程序（集成 API 服务）
└── cache.rs        # 缓存统计支持

web/peer-monitor/
├── index.html      # 主页面
├── styles.css      # 样式表
└── js/
    ├── api.js          # API 调用 (43 行)
    ├── dashboard.js    # 仪表板 (73 行)
    ├── performance.js  # 性能监控 (113 行)
    ├── connections.js  # 连接管理 (104 行)
    ├── cache.js        # 缓存管理 (139 行)
    ├── charts.js       # 图表绘制 (258 行)
    └── app.js          # 主应用 (222 行)
```

## 代码统计

- **后端代码**: 176 行 (Rust)
- **前端代码**: 952 行 (JavaScript)
- **总计**: 1,128 行

## 浏览器兼容性

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

## 未来扩展

- [ ] WebSocket 实时推送
- [ ] 日志流式传输
- [ ] 缓存清理 API
- [ ] 连接断开 API
- [ ] 性能告警
- [ ] 数据导出功能
