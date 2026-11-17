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
    PetOp,
    DogSkillGift,
    MysteryShopWatch,
    RanchAutomation,
    RanchBuy,
    RanchPlace,
    RanchAddGrass,
    RanchUseCan,
    RanchFeed,
    RanchProduce,
    RanchHarvest,
    RanchSell,
    RanchClean,
    RanchUpgrade,
    RanchCompleteUpgrade,
    RanchClaimTasks,
    RanchReportEntry,
    RanchExchange,
    RanchClaimIllustrated,
    RanchClaimDailyShare,
    RanchBuyCan,
    RanchSteal,
    RanchStroke,
    RanchRelease,
    RanchPutMosquito,
    RanchCleanFriend,
    RanchFriend,
    RanchFriendLeave,
    RanchSceneRestore,
    RanchTaskClaim,
    RanchDailyShare,
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
            Self::PetOp => "pet_op",
            Self::DogSkillGift => "dog_skill_gift",
            Self::MysteryShopWatch => "mystery_shop_watch",
            Self::RanchAutomation => "ranch_automation",
            Self::RanchBuy => "ranch_buy",
            Self::RanchPlace => "ranch_place",
            Self::RanchAddGrass => "ranch_addGrass",
            Self::RanchUseCan => "ranch_useCan",
            Self::RanchFeed => "ranch_feed",
            Self::RanchProduce => "ranch_produce",
            Self::RanchHarvest => "ranch_harvest",
            Self::RanchSell => "ranch_sell",
            Self::RanchClean => "ranch_clean",
            Self::RanchUpgrade => "ranch_upgrade",
            Self::RanchCompleteUpgrade => "ranch_completeUpgrade",
            Self::RanchClaimTasks => "ranch_claimTasks",
            Self::RanchReportEntry => "ranch_reportEntry",
            Self::RanchExchange => "ranch_exchange",
            Self::RanchClaimIllustrated => "ranch_claimIllustrated",
            Self::RanchClaimDailyShare => "ranch_claimDailyShare",
            Self::RanchBuyCan => "ranch_buyCan",
            Self::RanchSteal => "ranch_steal",
            Self::RanchStroke => "ranch_stroke",
            Self::RanchRelease => "ranch_release",
            Self::RanchPutMosquito => "ranch_putMosquito",
            Self::RanchCleanFriend => "ranch_cleanFriend",
            Self::RanchFriend => "ranch_friend",
            Self::RanchFriendLeave => "ranch_friend_leave",
            Self::RanchSceneRestore => "ranch_scene_restore",
            Self::RanchTaskClaim => "ranch_task_claim",
            Self::RanchDailyShare => "ranch_daily_share",
        }
    }

    /// 日志里显示的中文事件名。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FarmCycle => "农场巡查",
            Self::HarvestCrop => "收获作物",
            Self::RemovePlant => "清理枯株",
            Self::PlantSeed => "种植种子",
            Self::Fertilize => "施加化肥",
            Self::LandsNotify => "土地推送",
            Self::SeedPick => "选择种子",
            Self::SeedBuy => "购买种子",
            Self::FertilizerBuy => "购买化肥",
            Self::FertilizerGiftOpen => "开启礼包",
            Self::FertilizerBuyTimer => "购买化肥计时器",
            Self::TaskScan => "获取任务",
            Self::TaskClaim => "完成任务",
            Self::DailyTask => "每日任务",
            Self::ActivityPoints => "活跃度",
            Self::MallFreeGifts => "免费礼包",
            Self::DailyShare => "分享奖励",
            Self::VipDailyGift => "会员礼包",
            Self::MonthCardGift => "月卡礼包",
            Self::IllustratedRewards => "图鉴奖励",
            Self::EmailRewards => "邮箱领取",
            Self::SellSuccess => "出售成功",
            Self::SellDone => "出售完成",
            Self::UpgradeLand => "土地升级",
            Self::UnlockLand => "土地解锁",
            Self::FriendCycle => "好友巡查",
            Self::VisitFriend => "访问好友",
            Self::FriendScan => "好友扫描",
            Self::FriendRequest => "好友申请",
            Self::AcceptFriendRequest => "同意好友申请",
            Self::PendingFriendRequest => "待处理申请",
            Self::GetFriendList => "获取好友列表",
            Self::FriendListApi => "好友列表接口",
            Self::FriendPlantPatch => "好友气泡",
            Self::EnterFarm => "进入农场",
            Self::CareFriend => "照顾好友",
            Self::PatrolDone => "巡查完成",
            Self::AvatarProbe => "人机头像诊断",
            Self::VisitorGidBackfill => "访客补充好友GID",
            Self::BadActionLimit => "放虫放草次数上限",
            Self::HeartbeatTimeout => "心跳超时",
            Self::Login => "登录",
            Self::PetOp => "宠物操作",
            Self::DogSkillGift => "同气连枝礼包",
            Self::MysteryShopWatch => "神秘商人",
            Self::RanchAutomation => "牧场巡查",
            Self::RanchBuy => "购买牧场商品",
            Self::RanchPlace => "放养幼崽",
            Self::RanchAddGrass => "补充牧草",
            Self::RanchUseCan => "填充罐头",
            Self::RanchFeed => "喂养动物",
            Self::RanchProduce => "牧场生产",
            Self::RanchHarvest => "牧场收获",
            Self::RanchSell => "出售牧场产物",
            Self::RanchClean => "清理牧场",
            Self::RanchUpgrade => "升级牧场建筑",
            Self::RanchCompleteUpgrade => "完成建筑升级",
            Self::RanchClaimTasks => "领取牧场任务",
            Self::RanchReportEntry => "牧场成长任务",
            Self::RanchExchange => "牧场集市兑换",
            Self::RanchClaimIllustrated => "牧场图鉴奖励",
            Self::RanchClaimDailyShare => "牧场每日分享",
            Self::RanchBuyCan => "购买牧场罐头",
            Self::RanchSteal => "偷取好友牧场",
            Self::RanchStroke => "抚摸好友动物",
            Self::RanchRelease => "放生动物",
            Self::RanchPutMosquito => "放蚊子",
            Self::RanchCleanFriend => "清理好友牧场",
            Self::RanchFriend => "拜访好友牧场",
            Self::RanchFriendLeave => "离开好友牧场",
            Self::RanchSceneRestore => "恢复场景",
            Self::RanchTaskClaim => "牧场任务",
            Self::RanchDailyShare => "牧场分享礼包",
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
            Self::PetOp | Self::DogSkillGift | Self::MysteryShopWatch => "warehouse",
            Self::RanchAutomation
            | Self::RanchBuy
            | Self::RanchPlace
            | Self::RanchAddGrass
            | Self::RanchUseCan
            | Self::RanchFeed
            | Self::RanchProduce
            | Self::RanchHarvest
            | Self::RanchSell
            | Self::RanchClean
            | Self::RanchUpgrade
            | Self::RanchCompleteUpgrade
            | Self::RanchClaimTasks
            | Self::RanchReportEntry
            | Self::RanchExchange
            | Self::RanchClaimIllustrated
            | Self::RanchClaimDailyShare
            | Self::RanchBuyCan
            | Self::RanchSteal
            | Self::RanchStroke
            | Self::RanchRelease
            | Self::RanchPutMosquito
            | Self::RanchCleanFriend
            | Self::RanchFriend
            | Self::RanchFriendLeave
            | Self::RanchSceneRestore
            | Self::RanchTaskClaim
            | Self::RanchDailyShare => "ranch",
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
            "pet_op" => Some(Self::PetOp),
            "dog_skill_gift" => Some(Self::DogSkillGift),
            "mystery_shop_watch" => Some(Self::MysteryShopWatch),
            "ranch_automation" => Some(Self::RanchAutomation),
            "ranch_buy" => Some(Self::RanchBuy),
            "ranch_place" => Some(Self::RanchPlace),
            "ranch_addGrass" => Some(Self::RanchAddGrass),
            "ranch_useCan" => Some(Self::RanchUseCan),
            "ranch_feed" => Some(Self::RanchFeed),
            "ranch_produce" => Some(Self::RanchProduce),
            "ranch_harvest" => Some(Self::RanchHarvest),
            "ranch_sell" => Some(Self::RanchSell),
            "ranch_clean" => Some(Self::RanchClean),
            "ranch_upgrade" => Some(Self::RanchUpgrade),
            "ranch_completeUpgrade" => Some(Self::RanchCompleteUpgrade),
            "ranch_claimTasks" => Some(Self::RanchClaimTasks),
            "ranch_reportEntry" => Some(Self::RanchReportEntry),
            "ranch_exchange" => Some(Self::RanchExchange),
            "ranch_claimIllustrated" => Some(Self::RanchClaimIllustrated),
            "ranch_claimDailyShare" => Some(Self::RanchClaimDailyShare),
            "ranch_buyCan" => Some(Self::RanchBuyCan),
            "ranch_steal" => Some(Self::RanchSteal),
            "ranch_stroke" => Some(Self::RanchStroke),
            "ranch_release" => Some(Self::RanchRelease),
            "ranch_putMosquito" => Some(Self::RanchPutMosquito),
            "ranch_cleanFriend" => Some(Self::RanchCleanFriend),
            "ranch_friend" => Some(Self::RanchFriend),
            "ranch_friend_leave" => Some(Self::RanchFriendLeave),
            "ranch_scene_restore" => Some(Self::RanchSceneRestore),
            "ranch_task_claim" => Some(Self::RanchTaskClaim),
            "ranch_daily_share" => Some(Self::RanchDailyShare),
            _ => None,
        }
    }
}

