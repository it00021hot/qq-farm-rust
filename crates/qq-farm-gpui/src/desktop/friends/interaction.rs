use super::super::*;
use crate::desktop::personal::interaction_items::InteractionItemOption;

impl FarmDesktop {
    pub(in crate::desktop) fn sync_friend_interaction_picker(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let items = self
            .page_raw
            .pointer("/interactionItems/items")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let id = item.get("itemId").or_else(|| item.get("id")).and_then(|value| {
                    value.as_i64().or_else(|| value.as_str()?.trim().parse::<i64>().ok())
                })?;
                Some(InteractionItemOption {
                    id,
                    label: format!(
                        "{} x{}",
                        item.get("name").and_then(serde_json::Value::as_str).unwrap_or(
                            crate::i18n::text(
                                &self.preferences.language,
                                "互动道具",
                                "Interaction item",
                            ),
                        ),
                        item.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0)
                    ),
                })
            })
            .collect::<Vec<_>>();
        let mut selected = self
            .friends
            .selected_interaction_item_id
            .filter(|id| items.iter().any(|item| item.id == *id));
        if selected.is_none() {
            selected = items.first().map(|item| item.id);
        }
        self.friends.selected_interaction_item_id = selected;
        self.friends.interaction_picker.update(cx, |picker, cx| {
            picker.set_items(gpui_kit::component::select::SearchableVec::new(items), window, cx);
            match selected {
                Some(id) => picker.set_selected_value(&id, window, cx),
                None => picker.set_selected_index(None, window, cx),
            }
        });
    }
}

fn bulk_steal_message(
    succeeded: usize,
    skipped: usize,
    totals: &[(String, i64)],
    language: &str,
) -> String {
    let detail =
        totals.iter().map(|(name, count)| format!("{name}×{count}")).collect::<Vec<_>>().join("、");
    if language == "en-US" {
        format!(
            "Steal all done: {succeeded} ok, {skipped} skipped{}",
            if detail.is_empty() { String::new() } else { format!("：{detail}") }
        )
    } else {
        format!(
            "全部偷取完成：成功 {succeeded}，跳过 {skipped}{}",
            if detail.is_empty() { String::new() } else { format!("：{detail}") }
        )
    }
}

impl FarmDesktop {
    pub(super) fn select_friend_interaction_item(&mut self, item_id: i64, cx: &mut Context<Self>) {
        self.friends.selected_interaction_item_id = Some(item_id);
        cx.notify();
    }

