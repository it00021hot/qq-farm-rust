//! 面板日志 event id（存储只用英文 snake_case）。
//!
//! 中文展示名只出现在 UI 映射（desktop-ui `log-events.ts` / bot Dashboard）。

use std::fmt;

/// 稳定的面板日志事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelEvent {
    FarmCycle,
    HarvestCrop,
    RemovePlant,
    PlantSeed,
    Fertilize,
    LandsNotify,
    SeedPick,
    SeedBuy,
    FertilizerBuy,
    FertilizerGiftOpen,
    FertilizerBuyTimer,
    TaskScan,
    TaskClaim,
    DailyTask,
    ActivityPoints,
    MallFreeGifts,
    DailyShare,
    VipDailyGift,
    MonthCardGift,
    IllustratedRewards,
    EmailRewards,
    SellSuccess,
    SellDone,
    UpgradeLand,
    UnlockLand,
    FriendCycle,
    VisitFriend,
    FriendScan,
    FriendRequest,
    AcceptFriendRequest,
    PendingFriendRequest,
    GetFriendList,
    FriendListApi,
    FriendPlantPatch,
    EnterFarm,
    CareFriend,
    PatrolDone,
    AvatarProbe,
    VisitorGidBackfill,
    BadActionLimit,
    HeartbeatTimeout,
    Login,
}

