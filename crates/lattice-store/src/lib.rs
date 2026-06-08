use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};

use lattice_crypto::identity::MasterKeyPair;
use lattice_crypto::olm::OlmAccount;
use lattice_storage::sqlite::SqliteStorage;
use lattice_storage::tantivy_search::TantivySearch;

pub mod api;
pub mod server;

// === 可插拔后端(简单方式)===
// 当前后端通过类型别名 + 工厂函数集中,作为唯一扩展点。
// 增加新后端时:把 alias 改成 enum(为其 impl 对应 trait),工厂按配置选;
// 调用方(AppState/测试)用 alias 不变。现在只有 sqlite/tantivy 一种实现。
/// 当前生效的存储后端实现。
pub type ActiveStorage = SqliteStorage;
/// 当前生效的搜索后端实现。
pub type ActiveSearch = TantivySearch;

/// 按配置构造存储后端(可插拔扩展点)。
pub fn build_storage(backend: &str, db_path: &std::path::Path) -> Result<ActiveStorage> {
    match backend {
        "" | "sqlite" => SqliteStorage::open(db_path).map_err(|e| anyhow::anyhow!("{e}")),
        other => Err(anyhow::anyhow!("unknown storage_backend: {other}")),
    }
}

/// 在线连接类型
pub type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    tokio_tungstenite::tungstenite::Message,
>;

/// 在线连接映射：fingerprint → WebSocket sink
pub type OnlineConnections = Arc<RwLock<HashMap<String, Arc<Mutex<WsSink>>>>>;

/// 出站连接池：Store 地址 → WebSocket sink
pub type OutboundPool = Arc<RwLock<HashMap<String, Arc<Mutex<WsSink>>>>>;

#[derive(Deserialize)]
pub struct StoreConfig {
    #[serde(default = "default_ws_addr")]
    pub ws_addr: String,
    #[serde(default = "default_api_addr")]
    pub api_addr: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub proxy_store_address: String,
    #[serde(default)]
    pub display_name: String,
    /// REST 管理接口的 Bearer token；留空则启动时自动生成。
    #[serde(default)]
    pub api_token: String,
    /// 是否接受无签名消息/房间事件。默认 false（强制验签）。
    /// 仅调试或旧协议迁移时显式开启，生产务必保持 false。
    #[serde(default)]
    pub allow_unsigned: bool,
    /// 是否启用 DHT 节点发现。默认 false。开启时联系人 store_address 仍是主路径，
    /// DHT 仅在 store_address 缺失时作 fallback。
    #[serde(default)]
    pub enable_dht: bool,
    /// DHT WebSocket 监听地址（enable_dht 时生效）。
    #[serde(default = "default_dht_addr")]
    pub dht_addr: String,
    /// DHT 引导节点地址列表：启动时向它们发布自己的身份卡片、查询时向它们 lookup。
    #[serde(default)]
    pub dht_bootstrap: Vec<String>,
    /// 防骚扰准入:开启后,非联系人(不在联系人表)的消息一律拒绝。
    /// 体现"对方同意(加为联系人)后才能通信"。默认 false 保持兼容。
    #[serde(default)]
    pub require_contact: bool,
    /// 每个发送者每分钟最大消息数(防轰炸/刷屏)。0 表示不限流(默认)。
    #[serde(default)]
    pub rate_limit_per_min: u32,
    /// 存储后端(可插拔):"sqlite"(默认)。将来加新后端在此选。
    #[serde(default)]
    pub storage_backend: String,
}

fn default_dht_addr() -> String {
    "127.0.0.1:9102".to_string()
}

fn default_ws_addr() -> String {
    "127.0.0.1:9100".to_string()
}
fn default_api_addr() -> String {
    "127.0.0.1:9101".to_string()
}
fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            ws_addr: default_ws_addr(),
            api_addr: default_api_addr(),
            data_dir: default_data_dir(),
            proxy_store_address: String::new(),
            display_name: String::new(),
            api_token: String::new(),
            allow_unsigned: false,
            enable_dht: false,
            dht_addr: default_dht_addr(),
            dht_bootstrap: Vec::new(),
            require_contact: false,
            rate_limit_per_min: 0,
            storage_backend: String::new(),
        }
    }
}

