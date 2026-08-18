//! 活动中心 — 4 个生效活动的 RPC + 业务编排。
//!
//! 1:1 翻译原 `core/src/services/activity-center.ts` 的有效部分（原 1034 行，
//! 按用户决策只复刻生效的活动，不做 1:1 全搬）。被跳过的：
//!
//! - 复杂 `constellation-*.json` catalog 静态数据（运行时按需从 `reply.constellation` 拿）
//! - 256 行 `activity-center-state.ts` JSON 状态合并（见 `activity_center_state` 模块）
//! - `serializeMutation` 复杂并发（defer，rate limiter 已在 1F-6 覆盖）

mod constellation;
mod dto;
mod error;
mod qingmei;
mod rpc;
mod season;
mod shop;

#[cfg(test)]
pub(crate) use dto::{
    bytes_to_text, constellation_catalog_groups, constellation_catalog_version, constellation_dto,
    extract_goods, force_qingmei_seed_claimed_in_snapshot, is_qingmei_already_claimed_message,
    json_positive_decimal, qingmei_seed_claimed_path, text_content,
};

#[cfg(test)]
mod tests;

pub use error::{ActivityError, ActivityErrorCode};

pub use dto::{
    activity_dto, activity_item_dto, constellation_day_from_beijing_midnight, find_season_activity,
    item_dto, normalize_season, normalize_shop_from_reply, normalize_solar_terms, pass_dto,
    positive_decimal, star_sand_goods_dto, ConstellationDto, ConstellationGroupDto,
    ExchangeResultDto, ItemDto, QingMeiDto, SeasonActivityDto, SeasonDto, SeasonPassDto,
    SeasonPassNodeDto, SolarTermDto, SolarTermsConfigDto, SolarTermsDto, StarSandGoodsDto,
    StarSandShopDto,
};

// 兼容旧 `services::activity_center::*` 导入路径
pub use crate::constants::{
    CLAIM_QINGMEI_SEED_OPERATE_TYPE, CONSTELLATION_ACTIVITY_TYPE,
    CONTINUE_QINGMEI_BREW_OPERATE_TYPE, EXCHANGE_SHOP_OPERATE_TYPE,
    LIGHT_CONSTELLATION_OPERATE_TYPE, QINGMEI_BREW_ACTIVITY_ID, QINGMEI_DAILY_ACTIVITY_ID,
    QINGMEI_DAILY_ALREADY_CLAIMED_CODE, QINGMEI_DAILY_GRANT_ID, QINGMEI_ITEM_ID,
    QINGMEI_SHARED_SETTLEMENT_MODE, QINGMEI_SHARE_SCENE, QINGMEI_SHARE_SOURCE,
    QUERY_QINGMEI_OPERATE_TYPE, QUERY_SHOP_OPERATE_TYPE, SELL_QINGMEI_BREW_OPERATE_TYPE,
    SHOP_ACTIVITY_TYPE, START_QINGMEI_BREW_OPERATE_TYPE,
};

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::ConstellationData;
use crate::proto::generated::gamepb::seasonpb::GetSeasonInfoReply;
use crate::services::activity_center_state::ConstellationActivityState;
use crate::services::warehouse::WarehouseService;

pub(crate) use dto::{
    beijing_date_key, build_actions, load_qingmei_seed_claimed_date,
    persist_qingmei_seed_claimed_date, settled_error,
};

/// 活动中心服务
pub struct ActivityCenterService {
    gateway: Arc<Gateway>,
    /// 购买 / 点亮等写操作串行化
    mutation_lock: Arc<AsyncMutex<()>>,
    /// 缓存上一次拉取的赛季信息（用于轻量刷新）
    cached_season: Mutex<Option<GetSeasonInfoReply>>,
    qingmei_seed_claimed_date: Mutex<String>,
    account_id: Mutex<String>,
    warehouse: Mutex<Option<Arc<WarehouseService>>>,
    last_constellation_dynamic: Mutex<HashMap<String, ConstellationData>>,
    last_constellation_memory: Mutex<HashMap<String, ConstellationActivityState>>,
}

