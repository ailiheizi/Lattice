use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use serde::Serialize;
use tokio::sync::Mutex;
use tower_http::cors::{CorsLayer, Any};
use tower_http::services::ServeDir;

use crate::cache::{RelayCache, CacheStats, CacheEntryInfo};
use crate::observability::{ConnectionHistory, ConnectionInfo, SharedPeerObservability};
use crate::PeerConfig;

#[derive(Clone)]
pub struct ApiState {
    cache: Arc<Mutex<RelayCache>>,
    config: PeerConfig,
    start_time: u64,
    observability: SharedPeerObservability,
}

fn build_api_app(state: ApiState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/stats", get(get_stats))
        .route("/connections", get(get_connections))
        .route("/cache", get(get_cache))
        .route("/config", get(get_config))
        .nest_service("/", ServeDir::new("web/peer-monitor"))
        .layer(cors)
        .with_state(state)
}

pub async fn run_api_server(
    addr: String,
    cache: Arc<Mutex<RelayCache>>,
    config: PeerConfig,
    observability: SharedPeerObservability,
) -> anyhow::Result<()> {
    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let state = ApiState {
        cache,
        config,
        start_time,
        observability,
    };

    let app = build_api_app(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Peer API server listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct StatsResponse {
    cached_messages: usize,
    total_relayed: u64,
    total_delivered: u64,
    avg_latency_ms: f64,
    active_connections: usize,
    error_count: u64,
    uptime_seconds: u64,
}

async fn get_stats(State(state): State<ApiState>) -> Json<StatsResponse> {
    let cache = state.cache.lock().await;
    let cache_stats = cache.stats();
    let cache_entries = cache.get_all_entries();
    drop(cache);

    let observability = state.observability.lock().await;
    let snapshot = observability.snapshot();
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        - state.start_time;

    Json(StatsResponse {
        cached_messages: cache_stats.total_entries,
        total_relayed: snapshot.total_relayed,
        total_delivered: snapshot.total_delivered,
        avg_latency_ms: average_active_cache_age_ms(&cache_entries),
        active_connections: snapshot.active.len(),
        error_count: snapshot.error_count,
        uptime_seconds: uptime,
    })
}

#[derive(Serialize)]
struct ConnectionsResponse {
    active: Vec<ConnectionInfo>,
    history: Vec<ConnectionHistory>,
}

async fn get_connections(State(state): State<ApiState>) -> Json<ConnectionsResponse> {
    let observability = state.observability.lock().await;
    let snapshot = observability.snapshot();

    Json(ConnectionsResponse {
        active: snapshot.active,
        history: snapshot.history,
    })
}

#[derive(Serialize)]
struct CacheResponse {
    stats: CacheStats,
    entries: Vec<CacheEntryInfo>,
}

async fn get_cache(State(state): State<ApiState>) -> Json<CacheResponse> {
    let cache = state.cache.lock().await;
    let stats = cache.stats();
    let entries = cache.get_all_entries();
    drop(cache);

    Json(CacheResponse { stats, entries })
}

#[derive(Serialize)]
struct ConfigResponse {
    listen_addr: String,
    api_addr: String,
    max_cache_entries: usize,
    cache_ttl_ms: u64,
    eviction_interval_ms: u64,
    proxy_stores: Vec<String>,
}

async fn get_config(State(state): State<ApiState>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        listen_addr: state.config.listen_addr.clone(),
        api_addr: state.config.api_addr.clone(),
        max_cache_entries: state.config.max_cache_entries,
        cache_ttl_ms: state.config.cache_ttl_ms,
        eviction_interval_ms: state.config.eviction_interval_ms,
        proxy_stores: state.config.proxy_stores.clone(),
    })
}

