use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{Json, Response},
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use lattice_core::device::{DeviceError, DeviceManager};
use lattice_core::room;
use lattice_core::traits::search::SearchIndex;
use lattice_core::traits::storage::{Pagination, Storage, TimeRange};
use lattice_crypto::sign;
use lattice_proto::group::{HistoryVisibility, Room, RoomType};
use lattice_proto::identity::{Contact, DeviceInfo, Identity, KeyBundle, TrustLevel};
use lattice_proto::message::{
    envelope::Payload, Envelope, Message, MessageContent, MessageType, PlainPayload,
};

use crate::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/identity", get(get_identity))
        .route("/messages", post(send_message))
        .route("/messages/:room_id", get(get_messages))
        .route("/messages/id/:msg_id", get(get_message))
        .route("/messages/id/:msg_id", delete(delete_message))
        .route("/search", get(search_messages))
        .route("/contacts", get(list_contacts))
        .route("/contacts", post(add_contact))
        .route("/contacts/:fingerprint", get(get_contact))
        .route("/contacts/:fingerprint", delete(delete_contact))
        .route("/rooms", get(list_rooms))
        .route("/rooms", post(create_room))
        .route("/rooms/:room_id", get(get_room))
        .route("/rooms/:room_id/members", post(add_room_member))
        .route(
            "/rooms/:room_id/members/:fingerprint",
            delete(kick_room_member),
        )
        .route("/rooms/:room_id/leave", post(leave_room_handler))
        .route("/keys/one-time", get(get_one_time_keys))
        .route("/keys/generate", post(generate_one_time_keys))
        .route("/keys/bundle", post(upload_key_bundle))
        .route("/keys/claim/:fingerprint", get(claim_one_time_key))
        .route("/devices", post(register_device))
        .route("/devices/:user_fingerprint", get(list_devices))
        .route("/media", post(upload_media))
        .route("/media/:media_id", get(download_media))
        // 鉴权中间件：写操作（POST/DELETE）需 Bearer token，只读（GET）放行。
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state)
}

/// 写操作鉴权：对 POST/DELETE 等修改性请求校验 `Authorization: Bearer <token>`。
/// GET/HEAD/OPTIONS 等只读/预检请求放行（health、identity、消息读取、搜索等公开）。
async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    use axum::http::Method;

    let is_write = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if !is_write {
        return Ok(next.run(request).await);
    }

    // 未配置 token 视为未启用鉴权（仅测试场景），放行以保持兼容。
    if state.api_token.is_empty() {
        return Ok(next.run(request).await);
    }

    let provided = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match provided {
        Some(token) if token == state.api_token => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub async fn serve_api(
    listener: tokio::net::TcpListener,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    let app = router(state);

    tracing::info!("REST API server listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn run_api_server(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    serve_api(listener, state).await
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct IdentityResp {
    fingerprint: String,
    display_name: String,
    ws_addr: String,
    online_count: usize,
}

async fn get_identity(State(state): State<Arc<AppState>>) -> Json<IdentityResp> {
    let online_count = state.online.read().await.len();
    Json(IdentityResp {
        fingerprint: state.fingerprint.clone(),
        display_name: state.display_name.clone(),
        ws_addr: state.ws_addr.clone(),
        online_count,
    })
}

#[derive(Deserialize)]
struct SendMessageReq {
    room_id: String,
    #[allow(dead_code)]
    sender_fingerprint: String,
    text: String,
}

#[derive(Serialize)]
struct SendMessageResp {
    msg_id: String,
}

async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SendMessageReq>,
) -> Result<Json<SendMessageResp>, (StatusCode, String)> {
    let msg_id = uuid::Uuid::new_v4().to_string();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // 构造 Envelope 并签名
    let mut envelope = Envelope {
        msg_id: msg_id.clone(),
        sender_fingerprint: state.fingerprint.clone(),
        recipient_fingerprint: req.room_id.clone(),
        timestamp,
        signature: vec![],
        payload_hash: vec![],
        prev_hashes: Vec::new(),
        payload: Some(Payload::Plain(PlainPayload {
            content: Some(MessageContent {
                r#type: MessageType::Text as i32,
                text: req.text.clone(),
                ..Default::default()
            }),
        })),
    };

    // 用 Store 的私钥签名
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&state.identity.signing_key_bytes());
    match sign::sign_envelope(&signing_key, &envelope) {
        Ok((sig, hash)) => {
            envelope.signature = sig;
            envelope.payload_hash = hash;
        }
        Err(e) => {
            tracing::warn!("Failed to sign message: {e}");
        }
    }

    let msg = Message {
        msg_id: msg_id.clone(),
        room_id: req.room_id,
        sender_fingerprint: state.fingerprint.clone(),
        timestamp,
        content: Some(MessageContent {
            r#type: MessageType::Text as i32,
            text: req.text,
            ..Default::default()
        }),
        encrypted: false,
        verified: true, // 自己签的，自然 verified
        encrypted_payload: None,
        received_ts: timestamp,
        prev_hashes: Vec::new(),
        msg_hash: envelope.payload_hash.clone(),
        redacted: false,
        edited: false,
    };

    state
        .storage
        .save_message(&msg)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = state.search.index_message(&msg).await;

    Ok(Json(SendMessageResp { msg_id }))
}

#[derive(Deserialize)]
struct GetMessagesQuery {
    since: Option<u64>,
    until: Option<u64>,
    limit: Option<u32>,
    offset: Option<u64>,
}

#[derive(Serialize)]
struct MessageResp {
    msg_id: String,
    room_id: String,
    sender_fingerprint: String,
    timestamp: u64,
    text: String,
    encrypted: bool,
    verified: bool,
}

impl From<Message> for MessageResp {
    fn from(m: Message) -> Self {
        Self {
            msg_id: m.msg_id,
            room_id: m.room_id,
            sender_fingerprint: m.sender_fingerprint,
            timestamp: m.timestamp,
            text: m.content.map(|c| c.text).unwrap_or_default(),
            encrypted: m.encrypted,
            verified: m.verified,
        }
    }
}

async fn get_messages(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Query(q): Query<GetMessagesQuery>,
) -> Result<Json<Vec<MessageResp>>, (StatusCode, String)> {
    let range = TimeRange {
        start: q.since.unwrap_or(0),
        end: q.until.unwrap_or(9_999_999_999_999),
    };
    let page = Pagination {
        offset: q.offset.unwrap_or(0),
        limit: q.limit.unwrap_or(50),
    };

    let msgs = state
        .storage
        .get_messages(&room_id, &range, &page)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(msgs.into_iter().map(MessageResp::from).collect()))
}

