use super::super::*;
use std::time::Duration;

impl FarmDesktop {
    pub(in crate::desktop) fn show_game_config_success(
        &mut self,
        chinese: &'static str,
        english: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.game_config.feedback_generation = self.game_config.feedback_generation.wrapping_add(1);
        let generation = self.game_config.feedback_generation;
        self.game_config.success_feedback =
            Some(crate::i18n::text(&self.preferences.language, chinese, english).to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.game_config.feedback_generation == generation {
                    this.game_config.success_feedback = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
