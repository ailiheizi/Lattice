# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial implementation of NextIM decentralized IM system
- Store node with WebSocket server and REST API
- Peer relay node with message caching and forwarding
- End-to-end encryption (Olm for 1v1, Megolm for group chat)
- Message signing and verification (Ed25519 + SHA256)
- Three-tier trust model (Public/TOFU/Verified)
- Full-text search with CJK tokenization (Tantivy)
- DHT-based node discovery (Kademlia)
- STUN support for NAT traversal
- Multi-device synchronization
- Android FFI bindings (UniFFI)
- Web frontend interface
- Comprehensive test suite (112 tests)

### Technical Details
- Rust workspace with 8 crates
- Trait-based abstraction (Transport, Storage, SearchIndex)
- Protobuf-based protocol
- SQLite for message storage
- Tantivy for full-text search
- tokio async runtime
- WebSocket transport layer

## [0.1.0] - 2026-03-21

### Added
- Initial release
- Core functionality complete
- All 10 development phases implemented
- 103 unit tests + 9 integration tests
- Zero clippy warnings
- Complete documentation

### Components
- `nextim-proto`: Protobuf definitions and generated code
- `nextim-crypto`: Cryptography layer (identity, signing, E2EE)
- `nextim-core`: Core business logic with Trait definitions
- `nextim-transport`: WebSocket transport implementation
- `nextim-storage`: SQLite storage + Tantivy search
- `nextim-discovery`: DHT node discovery
- `nextim-store`: Store node binary
- `nextim-peer`: Peer relay node binary
- `nextim-ffi`: Android FFI bindings
- `nextim-tests`: Integration test suite

### Security
- Ed25519 + Curve25519 key pairs
- Mandatory message signing
- Optional E2EE encryption
- Three-tier trust model
- Secure key storage

### Performance
- Async architecture with tokio
- Efficient message routing
- LRU cache with TTL in Peer nodes
- Optimized SQLite queries
- Fast full-text search

### Documentation
- Complete README with quick start guide
- Architecture documentation
- Implementation plan
- API documentation
- Test coverage report

[Unreleased]: https://github.com/yourusername/NextIM/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/NextIM/releases/tag/v0.1.0
