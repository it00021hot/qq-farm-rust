//! 出售条件（对齐 bot `sell-conditions.ts`）。

use super::activity_windows::{activity_window_by_id, activity_windows_loaded, ActivityWindow};

/// 评估 `sell_cond` 所需的运行时上下文。
#[derive(Debug, Clone, Default)]
pub struct SellConditionContext {
    pub now_sec: i64,
    pub expire_time: i64,
    pub activity_windows_loaded: bool,
}

impl SellConditionContext {
    #[must_use]
    pub fn now(now_sec: i64) -> Self {
        Self {
            now_sec,
            expire_time: 0,
            activity_windows_loaded: activity_windows_loaded(),
        }
    }

    #[must_use]
    pub fn with_expire(mut self, expire_time: i64) -> Self {
        self.expire_time = expire_time;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSellCondition {
    type_name: String,
    value: String,
}

fn parse_sell_conditions(condition: &str) -> Vec<ParsedSellCondition> {
    condition
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| match part.split_once(':') {
            Some((ty, value)) => {
                ParsedSellCondition { type_name: ty.trim().to_string(), value: value.trim().to_string() }
            }
            None => ParsedSellCondition { type_name: part.to_string(), value: String::new() },
        })
        .collect()
}

fn is_activity_ended(window: Option<&ActivityWindow>, now_sec: i64) -> bool {
    match window {
        Some(w) => w.end_time <= now_sec,
        None => true,
    }
}

fn is_activity_active(window: Option<&ActivityWindow>, now_sec: i64) -> bool {
    match window {
        Some(w) => w.begin_time <= now_sec && now_sec <= w.end_time,
        None => false,
    }
}

fn is_single_sell_condition_satisfied(cond: &ParsedSellCondition, ctx: &SellConditionContext) -> bool {
    if cond.type_name == "道具过期后" {
        return ctx.expire_time > 0 && ctx.now_sec >= ctx.expire_time;
    }
    if !ctx.activity_windows_loaded || cond.value.is_empty() {
        return false;
    }
    let window = activity_window_by_id(&cond.value);
    match cond.type_name.as_str() {
        "活动结束后" => is_activity_ended(window.as_ref(), ctx.now_sec),
        "活动结束前" => !is_activity_ended(window.as_ref(), ctx.now_sec),
        "活动区间外" => !is_activity_active(window.as_ref(), ctx.now_sec),
        _ => false,
    }
}

/// 分号拼接的 `sell_cond` 是否全部满足。
#[must_use]
pub fn is_sell_condition_satisfied(condition: &str, ctx: &SellConditionContext) -> bool {
    let conditions = parse_sell_conditions(condition);
    !conditions.is_empty()
        && conditions.iter().all(|entry| is_single_sell_condition_satisfied(entry, ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::activity_windows::{clear_activity_windows_for_test, set_activity_windows};

    fn window(id: &str, begin: i64, end: i64) -> ActivityWindow {
        ActivityWindow { id: id.to_string(), name: id.to_string(), begin_time: begin, end_time: end }
    }

    #[test]
    fn item_expire_requires_timestamp() {
        let ctx = SellConditionContext { now_sec: 100, expire_time: 0, activity_windows_loaded: true };
        assert!(!is_sell_condition_satisfied("道具过期后", &ctx));
        let ctx = SellConditionContext { now_sec: 100, expire_time: 90, activity_windows_loaded: true };
        assert!(is_sell_condition_satisfied("道具过期后", &ctx));
    }

    #[test]
    fn activity_ended_uses_cached_window() {
        clear_activity_windows_for_test();
        set_activity_windows(vec![window("2026081800", 1, 50)]);
        let ctx = SellConditionContext { now_sec: 60, expire_time: 0, activity_windows_loaded: true };
        assert!(is_sell_condition_satisfied("活动结束后:2026081800", &ctx));
        let ctx = SellConditionContext { now_sec: 40, expire_time: 0, activity_windows_loaded: true };
        assert!(!is_sell_condition_satisfied("活动结束后:2026081800", &ctx));
        clear_activity_windows_for_test();
    }
}
