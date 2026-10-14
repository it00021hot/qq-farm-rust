use super::super::super::*;

const STELLAR_TABS: [(&str, &str, &str); 4] = [
    ("season", "千星游记", "Travel Pass"),
    ("constellation", "观星礼录", "Constellation"),
    ("shop", "星砂商店", "Star Sand Shop"),
    ("solarTerms", "节令小札", "Solar Terms"),
];

fn text_value(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .filter(|value| !value.is_null())
        .map(value_to_action_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn object_text(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| text_value(value.get(*key)))
}

fn number(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_u64().map(|value| value as f64))
            .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
    })
}

fn section_stellar_tab<'a>(section: &'a str, remembered: &'a str) -> &'a str {
    if matches!(section, "season" | "constellation" | "shop" | "solarTerms") {
        section
    } else {
        remembered
    }
}

fn stellar_title(
    raw: &serde_json::Value,
    tab: &str,
    solar_term_id: &str,
    language: &str,
) -> String {
    let localized = |chinese, english| crate::i18n::text(language, chinese, english).to_string();
    match tab {
        "constellation" => object_text(
            raw.get("constellation").unwrap_or(&serde_json::Value::Null),
            &["title", "displayName"],
        )
        .unwrap_or_else(|| localized("观星礼录", "Constellation")),
        "shop" => {
            object_text(raw.get("shop").unwrap_or(&serde_json::Value::Null), &["title", "name"])
                .unwrap_or_else(|| localized("星砂商店", "Star Sand Shop"))
        }
        "solarTerms" => {
            let solar = raw.get("solarTerms").unwrap_or(&serde_json::Value::Null);
            let selected = solar
                .get("terms")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .find(|term| {
                    term.get("id").map(value_to_action_string).as_deref() == Some(solar_term_id)
                });
            selected
                .and_then(|term| object_text(term, &["title", "name"]))
                .or_else(|| object_text(solar, &["title"]))
                .unwrap_or_else(|| localized("节令小札", "Solar Terms"))
        }
        _ => object_text(raw.get("season").unwrap_or(&serde_json::Value::Null), &["title"])
            .unwrap_or_else(|| localized("千星游记", "Travel Pass")),
    }
}

fn detail_title(desktop: &FarmDesktop) -> String {
    let raw = &desktop.page_raw;
    let language = &desktop.preferences.language;
    let localized = |chinese, english| crate::i18n::text(language, chinese, english).to_string();
    let special = match desktop.page_section.as_str() {
        "qixi" => {
            object_text(raw.get("qixi").unwrap_or(&serde_json::Value::Null), &["name", "title"])
                .or_else(|| Some(localized("鹊桥寄情", "Magpie Bridge")))
        }
        "charity" => {
            object_text(raw.get("charity").unwrap_or(&serde_json::Value::Null), &["name", "title"])
                .or_else(|| Some(localized("公益小红花", "Red Flower Charity")))
        }
        "qingmei" => object_text(
            raw.get("greenPlum")
                .or_else(|| raw.get("qingMei"))
                .or_else(|| raw.get("qingmei"))
                .unwrap_or(&serde_json::Value::Null),
            &["name"],
        )
        .or_else(|| Some(localized("青梅", "Green Plum"))),
        "weather" => Some(localized("雨落成诗", "Rainfall Poems")),
        _ => None,
    };
    special.unwrap_or_else(|| {
        let tab = section_stellar_tab(&desktop.page_section, &desktop.activities.stellar_section);
        stellar_title(raw, tab, &desktop.activities.solar_term_id, language)
    })
}

fn tab_has_badge(raw: &serde_json::Value, tab: &str) -> bool {
    match tab {
        "season" => {
            let claimable = raw
                .pointer("/season/pass/nodes")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|nodes| {
                    nodes.iter().any(|node| {
                        node.get("claimable").and_then(serde_json::Value::as_bool) == Some(true)
                    })
                });
            claimable && activity_action_enabled(raw, "claimPass") == Some(true)
        }
        "constellation" => raw
            .pointer("/constellation/groups")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|group| {
                    matches!(
                        group.get("visualState").and_then(serde_json::Value::as_str),
                        Some("lightable" | "claimableUnknown")
                    )
                })
            }),
        "shop" => {
            raw.pointer("/shop/goods").and_then(serde_json::Value::as_array).is_some_and(|goods| {
                goods.iter().any(|good| {
                    good.get("exchangeable").and_then(serde_json::Value::as_bool) == Some(true)
                })
            })
        }
        "solarTerms" => raw
            .pointer("/solarTerms/terms")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|terms| {
                terms.iter().any(|term| {
                    term.get("claimable").and_then(serde_json::Value::as_bool) == Some(true)
                        || term.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true)
                })
            }),
        _ => false,
    }
}

fn end_time_millis(desktop: &FarmDesktop) -> Option<i64> {
    let raw = &desktop.page_raw;
    let section = desktop.page_section.as_str();
    if section == "weather" {
        return None;
    }
    let selected_stellar = section_stellar_tab(section, &desktop.activities.stellar_section);
    let value = match section {
        "qixi" => raw.pointer("/qixi/endTime"),
        "charity" => raw.pointer("/charity/endTime"),
        "qingmei" => raw
            .pointer("/greenPlum/endTime")
            .or_else(|| raw.pointer("/qingMei/endTime"))
            .or_else(|| raw.pointer("/qingmei/endTime")),
        _ if selected_stellar == "shop" => raw.pointer("/shop/endTime"),
        _ if selected_stellar == "constellation" => {
            raw.pointer("/constellation/endTime").or_else(|| raw.pointer("/season/endTime"))
        }
        _ if selected_stellar == "solarTerms" => {
            let selected_end = raw
                .pointer("/solarTerms/terms")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .find(|term| {
                    term.get("id").map(value_to_action_string).as_deref()
                        == Some(desktop.activities.solar_term_id.as_str())
                })
                .and_then(|term| term.get("endTime"));
            selected_end.or_else(|| raw.pointer("/season/endTime"))
        }
        _ => raw.pointer("/season/endTime"),
    };
    let millis = number(value)?;
    if millis <= 0. {
        return None;
    }
    let millis = if millis > 1_000_000_000_000. { millis } else { millis * 1000. };
    Some(millis as i64)
}

