//! `qq-farm` —— CLI 工具入口。
//!
//! 阶段 0 子命令：
//! - `demo-crypto` — 加载 tsdk.wasm，验证加密/解密往返
//!
//! 阶段 1+ 计划新增：
//! - `proto-info` — 列出已编译的 protobuf 消息
//! - `pb-decode` — 与原项目 `pnpm pb-decode` 对齐
//! - `gen-game-config` — 重新下载并解析游戏配置

mod cli;
mod commands;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    qq_farm_core::utils::logger::init();
    let cli = cli::Cli::parse();
    cli.run()
}
