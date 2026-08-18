//! 商城 — 自动购买化肥 / 免费礼包 / 容器阈值检查。
//!
//! 1:1 翻译原 `core/src/services/mall.ts`（510 行）。
//!
//! ## 协议
//!
//! - `gamepb.mallpb.MallService.GetMallListBySlotType` — 拉取商城商品列表
//! - `gamepb.mallpb.MallService.Purchase` — 购买商品
//!
//! ## 业务
//!
//! - 自动购买化肥（有机 / 无机）支持按目标数量 + 单价 + 余额感知的批次数
//! - 免费礼包每日限领一次（`mall_free_gifts` key）
//! - 容器阈值检查：低于阈值时按配置补货
//! - 错误时识别"余额不足 / code=1000019"自动降批次数到 1
//!
//! ## 与 TS 的差异
//!
//! - 原 TS 通过 `getUserState().ticket` 读取点券余额来优化单批次数
//!   本实现不持有点券余额（属于 runtime engine 状态），按 `BUY_PER_ROUND` 默认值发起购买
//! - 简化点：当服务返回错误时直接 break，不再做"per-round > 1 降级到 1 后 continue"二次尝试

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use prost::Message;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::mallpb::{
    GetMallListBySlotTypeRequest, GetMallListBySlotTypeResponse, MallGoods, PurchaseRequest,
    PurchaseResponse,
};

const MALL_SERVICE: &str = "gamepb.mallpb.MallService";

/// 有机化肥商品 ID
pub const ORGANIC_FERTILIZER_MALL_GOODS_ID: i32 = 1002;
/// 无机化肥商品 ID
pub const INORGANIC_FERTILIZER_MALL_GOODS_ID: i32 = 1003;
/// 购买主流程冷却 10 分钟
pub const BUY_COOLDOWN_MS: i64 = 10 * 60 * 1000;
/// 免费礼包检查冷却 10 分钟
pub const CHECK_BUY_COOLDOWN_MS: i64 = 60 * 1000;
/// 单次最大轮次
pub const MAX_ROUNDS: usize = 100;
/// 单批购买数
pub const BUY_PER_ROUND: i32 = 10;
/// 免费礼包每日 key
pub const FREE_GIFTS_DAILY_KEY: &str = "mall_free_gifts";

/// 化肥类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MallFertilizerKind {
    Organic,
    Normal,
}

impl MallFertilizerKind {
    /// 从字符串解析类型（`"normal"` -> `Normal`，其他 -> `Organic`）
    /// 1:1 对齐原 TS 行为
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        if s == "normal" {
            Self::Normal
        } else {
            Self::Organic
        }
    }

    /// 序列化为字符串（`organic` / `normal`）
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Organic => "organic",
            Self::Normal => "normal",
        }
    }

    /// 中文显示名（用于日志）
    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Organic => "有机化肥",
            Self::Normal => "无机化肥",
        }
    }
}

/// 购买结果
#[derive(Debug, Clone, Default)]
pub struct BuyResult {
    pub bought: i32,
}

/// 商城每日状态
#[derive(Debug, Clone, Serialize)]
pub struct FertilizerBuyDailyState {
    pub key: &'static str,
    pub done_today: bool,
    pub paused_no_gold_today: bool,
    pub last_success_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FreeGiftDailyState {
    pub key: &'static str,
    pub done_today: bool,
    pub last_check_at: i64,
    pub last_claim_at: i64,
}

use serde::Serialize;

/// 商城服务
pub struct MallService {
    gateway: Arc<Gateway>,

    last_buy_at: Mutex<i64>,

    buy_done_date_key: Mutex<String>,
    buy_last_success_at: Mutex<i64>,
    buy_paused_no_gold_date_key: Mutex<String>,

