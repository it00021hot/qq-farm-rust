use super::super::*;

mod view;

pub(super) fn selected_solar_term_id(raw: &serde_json::Value, selected: &str) -> String {
    let terms = raw.pointer("/solarTerms/terms").and_then(serde_json::Value::as_array);
    let Some(terms) = terms else { return String::new() };
    if terms
        .iter()
        .any(|term| term.get("id").map(value_to_action_string).as_deref() == Some(selected))
    {
        return selected.to_string();
    }
    let current_id =
        raw.pointer("/solarTerms/currentTermId").map(value_to_action_string).filter(|id| {
            terms
                .iter()
                .any(|term| term.get("id").map(value_to_action_string).as_deref() == Some(id))
        });
    current_id
        .or_else(|| {
            terms
                .iter()
                .find(|term| term.get("current").and_then(serde_json::Value::as_bool) == Some(true))
                .and_then(|term| term.get("id"))
                .map(value_to_action_string)
        })
        .or_else(|| terms.first().and_then(|term| term.get("id")).map(value_to_action_string))
        .unwrap_or_default()
}

pub(super) fn selected_solar_term<'a>(
    raw: &'a serde_json::Value,
    selected_id: &str,
) -> Option<&'a serde_json::Value> {
    raw.pointer("/solarTerms/terms")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|term| term.get("id").map(value_to_action_string).as_deref() == Some(selected_id))
}

pub(super) fn solar_term_claimable(term: &serde_json::Value) -> bool {
    (term.get("claimable").and_then(serde_json::Value::as_bool) == Some(true)
        || term.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true))
        && term.get("claimed").and_then(serde_json::Value::as_bool) != Some(true)
        && term.get("locked").and_then(serde_json::Value::as_bool) != Some(true)
}

#[cfg(test)]
mod tests {
    use super::{selected_solar_term, selected_solar_term_id, solar_term_claimable};
    use serde_json::json;

    #[test]
    fn selection_preserves_valid_choice_then_prefers_current_term() {
        let raw = json!({"solarTerms": {
            "currentTermId": 12,
            "terms": [
                {"id": 10, "name": "立春"},
                {"id": 12, "name": "雨水", "current": true}
            ]
        }});
        assert_eq!(selected_solar_term_id(&raw, "10"), "10");
        assert_eq!(selected_solar_term_id(&raw, "missing"), "12");
        assert_eq!(
            selected_solar_term(&raw, "12").and_then(|term| term.get("name")).unwrap(),
            "雨水"
        );
    }

    #[test]
    fn solar_claim_requires_available_unclaimed_unlocked_term() {
        assert!(solar_term_claimable(&json!({"canClaim": true})));
        assert!(!solar_term_claimable(&json!({"claimable": true, "claimed": true})));
        assert!(!solar_term_claimable(&json!({"canClaim": true, "locked": true})));
    }
}
