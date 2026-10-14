mod autumn;
mod pet_diary;
mod qixi;
mod weather;

use super::super::*;

pub(super) struct ActivityPayload {
    pub(super) value: serde_json::Value,
    pub(super) pet_diary_friends: Option<serde_json::Value>,
}

pub(super) async fn load(
    app: &AppContext,
    account_id: &str,
    section: &'static str,
    autumn_mode: &str,
    include_pet_diary_friends: bool,
) -> Result<ActivityPayload, qq_farm_app::AppError> {
    match section {
        "petDiary" => pet_diary::load(app, account_id, include_pet_diary_friends).await,
        "autumn" => autumn::load(app, account_id, autumn_mode).await,
        "weather" => weather::load(app, account_id).await,
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动分段".into())),
    }
}

fn clear_pet_diary_snapshot(this: &mut FarmDesktop) {
    if let Some(root) = this.page_raw.as_object_mut() {
        root.remove("petDiary");
    }
    this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
}

fn clear_weather_snapshot(this: &mut FarmDesktop) {
    if let Some(weather) =
        this.page_raw.get_mut("weather").and_then(serde_json::Value::as_object_mut)
    {
        weather.remove("snapshot");
    }
    this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
}

impl FarmDesktop {
    pub(in crate::desktop) fn load_qixi_friends(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.activities.qixi_friends_loading {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else {
            if let Some(root) = self.page_raw.as_object_mut() {
                root.insert("activityFriends".into(), serde_json::json!([]));
            }
            self.rows = pages::page_data(Page::Activities, self.page_raw.clone()).rows;
            cx.notify();
            return;
        };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let revision = self.request_revision;
        self.activities.qixi_friends_request_generation =
            self.activities.qixi_friends_request_generation.wrapping_add(1);
        let generation = self.activities.qixi_friends_request_generation;
        self.activities.qixi_friends_loading = true;
        self.error = None;
        cx.notify();
        if self.test_mode {
            self.activities.qixi_friends_loading = false;
            cx.notify();
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qixi::load_friends(&app, &account_id, force)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.qixi_friends_request_generation == generation {
                    this.activities.qixi_friends_loading = false;
                }
                if this.activities.qixi_friends_request_generation != generation
                    || this.request_revision != revision
                    || this.page != Page::Activities
                    || this.page_section != "qixi"
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(friends)) => {
                        if let Some(root) = this.page_raw.as_object_mut() {
                            root.insert("activityFriends".into(), friends);
                        }
                        this.rows = pages::page_data(Page::Activities, this.page_raw.clone()).rows;
                    }
                    Ok(Err(error)) => this.error = Some(error),
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn load_activity_section(
        &mut self,
        section: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.load_activity_section_with_options(section, true, cx);
    }

    pub(in crate::desktop) fn refresh_pet_diary_snapshot(&mut self, cx: &mut Context<Self>) {
        self.load_activity_section_with_options("petDiary", false, cx);
    }

    fn load_activity_section_with_options(
        &mut self,
        section: &'static str,
        include_pet_diary_friends: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let revision = self.request_revision;
        let autumn_mode = if self.activities.autumn_mode == "happy" { "happy" } else { "wish" };
        self.loading = true;
        self.error = None;
        cx.notify();
        if self.test_mode {
            self.loading = false;
            cx.notify();
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    load(&app, &account_id, section, autumn_mode, include_pet_diary_friends)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.request_revision != revision
                    || this.page != Page::Activities
                    || this.page_section != section
                {
                    return;
                }
                this.loading = false;
                let mut load_happy_logs = false;
                match result {
                    Ok(Ok(payload)) => {
                        let mut payload = payload;
                        if section == "weather" {
                            weather::preserve_friends_on_error(&this.page_raw, &mut payload.value);
                        }
                        if section == "autumn" {
                            if autumn_mode == "wish" {
                                this.activities.autumn_choice_id =
                                    super::autumn_view::selected_autumn_choice(
                                        &payload.value,
                                        &this.activities.autumn_choice_id,
                                    );
                            }
                            load_happy_logs =
                                autumn_mode == "happy" && this.activities.autumn_logs_load_pending;
                            this.activities.autumn_logs_load_pending = false;
                        }
                        if let Some(root) = this.page_raw.as_object_mut() {
                            if section == "qixi" {
                                root.insert("activityFriends".into(), payload.value);
                            } else if section == "autumn" {
                                let autumn =
                                    root.entry("autumn").or_insert_with(|| serde_json::json!({}));
                                if !autumn.is_object() {
                                    *autumn = serde_json::json!({});
                                }
                                if let (Some(autumn), Some(incoming)) =
                                    (autumn.as_object_mut(), payload.value.as_object())
                                {
                                    autumn.extend(incoming.clone());
                                }
                            } else {
                                root.insert(section.into(), payload.value);
                            }
                            if let Some(friends) = payload.pet_diary_friends {
                                root.insert("petDiaryFriends".into(), friends);
                            }
                        }
                        if section == "weather" {
                            this.start_weather_clock(cx);
                        }
                        let data = pages::page_data(Page::Activities, this.page_raw.clone());
                        this.rows = data.rows;
                        if section == "petDiary" {
                            if let Some(handle) = this.window_handle {
                                let _ = handle.update(cx, |_, window, cx| {
                                    this.sync_pet_diary_friend_selector(window, cx);
                                });
                            }
                        }
                        let refresh_pet_friend = section == "petDiary"
                            && this.activities.pet_diary_battle_target.as_ref().is_some_and(
                                |(target_gid, _, _)| {
                                    this.activities.pet_diary_friend_gid.trim() == target_gid
                                },
                            );
                        if refresh_pet_friend {
                            this.load_pet_diary_friend(cx);
                            return;
                        }
                    }
                    Ok(Err(error)) => {
                        if section == "petDiary" {
                            clear_pet_diary_snapshot(this);
                        } else if section == "weather" {
                            clear_weather_snapshot(this);
                        }
                        this.error = Some(error);
                    }
                    Err(error) => {
                        if section == "petDiary" {
                            clear_pet_diary_snapshot(this);
                        } else if section == "weather" {
                            clear_weather_snapshot(this);
                        }
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ))
                    }
                }
                cx.notify();
                if load_happy_logs {
                    let action = if this.activities.autumn_log_tab == 0 {
                        "autumn-happy-logs-mine"
                    } else {
                        "autumn-happy-logs-others"
                    };
                    this.run_activity_action(action, cx);
                }
            });
        })
        .detach();
    }
}