    free_gift_done_date_key: Mutex<String>,
    free_gift_last_at: Mutex<i64>,
    free_gift_last_check_at: Mutex<i64>,
}

impl MallService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            last_buy_at: Mutex::new(0),
            buy_done_date_key: Mutex::new(String::new()),
            buy_last_success_at: Mutex::new(0),
            buy_paused_no_gold_date_key: Mutex::new(String::new()),
            free_gift_done_date_key: Mutex::new(String::new()),
            free_gift_last_at: Mutex::new(0),
            free_gift_last_check_at: Mutex::new(0),
        }
    }

    /// 拉取指定 slot 的商城商品列表
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_mall_list_by_slot_type(
        &self,
        slot_type: i32,
        sub_slot_type: i32,
    ) -> Result<GetMallListBySlotTypeResponse> {
        let req = GetMallListBySlotTypeRequest { slot_type, sub_slot_type };
        let body = self
            .gateway
            .request(MALL_SERVICE, "GetMallListBySlotType", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(GetMallListBySlotTypeResponse::decode(&body[..])?)
    }

    /// 拉取商品并解码每条 MallGoods
    ///
    /// # Errors
    /// - 同 [`Self::get_mall_list_by_slot_type`]
    pub async fn get_mall_goods_list(&self, slot_type: i32) -> Result<Vec<MallGoods>> {
        let reply = self.get_mall_list_by_slot_type(slot_type, 0).await?;
        Ok(reply.goods_list.into_iter().filter(|g| g.goods_id > 0).collect())
    }

    /// 购买商品
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    /// - 余额不足等业务错误（`code=1000019`）— 由调用方识别
    pub async fn purchase_mall_goods(&self, goods_id: i32, count: i32) -> Result<PurchaseResponse> {
        let req = PurchaseRequest { goods_id, count };
        let body =
            self.gateway.request(MALL_SERVICE, "Purchase", &req.encode_to_vec(), 10_000).await?;
        Ok(PurchaseResponse::decode(&body[..])?)
    }

    // ----- 化肥自动购买 -----

    /// 通过商城自动购买有机化肥（无目标数量限制）
    ///
    /// # Errors
    /// - 同 [`Self::purchase_mall_goods`]
    pub async fn auto_buy_organic_fertilizer_via_mall(&self) -> Result<i32> {
        self.auto_buy_fertilizer_via_mall(MallFertilizerKind::Organic, 0).await
    }

    /// 通过商城自动购买指定类型化肥
    ///
    /// `target_count` 为 0 表示不限数量，直到 MAX_ROUNDS 或余额不足为止
    ///
    /// # Errors
    /// - 同 [`Self::purchase_mall_goods`]
    pub async fn auto_buy_fertilizer_via_mall(
        &self,
        kind: MallFertilizerKind,
        target_count: i32,
    ) -> Result<i32> {
        let goods_list = self.get_mall_goods_list(1).await?;
        let goods = find_fertilizer_mall_goods(&goods_list, kind);
        let Some(goods) = goods else {
            return Ok(0);
        };
        let goods_id = goods.goods_id;
        if goods_id <= 0 {
            return Ok(0);
        }
        let single_price = parse_mall_price_value(goods.price.as_ref());
        let mut total_bought: i32 = 0;
        let mut per_round: i32 = BUY_PER_ROUND;
        let remaining_to_buy: i32 = if target_count > 0 { target_count } else { i32::MAX };

        for _ in 0..MAX_ROUNDS {
            if total_bought >= remaining_to_buy {
                break;
            }
            if single_price > 0 && per_round == 0 {
                *self.buy_paused_no_gold_date_key.lock() = get_date_key();
                break;
            }
            let buy_count = if target_count > 0 {
                per_round.min(remaining_to_buy - total_bought)
            } else {
                per_round
            };
            if buy_count <= 0 {
                break;
            }
            match self.purchase_mall_goods(goods_id, buy_count).await {
                Ok(_) => {
                    total_bought += buy_count;
                }
                Err(e) => {
                    let msg = e.to_string();
                    tracing::warn!("[商城] 购买化肥失败: {}", msg);
                    if is_insufficient_balance(&msg) {
                        if per_round > 1 {
                            per_round = 1;
                            continue;
                        }
                        *self.buy_paused_no_gold_date_key.lock() = get_date_key();
                    }
                    break;
                }
            }
            // 模拟 sleep(120) 抗频控
            tokio::time::sleep(Duration::from_millis(120)).await;
        }

        if total_bought > 0 {
            tracing::info!("[商城] 购买化肥成功，共购买 {} 个", total_bought);
        }

        Ok(total_bought)
    }

    /// 主入口：自动购买有机化肥（带冷却）
    pub async fn auto_buy_organic_fertilizer(&self, force: bool) -> i32 {
        if !self.acquire_buy_slot(force) {
            return 0;
        }
        match self.auto_buy_organic_fertilizer_via_mall().await {
            Ok(total) if total > 0 => {
                *self.buy_done_date_key.lock() = get_date_key();
                *self.buy_last_success_at.lock() = now_ms();
                tracing::info!("[商城] 自动购买有机化肥 x{}", total);
                total
            }
            _ => 0,
        }
    }

    /// 主入口：自动购买指定类型化肥（带冷却 + 目标数量）
    pub async fn auto_buy_fertilizer(
        &self,
        force: bool,
        kind: MallFertilizerKind,
        target_count: i32,
    ) -> i32 {
        if !self.acquire_buy_slot(force) {
            return 0;
        }
        match self.auto_buy_fertilizer_via_mall(kind, target_count).await {
            Ok(total) if total > 0 => {
                *self.buy_done_date_key.lock() = get_date_key();
                *self.buy_last_success_at.lock() = now_ms();
                let type_name = kind.type_name();
                tracing::info!("[商城] 自动购买{} x{}", type_name, total);
                total
            }
            _ => 0,
        }
    }

    fn acquire_buy_slot(&self, force: bool) -> bool {
        let now = now_ms();
        if !force && (now - *self.last_buy_at.lock()) < BUY_COOLDOWN_MS {
            return false;
        }
        *self.last_buy_at.lock() = now;
        true
    }

    // ----- 免费礼包 -----

    /// 每日领取免费礼包
    pub async fn buy_free_gifts(&self, force: bool) -> i32 {
        let now = now_ms();
        if !force && self.is_done_today_by_key(&self.free_gift_done_date_key) {
            return 0;
        }
        if !force && (now - *self.free_gift_last_check_at.lock()) < BUY_COOLDOWN_MS {
            return 0;
        }
        *self.free_gift_last_check_at.lock() = now;

        let goods_list = match self.get_mall_goods_list(1).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[商城] 领取免费礼包失败: {}", e);
                return 0;
            }
        };

        let free: Vec<&MallGoods> =
            goods_list.iter().filter(|g| g.is_free && g.goods_id > 0).collect();

        if free.is_empty() {
            *self.free_gift_done_date_key.lock() = get_date_key();
            tracing::info!("[商城] 今日暂无可领取免费礼包");
            return 0;
        }

        let mut bought: i32 = 0;
        for g in free {
            let id = g.goods_id;
            match self.purchase_mall_goods(id, 1).await {
                Ok(_) => bought += 1,
                Err(_) => {
                    // 单个失败跳过
                }
            }
        }
        *self.free_gift_done_date_key.lock() = get_date_key();
        if bought > 0 {
            *self.free_gift_last_at.lock() = now;
            tracing::info!("[商城] 自动购买免费礼包 x{}", bought);
        } else {
            tracing::info!("[商城] 本次未成功领取免费礼包");
        }
        bought
    }

    fn is_done_today_by_key(&self, key: &Mutex<String>) -> bool {
        *key.lock() == get_date_key()
    }

    // ----- 阈值检查 -----
    //
    // 注：阈值检查需要同时访问 warehouse（拉背包 / 解析小时数）和 mall（购买）。
    // 为避免 mall.rs 依赖 warehouse.rs 形成反向耦合，threshold 检查统一在
    // [`crate::services::commerce`] 中编排。

    // ----- 状态查询 -----

    #[must_use]
    pub fn get_fertilizer_buy_daily_state(&self) -> FertilizerBuyDailyState {
        FertilizerBuyDailyState {
            key: "fertilizer_buy",
            done_today: *self.buy_done_date_key.lock() == get_date_key(),
            paused_no_gold_today: *self.buy_paused_no_gold_date_key.lock() == get_date_key(),
            last_success_at: *self.buy_last_success_at.lock(),
        }
    }

    #[must_use]
    pub fn get_free_gift_daily_state(&self) -> FreeGiftDailyState {
        FreeGiftDailyState {
            key: FREE_GIFTS_DAILY_KEY,
            done_today: *self.free_gift_done_date_key.lock() == get_date_key(),
            last_check_at: *self.free_gift_last_check_at.lock(),
            last_claim_at: *self.free_gift_last_at.lock(),
        }
    }
}

