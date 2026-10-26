use super::super::*;

impl FarmDesktop {
    /// 好友操作结果统一走全局 toast（此前是页头内联提示，与全局反馈机制割裂）。
    pub(in crate::desktop) fn set_friend_feedback(
        &mut self,
        message: impl Into<String>,
        level: FriendFeedbackLevel,
        cx: &mut Context<Self>,
    ) {
        let success = !matches!(level, FriendFeedbackLevel::Error);
        self.show_toast(message, success, cx);
    }
}
