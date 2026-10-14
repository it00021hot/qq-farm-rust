use super::*;
use account_filter::{account_list_needs_refresh, should_ignore_other_account_event};

mod account_filter;
mod subscription;

pub(in crate::desktop) use subscription::subscribe_realtime;

impl FarmDesktop {
    pub(in crate::desktop) fn listen_events(&mut self, cx: &mut Context<Self>) {
        let mut receiver = std::mem::replace(&mut self.realtime, mpsc::channel(1).1);
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.next().await {
                let should_continue = this.update(cx, |this, cx| {
                    if this.quitting {
                        return;
                    }
                    match event {
                        DesktopEvent::Refresh => this.refresh(cx),
                        DesktopEvent::Realtime(events) => {
                            let mut refresh = false;
                            for event in events {
                                let refresh_accounts = this.page == Page::Accounts
                                    && account_list_needs_refresh(
                                        &event.event_type,
                                        &event.payload,
                                    );
                                if should_ignore_other_account_event(
                                    this.page,
                                    this.selected_account.as_deref(),
                                    &event,
                                ) {
                                    continue;
                                }
                                if this.page == Page::Friends {
                                    this.handle_friend_realtime_event(&event, cx);
                                }
                                match event.event_type.as_str() {
                                    "log:new" if this.page == Page::Dashboard => {
                                        if append_dashboard_log(this, event.payload.clone()) {
                                            let tag = event
                                                .payload
                                                .get("tag")
                                                .and_then(serde_json::Value::as_str)
                                                .unwrap_or("");
                                            let message = event
                                                .payload
                                                .get("msg")
                                                .or_else(|| event.payload.get("message"))
                                                .and_then(serde_json::Value::as_str)
                                                .unwrap_or("");
                                            if matches!(tag, "施肥" | "商城")
                                                || message.contains("化肥")
                                            {
                                                this.schedule_dashboard_bag_refresh(cx);
                                            }
                                        }
                                    }
                                    "account-log:new" if this.page == Page::Dashboard => {
                                        let mut payload = event.payload.clone();
                                        if let Some(object) = payload.as_object_mut() {
                                            let message =
                                                object.get("message").cloned().unwrap_or_default();
                                            object.entry("msg").or_insert(message);
                                            object
                                                .entry("tag")
                                                .or_insert_with(|| serde_json::json!("系统"));
                                            object
                                                .entry("isWarn")
                                                .or_insert_with(|| serde_json::json!(false));
                                            object.entry("ts").or_insert_with(|| {
                                                serde_json::json!(unix_time_millis())
                                            });
                                            if let Some(event) = object.get("event").cloned() {
                                                object.entry("meta").or_insert_with(
                                                    || serde_json::json!({"event": event}),
                                                );
                                            }
                                        }
                                        append_dashboard_log(this, payload);
                                    }
                                    "status:update" => {
                                        if this.page == Page::Dashboard {
                                            if let Some(status) = event.payload.get("status") {
                                                let was_online = this
                                                    .page_raw
                                                    .pointer("/status/runStatus")
                                                    .and_then(serde_json::Value::as_i64)
                                                    == Some(1);
                                                let run_status = this
                                                    .page_raw
                                                    .pointer("/status/runStatus")
                                                    .cloned();
                                                merge_json_object(
                                                    &mut this.page_raw["status"],
                                                    status,
                                                );
                                                if let Some(run_status) = run_status {
                                                    this.page_raw["status"]["runStatus"] =
                                                        run_status;
                                                }
                                                let is_online = this
                                                    .page_raw
                                                    .pointer("/status/runStatus")
                                                    .and_then(serde_json::Value::as_i64)
                                                    == Some(1);
                                                this.rows = pages::page_data(
                                                    Page::Dashboard,
                                                    this.page_raw.clone(),
                                                )
                                                .rows;
                                                if !is_online {
                                                    this.dashboard.coupon_baseline = None;
                                                    this.page_raw["bag"] = serde_json::Value::Null;
                                                    this.page_raw["diamond"] =
                                                        serde_json::Value::Null;
                                                }
                                                refresh |= was_online != is_online && is_online;
                                            } else {
                                                this.refresh_dashboard_status(cx);
                                            }
                                        } else if refresh_accounts {
                                            refresh = true;
                                        }
                                    }
                                    "farm_operation" | "farm_tick"
                                        if this.page == Page::Dashboard =>
                                    {
                                        this.refresh_dashboard_status(cx);
                                    }
                                    "account_status" => {
                                        let directory = accounts::read_accounts(&this.app);
                                        this.accounts.records = directory.records;
                                        this.accounts.avatars = directory.avatars;
                                        if this.page == Page::Dashboard {
                                            this.refresh_dashboard_status(cx);
                                        } else if refresh_accounts {
                                            refresh = true;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if refresh {
                                this.refresh(cx);
                            } else {
                                cx.notify();
                            }
                        }
                    }
                });
                if should_continue.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub(in crate::desktop) fn listen_tray(&mut self, cx: &mut Context<Self>) {
        let mut receiver = std::mem::replace(&mut self.tray_events, mpsc::unbounded().1);
        cx.spawn(async move |this, cx| {
            while let Some(action) = receiver.next().await {
                let _ = this.update(cx, |this, cx| match action {
                    crate::tray::Action::ShowWindow => {
                        crate::application::restore_main_window(cx);
                    }
                    crate::tray::Action::Quit => {
                        this.request_quit(cx);
                    }
                });
            }
        })
        .detach();
    }
}
