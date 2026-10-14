use super::super::super::*;

fn clear_pet_diary_friend_result(page_raw: &mut serde_json::Value) {
    if let Some(raw) = page_raw.as_object_mut() {
        raw.remove("petDiaryFriend");
        raw.remove("petDiaryFriendGid");
    }
}

fn begin_pet_diary_friend_lookup(page_raw: &mut serde_json::Value, requested_gid: &str) {
    let loaded_gid =
        page_raw.get("petDiaryFriendGid").and_then(serde_json::Value::as_str).unwrap_or_default();
    if loaded_gid != requested_gid {
        clear_pet_diary_friend_result(page_raw);
    }
}

impl FarmDesktop {
    pub(in crate::desktop::activities) fn refresh_pet_diary_friends(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.activities.pet_diary_friends_loading {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let revision = self.request_revision;
        self.activities.pet_diary_friends_request_generation =
            self.activities.pet_diary_friends_request_generation.wrapping_add(1);
        let generation = self.activities.pet_diary_friends_request_generation;
        self.activities.pet_diary_friends_loading = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::clear_friends_cache(&app, &account_id)
                        .map_err(|error| error.to_string())?;
                    let friends = qq_farm_app::friend::list_friends(&app, &account_id, false)
                        .await
                        .map_err(|error| error.to_string())?;
                    serde_json::to_value(friends).map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.pet_diary_friends_request_generation == generation {
                    this.activities.pet_diary_friends_loading = false;
                }
                if this.request_revision != revision
                    || this.page != Page::Activities
                    || this.page_section != "petDiary"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(friends)) => {
                        if let Some(root) = this.page_raw.as_object_mut() {
                            root.insert("petDiaryFriends".into(), friends);
                        }
                        this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                        if let Some(handle) = this.window_handle {
                            let _ = handle.update(cx, |_, window, cx| {
                                this.sync_pet_diary_friend_selector(window, cx);
                            });
                        }
                    }
                    Ok(Err(error)) => this.error = Some(error),
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop::activities) fn load_pet_diary_friend(&mut self, cx: &mut Context<Self>) {
        if self.activities.pet_diary_friend_loading {
            return;
        }
        let gid = self.activities.pet_diary_friend_gid.trim().to_string();
        if gid.is_empty() {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "请输入好友 GID。",
                    "Enter a friend GID.",
                )
                .into(),
            );
            cx.notify();
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let revision = self.request_revision;
        self.activities.pet_diary_friend_request_generation =
            self.activities.pet_diary_friend_request_generation.wrapping_add(1);
        let generation = self.activities.pet_diary_friend_request_generation;
        let response_gid = gid.clone();
        self.activities.pet_diary_friend_loading = true;
        self.activities.pet_diary_friend_error = None;
        self.error = None;
        if self
            .activities
            .pet_diary_battle_target
            .as_ref()
            .is_some_and(|(target_gid, _, _)| target_gid != &gid)
        {
            self.activities.pet_diary_battle_target = None;
        }
        begin_pet_diary_friend_lookup(&mut self.page_raw, &gid);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::activity::pet_diary_friend(&app, &account_id, &gid)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.pet_diary_friend_request_generation == generation {
                    this.activities.pet_diary_friend_loading = false;
                }
                if this.request_revision != revision
                    || this.page != Page::Activities
                    || this.page_section != "petDiary"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(value)) => {
                        if let Some(raw) = this.page_raw.as_object_mut() {
                            raw.insert("petDiaryFriend".into(), value);
                            raw.insert("petDiaryFriendGid".into(), response_gid.clone().into());
                        }
                        this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                    }
                    Ok(Err(error)) => {
                        clear_pet_diary_friend_result(&mut this.page_raw);
                        this.activities.pet_diary_friend_error = Some(error);
                        this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                    }
                    Err(error) => {
                        clear_pet_diary_friend_result(&mut this.page_raw);
                        this.activities.pet_diary_friend_error = Some(
                            crate::i18n::background_task_error(&this.preferences.language, error),
                        );
                        this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop::activities) fn select_pet_diary_battle_target(
        &mut self,
        treasure_id: String,
        challenge_id: String,
        cx: &mut Context<Self>,
    ) {
        let gid = self.activities.pet_diary_friend_gid.trim().to_string();
        self.activities.pet_diary_battle_target = Some((gid, treasure_id, challenge_id));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{begin_pet_diary_friend_lookup, clear_pet_diary_friend_result};
    use serde_json::json;

    #[test]
    fn repeated_lookup_keeps_the_loaded_treasure_result_visible_until_response() {
        let mut page = json!({
            "petDiaryFriendGid": "42",
            "petDiaryFriend": {"treasures": [{"id": 7}]}
        });

        begin_pet_diary_friend_lookup(&mut page, "42");

        assert_eq!(page.pointer("/petDiaryFriendGid").and_then(|gid| gid.as_str()), Some("42"));
        assert_eq!(page.pointer("/petDiaryFriend/treasures/0/id"), Some(&json!(7)));
    }

    #[test]
    fn failed_lookup_clears_stale_treasure_results() {
        let mut page = json!({
            "petDiaryFriendGid": "42",
            "petDiaryFriend": {"treasures": [{"id": 7}]}
        });

        clear_pet_diary_friend_result(&mut page);

        assert!(page.get("petDiaryFriendGid").is_none());
        assert!(page.get("petDiaryFriend").is_none());
    }
}
