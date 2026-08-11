//! 商业层 — 商城 + 神秘商店的业务编排与 DTO 转换。
//!
//! 1:1 翻译原 `core/src/services/commerce.ts`（213 行）。
//!
//! ## 职责
//!
//! - 把 mall / mystery-shop 的底层 RPC 包装成业务 API（含参数校验、库存检查、余额校验）
//! - 把后端 protobuf 消息转换成前端友好的 DTO（`serde::Serialize`）
//! - 串行化所有购买动作，避免并发购买时序问题（`serialize_purchase`）
//! - 化肥容器阈值检查：拉背包 → 算小时数 → 不足则调 mall 自动购买
//!
//! ## 与原 TS 的差异
//!
//! - `purchaseTail` 从 Promise 链改为 `tokio::sync::Mutex` + 排队任务
//! - `currencyBalances` 改为 best-effort：拉背包失败返回空 map（不阻断主流程）
//! - `boundedInteger` / `positiveInteger` 校验在调用方入口做

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex as AsyncMutex;

use crate::config::game_config::{global as global_game_config, Item as CatalogItem};
use crate::error::{Error, Result};
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::mallpb::MallGoods;
use crate::proto::generated::gamepb::mysteryshoppb::GetActiveNpcReply;
use crate::utils::time::get_server_time_secs;

use super::mall::{MallFertilizerKind, MallService};
use super::mystery_shop::MysteryShopService;
use super::warehouse::WarehouseService;

// =====================================================================
// DTO
// =====================================================================

/// 物品 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct ItemDto {
    pub id: i64,
    pub count: i64,
    pub name: String,
    pub image: String,
    pub rarity: i64,
    /// 货币余额（仅在 currency DTO 中设置；`None` 表示未知）
    pub balance: Option<i64>,
    /// 余额是否真实从背包查到（仅 currency DTO 关心）
    pub balance_known: bool,
}

/// 购买限制 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct PurchaseLimitDto {
    #[serde(rename = "type")]
    pub kind: i64,
    pub bought: i64,
    pub max: i64,
    /// 剩余可购买次数；`None` 表示无限
    pub remaining: Option<i64>,
}

/// 商城商品 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct MallGoodsDto {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: i64,
    pub rewards: Vec<ItemDto>,
    pub price: ItemDto,
    pub is_free: bool,
    pub limit: Option<PurchaseLimitDto>,
    pub is_limited: bool,
    pub discount_text: String,
    pub is_discounted: bool,
    pub discount_end_time: i64,
    pub available: bool,
    pub purchasable: bool,
}

/// 商城目录 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct MallCatalogDto {
    pub slot_type: i32,
    pub sub_slot_type: i32,
    pub server_time: i64,
    pub refresh_countdown: i64,
    pub currencies: Vec<ItemDto>,
    pub goods: Vec<MallGoodsDto>,
}

/// 购买结果 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct PurchaseResultDto {
    pub goods_id: i64,
    pub count: i64,
    pub rewards: Vec<ItemDto>,
    pub limit: Option<PurchaseLimitDto>,
}

/// 购买响应（含结果 + 刷新后目录）
#[derive(Debug, Clone, Serialize, Default)]
pub struct PurchaseResponseDto {
    pub purchase: PurchaseResultDto,
    pub catalog: MallCatalogDto,
}

/// 神秘商店 NPC DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct MysteryNpcDto {
    pub id: i64,
    pub reward: ItemDto,
    pub stock: i64,
    pub price: ItemDto,
    pub original_price: i64,
    pub unit_price: i64,
    pub unit_original_price: i64,
    pub discount_percent: i64,
}

/// 神秘商店状态 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct MysteryShopDto {
    pub active: bool,
    pub server_time: i64,
    pub active_time: i64,
    pub expire_time: i64,
    pub npc: Option<MysteryNpcDto>,
}

/// 神秘商店购买结果 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct MysteryPurchaseDto {
    pub npc_id: i64,
    pub reward: ItemDto,
    pub price: ItemDto,
    pub original_price: i64,
    pub discount_percent: i64,
}

