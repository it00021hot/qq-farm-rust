//! 商业层 — 商城 + 神秘商店的业务编排与 DTO 转换。
//!
//! 1:1 翻译原 `core/src/services/commerce.ts`（274 行，2026-09-24 SVIP 商城版）。
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

use crate::services::mall::{MallFertilizerKind, MallService};
use crate::services::mystery_shop::MysteryShopService;
use crate::services::warehouse::WarehouseService;

// =====================================================================
// DTO
// =====================================================================

/// 物品 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
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
///
/// bot `limitDto`：`limit_type === 0` 视为不限购，整体返回 `null`（见 [`limit_dto`]）；
/// 存在时 `remaining` 恒为数字（`max - bought` 截断到 0）。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseLimitDto {
    #[serde(rename = "type")]
    pub kind: i64,
    pub bought: i64,
    pub max: i64,
    pub remaining: i64,
}

/// SVIP 会员身份（bot `getMallCatalog` 注入的 membership）
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MembershipDto {
    pub is_svip: bool,
    pub remaining_days: i64,
}

/// 商城商品 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
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
    /// 1=common / 2=pet / 3=adornment（协议 field 12，与可用性无关）
    pub product_type: i64,
    /// 购买状态机：owned / sold_out / ad_required / share_required / unavailable /
    /// available / svip_required（bot `mallAvailability` + 非会员覆盖）
    pub purchase_status: String,
    pub unavailable_reason: String,
    /// 促销生效时的原价（划线价）；无促销为 `None`
    pub original_price: Option<i64>,
    pub discount_text: String,
    pub is_discounted: bool,
    pub discount_end_time: i64,
    pub available: bool,
    pub purchasable: bool,
}

/// 商城目录 DTO
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MallCatalogDto {
    pub slot_type: i32,
    pub sub_slot_type: i32,
    /// 仅 SVIP 分页（slot 4）注入
    pub membership: Option<MembershipDto>,
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
    /// 目录回读失败时为 `None`，此时 `refresh_required = true`
    pub catalog: Option<MallCatalogDto>,
    pub refresh_required: bool,
}

/// 神秘商店 NPC DTO（UI 读 camelCase：originalPrice / unitPrice 等）
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MysteryNpcDto {
    pub id: i64,
    pub reward: ItemDto,
    /// 每单数量（游戏界面商品图旁的 x8）。历史版本即显示该值；
    /// ActiveNPC.unknown_field_3 疑似真实剩余库存，未经游戏端确认，暂不上屏。
    pub stock: i64,
    pub price: ItemDto,
    pub original_price: i64,
    pub unit_price: i64,
    pub unit_original_price: i64,
    pub discount_percent: i64,
}

/// 神秘商店状态 DTO（UI 读 camelCase：activeTime / expireTime）
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
    InvalidMallSlot,
    GoodsNotFound,
    GoodsSoldOut,
    GoodsUnavailable,
    MallPriceChanged,
    MallBalanceUnavailable,
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
            Self::InvalidMallSlot => "INVALID_MALL_SLOT",
            Self::GoodsNotFound => "GOODS_NOT_FOUND",
            Self::GoodsSoldOut => "GOODS_SOLD_OUT",
            Self::GoodsUnavailable => "GOODS_UNAVAILABLE",
            Self::MallPriceChanged => "MALL_PRICE_CHANGED",
            Self::MallBalanceUnavailable => "MALL_BALANCE_UNAVAILABLE",
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
    /// SVIP 分页（slot 4）目录需要先刷新会员信息（bot 里懒加载 `require('./qqvip')`）
    qqvip: Arc<crate::services::qqvip::QQVipService>,

    /// 购买串行化队列
    purchase_lock: Arc<AsyncMutex<()>>,
}

impl CommerceService {
    #[must_use]
    pub fn new(
        mall: Arc<MallService>,
        mystery_shop: Arc<MysteryShopService>,
        warehouse: Arc<WarehouseService>,
        qqvip: Arc<crate::services::qqvip::QQVipService>,
    ) -> Self {
        Self { mall, mystery_shop, warehouse, qqvip, purchase_lock: Arc::new(AsyncMutex::new(())) }
    }

    // ----- 商城 -----

