//! 零成本空搜索后端(ZST):实现 `SearchIndex` 但不做任何索引,搜索恒返回空。
//!
//! 用途:`store-light` 变体 —— 不需要全文搜索的轻持久化节点。
//! 通过 `--no-default-features --features "storage-sqlite,search-noop"` 选用,
//! 编译期由 `ActiveSearch` 类型别名解析到此实现,运行时零开销(无 tantivy 依赖)。

use std::path::Path;

use lattice_core::error::Result;
use lattice_core::traits::search::{SearchIndex, SearchResult};
use lattice_proto::message::Message;

/// 空搜索实现:索引操作全部 no-op,搜索恒空。
#[derive(Default)]
pub struct NoopSearch;

impl NoopSearch {
    /// 与 `TantivySearch::open` 同签名,便于 `ActiveSearch::open` 在 run() 中统一调用。
    /// 路径参数被忽略(无索引需要持久化)。
    pub fn open(_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self)
    }

    /// 与 `TantivySearch::in_memory` 同签名(测试/轻量场景)。
    pub fn in_memory() -> Result<Self> {
        Ok(Self)
    }
}

impl SearchIndex for NoopSearch {
    async fn index_message(&self, _msg: &Message) -> Result<()> {
        Ok(())
    }

    async fn index_messages(&self, _msgs: &[Message]) -> Result<()> {
        Ok(())
    }

    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchResult>> {
        Ok(Vec::new())
    }

    async fn search_in_room(
        &self,
        _room_id: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResult>> {
        Ok(Vec::new())
    }

    async fn delete_index(&self, _msg_id: &str) -> Result<()> {
        Ok(())
    }

    async fn rebuild_index(&self) -> Result<()> {
        Ok(())
    }
}
