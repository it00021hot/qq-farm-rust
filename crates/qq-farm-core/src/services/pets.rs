//! 宠物（狗狗）信息与狗粮使用。
//!
//! 1:1 对应原 `core/src/services/pets.ts`：
//! - GetDogInfo / DeployDog / WithdrawDog / DogService.AddFood / GetProtectLogs 均有真实抓包依据；
//! - 狗粮库存必须读背包（GetDogInfo.items.field 3 是状态位不是数量）；
//! - 技能文案是客户端静态数据，不走协议。

use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::dogpb::{
    AddFoodReply, AddFoodRequest, DeployDogReply, DeployDogRequest, GetDogInfoReply,
    GetDogInfoRequest, GetProtectLogsReply, GetProtectLogsRequest, WithdrawDogRequest,
};

const DOG_SERVICE: &str = "gamepb.dogpb.DogService";
const MAX_PROTECT_DURATION_SECONDS: i64 = 30 * 24 * 60 * 60;
const PET_IDS: &[i64] = &[90001, 90002, 90003, 90011, 90021];

fn dog_food_duration(id: i64) -> Option<i64> {
    match id {
        90004 => Some(24 * 60 * 60),
        90005 => Some(3 * 24 * 60 * 60),
        90006 => Some(5 * 24 * 60 * 60),
        _ => None,
    }
}

fn rarity_label(rarity: i64) -> &'static str {
    match rarity {
        1 => "普通",
        2 => "稀有",
        3 => "珍品",
        4 => "天工",
        _ => "未知",
    }
}

fn obtain_condition(id: i64) -> &'static str {
    match id {
        90001 => "参与分享任务可获得",
        90002 => "商店购买：100 点券",
        90003 => "商店购买：200 点券",
        90011 => "商店购买：200 点券",
        90021 => "限时活动获得",
        _ => "游戏内活动或购买获得",
    }
}

/// 客户端静态技能文案（对齐 TS PET_SKILLS 常量表）
fn pet_skill_definitions(pet_id: i64) -> Vec<serde_json::Value> {
    let loyalty = |rate: i64| {
        serde_json::json!({
            "name": "忠心护主",
            "description": format!("作物被偷时，有{rate}%概率触发看护，成功后扣除偷窃者一定金币。"),
            "triggerRate": rate,
            "source": "game-config",
        })
    };
    match pet_id {
        90001 => vec![loyalty(10)],
        90002 => vec![loyalty(30)],
        90003 | 90011 => vec![loyalty(50)],
        90021 => vec![
            loyalty(50),
            serde_json::json!({
                "skillId": 2001,
                "name": "同气连枝",
                "description": "好友前来农场互助（浇水/除草/除虫）时，有概率掉落同气连枝礼包（每日限30次），主人与好友均可获得奖励。",
                "dailyLimit": 30,
                "source": "client-static",
            }),
        ],
        _ => vec![],
    }
}

pub struct PetService {
    gateway: Arc<Gateway>,
}

