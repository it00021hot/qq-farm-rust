//! 仓库系统 — 自动出售果实 / 开启化肥礼包。
//!
//! 1:1 翻译原 `core/src/services/warehouse.ts`（573 行）。
//!
//! ## 协议
//!
//! - `gamepb.itempb.ItemService.Bag` — 拉取背包
//! - `gamepb.itempb.ItemService.Sell` — 出售（单/批）
//! - `gamepb.itempb.ItemService.Use` — 使用道具（单/批）
//!
//! ## 业务
//!
//! - `sellAllFruits()` — 自动出售所有果实
//! - `openFertilizerGiftPacksSilently()` — 自动开启化肥礼包（含 990h 容量判断）
//! - `getBagDetail()` — 背包 UI 展示（分类 / 排序）
//! - `getBagSeeds()` — 背包里的种子汇总

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::itempb::{
    BagReply, BagRequest, BatchUseReply, BatchUseRequest, SellReply, SellRequest, UseReply,
    UseRequest,
};

const SELL_BATCH_SIZE: usize = 15;
const FERTILIZER_CONTAINER_LIMIT_HOURS: i64 = 990;
const NORMAL_CONTAINER_ID: i64 = 1011;
const ORGANIC_CONTAINER_ID: i64 = 1012;

const FERTILIZER_RELATED_IDS: &[i64] = &[
    100_003, 100_004, 80_001, 80_002, 80_003, 80_004, 80_011, 80_012, 80_013, 80_014,
];

// 化肥道具每小时数
fn normal_fertilizer_hours(id: i64) -> Option<i64> {
    match id {
        80_001 => Some(1),
        80_002 => Some(4),
        80_003 => Some(8),
        80_004 => Some(12),
        _ => None,
    }
}

fn organic_fertilizer_hours(id: i64) -> Option<i64> {
    match id {
        80_011 => Some(1),
        80_012 => Some(4),
        80_013 => Some(8),
        80_014 => Some(12),
        _ => None,
    }
}

/// 化肥类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FertilizerType {
    Normal,
    Organic,
    Other,
}

impl FertilizerType {
    #[must_use]
    pub fn from_interaction_type(s: &str) -> Self {
        match s {
            "fertilizer" => Self::Normal,
            "fertilizerpro" => Self::Organic,
            _ => Self::Other,
        }
    }
}

/// 化肥使用负载
#[derive(Debug, Clone)]
pub struct FertilizerUsePayload {
    pub id: i64,
    pub count: i64,
}

/// 仓库服务
pub struct WarehouseService {
    gateway: Arc<Gateway>,
    fertilizer_gift_done_date_key: Mutex<String>,
    fertilizer_gift_last_open_at: Mutex<i64>,
    account_id: Mutex<String>,
}