/// 化肥阈值检查单类型结果
#[derive(Debug, Clone, Serialize, Default)]
pub struct FertilizerThresholdResult {
    pub bought: i32,
    pub current_hours: f64,
    pub threshold_hours: f64,
    pub needed: bool,
    pub error: Option<String>,
}

/// 化肥阈值检查双类型结果
#[derive(Debug, Clone, Serialize, Default)]
pub struct FertilizerBothResult {
    pub organic_bought: i32,
    pub normal_bought: i32,
    pub organic_current_hours: f64,
    pub normal_current_hours: f64,
    pub error: Option<String>,
}

/// 化肥双类型检查选项
#[derive(Debug, Clone, Default)]
pub struct FertilizerBothOptions {
    pub buy_organic: bool,
    pub buy_normal: bool,
    pub organic_count: i32,
    pub organic_threshold_hours: f64,
    pub normal_count: i32,
    pub normal_threshold_hours: f64,
}

// =====================================================================
// 业务错误码
// =====================================================================

/// 业务层错误码（1:1 对齐原 TS `businessError` 的 code）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommerceErrorCode {
    InvalidGoodsId,
    InvalidPurchaseCount,
    GoodsNotFound,
    GoodsUnavailable,
    PurchaseLimitExceeded,
    InsufficientBalance,
    InvalidMysteryNpcId,
    MysteryOfferStale,
    MysteryOfferSoldOut,
    MysteryPurchaseNotConfirmed,
}

impl CommerceErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidGoodsId => "INVALID_GOODS_ID",
            Self::InvalidPurchaseCount => "INVALID_PURCHASE_COUNT",
            Self::GoodsNotFound => "GOODS_NOT_FOUND",
            Self::GoodsUnavailable => "GOODS_UNAVAILABLE",
            Self::PurchaseLimitExceeded => "PURCHASE_LIMIT_EXCEEDED",
            Self::InsufficientBalance => "INSUFFICIENT_BALANCE",
            Self::InvalidMysteryNpcId => "INVALID_MYSTERY_NPC_ID",
            Self::MysteryOfferStale => "MYSTERY_OFFER_STALE",
            Self::MysteryOfferSoldOut => "MYSTERY_OFFER_SOLD_OUT",
            Self::MysteryPurchaseNotConfirmed => "MYSTERY_PURCHASE_NOT_CONFIRMED",
        }
    }
}

/// 业务错误
#[derive(Debug, Clone)]
pub struct CommerceError {
    pub code: CommerceErrorCode,
    pub message: String,
}

impl std::fmt::Display for CommerceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for CommerceError {}

impl From<CommerceError> for Error {
    fn from(e: CommerceError) -> Self {
        Self::Business(e.to_string())
    }
}

// =====================================================================
// CommerceService
// =====================================================================

/// 商业编排服务
pub struct CommerceService {
    mall: Arc<MallService>,
    mystery_shop: Arc<MysteryShopService>,
    warehouse: Arc<WarehouseService>,

    /// 购买串行化队列
    purchase_lock: Arc<AsyncMutex<()>>,
}

impl CommerceService {
    #[must_use]
    pub fn new(
        mall: Arc<MallService>,
        mystery_shop: Arc<MysteryShopService>,
        warehouse: Arc<WarehouseService>,
    ) -> Self {
        Self {
            mall,
            mystery_shop,
            warehouse,
            purchase_lock: Arc::new(AsyncMutex::new(())),
        }
    }

    // ----- 商城 -----