impl StoreConfig {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: StoreConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

pub struct AppState {
    pub storage: ActiveStorage,
    pub search: ActiveSearch,
    pub online: OnlineConnections,
    pub outbound: OutboundPool,
    pub identity: MasterKeyPair,
    pub olm_account: Mutex<OlmAccount>,
    pub fingerprint: String,
    pub display_name: String,
    pub ws_addr: String,
    /// REST 管理接口的 Bearer token。空字符串表示未启用鉴权（仅应在测试中出现）。
    pub api_token: String,
    /// 是否接受无签名消息/房间事件。默认 false（强制验签）。
    pub allow_unsigned: bool,
    /// 是否启用 DHT fallback（store_address 缺失时才查 DHT）。
    pub enable_dht: bool,
    /// DHT 引导节点地址，转发缺地址时向它们 lookup。
    pub dht_bootstrap: Vec<String>,
    /// 防骚扰准入:开启后非联系人消息被拒。
    pub require_contact: bool,
    /// 每发送者限流器(防轰炸)。
    pub rate_limiter: Mutex<lattice_core::rate_limiter::RateLimiter>,
}

pub async fn run() -> Result<()> {
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("lattice=info".parse()?))
        .init();

    // 尝试加载配置文件，找不到则用默认值
    let config = match StoreConfig::load("lattice-store.toml") {
        Ok(c) => {
            tracing::info!("Loaded config from lattice-store.toml");
            c
        }
        Err(_) => {
            tracing::info!("No config file found, using defaults");
            StoreConfig::default()
        }
    };

    // 初始化存储
    std::fs::create_dir_all(&config.data_dir)?;
    let db_path = config.data_dir.join("store.db");
    let index_path = config.data_dir.join("search_index");

    let storage = build_storage(&config.storage_backend, &db_path)?;
    let search = TantivySearch::open(&index_path).map_err(|e| anyhow::anyhow!("{e}"))?;

    // 初始化身份密钥（从文件加载或新建）
    let key_path = config.data_dir.join("identity.key");
    let identity = if key_path.exists() {
        let data = std::fs::read(&key_path)?;
        if data.len() == 64 {
            let signing: [u8; 32] = data[..32].try_into().unwrap();
            let encryption: [u8; 32] = data[32..].try_into().unwrap();
            let kp = MasterKeyPair::from_bytes(&signing, &encryption);
            tracing::info!("Loaded identity: {}", kp.fingerprint());
            kp
        } else {
            let kp = MasterKeyPair::generate();
            let mut key_data = Vec::with_capacity(64);
            key_data.extend_from_slice(&kp.signing_key_bytes());
            key_data.extend_from_slice(&kp.encryption_key_bytes());
            std::fs::write(&key_path, &key_data)?;
            tracing::info!("Generated new identity: {}", kp.fingerprint());
            kp
        }
    } else {
        let kp = MasterKeyPair::generate();
        let mut key_data = Vec::with_capacity(64);
        key_data.extend_from_slice(&kp.signing_key_bytes());
        key_data.extend_from_slice(&kp.encryption_key_bytes());
        std::fs::write(&key_path, &key_data)?;
        tracing::info!("Generated new identity: {}", kp.fingerprint());
        kp
    };

    let fingerprint = identity.fingerprint();
    let display_name = if config.display_name.is_empty() {
        format!("Store-{}", &fingerprint[..8])
    } else {
        config.display_name.clone()
    };

    // 初始化 Olm 账户（用于 E2EE 密钥交换）
    let mut olm_account = OlmAccount::new();
    olm_account.generate_one_time_keys(10);
    olm_account.mark_keys_as_published();
    tracing::info!("Olm account ready with 10 one-time keys");

