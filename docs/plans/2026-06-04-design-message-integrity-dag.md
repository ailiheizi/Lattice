---
title: "design: Lattice 消息完整性与哈希 DAG"
type: design
date: 2026-06-04
status: draft
---

# Lattice 消息完整性与哈希 DAG 设计

## 0. 本文目的

确定 Lattice 消息层的**防伪造、防篡改、可排序**模型，覆盖通用 IM（1v1 + 群聊）的真实情况。本文是设计基线，实现以本文敲定的模型为准。

不在本文解决：传输 TLS、REST 鉴权（另见 `gap-remediation.md`）。本文聚焦消息内容与顺序的完整性。

## 1. 现状与缺陷（事实源）

当前签名实现：`crates/lattice-crypto/src/sign.rs`

```
payload_hash = sha256(payload)
signature    = sign(payload_hash)         // 只签 payload 哈希
```

三层缺陷：

1. **元数据未签名**：`msg_id` / `sender_fingerprint` / `recipient_fingerprint` 未进签名内容，可篡改而签名仍通过。证据 `sign.rs:56-64`。
2. **无链式绑定**：`proto/message.proto` 的 `Envelope` 无 `prev_hash`，消息间无关联，可重排/插入/凭空伪造历史。
3. **验签不强制**：`crates/lattice-store/src/server.rs:103` 验签失败/无签名/无公钥时 `verified=false`，但消息仍存储转发。Peer relay（`relay.rs:111`）更是零校验直接缓存。

**结论**：当前无法阻止一方伪造一整条逻辑自洽的历史，也无法阻止中间人篡改元数据。

## 2. 为什么是 DAG 而不是单链

单链假设消息严格线性，但真实 IM 不是。下列情况会击穿单链：

- 两人同时基于同一 `prev` 发消息 → 分叉
- 同一人多设备同时发 → 单人也分叉
- 离线/乱序到达 → 引用了本地还没有的 `prev`
- 中间消息永久丢失 → 单链在此断裂，后续全部无法验证

因此采用 **哈希 DAG（有向无环图）**：每条消息可引用**多个**父（`prev_hashes` 复数），并发即多个 head，合并时新消息同时指向它们。这是 Matrix / Git 的成熟做法。

三件事分开，各管各的：

| 关注点 | 机制 |
|--------|------|
| 防篡改/防伪造 | 每条消息签名覆盖全部关键字段 + prev_hashes |
| 处理并发 | DAG 多父结构 |
| 确定顺序 | 拓扑排序 + 确定性平局打破 |
| 时间可信 | Store 盖 `received_ts`，发送方 `origin_ts` 仅显示 |

## 3. 核心数据结构

### 3.1 消息哈希（签名内容）

```
msg_hash = sha256(
    msg_id              ‖
    sender_fingerprint  ‖
    recipient/room_id   ‖
    payload_bytes       ‖
    sort(prev_hashes)            // 父哈希排序后拼接，保证确定性
)
signature = sign(msg_hash, sender_ed25519_privkey)
```

**明确不纳入 `timestamp`**：发送方自填的时间不可信，纳入既不防伪又误导"顺序靠时间"。顺序完全由 DAG 拓扑决定。

### 3.2 proto 变更（`proto/message.proto` 的 Envelope）

```protobuf
message Envelope {
    string msg_id = 1;
    string sender_fingerprint = 2;
    string recipient_fingerprint = 3;
    uint64 timestamp = 4;              // 语义改为 origin_ts：发送方声称、仅显示、不可信、不进签名
    bytes signature = 5;              // 签 msg_hash（见 3.1）
    bytes payload_hash = 6;           // 语义升级为 msg_hash（覆盖全字段 + prev_hashes）
    repeated bytes prev_hashes = 7;   // 新增：父消息的 msg_hash 列表（可多父）
    // oneof payload 不变
}
```

`Message`（存储态）新增字段：`received_ts`（Store 盖戳）、`prev_hashes`、`seq`（全序计算后的稳定序号，可选缓存）。

### 3.3 时间的处理

- `origin_ts`（= 现 timestamp）：发送方声称，**仅用于显示**，明确标注不可信，不进签名。
- `received_ts`：Store 收到消息时用自己的时钟盖戳，用于本地排序/分页/时间窗判断。
- 全序的平局打破**不依赖时间**（见 4.2），received_ts 仅作辅助显示与重放时间窗。

## 4. 确定性全序

### 4.1 目标

所有节点对同一组消息独立计算，得到**完全一致**的线性顺序，否则各端历史不同。

### 4.2 算法

DAG 拓扑排序 + 确定性平局打破：

1. 按 DAG 依赖（prev_hashes）做拓扑排序，父永远排在子之前。
2. 同一拓扑层的并发消息（互不为祖先），按 **`msg_hash` 字典序**打破平局（纯密码学值，全节点一致，不可被时间操纵）。
3. 计算结果可缓存为 `seq`，DAG 增长时增量更新。

