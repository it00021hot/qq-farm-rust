//! CLI 命令定义。

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands;

/// `qq-farm` 顶层 CLI
#[derive(Debug, Parser)]
#[command(
    name = "qq-farm",
    version,
    about = "QQ 农场 Rust 重写版 CLI",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// 顶层命令枚举
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 加载 tsdk.wasm，跑一次加密 + 解密往返（阶段 0 验证用）
    #[command(name = "demo-crypto")]
    DemoCrypto(commands::demo_crypto::CryptoArgs),
    /// 启动 1 个 worker 连上 mock server，端到端验证（阶段 1B 验证用）
    #[command(name = "worker-demo")]
    WorkerDemo(commands::worker_demo::Args),
}

impl Cli {
    /// 执行命令
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::DemoCrypto(args) => commands::demo_crypto::execute(args),
            Command::WorkerDemo(args) => commands::worker_demo::execute(args),
        }
    }
}
