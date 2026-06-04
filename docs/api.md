# NextIM API 文档

本文档当前只作为公开概览入口，不再把旧示例直接当成最终契约。

## 当前状态

- 蓝图主锚点：`docs/plans/2026-03-18-feat-nextim-implementation-plan.md`
- 当前契约事实源：`.plans/nextim-dev/docs/api-contracts.md`
- 当前剩余缺口：`.plans/nextim-dev/docs/gap-remediation.md`
- 精确实现入口：`crates/nextim-store/src/api.rs`、`crates/nextim-peer/src/api.rs`、`proto/transport.proto`

本次审计已确认以下事项仍在收敛中，因此本文不再声称 API/WS 契约已经完整定稿：

- Store forwarding 相关行为
- 真实集成测试覆盖
- Peer observability 的稳定字段面
- DHT / STUN 运行时接线
- E2EE / 多设备运行时集成
- README、blueprint、代码入口之间的 contract drift
- **消息签名验证当前不强制**：响应中的 `"verified"` 字段仅表示"是否通过校验"，但校验未通过（签名为空、无发送方公钥、验签失败）的消息当前仍会被存储。签名内容也尚未覆盖全部元数据，且无 `prev_hash` 链式防伪。详见 `.plans/nextim-dev/docs/gap-remediation.md` 第 7 条。

## 目录

