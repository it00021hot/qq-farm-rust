use crate::desktop::Page;
use qq_farm_app::events::PanelRealtimeEvent;

pub(super) fn account_list_needs_refresh(event_type: &str, payload: &serde_json::Value) -> bool {
    match event_type {
        "account_status" => true,
        "status:update" => {
            payload.pointer("/status/wxAuthorized").and_then(serde_json::Value::as_bool)
                == Some(false)
        }
        _ => false,
    }
}

pub(super) fn should_ignore_other_account_event(
    page: Page,
    selected_account: Option<&str>,
    event: &PanelRealtimeEvent,
) -> bool {
    let belongs_to_other_account = realtime_account_id(event)
        .is_some_and(|id| selected_account.is_none_or(|selected| !same_account_id(&id, selected)));
    belongs_to_other_account
        && !(page == Page::Accounts
            && account_list_needs_refresh(&event.event_type, &event.payload))
}

fn realtime_account_id(event: &PanelRealtimeEvent) -> Option<String> {
    if let Some(value) = event.payload.get("accountId").filter(|value| account_id_truthy(value)) {
        let id = match value {
            serde_json::Value::String(id) => id.clone(),
            serde_json::Value::Number(id) => id.to_string(),
            serde_json::Value::Bool(id) => id.to_string(),
            _ => String::new(),
        };
        return (!id_is_zero(&id)).then_some(id);
    }
    event.account_id.as_ref().filter(|id| !id.is_empty()).filter(|id| !id_is_zero(id)).cloned()
}

fn id_is_zero(id: &str) -> bool {
    id.parse::<f64>().is_ok_and(|id| id == 0.0)
}

fn account_id_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

fn same_account_id(left: &str, right: &str) -> bool {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        account_list_needs_refresh, realtime_account_id, same_account_id,
        should_ignore_other_account_event,
    };
    use crate::desktop::Page;
    use serde_json::json;

    fn event(
        event_type: &str,
        account_id: Option<&str>,
        payload: serde_json::Value,
    ) -> qq_farm_app::events::PanelRealtimeEvent {
        qq_farm_app::events::PanelRealtimeEvent {
            event_type: event_type.into(),
            payload,
            account_id: account_id.map(str::to_owned),
        }
    }

    #[test]
    fn account_list_refresh_events_match_vue_account_page_subscriptions() {
        assert!(account_list_needs_refresh("account_status", &json!({})));
        assert!(account_list_needs_refresh(
            "status:update",
            &json!({ "status": { "wxAuthorized": false } })
        ));
        assert!(!account_list_needs_refresh(
            "status:update",
            &json!({ "status": { "wxAuthorized": true } })
        ));
        assert!(!account_list_needs_refresh(
            "status:update",
            &json!({ "status": { "runStatus": 1 } })
        ));
        assert!(!account_list_needs_refresh("log:new", &json!({})));
    }

    #[test]
    fn account_page_refreshes_for_other_accounts_without_leaking_other_page_events() {
        let lifecycle = event("account_status", Some("other"), json!({}));
        assert!(!should_ignore_other_account_event(Page::Accounts, Some("selected"), &lifecycle));
        assert!(should_ignore_other_account_event(Page::Dashboard, Some("selected"), &lifecycle));

        let expired_auth =
            event("status:update", Some("other"), json!({ "status": { "wxAuthorized": false } }));
        assert!(!should_ignore_other_account_event(
            Page::Accounts,
            Some("selected"),
            &expired_auth
        ));

        let unrelated_log = event("log:new", Some("other"), json!({}));
        assert!(should_ignore_other_account_event(
            Page::Accounts,
            Some("selected"),
            &unrelated_log
        ));
    }

    #[test]
    fn event_account_filter_uses_vue_payload_precedence_and_numeric_ids() {
        let payload_id = event("friend_plant", Some("selected"), json!({"accountId": "42"}));
        assert_eq!(realtime_account_id(&payload_id).as_deref(), Some("42"));
        assert!(same_account_id("042", "42"));
        assert!(should_ignore_other_account_event(Page::Friends, Some("7"), &payload_id));

        let zero_payload_id = event("friend_plant", Some("7"), json!({"accountId": 0}));
        assert_eq!(realtime_account_id(&zero_payload_id).as_deref(), Some("7"));
        assert!(!should_ignore_other_account_event(Page::Friends, Some("7"), &zero_payload_id));

        let missing_payload_id = event("friend_plant", Some("7"), json!({}));
        assert!(!should_ignore_other_account_event(Page::Friends, Some("7"), &missing_payload_id));
    }
}
