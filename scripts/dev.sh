#!/usr/bin/env bash
# qq-farm-rust 开发启动脚本。
#
# 用法：
#   ./scripts/dev.sh           # 默认 debug 模式启动
#   ./scripts/dev.sh release   # release 模式启动
#   ./scripts/dev.sh test      # 跑全量测试
#   ./scripts/dev.sh e2e       # 跑 E2E 测试
#   ./scripts/dev.sh clean     # 清理 build artifacts + data dir

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-debug}"
DATA_DIR="${FARM_DATA_DIR:-$HOME/.qq-farm-rust}"

case "$MODE" in
    debug)
        echo "🚀 Starting qq-farm-server (debug) on port 3007..."
        echo "   Data dir: $DATA_DIR"
        echo "   Health: http://localhost:3007/health"
        echo "   WS:      ws://localhost:3007/ws"
        echo ""
        export RUST_LOG="${RUST_LOG:-info}"
        export ADMIN_PORT="${ADMIN_PORT:-3007}"
        cargo run -p qq-farm-server
        ;;

    release)
        echo "🚀 Building release..."
        cargo build --release
        echo "🚀 Starting qq-farm-server (release)..."
        export RUST_LOG="${RUST_LOG:-info}"
        export ADMIN_PORT="${ADMIN_PORT:-3007}"
        ./target/release/qq-farm-server
        ;;

    test)
        echo "🧪 Running all tests..."
        cargo test --workspace
        ;;

    e2e)
        echo "🧪 Running E2E tests..."
        cargo test -p qq-farm-server --test e2e_integration
        ;;

    clean)
        echo "🧹 Cleaning..."
        cargo clean
        rm -rf "$DATA_DIR"
        echo "✅ Cleaned: target/ + $DATA_DIR"
        ;;

    *)
        echo "Usage: $0 {debug|release|test|e2e|clean}"
        exit 1
        ;;
esac
