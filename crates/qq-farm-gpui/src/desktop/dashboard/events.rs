pub(super) const EVENT_LABELS: &[(&str, &str, &str)] = &[
    ("farm_cycle", "农场巡查", "Farm patrol"),
    ("harvest_crop", "收获作物", "Crop harvest"),
    ("remove_plant", "清理枯株", "Remove withered crops"),
    ("plant_seed", "种植种子", "Plant seed"),
    ("fertilize", "施加化肥", "Fertilize"),
    ("lands_notify", "土地推送", "Land notification"),
    ("seed_pick", "选择种子", "Seed selection"),
    ("seed_buy", "购买种子", "Seed purchase"),
    ("fertilizer_buy", "购买化肥", "Fertilizer purchase"),
    ("fertilizer_gift_open", "开启礼包", "Open fertilizer gift"),
    ("fertilizer_buy_timer", "购买化肥计时器", "Fertilizer purchase timer"),
    ("task_scan", "获取任务", "Fetch tasks"),
    ("task_claim", "完成任务", "Claim task"),
    ("daily_task", "每日任务", "Daily task"),
    ("activity_points", "活跃度", "Activity points"),
    ("mall_free_gifts", "免费礼包", "Free gifts"),
    ("daily_share", "分享奖励", "Share reward"),
    ("vip_daily_gift", "会员礼包", "VIP daily gift"),
    ("month_card_gift", "月卡礼包", "Monthly card gift"),
    ("illustrated_rewards", "图鉴奖励", "Collection rewards"),
    ("email_rewards", "邮箱领取", "Mail rewards"),
    ("sell_success", "出售成功", "Sale succeeded"),
    ("sell_done", "出售完成", "Sale completed"),
    ("upgrade_land", "土地升级", "Upgrade land"),
    ("unlock_land", "土地解锁", "Unlock land"),
    ("friend_cycle", "好友巡查", "Friend patrol"),
    ("visit_friend", "访问好友", "Visit friend"),
    ("friend_scan", "好友扫描", "Scan friends"),
    ("friend_request", "好友申请", "Friend request"),
    ("accept_friend_request", "同意好友申请", "Accept friend request"),
    ("pending_friend_request", "待处理申请", "Pending request"),
    ("get_friend_list", "获取好友列表", "Fetch friend list"),
    ("friend_list_api", "好友列表接口", "Friend list API"),
    ("friend_plant_patch", "好友气泡", "Friend crop update"),
    ("enter_farm", "进入农场", "Enter farm"),
    ("care_friend", "照顾好友", "Help friend"),
    ("patrol_done", "巡查完成", "Patrol complete"),
    ("avatar_probe", "人机头像诊断", "Avatar diagnostics"),
    ("visitor_gid_backfill", "访客补充好友GID", "Backfill visitor GID"),
    ("bad_action_limit", "放虫放草次数上限", "Interaction limit reached"),
    ("heartbeat_timeout", "心跳超时", "Heartbeat timeout"),
    ("login", "登录", "Login"),
    ("disconnect_stop", "断开停止", "Stopped after disconnect"),
    ("kickout_stop", "踢下线", "Stopped after kickout"),
    ("kickout", "被踢下线", "Disconnected by server"),
    ("ws_400", "登录失效", "Session expired"),
    ("ws_close", "连接关闭", "Connection closed"),
    ("pet_op", "宠物操作", "Pet action"),
    ("dog_skill_gift", "同气连枝礼包", "Pet skill gift"),
    ("mystery_shop_watch", "神秘商人", "Mystery merchant"),
];

pub(super) fn label(event: &str, language: &str) -> String {
    EVENT_LABELS
        .iter()
        .find_map(|(key, chinese, english)| {
            (*key == event).then(|| crate::i18n::text(language, chinese, english).to_string())
        })
        .unwrap_or_else(|| event.to_string())
}

pub(super) fn is_hidden(event: &str) -> bool {
    event == "avatar_probe"
}

pub(super) fn should_show_chip(tag: &str, event: &str) -> bool {
    let event_label = label(event, "zh-CN");
    !event.is_empty() && tag != event_label && !(tag == "心跳" && event_label == "心跳超时")
}

#[cfg(test)]
mod tests {
    use super::{label, should_show_chip};

    #[test]
    fn event_labels_follow_the_selected_language_and_preserve_unknown_keys() {
        assert_eq!(label("harvest_crop", "zh-CN"), "收获作物");
        assert_eq!(label("harvest_crop", "en-US"), "Crop harvest");
        assert_eq!(label("future_event", "en-US"), "future_event");
    }

    #[test]
    fn event_chip_visibility_compares_the_backend_tag_with_the_event_key_and_label() {
        assert!(!should_show_chip("收获作物", "harvest_crop"));
        assert!(should_show_chip("harvest_crop", "harvest_crop"));
        assert!(!should_show_chip("心跳超时", "heartbeat_timeout"));
        assert!(!should_show_chip("心跳", "heartbeat_timeout"));
        assert!(should_show_chip("好友", "harvest_crop"));
    }
}
