mod view;

pub(super) fn has_claimable_reward(pass: &serde_json::Value) -> bool {
    pass.get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .any(|node| node.get("claimable").and_then(serde_json::Value::as_bool) == Some(true))
}

pub(super) fn claim_action_enabled(snapshot: &serde_json::Value) -> bool {
    snapshot.pointer("/actions/claimPass/enabled").and_then(serde_json::Value::as_bool).or_else(
        || snapshot.pointer("/capabilities/claimPass").and_then(serde_json::Value::as_bool),
    ) == Some(true)
}

#[cfg(test)]
mod tests {
    use super::{claim_action_enabled, has_claimable_reward};
    use serde_json::json;

    #[test]
    fn claim_all_requires_a_claimable_node_and_an_enabled_action() {
        let pass = json!({"nodes": [
            {"claimable": false, "claimed": true},
            {"claimable": true, "claimed": false}
        ]});
        assert!(has_claimable_reward(&pass));
        assert!(!has_claimable_reward(&json!({"nodes": [{"claimable": false}]})));
        assert!(claim_action_enabled(&json!({"actions": {"claimPass": {"enabled": true}}})));
        assert!(!claim_action_enabled(&json!({"actions": {"claimPass": {"enabled": false}}})));
        assert!(claim_action_enabled(&json!({"capabilities": {"claimPass": true}})));
    }
}