    /// 获取商城目录（含货币余额 + 商品 DTO）
    ///
    /// 对齐 bot `getMallCatalog`：slot 4（SVIP）先 `refreshVipInfo` +
    /// `getQQVipRewardsStatus` 注入 `membership`，非会员可浏览但禁购
    /// （`purchaseStatus = 'svip_required'`）。
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
        // bot：仅 SVIP 分页注入会员身份；subSlotType 只在显式为 1 时作为 is_manual_open。
        // 实机确认：非 QQ 会员调 RefreshVipInfo / GetQQVipRewardsStatus 会回
        // code=1021001 —— 此处按「非会员」降级（目录照常返回，商品全部标
        // svip_required 禁购），不整体报错。
        let membership = if slot_type == 4 {
            let not_vip = |e: &crate::error::Error| {
                crate::services::qqvip::is_not_qq_vip_error(&e.to_string())
            };
            if let Err(e) = self.qqvip.refresh_vip_info().await {
                if !not_vip(&e) {
                    return Err(e);
                }
                Some(MembershipDto { is_svip: false, remaining_days: 0 })
            } else {
                match self.qqvip.get_qq_vip_rewards_status().await {
                    Ok(status) => Some(MembershipDto {
                        is_svip: status.is_qq_vip,
                        remaining_days: status.remaining_days.max(0),
                    }),
                    Err(e) if not_vip(&e) => {
                        Some(MembershipDto { is_svip: false, remaining_days: 0 })
                    }
                    Err(e) => return Err(e),
                }
            }
        } else {
            None
        };
        let reply = self.mall.get_mall_list_by_slot_type(slot_type, sub_slot_type).await?;
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
                let mut dto = item_dto(&CoreItem { id: *id, ..Default::default() });
                dto.balance = balance;
                dto.balance_known = balance.is_some();
                dto
            })
            .collect();
        let now_secs = get_server_time_secs();
        let non_svip = membership.as_ref().is_some_and(|m| !m.is_svip);
        let goods: Vec<MallGoodsDto> = raw_goods
            .iter()
            .map(|g| {
                let mut product = mall_goods_dto(g, &balances, slot_type, now_secs);
                if non_svip {
                    product.available = false;
                    product.purchasable = false;
                    product.purchase_status = "svip_required".to_string();
                    product.unavailable_reason = "需要 SVIP 会员身份".to_string();
                }
                product
            })
            .collect();
        Ok(MallCatalogDto {
            slot_type,
            sub_slot_type,
            membership,
            server_time: now_secs * 1000,
            refresh_countdown: reply.refresh_countdown,
            currencies,
            goods,
        })
    }

    /// 购买商城商品（含前置校验）
    ///
    /// 1:1 对齐 bot `purchaseMallProduct`：仅支持 slot 1 / 4；sold_out 与
    /// 不可购买分开报错；`expected_price` 不符报 `MALL_PRICE_CHANGED`；
    /// 余额未确认报 `MALL_BALANCE_UNAVAILABLE`；目录回读失败降级 `refresh_required`。
    ///
    /// # Errors
    /// - [`CommerceError`]：参数非法 / 商品不存在 / 售罄 / 不可购买 / 价格变化 /
    ///   余额未确认 / 超限购 / 余额不足
    /// - 底层 RPC 错误
    pub async fn purchase_mall_product(
        &self,
        goods_id_input: &str,
        count_input: &str,
        slot_type_input: Option<i32>,
        expected_price: Option<ExpectedPrice>,
    ) -> Result<PurchaseResponseDto> {
        let goods_id =
            positive_integer(goods_id_input, CommerceErrorCode::InvalidGoodsId, "goodsId")?;
        let count =
            positive_integer(count_input, CommerceErrorCode::InvalidPurchaseCount, "count")?;
        if count > 9999 {
            return Err(CommerceError {
                code: CommerceErrorCode::InvalidPurchaseCount,
                message: "count exceeds 9999".to_string(),
            }
            .into());
        }

        let _guard = self.purchase_lock.lock().await;

        let slot_type = match slot_type_input.unwrap_or(1) {
            1 | 4 => slot_type_input.unwrap_or(1),
            _ => {
                return Err(CommerceError {
                    code: CommerceErrorCode::InvalidMallSlot,
                    message: "不支持的商城分页".to_string(),
                }
                .into());
            }
        };
        let before = self.get_mall_catalog(Some(slot_type), Some(0)).await?;
        let target =
            before.goods.iter().find(|g| g.id == goods_id).ok_or_else(|| CommerceError {
                code: CommerceErrorCode::GoodsNotFound,
                message: "Mall goods not found".to_string(),
            })?;
        if target.purchase_status == "sold_out" {
            return Err(CommerceError {
                code: CommerceErrorCode::GoodsSoldOut,
                message: target.unavailable_reason.clone(),
            }
            .into());
        }
        if !target.purchasable {
            return Err(CommerceError {
                code: CommerceErrorCode::GoodsUnavailable,
                message: "Mall goods is unavailable".to_string(),
            }
            .into());
        }
        if let Some(expected) = &expected_price {
            if expected.id != target.price.id || expected.count != target.price.count {
                return Err(CommerceError {
                    code: CommerceErrorCode::MallPriceChanged,
                    message: "商品价格已变化，请刷新商城后重新确认".to_string(),
                }
                .into());
            }
        }
        if !target.is_free
            && (target.price.id <= 0 || target.price.count <= 0 || target.price.balance.is_none())
        {
            return Err(CommerceError {
                code: CommerceErrorCode::MallBalanceUnavailable,
                message: "商品价格或余额未确认，请刷新后重试".to_string(),
            }
            .into());
        }
        if let Some(limit) = &target.limit {
            if limit.remaining < count {
                return Err(CommerceError {
                    code: CommerceErrorCode::PurchaseLimitExceeded,
                    message: "Purchase count exceeds the remaining limit".to_string(),
                }
                .into());
            }
        }
        if !target.is_free {
            if let Some(balance) = target.price.balance {
                if balance < target.price.count * count {
                    return Err(CommerceError {
                        code: CommerceErrorCode::InsufficientBalance,
                        message: "Insufficient currency balance".to_string(),
                    }
                    .into());
                }
            }
        }

        let reply = self.mall.purchase_mall_goods(goods_id, i64::from(count)).await?;
        let purchase = PurchaseResultDto {
            goods_id: reply.goods_id,
            count,
            rewards: reply.reward_items.iter().map(item_dto).collect(),
            limit: reply.purchase_limit.as_ref().and_then(limit_dto),
        };
        // bot：回读目录失败降级为 refreshRequired，不整体报错
        let catalog = match self.get_mall_catalog(Some(slot_type), Some(0)).await {
            Ok(c) => Some(c),
            Err(_) => None,
        };
        let refresh_required = catalog.is_none();
        Ok(PurchaseResponseDto { purchase, catalog, refresh_required })
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
            &CoreItem { id: npc.reward_item_id, count: reward_count, ..Default::default() },
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
                // 显示每单数量（= 游戏界面商品旁的 xN），与历史版本一致
                stock: reward_count,
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
    pub async fn purchase_mystery_offer(
        &self,
        npc_id_input: &str,
    ) -> Result<MysteryPurchaseResponseDto> {
        let npc_id =
            positive_integer(npc_id_input, CommerceErrorCode::InvalidMysteryNpcId, "npcId")?;
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
        if offer.id != npc_id {
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

        self.mystery_shop.buy(npc_id).await?;
        let shop = self.get_mystery_shop().await?;
        if shop.active
            && shop.npc.as_ref().is_some_and(|n| n.id == npc_id)
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
        let items = crate::services::warehouse::get_bag_items(&bag);
        let (normal, organic) =
            crate::services::warehouse::get_container_hours_from_bag_items(&items);
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
                return FertilizerBothResult { error: Some(e.to_string()), ..Default::default() };
            }
        };
        let items = crate::services::warehouse::get_bag_items(&bag);
        let (normal, organic) =
            crate::services::warehouse::get_container_hours_from_bag_items(&items);
        let mut result = FertilizerBothResult {
            organic_current_hours: organic as f64,
            normal_current_hours: normal as f64,
            ..Default::default()
        };

        if options.buy_organic && options.organic_count > 0 && options.organic_threshold_hours > 0.0
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

        if options.buy_organic && options.buy_normal && result.organic_bought > 0 {
            // 1000-2000ms 随机延迟，避免双类型购买被风控关联
            let delay_ms = 1000 + (rand::random::<u64>() % 1000);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        if options.buy_normal && options.normal_count > 0 && options.normal_threshold_hours > 0.0 {
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
                for item in crate::services::warehouse::get_bag_items(&reply) {
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
    let metadata: Option<CatalogItem> =
        if id > 0 { global_game_config().get_item_by_id(id) } else { None };
    let name = if !fallback_name.is_empty() {
        metadata.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| fallback_name.to_string())
    } else if id > 0 {
        metadata.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| format!("物品 #{}", id))
    } else {
        "未知物品".to_string()
    };
    let image = if id > 0 {
        global_game_config().get_item_image_by_id(id).unwrap_or_default()
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

/// 把 `PurchaseLimit` 转换为 DTO（1:1 对齐 bot `limitDto`）
///
/// `limit_type === 0` 视为不限购返回 `None`；存在时 `remaining` 恒为数字。
#[must_use]
pub fn limit_dto(
    limit: &crate::proto::generated::gamepb::mallpb::PurchaseLimit,
) -> Option<PurchaseLimitDto> {
    if limit.limit_type == 0 {
        return None;
    }
    let bought = limit.bought_count.max(0);
    let max = limit.limit_count.max(0);
    Some(PurchaseLimitDto {
        kind: i64::from(limit.limit_type).max(0),
        bought,
        max,
        remaining: (max - bought).max(0),
    })
}

/// 购买状态 + 原因（bot `mallAvailability`）
#[must_use]
pub fn mall_availability(
    goods: &MallGoods,
    limit: Option<&PurchaseLimitDto>,
    slot_type: i32,
) -> (&'static str, &'static str) {
    if goods.is_owned {
        return ("owned", "已拥有该商品");
    }
    if limit.is_some_and(|l| l.remaining == 0) {
        return (
            "sold_out",
            if goods.is_free {
                "奖励已领取"
            } else {
                "商品已售罄，已达到限购上限"
            },
        );
    }
    if goods.ad_only {
        return ("ad_required", "请在游戏内观看广告领取");
    }
    if goods.share.as_ref().is_some_and(|s| s.share_only && s.share_status != 2) {
        return ("share_required", "请在游戏内完成分享条件");
    }
    // 官方在正常可不限购的商品上省略 field 8；普通商城（slot 1）无 limit 时
    // 按「限制存在与否」区分不限购（bot commerce.ts:88-93 注释）。
    // SVIP 分页仍要求自身可用标记。
    if !goods.is_available && !(slot_type == 1 && limit.is_none()) {
        return ("unavailable", "商品当前不可购买");
    }
    ("available", "")
}

/// 购买确认用的期望价格（前端从目录带入，防服务端涨价）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub struct ExpectedPrice {
    pub id: i64,
    pub count: i64,
}

/// 把 `MallGoods` 转换为 DTO（1:1 对齐 bot `mallGoodsDto`）
#[must_use]
pub fn mall_goods_dto(
    goods: &MallGoods,
    balances: &HashMap<i64, i64>,
    slot_type: i32,
    now_secs: i64,
) -> MallGoodsDto {
    let mut price = item_dto(goods.price.as_ref().unwrap_or(&CoreItem::default()));
    let original_price_count = price.count;
    let discount_price = goods.discount_price.max(0);
    let promotion_start = goods.promotion_start_time;
    let promotion_end = goods.promotion_end_time;
    // 促销窗口按服务器时间判定（bot commerce.ts:104）
    let promotion_active =
        discount_price > 0 && promotion_start <= now_secs && promotion_end > now_secs;
    if promotion_active {
        price.count = discount_price;
    }
    let limit = goods.purchase_limit.as_ref().and_then(limit_dto);
    let is_free = goods.is_free && price.count == 0;
    let (status, reason) = mall_availability(goods, limit.as_ref(), slot_type);
    let balance = if price.id > 0 { balances.get(&price.id).copied() } else { None };
    price.balance = balance;
    MallGoodsDto {
        id: goods.goods_id,
        name: goods.name.clone(),
        kind: i64::from(goods.goods_type),
        rewards: goods.reward_items.iter().map(item_dto).collect(),
        is_free,
        limit: limit.clone(),
        is_limited: limit.is_some(),
        product_type: i64::from(goods.product_type),
        purchase_status: status.to_string(),
        unavailable_reason: reason.to_string(),
        original_price: promotion_active.then_some(original_price_count),
        // 促销未生效时不展示折扣文案（bot commerce.ts:125）
        discount_text: if discount_price > 0 && !promotion_active {
            String::new()
        } else {
            goods.discount_text.clone()
        },
        is_discounted: if discount_price > 0 { promotion_active } else { goods.is_discounted },
        discount_end_time: if discount_price > 0 { promotion_end } else { goods.discount_end_time }
            .max(0)
            * 1000,
        price,
        available: status == "available",
        purchasable: status == "available",
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
///
/// 对齐 bot `positiveInteger`：`/^[1-9]\d*$/` 且不超过 JS 安全整数（2^53-1）。
pub fn positive_integer(
    value: &str,
    code: CommerceErrorCode,
    label: &str,
) -> std::result::Result<i64, CommerceError> {
    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
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
        Err(_) => return Err(CommerceError { code, message: format!("{} is too large", label) }),
    };
    if n < 1 {
        return Err(CommerceError {
            code,
            message: format!("{} must be a positive integer", label),
        });
    }
    if n > MAX_SAFE_INTEGER {
        return Err(CommerceError { code, message: format!("{} is too large", label) });
    }
    Ok(n)
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
    fn positive_integer_accepts_large_ids() {
        assert_eq!(
            positive_integer(
                &i64::from(i32::MAX).to_string(),
                CommerceErrorCode::InvalidGoodsId,
                "goodsId"
            )
            .unwrap(),
            i64::from(i32::MAX)
        );
        // 超过 JS 安全整数报 too large
        assert!(positive_integer("9007199254740992", CommerceErrorCode::InvalidGoodsId, "goodsId")
            .is_err());
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
        assert_eq!(CommerceErrorCode::InvalidMallSlot.as_str(), "INVALID_MALL_SLOT");
        assert_eq!(CommerceErrorCode::GoodsNotFound.as_str(), "GOODS_NOT_FOUND");
        assert_eq!(CommerceErrorCode::GoodsSoldOut.as_str(), "GOODS_SOLD_OUT");
        assert_eq!(CommerceErrorCode::GoodsUnavailable.as_str(), "GOODS_UNAVAILABLE");
        assert_eq!(CommerceErrorCode::MallPriceChanged.as_str(), "MALL_PRICE_CHANGED");
        assert_eq!(CommerceErrorCode::MallBalanceUnavailable.as_str(), "MALL_BALANCE_UNAVAILABLE");
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
        let item = CoreItem { id: 0, count: 5, ..Default::default() };
        let dto = item_dto(&item);
        assert_eq!(dto.id, 0);
        assert_eq!(dto.count, 5);
        // 兜底名
        assert!(!dto.name.is_empty() || dto.name == "未知物品");
    }

    #[test]
    fn item_dto_negative_count_clamps_to_zero() {
        let item = CoreItem { id: 1, count: -10, ..Default::default() };
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
        let l = PurchaseLimit { limit_type: 1, bought_count: 3, limit_count: 10 };
        let dto = limit_dto(&l).expect("limit_type=1 应有限制");
        assert_eq!(dto.kind, 1);
        assert_eq!(dto.bought, 3);
        assert_eq!(dto.max, 10);
        assert_eq!(dto.remaining, 7);
    }

    #[test]
    fn limit_dto_unlimited_returns_none() {
        use crate::proto::generated::gamepb::mallpb::PurchaseLimit;
        // bot：limit_type === 0 视为不限购，整体返回 null
        let l = PurchaseLimit { limit_type: 0, bought_count: 0, limit_count: 0 };
        assert!(limit_dto(&l).is_none());
    }

    #[test]
    fn limit_dto_remaining_clamped_to_zero() {
        use crate::proto::generated::gamepb::mallpb::PurchaseLimit;
        let l = PurchaseLimit { limit_type: 1, bought_count: 10, limit_count: 10 };
        let dto = limit_dto(&l).expect("limit_type=1 应有限制");
        assert_eq!(dto.remaining, 0);
    }

    #[test]
    fn availability_slot1_no_limit_means_available() {
        // 普通商城（slot 1）无 limit 且 field 8 缺省（false）→ 官方视为可购买
        let goods = MallGoods { goods_id: 1002, ..Default::default() };
        let (status, _) = mall_availability(&goods, None, 1);
        assert_eq!(status, "available");
        // 同样条件在 SVIP 分页（slot 4）→ 不可购买
        let (status, _) = mall_availability(&goods, None, 4);
        assert_eq!(status, "unavailable");
    }

    #[test]
    fn availability_state_machine() {
        use crate::proto::generated::gamepb::mallpb::MallShareInfo;
        let base = MallGoods { goods_id: 1002, is_available: true, ..Default::default() };
        // owned 优先级最高
        let (s, r) = mall_availability(&MallGoods { is_owned: true, ..base.clone() }, None, 1);
        assert_eq!(s, "owned");
        assert_eq!(r, "已拥有该商品");
        // 售罄：limit.remaining == 0；免费品文案不同
        let limit_exhausted = PurchaseLimitDto { kind: 1, bought: 1, max: 1, remaining: 0 };
        let (s, r) = mall_availability(
            &MallGoods { is_free: true, ..base.clone() },
            Some(&limit_exhausted),
            1,
        );
        assert_eq!(s, "sold_out");
        assert_eq!(r, "奖励已领取");
        let (s, r) = mall_availability(&base.clone(), Some(&limit_exhausted), 1);
        assert_eq!(s, "sold_out");
        assert_eq!(r, "商品已售罄，已达到限购上限");
        // 广告领取
        let (s, r) = mall_availability(&MallGoods { ad_only: true, ..base.clone() }, None, 1);
        assert_eq!(s, "ad_required");
        assert_eq!(r, "请在游戏内观看广告领取");
        // 需分享（share_status != 2）
        let (s, _) = mall_availability(
            &MallGoods {
                share: Some(MallShareInfo {
                    share_only: true,
                    share_status: 1,
                    ..Default::default()
                }),
                ..base.clone()
            },
            None,
            1,
        );
        assert_eq!(s, "share_required");
        // 分享已完成（share_status == 2）→ 不再拦截
        let (s, _) = mall_availability(
            &MallGoods {
                share: Some(MallShareInfo {
                    share_only: true,
                    share_status: 2,
                    ..Default::default()
                }),
                ..base.clone()
            },
            None,
            1,
        );
        assert_eq!(s, "available");
    }

    #[test]
    fn promotion_price_substitution() {
        let now = 1_790_179_300; // 窗口内
        let mut goods = MallGoods {
            goods_id: 1060,
            is_available: true,
            discount_price: 780,
            promotion_start_time: 1_790_179_200,
            promotion_end_time: 1_790_783_999,
            price: Some(CoreItem { id: 1002, count: 880, ..Default::default() }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances, 1, now);
        assert_eq!(dto.price.count, 780, "促销期内按折扣价展示");
        assert_eq!(dto.original_price, Some(880), "划线价");
        assert!(dto.is_discounted);
        // 窗口外：原价 + 不展示折扣文案
        goods.promotion_end_time = now - 1;
        let dto = mall_goods_dto(&goods, &balances, 1, now);
        assert_eq!(dto.price.count, 880);
        assert_eq!(dto.original_price, None);
        assert!(!dto.is_discounted);
    }

    #[test]
    fn mall_goods_dto_free() {
        let goods = MallGoods {
            goods_id: 1,
            is_free: true,
            is_available: true,
            price: Some(CoreItem { id: 0, count: 0, ..Default::default() }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances, 1, 0);
        assert!(dto.is_free);
        assert!(dto.available);
        assert!(dto.purchasable);
        assert_eq!(dto.purchase_status, "available");
    }

    #[test]
    fn mall_goods_dto_paid_no_balance_known() {
        let goods = MallGoods {
            goods_id: 1002,
            is_free: false,
            is_available: true,
            price: Some(CoreItem { id: 1002, count: 2500, ..Default::default() }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances, 1, 0);
        assert!(!dto.is_free);
        assert!(dto.purchasable);
        assert_eq!(dto.price.id, 1002);
        assert_eq!(dto.price.count, 2500);
        assert_eq!(dto.price.balance, None);
    }

    #[test]
    fn mall_goods_dto_unavailable() {
        use crate::proto::generated::gamepb::mallpb::PurchaseLimit;
        // field 8 为 false 且有限制 → 不可购买（无限制的 slot 1 商品视为可购）
        let goods = MallGoods {
            goods_id: 1002,
            is_available: false,
            purchase_limit: Some(PurchaseLimit { limit_type: 1, bought_count: 0, limit_count: 5 }),
            ..Default::default()
        };
        let balances = HashMap::new();
        let dto = mall_goods_dto(&goods, &balances, 1, 0);
        assert!(!dto.purchasable);
        assert_eq!(dto.purchase_status, "unavailable");
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
        let dto = mall_goods_dto(&goods, &balances, 1, 0);
        assert!(!dto.purchasable);
        assert_eq!(dto.purchase_status, "sold_out");
    }

    #[test]
    fn fertilizer_both_options_default() {
        let opts = FertilizerBothOptions::default();
        assert!(!opts.buy_organic);
        assert!(!opts.buy_normal);
    }
}
