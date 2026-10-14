use super::common::append_entity_rows;
use crate::pages::DisplayRow;
use serde_json::Value;
fn activity_label(key: &str) -> &'static str {
    match key {
        "directory" | "activities" => "活动目录",
        "season" | "battlePass" | "battle_pass" | "seasonPass" => "赛季战令",
        "constellation" => "星座",
        "starSand" | "shop" | "starSandShop" => "星砂商店",
        "solarTerms" | "solar_terms" => "二十四节气",
        "qingmei" | "qingMei" | "greenPlum" | "greenPlumBrew" => "青梅酿造",
        "qixi" | "qixiBridge" => "七夕鹊桥",
        "charity" | "charityRedFlower" => "公益红花",
        "petDiary" | "pet_diary" => "萌宠成长日记",
        "petDiaryFriend" => "好友宝藏",
        "petDiaryRecords" => "萌宠互动日志",
        "petDiaryBattleResult" => "夺宝结果",
        "autumn" | "autumnWish" => "秋日活动",
        "weather" | "weatherActivity" => "天气活动",
        _ => "活动详情",
    }
}

fn activity_section(key: &str) -> &'static str {
    match key {
        "directory" | "activities" => "directory",
        "season" | "battlePass" | "battle_pass" | "seasonPass" => "season",
        "constellation" => "constellation",
        "starSand" | "shop" | "starSandShop" => "shop",
        "solarTerms" | "solar_terms" | "solarTerm" => "solarTerms",
        "qingmei" | "qingMei" | "greenPlum" | "greenPlumBrew" => "qingmei",
        "qixi" | "qixiBridge" => "qixi",
        "charity" | "charityRedFlower" => "charity",
        "petDiary" | "pet_diary" => "petDiary",
        "petDiaryFriend" => "petDiaryFriend",
        "petDiaryRecords" => "petDiaryRecords",
        "petDiaryBattleResult" => "petDiaryBattleResult",
        "autumn" | "autumnWish" => "autumn",
        "weather" | "weatherActivity" => "weather",
        _ => "directory",
    }
}

pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    let Some(object) = raw.as_object() else { return rows };
    for (key, value) in object {
        if !matches!(key.as_str(), "activityFriends" | "petDiaryFriends") {
            append_entity_rows(value, activity_section(key), activity_label(key), &mut rows);
        }
    }
    if let Some(friends) = object.get("activityFriends") {
        append_entity_rows(friends, "activityFriends", "活动赠送好友", &mut rows);
    }
    rows
}
