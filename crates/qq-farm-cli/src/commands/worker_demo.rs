//! 阶段 1B 验证：启动 1 个 worker，端到端演示。
//!
//! 流程：
//! 1. 启动本地 mock WS server
//! 2. 启动 `RuntimeEngine`
//! 3. 添加 1 个账号（用 mock URL 当 server_url）
//! 4. 给 worker 发 Connect 消息
//! 5. 订阅 WorkerEvent 流，统计事件
//! 6. 跑 N 秒后停止，输出汇总

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use qq_farm_core::models::AccountSession;
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};
use qq_farm_core::runtime::events::WorkerEvent;

use crate::commands::mock_ws;

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// 跑多少秒后停止
    #[arg(long, default_value_t = 5)]
    pub duration_secs: u64,
}

pub fn execute(args: Args) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create tokio runtime")?;
    rt.block_on(run_demo(args))
}

async fn run_demo(args: Args) -> Result<()> {
    println!("[worker-demo] 启动 mock WS server...");
    let (port, _server_handle) = mock_ws::start_echo_mock_ws_server().await;
    let mock_url = format!("ws://127.0.0.1:{port}/");
    println!("[worker-demo] mock URL: {mock_url}");

    let engine_defaults = EngineConfig::default();
    let data_root = std::env::temp_dir().join("qq-farm-rust-worker-demo");

    let config = EngineConfig {
        max_workers: 4,
        status_interval: Duration::from_secs(1),
        tsdk_wasm_path: engine_defaults.tsdk_wasm_path,
        data_root: data_root.clone(),
        gateway_template: GatewayConfigTemplate {
            server_url: mock_url.clone(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1.0".into(),
            headers: HashMap::new(),
        },
    };

    let engine = Arc::new(RuntimeEngine::assemble(config));
    let mut events = engine.subscribe_events();

    let account = AccountSession::new("acc-demo", "demo-openid", "DemoAccount");
    engine.start_worker(account.clone())?;
    println!("[worker-demo] worker 启动: account_id={}", account.id);

    let event_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let event_count_clone = event_count.clone();
    let event_task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            event_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match &event {
                WorkerEvent::Started {
                    account_id,
                    account_name,
                } => {
                    println!("[event] Started: {account_id} / {account_name}");
                }
                WorkerEvent::Stopped { account_id, reason } => {
                    println!("[event] Stopped: {account_id} reason={reason}");
                }
                WorkerEvent::Error { account_id, message } => {
                    println!("[event] Error: {account_id} {message}");
                }
                WorkerEvent::Status {
                    account_id,
                    account_name: _,
                    status,
                } => {
                    println!("[event] Status: {account_id} {status}");
                }
                WorkerEvent::Log {
                    account_id,
                    level,
                    message,
                    ..
                } => {
                    println!("[event] Log: {account_id} [{level}] {message}");
                }
                WorkerEvent::Schedulers { .. } => {}
            }
        }
    });

    println!("[worker-demo] 跑 {} 秒后停止...", args.duration_secs);
    tokio::time::sleep(Duration::from_secs(args.duration_secs)).await;

    println!("[worker-demo] shutdown...");
    engine.shutdown();

    tokio::time::sleep(Duration::from_millis(500)).await;
    event_task.abort();

    let total = event_count.load(std::sync::atomic::Ordering::SeqCst);
    println!("[worker-demo] 完成，共收到 {total} 个事件");

    Ok(())
}
