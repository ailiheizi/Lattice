//! Store REST API integration tests.
//!
//! These tests exercise the real `nextim-store` production API server on
//! ephemeral ports.

use std::collections::HashMap;
use std::sync::Arc;

use nextim_crypto::{identity::MasterKeyPair, olm::OlmAccount};
use nextim_storage::{sqlite::SqliteStorage, tantivy_search::TantivySearch};
use tokio::sync::{Mutex, RwLock};

use nextim_store::{api, AppState, OnlineConnections, OutboundPool};

fn test_state() -> Arc<AppState> {
    let storage = SqliteStorage::in_memory().expect("in-memory storage");
    let search = TantivySearch::in_memory().expect("in-memory search");
    let identity = MasterKeyPair::generate();
    let fingerprint = identity.fingerprint();

    let mut olm = OlmAccount::new();
    olm.generate_one_time_keys(5);
    olm.mark_keys_as_published();

    Arc::new(AppState {
        storage,
        search,
        online: Arc::new(RwLock::new(HashMap::new())) as OnlineConnections,
        outbound: Arc::new(RwLock::new(HashMap::new())) as OutboundPool,
        identity,
        olm_account: Mutex::new(olm),
        fingerprint,
        display_name: "Test Store".to_string(),
        ws_addr: "127.0.0.1:0".to_string(),
    })
}

async fn start_store_api() -> (String, tokio::task::JoinHandle<()>) {
    let state = test_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}", addr.port());
    let handle = tokio::spawn(async move {
        api::serve_api(listener, state).await.unwrap();
    });
    (url, handle)
}

