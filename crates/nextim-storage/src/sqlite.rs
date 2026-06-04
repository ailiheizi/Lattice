use std::path::Path;
use std::sync::Mutex;

use prost::Message as ProstMessage;
use rusqlite::{params, Connection};

use nextim_core::error::{NextImError, Result};
use nextim_core::traits::storage::{Pagination, PendingMessage, Storage, TimeRange};
use nextim_proto::group::{Room, RoomEvent};
use nextim_proto::identity::{Contact, DeviceInfo, KeyBundle};
use nextim_proto::message::Message;

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| NextImError::Storage(e.to_string()))?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| NextImError::Storage(e.to_string()))?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS messages (
                msg_id TEXT PRIMARY KEY,
                room_id TEXT NOT NULL,
                sender_fingerprint TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                received_ts INTEGER NOT NULL DEFAULT 0,
                msg_hash BLOB NOT NULL DEFAULT X'',
                prev_hashes BLOB NOT NULL DEFAULT X'',
                data BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_room_ts ON messages(room_id, timestamp);
            CREATE INDEX IF NOT EXISTS idx_messages_msg_hash ON messages(msg_hash);

            CREATE TABLE IF NOT EXISTS message_edges (
                child_hash BLOB NOT NULL,
                parent_hash BLOB NOT NULL,
                PRIMARY KEY (child_hash, parent_hash)
            );
            CREATE INDEX IF NOT EXISTS idx_message_edges_parent ON message_edges(parent_hash);
            CREATE INDEX IF NOT EXISTS idx_message_edges_child ON message_edges(child_hash);

            CREATE TABLE IF NOT EXISTS pending_messages (
                msg_hash BLOB PRIMARY KEY,
                data BLOB NOT NULL,
                received_ts INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS rooms (
                room_id TEXT PRIMARY KEY,
                data BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS room_events (
                msg_hash BLOB PRIMARY KEY,
                room_id TEXT NOT NULL,
                actor_fingerprint TEXT NOT NULL,
                event_type INTEGER NOT NULL,
                target_fingerprint TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                received_ts INTEGER NOT NULL DEFAULT 0,
                signature BLOB NOT NULL,
                data BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_room_events_room_ts ON room_events(room_id, timestamp);

            CREATE TABLE IF NOT EXISTS contacts (
                fingerprint TEXT PRIMARY KEY,
                data BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                user_fingerprint TEXT NOT NULL,
                data BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_devices_user ON devices(user_fingerprint);

            CREATE TABLE IF NOT EXISTS key_bundles (
                fingerprint TEXT PRIMARY KEY,
                data BLOB NOT NULL
            );
            ",
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }
}

impl Storage for SqliteStorage {
    async fn save_message(&self, msg: &Message) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let data = msg.encode_to_vec();
        // 用 u32 大端长度前缀串联每个哈希，便于后续 P3 直接按顺序解析。
        let prev_hashes = encode_hash_list(&msg.prev_hashes)?;
        conn.execute(
            "INSERT OR REPLACE INTO messages (msg_id, room_id, sender_fingerprint, timestamp, received_ts, msg_hash, prev_hashes, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                msg.msg_id,
                msg.room_id,
                msg.sender_fingerprint,
                msg.timestamp as i64,
                msg.received_ts as i64,
                msg.msg_hash,
                prev_hashes,
                data,
            ],
        ).map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_messages(
        &self,
        room_id: &str,
        range: &TimeRange,
        page: &Pagination,
    ) -> Result<Vec<Message>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT data FROM messages WHERE room_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3 ORDER BY timestamp ASC LIMIT ?4 OFFSET ?5",
            )
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    room_id,
                    range.start as i64,
                    range.end as i64,
                    page.limit,
                    page.offset as i64
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut messages = Vec::new();
        for row in rows {
            let data = row.map_err(|e| NextImError::Storage(e.to_string()))?;
            let msg = Message::decode(data.as_slice())
                .map_err(|e| NextImError::Serialization(e.to_string()))?;
            messages.push(msg);
        }
        Ok(messages)
    }

    async fn get_message(&self, msg_id: &str) -> Result<Option<Message>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM messages WHERE msg_id = ?1")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let result = stmt
            .query_row(params![msg_id], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        match result {
            Some(data) => {
                let msg = Message::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?;
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }

    async fn delete_message(&self, msg_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute("DELETE FROM messages WHERE msg_id = ?1", params![msg_id])
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn save_message_edge(&self, child_hash: &[u8], parent_hash: &[u8]) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO message_edges (child_hash, parent_hash) VALUES (?1, ?2)",
            params![child_hash, parent_hash],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_message_parents(&self, child_hash: &[u8]) -> Result<Vec<Vec<u8>>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT parent_hash FROM message_edges WHERE child_hash = ?1 ORDER BY parent_hash ASC")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(params![child_hash], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut parents = Vec::new();
        for row in rows {
            parents.push(row.map_err(|e| NextImError::Storage(e.to_string()))?);
        }
        Ok(parents)
    }

    async fn get_head_message_hashes(&self) -> Result<Vec<Vec<u8>>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT child_hash FROM message_edges WHERE child_hash NOT IN (SELECT parent_hash FROM message_edges) ORDER BY child_hash ASC",
            )
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut heads = Vec::new();
        for row in rows {
            heads.push(row.map_err(|e| NextImError::Storage(e.to_string()))?);
        }
        Ok(heads)
    }

    async fn save_pending_message(&self, pending: &PendingMessage) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO pending_messages (msg_hash, data, received_ts) VALUES (?1, ?2, ?3)",
            params![pending.msg_hash, pending.data, pending.received_ts as i64],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_pending_message(&self, msg_hash: &[u8]) -> Result<Option<PendingMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT msg_hash, data, received_ts FROM pending_messages WHERE msg_hash = ?1")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        stmt.query_row(params![msg_hash], |row| {
            Ok(PendingMessage {
                msg_hash: row.get(0)?,
                data: row.get(1)?,
                received_ts: row.get::<_, i64>(2)? as u64,
            })
        })
        .optional()
        .map_err(|e| NextImError::Storage(e.to_string()))
    }

    async fn list_pending_messages(&self) -> Result<Vec<PendingMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT msg_hash, data, received_ts FROM pending_messages ORDER BY received_ts ASC, msg_hash ASC")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(PendingMessage {
                    msg_hash: row.get(0)?,
                    data: row.get(1)?,
                    received_ts: row.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| NextImError::Storage(e.to_string()))?);
        }
        Ok(messages)
    }

    async fn delete_pending_message(&self, msg_hash: &[u8]) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM pending_messages WHERE msg_hash = ?1",
            params![msg_hash],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn save_room(&self, room: &Room) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let data = room.encode_to_vec();
        conn.execute(
            "INSERT OR REPLACE INTO rooms (room_id, data) VALUES (?1, ?2)",
            params![room.room_id, data],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_room(&self, room_id: &str) -> Result<Option<Room>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM rooms WHERE room_id = ?1")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let result = stmt
            .query_row(params![room_id], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        match result {
            Some(data) => Ok(Some(
                Room::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    async fn get_rooms(&self) -> Result<Vec<Room>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM rooms")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut rooms = Vec::new();
        for row in rows {
            let data = row.map_err(|e| NextImError::Storage(e.to_string()))?;
            rooms.push(
                Room::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            );
        }
        Ok(rooms)
    }

    async fn delete_room(&self, room_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute("DELETE FROM rooms WHERE room_id = ?1", params![room_id])
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn save_room_event(&self, event: &RoomEvent) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let data = event.encode_to_vec();
        let received_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT OR REPLACE INTO room_events (msg_hash, room_id, actor_fingerprint, event_type, target_fingerprint, timestamp, received_ts, signature, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.msg_hash,
                event.room_id,
                event.actor_fingerprint,
                event.r#type,
                event.target_fingerprint,
                event.timestamp as i64,
                received_ts,
                event.signature,
                data,
            ],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_room_events(&self, room_id: &str, since_timestamp: u64) -> Result<Vec<RoomEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT data FROM room_events WHERE room_id = ?1 AND timestamp >= ?2 ORDER BY timestamp ASC",
            )
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(params![room_id, since_timestamp as i64], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let data = row.map_err(|e| NextImError::Storage(e.to_string()))?;
            events.push(
                RoomEvent::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            );
        }
        Ok(events)
    }

    async fn save_contact(&self, contact: &Contact) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let fingerprint = contact
            .identity
            .as_ref()
            .map(|i| i.fingerprint.clone())
            .unwrap_or_default();
        let data = contact.encode_to_vec();
        conn.execute(
            "INSERT OR REPLACE INTO contacts (fingerprint, data) VALUES (?1, ?2)",
            params![fingerprint, data],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_contact(&self, fingerprint: &str) -> Result<Option<Contact>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM contacts WHERE fingerprint = ?1")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let result = stmt
            .query_row(params![fingerprint], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        match result {
            Some(data) => Ok(Some(
                Contact::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    async fn get_contacts(&self) -> Result<Vec<Contact>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM contacts")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut contacts = Vec::new();
        for row in rows {
            let data = row.map_err(|e| NextImError::Storage(e.to_string()))?;
            contacts.push(
                Contact::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            );
        }
        Ok(contacts)
    }

    async fn delete_contact(&self, fingerprint: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM contacts WHERE fingerprint = ?1",
            params![fingerprint],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn save_device(&self, device: &DeviceInfo) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let data = device.encode_to_vec();
        conn.execute(
            "INSERT OR REPLACE INTO devices (device_id, user_fingerprint, data) VALUES (?1, ?2, ?3)",
            params![device.device_id, device.user_fingerprint, data],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_devices(&self, user_fingerprint: &str) -> Result<Vec<DeviceInfo>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM devices WHERE user_fingerprint = ?1")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map(params![user_fingerprint], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let mut devices = Vec::new();
        for row in rows {
            let data = row.map_err(|e| NextImError::Storage(e.to_string()))?;
            devices.push(
                DeviceInfo::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            );
        }
        Ok(devices)
    }

    async fn save_key_bundle(&self, bundle: &KeyBundle) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let data = bundle.encode_to_vec();
        conn.execute(
            "INSERT OR REPLACE INTO key_bundles (fingerprint, data) VALUES (?1, ?2)",
            params![bundle.fingerprint, data],
        )
        .map_err(|e| NextImError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_key_bundle(&self, fingerprint: &str) -> Result<Option<KeyBundle>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| NextImError::Storage(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT data FROM key_bundles WHERE fingerprint = ?1")
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        let result = stmt
            .query_row(params![fingerprint], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| NextImError::Storage(e.to_string()))?;

        match result {
            Some(data) => Ok(Some(
                KeyBundle::decode(data.as_slice())
                    .map_err(|e| NextImError::Serialization(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }
}

/// rusqlite 的 optional() 扩展
trait OptionalExt<T> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

fn encode_hash_list(hashes: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    for hash in hashes {
        let len = u32::try_from(hash.len())
            .map_err(|_| NextImError::Storage("prev_hash is too large to encode".to_string()))?;
        encoded.extend_from_slice(&len.to_be_bytes());
        encoded.extend_from_slice(hash);
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextim_proto::{
        group::RoomEventType,
        message::{MessageContent, MessageType},
    };

    fn make_message(id: &str, room: &str, ts: u64, text: &str) -> Message {
        Message {
            msg_id: id.to_string(),
            room_id: room.to_string(),
            sender_fingerprint: "sender-abc".to_string(),
            timestamp: ts,
            content: Some(MessageContent {
                r#type: MessageType::Text as i32,
                text: text.to_string(),
                ..Default::default()
            }),
            encrypted: false,
            verified: true,
            encrypted_payload: None,
            received_ts: 0,
            prev_hashes: Vec::new(),
            msg_hash: Vec::new(),
        }
    }

    fn make_encrypted_message(id: &str, room: &str, ts: u64) -> Message {
        Message {
            msg_id: id.to_string(),
            room_id: room.to_string(),
            sender_fingerprint: "sender-abc".to_string(),
            timestamp: ts,
            content: None,
            encrypted: true,
            verified: false,
            encrypted_payload: Some(nextim_proto::message::EncryptedPayload {
                ciphertext: b"ciphertext".to_vec(),
                session_id: "session-1".to_string(),
                message_index: 7,
                encryption_type: nextim_proto::message::EncryptionType::Olm as i32,
            }),
            received_ts: 0,
            prev_hashes: Vec::new(),
            msg_hash: Vec::new(),
        }
    }

    fn make_room_event(room_id: &str, actor: &str, target: &str, ts: u64) -> RoomEvent {
        // 测试用唯一 msg_hash（room_events 主键），基于参数派生避免冲突。
        let msg_hash = format!("{room_id}:{actor}:{target}:{ts}").into_bytes();
        RoomEvent {
            room_id: room_id.to_string(),
            actor_fingerprint: actor.to_string(),
            r#type: RoomEventType::MemberJoin as i32,
            target_fingerprint: target.to_string(),
            timestamp: ts,
            signature: b"room-event-signature".to_vec(),
            prev_hashes: Vec::new(),
            msg_hash,
        }
    }

    #[tokio::test]
    async fn test_room_event_roundtrip() {
        let storage = SqliteStorage::in_memory().unwrap();
        let event = make_room_event("room1", "owner", "alice", 3000);

        storage.save_room_event(&event).await.unwrap();

        let events = storage.get_room_events("room1", 0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].room_id, "room1");
        assert_eq!(events[0].actor_fingerprint, "owner");
        assert_eq!(events[0].target_fingerprint, "alice");
        assert_eq!(events[0].timestamp, 3000);
        assert_eq!(events[0].signature, b"room-event-signature".to_vec());
    }

    #[tokio::test]
    async fn test_room_event_since_filter() {
        let storage = SqliteStorage::in_memory().unwrap();
        storage
            .save_room_event(&make_room_event("room1", "owner", "alice", 3000))
            .await
            .unwrap();
        storage
            .save_room_event(&make_room_event("room1", "owner", "bob", 4000))
            .await
            .unwrap();
        storage
            .save_room_event(&make_room_event("room2", "owner", "carol", 5000))
            .await
            .unwrap();

        let events = storage.get_room_events("room1", 3500).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target_fingerprint, "bob");
        assert_eq!(events[0].timestamp, 4000);
    }

    #[tokio::test]
    async fn test_message_crud() {
        let storage = SqliteStorage::in_memory().unwrap();
        let msg = make_message("m1", "room1", 1000, "hello");

        storage.save_message(&msg).await.unwrap();

        let got = storage.get_message("m1").await.unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().msg_id, "m1");

        storage.delete_message("m1").await.unwrap();
        assert!(storage.get_message("m1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_message_range_query() {
        let storage = SqliteStorage::in_memory().unwrap();

        for i in 0..10 {
            let msg = make_message(&format!("m{i}"), "room1", 1000 + i, &format!("msg {i}"));
            storage.save_message(&msg).await.unwrap();
        }

        let range = TimeRange {
            start: 1003,
            end: 1007,
        };
        let page = Pagination {
            offset: 0,
            limit: 100,
        };
        let msgs = storage.get_messages("room1", &range, &page).await.unwrap();
        assert_eq!(msgs.len(), 5);
    }

    #[tokio::test]
    async fn test_encrypted_message_roundtrip() {
        let storage = SqliteStorage::in_memory().unwrap();
        let msg = make_encrypted_message("m-encrypted", "room1", 2000);

        storage.save_message(&msg).await.unwrap();

        let got = storage
            .get_message("m-encrypted")
            .await
            .unwrap()
            .expect("encrypted message stored");
        assert!(got.encrypted);
        assert!(got.content.is_none());
        let payload = got.encrypted_payload.expect("encrypted payload preserved");
        assert_eq!(payload.ciphertext, b"ciphertext".to_vec());
        assert_eq!(payload.session_id, "session-1");
        assert_eq!(payload.message_index, 7);
    }

    #[tokio::test]
    async fn test_message_metadata_roundtrip() {
        let storage = SqliteStorage::in_memory().unwrap();
        let mut msg = make_message("m-meta", "room1", 1000, "hello");
        msg.received_ts = 2222;
        msg.msg_hash = vec![0xAA, 0xBB, 0xCC];
        msg.prev_hashes = vec![vec![0x01], vec![0x02, 0x03]];

        storage.save_message(&msg).await.unwrap();

        let got = storage
            .get_message("m-meta")
            .await
            .unwrap()
            .expect("message stored");
        assert_eq!(got.received_ts, 2222);
        assert_eq!(got.msg_hash, vec![0xAA, 0xBB, 0xCC]);
        assert_eq!(got.prev_hashes, vec![vec![0x01], vec![0x02, 0x03]]);
    }

    #[tokio::test]
    async fn test_message_edges_and_heads() {
        let storage = SqliteStorage::in_memory().unwrap();
        let root = vec![0x01];
        let left = vec![0x02];
        let right = vec![0x03];
        let merged = vec![0x04];

        storage.save_message_edge(&left, &root).await.unwrap();
        storage.save_message_edge(&right, &root).await.unwrap();
        storage.save_message_edge(&merged, &left).await.unwrap();

        let parents = storage.get_message_parents(&merged).await.unwrap();
        assert_eq!(parents, vec![left.clone()]);

        let heads = storage.get_head_message_hashes().await.unwrap();
        assert_eq!(heads, vec![right.clone(), merged.clone()]);
    }

    #[tokio::test]
    async fn test_pending_message_roundtrip() {
        let storage = SqliteStorage::in_memory().unwrap();
        let pending = PendingMessage {
            msg_hash: vec![0x10, 0x20],
            data: vec![0x30, 0x40],
            received_ts: 12345,
        };

        storage.save_pending_message(&pending).await.unwrap();

        let got = storage
            .get_pending_message(&[0x10, 0x20])
            .await
            .unwrap()
            .expect("pending message stored");
        assert_eq!(got.msg_hash, pending.msg_hash);
        assert_eq!(got.data, pending.data);
        assert_eq!(got.received_ts, pending.received_ts);

        let listed = storage.list_pending_messages().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].msg_hash, pending.msg_hash);

        storage.delete_pending_message(&[0x10, 0x20]).await.unwrap();
        assert!(storage
            .get_pending_message(&[0x10, 0x20])
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_room_crud() {
        let storage = SqliteStorage::in_memory().unwrap();
        let room = Room {
            room_id: "r1".to_string(),
            name: "Test Room".to_string(),
            ..Default::default()
        };

        storage.save_room(&room).await.unwrap();
        let got = storage.get_room("r1").await.unwrap();
        assert_eq!(got.unwrap().name, "Test Room");

        let rooms = storage.get_rooms().await.unwrap();
        assert_eq!(rooms.len(), 1);

        storage.delete_room("r1").await.unwrap();
        assert!(storage.get_room("r1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_contact_crud() {
        use nextim_proto::identity::{Identity, TrustLevel};

        let storage = SqliteStorage::in_memory().unwrap();
        let contact = Contact {
            identity: Some(Identity {
                fingerprint: "fp-abc".to_string(),
                display_name: "Alice".to_string(),
                ..Default::default()
            }),
            store_address: "ws://localhost:8080".to_string(),
            trust_level: TrustLevel::Tofu as i32,
            alias: "My Alice".to_string(),
            ..Default::default()
        };

        storage.save_contact(&contact).await.unwrap();
        let got = storage.get_contact("fp-abc").await.unwrap();
        assert_eq!(got.unwrap().alias, "My Alice");

        storage.delete_contact("fp-abc").await.unwrap();
        assert!(storage.get_contact("fp-abc").await.unwrap().is_none());
    }
}
