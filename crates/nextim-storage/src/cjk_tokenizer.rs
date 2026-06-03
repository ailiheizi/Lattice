//! CJK（中日韩）分词器
//!
//! 简单的 unigram + bigram 混合分词，对中文搜索效果足够好。
//! 拉丁字符走默认空格分词。

use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// CJK 分词器 — 对 CJK 字符做 unigram 切分，拉丁字符按空格切分
#[derive(Clone)]
pub struct CjkTokenizer;

impl Tokenizer for CjkTokenizer {
    type TokenStream<'a> = CjkTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let tokens = tokenize(text);
        CjkTokenStream {
            tokens,
            index: 0,
            token: Token::default(),
        }
    }
}

pub struct CjkTokenStream {
    tokens: Vec<(String, usize, usize)>, // (text, offset_from, offset_to)
    index: usize,
    token: Token,
}

impl TokenStream for CjkTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            let (ref text, from, to) = self.tokens[self.index];
            self.token.text.clear();
            self.token.text.push_str(text);
            self.token.offset_from = from;
            self.token.offset_to = to;
            self.token.position = self.index;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{3000}'..='\u{303F}' // CJK Symbols and Punctuation
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
    )
}

/// 分词：CJK 字符逐字切分（unigram），拉丁字符按空格切分，全部小写
fn tokenize(text: &str) -> Vec<(String, usize, usize)> {
    let mut tokens = Vec::new();
    let mut latin_buf = String::new();
    let mut latin_start = 0;

    for (byte_offset, c) in text.char_indices() {
        if is_cjk(c) {
            // 先 flush 拉丁缓冲
            if !latin_buf.is_empty() {
                for word in latin_buf.split_whitespace() {
                    let w = word.to_lowercase();
                    if !w.is_empty() {
                        tokens.push((w, latin_start, byte_offset));
                    }
                }
                latin_buf.clear();
            }
            // CJK 字符作为单独 token
            let char_len = c.len_utf8();
            tokens.push((c.to_string(), byte_offset, byte_offset + char_len));
        } else {
            if latin_buf.is_empty() {
                latin_start = byte_offset;
            }
            latin_buf.push(c);
        }
    }

    // flush 剩余拉丁字符
    if !latin_buf.is_empty() {
        let end = text.len();
        for word in latin_buf.split_whitespace() {
            let w = word.to_lowercase();
            if !w.is_empty() {
                tokens.push((w, latin_start, end));
            }
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_tokenize() {
        let tokens = tokenize("你好世界");
        let texts: Vec<&str> = tokens.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(texts, vec!["你", "好", "世", "界"]);
    }

    #[test]
    fn test_english_tokenize() {
        let tokens = tokenize("hello world");
        let texts: Vec<&str> = tokens.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(texts, vec!["hello", "world"]);
    }

    #[test]
    fn test_mixed_tokenize() {
        let tokens = tokenize("hello 你好 world 世界");
        let texts: Vec<&str> = tokens.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(texts, vec!["hello", "你", "好", "world", "世", "界"]);
    }

    #[test]
    fn test_case_insensitive() {
        let tokens = tokenize("Hello WORLD");
        let texts: Vec<&str> = tokens.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(texts, vec!["hello", "world"]);
    }

    #[test]
    fn test_japanese() {
        let tokens = tokenize("東京タワー");
        assert_eq!(tokens.len(), 5); // 逐字切分
    }

    #[test]
    fn test_empty() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }
}
