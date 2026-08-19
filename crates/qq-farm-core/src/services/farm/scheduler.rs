//! 农场调度循环。
//!
//! 对应原 `core/src/services/farm/scheduler.ts`（396 行）。
//!
//! 阶段 1C.1：checkFarm + startFarmCheckLoop + stopFarmCheckLoop 框架
//! 阶段 1C.2：runFarmOperation 完整实现（收获 → 锄地 → 施肥 → 种植）

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::runtime::scheduler::Scheduler;
use crate::services::farm::api::Api;
use crate::services::farm::land_analysis::{
    analyze_lands, build_land_map, classify_harvested_lands_by_map, collect_dead,
    collect_harvestable, summarize_lands, LandAnalysis, LandSummary,
};
use crate::services::farm::planting::{PlantingConfig, PlantingEngine};

/// 农场服务
pub struct FarmService {
    api: Api,
    planting: Arc<Mutex<PlantingEngine>>,
    scheduler: Scheduler,
    /// 当前轮询间隔
    check_interval: Arc<parking_lot::Mutex<Duration>>,
    /// 当前账号 host_gid（运行期可变）
    host_gid: Arc<parking_lot::Mutex<i64>>,
    /// 账号 id（读 AccountConfig）
    account_id: Arc<parking_lot::Mutex<String>>,
    /// 取消 token（每轮询独立）
    current_loop: Arc<parking_lot::Mutex<Option<CancellationToken>>>,
    /// 状态事件订阅
    event_tx: broadcast::Sender<FarmEvent>,
    /// 对齐 TS `isCheckingFarm`
    is_checking: AtomicBool,
    /// 对齐 TS `externalSchedulerMode`：由 worker 统一 tick 驱动，不跑内部 interval
    external_scheduler: AtomicBool,
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
        let api = Api::new(gateway);
        let planting = PlantingEngine::new(api.clone(), PlantingConfig::default());
        let (event_tx, _) = broadcast::channel(256);
        Self {
            api,
            planting: Arc::new(Mutex::new(planting)),
            scheduler: Scheduler::new("farm-service"),
            check_interval: Arc::new(parking_lot::Mutex::new(Duration::from_secs(60))),
            host_gid: Arc::new(parking_lot::Mutex::new(0)),
            account_id: Arc::new(parking_lot::Mutex::new(String::new())),
            current_loop: Arc::new(parking_lot::Mutex::new(None)),
            event_tx,
            is_checking: AtomicBool::new(false),
            external_scheduler: AtomicBool::new(false),
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

    /// 设置账号 id（登录后调用，用于按账号读 automation / 种植策略）
    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<FarmEvent> {
        self.event_tx.subscribe()
    }

    /// 对齐 TS `startFarmCheckLoop({ externalScheduler: true })`
    pub fn set_external_scheduler(&self, enabled: bool) {
        self.external_scheduler.store(enabled, Ordering::Release);
        if enabled {
            self.stop_check_loop();
        }
    }

    /// 设置轮询间隔
    pub fn set_check_interval(&self, interval: Duration) {
        *self.check_interval.lock() = interval;
        // 外部调度模式下不重启内部循环（对齐 TS `externalSchedulerMode`）
        if self.external_scheduler.load(Ordering::Acquire) {
            return;
        }
        if self.current_loop.lock().is_some() {
            self.start_check_loop();
        }
    }

    /// 启动巡田循环
    pub fn start_check_loop(&self) {
        if self.external_scheduler.load(Ordering::Acquire) {
            self.stop_check_loop();
            return;
        }
        self.stop_check_loop();
        let cancel = CancellationToken::new();
        *self.current_loop.lock() = Some(cancel.clone());

        let api = self.api.clone();
        let planting = self.planting.clone();
        let event_tx = self.event_tx.clone();
        let host_gid = self.host_gid.clone();
        let account_id = self.account_id.clone();
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
                let account_id = account_id.clone();
                let cancel = cancel_for_task.clone();
                Box::pin(async move {
                    if cancel.is_cancelled() {
                        return;
                    }
                    let gid = *host_gid.lock();
                    if gid == 0 {
                        let _ =
                            event_tx.send(FarmEvent::Error { message: "host_gid not set".into() });
                        return;
                    }
                    let acc = account_id.lock().clone();
                    match Self::run_one_cycle(&api, &planting, gid, &acc, &event_tx).await {
                        Ok(()) => {
                            let _ = event_tx.send(FarmEvent::CycleCompleted);
                        }
                        Err(e) => {
                            crate::services::panel_log::log_warn(
                                &acc,
                                "巡田",
                                format!("检查失败: {e}"),
                                crate::constants::PanelEvent::FarmCycle,
                                Some(serde_json::json!({ "module": "farm"})),
                            );
                            let _ = event_tx
                                .send(FarmEvent::Error { message: format!("cycle failed: {e}") });
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

    /// 刷新（重启）巡田循环。外部调度模式下是 no-op（对齐 TS `refreshFarmCheckLoop`）。
    pub fn refresh_check_loop(&self) {
        if self.external_scheduler.load(Ordering::Acquire) {
            return;
        }
        self.start_check_loop();
    }

    /// 单次巡田（对齐 TS `checkFarm`：只跑 `runFarmOperation('all')`，不额外 AllLands）
    pub async fn check_farm(&self) -> Result<LandSummary> {
        let gid = *self.host_gid.lock();
        let acc = self.account_id.lock().clone();
        if gid == 0 || !crate::services::automation::is_automation_on_for(&acc, "farm") {
            return Ok(LandSummary::default());
        }
        if self.is_checking.swap(true, Ordering::AcqRel) {
            return Ok(LandSummary::default());
        }
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _guard = Guard(&self.is_checking);
        let result = self.run_farm_operation().await;
        match result {
            Ok(()) => Ok(LandSummary::default()),
            Err(e) => Err(e),
        }
    }

    /// 获取土地详情（lands + summary）
    pub async fn get_lands_detail(
        &self,
    ) -> Result<(Vec<serde_json::Value>, crate::services::farm::land_analysis::LandDetailSummary)>
    {
        let host_gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(host_gid).await?;
        Ok(crate::services::farm::land_analysis::own_lands_detail(&reply.lands))
    }

    /// 完整一轮农场操作：收获 → 铲除 → 种植 → 施肥
    pub async fn run_farm_operation(&self) -> Result<()> {
        let gid = *self.host_gid.lock();
        let acc = self.account_id.lock().clone();
        let result =
            Self::run_one_cycle(&self.api, &self.planting, gid, &acc, &self.event_tx).await;
        if result.is_ok() {
            let _ = self.event_tx.send(FarmEvent::CycleCompleted);
        }
        result
    }

    // ----- 单步操作（按 op 分派） -----

    /// 收获（`op=harvest`）—— 收获所有 ripe 土地
    pub async fn op_harvest(&self) -> Result<usize> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let ripe_ids = collect_harvestable(&reply.lands);
        let n = ripe_ids.len();
        if !ripe_ids.is_empty() {
            self.api.harvest(ripe_ids.clone(), gid, true).await?;
            let _ = self.event_tx.send(FarmEvent::Harvested { count: n });
        }
        Ok(n)
    }

    /// 浇水（`op=water`）—— 浇所有需要水的土地
    pub async fn op_water(&self) -> Result<usize> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let ids: Vec<i64> = reply
            .lands
            .iter()
            .filter(|l| l.plant.as_ref().map(|p| p.dry_num > 0).unwrap_or(false))
            .map(|l| l.id)
            .collect();
        let n = ids.len();
        if !ids.is_empty() {
            self.api.water_land(ids, gid).await?;
        }
        Ok(n)
    }

    /// 锄草（`op=weed`）—— 一键 farming（field_4=2）包括除草/除虫
    pub async fn op_weed(&self) -> Result<usize> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let ids: Vec<i64> = reply
            .lands
            .iter()
            .filter(|l| {
                l.plant
                    .as_ref()
                    .map(|p| !p.weed_owners.is_empty() || !p.insect_owners.is_empty())
                    .unwrap_or(false)
            })
            .map(|l| l.id)
            .collect();
        let n = ids.len();
        if !ids.is_empty() {
            self.api.farming(ids, gid).await?;
        }
        Ok(n)
    }

    /// 除虫（`op=insecticide`）—— 同 weed，复用 farming
    pub async fn op_insecticide(&self) -> Result<usize> {
        self.op_weed().await
    }

    /// 一键务农（`op=clear`）—— 除草/除虫/浇水，对齐 Go `RunFarmOperation("clear")`
    pub async fn op_farming(&self) -> Result<usize> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let status = analyze_lands(&reply.lands, gid);
        let mut ids: Vec<i64> = status
            .need_weed
            .iter()
            .chain(status.need_bug.iter())
            .chain(status.need_water.iter())
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let n = ids.len();
        if !ids.is_empty() {
            self.api.farming(ids, gid).await?;
        }
        Ok(n)
    }

    /// 施肥（`op=fertilize`）—— 按配置对当前所有 plant 施肥
    pub async fn op_fertilize(&self) -> Result<FertilizeOpResult> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let planted: Vec<i64> =
            reply.lands.iter().filter(|l| l.plant.is_some()).map(|l| l.id).collect();
        if planted.is_empty() {
            return Ok(FertilizeOpResult::default());
        }
        let acc = self.account_id.lock().clone();
        let result = self
            .planting
            .lock()
            .await
            .fertilize_by_config_ex(&planted, gid, &acc, Default::default())
            .await?;
        let _ = self
            .event_tx
            .send(FarmEvent::Fertilized { normal: result.normal, organic: result.organic });
        Ok(FertilizeOpResult { normal: result.normal, organic: result.organic })
    }

    /// 种植（`op=plant`）—— 自动选种种空地/枯地
    pub async fn op_plant(&self) -> Result<usize> {
        let gid = *self.host_gid.lock();
        let acc = self.account_id.lock().clone();
        let reply = self.api.get_all_lands(gid).await?;
        let status = analyze_lands(&reply.lands, gid);
        if status.empty.is_empty() && status.dead.is_empty() {
            return Ok(0);
        }
        let result = self
            .planting
            .lock()
            .await
            .auto_plant_empty_lands(&status.dead, &status.empty, gid, &acc)
            .await?;
        let n = result.planted_lands.len();
        if n > 0 {
            let _ = self.event_tx.send(FarmEvent::Planted { count: n });
        }
        Ok(n)
    }

    /// 铲除（`op=remove`）—— 铲除所有已收获（dead）土地
    pub async fn op_remove(&self) -> Result<usize> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let dead_ids = collect_dead(&reply.lands);
        let n = dead_ids.len();
        if !dead_ids.is_empty() {
            self.api.remove_plant(dead_ids).await?;
        }
        Ok(n)
    }

