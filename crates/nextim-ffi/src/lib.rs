uniffi::include_scaffolding!("nextim");

use std::path::PathBuf;

use nextim_core::traits::search::SearchIndex;
use nextim_core::traits::storage::{Pagination, Storage, TimeRange};
use nextim_crypto::identity::MasterKeyPair;
use nextim_proto::message::{Message, MessageContent, MessageType};
use nextim_storage::sqlite::SqliteStorage;
use nextim_storage::tantivy_search::TantivySearch;

#[derive(Debug, thiserror::Error)]
pub enum NextImFfiError {
    #[error("crypto error: {0}")]
    CryptoError(String),
    #[error("storage error: {0}")]
    StorageError(String),
    #[error("transport error: {0}")]
    TransportError(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("{0}")]
    Other(String),
}

impl From<nextim_core::error::NextImError> for NextImFfiError {
    fn from(e: nextim_core::error::NextImError) -> Self {
        Self::Other(e.to_string())
    }
}

pub struct FfiIdentity {
    pub ed25519_public_key: Vec<u8>,
    pub curve25519_public_key: Vec<u8>,
    pub fingerprint: String,
    pub display_name: String,
    pub created_at: u64,
}

pub struct FfiIdentityCard {
    pub identity: FfiIdentity,
    pub store_address: String,
    pub proxy_store_address: String,
}

pub struct FfiMessage {
    pub msg_id: String,
    pub room_id: String,
    pub sender_fingerprint: String,
    pub timestamp: u64,
    pub text: String,
    pub encrypted: bool,
    pub verified: bool,
}

pub struct FfiSearchResult {
    pub msg_id: String,
    pub room_id: String,
    pub snippet: String,
    pub score: f32,
    pub timestamp: u64,
}

pub struct NextImClient {
    keypair: MasterKeyPair,
    storage: SqliteStorage,
    search: TantivySearch,
    rt: tokio::runtime::Runtime,
}

impl NextImClient {
    pub fn new(data_dir: String) -> Result<Self, NextImFfiError> {
        let path = PathBuf::from(&data_dir);
        std::fs::create_dir_all(&path).map_err(|e| NextImFfiError::StorageError(e.to_string()))?;

        let storage = SqliteStorage::open(path.join("store.db"))
            .map_err(|e| NextImFfiError::StorageError(e.to_string()))?;
        let search = TantivySearch::open(path.join("search_index"))
            .map_err(|e| NextImFfiError::StorageError(e.to_string()))?;

        let rt =
            tokio::runtime::Runtime::new().map_err(|e| NextImFfiError::Other(e.to_string()))?;

        Ok(Self {
            keypair: MasterKeyPair::generate(),
            storage,
            search,
            rt,
        })
    }

    pub fn create_identity(
        &self,
        display_name: String,
        store_address: String,
    ) -> Result<FfiIdentityCard, NextImFfiError> {
        let identity = self.keypair.to_identity(&display_name);
        Ok(FfiIdentityCard {
            identity: FfiIdentity {
                ed25519_public_key: identity.ed25519_public_key,
                curve25519_public_key: identity.curve25519_public_key,
                fingerprint: identity.fingerprint,
                display_name: identity.display_name,
                created_at: identity.created_at,
            },
            store_address,
            proxy_store_address: String::new(),
        })
    }

    pub fn get_fingerprint(&self) -> String {
        self.keypair.fingerprint()
    }

    pub fn sign_data(&self, data: Vec<u8>) -> Result<Vec<u8>, NextImFfiError> {
        Ok(self.keypair.sign(&data))
    }

