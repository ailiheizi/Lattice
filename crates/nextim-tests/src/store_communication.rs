//! Integration tests for real `nextim-store` WebSocket handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nextim_core::traits::storage::Storage;
use nextim_crypto::{identity::MasterKeyPair, olm::OlmAccount, sign};
use nextim_proto::{
    group::{Room, RoomEvent, RoomEventType},
    identity::{Contact, Identity},
    message::{
        envelope::Payload, EncryptedPayload, EncryptionType, Envelope, MessageContent, MessageType,
        PlainPayload,
    },
    transport::{frame, AckStatus, Frame, FrameType, MessageAck, SyncRequest},
};
use nextim_storage::{sqlite::SqliteStorage, tantivy_search::TantivySearch};
use prost::Message as ProstMessage;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message as WsMessage};

use nextim_store::{server, AppState, OnlineConnections, OutboundPool};

struct StoreFixture {
    _temp_dir: TempDir,
    state: Arc<AppState>,
}

fn test_state(data_dir: PathBuf) -> Arc<AppState> {
    let identity = MasterKeyPair::generate();
    let fingerprint = identity.fingerprint();

    Arc::new(AppState {
        storage: SqliteStorage::open(data_dir.join("store.db")).expect("sqlite storage"),
        search: TantivySearch::open(data_dir.join("search-index")).expect("tantivy search"),
        online: Arc::new(RwLock::new(HashMap::new())) as OnlineConnections,
        outbound: Arc::new(RwLock::new(HashMap::new())) as OutboundPool,
        identity,
        olm_account: Mutex::new(OlmAccount::new()),
        fingerprint,
        display_name: "WS Test Store".to_string(),
        ws_addr: "127.0.0.1:0".to_string(),
        api_token: String::new(),
        allow_unsigned: true,
        enable_dht: false,
        dht_bootstrap: Vec::new(),
        require_contact: false,
        rate_limiter: Mutex::new(nextim_core::rate_limiter::RateLimiter::new(60_000, 0)),
    })
}

fn make_envelope(msg_id: &str, sender: &str, recipient: &str, text: &str) -> Envelope {
    Envelope {
        msg_id: msg_id.to_string(),
        sender_fingerprint: sender.to_string(),
        recipient_fingerprint: recipient.to_string(),
        timestamp: 1000,
        signature: vec![],
        payload_hash: vec![],
        prev_hashes: Vec::new(),
        payload: Some(Payload::Plain(PlainPayload {
            content: Some(MessageContent {
                r#type: MessageType::Text as i32,
                text: text.to_string(),
                ..Default::default()
            }),
        })),
    }
}

fn make_encrypted_envelope(msg_id: &str, sender: &str, recipient: &str) -> Envelope {
    Envelope {
        msg_id: msg_id.to_string(),
        sender_fingerprint: sender.to_string(),
        recipient_fingerprint: recipient.to_string(),
        timestamp: 1000,
        signature: vec![],
        payload_hash: vec![],
        prev_hashes: Vec::new(),
        payload: Some(Payload::Encrypted(EncryptedPayload {
            ciphertext: b"encrypted-store-sync".to_vec(),
            session_id: "olm-session-1".to_string(),
            message_index: 42,
            encryption_type: EncryptionType::Olm as i32,
        })),
    }
}

fn sign_envelope_for_test(identity: &MasterKeyPair, mut envelope: Envelope) -> Envelope {
    let msg_hash = sign::compute_msg_hash(&envelope).expect("compute message hash");
    envelope.signature = identity.sign(&msg_hash);
    envelope.payload_hash = msg_hash;
    envelope
}

fn make_signed_contact(identity: &MasterKeyPair, store_address: &str) -> Contact {
    Contact {
        identity: Some(Identity {
            fingerprint: identity.fingerprint(),
            display_name: "Sender".to_string(),
            ed25519_public_key: identity.verifying_key().as_bytes().to_vec(),
            ..Default::default()
        }),
        store_address: store_address.to_string(),
        ..Default::default()
    }
}

