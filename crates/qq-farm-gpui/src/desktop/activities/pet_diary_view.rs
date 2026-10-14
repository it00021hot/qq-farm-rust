mod battle;
mod charms;
mod common;
pub(super) mod friend_picker;
mod hunt;
mod logs;
mod nurture;
mod rewards;
mod summary;

use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn render_pet_diary_panel(&self, cx: &mut Context<Self>) -> Div {
        let snapshot = self.page_raw.get("petDiary").unwrap_or(&serde_json::Value::Null);
        if snapshot.is_null() {
            let mut panel = div().v_flex().gap_3().child(summary::render(self, snapshot, cx));
            if let Some(error) = &self.error {
                let error_panel = div()
                    .id("pet-diary-error")
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone());
                #[cfg(test)]
                let error_panel = error_panel.test_support();
                panel = panel.child(error_panel);
            }
            return panel;
        }

        div()
            .v_flex()
            .gap_3()
            .child(summary::render(self, snapshot, cx))
            .when_some(self.activities.pet_diary_notice.as_ref(), |panel, notice| {
                panel.child(
                    div()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().success)
                        .px_4()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().success_foreground)
                        .child(notice.clone()),
                )
            })
            .when_some(self.activities.pet_diary_warning.as_ref(), |panel, warning| {
                panel.child(
                    div()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().group_box)
                        .px_4()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().warning)
                        .child(warning.clone()),
                )
            })
            .child(nurture::render(self, snapshot, cx))
            .child(hunt::render(self, snapshot, cx))
            .child(charms::render(self, snapshot, cx))
            .child(battle::render(self, snapshot, cx))
            .child(rewards::seeds(self, snapshot, cx))
            .child(rewards::shop(self, snapshot, cx))
            .child(rewards::stories(self, snapshot, cx))
            .child(rewards::solar_terms(self, snapshot, cx))
            .child(logs::render(self, cx))
    }
}
