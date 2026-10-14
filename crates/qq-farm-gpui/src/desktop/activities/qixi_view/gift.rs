use super::super::super::*;
use gpui_kit::component::input::NumberInput;

use super::shared::{item_image, item_label, number, panel};

fn friend_name(friend: &serde_json::Value, language: &str) -> String {
    let value = friend
        .get("nickname")
        .filter(|value| super::shared::js_truthy(value))
        .or_else(|| friend.get("name").filter(|value| super::shared::js_truthy(value)));
    let name = value.map(value_to_action_string).unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        crate::i18n::text(language, "好友", "Friend").into()
    } else {
        name.into()
    }
}

fn friend_gid(friend: &serde_json::Value) -> String {
    friend.get("gid").map(value_to_action_string).unwrap_or_default()
}

fn friend_level(friend: &serde_json::Value) -> Option<String> {
    friend.get("level").and_then(|level| match level {
        serde_json::Value::Null | serde_json::Value::Bool(false) => None,
        serde_json::Value::Number(value) if value.as_f64() == Some(0.) => None,
        serde_json::Value::String(value) if value.is_empty() => None,
        value => Some(value_to_action_string(value)),
    })
}

fn friend_matches(friend: &serde_json::Value, query: &str, language: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    friend_name(friend, language).to_lowercase().contains(query)
}

pub(super) fn render(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    window: &mut Window,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = desktop.preferences.language.as_str();
    let balance = number(activity.pointer("/balances/sachet")).unwrap_or_default();
    let known =
        activity.pointer("/balances/known").and_then(serde_json::Value::as_bool) != Some(false);
    let balance_label = if known {
        super::shared::display_count(activity.pointer("/balances/sachet"))
    } else {
        "--".into()
    };
    let gift_enabled =
        activity.pointer("/actions/gift/enabled").and_then(serde_json::Value::as_bool)
            == Some(true);
    let friends = desktop
        .page_raw
        .get("activityFriends")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let query = desktop.activities.qixi_friend_search.read(cx).value().trim().to_lowercase();
    let selected_gid = desktop.activities.qixi_selected_friend_gid.clone();
    let selected = friends.iter().find(|friend| friend_gid(friend) == selected_gid);
    let gift_pending = desktop.activity_action_is_pending("qixi-gift");
    let gift_count = super::super::qixi_controls::clamped_qixi_gift_count(
        &desktop.activities.qixi_gift_count.read(cx).value(),
        balance,
    );
    let maximum = balance.max(1);
    desktop.activities.qixi_gift_count.update(cx, |state, cx| {
        state.set_min(Some(1.), window, cx);
        state.set_max(Some(maximum as f64), window, cx);
        if super::super::qixi_controls::clamped_qixi_gift_count(&state.value(), balance)
            != gift_count
        {
            state.set_value(gift_count.to_string(), window, cx);
        }
    });
    let filtered = friends
        .iter()
        .filter(|friend| super::shared::js_number(friend.get("gid")).is_some_and(|gid| gid > 0.))
        .filter(|friend| friend_matches(friend, &query, language))
        .collect::<Vec<_>>();

    let mut friend_list = div().v_flex().gap_2().max_h(px(320.)).overflow_y_scrollbar();
    if filtered.is_empty() && !desktop.activities.qixi_friends_loading {
        friend_list = friend_list.child(
            div()
                .py_6()
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(language, "未找到可赠送好友", "No giftable friends")),
        );
    } else {
        for friend in filtered {
            let gid = friend_gid(friend);
            let name = friend_name(friend, language);
            let level = friend_level(friend);
            let avatar = friend
                .get("avatar")
                .filter(|value| super::shared::js_truthy(value))
                .or_else(|| friend.get("avatarUrl").filter(|value| super::shared::js_truthy(value)))
                .or_else(|| {
                    friend.get("avatar_url").filter(|value| super::shared::js_truthy(value))
                })
                .map(value_to_action_string)
                .unwrap_or_default();
            let avatar_row = serde_json::json!({"image": avatar});
            let is_selected = selected_gid == gid;
            let select_gid = gid.clone();
            friend_list = friend_list.child(
                Button::new(format!("qixi-friend-{gid}"))
                    .ghost()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activities.qixi_selected_friend_gid = select_gid.clone();
                        cx.notify();
                    }))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(if is_selected {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .bg(if is_selected {
                                cx.theme().background
                            } else {
                                cx.theme().group_box
                            })
                            .p_2()
                            .child(
                                render_asset_preview_sized(
                                    &DisplayRow {
                                        id: format!("qixi-friend-avatar-{gid}"),
                                        section: "activityFriends".into(),
                                        title: name.clone(),
                                        detail: String::new(),
                                        state: String::new(),
                                        record: avatar_row,
                                    },
                                    40.,
                                    cx,
                                )
                                .unwrap_or_else(|| {
                                    div()
                                        .size(px(40.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(20.))
                                        .bg(cx.theme().background)
                                        .text_sm()
                                        .child(name.chars().next().map_or_else(
                                            || crate::i18n::text(language, "友", "F").to_string(),
                                            |character| character.to_string(),
                                        ))
                                        .into_any_element()
                                }),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .min_w_0()
                                    .flex_1()
                                    .items_start()
                                    .child(div().truncate().text_sm().font_medium().child(name))
                                    .when_some(level, |row, level| {
                                        row.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("Lv{level}")),
                                        )
                                    }),
                            ),
                    ),
            );
        }
    }

    let mut recipient = panel(cx).flex_1();
    if let Some(friend) = selected {
        let name = friend_name(friend, language);
        recipient = recipient
            .child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                    language,
                    "赠送给",
                    "Gift to",
                )),
            )
            .child(div().text_lg().font_semibold().child(name));
        if let Some(level) = friend_level(friend) {
            recipient = recipient.child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(format!("Lv{level}")),
            );
        }
    } else {
        recipient = recipient.child(
            div()
                .py_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(language, "选择一位好友", "Pick a friend")),
        );
    }
    recipient = recipient.child(
        h_flex()
            .items_center()
            .gap_3()
            .child(item_image(
                activity.get("sachet").unwrap_or(&serde_json::Value::Null),
                40.,
                language,
                cx,
            ))
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_sm().font_medium().child(item_label(
                        activity.get("sachet").unwrap_or(&serde_json::Value::Null),
                        "鹊羽香囊",
                        "Feather sachet",
                        language,
                    )))
                    .child(
                        div().text_xs().text_color(cx.theme().muted_foreground).child(
                            crate::i18n::text(
                                language,
                                &format!("可用 {balance_label}"),
                                &format!("Available {balance_label}"),
                            )
                            .to_string(),
                        ),
                    ),
            ),
    );
    recipient = recipient.child(
        h_flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("qixi-gift-count-down")
                    .label("−")
                    .ghost()
                    .disabled(gift_pending || gift_count <= 1)
                    .on_click(cx.listener(|this, _, window, cx| {
                        adjust_count(this, -1, false, window, cx);
                    })),
            )
            .child(
                NumberInput::new(&desktop.activities.qixi_gift_count)
                    .w(px(112.))
                    .disabled(gift_pending),
            )
            .child(
                Button::new("qixi-gift-count-up")
                    .label("+")
                    .ghost()
                    .disabled(gift_pending || balance <= 0 || gift_count >= balance)
                    .on_click(cx.listener(|this, _, window, cx| {
                        adjust_count(this, 1, false, window, cx);
                    })),
            )
            .child(
                Button::new("qixi-gift-count-max")
                    .label(crate::i18n::text(language, "全部", "All"))
                    .ghost()
                    .disabled(gift_pending || balance <= 0 || gift_count >= balance)
                    .on_click(cx.listener(|this, _, window, cx| {
                        adjust_count(this, 0, true, window, cx);
                    })),
            ),
    );
    let can_gift = selected.is_some()
        && gift_enabled
        && balance > 0
        && gift_count > 0
        && gift_count <= balance;
    recipient = recipient.child(
        Button::new("qixi-gift-sachet")
            .label(if gift_pending {
                crate::i18n::text(language, "领取中…", "Claiming…")
            } else {
                crate::i18n::text(language, "赠送香囊", "Gift sachet")
            })
            .primary()
            .loading(gift_pending)
            .disabled(gift_pending || desktop.selected_account.is_none() || !can_gift)
            .on_click(cx.listener(|this, _, _, cx| this.run_activity_action("qixi-gift", cx))),
    );

    panel(cx)
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().font_medium().child(crate::i18n::text(
                    language,
                    "赠送香囊",
                    "Gift sachet",
                )))
                .child(
                    Button::new("qixi-refresh-friends")
                        .label(crate::i18n::text(language, "刷新列表", "Refresh List"))
                        .ghost()
                        .loading(desktop.activities.qixi_friends_loading)
                        .disabled(desktop.activities.qixi_friends_loading)
                        .on_click(cx.listener(|this, _, _, cx| this.load_qixi_friends(true, cx))),
                ),
        )
        .child(Input::new(&desktop.activities.qixi_friend_search).id("qixi-friend-search"))
        .child(div().h_flex().items_start().gap_4().child(friend_list.flex_1()).child(recipient))
}