fn make_room_event(
    master: &MasterKeyPair,
    room_id: &str,
    target: &str,
    timestamp: u64,
) -> RoomEvent {
    let mut event = RoomEvent {
        room_id: room_id.to_string(),
        actor_fingerprint: master.fingerprint(),
        r#type: RoomEventType::MemberJoin as i32,
        target_fingerprint: target.to_string(),
        timestamp,
        signature: Vec::new(),
        prev_hashes: Vec::new(),
        msg_hash: Vec::new(),
    };
    let hash = sign::compute_room_event_hash(&event).expect("room event hash");
    event.signature = master.sign(&hash);
    event.msg_hash = hash;
    event
}

fn test_fixture() -> StoreFixture {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let state = test_state(temp_dir.path().to_path_buf());
    StoreFixture {
        _temp_dir: temp_dir,
        state,
    }
}

async fn spawn_real_store_server() -> (StoreFixture, String, tokio::task::JoinHandle<()>) {
    let fixture = test_fixture();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let url = format!("ws://127.0.0.1:{}", addr.port());
    let state_for_server = fixture.state.clone();
    let handle = tokio::spawn(async move {
        server::run_ws_server(addr.to_string(), state_for_server)
            .await
            .expect("run ws server");
    });

    for _ in 0..20 {
        if connect_async(&url).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    (fixture, url, handle)
}

async fn spawn_ack_store() -> (
    String,
    oneshot::Receiver<Vec<u8>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = format!("ws://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(stream).await.unwrap();

        if let Some(Ok(WsMessage::Binary(data))) = ws.next().await {
            tx.send(data.to_vec()).unwrap();
            let ack = Frame {
                seq: 1,
                r#type: FrameType::Ack as i32,
                body: Some(frame::Body::Ack(MessageAck {
                    msg_id: "forward-msg".to_string(),
                    status: AckStatus::Received as i32,
                })),
            };
            ws.send(WsMessage::Binary(ack.encode_to_vec()))
                .await
                .unwrap();
        }
    });

    (addr, rx, handle)
}

