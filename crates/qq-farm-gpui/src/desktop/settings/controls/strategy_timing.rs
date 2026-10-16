use super::super::super::*;

use gpui_kit::component::switch::Switch;
use gpui_kit::component::time_field::TimeField;

impl FarmDesktop {
    pub(super) fn append_strategy_timing_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        controls = controls.child({
            let interval_row = |path: &'static str,
                                label: &'static str,
                                cx: &mut Context<FarmDesktop>| {
                self.append_integer_setting(h_flex().w_full(), path, label, Some(1), None, 1, cx)
            };
            let intervals = div()
                .id("settings-intervals-card")
                .v_flex()
                .gap_2()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .p_4()
                .child(
                    div()
                        .font_semibold()
                        .child(settings_text(&self.preferences.language, "巡田间隔（秒）")),
                )
                .child(
                    // 两列网格（对齐 Tauri sm:grid-cols-2 max-w-640）
                    div()
                        .id("settings-intervals-grid")
                        .grid()
                        .grid_cols(2)
                        .max_w(px(640.))
                        .gap_x_6()
                        .gap_y_2()
                        .child(interval_row("intervals/farmMin", "农场巡查最小", cx))
                        .child(interval_row("intervals/farmMax", "农场巡查最大", cx))
                        .child(interval_row("intervals/helpMin", "帮助巡查最小", cx))
                        .child(interval_row("intervals/helpMax", "帮助巡查最大", cx))
                        .child(interval_row("intervals/stealMin", "偷菜巡查最小", cx))
                        .child(interval_row("intervals/stealMax", "偷菜巡查最大", cx)),
                );
            #[cfg(test)]
            let intervals = intervals.test_support();
            intervals
        });

        let quiet_hours_enabled = self
            .settings
            .draft
            .pointer("/friendQuietHours/enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let quiet_hours = div()
            .id("settings-quiet-hours-card")
            .v_flex()
            .gap_2()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                div()
                    .font_semibold()
                    .child(settings_text(&self.preferences.language, "好友静默时段")),
            )
            .child(
                Switch::new("quiet-hours-enabled")
                    .label(settings_text(&self.preferences.language, "启用静默时段"))
                    .checked(quiet_hours_enabled)
                    .disabled(self.loading)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.set_setting_path_value(
                            "friendQuietHours/enabled",
                            serde_json::json!(*checked),
                            cx,
                        )
                    })),
            )
            .child(self.append_time_setting(
                h_flex().flex_1(),
                "friendQuietHours/start",
                "开始",
                !quiet_hours_enabled,
            ))
            .child(self.append_time_setting(
                h_flex().flex_1(),
                "friendQuietHours/end",
                "结束",
                !quiet_hours_enabled,
            ))
            .child(
                Switch::new("quiet-hours-continue-farm")
                    .label(settings_text(&self.preferences.language, "静默时继续农场巡查"))
                    .checked(
                        self.settings
                            .draft
                            .pointer("/friendQuietHours/continueFarm")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true),
                    )
                    .disabled(!quiet_hours_enabled || self.loading)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.set_setting_path_value(
                            "friendQuietHours/continueFarm",
                            serde_json::json!(*checked),
                            cx,
                        )
                    })),
            );
        #[cfg(test)]
        let quiet_hours = quiet_hours.test_support();
        controls = controls.child(quiet_hours);

        controls = controls.child({
            let delay = div()
                .id("settings-plant-delay-card")
                .v_flex()
                .gap_2()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .p_4()
                .child(
                    div()
                        .font_semibold()
                        .child(settings_text(&self.preferences.language, "种植与偷菜延迟设置")),
                )
                .child(
                    Switch::new("plant-order-random")
                        .label(settings_text(&self.preferences.language, "种植顺序随机"))
                        .checked(
                            self.settings
                                .draft
                                .get("plantOrderRandom")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(true),
                        )
                        .disabled(self.loading)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.set_setting_path_value(
                                "plantOrderRandom",
                                serde_json::json!(*checked),
                                cx,
                            )
                        })),
                )
                .child(self.append_integer_setting(
                    h_flex().flex_1(),
                    "plantDelaySeconds",
                    "种植延迟 (秒)",
                    Some(0),
                    Some(60),
                    1,
                    cx,
                ))
                .child(self.append_integer_setting(
                    h_flex().flex_1(),
                    "stealDelaySeconds",
                    "偷菜延迟 (秒)",
                    Some(0),
                    Some(60),
                    1,
                    cx,
                ));
            #[cfg(test)]
            let delay = delay.test_support();
            delay
        });
        controls
    }

    fn append_time_setting(
        &self,
        mut row: Div,
        path: &'static str,
        label: &'static str,
        disabled: bool,
    ) -> Div {
        let (field_id, field) = match path {
            "friendQuietHours/start" => {
                ("quiet-hours-start-field", &self.settings.quiet_hours_start)
            }
            "friendQuietHours/end" => ("quiet-hours-end-field", &self.settings.quiet_hours_end),
            _ => return row,
        };
        row =
            row.items_center()
                .justify_between()
                .max_w(px(640.))
                .gap_3()
                .child(div().text_sm().child(settings_text(&self.preferences.language, label)))
                .child(
                    div().id(field_id).test_support().child(
                        TimeField::new(field).disabled(disabled || self.loading).w(px(112.)),
                    ),
                );
        row
    }
}
