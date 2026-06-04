//! Peer 中转逻辑

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

use nextim_proto::transport::{frame, AckStatus, Frame, FrameType, MessageAck, Pong};

use crate::cache::RelayCache;
use crate::observability::SharedPeerObservability;

pub async fn run_relay_server(
    addr: String,
    cache: Arc<Mutex<RelayCache>>,
    observability: SharedPeerObservability,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Peer relay listening on {addr}");

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        let cache = cache.clone();
        let observability = observability.clone();

        tokio::spawn(async move {
            tracing::info!("Peer connection from {peer_addr}");
            match accept_async(tcp_stream).await {
                Ok(ws_stream) => {
                    let connection_id = {
                        let mut state = observability.lock().await;
                        state.register_connection(peer_addr.to_string())
                    };

                    let result = handle_connection(
                        ws_stream,
                        cache,
                        observability.clone(),
                        connection_id.clone(),
                    )
                    .await;

                    {
                        let mut state = observability.lock().await;
                        if result.is_err() {
                            state.record_error();
                        }
                        state.unregister_connection(&connection_id);
                    }

                    if let Err(e) = result {
                        tracing::warn!("Peer connection {peer_addr} error: {e}");
                    }
                }
                Err(e) => {
                    let mut state = observability.lock().await;
                    state.record_error();
                    tracing::warn!("WS handshake failed for {peer_addr}: {e}");
                }
            }
        });
    }
}

