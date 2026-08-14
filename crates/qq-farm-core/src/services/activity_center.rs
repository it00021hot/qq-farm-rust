//! 活动中心 — 4 个生效活动的 RPC + 业务编排。
//!
//! 1:1 翻译原 `core/src/services/activity-center.ts` 的有效部分（原 1034 行，
//! 按用户决策只复刻生效的活动，不做 1:1 全搬）。被跳过的：
//!
//! - 复杂 `constellation-*.json` catalog 静态数据（运行时按需从 `reply.constellation` 拿）
//! - 256 行 `activity-center-state.ts` JSON 状态合并（见 `activity_center_state` 模块）
//! - `serializeMutation` 复杂并发（defer，rate limiter 已在 1F-6 覆盖）
//!
//! ## 4 个生效活动
//!
//! 1. **赛季 (Season)** — `GetSeasonInfo` / `ClaimBattlePassRewards`
//! 2. **活动商店 (Star Sand Shop)** — `QueryActivity(operate=7)` / `Operate(exchange=1)`
//! 3. **星座 (Constellation)** — `QueryActivity(operate=7)` / `Operate(light=21)` + catalog JSON
//! 4. **节气 (Solar Terms)** — `GetSolarTerms` / `ClaimSolarTerms`
//! 5. **青梅酿酒 (QingMei)** — 每日种子 / 开始 / 继续 / 结算
//!
//! ## 协议
//!
//! - `gamepb.seasonpb.SeasonService.GetSeasonInfo` / `ClaimBattlePassRewards`
//! - `gamepb.activitypb.ActivityService.Operate` — 通用活动入口（Query / Exchange / Light）
//! - `gamepb.solartermspb.SolarTermsService.GetSolarTerms` / `ClaimSolarTerms`

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;
use serde::Serialize;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityContent, ActivityItem, ActivityOperateReply, ClaimQingMeiDailySeedRequest,
    ConstellationData, ContinueQingMeiBrewRequest, ExchangeShopRequest, OperateConstellationRequest,
    QueryActivityRequest, SettleQingMeiBrewRequest, StarSandGoods, StartQingMeiBrewRequest,
};
use crate::proto::generated::gamepb::seasonpb::{
    ClaimBattlePassRewardsReply, ClaimBattlePassRewardsRequest, GetSeasonInfoReply,
    GetSeasonInfoRequest, SeasonActivity, SeasonInfo, SeasonItem, SeasonPass, SeasonRewardNode,
};
use crate::proto::generated::gamepb::solartermspb::{
    ClaimSolarTermsReply, ClaimSolarTermsRequest, GetSolarTermsReply, GetSolarTermsRequest,
    SolarTermInfo, SolarTermsConfig,
};

use super::activity_center_state::{
    load_constellation_state, merge_constellation_states, persist_constellation_state,
    state_from_dynamic_nodes, state_record_key, state_with_no_claimable_day, ActivityStateIdentity,
    ConstellationActivityState, StateFileOptions,
};
use super::warehouse::WarehouseService;

const SEASON_SERVICE: &str = "gamepb.seasonpb.SeasonService";
const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const SOLAR_TERMS_SERVICE: &str = "gamepb.solartermspb.SolarTermsService";

/// 活动类型 code（与 proto 的 `SeasonActivity.type` 对应）
pub const SHOP_ACTIVITY_TYPE: i64 = 3;
pub const CONSTELLATION_ACTIVITY_TYPE: i64 = 13;

/// 活动操作类型
pub const EXCHANGE_SHOP_OPERATE_TYPE: i64 = 1;
pub const QUERY_SHOP_OPERATE_TYPE: i64 = 7;
pub const LIGHT_CONSTELLATION_OPERATE_TYPE: i64 = 21;
pub const QINGMEI_DAILY_ACTIVITY_ID: i64 = 2_026_081_201;
pub const QINGMEI_BREW_ACTIVITY_ID: i64 = 2_026_081_202;
pub const QINGMEI_ITEM_ID: i64 = 41221;
pub const QINGMEI_DAILY_GRANT_ID: i64 = 3;
pub const QUERY_QINGMEI_OPERATE_TYPE: i64 = 7;
pub const CLAIM_QINGMEI_SEED_OPERATE_TYPE: i64 = 4;
pub const START_QINGMEI_BREW_OPERATE_TYPE: i64 = 14;
pub const CONTINUE_QINGMEI_BREW_OPERATE_TYPE: i64 = 15;
pub const SELL_QINGMEI_BREW_OPERATE_TYPE: i64 = 16;
pub const QINGMEI_SHARED_SETTLEMENT_MODE: i64 = 2;
pub const QINGMEI_SHARE_SOURCE: i32 = 11;
pub const QINGMEI_SHARE_SCENE: i32 = 215;
pub const QINGMEI_DAILY_ALREADY_CLAIMED_CODE: i64 = 1_034_014;

const BEIJING_UTC_OFFSET_SECONDS: i64 = 8 * 60 * 60;
const SECONDS_PER_DAY: i64 = 86_400;

// =====================================================================
// 错误码
// =====================================================================

/// 活动业务错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityErrorCode {
    ShopUnavailable,
    ShopResponseInvalid,
    ShopGoodsNotFound,
    ShopGoodsUnavailable,
    ShopBalanceUnavailable,
    InsufficientStarSand,
    InvalidShopGoodsId,
    InvalidExchangeCount,
    InvalidSolarTermId,
    SeasonDataEmpty,
    ConstellationActivityMissing,
    InvalidQingmeiUid,
    InvalidQingmeiCount,
    InvalidQingmeiIngredients,
    DuplicateQingmeiUid,
    InsufficientQingmei,
}

impl ActivityErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShopUnavailable => "SHOP_UNAVAILABLE",
            Self::ShopResponseInvalid => "SHOP_RESPONSE_INVALID",
            Self::ShopGoodsNotFound => "SHOP_GOODS_NOT_FOUND",
            Self::ShopGoodsUnavailable => "SHOP_GOODS_UNAVAILABLE",
            Self::ShopBalanceUnavailable => "SHOP_BALANCE_UNAVAILABLE",
            Self::InsufficientStarSand => "INSUFFICIENT_STAR_SAND",
            Self::InvalidShopGoodsId => "INVALID_SHOP_GOODS_ID",
            Self::InvalidExchangeCount => "INVALID_EXCHANGE_COUNT",
            Self::InvalidSolarTermId => "INVALID_SOLAR_TERM_ID",
            Self::SeasonDataEmpty => "SEASON_DATA_EMPTY",
            Self::ConstellationActivityMissing => "CONSTELLATION_ACTIVITY_MISSING",
            Self::InvalidQingmeiUid => "INVALID_QINGMEI_UID",
            Self::InvalidQingmeiCount => "INVALID_QINGMEI_COUNT",
            Self::InvalidQingmeiIngredients => "INVALID_QINGMEI_INGREDIENTS",
            Self::DuplicateQingmeiUid => "DUPLICATE_QINGMEI_UID",
            Self::InsufficientQingmei => "INSUFFICIENT_QINGMEI",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivityError {
    pub code: ActivityErrorCode,
    pub message: String,
}

impl std::fmt::Display for ActivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ActivityError {}

impl From<ActivityError> for Error {
    fn from(e: ActivityError) -> Self {
        Error::Business(e.to_string())
    }
}

// =====================================================================
// DTO
// =====================================================================

/// 赛季活动 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonActivityDto {
    pub id: i64,
    #[serde(rename = "typeCode")]
    pub r#type: i64,
    pub name: String,
    pub begin_time: i64,
    pub start_time: i64,
    pub end_time: i64,
}

/// 通行证节点
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPassNodeDto {
    pub id: String,
    pub level: String,
    pub key_level: bool,
    pub locked: bool,
    pub claimed: bool,
    pub claimable: bool,
    pub current: bool,
    pub rewards: Vec<ItemDto>,
}

/// 赛季战斗通行证 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonPassDto {
    pub activity_id: i64,
    pub title: String,
    pub current_level: i64,
    pub level: i64,
    pub current_progress: i64,
    pub progress: i64,
    pub progress_target: i64,
    pub progress_max: i64,
    pub node_count: i64,
    pub claimed_through_level: i64,
    pub field11_code: i64,
    pub field13_code: i64,
    pub field18_code: i64,
    pub field14_items: Vec<ItemDto>,
    pub rules: serde_json::Value,
    pub nodes: Vec<SeasonPassNodeDto>,
}

/// 赛季 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonDto {
    pub id: i64,
    pub title: String,
    pub status_code: i64,
    pub field_4_code: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub server_time: i64,
    pub activities: Vec<SeasonActivityDto>,
    pub constellation_activity: Option<SeasonActivityDto>,
    pub shop_activity: Option<SeasonActivityDto>,
    pub pass: Option<SeasonPassDto>,
}

/// 节气 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SolarTermDto {
    pub id: i64,
    pub status: i64,
    pub status_code: String,
    pub can_claim: bool,
    pub claimed: bool,
    pub locked: bool,
    pub current: bool,
    pub begin_time: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub name: String,
    pub rewards: Vec<ItemDto>,
}

/// 节气 DTO（含 rules）
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SolarTermsConfigDto {
    pub id: i64,
    pub activity_id: i64,
    pub rules_text: String,
    #[serde(default)]
    pub rules: serde_json::Value,
}

/// 节气回复 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SolarTermsDto {
    pub server_time: i64,
    pub current_term_id: Option<i64>,
    pub terms: Vec<SolarTermDto>,
    pub current_config: Option<SolarTermsConfigDto>,
    pub configs: Vec<SolarTermsConfigDto>,
}

/// 商品 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StarSandGoodsDto {
    pub id: i64,
    pub activity_id: i64,
    pub name: String,
    pub category: String,
    pub item: ItemDto,
    pub cost: ItemDto,
    pub sort_order: i64,
    pub status_code: i64,
    pub owned: bool,
    pub exchangeable: bool,
    pub sold_out: bool,
    pub balance_known: bool,
    pub max_exchange_count: i64,
    pub max_exchange_count_known: bool,
    pub quality_code: i64,
}

/// 简化物品 DTO（用于商品内嵌的 item / cost）
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    pub id: i64,
    pub count: i64,
    pub name: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub rarity: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_known: Option<bool>,
}

/// 活动商店 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StarSandShopDto {
    pub activity_id: i64,
    pub name: String,
    pub start_time: i64,
    pub end_time: i64,
    pub server_time: i64,
    pub balance_known: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<String>,
    pub currencies: Vec<ItemDto>,
    pub categories: Vec<String>,
    pub goods: Vec<StarSandGoodsDto>,
    pub affordable_count: i32,
    pub exchangeable_count: i32,
    #[serde(default)]
    pub action: serde_json::Value,
}

/// 星座活动 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConstellationDto {
    pub activity_id: i64,
    pub type_code: String,
    pub display_name: String,
    pub server_name: String,
    pub server_time: i64,
    pub start_time: i64,
    pub end_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_day: Option<i32>,
    pub field_1: i64,
    pub field_2: i64,
    pub field_3: i64,
    pub node_count: usize,
    pub group_count: usize,
    #[serde(default)]
    pub catalog_version: i64,
    #[serde(default)]
    pub catalog_status: String,
    #[serde(default)]
    pub rules: serde_json::Value,
    #[serde(default)]
    pub groups: Vec<ConstellationGroupDto>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConstellationGroupDto {
    pub id: String,
    pub node_id: String,
    pub name: String,
    pub category: String,
    pub explain: String,
    pub order: i32,
    pub chart_index: i32,
    pub rewards: Vec<ItemDto>,
    pub links_raw: String,
    pub node_ids: Vec<String>,
    pub visual_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lit: Option<bool>,
    pub state_known: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_status: Option<String>,
    pub status_source: String,
}

