mod api;
mod cache;
mod observability;
mod relay;
pub mod stun;

use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use serde::Deserialize;

use cache::RelayCache;
use observability::PeerObservability;

#[derive(Deserialize, Clone)]
pub struct PeerConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    #[serde(default = "default_api_addr")]
    pub api_addr: String,
    #[serde(default = "default_max_cache")]
    pub max_cache_entries: usize,
    #[serde(default = "default_ttl")]
    pub cache_ttl_ms: u64,
    #[serde(default)]
    pub proxy_stores: Vec<String>,
    #[serde(default = "default_eviction_interval")]
    pub eviction_interval_ms: u64,
}

fn default_listen_addr() -> String {
    "127.0.0.1:9200".to_string()
}
fn default_api_addr() -> String {
    "127.0.0.1:9201".to_string()
}
fn default_max_cache() -> usize {
    10_000
}
fn default_ttl() -> u64 {
    3_600_000
}
fn default_eviction_interval() -> u64 {
    60_000
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            api_addr: default_api_addr(),
            max_cache_entries: default_max_cache(),
            cache_ttl_ms: default_ttl(),
            proxy_stores: vec![],
            eviction_interval_ms: default_eviction_interval(),
        }
    }
}

/// 定期检查超时消息并转投代收 Store
async fn run_eviction_loop(
    cache: Arc<Mutex<RelayCache>>,
    proxy_stores: Vec<String>,
    interval_ms: u64,
) {
    use futures_util::{SinkExt, StreamExt};
    use nextim_proto::transport::Frame;
    use prost::Message as ProstMessage;
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    let interval = std::time::Duration::from_millis(interval_ms);

    loop {
        tokio::time::sleep(interval).await;

        let expired = {
            let mut c = cache.lock().await;
            c.drain_expired()
        };

        if expired.is_empty() {
            continue;
        }

        tracing::info!("Evicting {} expired messages", expired.len());

        if proxy_stores.is_empty() {
            tracing::warn!(
                "No proxy stores configured, re-queuing {} expired messages",
                expired.len()
            );
            // 无代收 Store:回写缓存,避免静默丢失(等配置好 proxy 后下个周期重试)。
            let mut c = cache.lock().await;
            for (recipient, data) in expired {
                c.store(&recipient, data);
            }
            continue;
        }

        // 逐条转投;只有收到 ACK 才算投递成功,其余(连不上/无 ACK)回写缓存重试,防丢失。
        let mut remaining: Vec<(String, Vec<u8>)> = expired;
        for proxy_addr in &proxy_stores {
            if remaining.is_empty() {
                break;
            }
            match connect_async(proxy_addr).await {
                Ok((mut ws, _)) => {
                    let mut still_pending = Vec::new();
                    let mut delivered = 0usize;
                    for (recipient, data) in remaining {
                        let mut acked = false;
                        if ws.send(WsMessage::Binary(data.clone())).await.is_ok() {
                            if let Some(Ok(WsMessage::Binary(ack_data))) = ws.next().await {
                                if let Ok(ack) = Frame::decode(ack_data.as_ref()) {
                                    if ack.r#type == nextim_proto::transport::FrameType::Ack as i32
                                    {
                                        acked = true;
                                    }
                                }
                            }
                        }
                        if acked {
                            delivered += 1;
                        } else {
                            still_pending.push((recipient, data));
                        }
                    }
                    ws.close(None).await.ok();
                    tracing::info!(
                        "Forwarded {delivered} expired messages to {proxy_addr}, {} still pending",
                        still_pending.len()
                    );
                    remaining = still_pending;
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to proxy store {proxy_addr}: {e}");
                    // 连不上:remaining 不变,尝试下一个 proxy
                }
            }
        }

        // 仍未成功投递的消息回写缓存,下个周期重试(防静默丢失)。
        if !remaining.is_empty() {
            tracing::warn!(
                "Re-queuing {} undelivered expired messages for retry",
                remaining.len()
            );
            let mut c = cache.lock().await;
            for (recipient, data) in remaining {
                c.store(&recipient, data);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("nextim=info".parse()?))
        .init();

    let config = match std::fs::read_to_string("nextim-peer.toml") {
        Ok(content) => {
            let c: PeerConfig = toml::from_str(&content).unwrap_or_default();
            tracing::info!("Loaded config from nextim-peer.toml");
            c
        }
        Err(_) => {
            tracing::info!("No config file found, using defaults");
            PeerConfig::default()
        }
    };
    let cache = Arc::new(Mutex::new(RelayCache::new(
        config.max_cache_entries,
        config.cache_ttl_ms,
    )));
    let observability = Arc::new(Mutex::new(PeerObservability::default()));

    tracing::info!("NextIM Peer relay starting on {}", config.listen_addr);
    tracing::info!("REST API server starting on {}", config.api_addr);

    // 启动超时转投任务
    let eviction_cache = cache.clone();
    let proxy_stores = config.proxy_stores.clone();
    let eviction_interval = config.eviction_interval_ms;
    tokio::spawn(run_eviction_loop(
        eviction_cache,
        proxy_stores,
        eviction_interval,
    ));

    // 启动 REST API 服务
    let api_cache = cache.clone();
    let api_config = config.clone();
    let api_addr = config.api_addr.clone();
    let api_observability = observability.clone();
    tokio::spawn(async move {
        if let Err(e) =
            api::run_api_server(api_addr, api_cache, api_config, api_observability).await
        {
            tracing::error!("API server error: {e}");
        }
    });

    // 启动中转服务
    relay::run_relay_server(config.listen_addr, cache, observability).await?;

    Ok(())
}
