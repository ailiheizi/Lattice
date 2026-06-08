---
title: "design: NextIM E2EE 运行时打通"
type: design
date: 2026-06-07
status: draft
---

# NextIM E2EE 运行时打通设计

## 0. 目的

把已有的加密原语(Olm/Megolm)接成端到端加密的**运行时闭环**:
取对方预密钥 → 建立会话 → 加密发送 → 解密接收 → 群组密钥分发与轮换。
本文是设计基线,实现以此为准。决策点需主控拍板后再编码。

## 1. 现状(已核实)

**原语层已完整**:
- `nextim-crypto::olm`:`OlmAccount`(curve25519/ed25519 公钥、生成/发布 one-time keys、fallback key、`create_outbound_session`/`create_inbound_session`)、`OlmSession`(encrypt/decrypt/session_id、pickle 持久化)。
- `nextim-crypto::megolm`:`MegolmOutboundSession`(encrypt/session_key/message_index/pickle)、`MegolmInboundSession`(new(session_key)/decrypt/pickle)、`KeyRotationPolicy`(should_rotate)。
- proto:`EncryptedPayload{ciphertext, session_id, message_index, encryption_type(OLM/MEGOLM)}` 已在;消息验签(P1)对加密 payload 同样生效(签名覆盖 payload_bytes)。
- Store `/keys/one-time`(取预密钥)、`/keys/generate`(生成);`Storage::save_key_bundle`/`get_key_bundle`;多设备 `/devices`(已落地)。

**缺的是运行时编排**:
- 没有"客户端 A 取 B 的预密钥 → 建 Olm 出站会话"的流程接线。
- 没有会话状态的客户端侧管理(哪个联系人/设备对应哪个 OlmSession)。
- 群聊:没有"分发 Megolm session_key 给每个成员设备(经 Olm 加密)"的流程。
- 密钥轮换 `KeyRotationPolicy` 未接入发送路径。
- 多设备:每设备独立 Olm 会话,密钥分发要对每个设备做。

## 2. 关键认知:Store 看不到明文

E2EE 的核心约束:**Store 只转发密文**。Store 已有的存储/转发/DAG/签名都作用在 `EncryptedPayload.ciphertext` 上,不解密。所以 E2EE 主要是**客户端侧**工程 + Store 侧少量预密钥分发支持。这意味着大部分逻辑在 `nextim-ffi`(客户端)和 `nextim-crypto`,Store 改动小。

## 3. 1v1 E2EE 流程(Olm)

### 3.1 会话建立
1. A 要给 B 发加密消息,本地无 B 的 Olm 会话。
2. A 向 B 的 Store 请求 B 的预密钥包:`GET /keys/one-time`(返回 B 的 curve25519 identity key + 一个 one-time key)。
3. A `create_outbound_session(b_identity_key, b_one_time_key)` → OlmSession。
4. A 用该会话 encrypt 明文 → `EncryptedPayload{OLM, ciphertext, session_id}`,封进 Envelope(照常签名),发送。
5. B 收到首条 Olm 消息(pre-key message),`create_inbound_session` 建立入站会话并 decrypt。
6. 之后双向用已建立的 OlmSession(Double Ratchet,前向保密)。

### 3.2 会话持久化
- 客户端把 OlmSession pickle 后存本地(每联系人设备一个会话)。
- 重启后 from_pickle 恢复,不必重建。

## 4. 群聊 E2EE 流程(Megolm)

### 4.1 发送方
1. A 在房间首次发消息:`MegolmOutboundSession::new()`,得到 session_key。
2. A 把 session_key **经 Olm 1v1 加密**分发给房间每个成员的每个设备(用 3.1 的 Olm 会话)。这是一条特殊的 KEY_DISTRIBUTION 消息。
3. A 用 Megolm 出站会话 encrypt 房间消息 → `EncryptedPayload{MEGOLM, ciphertext, session_id, message_index}`。
4. 后续消息复用该 Megolm 会话,直到轮换。

### 4.2 接收方
1. 成员设备先收到经 Olm 加密的 session_key(KEY_DISTRIBUTION)→ Olm 解密 → `MegolmInboundSession::new(session_key)`。
2. 收到 Megolm 房间消息 → 用对应 session_id 的入站会话 decrypt。
3. message_index 防重放(入站会话拒绝已见 index)。

### 4.3 轮换(KeyRotationPolicy)
- 成员变更(踢人/退群)或 message_index/时间超阈值时,A 轮换 Megolm 会话(新 session_key 重新分发)。踢人后轮换确保被踢者无法解密后续。

