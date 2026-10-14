use std::time::Duration;

use super::super::*;

fn reward_label(item: &serde_json::Value) -> String {
    let name = item.get("name").map(value_to_action_string).unwrap_or_default().trim().to_string();
    let name = if name.is_empty() || name.chars().all(|character| character.is_ascii_digit()) {
        String::new()
    } else {
        name
    };
    let count = item
        .get("count")
        .filter(|count| !count.is_null())
        .map(value_to_action_string)
        .map(|count| format!("×{count}"))
        .unwrap_or_default();
    format!("{name}{count}").trim().to_string()
}

pub(super) fn action_result_message(result: &serde_json::Value, english: bool) -> (String, bool) {
    action_result_message_with_fallback(result, &[], english)
}

pub(super) fn action_result_message_with_fallback(
    result: &serde_json::Value,
    fallback_rewards: &[serde_json::Value],
    english: bool,
) -> (String, bool) {
    let informational = result.get("noClaimable").and_then(serde_json::Value::as_bool)
        == Some(true)
        || result.get("outcome").and_then(serde_json::Value::as_str) == Some("nothingToClaim");
    let message = result
        .get("message")
        .and_then(serde_json::Value::as_str)
        .filter(|message| !message.trim().is_empty())
        .map(str::to_string);
    let response_rewards = result
        .get("rewards")
        .or_else(|| result.pointer("/result/rewards"))
        .and_then(serde_json::Value::as_array);
    let should_use_fallback = response_rewards.is_none_or(Vec::is_empty);
    let rewards = if should_use_fallback && !fallback_rewards.is_empty() {
        fallback_rewards
    } else {
        response_rewards.map(Vec::as_slice).unwrap_or_default()
    }
    .iter()
    .map(reward_label)
    .filter(|label| !label.is_empty())
    .collect::<Vec<_>>();
    let message = message.unwrap_or_else(|| {
        if !rewards.is_empty() {
            let (claim_label, colon) =
                if english { ("Claimed", ": ") } else { ("领取成功", "：") };
            format!("{claim_label}{colon}{}", rewards.join("、"))
        } else if english {
            "Success".into()
        } else {
            "操作成功".into()
        }
    });
    (message, !informational)
}