/// 日志事件的中文名。认不出的英文键不显示，已经是中文的原文保留。
#[must_use]
pub fn event_label(raw: &str) -> String {
    let key = raw.trim();
    if let Some(event) = PanelEvent::parse(key) {
        return event.label().to_owned();
    }
    if key.chars().any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch)) {
        key.to_owned()
    } else {
        String::new()
    }
}

/// 牧场日志单独标成牧场，即使旧记录写在任务模块下。
#[must_use]
pub fn module_label(module: &str, event: &str, tag: &str) -> String {
    if module == "ranch" || event.starts_with("ranch_") || tag == "牧场" {
        "牧场".to_owned()
    } else {
        tag.to_owned()
    }
}

impl fmt::Display for PanelEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 全量事件列表（防回归测试用：desktop-ui log-events.ts 必须为每个 key 提供中文标签）
pub const ALL: &[PanelEvent] = &[
    PanelEvent::FarmCycle,
    PanelEvent::HarvestCrop,
    PanelEvent::RemovePlant,
    PanelEvent::PlantSeed,
    PanelEvent::Fertilize,
    PanelEvent::LandsNotify,
    PanelEvent::SeedPick,
    PanelEvent::SeedBuy,
    PanelEvent::FertilizerBuy,
    PanelEvent::FertilizerGiftOpen,
    PanelEvent::FertilizerBuyTimer,
    PanelEvent::TaskScan,
    PanelEvent::TaskClaim,
    PanelEvent::DailyTask,
    PanelEvent::ActivityPoints,
    PanelEvent::MallFreeGifts,
    PanelEvent::DailyShare,
    PanelEvent::VipDailyGift,
    PanelEvent::MonthCardGift,
    PanelEvent::IllustratedRewards,
    PanelEvent::EmailRewards,
    PanelEvent::SellSuccess,
    PanelEvent::SellDone,
    PanelEvent::UpgradeLand,
    PanelEvent::UnlockLand,
    PanelEvent::FriendCycle,
    PanelEvent::VisitFriend,
    PanelEvent::FriendScan,
    PanelEvent::FriendRequest,
    PanelEvent::AcceptFriendRequest,
    PanelEvent::PendingFriendRequest,
    PanelEvent::GetFriendList,
    PanelEvent::FriendListApi,
    PanelEvent::FriendPlantPatch,
    PanelEvent::EnterFarm,
    PanelEvent::CareFriend,
    PanelEvent::PatrolDone,
    PanelEvent::AvatarProbe,
    PanelEvent::VisitorGidBackfill,
    PanelEvent::BadActionLimit,
    PanelEvent::HeartbeatTimeout,
    PanelEvent::Login,
    PanelEvent::PetOp,
    PanelEvent::DogSkillGift,
    PanelEvent::MysteryShopWatch,
    PanelEvent::RanchAutomation,
    PanelEvent::RanchBuy,
    PanelEvent::RanchPlace,
    PanelEvent::RanchAddGrass,
    PanelEvent::RanchUseCan,
    PanelEvent::RanchFeed,
    PanelEvent::RanchProduce,
    PanelEvent::RanchHarvest,
    PanelEvent::RanchSell,
    PanelEvent::RanchClean,
    PanelEvent::RanchUpgrade,
    PanelEvent::RanchCompleteUpgrade,
    PanelEvent::RanchClaimTasks,
    PanelEvent::RanchReportEntry,
    PanelEvent::RanchExchange,
    PanelEvent::RanchClaimIllustrated,
    PanelEvent::RanchClaimDailyShare,
    PanelEvent::RanchBuyCan,
    PanelEvent::RanchSteal,
    PanelEvent::RanchStroke,
    PanelEvent::RanchRelease,
    PanelEvent::RanchPutMosquito,
    PanelEvent::RanchCleanFriend,
    PanelEvent::RanchFriend,
    PanelEvent::RanchFriendLeave,
    PanelEvent::RanchSceneRestore,
    PanelEvent::RanchTaskClaim,
    PanelEvent::RanchDailyShare,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_legacy_maps_to_english_id() {
        assert_eq!(PanelEvent::parse("巡田"), Some(PanelEvent::FarmCycle));
        assert_eq!(PanelEvent::parse("获取好友列表"), Some(PanelEvent::GetFriendList));
        assert_eq!(PanelEvent::FarmCycle.as_str(), "farm_cycle");
        assert_eq!(PanelEvent::RanchDailyShare.module(), "ranch");
        assert_eq!(event_label("ranch_daily_share"), "牧场分享礼包");
        assert_eq!(event_label("not_a_real_event"), "");
        assert_eq!(event_label("已经是中文"), "已经是中文");
        assert_eq!(module_label("task", "ranch_task_claim", "任务"), "牧场");
    }

    /// 前端 log-events.ts 必须为每个 PanelEvent key 提供中文标签
    /// （2026-09-11：mystery_shop_watch 裸奔事件标签事故的防回归）
    #[test]
    fn every_panel_event_has_frontend_label() {
        let ts_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../desktop-ui/src/views/farm/dashboard/log-events.ts"
        );
        let Ok(ts) = std::fs::read_to_string(ts_path) else {
            // desktop-ui 不在（独立发布 core）时跳过
            return;
        };
        for event in ALL {
            let key = event.as_str();
            // 要求独立属性行（`  key:`），避免子串误匹配
            let needle = format!("\n  {key}:");
            assert!(
                ts.contains(&needle) || ts.contains(&format!("\n\t{key}:")),
                "desktop-ui log-events.ts 缺少事件 {key} 的中文标签"
            );
        }
    }
}
