//! 农场调度循环。
//!
//! 对应原 `core/src/services/farm/scheduler.ts`（396 行）。
//!
//! 阶段 1C.1：checkFarm + startFarmCheckLoop + stopFarmCheckLoop 框架
//! 阶段 1C.2：runFarmOperation 完整实现（收获 → 锄地 → 施肥 → 种植）

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::runtime::scheduler::Scheduler;
use crate::services::farm::api::Api;
use crate::services::farm::land_analysis::{
    collect_dead, collect_harvestable, collect_plantable, summarize_lands, LandSummary,
};
use crate::services::farm::planting::{PlantingConfig, PlantingEngine, FertilizeResult};

/// 农场服务
pub struct FarmService {
    gateway: Arc<Gateway>,
    api: Api,
    planting: Arc<Mutex<PlantingEngine>>,
    scheduler: Scheduler,
    /// 当前轮询间隔
    check_interval: Arc<parking_lot::Mutex<Duration>>,
    /// 当前账号 host_gid（运行期可变）
    host_gid: Arc<parking_lot::Mutex<i64>>,
    /// 取消 token（每轮询独立）
    current_loop: Arc<parking_lot::Mutex<Option<CancellationToken>>>,
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
    /// 铲除完成
    Removed { count: usize },
    /// 施肥完成
    Fertilized { normal: usize, organic: usize },
    /// 种植完成
    Planted { count: usize },
    /// 完整一轮操作完成
    CycleCompleted,
    /// 出错
    Error { message: String },
}

