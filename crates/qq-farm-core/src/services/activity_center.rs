//! 活动中心 — 4 个生效活动的 RPC + 业务编排。
//!
//! 1:1 翻译原 `core/src/services/activity-center.ts` 的有效部分（原 1034 行，
//! 按用户决策只复刻生效的活动，不做 1:1 全搬）。被跳过的：
//!
//! - 复杂 `constellation-*.json` catalog 静态数据（运行时按需从 `reply.constellation` 拿）
//! - 256 行 `activity-center-state.ts` JSON 状态合并（defer 到 runtime engine）
//! - `serializeMutation` 复杂并发（defer，rate limiter 已在 1F-6 覆盖）
//!
//! ## 4 个生效活动
//!
//! 1. **赛季 (Season)** — `GetSeasonInfo` / `ClaimBattlePassRewards`
//! 2. **活动商店 (Star Sand Shop)** — `QueryActivity(operate=7)` / `Operate(exchange=1)`
//! 3. **星座 (Constellation)** — `QueryActivity(operate=7)` / `Operate(light=21)`
//! 4. **节气 (Solar Terms)** — `GetSolarTerms` / `ClaimSolarTerms`
//!
//! ## 协议
//!
//! - `gamepb.seasonpb.SeasonService.GetSeasonInfo` / `ClaimBattlePassRewards`
//! - `gamepb.activitypb.ActivityService.Operate` — 通用活动入口（Query / Exchange / Light）
//! - `gamepb.solartermspb.SolarTermsService.GetSolarTerms` / `ClaimSolarTerms`

use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;
use serde::Serialize;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityContent, ActivityData, ActivityItem, ActivityOperateReply, ConstellationData,
    ExchangeShopRequest, OperateConstellationRequest, QueryActivityRequest, StarSandGoods,
    StarSandGoodsList,
};
use crate::proto::generated::gamepb::seasonpb::{
    ClaimBattlePassRewardsReply, ClaimBattlePassRewardsRequest, GetSeasonInfoReply,
    GetSeasonInfoRequest, SeasonActivity, SeasonInfo, SeasonPass,
};
use crate::proto::generated::gamepb::solartermspb::{
    ClaimSolarTermsReply, ClaimSolarTermsRequest, GetSolarTermsReply, GetSolarTermsRequest,
    SolarTermInfo, SolarTermsConfig,
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
pub struct SeasonActivityDto {
    pub id: i64,
    pub r#type: i64,
    pub name: String,
    pub begin_time: i64,
    pub end_time: i64,
}

/// 赛季战斗通行证 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct SeasonPassDto {
    pub activity_id: i64,
    pub current_level: i64,
    pub current_progress: i64,
    pub progress_target: i64,
    pub node_count: i64,
    pub claimed_through_level: i64,
}

/// 赛季 DTO
#[derive(Debug, Clone, Serialize, Default)]
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
pub struct SolarTermDto {
    pub id: i64,
    pub status: i64,
    pub begin_time: i64,
    pub end_time: i64,
    pub name: String,
}

/// 节气 DTO（含 rules）
#[derive(Debug, Clone, Serialize, Default)]
pub struct SolarTermsConfigDto {
    pub id: i64,
    pub activity_id: i64,
    pub rules_text: String,
}

/// 节气回复 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct SolarTermsDto {
    pub server_time: i64,
    pub current_term_id: Option<i64>,
    pub terms: Vec<SolarTermDto>,
    pub current_config: Option<SolarTermsConfigDto>,
    pub configs: Vec<SolarTermsConfigDto>,
}

/// 商品 DTO
#[derive(Debug, Clone, Serialize, Default)]
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
    pub max_exchange_count: i64,
    pub max_exchange_count_known: bool,
    pub quality_code: i64,
}

/// 简化物品 DTO（用于商品内嵌的 item / cost）
#[derive(Debug, Clone, Serialize, Default)]
pub struct ItemDto {
    pub id: i64,
    pub count: i64,
    pub name: String,
}

/// 活动商店 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct StarSandShopDto {
    pub activity_id: i64,
    pub name: String,
    pub start_time: i64,
    pub end_time: i64,
    pub server_time: i64,
    pub balance_known: bool,
    pub currencies: Vec<ItemDto>,
    pub categories: Vec<String>,
    pub goods: Vec<StarSandGoodsDto>,
    pub affordable_count: i32,
    pub exchangeable_count: i32,
}

