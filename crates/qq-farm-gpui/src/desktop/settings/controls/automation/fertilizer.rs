use super::super::super::super::*;

impl FarmDesktop {
    pub(super) fn automation_fertilizer_controls(&self, cx: &mut Context<Self>) -> Div {
        div()
            .v_flex()
            .gap_2()
            .child(self.settings_section_title(div().v_flex().gap_2(), "施肥与购买", cx))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_4()
                    .child(self.render_automation_switch(
                        "fertilizer_gift",
                        "自动填充化肥",
                        false,
                        cx,
                    ))
                    .child(self.render_automation_switch(
                        "fertilizer_buy_organic",
                        "自动购买有机化肥",
                        false,
                        cx,
                    ))
                    .child(self.render_automation_switch(
                        "fertilizer_buy_normal",
                        "自动购买无机化肥",
                        false,
                        cx,
                    ))
                    .child(self.render_automation_switch(
                        "fertilizer_multi_season",
                        "多季补肥",
                        false,
                        cx,
                    )),
            )
            .child(div().text_sm().child(settings_text(&self.preferences.language, "施肥范围")))
            .child(
                h_flex().flex_wrap().gap_3().children(
                    [
                        ("purple-gold", "紫金土地"),
                        ("gold", "金土地"),
                        ("black", "黑土地"),
                        ("red", "红土地"),
                        ("normal", "普通土地"),
                    ]
                    .into_iter()
                    .map(|(item, label)| {
                        self.append_setting_checkbox(
                            h_flex().gap_2(),
                            "automation/fertilizer_land_types",
                            item,
                            label,
                            false,
                            cx,
                        )
                    }),
                ),
            )
            .child({
                let hint = crate::i18n::text(
                    &self.preferences.language,
                    "施肥前会优先按土地类型过滤，仅对命中范围的地块执行施肥策略。",
                    "Fertilizer runs only on lands matching the selected types.",
                )
                .to_string();
                div()
                    .id("fertilizer-land-types-hint")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .aria_label(hint.clone())
                    .child(hint)
                    .test_support()
            })
            .child(div().text_sm().child(settings_text(&self.preferences.language, "施肥策略")))
            .child(
                h_flex().flex_wrap().gap_1().children(
                    [
                        ("both", "普通 + 有机"),
                        ("smart", "智能"),
                        ("normal", "普通"),
                        ("organic", "有机"),
                        ("none", "不施肥"),
                    ]
                    .into_iter()
                    .map(|(value, label)| {
                        let selected = self
                            .automation_value()
                            .get("fertilizer")
                            .and_then(serde_json::Value::as_str)
                            == Some(value);
                        Button::new(format!("fertilizer-mode-{value}"))
                            .label(settings_text(&self.preferences.language, label))
                            .when(selected, |button| button.primary())
                            .when(!selected, |button| button.ghost())
                            .disabled(self.loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_setting_path_value(
                                    "automation/fertilizer",
                                    serde_json::json!(value),
                                    cx,
                                )
                            }))
                    }),
                ),
            )
            .when(
                self.automation_value().get("fertilizer").and_then(serde_json::Value::as_str)
                    == Some("both"),
                |panel| {
                    let hint = crate::i18n::text(
                        &self.preferences.language,
                        "每轮巡田：所有还能施无机肥的地各打 1 次；然后所有未成熟作物循环打有机肥直到成熟或肥料用尽。",
                        "Each farm tick: apply inorganic once to every land that can take it, then loop organic on all immature crops until mature or fertilizer runs out.",
                    )
                    .to_string();
                    panel.child(
                        div()
                            .id("fertilizer-both-hint")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .aria_label(hint.clone())
                            .child(hint)
                            .test_support(),
                    )
                },
            )
            .when(
                self.automation_value().get("fertilizer").and_then(serde_json::Value::as_str)
                    == Some("smart"),
                |panel| {
                    let hint = crate::i18n::text(
                        &self.preferences.language,
                        "距离成熟时间 ≤ 此秒数时施有机肥（默认300秒=5分钟）",
                        "Apply organic when time-to-mature ≤ this many seconds (default 300 seconds).",
                    )
                    .to_string();
                    panel
                        .child(self.append_integer_setting(
                            h_flex().flex_1(),
                            "automation/fertilizer_smart_seconds",
                            "快成熟判定秒数",
                            Some(30),
                            Some(3600),
                            30,
                            cx,
                        ))
                        .child(
                            div()
                                .id("fertilizer-smart-seconds-hint")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .aria_label(hint.clone())
                                .child(hint)
                                .test_support(),
                        )
                },
            )
            .when(self.automation_enabled("fertilizer_buy_organic"), |panel| {
                panel
                    .child(self.append_integer_setting(
                        h_flex().flex_1(),
                        "fertilizerBuyOrganicCount",
                        "购买数量",
                        Some(1),
                        Some(10_000),
                        1,
                        cx,
                    ))
                    .child(self.append_integer_setting(
                        h_flex().flex_1(),
                        "fertilizerBuyOrganicThresholdHours",
                        "触发阈值 (小时)",
                        Some(1),
                        Some(990),
                        1,
                        cx,
                    ))
            })
            .when(self.automation_enabled("fertilizer_buy_normal"), |panel| {
                panel
                    .child(self.append_integer_setting(
                        h_flex().flex_1(),
                        "fertilizerBuyNormalCount",
                        "购买数量",
                        Some(1),
                        Some(10_000),
                        1,
                        cx,
                    ))
                    .child(self.append_integer_setting(
                        h_flex().flex_1(),
                        "fertilizerBuyNormalThresholdHours",
                        "触发阈值 (小时)",
                        Some(1),
                        Some(990),
                        1,
                        cx,
                    ))
            })
            .when(
                self.automation_enabled("fertilizer_buy_organic")
                    || self.automation_enabled("fertilizer_buy_normal"),
                |panel| {
                    let hint = crate::i18n::text(
                        &self.preferences.language,
                        "有机/无机同一套机制：施肥后立即检测余量，低于触发阈值当场补购。登录和保存设置时也会各检测一次。双开时优先买有机化肥。",
                        "Organic and inorganic share the same flow: after fertilizing, remaining stock is checked and bought below threshold. Login and saving each trigger one check. When both are on, organic is preferred.",
                    )
                    .to_string();
                    panel.child(
                        div()
                            .id("fertilizer-buy-hint")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .aria_label(hint.clone())
                            .child(hint)
                            .test_support(),
                    )
                },
            )
    }
}
