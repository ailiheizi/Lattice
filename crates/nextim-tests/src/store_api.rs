//! Store REST API integration tests.
//!
//! These tests exercise the real `nextim-store` production API server on
//! ephemeral ports.

use std::collections::HashMap;
use std::sync::Arc;

use nextim_crypto::{
    identity::{DeviceKeyPair, MasterKeyPair},
    olm::OlmAccount,
};
use nextim_storage::{sqlite::SqliteStorage, tantivy_search::TantivySearch};
use tokio::sync::{Mutex, RwLock};

use base64::Engine;

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
        api_token: String::new(),
        allow_unsigned: true,
        enable_dht: false,
        dht_bootstrap: Vec::new(),
        require_contact: false,
        rate_limiter: Mutex::new(nextim_core::rate_limiter::RateLimiter::new(60_000, 0)),
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
    assert!(
        keys_before["curve25519_identity_key"]
            .as_str()
            .unwrap()
            .len()
            > 10
    );
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
    assert!(search_results
        .iter()
        .any(|result| result["msg_id"] == msg_id));
    assert!(search_results
        .iter()
        .any(|result| result["msg_id"] == second_msg_id));

    let room_search_results: Vec<serde_json::Value> = client
        .get(format!(
            "{url}/search?q=store&room_id=integration-room-a&limit=10"
        ))
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

