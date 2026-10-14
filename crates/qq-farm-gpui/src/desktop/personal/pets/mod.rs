mod state;
pub(in crate::desktop) use state::PetsState;

mod actions;
mod food;
mod logs;
mod refresh;
mod view;

use super::super::*;

#[derive(Clone, Copy)]
enum PetAction {
    Activate(i64),
    Deploy(i64),
    Withdraw,
    Food(i64, i64),
    Claim,
    Logs,
}

pub(super) fn dog_action_gates(dog: &serde_json::Value) -> (bool, bool) {
    let owned = dog.get("owned").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let active = dog.get("active").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let activatable = dog.get("activatable").and_then(serde_json::Value::as_bool).unwrap_or(false);
    (!owned && activatable, owned && !active)
}

pub(super) fn duration_label(seconds: i64, language: &str) -> String {
    if language == "en-US" {
        return if seconds <= 0 {
            "0d 0h".into()
        } else {
            format!("{}d {}h", seconds / 86_400, seconds % 86_400 / 3_600)
        };
    }
    if seconds <= 0 {
        "0 天".into()
    } else {
        format!("{} 天 {} 时", seconds / 86_400, seconds % 86_400 / 3_600)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dog_actions_follow_owned_activatable_and_active_fields() {
        use serde_json::json;
        assert_eq!(
            super::dog_action_gates(&json!({"owned":false,"activatable":true,"active":false})),
            (true, false)
        );
        assert_eq!(super::dog_action_gates(&json!({"owned":true,"active":false})), (false, true));
        assert_eq!(
            super::dog_action_gates(&json!({"owned":false,"activatable":false})),
            (false, false)
        );
        assert_eq!(super::dog_action_gates(&json!({"owned":true,"active":true})), (false, false));
    }

    #[test]
    fn protection_duration_matches_vue_day_hour_format() {
        assert_eq!(super::duration_label(0, "zh-CN"), "0 天");
        assert_eq!(super::duration_label(93_600, "zh-CN"), "1 天 2 时");
        assert_eq!(super::duration_label(93_600, "en-US"), "1d 2h");
    }
}