    pub(super) fn run_friend_interaction_item(&mut self, cx: &mut Context<Self>) {
        if self.friends.interaction_using {
            return;
        }
        let Some(item_id) = self.friends.selected_interaction_item_id else {
            self.set_friend_feedback(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先选择一个互动道具。",
                    "Select an interaction item first.",
                ),
                FriendFeedbackLevel::Info,
                cx,
            );
            return;
        };
        let friend_gid = self.friends.interaction_gid.read(cx).value().trim().parse::<i64>().ok();
        let land_ids = self
            .friends
            .interaction_land_ids
            .read(cx)
            .value()
            .split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace())
            .filter_map(|value| value.trim().parse::<i64>().ok())
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();
        let Some(friend_gid) = friend_gid.filter(|gid| *gid > 0) else {
            self.set_friend_feedback(
                crate::i18n::text(
                    &self.preferences.language,
                    "请输入有效的好友 GID。",
                    "Enter a valid friend GID.",
                ),
                FriendFeedbackLevel::Info,
                cx,
            );
            return;
        };
        if land_ids.is_empty() {
            self.set_friend_feedback(
                crate::i18n::text(
                    &self.preferences.language,
                    "请输入至少一个有效的好友地块编号，例如 1,2,3。",
                    "Enter at least one valid friend land ID, e.g. 1,2,3.",
                ),
                FriendFeedbackLevel::Info,
                cx,
            );
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.interaction_generation = self.friends.interaction_generation.wrapping_add(1);
        let generation = self.friends.interaction_generation;
        self.friends.interaction_using = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::friend_interaction_items_use(
                        &app,
                        &worker_account_id,
                        friend_gid,
                        item_id,
                        land_ids,
                    )
                    .await
                    .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.friends.interaction_generation != generation {
                    return;
                }
                this.friends.interaction_using = false;
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Friends
                    || this.page_section != "interactionItems"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(data)) => {
                        let message = data
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .filter(|message| !message.trim().is_empty())
                            .unwrap_or(crate::i18n::text(
                                &this.preferences.language,
                                "批量使用完成",
                                "Batch use done",
                            ))
                            .to_string();
                        this.set_friend_feedback(message, FriendFeedbackLevel::Success, cx);
                        this.refresh_friend_interaction_items(cx);
                    }
                    Ok(Err(error)) => {
                        this.set_friend_feedback(error, FriendFeedbackLevel::Error, cx);
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

    pub(in crate::desktop) fn finish_bulk_steal_target(
        &mut self,
        gid: i64,
        generation: u64,
        account_id: &str,
        request_succeeded: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.friends.steal_all_generation != generation {
            return false;
        }
        if self.friends.steal_all_current_gid == Some(gid) {
            self.friends.steal_all_current_gid = None;
        }
        if request_succeeded
            && self.selected_account.as_deref() == Some(account_id)
            && self.page == Page::Friends
            && self.page_section == "friends"
        {
            self.update_friend_operation_snapshot(gid, "steal");
            self.sync_friend_detail_after_operation(gid, cx);
        }
        cx.notify();
        true
    }

    pub(super) fn steal_all_friends(&mut self, cx: &mut Context<Self>) {
        if self.friends.steal_all_loading {
            return;
        }
        let gids = stealable_friends_matching(&self.page_raw)
            .iter()
            .map(|friend| super::helpers::friend_value_number(friend.get("gid")))
            .filter(|gid| *gid > 0)
            .collect::<Vec<_>>();
        if gids.is_empty() {
            self.set_friend_feedback(
                crate::i18n::text(
                    &self.preferences.language,
                    "当前没有可偷好友",
                    "No friends with stealable crops",
                ),
                FriendFeedbackLevel::Info,
                cx,
            );
            cx.notify();
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.friends.steal_all_generation = self.friends.steal_all_generation.wrapping_add(1);
        let generation = self.friends.steal_all_generation;
        self.friends.steal_all_loading = true;
        self.friends.steal_all_current_gid = None;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut succeeded = 0usize;
            let mut skipped = 0usize;
            let mut totals: Vec<(String, i64)> = Vec::new();
            for gid in gids {
                let _ = this.update(cx, |this, cx| {
                    if this.friends.steal_all_generation == generation
                        && this.selected_account.as_deref() == Some(account_id.as_str())
                        && this.page == Page::Friends
                        && this.page_section == "friends"
                    {
                        this.friends.steal_all_current_gid = Some(gid);
                        cx.notify();
                    }
                });
                let app = Arc::clone(&app);
                let account_id_for_target = account_id.clone();
                match runtime
                    .spawn(async move {
                        qq_farm_app::friend::friend_op(&app, &account_id_for_target, gid, "steal")
                            .await
                    })
                    .await
                {
                    Ok(Ok(result)) => {
                        if super::helpers::friend_value_number(result.get("count")) > 0 {
                            succeeded += 1;
                            if let Some(items) =
                                result.get("items").and_then(serde_json::Value::as_array)
                            {
                                for item in items {
                                    let Some(name) = item
                                        .get("name")
                                        .and_then(serde_json::Value::as_str)
                                        .map(str::trim)
                                        .filter(|name| !name.is_empty())
                                    else {
                                        continue;
                                    };
                                    let count =
                                        super::helpers::friend_value_number(item.get("count"));
                                    if let Some((_, total)) =
                                        totals.iter_mut().find(|(known, _)| known == name)
                                    {
                                        *total += count;
                                    } else {
                                        totals.push((name.to_string(), count));
                                    }
                                }
                            }
                        } else {
                            skipped += 1;
                        }
                        let _ = this.update(cx, |this, cx| {
                            this.finish_bulk_steal_target(gid, generation, &account_id, true, cx);
                        });
                    }
                    Ok(Err(_)) | Err(_) => {
                        skipped += 1;
                        let _ = this.update(cx, |this, cx| {
                            this.finish_bulk_steal_target(gid, generation, &account_id, false, cx);
                        });
                    }
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.friends.steal_all_generation != generation {
                    return;
                }
                this.friends.steal_all_loading = false;
                this.friends.steal_all_current_gid = None;
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Friends
                    || this.page_section != "friends"
                {
                    cx.notify();
                    return;
                }
                let message =
                    bulk_steal_message(succeeded, skipped, &totals, &this.preferences.language);
                this.set_friend_feedback(message, FriendFeedbackLevel::Success, cx);
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::super::helpers::friend_value_number;
    use super::bulk_steal_message;

    #[test]
    fn bulk_steal_summary_includes_vue_counts_and_grouped_rewards() {
        let totals = vec![("草莓".to_string(), 4), ("玉米".to_string(), 2)];
        assert_eq!(
            bulk_steal_message(2, 1, &totals, "zh-CN"),
            "全部偷取完成：成功 2，跳过 1：草莓×4、玉米×2"
        );
        assert_eq!(bulk_steal_message(0, 3, &[], "zh-CN"), "全部偷取完成：成功 0，跳过 3");
        assert_eq!(
            bulk_steal_message(2, 1, &totals, "en-US"),
            "Steal all done: 2 ok, 1 skipped：草莓×4、玉米×2"
        );
    }

    #[test]
    fn friend_counts_accept_numeric_strings_like_the_vue_page() {
        assert_eq!(friend_value_number(Some(&serde_json::json!(3))), 3);
        assert_eq!(friend_value_number(Some(&serde_json::json!("4"))), 4);
        assert_eq!(friend_value_number(Some(&serde_json::json!("invalid"))), 0);
    }
}
