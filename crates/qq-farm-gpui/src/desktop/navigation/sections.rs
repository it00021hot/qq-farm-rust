use super::super::*;

pub(crate) fn default_page_section(page: Page) -> &'static str {
    if page == Page::Activities {
        return "directory";
    }
    page_sections(page).first().map_or("all", |(key, _)| key)
}

pub(crate) fn page_sections(page: Page) -> &'static [(&'static str, &'static str)] {
    match page {
        Page::Dashboard => &[("all", "全部"), ("logs", "运行日志"), ("warnings", "提醒")],
        Page::Personal => &[
            ("lands", "我的农场"),
            ("bag", "我的背包"),
            ("dailyGifts", "我的任务"),
            ("pets", "宠物"),
            ("illustrated", "图鉴"),
            ("interactionItems", "互动道具"),
        ],
        Page::Friends => &[
            ("friends", "好友列表"),
            ("interactionItems", "互动道具"),
            ("blacklist", "好友黑名单"),
            ("records", "最近访客"),
        ],
        Page::Activities => &[],
        Page::Analytics => &[("strategy", "种植策略"), ("plantBlacklist", "种植黑名单")],
        Page::Mall => &[("normal", "普通商城"), ("svip", "SVIP 商城"), ("mystery", "神秘商人")],
        Page::MysteryShop => &[],
        Page::Settings => &[
            ("strategy", "种植策略"),
            ("automation", "自动化"),
            ("offlineReminder", "下线提醒"),
            ("systemConfig", "运行环境"),
            ("ui", "界面偏好"),
        ],
        Page::GameConfig => &[("seeds", "种子"), ("fruits", "果实"), ("items", "道具")],
        Page::Accounts => &[],
    }
}
