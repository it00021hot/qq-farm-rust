use super::super::*;
use std::time::Duration;

fn unix_time_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}
impl FarmDesktop {
    pub(in crate::desktop) fn sync_friend_plant_from_lands(
        &mut self,
        gid: i64,
        detail: &serde_json::Value,
    ) {
        let Some(lands) = detail.get("lands").and_then(serde_json::Value::as_array) else {
            return;
        };
        let plant = super::helpers::friend_plant_summary_from_lands(lands);
        if let Some(friends) =
            self.page_raw.get_mut("friends").and_then(serde_json::Value::as_array_mut)
        {
            if let Some(friend) = friends
                .iter_mut()
                .find(|friend| super::helpers::friend_number_at(friend, "/gid") == gid)
            {
                friend["plant"] = plant.clone();
            }
        }
        for row in self.rows.iter_mut().filter(|row| row.section == "friends") {
            if super::helpers::friend_number_at(&row.record, "/gid") == gid {
                row.record["plant"] = plant.clone();
                row.state = super::helpers::friend_status_localized(
                    &row.record,
                    &self.preferences.language,
                );
            }
        }
        crate::pages::sort_friend_rows(&mut self.rows);
    }

    pub(in crate::desktop) fn toggle_friend_detail(&mut self, gid: i64, cx: &mut Context<Self>) {
        if self.friends.detail.as_ref().is_some_and(|(expanded_gid, _)| *expanded_gid == gid)
            || self.friends.detail_loading_gid == Some(gid)
        {
            self.friends.detail = None;
            self.friends.detail_loading_gid = None;
            self.friends.detail_error = None;
            self.friends.countdown_generation = self.friends.countdown_generation.wrapping_add(1);
            cx.notify();
            return;
        }
        self.open_friend_detail(gid, cx);
    }

    pub(in crate::desktop) fn open_friend_detail(&mut self, gid: i64, cx: &mut Context<Self>) {
        let Some(account_id) = self.selected_account.clone() else { return };
        self.friends.countdown_generation = self.friends.countdown_generation.wrapping_add(1);
        let countdown_generation = self.friends.countdown_generation;
        self.friends.detail = None;
        self.friends.detail_loading_gid = Some(gid);
        self.friends.detail_error = None;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let account_id_for_check = account_id.clone();
        let revision = self.request_revision;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::friend_lands(&app, &account_id, gid)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.request_revision != revision
                    || this.friends.countdown_generation != countdown_generation
                    || this.selected_account.as_deref() != Some(account_id_for_check.as_str())
                {
                    return;
                }
                this.friends.detail_loading_gid = None;
                match result {
                    Ok(Ok(detail)) => {
                        let now = unix_time_seconds();
                        this.sync_friend_plant_from_lands(gid, &detail);
                        this.friends.detail = Some((gid, detail));
                        this.friends.detail_loaded_at = now;
                        this.friends.countdown_now_sec = now;
                        this.start_friend_countdown(countdown_generation, cx);
                    }
                    Ok(Err(error)) => this.friends.detail_error = Some((gid, error)),
                    Err(error) => {
                        this.friends.detail_error = Some((
                            gid,
                            if this.preferences.language == "en-US" {
                                format!("Background task ended: {error}")
                            } else {
                                format!("后台任务已结束：{error}")
                            },
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_friend_countdown(&self, generation: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let now = unix_time_seconds();
            let active = this.update(cx, |this, cx| {
                if generation != this.friends.countdown_generation
                    || this.page != Page::Friends
                    || this.friends.detail.is_none()
                {
                    return false;
                }
                this.friends.countdown_now_sec = now;
                let has_growing_crop = this
                    .friends
                    .detail
                    .as_ref()
                    .and_then(|(_, detail)| detail.get("lands"))
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|lands| {
                        lands.iter().any(|land| {
                            let remaining = land
                                .get("matureInSec")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or_default();
                            remaining > 0
                                && this.friends.detail_loaded_at.saturating_add(remaining) > now
                        })
                    });
                cx.notify();
                has_growing_crop
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }
}