    /// 获取商城目录（含货币余额 + 商品 DTO）
    ///
    /// # Errors
    /// - 拉取商城列表 / 背包失败
    pub async fn get_mall_catalog(
        &self,
        slot_type_input: Option<i32>,
        sub_slot_type_input: Option<i32>,
    ) -> Result<MallCatalogDto> {
        let slot_type = bounded_integer(slot_type_input, 1, 1, 100);
        let sub_slot_type = bounded_integer(sub_slot_type_input, 0, 0, 100);
        let reply = self
            .mall
            .get_mall_list_by_slot_type(slot_type, sub_slot_type)
            .await?;
        let raw_goods: Vec<MallGoods> = reply.goods_list;
        let currency_ids: Vec<i64> = raw_goods
            .iter()
            .filter_map(|g| g.price.as_ref().map(|i| i.id))
            .filter(|id| *id > 0)
            .collect();
        let balances = self.currency_balances(&currency_ids).await;
        let unique_currency_ids: Vec<i64> = {
            let mut set: HashSet<i64> = HashSet::new();
            for id in &currency_ids {
                set.insert(*id);
            }
            set.into_iter().collect()
        };
        let currencies: Vec<ItemDto> = unique_currency_ids
            .iter()
            .map(|id| {
                let balance = balances.get(id).copied();
                let mut dto = item_dto(&CoreItem {
                    id: *id,
                    ..Default::default()
                });
                dto.balance = balance;
                dto.balance_known = balance.is_some();
                dto
            })
            .collect();
        let goods: Vec<MallGoodsDto> = raw_goods
            .iter()
            .map(|g| mall_goods_dto(g, &balances))
            .collect();
        Ok(MallCatalogDto {
            slot_type,
            sub_slot_type,
            server_time: get_server_time_secs() * 1000,
            refresh_countdown: reply.refresh_countdown,
            currencies,
            goods,
        })
    }

    /// 购买商城商品（含前置校验）
    ///
    /// # Errors
    /// - [`CommerceError`]：参数非法 / 商品不存在 / 库存不足 / 余额不足
    /// - 底层 RPC 错误
    pub async fn purchase_mall_product(
        &self,
        goods_id_input: &str,
        count_input: &str,
    ) -> Result<PurchaseResponseDto> {
        let goods_id = positive_integer(goods_id_input, CommerceErrorCode::InvalidGoodsId, "goodsId")?;
        let count = positive_integer(count_input, CommerceErrorCode::InvalidPurchaseCount, "count")?;
        if count > 9999 {
            return Err(CommerceError {
                code: CommerceErrorCode::InvalidPurchaseCount,
                message: "count exceeds 9999".to_string(),
            }
            .into());
        }

        let _guard = self.purchase_lock.lock().await;

        let before = self.get_mall_catalog(Some(1), Some(0)).await?;
        let target = before
            .goods
            .iter()
            .find(|g| g.id == i64::from(goods_id))
            .ok_or_else(|| CommerceError {
                code: CommerceErrorCode::GoodsNotFound,
                message: "Mall goods not found".to_string(),
            })?;
        if !target.purchasable {
            return Err(CommerceError {
                code: CommerceErrorCode::GoodsUnavailable,
                message: "Mall goods is unavailable".to_string(),
            }
            .into());
        }
        if let Some(limit) = &target.limit {
            if let Some(remaining) = limit.remaining {
                if remaining < i64::from(count) {
                    return Err(CommerceError {
                        code: CommerceErrorCode::PurchaseLimitExceeded,
                        message: "Purchase count exceeds the remaining limit".to_string(),
                    }
                    .into());
                }
            }
        }
        if !target.is_free {
            if let (Some(balance), price_count) = (target.price.balance, target.price.count) {
                if balance < price_count * i64::from(count) {
                    return Err(CommerceError {
                        code: CommerceErrorCode::InsufficientBalance,
                        message: "Insufficient currency balance".to_string(),
                    }
                    .into());
                }
            }
        }

        let reply = self.mall.purchase_mall_goods(goods_id, count).await?;
        let purchase = PurchaseResultDto {
            goods_id: i64::from(reply.goods_id),
            count: i64::from(reply.count),
            rewards: reply.reward_items.iter().map(item_dto).collect(),
            limit: reply.purchase_limit.as_ref().map(limit_dto),
        };
        let catalog = self.get_mall_catalog(Some(1), Some(0)).await?;
        Ok(PurchaseResponseDto { purchase, catalog })
    }

    // ----- 神秘商店 -----