async fn get_message(
    State(state): State<Arc<AppState>>,
    Path(msg_id): Path<String>,
) -> Result<Json<Option<MessageResp>>, (StatusCode, String)> {
    let msg = state
        .storage
        .get_message(&msg_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(msg.map(MessageResp::from)))
}

async fn delete_message(
    State(state): State<Arc<AppState>>,
    Path(msg_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .storage
        .delete_message(&msg_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = state.search.delete_index(&msg_id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    room_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchResp {
    msg_id: String,
    room_id: String,
    snippet: String,
    score: f32,
    timestamp: u64,
}

async fn search_messages(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResp>>, (StatusCode, String)> {
    let limit = q.limit.unwrap_or(20);

    let results = if let Some(room_id) = &q.room_id {
        state.search.search_in_room(room_id, &q.q, limit).await
    } else {
        state.search.search(&q.q, limit).await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        results
            .into_iter()
            .map(|r| SearchResp {
                msg_id: r.msg_id,
                room_id: r.room_id,
                snippet: r.snippet,
                score: r.score,
                timestamp: r.timestamp,
            })
            .collect(),
    ))
}

// === 联系人管理 ===

#[derive(Deserialize)]
struct AddContactReq {
    fingerprint: String,
    display_name: String,
    store_address: String,
    #[serde(default)]
    trust_level: i32,
    #[serde(default)]
    alias: String,
}

#[derive(Serialize)]
struct ContactResp {
    fingerprint: String,
    display_name: String,
    store_address: String,
    trust_level: String,
    alias: String,
}

impl ContactResp {
    fn from_contact(c: Contact) -> Self {
        let trust = match TrustLevel::try_from(c.trust_level).unwrap_or(TrustLevel::Public) {
            TrustLevel::Public => "public",
            TrustLevel::Tofu => "tofu",
            TrustLevel::Verified => "verified",
        };
        Self {
            fingerprint: c
                .identity
                .as_ref()
                .map(|i| i.fingerprint.clone())
                .unwrap_or_default(),
            display_name: c
                .identity
                .as_ref()
                .map(|i| i.display_name.clone())
                .unwrap_or_default(),
            store_address: c.store_address,
            trust_level: trust.to_string(),
            alias: c.alias,
        }
    }
}

async fn list_contacts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ContactResp>>, (StatusCode, String)> {
    let contacts = state
        .storage
        .get_contacts()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        contacts
            .into_iter()
            .map(ContactResp::from_contact)
            .collect(),
    ))
}

async fn add_contact(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddContactReq>,
) -> Result<Json<ContactResp>, (StatusCode, String)> {
    let contact = Contact {
        identity: Some(Identity {
            fingerprint: req.fingerprint.clone(),
            display_name: req.display_name.clone(),
            ..Default::default()
        }),
        store_address: req.store_address,
        trust_level: req.trust_level,
        alias: req.alias,
        ..Default::default()
    };
    state
        .storage
        .save_contact(&contact)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ContactResp::from_contact(contact)))
}