#[tokio::test]
async fn real_ws_server_stores_and_syncs_messages() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "sync-room";
    let sender_identity = MasterKeyPair::generate();

    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "sync room".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&sender_identity, ""))
        .await
        .unwrap();

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let env = sign_envelope_for_test(
        &sender_identity,
        make_envelope(
            "msg-001",
            &sender_identity.fingerprint(),
            room,
            "hello store sync",
        ),
    );
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(env)),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec()))
        .await
        .unwrap();

    let ack = ws.next().await.unwrap().unwrap();
    let ack_frame = match ack {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary ack, got {other:?}"),
    };
    assert_eq!(ack_frame.r#type, FrameType::Ack as i32);
    if let Some(frame::Body::Ack(ack)) = ack_frame.body {
        assert_eq!(ack.msg_id, "msg-001");
        assert_eq!(ack.status, AckStatus::Received as i32);
    } else {
        panic!("expected ack body");
    }

    let stored = fixture
        .state
        .storage
        .get_message("msg-001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.room_id, room);
    assert_eq!(stored.content.unwrap().text, "hello store sync");
    assert!(stored.verified);
    assert!(stored.received_ts > 0);
    let expected_hash = sign::compute_msg_hash(&make_envelope(
        "msg-001",
        &sender_identity.fingerprint(),
        room,
        "hello store sync",
    ))
    .unwrap();
    assert_eq!(stored.msg_hash, expected_hash);
    assert!(stored.prev_hashes.is_empty());

    let sync = Frame {
        seq: 2,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
            requester_fingerprint: String::new(),
            requester_public_key: Vec::new(),
            signature: Vec::new(),
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        assert_eq!(response.messages.len(), 1);
        assert!(
            response.events.is_empty(),
            "room events are not currently included in store sync responses"
        );
        assert_eq!(response.messages[0].msg_id, "msg-001");
        assert_eq!(response.next_batch, stored.received_ts + 1);
        let payload = response.messages[0].payload.as_ref().unwrap();
        match payload {
            Payload::Plain(plain) => {
                assert_eq!(plain.content.as_ref().unwrap().text, "hello store sync");
            }
            other => panic!("expected plain payload, got {other:?}"),
        }
    } else {
        panic!("expected sync response body");
    }

    ws.close(None).await.unwrap();
    server_handle.abort();
}

#[tokio::test]
async fn real_ws_server_rejects_tampered_signed_messages() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "tamper-room";
    let sender_identity = MasterKeyPair::generate();

    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "tamper room".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&sender_identity, ""))
        .await
        .unwrap();

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let mut env = sign_envelope_for_test(
        &sender_identity,
        make_envelope(
            "msg-tampered",
            &sender_identity.fingerprint(),
            room,
            "hello tamper",
        ),
    );
    if let Some(Payload::Plain(plain)) = env.payload.as_mut() {
        if let Some(content) = plain.content.as_mut() {
            content.text = "tampered text".to_string();
        }
    }

    let frame = Frame {
        seq: 9,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(env)),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec()))
        .await
        .unwrap();

    let ack = ws.next().await.unwrap().unwrap();
    let ack_frame = match ack {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary ack, got {other:?}"),
    };
    assert_eq!(ack_frame.r#type, FrameType::Ack as i32);
    if let Some(frame::Body::Ack(ack)) = ack_frame.body {
        assert_eq!(ack.msg_id, "msg-tampered");
        assert_eq!(ack.status, AckStatus::Rejected as i32);
    } else {
        panic!("expected ack body");
    }

    let stored = fixture
        .state
        .storage
        .get_message("msg-tampered")
        .await
        .unwrap();
    assert!(stored.is_none(), "tampered message must not be stored");

    ws.close(None).await.unwrap();
    server_handle.abort();
}

#[tokio::test]
async fn real_ws_server_holds_out_of_order_messages_until_parent_arrives() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "pending-room";
    let sender_identity = MasterKeyPair::generate();

    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "pending room".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&sender_identity, ""))
        .await
        .unwrap();

    let parent = sign_envelope_for_test(
        &sender_identity,
        make_envelope("msg-parent", &sender_identity.fingerprint(), room, "parent"),
    );
    let parent_hash = parent.payload_hash.clone();
    let mut child = make_envelope("msg-child", &sender_identity.fingerprint(), room, "child");
    child.prev_hashes = vec![parent_hash.clone()];
    let child = sign_envelope_for_test(&sender_identity, child);

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let child_frame = Frame {
        seq: 11,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(child.clone())),
    };
    ws.send(WsMessage::Binary(child_frame.encode_to_vec()))
        .await
        .unwrap();
    let _ = ws.next().await.unwrap().unwrap();

    assert!(fixture
        .state
        .storage
        .get_message("msg-child")
        .await
        .unwrap()
        .is_none());
    assert!(fixture
        .state
        .storage
        .get_pending_message(&child.payload_hash)
        .await
        .unwrap()
        .is_some());

    let sync = Frame {
        seq: 12,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
            requester_fingerprint: String::new(),
            requester_public_key: Vec::new(),
            signature: Vec::new(),
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();
    let pending_sync = ws.next().await.unwrap().unwrap();
    let pending_sync_frame = match pending_sync {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };
    if let Some(frame::Body::SyncResponse(response)) = pending_sync_frame.body {
        assert!(response.messages.is_empty());
    } else {
        panic!("expected sync response body");
    }

    let parent_frame = Frame {
        seq: 13,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(parent)),
    };
    ws.send(WsMessage::Binary(parent_frame.encode_to_vec()))
        .await
        .unwrap();
    let _ = ws.next().await.unwrap().unwrap();

    assert!(fixture
        .state
        .storage
        .get_pending_message(&child.payload_hash)
        .await
        .unwrap()
        .is_none());

    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();
    let promoted_sync = ws.next().await.unwrap().unwrap();
    let promoted_sync_frame = match promoted_sync {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };
    if let Some(frame::Body::SyncResponse(response)) = promoted_sync_frame.body {
        let ids: Vec<_> = response
            .messages
            .into_iter()
            .map(|msg| msg.msg_id)
            .collect();
        assert_eq!(ids, vec!["msg-parent".to_string(), "msg-child".to_string()]);
    } else {
        panic!("expected sync response body");
    }

    ws.close(None).await.unwrap();
    server_handle.abort();
}