async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    cache: Arc<Mutex<RelayCache>>,
    observability: SharedPeerObservability,
    connection_id: String,
) -> anyhow::Result<()> {
    let (mut sink, mut stream) = ws_stream.split();

    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg {
            WsMessage::Binary(data) => {
                let frame = Frame::decode(data.as_ref())
                    .map_err(|e| anyhow::anyhow!("decode frame: {e}"))?;

                {
                    let mut state = observability.lock().await;
                    state.record_connection_message(&connection_id);
                }

                let response = handle_frame(frame, &cache, &observability).await?;
                if let Some(resp_frame) = response {
                    let encoded = resp_frame.encode_to_vec();
                    sink.send(WsMessage::Binary(encoded)).await?;
                }
            }
            WsMessage::Ping(payload) => {
                sink.send(WsMessage::Pong(payload)).await?;
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}

async fn handle_frame(
    frame: Frame,
    cache: &Mutex<RelayCache>,
    observability: &SharedPeerObservability,
) -> anyhow::Result<Option<Frame>> {
    match frame.body {
        Some(frame::Body::Message(ref envelope)) => {
            if envelope.signature.is_empty() {
                tracing::warn!("Rejecting message {}: missing signature", envelope.msg_id);
                return Ok(Some(rejected_ack(frame.seq, &envelope.msg_id)));
            }
            if envelope.payload_hash.len() != 32 {
                tracing::warn!(
                    "Rejecting message {}: invalid payload_hash length {}",
                    envelope.msg_id,
                    envelope.payload_hash.len()
                );
                return Ok(Some(rejected_ack(frame.seq, &envelope.msg_id)));
            }
            if envelope.payload.is_none() {
                tracing::warn!("Rejecting message {}: missing payload", envelope.msg_id);
                return Ok(Some(rejected_ack(frame.seq, &envelope.msg_id)));
            }

            // TODO(D-5): Peer 当前只有结构校验，无发送方公钥，后续按设计文档补 soft-fail/验签策略。
            let recipient = &envelope.recipient_fingerprint;
            let data = frame.encode_to_vec();

            let mut c = cache.lock().await;
            c.store(recipient, data);
            drop(c);

            {
                let mut state = observability.lock().await;
                state.record_relayed_message();
            }

            tracing::info!("Cached message {} for {}", envelope.msg_id, recipient);

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
            let recipient = if event.target_fingerprint.is_empty() {
                &event.room_id
            } else {
                &event.target_fingerprint
            };
            let data = frame.encode_to_vec();

            let mut c = cache.lock().await;
            c.store(recipient, data);
            drop(c);

            {
                let mut state = observability.lock().await;
                state.record_relayed_message();
            }

            tracing::info!(
                "Cached room event for room {} and recipient {}",
                event.room_id,
                recipient
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

        Some(frame::Body::SyncRequest(req)) => {
            // 接收方来取缓存的消息
            // 用 sender_fingerprint 作为 requester 标识（通过 room_ids[0] 传递）
            let requester = req.room_ids.first().cloned().unwrap_or_default();

            let mut c = cache.lock().await;
            let cached = c.drain_for(&requester);
            drop(c);

            let mut envelopes = Vec::new();
            let mut events = Vec::new();
            for data in cached {
                if let Ok(f) = Frame::decode(data.as_slice()) {
                    match f.body {
                        Some(frame::Body::Message(env)) => envelopes.push(env),
                        Some(frame::Body::RoomEvent(event)) => events.push(event),
                        _ => {}
                    }
                }
            }

            {
                let mut state = observability.lock().await;
                state.record_delivered_messages(envelopes.len() + events.len());
            }

            tracing::info!(
                "Delivered {} cached items to {}",
                envelopes.len() + events.len(),
                requester
            );

            let next_batch = envelopes
                .iter()
                .map(|message| message.timestamp)
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
                        timeline: Vec::new(),
                    },
                )),
            }))
        }

        Some(frame::Body::Ping(ping)) => Ok(Some(Frame {
            seq: frame.seq,
            r#type: FrameType::Pong as i32,
            body: Some(frame::Body::Pong(Pong {
                timestamp: ping.timestamp,
            })),
        })),

        _ => Ok(None),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::PeerObservability;
    use nextim_proto::{
        message::{envelope::Payload, Envelope, MessageContent, MessageType, PlainPayload},
        transport::{frame, AckStatus, FrameType, SyncRequest},
    };

    fn test_message_frame(seq: u64, msg_id: &str, recipient: &str) -> Frame {
        Frame {
            seq,
            r#type: FrameType::Message as i32,
            body: Some(frame::Body::Message(Envelope {
                msg_id: msg_id.to_string(),
                sender_fingerprint: "sender".to_string(),
                recipient_fingerprint: recipient.to_string(),
                timestamp: 1,
                signature: vec![1; 64],
                payload_hash: vec![2; 32],
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

    #[tokio::test]
    async fn handle_frame_message_updates_relayed_counter_and_returns_ack() {
        let cache = Mutex::new(RelayCache::new(10, 60_000));
        let observability = Arc::new(Mutex::new(PeerObservability::default()));

        let response = handle_frame(
            test_message_frame(7, "msg-1", "alice"),
            &cache,
            &observability,
        )
        .await
        .unwrap()
        .expect("message frames should be acknowledged");

        assert_eq!(response.seq, 7);
        assert_eq!(response.r#type, FrameType::Ack as i32);

        match response.body {
            Some(frame::Body::Ack(ack)) => {
                assert_eq!(ack.msg_id, "msg-1");
                assert_eq!(ack.status, AckStatus::Received as i32);
            }
            other => panic!("expected ack response, got {other:?}"),
        }

        let snapshot = observability.lock().await.snapshot();
        assert_eq!(snapshot.total_relayed, 1);
        assert_eq!(snapshot.total_delivered, 0);

        let cached = cache.lock().await.drain_for("alice");
        assert_eq!(cached.len(), 1);

        let cached_frame = Frame::decode(cached[0].as_slice()).expect("cached frame should decode");
        match cached_frame.body {
            Some(frame::Body::Message(envelope)) => {
                assert_eq!(envelope.msg_id, "msg-1");
                assert_eq!(envelope.recipient_fingerprint, "alice");
            }
            other => panic!("expected cached message frame, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_frame_rejects_message_without_signature() {
        let cache = Mutex::new(RelayCache::new(10, 60_000));
        let observability = Arc::new(Mutex::new(PeerObservability::default()));
        let mut frame = test_message_frame(7, "msg-1", "alice");
        if let Some(frame::Body::Message(envelope)) = frame.body.as_mut() {
            envelope.signature.clear();
        }

        let response = handle_frame(frame, &cache, &observability)
            .await
            .unwrap()
            .expect("message frames should return ack");

        match response.body {
            Some(frame::Body::Ack(ack)) => {
                assert_eq!(ack.status, AckStatus::Rejected as i32);
            }
            other => panic!("expected ack response, got {other:?}"),
        }

        let snapshot = observability.lock().await.snapshot();
        assert_eq!(snapshot.total_relayed, 0);
        assert!(cache.lock().await.drain_for("alice").is_empty());
    }

    #[tokio::test]
    async fn handle_frame_rejects_message_with_invalid_payload_hash_length() {
        let cache = Mutex::new(RelayCache::new(10, 60_000));
        let observability = Arc::new(Mutex::new(PeerObservability::default()));
        let mut frame = test_message_frame(7, "msg-1", "alice");
        if let Some(frame::Body::Message(envelope)) = frame.body.as_mut() {
            envelope.payload_hash = vec![2; 31];
        }

        let response = handle_frame(frame, &cache, &observability)
            .await
            .unwrap()
            .expect("message frames should return ack");

        match response.body {
            Some(frame::Body::Ack(ack)) => {
                assert_eq!(ack.status, AckStatus::Rejected as i32);
            }
            other => panic!("expected ack response, got {other:?}"),
        }

        let snapshot = observability.lock().await.snapshot();
        assert_eq!(snapshot.total_relayed, 0);
        assert!(cache.lock().await.drain_for("alice").is_empty());
    }

    #[tokio::test]
    async fn handle_frame_rejects_message_without_payload() {
        let cache = Mutex::new(RelayCache::new(10, 60_000));
        let observability = Arc::new(Mutex::new(PeerObservability::default()));
        let mut frame = test_message_frame(7, "msg-1", "alice");
        if let Some(frame::Body::Message(envelope)) = frame.body.as_mut() {
            envelope.payload = None;
        }

        let response = handle_frame(frame, &cache, &observability)
            .await
            .unwrap()
            .expect("message frames should return ack");

        match response.body {
            Some(frame::Body::Ack(ack)) => {
                assert_eq!(ack.status, AckStatus::Rejected as i32);
            }
            other => panic!("expected ack response, got {other:?}"),
        }

        let snapshot = observability.lock().await.snapshot();
        assert_eq!(snapshot.total_relayed, 0);
        assert!(cache.lock().await.drain_for("alice").is_empty());
    }

    #[tokio::test]
    async fn handle_frame_sync_request_updates_delivered_counter_from_cached_messages() {
        let cache = Mutex::new(RelayCache::new(10, 60_000));
        let observability = Arc::new(Mutex::new(PeerObservability::default()));

        {
            let mut cache = cache.lock().await;
            cache.store(
                "alice",
                test_message_frame(1, "msg-1", "alice").encode_to_vec(),
            );
            cache.store(
                "alice",
                test_message_frame(2, "msg-2", "alice").encode_to_vec(),
            );
            cache.store("bob", test_message_frame(3, "msg-3", "bob").encode_to_vec());
        }

        let response = handle_frame(
            Frame {
                seq: 42,
                r#type: FrameType::SyncRequest as i32,
                body: Some(frame::Body::SyncRequest(SyncRequest {
                    since_timestamp: 0,
                    room_ids: vec!["alice".to_string()],
                    requester_fingerprint: String::new(),
                })),
            },
            &cache,
            &observability,
        )
        .await
        .unwrap()
        .expect("sync request should produce a response");

        assert_eq!(response.seq, 42);
        assert_eq!(response.r#type, FrameType::SyncResponse as i32);

        match response.body {
            Some(frame::Body::SyncResponse(sync_response)) => {
                let message_ids = sync_response
                    .messages
                    .iter()
                    .map(|message| message.msg_id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(message_ids, vec!["msg-1", "msg-2"]);
                assert!(sync_response.events.is_empty());
                assert_eq!(sync_response.next_batch, 2);
            }
            other => panic!("expected sync response, got {other:?}"),
        }

        let snapshot = observability.lock().await.snapshot();
        assert_eq!(snapshot.total_relayed, 0);
        assert_eq!(snapshot.total_delivered, 2);

        let mut cache = cache.lock().await;
        assert!(cache.drain_for("alice").is_empty());
        assert_eq!(cache.drain_for("bob").len(), 1);
    }
}
