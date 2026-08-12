//! 阶段 1C 验证：完整农场操作端到端 demo。
//!
//! 流程：
//! 1. 启动本地 mock WS server（自动回显 mock "getAllLands" 响应）
//! 2. 启动 RuntimeEngine + 1 个 worker
//! 3. worker 加载 TSDK + 连接
//! 4. 触发 farm-check，调 run_farm_operation
//! 5. 输出事件 + 统计

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use futures::{SinkExt, StreamExt};
use qq_farm_core::models::Account;
use qq_farm_core::network::gateway::{Gateway, GatewayConfig};
use qq_farm_core::network::encryptor::{Encryptor, TsdkEncryptor};
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};
use qq_farm_core::runtime::events::WorkerEvent;
use qq_farm_core::runtime::worker_message::WorkerMessage;
use qq_farm_core::services::farm::scheduler::{FarmEvent, FarmService};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::accept_async;

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// 跑多少秒后停止
    #[arg(long, default_value_t = 8)]
    pub duration_secs: u64,

    /// 测试 host_gid
    #[arg(long, default_value_t = 1001)]
    pub host_gid: i64,
}

pub fn execute(args: Args) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create tokio runtime")?;
    rt.block_on(run_demo(args))
}

async fn run_demo(args: Args) -> Result<()> {
    println!("[farm-demo] 启动 mock WS server（返回固定 lands）...");
    let (port, _server_handle) = start_mock_ws_server().await;
    let mock_url = format!("ws://127.0.0.1:{port}/");
    println!("[farm-demo] mock URL: {mock_url}");

    // 构造引擎
    let wasm_path = std::env::var("TSDK_WASM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assets/tsdk.wasm"));
    let data_root = std::env::temp_dir().join("qq-farm-rust-farm-demo");

    let gateway_template = GatewayConfigTemplate {
        server_url: mock_url.clone(),
        platform: "test".into(),
        os: "linux".into(),
        client_version: "0.1.0".into(),
        headers: HashMap::new(),
    };

    let engine = RuntimeEngine::assemble(EngineConfig {
        max_workers: 4,
        status_interval: Duration::from_secs(1),
        tsdk_wasm_path: wasm_path,
        data_root,
        gateway_template,
    });

    let account = Account::new("acc-farm", "demo-openid", "DemoAccount");
    engine.start_worker(account.clone())?;
    println!("[farm-demo] worker 启动: account_id={}", account.id);

    // 订阅 worker 事件
    let mut worker_events = engine.subscribe_events();
    let worker_task = tokio::spawn(async move {
        while let Ok(event) = worker_events.recv().await {
            match &event {
                WorkerEvent::Started { account_id, .. } => {
                    println!("[worker] Started: {account_id}");
                }
                WorkerEvent::Stopped { account_id, reason } => {
                    println!("[worker] Stopped: {account_id} reason={reason}");
                }
                WorkerEvent::Error { account_id, message } => {
                    println!("[worker] Error: {account_id} {message}");
                }
                _ => {}
            }
        }
    });

    // 等待 TSDK 加载
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 构造 FarmService 直接调（简化：跳过 login 流程）
    // —— 实际应该从 worker 内部触发，但阶段 1C demo 直接演示逻辑
    println!("[farm-demo] 构造 FarmService + 跑一次完整操作...");
    let gateway = Arc::new(construct_gateway_for_demo(&mock_url)?);
    // 先连接 + 标记 online（demo 简化：跳过完整 login 流程）
    gateway.connect().await.context("connect")?;
    gateway.mark_online();
    println!("[farm-demo] gateway connected + marked online");
    let farm = FarmService::new(gateway.clone());
    farm.set_host_gid(args.host_gid);

    // 设置测试用 preferred_seed_id（demo 用真实种子 ID 1001）
    {
        let planting = farm.planting();
        let mut engine = planting.lock().await;
        let mut config = engine.config().clone();
        config.preferred_seed_id = 1001; // 假设是某个种子 ID
        engine.set_config(config);
    }
    let mut farm_events = farm.subscribe();

    let farm_task = tokio::spawn(async move {
        while let Ok(event) = farm_events.recv().await {
            match &event {
                FarmEvent::Checked { summary, phase_hint } => {
                    println!("[farm] Checked: phase={phase_hint}, summary={summary:?}");
                }
                FarmEvent::Harvested { count } => println!("[farm] Harvested: {count}"),
                FarmEvent::Fertilized { normal, organic } => {
                    println!("[farm] Fertilized: normal={normal}, organic={organic}")
                }
                FarmEvent::Planted { count } => println!("[farm] Planted: {count}"),
                FarmEvent::CycleCompleted => println!("[farm] CycleCompleted ✓"),
                FarmEvent::Error { message } => println!("[farm] Error: {message}"),
                FarmEvent::Removed { count } => println!("[farm] Removed: {count}"),
            }
        }
    });

    // 跑一次操作循环
    if let Err(e) = farm.run_farm_operation().await {
        println!("[farm-demo] run_farm_operation 失败: {e}");
    } else {
        println!("[farm-demo] run_farm_operation 完成 ✓");
    }

    // 持续跑 N 秒
    tokio::time::sleep(Duration::from_secs(args.duration_secs)).await;

    // 关闭
    println!("[farm-demo] shutdown...");
    engine.shutdown();
    farm.shutdown();
    worker_task.abort();
    farm_task.abort();
    println!("[farm-demo] 完成");

    Ok(())
}