#[tokio::test]
async fn real_ws_server_stores_and_syncs_room_events() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "room-event-sync-room";
    let actor = MasterKeyPair::generate();
    let actor_fp = actor.fingerprint();

    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "room event sync room".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();

    // 存 actor 公钥到联系人，使 Store 能验证房间事件签名。
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&actor, ""))
        .await
        .unwrap();

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let frame = Frame {
        seq: 3,
        r#type: FrameType::RoomEvent as i32,
        body: Some(frame::Body::RoomEvent(make_room_event(
            &actor, room, "alice-fp", 2000,
        ))),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec()))
        .await
        .unwrap();

    let ack = ws.next().await.unwrap().unwrap();
    let ack_frame = match ack {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary ack, got {other:?}"),
    };
    assert_eq!(ack_frame.r#type, FrameType::Ack as i32);
    if let Some(frame::Body::Ack(ack)) = ack_frame.body {
        assert_eq!(ack.msg_id, "2000");
        assert_eq!(ack.status, AckStatus::Received as i32);
    } else {
        panic!("expected ack body");
    }

    let stored_events = fixture
        .state
        .storage
        .get_room_events(room, 0)
        .await
        .unwrap();
    assert_eq!(stored_events.len(), 1);
    assert_eq!(stored_events[0].actor_fingerprint, actor_fp);
    assert_eq!(stored_events[0].target_fingerprint, "alice-fp");
    assert_eq!(stored_events[0].timestamp, 2000);

    let sync = Frame {
        seq: 4,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
            requester_fingerprint: String::new(),
            requester_public_key: Vec::new(),
            signature: Vec::new(),
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        assert!(response.messages.is_empty());
        assert_eq!(response.events.len(), 1);
        assert_eq!(response.events[0].room_id, room);
        assert_eq!(response.events[0].actor_fingerprint, actor_fp);
        assert_eq!(response.events[0].target_fingerprint, "alice-fp");
        assert_eq!(response.events[0].timestamp, 2000);
        // next_batch 现在是 received_ts 游标（Store 盖的真实毫秒戳），不再等于 origin timestamp+1。
        // 用它再次 sync 应取不到该事件（游标语义正确）。
        assert!(
            response.next_batch > 2000,
            "next_batch should be a received_ts cursor"
        );
        let cursor_after = response.next_batch;

        // P4：房间事件已纳入统一 DAG 时间线，并填充了 msg_hash。
        assert_eq!(response.timeline.len(), 1);
        let item = &response.timeline[0];
        assert!(
            !item.msg_hash.is_empty(),
            "timeline item should carry msg_hash"
        );
        match &item.item {
            Some(nextim_proto::transport::sync_timeline_item::Item::RoomEvent(ev)) => {
                assert_eq!(ev.actor_fingerprint, actor_fp);
                assert_eq!(ev.target_fingerprint, "alice-fp");
            }
            other => panic!("expected room event in timeline, got {other:?}"),
        }

        // 游标语义验证（Major 2）：用 next_batch 作为 since 再次 sync，
        // 该事件不应重复返回（received_ts 严格大于过滤）。
        let resync = Frame {
            seq: 5,
            r#type: FrameType::SyncRequest as i32,
            body: Some(frame::Body::SyncRequest(SyncRequest {
                since_timestamp: cursor_after,
                room_ids: vec![room.to_string()],
                requester_fingerprint: String::new(),
                requester_public_key: Vec::new(),
                signature: Vec::new(),
            })),
        };
        ws.send(WsMessage::Binary(resync.encode_to_vec()))
            .await
            .unwrap();
        let resync_resp = ws.next().await.unwrap().unwrap();
        if let WsMessage::Binary(data) = resync_resp {
            if let Some(frame::Body::SyncResponse(r2)) = Frame::decode(data.as_ref()).unwrap().body
            {
                assert!(
                    r2.events.is_empty(),
                    "cursor must not re-deliver same event"
                );
                assert!(
                    r2.timeline.is_empty(),
                    "cursor must not re-deliver in timeline"
                );
            } else {
                panic!("expected sync response on resync");
            }
        } else {
            panic!("expected binary resync response");
        }
    } else {
        panic!("expected sync response body");
    }

    ws.close(None).await.unwrap();
    server_handle.abort();
}