    // REST 管理接口的 Bearer token：配置提供则用，否则随机生成并打印一次。
    let api_token = if config.api_token.is_empty() {
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        tracing::warn!(
            "No api_token in config; generated one for this session: {token}\n  \
             Set `api_token` in lattice-store.toml to persist it across restarts."
        );
        token
    } else {
        config.api_token.clone()
    };

    let state = Arc::new(AppState {
        storage,
        search,
        online: Arc::new(RwLock::new(HashMap::new())),
        outbound: Arc::new(RwLock::new(HashMap::new())),
        fingerprint: fingerprint.clone(),
        display_name: display_name.clone(),
        ws_addr: config.ws_addr.clone(),
        identity,
        olm_account: Mutex::new(olm_account),
        api_token,
        allow_unsigned: config.allow_unsigned,
        enable_dht: config.enable_dht,
        dht_bootstrap: config.dht_bootstrap.clone(),
        require_contact: config.require_contact,
        rate_limiter: Mutex::new(lattice_core::rate_limiter::RateLimiter::new(
            60_000,
            config.rate_limit_per_min,
        )),
    });

    // DHT 节点发现（可选）：启动本地 discovery 服务，并向引导节点发布自己的签名身份卡片。
    // 联系人 store_address 仍是主路径，DHT 仅作 fallback（见 server.rs try_forward_message）。
    if config.enable_dht {
        use lattice_proto::discovery::IdentityCard;
        let mut card = IdentityCard {
            fingerprint: fingerprint.clone(),
            display_name: display_name.clone(),
            ed25519_public_key: state.identity.verifying_key().as_bytes().to_vec(),
            curve25519_public_key: state.identity.encryption_public_key().as_bytes().to_vec(),
            store_address: config.ws_addr.clone(),
            proxy_store_address: config.proxy_store_address.clone(),
            signature: Vec::new(),
        };
        let signing = ed25519_dalek::SigningKey::from_bytes(&state.identity.signing_key_bytes());
        card.signature = lattice_crypto::sign::sign_identity_card(&signing, &card);

        // 启动本地 DHT 服务
        let dht_store = std::sync::Arc::new(tokio::sync::Mutex::new(
            lattice_discovery::dht::DhtStore::new(
                lattice_discovery::dht::NodeId::from_data(fingerprint.as_bytes()),
                20,
            ),
        ));
        let dht_addr = config.dht_addr.clone();
        let dht_store_server = dht_store.clone();
        tokio::spawn(async move {
            if let Err(e) =
                lattice_discovery::service::run_server(&dht_addr, dht_store_server).await
            {
                tracing::warn!("DHT server exited: {e}");
            }
        });

        // 向引导节点发布自己
        for bootstrap in &config.dht_bootstrap {
            let bootstrap = bootstrap.clone();
            let card = card.clone();
            tokio::spawn(async move {
                match lattice_discovery::service::publish_to(&bootstrap, &card).await {
                    Ok(()) => {
                        tracing::info!("Published identity card to DHT bootstrap {bootstrap}")
                    }
                    Err(e) => tracing::warn!("Failed to publish to DHT bootstrap {bootstrap}: {e}"),
                }
            });
        }
        tracing::info!(
            "  DHT:         ws://{} (bootstrap: {:?})",
            config.dht_addr,
            config.dht_bootstrap
        );
    }

    tracing::info!("Lattice Store starting...");
    tracing::info!("  Fingerprint: {fingerprint}");
    tracing::info!("  Display:     {display_name}");
    tracing::info!("  WebSocket:   ws://{}", config.ws_addr);
    tracing::info!("  REST API:    http://{}", config.api_addr);

    // 启动 WebSocket 服务和 REST API
    let ws_state = state.clone();
    let ws_handle = tokio::spawn(server::run_ws_server(config.ws_addr, ws_state));
    let api_handle = tokio::spawn(api::run_api_server(config.api_addr, state));

    tokio::select! {
        r = ws_handle => r??,
        r = api_handle => r??,
    }

    Ok(())
}