/// 星座活动 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct ConstellationDto {
    pub activity_id: i64,
    pub server_time: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub current_day: i32,
    pub field_1: i64,
    pub field_2: i64,
    pub field_3: i64,
    pub node_count: usize,
    pub group_count: usize,
}

/// 兑换结果 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct ExchangeResultDto {
    pub purchase_count: i64,
    pub total_item_count: i64,
    pub total_cost: i64,
    pub rewards: Vec<ItemDto>,
    pub shop: StarSandShopDto,
    pub message: String,
}

/// 星座点亮结果 DTO
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome")]
pub enum LightConstellationResult {
    /// 成功点亮
    Lighted {
        rewards: Vec<ItemDto>,
        constellation: Option<ConstellationDto>,
    },
    /// 今日已无可领取
    NothingToClaim { message: String },
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
}

impl ActivityCenterService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            mutation_lock: Arc::new(AsyncMutex::new(())),
            cached_season: Mutex::new(None),
        }
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
    pub async fn get_current_season_event(&self) -> Result<SeasonDto> {
        let reply = self.query_season().await?;
        normalize_season(&reply).ok_or_else(|| ActivityError {
            code: ActivityErrorCode::SeasonDataEmpty,
            message: "当前赛季数据为空".to_string(),
        }.into())
    }

    /// 领取战斗通行证奖励
    pub async fn claim_battle_pass_rewards(&self) -> Result<ClaimBattlePassRewardsReply> {
        let body = self
            .gateway
            .request(
                SEASON_SERVICE,
                "ClaimBattlePassRewards",
                &ClaimBattlePassRewardsRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(ClaimBattlePassRewardsReply::decode(&body[..])?)
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
    pub async fn claim_solar_term(&self, term_id: &str) -> Result<ClaimSolarTermsReply> {
        let parsed = positive_decimal(term_id, ActivityErrorCode::InvalidSolarTermId, "termId")?;
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
        Ok(ClaimSolarTermsReply::decode(&body[..])?)
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
        let shop_activity = find_season_activity(&season_reply, SHOP_ACTIVITY_TYPE)
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
            &season_reply,
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

        let unit_item_count = raw_goods.item.as_ref().map(|i| i.count).unwrap_or(0);
        let total_item_count = if unit_item_count > 0 {
            unit_item_count * count
        } else {
            0
        };
        let received = if raw_goods.item.as_ref().map(|i| i.id).unwrap_or(0) > 0
            && total_item_count > 0
        {
            vec![ItemDto {
                id: raw_goods.item.as_ref().map(|i| i.id).unwrap_or(0),
                count: total_item_count,
                name: raw_goods
                    .item
                    .as_ref()
                    .map(|i| i.name.clone())
                    .unwrap_or_default(),
            }]
        } else {
            vec![]
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

        let cost_name = raw_goods
            .cost
            .as_ref()
            .map(|c| c.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "星砂".to_string());
        let message = format!("兑换成功，共消耗 {} {}", total_cost, cost_name);

        Ok(ExchangeResultDto {
            purchase_count: count,
            total_item_count,
            total_cost,
            rewards: received,
            shop,
            message,
        })
    }

    // ----- 星座 -----

    /// 点亮星座（一次性操作）
    pub async fn light_constellation(&self) -> Result<LightConstellationResult> {
        let _guard = self.mutation_lock.lock().await;
        let season_reply = self.query_season().await?;
        let activity = find_season_activity(&season_reply, CONSTELLATION_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ConstellationActivityMissing,
                message: "服务端未发现星座活动".to_string(),
            })?;
        let req = OperateConstellationRequest {
            activity_id: activity.activity_id,
            operate_type: LIGHT_CONSTELLATION_OPERATE_TYPE,
            field_119: Some(
                crate::proto::generated::gamepb::activitypb::operate_constellation_request::Empty {},
            ),
        };
        let body = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000)
            .await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != activity.activity_id {
            return Err(Error::Protocol("星座操作返回了不匹配的活动 ID".to_string()));
        }
        if reply.operate_type != LIGHT_CONSTELLATION_OPERATE_TYPE {
            return Err(Error::Protocol(format!(
                "星座操作返回了未知操作类型: {}",
                reply.operate_type
            )));
        }
        if reply.data.is_none() {
            return Err(Error::Protocol("星座操作成功但回包缺少数据".to_string()));
        }

        let server_time = season_reply
            .season_info
            .as_ref()
            .map(|s| s.server_time)
            .unwrap_or(0);
        let current_day = constellation_day_from_beijing_midnight(
            activity.begin_time,
            server_time,
        )
        .unwrap_or(0);
        let constellation_dto = ConstellationDto {
            activity_id: activity.activity_id,
            server_time,
            start_time: activity.begin_time,
            end_time: activity.end_time,
            current_day,
            field_1: reply.data.as_ref().map(|d| d.constellation.as_ref().map(|c| c.field_1).unwrap_or(0)).unwrap_or(0),
            field_2: reply.data.as_ref().map(|d| d.constellation.as_ref().map(|c| c.field_2).unwrap_or(0)).unwrap_or(0),
            field_3: reply.data.as_ref().map(|d| d.constellation.as_ref().map(|c| c.field_3).unwrap_or(0)).unwrap_or(0),
            node_count: reply.data.as_ref().map(|d| d.constellation.as_ref().map(|c| c.nodes.len()).unwrap_or(0)).unwrap_or(0),
            group_count: reply.data.as_ref().map(|d| d.constellation.as_ref().map(|c| c.groups.len()).unwrap_or(0)).unwrap_or(0),
        };

        Ok(LightConstellationResult::Lighted {
            rewards: vec![],
            constellation: Some(constellation_dto),
        })
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
    ItemDto {
        id: item.id,
        count: item.count,
        name: String::new(),
    }
}

/// 把 `activitypb::ActivityItem` 序列化为简化的 DTO
pub fn activity_item_dto(item: &ActivityItem) -> ItemDto {
    ItemDto {
        id: item.item_id,
        count: item.count,
        name: String::new(),
    }
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
        end_time: a.end_time,
    }
}

