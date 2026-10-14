mod handlers;
mod inputs;
mod targeted;
mod weather;

use super::super::*;
use super::feedback::{self, merge_activity_snapshot};

fn targeted_action_key(action: &str, target: &str) -> String {
    format!("{action}:{target}")
}

impl FarmDesktop {
    fn finish_activity_action_result(
        &mut self,
        action: &str,
        result: &serde_json::Value,
        fallback_rewards: &[serde_json::Value],
        message_override: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let english = self.preferences.language == "en-US";
        let (message, success) = if let Some(message) = message_override {
            (message, true)
        } else if fallback_rewards.is_empty() {
            feedback::action_result_message(result, english)
        } else {
            feedback::action_result_message_with_fallback(result, fallback_rewards, english)
        };
        self.show_activity_action_notice(message, success, cx);
        if merge_activity_snapshot(&mut self.page_raw, result, action) {
            self.rows = pages::page_data(Page::Activities, self.page_raw.clone()).rows;
            cx.notify();
        } else {
            self.refresh(cx);
        }
    }

    pub(in crate::desktop) fn activity_action_is_pending(&self, action: &str) -> bool {
        self.activities.pending_actions.contains_key(action)
    }

    pub(in crate::desktop) fn activity_target_is_pending(
        &self,
        action: &str,
        target: &str,
    ) -> bool {
        self.activities.pending_actions.contains_key(&targeted_action_key(action, target))
    }

    pub(in crate::desktop) fn run_activity_action(
        &mut self,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.loading && !matches!(action, "qixi-bridge" | "qixi-gift") {
            return;
        }
        let qingmei_input = if action == "qingmei-start" {
            match inputs::qingmei::prepare(
                &self.page_raw,
                &self.activities.qingmei_selected_ingredients,
                &self.preferences.language,
            ) {
                Ok(input) => Some(input),
                Err(error) => {
                    self.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let qixi_gift = if action == "qixi-gift" {
            let selected_gid = self.activities.qixi_selected_friend_gid.trim();
            let count = self.activities.qixi_gift_count.read(cx).value().to_string();
            match inputs::qixi::prepare(
                &self.page_raw,
                selected_gid,
                &count,
                &self.preferences.language,
            ) {
                Ok(gift) => Some(gift),
                Err(error) => {
                    self.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let qixi_gift_notice = qixi_gift.map(|(gid, count)| {
            super::qixi_controls::qixi_gift_success_message(
                &self.page_raw,
                gid,
                count,
                &self.preferences.language,
            )
        });
        let autumn_choice = if action == "autumn-draw" {
            match inputs::autumn::prepare(
                &self.page_raw,
                &self.activities.autumn_choice_id,
                &self.preferences.language,
            ) {
                Ok(choice) => Some(choice),
                Err(error) => {
                    self.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let fallback_rewards = feedback::fallback_rewards(&self.page_raw, action, None);
        let Some(account_id) = self.selected_account.clone() else { return };
        if self.activities.pending_actions.contains_key(action) {
            return;
        }
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let view_revision = self.activities.view_revision;
        self.activities.action_sequence = self.activities.action_sequence.wrapping_add(1);
        let action_token = self.activities.action_sequence;
        self.activities.pending_actions.insert(action.to_string(), action_token);
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    handlers::execute(
                        app,
                        account_id,
                        action,
                        qingmei_input,
                        qixi_gift,
                        autumn_choice,
                    )
                    .await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.pending_actions.get(action) != Some(&action_token) {
                    return;
                }
                this.activities.pending_actions.remove(action);
                if this.activities.view_revision != view_revision
                    || this.page != Page::Activities
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(value)) if action.starts_with("autumn-happy-logs-") => {
                        let requested_tab = if action.ends_with("-mine") { 0 } else { 1 };
                        if this.activities.autumn_log_tab != requested_tab {
                            cx.notify();
                            return;
                        }
                        if let Some(happy) = this
                            .page_raw
                            .pointer_mut("/autumn/happy")
                            .and_then(serde_json::Value::as_object_mut)
                        {
                            if let Some(activity) =
                                value.get("activity").filter(|activity| activity.is_object())
                            {
                                *happy = activity.as_object().cloned().unwrap_or_default();
                            }
                            happy.insert(
                                "logs".into(),
                                value.get("result").cloned().unwrap_or(value),
                            );
                            this.rows =
                                pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                        }
                        cx.notify();
                    }
                    Ok(Ok(value)) => this.finish_activity_action_result(
                        action,
                        &value,
                        &fallback_rewards,
                        qixi_gift_notice,
                        cx,
                    ),
                    Ok(Err(error)) => {
                        this.error = Some(error.to_string());
                        cx.notify();
                    }
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn run_activity_with_id(
        &mut self,
        action: &'static str,
        target: String,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let fallback_rewards = feedback::fallback_rewards(&self.page_raw, action, Some(&target));
        let action_key = targeted_action_key(action, &target);
        if self.activities.pending_actions.contains_key(&action_key) {
            return;
        }
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let view_revision = self.activities.view_revision;
        self.activities.action_sequence = self.activities.action_sequence.wrapping_add(1);
        let action_token = self.activities.action_sequence;
        self.activities.pending_actions.insert(action_key.clone(), action_token);
        let account_id_check = account_id.clone();
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    targeted::execute(app, account_id, action, target)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.pending_actions.get(&action_key) != Some(&action_token) {
                    return;
                }
                this.activities.pending_actions.remove(&action_key);
                if this.activities.view_revision != view_revision
                    || this.page != Page::Activities
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(value)) => this.finish_activity_action_result(
                        action,
                        &value,
                        &fallback_rewards,
                        None,
                        cx,
                    ),
                    Ok(Err(error)) => {
                        this.error = Some(error);
                        cx.notify();
                    }
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