/// 构造 Gateway 用 mock URL（不通过 worker 加载 TSDK，直接拿一个）
fn construct_gateway_for_demo(mock_url: &str) -> Result<Gateway> {
    // 阶段 1C demo 用 NoopEncryptor（明文透传），避免 mock server 也要跑 TSDK
    let encryptor: Arc<dyn Encryptor> = Arc::new(NoopEncryptor);
    Ok(Gateway::new(
        GatewayConfig {
            server_url: mock_url.to_string(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1.0".into(),
            auth_code: "demo".into(),
            headers: HashMap::new(),
        },
        encryptor,
    ))
}

/// Noop 加密器（明文透传，用于 mock 测试）
struct NoopEncryptor;
impl Encryptor for NoopEncryptor {
    fn encrypt(&self, plaintext: &[u8]) -> qq_farm_core::error::Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }
    fn decrypt(&self, ciphertext: &[u8]) -> qq_farm_core::error::Result<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }
}

/// Mock WS server：收到 "AllLands" → 返回 mock lands 响应；其他 echo
async fn start_mock_ws_server() -> (u16, tokio::task::JoinHandle<()>) {
    use prost::Message as _;
    use qq_farm_core::proto::generated::gatepb::{Message as GateMessage, Meta};
    use qq_farm_core::proto::generated::gamepb::plantpb::{AllLandsReply, LandInfo, PlantInfo, PlantPhaseInfo};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(mut ws) = accept_async(stream).await {
                while let Some(msg) = ws.next().await {
                    let msg = match msg {
                        Ok(m) => m,
                        Err(_) => break,
                    };
                    if let WsMessage::Binary(data) = msg {
                        // 解析请求
                        if let Ok(req) = GateMessage::decode(&data[..]) {
                            let method = req.meta.as_ref().map(|m| m.method_name.clone()).unwrap_or_default();
                            let client_seq = req.meta.as_ref().map(|m| m.client_seq).unwrap_or(0);

                            // 构造 mock 响应
                            let body = if method == "AllLands" {
                                // 返回 4 块地：2 个 Seed（可种）+ 1 个 Ripe（可收）+ 1 个 Growing
                                let lands = vec![
                                    LandInfo {
                                        id: 1, unlocked: true, level: 1, plant: None, ..Default::default()
                                    },
                                    LandInfo {
                                        id: 2, unlocked: true, level: 1, plant: None, ..Default::default()
                                    },
                                    LandInfo {
                                        id: 3, unlocked: true, level: 3,
                                        plant: Some(PlantInfo {
                                            id: 1, name: "carrot".into(),
                                            phases: vec![PlantPhaseInfo { phase: 3, ..Default::default() }],
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    },
                                    LandInfo {
                                        id: 4, unlocked: true, level: 2,
                                        plant: Some(PlantInfo {
                                            id: 1, name: "carrot".into(),
                                            phases: vec![PlantPhaseInfo { phase: 2, ..Default::default() }],
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    },
                                ];
                                AllLandsReply { lands, ..Default::default() }.encode_to_vec()
                            } else if method == "Harvest" {
                                AllLandsReply::default().encode_to_vec()
                            } else {
                                AllLandsReply::default().encode_to_vec()
                            };

                            // 构造响应 frame
                            let resp_meta = Meta {
                                service_name: req.meta.as_ref().map(|m| m.service_name.clone()).unwrap_or_default(),
                                method_name: method.clone(),
                                message_type: 2, // Response
                                client_seq,
                                server_seq: 0,
                                error_code: 0,
                                error_message: String::new(),
                                ..Default::default()
                            };
                            let resp = GateMessage {
                                meta: Some(resp_meta),
                                body: body.into(),
                                token: String::new(),
                            };
                            let resp_bytes = resp.encode_to_vec();
                            let _ = ws.send(WsMessage::Binary(resp_bytes)).await;
                        }
                    } else if matches!(msg, WsMessage::Close(_)) {
                        break;
                    }
                }
            }
        }
    });
    (port, handle)
}
