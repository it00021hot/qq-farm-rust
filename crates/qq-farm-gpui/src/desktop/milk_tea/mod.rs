//! 奶茶活动页（bot `milk-tea.ts`，1.15 新活动）。

mod state;
pub(crate) use state::MilkTeaState;

mod view;

use super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn invalidate_milk_tea(&mut self) {
        self.milk_tea.generation = self.milk_tea.generation.wrapping_add(1);
        self.milk_tea.pending = false;
        self.milk_tea.error = None;
        self.milk_tea.craft_selected.clear();
    }

    /// 触发一次奶茶活动请求：`action=None` 拉页面快照（page 由 page_section 决定）。
    pub(in crate::desktop) fn milk_tea_request(
        &mut self,
        action: Option<&'static str>,
        input: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.test_mode || self.milk_tea.pending {
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
        let page = self.page_section.clone();
        self.milk_tea.generation = self.milk_tea.generation.wrapping_add(1);
        let generation = self.milk_tea.generation;
        self.milk_tea.pending = true;
        self.milk_tea.error = None;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    match action {
                        Some(action) => {
                            qq_farm_app::milk_tea::operate(&app, &account_id, action, &input).await
                        }
                        None => {
                            qq_farm_app::milk_tea::snapshot(&app, &account_id, &page, &input).await
                        }
                    }
                    .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.milk_tea.generation != generation {
                    return;
                }
                this.milk_tea.pending = false;
                if this.selected_account.as_deref() != Some(account_id_guard.as_str())
                    || this.page != Page::MilkTea
                {
                    cx.notify();
                    return;
                }
                let language = this.preferences.language.clone();
                match result {
                    Ok(Ok(value)) => {
                        if action.is_some() {
                            let text = milk_tea_reward_text(&value, &language);
                            let refresh_required =
                                value.get("refreshRequired").and_then(serde_json::Value::as_bool)
                                    == Some(true);
                            this.show_toast(text, !refresh_required, cx);
                        }
                        let activity = value.get("activity").cloned().unwrap_or(value);
                        if activity.is_null() {
                            // 写操作成功但刷新失败：重新拉一次页面
                            this.fetch_milk_tea_page(cx);
                        } else {
                            this.page_raw = activity;
                            this.milk_tea.error = None;
                            this.reconcile_milk_tea_page();
                        }
                    }
                    Ok(Err(error)) => {
                        if action.is_some() {
                            this.show_toast(error, false, cx);
                        } else {
                            this.milk_tea.error = Some(error);
                        }
                    }
                    Err(error) => {
                        let message = crate::i18n::background_task_error(&language, error);
                        if action.is_some() {
                            this.show_toast(message, false, cx);
                        } else {
                            this.milk_tea.error = Some(message);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 拉取当前分段页面的快照（含分段参数：雇佣时长 / 排行页 / 记录日期）。
    pub(in crate::desktop) fn fetch_milk_tea_page(&mut self, cx: &mut Context<Self>) {
        let input = match self.page_section.as_str() {
            "hireList" => serde_json::json!({ "durationId": self.milk_tea.hire_duration_id }),
            "rank" => serde_json::json!({ "pageNumber": self.milk_tea.rank_page }),
            "logs" if self.milk_tea.logs_day > 0 => {
                serde_json::json!({ "day": self.milk_tea.logs_day })
            }
            _ => serde_json::json!({}),
        };
        self.milk_tea_request(None, input, cx);
    }

    /// 载入后归一页面参数（记录日期取回包天数、排行榜页码跟随回包）。
    fn reconcile_milk_tea_page(&mut self) {
        if self.page_section == "logs" {
            if let Some(day) =
                self.page_raw.pointer("/logs/day").and_then(serde_json::Value::as_i64)
            {
                self.milk_tea.logs_day = day;
            }
        }
        if self.page_section == "rank" {
            if let Some(page) =
                self.page_raw.pointer("/rank/page").and_then(serde_json::Value::as_i64)
            {
                self.milk_tea.rank_page = page;
            }
        }
    }
}

/// 写操作反馈文案（奖励优先，否则通用成功文案）。
pub(in crate::desktop) fn milk_tea_reward_text(
    value: &serde_json::Value,
    language: &str,
) -> String {
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
