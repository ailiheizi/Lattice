#!/bin/bash
# Lattice Android 构建脚本
# 前置条件：
#   1. rustup target add aarch64-linux-android
#   2. cargo install cargo-ndk
#   3. 设置 ANDROID_NDK_HOME 环境变量

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "=== Building Lattice FFI for Android ==="

# 检查工具链
if ! command -v cargo-ndk &> /dev/null; then
    echo "Error: cargo-ndk not found. Install with: cargo install cargo-ndk"
    exit 1
fi

if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "Error: ANDROID_NDK_HOME not set"
    exit 1
fi

# 添加 Android target（如果没有）
rustup target add aarch64-linux-android 2>/dev/null || true

# 编译
cd "$PROJECT_ROOT"
cargo ndk -t aarch64-linux-android -o android/app/src/main/jniLibs build -p lattice-ffi --release

# 生成 Kotlin 绑定
cargo run -p uniffi-bindgen -- generate \
    crates/lattice-ffi/src/lattice.udl \
    --language kotlin \
    --out-dir android/app/src/main/java/

echo "=== Build complete ==="
echo "  .so: android/app/src/main/jniLibs/arm64-v8a/liblattice_ffi.so"
echo "  Kotlin: android/app/src/main/java/com/lattice/"