#[tokio::test]
async fn real_ws_server_preserves_encrypted_payloads_during_sync() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "encrypted-sync-room";
    let sender_identity = MasterKeyPair::generate();

    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "encrypted sync room".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&sender_identity, ""))
        .await
        .unwrap();

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let env = sign_envelope_for_test(
        &sender_identity,
        make_encrypted_envelope("msg-enc-001", &sender_identity.fingerprint(), room),
    );
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(env)),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec()))
        .await
        .unwrap();

    let ack = ws.next().await.unwrap().unwrap();
    let ack_frame = match ack {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary ack, got {other:?}"),
    };
    assert_eq!(ack_frame.r#type, FrameType::Ack as i32);

    let stored = fixture
        .state
        .storage
        .get_message("msg-enc-001")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.room_id, room);
    assert!(stored.encrypted);
    assert!(stored.content.is_none());
    assert!(stored.verified);
    let stored_payload = stored
        .encrypted_payload
        .as_ref()
        .expect("encrypted payload stored");
    assert_eq!(stored_payload.ciphertext, b"encrypted-store-sync".to_vec());
    assert_eq!(stored_payload.session_id, "olm-session-1");
    assert_eq!(stored_payload.message_index, 42);
    assert_eq!(stored_payload.encryption_type, EncryptionType::Olm as i32);

    let sync = Frame {
        seq: 2,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
            requester_fingerprint: String::new(),
            requester_public_key: Vec::new(),
            signature: Vec::new(),
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].msg_id, "msg-enc-001");
        assert_eq!(response.next_batch, stored.received_ts + 1);
        let payload = response.messages[0].payload.as_ref().unwrap();
        match payload {
            Payload::Encrypted(encrypted) => {
                assert_eq!(encrypted.ciphertext, b"encrypted-store-sync".to_vec());
                assert_eq!(encrypted.session_id, "olm-session-1");
                assert_eq!(encrypted.message_index, 42);
                assert_eq!(encrypted.encryption_type, EncryptionType::Olm as i32);
            }
            other => panic!("expected encrypted payload, got {other:?}"),
        }
    } else {
        panic!("expected sync response body");
    }

    ws.close(None).await.unwrap();
    server_handle.abort();
}

