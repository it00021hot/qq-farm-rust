use super::super::*;

fn remove_pending_delete(
    delete_loading: &mut std::collections::HashMap<i64, u64>,
    gid: i64,
    generation: u64,
) -> bool {
    if delete_loading.get(&gid) != Some(&generation) {
        return false;
    }
    delete_loading.remove(&gid);
    true
}

impl FarmDesktop {
    pub(in crate::desktop) fn delete_friend(&mut self, gid: i64, cx: &mut Context<Self>) {
        if self.friends.delete_loading.contains_key(&gid) {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let name = self
            .page_raw
            .get("friends")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .find(|friend| friend.get("gid").and_then(serde_json::Value::as_i64) == Some(gid))
            .and_then(|friend| friend.get("nickname").and_then(serde_json::Value::as_str))
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(crate::i18n::text(&self.preferences.language, "好友", "Friend"))
            .to_string();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.delete_generation = self.friends.delete_generation.wrapping_add(1);
        let generation = self.friends.delete_generation;
        self.friends.delete_loading.insert(gid, generation);
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::delete_friend(&app, &worker_account_id, gid)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !remove_pending_delete(&mut this.friends.delete_loading, gid, generation) {
                    return;
                }
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Friends
                    || this.page_section != "friends"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(())) => {
                        this.set_friend_feedback(
                            if this.preferences.language == "en-US" {
                                format!("Deleted friend \"{name}\"")
                            } else {
                                format!("已删除好友「{name}」")
                            },
                            FriendFeedbackLevel::Success,
                            cx,
                        );
                        this.refresh(cx);
                    }
                    Ok(Err(error)) => {
                        this.set_friend_feedback(error, FriendFeedbackLevel::Error, cx);
                    }
                    Err(error) => {
                        this.set_friend_feedback(
                            if this.preferences.language == "en-US" {
                                format!("Background task ended: {error}")
                            } else {
                                format!("后台任务已结束：{error}")
                            },
                            FriendFeedbackLevel::Error,
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::remove_pending_delete;
    use std::collections::HashMap;

    #[test]
    fn concurrent_delete_completions_clear_only_their_own_request() {
        let mut pending = HashMap::from([(123, 1), (456, 2)]);

        assert!(remove_pending_delete(&mut pending, 123, 1));
        assert_eq!(pending, HashMap::from([(456, 2)]));
        assert!(!remove_pending_delete(&mut pending, 456, 1));
        assert_eq!(pending, HashMap::from([(456, 2)]));
        assert!(remove_pending_delete(&mut pending, 456, 2));
        assert!(pending.is_empty());
    }

    #[test]
    fn invalidated_or_replaced_delete_requests_ignore_stale_completions() {
        let mut pending = HashMap::from([(123, 3)]);
        assert!(!remove_pending_delete(&mut pending, 123, 2));
        assert_eq!(pending, HashMap::from([(123, 3)]));

        pending.clear();
        assert!(!remove_pending_delete(&mut pending, 123, 3));
        assert!(pending.is_empty());
    }
}
