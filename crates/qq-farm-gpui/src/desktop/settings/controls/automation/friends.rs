use super::super::super::super::*;

impl FarmDesktop {
    pub(super) fn automation_friend_controls(&self, cx: &mut Context<Self>) -> Div {
        let friend_enabled = self.automation_enabled("friend");
        let auto_accept_enabled = self.automation_enabled("friend_auto_accept");
        div()
            .v_flex()
            .gap_2()
            .child(self.settings_section_title(div().v_flex().gap_2(), "自动好友互动", cx))
            .when(friend_enabled, |panel| {
                panel
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_4()
                            .child(self.render_automation_switch(
                                "friend_steal",
                                "自动偷菜",
                                false,
                                cx,
                            ))
                            .child(self.render_automation_switch(
                                "friend_help",
                                "自动帮忙",
                                false,
                                cx,
                            ))
                            .child(self.render_automation_switch(
                                "friend_bad",
                                "自动捣乱",
                                false,
                                cx,
                            ))
                            .child(self.render_automation_switch(
                                "friend_help_exp_limit",
                                "经验满不帮忙",
                                false,
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_4()
                            .child(self.render_automation_switch(
                                "friend_auto_accept",
                                "好友申请自动通过",
                                false,
                                cx,
                            ))
                            .child(self.append_integer_setting_disabled(
                                h_flex().flex_1(),
                                "autoAcceptFriendMinLevel",
                                "最低等级",
                                Some(0),
                                Some(200),
                                1,
                                !auto_accept_enabled,
                                cx,
                            ))
                            .child(self.append_setting_switch(
                                h_flex().flex_1(),
                                "autoAcceptRequireOwnLevel",
                                "不低于自己等级",
                                !auto_accept_enabled,
                                cx,
                            ))
                            .child(self.append_setting_switch(
                                h_flex().flex_1(),
                                "autoAcceptHarvestStealEnabled",
                                "收偷比过滤",
                                !auto_accept_enabled,
                                cx,
                            )),
                    )
                    .child({
                        let hint = settings_text(&self.preferences.language, "0 = 不限");
                        div()
                            .id("auto-accept-min-level-hint")
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .aria_label(hint.clone())
                            .child(hint)
                            .test_support()
                    })
                    .when(self.setting_enabled("autoAcceptHarvestStealEnabled"), |panel| {
                        panel.child(
                            h_flex()
                                .flex_wrap()
                                .gap_4()
                                .child(self.append_integer_setting_disabled(
                                    h_flex().flex_1(),
                                    "autoAcceptHarvestStealHarvest",
                                    "收获份数",
                                    Some(0),
                                    Some(9999),
                                    1,
                                    !auto_accept_enabled,
                                    cx,
                                ))
                                .child(self.append_integer_setting_disabled(
                                    h_flex().flex_1(),
                                    "autoAcceptHarvestStealSteal",
                                    "偷菜份数",
                                    Some(1),
                                    Some(9999),
                                    1,
                                    !auto_accept_enabled,
                                    cx,
                                )),
                        )
                    })
                    .child({
                        let hint = crate::i18n::text(
                            &self.preferences.language,
                            "开启后自动通过好友申请；可按等级与生涯收偷比过滤（收偷比 = 偷菜份数 / 收获份数）。",
                            "Friend requests are accepted automatically; filter by level and career steal/harvest ratio (ratio = steal / harvest).",
                        )
                        .to_string();
                        div()
                            .id("settings-auto-accept-hint")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .aria_label(hint.clone())
                            .child(hint)
                            .test_support()
                    })
            })
    }
}