pub(super) fn fallback_rewards(
    page_raw: &serde_json::Value,
    action: &str,
    target: Option<&str>,
) -> Vec<serde_json::Value> {
    match action {
        "battle-pass" => page_raw
            .pointer("/season/pass/nodes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|node| node.get("claimable").and_then(serde_json::Value::as_bool) == Some(true))
            .filter_map(|node| node.get("rewards").and_then(serde_json::Value::as_array))
            .flatten()
            .cloned()
            .collect(),
        "solar-term" => page_raw
            .pointer("/solarTerms/terms")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .find(|term| term.get("id").map(value_to_action_string).as_deref() == target)
            .and_then(|term| term.get("rewards").and_then(serde_json::Value::as_array))
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub(crate) fn star_sand_purchase_feedback(
    result: &serde_json::Value,
    item: Option<&serde_json::Value>,
    goods_id: &serde_json::Value,
    goods_name: &str,
    count: i64,
    english: bool,
) -> (String, bool) {
    let fallback = star_sand_fallback_reward(item, goods_id, goods_name, count);
    action_result_message_with_fallback(result, &[fallback], english)
}

fn star_sand_fallback_reward(
    item: Option<&serde_json::Value>,
    goods_id: &serde_json::Value,
    goods_name: &str,
    count: i64,
) -> serde_json::Value {
    let mut reward = match item.filter(|item| !item.is_null()) {
        Some(item) => item.clone(),
        None => serde_json::json!({"id": goods_id, "name": goods_name}),
    };
    let item_count = reward
        .get("count")
        .filter(|value| !value.is_null())
        .map(value_to_action_string)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value != 0.0)
        .unwrap_or(1.0);
    let quantity = item_count * count.max(1) as f64;
    let quantity = if quantity.fract() == 0.0 {
        serde_json::Number::from(quantity as i64)
    } else {
        serde_json::Number::from_f64(quantity).unwrap_or_else(|| serde_json::Number::from(1))
    };
    if let Some(reward) = reward.as_object_mut() {
        reward.insert("count".into(), serde_json::Value::Number(quantity));
    }
    reward
}

pub(super) fn merge_activity_snapshot(
    page_raw: &mut serde_json::Value,
    result: &serde_json::Value,
    action: &str,
) -> bool {
    if let Some(snapshot) = result.get("snapshot").filter(|value| value.is_object()) {
        let Some(page) = page_raw.as_object_mut() else { return false };
        let mut merged = false;
        for key in [
            "season",
            "constellation",
            "shop",
            "solarTerms",
            "qingMei",
            "greenPlum",
            "qixi",
            "charity",
            "activities",
            "capabilities",
            "actions",
            "errors",
        ] {
            if let Some(value) = snapshot.get(key) {
                page.insert(key.into(), value.clone());
                merged = true;
            }
        }
        return merged;
    }

    if action.starts_with("autumn-") {
        let Some(activity) = result.get("activity").filter(|value| value.is_object()) else {
            return false;
        };
        let key = if matches!(action, "autumn-draw" | "autumn-claim") { "wish" } else { "happy" };
        let Some(page) = page_raw.as_object_mut() else { return false };
        let autumn = page.entry("autumn").or_insert_with(|| serde_json::json!({}));
        let Some(autumn) = autumn.as_object_mut() else { return false };
        autumn.insert(key.into(), activity.clone());
        return true;
    }

    false
}

impl FarmDesktop {
    pub(in crate::desktop) fn show_activity_action_notice(
        &mut self,
        message: impl Into<String>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        self.activities.action_notice_generation =
            self.activities.action_notice_generation.wrapping_add(1);
        let generation = self.activities.action_notice_generation;
        self.activities.action_notice = Some((message.into(), success));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.activities.action_notice_generation == generation {
                    this.activities.action_notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        action_result_message, action_result_message_with_fallback, fallback_rewards,
        merge_activity_snapshot, star_sand_purchase_feedback,
    };
    use serde_json::json;

    #[test]
    fn action_feedback_uses_server_message_rewards_and_information_state() {
        assert_eq!(
            action_result_message(
                &json!({"rewards": [{"id": 9, "name": "元气糕", "count": 2}]}),
                false
            ),
            ("领取成功：元气糕×2".into(), true)
        );
        assert_eq!(
            action_result_message(&json!({"message": "今日已领取", "noClaimable": true}), false),
            ("今日已领取".into(), false)
        );
        assert_eq!(action_result_message(&json!({}), true), ("Success".into(), true));
        assert_eq!(
            action_result_message(&json!({"rewards": [{"id": 9, "count": 2}]}), true),
            ("Claimed: ×2".into(), true)
        );
        assert_eq!(
            action_result_message(&json!({"rewards": [{"id": 9, "count": 2}]}), false),
            ("领取成功：×2".into(), true)
        );
    }

    #[test]
    fn claim_feedback_uses_vue_fallback_rewards_and_preserves_server_messages() {
        let fallback = vec![json!({"name": "旅行种子", "count": 2})];
        assert_eq!(
            action_result_message_with_fallback(&json!({"rewards": []}), &fallback, false),
            ("领取成功：旅行种子×2".into(), true)
        );
        assert_eq!(
            action_result_message_with_fallback(
                &json!({"message": "已领取", "rewards": []}),
                &fallback,
                false
            ),
            ("已领取".into(), true)
        );
        assert_eq!(
            action_result_message_with_fallback(
                &json!({"result": {"rewards": [{"name": "服务端奖励"}]}}),
                &fallback,
                false
            ),
            ("领取成功：服务端奖励".into(), true)
        );
        assert_eq!(
            action_result_message_with_fallback(&json!({"noClaimable": true}), &[], true),
            ("Success".into(), false)
        );
    }

    #[test]
    fn pass_and_solar_claims_use_the_vue_snapshot_reward_fallbacks() {
        let page = json!({
            "season": {"pass": {"nodes": [
                {"claimable": false, "rewards": [{"name": "Locked"}]},
                {"claimable": true, "rewards": [{"name": "Seed"}, {"name": "Fertilizer"}]}
            ]}},
            "solarTerms": {"terms": [
                {"id": 1, "rewards": [{"name": "Spring"}]},
                {"id": "2", "rewards": [{"name": "Rain"}]}
            ]}
        });
        assert_eq!(
            fallback_rewards(&page, "battle-pass", None),
            vec![json!({"name": "Seed"}), json!({"name": "Fertilizer"})]
        );
        assert_eq!(fallback_rewards(&page, "solar-term", Some("2")), vec![json!({"name": "Rain"})]);
    }

    #[test]
    fn star_sand_feedback_multiplies_fallback_item_count_like_vue() {
        assert_eq!(
            star_sand_purchase_feedback(
                &json!({"rewards": []}),
                Some(&json!({"name": "Star seed", "count": "3"})),
                &json!(8),
                "Star seed",
                2,
                true,
            ),
            ("Claimed: Star seed×6".into(), true)
        );
        assert_eq!(
            star_sand_purchase_feedback(&json!({}), None, &json!(8), "Star seed", 4, false,),
            ("领取成功：Star seed×4".into(), true)
        );
    }

    #[test]
    fn snapshot_updates_activity_sections_without_erasing_other_page_data() {
        let mut page = json!({"petDiary": {"nurture": {"growth": 4}}, "season": {"level": 1}});
        assert!(merge_activity_snapshot(
            &mut page,
            &json!({"snapshot": {"season": {"level": 2}, "activities": [{"id": "qixi"}]}}),
            "battle-pass"
        ));
        assert_eq!(page["season"]["level"], 2);
        assert_eq!(page["activities"][0]["id"], "qixi");
        assert_eq!(page["petDiary"]["nurture"]["growth"], 4);

        assert!(merge_activity_snapshot(
            &mut page,
            &json!({"activity": {"score": 22}}),
            "autumn-happy-daily"
        ));
        assert_eq!(page["autumn"]["happy"]["score"], 22);
    }
}