#[tokio::test]
async fn real_ws_server_forwards_to_recipient_store_from_contacts() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let recipient = "bob-fp";
    let sender_identity = MasterKeyPair::generate();
    let (forward_addr, received, forward_handle) = spawn_ack_store().await;

    fixture
        .state
        .storage
        .save_contact(&Contact {
            identity: Some(Identity {
                fingerprint: recipient.to_string(),
                display_name: "Bob".to_string(),
                ..Default::default()
            }),
            store_address: forward_addr.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&sender_identity, ""))
        .await
        .unwrap();

    let (mut ws, _) = connect_async(&url).await.unwrap();
    let signed_message = sign_envelope_for_test(
        &sender_identity,
        make_envelope(
            "forward-msg",
            &sender_identity.fingerprint(),
            recipient,
            "hello forwarded store",
        ),
    );
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(signed_message)),
    };
    let encoded = frame.encode_to_vec();
    ws.send(WsMessage::Binary(encoded.clone())).await.unwrap();

    let ack = ws.next().await.unwrap().unwrap();
    let ack_frame = match ack {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary ack, got {other:?}"),
    };
    assert_eq!(ack_frame.r#type, FrameType::Ack as i32);

    let forwarded = tokio::time::timeout(Duration::from_secs(2), received)
        .await
        .expect("forward timeout")
        .expect("forward receiver result");
    assert_eq!(forwarded, encoded);

    ws.close(None).await.unwrap();
    server_handle.abort();
    forward_handle.abort();
}

#[tokio::test]
async fn real_ws_server_unifies_messages_and_room_events_in_timeline() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "unified-timeline-room";
    let actor = MasterKeyPair::generate();

    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "unified timeline".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();

    // 存 actor 公钥到联系人，使房间事件验签可通过。
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&actor, ""))
        .await
        .unwrap();

    // 直接存一条已带 msg_hash 的消息（绕过 WS 签名链，专注验证 timeline 合并）。
    let stored_msg = nextim_proto::message::Message {
        msg_id: "unified-msg-1".to_string(),
        room_id: room.to_string(),
        sender_fingerprint: "alice-fp".to_string(),
        timestamp: 500,
        content: Some(MessageContent {
            r#type: MessageType::Text as i32,
            text: "hello timeline".to_string(),
            ..Default::default()
        }),
        encrypted: false,
        verified: true,
        encrypted_payload: None,
        received_ts: 1000,
        prev_hashes: Vec::new(),
        msg_hash: b"unified-msg-hash-1".to_vec(),
        redacted: false,
        edited: false,
    };
    fixture
        .state
        .storage
        .save_message(&stored_msg)
        .await
        .unwrap();

    let (mut ws, _) = connect_async(&url).await.unwrap();

    // 再发一个房间事件到同一房间。
    let event_frame = Frame {
        seq: 1,
        r#type: FrameType::RoomEvent as i32,
        body: Some(frame::Body::RoomEvent(make_room_event(
            &actor, room, "bob-fp", 3000,
        ))),
    };
    ws.send(WsMessage::Binary(event_frame.encode_to_vec()))
        .await
        .unwrap();
    let _ = ws.next().await.unwrap().unwrap(); // ack

    let sync = Frame {
        seq: 2,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
            requester_fingerprint: String::new(),
            requester_public_key: Vec::new(),
            signature: Vec::new(),
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        // 统一时间线应同时包含消息与房间事件两类。
        assert_eq!(
            response.timeline.len(),
            2,
            "timeline should merge message + room event"
        );
        let mut has_message = false;
        let mut has_event = false;
        for item in &response.timeline {
            assert!(!item.msg_hash.is_empty());
            match &item.item {
                Some(nextim_proto::transport::sync_timeline_item::Item::Message(_)) => {
                    has_message = true
                }
                Some(nextim_proto::transport::sync_timeline_item::Item::RoomEvent(_)) => {
                    has_event = true
                }
                None => panic!("timeline item missing inner item"),
            }
        }
        assert!(has_message && has_event, "timeline must contain both kinds");
    } else {
        panic!("expected sync response body");
    }

    ws.close(None).await.unwrap();
    server_handle.abort();
}

