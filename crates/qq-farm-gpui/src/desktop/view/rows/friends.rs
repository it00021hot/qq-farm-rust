use super::super::super::*;

mod avatar;
mod visitors;

fn format_gold(value: i64) -> String {
    value
        .max(0)
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(",")
}

fn vue_truthy(value: Option<&serde_json::Value>) -> bool {
    match value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::Number(value)) => value.as_f64().is_some_and(|value| value != 0.),
        Some(serde_json::Value::String(value)) => !value.is_empty(),
        Some(_) => true,
    }
}

fn show_gold_badge(section: &str, record: &serde_json::Value) -> bool {
    let gold = record.get("gold");
    section == "friends"
        || ((vue_truthy(record.get("level")) || vue_truthy(gold))
            && gold.is_some_and(|value| !value.is_null()))
}

fn friend_pet_badge(record: &serde_json::Value, language: &str) -> Option<(bool, String)> {
    let pet_state = record.get("petState").and_then(serde_json::Value::as_str);
    let pet_name = record
        .pointer("/pet/name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty());
    match pet_state {
        Some("protect") => Some((true, crate::i18n::text(language, "护主犬", "Guard dog").into())),
        Some("other") if vue_truthy(record.get("pet")) => {
            Some((false, pet_name.unwrap_or(crate::i18n::text(language, "宠物", "Pet")).into()))
        }
        _ => None,
    }
}

impl FarmDesktop {
    pub(super) fn render_friends_business_row(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if row.section == "records" {
            return self.render_visitor_row(row, index, cx);
        }
        if row.section == "interactionItems" {
            return self.render_friend_interaction_row(row, index, cx);
        }
        let language = &self.preferences.language;
        let title = row.title.clone();
        let detail = row.detail.clone();
        let state = row.state.clone();
        let is_friend = row.section == "friends" && row.id.starts_with("friends/");
        let gid =
            is_friend.then(|| friend_value_number(row.record.get("gid"))).filter(|gid| *gid > 0);
        let expanded = gid.is_some_and(|gid| {
            self.friends.detail.as_ref().is_some_and(|(open_gid, _)| *open_gid == gid)
                || self.friends.detail_loading_gid == Some(gid)
                || self
                    .friends
                    .detail_error
                    .as_ref()
                    .is_some_and(|(error_gid, _)| *error_gid == gid)
        });
        let pet_badge = if row.section == "friends" {
            friend_pet_badge(&row.record, language).map(|(is_protect, label)| {
                let pet_icon = row
                    .record
                    .pointer("/pet/image")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|raw| crate::assets::resolve_image_source(raw).ok());
                (is_protect, label, pet_icon)
            })
        } else {
            None
        };

        let url = row.record.get("avatar").and_then(serde_json::Value::as_str);
        let avatar = avatar::render_avatar(
            url,
            title.chars().next().unwrap_or('友'),
            format!("friend-avatar-{}", row.id),
            40.,
            cx,
        );

        let mut header = h_flex()
            .id(gid.map_or_else(
                || format!("friend-row-{}", row.id),
                |gid| format!("friend-card-{gid}"),
            ))
            .items_center()
            .gap_3()
            .p_3()
            .when(gid.is_some(), |header| {
                header.cursor_pointer().hover(|style| style.bg(cx.theme().group_box.opacity(0.6)))
            })
            .child(div().child(avatar));

        let metadata = if matches!(row.section.as_str(), "friends" | "blacklist") {
            let gid_value = friend_value_number(row.record.get("gid"));
            let level = friend_value_number(row.record.get("level"));
            let gold = friend_value_number(row.record.get("gold"));
            let blacklist = row.section == "blacklist";
            let mut badges = h_flex().items_center().flex_wrap().gap_2();
            if level > 0 {
                badges = badges.child(if blacklist {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Lv{level}"))
                } else {
                    div()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().background)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .child(format!("Lv{level}"))
                });
            }
            if let Some((is_protect, label, pet_icon)) = pet_badge {
                badges = badges.child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().background)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .when_some(pet_icon, |badge, url| {
                            badge.child(img(url).size(px(16.)).object_fit(ObjectFit::Contain))
                        })
                        .child(if is_protect { format!("🐕 {label}") } else { label }),
                );
            }
            let show_gold = show_gold_badge(&row.section, &row.record);
            if show_gold {
                let gold_label = format!(
                    "{} {}",
                    crate::i18n::text(language, "金币", "Gold"),
                    format_gold(gold)
                );
                let gold_badge = if blacklist {
                    div()
                        .id(format!("friend-gold-{gid_value}"))
                        .aria_label(gold_label.clone())
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(gold_label)
                } else {
                    div()
                        .id(format!("friend-gold-{gid_value}"))
                        .aria_label(gold_label.clone())
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().warning.opacity(0.16))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(gold_label)
                };
                #[cfg(test)]
                let gold_badge = gold_badge.test_support();
                badges = badges.child(gold_badge);
            }
            div()
                .v_flex()
                .min_w_0()
                .flex_1()
                .gap_1()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(div().truncate().font_medium().child(title))
                        .when(gid_value > 0, |line| {
                            line.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("({gid_value})")),
                            )
                        }),
                )
                .child(badges)
                .when(row.section == "friends", |panel| {
                    let actionable = has_friend_action(&row.record);
                    panel.child(
                        div()
                            .text_xs()
                            .text_color(if actionable {
                                cx.theme().success
                            } else {
                                cx.theme().muted_foreground
                            })
                            .when(actionable, |line| line.font_medium())
                            .child(friend_status_localized(&row.record, language)),
                    )
                })
                .into_any_element()
        } else {
            div()
                .v_flex()
                .min_w_0()
                .flex_1()
                .gap_1()
                .child(div().truncate().font_medium().child(title))
                .when(!detail.is_empty(), |line| {
                    line.child(
                        div().text_sm().text_color(cx.theme().muted_foreground).child(detail),
                    )
                })
                .when(!state.is_empty(), |line| {
                    line.child(
                        div()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().background)
                            .px_2()
                            .py_1()
                            .text_xs()
                            .child(state),
                    )
                })
                .into_any_element()
        };
        header = header.child(metadata);

        if gid.is_some() || row.section == "interactionItems" || row.id.starts_with("blacklist/") {
            let actions = self.render_row_actions(row, index, cx);
            header = header.child(
                div()
                    .id(("friend-actions", index))
                    .h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap_1()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(actions),
            );
        }
        if let Some(gid) = gid {
            let toggle_label = if expanded {
                crate::i18n::text(language, "收起好友农场", "Collapse friend farm")
            } else {
                crate::i18n::text(language, "展开好友农场", "Expand friend farm")
            };
            header = header.child(
                div()
                    .id(("friend-detail-toggle", gid as u64))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new(format!("friend-detail-toggle-{gid}"))
                            .label(if expanded { "⌄" } else { "›" })
                            .accessibility_label(toggle_label)
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_friend_detail(gid, cx)
                            })),
                    ),
            );
            header = header
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_friend_detail(gid, cx)));
        }

        #[cfg(test)]
        let header = header.test_support();

        let mut card = div()
            .v_flex()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .child(header);
        if let Some(gid) = gid.filter(|_| expanded) {
            if let Some((detail_gid, detail)) =
                self.friends.detail.as_ref().filter(|(id, _)| *id == gid)
            {
                let detail = div()
                    .id(format!("friend-detail-{gid}"))
                    .child(self.render_friend_detail(*detail_gid, detail, cx));
                #[cfg(test)]
                let detail = detail.test_support();
                card = card.child(detail);
            } else if self.friends.detail_loading_gid == Some(gid) {
                card = card.child(
                    div()
                        .id(format!("friend-detail-loading-{gid}"))
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .p_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(
                            language,
                            "正在加载好友农场…",
                            "Loading friend farm…",
                        )),
                );
            } else if let Some((_, error)) =
                self.friends.detail_error.as_ref().filter(|(error_gid, _)| *error_gid == gid)
            {
                let band = div()
                    .id(format!("friend-detail-error-{gid}"))
                    .aria_label(error.clone())
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .p_4()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div().text_sm().text_color(cx.theme().danger).child(error.clone()),
                            )
                            .child(
                                Button::new(format!("friend-detail-retry-{gid}"))
                                    .label(crate::i18n::text(language, "重试", "Retry"))
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_friend_detail(gid, cx)
                                    })),
                            ),
                    );
                #[cfg(test)]
                let band = band.test_support();
                card = card.child(band);
            }
        }
        card.into_any_element()
    }

    fn render_friend_interaction_row(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = &self.preferences.language;
        let count = row.record.get("count").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let sellable = row
            .record
            .get("saleConditionSatisfiedCount")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        let description =
            row.record.get("description").and_then(serde_json::Value::as_str).unwrap_or("");
        let row_element = h_flex()
            .id(format!("friend-interaction-row-{index}"))
            .items_center()
            .justify_between()
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_3()
            .child(
                h_flex()
                    .items_center()
                    .min_w_0()
                    .gap_3()
                    .child(div().font_medium().child(row.title.clone()))
                    .child(interaction_count_badge(format!("x{count}"), cx))
                    .when(sellable > 0, |line| {
                        line.child(interaction_sellable_badge(
                            crate::i18n::text(
                                language,
                                &format!("可出售: {sellable}"),
                                &format!("Sellable: {sellable}"),
                            )
                            .to_string(),
                            cx,
                        ))
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .max_w(rems(22.5))
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(description.to_string()),
            );
        #[cfg(test)]
        let row_element = row_element.test_support();
        row_element.into_any_element()
    }
}

fn interaction_count_badge(label: String, cx: &App) -> Div {
    div().rounded_full().bg(cx.theme().muted).px_2().py_1().text_xs().child(label)
}

fn interaction_sellable_badge(label: String, cx: &App) -> Div {
    div()
        .rounded_full()
        .bg(cx.theme().warning.opacity(0.16))
        .px_2()
        .py_1()
        .text_xs()
        .text_color(cx.theme().warning)
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::{friend_pet_badge, show_gold_badge};
    use serde_json::json;

    #[test]
    fn friend_gold_visibility_matches_vue_active_and_blacklist_gates() {
        assert!(show_gold_badge("friends", &json!({})));
        assert!(!show_gold_badge("blacklist", &json!({"level": 0, "gold": 0})));
        assert!(show_gold_badge("blacklist", &json!({"level": 5, "gold": 0})));
        assert!(show_gold_badge("blacklist", &json!({"level": "0", "gold": 0})));
        assert!(show_gold_badge("blacklist", &json!({"gold": 12})));
        assert!(!show_gold_badge("blacklist", &json!({"level": 5})));
    }

    #[test]
    fn friend_pet_badges_require_a_pet_object_for_non_guard_pets() {
        assert_eq!(friend_pet_badge(&json!({"petState":"other"}), "zh-CN"), None);
        assert_eq!(
            friend_pet_badge(&json!({"petState":"other","pet":{}}), "zh-CN"),
            Some((false, "宠物".into()))
        );
        assert_eq!(
            friend_pet_badge(&json!({"petState":"other","pet":{"name":"狐狸"}}), "zh-CN"),
            Some((false, "狐狸".into()))
        );
        assert_eq!(
            friend_pet_badge(&json!({"petState":"protect"}), "zh-CN"),
            Some((true, "护主犬".into()))
        );
    }
}