#[tokio::test]
async fn health_identity_and_keys_use_real_store_state() {
    let (url, handle) = start_store_api().await;
    let client = reqwest::Client::new();

    let health = client.get(format!("{url}/health")).send().await.unwrap();
    assert_eq!(health.status(), 200);
    assert_eq!(health.text().await.unwrap(), "ok");

    let identity: serde_json::Value = client
        .get(format!("{url}/identity"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(identity["display_name"], "Test Store");
    assert_eq!(identity["online_count"], 0);
    assert!(identity["fingerprint"].as_str().unwrap().len() >= 8);

    let keys_before: serde_json::Value = client
        .get(format!("{url}/keys/one-time"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let generated: serde_json::Value = client
        .post(format!("{url}/keys/generate"))
        .json(&serde_json::json!({ "count": 7 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(
        keys_before["curve25519_identity_key"],
        generated["curve25519_identity_key"]
    );
    assert!(keys_before["curve25519_identity_key"]
        .as_str()
        .unwrap()
        .len()
        > 10);
    assert!(generated["one_time_keys"].is_array());

    handle.abort();
}

#[tokio::test]
async fn store_api_persists_messages_and_search_via_real_routes() {
    let (url, handle) = start_store_api().await;
    let client = reqwest::Client::new();

    let send_one: serde_json::Value = client
        .post(format!("{url}/messages"))
        .json(&serde_json::json!({
            "room_id": "integration-room-a",
            "sender_fingerprint": "ignored-by-server",
            "text": "real store api path"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let send_two: serde_json::Value = client
        .post(format!("{url}/messages"))
        .json(&serde_json::json!({
            "room_id": "integration-room-b",
            "sender_fingerprint": "ignored-by-server",
            "text": "second store path"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let msg_id = send_one["msg_id"].as_str().unwrap().to_string();
    let second_msg_id = send_two["msg_id"].as_str().unwrap().to_string();

    let messages: Vec<serde_json::Value> = client
        .get(format!("{url}/messages/integration-room-a?limit=10"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["msg_id"], msg_id);
    assert_eq!(messages[0]["text"], "real store api path");
    assert_eq!(messages[0]["verified"], true);

    let fetched: serde_json::Value = client
        .get(format!("{url}/messages/id/{msg_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(fetched["msg_id"], msg_id);
    assert_eq!(fetched["room_id"], "integration-room-a");

    let search_results: Vec<serde_json::Value> = client
        .get(format!("{url}/search?q=store&limit=10"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(search_results.len(), 2);
    assert!(search_results.iter().any(|result| result["msg_id"] == msg_id));
    assert!(search_results
        .iter()
        .any(|result| result["msg_id"] == second_msg_id));

    let room_search_results: Vec<serde_json::Value> = client
        .get(format!("{url}/search?q=store&room_id=integration-room-a&limit=10"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(room_search_results.len(), 1);
    assert_eq!(room_search_results[0]["msg_id"], msg_id);

    let delete_status = client
        .delete(format!("{url}/messages/id/{msg_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(delete_status.status(), 204);

    let deleted: serde_json::Value = client
        .get(format!("{url}/messages/id/{msg_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(deleted.is_null());

    let post_delete_results: Vec<serde_json::Value> = client
        .get(format!("{url}/search?q=real&limit=10"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(post_delete_results.is_empty());

    handle.abort();
}

#[tokio::test]
async fn store_api_manages_contacts_and_rooms_via_real_routes() {
    let (url, handle) = start_store_api().await;
    let client = reqwest::Client::new();

    let initial_contacts: Vec<serde_json::Value> = client
        .get(format!("{url}/contacts"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(initial_contacts.is_empty());

    let contact: serde_json::Value = client
        .post(format!("{url}/contacts"))
        .json(&serde_json::json!({
            "fingerprint": "contact-fp",
            "display_name": "Alice",
            "store_address": "ws://127.0.0.1:3200",
            "trust_level": 2,
            "alias": "Trusted Alice"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(contact["fingerprint"], "contact-fp");
    assert_eq!(contact["trust_level"], "verified");

    let contacts: Vec<serde_json::Value> = client
        .get(format!("{url}/contacts"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(contacts.len(), 1);

    let fetched_contact: serde_json::Value = client
        .get(format!("{url}/contacts/contact-fp"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fetched_contact["alias"], "Trusted Alice");

    let room: serde_json::Value = client
        .post(format!("{url}/rooms"))
        .json(&serde_json::json!({
            "name": "integration room",
            "creator_fingerprint": "creator-fp",
            "encrypted": false,
            "room_type": "group",
            "history_visibility": "full"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let room_id = room["room_id"].as_str().unwrap().to_string();
    assert_eq!(room["member_count"], 1);

    let rooms: Vec<serde_json::Value> = client
        .get(format!("{url}/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0]["room_id"], room_id);

    let add_member_status = client
        .post(format!("{url}/rooms/{room_id}/members"))
        .json(&serde_json::json!({
            "actor_fingerprint": "creator-fp",
            "new_member_fingerprint": "alice-fp"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(add_member_status.status(), 200);

    let add_second_member_status = client
        .post(format!("{url}/rooms/{room_id}/members"))
        .json(&serde_json::json!({
            "actor_fingerprint": "creator-fp",
            "new_member_fingerprint": "bob-fp"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(add_second_member_status.status(), 200);

    let room_after_add: serde_json::Value = client
        .get(format!("{url}/rooms/{room_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(room_after_add["member_count"], 3);

    let leave_status = client
        .post(format!("{url}/rooms/{room_id}/leave"))
        .json(&serde_json::json!({
            "member_fingerprint": "alice-fp"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(leave_status.status(), 200);

    let kick_status = client
        .delete(format!(
            "{url}/rooms/{room_id}/members/bob-fp?actor_fingerprint=creator-fp"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(kick_status.status(), 200);

    let room_after_member_changes: serde_json::Value = client
        .get(format!("{url}/rooms/{room_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(room_after_member_changes["member_count"], 1);

    let delete_contact_status = client
        .delete(format!("{url}/contacts/contact-fp"))
        .send()
        .await
        .unwrap();
    assert_eq!(delete_contact_status.status(), 204);

    let remaining_contacts: Vec<serde_json::Value> = client
        .get(format!("{url}/contacts"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(remaining_contacts.is_empty());

    handle.abort();
}