fn average_active_cache_age_ms(entries: &[CacheEntryInfo]) -> f64 {
    let mut total_age_ms = 0u64;
    let mut active_entries = 0u64;

    for entry in entries {
        if !entry.is_expired {
            total_age_ms += entry.age_ms;
            active_entries += 1;
        }
    }

    if active_entries == 0 {
        0.0
    } else {
        total_age_ms as f64 / active_entries as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::PeerObservability;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde::de::DeserializeOwned;
    use serde_json::Value;
    use tower::util::ServiceExt;

    fn test_state(cache: RelayCache, observability: PeerObservability) -> ApiState {
        ApiState {
            cache: Arc::new(Mutex::new(cache)),
            config: PeerConfig::default(),
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            observability: Arc::new(Mutex::new(observability)),
        }
    }

    async fn read_json<T>(response: axum::response::Response) -> T
    where
        T: DeserializeOwned,
    {
        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        let body = to_bytes(body, usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn stats_endpoint_derives_latency_from_cache_entries() {
        let mut cache = RelayCache::new(10, 1_000);
        cache.store("alice", b"first".to_vec());
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        cache.store("bob", b"second".to_vec());
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let app = build_api_app(test_state(cache, PeerObservability::default()));

        let stats: Value = read_json(
            app.clone()
                .oneshot(Request::get("/stats").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;

        let cache_response: Value = read_json(
            app.oneshot(Request::get("/cache").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;

        let active_entries = cache_response["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| !entry["is_expired"].as_bool().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(stats["cached_messages"], active_entries.len());
        assert_eq!(active_entries.len(), 2);

        let expected_avg_latency_ms = active_entries
            .iter()
            .map(|entry| entry["age_ms"].as_u64().unwrap() as f64)
            .sum::<f64>()
            / active_entries.len() as f64;
        let avg_latency_ms = stats["avg_latency_ms"].as_f64().unwrap();
        assert!((avg_latency_ms - expected_avg_latency_ms).abs() <= 2.0);
    }

    #[tokio::test]
    async fn connections_endpoint_reports_active_and_history() {
        let mut observability = PeerObservability::default();
        let active_id = observability.register_connection("127.0.0.1:9000".to_string());
        observability.record_connection_message(&active_id);
        let closed_id = observability.register_connection("127.0.0.1:9001".to_string());
        observability.record_connection_message(&closed_id);
        observability.unregister_connection(&closed_id);

        let response: Value = read_json(
            build_api_app(test_state(RelayCache::new(10, 60_000), observability))
                .oneshot(Request::get("/connections").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;

        let active = response["active"].as_array().unwrap();
        let history = response["history"].as_array().unwrap();

        assert_eq!(active.len(), 1);
        assert_eq!(active[0]["id"], "conn-1");
        assert_eq!(active[0]["remote_addr"], "127.0.0.1:9000");
        assert_eq!(active[0]["message_count"], 1);
        assert_eq!(active[0]["status"], "connected");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["remote_addr"], "127.0.0.1:9001");
        assert_eq!(history[0]["message_count"], 1);
        assert!(history[0]["disconnected_at"].as_u64().unwrap()
            >= history[0]["connected_at"].as_u64().unwrap());
    }

    #[tokio::test]
    async fn cache_endpoint_reports_stats_and_entries() {
        let mut cache = RelayCache::new(3, 40);
        cache.store("alice", b"stale".to_vec());
        cache.store("bob", b"fresh".to_vec());
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        let response: Value = read_json(
            build_api_app(test_state(cache, PeerObservability::default()))
                .oneshot(Request::get("/cache").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;

        assert_eq!(response["stats"]["total_entries"], 2);
        assert_eq!(response["stats"]["active_entries"], 0);
        assert_eq!(response["stats"]["expired_entries"], 2);
        assert_eq!(response["stats"]["unique_recipients"], 2);
        assert_eq!(response["stats"]["max_entries"], 3);
        assert_eq!(response["stats"]["ttl_ms"], 40);

        let entries = response["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| entry["is_expired"] == true));
        assert!(entries.iter().any(|entry| {
            entry["recipient_fingerprint"] == "alice" && entry["size_bytes"] == 5
        }));
        assert!(entries.iter().any(|entry| {
            entry["recipient_fingerprint"] == "bob" && entry["size_bytes"] == 5
        }));
    }

    #[tokio::test]
    async fn config_endpoint_returns_runtime_configuration() {
        let state = ApiState {
            cache: Arc::new(Mutex::new(RelayCache::new(25, 90_000))),
            config: PeerConfig {
                listen_addr: "0.0.0.0:9200".to_string(),
                api_addr: "127.0.0.1:9300".to_string(),
                max_cache_entries: 25,
                cache_ttl_ms: 90_000,
                proxy_stores: vec![
                    "ws://store-a.example/ws".to_string(),
                    "ws://store-b.example/ws".to_string(),
                ],
                eviction_interval_ms: 15_000,
            },
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            observability: Arc::new(Mutex::new(PeerObservability::default())),
        };

        let response: Value = read_json(
            build_api_app(state)
                .oneshot(Request::get("/config").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;

        assert_eq!(response["listen_addr"], "0.0.0.0:9200");
        assert_eq!(response["api_addr"], "127.0.0.1:9300");
        assert_eq!(response["max_cache_entries"], 25);
        assert_eq!(response["cache_ttl_ms"], 90_000);
        assert_eq!(response["eviction_interval_ms"], 15_000);
        assert_eq!(
            response["proxy_stores"],
            serde_json::json!(["ws://store-a.example/ws", "ws://store-b.example/ws"])
        );
    }
}
