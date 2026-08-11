//! Runtime 引擎。
//!
//! 顶层入口，管理多个 Worker。
//!
//! - [`RuntimeEngine::new`] —— 创建
//! - [`RuntimeEngine::start_worker`] —— 启动一个 worker
//! - [`RuntimeEngine::stop_worker`] / [`RuntimeEngine::restart_worker`] —— 控制
//! - [`RuntimeEngine::subscribe_events`] —— 订阅 worker 生命周期事件
//! - [`RuntimeEngine::broadcast_config`] —— 广播配置到所有 worker
//!
//! 不起 HTTP / Socket.io，这些由 server crate 注入。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::broadcast;

use crate::error::Result;
use crate::models::Account;
use crate::network::gateway::GatewayConfig;
use crate::runtime::events::WorkerEvent;
use crate::runtime::worker::{Worker, WorkerConfig};
use crate::runtime::worker_handle::WorkerHandle;

/// 引擎配置
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// 最多同时跑多少个 worker
    pub max_workers: usize,
    /// 状态上报间隔
    pub status_interval: Duration,
    /// TSDK wasm 路径
    pub tsdk_wasm_path: PathBuf,
    /// 数据根目录（每个 worker 一个子目录）
    pub data_root: PathBuf,
    /// 网关配置模板（每个 worker 的 `GatewayConfig` 由模板 + 账号 code 组合）
    pub gateway_template: GatewayConfigTemplate,
}

/// 网关配置模板（不含 code）
#[derive(Debug, Clone)]
pub struct GatewayConfigTemplate {
    pub server_url: String,
    pub platform: String,
    pub os: String,
    pub client_version: String,
    pub headers: std::collections::HashMap<String, String>,
}

/// Worker 摘要信息
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub account_id: String,
    pub account_name: String,
    pub running: bool,
}

/// Runtime 引擎
pub struct RuntimeEngine {
    config: EngineConfig,
    workers: Arc<RwLock<HashMap<String, WorkerHandle>>>,
    events: broadcast::Sender<WorkerEvent>,
}

impl RuntimeEngine {
    /// 创建引擎
    #[must_use]
    pub fn new(config: EngineConfig) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            config,
            workers: Arc::new(RwLock::new(HashMap::new())),
            events,
        }
    }

    /// 订阅 worker 事件
    pub fn subscribe_events(&self) -> broadcast::Receiver<WorkerEvent> {
        self.events.subscribe()
    }

    /// 当前 worker 数
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.read().len()
    }

    /// 列出所有 worker
    #[must_use]
    pub fn list_workers(&self) -> Vec<WorkerInfo> {
        self.workers
            .read()
            .values()
            .map(|h| WorkerInfo {
                account_id: h.account_id.clone(),
                account_name: h.account_id.clone(), // 阶段 1B 简化
                running: !h.is_cancelled(),
            })
            .collect()
    }

    /// 启动一个 worker
    pub fn start_worker(&self, account: Account) -> Result<()> {
        let max = self.config.max_workers;
        {
            let workers = self.workers.read();
            if workers.contains_key(&account.id) {
                tracing::warn!(account_id = %account.id, "worker already running");
                return Ok(());
            }
            if workers.len() >= max {
                return Err(crate::error::Error::Internal(format!(
                    "max workers reached ({max})"
                )));
            }
        }

        let gateway_config = GatewayConfig {
            server_url: self.config.gateway_template.server_url.clone(),
            platform: self.config.gateway_template.platform.clone(),
            os: self.config.gateway_template.os.clone(),
            client_version: self.config.gateway_template.client_version.clone(),
            auth_code: account.open_id.clone(),
            headers: self.config.gateway_template.headers.clone(),
        };

        let worker_config = WorkerConfig {
            gateway: gateway_config,
            status_interval: self.config.status_interval,
            tsdk_wasm_path: self.config.tsdk_wasm_path.clone(),
            data_dir: self.config.data_root.clone(),
        };

        let worker = Worker::new(account, worker_config, self.events.clone());
        let handle = worker.spawn();
        self.workers.write().insert(handle.account_id.clone(), handle);
        Ok(())
    }

    /// 停止一个 worker
    pub fn stop_worker(&self, account_id: &str) {
        let handle = self.workers.write().remove(account_id);
        if let Some(h) = handle {
            h.cancel();
            tracing::info!(account_id, "worker stop requested");
        }
    }

    /// 重启一个 worker
    pub fn restart_worker(&self, account: Account) -> Result<()> {
        self.stop_worker(&account.id);
        self.start_worker(account)
    }

    /// 广播 ReloadConfig 消息到所有 worker
    pub fn broadcast_config(&self, _config: serde_json::Value) {
        use crate::runtime::worker_message::WorkerMessage;
        let workers = self.workers.read();
        for h in workers.values() {
            let _ = h.try_send(WorkerMessage::ReloadConfig);
        }
    }

    /// 广播 Disconnect 消息
    pub fn broadcast_disconnect(&self) {
        use crate::runtime::worker_message::WorkerMessage;
        let workers = self.workers.read();
        for h in workers.values() {
            let _ = h.try_send(WorkerMessage::Disconnect);
        }
    }

    /// 关闭所有 worker
    pub fn shutdown(&self) {
        let handles: Vec<_> = self.workers.write().drain().collect();
        for (_, h) in handles {
            h.cancel();
        }
    }
}
