//! 神秘商人自动化决策层。
//!
//! 1:1 对应原 `core/src/services/mystery-shop-auto.ts`（纯函数 + tick 编排）：
//! - 到货通知 / 自动购买由 7 个 automation 开关控制；
//! - visitKey=`npcId:activeTime` 去重，避免同一波到货重复通知/重复购买；
//! - 余额未知 / 不足 / 币种未允许时跳过购买并记日志。

use std::sync::Arc;

use crate::models::types::AutomationConfig;
use crate::services::commerce::service::{CommerceService, MysteryShopDto};

pub const GOLD_ITEM_ID: i64 = 1001;
pub const COUPON_ITEM_ID: i64 = 1002;
pub const DIAMOND_ITEM_ID: i64 = 1004;
pub const GOLD_BEAN_ITEM_ID: i64 = 1005;

pub const AUTO_BUY_CHECK_INTERVAL_MS: u64 = 10 * 60 * 1000;
pub const AUTO_BUY_INITIAL_DELAY_MS: u64 = 10 * 1000;
pub const AUTO_BUY_AFTER_SAVE_DELAY_MS: u64 = 2 * 1000;

/// 去重状态（visitKey 记忆）
#[derive(Debug, Default, Clone)]
pub struct MysteryShopAutoState {
    pub last_arrival_key: String,
    pub last_purchase_key: String,
}