## 5. 多设备交互
- 每个用户多设备,每设备独立 Olm 身份。密钥分发要对"用户的所有设备"做(用 `/devices/:user` 拿设备列表 → 对每个设备的 identity key 建 Olm 会话分发)。
- 新设备加入:需补发当前 Megolm 会话(或从该设备加入点之后才可解密,取决于历史可见性策略)。

## 6. 与现有系统的交互
- **签名(P1)**:加密消息的 Envelope 仍签名(覆盖 ciphertext),防伪造。E2EE 与签名正交。
- **DAG**:密文消息照常进 DAG(msg_hash 覆盖 ciphertext)。
- **DHT/转发**:Store 转发密文,不受影响。
- **准入/限流**:作用在 Envelope 层,与加密正交。

## 7. 需新增的 proto / 端点(预估)
- proto:`KeyDistribution{ room_id, session_id, encrypted_session_key(Olm 密文), sender_device }` + frame type;或复用 EncryptedPayload 加一个 content 类型。
- Store:`GET /keys/claim/:fingerprint` 取并消费一个 one-time key(当前 /keys/one-time 是自己的,需要"取他人的")。**这是 Store 侧主要缺口**:claim 别人的预密钥。
- 客户端(ffi):会话管理器(联系人设备→OlmSession 映射 + Megolm 出/入站会话表 + pickle 持久化)。

## 8. 决策点(需主控拍板)

> **已敲定(2026-06-07)**:
> - D-1 → 核心编排放 nextim-crypto/core(纯逻辑可单测,ffi/store 只调用)。
> - D-2 → Store 加 `GET /keys/claim/:fingerprint`,取并消费一个 OTK(防重用),耗尽回退 fallback key。
> - D-3 → 新建 proto `KeyDistribution` 消息 + 专用 frame type 分发群组密钥。
> - D-4 → 新设备加入后才可解密群消息(不补发旧 Megolm session,最简单且隐私最好)。
> - D-5 → MVP 用 TOFU(首次信任 identity key);交叉签名/设备验证留后续。


- **D-1 实现边界**:E2EE 主要在客户端(ffi)。当前 ffi 是最小绑定。是先在 **nextim-crypto + 一个新 `session` 编排模块**做核心逻辑(可单测,不依赖真实客户端),还是直接在 ffi?推荐前者:核心编排在 crypto/core,ffi/store 只调用。
- **D-2 预密钥 claim**:Store 加 `claim_one_time_key(fingerprint)` 端点(取并删一个 OTK,防重用)。预密钥耗尽时回退 fallback key。确认这个端点设计。
- **D-3 群组密钥分发载体**:新 proto 消息 vs 复用 EncryptedPayload。
- **D-4 多设备历史可见性**:新设备能否解密加入前的群消息(需补发旧 Megolm session)?默认"加入后才可见"最简单。
- **D-5 信任/验证**:首次会话是否要求设备已验证(交叉签名)?MVP 可 TOFU(首次信任 identity key)。

## 9. 分阶段路线(建议)
- **E1**:Store 预密钥 claim 端点(claim_one_time_key,取并消费)+ 测试。✅ 已落地(`/keys/bundle` + `/keys/claim`,`key_bundle_upload_then_claim_consumes_otk_and_falls_back`)。
- **E2**:nextim-crypto/core 会话编排模块(1v1 Olm:建会话/加密/解密/持久化),纯逻辑单测。✅ 已落地(`nextim_crypto::session::OlmSessionManager`,5 单测覆盖往返/无会话/类型校验/pickle 恢复/非法 key)。
- **E3**:1v1 端到端(两个身份,A 加密发 B 解密,经 Store 转发密文)集成测试。✅ 已落地(`e2ee_1v1_roundtrip_through_real_store`:真实 Olm 密文经真实 Store WS 存储/sync,Bob 解密还原明文,断言 Store 只见密文)。
- **E4**:群组 Megolm(session_key 经 Olm 分发 + Megolm 收发)。
- **E5**:轮换(成员变更触发)+ 多设备分发。

每阶段带测试,CI 绿。E2/E3 是 1v1 闭环(最小可用 E2EE),E4/E5 是群组扩展。

## 10. 不做(本设计范围外)
- 密钥备份/恢复(单独课题)。
- 交叉签名/设备验证 UI(MVP 用 TOFU)。
- 完整 Matrix 兼容的 key-sharing 协议。
