use super::super::*;

mod view;

pub(super) fn selected_constellation_id(raw: &serde_json::Value, selected: &str) -> String {
    let Some(groups) = raw.pointer("/constellation/groups").and_then(serde_json::Value::as_array)
    else {
        return String::new();
    };
    if groups
        .iter()
        .any(|group| group.get("id").map(value_to_action_string).as_deref() == Some(selected))
    {
        return selected.to_string();
    }

    let mut ordered = groups.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|group| {
        group
            .get("order")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
            .unwrap_or(999)
    });
    let actionable = ordered.iter().find(|group| {
        matches!(
            group.get("visualState").and_then(serde_json::Value::as_str),
            Some("lightable" | "claimableUnknown")
        )
    });
    if let Some(id) = actionable.and_then(|group| group.get("id")) {
        return value_to_action_string(id);
    }

    let current_day = raw
        .pointer("/constellation/currentDay")
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()));
    if let Some(id) = current_day
        .and_then(|day| {
            ordered.iter().find(|group| {
                group
                    .get("order")
                    .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
                    == Some(day)
            })
        })
        .and_then(|group| group.get("id"))
    {
        return value_to_action_string(id);
    }
    if let Some(id) = ordered
        .iter()
        .find(|group| {
            group.get("visualState").and_then(serde_json::Value::as_str) == Some("unknown")
        })
        .and_then(|group| group.get("id"))
    {
        return value_to_action_string(id);
    }
    ordered
        .iter()
        .rev()
        .find(|group| group.get("visualState").and_then(serde_json::Value::as_str) == Some("lit"))
        .and_then(|group| group.get("id"))
        .map(value_to_action_string)
        .or_else(|| ordered.first().and_then(|group| group.get("id")).map(value_to_action_string))
        .unwrap_or_default()
}

pub(super) fn selected_constellation<'a>(
    raw: &'a serde_json::Value,
    selected_id: &str,
) -> Option<&'a serde_json::Value> {
    raw.pointer("/constellation/groups")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|group| group.get("id").map(value_to_action_string).as_deref() == Some(selected_id))
}

pub(super) fn selected_constellation_is_lightable(
    raw: &serde_json::Value,
    selected_id: &str,
) -> bool {
    let can_light = selected_constellation(raw, selected_id).is_some_and(|group| {
        matches!(
            group.get("visualState").and_then(serde_json::Value::as_str),
            Some("lightable" | "claimableUnknown")
        )
    });
    can_light && activity_action_enabled(raw, "lightConstellation") != Some(false)
}

#[cfg(test)]
mod tests {
    use super::{selected_constellation_id, selected_constellation_is_lightable};
    use serde_json::json;

    #[test]
    fn default_focus_prefers_actionable_group_and_preserves_a_valid_choice() {
        let raw = json!({
            "constellation": {
                "currentDay": 2,
                "groups": [
                    {"id": "future", "order": 3, "visualState": "locked"},
                    {"id": "today", "order": 2, "visualState": "claimableUnknown"},
                    {"id": "yesterday", "order": 1, "visualState": "unknown"}
                ]
            }
        });

        assert_eq!(selected_constellation_id(&raw, ""), "today");
        assert_eq!(selected_constellation_id(&raw, "future"), "future");
        assert_eq!(
            selected_constellation_id(
                &json!({"constellation": {"currentDay": 1, "groups": [
                    {"id": "later", "order": 2, "visualState": "unknown"},
                    {"id": "current", "order": 1, "visualState": "lit"}
                ]}}),
                ""
            ),
            "current"
        );
    }

    #[test]
    fn light_action_requires_selected_claimable_group_and_backend_gate() {
        let raw = json!({
            "actions": {"lightConstellation": {"enabled": true}},
            "constellation": {"groups": [
                {"id": "ready", "visualState": "lightable"},
                {"id": "locked", "visualState": "locked"}
            ]}
        });
        assert!(selected_constellation_is_lightable(&raw, "ready"));
        assert!(!selected_constellation_is_lightable(&raw, "locked"));
        assert!(!selected_constellation_is_lightable(&raw, "missing"));
        assert!(!selected_constellation_is_lightable(
            &json!({
                "actions": {"lightConstellation": {"enabled": false}},
                "constellation": {"groups": [{"id": "ready", "visualState": "lightable"}]}
            }),
            "ready"
        ));
    }
}
