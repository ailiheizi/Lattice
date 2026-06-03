# NextIM Store REST API 文档

管理面板使用的 Store REST API 端点说明。

## 基础信息

**Base URL**: `http://localhost:9100`

**Content-Type**: `application/json`

**认证**: 当前版本无需认证（生产环境建议添加）

## API 端点

### 1. 统计信息

#### 获取系统统计

```http
GET /stats
```

**响应示例**:
```json
{
  "total_messages": 1234,
  "total_contacts": 56,
  "total_rooms": 12,
  "storage_used": 104857600,
  "storage_total": 1073741824,
  "message_trend": {
    "labels": ["2026-03-15", "2026-03-16", ...],
    "values": [45, 67, 89, ...]
  },
  "recent_activity": {
    "labels": ["00:00", "01:00", ...],
    "values": [12, 8, 15, ...]
  }
}
```

**字段说明**:
- `total_messages`: 总消息数
- `total_contacts`: 联系人总数
- `total_rooms`: 房间总数
- `storage_used`: 已使用存储（字节）
- `storage_total`: 总存储空间（字节）
- `message_trend`: 消息趋势数据（可选）
- `recent_activity`: 最近活动数据（可选）

---

### 2. 消息管理

#### 获取消息列表

```http
GET /messages?limit=100
```

**查询参数**:
- `limit` (可选): 返回消息数量，默认 100

**响应示例**:
```json
[
  {
    "id": "msg_123",
    "message_id": "msg_123",
    "from": "user_fingerprint_1",
    "to": "user_fingerprint_2",
    "content": "Hello, world!",
    "message_type": "text",
    "timestamp": "2026-03-22T12:00:00Z",
    "room_id": "room_abc"
  }
]
```

**字段说明**:
- `id` / `message_id`: 消息唯一标识
- `from`: 发送者指纹
- `to`: 接收者指纹
- `content`: 消息内容
- `message_type`: 消息类型（text/image/file/audio/video）
- `timestamp`: 时间戳
- `room_id`: 所属房间 ID（可选）

#### 获取特定联系人的消息

```http
GET /messages/:contact_id?limit=100
```

**路径参数**:
- `contact_id`: 联系人 ID

**查询参数**:
- `limit` (可选): 返回消息数量

**响应**: 同上

#### 删除消息

```http
DELETE /messages/:message_id
```

**路径参数**:
- `message_id`: 消息 ID

**响应**:
```json
{
  "success": true,
  "message": "Message deleted"
}
```

---

### 3. 联系人管理

#### 获取联系人列表

```http
GET /contacts
```

**响应示例**:
```json
[
  {
    "id": "contact_fingerprint",
    "contact_id": "contact_fingerprint",
    "name": "Alice",
    "last_seen": "2026-03-22T12:00:00Z",
    "message_count": 45,
    "store_address": "ws://example.com:9100"
  }
]
```

**字段说明**:
- `id` / `contact_id`: 联系人指纹
- `name`: 显示名称
- `last_seen`: 最后在线时间
- `message_count`: 消息数量
- `store_address`: Store 服务地址

#### 获取特定联系人

```http
GET /contacts/:contact_id
```

**路径参数**:
- `contact_id`: 联系人 ID

**响应**: 单个联系人对象

#### 删除联系人

```http
DELETE /contacts/:contact_id
```

**路径参数**:
- `contact_id`: 联系人 ID

**响应**:
```json
{
  "success": true,
  "message": "Contact deleted"
}
```

**注意**: 删除联系人会级联删除相关消息

---

### 4. 房间管理

#### 获取房间列表

```http
GET /rooms
```

**响应示例**:
```json
[
  {
    "id": "room_abc",
    "room_id": "room_abc",
    "name": "General Chat",
    "created_at": "2026-03-20T10:00:00Z",
    "members": ["user1", "user2", "user3"],
    "message_count": 234,
    "room_type": "group",
    "encrypted": true
  }
]
```

