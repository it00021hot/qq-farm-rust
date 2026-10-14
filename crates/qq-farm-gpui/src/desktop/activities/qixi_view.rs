mod gift;
mod shared;
mod stages;
mod summary;

use super::super::*;

impl FarmDesktop {
    pub(crate) fn render_qixi_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let Some(activity) = self.page_raw.get("qixi").filter(|value| value.is_object()) else {
            return div()
                .v_flex()
                .items_center()
                .gap_3()
                .py_12()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(
                    language,
                    "七夕活动数据尚未加载",
                    "Magpie Bridge data has not loaded",
                ))
                .child(
                    Button::new("qixi-refresh")
                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                );
        };

        div()
            .v_flex()
            .gap_4()
            .child(summary::balances(activity, language, cx))
            .child(stages::render(self, activity, cx))
            .child(gift::render(self, activity, window, cx))
            .child(shared::rules(activity, self.activities.qixi_rules_expanded, language, cx))
    }
}
