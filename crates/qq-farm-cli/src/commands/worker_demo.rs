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
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use futures::{SinkExt, StreamExt};
use qq_farm_core::models::Account;
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};
use qq_farm_core::runtime::events::WorkerEvent;
use qq_farm_core::runtime::worker_message::WorkerMessage;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::accept_async;

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
    let (port, _server_handle) = start_mock_ws_server().await;
    let mock_url = format!("ws://127.0.0.1:{port}/");
    println!("[worker-demo] mock URL: {mock_url}");

    // 构造引擎
    let wasm_path = std::env::var("TSDK_WASM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assets/tsdk.wasm"));
    let data_root = std::env::temp_dir().join("qq-farm-rust-worker-demo");

    let config = EngineConfig {
        max_workers: 4,
        status_interval: Duration::from_secs(1),
        tsdk_wasm_path: wasm_path,
        data_root: data_root.clone(),
        gateway_template: GatewayConfigTemplate {
            server_url: mock_url.clone(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1.0".into(),
            headers: HashMap::new(),
        },
    };

    let engine = RuntimeEngine::new(config);
    let mut events = engine.subscribe_events();

    // 启动 worker
    let account = Account::new("acc-demo", "demo-openid", "DemoAccount");
    engine.start_worker(account.clone())?;
    println!("[worker-demo] worker 启动: account_id={}", account.id);

    // 拿到 handle
    // 阶段 1B 简化：start_worker 后 handle 在 engine 内部，我们通过 broadcast events 监听
    // 后续会暴露 engine.worker_handles() 方法

    // 订阅事件
    let event_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let event_count_clone = event_count.clone();
    let event_task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            event_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match &event {
                WorkerEvent::Started { account_id, account_name } => {
                    println!("[event] Started: {account_id} / {account_name}");
                }
                WorkerEvent::Stopped { account_id, reason } => {
                    println!("[event] Stopped: {account_id} reason={reason}");
                }
                WorkerEvent::Error { account_id, message } => {
                    println!("[event] Error: {account_id} {message}");
                }
                WorkerEvent::Status { account_id, account_name: _, status } => {
                    println!("[event] Status: {account_id} {status}");
                }
                WorkerEvent::Log { account_id, level, message, .. } => {
                    println!("[event] Log: {account_id} [{level}] {message}");
                }
                WorkerEvent::Schedulers { .. } => {}
            }
        }
    });

    // 跑 N 秒
    println!("[worker-demo] 跑 {} 秒后停止...", args.duration_secs);
    tokio::time::sleep(Duration::from_secs(args.duration_secs)).await;

    // 关闭所有 worker
    println!("[worker-demo] shutdown...");
    engine.shutdown();

    // 等一下让 task 退出
    tokio::time::sleep(Duration::from_millis(500)).await;
    event_task.abort();

    let total = event_count.load(std::sync::atomic::Ordering::SeqCst);
    println!("[worker-demo] 完成，共收到 {total} 个事件");

    Ok(())
}

use std::sync::Arc;

/// 启动 mock WS server：accept 一个连接，回显所有收到的 binary 帧（加 "DEMO:" 前缀）
async fn start_mock_ws_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(ws) = accept_async(stream).await {
                let mut ws = ws;
                // 立刻发一个 binary 帧（模拟登录响应之类）
                let _ = ws.send(WsMessage::Binary(b"welcome".to_vec())).await;
                while let Some(msg) = ws.next().await {
                    if let Ok(WsMessage::Binary(data)) = msg {
                        let mut echoed = b"DEMO:".to_vec();
                        echoed.extend_from_slice(&data);
                        if ws.send(WsMessage::Binary(echoed)).await.is_err() {
                            break;
                        }
                    } else if matches!(msg, Ok(WsMessage::Close(_))) {
                        break;
                    }
                }
            }
        }
    });
    (port, handle)
}