#[tokio::test]
async fn store_api_registers_and_lists_devices_via_real_routes() {
    let (url, handle) = start_store_api().await;
    let client = reqwest::Client::new();

    // 用真实主密钥；user_fingerprint 必须等于主公钥指纹。
    let master = MasterKeyPair::generate();
    let user = master.fingerprint();
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let master_b64 = b64(master.verifying_key().as_bytes());

    // 构造一台由主密钥签名的设备注册请求体。
    let make_device_req = |master: &MasterKeyPair, device_id: &str, name: &str| {
        let dk = DeviceKeyPair::generate();
        let info = master.sign_device(
            device_id,
            name,
            &dk.verifying_key(),
            &dk.encryption_public_key(),
        );
        serde_json::json!({
            "device_id": device_id,
            "user_fingerprint": master.fingerprint(),
            "device_ed25519_key": b64(&info.device_ed25519_key),
            "device_curve25519_key": b64(&info.device_curve25519_key),
            "signature": b64(&info.signature),
            "master_ed25519_key": b64(master.verifying_key().as_bytes()),
            "device_name": name,
        })
    };

    // 初始无设备
    let empty: Vec<serde_json::Value> = client
        .get(format!("{url}/devices/{user}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(empty.is_empty());

    // 合法注册第一台设备
    let phone: serde_json::Value = client
        .post(format!("{url}/devices"))
        .json(&make_device_req(&master, "phone-1", "Alice Phone"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(phone["device_id"], "phone-1");
    assert_eq!(phone["device_name"], "Alice Phone");
    assert!(phone["created_at"].as_u64().unwrap() > 0);

    // 合法注册第二台设备
    let laptop_status = client
        .post(format!("{url}/devices"))
        .json(&make_device_req(&master, "laptop-1", "Alice Laptop"))
        .send()
        .await
        .unwrap();
    assert_eq!(laptop_status.status(), 200);

    // 新设备发现同账号已注册设备：列表返回两台
    let devices: Vec<serde_json::Value> = client
        .get(format!("{url}/devices/{user}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(devices.len(), 2);
    assert!(devices.iter().any(|d| d["device_id"] == "phone-1"));
    assert!(devices.iter().any(|d| d["device_id"] == "laptop-1"));

    // 重复 device_id → 409 Conflict
    let duplicate = client
        .post(format!("{url}/devices"))
        .json(&make_device_req(&master, "phone-1", "dup"))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), 409);

    // 冒名攻击：用攻击者自己的主密钥签名，但声称是受害者 user_fingerprint → 403
    let attacker = MasterKeyPair::generate();
    let dk = DeviceKeyPair::generate();
    let forged = attacker.sign_device(
        "evil-1",
        "evil",
        &dk.verifying_key(),
        &dk.encryption_public_key(),
    );
    let impersonation = client
        .post(format!("{url}/devices"))
        .json(&serde_json::json!({
            "device_id": "evil-1",
            "user_fingerprint": user,                       // 声称是受害者
            "device_ed25519_key": b64(&forged.device_ed25519_key),
            "device_curve25519_key": b64(&forged.device_curve25519_key),
            "signature": b64(&forged.signature),
            "master_ed25519_key": master_b64.clone(),       // 受害者真公钥，但签名不是它签的
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        impersonation.status(),
        403,
        "forged device signature must be rejected"
    );

    // 主公钥与声称指纹不符 → 403
    let mismatch = client
        .post(format!("{url}/devices"))
        .json(&serde_json::json!({
            "device_id": "x-1",
            "user_fingerprint": user,
            "device_ed25519_key": b64(&forged.device_ed25519_key),
            "device_curve25519_key": b64(&forged.device_curve25519_key),
            "signature": b64(&forged.signature),
            "master_ed25519_key": b64(attacker.verifying_key().as_bytes()), // 公钥指纹≠user
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        mismatch.status(),
        403,
        "master key not matching fingerprint must be rejected"
    );

    handle.abort();
}

async fn start_store_api_with_token(token: &str) -> (String, tokio::task::JoinHandle<()>) {
    let storage = SqliteStorage::in_memory().expect("in-memory storage");
    let search = TantivySearch::in_memory().expect("in-memory search");
    let identity = MasterKeyPair::generate();
    let fingerprint = identity.fingerprint();
    let mut olm = OlmAccount::new();
    olm.generate_one_time_keys(5);
    olm.mark_keys_as_published();

    let state = Arc::new(AppState {
        storage,
        search,
        online: Arc::new(RwLock::new(HashMap::new())) as OnlineConnections,
        outbound: Arc::new(RwLock::new(HashMap::new())) as OutboundPool,
        identity,
        olm_account: Mutex::new(olm),
        fingerprint,
        display_name: "Auth Test Store".to_string(),
        ws_addr: "127.0.0.1:0".to_string(),
        api_token: token.to_string(),
        allow_unsigned: true,
        enable_dht: false,
        dht_bootstrap: Vec::new(),
        require_contact: false,
        rate_limiter: Mutex::new(nextim_core::rate_limiter::RateLimiter::new(60_000, 0)),
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://127.0.0.1:{}", addr.port());
    let handle = tokio::spawn(async move {
        api::serve_api(listener, state).await.unwrap();
    });
    (url, handle)
}

#[tokio::test]
async fn write_endpoints_require_bearer_token() {
    let token = "secret-admin-token";
    let (url, handle) = start_store_api_with_token(token).await;
    let client = reqwest::Client::new();

    // 只读端点无需 token：放行。
    let health = client.get(format!("{url}/health")).send().await.unwrap();
    assert_eq!(health.status(), 200);
    let identity = client.get(format!("{url}/identity")).send().await.unwrap();
    assert_eq!(identity.status(), 200);

    // 写端点无 token → 401。
    let no_token = client
        .post(format!("{url}/rooms"))
        .json(&serde_json::json!({
            "name": "x", "creator_fingerprint": "fp", "room_type": "Group"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(no_token.status(), 401);

    // 写端点错 token → 401。
    let bad_token = client
        .post(format!("{url}/rooms"))
        .header("Authorization", "Bearer wrong-token")
        .json(&serde_json::json!({
            "name": "x", "creator_fingerprint": "fp", "room_type": "Group"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_token.status(), 401);

    // 写端点有效 token → 通过（非 401）。
    let good_token = client
        .post(format!("{url}/rooms"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "name": "authorized room", "creator_fingerprint": "fp", "room_type": "Group"
        }))
        .send()
        .await
        .unwrap();
    assert_ne!(good_token.status(), 401, "valid token must not be rejected");

    // DELETE 无 token → 401。
    let delete_no_token = client
        .delete(format!("{url}/messages/id/whatever"))
        .send()
        .await
        .unwrap();
    assert_eq!(delete_no_token.status(), 401);

    handle.abort();
}

#[tokio::test]
async fn media_upload_download_roundtrip_and_dedup() {
    let (url, handle) = start_store_api().await;
    let client = reqwest::Client::new();
    let payload = b"\x89PNG\r\n\x1a\n fake image bytes".to_vec();

    // 上传
    let resp: serde_json::Value = client
        .post(format!("{url}/media?media_type=image/png"))
        .body(payload.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let media_id = resp["media_id"].as_str().unwrap().to_string();
    assert_eq!(resp["size"].as_u64().unwrap() as usize, payload.len());
    assert_eq!(media_id.len(), 64, "media_id is hex SHA-256");

    // 下载,内容与 content-type 一致
    let dl = client
        .get(format!("{url}/media/{media_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(dl.status(), 200);
    assert_eq!(dl.headers().get("content-type").unwrap(), "image/png");
    let body = dl.bytes().await.unwrap();
    assert_eq!(body.as_ref(), payload.as_slice());

    // 内容寻址去重:重复上传同内容 → 相同 media_id
    let resp2: serde_json::Value = client
        .post(format!("{url}/media"))
        .body(payload.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp2["media_id"].as_str().unwrap(), media_id);

    // 不存在的 media_id → 404
    let missing = client
        .get(format!("{url}/media/{}", "0".repeat(64)))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);

    handle.abort();
}
