//! 阶段 1C 验证：完整农场操作端到端 demo。
//!
//! 流程：
//! 1. 启动本地 mock WS server（自动回显 mock "getAllLands" 响应）
//! 2. 启动 RuntimeEngine + 1 个 worker
//! 3. worker 加载 TSDK + 连接
//! 4. 触发 farm-check，调 run_farm_operation
//! 5. 输出事件 + 统计

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use prost::Message as _;
use qq_farm_core::models::AccountSession;
use qq_farm_core::network::encryptor::{Encryptor, NoopEncryptor};
use qq_farm_core::network::gateway::{Gateway, GatewayConfig};
use qq_farm_core::proto::generated::gamepb::plantpb::{
    AllLandsReply, LandInfo, PlantInfo, PlantPhaseInfo,
};
use qq_farm_core::runtime::engine::{EngineConfig, GatewayConfigTemplate, RuntimeEngine};
use qq_farm_core::runtime::events::WorkerEvent;
use qq_farm_core::services::farm::scheduler::{FarmEvent, FarmService};

use crate::commands::mock_ws;

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
    let (port, _server_handle) = mock_ws::start_gate_mock_ws_server(Box::new(farm_gate_handler)).await;
    let mock_url = format!("ws://127.0.0.1:{port}/");
    println!("[farm-demo] mock URL: {mock_url}");

    let engine_defaults = EngineConfig::default();
    let data_root = std::env::temp_dir().join("qq-farm-rust-farm-demo");

    let gateway_template = GatewayConfigTemplate {
        server_url: mock_url.clone(),
        platform: "test".into(),
        os: "linux".into(),
        client_version: "0.1.0".into(),
        headers: HashMap::new(),
    };

    let engine = Arc::new(RuntimeEngine::assemble(EngineConfig {
        max_workers: 4,
        status_interval: Duration::from_secs(1),
        tsdk_wasm_path: engine_defaults.tsdk_wasm_path,
        data_root,
        gateway_template,
    }));

    let account = AccountSession::new("acc-farm", "demo-openid", "DemoAccount");
    engine.start_worker(account.clone())?;
    println!("[farm-demo] worker 启动: account_id={}", account.id);

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

    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("[farm-demo] 构造 FarmService + 跑一次完整操作...");
    let gateway = Arc::new(construct_gateway_for_demo(&mock_url)?);
    gateway.connect().await.context("connect")?;
    gateway.mark_online();
    println!("[farm-demo] gateway connected + marked online");
    let farm = FarmService::new(gateway.clone());
    farm.set_host_gid(args.host_gid);

    {
        let planting = farm.planting();
        let mut engine = planting.lock().await;
        let mut config = engine.config().clone();
        config.preferred_seed_id = 1001;
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

    farm.run_farm_operation()
        .await
        .context("run_farm_operation")?;
    println!("[farm-demo] run_farm_operation 完成 ✓");

    tokio::time::sleep(Duration::from_secs(args.duration_secs)).await;

    println!("[farm-demo] shutdown...");
    engine.shutdown();
    farm.shutdown();
    worker_task.abort();
    farm_task.abort();
    println!("[farm-demo] 完成");

    Ok(())
}

fn construct_gateway_for_demo(mock_url: &str) -> Result<Gateway> {
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

fn farm_gate_handler(method: &str) -> Vec<u8> {
    if method == "AllLands" {
        let lands = vec![
            LandInfo {
                id: 1,
                unlocked: true,
                level: 1,
                plant: None,
                ..Default::default()
            },
            LandInfo {
                id: 2,
                unlocked: true,
                level: 1,
                plant: None,
                ..Default::default()
            },
            LandInfo {
                id: 3,
                unlocked: true,
                level: 3,
                plant: Some(PlantInfo {
                    id: 1,
                    name: "carrot".into(),
                    phases: vec![PlantPhaseInfo {
                        phase: 3,
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            LandInfo {
                id: 4,
                unlocked: true,
                level: 2,
                plant: Some(PlantInfo {
                    id: 1,
                    name: "carrot".into(),
                    phases: vec![PlantPhaseInfo {
                        phase: 2,
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
        ];
        AllLandsReply {
            lands,
            ..Default::default()
        }
        .encode_to_vec()
    } else {
        AllLandsReply::default().encode_to_vec()
    }
}
