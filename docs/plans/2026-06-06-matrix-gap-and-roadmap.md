---
title: "design: Lattice vs Matrix 能力差距与路线图"
type: design
date: 2026-06-06
status: living
---

# Lattice vs Matrix 能力差距与路线图

本文以 Matrix 为成熟参照系,记录 Lattice 当前能力、与 Matrix 的差距、以及推进路线。
重点包含**防骚扰准入控制**的设计(去中心化 IM 的核心命题)。

## 1. 已具备的能力(已实现并验证)

| 能力 | 状态 | 证据 |
|------|------|------|
| 1v1/群聊消息收发、存储、同步 | ✅ | `lattice-store/src/server.rs`,集成测试 |
| 房间成员管理(建房/加入/退出/踢人 + RoomEvent) | ✅ | `lattice-core/src/room.rs` |
| 消息哈希 DAG + 强制签名验证 | ✅ | `docs/plans/2026-06-04-design-message-integrity-dag.md`(P1-P4) |
| sync 增量游标(received_ts 统一) | ✅ | `get_messages_since`/`get_room_events_since` |
| Store→Store/Peer 转发 + DHT 地址发现 fallback | ✅ | `try_forward_message`,discovery 服务 |
| 消息撤回/编辑(signed, author-only, tombstone) | ✅ | `MessageOp` + `verify_message_op` |
| 消息反应 reactions(signed emoji add/remove) | ✅ | `Reaction` + `verify_reaction` |
| 全文搜索(Tantivy,中文分词) | ✅ | `lattice-storage/src/tantivy_search.rs` |
| REST Bearer 鉴权、设备注册防冒名 | ✅ | `auth_middleware`,`verify_device_signature` |
| 多设备注册/发现 | ✅ | `/devices` 端点 |

## 2. 与 Matrix 的差距(按优先级)

### 第一梯队:IM 基础体验
- ✅ 消息撤回/编辑(已完成)
- ✅ reactions(已完成)
- ✅ 已读/送达回执(已完成:ReadReceipt 签名+存储+server,AckStatus::Read 接线)
- ✅ typing indicator(已完成:瞬态信令,转发给房间在线成员,不持久化)
- ✅ **防骚扰准入(已完成)**:`require_contact` 开关,非联系人消息拒收(见第 3 节)

### 第二梯队:加密闭环
- ⬜ E2EE 运行时未打通(Olm/Megolm 原语在,客户端↔Store 密钥协商未接)
- ⬜ 设备交叉签名 / 设备验证
- ⬜ 密钥备份 / 恢复

### 第三梯队:联邦成熟度
- ⬜ state resolution(成员/权限并发冲突收敛,设计已预留 D-6)
- ⬜ STUN/TURN NAT 穿透
- ⬜ 推送通知(push)
- ⬜ 媒体存储 / CDN

## 3. 防骚扰准入控制(核心设计)

### 3.1 问题
去中心化 IM 的难点:任何人知道你的 fingerprint + store_address 就能给你发消息。
当前强制签名验证只防"伪造"(冒充熟人),**不防"未授权陌生人骚扰"**。

### 3.2 分层防御

**第 1 层 — 准入(对方同意后才通信)**
用户的核心诉求:陌生人不能直接发消息,需对方同意。两种落地:
- **方案 A(本轮采用)**:接线 `lattice-core::exchange::ExchangePolicy`——server 收消息时调
  `should_accept(sender, signature_valid, key_verified)`,按信任等级 + 黑名单判定;非联系人默认拒。
  代码已存在(ExchangePolicy 有 Public/TOFU/Verified 三档 + block/unblock + should_accept),只差接进 `handle_frame`。
- **方案 B(后续)**:完整好友请求握手——陌生人只能发 ContactRequest(带身份卡片),
  对方同意后互加联系人才能通信。需新 proto + 状态机。

**第 2 层 — 信任分级**(ExchangePolicy 已实现)
- `Public`:接受所有(不推荐)
- `TOFU`:首次信任后校验指纹
- `Verified`:仅接受已验证指纹

**第 3 层 — 签名验证**(已完成)
确保发件人身份真实,骚扰者无法冒充熟人。

**第 4 层 — 限流**(可选,后续)
单发件人消息速率上限,防轰炸。

### 3.3 通信路径(用户提出的两种)
- **Store 中转(主路径,采用)**:消息经对方 `store_address`(代理 IP)转发。准入在对方 Store 侧执行。
  这是当前架构,准入控制接 ExchangePolicy 即可。
- **P2P 直连(后续优化)**:双方都同意后可选直连提速。依赖 STUN NAT 穿透(未实现),
  且暴露 IP 有隐私顾虑,故**暂不做**,作为"已是联系人"后的可选加速方向。

### 3.4 本轮实现范围
接线 ExchangePolicy 到 server 消息处理:非联系人/黑名单/低信任发件人的消息被拒(REJECTED),
联系人正常通行。带集成测试(陌生人被拒、联系人通过、拉黑生效)。

## 4. 推进顺序(建议)
1. 防骚扰准入(ExchangePolicy 接线)— 本轮
2. 已读回执接线 + typing(第一梯队收尾)
3. E2EE 运行时(第二梯队,周期长)
4. 联邦成熟度(第三梯队)

## 5. 事实源
- 本文为 living 文档,随实现推进更新。
- 实现细节以代码 + `.plans/lattice-dev/docs/` 控制面为准。