> 备选：可加入 `received_ts` 作为平局打破的首要键、msg_hash 为次要键，让顺序更贴近"到达感知"。但 received_ts 跨 Store 不统一，会牺牲全节点一致性。**推荐默认纯 msg_hash 排序**，received_ts 仅显示。此点列为决策点 D-3。

## 5. 各情况的处理（穷举）

### 5.1 并发与分叉
- **同时发 / 多设备同时发**：产生多个 head，DAG 自然容纳。下一条消息把当前所有 head 作为 prev_hashes 引用，分叉自动合并。
- **分叉长期不合并**：任意新消息都会把已知 head 全部纳入 prev，因此只要有后续发言就会收敛；无后续发言时，全序仍可由拓扑+msg_hash 唯一确定，不依赖合并。
- **恶意制造分叉**：限制单条 `prev_hashes` 数量上限（如 ≤20），超限拒绝；对引用过旧 head（深度超阈值）的消息标记并降权。

### 5.2 缺失与乱序
- **乱序到达**：引用了未知 prev → 放入**挂起区（pending）**，不落地、不验签通过；父到达后再校验落地。
- **永久丢失**：挂起区条目设 TTL + 主动拉取（向 Store / 发送方请求缺失 msg_hash）；超时仍缺则标记"历史空洞"，允许后续消息以"已知最早可达 head"继续，避免全链瘫痪。
- **挂起区膨胀**：容量上限 + TTL 清理 + 速率限制。

### 5.3 攻击面
- **重放**：`msg_id` 唯一约束 + prev_hashes 链 + received_ts 时间窗，三层。
- **伪造自洽历史**：签名 + msg_hash + DAG，改任一条则 hash 变、签名失效、引用对不上。
- **省略攻击**（中继故意不转发）：prev_hashes 暴露"缺了某个父"，接收方可据此发现并拉取，使省略可被检测。
- **DoS**：payload 大小上限、prev_hashes 数量上限、链深快照（见 7）。

### 5.4 身份与多设备
- **多设备 head 共享**：设备通过 `/devices` 发现同账号设备（已实现），同步各自 head；新设备从 Store 拉全量 DAG 追平。
- **设备密钥轮换**：保留**设备密钥历史**，旧消息按其签名时的设备公钥验证；设备注销不删除其历史公钥。列为决策点 D-4。

### 5.5 群组动态
- **成员变更事件上链**：RoomEvent（已闭环）也纳入同一 DAG，与消息统一排序，保证"谁在何时入群/退群"相对消息位置一致。
- **新成员历史可见性**：复用 `HistoryVisibility`，决定新成员可见的 DAG 起点。

## 6. 存储设计（SQLite）

- `messages` 表增列：`prev_hashes`（BLOB，编码的哈希列表）、`received_ts`、`seq`、`msg_hash`。
- 新增 `message_edges(child_hash, parent_hash)` 表表达 DAG 边，建索引支持高效查 head（无出边的节点）与祖先查询。
- 挂起区：`pending_messages` 表或内存结构（带 TTL）。

## 7. 性能

- **校验成本**：长历史不必每次从头验。引入**检查点/快照**：在某个全节点共识的 DAG 切点做可信快照，之后只验增量。
- **拓扑排序**：增量维护 `seq`，新消息只影响其后代。

## 8. 与现有代码的关系

- `sign.rs`：`sign_envelope` / `verify_envelope` 改为按 3.1 计算 msg_hash；新增 prev_hashes 参数。
- `server.rs:103`：验签改为**强制**——要求验签的消息未通过则拒绝存储/转发（返回错误 ACK）。
- `relay.rs:111`：Peer 转发前至少校验签名结构与大小上限（Peer 无发送方公钥时按策略缓存或拒绝，列为决策点 D-5）。
- `lattice-core`：新增 DAG 模块（head 跟踪、拓扑排序、挂起区）。

## 9. 决策点（需主控拍板）

- **D-1 数据模型**：确认采用 DAG（prev_hashes 复数）。备选：每发送者单链（更简单，但群聊并发表达力弱）。
- **D-2 缺失容忍**：严格（缺父拒绝整条）vs 宽松（容忍空洞、标记不完整）。推荐宽松。
- **D-3 全序平局打破**：纯 msg_hash 字典序（全节点一致，推荐）vs received_ts 优先（更贴到达感、但跨 Store 不统一）。
- **D-4 设备密钥历史**：注销设备后是否保留其公钥以验旧消息（推荐保留）。
- **D-5 Peer 转发校验**：Peer 无公钥时对未验签消息的策略（缓存转发 / 拒绝 / 仅结构校验）。

## 10. 分阶段实现路线（建议）

1. **P1 单条签名加强**（不依赖 DAG）：msg_hash 覆盖全字段（不含 timestamp）；`server.rs` 强制验签。立即消除"元数据可篡改 + 验签不强制"两个 Critical/High，可独立测试。
2. **P2 proto + 存储**：加 `prev_hashes` / `received_ts` / DAG 边表。
3. **P3 DAG 与全序**：head 跟踪、拓扑排序、挂起区、缺父拉取。
4. **P4 群组与多设备收敛**：RoomEvent 入 DAG、多设备 head 同步、历史可见性。
5. **P5 性能**：快照/检查点、增量 seq。

