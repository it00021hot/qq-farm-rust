//! 调试命令：用已确认的 `login_buffer` 走 MMTLS 原生协议拿 `wx.login` code。
//!
//! 用于扫码登录调试时，服务端 task 可能因 TTL 过期被删，但 `login_buffer` 已由
//! 手动 confirm 流程拿到。此命令绕过 HTTP QR 轮询，直接调用
//! `native_protocol::get_native_wx_login_code`。
//!
//! 用法：
//!   qq-farm wx-code --login-buffer '<base64>' --app-id wx5306c5978fdb76e4

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use qq_farm_core::constants::WX_MINI_APP_ID;

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// 已确认授权的 login_buffer（base64，来自 confirm 流程）
    #[arg(long)]
    pub login_buffer: String,

    /// 目标小程序 app_id
    #[arg(long, default_value = WX_MINI_APP_ID)]
    pub app_id: String,
}

pub fn execute(args: Args) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create tokio runtime")?;
    rt.block_on(run(args))
}

async fn run(args: Args) -> Result<()> {
    let code = qq_farm_core::services::wx_login::native_protocol::get_native_wx_login_code(
        &args.login_buffer,
        &args.app_id,
    )
    .await
    .map_err(|e| anyhow::anyhow!("MMTLS 拿 wx.login code 失败: {e}"))?;

    println!("wx.login code: {code}");
    Ok(())
}
