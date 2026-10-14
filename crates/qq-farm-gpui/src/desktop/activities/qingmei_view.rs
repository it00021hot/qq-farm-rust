mod brew;
mod ingredients;
mod shared;
mod summary;

use super::super::*;

impl FarmDesktop {
    pub(crate) fn render_qingmei_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let activity = self
            .page_raw
            .get("qingMei")
            .or_else(|| self.page_raw.get("greenPlum"))
            .or_else(|| self.page_raw.get("qingmei"));
        let Some(activity) = activity.filter(|value| value.is_object()) else {
            return div()
                .v_flex()
                .items_center()
                .gap_3()
                .py_12()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(
                    language,
                    "青梅酿造活动数据尚未加载",
                    "Green plum brewing data has not loaded yet.",
                ))
                .child(
                    Button::new("qingmei-refresh")
                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                );
        };

        div()
            .v_flex()
            .gap_4()
            .child(summary::header(activity, language, cx))
            .child(summary::seed_balance(self, activity, cx))
            .child(brew::render(self, activity, cx))
            .child(shared::rules(activity, language, cx))
    }
}
