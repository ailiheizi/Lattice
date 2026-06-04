use std::path::Path;
use std::sync::Mutex;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, STORED, STRING, Field, NumericOptions, OwnedValue, TextFieldIndexing, TextOptions, IndexRecordOption};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

use nextim_core::error::{NextImError, Result};
use nextim_core::traits::search::{SearchIndex, SearchResult};
use nextim_proto::message::Message;

use crate::cjk_tokenizer::CjkTokenizer;

const CJK_TOKENIZER_NAME: &str = "cjk";

pub struct TantivySearch {
    index: Index,
    reader: IndexReader,
    writer: Mutex<IndexWriter>,
    // schema fields
    f_msg_id: Field,
    f_room_id: Field,
    f_text: Field,
    f_timestamp: Field,
}

impl TantivySearch {
    /// 创建基于目录的索引
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let schema = Self::build_schema();
        std::fs::create_dir_all(path.as_ref())
            .map_err(|e| NextImError::Search(e.to_string()))?;
        let dir = tantivy::directory::MmapDirectory::open(path)
            .map_err(|e| NextImError::Search(e.to_string()))?;
        let index = Index::open_or_create(dir, schema.clone())
            .map_err(|e| NextImError::Search(e.to_string()))?;
        Self::from_index(index, &schema)
    }

    /// 创建内存索引（用于测试）
    pub fn in_memory() -> Result<Self> {
        let schema = Self::build_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, &schema)
    }

    fn build_schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_text_field("msg_id", STRING | STORED);

        let cjk_indexing = TextFieldIndexing::default()
            .set_tokenizer(CJK_TOKENIZER_NAME)
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let cjk_options = TextOptions::default()
            .set_indexing_options(cjk_indexing)
            .set_stored();

        builder.add_text_field("room_id", cjk_options.clone());
        builder.add_text_field("text", cjk_options);
        builder.add_u64_field("timestamp", NumericOptions::default().set_stored());
        builder.build()
    }

    fn from_index(index: Index, schema: &Schema) -> Result<Self> {
        index.tokenizers().register(CJK_TOKENIZER_NAME, CjkTokenizer);

        let writer = index
            .writer(50_000_000) // 50MB heap
            .map_err(|e| NextImError::Search(e.to_string()))?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| NextImError::Search(e.to_string()))?;

        let f_msg_id = schema.get_field("msg_id").unwrap();
        let f_room_id = schema.get_field("room_id").unwrap();
        let f_text = schema.get_field("text").unwrap();
        let f_timestamp = schema.get_field("timestamp").unwrap();

        Ok(Self {
            index,
            reader,
            writer: Mutex::new(writer),
            f_msg_id,
            f_room_id,
            f_text,
            f_timestamp,
        })
    }

    fn commit(&self) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|e| NextImError::Search(e.to_string()))?;
        writer.commit().map_err(|e| NextImError::Search(e.to_string()))?;
        Ok(())
    }
}

impl SearchIndex for TantivySearch {
    async fn index_message(&self, msg: &Message) -> Result<()> {
        let text = msg
            .content
            .as_ref()
            .map(|c| c.text.as_str())
            .unwrap_or("");

        if text.is_empty() {
            return Ok(());
        }

        let writer = self.writer.lock().map_err(|e| NextImError::Search(e.to_string()))?;
        writer.add_document(doc!(
            self.f_msg_id => msg.msg_id.as_str(),
            self.f_room_id => msg.room_id.as_str(),
            self.f_text => text,
            self.f_timestamp => msg.timestamp,
        )).map_err(|e| NextImError::Search(e.to_string()))?;
        drop(writer);

        self.commit()?;
        Ok(())
    }

    async fn index_messages(&self, msgs: &[Message]) -> Result<()> {
        let writer = self.writer.lock().map_err(|e| NextImError::Search(e.to_string()))?;
        for msg in msgs {
            let text = msg
                .content
                .as_ref()
                .map(|c| c.text.as_str())
                .unwrap_or("");
            if text.is_empty() {
                continue;
            }
            writer.add_document(doc!(
                self.f_msg_id => msg.msg_id.as_str(),
                self.f_room_id => msg.room_id.as_str(),
                self.f_text => text,
                self.f_timestamp => msg.timestamp,
            )).map_err(|e| NextImError::Search(e.to_string()))?;
        }
        drop(writer);

        self.commit()?;
        Ok(())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.reader.reload().map_err(|e| NextImError::Search(e.to_string()))?;
        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.f_text]);
        let parsed = query_parser
            .parse_query(query)
            .map_err(|e| NextImError::Search(e.to_string()))?;

        let top_docs = searcher
            .search(&parsed, &TopDocs::with_limit(limit))
            .map_err(|e| NextImError::Search(e.to_string()))?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_addr)
                .map_err(|e| NextImError::Search(e.to_string()))?;

            let msg_id = get_text_field(&doc, self.f_msg_id);
            let room_id = get_text_field(&doc, self.f_room_id);
            let snippet = get_text_field(&doc, self.f_text);
            let timestamp = get_u64_field(&doc, self.f_timestamp);

            results.push(SearchResult {
                msg_id,
                room_id,
                snippet,
                score,
                timestamp,
            });
        }
        Ok(results)
    }

    async fn search_in_room(
        &self,
        room_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // 组合查询: room_id:xxx AND text:query
        let combined = format!("room_id:\"{room_id}\" AND ({query})");
        self.reader.reload().map_err(|e| NextImError::Search(e.to_string()))?;
        let searcher = self.reader.searcher();
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.f_text, self.f_room_id]);
        let parsed = query_parser
            .parse_query(&combined)
            .map_err(|e| NextImError::Search(e.to_string()))?;

        let top_docs = searcher
            .search(&parsed, &TopDocs::with_limit(limit))
            .map_err(|e| NextImError::Search(e.to_string()))?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_addr)
                .map_err(|e| NextImError::Search(e.to_string()))?;

            let msg_id = get_text_field(&doc, self.f_msg_id);
            let r_id = get_text_field(&doc, self.f_room_id);
            let snippet = get_text_field(&doc, self.f_text);
            let timestamp = get_u64_field(&doc, self.f_timestamp);

            results.push(SearchResult {
                msg_id,
                room_id: r_id,
                snippet,
                score,
                timestamp,
            });
        }
        Ok(results)
    }

    async fn delete_index(&self, msg_id: &str) -> Result<()> {
        let term = tantivy::Term::from_field_text(self.f_msg_id, msg_id);
        let writer = self.writer.lock().map_err(|e| NextImError::Search(e.to_string()))?;
        writer.delete_term(term);
        drop(writer);
        self.commit()?;
        Ok(())
    }

    async fn rebuild_index(&self) -> Result<()> {
        let writer = self.writer.lock().map_err(|e| NextImError::Search(e.to_string()))?;
        writer.delete_all_documents().map_err(|e| NextImError::Search(e.to_string()))?;
        drop(writer);
        self.commit()?;
        Ok(())
    }
}

