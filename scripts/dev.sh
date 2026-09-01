#!/usr/bin/env bash
# qq-farm-rust 开发启动脚本（桌面版）。
#
# 用法：
#   ./scripts/dev.sh           # 默认启动 Tauri 桌面端（含前端 dev）
#   ./scripts/dev.sh test      # 跑全量测试
#   ./scripts/dev.sh check     # cargo check + 前端 typecheck
#   ./scripts/dev.sh clean     # 清理 build artifacts + data dir

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-dev}"
DATA_DIR="${FARM_DATA_DIR:-$HOME/.qq-farm-rust}"

case "$MODE" in
    dev)
        echo "🚀 Starting qq-farm-desktop (tauri dev)..."
        echo "   Data dir: $DATA_DIR"
        echo ""
        export RUST_LOG="${RUST_LOG:-info}"
        cargo tauri dev
        ;;

    test)
        echo "🧪 Running all tests..."
        cargo test --workspace
        ;;

    check)
        echo "🔍 cargo check --workspace..."
        cargo check --workspace --all-targets
        echo "🔍 desktop-ui typecheck..."
        (cd desktop-ui && pnpm typecheck)
        ;;

    clean)
        echo "🧹 Cleaning..."
        cargo clean
        rm -rf "$DATA_DIR"
        echo "✅ Cleaned: target/ + $DATA_DIR"
        ;;

    *)
        echo "Usage: $0 {dev|test|check|clean}"
        exit 1
        ;;
esac
