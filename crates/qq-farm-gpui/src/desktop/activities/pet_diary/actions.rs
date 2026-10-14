use super::super::super::*;

fn success_message(action: &str, value: &serde_json::Value, language: &str) -> String {
    let message = match action {
        "initialize" => crate::i18n::text(language, "领养成功", "Adoption complete"),
        "feed" => crate::i18n::text(language, "投喂成功", "Feeding complete"),
        "claimDog" => crate::i18n::text(language, "已领取永久比熊", "Permanent bichon claimed"),
        "draw" => crate::i18n::text(language, "寻宝成功", "Treasure hunt complete"),
        "openTreasure" => crate::i18n::text(language, "宝藏已开启", "Treasure opened"),
        "compensation" => crate::i18n::text(language, "补偿已领取", "Compensation claimed"),
        "refreshCharm" => crate::i18n::text(language, "锦囊已刷新", "Charm pool refreshed"),
        "equipCharm" => crate::i18n::text(language, "锦囊已装备", "Charm equipped"),
        "battle" => crate::i18n::text(language, "夺宝完成", "Plunder complete"),
        "skipBattle" => crate::i18n::text(language, "已更新", "Updated"),
        "seeds" => crate::i18n::text(language, "种子已领取", "Seeds claimed"),
        "exchange" => crate::i18n::text(language, "兑换成功", "Exchange complete"),
        "story" => crate::i18n::text(language, "手记奖励已领取", "Journal reward claimed"),
        "solar" => crate::i18n::text(language, "节令好礼已领取", "Seasonal reward claimed"),
        _ => value.get("message").and_then(serde_json::Value::as_str).unwrap_or(crate::i18n::text(
            language,
            "操作成功",
            "Operation complete",
        )),
    };
    let rewards = value
        .get("rewards")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            let id = item.get("id").map(value_to_action_string).unwrap_or_default();
            let name = item
                .get("name")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if id.is_empty() {
                        crate::i18n::text(language, "道具", "Item").to_string()
                    } else {
                        format!("{} {id}", crate::i18n::text(language, "道具", "Item"))
                    }
                });
            let count = item.get("count").map(value_to_action_string).unwrap_or_default();
            format!("{name} ×{count}")
        })
        .collect::<Vec<_>>();
    if rewards.is_empty() {
        message.to_string()
    } else {
        let (separator, list_separator) =
            if language == "en-US" { (": ", ", ") } else { ("：", "、") };
        format!("{message}{separator}{}", rewards.join(list_separator))
    }
}

fn action_key(action: &str) -> String {
    format!("petDiary:{action}")
}

impl FarmDesktop {
    pub(in crate::desktop) fn pet_diary_action_is_pending(&self, action: &str) -> bool {
        self.activities.pending_actions.contains_key(&action_key(action))
    }

    pub(in crate::desktop::activities) fn run_pet_diary_action(
        &mut self,
        action: &'static str,
        params: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.loading
            || self.activities.pending_actions.keys().any(|key| key.starts_with("petDiary:"))
        {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let view_revision = self.activities.view_revision;
        let key = action_key(action);
        self.activities.action_sequence = self.activities.action_sequence.wrapping_add(1);
        let action_token = self.activities.action_sequence;
        self.activities.pending_actions.insert(key.clone(), action_token);
        self.error = None;
        self.activities.pet_diary_notice = None;
        self.activities.pet_diary_warning = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::activity::operate_pet_diary(&app, &account_id, action, &params)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.pending_actions.get(&key) != Some(&action_token) {
                    return;
                }
                this.activities.pending_actions.remove(&key);
                if this.activities.view_revision != view_revision
                    || this.page != Page::Activities
                    || this.page_section != "petDiary"
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(value)) => {
                        this.activities.pet_diary_notice =
                            Some(success_message(action, &value, &this.preferences.language));
                        this.activities.pet_diary_warning = value
                            .get("refreshError")
                            .and_then(serde_json::Value::as_str)
                            .filter(|warning| !warning.trim().is_empty())
                            .map(str::to_string);
                        if action == "battle" {
                            if let Some(raw) = this.page_raw.as_object_mut() {
                                raw.insert(
                                    "petDiaryBattleResult".into(),
                                    value.get("result").cloned().unwrap_or(value.clone()),
                                );
                            }
                        }
                        let snapshot = value.get("snapshot").filter(|value| !value.is_null());
                        if let Some(snapshot) = snapshot {
                            if let Some(raw) = this.page_raw.as_object_mut() {
                                raw.insert("petDiary".into(), snapshot.clone());
                            }
                            this.rows =
                                pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                        }
                        if action == "battle" {
                            this.load_pet_diary_friend(cx);
                        } else {
                            // Vue keeps the previous snapshot when the post-operation
                            // refresh fails and only surfaces the refresh warning.
                            cx.notify();
                        }
                    }
                    Ok(Err(error)) => {
                        if action == "battle" {
                            this.load_pet_diary_friend(cx);
                        }
                        this.error = Some(error);
                        cx.notify();
                    }
                    Err(error) => {
                        let message =
                            crate::i18n::background_task_error(&this.preferences.language, error);
                        if action == "battle" {
                            this.load_pet_diary_friend(cx);
                        }
                        this.error = Some(message);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::success_message;
    use serde_json::json;

    #[test]
    fn action_feedback_matches_the_activity_success_copy_and_includes_rewards() {
        assert_eq!(success_message("feed", &json!({}), "zh-CN"), "投喂成功");
        assert_eq!(success_message("feed", &json!({}), "en-US"), "Feeding complete");
        assert_eq!(
            success_message(
                "seeds",
                &json!({"rewards": [{"id": 9, "name": "元气糕", "count": 2}]}),
                "zh-CN"
            ),
            "种子已领取：元气糕 ×2"
        );
        assert_eq!(
            success_message("futureAction", &json!({"message": "已完成"}), "en-US"),
            "已完成"
        );
        assert_eq!(
            success_message("futureAction", &json!({"rewards": [{"id": 3, "count": 1}]}), "en-US"),
            "Operation complete: Item 3 ×1"
        );
        assert_eq!(
            success_message("feed", &json!({"rewards": [{"id": 3, "name": "123"}]}), "zh-CN"),
            "投喂成功：123 ×"
        );
    }
}