fn remaining_text(end_millis: i64, now_millis: i64, language: &str) -> String {
    let remaining = end_millis.saturating_sub(now_millis).max(0);
    if remaining == 0 {
        return crate::i18n::text(language, "活动已结束", "Event ended").into();
    }
    let days = remaining / 86_400_000;
    let hours = (remaining % 86_400_000) / 3_600_000;
    let minutes = (remaining % 3_600_000) / 60_000;
    if language == "en-US" {
        if days > 0 {
            format!("Left: {days}d {hours}h")
        } else {
            format!("Left: {hours}h {minutes}m")
        }
    } else if days > 0 {
        format!("剩余：{days}天{hours}小时")
    } else {
        format!("剩余：{hours}小时{minutes}分钟")
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_activity_detail_navigation(
        &self,
        cx: &mut Context<Self>,
    ) -> Div {
        let section = self.page_section.as_str();
        let is_stellar = matches!(section, "season" | "constellation" | "shop" | "solarTerms");
        let language = self.preferences.language.as_str();
        let title_text = detail_title(self);
        let detail_title = div()
            .id("activity-detail-title")
            .aria_label(title_text.clone())
            .font_semibold()
            .text_lg()
            .child(title_text);
        #[cfg(test)]
        let detail_title = detail_title.test_support();
        let mut header = div().v_flex().gap_3().child(
            h_flex()
                .items_center()
                .gap_3()
                .child(
                    Button::new("activity-back-to-directory")
                        .label(crate::i18n::text(language, "返回活动列表", "Back to activities"))
                        .ghost()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page_section("directory", window, cx)
                        })),
                )
                .child(detail_title)
                .child(div().flex_1())
                .child(
                    Button::new("activity-detail-refresh")
                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                        .ghost()
                        .loading(self.loading)
                        .disabled(self.loading || self.selected_account.is_none())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                ),
        );

        if let Some(end_time) = end_time_millis(self) {
            let remaining_label =
                remaining_text(end_time, chrono::Local::now().timestamp_millis(), language);
            let remaining = div()
                .id("activity-detail-remaining")
                .aria_label(remaining_label.clone())
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(remaining_label);
            #[cfg(test)]
            let remaining = remaining.test_support();
            header = header.child(remaining);
        }

        if is_stellar && matches!(section, "season" | "shop") {
            let balance_label = crate::i18n::text(language, "星砂", "Star Sand");
            let balance =
                div().id("activity-shop-balance").text_sm().text_color(cx.theme().warning).child(
                    format!(
                        "{balance_label} {}",
                        super::super::shop::balance(
                            self.page_raw.get("shop").unwrap_or(&serde_json::Value::Null)
                        )
                    ),
                );
            #[cfg(test)]
            let balance = balance.test_support();
            header = header.child(balance);
        }

        if is_stellar {
            header = header.child(h_flex().gap_2().children(
                STELLAR_TABS.into_iter().enumerate().map(|(index, (key, chinese, english))| {
                    let label = crate::i18n::text(language, chinese, english);
                    let label = if tab_has_badge(&self.page_raw, key) {
                        format!("{label} •")
                    } else {
                        label.to_string()
                    };
                    Button::new(("activity-detail-tab", index))
                        .label(label)
                        .when(section == key, |button| button.primary())
                        .when(section != key, |button| button.ghost())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_page_section(key, window, cx)
                        }))
                }),
            ));
        }
        header
    }
}

#[cfg(test)]
mod tests {
    use super::{remaining_text, tab_has_badge};
    use crate::desktop::activities::shop::balance as shop_balance;
    use serde_json::json;

    #[test]
    fn remaining_activity_time_matches_vue_day_and_hour_formats() {
        assert_eq!(remaining_text(90_000_000, 0, "zh-CN"), "剩余：1天1小时");
        assert_eq!(remaining_text(3_660_000, 0, "zh-CN"), "剩余：1小时1分钟");
        assert_eq!(remaining_text(3_660_000, 0, "en-US"), "Left: 1h 1m");
        assert_eq!(remaining_text(0, 0, "en-US"), "Event ended");
    }

    #[test]
    fn stellar_tab_badges_follow_vue_actionable_data_and_shop_balance_fallbacks() {
        let raw = json!({
            "season":{"pass":{"nodes":[{"claimable":true}]}},
            "constellation":{"groups":[{"visualState":"claimableUnknown"}]},
            "shop":{"goods":[{"exchangeable":true}],"currencies":[{"balance":12}]},
            "solarTerms":{"terms":[{"canClaim":true}]},
            "actions":{"claimPass":{"enabled":true}}
        });
        for tab in ["season", "constellation", "shop", "solarTerms"] {
            assert!(tab_has_badge(&raw, tab), "tab {tab}");
        }
        assert!(!tab_has_badge(
            &json!({
                "season":{"pass":{"nodes":[{"claimable":true}]}},
                "actions":{"claimPass":{"enabled":false}}
            }),
            "season"
        ));
        assert_eq!(shop_balance(raw.get("shop").unwrap()), "12");
        assert_eq!(shop_balance(&json!({"balanceKnown":false,"balance":12})), "--");
    }
}