每阶段带集成测试（构造链/DAG → 篡改任一条 → 验证必须失败；并发分叉 → 全节点全序一致）。

## 11. 验证策略

- 单条篡改：改 msg_id / sender / payload / prev 任一 → 验签失败。
- 伪造历史：自签一条引用不存在父的消息 → 挂起且不可落地。
- 并发全序：两端各造分叉 → 独立计算 seq 必须一致。
- 缺失容忍：抽掉中间一条 → 后续不瘫痪、空洞被标记。
- 重放：重发旧 msg_id → 被唯一约束 + 时间窗拒绝。

## 12. Matrix / Synapse 对照与借鉴

研究自 Matrix room DAG 规范、Synapse "Room DAG concepts" 文档、及 Andy Balaam《Message order in Matrix: deliberately inconsistent》(2024-12)。Matrix 是踩过这些坑的成熟系统，以下对照验证并修正我们的设计。

### 12.1 验证了我们方向的点

- **DAG + prev_events 多父**：Matrix 事件正是通过 `prev_events` 连成 DAG，与我们的 `prev_hashes` 一致。证实"单链不够、必须 DAG"的判断正确。
- **prev_events 数量上限**：Matrix 限制单事件 ≤20 个 prev_events（v12）、auth_events 类似上限。对应我们 5.1 的"prev_hashes 数量上限"，借鉴其具体值。
- **时间戳不可信**：Matrix 的 `origin_server_ts` 明确不用于安全决策，与我们"timestamp 不进签名、仅显示"完全一致。

### 12.2 Synapse 的双序模型（修正我们的 D-3）

Synapse 不是单一全序，而是 **`(topological_ordering, stream_ordering)` 双键**：
- `topological_ordering` = depth（= max(prev 的 depth)+1），表达因果。
- `stream_ordering` = 事件**到达本服务器的顺序**（自增整数；backfill 的用负数递减）。
- **`/sync`（实时）按 stream_ordering**：看到消息按"到达顺序"，不跳过迟到的。
- **`/messages`（翻历史）按 (topological, stream)**：还原"当时大家看到的样子"。

**对 D-3 的修正**：我们原方案"纯 msg_hash 字典序"保证全节点一致，但 Matrix 经验表明：实时流更应按**到达顺序**（received_ts / 类 stream_ordering），翻历史才按拓扑。Andy Balaam 那篇的核心结论是——Matrix 当前"故意不一致"正是痛点，他建议**homeserver 给每条消息盖到达时间戳并据此排序**（spec issue #852），这恰好印证我们的 `received_ts` 设计。

**修订建议**：采用类 Synapse 双序——`depth`(拓扑) 为主序，`received_ts`/到达序为平局打破；msg_hash 字典序作为最终确定性兜底（当 received_ts 跨 Store 冲突时）。比纯 msg_hash 更贴近用户直觉，又保留确定性。

### 12.3 Outliers / Extremities（强化我们的缺失处理 5.2）

- **Outlier**：尚未关联到 DAG、不知道其 state 的"浮动"事件 → 正是我们的"挂起区"概念，Matrix 已验证此模式。
- **Forward extremity**：未被任何事件引用的最新事件（= head），发新消息时作为 prev_events。直接对应我们的 head 跟踪，命名和语义可借鉴。
- **Backward extremity**：backfill 的边界，标记"往回填到哪了"。我们处理"历史空洞/拉取缺父"时应引入类似的 backward marker。

### 12.4 State resolution（新增认知）

Matrix 有 **state resolution**：当多个分支对房间状态（成员/权限）有冲突时，用确定性算法收敛，"历史可被改写"。我们的 RoomEvent 入 DAG（5.5）会遇到同样问题——成员变更的并发冲突需要状态收敛规则。当前 Lattice 规模小可暂不实现完整 state res，但设计上要预留：成员/权限类事件的冲突收敛单列为后续课题（记为 D-6）。

### 12.5 Soft-fail（验证我们的强制验签）

Matrix 有 **soft-fail** 机制：事件签名/auth 通过但违反当前房间规则时，标记为 soft-failed——不进 forward extremities（不被后续引用）、不推给客户端，但仍存于 DAG。这比我们原计划的"直接拒绝"更细腻：**区分"密码学无效"(硬拒) 与 "逻辑上不该展示"(软失败)**。

**对 D-5 的启发**：Peer/Store 处理可疑消息时分两档——签名/hash 无效 → 硬拒绝不存储；签名有效但来源存疑/违规 → 可存但 soft-fail（不转发、不展示）。

### 12.6 据此更新的决策点

- **D-3 修订**：改采类 Synapse 双序（depth 主序 + received_ts 平局 + msg_hash 兜底），取代纯 msg_hash。
- **D-6 新增**：成员/权限类事件的并发冲突是否引入轻量 state resolution（当前可暂缓，但 proto/存储预留）。
- **D-5 细化**：采用 soft-fail 两档模型（硬拒 vs 软失败），而非单一拒绝。