/// 把 `SeasonPass` 转为 DTO
#[must_use]
pub fn pass_dto(p: &SeasonPass) -> SeasonPassDto {
    SeasonPassDto {
        activity_id: p.activity_id,
        current_level: p.current_level,
        current_progress: p.current_progress,
        progress_target: p.progress_target,
        node_count: p.node_count,
        claimed_through_level: p.claimed_through_level,
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
    SolarTermDto {
        id: t.term_id,
        status: t.status,
        begin_time: t.begin_time,
        end_time: t.end_time,
        name: bytes_to_text(&t.name),
    }
}

fn solar_terms_config_dto(c: &SolarTermsConfig) -> SolarTermsConfigDto {
    SolarTermsConfigDto {
        id: c.config_id,
        activity_id: c.activity_id,
        rules_text: bytes_to_text(&c.rules_json),
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
        max_exchange_count: max_count,
        max_exchange_count_known: balance_known,
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
            .map(|id| ItemDto {
                id,
                count: *balances.get(&id).unwrap_or(&0),
                name: String::new(),
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

    StarSandShopDto {
        activity_id,
        name,
        start_time: shop_activity.begin_time,
        end_time: shop_activity.end_time,
        server_time,
        balance_known,
        currencies,
        categories,
        goods,
        affordable_count,
        exchangeable_count,
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

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::corepb::Item as CoreItem;
    use crate::proto::generated::gamepb::activitypb::{ActivityContent, ActivityItem};

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
            ..Default::default()
        };
        let dto = pass_dto(&p);
        assert_eq!(dto.activity_id, 10);
        assert_eq!(dto.current_level, 5);
        assert_eq!(dto.claimed_through_level, 3);
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
        let r = LightConstellationResult::Lighted {
            rewards: vec![],
            constellation: None,
        };
        match r {
            LightConstellationResult::Lighted { .. } => {}
            _ => panic!("expected Lighted"),
        }
    }

    #[test]
    fn light_constellation_result_nothing() {
        let r = LightConstellationResult::NothingToClaim {
            message: "已领".to_string(),
        };
        match r {
            LightConstellationResult::NothingToClaim { message } => {
                assert_eq!(message, "已领");
            }
            _ => panic!("expected NothingToClaim"),
        }
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
        assert_eq!(dto.current_day, 0);
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
            constellation: None,
        };
        let reply = ActivityOperateReply {
            activity_id: 1,
            operate_type: 7,
            data: Some(data),
        };
        let raw = extract_goods(&reply);
        assert!(raw.is_empty());
    }
}