/// 从 TantivyDocument 中提取文本字段
fn get_text_field(doc: &TantivyDocument, field: Field) -> String {
    doc.get_all(field)
        .next()
        .map(OwnedValue::from)
        .and_then(|v| match v {
            OwnedValue::Str(s) => Some(s),
            _ => None,
        })
        .unwrap_or_default()
}

/// 从 TantivyDocument 中提取 u64 字段
fn get_u64_field(doc: &TantivyDocument, field: Field) -> u64 {
    doc.get_all(field)
        .next()
        .map(OwnedValue::from)
        .and_then(|v| match v {
            OwnedValue::U64(n) => Some(n),
            _ => None,
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextim_proto::message::{MessageContent, MessageType};

    fn make_msg(id: &str, room: &str, ts: u64, text: &str) -> Message {
        Message {
            msg_id: id.to_string(),
            room_id: room.to_string(),
            sender_fingerprint: "sender".to_string(),
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

    #[tokio::test]
    async fn test_index_and_search_english() {
        let search = TantivySearch::in_memory().unwrap();

        search.index_message(&make_msg("m1", "r1", 1000, "hello world")).await.unwrap();
        search.index_message(&make_msg("m2", "r1", 1001, "goodbye world")).await.unwrap();
        search.index_message(&make_msg("m3", "r1", 1002, "hello rust")).await.unwrap();

        let results = search.search("hello", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_search_in_room() {
        let search = TantivySearch::in_memory().unwrap();

        search.index_message(&make_msg("m1", "room-a", 1000, "hello world")).await.unwrap();
        search.index_message(&make_msg("m2", "room-b", 1001, "hello rust")).await.unwrap();

        let results = search.search_in_room("room-a", "hello", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].room_id, "room-a");
    }

    #[tokio::test]
    async fn test_delete_index() {
        let search = TantivySearch::in_memory().unwrap();

        search.index_message(&make_msg("m1", "r1", 1000, "hello world")).await.unwrap();
        let results = search.search("hello", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        search.delete_index("m1").await.unwrap();
        // 需要 reload 后才能看到删除效果
        let results = search.search("hello", 10).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_index() {
        let search = TantivySearch::in_memory().unwrap();

        let msgs = vec![
            make_msg("m1", "r1", 1000, "first message"),
            make_msg("m2", "r1", 1001, "second message"),
            make_msg("m3", "r1", 1002, "third message"),
        ];
        search.index_messages(&msgs).await.unwrap();

        let results = search.search("message", 10).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_chinese_search() {
        let search = TantivySearch::in_memory().unwrap();

        search.index_message(&make_msg("m1", "r1", 1000, "你好世界")).await.unwrap();
        search.index_message(&make_msg("m2", "r1", 1001, "今天天气很好")).await.unwrap();
        search.index_message(&make_msg("m3", "r1", 1002, "hello 你好")).await.unwrap();

        // 搜索 "你" 应该匹配 m1 和 m3
        let results = search.search("你", 10).await.unwrap();
        assert_eq!(results.len(), 2);

        // 搜索 "天" 应该匹配 m2
        let results = search.search("天", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].msg_id, "m2");
    }

    #[tokio::test]
    async fn test_mixed_language_search() {
        let search = TantivySearch::in_memory().unwrap();

        search.index_message(&make_msg("m1", "r1", 1000, "rust 编程语言")).await.unwrap();
        search.index_message(&make_msg("m2", "r1", 1001, "go 编程")).await.unwrap();
        search.index_message(&make_msg("m3", "r1", 1002, "rust is great")).await.unwrap();

        // 搜索英文
        let results = search.search("rust", 10).await.unwrap();
        assert_eq!(results.len(), 2); // m1 和 m3

        // 搜索中文
        let results = search.search("编", 10).await.unwrap();
        assert_eq!(results.len(), 2); // m1 和 m2
    }
}