- [REST API](#rest-api)
  - [身份相关](#身份相关)
  - [消息相关](#消息相关)
  - [联系人相关](#联系人相关)
  - [房间相关](#房间相关)
  - [搜索相关](#搜索相关)
- [WebSocket 协议](#websocket-协议)
- [错误处理](#错误处理)

## REST API

### 基础信息

- **Base URL**: 以实际启动配置为准，旧文档中的默认端口仅可视为历史示例
- **Content-Type**: `application/json`
- **字符编码**: UTF-8

### 当前说明

- Store 与 Peer 当前都带有 REST 管理面。
- 本页后续各小节中的 endpoint 与示例多数来自较早版本文档，只能帮助理解接口形态，不能替代事实源。
- 若要做实现、联调或测试，请直接回到 `.plans/nextim-dev/docs/api-contracts.md` 与代码入口核对。

### 身份相关

#### 获取身份信息

获取当前 Store 节点的身份信息。字段名与返回体仍需和代码入口继续核对。

**请求：**
```http
GET /identity
```

**响应：**
```json
{
  "fingerprint": "a1b2c3d4e5f6...",
  "display_name": "Alice",
  "ed25519_public_key": "base64_encoded_key",
  "curve25519_public_key": "base64_encoded_key",
  "created_at": 1710000000000
}
```

**示例：**
```bash
curl http://localhost:9100/identity
```

---

#### 获取 E2EE 密钥

获取端到端加密所需的密钥信息。该接口存在不等于 E2EE 运行时链路已完成闭环。

**请求：**
```http
GET /identity/keys
```

**响应：**
```json
{
  "identity_key": "base64_encoded_key",
  "one_time_keys": [
    {
      "key_id": "otk_001",
      "key": "base64_encoded_key"
    }
  ]
}
```

---

### 消息相关

#### 发送消息

发送消息到指定房间。消息写入、转发、加密与多设备同步的整体语义仍待统一验证。

**请求：**
```http
POST /messages
Content-Type: application/json

{
  "room_id": "room-123",
  "text": "Hello, World!",
  "encrypted": false
}
```

**响应：**
```json
{
  "msg_id": "msg-uuid-123",
  "timestamp": 1710000000000
}
```

**示例：**
```bash
curl -X POST http://localhost:9100/messages \
  -H "Content-Type: application/json" \
  -d '{
    "room_id": "room-123",
    "text": "Hello, World!"
  }'
```

---

#### 获取房间消息

获取指定房间的消息列表。路径与查询参数是否完全匹配当前实现，仍需以控制面契约文档为准。

**请求：**
```http
GET /messages/:room_id?since=0&until=9999999999999&limit=50
```

**响应：**
```json
[
  {
    "msg_id": "msg-uuid-123",
    "room_id": "room-123",
    "sender_fingerprint": "a1b2c3d4e5f6...",
    "timestamp": 1710000000000,
    "text": "Hello, World!",
    "encrypted": false,
    "verified": true
  }
]
```

---

### 联系人相关

#### 添加联系人

添加新的联系人。当前示例用于说明资源形态，不代表字段集合已经冻结。

**请求：**
```http
POST /contacts
Content-Type: application/json

{
  "fingerprint": "a1b2c3d4e5f6...",
  "display_name": "Bob",
  "trust_level": "TOFU"
}
```

**响应：**
```json
{
  "success": true,
  "contact": {
    "fingerprint": "a1b2c3d4e5f6...",
    "display_name": "Bob",
    "trust_level": "TOFU",
    "added_at": 1710000000000
  }
}
```

---

#### 获取联系人列表

获取所有联系人。精确字段请回看代码入口与控制面契约文档。

**请求：**
```http
GET /contacts
```

**响应：**
```json
[
  {
    "fingerprint": "a1b2c3d4e5f6...",
    "display_name": "Bob",
    "trust_level": "TOFU",
    "added_at": 1710000000000
  }
]
```

---

### 房间相关

#### 创建房间

创建新的房间（群组）。房间成员管理与更多接口仍在收敛中。

**请求：**
```http
POST /rooms
Content-Type: application/json

{
  "name": "My Group",
  "room_type": "Group",
  "visibility": "Private"
}
```

**响应：**
```json
{
  "room_id": "room-123",
  "name": "My Group",
  "room_type": "Group",
  "visibility": "Private",
  "creator": "a1b2c3d4e5f6...",
  "created_at": 1710000000000
}
```

---

#### 获取房间列表

获取所有房间。当前文档未覆盖所有已出现的房间相关入口。

**请求：**
```http
GET /rooms
```

**响应：**
```json
[
  {
    "room_id": "room-123",
    "name": "My Group",
    "room_type": "Group",
    "visibility": "Private",
    "member_count": 5,
    "last_message_at": 1710000000000
  }
]
```

---

### 搜索相关

#### 搜索消息

全文搜索消息内容。搜索接口已存在，但请求/响应字段仍需与代码继续逐项核对。

**请求：**
```http
GET /search?q=keyword&limit=10&room_id=room-123
```

**响应：**
```json
[
  {
    "msg_id": "msg-uuid-123",
    "room_id": "room-123",
    "snippet": "...Hello, <em>World</em>!...",
    "score": 0.95,
    "timestamp": 1710000000000
  }
]
```

**示例：**
```bash
# 全局搜索
curl "http://localhost:9100/search?q=hello&limit=10"

# 中文搜索
curl "http://localhost:9100/search?q=你好"
```

---

## WebSocket 协议

### 连接

**WebSocket URL**: 以实际 `ws_addr` 配置为准

**连接示例：**
```javascript
const ws = new WebSocket('ws://localhost:9100');

ws.onopen = () => {
  console.log('Connected to Store');
};

ws.onmessage = (event) => {
  const data = new Uint8Array(event.data);
  // 解析 Protobuf Frame
};
```

### 消息格式

旧版 `Frame { type, payload }` 示例已不能代表当前事实。

当前应以以下路径作为 WebSocket / Protobuf 契约来源：

- `proto/transport.proto`
- `nextim_proto::transport::Frame`
- `.plans/nextim-dev/docs/api-contracts.md`

在完成契约收敛前，任何联调方都不应只依赖本页旧 frame 示例实现客户端或服务端。

---

## 错误处理

### HTTP 错误码

| 状态码 | 说明 |
|--------|------|
| 200 OK | 请求成功 |
| 400 Bad Request | 请求参数错误 |
| 404 Not Found | 资源不存在 |
| 409 Conflict | 资源冲突 |
| 500 Internal Server Error | 服务器内部错误 |

### 错误响应格式

```json
{
  "error": "error_code",
  "message": "Human readable error message"
}
```

---

**注意：** API 正在按审计结果收敛中。若发现本页示例与代码不一致，请以控制面契约文档和实现入口为准。
