//! 快乐不独享（分享奖励）页（bot `share-reward-activity.ts` 实例 2026110104）。

mod state;
pub(crate) use state::ShareRewardState;

mod view;

use super::*;

/// 当前有效期实例（bot share-reward-instances.json）。
pub(crate) const SHARE_REWARD_ACTIVITY_ID: &str = "2026110104";

impl FarmDesktop {
    pub(in crate::desktop) fn invalidate_share_reward(&mut self) {
        self.share_reward.generation = self.share_reward.generation.wrapping_add(1);
        self.share_reward.pending = false;
        self.share_reward.error = None;
        self.share_reward.logs.clear();
    }

    /// 触发一次分享奖励请求：`action=None` 拉快照。
    pub(in crate::desktop) fn share_reward_request(
        &mut self,
        action: Option<&'static str>,
        input: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.test_mode || self.share_reward.pending {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先添加账号",
                    "Add an account first",
                )
                .into(),
            );
            cx.notify();
            return;
        };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let account_id_guard = account_id.clone();
        self.share_reward.generation = self.share_reward.generation.wrapping_add(1);
        let generation = self.share_reward.generation;
        self.share_reward.pending = true;
        self.share_reward.error = None;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    match action {
                        Some(action) => {
                            qq_farm_app::share_reward::operate(
                                &app,
                                &account_id,
                                SHARE_REWARD_ACTIVITY_ID,
                                action,
                                &input,
                            )
                            .await
                        }
                        None => {
                            qq_farm_app::share_reward::snapshot(
                                &app,
                                &account_id,
                                SHARE_REWARD_ACTIVITY_ID,
                            )
                            .await
                        }
                    }
                    .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.share_reward.generation != generation {
                    return;
                }
                this.share_reward.pending = false;
                if this.selected_account.as_deref() != Some(account_id_guard.as_str())
                    || this.page != Page::ShareReward
                {
                    cx.notify();
                    return;
                }
                let language = this.preferences.language.clone();
                match result {
                    Ok(Ok(value)) => {
                        if action.is_some() {
                            let text = share_reward_reward_text(&value, &language);
                            let refresh_required =
                                value.get("refreshRequired").and_then(serde_json::Value::as_bool)
                                    == Some(true);
                            this.show_toast(text, !refresh_required, cx);
                        }
                        let activity =
                            value.get("activity").cloned().unwrap_or_else(|| value.clone());
                        if activity.is_null() {
                            this.refresh(cx);
                        } else {
                            this.page_raw = activity;
                            this.share_reward.error = None;
                        }
                        if action == Some("logs") {
                            this.share_reward.logs = value
                                .pointer("/result/logs")
                                .and_then(serde_json::Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                        }
                    }
                    Ok(Err(error)) => {
                        if action.is_some() {
                            this.show_toast(error, false, cx);
                        } else {
                            this.share_reward.error = Some(error);
                        }
                    }
                    Err(error) => {
                        let message = crate::i18n::background_task_error(&language, error);
                        if action.is_some() {
                            this.show_toast(message, false, cx);
                        } else {
                            this.share_reward.error = Some(message);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn share_reward_reward_text(value: &serde_json::Value, language: &str) -> String {
    let rewards = value
        .get("rewards")
        .and_then(serde_json::Value::as_array)
        .map(|rewards| {
            rewards
                .iter()
                .map(|reward| {
                    let name = reward.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
                    let count =
                        reward.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0);
                    format!("{name} ×{count}")
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    if rewards.is_empty() {
        crate::i18n::text(language, "操作成功", "Done").to_string()
    } else {
        rewards
    }
}