    /// 获取神秘商店状态
    ///
    /// # Errors
    /// - 拉取 NPC 信息失败
    pub async fn get_mystery_shop(&self) -> Result<MysteryShopDto> {
        let reply: GetActiveNpcReply = self.mystery_shop.get_active_npc().await?;
        let server_time = get_server_time_secs() * 1000;
        let npc = reply.npc;
        if !reply.is_active || npc.is_none() {
            return Ok(MysteryShopDto {
                active: false,
                server_time,
                active_time: reply.active_time * 1000,
                expire_time: reply.expire_time * 1000,
                npc: None,
            });
        }
        let npc = npc.unwrap();
        let currency_id = npc.currency_item_id;
        let reward_count = i64::from(npc.reward_count);
        let unit_price = npc.price;
        let unit_original_price = npc.original_price;
        let balances = self.currency_balances(&[currency_id]).await;
        let balance = balances.get(&currency_id).copied();
        let mut price_dto = item_dto(&CoreItem {
            id: currency_id,
            count: unit_price * reward_count,
            ..Default::default()
        });
        price_dto.balance = balance;
        price_dto.balance_known = balance.is_some();

        let reward = item_dto_with_fallback(
            &CoreItem {
                id: npc.reward_item_id,
                count: reward_count,
                ..Default::default()
            },
            "神秘商品",
        );

        Ok(MysteryShopDto {
            active: true,
            server_time,
            active_time: reply.active_time * 1000,
            expire_time: reply.expire_time * 1000,
            npc: Some(MysteryNpcDto {
                id: npc.npc_id,
                reward,
                stock: i64::from(npc.stock_count),
                price: price_dto,
                original_price: unit_original_price * reward_count,
                unit_price,
                unit_original_price,
                discount_percent: i64::from(npc.discount_percent),
            }),
        })
    }

    /// 购买神秘商店商品（含前置校验 + 二次确认）
    ///
    /// # Errors
    /// - [`CommerceError`]：参数非法 / 商品已下架 / 库存为 0 / 余额不足
    /// - 二次确认失败（购买后库存未减）
    /// - 底层 RPC 错误
    pub async fn purchase_mystery_offer(&self, npc_id_input: &str) -> Result<MysteryPurchaseResponseDto> {
        let npc_id = positive_integer(npc_id_input, CommerceErrorCode::InvalidMysteryNpcId, "npcId")?;
        let _guard = self.purchase_lock.lock().await;

        let before = self.get_mystery_shop().await?;
        if !before.active {
            return Err(CommerceError {
                code: CommerceErrorCode::MysteryOfferStale,
                message: "Mystery shop offer is no longer available".to_string(),
            }
            .into());
        }
        let offer = before.npc.as_ref().ok_or_else(|| CommerceError {
            code: CommerceErrorCode::MysteryOfferStale,
            message: "Mystery shop offer is no longer available".to_string(),
        })?;
        if offer.id != i64::from(npc_id) {
            return Err(CommerceError {
                code: CommerceErrorCode::MysteryOfferStale,
                message: "Mystery shop offer is no longer available".to_string(),
            }
            .into());
        }
        if offer.stock <= 0 {
            return Err(CommerceError {
                code: CommerceErrorCode::MysteryOfferSoldOut,
                message: "Mystery shop offer is sold out".to_string(),
            }
            .into());
        }
        if let Some(balance) = offer.price.balance {
            if balance < offer.price.count {
                return Err(CommerceError {
                    code: CommerceErrorCode::InsufficientBalance,
                    message: "Insufficient currency balance".to_string(),
                }
                .into());
            }
        }

        self.mystery_shop.buy(npc_id as i64).await?;
        let shop = self.get_mystery_shop().await?;
        if shop.active
            && shop.npc.as_ref().is_some_and(|n| n.id == i64::from(npc_id))
            && shop.npc.as_ref().is_some_and(|n| n.stock >= offer.stock)
        {
            return Err(CommerceError {
                code: CommerceErrorCode::MysteryPurchaseNotConfirmed,
                message: "Mystery shop purchase was not confirmed".to_string(),
            }
            .into());
        }

        Ok(MysteryPurchaseResponseDto {
            purchase: MysteryPurchaseDto {
                npc_id: offer.id,
                reward: offer.reward.clone(),
                price: offer.price.clone(),
                original_price: offer.original_price,
                discount_percent: offer.discount_percent,
            },
            shop,
        })
    }

    // ----- 化肥阈值检查（跨服务编排） -----