async fn get_contact(
    State(state): State<Arc<AppState>>,
    Path(fingerprint): Path<String>,
) -> Result<Json<Option<ContactResp>>, (StatusCode, String)> {
    let contact = state
        .storage
        .get_contact(&fingerprint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(contact.map(ContactResp::from_contact)))
}

async fn delete_contact(
    State(state): State<Arc<AppState>>,
    Path(fingerprint): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .storage
        .delete_contact(&fingerprint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// === 房间管理 ===

#[derive(Deserialize)]
struct CreateRoomReq {
    name: String,
    creator_fingerprint: String,
    #[serde(default)]
    encrypted: bool,
    #[serde(default = "default_room_type")]
    room_type: String,
    #[serde(default = "default_history_visibility")]
    history_visibility: String,
}

fn default_room_type() -> String {
    "group".to_string()
}
fn default_history_visibility() -> String {
    "full".to_string()
}

#[derive(Serialize)]
struct RoomResp {
    room_id: String,
    name: String,
    room_type: String,
    creator_fingerprint: String,
    encrypted: bool,
    member_count: usize,
}

impl RoomResp {
    fn from_room(r: &Room) -> Self {
        let rt = match RoomType::try_from(r.r#type).unwrap_or(RoomType::Group) {
            RoomType::Direct => "direct",
            RoomType::Group => "group",
            RoomType::Channel => "channel",
        };
        Self {
            room_id: r.room_id.clone(),
            name: r.name.clone(),
            room_type: rt.to_string(),
            creator_fingerprint: r.creator_fingerprint.clone(),
            encrypted: r.encrypted,
            member_count: r.members.len(),
        }
    }
}

async fn list_rooms(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RoomResp>>, (StatusCode, String)> {
    let rooms = state
        .storage
        .get_rooms()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rooms.iter().map(RoomResp::from_room).collect()))
}

async fn create_room(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRoomReq>,
) -> Result<Json<RoomResp>, (StatusCode, String)> {
    let room_type = match req.room_type.as_str() {
        "direct" => RoomType::Direct,
        "channel" => RoomType::Channel,
        _ => RoomType::Group,
    };
    let visibility = match req.history_visibility.as_str() {
        "join_only" => HistoryVisibility::JoinOnly,
        _ => HistoryVisibility::Full,
    };

    let room_id = uuid::Uuid::new_v4().to_string();
    let new_room = room::create_room(
        &room_id,
        &req.name,
        room_type,
        &req.creator_fingerprint,
        req.encrypted,
        visibility,
    );

    state
        .storage
        .save_room(&new_room)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RoomResp::from_room(&new_room)))
}

async fn get_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
) -> Result<Json<Option<RoomResp>>, (StatusCode, String)> {
    let r = state
        .storage
        .get_room(&room_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(r.as_ref().map(RoomResp::from_room)))
}

#[derive(Deserialize)]
struct AddMemberReq {
    actor_fingerprint: String,
    new_member_fingerprint: String,
}

async fn add_room_member(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(req): Json<AddMemberReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut r = state
        .storage
        .get_room(&room_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "room not found".to_string()))?;

    room::add_member(&mut r, &req.actor_fingerprint, &req.new_member_fingerprint)
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    state
        .storage
        .save_room(&r)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}

async fn kick_room_member(
    State(state): State<Arc<AppState>>,
    Path((room_id, fingerprint)): Path<(String, String)>,
    Query(q): Query<KickQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut r = state
        .storage
        .get_room(&room_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "room not found".to_string()))?;

    room::kick_member(&mut r, &q.actor_fingerprint, &fingerprint)
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    state
        .storage
        .save_room(&r)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct KickQuery {
    actor_fingerprint: String,
}

#[derive(Deserialize)]
struct LeaveReq {
    member_fingerprint: String,
}

async fn leave_room_handler(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(req): Json<LeaveReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut r = state
        .storage
        .get_room(&room_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "room not found".to_string()))?;

    room::leave_room(&mut r, &req.member_fingerprint)
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    state
        .storage
        .save_room(&r)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}

// === E2EE 密钥管理 ===

#[derive(Serialize)]
struct OneTimeKeysResp {
    curve25519_identity_key: String,
    one_time_keys: Vec<String>,
}

