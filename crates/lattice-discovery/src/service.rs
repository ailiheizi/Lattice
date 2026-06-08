use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

use lattice_crypto::sign::verify_identity_card;
use lattice_proto::discovery::{
    dht_envelope::Body, DhtEnvelope, DhtPing, DhtPong, IdentityCard, Lookup, LookupResult, Publish,
};

use crate::dht::DhtStore;

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("protobuf decode error: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("protobuf encode error: {0}")]
    Encode(#[from] prost::EncodeError),

    #[error("unexpected websocket frame")]
    UnexpectedFrame,
}

pub struct DiscoveryService {
    store: Arc<Mutex<DhtStore>>,
}

impl DiscoveryService {
    pub fn new(store: Arc<Mutex<DhtStore>>) -> Self {
        Self { store }
    }

    pub async fn handle_envelope(&self, envelope: DhtEnvelope) -> Option<DhtEnvelope> {
        match envelope.body {
            Some(Body::Publish(Publish { card: Some(card) })) => {
                match verify_identity_card(&card) {
                    Ok(true) => {
                        self.store.lock().await.publish(card);
                    }
                    Ok(false) => {
                        tracing::warn!(
                            "Rejecting publish because identity-card verification returned false"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            "Rejecting publish because identity-card verification failed: {error}"
                        );
                    }
                }
                None
            }
            Some(Body::Lookup(Lookup { fingerprint })) => {
                let card = self.store.lock().await.lookup(&fingerprint).cloned();
                Some(DhtEnvelope {
                    body: Some(Body::LookupResult(LookupResult { card })),
                })
            }
            Some(Body::Ping(DhtPing { timestamp })) => Some(DhtEnvelope {
                body: Some(Body::Pong(DhtPong { timestamp })),
            }),
            _ => None,
        }
    }
}

pub async fn run_server(addr: &str, store: Arc<Mutex<DhtStore>>) -> Result<(), DiscoveryError> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Discovery WebSocket server listening on {addr}");

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        let service = DiscoveryService::new(store.clone());
        tokio::spawn(async move {
            match accept_async(tcp_stream).await {
                Ok(ws_stream) => {
                    if let Err(error) = handle_connection(service, ws_stream).await {
                        tracing::warn!("Discovery connection {peer_addr} failed: {error}");
                    }
                }
                Err(error) => {
                    tracing::warn!("Discovery handshake failed for {peer_addr}: {error}");
                }
            }
        });
    }
}

async fn handle_connection(
    service: DiscoveryService,
    ws_stream: tokio_tungstenite::WebSocketStream<TcpStream>,
) -> Result<(), DiscoveryError> {
    let (mut sink, mut stream) = ws_stream.split();

    while let Some(message) = stream.next().await {
        match message? {
            WsMessage::Binary(data) => {
                let envelope = DhtEnvelope::decode(data.as_ref())?;
                if let Some(response) = service.handle_envelope(envelope).await {
                    sink.send(WsMessage::Binary(response.encode_to_vec()))
                        .await?;
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

pub async fn publish_to(bootstrap_addr: &str, card: &IdentityCard) -> Result<(), DiscoveryError> {
    let mut ws = connect_outbound(bootstrap_addr).await?;
    let envelope = DhtEnvelope {
        body: Some(Body::Publish(Publish {
            card: Some(card.clone()),
        })),
    };

    ws.send(WsMessage::Binary(envelope.encode_to_vec())).await?;
    ws.close(None).await?;
    Ok(())
}

pub async fn lookup_from(
    addr: &str,
    fingerprint: &str,
) -> Result<Option<IdentityCard>, DiscoveryError> {
    let mut ws = connect_outbound(addr).await?;
    let envelope = DhtEnvelope {
        body: Some(Body::Lookup(Lookup {
            fingerprint: fingerprint.to_string(),
        })),
    };

    ws.send(WsMessage::Binary(envelope.encode_to_vec())).await?;

    while let Some(message) = ws.next().await {
        match message? {
            WsMessage::Binary(data) => {
                let response = DhtEnvelope::decode(data.as_ref())?;
                if let Some(Body::LookupResult(result)) = response.body {
                    ws.close(None).await?;
                    return Ok(result.card);
                }
            }
            WsMessage::Ping(payload) => {
                ws.send(WsMessage::Pong(payload)).await?;
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    Err(DiscoveryError::UnexpectedFrame)
}

async fn connect_outbound(
    addr: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, DiscoveryError> {
    let (ws, _) = connect_async(addr).await?;
    Ok(ws)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::time::{sleep, Duration};

    use lattice_crypto::{identity::MasterKeyPair, sign::compute_identity_card_hash};

    use crate::dht::NodeId;

    fn make_signed_card() -> IdentityCard {
        let master = MasterKeyPair::generate();
        let mut card = IdentityCard {
            fingerprint: master.fingerprint(),
            display_name: "Alice".to_string(),
            ed25519_public_key: master.verifying_key().as_bytes().to_vec(),
            curve25519_public_key: master.encryption_public_key().as_bytes().to_vec(),
            store_address: "ws://127.0.0.1:9100".to_string(),
            proxy_store_address: "ws://127.0.0.1:9200".to_string(),
            signature: Vec::new(),
        };
        let hash = compute_identity_card_hash(&card);
        card.signature = master.sign(&hash);
        card
    }

    async fn spawn_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let store = Arc::new(Mutex::new(DhtStore::new(
            NodeId::from_data(b"discovery-test"),
            20,
        )));
        let server_addr = addr.to_string();
        let handle = tokio::spawn(async move {
            run_server(&server_addr, store)
                .await
                .expect("run discovery server");
        });

        let url = format!("ws://127.0.0.1:{}", addr.port());
        for _ in 0..20 {
            if connect_async(&url).await.is_ok() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }

        (url, handle)
    }

    #[tokio::test]
    async fn publish_and_lookup_roundtrip() {
        let (url, handle) = spawn_server().await;
        let card = make_signed_card();

        publish_to(&url, &card).await.unwrap();
        let looked_up = lookup_from(&url, &card.fingerprint).await.unwrap();

        assert_eq!(looked_up, Some(card));
        handle.abort();
    }

    #[tokio::test]
    async fn forged_card_is_rejected_by_server() {
        let (url, handle) = spawn_server().await;
        let mut card = make_signed_card();
        card.store_address = "ws://127.0.0.1:9999".to_string();

        publish_to(&url, &card).await.unwrap();
        let looked_up = lookup_from(&url, &card.fingerprint).await.unwrap();

        assert!(looked_up.is_none());
        handle.abort();
    }
}
