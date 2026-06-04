use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

use nextim_core::dag::{self, DagNode};
use nextim_core::traits::search::SearchIndex;
use nextim_core::traits::storage::{PendingMessage, Storage};
use nextim_crypto::sign;
use nextim_proto::message::Message;
use nextim_proto::transport::{frame, AckStatus, Frame, FrameType, MessageAck, Pong};

use crate::AppState;

const FORWARD_ACK_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn run_ws_server(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("WebSocket server listening on {addr}");

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            tracing::info!("New WS connection from {peer_addr}");
            match accept_async(tcp_stream).await {
                Ok(ws_stream) => {
                    if let Err(e) = handle_connection(ws_stream, state).await {
                        tracing::warn!("Connection {peer_addr} error: {e}");
                    }
                    tracing::info!("Connection {peer_addr} closed");
                }
                Err(e) => tracing::warn!("WS handshake failed for {peer_addr}: {e}"),
            }
        });
    }
}

async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let (sink, mut stream) = ws_stream.split();
    let sink = Arc::new(Mutex::new(sink));
    let mut registered_fingerprint: Option<String> = None;

    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg {
            WsMessage::Binary(data) => {
                let frame = Frame::decode(data.as_ref())
                    .map_err(|e| anyhow::anyhow!("decode frame: {e}"))?;

                // 从第一条消息中提取发送方指纹，注册在线连接
                if registered_fingerprint.is_none() {
                    if let Some(frame::Body::Message(ref env)) = frame.body {
                        let fp = env.sender_fingerprint.clone();
                        if !fp.is_empty() {
                            state.online.write().await.insert(fp.clone(), sink.clone());
                            registered_fingerprint = Some(fp.clone());
                            tracing::info!("Registered online: {fp}");
                        }
                    }
                }

                let response = handle_frame(frame, &state, &sink).await?;
                if let Some(resp_frame) = response {
                    let encoded = resp_frame.encode_to_vec();
                    sink.lock().await.send(WsMessage::Binary(encoded)).await?;
                }
            }
            WsMessage::Ping(payload) => {
                sink.lock().await.send(WsMessage::Pong(payload)).await?;
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    // 连接断开，移除在线状态
    if let Some(fp) = &registered_fingerprint {
        state.online.write().await.remove(fp);
        tracing::info!("Unregistered online: {fp}");
    }

    Ok(())
}

async fn handle_frame(
    frame: Frame,
    state: &Arc<AppState>,
    _sender_sink: &Arc<Mutex<crate::WsSink>>,
) -> anyhow::Result<Option<Frame>> {
    match frame.body {
        Some(frame::Body::Message(ref envelope)) => {
            if !envelope.signature.is_empty() {
                let sender_key = state
                    .storage
                    .get_contact(&envelope.sender_fingerprint)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|c| c.identity)
                    .map(|i| i.ed25519_public_key);

                let verification_error = match sender_key {
                    Some(key) => match sign::verify_envelope(&key, envelope) {
                        Ok(true) => None,
                        Ok(false) => Some("signature verification returned false".to_string()),
                        Err(error) => Some(error.to_string()),
                    },
                    None => Some(format!(
                        "missing sender public key for {}",
                        envelope.sender_fingerprint
                    )),
                };

                if let Some(error) = verification_error {
                    tracing::warn!(
                        "Rejecting signed message {} from {}: {}",
                        envelope.msg_id,
                        envelope.sender_fingerprint,
                        error
                    );
                    return Ok(Some(rejected_ack(frame.seq, &envelope.msg_id)));
                }

                tracing::debug!("Signature verified for {}", envelope.msg_id);
            } else if !state.allow_unsigned {
                // 强制验签：无签名消息默认拒绝（除非显式配置 allow_unsigned）。
                tracing::warn!(
                    "Rejecting unsigned message {} from {} (allow_unsigned=false)",
                    envelope.msg_id,
                    envelope.sender_fingerprint
                );
                return Ok(Some(rejected_ack(frame.seq, &envelope.msg_id)));
            } else {
                tracing::debug!(
                    "Accepting unsigned message {} (allow_unsigned=true)",
                    envelope.msg_id
                );
            }

            let verified = !envelope.signature.is_empty();
            let received_ts = now_received_ts();
            let msg_hash = sign::compute_msg_hash(envelope).map_err(|e| {
                anyhow::anyhow!("compute message hash for {}: {e}", envelope.msg_id)
            })?;

            persist_incoming_message(state, envelope, received_ts, verified, msg_hash.clone())
                .await?;

            let recipient = &envelope.recipient_fingerprint;
            let frame_data = frame.encode_to_vec();
            {
                let online = state.online.read().await;
                if let Some(recipient_sink) = online.get(recipient) {
                    let mut sink = recipient_sink.lock().await;
                    if sink
                        .send(WsMessage::Binary(frame_data.clone()))
                        .await
                        .is_ok()
                    {
                        tracing::info!("Pushed message to online recipient {recipient}");
                    }
                }
            }

            let recipient_clone = recipient.to_owned();
            let state = Arc::clone(state);
            tokio::spawn(async move {
                if let Err(error) = try_forward_message(state, recipient_clone, frame_data).await {
                    tracing::warn!("Failed to forward to recipient store: {error}");
                }
            });

            Ok(Some(Frame {
                seq: frame.seq,
                r#type: FrameType::Ack as i32,
                body: Some(frame::Body::Ack(MessageAck {
                    msg_id: envelope.msg_id.clone(),
                    status: AckStatus::Received as i32,
                })),
            }))
        }

        Some(frame::Body::RoomEvent(ref event)) => {
            let mut event = event.clone();
            // 始终用服务端重算的 hash，不信任客户端来包的 msg_hash。
            let computed_hash = nextim_crypto::sign::compute_room_event_hash(&event)
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            if !event.signature.is_empty() {
                // 取 actor 主公钥验签（含 hash 比对）。
                let actor_key = state
                    .storage
                    .get_contact(&event.actor_fingerprint)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|c| c.identity)
                    .map(|i| i.ed25519_public_key);

                let verify_error = match actor_key {
                    Some(key) => match nextim_crypto::sign::verify_room_event(&key, &event) {
                        Ok(true) => None,
                        Ok(false) => Some("room event signature returned false".to_string()),
                        Err(error) => Some(error.to_string()),
                    },
                    None => Some(format!(
                        "missing actor public key for {}",
                        event.actor_fingerprint
                    )),
                };

                if let Some(error) = verify_error {
                    tracing::warn!(
                        "Rejecting room event in {} by {}: {}",
                        event.room_id,
                        event.actor_fingerprint,
                        error
                    );
                    return Ok(Some(rejected_ack(frame.seq, &event.timestamp.to_string())));
                }
            } else if !state.allow_unsigned {
                tracing::warn!(
                    "Rejecting unsigned room event in {} by {} (allow_unsigned=false)",
                    event.room_id,
                    event.actor_fingerprint
                );
                return Ok(Some(rejected_ack(frame.seq, &event.timestamp.to_string())));
            }

            // 落库前用重算 hash 覆盖（防止接受 allow_unsigned 时来包带伪造 hash）。
            event.msg_hash = computed_hash;

            state
                .storage
                .save_room_event(&event)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            tracing::info!(
                "Stored room event for room {} by {}",
                event.room_id,
                event.actor_fingerprint
            );

            Ok(Some(Frame {
                seq: frame.seq,
                r#type: FrameType::Ack as i32,
                body: Some(frame::Body::Ack(MessageAck {
                    msg_id: event.timestamp.to_string(),
                    status: AckStatus::Received as i32,
                })),
            }))
        }

        Some(frame::Body::Ping(ping)) => Ok(Some(Frame {
            seq: frame.seq,
            r#type: FrameType::Pong as i32,
            body: Some(frame::Body::Pong(Pong {
                timestamp: ping.timestamp,
            })),
        })),

        Some(frame::Body::SyncRequest(req)) => {
            let room_ids = if req.room_ids.is_empty() {
                let rooms = state
                    .storage
                    .get_rooms()
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                rooms.into_iter().map(|r| r.room_id).collect()
            } else {
                req.room_ids
            };

            let mut envelopes = Vec::new();
            let mut ordered_messages_by_hash = BTreeMap::new();
            let mut events = Vec::new();
            for room_id in &room_ids {
                let msgs = state
                    .storage
                    .get_messages_since(room_id, req.since_timestamp)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                for msg in order_messages_for_sync(msgs) {
                    ordered_messages_by_hash.insert(msg.msg_hash.clone(), msg.clone());
                    let payload = if let Some(encrypted_payload) = msg.encrypted_payload {
                        Some(nextim_proto::message::envelope::Payload::Encrypted(
                            encrypted_payload,
                        ))
                    } else {
                        msg.content.map(|c| {
                            nextim_proto::message::envelope::Payload::Plain(
                                nextim_proto::message::PlainPayload { content: Some(c) },
                            )
                        })
                    };
                    envelopes.push(nextim_proto::message::Envelope {
                        msg_id: msg.msg_id,
                        sender_fingerprint: msg.sender_fingerprint,
                        recipient_fingerprint: msg.room_id,
                        timestamp: msg.timestamp,
                        signature: vec![],
                        payload_hash: msg.msg_hash,
                        prev_hashes: msg.prev_hashes,
                        payload,
                    });
                }

                let room_events = state
                    .storage
                    .get_room_events_since(room_id, req.since_timestamp)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                events.extend(room_events);
            }

            // 统一时间线：把消息与房间事件放进同一个 DAG 全序，
            // 使「谁在何时入群/退群」相对消息的位置在所有节点一致。
            let mut timeline_nodes: Vec<nextim_core::dag::DagNode> = Vec::new();
            for env in &envelopes {
                timeline_nodes.push(nextim_core::dag::DagNode {
                    msg_hash: env.payload_hash.clone(),
                    prev_hashes: env.prev_hashes.clone(),
                    received_ts: ordered_messages_by_hash
                        .get(env.payload_hash.as_slice())
                        .map(|m| m.received_ts)
                        .unwrap_or(env.timestamp),
                });
            }
            for record in &events {
                timeline_nodes.push(nextim_core::dag::DagNode {
                    msg_hash: record.event.msg_hash.clone(),
                    prev_hashes: record.event.prev_hashes.clone(),
                    received_ts: record.received_ts,
                });
            }
            let ordered = nextim_core::dag::deterministic_order(&timeline_nodes);
            let envelopes_by_hash: BTreeMap<Vec<u8>, &nextim_proto::message::Envelope> = envelopes
                .iter()
                .map(|e| (e.payload_hash.clone(), e))
                .collect();
            let events_by_hash: BTreeMap<Vec<u8>, &nextim_proto::group::RoomEvent> = events
                .iter()
                .map(|r| (r.event.msg_hash.clone(), &r.event))
                .collect();
            let mut timeline = Vec::with_capacity(ordered.len());
            for node in &ordered {
                let item = if let Some(env) = envelopes_by_hash.get(&node.msg_hash) {
                    Some(nextim_proto::transport::sync_timeline_item::Item::Message(
                        (*env).clone(),
                    ))
                } else {
                    events_by_hash.get(&node.msg_hash).map(|event| {
                        nextim_proto::transport::sync_timeline_item::Item::RoomEvent(
                            (*event).clone(),
                        )
                    })
                };
                if item.is_some() {
                    timeline.push(nextim_proto::transport::SyncTimelineItem {
                        msg_hash: node.msg_hash.clone(),
                        item,
                    });
                }
            }

            let next_batch = envelopes
                .iter()
                .map(|e| e.payload_hash.as_slice())
                .filter_map(|hash| {
                    ordered_messages_by_hash
                        .get(hash)
                        .map(|message| message.received_ts)
                })
                .chain(events.iter().map(|r| r.received_ts))
                .max()
                .map(|timestamp| timestamp + 1)
                .unwrap_or(req.since_timestamp);

            Ok(Some(Frame {
                seq: frame.seq,
                r#type: FrameType::SyncResponse as i32,
                body: Some(frame::Body::SyncResponse(
                    nextim_proto::transport::SyncResponse {
                        messages: envelopes,
                        events: events.into_iter().map(|r| r.event).collect(),
                        next_batch,
                        timeline,
                    },
                )),
            }))
        }

        _ => {
            tracing::debug!("Unhandled frame type: {}", frame.r#type);
            Ok(None)
        }
    }
}