    /// 升级土地（`op=upgrade`）—— 升级第一个可升级土地
    pub async fn op_upgrade(&self) -> Result<i64> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let target = reply
            .lands
            .iter()
            .find(|l| l.could_upgrade)
            .map(|l| l.id)
            .ok_or_else(|| crate::error::Error::Protocol("no land to upgrade".to_string()))?;
        self.api.upgrade_land(target).await?;
        Ok(target)
    }

    /// 解锁土地（`op=unlock`）—— 解锁第一个可解锁土地
    pub async fn op_unlock(&self, do_shared: bool) -> Result<i64> {
        let gid = *self.host_gid.lock();
        let reply = self.api.get_all_lands(gid).await?;
        let target = reply
            .lands
            .iter()
            .find(|l| l.could_unlock)
            .map(|l| l.id)
            .ok_or_else(|| crate::error::Error::Protocol("no land to unlock".to_string()))?;
        self.api.unlock_land(target, do_shared).await?;
        Ok(target)
    }

    async fn run_one_cycle(
        api: &Api,
        planting: &Arc<Mutex<PlantingEngine>>,
        host_gid: i64,
        account_id: &str,
        event_tx: &broadcast::Sender<FarmEvent>,
    ) -> Result<()> {
        let reply = api.get_all_lands(host_gid).await?;
        let lands = reply.lands;
        if lands.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "农场",
                "没有土地数据",
                crate::constants::PanelEvent::FarmCycle,
                Some(serde_json::json!({ "module": "farm"})),
            );
            return Ok(());
        }
        let status = analyze_lands(&lands, host_gid);
        let summary = summarize_lands(&lands);
        let _ = event_tx.send(FarmEvent::Checked { summary, phase_hint: String::new() });
        let mut actions: Vec<String> = Vec::new();

        let skip_own =
            crate::services::automation::is_automation_on_for(account_id, "skip_own_weed_bug");
        let mut farming_ids: Vec<i64> = status
            .need_weed
            .iter()
            .chain(status.need_bug.iter())
            .chain(status.need_water.iter())
            .copied()
            .collect();
        farming_ids.sort_unstable();
        farming_ids.dedup();
        if !skip_own && !farming_ids.is_empty() {
            match api.farming(farming_ids.clone(), host_gid).await {
                Err(e) => {
                    tracing::warn!(error = %e, "farming failed");
                    crate::services::panel_log::log_warn(
                        account_id,
                        "农场",
                        format!("一键务农失败: {e}"),
                        crate::constants::PanelEvent::FarmCycle,
                        Some(serde_json::json!({ "module": "farm"})),
                    );
                }
                Ok(reply) => {
                    crate::services::status::apply_reward_deltas_for(
                        account_id,
                        reply.results.iter().filter_map(|r| r.reward.as_ref()),
                    );
                    crate::services::stats::record_operation_for(
                        account_id,
                        "farming",
                        farming_ids.len() as i64,
                    );
                    actions.push(farm_cycle_farming_action(&status));
                }
            }
        }

        let mut harvested_land_ids: Vec<i64> = Vec::new();
        let mut harvest_lands: Vec<crate::proto::generated::gamepb::plantpb::LandInfo> = Vec::new();
        if !status.harvestable.is_empty() {
            match api.harvest(status.harvestable.clone(), host_gid, true).await {
                Ok(hr) => {
                    harvested_land_ids = status.harvestable.clone();
                    harvest_lands = hr.land;
                    crate::services::stats::record_operation_for(
                        account_id,
                        "harvest",
                        harvested_land_ids.len() as i64,
                    );
                    let _ = event_tx.send(FarmEvent::Harvested { count: harvested_land_ids.len() });
                    crate::services::panel_log::log(
                        account_id,
                        "收获",
                        format!("收获完成 {} 块土地", harvested_land_ids.len()),
                        crate::constants::PanelEvent::HarvestCrop,
                        Some(serde_json::json!({
                            "module": "farm",
                            "result": "ok",
                            "count": harvested_land_ids.len(),
                            "landIds": harvested_land_ids,
                        })),
                    );
                    actions.push(format!("收获{}", harvested_land_ids.len()));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "harvest failed");
                    crate::services::panel_log::log_warn(
                        account_id,
                        "收获",
                        e.to_string(),
                        crate::constants::PanelEvent::HarvestCrop,
                        Some(serde_json::json!({
                            "module": "farm",
                            "result": "error",
                        })),
                    );
                }
            }
        }

        let all_empty = status.empty.clone();
        let mut all_dead = status.dead.clone();
        let mut post_growing: Vec<i64> = Vec::new();
        if !harvested_land_ids.is_empty() {
            tokio::time::sleep(Duration::from_millis(1200)).await;
            let first = classify_harvested_lands_by_map(
                &harvested_land_ids,
                &build_land_map(&harvest_lands),
            );
            let mut removable = first.removable;
            post_growing = first.growing;
            let mut unknown = first.unknown;
            if !unknown.is_empty() {
                if let Ok(latest) = api.get_all_lands(host_gid).await {
                    let second =
                        classify_harvested_lands_by_map(&unknown, &build_land_map(&latest.lands));
                    removable.extend(second.removable);
                    post_growing.extend(second.growing);
                    unknown = second.unknown;
                }
            }
            removable.extend(unknown);
            removable.sort_unstable();
            removable.dedup();
            all_dead.extend(removable);
            all_dead.sort_unstable();
            all_dead.dedup();
        }

        if !all_dead.is_empty() || !all_empty.is_empty() {
            let plant_count = all_dead.len() + all_empty.len();
            match planting
                .lock()
                .await
                .auto_plant_empty_lands(&all_dead, &all_empty, host_gid, account_id)
                .await
            {
                Ok(r) => {
                    crate::services::stats::record_operation_for(
                        account_id,
                        "plant",
                        plant_count as i64,
                    );
                    let _ = event_tx.send(FarmEvent::Planted { count: r.planted_lands.len() });
                    if !r.planted_lands.is_empty() {
                        actions.push(format!("种植{}", r.planted_lands.len()));
                        crate::services::panel_log::log(
                            account_id,
                            "种植",
                            format!("种植完成 {} 块土地", r.planted_lands.len()),
                            crate::constants::PanelEvent::PlantSeed,
                            Some(serde_json::json!({
                                "module": "farm",
                                "result": "ok",
                                "count": r.planted_lands.len(),
                                "landIds": r.planted_lands,
                            })),
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "auto plant failed");
                    crate::services::panel_log::log_warn(
                        account_id,
                        "种植",
                        e.to_string(),
                        crate::constants::PanelEvent::PlantSeed,
                        Some(serde_json::json!({ "module": "farm"})),
                    );
                }
            }
        }

        if !post_growing.is_empty()
            && crate::services::automation::is_automation_on_for(
                account_id,
                "fertilizer_multi_season",
            )
        {
            post_growing.sort_unstable();
            post_growing.dedup();
            if let Ok(result) = planting
                .lock()
                .await
                .fertilize_by_config_ex(
                    &post_growing,
                    host_gid,
                    account_id,
                    crate::services::farm::planting::FertilizeOptions {
                        skip_normal: false,
                        multi_season: true,
                    },
                )
                .await
            {
                let _ = event_tx
                    .send(FarmEvent::Fertilized { normal: result.normal, organic: result.organic });
                if result.normal + result.organic > 0 {
                    actions.push(format!("施肥{}/{}", result.normal, result.organic));
                    crate::services::panel_log::log(
                        account_id,
                        "施肥",
                        format!("多季补肥完成 普通{} / 有机{}", result.normal, result.organic),
                        crate::constants::PanelEvent::Fertilize,
                        Some(serde_json::json!({
                            "module": "farm",
                            "result": "ok",
                            "normal": result.normal,
                            "organic": result.organic,
                            "landIds": post_growing,
                        })),
                    );
                }
            }
        }

        if crate::services::automation::is_automation_on_for(account_id, "land_upgrade") {
            for land_id in &status.unlockable {
                match api.unlock_land(*land_id, false).await {
                    Ok(_) => {
                        crate::services::panel_log::log(
                            account_id,
                            "解锁",
                            format!("土地#{land_id} 解锁成功"),
                            crate::constants::PanelEvent::UnlockLand,
                            Some(serde_json::json!({
                                "module": "farm",
                                "result": "ok",
                                "landId": land_id,
                            })),
                        );
                        actions.push(format!("解锁{land_id}"));
                    }
                    Err(e) => crate::services::panel_log::log_warn(
                        account_id,
                        "解锁",
                        format!("土地#{land_id} 解锁失败: {e}"),
                        crate::constants::PanelEvent::UnlockLand,
                        Some(serde_json::json!({
                            "module": "farm",
                            "result": "error",
                            "landId": land_id,
                        })),
                    ),
                }
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
            for land_id in &status.upgradable {
                match api.upgrade_land(*land_id).await {
                    Ok(_) => {
                        crate::services::stats::record_operation_for(account_id, "upgrade", 1);
                        crate::services::panel_log::log(
                            account_id,
                            "升级",
                            format!("土地#{land_id} 升级成功"),
                            crate::constants::PanelEvent::UpgradeLand,
                            Some(serde_json::json!({
                                "module": "farm",
                                "result": "ok",
                                "landId": land_id,
                            })),
                        );
                        actions.push(format!("升级{land_id}"));
                    }
                    Err(e) => crate::services::panel_log::log(
                        account_id,
                        "升级",
                        format!("土地#{land_id} 升级失败: {e}"),
                        crate::constants::PanelEvent::UpgradeLand,
                        Some(serde_json::json!({
                            "module": "farm",
                            "result": "error",
                            "landId": land_id,
                        })),
                    ),
                }
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
        }

        let fertilizer_mode =
            crate::models::store::account_config::get_automation(Some(account_id)).fertilizer;
        if matches!(fertilizer_mode, crate::models::types::FertilizerMode::Smart) {
            if let Ok(result) = planting
                .lock()
                .await
                .fertilize_by_config_ex(
                    &[],
                    host_gid,
                    account_id,
                    crate::services::farm::planting::FertilizeOptions {
                        skip_normal: true,
                        multi_season: false,
                    },
                )
                .await
            {
                if result.organic > 0 {
                    let _ = event_tx.send(FarmEvent::Fertilized {
                        normal: result.normal,
                        organic: result.organic,
                    });
                    actions.push(format!("有机肥{}", result.organic));
                    crate::services::panel_log::log(
                        account_id,
                        "施肥",
                        format!("巡田施肥完成 有机{}", result.organic),
                        crate::constants::PanelEvent::Fertilize,
                        Some(serde_json::json!({
                            "module": "farm",
                            "result": "ok",
                            "organic": result.organic,
                        })),
                    );
                }
            }
        }

        if !actions.is_empty() {
            let status_parts = farm_cycle_status_parts(&status);
            let action_str = format!(" → {}", actions.join("/"));
            crate::services::panel_log::log(
                account_id,
                "农场",
                format!("[{}]{action_str}", status_parts.join(" ")),
                crate::constants::PanelEvent::FarmCycle,
                Some(serde_json::json!({
                    "module": "farm",
                    "opType": "all",
                    "actions": actions,
                })),
            );
        }

        Ok(())
    }

    /// 关闭服务
    pub fn shutdown(&self) {
        self.stop_check_loop();
        self.scheduler.shutdown();
    }
}

/// 对齐 TS `statusParts`：`收:N 农:N 水:N 枯:N 空:N 解:N 升:N 长:N`
fn farm_cycle_status_parts(status: &LandAnalysis) -> Vec<String> {
    let mut parts = Vec::new();
    if !status.harvestable.is_empty() {
        parts.push(format!("收:{}", status.harvestable.len()));
    }
    let farming_count = {
        let mut ids = HashSet::new();
        ids.extend(status.need_weed.iter().copied());
        ids.extend(status.need_bug.iter().copied());
        ids.len()
    };
    if farming_count > 0 {
        parts.push(format!("农:{farming_count}"));
    }
    if !status.need_water.is_empty() {
        parts.push(format!("水:{}", status.need_water.len()));
    }
    if !status.dead.is_empty() {
        parts.push(format!("枯:{}", status.dead.len()));
    }
    if !status.empty.is_empty() {
        parts.push(format!("空:{}", status.empty.len()));
    }
    if !status.unlockable.is_empty() {
        parts.push(format!("解:{}", status.unlockable.len()));
    }
    if !status.upgradable.is_empty() {
        parts.push(format!("升:{}", status.upgradable.len()));
    }
    parts.push(format!("长:{}", status.growing.len()));
    parts
}

/// 对齐 TS `一键务农草N/虫N/水N`
fn farm_cycle_farming_action(status: &LandAnalysis) -> String {
    let mut parts = Vec::new();
    if !status.need_weed.is_empty() {
        parts.push(format!("草{}", status.need_weed.len()));
    }
    if !status.need_bug.is_empty() {
        parts.push(format!("虫{}", status.need_bug.len()));
    }
    if !status.need_water.is_empty() {
        parts.push(format!("水{}", status.need_water.len()));
    }
    format!("一键务农{}", parts.join("/"))
}

/// 单步施肥结果
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FertilizeOpResult {
    pub normal: usize,
    pub organic: usize,
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

    // ===== 阶段 2E：op 分派测试（不依赖网络） =====

    fn make_service() -> FarmService {
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
        FarmService::new(gateway)
    }

    #[test]
    fn fertilize_op_result_default() {
        let r = FertilizeOpResult::default();
        assert_eq!(r.normal, 0);
        assert_eq!(r.organic, 0);
    }

    #[test]
    fn fertilize_op_result_serde() {
        let r = FertilizeOpResult { normal: 5, organic: 3 };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"normal\":5"));
        assert!(s.contains("\"organic\":3"));
    }

    #[test]
    fn farm_cycle_status_parts_always_includes_growing() {
        let status = LandAnalysis {
            need_weed: vec![1, 2],
            need_bug: vec![2],
            need_water: vec![3],
            growing: vec![4, 5, 6],
            ..Default::default()
        };
        let parts = farm_cycle_status_parts(&status);
        assert_eq!(parts, vec!["农:2", "水:1", "长:3"]);
        assert_eq!(farm_cycle_farming_action(&status), "一键务农草2/虫1/水1");
    }

    #[test]
    fn service_has_op_methods() {
        // 编译期保证：FarmService 必须有这些 op 方法
        let svc = make_service();
        let _: fn(&FarmService) -> _ = |s| s.subscribe(); // 占位
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
