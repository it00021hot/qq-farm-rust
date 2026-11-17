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
    tasks: Vec<GrowthTask>,
    gifts: Vec<GiftLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct GrowthTask {
    id: i64,
    desc: String,
    progress: TaskProgress,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TaskProgress {
    Idle,
    Done,
    Ratio { current: i64, total: i64 },
}

#[derive(Clone, PartialEq, Eq)]
struct GiftLine {
    key: String,
    label_key: &'static str,
    status_key: &'static str,
    detail: String,
    note: GiftNote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GiftNote {
    None,
    Schedule { hour: Option<i64> },
    Checking,
    Retry { time: Option<i64> },
    EmptyMail { time: Option<i64> },
    Recent { time: Option<i64> },
    Share { status: ShareNote, time: i64 },
    VipClosed,
    VipEmpty,
    MonthClosed,
    Finished { time: i64 },
    Ran { time: i64 },
    Checked { time: i64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShareNote {
    Open,
    Closed,
    Failed,
    Unknown,
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
            move || board.get().is_some_and(|board| board.tasks.is_empty()),
            move || notice(move || text_key("gifts.noGrowth")),
        ))
        .child(each(
            move || board.get().map(|board| board.tasks).unwrap_or_default(),
            |task: &GrowthTask| task.id.to_string(),
            move |task, _| growth_line(task.get_untracked()),
        ))
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

fn growth_line(task: GrowthTask) -> View {
    let desc = SharedString::from(task.desc);
    let progress = task.progress;
    view().flex_row().items_center().gap_3().py_2().child(text(desc).text_sm().flex_1()).child(
        text(move || SharedString::from(progress_copy(&progress)))
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
    )
}

fn progress_copy(progress: &TaskProgress) -> String {
    match progress {
        TaskProgress::Idle => text_key("gifts.notStarted").to_string(),
        TaskProgress::Done => text_key("gifts.completed").to_string(),
        TaskProgress::Ratio { current, total } => text_key("gifts.progress")
            .replace("{current}", &current.to_string())
            .replace("{total}", &total.to_string()),
    }
}

fn gift_line(gift: GiftLine) -> View {
    let detail = SharedString::from(gift.detail);
    let note = gift.note;
    let label_key = gift.label_key;
    let status_key = gift.status_key;
    let done = matches!(status_key, "gifts.done" | "gifts.checkedEmpty");
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(move || text_key(label_key)).text_sm().flex_1())
        .child(text(detail).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || note != GiftNote::None,
            move || {
                text(move || SharedString::from(note_copy(&note)))
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
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
    GiftBoard { growth: format!("{completed}/{total}"), tasks: growth_tasks(value), gifts }
}

fn growth_tasks(value: &Value) -> Vec<GrowthTask> {
    value
        .get("growth")
        .and_then(|growth| growth.get("tasks"))
        .and_then(Value::as_array)
        .map(|tasks| {
            tasks.iter().enumerate().map(|(index, task)| growth_task(index, task)).collect()
        })
        .unwrap_or_default()
}

fn growth_task(index: usize, value: &Value) -> GrowthTask {
    let id = value.get("id").and_then(Value::as_i64).unwrap_or(index as i64);
    let desc = value.get("desc").and_then(Value::as_str).unwrap_or("").trim();
    let desc = if desc.is_empty() { format!("成长任务#{id}") } else { desc.to_owned() };
    let current = value.get("progress").and_then(Value::as_i64).unwrap_or(0);
    let total = value.get("totalProgress").and_then(Value::as_i64).unwrap_or(0);
    GrowthTask { id, desc, progress: task_progress(current, total) }
}

fn task_progress(current: i64, total: i64) -> TaskProgress {
    if current == 0 && total == 0 {
        TaskProgress::Idle
    } else if total > 0 && current >= total {
        TaskProgress::Done
    } else {
        TaskProgress::Ratio { current, total }
    }
}

fn gift_line_from(value: &Value) -> Option<GiftLine> {
    let key = value.get("key").and_then(Value::as_str).filter(|key| !key.is_empty())?;
    let label_key = gift_label_key(key)?;
    Some(GiftLine {
        key: key.to_owned(),
        label_key,
        status_key: gift_status_key(value),
        detail: gift_detail(value),
        note: gift_note(value),
    })
}

fn gift_note(value: &Value) -> GiftNote {
    let key = value.get("key").and_then(Value::as_str).unwrap_or("");
    let result = value.get("result").and_then(Value::as_str).unwrap_or("");
    let done = value.get("doneToday").and_then(Value::as_bool).unwrap_or(false);
    let time = value.get("lastAt").and_then(Value::as_i64).filter(|time| *time > 0);
    if key == "email_rewards" {
        if result == "running" {
            return GiftNote::Checking;
        }
        if result == "error" || result == "partial" {
            return GiftNote::Retry { time };
        }
        if result == "none" && done {
            return GiftNote::EmptyMail { time };
        }
        if !done {
            return GiftNote::Schedule { hour: value.get("scheduledHour").and_then(Value::as_i64) };
        }
    }
    if result == "error" || result == "partial" {
        return GiftNote::Recent { time };
    }
    if key == "daily_share" && value.get("mode").and_then(Value::as_str) == Some("check_only") {
        let status = match value.get("checkStatus").and_then(Value::as_str).unwrap_or("") {
            "entry_available" => ShareNote::Open,
            "entry_unavailable" => ShareNote::Closed,
            "check_failed" => ShareNote::Failed,
            _ => ShareNote::Unknown,
        };
        return GiftNote::Share { status, time: time.unwrap_or(0) };
    }
    if key == "vip_daily_gift" {
        let is_vip = value.get("isVip").and_then(Value::as_bool);
        let has_gift = value.get("hasGift").and_then(Value::as_bool);
        if is_vip == Some(false) || (is_vip.is_none() && has_gift == Some(false)) {
            return GiftNote::VipClosed;
        }
        if is_vip == Some(true) && has_gift == Some(false) {
            return GiftNote::VipEmpty;
        }
    }
    if key == "month_card_gift" && value.get("hasCard").and_then(Value::as_bool) == Some(false) {
        return GiftNote::MonthClosed;
    }
    let Some(time) = time else {
        return GiftNote::None;
    };
    if done {
        GiftNote::Finished { time }
    } else if value.get("enabled").and_then(Value::as_bool) == Some(true) {
        GiftNote::Ran { time }
    } else {
        GiftNote::Checked { time }
    }
}

fn note_copy(note: &GiftNote) -> String {
    match note {
        GiftNote::None => String::new(),
        GiftNote::Schedule { hour: Some(hour) } => {
            text_key("gifts.schedule").replace("{hour}", &format!("{hour:02}"))
        }
        GiftNote::Schedule { hour: None } => text_key("gifts.scheduleAny").to_string(),
        GiftNote::Checking => text_key("gifts.emailChecking").to_string(),
        GiftNote::Retry { time } => timed("gifts.retry", "gifts.retryPending", *time),
        GiftNote::EmptyMail { time } => timed("gifts.mailEmptyAt", "gifts.mailEmpty", *time),
        GiftNote::Recent { time } => timed("gifts.recent", "gifts.retryLater", *time),
        GiftNote::Share { status, time } => share_copy(*status, *time),
        GiftNote::VipClosed => text_key("gifts.vipClosed").to_string(),
        GiftNote::VipEmpty => text_key("gifts.vipEmpty").to_string(),
        GiftNote::MonthClosed => text_key("gifts.monthClosed").to_string(),
        GiftNote::Finished { time } => {
            text_key("gifts.finishedAt").replace("{time}", &clock(*time))
        }
        GiftNote::Ran { time } => text_key("gifts.ranAt").replace("{time}", &clock(*time)),
        GiftNote::Checked { time } => text_key("gifts.checkedAt").replace("{time}", &clock(*time)),
    }
}

fn timed(with_time: &'static str, without_time: &'static str, time: Option<i64>) -> String {
    match time {
        Some(time) => text_key(with_time).replace("{time}", &clock(time)),
        None => text_key(without_time).to_string(),
    }
}

fn share_copy(status: ShareNote, time: i64) -> String {
    let label = text_key(match status {
        ShareNote::Open => "gifts.shareOpen",
        ShareNote::Closed => "gifts.shareClosed",
        ShareNote::Failed => "gifts.shareFailed",
        ShareNote::Unknown => "gifts.shareUnknown",
    });
    if time > 0 {
        format!("{label}{}", text_key("gifts.shareAt").replace("{time}", &clock(time)))
    } else {
        label.to_string()
    }
}

fn clock(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|time| time.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| "--".to_owned())
}

fn gift_detail(value: &Value) -> String {
    let result = text_field(value, "result");
    let error = text_field(value, "lastError");
    if matches!(result.as_str(), "error" | "partial") && !error.is_empty() {
        return error;
    }
    match (
        value.get("completedCount").and_then(Value::as_i64),
        value.get("totalCount").and_then(Value::as_i64),
    ) {
        (Some(completed), Some(total)) if total > 0 => format!("{completed}/{total}"),
        _ => String::new(),
    }
}

fn text_field(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
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
    let key = value.get("key").and_then(Value::as_str).unwrap_or("");
    let result = value.get("result").and_then(Value::as_str).unwrap_or("");
    let done = value.get("doneToday").and_then(Value::as_bool).unwrap_or(false);
    if key == "email_rewards" {
        return match result {
            "running" => "gifts.checking",
            "error" => "gifts.checkFailed",
            "partial" => "gifts.partial",
            "none" if done => "gifts.checkedEmpty",
            _ => fallback_status(value, done),
        };
    }
    if result == "error" {
        return "gifts.claimFailed";
    }
    if result == "partial" {
        return "gifts.partial";
    }
    if key == "mall_free_gifts" && result == "none" && done {
        return "gifts.noFree";
    }
    if key == "daily_share" && value.get("mode").and_then(Value::as_str) == Some("check_only") {
        return "gifts.shareOnly";
    }
    if key == "vip_daily_gift" {
        let is_vip = value.get("isVip").and_then(Value::as_bool);
        let has_gift = value.get("hasGift").and_then(Value::as_bool);
        if is_vip == Some(false) || (is_vip.is_none() && has_gift == Some(false)) {
            return "gifts.notOpened";
        }
        if is_vip == Some(true) && has_gift == Some(false) {
            return "gifts.noGift";
        }
    }
    if key == "month_card_gift" && value.get("hasCard").and_then(Value::as_bool) == Some(false) {
        return "gifts.notOpened";
    }
    fallback_status(value, done)
}

fn fallback_status(value: &Value, done: bool) -> &'static str {
    if done {
        "gifts.done"
    } else if value.get("enabled").and_then(Value::as_bool) == Some(false) {
        "gifts.off"
    } else {
        "gifts.pending"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{gift_board, gift_note, task_progress, GiftNote, ShareNote, TaskProgress};

    #[test]
    fn known_gifts_keep_progress_and_skip_unknown_keys() {
        let board = gift_board(&json!({
            "growth": {
                "completedCount": 1,
                "totalCount": 4,
                "tasks": [
                    { "id": 8, "desc": "收获作物", "progress": 3, "totalProgress": 10 },
                    { "id": 9, "desc": "  ", "progress": 2, "totalProgress": 2 },
                    { "progress": 0, "totalProgress": 0 }
                ]
            },
            "gifts": [
                { "key": "task_claim", "doneToday": false, "completedCount": 2, "totalCount": 5 },
                { "key": "vip_daily_gift", "doneToday": false, "result": "error", "lastError": "超时" },
                { "key": "email_rewards", "doneToday": true, "result": "none" },
                { "key": "month_card_gift", "hasCard": false, "doneToday": false },
                { "key": "task_claim", "enabled": false, "doneToday": false },
                { "key": "future_gift", "doneToday": true }
            ]
        }));
        assert_eq!(board.growth, "1/4");
        assert_eq!(board.gifts.len(), 5);
        assert_eq!(board.gifts[0].label_key, "gifts.task");
        assert_eq!(board.gifts[0].detail, "2/5");
        assert_eq!(board.gifts[0].status_key, "gifts.pending");
        assert_eq!(board.gifts[1].status_key, "gifts.claimFailed");
        assert_eq!(board.gifts[1].detail, "超时");
        assert_eq!(board.gifts[2].status_key, "gifts.checkedEmpty");
        assert_eq!(board.gifts[2].note, GiftNote::EmptyMail { time: None });
        assert_eq!(board.gifts[3].note, GiftNote::MonthClosed);
        assert_eq!(
            gift_note(&json!({
                "key": "email_rewards",
                "doneToday": false,
                "scheduledHour": 8
            })),
            GiftNote::Schedule { hour: Some(8) }
        );
        assert_eq!(
            gift_note(&json!({
                "key": "daily_share",
                "mode": "check_only",
                "checkStatus": "entry_available"
            })),
            GiftNote::Share { status: ShareNote::Open, time: 0 }
        );
        assert_eq!(board.gifts[3].status_key, "gifts.notOpened");
        assert_eq!(board.gifts[4].status_key, "gifts.off");
        assert_eq!(board.tasks[0].desc, "收获作物");
        assert_eq!(board.tasks[0].progress, TaskProgress::Ratio { current: 3, total: 10 });
        assert_eq!(board.tasks[1].progress, TaskProgress::Done);
        assert_eq!(board.tasks[2].desc, "成长任务#2");
        assert_eq!(task_progress(0, 0), TaskProgress::Idle);
    }
}
