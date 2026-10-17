use super::super::*;

impl FarmDesktop {
    pub(super) fn render_page_sections(&self, cx: &mut Context<Self>) -> Div {
        let options = page_sections(self.page);
        if options.is_empty() {
            return div();
        }
        h_flex().gap_1().px_6().py_2().border_b_1().border_color(cx.theme().border).children(
            options.iter().enumerate().map(|(index, (key, label))| {
                let label =
                    self.page.section_title_for(key, &self.preferences.language).unwrap_or(label);
                let badge = match (self.page, *key) {
                    (Page::Analytics, "plantBlacklist") => Some((
                        self.page_raw
                            .get("plantBlacklist")
                            .and_then(serde_json::Value::as_array)
                            .map_or(0, Vec::len),
                        "analytics-blacklist-tab-count",
                    )),
                    (Page::Friends, "blacklist") => Some((
                        self.page_raw
                            .get("blacklist")
                            .and_then(serde_json::Value::as_array)
                            .map_or(0, Vec::len),
                        "friends-blacklist-tab-count",
                    )),
                    _ => None,
                };
                let button = Button::new(("page-section", index))
                    .label(label)
                    .when(self.page_section == *key, |button| button.primary())
                    .when(self.page_section != *key, |button| button.ghost());
                let button = match badge {
                    Some((count, id)) if count > 0 => {
                        button.accessibility_label(format!("{label}, {count}")).child(
                            div()
                                .id(id)
                                .aria_label(count.to_string())
                                .test_support()
                                .rounded_full()
                                .bg(cx.theme().danger.opacity(0.12))
                                .px_1p5()
                                .py_0p5()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(count.to_string()),
                        )
                    }
                    _ => button,
                };
                button.on_click(
                    cx.listener(move |this, _, window, cx| this.set_page_section(key, window, cx)),
                )
            }),
        )
    }
    pub(super) fn render_page_controls(&self, cx: &mut Context<Self>) -> Div {
        // Settings tabs mix label/control blocks with tall section cards; top
        // alignment keeps every block on the same reading line instead of
        // floating the shorter ones half-way down the tallest card. Toolbars on
        // other pages keep the centered default so their buttons and labels
        // stay on one baseline.
        let mut controls = h_flex()
            .gap_2()
            .flex_wrap()
            .when(self.page == Page::Settings, |controls| controls.items_start());
        controls = self.append_dashboard_controls(controls, cx);
        controls = self.append_personal_controls(controls, cx);
        controls = self.append_friend_controls(controls, cx);
        controls = self.append_settings_controls(controls, cx);
        controls = self.append_game_config_controls(controls, cx);
        div().px_6().py_2().v_flex().gap_2().child(controls)
    }
}