    pub fn verify_signature(
        &self,
        public_key: Vec<u8>,
        data: Vec<u8>,
        signature: Vec<u8>,
    ) -> Result<bool, NextImFfiError> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let key_bytes: [u8; 32] = public_key
            .as_slice()
            .try_into()
            .map_err(|_| NextImFfiError::CryptoError("invalid key length".into()))?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| NextImFfiError::CryptoError(e.to_string()))?;

        let sig_bytes: [u8; 64] = signature
            .as_slice()
            .try_into()
            .map_err(|_| NextImFfiError::CryptoError("invalid signature length".into()))?;
        let sig = Signature::from_bytes(&sig_bytes);

        match verifying_key.verify(&data, &sig) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn send_message(&self, room_id: String, text: String) -> Result<String, NextImFfiError> {
        let msg_id = uuid::Uuid::new_v4().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let msg = Message {
            msg_id: msg_id.clone(),
            room_id,
            sender_fingerprint: self.keypair.fingerprint(),
            timestamp,
            content: Some(MessageContent {
                r#type: MessageType::Text as i32,
                text,
                ..Default::default()
            }),
            encrypted: false,
            verified: true,
            encrypted_payload: None,
            received_ts: timestamp,
            prev_hashes: Vec::new(),
            msg_hash: Vec::new(),
            redacted: false,
            edited: false,
        };

        self.rt.block_on(async {
            self.storage
                .save_message(&msg)
                .await
                .map_err(|e| NextImFfiError::StorageError(e.to_string()))?;
            let _ = self.search.index_message(&msg).await;
            Ok(msg_id)
        })
    }

    pub fn get_messages(
        &self,
        room_id: String,
        since: u64,
        until: u64,
        limit: u32,
    ) -> Result<Vec<FfiMessage>, NextImFfiError> {
        self.rt.block_on(async {
            let range = TimeRange {
                start: since,
                end: until,
            };
            let page = Pagination { offset: 0, limit };
            let msgs = self
                .storage
                .get_messages(&room_id, &range, &page)
                .await
                .map_err(|e| NextImFfiError::StorageError(e.to_string()))?;

            Ok(msgs
                .into_iter()
                .map(|m| FfiMessage {
                    msg_id: m.msg_id,
                    room_id: m.room_id,
                    sender_fingerprint: m.sender_fingerprint,
                    timestamp: m.timestamp,
                    text: m.content.map(|c| c.text).unwrap_or_default(),
                    encrypted: m.encrypted,
                    verified: m.verified,
                })
                .collect())
        })
    }

    pub fn search(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<FfiSearchResult>, NextImFfiError> {
        self.rt.block_on(async {
            let results = self
                .search
                .search(&query, limit as usize)
                .await
                .map_err(|e| NextImFfiError::StorageError(e.to_string()))?;

            Ok(results
                .into_iter()
                .map(|r| FfiSearchResult {
                    msg_id: r.msg_id,
                    room_id: r.room_id,
                    snippet: r.snippet,
                    score: r.score,
                    timestamp: r.timestamp,
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> (NextImClient, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let client = NextImClient::new(dir.path().to_str().unwrap().to_string()).unwrap();
        (client, dir) // dir must outlive client
    }

    #[test]
    fn test_create_identity() {
        let (client, _dir) = make_client();
        let card = client
            .create_identity("Alice".into(), "ws://localhost:9100".into())
            .unwrap();
        assert_eq!(card.identity.display_name, "Alice");
        assert!(!card.identity.fingerprint.is_empty());
        assert_eq!(card.identity.fingerprint.len(), 64);
    }

    #[test]
    fn test_get_fingerprint() {
        let (client, _dir) = make_client();
        let fp = client.get_fingerprint();
        assert_eq!(fp.len(), 64);
    }

    #[test]
    fn test_sign_and_verify() {
        let (client, _dir) = make_client();
        let data = b"hello ffi".to_vec();
        let sig = client.sign_data(data.clone()).unwrap();
        assert_eq!(sig.len(), 64);

        let card = client
            .create_identity("Test".into(), "ws://x".into())
            .unwrap();
        let valid = client
            .verify_signature(card.identity.ed25519_public_key, data, sig)
            .unwrap();
        assert!(valid);
    }

    #[test]
    fn test_verify_wrong_data() {
        let (client, _dir) = make_client();
        let sig = client.sign_data(b"original".to_vec()).unwrap();
        let card = client.create_identity("T".into(), "ws://x".into()).unwrap();
        let valid = client
            .verify_signature(card.identity.ed25519_public_key, b"tampered".to_vec(), sig)
            .unwrap();
        assert!(!valid);
    }

    #[test]
    fn test_send_and_get_messages() {
        let (client, _dir) = make_client();
        let msg_id = client
            .send_message("room-1".into(), "hello ffi".into())
            .unwrap();
        assert!(!msg_id.is_empty());

        let msgs = client
            .get_messages("room-1".into(), 0, 9_999_999_999_999, 50)
            .unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text, "hello ffi");
        assert_eq!(msgs[0].room_id, "room-1");
    }

    #[test]
    fn test_search() {
        let (client, _dir) = make_client();
        client
            .send_message("r1".into(), "rust programming".into())
            .unwrap();
        client
            .send_message("r1".into(), "go language".into())
            .unwrap();
        client
            .send_message("r1".into(), "rust is fast".into())
            .unwrap();

        let results = client.search("rust".into(), 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_chinese() {
        let (client, _dir) = make_client();
        client.send_message("r1".into(), "你好世界".into()).unwrap();
        client
            .send_message("r1".into(), "今天天气好".into())
            .unwrap();

        let results = client.search("你".into(), 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_multiple_rooms() {
        let (client, _dir) = make_client();
        client
            .send_message("room-a".into(), "msg a".into())
            .unwrap();
        client
            .send_message("room-b".into(), "msg b".into())
            .unwrap();

        let a = client
            .get_messages("room-a".into(), 0, 9_999_999_999_999, 50)
            .unwrap();
        let b = client
            .get_messages("room-b".into(), 0, 9_999_999_999_999, 50)
            .unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_eq!(a[0].text, "msg a");
        assert_eq!(b[0].text, "msg b");
    }
}
