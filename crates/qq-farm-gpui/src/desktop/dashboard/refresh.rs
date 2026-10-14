use super::super::*;
use std::time::Duration;

use super::format;

impl FarmDesktop {
    pub(in crate::desktop) fn reconcile_dashboard_snapshot(&mut self) {
        self.dashboard.logs_follow_tail = true;
        self.dashboard.logs_scroll.scroll_to_bottom();

        let is_online =
            self.page_raw.pointer("/status/runStatus").and_then(serde_json::Value::as_i64)
                == Some(1);
        if is_online {
            if let Some(bag) = self.page_raw.get("bag").filter(|bag| bag.is_object()) {
                self.dashboard
                    .coupon_baseline
                    .get_or_insert_with(|| super::super::runtime::bag_item_count(bag, 1002));
            }
        } else {
            self.dashboard.coupon_baseline = None;
            self.page_raw["bag"] = serde_json::Value::Null;
            self.page_raw["diamond"] = serde_json::Value::Null;
        }
    }

    pub(in crate::desktop) fn schedule_dashboard_bag_refresh(&mut self, cx: &mut Context<Self>) {
        if self.dashboard.bag_refresh_pending || self.selected_account.is_none() {
            return;
        }
        self.dashboard.bag_refresh_pending = true;
        self.dashboard.bag_refresh_generation =
            self.dashboard.bag_refresh_generation.wrapping_add(1);
        let generation = self.dashboard.bag_refresh_generation;
        let account_id = self.selected_account.clone().unwrap_or_default();
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let result = runtime
                .spawn(async move {
                    let (bag, diamond) = tokio::join!(
                        qq_farm_app::farm::bag(&app, &account_id),
                        qq_farm_app::farm::diamond_balance(&app, &account_id),
                    );
                    (bag.ok(), diamond.ok())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.dashboard.bag_refresh_generation
                    || this.page != Page::Dashboard
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    return;
                }
                this.dashboard.bag_refresh_pending = false;
                if let Ok((Some(bag), diamond)) = result {
                    this.page_raw["bag"] = serde_json::to_value(bag).unwrap_or_default();
                    let coupon_count = this
                        .page_raw
                        .get("bag")
                        .map_or(0, |bag| super::super::runtime::bag_item_count(bag, 1002));
                    this.dashboard.coupon_baseline.get_or_insert(coupon_count);
                    if let Some(diamond) = diamond {
                        this.page_raw["diamond"] = serde_json::json!(diamond);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn refresh_dashboard_status(&mut self, cx: &mut Context<Self>) {
        let Some(account_id) = self.selected_account.clone() else { return };
        let account_id_check = account_id.clone();
        self.dashboard.status_revision = self.dashboard.status_revision.wrapping_add(1);
        let revision = self.dashboard.status_revision;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(
                    async move { qq_farm_app::farm::panel_status_with_progress(&app, &account_id) },
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if revision != this.dashboard.status_revision
                    || this.page != Page::Dashboard
                    || this.selected_account.as_deref() != Some(account_id_check.as_str())
                {
                    return;
                }
                let Ok(status) = result else { return };
                let was_online = format::int_at(&this.page_raw["status"], &["runStatus"]) == 1;
                this.page_raw["status"] = serde_json::to_value(status).unwrap_or_default();
                let is_online = format::int_at(&this.page_raw["status"], &["runStatus"]) == 1;
                if !is_online {
                    this.dashboard.coupon_baseline = None;
                    this.page_raw["bag"] = serde_json::Value::Null;
                    this.page_raw["diamond"] = serde_json::Value::Null;
                }
                this.rows = pages::page_data(Page::Dashboard, this.page_raw.clone()).rows;
                if was_online != is_online && is_online {
                    this.refresh(cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
