use crate::error::Result;
use nextim_proto::message::Message;

/// 搜索结果
pub struct SearchResult {
    pub msg_id: String,
    pub room_id: String,
    pub snippet: String,
    pub score: f32,
    pub timestamp: u64,
}

/// 搜索层抽象 — 全文检索
pub trait SearchIndex: Send + Sync {
    /// 索引一条消息
    fn index_message(&self, msg: &Message) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 批量索引
    fn index_messages(
        &self,
        msgs: &[Message],
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 全文搜索
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<SearchResult>>> + Send;

    /// 在指定房间内搜索
    fn search_in_room(
        &self,
        room_id: &str,
        query: &str,
        limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<SearchResult>>> + Send;

    /// 删除消息索引
    fn delete_index(&self, msg_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 重建全部索引
    fn rebuild_index(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}
