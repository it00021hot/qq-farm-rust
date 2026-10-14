mod happy;
mod shared;
mod wish;
pub(super) use wish::selected_autumn_choice;

use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn render_autumn_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mode = if self.activities.autumn_mode == "happy" { "happy" } else { "wish" };
        let activity = self.page_raw.pointer(&format!("/autumn/{mode}"));
        let Some(activity) = activity.filter(|value| value.is_object()) else {
            return div()
                .v_flex()
                .items_center()
                .gap_3()
                .py_12()
                .text_color(cx.theme().muted_foreground)
                .child(crate::i18n::text(
                    &self.preferences.language,
                    "活动动态状态未返回",
                    "Activity state is unavailable",
                ))
                .child(
                    Button::new("autumn-refresh")
                        .label(crate::i18n::text(&self.preferences.language, "刷新", "Refresh"))
                        .ghost()
                        .loading(self.loading)
                        .disabled(self.selected_account.is_none() || self.loading)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.load_activity_section("autumn", cx)),
                        ),
                );
        };

        div()
            .v_flex()
            .gap_4()
            .child(
                h_flex()
                    .items_start()
                    .gap_2()
                    .child(div().flex_1().child(shared::header(
                        activity,
                        mode,
                        &self.preferences.language,
                        cx,
                    )))
                    .child(
                        Button::new("autumn-refresh")
                            .label(crate::i18n::text(&self.preferences.language, "刷新", "Refresh"))
                            .ghost()
                            .loading(self.loading)
                            .disabled(self.selected_account.is_none() || self.loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_activity_section("autumn", cx)
                            })),
                    ),
            )
            .child(shared::rules(activity, cx))
            .child(if mode == "happy" {
                happy::render(self, activity, cx)
            } else {
                wish::render(self, activity, window, cx)
            })
    }
}
