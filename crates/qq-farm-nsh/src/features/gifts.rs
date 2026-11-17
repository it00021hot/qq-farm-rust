use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, AccountRow};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, notice, BadgeTone, ButtonVariant};

#[derive(Clone, PartialEq, Eq)]
struct GiftBoard {
    growth: String,
    gifts: Vec<GiftLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct GiftLine {
    key: String,
    label_key: &'static str,
    status_key: &'static str,
    detail: String,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let board = resource(
        move || running_account(account_id, rows),
        |id, _| async move {
            let context = session::session().context.clone();
            let value = session::call(async move {
                qq_farm_app::farm::daily_gift_overview(&context, &id).await
            })
            .await?;
            Ok(gift_board(&value))
        },
    );
    view()
        .gap_3()
        .child(gate_notice(account_id, rows, "farm.needRunning"))
        .child(when(
            move || running(account_id, rows) && board.loading() && board.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || running(account_id, rows) && board.error().is_some(),
            move || load_error(board),
        ))
        .child(when(
            move || running(account_id, rows) && board.get().is_some(),
            move || gift_body(board),
        ))
}

fn running(account_id: Signal<Option<String>>, rows: Resource<Vec<AccountRow>, String>) -> bool {
    account_id.get().zip(rows.get()).is_some_and(|(id, rows)| account_is_running(&rows, &id))
}

fn running_account(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> Option<String> {
    running(account_id, rows).then(|| account_id.get()).flatten()
}

fn load_error(board: Resource<GiftBoard, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            board.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || board.refetch()),
        )
}

fn gift_body(board: Resource<GiftBoard, String>) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .on_press(move || board.refetch()),
        )
        .child(
            text(move || {
                let growth = board.get().map(|board| board.growth).unwrap_or_default();
                SharedString::from(format!("{} {growth}", text_key("gifts.growth")))
            })
            .text_sm(),
        )
        .child(when(
            move || board.get().is_some_and(|board| board.gifts.is_empty()),
            move || notice(move || text_key("gifts.empty")),
        ))
        .child(each(
            move || board.get().map(|board| board.gifts).unwrap_or_default(),
            |gift: &GiftLine| gift.key.clone(),
            move |gift, _| gift_line(gift.get_untracked()),
        ))
}

fn gift_line(gift: GiftLine) -> View {
    let detail = SharedString::from(gift.detail);
    let label_key = gift.label_key;
    let status_key = gift.status_key;
    let done = status_key == "gifts.done";
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(move || text_key(label_key)).text_sm().flex_1())
        .child(text(detail).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(badge(
            move || text_key(status_key),
            if done { BadgeTone::Success } else { BadgeTone::Neutral },
        ))
}

fn gift_board(value: &Value) -> GiftBoard {
    let growth = value.get("growth");
    let completed =
        growth.and_then(|item| item.get("completedCount")).and_then(Value::as_i64).unwrap_or(0);
    let total = growth.and_then(|item| item.get("totalCount")).and_then(Value::as_i64).unwrap_or(0);
    let gifts = value
        .get("gifts")
        .and_then(Value::as_array)
        .map(|gifts| gifts.iter().filter_map(gift_line_from).collect())
        .unwrap_or_default();
    GiftBoard { growth: format!("{completed}/{total}"), gifts }
}

fn gift_line_from(value: &Value) -> Option<GiftLine> {
    let key = value.get("key").and_then(Value::as_str).filter(|key| !key.is_empty())?;
    let label_key = gift_label_key(key)?;
    let error = value.get("lastError").and_then(Value::as_str).unwrap_or("");
    let completed = value.get("completedCount").and_then(Value::as_i64);
    let total = value.get("totalCount").and_then(Value::as_i64);
    let detail = if !error.is_empty() {
        error.to_owned()
    } else if let (Some(completed), Some(total)) = (completed, total) {
        format!("{completed}/{total}")
    } else {
        String::new()
    };
    Some(GiftLine { key: key.to_owned(), label_key, status_key: gift_status_key(value), detail })
}

fn gift_label_key(key: &str) -> Option<&'static str> {
    Some(match key {
        "task_claim" => "gifts.task",
        "email_rewards" => "gifts.email",
        "mall_free_gifts" => "gifts.mallFree",
        "daily_share" => "gifts.share",
        "vip_daily_gift" => "gifts.vip",
        "month_card_gift" => "gifts.month",
        _ => return None,
    })
}

fn gift_status_key(value: &Value) -> &'static str {
    if value.get("doneToday").and_then(Value::as_bool).unwrap_or(false) {
        "gifts.done"
    } else if value.get("canClaim").and_then(Value::as_bool).unwrap_or(false) {
        "gifts.ready"
    } else {
        "gifts.pending"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::gift_board;

    #[test]
    fn known_gifts_keep_progress_and_skip_unknown_keys() {
        let board = gift_board(&json!({
            "growth": { "completedCount": 1, "totalCount": 4 },
            "gifts": [
                { "key": "task_claim", "doneToday": false, "completedCount": 2, "totalCount": 5 },
                { "key": "vip_daily_gift", "doneToday": false, "canClaim": true, "lastError": "超时" },
                { "key": "future_gift", "doneToday": true }
            ]
        }));
        assert_eq!(board.growth, "1/4");
        assert_eq!(board.gifts.len(), 2);
        assert_eq!(board.gifts[0].label_key, "gifts.task");
        assert_eq!(board.gifts[0].detail, "2/5");
        assert_eq!(board.gifts[0].status_key, "gifts.pending");
        assert_eq!(board.gifts[1].status_key, "gifts.ready");
        assert_eq!(board.gifts[1].detail, "超时");
    }
}