impl PetService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    async fn get_dog_info(&self) -> Result<GetDogInfoReply> {
        let req = GetDogInfoRequest { host_gid: 0 };
        let body = self.gateway.request(DOG_SERVICE, "GetDogInfo", &req.encode_to_vec()).await?;
        Ok(GetDogInfoReply::decode(&body[..])?)
    }

    /// 宠物快照：狗列表（含技能用量）、狗粮（背包库存）、护主剩余时间、待领礼包数。
    pub async fn get_pet_info(&self) -> Result<serde_json::Value> {
        let reply = self.get_dog_info().await?;
        let bag = crate::services::warehouse::WarehouseService::get_bag_via(&self.gateway).await?;
        Ok(self.build_pet_snapshot(&reply, &bag))
    }

    #[must_use]
    pub fn build_pet_snapshot(
        &self,
        reply: &GetDogInfoReply,
        bag: &crate::proto::generated::gamepb::itempb::BagReply,
    ) -> serde_json::Value {
        let current_dog_id = reply.current_dog_id;
        let skill_usages = &reply.skill_usages;

        let mut dog_ids: Vec<i64> = PET_IDS.to_vec();
        // 保留服务端新增的宠物，避免客户端配置尚未更新时静默丢失数据。
        for raw in &reply.dogs {
            if raw.id > 0 && !dog_ids.contains(&raw.id) {
                dog_ids.push(raw.id);
            }
        }
        let dogs: Vec<serde_json::Value> = dog_ids
            .iter()
            .map(|&id| {
                let raw = reply.dogs.iter().find(|d| d.id == id);
                let gc = global_game_config();
                let info = gc.get_item_by_id(id);
                let name = raw
                    .and_then(|d| (d.name.trim().is_empty()).then_some(d.name.clone()).or(Some(d.name.clone())))
                    .filter(|n| !n.trim().is_empty())
                    .or_else(|| info.as_ref().map(|i| i.name.clone()))
                    .unwrap_or_else(|| format!("宠物#{id}"));
                let rarity = info.as_ref().and_then(|i| i.rarity).unwrap_or(0);
                let owned = raw.map(|d| d.owned == 1).unwrap_or(false) || id == current_dog_id;

                // 技能用量合并（同气连枝：used_count/daily_limit）
                let skills: Vec<serde_json::Value> = pet_skill_definitions(id)
                    .into_iter()
                    .map(|mut def| {
                        let skill_id = def.get("skillId").and_then(|v| v.as_i64()).unwrap_or(0);
                        if skill_id > 0 {
                            if let Some(usage) = skill_usages
                                .iter()
                                .find(|u| u.skill_id == skill_id && u.dog_id == id)
                            {
                                let daily_limit = if usage.daily_limit > 0 {
                                    usage.daily_limit
                                } else {
                                    def.get("dailyLimit").and_then(|v| v.as_i64()).unwrap_or(0)
                                };
                                def["dailyLimit"] = serde_json::json!(daily_limit);
                                def["usedCount"] = serde_json::json!(usage.used_count);
                                def["remainingCount"] =
                                    serde_json::json!((daily_limit - usage.used_count).max(0));
                            }
                        }
                        def
                    })
                    .collect();
                let skill_description = skills
                    .first()
                    .and_then(|s| s.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("暂无技能说明")
                    .to_string();

                serde_json::json!({
                    "id": id,
                    "name": name,
                    "image": gc.get_item_image_by_id(id).unwrap_or_default(),
                    "rarity": rarity,
                    "rarityLabel": rarity_label(rarity),
                    "skills": skills,
                    "skillDescription": skill_description,
                    "obtainCondition": obtain_condition(id),
                    "price": raw.map(|d| d.price).unwrap_or(0),
                    "level": raw.map(|d| d.level).unwrap_or(0),
                    "status": raw.map(|d| d.status).unwrap_or(0),
                    "owned": owned,
                    "active": id == current_dog_id,
                })
            })
            .collect();

        // 狗粮库存：背包是唯一依据
        let bag_items = bag.item_bag.as_ref().map(|b| b.items.as_slice()).unwrap_or(&[]);
        let mut bag_counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for item in bag_items {
            if dog_food_duration(item.id).is_some() && item.count > 0 {
                *bag_counts.entry(item.id).or_insert(0) += item.count;
            }
        }
        let gc = global_game_config();
        let foods: Vec<serde_json::Value> = [90004i64, 90005, 90006]
            .iter()
            .map(|&id| {
                let raw = reply.items.iter().find(|f| f.id == id);
                let fallback = dog_food_duration(id).unwrap_or(1);
                let info = gc.get_item_by_id(id);
                serde_json::json!({
                    "id": id,
                    "name": info.map(|i| i.name.clone()).unwrap_or_else(|| format!("狗粮#{id}")),
                    "image": gc.get_item_image_by_id(id).unwrap_or_default(),
                    "duration": raw.map(|f| f.duration).filter(|d| *d > 0).unwrap_or(fallback),
                    "count": bag_counts.get(&id).copied().unwrap_or(0),
                })
            })
            .collect();

        let protect_duration = reply.protect_time.max(0);
        let max_protect_duration = {
            let v = reply.max_protect_time;
            if v > 0 { v } else { MAX_PROTECT_DURATION_SECONDS }.max(protect_duration)
        };
        serde_json::json!({
            "dogs": dogs,
            "foods": foods,
            "protectDuration": protect_duration,
            "maxProtectDuration": max_protect_duration,
            "remainingDuration": protect_duration,
            "pendingGiftCount": reply.pending_gift_count.max(0),
            "activeDogId": current_dog_id,
            "activeControlSupported": true,
            "guardianRecordsSupported": true,
            "skillCatalog": {
                "source": "client-static",
                "requestVerified": true,
            },
        })
    }

    /// 上场宠物（带事后状态确认）
    pub async fn deploy_dog(&self, dog_id: i64) -> Result<serde_json::Value> {
        if dog_id <= 0 {
            return Err(Error::Business("缺少宠物 ID".into()));
        }
        let before = self.get_dog_info().await?;
        let dog = before.dogs.iter().find(|d| d.id == dog_id);
        let owned = dog.map(|d| d.owned == 1).unwrap_or(false) || dog_id == before.current_dog_id;
        let dog_name = dog.map(|d| d.name.clone()).unwrap_or_default();
        if !owned {
            return Err(Error::Business("未获得该宠物，无法上场".into()));
        }
        let req = DeployDogRequest { dog_id };
        let body = self.gateway.request(DOG_SERVICE, "DeployDog", &req.encode_to_vec()).await?;
        let _reply = DeployDogReply::decode(&body[..])?;
        let snapshot = self.get_pet_info().await?;
        if snapshot["activeDogId"].as_i64() != Some(dog_id) {
            return Err(Error::Business("宠物上场状态未更新，请稍后重试".into()));
        }
        crate::services::panel_log::log(
            "",
            "宠物",
            format!("上场{}", if dog_name.is_empty() { format!("宠物#{dog_id}") } else { dog_name }),
            crate::constants::PanelEvent::PetOp,
            Some(serde_json::json!({ "module": "dog", "dogId": dog_id })),
        );
        Ok(serde_json::json!({ "snapshot": snapshot, "operation": { "type": "deploy", "dogId": dog_id } }))
    }

    /// 收回宠物
    pub async fn withdraw_dog(&self) -> Result<serde_json::Value> {
        let before = self.get_dog_info().await?;
        let current = before.current_dog_id;
        if current == 0 {
            let snapshot = self.get_pet_info().await?;
            return Ok(serde_json::json!({ "snapshot": snapshot, "operation": { "type": "withdraw", "dogId": 0 } }));
        }
        let req = WithdrawDogRequest {};
        let body = self.gateway.request(DOG_SERVICE, "WithdrawDog", &req.encode_to_vec()).await?;
        let _ = crate::proto::generated::gamepb::dogpb::WithdrawDogReply::decode(&body[..])?;
        let snapshot = self.get_pet_info().await?;
        if snapshot["activeDogId"].as_i64().unwrap_or(0) != 0 {
            return Err(Error::Business("宠物收回状态未更新，请稍后重试".into()));
        }
        crate::services::panel_log::log(
            "",
            "宠物",
            "收回当前宠物",
            crate::constants::PanelEvent::PetOp,
            Some(serde_json::json!({ "module": "dog", "dogId": current })),
        );
        Ok(serde_json::json!({ "snapshot": snapshot, "operation": { "type": "withdraw", "dogId": current } }))
    }

    /// 使用狗粮（走 DogService.AddFood，非 ItemService.Use；三重校验对齐 node）
    pub async fn use_dog_food(&self, item_id: i64, count: i64, uid: i64) -> Result<serde_json::Value> {
        let count = count.max(1);
        let Some(duration) = dog_food_duration(item_id) else {
            return Err(Error::Business("该物品不是可用狗粮".into()));
        };
        let before = self.get_dog_info().await?;
        let current_duration = before.protect_time.max(0);
        let requested_duration = duration * count;
        let max_protect = {
            let v = before.max_protect_time;
            if v > 0 { v } else { MAX_PROTECT_DURATION_SECONDS }
        };
        if current_duration + requested_duration > max_protect {
            let remaining = (max_protect - current_duration).max(0);
            return Err(Error::Business(format!(
                "狗粮使用后将超过 30 天上限，当前最多还可增加 {} 天",
                remaining / 86_400
            )));
        }

        // 背包可用数量（排除锁定）
        let bag = crate::services::warehouse::WarehouseService::get_bag_via(&self.gateway).await?;
        let bag_items = bag.item_bag.as_ref().map(|b| b.items.as_slice()).unwrap_or(&[]);
        let available: i64 = bag_items
            .iter()
            .filter(|it| it.id == item_id && (uid <= 0 || it.uid == uid) && !it.locked)
            .map(|it| it.count.max(0))
            .sum();
        if available < count {
            return Err(Error::Business(format!("狗粮可用数量不足：需要 {count}，当前 {available}")));
        }

        let req = AddFoodRequest { item_id, count };
        let body = self.gateway.request(DOG_SERVICE, "AddFood", &req.encode_to_vec()).await?;
        let reply = AddFoodReply::decode(&body[..])?;
        if reply.protect_time <= current_duration {
            return Err(Error::Business("狗粮剩余时间未更新，请稍后重试".into()));
        }
        let item_name = global_game_config()
            .get_item_by_id(item_id)
            .map(|i| i.name)
            .unwrap_or_else(|| format!("狗粮#{item_id}"));
        crate::services::panel_log::log(
            "",
            "宠物",
            format!("使用{item_name} x{count}"),
            crate::constants::PanelEvent::PetOp,
            Some(serde_json::json!({ "module": "dog", "itemId": item_id, "count": count })),
        );
        let latest = self.get_pet_info().await?;
        if latest["protectDuration"].as_i64().unwrap_or(0) <= current_duration {
            return Err(Error::Business("狗粮已提交，但刷新后剩余时间未增加".into()));
        }
        Ok(serde_json::json!({
            "snapshot": latest,
            "used": { "itemId": item_id, "count": count, "duration": requested_duration },
        }))
    }

    /// 守护记录（真实点击固定 0/100/0）
    pub async fn get_protect_logs(&self) -> Result<serde_json::Value> {
        let req = GetProtectLogsRequest { field_1: 0, count: 100, field_3: 0 };
        let body = self.gateway.request(DOG_SERVICE, "GetProtectLogs", &req.encode_to_vec()).await?;
        let reply = GetProtectLogsReply::decode(&body[..])?;
        let logs: Vec<serde_json::Value> = reply
            .logs
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let friend_name = if entry.friend_name.trim().is_empty() {
                    format!("用户#{}", entry.friend_gid)
                } else {
                    entry.friend_name.clone()
                };
                serde_json::json!({
                    "id": format!("{}-{}-{}", entry.friend_gid, entry.timestamp, index),
                    "friendGid": entry.friend_gid,
                    "friendName": friend_name,
                    "friendAvatar": entry.friend_avatar,
                    "timestamp": entry.timestamp,
                    "stolenCount": entry.stolen_count,
                    "protectedGold": entry.protected_gold,
                    "dogId": entry.dog_id,
                    "dogName": entry.dog_name,
                })
            })
            .collect();
        let total = reply.total.max(logs.len() as i64);
        Ok(serde_json::json!({ "logs": logs, "total": total, "offset": 0, "limit": 100 }))
    }
}

