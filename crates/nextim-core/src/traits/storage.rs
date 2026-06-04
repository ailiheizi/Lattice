use crate::error::Result;
use nextim_proto::{
    group::{Room, RoomEvent},
    identity::{Contact, DeviceInfo, KeyBundle},
    message::Message,
};

/// 时间范围查询
pub struct TimeRange {
    pub start: u64,
    pub end: u64,
}

/// 分页参数
pub struct Pagination {
    pub offset: u64,
    pub limit: u32,
}

/// 挂起区消息：引用了未知父消息的原始 Envelope 存档。
pub struct PendingMessage {
    pub msg_hash: Vec<u8>,
    pub data: Vec<u8>,
    pub received_ts: u64,
}

/// 存储层抽象 — 数据持久化
pub trait Storage: Send + Sync {
    // === 消息 ===
    fn save_message(&self, msg: &Message) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_messages(
        &self,
        room_id: &str,
        range: &TimeRange,
        page: &Pagination,
    ) -> impl std::future::Future<Output = Result<Vec<Message>>> + Send;
    fn get_message(
        &self,
        msg_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Message>>> + Send;
    fn delete_message(&self, msg_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    fn save_message_edge(
        &self,
        child_hash: &[u8],
        parent_hash: &[u8],
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_message_parents(
        &self,
        child_hash: &[u8],
    ) -> impl std::future::Future<Output = Result<Vec<Vec<u8>>>> + Send;
    fn get_head_message_hashes(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<u8>>>> + Send;
    fn save_pending_message(
        &self,
        pending: &PendingMessage,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_pending_message(
        &self,
        msg_hash: &[u8],
    ) -> impl std::future::Future<Output = Result<Option<PendingMessage>>> + Send;
    fn list_pending_messages(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<PendingMessage>>> + Send;
    fn delete_pending_message(
        &self,
        msg_hash: &[u8],
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    // === 房间/群组 ===
    fn save_room(&self, room: &Room) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_room(
        &self,
        room_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Room>>> + Send;
    fn get_rooms(&self) -> impl std::future::Future<Output = Result<Vec<Room>>> + Send;
    fn delete_room(&self, room_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    fn save_room_event(
        &self,
        event: &RoomEvent,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_room_events(
        &self,
        room_id: &str,
        since_timestamp: u64,
    ) -> impl std::future::Future<Output = Result<Vec<RoomEvent>>> + Send;

    // === 联系人 ===
    fn save_contact(
        &self,
        contact: &Contact,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_contact(
        &self,
        fingerprint: &str,
    ) -> impl std::future::Future<Output = Result<Option<Contact>>> + Send;
    fn get_contacts(&self) -> impl std::future::Future<Output = Result<Vec<Contact>>> + Send;
    fn delete_contact(
        &self,
        fingerprint: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    // === 设备 ===
    fn save_device(
        &self,
        device: &DeviceInfo,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_devices(
        &self,
        user_fingerprint: &str,
    ) -> impl std::future::Future<Output = Result<Vec<DeviceInfo>>> + Send;

    // === 密钥 ===
    fn save_key_bundle(
        &self,
        bundle: &KeyBundle,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_key_bundle(
        &self,
        fingerprint: &str,
    ) -> impl std::future::Future<Output = Result<Option<KeyBundle>>> + Send;
}