impl WarehouseService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            fertilizer_gift_done_date_key: Mutex::new(String::new()),
            fertilizer_gift_last_open_at: Mutex::new(0),
            account_id: Mutex::new(String::new()),
        }
    }

    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
    }

    /// 拉取背包
    pub async fn get_bag(&self) -> Result<BagReply> {
        let req = BagRequest {};
        let body = self
            .gateway
            .request("gamepb.itempb.ItemService", "Bag", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(BagReply::decode(&body)?)
    }

    /// 拉取背包（无 WarehouseService 实例时使用）
    ///
    /// 仅供其他 service 在不持 warehouse 引用时调用。
    pub async fn get_bag_via(gateway: &Arc<Gateway>) -> Result<BagReply> {
        let req = BagRequest {};
        let body = gateway
            .request("gamepb.itempb.ItemService", "Bag", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(BagReply::decode(&body[..])?)
    }

    /// 出售物品。对齐 bot `sellItems`：不可售物品在发 RPC 前拒绝。
    pub async fn sell_items(&self, items: &[(i64, i64, i64)]) -> Result<SellReply> {
        if items.is_empty() {
            return Err(crate::error::Error::Business(
                "没有可出售的物品".to_string(),
            ));
        }
        let gc = crate::config::game_config::global();
        for &(id, count, _) in items {
            if id <= 0 || count <= 0 {
                return Err(crate::error::Error::Business(
                    "出售物品参数无效".to_string(),
                ));
            }
            let sell_info = gc.get_effective_sell_info_by_id(id);
            if !sell_info.sellable {
                let name = gc
                    .get_item_by_id(id)
                    .map(|i| i.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("物品{id}"));
                return Err(crate::error::Error::Business(format!(
                    "{name}当前不可出售"
                )));
            }
        }
        let payload: Vec<CoreItem> = items
            .iter()
            .map(|(id, count, uid)| core_item(*id, *count, *uid))
            .collect();
        let req = SellRequest { items: payload };
        let body = self
            .gateway
            .request("gamepb.itempb.ItemService", "Sell", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(SellReply::decode(&body)?)
    }

    /// 使用背包物品。对齐原 `warehouse.useItem`：`UseRequest { item: { id, count, uid } }`。
    pub async fn use_item(&self, item_id: i64, count: i64, uid: i64) -> Result<UseReply> {
        let count = count.max(1);
        let bag = self.get_bag().await?;
        let bag_items = get_bag_items(&bag);
        let candidates: Vec<BagItemLite> = bag_items
            .iter()
            .cloned()
            .filter(|it| it.id == item_id && (uid <= 0 || it.uid == uid))
            .collect();
        let available: i64 = candidates.iter().map(|it| it.count.max(0)).sum();
        if available < count {
            return Err(crate::error::Error::Business(format!(
                "物品数量不足: 需要 {count}，当前 {available}"
            )));
        }
        let single = candidates.iter().find(|it| it.count >= count).cloned();
        if single.is_none() && candidates.len() > 1 {
            let mut remaining = count;
            let mut batch = Vec::new();
            for candidate in &candidates {
                let use_count = remaining.min(candidate.count.max(0));
                if use_count <= 0 {
                    continue;
                }
                batch.push((item_id, use_count, candidate.uid));
                remaining -= use_count;
                if remaining == 0 {
                    break;
                }
            }
            let reply = self.batch_use_items(&batch).await?;
            return Ok(UseReply {
                used_items: reply.used_items,
                items: reply.items,
            });
        }
        let Some(item) = single else {
            return Err(crate::error::Error::Business(format!(
                "背包中未找到物品 {item_id}"
            )));
        };
        let req = UseRequest {
            item: Some(core_item(item_id, count, item.uid)),
        };
        let body = self
            .gateway
            .request(
                "gamepb.itempb.ItemService",
                "Use",
                &req.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(UseReply::decode(&body)?)
    }

    /// 批量使用
    pub async fn batch_use_items(
        &self,
        items: &[(i64, i64, i64)],
    ) -> Result<BatchUseReply> {
        let payload: Vec<CoreItem> = items
            .iter()
            .map(|(id, count, uid)| core_item(*id, *count, *uid))
            .collect();
        let req = BatchUseRequest { items: payload };
        let body = self
            .gateway
            .request("gamepb.itempb.ItemService", "BatchUse", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(BatchUseReply::decode(&body)?)
    }

    /// 自动开启化肥礼包
    ///
    /// 返回 (opened_count, normal_hours, organic_hours)
    pub async fn auto_open_fertilizer_gift_packs(&self) -> (i64, i64, i64) {
        let bag = match self.get_bag().await {
            Ok(b) => b,
            Err(_) => return (0, 0, 0),
        };
        let items = get_bag_items(&bag);
        let payloads = collect_fertilizer_use_payload(&items);
        if payloads.is_empty() {
            return (0, 0, 0);
        }
        let (mut normal_h, mut organic_h) = get_container_hours_from_bag_items(&items);

        let mut opened: i64 = 0;
        for row in payloads {
            let item_id = row.id;
            let raw_count = row.count.max(1);
            let (ftype, per_item_hours) = get_fertilizer_item_type_and_hours(item_id);
            let mut use_count = raw_count;

            // 容器 990h 上限判断
            if ftype == FertilizerType::Normal || ftype == FertilizerType::Organic {
                let current = if ftype == FertilizerType::Normal {
                    normal_h
                } else {
                    organic_h
                };
                if current >= FERTILIZER_CONTAINER_LIMIT_HOURS {
                    continue;
                }
                if per_item_hours > 0 {
                    let remain =
                        (FERTILIZER_CONTAINER_LIMIT_HOURS - current).max(0);
                    let max_count = remain / per_item_hours;
                    use_count = raw_count.min(max_count).max(0);
                    if use_count <= 0 {
                        continue;
                    }
                }
            }

            let res = self.batch_use_items(&[(item_id, use_count, 0)]).await;
            if res.is_ok() {
                opened += use_count;
                if ftype == FertilizerType::Normal && per_item_hours > 0 {
                    normal_h += use_count * per_item_hours;
                }
                if ftype == FertilizerType::Organic && per_item_hours > 0 {
                    organic_h += use_count * per_item_hours;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        if opened > 0 {
            *self.fertilizer_gift_done_date_key.lock() = get_date_key();
            *self.fertilizer_gift_last_open_at.lock() = crate::utils::time::now_ms();
            tracing::info!("[仓库] 自动使用化肥类道具 x{opened}");
            crate::services::panel_log::log(
                &self.account_id.lock(),
                "仓库",
                format!("自动使用化肥类道具 x{opened}"),
                Some(serde_json::json!({ "module": "warehouse", "event": "fertilizer_gift_open", "count": opened })),
            );
        }
        (opened, normal_h, organic_h)
    }

    /// 自动出售所有果实。对齐 TS `sellAllFruits`：
    /// 果实 + `getEffectiveSellInfo.sellable`，批量失败逐个重试，成功记入今日统计「出售」。
    pub async fn sell_all_fruits(&self) -> i64 {
        let account_id = self.account_id.lock().clone();
        if !crate::services::automation::is_automation_on_for(&account_id, "sell") {
            return 0;
        }
        let bag = match self.get_bag().await {
            Ok(b) => b,
            Err(e) => {
                crate::services::panel_log::log_warn(
                    &account_id,
                    "仓库",
                    format!("出售失败: {e}"),
                    Some(serde_json::json!({
                        "module": "warehouse",
                        "event": "sell_done",
                        "result": "error",
                    })),
                );
                return 0;
            }
        };
        let items = get_bag_items(&bag);
        let gc = crate::config::game_config::global();
        let mut to_sell: Vec<(i64, i64, i64)> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for it in &items {
            if it.count <= 0 {
                continue;
            }
            if !is_fruit_item_id(it.id) {
                continue;
            }
            if !gc.get_effective_sell_info_by_id(it.id).sellable {
                continue;
            }
            let fruit_name = gc.get_fruit_name(it.id);
            let label = if fruit_name.is_empty() {
                format!("物品{}x{}", it.id, it.count)
            } else {
                format!("{fruit_name}x{}", it.count)
            };
            names.push(label);
            to_sell.push((it.id, it.count, it.uid));
        }

        if to_sell.is_empty() {
            crate::services::panel_log::log(
                &account_id,
                "仓库",
                "无果实可出售",
                Some(serde_json::json!({
                    "module": "warehouse",
                    "event": "sell_done",
                    "result": "empty",
                })),
            );
            return 0;
        }

        let gold_before = crate::services::status::status_data_for(&account_id).gold;
        let mut known_gold = gold_before;
        let mut server_gold_total: i64 = 0;
        for chunk in to_sell.chunks(SELL_BATCH_SIZE) {
            match self.sell_items(chunk).await {
                Ok(reply) => {
                    let inferred = derive_gold_gain_from_sell_reply(&reply, known_gold);
                    if inferred.gain > 0 {
                        server_gold_total += inferred.gain;
                    }
                    known_gold = inferred.next_known_gold;
                }
                Err(batch_err) => {
                    crate::services::panel_log::log_warn(
                        &account_id,
                        "仓库",
                        format!("批量出售失败，改为逐个重试: {batch_err}"),
                        Some(serde_json::json!({ "module": "warehouse", "event": "sell_done" })),
                    );
                    for item in chunk {
                        match self.sell_items(&[*item]).await {
                            Ok(reply) => {
                                let inferred =
                                    derive_gold_gain_from_sell_reply(&reply, known_gold);
                                if inferred.gain > 0 {
                                    server_gold_total += inferred.gain;
                                }
                                known_gold = inferred.next_known_gold;
                            }
                            Err(single_err) => {
                                crate::services::panel_log::log_warn(
                                    &account_id,
                                    "仓库",
                                    format!(
                                        "跳过不可售物品: ID={} x{} ({single_err})",
                                        item.0, item.1
                                    ),
                                    Some(serde_json::json!({
                                        "module": "warehouse",
                                        "event": "sell_done",
                                        "result": "skip",
                                        "itemId": item.0,
                                        "count": item.1,
                                    })),
                                );
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        let mut gold_after = gold_before;
        let wait_start = crate::utils::time::now_ms();
        while crate::utils::time::now_ms() - wait_start < 2000 {
            let current = crate::services::status::status_data_for(&account_id).gold;
            if current != gold_before {
                gold_after = current;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        let notify_delta = if gold_after > gold_before {
            gold_after - gold_before
        } else {
            0
        };
        let mut bag_delta: i64 = 0;
        if notify_delta <= 0 && server_gold_total <= 0 {
            if let Ok(bag_after) = self.get_bag().await {
                let bag_gold = get_gold_from_items(&get_bag_items(&bag_after));
                if bag_gold > gold_before {
                    bag_delta = bag_gold - gold_before;
                }
            }
        }
        let total_gold = notify_delta.max(server_gold_total).max(bag_delta);
        if notify_delta <= 0 && total_gold > 0 {
            crate::services::status::update_status_gold_for(&account_id, gold_before + total_gold);
        }
        let event = if total_gold > 0 {
            "sell_success"
        } else {
            "sell_done"
        };
        crate::services::panel_log::log(
            &account_id,
            "仓库",
            format!(
                "出售 {}{}",
                names.join(", "),
                if total_gold > 0 {
                    format!("，获得 {total_gold} 金币")
                } else {
                    String::new()
                }
            ),
            Some(serde_json::json!({
                "module": "warehouse",
                "event": event,
                "result": if total_gold > 0 { "ok" } else { "unknown_gain" },
                "count": to_sell.len(),
                "gold": total_gold,
            })),
        );
        if total_gold > 0 {
            crate::services::stats::record_operation_for(&account_id, "sell", 1);
        }
        total_gold
    }

    /// 获取背包 UI 详情（按 UID 分行，对齐 bot `getBagDetail`）
    pub async fn get_bag_detail(&self) -> Result<BagDetail> {
        let bag = self.get_bag().await?;
        Ok(build_bag_detail_from_items(&get_bag_items(&bag)))
    }

    /// 获取背包里的种子汇总
    pub async fn get_bag_seeds(&self) -> Result<Vec<BagSeedInfo>> {
        let bag = self.get_bag().await?;
        let raw_items = get_bag_items(&bag);
        let gc = crate::config::game_config::global();
        let mut merged: HashMap<i64, BagSeedInfo> = HashMap::new();
        for it in &raw_items {
            let seed_id = it.id;
            let count = it.count;
            if seed_id <= 0 || count <= 0 {
                continue;
            }
            let plant = gc.get_plant_by_seed_id(seed_id);
            let Some(plant) = plant else { continue };
            let item_info = gc.get_item_by_id(seed_id);
            let required_level = item_info
                .as_ref()
                .and_then(|i| i.level)
                .filter(|l| *l > 0)
                .or(plant.land_level_need)
                .unwrap_or(0);
            let image = gc.get_seed_image_by_seed_id(seed_id).or_else(|| gc.get_item_image_by_id(seed_id));
            let plant_size = plant.size.unwrap_or(1).max(1);
            let row = merged.entry(seed_id).or_insert_with(|| BagSeedInfo {
                seed_id,
                name: plant.name.clone(),
                count: 0,
                required_level,
                image,
                plant_size,
            });
            row.count += count;
        }
        Ok(merged.into_values().collect())
    }

    /// 获取化肥礼包每日状态
    #[must_use]
    pub fn get_fertilizer_gift_daily_state(&self) -> serde_json::Value {
        serde_json::json!({
            "key": "fertilizer_gift_open",
            "doneToday": *self.fertilizer_gift_done_date_key.lock() == get_date_key(),
            "lastOpenAt": *self.fertilizer_gift_last_open_at.lock(),
        })
    }
}

// =====================================================================
// 数据类型
// =====================================================================

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BagDetail {
    pub total_kinds: usize,
    pub items: Vec<BagItemView>,
    pub original_items: Vec<OriginalBagItem>,
    #[serde(default)]
    pub system_items: Vec<SystemBagItem>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalBagItem {
    pub id: i64,
    pub count: i64,
    pub uid: i64,
    #[serde(default)]
    pub mutant_types: Vec<i64>,
    pub group_key: String,
}

/// 无 UID 的余额/容器等系统条目（对齐 bot systemItems 精简字段）
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemBagItem {
    pub id: i64,
    pub count: i64,
    pub name: String,
    pub interaction_type: String,
    pub hours_text: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BagItemView {
    pub key: String,
    pub id: i64,
    pub count: i64,
    pub uid: i64,
    #[serde(default)]
    pub mutant_types: Vec<i64>,
    pub name: String,
    pub image: Option<String>,
    pub category: String,
    pub item_type: i64,
    pub sellable: bool,
    pub sell_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_condition: Option<String>,
    pub price_id: i64,
    pub price: i64,
    pub price_unit: String,
    pub level: i64,
    pub interaction_type: String,
    pub hours_text: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BagSeedInfo {
    pub seed_id: i64,
    pub name: String,
    pub count: i64,
    pub required_level: i64,
    pub image: Option<String>,
    pub plant_size: i64,
}

// =====================================================================
// 辅助
// =====================================================================

fn get_date_key() -> String {
    use chrono::Datelike;
    use chrono::Local;
    let now = Local::now();
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

/// 从 BagReply 中提取 item 列表
pub fn get_bag_items(bag: &BagReply) -> Vec<BagItemLite> {
    if let Some(ref item_bag) = bag.item_bag {
        item_bag
            .items
            .iter()
            .map(|i| BagItemLite {
                id: i.id,
                count: i.count,
                uid: i.uid,
                mutant_types: get_mutant_types_from_slice(&i.mutant_types),
            })
            .collect()
    } else {
        vec![]
    }
}

/// 对齐 bot `getMutantTypes`
fn get_mutant_types_from_slice(values: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = values.iter().copied().filter(|v| *v > 0).collect();
    out.sort_unstable();
    out
}

/// 纯函数构建背包详情（便于单测，对齐 bot `getBagDetail`）
pub fn build_bag_detail_from_items(raw_items: &[BagItemLite]) -> BagDetail {
    let gc = crate::config::game_config::global();
    let mut original_items = Vec::new();
    let mut system_items = Vec::new();
    let mut merged: HashMap<String, BagItemView> = HashMap::new();

    for it in raw_items {
        let id = it.id;
        let count = it.count;
        let uid = it.uid;
        if id <= 0 || count <= 0 {
            continue;
        }
        if uid <= 0 {
            let item_info = gc.get_item_by_id(id);
            let interaction_type = item_info
                .as_ref()
                .and_then(|i| i.interaction_type.clone())
                .unwrap_or_default();
            let hours_text = if interaction_type == "fertilizerbucket" {
                let h = ((count as f64) / 3600.0 * 10.0).floor() / 10.0;
                format!("{h:.1}小时")
            } else {
                String::new()
            };
            system_items.push(SystemBagItem {
                id,
                count,
                name: item_info
                    .as_ref()
                    .map(|i| i.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("物品{id}")),
                interaction_type,
                hours_text,
            });
            continue;
        }

        let mutant_types = it.mutant_types.clone();
        let group_key = format!("uid:{uid}");
        original_items.push(OriginalBagItem {
            id,
            count,
            uid,
            mutant_types: mutant_types.clone(),
            group_key: group_key.clone(),
        });

        let item_info = gc.get_item_by_id(id);
        let mut name = item_info
            .as_ref()
            .map(|i| i.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_default();
        let mut category = "item".to_string();
        if id == 1 || id == 1001 {
            name = "金币".to_string();
            category = "gold".to_string();
        } else if id == 1101 {
            name = "经验".to_string();
            category = "exp".to_string();
        } else if gc.get_plant_by_fruit_id(id).is_some() {
            if name.is_empty() {
                name = format!("{}果实", gc.get_fruit_name(id));
            }
            category = "fruit".to_string();
        } else if gc.get_plant_by_seed_id(id).is_some() {
            let p = gc.get_plant_by_seed_id(id);
            if name.is_empty() {
                name = format!(
                    "{}种子",
                    p.map(|p| p.name.clone()).unwrap_or_else(|| "未知".to_string())
                );
            }
            category = "seed".to_string();
        }
        if name.is_empty() {
            name = format!("物品{id}");
        }
        let interaction_type = item_info
            .as_ref()
            .and_then(|i| i.interaction_type.clone())
            .unwrap_or_default();
        let sell_info = item_info
            .as_ref()
            .map(|i| gc.get_effective_sell_info(i))
            .unwrap_or_default();
        let (price_id, price) = if let Some(&(c, p)) = sell_info.sells.first() {
            (c, p)
        } else {
            (0, 0)
        };
        let price_unit = match price_id {
            1005 => "金豆豆",
            1002 => "点券",
            _ => "金",
        };

        let row = merged.entry(group_key.clone()).or_insert_with(|| BagItemView {
            key: group_key,
            id,
            count: 0,
            uid,
            mutant_types: mutant_types.clone(),
            name: name.clone(),
            image: gc.get_item_image_by_id(id),
            category: category.clone(),
            item_type: item_info.as_ref().map(|i| i.item_type).unwrap_or(0),
            sellable: sell_info.sellable,
            sell_status: sell_info.status.to_string(),
            sell_condition: sell_info.condition.clone(),
            price_id,
            price,
            price_unit: price_unit.to_string(),
            level: item_info.as_ref().and_then(|i| i.level).unwrap_or(0),
            interaction_type: interaction_type.clone(),
            hours_text: String::new(),
        });
        row.count += count;
    }

    let mut items: Vec<BagItemView> = merged.into_values().collect();
    for row in &mut items {
        if row.interaction_type == "fertilizerbucket" && row.count > 0 {
            let h = ((row.count as f64) / 3600.0 * 10.0).floor() / 10.0;
            row.hours_text = format!("{h:.1}小时");
        } else {
            row.hours_text.clear();
        }
    }
    items.sort_by(|a, b| {
        let priority = |t: i64| match t {
            17 => 0,
            5 => 1,
            6 => 2,
            x if x > 0 => 1000 + x,
            _ => i64::MAX,
        };
        let pa = priority(a.item_type);
        let pb = priority(b.item_type);
        pa.cmp(&pb)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.id.cmp(&b.id))
    });

    BagDetail {
        total_kinds: items.len(),
        items,
        original_items,
        system_items,
    }
}

/// 简化的 item 表示（用于跨函数）
#[derive(Debug, Clone)]
pub struct BagItemLite {
    pub id: i64,
    pub count: i64,
    pub uid: i64,
    pub mutant_types: Vec<i64>,
}

impl BagItemLite {
    #[must_use]
    pub fn new(id: i64, count: i64, uid: i64) -> Self {
        Self {
            id,
            count,
            uid,
            mutant_types: vec![],
        }
    }
}

fn core_item(id: i64, count: i64, uid: i64) -> CoreItem {
    CoreItem {
        id,
        count,
        expire_time: 0,
        uid,
        is_new: false,
        mutant_types: vec![],
        show: None,
    }
}

/// 判断是否是果实
pub fn is_fruit_item_id(id: i64) -> bool {
    use crate::config::game_config::global as global_game_config;
    let gc = global_game_config();
    gc.get_plant_by_fruit_id(id).is_some()
}

/// 合并化肥使用负载
pub fn collect_fertilizer_use_payload(items: &[BagItemLite]) -> Vec<FertilizerUsePayload> {
    let mut merged: HashMap<i64, i64> = HashMap::new();
    for it in items {
        if !is_fertilizer_related_item_id(it.id) {
            continue;
        }
        if it.count <= 0 {
            continue;
        }
        *merged.entry(it.id).or_insert(0) += it.count;
    }
    merged
        .into_iter()
        .map(|(id, count)| FertilizerUsePayload { id, count })
        .collect()
}

/// 判断是否是化肥相关物品
pub fn is_fertilizer_related_item_id(id: i64) -> bool {
    if id <= 0 {
        return false;
    }
    if id == NORMAL_CONTAINER_ID || id == ORGANIC_CONTAINER_ID {
        return false;
    }
    if FERTILIZER_RELATED_IDS.contains(&id) {
        return true;
    }
    use crate::config::game_config::global as global_game_config;
    let gc = global_game_config();
    let info = gc.get_item_by_id(id);
    let interaction = info
        .as_ref()
        .and_then(|i| i.interaction_type.as_deref())
        .unwrap_or("");
    matches!(interaction, "fertilizer" | "fertilizerpro")
}

/// 从背包里获取化肥容器小时数
pub fn get_container_hours_from_bag_items(items: &[BagItemLite]) -> (i64, i64) {
    let mut normal = 0;
    let mut organic = 0;
    for it in items {
        if it.id == NORMAL_CONTAINER_ID {
            normal = it.count;
        }
        if it.id == ORGANIC_CONTAINER_ID {
            organic = it.count;
        }
    }
    (normal / 3600, organic / 3600)
}

/// 化肥类型 + 每件小时
pub fn get_fertilizer_item_type_and_hours(id: i64) -> (FertilizerType, i64) {
    if let Some(h) = normal_fertilizer_hours(id) {
        return (FertilizerType::Normal, h);
    }
    if let Some(h) = organic_fertilizer_hours(id) {
        return (FertilizerType::Organic, h);
    }
    use crate::config::game_config::global as global_game_config;
    let gc = global_game_config();
    let info = gc.get_item_by_id(id);
    let interaction = info
        .as_ref()
        .and_then(|i| i.interaction_type.as_deref())
        .unwrap_or("");
    let t = FertilizerType::from_interaction_type(interaction);
    let h = if t == FertilizerType::Other { 0 } else { 1 };
    (t, h)
}

/// 从背包获取金币（id=1 或 1001）
pub fn get_gold_from_items(items: &[BagItemLite]) -> i64 {
    for it in items {
        if (it.id == 1 || it.id == 1001) && it.count > 0 {
            return it.count;
        }
    }
    0
}

fn gold_from_core_items(items: &[CoreItem]) -> i64 {
    for it in items {
        if (it.id == 1 || it.id == 1001) && it.count > 0 {
            return it.count;
        }
    }
    0
}

struct GoldGain {
    gain: i64,
    next_known_gold: i64,
}

/// 对齐 TS `deriveGoldGainFromSellReply`
fn derive_gold_gain_from_sell_reply(reply: &SellReply, last_known_gold: i64) -> GoldGain {
    let from_get = gold_from_core_items(&reply.get_items);
    if from_get > 0 {
        return GoldGain {
            gain: from_get,
            next_known_gold: last_known_gold,
        };
    }
    let current_or_delta = gold_from_core_items(&reply.sell_items);
    if current_or_delta <= 0 {
        return GoldGain {
            gain: 0,
            next_known_gold: last_known_gold,
        };
    }
    if last_known_gold > 0 && current_or_delta >= last_known_gold {
        return GoldGain {
            gain: current_or_delta - last_known_gold,
            next_known_gold: current_or_delta,
        };
    }
    GoldGain {
        gain: current_or_delta,
        next_known_gold: last_known_gold,
    }
}

trait EncodeExt {
    fn encode_to_vec(&self) -> Vec<u8>;
}

impl<T: prost::Message> EncodeExt for T {
    fn encode_to_vec(&self) -> Vec<u8> {
        prost::Message::encode_to_vec(self)
    }
}

trait DecodeExt: Sized {
    fn decode(_: &[u8]) -> Result<Self>;
}

impl<T: prost::Message + Default> DecodeExt for T {
    fn decode(bytes: &[u8]) -> Result<Self> {
        T::decode(bytes).map_err(crate::error::Error::from)
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_fruit_item_id_checks_game_config() {
        use crate::config::game_config::global as global_game_config;
        let gc = global_game_config();
        // 白萝卜 seed_id=29999
        let plant = gc.get_plant_by_seed_id(29999);
        assert!(plant.is_some());
        // 找一个 fruit_id（取 plant 的 fruit.id）
        if let Some(p) = plant {
            if let Some(ref f) = p.fruit {
                assert!(is_fruit_item_id(f.id));
            }
        }
    }

    #[test]
    fn is_fertilizer_related_excludes_containers() {
        assert!(!is_fertilizer_related_item_id(1011));
        assert!(!is_fertilizer_related_item_id(1012));
        assert!(!is_fertilizer_related_item_id(0));
        assert!(!is_fertilizer_related_item_id(-1));
    }

    #[test]
    fn is_fertilizer_related_includes_known_ids() {
        assert!(is_fertilizer_related_item_id(80_001));
        assert!(is_fertilizer_related_item_id(80_011));
        assert!(is_fertilizer_related_item_id(100_003));
        assert!(is_fertilizer_related_item_id(100_004));
    }

    #[test]
    fn get_container_hours_calculates() {
        let items = vec![
            BagItemLite::new(1011, 3600, 0),
            BagItemLite::new(1012, 7200, 0),
        ];
        let (n, o) = get_container_hours_from_bag_items(&items);
        assert_eq!(n, 1);
        assert_eq!(o, 2);
    }

    #[test]
    fn get_fertilizer_item_type_known() {
        let (t, h) = get_fertilizer_item_type_and_hours(80_001);
        assert_eq!(t, FertilizerType::Normal);
        assert_eq!(h, 1);

        let (t, h) = get_fertilizer_item_type_and_hours(80_014);
        assert_eq!(t, FertilizerType::Organic);
        assert_eq!(h, 12);

        let (t, _) = get_fertilizer_item_type_and_hours(99_999);
        assert_eq!(t, FertilizerType::Other);
    }

    #[test]
    fn collect_fertilizer_use_payload_dedup() {
        let items = vec![
            BagItemLite::new(80_001, 3, 0),
            BagItemLite::new(80_001, 2, 1),
            BagItemLite::new(1011, 100, 0),
            BagItemLite::new(80_011, 1, 0),
        ];
        let merged = collect_fertilizer_use_payload(&items);
        assert_eq!(merged.len(), 2); // 80_001 和 80_011
        let normal_count = merged.iter().find(|p| p.id == 80_001).unwrap().count;
        assert_eq!(normal_count, 5);
    }

    #[test]
    fn get_gold_from_items_finds_gold_id() {
        let items = vec![
            BagItemLite::new(100, 1, 0),
            BagItemLite::new(1, 500, 0),
            BagItemLite::new(1101, 100, 0),
        ];
        assert_eq!(get_gold_from_items(&items), 500);
    }

    #[test]
    fn get_gold_from_items_handles_1001_too() {
        let items = vec![BagItemLite::new(1001, 1000, 0)];
        assert_eq!(get_gold_from_items(&items), 1000);
    }

    #[test]
    fn get_gold_from_items_empty() {
        assert_eq!(get_gold_from_items(&[]), 0);
    }

    #[test]
    fn derive_gold_gain_prefers_get_items() {
        let reply = SellReply {
            sell_items: vec![],
            get_items: vec![core_item(1001, 88, 0)],
        };
        let inferred = derive_gold_gain_from_sell_reply(&reply, 1000);
        assert_eq!(inferred.gain, 88);
        assert_eq!(inferred.next_known_gold, 1000);
    }

    #[test]
    fn use_request_encodes_nested_item_not_scalar_ids() {
        let req = UseRequest {
            item: Some(core_item(101351, 1, 42)),
        };
        let bytes = prost::Message::encode_to_vec(&req);
        assert!(!bytes.is_empty());
        // field 1 + wire type 2 (length-delimited) = 0x0A
        // 旧错误形状 item_id 是 varint，tag 会是 0x08，服务端就会 1000020。
        assert_eq!(bytes[0], 0x0A, "UseRequest.item must be a nested message");
        let decoded = UseRequest::decode(&bytes[..]).expect("decode");
        let item = decoded.item.expect("item");
        assert_eq!(item.id, 101351);
        assert_eq!(item.count, 1);
        assert_eq!(item.uid, 42);
    }

    #[test]
    fn warehouse_service_construction() {
        use crate::network::gateway::{Gateway, GatewayConfig};
        use crate::network::encryptor::NoopEncryptor;
        let cfg = GatewayConfig {
            server_url: "ws://127.0.0.1:0".into(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1".into(),
            auth_code: "test".into(),
            headers: Default::default(),
        };
        let _ = WarehouseService::new(Arc::new(Gateway::new(cfg, Arc::new(NoopEncryptor))));
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
    }

    #[test]
    fn bag_detail_splits_by_uid_not_item_id() {
        let detail = build_bag_detail_from_items(&[
            BagItemLite {
                id: 41221,
                count: 2,
                uid: 100,
                mutant_types: vec![1],
            },
            BagItemLite {
                id: 41221,
                count: 3,
                uid: 200,
                mutant_types: vec![],
            },
            BagItemLite::new(1011, 3600, 0),
        ]);
        assert_eq!(detail.items.len(), 2);
        assert_eq!(detail.original_items.len(), 2);
        assert_eq!(detail.system_items.len(), 1);
        assert_eq!(detail.system_items[0].id, 1011);
        let keys: Vec<_> = detail.items.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"uid:100"));
        assert!(keys.contains(&"uid:200"));
        let stack = detail.items.iter().find(|i| i.uid == 100).unwrap();
        assert_eq!(stack.count, 2);
        assert_eq!(stack.mutant_types, vec![1]);
        assert_eq!(detail.original_items[0].group_key, "uid:100");
    }

    #[test]
    fn sell_items_rejects_unsellable_before_rpc() {
        // 同步校验逻辑：不可售物品应在构建请求前被识别
        let gc = crate::config::game_config::global();
        // 用一个明确不在可售列表的大 id
        let info = gc.get_effective_sell_info_by_id(9_999_999_001);
        assert!(!info.sellable);
    }
}
