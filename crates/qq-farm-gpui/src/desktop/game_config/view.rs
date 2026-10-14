use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn render_game_config_overlays(&self, cx: &mut Context<Self>) -> Div {
        div().when_some(self.game_config.success_feedback.as_ref(), |panel, message| {
            let feedback = div()
                .id("game-config-success-feedback")
                .rounded(cx.theme().radius)
                .bg(cx.theme().group_box)
                .px_4()
                .py_3()
                .text_color(cx.theme().success)
                .child(message.clone());
            #[cfg(test)]
            let feedback = feedback.test_support();
            panel.child(feedback)
        })
    }
}