/// 供礼包服务复用（避免循环依赖：pets 持 gateway 直查）
pub async fn get_dog_info_via(gateway: &Arc<Gateway>) -> Result<GetDogInfoReply> {
    let req = GetDogInfoRequest { host_gid: 0 };
    let body = gateway.request(DOG_SERVICE, "GetDogInfo", &req.encode_to_vec()).await?;
    Ok(GetDogInfoReply::decode(&body[..])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::dogpb::{
        DogInfo, DogItem, DogSkillUsage, GetDogInfoReply as Reply,
    };

    fn bag_with(id: i64, count: i64) -> crate::proto::generated::gamepb::itempb::BagReply {
        let mut bag = crate::proto::generated::gamepb::itempb::BagReply::default();
        let mut item_bag = crate::proto::generated::corepb::ItemBag::default();
        item_bag.items.push(crate::proto::generated::corepb::Item {
            id,
            count,
            ..Default::default()
        });
        bag.item_bag = Some(item_bag);
        bag
    }

    #[test]
    fn pet_snapshot_merges_bag_food_counts() {
        use crate::network::encryptor::NoopEncryptor;
        use crate::network::gateway::{Gateway, GatewayConfig};
        let cfg = GatewayConfig {
            server_url: "ws://127.0.0.1:0".into(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1".into(),
            auth_code: "test".into(),
            headers: Default::default(),
        };
        let gateway = Gateway::new(cfg, Arc::new(NoopEncryptor));
        let svc = PetService::new(Arc::new(gateway));
        let reply = Reply {
            dogs: vec![DogInfo {
                id: 90021,
                name: "小柴".into(),
                price: 0,
                status: 1,
                level: 1,
                field_6: 0,
                owned: 1,
            }],
            current_dog_id: 90021,
            protect_time: 3600,
            max_protect_time: 2_592_000,
            items: vec![DogItem { id: 90004, duration: 86_400, status: 1 }],
            field_6: 0,
            pending_gift_count: 3,
            skill_usages: vec![DogSkillUsage {
                skill_id: 2001,
                used_count: 7,
                daily_limit: 30,
                dog_id: 90021,
            }],
        };
        let snapshot = svc.build_pet_snapshot(&reply, &bag_with(90004, 12));
        assert_eq!(snapshot["activeDogId"].as_i64(), Some(90021));
        assert_eq!(snapshot["pendingGiftCount"].as_i64(), Some(3));
        // 狗粮库存取背包而非 DogItem 状态位
        let food = snapshot["foods"].as_array().unwrap().iter()
            .find(|f| f["id"].as_i64() == Some(90004)).unwrap();
        assert_eq!(food["count"].as_i64(), Some(12));
        // 同气连枝用量合并
        let dog = snapshot["dogs"].as_array().unwrap().iter()
            .find(|d| d["id"].as_i64() == Some(90021)).unwrap();
        let skill = dog["skills"].as_array().unwrap().iter()
            .find(|s| s["skillId"].as_i64() == Some(2001)).unwrap();
        assert_eq!(skill["usedCount"].as_i64(), Some(7));
        assert_eq!(skill["remainingCount"].as_i64(), Some(23));
    }
}