fn adjust_count(
    desktop: &mut FarmDesktop,
    delta: i64,
    use_balance: bool,
    window: &mut Window,
    cx: &mut Context<FarmDesktop>,
) {
    let balance = desktop
        .page_raw
        .pointer("/qixi/balances/sachet")
        .and_then(|value| number(Some(value)))
        .unwrap_or_default();
    let current = super::super::qixi_controls::clamped_qixi_gift_count(
        &desktop.activities.qixi_gift_count.read(cx).value(),
        balance,
    );
    let next = if use_balance { balance.max(1) } else { current.saturating_add(delta) }
        .clamp(1, balance.max(1));
    desktop
        .activities
        .qixi_gift_count
        .update(cx, |state, cx| state.set_value(next.to_string(), window, cx));
}

#[cfg(test)]
mod tests {
    use super::friend_matches;
    use serde_json::json;

    #[test]
    fn qixi_friend_search_matches_display_name_but_not_gid() {
        let friend = json!({"gid": 123456, "nickname": "小明"});
        assert!(friend_matches(&friend, "小明", "zh-CN"));
        assert!(!friend_matches(&friend, "123456", "zh-CN"));
    }

    #[test]
    fn qixi_friend_name_uses_vue_truthiness_before_trimming() {
        let friend = json!({"nickname": " ", "name": "Fallback name"});
        assert!(!friend_matches(&friend, "fallback", "en-US"));
        assert!(friend_matches(&friend, "friend", "en-US"));
    }
}
