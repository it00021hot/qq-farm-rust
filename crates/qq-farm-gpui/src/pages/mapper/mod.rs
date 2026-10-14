mod accounts;
mod activities;
mod analytics;
mod commerce;
mod common;
mod dashboard;
mod entity;
mod format;
mod friends;
mod game_config;
mod personal;
mod settings;
use crate::pages::{DisplayRow, Page, PageData};
use serde_json::Value;
pub(crate) fn page_data(page: Page, raw: Value) -> PageData {
    let summary = if page == Page::MysteryShop
        || (page == Page::Mall && raw.get("slotType").and_then(Value::as_i64) == Some(-1))
    {
        "神秘商店限时商品".to_string()
    } else if raw.is_null() && !matches!(page, Page::Accounts | Page::GameConfig | Page::Analytics)
    {
        "尚未添加账号，请先在侧栏添加账号".to_string()
    } else {
        match page {
            Page::Dashboard => "账号状态与近期运行记录",
            Page::Personal => "土地、仓库与每日任务",
            Page::Friends => "好友农场与互动记录",
            Page::Activities => "正在开放的活动和奖励",
            Page::Analytics => "农场收益、种植策略与黑名单",
            Page::Mall => "游戏商城商品与余额",
            Page::MysteryShop => "神秘商店限时商品",
            Page::Settings => "农场策略、自动化与系统选项",
            Page::GameConfig => "种子、果实与道具配置",
            Page::Accounts => "账号连接状态与本地凭据",
        }
        .to_string()
    };
    let mut rows = rows_from_json(&raw, page);
    if page == Page::Friends {
        friends::sort_rows(&mut rows);
    }
    PageData { summary, rows, raw }
}

pub(crate) fn sort_friend_rows(rows: &mut [DisplayRow]) {
    friends::sort_rows(rows);
}

fn rows_from_json(raw: &Value, page: Page) -> Vec<DisplayRow> {
    match page {
        Page::Dashboard => dashboard::rows(raw),
        Page::Personal => personal::rows(raw),
        Page::Friends => friends::rows(raw),
        Page::Activities => activities::rows(raw),
        Page::Analytics => analytics::rows(raw),
        Page::Mall => commerce::mall_rows(raw),
        Page::MysteryShop => commerce::mystery_shop_rows(raw),
        Page::Settings => settings::rows(raw),
        Page::GameConfig => game_config::rows(raw),
        Page::Accounts => accounts::rows(raw),
    }
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
