use super::super::*;

pub(super) fn clamped_qixi_gift_count(value: &str, balance: i64) -> i64 {
    let maximum = balance.max(1);
    let parsed = value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(f64::trunc)
        .unwrap_or(1.);
    (parsed as i64).clamp(1, maximum)
}

pub(super) fn qixi_friend_is_selected(raw: &serde_json::Value, gid: &str) -> bool {
    let Some(gid) = gid.parse::<i64>().ok().filter(|gid| *gid > 0) else { return false };
    raw.get("activityFriends").and_then(serde_json::Value::as_array).is_some_and(|friends| {
        friends.iter().any(|friend| {
            friend.get("gid").map(value_to_action_string).as_deref() == Some(&gid.to_string())
        })
    })
}

pub(super) fn qixi_gift_success_message(
    raw: &serde_json::Value,
    gid: i64,
    count: i64,
    language: &str,
) -> String {
    let friend_name = raw
        .get("activityFriends")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|friend| {
            friend.get("gid").map(value_to_action_string).as_deref() == Some(&gid.to_string())
        })
        .and_then(|friend| {
            ["nickname", "name"].into_iter().find_map(|key| {
                let value = friend.get(key)?;
                let truthy = match value {
                    serde_json::Value::Null => false,
                    serde_json::Value::Bool(value) => *value,
                    serde_json::Value::Number(value) => {
                        value.as_f64().is_some_and(|value| value != 0.)
                    }
                    serde_json::Value::String(value) => !value.is_empty(),
                    _ => true,
                };
                truthy.then(|| value_to_action_string(value))
            })
        })
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| crate::i18n::text(language, "好友", "Friend").into());
    crate::i18n::text(
        language,
        &format!("已向 {friend_name} 赠送 {count} 个鹊羽香囊"),
        &format!("Sent {count} sachets to {friend_name}"),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::{clamped_qixi_gift_count, qixi_friend_is_selected, qixi_gift_success_message};
    use serde_json::json;

    #[test]
    fn gift_requires_a_friend_from_the_loaded_qixi_roster() {
        let raw = json!({"activityFriends": [{"gid": 123}, {"gid": "456"}]});

        assert!(qixi_friend_is_selected(&raw, "123"));
        assert!(qixi_friend_is_selected(&raw, "456"));
        assert!(!qixi_friend_is_selected(&raw, "789"));
        assert!(!qixi_friend_is_selected(&raw, "0"));
        assert!(!qixi_friend_is_selected(&raw, "not-a-gid"));
    }

    #[test]
    fn gift_count_tracks_balance_and_keeps_one_as_the_empty_balance_floor() {
        assert_eq!(clamped_qixi_gift_count("9", 4), 4);
        assert_eq!(clamped_qixi_gift_count("2.9", 4), 2);
        assert_eq!(clamped_qixi_gift_count("1.5", 4), 1);
        assert_eq!(clamped_qixi_gift_count("0", 4), 1);
        assert_eq!(clamped_qixi_gift_count("invalid", 4), 1);
        assert_eq!(clamped_qixi_gift_count("9", 0), 1);
    }

    #[test]
    fn gift_success_feedback_matches_vue_friend_name_and_quantity() {
        let raw = json!({
            "activityFriends": [
                {"gid": "123", "nickname": "  小明  ", "name": "backup"},
                {"gid": 456, "nickname": "", "name": "Alex"},
                {"gid": 789, "nickname": " ", "name": ""}
            ]
        });

        assert_eq!(qixi_gift_success_message(&raw, 123, 3, "zh-CN"), "已向 小明 赠送 3 个鹊羽香囊");
        assert_eq!(qixi_gift_success_message(&raw, 456, 2, "en-US"), "Sent 2 sachets to Alex");
        assert_eq!(qixi_gift_success_message(&raw, 789, 1, "zh-CN"), "已向 好友 赠送 1 个鹊羽香囊");
    }
}
