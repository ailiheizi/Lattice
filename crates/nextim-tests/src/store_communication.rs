//! Integration tests for real `nextim-store` WebSocket handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nextim_core::traits::storage::Storage;
use nextim_crypto::{identity::MasterKeyPair, olm::OlmAccount};
use nextim_proto::{
    group::{Room, RoomEvent, RoomEventType},
    identity::{Contact, Identity},
    message::{
        EncryptionType, EncryptedPayload, Envelope, MessageContent, MessageType, PlainPayload,
        envelope::Payload,
    },
    transport::{AckStatus, Frame, FrameType, MessageAck, SyncRequest, frame},
};
use nextim_storage::{sqlite::SqliteStorage, tantivy_search::TantivySearch};
use prost::Message as ProstMessage;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio_tungstenite::{
    accept_async, connect_async,
    tungstenite::Message as WsMessage,
};

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
        payload: Some(Payload::Encrypted(EncryptedPayload {
            ciphertext: b"encrypted-store-sync".to_vec(),
            session_id: "olm-session-1".to_string(),
            message_index: 42,
            encryption_type: EncryptionType::Olm as i32,
        })),
    }
}

fn make_room_event(room_id: &str, actor: &str, target: &str, timestamp: u64) -> RoomEvent {
    RoomEvent {
        room_id: room_id.to_string(),
        actor_fingerprint: actor.to_string(),
        r#type: RoomEventType::MemberJoin as i32,
        target_fingerprint: target.to_string(),
        timestamp,
        signature: b"room-event-signature".to_vec(),
    }
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

async fn spawn_ack_store() -> (String, oneshot::Receiver<Vec<u8>>, tokio::task::JoinHandle<()>) {
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
            ws.send(WsMessage::Binary(ack.encode_to_vec().into())).await.unwrap();
        }
    });

    (addr, rx, handle)
}

#[tokio::test]
async fn real_ws_server_stores_and_syncs_messages() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "sync-room";

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

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let env = make_envelope("msg-001", "alice", room, "hello store sync");
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(env)),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec().into())).await.unwrap();

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

    let sync = Frame {
        seq: 2,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec().into())).await.unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        assert_eq!(response.messages.len(), 1);
        assert!(response.events.is_empty(), "room events are not currently included in store sync responses");
        assert_eq!(response.messages[0].msg_id, "msg-001");
        assert_eq!(response.next_batch, 1001);
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
async fn real_ws_server_stores_and_syncs_room_events() {
    let (fixture, url, server_handle) = spawn_real_store_server().await;
    let room = "room-event-sync-room";

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

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let frame = Frame {
        seq: 3,
        r#type: FrameType::RoomEvent as i32,
        body: Some(frame::Body::RoomEvent(make_room_event(
            room,
            "owner-fp",
            "alice-fp",
            2000,
        ))),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec().into())).await.unwrap();

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
    assert_eq!(stored_events[0].actor_fingerprint, "owner-fp");
    assert_eq!(stored_events[0].target_fingerprint, "alice-fp");
    assert_eq!(stored_events[0].timestamp, 2000);

    let sync = Frame {
        seq: 4,
        r#type: FrameType::SyncRequest as i32,
        body: Some(frame::Body::SyncRequest(SyncRequest {
            since_timestamp: 0,
            room_ids: vec![room.to_string()],
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec().into())).await.unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        assert!(response.messages.is_empty());
        assert_eq!(response.events.len(), 1);
        assert_eq!(response.events[0].room_id, room);
        assert_eq!(response.events[0].actor_fingerprint, "owner-fp");
        assert_eq!(response.events[0].target_fingerprint, "alice-fp");
        assert_eq!(response.events[0].timestamp, 2000);
        assert_eq!(response.next_batch, 2001);
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

    let (mut ws, _) = connect_async(&url).await.unwrap();

    let env = make_encrypted_envelope("msg-enc-001", "alice", room);
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(env)),
    };
    ws.send(WsMessage::Binary(frame.encode_to_vec().into())).await.unwrap();

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
    let stored_payload = stored.encrypted_payload.as_ref().expect("encrypted payload stored");
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
        })),
    };
    ws.send(WsMessage::Binary(sync.encode_to_vec().into())).await.unwrap();

    let sync_resp = ws.next().await.unwrap().unwrap();
    let sync_frame = match sync_resp {
        WsMessage::Binary(data) => Frame::decode(data.as_ref()).unwrap(),
        other => panic!("expected binary sync response, got {other:?}"),
    };

    if let Some(frame::Body::SyncResponse(response)) = sync_frame.body {
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].msg_id, "msg-enc-001");
        assert_eq!(response.next_batch, 1001);
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

    let (mut ws, _) = connect_async(&url).await.unwrap();
    let frame = Frame {
        seq: 1,
        r#type: FrameType::Message as i32,
        body: Some(frame::Body::Message(make_envelope(
            "forward-msg",
            "alice-fp",
            recipient,
            "hello forwarded store",
        ))),
    };
    let encoded = frame.encode_to_vec();
    ws.send(WsMessage::Binary(encoded.clone().into())).await.unwrap();

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
