//! NextIM 节点发现
//!
//! 当前 crate 只提供轻量级 Kademlia 风格 DHT / identity-card 原语，
//! 供后续运行时集成使用。
//! 目前 `nextim-store` / `nextim-peer` 尚未依赖或接线本 crate，
//! 因此它还不能被视为已落地的运行时节点发现能力。
//! - key: 公钥 SHA-256 指纹
//! - value: Store 地址（WebSocket URL）

pub mod dht;
pub mod identity_card;
pub mod service;
