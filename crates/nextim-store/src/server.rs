use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

use nextim_core::traits::search::SearchIndex;
use nextim_core::traits::storage::Storage;
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
            // 验证签名（如果有）
            let verified = if !envelope.signature.is_empty() && !envelope.payload_hash.is_empty() {
                // 尝试从联系人获取发送方公钥
                let sender_key = state
                    .storage
                    .get_contact(&envelope.sender_fingerprint)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|c| c.identity)
                    .map(|i| i.ed25519_public_key);

                if let Some(key) = sender_key {
                    match sign::verify_envelope(&key, envelope) {
                        Ok(true) => {
                            tracing::debug!("Signature verified for {}", envelope.msg_id);
                            true
                        }
                        _ => {
                            tracing::warn!("Signature verification failed for {}", envelope.msg_id);
                            false
                        }
                    }
                } else {
                    tracing::debug!(
                        "No public key for sender {}, skipping verification",
                        envelope.sender_fingerprint
                    );
                    false
                }
            } else {
                false
            };

            // 存储消息
            let msg = Message {
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
            };

            state
                .storage
                .save_message(&msg)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let _ = state.search.index_message(&msg).await;

            tracing::info!(
                "Stored message {} from {}",
                msg.msg_id,
                msg.sender_fingerprint
            );

            // 实时推送：如果接收方在线，直接推送
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

            // 也尝试转发给接收方的 Store（异步，不阻塞）
            let recipient_clone = recipient.to_owned();
            let state = Arc::clone(state);
            tokio::spawn(async move {
                if let Err(error) = try_forward_message(state, recipient_clone, frame_data).await {
                    tracing::warn!("Failed to forward to recipient store: {error}");
                }
            });

            // 返回 ACK
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
            state
                .storage
                .save_room_event(event)
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
            use nextim_core::traits::storage::{Pagination, TimeRange};

            let range = TimeRange {
                start: req.since_timestamp,
                end: 9_999_999_999_999,
            };
            let page = Pagination {
                offset: 0,
                limit: 100,
            };

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
            let mut events = Vec::new();
            for room_id in &room_ids {
                let msgs = state
                    .storage
                    .get_messages(room_id, &range, &page)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                for msg in msgs {
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
                        payload_hash: vec![],
                        payload,
                    });
                }

                let room_events = state
                    .storage
                    .get_room_events(room_id, req.since_timestamp)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                events.extend(room_events);
            }

            let next_batch = envelopes
                .iter()
                .map(|e| e.timestamp)
                .chain(events.iter().map(|event| event.timestamp))
                .max()
                .map(|timestamp| timestamp + 1)
                .unwrap_or(req.since_timestamp);

            Ok(Some(Frame {
                seq: frame.seq,
                r#type: FrameType::SyncResponse as i32,
                body: Some(frame::Body::SyncResponse(
                    nextim_proto::transport::SyncResponse {
                        messages: envelopes,
                        events,
                        next_batch,
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
}
