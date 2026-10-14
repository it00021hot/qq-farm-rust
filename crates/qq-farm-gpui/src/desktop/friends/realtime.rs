use super::super::*;
use qq_farm_app::events::PanelRealtimeEvent;
use serde_json::{Map, Value};
use std::time::Duration;

const FRIEND_LIST_REFRESH_DELAY: Duration = Duration::from_millis(800);

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) => "".into(),
        Value::Object(_) => "[object Object]".into(),
    }
}

fn first_truthy<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().filter_map(|key| payload.get(*key)).find(|value| js_truthy(value))
}

fn number(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_f64().map(|value| value as i64))
                .or_else(|| value.as_str()?.trim().parse::<f64>().ok().map(|value| value as i64))
                .or_else(|| value.as_bool().map(i64::from))
        })
        .unwrap_or_default()
}

fn friend_plant_patch(event: &PanelRealtimeEvent) -> Option<(i64, Value)> {
    if event.event_type != "friend_plant" {
        return None;
    }
    let payload = &event.payload;
    let gid = number(first_truthy(payload, &["friendGid", "gid"]));
    if gid <= 0 {
        return None;
    }
    let plant = payload.get("plant").filter(|plant| plant.is_object()).unwrap_or(payload);
    let mut counts = Map::new();
    for field in ["stealNum", "dryNum", "weedNum", "insectNum"] {
        let value = plant
            .get(field)
            .filter(|value| !value.is_null())
            .or_else(|| payload.get(field).filter(|value| !value.is_null()));
        counts.insert(field.into(), Value::from(number(value)));
    }
    Some((gid, Value::Object(counts)))
}

fn friend_interaction_steal_gid(event: &PanelRealtimeEvent) -> Option<Option<i64>> {
    if event.event_type != "friend_interact" {
        return None;
    }
    let payload = &event.payload;
    let action = first_truthy(payload, &["action", "event"]).map(js_string).unwrap_or_default();
    let result =
        payload.get("result").filter(|value| js_truthy(value)).map(js_string).unwrap_or_default();
    if !(action.contains("steal") || action == "偷菜") || result == "error" {
        return None;
    }
    Some(first_truthy(payload, &["targetGid", "friendGid", "gid"]).map(|gid| number(Some(gid))))
}

fn log_indicates_friend_visit_or_steal(event: &PanelRealtimeEvent) -> bool {
    if !matches!(
        event.event_type.as_str(),
        "log" | "log:new" | "account_log" | "account-log:new" | "worker_log"
    ) {
        return false;
    }
    let payload = &event.payload;
    let message =
        payload.get("message").filter(|value| js_truthy(value)).map(js_string).unwrap_or_default();
    let event_name =
        payload.get("event").filter(|value| js_truthy(value)).map(js_string).unwrap_or_default();
    event_name == "visit_friend"
        || message.contains("偷菜")
        || message
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0] == '偷' && pair[1].is_ascii_digit())
}

fn friend_log_gid(event: &PanelRealtimeEvent) -> Option<i64> {
    first_truthy(&event.payload, &["friendGid", "targetGid"]).map(|gid| number(Some(gid)))
}

fn apply_friend_plant_patch(desktop: &mut FarmDesktop, gid: i64, plant: Value) {
    let Some(friends) = desktop.page_raw.get_mut("friends").and_then(Value::as_array_mut) else {
        return;
    };
    let Some(friend) =
        friends.iter_mut().find(|friend| super::helpers::friend_number_at(friend, "/gid") == gid)
    else {
        return;
    };
    friend["plant"] = plant.clone();
    for row in desktop.rows.iter_mut().filter(|row| row.section == "friends") {
        if super::helpers::friend_number_at(&row.record, "/gid") == gid {
            row.record["plant"] = plant.clone();
            row.state =
                super::helpers::friend_status_localized(&row.record, &desktop.preferences.language);
        }
    }
    crate::pages::sort_friend_rows(&mut desktop.rows);
}

impl FarmDesktop {
    pub(in crate::desktop) fn handle_friend_realtime_event(
        &mut self,
        event: &PanelRealtimeEvent,
        cx: &mut Context<Self>,
    ) {
        if let Some((gid, plant)) = friend_plant_patch(event) {
            apply_friend_plant_patch(self, gid, plant);
            return;
        }
        if let Some(gid) = friend_interaction_steal_gid(event) {
            self.schedule_friend_list_refresh(gid, cx);
        } else if log_indicates_friend_visit_or_steal(event) {
            self.schedule_friend_list_refresh(friend_log_gid(event), cx);
        }
    }