    /// 单种化肥阈值检查
    pub async fn check_and_buy_fertilizer_by_threshold(
        &self,
        kind: MallFertilizerKind,
        count: i32,
        threshold_hours: f64,
    ) -> FertilizerThresholdResult {
        if count <= 0 || threshold_hours <= 0.0 {
            return FertilizerThresholdResult::default();
        }
        let bag = match self.warehouse.get_bag().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("[商城] 检测化肥容器失败: {}", e);
                return FertilizerThresholdResult {
                    error: Some(e.to_string()),
                    ..Default::default()
                };
            }
        };
        let items = super::warehouse::get_bag_items(&bag);
        let (normal, organic) = super::warehouse::get_container_hours_from_bag_items(&items);
        let current_hours = match kind {
            MallFertilizerKind::Normal => normal as f64,
            MallFertilizerKind::Organic => organic as f64,
        };
        tracing::info!(
            "[商城] 检测{}容器: 剩余 {:.1} 小时，阈值 {} 小时",
            kind.type_name(),
            current_hours,
            threshold_hours
        );
        if current_hours < threshold_hours {
            let bought = self.mall.auto_buy_fertilizer(true, kind, count).await;
            return FertilizerThresholdResult {
                bought,
                current_hours,
                threshold_hours,
                needed: true,
                error: None,
            };
        }
        FertilizerThresholdResult {
            bought: 0,
            current_hours,
            threshold_hours,
            needed: false,
            error: None,
        }
    }

    /// 双类型化肥阈值检查
    pub async fn check_and_buy_fertilizer_both(
        &self,
        options: FertilizerBothOptions,
    ) -> FertilizerBothResult {
        if !options.buy_organic && !options.buy_normal {
            return FertilizerBothResult::default();
        }
        let bag = match self.warehouse.get_bag().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("[商城] 检测化肥容器失败: {}", e);
                return FertilizerBothResult {
                    error: Some(e.to_string()),
                    ..Default::default()
                };
            }
        };
        let items = super::warehouse::get_bag_items(&bag);
        let (normal, organic) = super::warehouse::get_container_hours_from_bag_items(&items);
        let mut result = FertilizerBothResult {
            organic_current_hours: organic as f64,
            normal_current_hours: normal as f64,
            ..Default::default()
        };

        if options.buy_organic
            && options.organic_count > 0
            && options.organic_threshold_hours > 0.0
        {
            tracing::info!(
                "[商城] 检测有机化肥容器: 剩余 {:.1} 小时，阈值 {} 小时",
                result.organic_current_hours,
                options.organic_threshold_hours
            );
            if result.organic_current_hours < options.organic_threshold_hours {
                result.organic_bought = self
                    .mall
                    .auto_buy_fertilizer(true, MallFertilizerKind::Organic, options.organic_count)
                    .await;
            }
        }

        if options.buy_organic
            && options.buy_normal
            && result.organic_bought > 0
        {
            // 1000-2000ms 随机延迟，避免双类型购买被风控关联
            let delay_ms = 1000 + (rand::random::<u64>() % 1000);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        if options.buy_normal
            && options.normal_count > 0
            && options.normal_threshold_hours > 0.0
        {
            tracing::info!(
                "[商城] 检测无机化肥容器: 剩余 {:.1} 小时，阈值 {} 小时",
                result.normal_current_hours,
                options.normal_threshold_hours
            );
            if result.normal_current_hours < options.normal_threshold_hours {
                result.normal_bought = self
                    .mall
                    .auto_buy_fertilizer(true, MallFertilizerKind::Normal, options.normal_count)
                    .await;
            }
        }

        result
    }

    /// 读取指定货币 ID 的余额（best-effort）
    async fn currency_balances(&self, ids: &[i64]) -> HashMap<i64, i64> {
        let wanted: HashSet<i64> = ids.iter().copied().filter(|id| *id > 0).collect();
        let mut balances: HashMap<i64, i64> = HashMap::new();
        if wanted.is_empty() {
            return balances;
        }
        match self.warehouse.get_bag().await {
            Ok(reply) => {
                for item in super::warehouse::get_bag_items(&reply) {
                    if wanted.contains(&item.id) {
                        balances.insert(item.id, item.count.max(0));
                    }
                }
            }
            Err(_) => {
                // 拉背包失败：返回空 map（catalog 数据仍可用）
            }
        }
        balances
    }
}