**字段说明**:
- `id` / `room_id`: 房间唯一标识
- `name`: 房间名称
- `created_at`: 创建时间
- `members`: 成员列表
- `message_count`: 消息数量
- `room_type`: 房间类型（group/direct/channel）
- `encrypted`: 是否加密

#### 获取特定房间

```http
GET /rooms/:room_id
```

**路径参数**:
- `room_id`: 房间 ID

**响应**: 单个房间对象

#### 删除房间

```http
DELETE /rooms/:room_id
```

**路径参数**:
- `room_id`: 房间 ID

**响应**:
```json
{
  "success": true,
  "message": "Room deleted"
}
```

**注意**: 删除房间会级联删除房间内所有消息

---

### 5. 日志查询（可选）

#### 获取系统日志

```http
GET /logs?limit=100
```

**查询参数**:
- `limit` (可选): 返回日志条数

**响应示例**:
```json
[
  {
    "timestamp": "2026-03-22T12:00:00Z",
    "level": "info",
    "source": "store",
    "message": "Server started on port 9100"
  }
]
```

**字段说明**:
- `timestamp`: 时间戳
- `level`: 日志级别（error/warning/info/debug）
- `source`: 日志来源
- `message`: 日志消息

**注意**: 此端点可能不存在，管理面板已做兼容处理

---

## 错误响应

所有 API 在出错时返回标准错误格式：

```json
{
  "error": "Error message",
  "code": "ERROR_CODE",
  "details": "Additional details"
}
```

**HTTP 状态码**:
- `200 OK`: 成功
- `400 Bad Request`: 请求参数错误
- `404 Not Found`: 资源不存在
- `500 Internal Server Error`: 服务器错误

---

## CORS 配置

如果管理面板和 Store 服务不在同一域名，需要配置 CORS：

**必需的响应头**:
```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type
```

**生产环境建议**:
- 限制 `Access-Control-Allow-Origin` 为特定域名
- 添加认证机制
- 使用 HTTPS

---

## 速率限制

建议实现速率限制以防止滥用：

- 每个 IP 每分钟最多 60 个请求
- 删除操作每分钟最多 10 次
- 统计查询每秒最多 1 次

---

## 数据格式约定

### 时间戳

所有时间戳使用 ISO 8601 格式：
```
2026-03-22T12:00:00Z
```

### 指纹格式

用户指纹为 Base64 编码的公钥哈希：
```
ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/
```

### 消息类型

支持的消息类型：
- `text`: 文本消息
- `image`: 图片消息
- `file`: 文件消息
- `audio`: 音频消息
- `video`: 视频消息

### 房间类型

支持的房间类型：
- `group`: 群组聊天
- `direct`: 一对一私聊
- `channel`: 频道（单向广播）

---

## 实现建议

### 分页

对于大数据量，建议实现分页：

```http
GET /messages?limit=50&offset=100
```

### 排序

支持按时间排序：

```http
GET /messages?sort=desc&order_by=timestamp
```

### 过滤

支持多条件过滤：

```http
GET /messages?from=user1&type=text&after=2026-03-20
```

### 批量操作

支持批量删除：

```http
POST /messages/batch-delete
Content-Type: application/json

{
  "ids": ["msg1", "msg2", "msg3"]
}
```

---

## 测试工具

使用管理面板提供的 `test.html` 测试所有 API 端点：

```bash
# 启动管理面板
cd web/store-admin
python -m http.server 8080

# 访问测试页面
open http://localhost:8080/test.html
```

---

## 安全建议

1. **认证**: 添加 API 密钥或 JWT 认证
2. **授权**: 实现基于角色的访问控制
3. **加密**: 使用 HTTPS 传输
4. **验证**: 验证所有输入参数
5. **日志**: 记录所有 API 访问
6. **限流**: 实现速率限制
7. **审计**: 记录敏感操作

---

## 版本控制

建议在 URL 中包含版本号：

```
http://localhost:9100/v1/stats
http://localhost:9100/v1/messages
```

---

## 联系支持

如有 API 相关问题，请：
1. 查看 Store 服务日志
2. 使用 test.html 测试端点
3. 检查网络请求详情
4. 提交 Issue 并附上错误信息
