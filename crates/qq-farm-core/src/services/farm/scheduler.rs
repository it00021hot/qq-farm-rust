//! 农场调度循环。
//!
//! 对应原 `core/src/services/farm/scheduler.ts`（396 行）。
//!
//! 阶段 1C.1 范围：框架（checkFarm / startFarmCheckLoop / stopFarmCheckLoop）。
//! 阶段 1C.2 范围：runFarmOperation 完整实现（收获 → 锄地 → 施肥 → 种植）。

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::runtime::scheduler::Scheduler;
use crate::services::farm::api::Api;
use crate::services::farm::land_analysis::LandSummary;

/// 农场服务
pub struct FarmService {
    gateway: Arc<Gateway>,
    api: Api,
    scheduler: Scheduler,
    /// 当前轮询间隔
    check_interval: Arc<Mutex<Duration>>,
    /// 取消 token（每轮询独立）
    current_loop: Arc<Mutex<Option<CancellationToken>>>,
    /// 状态事件订阅
    event_tx: broadcast::Sender<FarmEvent>,
}

/// 农场服务事件
#[derive(Debug, Clone)]
pub enum FarmEvent {
    /// 巡田发现
    Checked { summary: LandSummary, phase_hint: String },
    /// 收获完成
    Harvested { count: usize },
    /// 施肥完成
    Fertilized { count: usize },
    /// 种植完成
    Planted { count: usize },
    /// 出错
    Error { message: String },
}

impl FarmService {
    /// 创建
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            api: Api::new(gateway.clone()),
            gateway,
            scheduler: Scheduler::new("farm-service"),
            check_interval: Arc::new(Mutex::new(Duration::from_secs(60))),
            current_loop: Arc::new(Mutex::new(None)),
            event_tx,
        }
    }

    /// 获取 API 客户端（业务层用）
    #[must_use]
    pub fn api(&self) -> &Api {
        &self.api
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<FarmEvent> {
        self.event_tx.subscribe()
    }

    /// 启动巡田循环
    pub fn start_check_loop(&self) {
        self.stop_check_loop();
        let cancel = CancellationToken::new();
        *self.current_loop.lock() = Some(cancel.clone());

        let api = self.api.clone();
        let event_tx = self.event_tx.clone();
        let current_loop = self.current_loop.clone();
        let interval = *self.check_interval.lock();

        self.scheduler.set_interval_task(
            "farm_check",
            interval,
            Arc::new(move || {
                let api = api.clone();
                let event_tx = event_tx.clone();
                Box::pin(async move {
                    match Self::check_once(&api).await {
                        Ok((summary, phase)) => {
                            let _ = event_tx.send(FarmEvent::Checked { summary, phase_hint: phase });
                        }
                        Err(e) => {
                            let _ = event_tx.send(FarmEvent::Error {
                                message: format!("check failed: {e}"),
                            });
                        }
                    }
                })
            }),
        );

        // 暂不实现 refresh —— 阶段 1C.1 简化
        let _ = current_loop;
    }

    /// 停止巡田循环
    pub fn stop_check_loop(&self) {
        if let Some(token) = self.current_loop.lock().take() {
            token.cancel();
        }
        self.scheduler.clear("farm_check");
    }

    /// 刷新（重启）巡田循环
    pub fn refresh_check_loop(&self) {
        self.start_check_loop();
    }

    /// 单次巡田
    pub async fn check_farm(&self) -> Result<LandSummary> {
        let (summary, _) = Self::check_once(&self.api).await?;
        Ok(summary)
    }

    async fn check_once(api: &Api) -> Result<(LandSummary, String)> {
        let reply = api.get_all_lands(0).await?;
        let lands = reply.lands;
        let summary = crate::services::farm::land_analysis::summarize_lands(&lands);
        let phase = if summary.ripe > 0 {
            "需要收获".to_string()
        } else if summary.plantable > 0 {
            "可种植".to_string()
        } else {
            "生长中".to_string()
        };
        Ok((summary, phase))
    }

    /// 执行一次农场操作（收获 + 锄地 + 施肥 + 种植）
    ///
    /// 阶段 1C.2 实现
    pub async fn run_farm_operation(&self) -> Result<()> {
        let _ = self.gateway;
        Ok(())
    }

    /// 关闭服务
    pub fn shutdown(&self) {
        self.stop_check_loop();
        self.scheduler.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn event_channel_works() {
        let gateway = Arc::new(crate::network::gateway::Gateway::new(
            crate::network::gateway::GatewayConfig {
                server_url: "ws://localhost".into(),
                platform: "test".into(),
                os: "linux".into(),
                client_version: "0.1.0".into(),
                auth_code: "x".into(),
                headers: Default::default(),
            },
            Arc::new(NoopEncryptor),
        ));
        let svc = FarmService::new(gateway);
        let mut rx = svc.subscribe();
        // 手动 send 测试
        let _ = svc.event_tx.send(FarmEvent::Checked {
            summary: LandSummary::default(),
            phase_hint: "test".into(),
        });
        let event = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("recv");
        match event {
            FarmEvent::Checked { phase_hint, .. } => assert_eq!(phase_hint, "test"),
            _ => panic!("expected Checked"),
        }
    }
}

// NoopEncryptor for test
pub struct NoopEncryptor;
impl crate::network::encryptor::Encryptor for NoopEncryptor {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }
}
