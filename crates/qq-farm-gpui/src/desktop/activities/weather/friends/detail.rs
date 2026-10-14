use super::*;

pub(super) fn render_detail(
    desktop: &FarmDesktop,
    snapshot: &serde_json::Value,
    friend: Option<&serde_json::Value>,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let Some(friend) = friend else {
        return panel(cx).child(empty(
            crate::i18n::text(
                language,
                "点击好友读取现场天气",
                "Select a friend to read their current weather",
            ),
            cx,
        ));
    };
    let state = &desktop.activities.weather;
    let friend_gid = gid(friend);
    let parsed_gid = friend_gid.parse::<i64>().ok();
    let can_collect = flag(friend, "canCollect");
    let cloud_eligible = !list(friend, "eligibleCloudLandIds").is_empty();
    let mut view = panel(cx)
        .child(
            h_flex().items_center().gap_3().child(avatar(friend, language, cx)).child(
                div()
                    .v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(div().font_medium().child(friend_name(friend, language)))
                    .child(muted(
                        format!(
                            "Lv.{} · GID {friend_gid}",
                            friend
                                .get("level")
                                .map(value_to_action_string)
                                .unwrap_or_else(|| "--".into())
                        ),
                        cx,
                    )),
            ),
        )
        .child(muted(availability_label(friend, language), cx));
    let weather = friend.get("weather").unwrap_or(&serde_json::Value::Null);
    if flag(weather, "active") {
        let at = state.scanned_friends.get(&friend_gid).map_or(state.now_sec, |scan| scan.at_sec);
        view = view.child(muted(
            format!(
                "{} {}",
                crate::i18n::text(language, "雷雨剩余", "Storm remaining"),
                super::super::clock::remaining_label(
                    number(weather.get("remainingSec")).unwrap_or_default(),
                    state.now_sec,
                    at,
                    language,
                )
            ),
            cx,
        ));
    }
    for field in ["availabilityReason", "scanError"] {
        view = view.when_some(text(friend, field), |view, reason| view.child(muted(reason, cx)));
    }
    for (key, label, english_label, action, action_gate, valid) in [
        (
            "weather-collect-selected",
            "采雨",
            "Collect rain",
            "collect",
            "collectWeather",
            can_collect,
        ),
        ("weather-frog-selected", "青蛙使坏", "Frog mischief", "frog", "frogMischief", true),
        (
            "weather-cloud-selected",
            "乌云使坏",
            "Cloud mischief",
            "cloud",
            "cloudMischief",
            cloud_eligible,
        ),
        (
            "weather-scan-selected",
            "重新读取现场天气",
            "Rescan current weather",
            "scan",
            "scanFriendWeather",
            true,
        ),
    ] {
        let label = crate::i18n::text(language, label, english_label);
        let busy = if action == "scan" {
            state.scanning_gids.contains(&friend_gid)
        } else {
            state.pending_actions.contains(action)
        };
        view = view.child(
            Button::new(key)
                .label(label)
                .w_full()
                .when(action == "collect", |button| button.primary())
                .loading(busy)
                .disabled(
                    desktop.loading
                        || parsed_gid.is_none()
                        || !valid
                        || !gate(snapshot, action_gate)
                        || if action == "scan" {
                            state.scanning_gids.contains(&friend_gid)
                        } else {
                            state.pending_actions.contains(action)
                        },
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run_weather_action(action, parsed_gid, cx)
                })),
        );
    }
    view
}