async fn get_one_time_keys(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OneTimeKeysResp>, (StatusCode, String)> {
    let account = state.olm_account.lock().await;
    let identity_key = account.curve25519_key();
    let otks = account.one_time_keys();

    Ok(Json(OneTimeKeysResp {
        curve25519_identity_key: base64_encode(identity_key.as_bytes()),
        one_time_keys: otks.iter().map(|k| base64_encode(k.as_bytes())).collect(),
    }))
}

#[derive(Deserialize)]
struct GenerateKeysReq {
    #[serde(default = "default_key_count")]
    count: usize,
}

fn default_key_count() -> usize {
    10
}

async fn generate_one_time_keys(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GenerateKeysReq>,
) -> Result<Json<OneTimeKeysResp>, (StatusCode, String)> {
    let mut account = state.olm_account.lock().await;
    account.generate_one_time_keys(req.count);
    account.mark_keys_as_published();

    let identity_key = account.curve25519_key();
    let otks = account.one_time_keys();

    Ok(Json(OneTimeKeysResp {
        curve25519_identity_key: base64_encode(identity_key.as_bytes()),
        one_time_keys: otks.iter().map(|k| base64_encode(k.as_bytes())).collect(),
    }))
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, (StatusCode, String)> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid base64: {e}")))
}

// === 多设备管理 ===

#[derive(Deserialize)]
struct RegisterDeviceReq {
    device_id: String,
    user_fingerprint: String,
    device_ed25519_key: String,    // base64
    device_curve25519_key: String, // base64
    signature: String,             // base64
    /// 用户主公钥（base64）。Store 据此验证：(1) 其指纹等于 user_fingerprint，
    /// (2) 设备签名确由该主私钥签发。无主私钥者无法伪造，防止冒名注册。
    master_ed25519_key: String,
    #[serde(default)]
    device_name: String,
}

#[derive(Serialize)]
struct DeviceResp {
    device_id: String,
    user_fingerprint: String,
    device_ed25519_key: String,
    device_curve25519_key: String,
    signature: String,
    device_name: String,
    created_at: u64,
}

impl From<&DeviceInfo> for DeviceResp {
    fn from(d: &DeviceInfo) -> Self {
        DeviceResp {
            device_id: d.device_id.clone(),
            user_fingerprint: d.user_fingerprint.clone(),
            device_ed25519_key: base64_encode(&d.device_ed25519_key),
            device_curve25519_key: base64_encode(&d.device_curve25519_key),
            signature: base64_encode(&d.signature),
            device_name: d.device_name.clone(),
            created_at: d.created_at,
        }
    }
}

/// 注册一台设备到当前用户。使用 `DeviceManager` 校验同用户重复注册，
/// 通过后持久化到存储。新设备随后可通过 `GET /devices/:user_fingerprint`
/// 拉取同一用户已注册的设备列表，构成多设备发现的最小闭环。
async fn register_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterDeviceReq>,
) -> Result<Json<DeviceResp>, (StatusCode, String)> {
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let device = DeviceInfo {
        device_id: req.device_id,
        user_fingerprint: req.user_fingerprint.clone(),
        device_ed25519_key: base64_decode(&req.device_ed25519_key)?,
        device_curve25519_key: base64_decode(&req.device_curve25519_key)?,
        signature: base64_decode(&req.signature)?,
        device_name: req.device_name,
        created_at,
    };

    // 身份校验：防止冒名给他人注册设备。
    // (1) 主公钥的指纹必须等于声称的 user_fingerprint（公钥与身份绑定）。
    // (2) 设备签名必须由该主私钥签发（证明请求者持有主私钥）。
    let master_key = base64_decode(&req.master_ed25519_key)?;
    if lattice_crypto::identity::compute_fingerprint(&master_key) != req.user_fingerprint {
        return Err((
            StatusCode::FORBIDDEN,
            "master key fingerprint does not match user_fingerprint".to_string(),
        ));
    }
    match lattice_crypto::identity::verify_device_signature(&master_key, &device) {
        Ok(true) => {}
        _ => {
            return Err((
                StatusCode::FORBIDDEN,
                "device signature not signed by the claimed master key".to_string(),
            ))
        }
    }

    // 用已有设备列表初始化管理器并校验（防止同设备 ID 重复、防止跨用户注册）
    let existing = state
        .storage
        .get_devices(&req.user_fingerprint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut manager = DeviceManager::from_devices(&req.user_fingerprint, existing);
    manager
        .register_device(device.clone())
        .map_err(|e| match e {
            DeviceError::WrongUser => (StatusCode::BAD_REQUEST, e.to_string()),
            DeviceError::AlreadyRegistered(_) => (StatusCode::CONFLICT, e.to_string()),
            DeviceError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        })?;

    state
        .storage
        .save_device(&device)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(DeviceResp::from(&device)))
}