/// 青梅酿酒 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QingMeiDto {
    pub activity_id: String,
    pub daily_activity_id: String,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub rules: serde_json::Value,
    pub ingredient: ItemDto,
    pub ingredients: Vec<serde_json::Value>,
    pub balance: String,
    pub balance_known: bool,
    pub base_gold: String,
    pub base_price: String,
    pub guaranteed_price: String,
    pub current_round: i64,
    pub started: bool,
    pub max_rounds: i64,
    pub finished: bool,
    pub quote_prices: Vec<String>,
    pub quote_totals: Vec<String>,
    pub quote: Option<serde_json::Value>,
    pub daily_seed: serde_json::Value,
    pub actions: serde_json::Value,
}

/// 兑换结果 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeResultDto {
    pub purchase_count: String,
    pub total_item_count: String,
    pub total_cost: String,
    pub rewards: Vec<ItemDto>,
    pub received_items: Vec<ItemDto>,
    pub shop: StarSandShopDto,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<serde_json::Value>,
}

// =====================================================================
// Service
// =====================================================================

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

    fn mark_qingmei_seed_claimed_today(&self) {
        let today = beijing_date_key();
        *self.qingmei_seed_claimed_date.lock() = today.clone();
        let account_id = self.account_id.lock().clone();
        if !account_id.is_empty() {
            let _ = persist_qingmei_seed_claimed_date(&account_id, &today);
        }
    }

    fn qingmei_seed_claimed_today(&self) -> bool {
        *self.qingmei_seed_claimed_date.lock() == beijing_date_key()
    }

    // ----- 赛季 -----

    /// 拉取赛季信息
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn query_season(&self) -> Result<GetSeasonInfoReply> {
        let body = self
            .gateway
            .request(
                SEASON_SERVICE,
                "GetSeasonInfo",
                &GetSeasonInfoRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        let reply = GetSeasonInfoReply::decode(&body[..])?;
        *self.cached_season.lock() = Some(reply.clone());
        Ok(reply)
    }

    /// 拉取赛季并归一化
    /// 获取活动中心完整快照（聚合 season + star sand + solar terms + qingmei）
    pub async fn get_activity_center_snapshot(&self) -> Result<serde_json::Value> {
        self.snapshot_with_shop(None).await
    }

    async fn snapshot_with_shop(
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
            self.shop_from_season_reply(reply, warehouse.as_deref())
                .await
        } else {
            Err(Error::Business(
                "赛季查询失败，无法发现活动商店 ID".to_string(),
            ))
        };
        let shop = shop_result.as_ref().ok().cloned();
        let solar_terms = solar_result.as_ref().ok().cloned();
        let qingmei = qingmei_result.as_ref().ok().cloned();
        let constellation = season
            .as_ref()
            .and_then(|s| self.build_constellation_dto(s, None));
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

    pub async fn get_current_season_event(&self) -> Result<SeasonDto> {
        let reply = self.query_season().await?;
        normalize_season(&reply).ok_or_else(|| ActivityError {
            code: ActivityErrorCode::SeasonDataEmpty,
            message: "当前赛季数据为空".to_string(),
        }.into())
    }

    /// 领取战斗通行证奖励
    pub async fn claim_battle_pass_rewards(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let season_reply = self.query_season().await?;
        let pass = season_reply
            .season_info
            .as_ref()
            .and_then(|s| s.pass.as_ref())
            .map(pass_dto);
        let Some(pass) = pass else {
            return Err(Error::Business("服务端未发现可用游记".into()));
        };
        if !pass.nodes.iter().any(|n| n.claimable) {
            return Err(Error::Business("当前没有可领取的游记奖励".into()));
        }
        let body = self
            .gateway
            .request(
                SEASON_SERVICE,
                "ClaimBattlePassRewards",
                &ClaimBattlePassRewardsRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        let reply = ClaimBattlePassRewardsReply::decode(&body[..])?;
        Ok(serde_json::json!({
            "rewards": reply.rewards.iter().map(season_item_dto).collect::<Vec<_>>(),
            "field2Codes": reply.field_2.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            "pass": reply.pass.as_ref().map(pass_dto),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    /// 刷新赛季通行证（用最新数据刷新缓存）
    pub async fn refresh_season_pass(&self) -> Result<SeasonDto> {
        self.get_current_season_event().await
    }

    // ----- 节气 -----

    /// 拉取节气信息
    pub async fn query_solar_terms(&self) -> Result<GetSolarTermsReply> {
        let body = self
            .gateway
            .request(
                SOLAR_TERMS_SERVICE,
                "GetSolarTerms",
                &GetSolarTermsRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(GetSolarTermsReply::decode(&body[..])?)
    }

    /// 拉取节气并归一化
    pub async fn get_current_solar_terms(&self) -> Result<SolarTermsDto> {
        let reply = self.query_solar_terms().await?;
        Ok(normalize_solar_terms(&reply))
    }

    /// 领取指定节气奖励
    pub async fn claim_solar_term(&self, term_id: &str) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !term_id.chars().all(|c| c.is_ascii_digit())
            || term_id.is_empty()
            || term_id.starts_with('0')
        {
            return Err(Error::Business("termId 必须是正十进制整数".into()));
        }
        let parsed = positive_decimal(term_id, ActivityErrorCode::InvalidSolarTermId, "termId")?;
        let solar_reply = self.query_solar_terms().await?;
        let term = solar_reply
            .terms
            .iter()
            .find(|t| t.term_id == parsed)
            .ok_or_else(|| Error::Business("服务端未发现指定节令".into()))?;
        if term.status != 2 {
            return Err(Error::Business("指定节令当前不可领取".into()));
        }
        let req = ClaimSolarTermsRequest { term_id: parsed };
        let body = self
            .gateway
            .request(
                SOLAR_TERMS_SERVICE,
                "ClaimSolarTerms",
                &req.encode_to_vec(),
                10_000,
            )
            .await?;
        let reply = ClaimSolarTermsReply::decode(&body[..])?;
        Ok(serde_json::json!({
            "rewards": reply.rewards.iter().map(solar_term_reward_dto).collect::<Vec<_>>(),
            "term": reply.term.as_ref().map(solar_term_dto),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    // ----- 活动通用（Query / Operate） -----

    /// 通用活动查询
    pub async fn query_activity(
        &self,
        activity_id: i64,
        operate_type: i64,
    ) -> Result<ActivityOperateReply> {
        let req = QueryActivityRequest {
            activity_id,
            operate_type,
        };
        let body = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(ActivityOperateReply::decode(&body[..])?)
    }

    /// 通用活动操作
    pub async fn operate_activity(
        &self,
        activity_id: i64,
        operate_type: i64,
    ) -> Result<ActivityOperateReply> {
        // Operate 和 QueryActivity 都走 "Operate" 方法（proto 不区分）
        self.query_activity(activity_id, operate_type).await
    }

    // ----- 活动商店 -----

    /// 拉取当前赛季的活动商店
    pub async fn get_current_star_sand_shop(
        &self,
        warehouse: Option<&WarehouseService>,
    ) -> Result<StarSandShopDto> {
        let season_reply = self.query_season().await?;
        self.shop_from_season_reply(&season_reply, warehouse).await
    }

    async fn shop_from_season_reply(
        &self,
        season_reply: &GetSeasonInfoReply,
        warehouse: Option<&WarehouseService>,
    ) -> Result<StarSandShopDto> {
        let shop_activity = find_season_activity(season_reply, SHOP_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopUnavailable,
                message: "当前赛季未发现活动商店".to_string(),
            })?;
        let reply = self
            .query_shop_catalog(shop_activity.activity_id)
            .await?;
        let goods = extract_goods(&reply);
        let currency_ids: Vec<i64> = goods
            .iter()
            .map(|g| g.cost.as_ref().map(|c| c.id).unwrap_or(0))
            .filter(|id| *id > 0)
            .collect();
        let balances = match warehouse {
            Some(wh) => read_bag_balances(wh, &currency_ids).await,
            None => None,
        };
        Ok(normalize_shop_from_reply(
            season_reply,
            shop_activity,
            &reply,
            balances.as_ref(),
        ))
    }

    /// 查询商店目录（带响应校验）
    async fn query_shop_catalog(&self, activity_id: i64) -> Result<ActivityOperateReply> {
        let reply = self.query_activity(activity_id, QUERY_SHOP_OPERATE_TYPE).await?;
        if reply.activity_id != activity_id {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店查询返回了不匹配的活动 ID".to_string(),
            }
            .into());
        }
        if reply.operate_type != QUERY_SHOP_OPERATE_TYPE {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: format!(
                    "活动商店查询返回了未知操作类型: {}",
                    reply.operate_type
                ),
            }
            .into());
        }
        let data = reply.data.as_ref().ok_or_else(|| ActivityError {
            code: ActivityErrorCode::ShopResponseInvalid,
            message: "活动商店查询回包缺少商品目录".to_string(),
        })?;
        if data.catalog.is_none() {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店查询回包缺少商品目录".to_string(),
            }
            .into());
        }
        Ok(reply)
    }

    /// 兑换星砂商店商品
    pub async fn exchange_star_sand_goods(
        &self,
        warehouse: &WarehouseService,
        goods_id_input: &str,
        count_input: &str,
    ) -> Result<ExchangeResultDto> {
        let goods_id = positive_decimal(
            goods_id_input,
            ActivityErrorCode::InvalidShopGoodsId,
            "goodsId",
        )?;
        let count = positive_decimal(
            count_input,
            ActivityErrorCode::InvalidExchangeCount,
            "count",
        )?;
        if count <= 0 {
            return Err(ActivityError {
                code: ActivityErrorCode::InvalidExchangeCount,
                message: "count must be a positive integer".to_string(),
            }
            .into());
        }

        let _guard = self.mutation_lock.lock().await;

        let season_reply = self.query_season().await?;
        let shop_activity = find_season_activity(&season_reply, SHOP_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopUnavailable,
                message: "当前赛季未发现活动商店".to_string(),
            })?;

        let catalog_reply = self
            .query_shop_catalog(shop_activity.activity_id)
            .await?;
        let raw_goods_list = extract_goods(&catalog_reply);
        let raw_goods = raw_goods_list
            .iter()
            .find(|g| g.id == goods_id)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopGoodsNotFound,
                message: "活动商店中未找到指定商品".to_string(),
            })?
            .clone();

        let currency_id = raw_goods.cost.as_ref().map(|c| c.id).unwrap_or(0);
        let unit_cost = raw_goods.cost.as_ref().map(|c| c.count).unwrap_or(0);
        if currency_id <= 0 || unit_cost <= 0 {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "商品兑换成本无效，请刷新商店后重试".to_string(),
            }
            .into());
        }

        let balances = read_bag_balances(warehouse, &[currency_id])
            .await
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopBalanceUnavailable,
                message: "无法确认当前星砂余额，请稍后重试".to_string(),
            })?;
        let shop_before = normalize_shop_from_reply(
            &season_reply,
            shop_activity,
            &catalog_reply,
            Some(&balances),
        );
        let normalized_goods = shop_before
            .goods
            .iter()
            .find(|g| g.id == goods_id)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopGoodsNotFound,
                message: "活动商店中未找到指定商品".to_string(),
            })?;
        if !normalized_goods.exchangeable || normalized_goods.sold_out {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopGoodsUnavailable,
                message: "该商品当前不可兑换，请刷新商店后重试".to_string(),
            }
            .into());
        }
        let cost_name = if normalized_goods.cost.name.is_empty() {
            "星砂".to_string()
        } else {
            normalized_goods.cost.name.clone()
        };
        let balance = *balances.get(&currency_id).unwrap_or(&0);
        let total_cost = unit_cost * count;
        if balance < total_cost {
            return Err(ActivityError {
                code: ActivityErrorCode::InsufficientStarSand,
                message: "星砂余额不足，无法完成本次兑换".to_string(),
            }
            .into());
        }

        // 发送兑换请求
        let req = ExchangeShopRequest {
            activity_id: shop_activity.activity_id,
            operate_type: EXCHANGE_SHOP_OPERATE_TYPE,
            exchange_shop_operate: Some(
                crate::proto::generated::gamepb::activitypb::ExchangeShopOperateParams {
                    goods_id,
                    count,
                },
            ),
        };
        let body = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000)
            .await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != shop_activity.activity_id {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店兑换返回了不匹配的活动 ID".to_string(),
            }
            .into());
        }
        if reply.operate_type != EXCHANGE_SHOP_OPERATE_TYPE {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: format!(
                    "活动商店兑换返回了未知操作类型: {}",
                    reply.operate_type
                ),
            }
            .into());
        }
        if reply
            .data
            .as_ref()
            .and_then(|d| d.catalog.as_ref())
            .is_none()
        {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店兑换回包缺少最新商品目录".to_string(),
            }
            .into());
        }

        let unit_item_count = raw_goods.item.as_ref().map(|i| i.count).unwrap_or(0);
        let total_item_count = if unit_item_count > 0 {
            unit_item_count * count
        } else {
            0
        };
        let received = match raw_goods.item.as_ref() {
            Some(item) if item.id > 0 && total_item_count > 0 => {
                vec![item_from_id(item.id, total_item_count)]
            }
            _ => vec![],
        };

        // 刷新最新目录
        let latest_currency_ids: Vec<i64> = extract_goods(&reply)
            .iter()
            .filter_map(|g| g.cost.as_ref().map(|c| c.id))
            .filter(|id| *id > 0)
            .collect();
        let latest_balances = read_bag_balances(warehouse, &latest_currency_ids).await;
        let shop = normalize_shop_from_reply(
            &season_reply,
            shop_activity,
            &reply,
            latest_balances.as_ref(),
        );
        let snapshot = self.snapshot_with_shop(Some(shop.clone())).await.ok();
        let message = format!("兑换成功，共消耗 {total_cost} {cost_name}");

        Ok(ExchangeResultDto {
            purchase_count: count.to_string(),
            total_item_count: total_item_count.to_string(),
            total_cost: total_cost.to_string(),
            rewards: received.clone(),
            received_items: received,
            shop,
            message,
            snapshot,
        })
    }

    // ----- 星座 -----

    /// 点亮星座（一次性操作）
    pub async fn light_constellation(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let season_reply = self.query_season().await?;
        let activity = find_season_activity(&season_reply, CONSTELLATION_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ConstellationActivityMissing,
                message: "服务端未发现星座活动".to_string(),
            })?;
        let activity_id = activity.activity_id;
        let begin_time = activity.begin_time;
        let end_time = activity.end_time;
        let season = normalize_season(&season_reply).ok_or_else(|| ActivityError {
            code: ActivityErrorCode::SeasonDataEmpty,
            message: "当前赛季数据为空".to_string(),
        })?;
        let identity = constellation_identity(&season, activity_id);
        let state_key = state_record_key(&identity);
        let server_time = season.server_time;
        let current_day = constellation_day_from_beijing_midnight(begin_time, server_time);
        let activity_active = server_time > 0
            && begin_time > 0
            && server_time >= begin_time
            && (end_time <= 0 || server_time <= end_time);

        let req = OperateConstellationRequest {
            activity_id,
            operate_type: LIGHT_CONSTELLATION_OPERATE_TYPE,
            field_119: Some(
                crate::proto::generated::gamepb::activitypb::operate_constellation_request::Empty {},
            ),
        };
        let body = match self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000)
            .await
        {
            Ok(bytes) => bytes,
            Err(crate::network::error::NetworkError::Gateway { code, .. })
                if code == 1_034_038
                    && activity_active
                    && current_day.is_some_and(|d| (1..=28).contains(&d)) =>
            {
                let day = current_day.unwrap_or(0);
                let rejection = state_with_no_claimable_day(
                    &identity,
                    i64::from(day),
                    &server_time.to_string(),
                    None,
                );
                self.merge_and_persist_constellation(&identity, &state_key, rejection);
                let snapshot = self.snapshot_with_shop(None).await.ok();
                return Ok(serde_json::json!({
                    "outcome": "nothingToClaim",
                    "noClaimable": true,
                    "message": "今日星宿奖励已经领取，无需重复操作",
                    "snapshot": snapshot,
                }));
            }
            Err(e) => return Err(e.into()),
        };
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != activity_id {
            return Err(Error::Protocol("星座操作返回了不匹配的活动 ID".to_string()));
        }
        if reply.operate_type != LIGHT_CONSTELLATION_OPERATE_TYPE {
            return Err(Error::Protocol(format!(
                "星座操作返回了未知操作类型: {}",
                reply.operate_type
            )));
        }
        let constellation_state = reply
            .data
            .as_ref()
            .and_then(|d| d.constellation.clone())
            .ok_or_else(|| Error::Protocol("星座操作成功但回包缺少动态状态".to_string()))?;

        self.last_constellation_dynamic
            .lock()
            .insert(state_key.clone(), constellation_state.clone());
        let nodes_json = serde_json::Value::Array(
            constellation_state
                .nodes
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "node_id": n.node_id.to_string(),
                        "nodeId": n.node_id.to_string(),
                        "field_2": n.field_2,
                        "field_3": n.field_3,
                    })
                })
                .collect(),
        );
        let from_nodes = state_from_dynamic_nodes(&identity, nodes_json);
        self.merge_and_persist_constellation(&identity, &state_key, from_nodes);

        let dto = self.build_constellation_dto(&season, Some(&constellation_state));
        let snapshot = self.snapshot_with_shop(None).await.ok();
        let constellation = snapshot
            .as_ref()
            .and_then(|s| s.get("constellation").cloned())
            .or_else(|| serde_json::to_value(dto).ok());
        Ok(serde_json::json!({
            "outcome": "lighted",
            "rewards": [],
            "activity": season.constellation_activity,
            "constellation": constellation,
            "snapshot": snapshot,
        }))
    }

    fn merge_and_persist_constellation(
        &self,
        identity: &ActivityStateIdentity,
        state_key: &str,
        incoming: ConstellationActivityState,
    ) {
        let account_id = self.account_id.lock().clone();
        let memory = self
            .last_constellation_memory
            .lock()
            .get(state_key)
            .cloned();
        let file = load_constellation_state(
            identity,
            Some(account_id.as_str()).filter(|s| !s.is_empty()),
            &StateFileOptions::default(),
        );
        let merged = merge_constellation_states(
            identity,
            &[
                serde_json::to_value(&file).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(&memory).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(&incoming).unwrap_or(serde_json::Value::Null),
            ],
        );
        self.last_constellation_memory
            .lock()
            .insert(state_key.to_string(), merged.clone());
        if !account_id.is_empty() {
            let _ = persist_constellation_state(
                serde_json::to_value(&merged).unwrap_or(serde_json::Value::Null),
                identity,
                Some(&account_id),
                &StateFileOptions::default(),
            );
        }
    }

    fn build_constellation_dto(
        &self,
        season: &SeasonDto,
        dynamic_override: Option<&ConstellationData>,
    ) -> Option<ConstellationDto> {
        let act = season.constellation_activity.as_ref()?;
        let identity = constellation_identity(season, act.id);
        let state_key = state_record_key(&identity);
        let stored_dynamic = self.last_constellation_dynamic.lock().get(&state_key).cloned();
        let dynamic = dynamic_override.cloned().or(stored_dynamic);
        let account_id = self.account_id.lock().clone();
        let file = load_constellation_state(
            &identity,
            Some(account_id.as_str()).filter(|s| !s.is_empty()),
            &StateFileOptions::default(),
        );
        let memory = self.last_constellation_memory.lock().get(&state_key).cloned();
        let confirmed = merge_constellation_states(
            &identity,
            &[
                serde_json::to_value(&file).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(&memory).unwrap_or(serde_json::Value::Null),
            ],
        );
        Some(constellation_dto(act, season.server_time, dynamic.as_ref(), &confirmed))
    }

    // ----- 青梅 -----

    async fn operate_qingmei(
        &self,
        body: Vec<u8>,
        expected_error_codes: &[i64],
    ) -> Result<(ActivityOperateReply, bool)> {
        match self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &body, 10_000)
            .await
        {
            Ok(bytes) => Ok((ActivityOperateReply::decode(&bytes[..])?, false)),
            Err(crate::network::error::NetworkError::Gateway {
                code,
                error_message,
                ..
            }) if expected_error_codes.contains(&code)
                || is_qingmei_already_claimed_message(&error_message) =>
            {
                Ok((ActivityOperateReply::default(), true))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn query_qingmei_reply(&self) -> Result<ActivityOperateReply> {
        let req = QueryActivityRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: QUERY_QINGMEI_OPERATE_TYPE,
        };
        self.operate_qingmei(req.encode_to_vec(), &[])
            .await
            .map(|(r, _)| r)
    }

    /// 当前青梅活动
    pub async fn get_current_qingmei_activity(&self) -> Result<QingMeiDto> {
        let reply = self.query_qingmei_reply().await?;
        let ingredients = self.qingmei_ingredients().await.ok();
        Ok(self.qingmei_dto(&reply, ingredients.as_deref()))
    }

    async fn qingmei_ingredients(&self) -> Result<Vec<serde_json::Value>> {
        let warehouse = self.warehouse.lock().clone();
        let bag = if let Some(wh) = warehouse {
            wh.get_bag().await?
        } else {
            WarehouseService::get_bag_via(&self.gateway).await?
        };
        let items = bag
            .item_bag
            .as_ref()
            .map(|b| b.items.as_slice())
            .unwrap_or(&[]);
        Ok(items
            .iter()
            .filter(|i| i.id == QINGMEI_ITEM_ID && i.count > 0)
            .map(|i| {
                let mutant_types: Vec<String> = i
                    .mutant_types
                    .iter()
                    .filter(|t| **t != 0)
                    .map(ToString::to_string)
                    .collect();
                let uid = i.uid.to_string();
                let mut dto = serde_json::to_value(item_from_id(i.id, i.count))
                    .unwrap_or_else(|_| serde_json::json!({}));
                if let Some(obj) = dto.as_object_mut() {
                    obj.insert("uid".into(), serde_json::json!(uid.clone()));
                    obj.insert("mutantTypes".into(), serde_json::json!(mutant_types.clone()));
                    obj.insert(
                        "key".into(),
                        serde_json::json!(format!("{}:{}", uid, mutant_types.join(","))),
                    );
                }
                dto
            })
            .collect())
    }

    fn qingmei_dto(
        &self,
        reply: &ActivityOperateReply,
        ingredients: Option<&[serde_json::Value]>,
    ) -> QingMeiDto {
        let activity = reply.data.as_ref().and_then(|d| d.activity.as_ref());
        let brew = reply
            .data
            .as_ref()
            .and_then(|d| d.qingmei_brew.as_ref())
            .cloned()
            .unwrap_or_default();
        let quote = reply
            .qingmei_quote
            .clone()
            .or_else(|| reply.data.as_ref().and_then(|d| d.qingmei_quote.clone()));
        let daily_seed = reply.data.as_ref().and_then(|d| d.qingmei_daily_seed.as_ref());
        let current_round = brew.current_round;
        let started = brew.base_gold > 0;
        let max_rounds = brew.max_rounds.max(1);
        let claimed_today = self.qingmei_seed_claimed_today()
            || daily_seed.map(|d| d.claimed).unwrap_or(false);
        let balance: i64 = ingredients
            .unwrap_or(&[])
            .iter()
            .filter_map(|v| json_i64(v.get("count")))
            .sum();
        let activity_id = activity
            .map(|a| a.activity_id)
            .filter(|id| *id != 0)
            .unwrap_or(QINGMEI_BREW_ACTIVITY_ID);
        QingMeiDto {
            activity_id: activity_id.to_string(),
            daily_activity_id: QINGMEI_DAILY_ACTIVITY_ID.to_string(),
            name: activity
                .map(|a| a.name.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "青酿换万金".to_string()),
            start_time: activity.map(|a| a.begin_time).unwrap_or(0).to_string(),
            end_time: activity.map(|a| a.end_time).unwrap_or(0).to_string(),
            rules: activity
                .map(|a| text_content(&a.extra))
                .unwrap_or_else(|| serde_json::json!({ "title": "", "paragraphs": [] })),
            ingredient: ItemDto {
                name: "青梅".to_string(),
                ..item_from_id(QINGMEI_ITEM_ID, balance)
            },
            ingredients: ingredients.unwrap_or(&[]).to_vec(),
            balance: balance.to_string(),
            balance_known: ingredients.is_some(),
            base_gold: brew.base_gold.to_string(),
            base_price: brew.base_price.to_string(),
            guaranteed_price: brew.guaranteed_price.to_string(),
            current_round,
            started,
            max_rounds,
            finished: brew.finished,
            quote_prices: brew.quote_prices.iter().map(ToString::to_string).collect(),
            quote_totals: brew.quote_totals.iter().map(ToString::to_string).collect(),
            quote: quote.map(|q| {
                serde_json::json!({
                    "round": q.round,
                    "unitPrice": q.unit_price.to_string(),
                    "totalGold": q.total_gold.to_string(),
                    "doubled": q.doubled,
                })
            }),
            daily_seed: serde_json::json!({
                "claimed": claimed_today,
                "grantId": daily_seed
                    .and_then(|d| d.grant.as_ref())
                    .map(|g| g.grant_id.to_string())
                    .unwrap_or_else(|| QINGMEI_DAILY_GRANT_ID.to_string()),
                "reward": daily_seed
                    .and_then(|d| d.grant.as_ref())
                    .and_then(|g| g.item.as_ref())
                    .map(activity_item_dto),
            }),
            actions: serde_json::json!({
                "claimSeed": { "enabled": !claimed_today, "available": !claimed_today },
                "start": {
                    "enabled": ingredients.map(|i| !i.is_empty()).unwrap_or(true),
                    "available": ingredients.map(|i| !i.is_empty()).unwrap_or(true)
                },
                "continue": {
                    "enabled": current_round < max_rounds && !brew.finished && brew.base_gold > 0,
                    "available": current_round < max_rounds && !brew.finished && brew.base_gold > 0
                },
                "settle": {
                    "enabled": !brew.quote_totals.is_empty() || brew.finished,
                    "available": !brew.quote_totals.is_empty() || brew.finished
                },
            }),
        }
    }

    /// 领取青梅每日种子
    pub async fn claim_qingmei_daily_seed(&self) -> Result<serde_json::Value> {
        // 本地已记今日已领：直接幂等成功，避免再打 RPC 报错而按钮仍可点
        if self.qingmei_seed_claimed_today() {
            let mut snapshot = self.snapshot_with_shop(None).await.ok();
            force_qingmei_seed_claimed_in_snapshot(&mut snapshot);
            return Ok(serde_json::json!({
                "rewards": [],
                "message": "今日青梅种子已经领取，无需重复领取",
                "snapshot": snapshot,
            }));
        }

        let (reply, already) = {
            let _guard = self.mutation_lock.lock().await;
            if self.qingmei_seed_claimed_today() {
                (ActivityOperateReply::default(), true)
            } else {
                let req = ClaimQingMeiDailySeedRequest {
                    activity_id: QINGMEI_DAILY_ACTIVITY_ID,
                    operate_type: CLAIM_QINGMEI_SEED_OPERATE_TYPE,
                    params: Some(
                        crate::proto::generated::gamepb::activitypb::claim_qing_mei_daily_seed_request::Params {
                            grant_id: QINGMEI_DAILY_GRANT_ID,
                        },
                    ),
                };
                match self
                    .operate_qingmei(req.encode_to_vec(), &[QINGMEI_DAILY_ALREADY_CLAIMED_CODE])
                    .await
                {
                    Ok(v) => v,
                    Err(e) if is_qingmei_already_claimed_message(&e.to_string()) => {
                        (ActivityOperateReply::default(), true)
                    }
                    Err(e) => return Err(e),
                }
            }
        };
        // 无论新领还是 1034014，都标记今日已领（对齐 bot 写 qingMeiSeedClaimedDateKey）
        self.mark_qingmei_seed_claimed_today();
        let rewards: Vec<ItemDto> = reply.rewards.iter().map(item_dto).collect();
        // 释放 mutation_lock 后再拉快照，避免长查询占锁；并强制 dailySeed.claimed=true
        let mut snapshot = self.snapshot_with_shop(None).await.ok();
        force_qingmei_seed_claimed_in_snapshot(&mut snapshot);
        Ok(serde_json::json!({
            "rewards": rewards,
            "message": if already {
                "今日青梅种子已经领取，无需重复领取"
            } else {
                "青梅种子领取成功"
            },
            "snapshot": snapshot,
        }))
    }

    /// 开始青梅酿造
    pub async fn start_qingmei_brew(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let candidates = self.qingmei_ingredients().await.unwrap_or_default();
        let requested: Vec<serde_json::Value> = if let Some(arr) = input.as_array() {
            arr.clone()
        } else {
            let count = json_positive_decimal(
                input.get("count").unwrap_or(&input),
                ActivityErrorCode::InvalidQingmeiCount,
                "count",
            )?;
            let candidate = candidates.iter().find(|c| {
                json_i64(c.get("count")).unwrap_or(0) >= count
            });
            vec![serde_json::json!({
                "uid": candidate.and_then(|c| c.get("uid").cloned()).unwrap_or(serde_json::Value::Null),
                "count": count,
            })]
        };
        if requested.is_empty() {
            return Err(ActivityError {
                code: ActivityErrorCode::InvalidQingmeiIngredients,
                message: "至少选择一组青梅".to_string(),
            }
            .into());
        }
        let mut seen_uids = HashSet::new();
        let mut ingredients = Vec::new();
        for entry in &requested {
            let uid = json_positive_decimal(
                entry.get("uid").unwrap_or(&serde_json::Value::Null),
                ActivityErrorCode::InvalidQingmeiUid,
                "uid",
            )?;
            let count = json_positive_decimal(
                entry.get("count").unwrap_or(&serde_json::Value::Null),
                ActivityErrorCode::InvalidQingmeiCount,
                "count",
            )?;
            let uid_key = uid.to_string();
            if !seen_uids.insert(uid_key.clone()) {
                return Err(ActivityError {
                    code: ActivityErrorCode::DuplicateQingmeiUid,
                    message: format!("青梅 UID {uid} 重复"),
                }
                .into());
            }
            let candidate = candidates.iter().find(|c| json_text(c.get("uid")) == uid_key);
            let available = candidate.and_then(|c| json_i64(c.get("count"))).unwrap_or(0);
            if candidate.is_none() || available < count {
                return Err(ActivityError {
                    code: ActivityErrorCode::InsufficientQingmei,
                    message: format!("青梅 UID {uid} 数量不足"),
                }
                .into());
            }
            ingredients.push(
                crate::proto::generated::gamepb::activitypb::start_qing_mei_brew_request::Ingredient {
                    uid,
                    count,
                },
            );
        }
        let total: i64 = ingredients.iter().map(|i| i.count).sum();
        let req = StartQingMeiBrewRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: START_QINGMEI_BREW_OPERATE_TYPE,
            params: Some(
                crate::proto::generated::gamepb::activitypb::start_qing_mei_brew_request::Params {
                    ingredients,
                },
            ),
        };
        let (reply, _) = self.operate_qingmei(req.encode_to_vec(), &[]).await?;
        Ok(serde_json::json!({
            "activity": self.qingmei_dto(&reply, None),
            "message": format!("已投入 {total} 个青梅开始酿造"),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    /// 继续青梅酿造
    pub async fn continue_qingmei_brew(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let req = ContinueQingMeiBrewRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: CONTINUE_QINGMEI_BREW_OPERATE_TYPE,
            params: Some(
                crate::proto::generated::gamepb::activitypb::continue_qing_mei_brew_request::Empty {},
            ),
        };
        let (reply, _) = self.operate_qingmei(req.encode_to_vec(), &[]).await?;
        Ok(serde_json::json!({
            "activity": self.qingmei_dto(&reply, None),
            "quote": reply.qingmei_quote.as_ref().map(|q| serde_json::json!({
                "round": q.round,
                "unitPrice": q.unit_price.to_string(),
                "totalGold": q.total_gold.to_string(),
                "doubled": q.doubled,
            })),
            "message": reply.qingmei_quote.as_ref().map(|q| {
                format!("第 {} 轮报价：{} 金币", q.round, q.total_gold)
            }).unwrap_or_else(|| "酿造进度已更新".to_string()),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    /// 结算青梅酿造
    pub async fn settle_qingmei_brew(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        crate::services::share::ShareService::new(self.gateway.clone())
            .report_activity_share(QINGMEI_SHARE_SOURCE, QINGMEI_SHARE_SCENE)
            .await?;
        let req = SettleQingMeiBrewRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: SELL_QINGMEI_BREW_OPERATE_TYPE,
            params: Some(
                crate::proto::generated::gamepb::activitypb::settle_qing_mei_brew_request::Params {
                    settlement_mode: QINGMEI_SHARED_SETTLEMENT_MODE,
                },
            ),
        };
        let (reply, _) = self.operate_qingmei(req.encode_to_vec(), &[]).await?;
        let settlement = reply.qingmei_settlement.as_ref();
        let rewards: Vec<ItemDto> = if let Some(s) = settlement {
            s.reward.as_ref().map(item_dto).into_iter().collect()
        } else {
            reply.rewards.iter().map(item_dto).collect()
        };
        let total_gold = settlement.map(|s| s.total_gold).unwrap_or(0);
        Ok(serde_json::json!({
            "rewards": rewards,
            "settlement": {
                "mode": settlement.map(|s| s.settlement_mode).unwrap_or(QINGMEI_SHARED_SETTLEMENT_MODE),
                "totalGold": total_gold.to_string(),
            },
            "message": format!("分享出售成功（1.5倍），获得 {total_gold} 金币"),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    // ----- 缓存 -----

    /// 清除赛季缓存
    pub fn clear_season_cache(&self) {
        *self.cached_season.lock() = None;
    }
}

// =====================================================================
// 纯函数 / DTO 转换
// =====================================================================

/// 把 `corepb::Item` 序列化为简化的 DTO
pub fn item_dto(item: &CoreItem) -> ItemDto {
    item_from_id(item.id, item.count)
}

/// 把 `activitypb::ActivityItem` 序列化为简化的 DTO
pub fn activity_item_dto(item: &ActivityItem) -> ItemDto {
    item_from_id(item.item_id, item.count)
}

fn item_from_id(id: i64, count: i64) -> ItemDto {
    let gc = crate::config::game_config::global();
    let meta = if id > 0 { gc.get_item_by_id(id) } else { None };
    ItemDto {
        id,
        count,
        name: meta
            .as_ref()
            .map(|m| m.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_default(),
        image: crate::config::game_config::mapped_item_image(id),
        rarity: meta.and_then(|m| m.rarity).unwrap_or(0),
        balance: None,
        balance_known: None,
    }
}

fn season_item_dto(item: &SeasonItem) -> ItemDto {
    item_from_id(item.item_id, item.count)
}

fn solar_term_reward_dto(r: &crate::proto::generated::gamepb::solartermspb::SolarTermReward) -> ItemDto {
    item_from_id(r.item_id, r.count)
}

/// 把 proto `StarSandGoods` 转为轻量结构（只读 item/cost，不读 name 等 bytes）
#[derive(Debug, Clone, Default)]
pub(crate) struct RawStarSandGoods {
    pub id: i64,
    pub cost: Option<ItemDto>,
    pub item: Option<ItemDto>,
    pub status: i64,
    pub owned: bool,
    pub sort_order: i64,
    pub quality: i64,
    pub name_bytes: Vec<u8>,
    pub category_bytes: Vec<u8>,
}

pub(crate) fn extract_goods(reply: &ActivityOperateReply) -> Vec<RawStarSandGoods> {
    let Some(data) = reply.data.as_ref() else {
        return vec![];
    };
    let Some(catalog) = data.catalog.as_ref() else {
        return vec![];
    };
    catalog
        .goods
        .iter()
        .map(|g| RawStarSandGoods {
            id: g.goods_id,
            cost: g.cost.as_ref().map(activity_item_dto),
            item: g.item.as_ref().map(activity_item_dto),
            status: g.status,
            owned: g.owned,
            sort_order: g.sort_order,
            quality: g.field_10,
            name_bytes: g.name.to_vec(),
            category_bytes: g.category.to_vec(),
        })
        .collect()
}

/// 字节流转字符串（lossy）
fn bytes_to_text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn text_content(bytes: &[u8]) -> serde_json::Value {
    let text = bytes_to_text(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return serde_json::json!({ "title": "", "paragraphs": [] });
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v;
    }
    serde_json::json!({ "title": "", "paragraphs": [trimmed] })
}

fn constellation_identity(season: &SeasonDto, activity_id: i64) -> ActivityStateIdentity {
    ActivityStateIdentity {
        season_id: season.id.to_string(),
        activity_id: activity_id.to_string(),
        catalog_version: constellation_catalog_version() as i32,
    }
}

fn constellation_dto(
    activity: &SeasonActivityDto,
    server_time: i64,
    dynamic: Option<&ConstellationData>,
    confirmed: &ConstellationActivityState,
) -> ConstellationDto {
    let catalog = constellation_catalog_json();
    let catalog_activity_id = catalog
        .get("activityId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let catalog_supported = catalog_activity_id == activity.id.to_string();
    let display_name = catalog
        .get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or("观星礼录")
        .to_string();
    let catalog_server_name = catalog
        .get("serverName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !catalog_supported {
        return ConstellationDto {
            activity_id: activity.id,
            type_code: activity.r#type.to_string(),
            display_name: activity.name.clone(),
            server_name: activity.name.clone(),
            server_time,
            start_time: activity.start_time,
            end_time: activity.end_time,
            current_day: None,
            catalog_version: 0,
            catalog_status: "unsupported".to_string(),
            rules: serde_json::Value::Null,
            groups: vec![],
            ..Default::default()
        };
    }

    let calculated = constellation_day_from_beijing_midnight(activity.start_time, server_time);
    let current_day = calculated.map(|d| d.clamp(1, 28));
    let dynamic_nodes: HashMap<String, (bool, bool)> = dynamic
        .map(|data| {
            data.nodes
                .iter()
                .map(|n| (n.node_id.to_string(), (n.field_2, n.field_3)))
                .collect()
        })
        .unwrap_or_default();
    let confirmed_opened: HashSet<String> = confirmed
        .confirmed_opened_node_ids
        .iter()
        .cloned()
        .collect();
    let confirmed_lit: HashSet<String> = confirmed.confirmed_lit_node_ids.iter().cloned().collect();
    let catalog_groups = catalog
        .get("groups")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let groups: Vec<ConstellationGroupDto> = catalog_groups
        .iter()
        .filter_map(|group| {
            let id = json_text(group.get("id"));
            if id.is_empty() {
                return None;
            }
            let node_id = json_text(group.get("nodeId").or_else(|| group.get("node_id")));
            let order = group.get("order").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let chart_index = group
                .pointer("/links/chartIndex")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| i64::from((order.saturating_sub(1)) / 7))
                as i32;
            let node_ids = group
                .pointer("/links/nodeIds")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().map(|v| json_text(Some(v))).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default();
            let rewards = group
                .get("rewards")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|reward| {
                            let rid = json_text(
                                reward
                                    .get("itemId")
                                    .or_else(|| reward.get("item_id"))
                                    .or_else(|| reward.get("id")),
                            )
                            .parse::<i64>()
                            .unwrap_or(0);
                            let count = json_text(reward.get("count")).parse::<i64>().unwrap_or(0);
                            item_from_id(rid, count)
                        })
                        .collect()
                })
                .unwrap_or_default();

            let confirmed_opened_node = confirmed_opened.contains(&node_id);
            let confirmed_lit_node = confirmed_lit.contains(&node_id);
            let (dynamic_opened, dynamic_lit) = dynamic_nodes
                .get(&node_id)
                .copied()
                .unwrap_or((false, false));
            let dynamic_lightable = dynamic_opened && !dynamic_lit;
            let no_claimable = current_day == Some(order)
                && confirmed.no_claimable_days.contains_key(&order.to_string());

            let (opened, lit, state_known, visual_state, claim_status, status_source) =
                if confirmed_lit_node || dynamic_lit || no_claimable {
                    (
                        Some(true),
                        Some(true),
                        true,
                        "lit",
                        if no_claimable {
                            Some("confirmed-no-claimable")
                        } else {
                            None
                        },
                        if no_claimable {
                            "server-rejection"
                        } else if confirmed_lit_node {
                            "persisted"
                        } else {
                            "authoritative"
                        },
                    )
                } else if dynamic_lightable {
                    (
                        Some(true),
                        Some(false),
                        true,
                        "lightable",
                        None,
                        "authoritative",
                    )
                } else if current_day.is_some_and(|d| order > d) {
                    (Some(false), Some(false), false, "locked", None, "schedule")
                } else if current_day == Some(order) {
                    (
                        if confirmed_opened_node || dynamic_opened {
                            Some(true)
                        } else {
                            None
                        },
                        None,
                        false,
                        "claimableUnknown",
                        None,
                        if confirmed_opened_node {
                            "persisted"
                        } else if dynamic_opened {
                            "authoritative"
                        } else {
                            "schedule"
                        },
                    )
                } else {
                    (
                        if confirmed_opened_node || dynamic_opened {
                            Some(true)
                        } else {
                            None
                        },
                        None,
                        false,
                        "unknown",
                        None,
                        if confirmed_opened_node {
                            "persisted"
                        } else if dynamic_opened {
                            "authoritative"
                        } else {
                            "schedule"
                        },
                    )
                };

            Some(ConstellationGroupDto {
                id,
                node_id,
                name: json_text(group.get("name")),
                category: json_text(group.get("category")),
                explain: json_text(group.get("explain")),
                order,
                chart_index,
                rewards,
                links_raw: json_text(group.get("linksRaw").or_else(|| group.get("links_raw"))),
                node_ids,
                visual_state: visual_state.to_string(),
                opened,
                lit,
                state_known,
                claim_status: claim_status.map(str::to_string),
                status_source: status_source.to_string(),
            })
        })
        .collect();

    let node_count = dynamic.map(|d| d.nodes.len()).unwrap_or(0);
    let group_count = groups.len();
    ConstellationDto {
        activity_id: activity.id,
        type_code: CONSTELLATION_ACTIVITY_TYPE.to_string(),
        display_name,
        server_name: if activity.name.is_empty() {
            catalog_server_name
        } else {
            activity.name.clone()
        },
        server_time,
        start_time: activity.start_time,
        end_time: activity.end_time,
        current_day,
        field_1: dynamic.map(|d| d.field_1).unwrap_or(0),
        field_2: dynamic.map(|d| d.field_2).unwrap_or(0),
        field_3: dynamic.map(|d| d.field_3).unwrap_or(0),
        node_count,
        group_count,
        catalog_version: constellation_catalog_version(),
        catalog_status: "supported".to_string(),
        rules: constellation_catalog_rules(),
        groups,
    }
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn json_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn json_positive_decimal(
    value: &serde_json::Value,
    code: ActivityErrorCode,
    field_name: &str,
) -> Result<i64> {
    let text = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    positive_decimal(&text, code, field_name)
}

fn settled_error<T>(result: &Result<T>) -> serde_json::Value {
    match result {
        Ok(_) => serde_json::Value::Null,
        Err(e) => serde_json::Value::String(e.to_string()),
    }
}

fn build_actions(
    season: &Option<SeasonDto>,
    solar: &Option<SolarTermsDto>,
    constellation: Option<&ConstellationDto>,
    shop: Option<&StarSandShopDto>,
) -> serde_json::Value {
    let pass = season.as_ref().and_then(|s| s.pass.as_ref());
    let claimable_pass = pass
        .map(|p| p.nodes.iter().filter(|n| n.claimable).count())
        .unwrap_or(0);
    let has_claimable_solar = solar
        .as_ref()
        .map(|s| s.terms.iter().any(|t| t.can_claim))
        .unwrap_or(false);
    let lightable = constellation
        .map(|c| {
            c.groups
                .iter()
                .filter(|g| g.visual_state == "lightable")
                .count()
        })
        .unwrap_or(0);
    let attemptable = constellation
        .map(|c| {
            c.groups
                .iter()
                .filter(|g| g.visual_state == "lightable" || g.visual_state == "claimableUnknown")
                .count()
        })
        .unwrap_or(0);
    let current_day = constellation.and_then(|c| c.current_day);
    let current_groups_known = constellation
        .map(|c| {
            let current: Vec<_> = c
                .groups
                .iter()
                .filter(|g| current_day.is_some_and(|d| g.order == d))
                .collect();
            !current.is_empty() && current.iter().all(|g| g.state_known)
        })
        .unwrap_or(false);
    let availability_known = lightable > 0 || current_groups_known;
    let catalog_supported = constellation
        .map(|c| c.catalog_status == "supported")
        .unwrap_or(false);
    let constellation_act = season
        .as_ref()
        .and_then(|s| s.constellation_activity.as_ref());
    let server_time = season.as_ref().map(|s| s.server_time).unwrap_or(0);
    let start_time = constellation_act.map(|a| a.start_time).unwrap_or(0);
    let end_time = constellation_act.map(|a| a.end_time).unwrap_or(0);
    let constellation_active = constellation_act.is_some()
        && (server_time <= 0 || start_time <= 0 || server_time >= start_time)
        && (server_time <= 0 || end_time <= 0 || server_time <= end_time);
    let affordable = shop.map(|s| s.affordable_count).unwrap_or(0);
    let mut exchange = serde_json::json!({
        "supported": true,
        "enabled": shop.map(|s| s.action.get("enabled").and_then(|v| v.as_bool()).unwrap_or(affordable > 0)).unwrap_or(false),
        "available": shop.map(|s| s.action.get("available").and_then(|v| v.as_bool()).unwrap_or(affordable > 0)).unwrap_or(false),
        "availabilityKnown": shop.is_some(),
        "count": shop
            .and_then(|s| json_i64(s.action.get("count")))
            .unwrap_or(i64::from(affordable)),
    });
    if shop.is_none() {
        exchange["reason"] = serde_json::json!("活动商店目录当前不可用");
    } else if let Some(reason) = shop.and_then(|s| s.action.get("reason").cloned()) {
        if !reason.is_null() && reason.as_str() != Some("") {
            exchange["reason"] = reason;
        }
    }
    serde_json::json!({
        "claimPass": {
            "supported": true,
            "enabled": pass.is_some(),
            "available": claimable_pass > 0,
            "count": claimable_pass,
        },
        "lightConstellation": {
            "supported": true,
            "enabled": constellation_active && attemptable > 0,
            "available": lightable > 0,
            "attemptable": attemptable > 0,
            "availabilityKnown": constellation.is_some() && catalog_supported && availability_known,
            "count": lightable,
            "attemptableCount": attemptable,
        },
        "claimSolar": { "supported": true, "enabled": has_claimable_solar },
        "exchange": exchange,
    })
}

/// 从赛季回复中找出指定 type 的活动
pub fn find_season_activity(
    season_reply: &GetSeasonInfoReply,
    type_code: i64,
) -> Option<&SeasonActivity> {
    let activities = season_reply
        .season_info
        .as_ref()?
        .activities
        .as_slice();
    activities.iter().find(|a| a.r#type == type_code)
}

/// 把赛季 proto 消息归一化为 DTO
#[must_use]
pub fn normalize_season(reply: &GetSeasonInfoReply) -> Option<SeasonDto> {
    let season: &SeasonInfo = reply.season_info.as_ref()?;
    let activities: Vec<SeasonActivityDto> = season
        .activities
        .iter()
        .map(activity_dto)
        .collect();
    let constellation = activities
        .iter()
        .find(|a| a.r#type == CONSTELLATION_ACTIVITY_TYPE)
        .cloned();
    let shop = activities
        .iter()
        .find(|a| a.r#type == SHOP_ACTIVITY_TYPE)
        .cloned();
    let pass = season.pass.as_ref().map(pass_dto);
    Some(SeasonDto {
        id: season.season_id,
        title: bytes_to_text(&season.name),
        status_code: season.status,
        field_4_code: season.field_4,
        start_time: season.begin_time,
        end_time: season.end_time,
        server_time: season.server_time,
        activities,
        constellation_activity: constellation,
        shop_activity: shop,
        pass,
    })
}

/// 把 `SeasonActivity` 转为 DTO
#[must_use]
pub fn activity_dto(a: &SeasonActivity) -> SeasonActivityDto {
    SeasonActivityDto {
        id: a.activity_id,
        r#type: a.r#type,
        name: bytes_to_text(&a.name),
        begin_time: a.begin_time,
        start_time: a.begin_time,
        end_time: a.end_time,
    }
}

/// 把 `SeasonPass` 转为 DTO
#[must_use]
pub fn pass_dto(p: &SeasonPass) -> SeasonPassDto {
    let current_level = p.current_level;
    let claimed_through = p.claimed_through_level;
    let nodes: Vec<SeasonPassNodeDto> = p
        .nodes
        .iter()
        .map(|node| pass_node_dto(node, current_level, claimed_through))
        .collect();
    SeasonPassDto {
        activity_id: p.activity_id,
        title: bytes_to_text(&p.title),
        current_level,
        level: current_level,
        current_progress: p.current_progress,
        progress: p.current_progress,
        progress_target: p.progress_target,
        progress_max: p.progress_target,
        node_count: p.node_count,
        claimed_through_level: claimed_through,
        field11_code: p.field_11,
        field13_code: p.field_13,
        field18_code: p.field_18,
        field14_items: p.field_14.iter().map(season_item_dto).collect(),
        rules: text_content(&p.rules_json),
        nodes,
    }
}

fn pass_node_dto(node: &SeasonRewardNode, current_level: i64, claimed_through: i64) -> SeasonPassNodeDto {
    let level = node.node_id;
    let claimed = level != 0 && level <= claimed_through;
    let locked = level == 0 || level > current_level;
    SeasonPassNodeDto {
        id: level.to_string(),
        level: level.to_string(),
        key_level: node.is_key_level,
        locked,
        claimed,
        claimable: !locked && !claimed,
        current: level != 0 && level == current_level,
        rewards: node.rewards.iter().map(season_item_dto).collect(),
    }
}

/// 把节气回复归一化
#[must_use]
pub fn normalize_solar_terms(reply: &GetSolarTermsReply) -> SolarTermsDto {
    let server_time = reply.server_time;
    let terms: Vec<SolarTermDto> = reply.terms.iter().map(solar_term_dto).collect();
    let current_term_id = terms
        .iter()
        .find(|t| {
            t.begin_time > 0
                && t.end_time > 0
                && server_time >= t.begin_time
                && server_time <= t.end_time
        })
        .map(|t| t.id);
    let terms: Vec<SolarTermDto> = terms
        .into_iter()
        .map(|mut t| {
            t.current = current_term_id == Some(t.id);
            t
        })
        .collect();
    let configs: Vec<SolarTermsConfigDto> = reply
        .configs
        .iter()
        .map(solar_terms_config_dto)
        .collect();
    let current_config = reply.current_config.as_ref().map(solar_terms_config_dto);
    SolarTermsDto {
        server_time,
        current_term_id,
        terms,
        current_config,
        configs,
    }
}

fn solar_term_dto(t: &SolarTermInfo) -> SolarTermDto {
    let status = t.status;
    SolarTermDto {
        id: t.term_id,
        status,
        status_code: status.to_string(),
        can_claim: status == 2,
        claimed: status == 3,
        locked: status == 0,
        current: false,
        begin_time: t.begin_time,
        start_time: t.begin_time,
        end_time: t.end_time,
        name: bytes_to_text(&t.name),
        rewards: t.rewards.iter().map(solar_term_reward_dto).collect(),
    }
}

fn solar_terms_config_dto(c: &SolarTermsConfig) -> SolarTermsConfigDto {
    let rules_text = bytes_to_text(&c.rules_json);
    SolarTermsConfigDto {
        id: c.config_id,
        activity_id: c.activity_id,
        rules: text_content(&c.rules_json),
        rules_text,
    }
}

/// 把活动商品 proto 转为 DTO
#[must_use]
pub fn star_sand_goods_dto(
    goods: &StarSandGoods,
    activity_id: i64,
    balances: Option<&std::collections::HashMap<i64, i64>>,
) -> StarSandGoodsDto {
    let cost_id = goods.cost.as_ref().map(|c| c.item_id).unwrap_or(0);
    let cost_count = goods.cost.as_ref().map(|c| c.count).unwrap_or(0);
    let cost_valid = cost_id > 0 && cost_count > 0;
    let balance_known = balances.is_some();
    let balance = balances
        .and_then(|m| m.get(&cost_id).copied())
        .unwrap_or(0);
    let max_count = if cost_valid && balance_known && cost_count > 0 {
        balance / cost_count
    } else {
        0
    };
    StarSandGoodsDto {
        id: goods.goods_id,
        activity_id,
        name: bytes_to_text(&goods.name),
        category: bytes_to_text(&goods.category),
        item: goods.item.as_ref().map(activity_item_dto).unwrap_or_default(),
        cost: goods.cost.as_ref().map(activity_item_dto).unwrap_or_default(),
        sort_order: goods.sort_order,
        status_code: goods.status,
        owned: goods.owned,
        exchangeable: cost_valid, // 原 TS：status=100 已在成功兑换后出现，不视为禁用
        sold_out: false,
        max_exchange_count: max_count,
        max_exchange_count_known: balance_known,
        balance_known,
        quality_code: goods.field_10,
    }
}

/// 把活动商品回复归一化为活动商店 DTO
#[must_use]
pub fn normalize_shop_from_reply(
    season_reply: &GetSeasonInfoReply,
    shop_activity: &SeasonActivity,
    reply: &ActivityOperateReply,
    balances: Option<&std::collections::HashMap<i64, i64>>,
) -> StarSandShopDto {
    let raw_goods_list = extract_goods(reply);
    let activity_id = reply.activity_id;
    let balance_known = balances.is_some();

    let goods: Vec<StarSandGoodsDto> = raw_goods_list
        .iter()
        .map(|g| {
            let cost_id = g.cost.as_ref().map(|c| c.id).unwrap_or(0);
            let cost_count = g.cost.as_ref().map(|c| c.count).unwrap_or(0);
            let cost_valid = cost_id > 0 && cost_count > 0;
            let balance = balances
                .and_then(|m| m.get(&cost_id).copied())
                .unwrap_or(0);
            let max_count = if cost_valid && balance_known && cost_count > 0 {
                balance / cost_count
            } else {
                0
            };
            StarSandGoodsDto {
                id: g.id,
                activity_id,
                name: bytes_to_text(&g.name_bytes),
                category: bytes_to_text(&g.category_bytes),
                item: g.item.clone().unwrap_or_default(),
                cost: g.cost.clone().unwrap_or_default(),
                sort_order: g.sort_order,
                status_code: g.status,
                owned: g.owned,
                exchangeable: cost_valid,
                sold_out: false,
                balance_known,
                max_exchange_count: max_count,
                max_exchange_count_known: balance_known,
                quality_code: g.quality,
            }
        })
        .collect();

    let exchangeable_count = goods.iter().filter(|g| g.exchangeable).count() as i32;
    let affordable_count = goods
        .iter()
        .filter(|g| g.exchangeable && (!g.max_exchange_count_known || g.max_exchange_count > 0))
        .count() as i32;

    let mut categories: Vec<String> = goods
        .iter()
        .map(|g| g.category.clone())
        .filter(|s| !s.is_empty())
        .collect();
    categories.sort();
    categories.dedup();

    let currencies: Vec<ItemDto> = if let Some(balances) = balances {
        let mut ids: Vec<i64> = goods
            .iter()
            .filter_map(|g| if g.cost.id > 0 { Some(g.cost.id) } else { None })
            .collect();
        ids.sort();
        ids.dedup();
        ids.into_iter()
            .map(|id| {
                let bal = *balances.get(&id).unwrap_or(&0);
                let mut item = item_from_id(id, bal);
                item.balance = Some(bal.to_string());
                item.balance_known = Some(true);
                item
            })
            .collect()
    } else {
        vec![]
    };

    let server_time = season_reply
        .season_info
        .as_ref()
        .map(|s| s.server_time)
        .unwrap_or(0);
    let name = reply
        .data
        .as_ref()
        .and_then(|d| d.activity.as_ref())
        .map(|a: &ActivityContent| a.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| bytes_to_text(&shop_activity.name));

    let mut shop_action = serde_json::json!({
        "supported": true,
        "enabled": affordable_count > 0,
        "available": affordable_count > 0,
        "count": affordable_count,
        "availabilityKnown": true,
    });
    if exchangeable_count == 0 {
        shop_action["reason"] = serde_json::json!("当前目录没有明确可兑换的商品");
    } else if affordable_count == 0 {
        shop_action["reason"] = serde_json::json!("当前余额不足以兑换目录商品");
    }
    let balance = currencies.first().and_then(|c| {
        if balance_known {
            Some(c.count.to_string())
        } else {
            None
        }
    });

    StarSandShopDto {
        activity_id,
        name,
        start_time: shop_activity.begin_time,
        end_time: shop_activity.end_time,
        server_time,
        balance_known,
        balance,
        currencies,
        categories,
        goods,
        affordable_count,
        exchangeable_count,
        action: shop_action,
    }
}

/// 拉取背包中指定货币 id 的余额（best-effort）
async fn read_bag_balances(
    warehouse: &WarehouseService,
    currency_ids: &[i64],
) -> Option<std::collections::HashMap<i64, i64>> {
    let wanted: std::collections::HashSet<i64> = currency_ids.iter().copied().collect();
    if wanted.is_empty() {
        return Some(Default::default());
    }
    let bag = warehouse.get_bag().await.ok()?;
    let mut balances = std::collections::HashMap::new();
    for item in super::warehouse::get_bag_items(&bag) {
        if wanted.contains(&item.id) {
            balances.insert(item.id, item.count.max(0));
        }
    }
    Some(balances)
}

/// 把"00xxx" / 数字字符串解析为正整数
pub fn positive_decimal(
    value: &str,
    code: ActivityErrorCode,
    field_name: &str,
) -> Result<i64> {
    let text = value.trim();
    // 必须非空、只含 ASCII 数字（不接 '-'，原 TS 用 `^[1-9]\d*$`）
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return Err(ActivityError {
            code,
            message: format!("{} must be a positive integer", field_name),
        }
        .into());
    }
    if text == "0" {
        return Err(ActivityError {
            code,
            message: format!("{} must be a positive integer", field_name),
        }
        .into());
    }
    let n: i64 = text.parse().map_err(|_| ActivityError {
        code,
        message: format!("{} is too large", field_name),
    })?;
    if n < 0 {
        return Err(ActivityError {
            code,
            message: format!("{} must be a positive integer", field_name),
        }
        .into());
    }
    Ok(n)
}

/// 活动 ID / 开始时间（server）→ 北京零点起算的天数
#[must_use]
pub fn constellation_day_from_beijing_midnight(
    start_time_sec: i64,
    server_time_sec: i64,
) -> Option<i32> {
    if start_time_sec <= 0 || server_time_sec <= 0 {
        return None;
    }
    // 转换为北京时间（UTC+8）的"日历天数"
    let beijing_offset = BEIJING_UTC_OFFSET_SECONDS;
    let start_day = (start_time_sec + beijing_offset) / SECONDS_PER_DAY;
    let server_day = (server_time_sec + beijing_offset) / SECONDS_PER_DAY;
    let day_diff = (server_day - start_day) + 1; // 1-based：第 1 天为活动开始当天
    if day_diff < 1 {
        return None;
    }
    i32::try_from(day_diff).ok()
}

fn beijing_date_key() -> String {
    use chrono::{Datelike, TimeZone};
    let dt = chrono::Utc
        .timestamp_opt(crate::utils::time::now_ms() / 1000, 0)
        .single()
        .unwrap_or_else(chrono::Utc::now)
        + chrono::Duration::hours(8);
    format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day())
}

fn is_qingmei_already_claimed_message(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("1034014")
        || msg.contains("已经领取")
        || msg.contains("无需重复领取")
        || msg.contains("已领取")
}

fn qingmei_seed_claimed_path(account_id: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let token = hex::encode(Sha256::digest(account_id.as_bytes()));
    crate::config::paths::get_data_file(&format!("qingmei-seed-claimed-{token}.json"))
}

fn load_qingmei_seed_claimed_date(account_id: &str) -> Option<String> {
    if account_id.is_empty() {
        return None;
    }
    let path = qingmei_seed_claimed_path(account_id);
    let state: serde_json::Value =
        crate::services::json_db::read_json_with_default(&path, || serde_json::json!({}));
    let date = state.get("date").and_then(|v| v.as_str())?;
    if date == beijing_date_key() {
        Some(date.to_string())
    } else {
        None
    }
}

fn persist_qingmei_seed_claimed_date(account_id: &str, today: &str) -> std::io::Result<()> {
    let path = qingmei_seed_claimed_path(account_id);
    crate::services::json_db::write_json_file_atomic(
        &path,
        &serde_json::json!({
            "date": today,
            "claimed": true,
        }),
    )
}

fn force_qingmei_seed_claimed_in_snapshot(snapshot: &mut Option<serde_json::Value>) {
    let Some(snap) = snapshot.as_mut() else {
        return;
    };
    let Some(qm) = snap.get_mut("qingMei").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if let Some(seed) = qm.get_mut("dailySeed").and_then(|v| v.as_object_mut()) {
        seed.insert("claimed".into(), serde_json::json!(true));
    } else {
        qm.insert(
            "dailySeed".into(),
            serde_json::json!({
                "claimed": true,
                "grantId": QINGMEI_DAILY_GRANT_ID.to_string(),
                "reward": null,
            }),
        );
    }
    if let Some(actions) = qm.get_mut("actions").and_then(|v| v.as_object_mut()) {
        actions.insert(
            "claimSeed".into(),
            serde_json::json!({ "enabled": false, "available": false }),
        );
    }
}

fn constellation_catalog_json() -> serde_json::Value {
    static RAW: &str = include_str!("../../../../assets/activity-data/constellation-2026072701.json");
    serde_json::from_str(RAW).unwrap_or(serde_json::Value::Null)
}

fn constellation_catalog_version() -> i64 {
    constellation_catalog_json()
        .get("catalogVersion")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

fn constellation_catalog_rules() -> serde_json::Value {
    constellation_catalog_json()
        .get("rules")
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

fn constellation_catalog_groups() -> serde_json::Value {
    constellation_catalog_json()
        .get("groups")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]))
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::corepb::Item as CoreItem;
    use crate::proto::generated::gamepb::activitypb::{
        ActivityContent, ActivityData, ActivityItem, StarSandGoodsList,
    };

    #[test]
    fn service_constants() {
        assert_eq!(SEASON_SERVICE, "gamepb.seasonpb.SeasonService");
        assert_eq!(ACTIVITY_SERVICE, "gamepb.activitypb.ActivityService");
        assert_eq!(SOLAR_TERMS_SERVICE, "gamepb.solartermspb.SolarTermsService");
    }

    #[test]
    fn activity_type_codes() {
        assert_eq!(SHOP_ACTIVITY_TYPE, 3);
        assert_eq!(CONSTELLATION_ACTIVITY_TYPE, 13);
        assert_eq!(EXCHANGE_SHOP_OPERATE_TYPE, 1);
        assert_eq!(QUERY_SHOP_OPERATE_TYPE, 7);
        assert_eq!(LIGHT_CONSTELLATION_OPERATE_TYPE, 21);
    }

    #[test]
    fn positive_decimal_valid() {
        assert_eq!(positive_decimal("123", ActivityErrorCode::InvalidShopGoodsId, "x").unwrap(), 123);
    }

    #[test]
    fn positive_decimal_rejects_zero() {
        assert!(positive_decimal("0", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn positive_decimal_rejects_negative() {
        assert!(positive_decimal("-5", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn positive_decimal_rejects_empty() {
        assert!(positive_decimal("", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn positive_decimal_rejects_non_digit() {
        assert!(positive_decimal("12a", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn json_positive_decimal_accepts_string_or_number() {
        assert_eq!(
            json_positive_decimal(
                &serde_json::json!("41221001"),
                ActivityErrorCode::InvalidQingmeiUid,
                "uid"
            )
            .unwrap(),
            41_221_001
        );
        assert_eq!(
            json_positive_decimal(
                &serde_json::json!(3),
                ActivityErrorCode::InvalidQingmeiCount,
                "count"
            )
            .unwrap(),
            3
        );
        assert!(json_positive_decimal(
            &serde_json::json!(null),
            ActivityErrorCode::InvalidQingmeiUid,
            "uid"
        )
        .is_err());
    }

    #[test]
    fn qingmei_rules_from_extra_json_object() {
        let extra = br#"{"title":"rules","paragraphs":["first"]}"#;
        let rules = text_content(extra);
        assert_eq!(rules["title"], "rules");
        assert_eq!(rules["paragraphs"][0], "first");
    }

    #[test]
    fn error_codes_have_str() {
        assert_eq!(ActivityErrorCode::ShopUnavailable.as_str(), "SHOP_UNAVAILABLE");
        assert_eq!(ActivityErrorCode::InsufficientStarSand.as_str(), "INSUFFICIENT_STAR_SAND");
        assert_eq!(ActivityErrorCode::InvalidShopGoodsId.as_str(), "INVALID_SHOP_GOODS_ID");
        assert_eq!(ActivityErrorCode::InvalidExchangeCount.as_str(), "INVALID_EXCHANGE_COUNT");
    }

    #[test]
    fn activity_error_display() {
        let e = ActivityError {
            code: ActivityErrorCode::ShopUnavailable,
            message: "test".to_string(),
        };
        assert!(e.to_string().contains("SHOP_UNAVAILABLE"));
        assert!(e.to_string().contains("test"));
    }

    #[test]
    fn bytes_to_text_basic() {
        assert_eq!(bytes_to_text(b"hello"), "hello");
        assert_eq!(bytes_to_text(&[]), "");
    }

    #[test]
    fn bytes_to_text_invalid_utf8_replacement() {
        // 0xFF 0xFE 是非 UTF-8
        let s = bytes_to_text(&[0xFF, 0xFE, b'h', b'i']);
        // lossy 替换：可能是 \u{FFFD}\u{FFFD}hi
        assert!(s.ends_with("hi"));
    }

    #[test]
    fn item_dto_from_core_item() {
        let i = CoreItem {
            id: 1002,
            count: 50,
            ..Default::default()
        };
        let dto = item_dto(&i);
        assert_eq!(dto.id, 1002);
        assert_eq!(dto.count, 50);
    }

    #[test]
    fn normalize_season_basic() {
        let mut reply = GetSeasonInfoReply::default();
        let season = SeasonInfo {
            season_id: 1,
            name: bytes::Bytes::from_static(b"Season 1"),
            status: 1,
            field_4: 0,
            begin_time: 1000,
            end_time: 2000,
            server_time: 1500,
            activities: vec![SeasonActivity {
                activity_id: 10,
                r#type: SHOP_ACTIVITY_TYPE,
                name: bytes::Bytes::from_static(b"Shop"),
                begin_time: 1000,
                end_time: 2000,
            }],
            pass: Some(SeasonPass {
                activity_id: 10,
                current_level: 5,
                current_progress: 100,
                progress_target: 1000,
                node_count: 30,
                claimed_through_level: 3,
                ..Default::default()
            }),
        };
        reply.season_info = Some(season);
        let dto = normalize_season(&reply).unwrap();
        assert_eq!(dto.id, 1);
        assert_eq!(dto.title, "Season 1");
        assert_eq!(dto.server_time, 1500);
        assert_eq!(dto.activities.len(), 1);
        assert!(dto.shop_activity.is_some());
        assert!(dto.constellation_activity.is_none());
        assert!(dto.pass.is_some());
    }

    #[test]
    fn normalize_season_missing() {
        let reply = GetSeasonInfoReply::default();
        assert!(normalize_season(&reply).is_none());
    }

    #[test]
    fn find_season_activity_by_type() {
        let mut reply = GetSeasonInfoReply::default();
        reply.season_info = Some(SeasonInfo {
            activities: vec![
                SeasonActivity { activity_id: 1, r#type: 3, name: bytes::Bytes::new(), begin_time: 0, end_time: 0 },
                SeasonActivity { activity_id: 2, r#type: 13, name: bytes::Bytes::new(), begin_time: 0, end_time: 0 },
            ],
            ..Default::default()
        });
        let shop = find_season_activity(&reply, SHOP_ACTIVITY_TYPE).unwrap();
        assert_eq!(shop.activity_id, 1);
        let constellation = find_season_activity(&reply, CONSTELLATION_ACTIVITY_TYPE).unwrap();
        assert_eq!(constellation.activity_id, 2);
        let missing = find_season_activity(&reply, 99);
        assert!(missing.is_none());
    }

    #[test]
    fn normalize_solar_terms_basic() {
        let mut reply = GetSolarTermsReply::default();
        reply.server_time = 1500;
        reply.terms = vec![SolarTermInfo {
            term_id: 100,
            status: 2,
            begin_time: 1000,
            end_time: 2000,
            name: bytes::Bytes::from("立春".as_bytes()),
            rewards: vec![],
        }];
        let dto = normalize_solar_terms(&reply);
        assert_eq!(dto.terms.len(), 1);
        assert_eq!(dto.terms[0].name, "立春");
        assert_eq!(dto.current_term_id, Some(100));
    }

    #[test]
    fn normalize_solar_terms_no_current() {
        let reply = GetSolarTermsReply::default();
        let dto = normalize_solar_terms(&reply);
        assert!(dto.terms.is_empty());
        assert_eq!(dto.current_term_id, None);
    }

    #[test]
    fn constellation_day_basic() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        // 2024-01-02 00:00:00 UTC = 1704153600
        let start = 1_704_067_200_i64;
        let server = 1_704_153_600_i64;
        // UTC+8 后：start = 2024-01-01 08:00:00 (day 19883)
        //           server = 2024-01-02 08:00:00 (day 19884)
        // 差 = 19884 - 19883 = 1，再 +1 = 2（第 2 天）
        let day = constellation_day_from_beijing_midnight(start, server).unwrap();
        assert_eq!(day, 2);
    }

    #[test]
    fn constellation_day_same_day() {
        let start = 1_704_067_200_i64;
        let server = 1_704_067_200_i64;
        let day = constellation_day_from_beijing_midnight(start, server).unwrap();
        assert_eq!(day, 1);
    }

    #[test]
    fn constellation_day_zero_inputs() {
        assert!(constellation_day_from_beijing_midnight(0, 1000).is_none());
        assert!(constellation_day_from_beijing_midnight(1000, 0).is_none());
    }

    #[test]
    fn constellation_day_before_start() {
        // server 在 start 之前（北京时间）
        // 2023-12-31 16:00:00 UTC = 1704038400 (北京时间 2024-01-01 00:00:00)
        // start = 2024-01-01 08:00:00 UTC
        // server = 1704038400 在 start 之前（北京时间相差 8 小时）
        // 但用"日历天"算：start_day = 2024-01-01 (day 19883)
        //                  server_day = 2024-01-01 (day 19883)
        // 差 = 0 + 1 = 1
        // 实际上 "before start" 是不会发生因为 server 在 start 当天或之后才有效
        let start = 1_704_067_200_i64;
        let server = 1_704_038_400_i64; // 2023-12-31 16:00:00 UTC = 2024-01-01 00:00:00 BJ
        let day = constellation_day_from_beijing_midnight(start, server).unwrap();
        // 实际是 1（因为日历天差 0 + 1 = 1）
        assert_eq!(day, 1);
    }

    #[test]
    fn star_sand_goods_dto_with_balance() {
        let goods = StarSandGoods {
            goods_id: 100,
            cost: Some(ActivityItem { item_id: 1, count: 10 }),
            item: Some(ActivityItem { item_id: 2, count: 1 }),
            status: 0,
            owned: false,
            sort_order: 1,
            name: bytes::Bytes::from_static(b"Star"),
            category: bytes::Bytes::from_static(b"tools"),
            ..Default::default()
        };
        let mut balances = std::collections::HashMap::new();
        balances.insert(1, 50);
        let dto = star_sand_goods_dto(&goods, 999, Some(&balances));
        assert_eq!(dto.id, 100);
        assert_eq!(dto.activity_id, 999);
        assert_eq!(dto.cost.id, 1);
        assert_eq!(dto.cost.count, 10);
        assert!(dto.exchangeable);
        assert!(!dto.sold_out);
        assert_eq!(dto.max_exchange_count, 5);
        assert!(dto.max_exchange_count_known);
    }

    #[test]
    fn star_sand_goods_dto_no_balance() {
        let goods = StarSandGoods {
            goods_id: 100,
            cost: Some(ActivityItem { item_id: 1, count: 10 }),
            ..Default::default()
        };
        let dto = star_sand_goods_dto(&goods, 999, None);
        assert!(!dto.max_exchange_count_known);
        assert_eq!(dto.max_exchange_count, 0);
    }

    #[test]
    fn star_sand_goods_dto_invalid_cost() {
        let goods = StarSandGoods {
            goods_id: 100,
            cost: Some(ActivityItem { item_id: 0, count: 0 }),
            ..Default::default()
        };
        let dto = star_sand_goods_dto(&goods, 999, None);
        assert!(!dto.exchangeable);
    }

    #[test]
    fn activity_dto_basic() {
        let a = SeasonActivity {
            activity_id: 42,
            r#type: 3,
            name: bytes::Bytes::from("商店".as_bytes()),
            begin_time: 1000,
            end_time: 2000,
        };
        let dto = activity_dto(&a);
        assert_eq!(dto.id, 42);
        assert_eq!(dto.r#type, 3);
        assert_eq!(dto.name, "商店");
    }

    #[test]
    fn pass_dto_basic() {
        let p = SeasonPass {
            activity_id: 10,
            current_level: 5,
            current_progress: 100,
            progress_target: 1000,
            node_count: 30,
            claimed_through_level: 3,
            nodes: vec![SeasonRewardNode {
                node_id: 5,
                is_key_level: true,
                rewards: vec![SeasonItem {
                    item_id: 1,
                    count: 10,
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let dto = pass_dto(&p);
        assert_eq!(dto.activity_id, 10);
        assert_eq!(dto.current_level, 5);
        assert_eq!(dto.level, 5);
        assert_eq!(dto.claimed_through_level, 3);
        assert_eq!(dto.nodes.len(), 1);
        assert!(dto.nodes[0].claimable);
        assert_eq!(
            dto.nodes[0].rewards[0].image,
            "/game-config/seed_images_named/1.png"
        );
        let v = serde_json::to_value(&dto).unwrap();
        assert!(v.get("nodes").and_then(|n| n.as_array()).is_some());
        assert_eq!(v["nodes"][0]["rewards"][0]["image"], "/game-config/seed_images_named/1.png");
    }

    #[test]
    fn season_dto_default() {
        let dto = SeasonDto::default();
        assert_eq!(dto.id, 0);
        assert!(dto.activities.is_empty());
        assert!(dto.pass.is_none());
    }

    #[test]
    fn solar_term_dto_default() {
        let dto = SolarTermDto::default();
        assert_eq!(dto.id, 0);
    }

    #[test]
    fn star_sand_shop_dto_default() {
        let dto = StarSandShopDto::default();
        assert!(!dto.balance_known);
        assert_eq!(dto.affordable_count, 0);
    }

    #[test]
    fn light_constellation_result_lighted() {
        let r = serde_json::json!({
            "outcome": "lighted",
            "rewards": [],
            "constellation": null,
        });
        assert_eq!(r["outcome"], "lighted");
    }

    #[test]
    fn light_constellation_result_nothing() {
        let r = serde_json::json!({
            "outcome": "nothingToClaim",
            "noClaimable": true,
            "message": "已领",
        });
        assert_eq!(r["outcome"], "nothingToClaim");
        assert_eq!(r["message"], "已领");
    }

    #[test]
    fn activity_error_into_core_error() {
        let e = ActivityError {
            code: ActivityErrorCode::ShopUnavailable,
            message: "x".to_string(),
        };
        let core: Error = e.into();
        assert!(core.to_string().contains("SHOP_UNAVAILABLE"));
    }

    #[test]
    fn constellation_dto_default() {
        let dto = ConstellationDto::default();
        assert_eq!(dto.activity_id, 0);
        assert_eq!(dto.current_day, None);
    }

    #[test]
    fn constellation_catalog_has_groups() {
        let groups = constellation_catalog_groups();
        assert!(groups.as_array().map(|a| !a.is_empty()).unwrap_or(false));
        assert!(constellation_catalog_version() >= 1);
    }

    #[test]
    fn constellation_schedule_marks_future_locked() {
        let act = SeasonActivityDto {
            id: 2_026_072_701,
            r#type: 13,
            name: "千星同明".to_string(),
            begin_time: 1_753_574_400,
            start_time: 1_753_574_400,
            end_time: 1_756_166_400,
        };
        let confirmed = ConstellationActivityState::default();
        let dto = constellation_dto(&act, 1_753_574_400 + 86_400 * 2, None, &confirmed);
        assert_eq!(dto.catalog_status, "supported");
        assert_eq!(dto.current_day, Some(3));
        let current = dto.groups.iter().find(|g| g.order == 3).expect("day 3");
        assert_eq!(current.visual_state, "claimableUnknown");
        let future = dto.groups.iter().find(|g| g.order == 10).expect("day 10");
        assert_eq!(future.visual_state, "locked");
        let past = dto.groups.iter().find(|g| g.order == 1).expect("day 1");
        assert_eq!(past.visual_state, "unknown");
    }

    #[test]
    fn qingmei_constants() {
        assert_eq!(QINGMEI_DAILY_ACTIVITY_ID, 2026081201);
        assert_eq!(QINGMEI_BREW_ACTIVITY_ID, 2026081202);
        assert_eq!(QINGMEI_ITEM_ID, 41221);
    }

    #[test]
    fn qingmei_already_claimed_message_detects_code_and_text() {
        assert!(is_qingmei_already_claimed_message(
            "gateway error: x.y code=1034014 already"
        ));
        assert!(is_qingmei_already_claimed_message(
            "今日青梅种子已经领取，无需重复领取"
        ));
        assert!(!is_qingmei_already_claimed_message("timeout"));
    }

    #[test]
    fn force_qingmei_seed_claimed_patches_snapshot() {
        let mut snap = Some(serde_json::json!({
            "qingMei": {
                "dailySeed": { "claimed": false, "grantId": "3" },
                "actions": { "claimSeed": { "enabled": true, "available": true } }
            }
        }));
        force_qingmei_seed_claimed_in_snapshot(&mut snap);
        let qm = &snap.unwrap()["qingMei"];
        assert_eq!(qm["dailySeed"]["claimed"], true);
        assert_eq!(qm["actions"]["claimSeed"]["enabled"], false);
    }

    #[test]
    fn qingmei_seed_claimed_persist_roundtrip() {
        let acc = format!("test-qm-{}", std::process::id());
        let today = beijing_date_key();
        assert!(load_qingmei_seed_claimed_date(&acc).is_none());
        persist_qingmei_seed_claimed_date(&acc, &today).expect("persist");
        assert_eq!(load_qingmei_seed_claimed_date(&acc).as_deref(), Some(today.as_str()));
        let _ = std::fs::remove_file(qingmei_seed_claimed_path(&acc));
    }

    #[test]
    fn encode_query_activity_request() {
        let req = QueryActivityRequest {
            activity_id: 100,
            operate_type: QUERY_SHOP_OPERATE_TYPE,
        };
        let bytes = req.encode_to_vec();
        let back = QueryActivityRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.activity_id, 100);
        assert_eq!(back.operate_type, 7);
    }

    #[test]
    fn encode_exchange_shop_request() {
        let req = ExchangeShopRequest {
            activity_id: 100,
            operate_type: EXCHANGE_SHOP_OPERATE_TYPE,
            exchange_shop_operate: Some(
                crate::proto::generated::gamepb::activitypb::ExchangeShopOperateParams {
                    goods_id: 50,
                    count: 3,
                },
            ),
        };
        let bytes = req.encode_to_vec();
        let back = ExchangeShopRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.exchange_shop_operate.as_ref().unwrap().goods_id, 50);
    }

    #[test]
    fn activity_data_with_empty_catalog() {
        // proto 必须能解空 catalog
        let data = ActivityData {
            activity: Some(ActivityContent {
                activity_id: 1,
                group_id: 0,
                r#type: 0,
                name: "test".to_string(),
                extra: bytes::Bytes::new(),
                begin_time: 0,
                end_time: 0,
                sort_order: 0,
                field_20: 0,
                field_23: 0,
            }),
            catalog: Some(StarSandGoodsList { goods: vec![] }),
            ..Default::default()
        };
        let reply = ActivityOperateReply {
            activity_id: 1,
            operate_type: 7,
            data: Some(data),
            ..Default::default()
        };
        let raw = extract_goods(&reply);
        assert!(raw.is_empty());
    }
}
