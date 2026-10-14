use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_friend_feedback(
        &mut self,
        message: impl Into<String>,
        level: FriendFeedbackLevel,
        cx: &mut Context<Self>,
    ) {
        self.friends.feedback_generation = self.friends.feedback_generation.wrapping_add(1);
        let generation = self.friends.feedback_generation;
        self.friends.feedback = Some((message.into(), level));
        cx.notify();

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.friends.feedback_generation == generation {
                    this.friends.feedback = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
