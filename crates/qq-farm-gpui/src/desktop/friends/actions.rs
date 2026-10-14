use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn update_friend_operation_snapshot(
        &mut self,
        gid: i64,
        operation: &str,
    ) {
        if let Some(friends) =
            self.page_raw.get_mut("friends").and_then(serde_json::Value::as_array_mut)
        {
            for friend in friends {
                super::helpers::apply_friend_operation_snapshot(friend, gid, operation);
            }
        }
        let plant = self
            .page_raw
            .get("friends")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .find(|friend| super::helpers::friend_number_at(friend, "/gid") == gid)
            .and_then(|friend| friend.get("plant"))
            .cloned();
        for row in self.rows.iter_mut().filter(|row| row.section == "friends") {
            if super::helpers::friend_number_at(&row.record, "/gid") != gid {
                continue;
            }
            if let Some(plant) = &plant {
                row.record["plant"] = plant.clone();
            }
            row.state =
                super::helpers::friend_status_localized(&row.record, &self.preferences.language);
        }
        crate::pages::sort_friend_rows(&mut self.rows);
    }

    pub(super) fn sync_friend_detail_after_operation(&mut self, gid: i64, cx: &mut Context<Self>) {
        if self.friends.detail.as_ref().is_some_and(|(open_gid, _)| *open_gid == gid)
            || self.friends.detail_loading_gid == Some(gid)
        {
            self.open_friend_detail(gid, cx);
        }
    }

    pub(in crate::desktop) fn confirm_friend_action(
        &mut self,
        gid: i64,
        action: &str,
        cx: &mut Context<Self>,
    ) {
        match action {
            "bad" => self.run_friend_action(gid, "bad", cx),
            "blacklist" | "unblacklist" => self.toggle_friend_blacklist(gid, cx),
            "delete" => self.delete_friend(gid, cx),
            _ => {
                self.error = Some(
                    crate::i18n::text(
                        &self.preferences.language,
                        "未知好友操作",
                        "Unknown friend action",
                    )
                    .into(),
                );
                cx.notify();
            }
        }
    }

    pub(in crate::desktop) fn invalidate_friend_operations(&mut self) {
        self.friends.realtime_refresh_generation =
            self.friends.realtime_refresh_generation.wrapping_add(1);
        self.friends.operation_generation = self.friends.operation_generation.wrapping_add(1);
        self.friends.operation_loading.clear();
        self.friends.steal_all_generation = self.friends.steal_all_generation.wrapping_add(1);
        self.friends.steal_all_loading = false;
        self.friends.steal_all_current_gid = None;
        self.friends.blacklist_generation = self.friends.blacklist_generation.wrapping_add(1);
        self.friends.blacklist_loading = false;
        self.friends.delete_generation = self.friends.delete_generation.wrapping_add(1);
        self.friends.delete_loading.clear();
        self.friends.feedback_generation = self.friends.feedback_generation.wrapping_add(1);
        self.friends.feedback = None;
        self.friends.interaction_generation = self.friends.interaction_generation.wrapping_add(1);
        self.friends.interaction_using = false;
        self.friends.records_refresh_generation =
            self.friends.records_refresh_generation.wrapping_add(1);
        self.friends.records_refresh_loading = false;
        self.friends.interaction_items_refresh_generation =
            self.friends.interaction_items_refresh_generation.wrapping_add(1);
        self.friends.interaction_items_refresh_loading = false;
    }

    pub(super) fn run_friend_action(&mut self, gid: i64, op: &'static str, cx: &mut Context<Self>) {
        if self.page != Page::Friends || self.page_section != "friends" {
            return;
        }
        if op == "steal" && self.friends.steal_all_current_gid == Some(gid) {
            return;
        }
        let key = (gid, op);
        if self.friends.operation_loading.contains_key(&key) {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.operation_generation = self.friends.operation_generation.wrapping_add(1);
        let generation = self.friends.operation_generation;
        self.friends.operation_loading.insert(key, generation);
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::friend_op(&app, &worker_account_id, gid, op).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.friends.operation_loading.get(&key) != Some(&generation) {
                    return;
                }
                this.friends.operation_loading.remove(&key);
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Friends
                    || this.page_section != "friends"
                {
                    cx.notify();
                    return;
                }

                match result {
                    Ok(Ok(data)) => {
                        let count = super::helpers::friend_value_number(data.get("count"));
                        if count > 0 {
                            let summary = data
                                .get("summary")
                                .or_else(|| data.get("helpSummary"))
                                .and_then(serde_json::Value::as_str)
                                .map(str::trim)
                                .filter(|summary| !summary.is_empty())
                                .unwrap_or(crate::i18n::text(
                                    &this.preferences.language,
                                    "操作成功",
                                    "Operation succeeded",
                                ));
                            this.set_friend_feedback(
                                summary.to_string(),
                                FriendFeedbackLevel::Success,
                                cx,
                            );
                        } else {
                            this.set_friend_feedback(
                                crate::i18n::text(
                                    &this.preferences.language,
                                    if op == "steal" { "无可偷作物" } else { "无可操作" },
                                    if op == "steal" {
                                        "No crops to steal"
                                    } else {
                                        "Nothing to do"
                                    },
                                ),
                                FriendFeedbackLevel::Info,
                                cx,
                            );
                        }
                        this.update_friend_operation_snapshot(gid, op);
                        this.sync_friend_detail_after_operation(gid, cx);
                    }
                    Ok(Err(error)) => {
                        this.set_friend_feedback(error.to_string(), FriendFeedbackLevel::Error, cx);
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