/// 神秘商店购买响应
#[derive(Debug, Clone, Serialize, Default)]
pub struct MysteryPurchaseResponseDto {
    pub purchase: MysteryPurchaseDto,
    pub shop: MysteryShopDto,
}

// =====================================================================
// 纯函数（DTO 转换）
// =====================================================================

/// 把 `corepb.Item` 转换为前端 DTO
pub fn item_dto(item: &CoreItem) -> ItemDto {
    item_dto_with_fallback(item, "")
}

/// 把 `corepb.Item` 转换为前端 DTO（带 fallback 名称）
pub fn item_dto_with_fallback(item: &CoreItem, fallback_name: &str) -> ItemDto {
    let id = item.id;
    let metadata: Option<CatalogItem> = if id > 0 {
        global_game_config().get_item_by_id(id)
    } else {
        None
    };
    let name = if !fallback_name.is_empty() {
        metadata
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| fallback_name.to_string())
    } else if id > 0 {
        metadata
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| format!("物品 #{}", id))
    } else {
        "未知物品".to_string()
    };
    let image = if id > 0 {
        global_game_config()
            .get_item_image_by_id(id)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let rarity = metadata.and_then(|m| m.rarity).unwrap_or(0);

    ItemDto {
        id,
        count: item.count.max(0),
        name,
        image,
        rarity,
        balance: None,
        balance_known: false,
    }
}

/// 把 `PurchaseLimit` 转换为 DTO
pub fn limit_dto(limit: &crate::proto::generated::gamepb::mallpb::PurchaseLimit) -> PurchaseLimitDto {
    let bought = limit.bought_count as i64;
    let max = limit.limit_count as i64;
    let remaining = if max > 0 {
        Some((max - bought).max(0))
    } else {
        None
    };
    PurchaseLimitDto {
        kind: limit.limit_type as i64,
        bought,
        max,
        remaining,
    }
}

/// 把 `MallGoods` 转换为 DTO
pub fn mall_goods_dto(goods: &MallGoods, balances: &HashMap<i64, i64>) -> MallGoodsDto {
    let price = item_dto(goods.price.as_ref().unwrap_or(&CoreItem::default()));
    let limit = goods.purchase_limit.as_ref().map(limit_dto);
    let is_free = goods.is_free || price.id == 0 || price.count == 0;
    let available = goods.is_available;
    let balance = if price.id > 0 {
        balances.get(&price.id).copied()
    } else {
        None
    };
    let mut price_dto = price;
    price_dto.balance = balance;
    let purchasable = available
        && limit
            .as_ref()
            .map_or(true, |l| l.remaining.is_none_or(|r| r > 0));
    MallGoodsDto {
        id: goods.goods_id as i64,
        name: goods.name.clone(),
        kind: goods.goods_type as i64,
        rewards: goods.reward_items.iter().map(item_dto).collect(),
        price: price_dto,
        is_free,
        limit,
        is_limited: goods.is_limited,
        discount_text: goods.discount_text.clone(),
        is_discounted: goods.is_discounted,
        discount_end_time: goods.discount_end_time * 1000,
        available,
        purchasable,
    }
}

/// 把任意输入限定到 [min, max] 整数范围，非法值回退 fallback
pub fn bounded_integer<T>(value: Option<T>, fallback: i32, min: i32, max: i32) -> i32
where
    T: TryInto<i32>,
{
    let v: Option<i32> = value.and_then(|x| x.try_into().ok());
    if let Some(n) = v {
        if n >= min && n <= max {
            return n;
        }
    }
    fallback
}

