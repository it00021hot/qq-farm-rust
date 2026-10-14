use std::time::Duration;

use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn invalidate_personal_bag_clock(&mut self) {
        self.farm.bag.tick_generation = self.farm.bag.tick_generation.wrapping_add(1);
        self.farm.bag.refresh_generation = self.farm.bag.refresh_generation.wrapping_add(1);
        self.farm.bag.refreshing = false;
        self.farm.bag.operations_generation = self.farm.bag.operations_generation.wrapping_add(1);
        self.farm.bag.selling_ids.clear();
        self.farm.bag.using_ids.clear();
        self.farm.bag.batch_selling = false;
        self.farm.bag.notice = None;
        self.farm.bag.notice_generation = self.farm.bag.notice_generation.wrapping_add(1);
    }

    pub(in crate::desktop) fn start_personal_bag_clock(&mut self, cx: &mut Context<Self>) {
        if self.test_mode
            || self.page != Page::Personal
            || self.page_section != "bag"
            || self.page_raw.pointer("/bag/items").and_then(serde_json::Value::as_array).is_none()
        {
            return;
        }
        self.farm.bag.tick_generation = self.farm.bag.tick_generation.wrapping_add(1);
        let generation = self.farm.bag.tick_generation;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(60)).await;
            let active = this.update(cx, |this, cx| {
                if generation != this.farm.bag.tick_generation
                    || this.page != Page::Personal
                    || this.page_section != "bag"
                {
                    return false;
                }
                if !this.loading {
                    this.refresh_personal_bag(cx);
                }
                true
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    pub(in crate::desktop) fn refresh_personal_bag(&mut self, cx: &mut Context<Self>) {
        if self.test_mode
            || self.loading
            || self.farm.bag.refreshing
            || self.page != Page::Personal
            || self.page_section != "bag"
        {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        self.farm.bag.refresh_generation = self.farm.bag.refresh_generation.wrapping_add(1);
        let generation = self.farm.bag.refresh_generation;
        let revision = self.request_revision;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.farm.bag.refreshing = true;
        self.farm.bag.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::farm::bag(&app, &account_id)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.farm.bag.refresh_generation {
                    return;
                }
                this.farm.bag.refreshing = false;
                if revision != this.request_revision
                    || this.page != Page::Personal
                    || this.page_section != "bag"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(bag)) => {
                        this.page_raw["bag"] =
                            serde_json::to_value(bag).unwrap_or(serde_json::Value::Null);
                        this.farm.bag.selected_ids.clear();
                        this.farm.bag.error = None;
                        this.start_personal_bag_clock(cx);
                    }
                    Ok(Err(error)) => this.farm.bag.error = Some(error),
                    Err(error) => {
                        this.farm.bag.error = Some(if this.preferences.language == "en-US" {
                            format!("Background task failed: {error}")
                        } else {
                            format!("后台任务已结束：{error}")
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