fn rejected_ack(seq: u64, msg_id: &str) -> Frame {
    Frame {
        seq,
        r#type: FrameType::Ack as i32,
        body: Some(frame::Body::Ack(MessageAck {
            msg_id: msg_id.to_string(),
            status: AckStatus::Rejected as i32,
        })),
    }
}

fn now_received_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn envelope_to_message(
    envelope: &nextim_proto::message::Envelope,
    received_ts: u64,
    verified: bool,
    msg_hash: Vec<u8>,
) -> Message {
    Message {
        msg_id: envelope.msg_id.clone(),
        room_id: envelope.recipient_fingerprint.clone(),
        sender_fingerprint: envelope.sender_fingerprint.clone(),
        timestamp: envelope.timestamp,
        content: match &envelope.payload {
            Some(nextim_proto::message::envelope::Payload::Plain(p)) => p.content.clone(),
            _ => None,
        },
        encrypted: matches!(
            envelope.payload,
            Some(nextim_proto::message::envelope::Payload::Encrypted(_))
        ),
        verified,
        encrypted_payload: match &envelope.payload {
            Some(nextim_proto::message::envelope::Payload::Encrypted(payload)) => {
                Some(payload.clone())
            }
            _ => None,
        },
        received_ts,
        prev_hashes: envelope.prev_hashes.clone(),
        msg_hash,
    }
}