/// 列出指定用户的全部已注册设备（多设备同步：新设备据此发现同账号其他设备）。
async fn list_devices(
    State(state): State<Arc<AppState>>,
    Path(user_fingerprint): Path<String>,
) -> Result<Json<Vec<DeviceResp>>, (StatusCode, String)> {
    let devices = state
        .storage
        .get_devices(&user_fingerprint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(devices.iter().map(DeviceResp::from).collect()))
}

// === 媒体上传/下载(内容寻址)===

#[derive(Deserialize)]
struct UploadMediaQuery {
    #[serde(default)]
    media_type: String,
}

#[derive(Serialize)]
struct UploadMediaResp {
    media_id: String,
    size: usize,
}

/// 上传媒体:media_id = hex(SHA-256(body)),内容寻址天然去重。
/// 写操作,受 auth_middleware 的 Bearer token 保护。
async fn upload_media(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UploadMediaQuery>,
    body: axum::body::Bytes,
) -> Result<Json<UploadMediaResp>, (StatusCode, String)> {
    if body.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty media body".to_string()));
    }
    let media_id = hex_encode(&sign::sha256(&body));
    let media_type = if q.media_type.is_empty() {
        "application/octet-stream".to_string()
    } else {
        q.media_type
    };
    state
        .storage
        .save_media(&media_id, &body, &media_type)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let size = body.len();
    Ok(Json(UploadMediaResp { media_id, size }))
}

/// 下载媒体:media_id 是内容哈希(不可枚举猜测),只读公开。
async fn download_media(
    State(state): State<Arc<AppState>>,
    Path(media_id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let (data, media_type) = state
        .storage
        .get_media(&media_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "media not found".to_string()))?;
    use axum::response::IntoResponse;
    Ok(([(axum::http::header::CONTENT_TYPE, media_type)], data).into_response())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// === E2EE 预密钥包:上传与 claim ===

#[derive(Deserialize)]
struct UploadKeyBundleReq {
    fingerprint: String,
    identity_key: String,       // base64
    one_time_keys: Vec<String>, // base64 列表
    #[serde(default)]
    fallback_key: String, // base64
    #[serde(default)]
    signature: String, // base64
}

/// 上传自己的预密钥包(写操作,受 Bearer token 保护)。
async fn upload_key_bundle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UploadKeyBundleReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut one_time_keys = Vec::with_capacity(req.one_time_keys.len());
    for k in &req.one_time_keys {
        one_time_keys.push(base64_decode(k)?);
    }
    let bundle = KeyBundle {
        fingerprint: req.fingerprint,
        identity_key: base64_decode(&req.identity_key)?,
        one_time_keys,
        fallback_key: if req.fallback_key.is_empty() {
            Vec::new()
        } else {
            base64_decode(&req.fallback_key)?
        },
        signature: if req.signature.is_empty() {
            Vec::new()
        } else {
            base64_decode(&req.signature)?
        },
    };
    state
        .storage
        .save_key_bundle(&bundle)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct ClaimedKeyResp {
    identity_key: String,         // base64
    one_time_key: Option<String>, // base64,消费的 OTK;耗尽则 None
    fallback_key: Option<String>, // base64,OTK 耗尽时回退
}

/// claim 目标用户的一个预密钥:取 identity_key + 消费(弹出)一个 one-time key,
/// 回写减少后的 bundle(防重用)。OTK 耗尽时返回 fallback_key。
async fn claim_one_time_key(
    State(state): State<Arc<AppState>>,
    Path(fingerprint): Path<String>,
) -> Result<Json<ClaimedKeyResp>, (StatusCode, String)> {
    let mut bundle = state
        .storage
        .get_key_bundle(&fingerprint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "no key bundle for user".to_string()))?;

    let one_time_key = if bundle.one_time_keys.is_empty() {
        None
    } else {
        // 消费一个 OTK 并回写
        let otk = bundle.one_time_keys.remove(0);
        state
            .storage
            .save_key_bundle(&bundle)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Some(base64_encode(&otk))
    };
    let fallback_key = if one_time_key.is_none() && !bundle.fallback_key.is_empty() {
        Some(base64_encode(&bundle.fallback_key))
    } else {
        None
    };

    Ok(Json(ClaimedKeyResp {
        identity_key: base64_encode(&bundle.identity_key),
        one_time_key,
        fallback_key,
    }))
}
