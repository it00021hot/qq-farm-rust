//! 操作窗口内的 ItemNotify 捕获 —— 给手动操作（开礼包 / 偷菜）提供获得明细。
//!
//! 游戏协议里 Harvest / Use 的回包大多不带获得物品（如 `HarvestReply` 没有 items），
//! 实际所得通过紧随回包的 `ItemNotify` 推送（见 plantpb `FertilizerUse` 注释的交叉验证）。
//! 这里在操作期间临时订阅 Notify，收集窗口内的物品增量，Drop 自动退订。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::network::gateway::Gateway;
use crate::network::notify::{ItemChgLite, NotifyEvent};

/// 回包之后留给尾随 ItemNotify 的排空窗口
const DRAIN_WINDOW_MS: u64 = 400;

/// 运行 `fut` 并收集期间（含回包后排空窗口）的 ItemNotify 物品增量。
pub async fn capture_deltas<F>(gateway: &Arc<Gateway>, fut: F) -> (F::Output, Vec<ItemChgLite>)
where
    F: Future,
{
    let mut sub = gateway.subscribe_notify_scoped();
    let result = fut.await;
    let mut deltas = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(DRAIN_WINDOW_MS);
    loop {
        match tokio::time::timeout_at(deadline, sub.recv()).await {
            Ok(Some(NotifyEvent::ItemChanged { items, .. })) => deltas.extend(items),
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
    (result, deltas)
}

/// 一条获得 / 消耗明细（按物品聚合后的增量）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GainEntry {
    pub id: i64,
    pub delta: i64,
}

/// 把 ItemNotify 增量按物品聚合并求和；`positive_only` 时只保留获得。
#[must_use]
pub fn aggregate_deltas(deltas: &[ItemChgLite], positive_only: bool) -> Vec<GainEntry> {
    let mut acc: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for chg in deltas {
        if positive_only && chg.delta <= 0 {
            continue;
        }
        *acc.entry(chg.id).or_insert(0) += chg.delta;
    }
    acc.into_iter().filter(|(_, delta)| *delta != 0).map(|(id, delta)| GainEntry { id, delta }).collect()
}

/// 物品显示名：货币走固定文案，果实用作物名，其余用 ItemInfo；都没有回退 `物品#id`。
#[must_use]
pub fn gain_display_name(id: i64) -> String {
    match id {
        1 | 1001 => "金币".to_string(),
        2 | 1101 => "经验".to_string(),
        1002 => "点券".to_string(),
        1005 => "金豆".to_string(),
        _ => {
            let gc = crate::config::game_config::global();
            if let Some(plant) = gc.get_plant_by_fruit_id(id) {
                let name = gc.get_plant_name(plant.id);
                if !name.is_empty() {
                    return name;
                }
            }
            gc.get_item_by_id(id).map(|it| it.name).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("物品#{id}"))
        }
    }
}

/// 明细转可读文案：`白萝卜×12、金币×100`。
#[must_use]
pub fn format_gains(entries: &[GainEntry]) -> String {
    entries
        .iter()
        .map(|e| format!("{}×{}", gain_display_name(e.id), e.delta))
        .collect::<Vec<_>>()
        .join("、")
}

/// 明细转面板 DTO（id / count / name / image）。
#[must_use]
pub fn gain_dtos(entries: &[GainEntry]) -> Vec<serde_json::Value> {
    entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "count": e.delta,
                "name": gain_display_name(e.id),
                "image": crate::config::game_config::mapped_item_image(e.id),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chg(id: i64, delta: i64) -> ItemChgLite {
        ItemChgLite { id, count: 0, delta }
    }

    #[test]
    fn aggregate_sums_and_drops_zero() {
        let deltas = vec![chg(9, 5), chg(9, 7), chg(1, 100), chg(3, -2), chg(4, 2), chg(4, -2)];
        let all = aggregate_deltas(&deltas, false);
        assert_eq!(
            all,
            vec![GainEntry { id: 1, delta: 100 }, GainEntry { id: 3, delta: -2 }, GainEntry { id: 9, delta: 12 }]
        );
        // positive_only 逐条过滤再求和：+2 保留，负数不参与
        let pos = aggregate_deltas(&deltas, true);
        assert_eq!(
            pos,
            vec![GainEntry { id: 1, delta: 100 }, GainEntry { id: 4, delta: 2 }, GainEntry { id: 9, delta: 12 }]
        );
    }

    #[test]
    fn format_uses_currency_names() {
        let entries = vec![GainEntry { id: 1, delta: 100 }, GainEntry { id: 1002, delta: 10 }];
        assert_eq!(format_gains(&entries), "金币×100、点券×10");
    }

    #[test]
    fn unknown_item_falls_back_to_id() {
        assert_eq!(gain_display_name(999_999), "物品#999999");
    }

    #[tokio::test]
    async fn scoped_subscription_unsubscribes_on_drop() {
        use crate::network::encryptor::NoopEncryptor;
        let gateway = Arc::new(Gateway::new(
            crate::network::gateway::GatewayConfig {
                server_url: "wss://gate.example.com/ws".to_string(),
                platform: "qq".to_string(),
                os: "Windows".to_string(),
                client_version: "1.13.3.16_20260826".to_string(),
                auth_code: "test".to_string(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(NoopEncryptor),
        ));
        let (result, deltas) = capture_deltas(&gateway, async { 42 }).await;
        assert_eq!(result, 42);
        assert!(deltas.is_empty());
        // 捕获结束后临时订阅必须退订，不残留发送端
        assert_eq!(gateway.notify_subscriber_count(), 0);
    }
}
