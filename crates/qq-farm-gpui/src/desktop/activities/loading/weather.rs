use super::*;

pub(super) fn preserve_friends_on_error(
    previous: &serde_json::Value,
    incoming: &mut serde_json::Value,
) {
    if !incoming.get("friendsError").is_some_and(serde_json::Value::is_string) {
        return;
    }
    if let Some(friends) = previous.pointer("/weather/friends").filter(|value| value.is_array()) {
        incoming["friends"] = friends.clone();
        incoming["friendsError"] = serde_json::Value::Null;
    }
}

pub(super) async fn load(
    app: &AppContext,
    account_id: &str,
) -> Result<super::ActivityPayload, qq_farm_app::AppError> {
    let (snapshot, friends) = tokio::join!(
        qq_farm_app::weather::snapshot(app, account_id),
        qq_farm_app::weather::friends(app, account_id),
    );
    let snapshot = snapshot?;
    // 好友目录读取失败不应隐藏已成功读取的活动、补给和研究数据。
    let (friends, friends_error) = match friends {
        Ok(friends) => (friends, None),
        Err(error) => (serde_json::json!([]), Some(error.to_string())),
    };

    Ok(super::ActivityPayload {
        value: serde_json::json!({"snapshot": snapshot, "friends": friends, "friendsError": friends_error}),
        pet_diary_friends: None,
    })
}

#[cfg(test)]
mod tests {
    use super::preserve_friends_on_error;
    use serde_json::json;

    #[test]
    fn failed_weather_friend_refresh_keeps_the_last_successful_list() {
        let previous = json!({"weather": {"friends": [{"gid": "12"}]}});
        let mut incoming =
            json!({"snapshot": {"active": true}, "friends": [], "friendsError": "busy"});

        preserve_friends_on_error(&previous, &mut incoming);

        assert_eq!(incoming["friends"], json!([{"gid": "12"}]));
        assert!(incoming["friendsError"].is_null());
    }

    #[test]
    fn initial_weather_friend_failure_keeps_the_empty_state_and_error() {
        let mut incoming =
            json!({"snapshot": {"active": true}, "friends": [], "friendsError": "busy"});

        preserve_friends_on_error(&json!({}), &mut incoming);

        assert!(incoming["friends"].as_array().is_some_and(Vec::is_empty));
        assert_eq!(incoming["friendsError"], "busy");
    }
}