impl PanelEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FarmCycle => "farm_cycle",
            Self::HarvestCrop => "harvest_crop",
            Self::RemovePlant => "remove_plant",
            Self::PlantSeed => "plant_seed",
            Self::Fertilize => "fertilize",
            Self::LandsNotify => "lands_notify",
            Self::SeedPick => "seed_pick",
            Self::SeedBuy => "seed_buy",
            Self::FertilizerBuy => "fertilizer_buy",
            Self::FertilizerGiftOpen => "fertilizer_gift_open",
            Self::FertilizerBuyTimer => "fertilizer_buy_timer",
            Self::TaskScan => "task_scan",
            Self::TaskClaim => "task_claim",
            Self::DailyTask => "daily_task",
            Self::ActivityPoints => "activity_points",
            Self::MallFreeGifts => "mall_free_gifts",
            Self::DailyShare => "daily_share",
            Self::VipDailyGift => "vip_daily_gift",
            Self::MonthCardGift => "month_card_gift",
            Self::IllustratedRewards => "illustrated_rewards",
            Self::EmailRewards => "email_rewards",
            Self::SellSuccess => "sell_success",
            Self::SellDone => "sell_done",
            Self::UpgradeLand => "upgrade_land",
            Self::UnlockLand => "unlock_land",
            Self::FriendCycle => "friend_cycle",
            Self::VisitFriend => "visit_friend",
            Self::FriendScan => "friend_scan",
            Self::FriendRequest => "friend_request",
            Self::AcceptFriendRequest => "accept_friend_request",
            Self::PendingFriendRequest => "pending_friend_request",
            Self::GetFriendList => "get_friend_list",
            Self::FriendListApi => "friend_list_api",
            Self::FriendPlantPatch => "friend_plant_patch",
            Self::EnterFarm => "enter_farm",
            Self::CareFriend => "care_friend",
            Self::PatrolDone => "patrol_done",
            Self::AvatarProbe => "avatar_probe",
            Self::VisitorGidBackfill => "visitor_gid_backfill",
            Self::BadActionLimit => "bad_action_limit",
            Self::HeartbeatTimeout => "heartbeat_timeout",
            Self::Login => "login",
        }
    }

    #[must_use]
    pub const fn module(self) -> &'static str {
        match self {
            Self::FarmCycle
            | Self::HarvestCrop
            | Self::RemovePlant
            | Self::PlantSeed
            | Self::Fertilize
            | Self::LandsNotify
            | Self::SeedPick
            | Self::SeedBuy
            | Self::UpgradeLand
            | Self::UnlockLand => "farm",
            Self::FertilizerBuy | Self::FertilizerGiftOpen | Self::FertilizerBuyTimer => {
                "warehouse"
            }
            Self::SellSuccess | Self::SellDone => "warehouse",
            Self::TaskScan
            | Self::TaskClaim
            | Self::DailyTask
            | Self::ActivityPoints
            | Self::IllustratedRewards => "task",
            Self::MallFreeGifts
            | Self::DailyShare
            | Self::VipDailyGift
            | Self::MonthCardGift
            | Self::EmailRewards => "task",
            Self::FriendCycle
            | Self::VisitFriend
            | Self::FriendScan
            | Self::FriendRequest
            | Self::AcceptFriendRequest
            | Self::PendingFriendRequest
            | Self::GetFriendList
            | Self::FriendListApi
            | Self::FriendPlantPatch
            | Self::EnterFarm
            | Self::CareFriend
            | Self::PatrolDone
            | Self::AvatarProbe
            | Self::VisitorGidBackfill
            | Self::BadActionLimit => "friend",
            Self::HeartbeatTimeout | Self::Login => "system",
        }
    }

    /// 把历史中文 / 英文 id 规范成枚举；无法识别时返回 None。
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "farm_cycle" | "巡田" => Some(Self::FarmCycle),
            "harvest_crop" => Some(Self::HarvestCrop),
            "remove_plant" => Some(Self::RemovePlant),
            "plant_seed" => Some(Self::PlantSeed),
            "fertilize" => Some(Self::Fertilize),
            "lands_notify" => Some(Self::LandsNotify),
            "seed_pick" => Some(Self::SeedPick),
            "seed_buy" => Some(Self::SeedBuy),
            "fertilizer_buy" => Some(Self::FertilizerBuy),
            "fertilizer_gift_open" => Some(Self::FertilizerGiftOpen),
            "fertilizer_buy_timer" | "购买化肥计时器" => Some(Self::FertilizerBuyTimer),
            "task_scan" | "检查任务" => Some(Self::TaskScan),
            "task_claim" | "领取任务" => Some(Self::TaskClaim),
            "daily_task" | "每日任务" => Some(Self::DailyTask),
            "activity_points" | "活跃度" => Some(Self::ActivityPoints),
            "mall_free_gifts" => Some(Self::MallFreeGifts),
            "daily_share" => Some(Self::DailyShare),
            "vip_daily_gift" => Some(Self::VipDailyGift),
            "month_card_gift" => Some(Self::MonthCardGift),
            "illustrated_rewards" | "图鉴" => Some(Self::IllustratedRewards),
            "email_rewards" => Some(Self::EmailRewards),
            "sell_success" => Some(Self::SellSuccess),
            "sell_done" => Some(Self::SellDone),
            "upgrade_land" => Some(Self::UpgradeLand),
            "unlock_land" => Some(Self::UnlockLand),
            "friend_cycle" | "好友巡查循环" => Some(Self::FriendCycle),
            "visit_friend" => Some(Self::VisitFriend),
            "friend_scan" | "好友扫描" => Some(Self::FriendScan),
            "friend_request" | "好友申请" => Some(Self::FriendRequest),
            "accept_friend_request" | "同意好友申请" => Some(Self::AcceptFriendRequest),
            "pending_friend_request" | "待处理申请" => Some(Self::PendingFriendRequest),
            "get_friend_list" | "获取好友列表" => Some(Self::GetFriendList),
            "friend_list_api" | "好友列表接口" => Some(Self::FriendListApi),
            "friend_plant_patch" => Some(Self::FriendPlantPatch),
            "enter_farm" | "进入农场" => Some(Self::EnterFarm),
            "care_friend" | "照顾好友" => Some(Self::CareFriend),
            "patrol_done" | "巡查完成" => Some(Self::PatrolDone),
            "avatar_probe" | "人机头像诊断" => Some(Self::AvatarProbe),
            "visitor_gid_backfill" | "访客补充好友GID" => Some(Self::VisitorGidBackfill),
            "bad_action_limit" | "放虫放草次数上限" => Some(Self::BadActionLimit),
            "heartbeat_timeout" => Some(Self::HeartbeatTimeout),
            "login" => Some(Self::Login),
            _ => None,
        }
    }
}

impl fmt::Display for PanelEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_legacy_maps_to_english_id() {
        assert_eq!(PanelEvent::parse("巡田"), Some(PanelEvent::FarmCycle));
        assert_eq!(PanelEvent::parse("获取好友列表"), Some(PanelEvent::GetFriendList));
        assert_eq!(PanelEvent::FarmCycle.as_str(), "farm_cycle");
    }
}