    fn schedule_friend_list_refresh(&mut self, gid: Option<i64>, cx: &mut Context<Self>) {
        if let Some(gid) = gid.filter(|gid| *gid > 0) {
            self.update_friend_operation_snapshot(gid, "steal");
        }
        self.friends.realtime_refresh_generation =
            self.friends.realtime_refresh_generation.wrapping_add(1);
        let generation = self.friends.realtime_refresh_generation;
        if self.test_mode {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FRIEND_LIST_REFRESH_DELAY).await;
            let should_refresh = this.update(cx, |this, cx| {
                if this.quitting
                    || this.page != Page::Friends
                    || this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.friends.realtime_refresh_generation != generation
                {
                    return false;
                }
                let expanded_gid = this
                    .friends
                    .detail
                    .as_ref()
                    .map(|(gid, _)| *gid)
                    .or(this.friends.detail_loading_gid);
                if let Some(gid) = expanded_gid {
                    this.open_friend_detail(gid, cx);
                }
                true
            });
            if !matches!(should_refresh, Ok(true)) {
                return;
            }

            let request_account_id = account_id.clone();
            let result = runtime
                .spawn(async move {
                    let (friends, blacklist) = tokio::join!(
                        qq_farm_app::friend::list_friends(&app, &request_account_id, false),
                        async { qq_farm_app::friend::friend_blacklist(&request_account_id) }
                    );
                    (friends.map_err(|error| error.to_string()), blacklist)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.quitting
                    || this.page != Page::Friends
                    || this.selected_account.as_deref() != Some(account_id.as_str())
                    || this.friends.realtime_refresh_generation != generation
                {
                    return;
                }
                if let Ok((friends, blacklist)) = result {
                    if let Ok(friends) = friends {
                        this.page_raw["friends"] =
                            serde_json::to_value(friends).unwrap_or(Value::Null);
                    }
                    this.page_raw["blacklist"] = blacklist;
                    let mapped = pages::page_data(Page::Friends, this.page_raw.clone());
                    this.summary = mapped.summary;
                    this.rows = mapped.rows;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        friend_interaction_steal_gid, friend_log_gid, friend_plant_patch,
        log_indicates_friend_visit_or_steal,
    };
    use qq_farm_app::events::PanelRealtimeEvent;
    use serde_json::json;

    fn event(event_type: &str, payload: serde_json::Value) -> PanelRealtimeEvent {
        PanelRealtimeEvent { event_type: event_type.into(), payload, account_id: None }
    }

    #[test]
    fn friend_plant_event_matches_vue_field_fallbacks_and_numeric_coercion() {
        let event = event(
            "friend_plant",
            json!({
                "friendGid": 0,
                "gid": "42",
                "plant": {"stealNum": "2", "dryNum": null},
                "stealNum": 3,
                "dryNum": "1",
                "weedNum": 4,
                "insectNum": 0
            }),
        );
        let (gid, plant) = friend_plant_patch(&event).expect("friend plant patch");
        assert_eq!(gid, 42);
        assert_eq!(plant, json!({"stealNum":2, "dryNum":1, "weedNum":4, "insectNum":0}));
    }

    #[test]
    fn friend_interaction_refreshes_only_successful_steal_actions() {
        assert_eq!(
            friend_interaction_steal_gid(&event(
                "friend_interact",
                json!({"action":"steal", "targetGid":"42", "result":"ok"})
            )),
            Some(Some(42))
        );
        assert_eq!(
            friend_interaction_steal_gid(&event(
                "friend_interact",
                json!({"event":"偷菜", "result":""})
            )),
            Some(None)
        );
        assert_eq!(
            friend_interaction_steal_gid(&event(
                "friend_interact",
                json!({"action":"steal", "result":"error"})
            )),
            None
        );
        assert_eq!(
            friend_interaction_steal_gid(&event("friend_interact", json!({"action":"help"}))),
            None
        );
    }

    #[test]
    fn friend_logs_match_visit_and_steal_signals_from_vue() {
        let visit = event("account-log:new", json!({"event":"visit_friend"}));
        let steal_count = event("worker_log", json!({"message":"偷2个苹果"}));
        let steal_phrase = event("log:new", json!({"message":"好友偷菜成功"}));
        let unrelated = event("log:new", json!({"message":"收获了2个苹果"}));
        assert!(log_indicates_friend_visit_or_steal(&visit));
        assert!(log_indicates_friend_visit_or_steal(&steal_count));
        assert!(log_indicates_friend_visit_or_steal(&steal_phrase));
        assert!(!log_indicates_friend_visit_or_steal(&unrelated));
        assert_eq!(friend_log_gid(&event("log:new", json!({"targetGid":"52"}))), Some(52));
    }
}