// =====================================================================
// 纯函数
// =====================================================================

/// 解析价格字段为整数
///
/// 1:1 对齐原 TS `parseMallPriceValue`，但简化了"字节流"分支：
/// proto 中 `price` 即 `Option<corepb.Item>`，所以直接取 `count`。
///
/// 旧版 TS 同时支持 number / object / bytes（Any 序列化）三种输入，
/// 当前 MallGoods 走的是 object 分支。
pub fn parse_mall_price_value(price: Option<&CoreItem>) -> i32 {
    let Some(item) = price else {
        return 0;
    };
    let count = item.count;
    if count <= 0 {
        return 0;
    }
    count.min(i32::MAX as i64) as i32
}

/// 旧版 TS `parseMallPriceValue` 的 bytes 分支实现
///
/// 当 `MallGoods.price` 字段以 bytes（Any 序列化）形式存在时，扫描
/// field=2 的 varint 作为 `count`。当前 MallGoods proto 直接使用
/// `corepb.Item`，不进入此分支；保留以备兼容旧协议。
#[allow(dead_code)]
pub fn parse_mall_price_value_from_bytes(price_field_bytes: &[u8]) -> i32 {
    if price_field_bytes.is_empty() {
        return 0;
    }
    let mut idx: usize = 0;
    let mut parsed: i32 = 0;
    let len = price_field_bytes.len();
    while idx < len {
        let key_byte = price_field_bytes[idx];
        idx += 1;
        let field = (key_byte >> 3) & 0x0F;
        let wire = key_byte & 0x07;
        if wire != 0 {
            break;
        }
        let mut val: i64 = 0;
        let mut shift: u32 = 0;
        loop {
            if idx >= len {
                break;
            }
            let b = price_field_bytes[idx];
            idx += 1;
            val |= i64::from(b & 0x7F) << shift;
            if (b & 0x80) == 0 {
                break;
            }
            shift += 7;
            if shift > 63 {
                return 0;
            }
        }
        if field == 2 {
            parsed = val.clamp(0, i32::MAX as i64) as i32;
        }
    }
    parsed.max(0)
}