async fn get_all_messages_for_room(
    state: &Arc<AppState>,
    room_id: &str,
) -> anyhow::Result<Vec<Message>> {
    use nextim_core::traits::storage::{Pagination, TimeRange};

    state
        .storage
        .get_messages(
            room_id,
            &TimeRange {
                start: 0,
                end: i64::MAX as u64,
            },
            &Pagination {
                offset: 0,
                limit: u32::MAX,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

async fn known_hashes_for_room(
    state: &Arc<AppState>,
    room_id: &str,
) -> anyhow::Result<BTreeSet<Vec<u8>>> {
    let messages = get_all_messages_for_room(state, room_id).await?;
    Ok(messages
        .into_iter()
        .map(|message| message.msg_hash)
        .collect())
}

fn collect_missing_parents(message: &Message, known_hashes: &BTreeSet<Vec<u8>>) -> Vec<Vec<u8>> {
    dag::missing_parents(
        &DagNode {
            msg_hash: message.msg_hash.clone(),
            prev_hashes: message.prev_hashes.clone(),
            received_ts: message.received_ts,
        },
        known_hashes,
    )
}

async fn store_finalized_message(state: &Arc<AppState>, msg: &Message) -> anyhow::Result<()> {
    state
        .storage
        .save_message(msg)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    for parent_hash in &msg.prev_hashes {
        state
            .storage
            .save_message_edge(&msg.msg_hash, parent_hash)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    let _ = state.search.index_message(msg).await;
    Ok(())
}

async fn promote_pending_messages(state: &Arc<AppState>, room_id: &str) -> anyhow::Result<()> {
    loop {
        let pending = state
            .storage
            .list_pending_messages()
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut promoted_any = false;

        for pending_msg in pending {
            let envelope = nextim_proto::message::Envelope::decode(pending_msg.data.as_slice())
                .map_err(|e| anyhow::anyhow!("decode pending envelope: {e}"))?;
            if envelope.recipient_fingerprint != room_id {
                continue;
            }

            let message = envelope_to_message(
                &envelope,
                pending_msg.received_ts,
                true,
                pending_msg.msg_hash.clone(),
            );
            let known_hashes = known_hashes_for_room(state, room_id).await?;
            if !collect_missing_parents(&message, &known_hashes).is_empty() {
                continue;
            }

            store_finalized_message(state, &message).await?;
            state
                .storage
                .delete_pending_message(&pending_msg.msg_hash)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            tracing::info!("Promoted pending message {} into room DAG", message.msg_id);
            promoted_any = true;
        }

        if !promoted_any {
            break;
        }
    }

    Ok(())
}

async fn persist_incoming_message(
    state: &Arc<AppState>,
    envelope: &nextim_proto::message::Envelope,
    received_ts: u64,
    verified: bool,
    msg_hash: Vec<u8>,
) -> anyhow::Result<()> {
    let message = envelope_to_message(envelope, received_ts, verified, msg_hash.clone());
    let known_hashes = known_hashes_for_room(state, &message.room_id).await?;
    let missing_parents = collect_missing_parents(&message, &known_hashes);

    if !missing_parents.is_empty() {
        state
            .storage
            .save_pending_message(&PendingMessage {
                msg_hash: msg_hash.clone(),
                data: envelope.encode_to_vec(),
                received_ts,
            })
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::info!(
            "Stored pending message {} with {} missing parent(s)",
            message.msg_id,
            missing_parents.len()
        );
        tracing::debug!(
            "TODO(P4): fetch missing parent hashes for {}: {:?}",
            message.msg_id,
            missing_parents
        );
        return Ok(());
    }

    store_finalized_message(state, &message).await?;
    promote_pending_messages(state, &message.room_id).await?;
    tracing::info!(
        "Stored message {} from {}",
        message.msg_id,
        message.sender_fingerprint
    );
    Ok(())
}

fn order_messages_for_sync(messages: Vec<Message>) -> Vec<Message> {
    let nodes: Vec<DagNode> = messages
        .iter()
        .map(|message| DagNode {
            msg_hash: message.msg_hash.clone(),
            prev_hashes: message.prev_hashes.clone(),
            received_ts: message.received_ts,
        })
        .collect();
    let ordered = dag::deterministic_order(&nodes);
    let mut by_hash = BTreeMap::new();
    for message in messages {
        by_hash.insert(message.msg_hash.clone(), message);
    }

    ordered
        .into_iter()
        .filter_map(|node| by_hash.remove(&node.msg_hash))
        .collect()
}

/// 尝试转发消息给接收方 Store
async fn try_forward_message(
    state: Arc<AppState>,
    recipient_fingerprint: String,
    frame_data: Vec<u8>,
) -> anyhow::Result<()> {
    let contact = state
        .storage
        .get_contact(&recipient_fingerprint)
        .await
        .map_err(|e| anyhow::anyhow!("lookup recipient contact {recipient_fingerprint}: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("recipient contact not found: {recipient_fingerprint}"))?;

    let addresses = forwarding_addresses(&contact);
    if addresses.is_empty() {
        return Err(anyhow::anyhow!(
            "recipient {recipient_fingerprint} has no store or proxy address"
        ));
    }

    let mut last_error: Option<anyhow::Error> = None;

    for addr in addresses {
        match forward_to_store(&addr, &frame_data).await {
            Ok(()) => {
                tracing::info!(
                    "Forwarded message for recipient {} via {}",
                    recipient_fingerprint,
                    addr
                );
                return Ok(());
            }
            Err(error) => {
                tracing::warn!(
                    "Forward attempt to {} for recipient {} failed: {}",
                    addr,
                    recipient_fingerprint,
                    error
                );
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("forwarding failed for recipient {recipient_fingerprint}")
    }))
}

fn forwarding_addresses(contact: &nextim_proto::identity::Contact) -> Vec<String> {
    let mut addresses = Vec::new();

    let primary = contact.store_address.trim();
    if !primary.is_empty() {
        addresses.push(primary.to_string());
    }

    let proxy = contact.proxy_store_address.trim();
    if !proxy.is_empty() && proxy != primary {
        addresses.push(proxy.to_string());
    }

    addresses
}

/// 通过 WebSocket 连接转发消息到目标 Store
async fn forward_to_store(addr: &str, frame_data: &[u8]) -> anyhow::Result<()> {
    let mut ws = connect_outbound(addr).await?;

    send_frame_and_wait_for_ack(&mut ws, addr, frame_data).await?;

    ws.close(None).await.ok();
    Ok(())
}

async fn connect_outbound(
    addr: &str,
) -> anyhow::Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let (ws, _) = connect_async(addr)
        .await
        .map_err(|e| anyhow::anyhow!("connect to {addr}: {e}"))?;

    Ok(ws)
}

async fn send_frame_and_wait_for_ack(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    addr: &str,
    frame_data: &[u8],
) -> anyhow::Result<()> {
    ws.send(WsMessage::Binary(frame_data.to_vec()))
        .await
        .map_err(|e| anyhow::anyhow!("send to {addr}: {e}"))?;

    let ack_result = timeout(FORWARD_ACK_TIMEOUT, async {
        while let Some(message) = ws.next().await {
            match message {
                Ok(WsMessage::Binary(data)) => {
                    let ack = Frame::decode(data.as_ref())
                        .map_err(|e| anyhow::anyhow!("decode ack from {addr}: {e}"))?;

                    if ack.r#type == FrameType::Ack as i32 {
                        tracing::debug!("Received ACK from {addr}");
                        return Ok(());
                    }
                }
                Ok(WsMessage::Ping(payload)) => {
                    ws.send(WsMessage::Pong(payload))
                        .await
                        .map_err(|e| anyhow::anyhow!("respond ping from {addr}: {e}"))?;
                }
                Ok(WsMessage::Close(_)) => break,
                Ok(_) => {}
                Err(e) => return Err(anyhow::anyhow!("read ack from {addr}: {e}")),
            }
        }

        Err(anyhow::anyhow!("store {addr} closed before ACK"))
    })
    .await;

    match ack_result {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!(
            "timed out waiting for ACK from {addr} after {:?}",
            FORWARD_ACK_TIMEOUT
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use nextim_crypto::{identity::MasterKeyPair, olm::OlmAccount};
    use nextim_proto::{
        group::{RoomEvent, RoomEventType},
        identity::{Contact, Identity},
        message::{envelope::Payload, Envelope, MessageContent, MessageType, PlainPayload},
    };
    use nextim_storage::{sqlite::SqliteStorage, tantivy_search::TantivySearch};
    use tokio::sync::{oneshot, RwLock};
    use tokio_tungstenite::{accept_async, connect_async};

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            storage: SqliteStorage::in_memory().expect("in-memory storage"),
            search: TantivySearch::in_memory().expect("in-memory search"),
            online: Arc::new(RwLock::new(HashMap::new())),
            outbound: Arc::new(RwLock::new(HashMap::new())),
            identity: MasterKeyPair::generate(),
            olm_account: Mutex::new(OlmAccount::new()),
            fingerprint: "store-fp".to_string(),
            display_name: "Store".to_string(),
            ws_addr: "127.0.0.1:0".to_string(),
            api_token: String::new(),
            allow_unsigned: true,
        })
    }

    fn test_frame(recipient: &str) -> Frame {
        Frame {
            seq: 42,
            r#type: FrameType::Message as i32,
            body: Some(frame::Body::Message(Envelope {
                msg_id: "msg-1".to_string(),
                sender_fingerprint: "sender-fp".to_string(),
                recipient_fingerprint: recipient.to_string(),
                timestamp: 123,
                signature: vec![],
                payload_hash: vec![],
                prev_hashes: Vec::new(),
                payload: Some(Payload::Plain(PlainPayload {
                    content: Some(MessageContent {
                        r#type: MessageType::Text as i32,
                        text: "hello".to_string(),
                        ..Default::default()
                    }),
                })),
            })),
        }
    }

    fn test_room_event_frame() -> Frame {
        Frame {
            seq: 99,
            r#type: FrameType::RoomEvent as i32,
            body: Some(frame::Body::RoomEvent(RoomEvent {
                room_id: "room-1".to_string(),
                actor_fingerprint: "owner-fp".to_string(),
                r#type: RoomEventType::MemberJoin as i32,
                target_fingerprint: "alice-fp".to_string(),
                timestamp: 456,
                signature: vec![],
                prev_hashes: Vec::new(),
                msg_hash: Vec::new(),
            })),
        }
    }

    fn test_contact(fingerprint: &str, store_address: &str, proxy_store_address: &str) -> Contact {
        Contact {
            identity: Some(Identity {
                fingerprint: fingerprint.to_string(),
                display_name: "Recipient".to_string(),
                ..Default::default()
            }),
            store_address: store_address.to_string(),
            proxy_store_address: proxy_store_address.to_string(),
            ..Default::default()
        }
    }

    async fn spawn_ack_store() -> (String, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test ws server");
        let addr = format!("ws://{}", listener.local_addr().expect("local addr"));
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept tcp connection");
            let mut ws = accept_async(stream)
                .await
                .expect("accept websocket handshake");

            if let Some(Ok(WsMessage::Binary(data))) = ws.next().await {
                tx.send(data).expect("send frame to test");
                let ack = Frame {
                    seq: 42,
                    r#type: FrameType::Ack as i32,
                    body: Some(frame::Body::Ack(MessageAck {
                        msg_id: "msg-1".to_string(),
                        status: AckStatus::Received as i32,
                    })),
                };

                ws.send(WsMessage::Binary(ack.encode_to_vec()))
                    .await
                    .expect("send ack response");
            }
        });

        (addr, rx)
    }

    async fn spawn_non_ack_store() -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test ws server");
        let addr = format!("ws://{}", listener.local_addr().expect("local addr"));

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept tcp connection");
            let mut ws = accept_async(stream)
                .await
                .expect("accept websocket handshake");

            if let Some(Ok(WsMessage::Binary(_))) = ws.next().await {
                tokio::time::sleep(FORWARD_ACK_TIMEOUT + Duration::from_millis(200)).await;
            }
        });

        addr
    }

    async fn test_sender_sink() -> Arc<Mutex<crate::WsSink>> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind sender sink listener");
        let addr = listener.local_addr().expect("sender sink local addr");

        let accept = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept sender sink tcp");
            let ws = accept_async(stream)
                .await
                .expect("accept sender sink websocket");
            let (sink, _stream) = ws.split();
            sink
        });

        let client = tokio::spawn(async move {
            let url = format!("ws://{addr}");
            let (_ws, _) = connect_async(&url)
                .await
                .expect("connect sender sink websocket");
            futures_util::future::pending::<()>().await;
        });

        let sink = accept.await.expect("join sender sink accept task");
        client.abort();
        Arc::new(Mutex::new(sink))
    }

    #[test]
    fn forwarding_addresses_prefer_store_then_proxy_without_duplicates() {
        let contact = test_contact("recipient-fp", " ws://primary ", "ws://proxy");

        let addresses = forwarding_addresses(&contact);

        assert_eq!(
            addresses,
            vec!["ws://primary".to_string(), "ws://proxy".to_string()]
        );
    }

    #[tokio::test]
    async fn try_forward_message_uses_proxy_when_primary_fails() {
        let state = test_state();
        let recipient = "recipient-fp";
        let (proxy_addr, received) = spawn_ack_store().await;
        let frame = test_frame(recipient);

        state
            .storage
            .save_contact(&test_contact(recipient, "ws://127.0.0.1:1", &proxy_addr))
            .await
            .expect("save contact");

        try_forward_message(state, recipient.to_string(), frame.encode_to_vec())
            .await
            .expect("forward via proxy address");

        let forwarded = tokio::time::timeout(Duration::from_secs(2), received)
            .await
            .expect("forward timeout")
            .expect("proxy store received frame");

        assert_eq!(forwarded, frame.encode_to_vec());
    }

    #[tokio::test]
    async fn try_forward_message_times_out_when_store_never_acknowledges() {
        let state = test_state();
        let recipient = "recipient-fp";
        let store_addr = spawn_non_ack_store().await;
        let frame = test_frame(recipient);

        state
            .storage
            .save_contact(&test_contact(recipient, &store_addr, ""))
            .await
            .expect("save contact");

        let error = try_forward_message(state, recipient.to_string(), frame.encode_to_vec())
            .await
            .expect_err("forward should time out without ACK");

        assert!(error.to_string().contains("timed out waiting for ACK"));
    }

    #[tokio::test]
    async fn handle_frame_room_event_returns_ack_and_persists_event() {
        let state = test_state();
        let sink = test_sender_sink().await;

        let response = handle_frame(test_room_event_frame(), &state, &sink)
            .await
            .expect("room event handling should not error")
            .expect("room event should return ack");

        assert_eq!(response.seq, 99);
        assert_eq!(response.r#type, FrameType::Ack as i32);
        match response.body {
            Some(frame::Body::Ack(ack)) => {
                assert_eq!(ack.msg_id, "456");
                assert_eq!(ack.status, AckStatus::Received as i32);
            }
            other => panic!("expected ack response, got {other:?}"),
        }

        let events = state
            .storage
            .get_room_events("room-1", 0)
            .await
            .expect("room events query");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].room_id, "room-1");
        assert_eq!(events[0].actor_fingerprint, "owner-fp");
        assert_eq!(events[0].target_fingerprint, "alice-fp");
        assert_eq!(events[0].timestamp, 456);
    }

    #[tokio::test]
    async fn handle_frame_message_stores_received_ts_and_msg_hash() {
        let state = test_state();
        let sink = test_sender_sink().await;
        let frame = test_frame("recipient-fp");
        let expected_hash = if let Some(frame::Body::Message(envelope)) = frame.body.as_ref() {
            sign::compute_msg_hash(envelope).expect("compute expected message hash")
        } else {
            panic!("expected message frame");
        };

        let response = handle_frame(frame, &state, &sink)
            .await
            .expect("message handling should not error")
            .expect("message should return ack");
        assert_eq!(response.r#type, FrameType::Ack as i32);

        let stored = state
            .storage
            .get_message("msg-1")
            .await
            .expect("query stored message")
            .expect("message stored");
        assert!(stored.received_ts > 0);
        assert_eq!(stored.msg_hash, expected_hash);
        assert!(stored.prev_hashes.is_empty());
    }

    #[tokio::test]
    async fn handle_frame_message_with_missing_parent_goes_pending_until_parent_arrives() {
        let state = test_state();
        let sink = test_sender_sink().await;

        let parent_envelope = Envelope {
            msg_id: "parent-msg".to_string(),
            sender_fingerprint: "sender-fp".to_string(),
            recipient_fingerprint: "room-pending".to_string(),
            timestamp: 100,
            signature: vec![],
            payload_hash: vec![0xAA, 0xBB],
            prev_hashes: Vec::new(),
            payload: Some(Payload::Plain(PlainPayload {
                content: Some(MessageContent {
                    r#type: MessageType::Text as i32,
                    text: "parent".to_string(),
                    ..Default::default()
                }),
            })),
        };
        let parent_hash = sign::compute_msg_hash(&parent_envelope).expect("parent hash");

        let child_envelope = Envelope {
            msg_id: "child-msg".to_string(),
            sender_fingerprint: "sender-fp".to_string(),
            recipient_fingerprint: "room-pending".to_string(),
            timestamp: 200,
            signature: vec![],
            payload_hash: vec![0xCC, 0xDD],
            prev_hashes: vec![parent_hash.clone()],
            payload: Some(Payload::Plain(PlainPayload {
                content: Some(MessageContent {
                    r#type: MessageType::Text as i32,
                    text: "child".to_string(),
                    ..Default::default()
                }),
            })),
        };
        let child_hash = sign::compute_msg_hash(&child_envelope).expect("child hash");
        let child_frame = Frame {
            seq: 7,
            r#type: FrameType::Message as i32,
            body: Some(frame::Body::Message(child_envelope.clone())),
        };

        handle_frame(child_frame, &state, &sink)
            .await
            .expect("child message should be accepted")
            .expect("child message should return ack");

        assert!(state
            .storage
            .get_message("child-msg")
            .await
            .unwrap()
            .is_none());
        assert!(state
            .storage
            .get_pending_message(&child_hash)
            .await
            .unwrap()
            .is_some());

        let parent_frame = Frame {
            seq: 8,
            r#type: FrameType::Message as i32,
            body: Some(frame::Body::Message(parent_envelope)),
        };

        handle_frame(parent_frame, &state, &sink)
            .await
            .expect("parent message should be accepted")
            .expect("parent message should return ack");

        assert!(state
            .storage
            .get_pending_message(&child_hash)
            .await
            .unwrap()
            .is_none());
        let child = state
            .storage
            .get_message("child-msg")
            .await
            .unwrap()
            .expect("child promoted");
        assert_eq!(child.prev_hashes, vec![parent_hash.clone()]);
        let parent = state
            .storage
            .get_message("parent-msg")
            .await
            .unwrap()
            .expect("parent stored");
        assert_eq!(parent.msg_hash, parent_hash);
    }
}
