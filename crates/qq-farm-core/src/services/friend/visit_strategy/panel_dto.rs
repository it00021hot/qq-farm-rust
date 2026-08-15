//! 好友面板 DTO 与 proto 映射。

use std::sync::Arc;

use crate::services::friend::api::FriendApi;
use crate::services::friend::gid_manager::GidManager;

/// 好友摘要
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendSummary {
    pub gid: i64,
    pub name: String,
    pub avatar_url: String,
    pub level: i64,
    pub gold: i64,
    pub plant: Option<FriendPlantSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendPlantSummary {
    pub steal_num: i64,
    pub dry_num: i64,
    pub weed_num: i64,
    pub insect_num: i64,
}

/// 把 proto `GameFriend` 映射成面板 DTO（对齐 visit-strategy.ts）
#[must_use]
pub fn game_friend_to_summary(
    f: crate::proto::generated::gamepb::friendpb::GameFriend,
) -> FriendSummary {
    let gid = f.gid;
    let name = if !f.remark.trim().is_empty() {
        f.remark
    } else if !f.name.trim().is_empty() {
        f.name
    } else {
        format!("GID:{gid}")
    };
    FriendSummary {
        gid,
        name,
        avatar_url: f.avatar_url.trim().to_string(),
        level: f.level,
        gold: f.gold,
        plant: f.plant.map(|p| FriendPlantSummary {
            steal_num: p.steal_plant_num,
            dry_num: p.dry_num,
            weed_num: p.weed_num,
            insect_num: p.insect_num,
        }),
    }
}

/// 上层调用便利方法：使用 GidManager + FriendApi 拉取好友列表
pub async fn get_friends_list_via(
    api: &FriendApi,
    _gid_manager: &GidManager,
    my_gid: i64,
) -> Vec<FriendSummary> {
    let friends = match api.get_all_game_friends().await {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    friends
        .into_iter()
        .filter(|f| f.gid != my_gid && f.name != "小小农夫" && f.remark != "小小农夫")
        .map(game_friend_to_summary)
        .collect()
}

#[allow(dead_code)]
pub(crate) fn _silence_unused(_: &Arc<FriendApi>) {}
