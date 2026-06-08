# Makefile for Lattice

.PHONY: help build test clean run-store run-peer docker-build docker-up fmt clippy check install

# Default target
help:
	@echo "Lattice Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  build        - Build all binaries in release mode"
	@echo "  test         - Run all tests"
	@echo "  clean        - Clean build artifacts"
	@echo "  run-store    - Run Store node"
	@echo "  run-peer     - Run Peer node"
	@echo "  docker-build - Build Docker images"
	@echo "  docker-up    - Start services with docker-compose"
	@echo "  fmt          - Format code"
	@echo "  clippy       - Run clippy linter"
	@echo "  check        - Run all checks (fmt, clippy, test)"
	@echo "  install      - Install binaries to system"

# Build all binaries
build:
	cargo build --release --workspace

# Run tests
test:
	cargo test --workspace

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

# Run Store node
run-store:
	cargo run --release --bin lattice-store

# Run Peer node
run-peer:
	cargo run --release --bin lattice-peer

# Build Docker images
docker-build:
	docker build -f Dockerfile.store -t lattice-store:latest .
	docker build -f Dockerfile.peer -t lattice-peer:latest .

# Start services with docker-compose
docker-up:
	docker-compose up -d

# Stop services
docker-down:
	docker-compose down

# Format code
fmt:
	cargo fmt --all

# Run clippy
clippy:
	cargo clippy --workspace --all-targets --all-features

# Run all checks
check: fmt clippy test
	@echo "All checks passed!"

# Install binaries
install:
	cargo install --path crates/lattice-store
	cargo install --path crates/lattice-peer

# Development mode (watch for changes)
dev-store:
	cargo watch -x 'run --bin lattice-store'

dev-peer:
	cargo watch -x 'run --bin lattice-peer'

# Generate documentation
docs:
	cargo doc --workspace --no-deps --open

# Run benchmarks
bench:
	cargo bench --workspace

# Check for security vulnerabilities
audit:
	cargo audit

# Update dependencies
update:
	cargo update