/// 从商品列表中按 kind 找化肥商品
pub fn find_fertilizer_mall_goods(
    goods_list: &[MallGoods],
    kind: MallFertilizerKind,
) -> Option<MallGoods> {
    let target = match kind {
        MallFertilizerKind::Organic => ORGANIC_FERTILIZER_MALL_GOODS_ID,
        MallFertilizerKind::Normal => INORGANIC_FERTILIZER_MALL_GOODS_ID,
    };
    goods_list.iter().find(|g| g.goods_id == target).cloned()
}

/// 判断错误信息是否表示"余额不足"
fn is_insufficient_balance(msg: &str) -> bool {
    msg.contains("余额不足") || msg.contains("点券不足") || msg.contains("code=1000019")
}

fn get_date_key() -> String {
    use chrono::Datelike;
    use chrono::Local;
    let now = Local::now();
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::corepb::Item;

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(MALL_SERVICE, "gamepb.mallpb.MallService");
    }

    #[test]
    fn fertilizer_kind_strings() {
        assert_eq!(MallFertilizerKind::Organic.as_str(), "organic");
        assert_eq!(MallFertilizerKind::Normal.as_str(), "normal");
        assert_eq!(MallFertilizerKind::from_str("organic"), MallFertilizerKind::Organic);
        assert_eq!(MallFertilizerKind::from_str("normal"), MallFertilizerKind::Normal);
        assert_eq!(MallFertilizerKind::from_str("other"), MallFertilizerKind::Organic);
    }

    #[test]
    fn parse_price_default_item_returns_zero() {
        let item = CoreItem::default();
        assert_eq!(parse_mall_price_value(Some(&item)), 0);
        assert_eq!(parse_mall_price_value(None), 0);
    }

    #[test]
    fn parse_price_from_item_count() {
        let item = CoreItem { id: 1002, count: 2500, ..Default::default() };
        assert_eq!(parse_mall_price_value(Some(&item)), 2500);
    }

    #[test]
    fn parse_price_negative_clamped_to_zero() {
        let item = CoreItem { id: 1002, count: -5, ..Default::default() };
        assert_eq!(parse_mall_price_value(Some(&item)), 0);
    }

    #[test]
    fn parse_price_from_bytes_field2() {
        // 构造 field=2, value=2500 的 varint
        // key = (2 << 3) | 0 = 0x10
        // val = 2500
        let mut bytes = vec![0x10u8];
        let mut v = 2500i64;
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                bytes.push(byte);
                break;
            } else {
                bytes.push(byte | 0x80);
            }
        }
        assert_eq!(parse_mall_price_value_from_bytes(&bytes), 2500);
    }

    #[test]
    fn find_fertilizer_goods_organic() {
        let mut list = vec![];
        list.push(MallGoods { goods_id: 9999, ..Default::default() });
        list.push(MallGoods { goods_id: ORGANIC_FERTILIZER_MALL_GOODS_ID, ..Default::default() });
        let found = find_fertilizer_mall_goods(&list, MallFertilizerKind::Organic);
        assert!(found.is_some());
        assert_eq!(found.unwrap().goods_id, ORGANIC_FERTILIZER_MALL_GOODS_ID);
    }

    #[test]
    fn find_fertilizer_goods_normal() {
        let mut list = vec![];
        list.push(MallGoods { goods_id: INORGANIC_FERTILIZER_MALL_GOODS_ID, ..Default::default() });
        let found = find_fertilizer_mall_goods(&list, MallFertilizerKind::Normal);
        assert!(found.is_some());
    }

    #[test]
    fn find_fertilizer_goods_missing() {
        let list: Vec<MallGoods> = vec![];
        assert!(find_fertilizer_mall_goods(&list, MallFertilizerKind::Organic).is_none());
    }

    #[test]
    fn insufficient_balance_detection() {
        assert!(is_insufficient_balance("余额不足，请充值"));
        assert!(is_insufficient_balance("点券不足"));
        assert!(is_insufficient_balance("server reply: code=1000019 msg=..."));
        assert!(!is_insufficient_balance("网络错误"));
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
        assert_eq!(k.chars().nth(4), Some('-'));
        assert_eq!(k.chars().nth(7), Some('-'));
    }

    #[test]
    fn buy_result_default() {
        let r = BuyResult::default();
        assert_eq!(r.bought, 0);
    }

    #[test]
    fn purchase_request_roundtrip() {
        let req = PurchaseRequest { goods_id: 1002, count: 10 };
        let bytes = req.encode_to_vec();
        let back = PurchaseRequest::decode(&bytes[..]).unwrap();
        assert_eq!(back.goods_id, 1002);
        assert_eq!(back.count, 10);
    }

    #[test]
    fn mall_goods_keeps_is_free_flag() {
        let g = MallGoods { goods_id: 1, is_free: true, ..Default::default() };
        assert!(g.is_free);
    }

    #[test]
    fn item_with_price_field_used_in_mall() {
        // 模拟原 TS 中 goods.price 来自 corepb.Item 的场景
        let price = Item { id: 1002, count: 2500, ..Default::default() };
        let _ = price; // 演示：业务侧读取 price.count
    }
}
