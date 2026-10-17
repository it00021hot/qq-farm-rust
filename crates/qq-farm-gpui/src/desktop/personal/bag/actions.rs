use super::super::super::*;
use super::feedback::operation_summary;
use super::helpers::{can_sell, filtered_items, number};

impl FarmDesktop {
    pub(in crate::desktop) fn run_bag_action(
        &mut self,
        item_id: i64,
        uid: i64,
        sell: bool,
        cx: &mut Context<Self>,
    ) {
        if self.loading
            || (sell && self.farm.bag.batch_selling)
            || (if sell { &self.farm.bag.selling_ids } else { &self.farm.bag.using_ids })
                .contains(&item_id)
        {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let language = self.preferences.language.clone();
        let generation = self.farm.bag.operations_generation;
        let sell_items = self
            .page_raw
            .pointer("/bag/originalItems")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| item.get("id").and_then(serde_json::Value::as_i64) == Some(item_id))
            .filter_map(|item| {
                Some((
                    item.get("id")?.as_i64()?,
                    item.get("count")?.as_i64()?,
                    item.get("uid")?.as_i64()?,
                ))
            })
            .collect::<Vec<_>>();
        if sell && sell_items.is_empty() {
            self.show_personal_bag_notice(
                crate::i18n::text(
                    &self.preferences.language,
                    "未找到可出售物品",
                    "No sellable items were found",
                ),
                false,
                cx,
            );
            return;
        }
        if sell {
            self.farm.bag.selling_ids.insert(item_id);
        } else {
            self.farm.bag.using_ids.insert(item_id);
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    if sell {
                        qq_farm_app::farm::bag_sell(&app, &worker_account_id, &sell_items)
                            .await
                            .map_err(|error| error.to_string())
                    } else {
                        qq_farm_app::farm::bag_use(&app, &worker_account_id, item_id, 1, uid)
                            .await
                            .map_err(|error| error.to_string())
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.bag.operations_generation != generation {
                    return;
                }
                this.farm.bag.selling_ids.remove(&item_id);
                this.farm.bag.using_ids.remove(&item_id);
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Personal
                    || this.page_section != "bag"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(result)) => {
                        this.show_personal_bag_notice(
                            operation_summary(
                                &result,
                                &language,
                                if sell { "出售成功" } else { "使用成功" },
                                if sell { "Sold successfully" } else { "Used successfully" },
                            ),
                            true,
                            cx,
                        );
                        this.reload_bag_after_operation(cx);
                    }
                    Ok(Err(error)) => {
                        this.show_personal_bag_notice(error, false, cx);
                    }
                    Err(error) => {
                        this.show_personal_bag_notice(
                            if language == "en-US" {
                                format!("Background task failed: {error}")
                            } else {
                                format!("后台任务已结束：{error}")
                            },
                            false,
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn set_bag_selection(
        &mut self,
        item_id: i64,
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        if selected {
            self.farm.bag.selected_ids.insert(item_id);
        } else {
            self.farm.bag.selected_ids.remove(&item_id);
        }
        cx.notify();
    }

    pub(in crate::desktop) fn select_all_bag_sellable(&mut self, cx: &mut Context<Self>) {
        self.farm.bag.selected_ids = filtered_items(&self.page_raw, &self.farm.bag.category)
            .into_iter()
            .filter(|item| can_sell(item))
            .map(|item| number(item.get("id")))
            .collect();
        cx.notify();
    }

    pub(in crate::desktop) fn run_bag_batch_sell(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.farm.bag.batch_selling {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let selected = self.farm.bag.selected_ids.clone();
        let selected = filtered_items(&self.page_raw, &self.farm.bag.category)
            .into_iter()
            .filter(|item| can_sell(item) && selected.contains(&number(item.get("id"))))
            .map(|item| number(item.get("id")))
            .collect::<HashSet<_>>();
        let items = self
            .page_raw
            .pointer("/bag/originalItems")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| {
                item.get("id")
                    .and_then(serde_json::Value::as_i64)
                    .is_some_and(|id| selected.contains(&id))
            })
            .filter_map(|item| {
                Some((
                    item.get("id")?.as_i64()?,
                    item.get("count")?.as_i64()?,
                    item.get("uid")?.as_i64()?,
                ))
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            self.show_personal_bag_notice(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先勾选可出售物品",
                    "Select sellable items first",
                ),
                false,
                cx,
            );
            return;
        }
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let language = self.preferences.language.clone();
        let generation = self.farm.bag.operations_generation;
        self.farm.bag.batch_selling = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let worker_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    qq_farm_app::farm::bag_sell(&app, &worker_account_id, &items)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.bag.operations_generation != generation {
                    return;
                }
                this.farm.bag.batch_selling = false;
                if this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.page != Page::Personal
                    || this.page_section != "bag"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(result)) => {
                        this.farm.bag.selected_ids.clear();
                        this.farm.bag.batch_mode = false;
                        this.show_personal_bag_notice(
                            operation_summary(
                                &result,
                                &language,
                                "批量出售成功",
                                "Batch sell succeeded",
                            ),
                            true,
                            cx,
                        );
                        this.reload_bag_after_operation(cx);
                    }
                    Ok(Err(error)) => {
                        this.show_personal_bag_notice(error, false, cx);
                    }
                    Err(error) => {
                        this.show_personal_bag_notice(
                            if language == "en-US" {
                                format!("Background task failed: {error}")
                            } else {
                                format!("后台任务已结束：{error}")
                            },
                            false,
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }
    fn reload_bag_after_operation(&mut self, cx: &mut Context<Self>) {
        self.farm.bag.refresh_generation = self.farm.bag.refresh_generation.wrapping_add(1);
        self.farm.bag.refreshing = false;
        self.refresh_personal_bag(cx);
    }
}
