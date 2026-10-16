//! 图鉴快照（作物 type=1 / 变异 type=2）。
//!
//! 1:1 对应原 `core/src/services/illustrated.ts`：
//! 并行拉 GetIllustratedListV2 + GetIllustratedLevelListV2，合并等级/进度/每级奖励/
//! 变异分组/超变 buff。自动领取在 task.rs（ClaimAllRewardsV2），这里只做展示。

use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::illustratedpb::{
    GetIllustratedLevelListV2Reply, GetIllustratedLevelListV2Request, GetIllustratedListV2Reply,
    GetIllustratedListV2Request, IllustratedItem, IllustratedReward,
};

const SERVICE: &str = "gamepb.illustratedpb.IllustratedService";

pub struct IllustratedService {
    gateway: Arc<Gateway>,
    /// 账号 id：商店种子列表（shopSeed 富化）读等级快照用
    account_id: String,
}

fn reward_dto(reward: Option<&IllustratedReward>) -> Option<serde_json::Value> {
    let reward = reward?;
    if reward.item_id == 0 {
        return None;
    }
    let gc = global_game_config();
    let name = gc
        .get_item_by_id(reward.item_id)
        .map(|i| i.name)
        .unwrap_or_else(|| format!("物品{}", reward.item_id));
    Some(serde_json::json!({
        "itemId": reward.item_id,
        "count": reward.count.max(0),
        "name": name,
        "image": gc.get_item_image_by_id(reward.item_id).unwrap_or_default(),
    }))
}

