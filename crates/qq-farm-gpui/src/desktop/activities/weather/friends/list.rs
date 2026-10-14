use super::*;
use gpui_kit::component::scroll::ScrollableElement;

pub(super) fn render_friend_list(
    desktop: &FarmDesktop,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let state = &desktop.activities.weather;
    let query = state.search.read(cx).value().trim().to_lowercase();
    let filtered = base_friends(desktop)
        .iter()
        .filter(|friend| number(friend.get("gid")).is_some_and(|v| v > 0))
        .filter(|friend| {
            query.is_empty()
                || friend_name(friend, language).to_lowercase().contains(&query)
                || gid(friend).contains(&query)
        })
        .take(FRIEND_SHOW_LIMIT)
        .collect::<Vec<_>>();
    let mut list_view = div()
        .v_flex()
        .min_w_0()
        .gap_2()
        .child(Input::new(&state.search).id("weather-friend-search").w_full());
    if filtered.is_empty() {
        return list_view
            .child(empty(
                if query.is_empty() {
                    crate::i18n::text(language, "暂无好友", "No friends")
                } else {
                    crate::i18n::text(language, "没有匹配的好友", "No matching friends")
                },
                cx,
            ))
            .when_some(
                desktop
                    .page_raw
                    .pointer("/weather/friendsError")
                    .and_then(serde_json::Value::as_str),
                |view, error| view.child(muted(error.to_string(), cx)),
            );
    }
    let buttons = filtered.iter().map(|base| {
        let friend = merged_friend(desktop, base);
        let friend_gid = gid(&friend);
        let selected = state.selected_gid == friend_gid;
        let scanning = state.scanning_gids.contains(&friend_gid);
        let label = friend_name(&friend, language);
        let level = friend.get("level").map(value_to_action_string).unwrap_or_else(|| "--".into());
        Button::new(format!("weather-friend-{friend_gid}"))
            .ghost()
            .w_full()
            .accessibility_label(if language == "en-US" {
                format!("{label}, {}", availability_label(&friend, language))
            } else {
                format!("{label}，{}", availability_label(&friend, language))
            })
            .disabled(desktop.loading)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activities.weather.selected_gid = friend_gid.clone();
                if gate(snapshot_value(&this.page_raw), "scanFriendWeather")
                    && !this.activities.weather.scanned_friends.contains_key(&friend_gid)
                {
                    this.run_weather_action("scan", friend_gid.parse().ok(), cx);
                }
                cx.notify();
            }))
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .w_full()
                    .rounded(cx.theme().radius)
                    .p_2()
                    .bg(if selected { cx.theme().sidebar_accent } else { cx.theme().group_box })
                    .child(avatar(&friend, language, cx))
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .min_w_0()
                            .flex_1()
                            .items_start()
                            .child(div().text_sm().font_medium().truncate().child(label))
                            .child(muted(format!("Lv.{level} · GID {}", gid(&friend)), cx)),
                    )
                    .child(muted(
                        if scanning {
                            crate::i18n::text(language, "正在读取…", "Reading…")
                        } else {
                            availability_label(&friend, language)
                        },
                        cx,
                    )),
            )
    });
    list_view = list_view
        .child(div().v_flex().gap_1().max_h(rems(22.)).overflow_y_scrollbar().children(buttons));
    list_view.when(filtered.len() >= FRIEND_SHOW_LIMIT, |view| {
        view.child(muted(
            crate::i18n::text(
                language,
                "列表最多显示 60 位好友，可按名称或 GID 搜索。",
                "Showing up to 60 friends. Search by name or GID.",
            ),
            cx,
        ))
    })
}