/// E3:1v1 端到端 E2EE —— Alice 真实 Olm 加密 → 经真实 Store WS 存储/sync(密文)
/// → Bob 真实 Olm 解密还原明文。验证 Store 全程只见密文。
#[tokio::test]
async fn e2ee_1v1_roundtrip_through_real_store() {
    use nextim_crypto::session::OlmSessionManager;

    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "e2ee-room";

    // Alice/Bob 各有签名身份(ed25519)+ Olm 编排器(curve25519 加密)。
    let alice_identity = MasterKeyPair::generate();
    let mut alice_olm = OlmSessionManager::new(OlmAccount::new());
    let mut bob_olm = OlmSessionManager::new(OlmAccount::new());

    // Bob 发布预密钥(模拟 E1 的 /keys/bundle);Alice 取到后建出站会话(模拟 /keys/claim)。
    let bob_otks = bob_olm.publish_one_time_keys(1);
    let bob_identity_key = bob_olm.identity_key_bytes();
    alice_olm
        .establish_outbound(&bob_identity_key, &bob_otks[0])
        .unwrap();

    // 房间 + Alice 作为已签名联系人(放行写入)。
    fixture
        .state
        .storage
        .save_room(&Room {
            room_id: room.to_string(),
            name: "e2ee room".to_string(),
            creator_fingerprint: "creator".to_string(),
            members: vec![],
            ..Default::default()
        })
        .await
        .unwrap();
    fixture
        .state
        .storage
        .save_contact(&make_signed_contact(&alice_identity, ""))
        .await
        .unwrap();

    // Alice 真实加密明文 → EncryptedPayload。
    let plaintext = b"top secret end-to-end message";
    let payload = alice_olm.encrypt(&bob_identity_key, plaintext).unwrap();
    // 密文必须真实区别于明文。
    assert_ne!(payload.ciphertext, plaintext.to_vec());
    assert_eq!(payload.encryption_type, EncryptionType::Olm as i32);

    let env = sign_envelope_for_test(
        &alice_identity,
        Envelope {
            msg_id: "e2ee-msg-1".to_string(),
            sender_fingerprint: alice_identity.fingerprint(),
            recipient_fingerprint: room.to_string(),
            timestamp: 1000,
            signature: vec![],
            payload_hash: vec![],
            prev_hashes: Vec::new(),
            payload: Some(Payload::Encrypted(payload.clone())),
        },
    );
    let (mut ws, _) = connect_async(&url).await.unwrap();
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(env)),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec()))
        .await
        .unwrap();
    let ack = ws.next().await.unwrap().unwrap();
    let ack_frame = match ack {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary ack, got {other:?}"),
    };
    assert_eq!(ack_frame.r#type, FrameType::Ack as i32);

    // Store 只见密文:无明文 content,密文原样存储。
    let stored = fixture
        .state
        .storage
        .get_message("e2ee-msg-1")
        .await
        .unwrap()
        .unwrap();
    assert!(stored.encrypted);
    assert!(stored.content.is_none(), "Store must not see plaintext");
    let stored_payload = stored.encrypted_payload.as_ref().unwrap();
    assert_eq!(stored_payload.ciphertext, payload.ciphertext);

    // Bob 经 sync 取回密文。
    let sync = Frame {
        seq: 2,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
            requester_fingerprint: String::new(),
            requester_public_key: Vec::new(),
            signature: Vec::new(),
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec()))
        .await
        .unwrap();
    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };
    let synced_payload = match sync_frame.body {
        Some(frame::Body::SyncResponse(response)) => {
            assert_eq!(response.messages.len(), 1);
            match response.messages[0].payload.as_ref().unwrap() {
                Payload::Encrypted(e) => e.clone(),
                other => panic!("expected encrypted payload, got {other:?}"),
            }
        }
        other => panic!("expected sync response body, got {other:?}"),
    };

    // Bob 真实解密(首条 PreKey → 自动建入站会话),还原明文。
    let alice_identity_key = alice_olm.identity_key_bytes();
    let decrypted = bob_olm
        .decrypt(&alice_identity_key, &synced_payload)
        .unwrap();
    assert_eq!(decrypted, plaintext);

    ws.close(None).await.unwrap();
    server_handle.abort();
}
