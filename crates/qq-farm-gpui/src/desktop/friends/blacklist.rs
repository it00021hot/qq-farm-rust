use super::super::*;

impl FarmDesktop {
    pub(super) fn toggle_friend_blacklist(&mut self, gid: i64, cx: &mut Context<Self>) {
        if self.friends.blacklist_loading {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.blacklist_generation = self.friends.blacklist_generation.wrapping_add(1);
        let generation = self.friends.blacklist_generation;
        self.friends.blacklist_loading = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::toggle_friend_blacklist(&worker_account_id, gid);
                    qq_farm_app::friend::friend_blacklist(&worker_account_id)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.friends.blacklist_generation != generation {
                    return;
                }
                this.friends.blacklist_loading = false;
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Friends
                    || this.page_section != "friends"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(_) => {
                        this.set_friend_feedback(
                            crate::i18n::text(
                                &this.preferences.language,
                                "黑名单已更新",
                                "Blacklist updated",
                            ),
                            FriendFeedbackLevel::Success,
                            cx,
                        );
                        this.refresh(cx);
                    }
                    Err(error) => this.set_friend_feedback(
                        if this.preferences.language == "en-US" {
                            format!("Background task ended: {error}")
                        } else {
                            format!("后台任务已结束：{error}")
                        },
                        FriendFeedbackLevel::Error,
                        cx,
                    ),
                }
            });
        })
        .detach();
    }
}