impl ActivityCenterService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            mutation_lock: Arc::new(AsyncMutex::new(())),
            cached_season: Mutex::new(None),
            qingmei_seed_claimed_date: Mutex::new(String::new()),
            account_id: Mutex::new(String::new()),
            warehouse: Mutex::new(None),
            last_constellation_dynamic: Mutex::new(HashMap::new()),
            last_constellation_memory: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
        // 重启 worker 后从落盘恢复「今日已领」，避免按钮仍可点、再点报 1034014
        if let Some(date) = load_qingmei_seed_claimed_date(account_id) {
            *self.qingmei_seed_claimed_date.lock() = date;
        }
    }

    pub fn set_warehouse(&self, warehouse: Arc<WarehouseService>) {
        *self.warehouse.lock() = Some(warehouse);
    }

    pub(crate) fn mark_qingmei_seed_claimed_today(&self) {
        let today = beijing_date_key();
        *self.qingmei_seed_claimed_date.lock() = today.clone();
        let account_id = self.account_id.lock().clone();
        if !account_id.is_empty() {
            let _ = persist_qingmei_seed_claimed_date(&account_id, &today);
        }
    }

    pub(crate) fn qingmei_seed_claimed_today(&self) -> bool {
        *self.qingmei_seed_claimed_date.lock() == beijing_date_key()
    }

    /// 获取活动中心完整快照（聚合 season + star sand + solar terms + qingmei）
    pub async fn get_activity_center_snapshot(&self) -> Result<serde_json::Value> {
        self.snapshot_with_shop(None).await
    }

    pub(crate) async fn snapshot_with_shop(
        &self,
        shop_override: Option<StarSandShopDto>,
    ) -> Result<serde_json::Value> {
        let season_reply_result = self.query_season().await;
        let season_result: Result<SeasonDto> = match &season_reply_result {
            Ok(reply) => normalize_season(reply).ok_or_else(|| {
                ActivityError {
                    code: ActivityErrorCode::SeasonDataEmpty,
                    message: "当前赛季数据为空".to_string(),
                }
                .into()
            }),
            Err(e) => Err(Error::internal(e.to_string())),
        };
        let solar_result = self.get_current_solar_terms().await;
        let qingmei_result = self.get_current_qingmei_activity().await;
        let season = season_result.as_ref().ok().cloned();
        let warehouse = self.warehouse.lock().clone();
        let shop_result = if let Some(shop) = shop_override {
            Ok(shop)
        } else if let Ok(ref reply) = season_reply_result {
            self.shop_from_season_reply(reply, warehouse.as_deref()).await
        } else {
            Err(Error::Business("赛季查询失败，无法发现活动商店 ID".to_string()))
        };
        let shop = shop_result.as_ref().ok().cloned();
        let solar_terms = solar_result.as_ref().ok().cloned();
        let qingmei = qingmei_result.as_ref().ok().cloned();
        let constellation = season.as_ref().and_then(|s| self.build_constellation_dto(s, None));
        let actions = build_actions(&season, &solar_terms, constellation.as_ref(), shop.as_ref());
        Ok(serde_json::json!({
            "season": season,
            "constellation": constellation,
            "shop": shop,
            "solarTerms": solar_terms,
            "qingMei": qingmei,
            "capabilities": {
                "claimPass": true,
                "lightConstellation": true,
                "claimSolar": true,
                "exchange": true,
            },
            "actions": actions,
            "errors": {
                "season": settled_error(&season_result),
                "shop": settled_error(&shop_result),
                "solarTerms": settled_error(&solar_result),
                "qingMei": settled_error(&qingmei_result),
            },
        }))
    }

    /// 清除赛季缓存
    pub fn clear_season_cache(&self) {
        *self.cached_season.lock() = None;
    }
}
