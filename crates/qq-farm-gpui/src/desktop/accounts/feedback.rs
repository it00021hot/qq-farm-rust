use super::super::*;
use std::time::Duration;

impl FarmDesktop {
    pub(in crate::desktop) fn show_account_success(
        &mut self,
        chinese: &'static str,
        english: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.accounts.feedback_generation = self.accounts.feedback_generation.wrapping_add(1);
        let generation = self.accounts.feedback_generation;
        self.accounts.success_feedback =
            Some(crate::i18n::text(&self.preferences.language, chinese, english).to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.accounts.feedback_generation == generation {
                    this.accounts.success_feedback = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