fn item_dto(item: &IllustratedItem) -> serde_json::Value {
    let gc = global_game_config();
    let seed_id = item.seed_id;
    let name = gc
        .get_item_by_id(seed_id)
        .map(|i| i.name)
        .or_else(|| gc.get_plant_by_seed_id(seed_id).map(|p| p.name))
        .unwrap_or_else(|| format!("种子#{seed_id}"));
    let attributes: Vec<serde_json::Value> = item
        .attributes
        .iter()
        .filter(|a| a.r#type != 0 || a.value != 0)
        .map(|a| serde_json::json!({ "type": a.r#type, "param": a.param, "value": a.value }))
        .collect();
    serde_json::json!({
        "seedId": seed_id,
        "name": name,
        "image": gc.get_item_image_by_id(seed_id).unwrap_or_default(),
        "rewardCategory": item.reward_category,
        "group": gc.illustrated_mutant_group(seed_id),
        "sort": gc.get_illustrated_by_param(seed_id).map(|e| e.sort).unwrap_or(0),
        "cropCategory": item.crop_category,
        "unlocked": item.unlocked,
        "progress": item.progress.max(0),
        "isNew": item.is_new,
        "reward": reward_dto(item.reward.as_ref()),
        "attributes": attributes,
    })
}

fn buff_dto(entry: &crate::config::game_config::BuffConfigItem) -> serde_json::Value {
    let value = entry.attr_value;
    serde_json::json!({
        "id": entry.id,
        "level": entry.source_param,
        "name": entry.attr_id,
        "value": value,
        "valueType": if value > 10 { "probability" } else { "quantity" },
    })
}

impl IllustratedService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, account_id: String::new() }
    }

    /// 注入账号 id（面板快照的 shopSeed 需要按账号等级判断 locked）
    #[must_use]
    pub fn with_account_id(mut self, account_id: &str) -> Self {
        self.account_id = account_id.to_string();
        self
    }

    async fn get_list(&self, kind: i32) -> Result<GetIllustratedListV2Reply> {
        // 对齐抓包：refresh=false 显式编码（field 1 = 0）
        let req = GetIllustratedListV2Request { refresh: false, r#type: kind };
        let body =
            self.gateway.request(SERVICE, "GetIllustratedListV2", &req.encode_to_vec()).await?;
        Ok(GetIllustratedListV2Reply::decode(&body[..])?)
    }

    async fn get_levels(&self, kind: i32) -> Result<GetIllustratedLevelListV2Reply> {
        let req = GetIllustratedLevelListV2Request { r#type: kind };
        let body = self
            .gateway
            .request(SERVICE, "GetIllustratedLevelListV2", &req.encode_to_vec())
            .await?;
        Ok(GetIllustratedLevelListV2Reply::decode(&body[..])?)
    }

    fn normalize_book(
        &self,
        kind: i32,
        list: &GetIllustratedListV2Reply,
        level_reply: &GetIllustratedLevelListV2Reply,
    ) -> serde_json::Value {
        let gc = global_game_config();
        let levels: Vec<serde_json::Value> = level_reply
            .levels
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "level": entry.level,
                    "progress": entry.progress.max(0),
                    "claimed": entry.claimed,
                    "rewards": entry.rewards.iter().filter_map(|r| reward_dto(Some(r))).collect::<Vec<_>>(),
                })
            })
            .collect();
        let current_level = list.level.max(level_reply.level);
        let next_level_progress = if list.next_level_progress > 0 {
            list.next_level_progress
        } else {
            level_reply
                .levels
                .iter()
                .filter(|l| l.level > current_level)
                .map(|l| l.progress.max(0))
                .min()
                .unwrap_or(0)
        };
        let mut items: Vec<serde_json::Value> = list.items.iter().map(item_dto).collect();
        items.sort_by_key(|it| it["sort"].as_i64().unwrap_or(0));
        serde_json::json!({
            "type": kind,
            "level": current_level,
            "progress": list.progress.max(level_reply.progress),
            "nextLevelProgress": next_level_progress,
            "currentBonus": reward_dto(list.current_bonus.as_ref()),
            "attributeBonuses": list.attribute_bonuses.iter().filter_map(|r| reward_dto(Some(r))).collect::<Vec<_>>(),
            "buffs": if kind == 2 { gc.illustrated_buffs().iter().map(buff_dto).collect::<Vec<_>>() } else { Vec::new() },
            "currentBuffs": if kind == 2 { gc.illustrated_buffs_by_level(current_level as i64).iter().map(buff_dto).collect::<Vec<_>>() } else { Vec::new() },
            "items": items,
            "levels": levels,
        })
    }

    /// 图鉴快照：作物（type=1）+ 变异（type=2）
    pub async fn get_snapshot(&self) -> Result<serde_json::Value> {
        let (crop_list, mutant_list) = tokio::join!(self.get_list(1), self.get_list(2));
        let crop_list = crop_list?;
        let mutant_list = mutant_list?;
        let (crop_levels, mutant_levels) = tokio::join!(self.get_levels(1), self.get_levels(2));
        let crop_levels = crop_levels?;
        let mutant_levels = mutant_levels?;
        let mut crop = self.normalize_book(1, &crop_list, &crop_levels);
        self.attach_shop_seeds(&mut crop).await;
        Ok(serde_json::json!({
            "crop": crop,
            "mutant": self.normalize_book(2, &mutant_list, &mutant_levels),
            "updatedAt": crate::utils::time::now_ms(),
        }))
    }

    /// 作物条目挂商店种子购买信息（对齐 bot `buildIllustratedSnapshot`，0f09702）：
    /// seedId 归一为植物种子 id，可购买的挂完整 shopSeed，仅本地目录可查的挂
    /// `unknownMeta` 版本（goodsId=0），都没有则 null。
    async fn attach_shop_seeds(&self, crop: &mut serde_json::Value) {
        let gc = global_game_config();
        let seeds = crate::services::farm::planting::fetch_available_seeds(
            self.gateway.clone(),
            &self.account_id,
            false,
        )
        .await
        .unwrap_or_default();
        let seed_by_id: std::collections::HashMap<i64, serde_json::Value> = seeds
            .iter()
            .filter_map(|seed| {
                seed.get("seedId").and_then(serde_json::Value::as_i64).map(|id| (id, seed.clone()))
            })
            .collect();
        let local_seeds: std::collections::HashMap<i64, serde_json::Value> = gc
            .get_all_seeds()
            .into_iter()
            .filter_map(|s| serde_json::to_value(&s).ok().map(|v| (s.seed_id, v)))
            .collect();
        let Some(items) = crop.get_mut("items").and_then(serde_json::Value::as_array_mut) else {
            return;
        };
        for item in items.iter_mut() {
            let Some(obj) = item.as_object_mut() else { continue };
            let item_id = obj.get("seedId").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let plant =
                gc.get_plant_by_seed_id(item_id).or_else(|| gc.get_plant_by_fruit_id(item_id));
            let seed_id =
                plant.as_ref().and_then(|p| p.seed_id).filter(|id| *id > 0).unwrap_or(item_id);
            let shop_seed = match seed_by_id.get(&seed_id) {
                Some(shop) => shop.clone(),
                None => match local_seeds.get(&seed_id) {
                    Some(local) => {
                        let mut value = local.clone();
                        if let Some(local_obj) = value.as_object_mut() {
                            local_obj.insert("goodsId".into(), serde_json::json!(0));
                            local_obj.insert("price".into(), serde_json::Value::Null);
                            local_obj.insert("unknownMeta".into(), serde_json::json!(true));
                            local_obj.insert("requiresUnlockCard".into(), serde_json::json!(false));
                        }
                        value
                    }
                    None => serde_json::Value::Null,
                },
            };
            obj.insert("seedId".into(), serde_json::json!(seed_id));
            obj.insert("shopSeed".into(), shop_seed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutant_group_by_param() {
        crate::config::game_config::global().load();
        let gc = crate::config::game_config::global();
        // 40002001 是普通果实 → gold
        assert_eq!(gc.illustrated_mutant_group(40002001), "gold");
    }

    #[test]
    fn item_dto_maps_fields() {
        crate::config::game_config::global().load();
        let item = IllustratedItem {
            seed_id: 40002001,
            reward_category: 2,
            unlocked: true,
            progress: 40,
            crop_category: 1,
            reward: Some(IllustratedReward { item_id: 1002, count: 40 }),
            is_new: false,
            attributes: vec![],
        };
        let dto = item_dto(&item);
        assert_eq!(dto["seedId"].as_i64(), Some(40002001));
        assert_eq!(dto["unlocked"].as_bool(), Some(true));
        assert!(dto["reward"].is_object());
    }
}