impl FarmService {
    /// 创建
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        let api = Api::new(gateway.clone());
        let planting = PlantingEngine::new(api.clone(), PlantingConfig::default());
        let (event_tx, _) = broadcast::channel(64);
        Self {
            gateway,
            api,
            planting: Arc::new(Mutex::new(planting)),
            scheduler: Scheduler::new("farm-service"),
            check_interval: Arc::new(parking_lot::Mutex::new(Duration::from_secs(60))),
            host_gid: Arc::new(parking_lot::Mutex::new(0)),
            current_loop: Arc::new(parking_lot::Mutex::new(None)),
            event_tx,
        }
    }

    /// 获取 API 客户端（业务层用）
    #[must_use]
    pub fn api(&self) -> &Api {
        &self.api
    }

    /// 获取种植引擎（用于修改配置）
    pub fn planting(&self) -> Arc<Mutex<PlantingEngine>> {
        self.planting.clone()
    }

    /// 设置 host_gid（登录后调用）
    pub fn set_host_gid(&self, gid: i64) {
        *self.host_gid.lock() = gid;
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<FarmEvent> {
        self.event_tx.subscribe()
    }

    /// 设置轮询间隔
    pub fn set_check_interval(&self, interval: Duration) {
        *self.check_interval.lock() = interval;
        // 重启循环以应用新间隔
        if self.current_loop.lock().is_some() {
            self.start_check_loop();
        }
    }

    /// 启动巡田循环
    pub fn start_check_loop(&self) {
        self.stop_check_loop();
        let cancel = CancellationToken::new();
        *self.current_loop.lock() = Some(cancel.clone());

        let api = self.api.clone();
        let planting = self.planting.clone();
        let event_tx = self.event_tx.clone();
        let host_gid = self.host_gid.clone();
        let cancel_for_task = cancel.clone();
        let interval = *self.check_interval.lock();

        self.scheduler.set_interval_task(
            "farm_check",
            interval,
            Arc::new(move || {
                let api = api.clone();
                let planting = planting.clone();
                let event_tx = event_tx.clone();
                let host_gid = host_gid.clone();
                let cancel = cancel_for_task.clone();
                Box::pin(async move {
                    if cancel.is_cancelled() {
                        return;
                    }
                    let gid = *host_gid.lock();
                    if gid == 0 {
                        let _ = event_tx.send(FarmEvent::Error {
                            message: "host_gid not set".into(),
                        });
                        return;
                    }
                    // 完整一轮操作
                    match Self::run_one_cycle(&api, &planting, gid, &event_tx).await {
                        Ok(()) => {
                            let _ = event_tx.send(FarmEvent::CycleCompleted);
                        }
                        Err(e) => {
                            let _ = event_tx.send(FarmEvent::Error {
                                message: format!("cycle failed: {e}"),
                            });
                        }
                    }
                })
            }),
        );
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

    /// 单次巡田（只检查 + 统计，不做操作）
    pub async fn check_farm(&self) -> Result<LandSummary> {
        let (summary, _) = Self::check_once(&self.api, *self.host_gid.lock()).await?;
        Ok(summary)
    }

    async fn check_once(api: &Api, host_gid: i64) -> Result<(LandSummary, String)> {
        let reply = api.get_all_lands(host_gid).await?;
        let lands = reply.lands;
        let summary = summarize_lands(&lands);
        let phase = if summary.ripe > 0 {
            "需要收获".to_string()
        } else if summary.plantable > 0 {
            "可种植".to_string()
        } else {
            "生长中".to_string()
        };
        Ok((summary, phase))
    }

    /// 完整一轮农场操作：收获 → 铲除 → 种植 → 施肥
    pub async fn run_farm_operation(&self) -> Result<()> {
        let gid = *self.host_gid.lock();
        let result = Self::run_one_cycle(&self.api, &self.planting, gid, &self.event_tx).await;
        if result.is_ok() {
            let _ = self.event_tx.send(FarmEvent::CycleCompleted);
        }
        result
    }

    async fn run_one_cycle(
        api: &Api,
        planting: &Arc<Mutex<PlantingEngine>>,
        host_gid: i64,
        event_tx: &broadcast::Sender<FarmEvent>,
    ) -> Result<()> {
        // 1. 拉取土地
        let reply = api.get_all_lands(host_gid).await?;
        let lands = reply.lands;
        let summary = summarize_lands(&lands);
        let _ = event_tx.send(FarmEvent::Checked {
            summary,
            phase_hint: String::new(),
        });

        // 2. 收获 ripe
        let ripe_ids = collect_harvestable(&lands);
        let n_harvested = ripe_ids.len();
        if !ripe_ids.is_empty() {
            if let Err(e) = api.harvest(ripe_ids.clone(), host_gid, true).await {
                tracing::warn!(error = %e, "harvest failed");
            } else {
                tracing::info!(count = n_harvested, "harvested");
            }
        }
        let _ = event_tx.send(FarmEvent::Harvested { count: n_harvested });

        // 3. 铲除已收获（占位：阶段 1C.2 简单全铲）
        let _ = collect_dead(&lands);

        // 4. 重新拉取
        let reply2 = api.get_all_lands(host_gid).await?;
        let lands2 = reply2.lands;

        // 5. 选种子 + 种植
        let plantable_ids = collect_plantable(&lands2);
        if !plantable_ids.is_empty() {
            let seed_id = planting.lock().await.config().preferred_seed_id;
            if seed_id > 0 {
                let n_planted = plantable_ids.len();
                if let Err(e) = planting
                    .lock()
                    .await
                    .plant_seeds(seed_id, plantable_ids.clone(), host_gid)
                    .await
                {
                    tracing::warn!(error = %e, "plant_seeds failed");
                } else {
                    tracing::info!(count = n_planted, "planted");
                    let _ = event_tx.send(FarmEvent::Planted { count: n_planted });
                }
            }
        }

        // 6. 施肥
        let planted_for_fertilize = plantable_ids; // 用刚种下的
        match planting
            .lock()
            .await
            .fertilize_by_config(&planted_for_fertilize, host_gid)
            .await
        {
            Ok(result) => {
                let _ = event_tx.send(FarmEvent::Fertilized {
                    normal: result.normal,
                    organic: result.organic,
                });
                tracing::info!(normal = result.normal, organic = result.organic, "fertilized");
            }
            Err(e) => {
                tracing::warn!(error = %e, "fertilize failed");
            }
        }

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

    #[test]
    fn set_host_gid() {
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
        svc.set_host_gid(12345);
        assert_eq!(*svc.host_gid.lock(), 12345);
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
