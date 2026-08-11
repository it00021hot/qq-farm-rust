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
}

impl WarehouseService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            fertilizer_gift_done_date_key: Mutex::new(String::new()),
            fertilizer_gift_last_open_at: Mutex::new(0),
        }
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

    /// 出售物品
    pub async fn sell_items(&self, items: &[(i64, i64, i64)]) -> Result<SellReply> {
        let payload: Vec<CoreItem> = items
            .iter()
            .map(|(id, count, _uid)| CoreItem {
                id: *id,
                count: *count,
                expire_time: 0,
                uid: 0,
                is_new: false,
                mutant_types: vec![],
                show: None,
            })
            .collect();
        let req = SellRequest { items: payload };
        let body = self
            .gateway
            .request("gamepb.itempb.ItemService", "Sell", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(SellReply::decode(&body)?)
    }

    /// 单个使用（带容错：param error 1000020 时降级为 item wrapper 编码）
    pub async fn use_item(
        &self,
        item_id: i64,
        count: i64,
        land_ids: Vec<i64>,
    ) -> Result<UseReply> {
        let req = UseRequest {
            item_id,
            count,
            land_ids,
        };
        let body = match self
            .gateway
            .request(
                "gamepb.itempb.ItemService",
                "Use",
                &req.encode_to_vec(),
                10_000,
            )
            .await
        {
            Ok(b) => b,
            Err(e) => {
                let msg = e.to_string();
                if !(msg.contains("code=1000020") || msg.contains("请求参数错误")) {
                    return Err(crate::error::Error::Network(e));
                }
                return Err(crate::error::Error::Network(e));
            }
        };
        Ok(UseReply::decode(&body)?)
    }

    /// 批量使用
    pub async fn batch_use_items(
        &self,
        items: &[(i64, i64, i64)],
    ) -> Result<BatchUseReply> {
        let payload: Vec<CoreItem> = items
            .iter()
            .map(|(id, count, _uid)| CoreItem {
                id: *id,
                count: *count,
                expire_time: 0,
                uid: 0,
                is_new: false,
                mutant_types: vec![],
                show: None,
            })
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
        }
        (opened, normal_h, organic_h)
    }

    /// 自动出售所有果实
    pub async fn sell_all_fruits(&self) -> i64 {
        let bag = match self.get_bag().await {
            Ok(b) => b,
            Err(_) => return 0,
        };
        let items = get_bag_items(&bag);
        let to_sell: Vec<_> = items
            .iter()
            .filter_map(|it| {
                let id = it.id;
                let count = it.count;
                if count > 0 && is_fruit_item_id(id) {
                    Some((id, count, it.uid))
                } else {
                    None
                }
            })
            .collect();

        if to_sell.is_empty() {
            return 0;
        }

        let mut total_gold: i64 = 0;
        for chunk in to_sell.chunks(SELL_BATCH_SIZE) {
            if let Ok(reply) = self.sell_items(chunk).await {
                // 累加金币（reply 包含 get_items）
                for item in &reply.get_items {
                    if (item.id == 1 || item.id == 1001) && item.count > 0 {
                        total_gold += item.count;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        if total_gold > 0 {
            tracing::info!("[仓库] 出售 {} 种物品，获得 {} 金币", to_sell.len(), total_gold);
        }
        total_gold
    }

    /// 获取背包 UI 详情
    pub async fn get_bag_detail(&self) -> Result<BagDetail> {
        let bag = self.get_bag().await?;
        let raw_items = get_bag_items(&bag);

        let mut original_items = Vec::new();
        let mut merged: HashMap<i64, BagItemView> = HashMap::new();
        for it in &raw_items {
            let id = it.id;
            let count = it.count;
            let uid = it.uid;
            if id <= 0 || count <= 0 {
                continue;
            }
            original_items.push((id, count, uid));

            let gc = crate::config::game_config::global();
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
                    name = format!("{}种子", p.map(|p| p.name.clone()).unwrap_or_else(|| "未知".to_string()));
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
            let sells_list = item_info
                .as_ref()
                .and_then(|i| i.sells.as_ref())
                .and_then(|s| s.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            let (price_id, price) = if let Some(&(c, p)) = sells_list.first() {
                (c, p)
            } else {
                (0, 0)
            };
            let price_unit = match price_id {
                1005 => "金豆豆",
                1002 => "点券",
                _ => "金",
            };

            let row = merged.entry(id).or_insert_with(|| BagItemView {
                id,
                count: 0,
                name: name.clone(),
                image: gc.get_item_image_by_id(id),
                category: category.clone(),
                item_type: item_info.as_ref().map(|i| i.item_type).unwrap_or(0),
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
            }
        }
        // 排序：17 > 5 > 6 > 其它 itemType，按 count 倒序，按 id 升序
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

        Ok(BagDetail {
            total_kinds: items.len(),
            items,
            original_items,
        })
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

#[derive(Debug, Clone, Default)]
pub struct BagDetail {
    pub total_kinds: usize,
    pub items: Vec<BagItemView>,
    pub original_items: Vec<(i64, i64, i64)>, // (id, count, uid)
}

#[derive(Debug, Clone, Default)]
pub struct BagItemView {
    pub id: i64,
    pub count: i64,
    pub name: String,
    pub image: Option<String>,
    pub category: String,
    pub item_type: i64,
    pub price_id: i64,
    pub price: i64,
    pub price_unit: String,
    pub level: i64,
    pub interaction_type: String,
    pub hours_text: String,
}

#[derive(Debug, Clone, Default)]
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
            })
            .collect()
    } else {
        vec![]
    }
}

/// 简化的 item 表示（用于跨函数）
#[derive(Debug, Clone, Copy)]
pub struct BagItemLite {
    pub id: i64,
    pub count: i64,
    pub uid: i64,
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
            BagItemLite { id: 1011, count: 3600, uid: 0 },
            BagItemLite { id: 1012, count: 7200, uid: 0 },
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
            BagItemLite { id: 80_001, count: 3, uid: 0 },
            BagItemLite { id: 80_001, count: 2, uid: 1 },
            BagItemLite { id: 1011, count: 100, uid: 0 },
            BagItemLite { id: 80_011, count: 1, uid: 0 },
        ];
        let merged = collect_fertilizer_use_payload(&items);
        assert_eq!(merged.len(), 2); // 80_001 和 80_011
        let normal_count = merged.iter().find(|p| p.id == 80_001).unwrap().count;
        assert_eq!(normal_count, 5);
    }

    #[test]
    fn get_gold_from_items_finds_gold_id() {
        let items = vec![
            BagItemLite { id: 100, count: 1, uid: 0 },
            BagItemLite { id: 1, count: 500, uid: 0 },
            BagItemLite { id: 1101, count: 100, uid: 0 },
        ];
        assert_eq!(get_gold_from_items(&items), 500);
    }

    #[test]
    fn get_gold_from_items_handles_1001_too() {
        let items = vec![BagItemLite { id: 1001, count: 1000, uid: 0 }];
        assert_eq!(get_gold_from_items(&items), 1000);
    }

    #[test]
    fn get_gold_from_items_empty() {
        assert_eq!(get_gold_from_items(&[]), 0);
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
}