/// 单次 tick 决策
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MysteryShopDecision {
    pub skip_reason: Option<SkipReason>,
    pub visit_key: Option<String>,
    pub notify_arrival: bool,
    pub should_buy: bool,
    pub skip_buy_reason: Option<SkipBuyReason>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    #[default]
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipBuyReason {
    AutoBuyOff,
    CurrencyNotAllowed,
    BalanceUnknown,
    Insufficient,
}

/// 是否需要监控（自动购买或到货通知任一开启）
#[must_use]
pub fn is_watch_enabled(automation: &AutomationConfig) -> bool {
    automation.mystery_shop_auto_buy || automation.mystery_shop_arrival_notify
}

#[must_use]
pub fn is_currency_allowed(currency_id: i64, automation: &AutomationConfig) -> bool {
    match currency_id {
        GOLD_ITEM_ID => automation.mystery_shop_allow_gold,
        COUPON_ITEM_ID => automation.mystery_shop_allow_coupon,
        DIAMOND_ITEM_ID => automation.mystery_shop_allow_diamond,
        GOLD_BEAN_ITEM_ID => automation.mystery_shop_allow_gold_bean,
        _ => false,
    }
}

#[must_use]
pub fn mystery_shop_visit_key(shop: &MysteryShopDto) -> String {
    let npc_id = shop.npc.as_ref().map(|n| n.id.max(0)).unwrap_or(0);
    format!("{}:{}", npc_id, shop.active_time.max(0))
}

#[must_use]
pub fn decide_tick(
    shop: &MysteryShopDto,
    automation: &AutomationConfig,
    state: &MysteryShopAutoState,
) -> MysteryShopDecision {
    let Some(npc) = shop.npc.as_ref() else {
        return MysteryShopDecision {
            skip_reason: Some(SkipReason::Inactive),
            ..Default::default()
        };
    };
    if !shop.active || npc.id <= 0 || npc.reward.count <= 0 {
        return MysteryShopDecision {
            skip_reason: Some(SkipReason::Inactive),
            ..Default::default()
        };
    }

    let visit_key = mystery_shop_visit_key(shop);
    let notify_arrival =
        automation.mystery_shop_arrival_notify && state.last_arrival_key != visit_key;

    if !automation.mystery_shop_auto_buy {
        return MysteryShopDecision {
            visit_key: Some(visit_key),
            notify_arrival,
            should_buy: false,
            skip_buy_reason: Some(SkipBuyReason::AutoBuyOff),
            skip_reason: None,
        };
    }
    if !is_currency_allowed(npc.price.id, automation) {
        return MysteryShopDecision {
            visit_key: Some(visit_key),
            notify_arrival,
            should_buy: false,
            skip_buy_reason: Some(SkipBuyReason::CurrencyNotAllowed),
            skip_reason: None,
        };
    }
    // 余额校验：ItemDto.balance 为 Option<i64>（None = 未知）
    let afford = match npc.price.balance {
        None => Err(SkipBuyReason::BalanceUnknown),
        Some(balance) if balance < npc.price.count => Err(SkipBuyReason::Insufficient),
        Some(_) => Ok(()),
    };
    match afford {
        Ok(()) => MysteryShopDecision {
            visit_key: Some(visit_key),
            notify_arrival,
            should_buy: true,
            skip_buy_reason: None,
            skip_reason: None,
        },
        Err(reason) => MysteryShopDecision {
            visit_key: Some(visit_key),
            notify_arrival,
            should_buy: false,
            skip_buy_reason: Some(reason),
            skip_reason: None,
        },
    }
}

/// 推送文案
#[must_use]
pub fn build_push(
    shop: &MysteryShopDto,
    arrival: bool,
    purchase: bool,
) -> Option<(String, String)> {
    if !arrival && !purchase {
        return None;
    }
    let npc = shop.npc.as_ref()?;
    let reward_name = if npc.reward.name.is_empty() {
        "神秘商品".to_string()
    } else {
        npc.reward.name.clone()
    };
    let item = format!("{reward_name} x{}", npc.reward.count);
    let price_name =
        if npc.price.name.is_empty() { "货币".to_string() } else { npc.price.name.clone() };
    let price = format!("{} {price_name}", with_thousands(npc.price.count));
    let diff_ms = shop.expire_time.saturating_sub(crate::utils::time::now_ms());
    let remain = if diff_ms > 0 {
        let hours = diff_ms / 3_600_000;
        let minutes = (diff_ms % 3_600_000) / 60_000;
        Some(format!("\n剩余 {hours}小时{minutes}分"))
    } else {
        None
    };
    if arrival && purchase {
        return Some((
            "神秘商人已自动购买".to_string(),
            format!("到货 {item}\n花费 {price}{}", remain.unwrap_or_default()),
        ));
    }
    if purchase {
        return Some(("神秘商人已自动购买".to_string(), format!("购买 {item}\n花费 {price}")));
    }
    Some((
        "神秘商人到货".to_string(),
        format!("{item}\n价格 {price}{}", remain.unwrap_or_default()),
    ))
}

fn with_thousands(v: i64) -> String {
    let raw = v.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    let bytes = raw.as_bytes();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

/// tick 执行结果
#[derive(Debug, Default)]
pub struct TickOutcome {
    pub skipped: bool,
    pub bought: bool,
    /// 需要外发推送的文案（title, content）
    pub push: Option<(String, String)>,
}

/// 执行一次监控 tick：查询 → 决策 → 购买 → 通知。
/// 返回 Err 仅当查询/购买过程出现业务错误且调用方需要感知（内部已记日志）。
pub async fn check_tick(
    commerce: &Arc<CommerceService>,
    automation: &AutomationConfig,
    state: &mut MysteryShopAutoState,
    account_id: &str,
) -> TickOutcome {
    if !is_watch_enabled(automation) {
        return TickOutcome { skipped: true, ..Default::default() };
    }
    let shop = match commerce.get_mystery_shop().await {
        Ok(shop) => shop,
        Err(err) => {
            tracing::warn!(account_id = %account_id, error = %err, "神秘商人检查失败");
            return TickOutcome { skipped: true, ..Default::default() };
        }
    };
    let decision = decide_tick(&shop, automation, state);
    if decision.skip_reason.is_some() {
        return TickOutcome { skipped: true, ..Default::default() };
    }
    let visit_key = decision.visit_key.clone().unwrap_or_default();

    let mut bought = false;
    if decision.should_buy {
        let npc_id = shop.npc.as_ref().map(|n| n.id).unwrap_or(0);
        match commerce.purchase_mystery_offer(&npc_id.to_string()).await {
            Ok(_) => {
                bought = true;
                let npc = shop.npc.as_ref().cloned().unwrap_or_default();
                crate::services::panel_log::log(
                    account_id,
                    "商城",
                    format!(
                        "神秘商人自动购买成功：{} x{}，花费 {} {}",
                        npc.reward.name, npc.reward.count, npc.price.count, npc.price.name
                    ),
                    crate::constants::PanelEvent::MysteryShopWatch,
                    Some(serde_json::json!({
                        "module": "shop",
                        "itemId": npc.reward.id,
                        "count": npc.reward.count,
                        "currencyId": npc.price.id,
                        "price": npc.price.count,
                    })),
                );
            }
            Err(err) => {
                tracing::warn!(account_id = %account_id, error = %err, "神秘商人自动购买失败");
            }
        }
    } else if let Some(reason) = decision.skip_buy_reason {
        let npc = shop.npc.as_ref().cloned().unwrap_or_default();
        let currency_name = if npc.price.name.is_empty() {
            "该货币".to_string()
        } else {
            npc.price.name.clone()
        };
        let message = match reason {
            SkipBuyReason::CurrencyNotAllowed => {
                format!("神秘商人自动购买已跳过：未允许使用{currency_name}")
            }
            SkipBuyReason::Insufficient => {
                format!("神秘商人自动购买已跳过：{currency_name}余额不足")
            }
            SkipBuyReason::BalanceUnknown => {
                format!("神秘商人自动购买已跳过：未能读取{currency_name}余额")
            }
            SkipBuyReason::AutoBuyOff => String::new(),
        };
        if !message.is_empty() {
            crate::services::panel_log::log(
                account_id,
                "商城",
                message,
                crate::constants::PanelEvent::MysteryShopWatch,
                Some(serde_json::json!({ "module": "shop", "isWarn": true })),
            );
        }
    }

    let arrival = decision.notify_arrival;
    let purchase =
        bought && automation.mystery_shop_purchase_notify && state.last_purchase_key != visit_key;
    let push = build_push(&shop, arrival, purchase);
    if arrival {
        state.last_arrival_key = visit_key.clone();
    }
    if purchase {
        state.last_purchase_key = visit_key;
    }
    TickOutcome { skipped: false, bought, push }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::commerce::service::{ItemDto, MysteryNpcDto};

    fn automation(tweak: impl FnOnce(&mut AutomationConfig)) -> AutomationConfig {
        let mut a = AutomationConfig::default();
        tweak(&mut a);
        a
    }

    fn shop(
        active: bool,
        npc_id: i64,
        currency: i64,
        price: i64,
        balance: Option<i64>,
    ) -> MysteryShopDto {
        MysteryShopDto {
            active,
            server_time: 0,
            active_time: 111,
            expire_time: 0,
            npc: Some(MysteryNpcDto {
                id: npc_id,
                reward: ItemDto { count: 2, name: "冬瓜".into(), ..Default::default() },
                stock: 1,
                price: ItemDto {
                    id: currency,
                    count: price,
                    name: "金币".into(),
                    balance,
                    balance_known: balance.is_some(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        }
    }

    #[test]
    fn inactive_shop_skips() {
        let state = MysteryShopAutoState::default();
        let d = decide_tick(
            &shop(false, 1, GOLD_ITEM_ID, 100, Some(999)),
            &automation(|a| a.mystery_shop_auto_buy = true),
            &state,
        );
        assert_eq!(d.skip_reason, Some(SkipReason::Inactive));
        assert!(!d.should_buy);
    }

    #[test]
    fn buys_when_allowed_and_affordable() {
        let state = MysteryShopAutoState::default();
        let d = decide_tick(
            &shop(true, 5, GOLD_ITEM_ID, 100, Some(500)),
            &automation(|a| {
                a.mystery_shop_auto_buy = true;
                a.mystery_shop_allow_gold = true;
            }),
            &state,
        );
        assert!(d.should_buy);
        assert_eq!(d.visit_key.as_deref(), Some("5:111"));
    }

    #[test]
    fn skips_currency_not_allowed_or_insufficient() {
        let state = MysteryShopAutoState::default();
        let cfg = automation(|a| {
            a.mystery_shop_auto_buy = true;
            a.mystery_shop_allow_gold = false;
        });
        let d = decide_tick(&shop(true, 5, GOLD_ITEM_ID, 100, Some(500)), &cfg, &state);
        assert_eq!(d.skip_buy_reason, Some(SkipBuyReason::CurrencyNotAllowed));

        let cfg2 = automation(|a| {
            a.mystery_shop_auto_buy = true;
            a.mystery_shop_allow_gold = true;
        });
        let d2 = decide_tick(&shop(true, 5, GOLD_ITEM_ID, 100, Some(50)), &cfg2, &state);
        assert_eq!(d2.skip_buy_reason, Some(SkipBuyReason::Insufficient));

        let d3 = decide_tick(&shop(true, 5, GOLD_ITEM_ID, 100, None), &cfg2, &state);
        assert_eq!(d3.skip_buy_reason, Some(SkipBuyReason::BalanceUnknown));
    }

    #[test]
    fn arrival_notifies_once_per_visit() {
        let mut state = MysteryShopAutoState::default();
        let cfg = automation(|a| a.mystery_shop_arrival_notify = true);
        let s = shop(true, 5, GOLD_ITEM_ID, 100, Some(999));
        let d = decide_tick(&s, &cfg, &state);
        assert!(d.notify_arrival);
        state.last_arrival_key = d.visit_key.clone().unwrap_or_default();
        let d2 = decide_tick(&s, &cfg, &state);
        assert!(!d2.notify_arrival);
    }

    #[test]
    fn push_text_formats_thousands() {
        let s = shop(true, 5, GOLD_ITEM_ID, 123456, Some(999));
        let (title, content) = build_push(&s, true, false).unwrap();
        assert_eq!(title, "神秘商人到货");
        assert!(content.contains("123,456 金币"), "content={content}");
    }
}