/// 把字符串解析为正整数，非法则返回业务错误
pub fn positive_integer(
    value: &str,
    code: CommerceErrorCode,
    label: &str,
) -> std::result::Result<i32, CommerceError> {
    let text = value.trim();
    if text.is_empty() {
        return Err(CommerceError {
            code,
            message: format!("{} must be a positive integer", label),
        });
    }
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return Err(CommerceError {
            code,
            message: format!("{} must be a positive integer", label),
        });
    }
    let n: i64 = match text.parse() {
        Ok(n) => n,
        Err(_) => {
            return Err(CommerceError {
                code,
                message: format!("{} is too large", label),
            })
        }
    };
    if n < 1 {
        return Err(CommerceError {
            code,
            message: format!("{} must be a positive integer", label),
        });
    }
    if n > i32::MAX as i64 {
        return Err(CommerceError {
            code,
            message: format!("{} is too large", label),
        });
    }
    Ok(n as i32)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_integer_clamps() {
        assert_eq!(bounded_integer::<i32>(Some(50), 1, 1, 100), 50);
        assert_eq!(bounded_integer::<i32>(Some(0), 1, 1, 100), 1); // out of range -> fallback
        assert_eq!(bounded_integer::<i32>(Some(101), 1, 1, 100), 1);
        assert_eq!(bounded_integer::<i32>(None, 7, 1, 100), 7);
    }

    #[test]
    fn positive_integer_valid() {
        assert_eq!(
            positive_integer("123", CommerceErrorCode::InvalidGoodsId, "goodsId").unwrap(),
            123
        );
    }

    #[test]
    fn positive_integer_rejects_zero() {
        assert!(positive_integer("0", CommerceErrorCode::InvalidGoodsId, "goodsId").is_err());
    }

    #[test]
    fn positive_integer_rejects_negative() {
        assert!(positive_integer("-1", CommerceErrorCode::InvalidGoodsId, "goodsId").is_err());
    }

    #[test]
    fn positive_integer_rejects_empty() {
        assert!(positive_integer("", CommerceErrorCode::InvalidGoodsId, "goodsId").is_err());
        assert!(positive_integer("   ", CommerceErrorCode::InvalidGoodsId, "goodsId").is_err());
    }

    #[test]
    fn positive_integer_rejects_non_digit() {
        assert!(positive_integer("12a", CommerceErrorCode::InvalidGoodsId, "goodsId").is_err());
    }

    #[test]
    fn positive_integer_rejects_overflow() {
        let huge = "99999999999999999999";
        assert!(positive_integer(huge, CommerceErrorCode::InvalidGoodsId, "goodsId").is_err());
    }

    #[test]
    fn positive_integer_accepts_max_i32() {
        assert_eq!(
            positive_integer(&i32::MAX.to_string(), CommerceErrorCode::InvalidGoodsId, "goodsId").unwrap(),
            i32::MAX
        );
    }

    #[test]
    fn commerce_error_display_includes_code() {
        let e = CommerceError {
            code: CommerceErrorCode::GoodsNotFound,
            message: "Mall goods not found".to_string(),
        };
        let s = e.to_string();
        assert!(s.contains("GOODS_NOT_FOUND"));
        assert!(s.contains("Mall goods not found"));
    }

    #[test]
    fn commerce_error_into_core_error() {
        let e = CommerceError {
            code: CommerceErrorCode::InsufficientBalance,
            message: "no money".to_string(),
        };
        let core: Error = e.into();
        let s = core.to_string();
        assert!(s.contains("business error"));
        assert!(s.contains("INSUFFICIENT_BALANCE"));
    }

    #[test]
    fn error_codes_match_ts() {
        assert_eq!(CommerceErrorCode::InvalidGoodsId.as_str(), "INVALID_GOODS_ID");
        assert_eq!(CommerceErrorCode::InvalidPurchaseCount.as_str(), "INVALID_PURCHASE_COUNT");
        assert_eq!(CommerceErrorCode::GoodsNotFound.as_str(), "GOODS_NOT_FOUND");
        assert_eq!(CommerceErrorCode::GoodsUnavailable.as_str(), "GOODS_UNAVAILABLE");
        assert_eq!(CommerceErrorCode::PurchaseLimitExceeded.as_str(), "PURCHASE_LIMIT_EXCEEDED");
        assert_eq!(CommerceErrorCode::InsufficientBalance.as_str(), "INSUFFICIENT_BALANCE");
        assert_eq!(CommerceErrorCode::InvalidMysteryNpcId.as_str(), "INVALID_MYSTERY_NPC_ID");
        assert_eq!(CommerceErrorCode::MysteryOfferStale.as_str(), "MYSTERY_OFFER_STALE");
        assert_eq!(CommerceErrorCode::MysteryOfferSoldOut.as_str(), "MYSTERY_OFFER_SOLD_OUT");
        assert_eq!(
            CommerceErrorCode::MysteryPurchaseNotConfirmed.as_str(),
            "MYSTERY_PURCHASE_NOT_CONFIRMED"
        );
    }

    #[test]
    fn item_dto_unknown_id() {
        let item = CoreItem {
            id: 0,
            count: 5,
            ..Default::default()
        };
        let dto = item_dto(&item);
        assert_eq!(dto.id, 0);
        assert_eq!(dto.count, 5);
        // 兜底名
        assert!(!dto.name.is_empty() || dto.name == "未知物品");
    }

    #[test]
    fn item_dto_negative_count_clamps_to_zero() {
        let item = CoreItem {
            id: 1,
            count: -10,
            ..Default::default()
        };
        let dto = item_dto(&item);
        assert_eq!(dto.count, 0);
    }

    #[test]
    fn item_dto_with_fallback_uses_fallback_when_no_metadata() {
        let item = CoreItem {
            id: 99999999, // unlikely to be in gameConfig
            count: 1,
            ..Default::default()
        };
        let dto = item_dto_with_fallback(&item, "我的神秘物品");
        // 没有 metadata，name 会用 fallback
        assert_eq!(dto.name, "我的神秘物品");
    }

    #[test]
    fn limit_dto_with_max() {
        use crate::proto::generated::gamepb::mallpb::PurchaseLimit;
        let l = PurchaseLimit {
            limit_type: 1,
            bought_count: 3,
            limit_count: 10,
        };
        let dto = limit_dto(&l);
        assert_eq!(dto.kind, 1);
        assert_eq!(dto.bought, 3);
        assert_eq!(dto.max, 10);
        assert_eq!(dto.remaining, Some(7));
    }

    #[test]
    fn limit_dto_with_zero_max_returns_none_remaining() {
        use crate::proto::generated::gamepb::mallpb::PurchaseLimit;
        let l = PurchaseLimit {
            limit_type: 1,
            bought_count: 0,
            limit_count: 0,
        };
        let dto = limit_dto(&l);
        assert_eq!(dto.remaining, None);
    }

    #[test]
    fn mall_goods_dto_free() {
        use crate::proto::generated::corepb::Item as CoreItem;
        let goods = MallGoods {
            goods_id: 1,
            is_free: true,
            is_available: true,
            price: Some(CoreItem {
                id: 0,
                count: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances);
        assert!(dto.is_free);
        assert!(dto.available);
        assert!(dto.purchasable);
    }

    #[test]
    fn mall_goods_dto_paid_no_balance_known() {
        use crate::proto::generated::corepb::Item as CoreItem;
        let goods = MallGoods {
            goods_id: 1002,
            is_free: false,
            is_available: true,
            price: Some(CoreItem {
                id: 1002,
                count: 2500,
                ..Default::default()
            }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances);
        assert!(!dto.is_free);
        assert!(dto.purchasable);
        assert_eq!(dto.price.id, 1002);
        assert_eq!(dto.price.count, 2500);
        assert_eq!(dto.price.balance, None);
    }

    #[test]
    fn mall_goods_dto_unavailable() {
        let goods = MallGoods {
            goods_id: 1002,
            is_available: false,
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances);
        assert!(!dto.purchasable);
    }

    #[test]
    fn mall_goods_dto_purchase_limit_exhausted() {
        use crate::proto::generated::gamepb::mallpb::PurchaseLimit;
        let goods = MallGoods {
            goods_id: 1002,
            is_available: true,
            purchase_limit: Some(PurchaseLimit {
                limit_type: 1,
                bought_count: 10,
                limit_count: 10,
            }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances);
        assert!(!dto.purchasable);
    }

    #[test]
    fn fertilizer_both_options_default() {
        let opts = FertilizerBothOptions::default();
        assert!(!opts.buy_organic);
        assert!(!opts.buy_normal);
    }
}
