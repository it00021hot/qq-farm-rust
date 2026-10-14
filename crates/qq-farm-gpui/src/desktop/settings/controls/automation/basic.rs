use super::super::super::super::*;

impl FarmDesktop {
    pub(super) fn automation_basics(&self, cx: &mut Context<Self>) -> Div {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .font_semibold()
                    .child(settings_text(&self.preferences.language, "基础自动化")),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_4()
                    .child(self.render_automation_switch("farm", "自动种植收获", false, cx))
                    .child(self.render_automation_switch(
                        "farm_push",
                        "推送触发巡田",
                        !self.automation_enabled("farm"),
                        cx,
                    ))
                    .child(self.render_automation_switch("land_upgrade", "自动升级土地", false, cx))
                    .child(self.render_automation_switch("task", "自动做任务", false, cx))
                    .child(self.render_automation_switch("sell", "自动卖果实", false, cx))
                    .child(self.render_automation_switch("friend", "自动好友互动", false, cx))
                    .child(self.render_automation_switch(
                        "skip_own_weed_bug",
                        "巡田时跳过一键务农",
                        false,
                        cx,
                    )),
            )
    }
}
