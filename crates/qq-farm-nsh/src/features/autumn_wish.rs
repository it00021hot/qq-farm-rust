use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

const ACTIVITY_ID: &str = "2026092401";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WishIntent {
    Reload,
    Draw,
    Claim,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct WishSession {
    pub account_id: String,
    pub generation: u64,
    pub busy: bool,
    pub board: Option<WishBoard>,
    pub failure: Option<String>,
    pub note: Option<String>,
    pub attempted: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct WishBoard {
    title: String,
    active: bool,
    window_key: &'static str,
    remaining: String,
    day: String,
    draws: i64,
    choices: Vec<WishChoice>,
    days: Vec<WishDay>,
    pending: Option<WishSlip>,
    can_draw: bool,
    can_claim: bool,
    rules: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct WishChoice {
    id: i64,
    name: String,
}

#[derive(Clone, PartialEq, Eq)]
struct WishDay {
    day: i64,
    name: String,
    count: i64,
    today: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct WishSlip {
    choose_id: i64,
    text: String,
    rewards: Vec<WishReward>,
}

#[derive(Clone, PartialEq, Eq)]
struct WishReward {
    id: i64,
    name: String,
    count: i64,
}

#[derive(Clone)]
pub(crate) struct WishCommandOwned {
    account_id: String,
    generation: u64,
    intent: WishIntent,
    choose_id: i64,
}

pub(crate) enum WishApply {
    Loaded(Result<WishBoard, String>),
    Wrote(Result<Value, String>),
}

pub(crate) fn window_key(start_ms: i64, end_ms: i64, now_ms: i64, active: bool) -> &'static str {
    if start_ms > 0 && start_ms > now_ms {
        "activities.wishUpcoming"
    } else if end_ms > 0 && now_ms >= end_ms {
        "activities.wishEnded"
    } else if active {
        "activities.wishOpen"
    } else {
        "activities.wishUnknown"
    }
}

pub(crate) fn switch_group(group: i64) -> (i64, i64) {
    let next = if group == 0 { 1 } else { 0 };
    (next, next * 3 + 1)
}

pub(crate) fn board_from(value: &Value) -> Option<WishBoard> {
    if value.get("id").and_then(Value::as_str) != Some(ACTIVITY_ID) {
        return None;
    }
    let start_ms = value.get("startTime").and_then(Value::as_i64).unwrap_or(0);
    let end_ms = value.get("endTime").and_then(Value::as_i64).unwrap_or(0);
    let now_ms = value.get("serverTime").and_then(Value::as_i64).unwrap_or(0);
    let active = value.get("active").and_then(Value::as_bool) == Some(true);
    let day = value.get("day").and_then(Value::as_i64);
    let choices = value
        .get("choices")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(choice_from).collect())
        .unwrap_or_default();
    let days = value
        .get("rewardDays")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| day_from(row, day)).collect())
        .unwrap_or_default();
    Some(WishBoard {
        title: value.get("title").and_then(Value::as_str).unwrap_or("秋祈良愿").to_owned(),
        active,
        window_key: window_key(start_ms, end_ms, now_ms, active),
        remaining: remaining_line(
            end_ms,
            now_ms,
            active,
            window_key(start_ms, end_ms, now_ms, active),
        ),
        day: day.map(|day| day.to_string()).unwrap_or_else(|| "-".into()),
        draws: value.get("remaining").and_then(Value::as_i64).unwrap_or(0),
        choices,
        days,
        pending: value.get("pending").filter(|pending| !pending.is_null()).and_then(slip_from),
        can_draw: value.get("canDraw").and_then(Value::as_bool) == Some(true),
        can_claim: value.get("canClaim").and_then(Value::as_bool) == Some(true),
        rules: value
            .get("rules")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_str()).map(str::to_owned).collect())
            .unwrap_or_default(),
    })
}

pub(crate) fn press(
    session: &mut WishSession,
    intent: WishIntent,
    choose_id: i64,
    running: bool,
) -> Option<WishCommandOwned> {
    if !running || session.account_id.is_empty() || session.busy {
        return None;
    }
    let chosen = match intent {
        WishIntent::Reload => 0,
        WishIntent::Draw => {
            let board = session.board.as_ref()?;
            if !board.can_draw
                || board.pending.is_some()
                || !board.choices.iter().any(|choice| choice.id == choose_id)
            {
                return None;
            }
            choose_id
        }
        WishIntent::Claim => {
            let board = session.board.as_ref()?;
            let pending = board.pending.as_ref()?;
            if !board.can_claim
                || !board.choices.iter().any(|choice| choice.id == pending.choose_id)
            {
                return None;
            }
            pending.choose_id
        }
    };
    session.busy = true;
    session.attempted = true;
    session.generation += 1;
    session.failure = None;
    Some(WishCommandOwned {
        account_id: session.account_id.clone(),
        generation: session.generation,
        intent,
        choose_id: chosen,
    })
}

pub(crate) fn reset_account(session: &mut WishSession, account_id: String) {
    if session.account_id == account_id {
        return;
    }
    session.account_id = account_id;
    session.generation += 1;
    session.busy = false;
    session.board = None;
    session.failure = None;
    session.note = None;
    session.attempted = false;
}

pub(crate) fn apply(
    session: &mut WishSession,
    command: &WishCommandOwned,
    outcome: WishApply,
) -> bool {
    if session.account_id != command.account_id || session.generation != command.generation {
        return false;
    }
    session.busy = false;
    match outcome {
        WishApply::Loaded(Ok(board)) => {
            session.board = Some(board);
            session.failure = None;
        }
        WishApply::Loaded(Err(message)) => session.failure = Some(message),
        WishApply::Wrote(Err(message)) => {
            session.board = None;
            session.note = None;
            session.failure = Some(message);
        }
        WishApply::Wrote(Ok(value)) => apply_write(session, command.intent, &value),
    }
    true
}

pub(crate) fn watch(account: impl Fn() -> Option<String> + 'static, session: Signal<WishSession>) {
    effect(move || {
        let next = account().unwrap_or_default();
        let current = session.get_untracked();
        if current.account_id != next {
            let mut owned = current;
            reset_account(&mut owned, next.clone());
            session.set(owned);
        }
        let current = session.get_untracked();
        if next.is_empty() || current.busy || current.board.is_some() || current.attempted {
            return;
        }
        let mut owned = current;
        if let Some(command) = press(&mut owned, WishIntent::Reload, 0, true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

pub(crate) fn panel(
    session: Signal<WishSession>,
    page: ActivityPage,
    group: Signal<i64>,
    choice: Signal<i64>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || session.get().busy && session.get().board.is_none(),
            move || notice(move || text_key("activities.wishLoading")),
        ))
        .child(when(
            move || session.get().failure.is_some(),
            move || {
                crate::ui::failure(move || {
                    SharedString::from(session.get().failure.clone().unwrap_or_default())
                })
            },
        ))
        .child(when(
            move || !session.get().busy && session.get().board.is_none(),
            move || {
                view().gap_2().child(notice(move || text_key("activities.wishEmpty"))).child(
                    button(move || text_key("activities.wishReload"), ButtonVariant::Secondary)
                        .on_press(move || fire(session, page, WishIntent::Reload, 0)),
                )
            },
        ))
        .child(when(
            move || session.get().note.is_some(),
            move || {
                notice(move || SharedString::from(session.get().note.clone().unwrap_or_default()))
            },
        ))
        .child(when(
            move || session.get().board.is_some(),
            move || body(session, page, group, choice),
        ))
}

fn fire(session: Signal<WishSession>, page: ActivityPage, intent: WishIntent, choose_id: i64) {
    let running = page
        .account_id
        .get_untracked()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| super::accounts::account_is_running(&rows, &id));
    let mut owned = session.get_untracked();
    let Some(command) = press(&mut owned, intent, choose_id, running) else {
        return;
    };
    session.set(owned);
    launch(session, command);
}

fn launch(session: Signal<WishSession>, command: WishCommandOwned) {
    spawn(async move {
        let context = session::session().context.clone();
        let account_id = command.account_id.clone();
        let choose_id = command.choose_id;
        let intent = command.intent;
        let result = session::call(async move {
            match intent {
                WishIntent::Reload => {
                    qq_farm_app::autumn_wish::snapshot(&context, &account_id).await
                }
                WishIntent::Draw => {
                    qq_farm_app::autumn_wish::operate(&context, &account_id, "draw", choose_id)
                        .await
                }
                WishIntent::Claim => {
                    qq_farm_app::autumn_wish::operate(&context, &account_id, "claim", choose_id)
                        .await
                }
            }
        })
        .await;
        let mut owned = session.get_untracked();
        let outcome = match intent {
            WishIntent::Reload => WishApply::Loaded(result.and_then(|value| {
                board_from(&value).ok_or_else(|| "活动动态状态未返回，请稍后刷新".into())
            })),
            WishIntent::Draw | WishIntent::Claim => WishApply::Wrote(result),
        };
        let _accepted = apply(&mut owned, &command, outcome);
        session.set(owned);
    });
}

fn body(
    session: Signal<WishSession>,
    page: ActivityPage,
    group: Signal<i64>,
    choice: Signal<i64>,
) -> impl IntoElement {
    let board = session.get().board.clone().unwrap_or_else(empty_board);
    let title = board.title.clone();
    let window_key = board.window_key;
    let remaining = board.remaining.clone();
    let day = board.day.clone();
    let draws = board.draws;
    view()
        .gap_3()
        .child(text(title).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(text(move || text_key("activities.wishTagline")).text_sm())
        .child(text(move || text_key("activities.wishIntro")).text_sm())
        .child(text(move || text_key(window_key)).text_sm())
        .child(text(remaining).text_sm())
        .child(text(move || text_key("activities.wishNoCost")).text_sm())
        .child(text(format!(
            "{} {day} · {} {draws}",
            text_key("activities.wishDay"),
            text_key("activities.wishDraws")
        )))
        .child(slip_or_choices(session, page, group, choice, &board))
        .child(days(board.days))
        .child(rules(board.rules))
}

fn slip_or_choices(
    session: Signal<WishSession>,
    page: ActivityPage,
    group: Signal<i64>,
    choice: Signal<i64>,
    board: &WishBoard,
) -> impl IntoElement {
    let slip = board.pending.clone();
    let can_claim = board.can_claim;
    let can_draw = board.can_draw;
    let choices = board.choices.clone();
    let slip_text = slip.as_ref().map(|slip| {
        if slip.text.is_empty() {
            text_key("activities.wishFallback").to_string()
        } else {
            slip.text.clone()
        }
    });
    let slip_rewards = slip.as_ref().map(|slip| reward_line(&slip.rewards)).unwrap_or_default();
    let has_slip = slip_text.is_some();
    view()
        .gap_2()
        .child(when(move || has_slip, {
            let slip_text = slip_text.clone().unwrap_or_default();
            let slip_rewards = slip_rewards.clone();
            move || {
                view()
                    .gap_2()
                    .child(text(move || text_key("activities.wishPending")))
                    .child(text(slip_text.clone()))
                    .child(text(slip_rewards.clone()).text_sm())
                    .child(
                        button(move || text_key("activities.wishClaim"), ButtonVariant::Primary)
                            .disabled(move || session.get().busy || !can_claim)
                            .on_press(move || fire(session, page, WishIntent::Claim, 0)),
                    )
            }
        }))
        .child(when(
            move || !has_slip,
            move || choice_panel(session, page, group, choice, can_draw, choices.clone()),
        ))
}

fn choice_panel(
    session: Signal<WishSession>,
    page: ActivityPage,
    group: Signal<i64>,
    choice: Signal<i64>,
    can_draw: bool,
    choices: Vec<WishChoice>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(text(move || {
            if can_draw {
                text_key("activities.wishReady")
            } else {
                text_key("activities.wishDone")
            }
        }))
        .child(each(
            move || visible_choices(&choices, group.get()),
            |choice_row: &WishChoice| choice_row.id,
            move |choice_row, _| {
                let id = choice_row.get_untracked().id;
                let name = choice_row.get_untracked().name.clone();
                button(move || SharedString::from(name.clone()), ButtonVariant::Quiet)
                    .selected(move || choice.get() == id)
                    .disabled(move || session.get().busy || !can_draw)
                    .on_press(move || choice.set(id))
            },
        ))
        .child(
            button(move || text_key("activities.wishSwitch"), ButtonVariant::Secondary)
                .disabled(move || session.get().busy || !can_draw)
                .on_press(move || {
                    let (next_group, next_choice) = switch_group(group.get_untracked());
                    group.set(next_group);
                    choice.set(next_choice);
                }),
        )
        .child(
            button(move || text_key("activities.wishDraw"), ButtonVariant::Primary)
                .disabled(move || session.get().busy || !can_draw)
                .on_press(move || fire(session, page, WishIntent::Draw, choice.get_untracked())),
        )
}

fn days(days: Vec<WishDay>) -> impl IntoElement {
    view().gap_2().child(text(move || text_key("activities.wishDays"))).child(each(
        move || days.clone(),
        |day: &WishDay| day.day,
        move |day, _| {
            let day = day.get_untracked();
            let label = if day.today {
                text_key("activities.wishToday").to_string()
            } else {
                format!("{} {}", text_key("activities.wishDayNumber"), day.day)
            };
            text(format!("{label} {} ×{}", day.name, day.count)).text_sm()
        },
    ))
}

fn rules(rules: Vec<String>) -> impl IntoElement {
    view().gap_1().child(text(move || text_key("activities.wishRules"))).child(each(
        move || rules.clone(),
        |rule: &String| rule.clone(),
        move |rule, _| text(rule.get_untracked().clone()).text_sm(),
    ))
}

fn visible_choices(choices: &[WishChoice], group: i64) -> Vec<WishChoice> {
    let start = usize::try_from(group.max(0)).unwrap_or(0).saturating_mul(3);
    choices.iter().skip(start).take(3).cloned().collect()
}

fn choice_from(value: &Value) -> Option<WishChoice> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
    (!name.is_empty()).then_some(WishChoice { id, name })
}

fn day_from(value: &Value, today: Option<i64>) -> Option<WishDay> {
    let day = value.get("day").and_then(Value::as_i64)?;
    let reward = value.get("reward")?;
    let count = reward.get("count").and_then(Value::as_i64).unwrap_or(0);
    let name = reward.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
    Some(WishDay { day, name, count, today: Some(day) == today })
}

fn slip_from(value: &Value) -> Option<WishSlip> {
    let choose_id = value.get("chooseId").and_then(Value::as_i64).unwrap_or(0);
    let text = value.get("text").and_then(Value::as_str).unwrap_or("").to_owned();
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(reward_from).collect())
        .unwrap_or_default();
    Some(WishSlip { choose_id, text, rewards })
}

fn reward_from(value: &Value) -> Option<WishReward> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("物品 #{id}"));
    Some(WishReward { id, name, count })
}

fn reward_line(rewards: &[WishReward]) -> String {
    rewards
        .iter()
        .map(|reward| format!("{} ×{}", reward.name, reward.count))
        .collect::<Vec<_>>()
        .join("、")
}

fn remaining_line(end_ms: i64, now_ms: i64, active: bool, window: &str) -> String {
    if active && window == "activities.wishOpen" && end_ms > now_ms {
        let hours = (end_ms - now_ms + 3_599_999) / 3_600_000;
        return format!("剩余 {} 天 {} 时", hours / 24, hours % 24);
    }
    String::new()
}

fn apply_write(session: &mut WishSession, intent: WishIntent, value: &Value) {
    let refresh = value.get("refreshRequired").and_then(Value::as_bool) == Some(true);
    session.note = Some(reward_note(value, refresh, intent));
    let activity = value.get("activity");
    if refresh || activity.is_none_or(Value::is_null) {
        session.board = None;
        session.failure = None;
        return;
    }
    if let Some(board) = activity.and_then(board_from) {
        session.board = Some(board);
        session.failure = None;
    } else {
        session.board = None;
        session.failure = Some("活动动态状态未返回，请稍后刷新".into());
    }
}

fn reward_note(value: &Value, refresh: bool, intent: WishIntent) -> String {
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(reward_from).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut message = reward_line(&rewards);
    if message.is_empty() {
        message = match intent {
            WishIntent::Draw => "签文已揭晓，请领取好运奖励".into(),
            _ => "操作已完成".into(),
        };
    }
    if refresh {
        message.push_str("。请刷新确认最新进度");
    }
    message
}

fn empty_board() -> WishBoard {
    WishBoard {
        title: "秋祈良愿".into(),
        active: false,
        window_key: "activities.wishUnknown",
        remaining: String::new(),
        day: "-".into(),
        draws: 0,
        choices: Vec::new(),
        days: Vec::new(),
        pending: None,
        can_draw: false,
        can_claim: false,
        rules: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn session_with(board: WishBoard) -> WishSession {
        WishSession {
            account_id: "acc".into(),
            generation: 4,
            board: Some(board),
            ..WishSession::default()
        }
    }

    fn open_board() -> WishBoard {
        board_from(&json!({
            "id": "2026092401",
            "title": "秋祈良愿",
            "serverTime": 1_000_000,
            "startTime": 900_000,
            "endTime": 5_000_000,
            "active": true,
            "remaining": 1,
            "day": 3,
            "canDraw": true,
            "canClaim": false,
            "choices": [{ "id": 1, "name": "财运" }, { "id": 4, "name": "生活" }],
            "rewardDays": [{ "day": 3, "reward": { "id": 6001, "count": 20, "name": "点券" } }],
            "rules": ["每日一签"]
        }))
        .unwrap()
    }

    #[test]
    fn zero_and_missing_times_are_unknown_and_the_end_instant_is_closed() {
        assert_eq!(window_key(0, 0, 1_000, false), "activities.wishUnknown");
        assert_eq!(window_key(500, 0, 1_000, false), "activities.wishUnknown");
        assert_eq!(window_key(2_000, 3_000, 1_000, false), "activities.wishUpcoming");
        assert_eq!(window_key(100, 1_000, 1_000, false), "activities.wishEnded");
        assert_eq!(window_key(100, 2_000, 1_000, true), "activities.wishOpen");
        assert_eq!(window_key(0, 2_000, 1_000, true), "activities.wishOpen");
    }

    #[test]
    fn disabled_busy_and_stopped_sessions_do_not_produce_a_command() {
        let mut session = session_with(open_board());
        assert!(press(&mut session, WishIntent::Draw, 1, false).is_none());
        assert_eq!(session.generation, 4);
        let command = press(&mut session, WishIntent::Draw, 1, true).unwrap();
        assert_eq!(command.choose_id, 1);
        assert_eq!(command.generation, 5);
        assert!(session.busy);
        assert!(press(&mut session, WishIntent::Draw, 1, true).is_none());
        assert_eq!(switch_group(0), (1, 4));
        assert_eq!(session.generation, 5);
    }

    #[test]
    fn claim_uses_the_slip_choice_and_a_stale_reply_is_dropped() {
        let mut board = open_board();
        board.can_draw = false;
        board.can_claim = true;
        board.pending = Some(WishSlip {
            choose_id: 4,
            text: "一步之遥".into(),
            rewards: vec![WishReward { id: 6001, count: 20, name: "点券".into() }],
        });
        let mut session = session_with(board);
        let command = press(&mut session, WishIntent::Claim, 1, true).unwrap();
        assert_eq!(command.choose_id, 4);
        let mut other = session.clone();
        reset_account(&mut other, "other".into());
        assert!(other.board.is_none());
        assert!(!apply(
            &mut other,
            &command,
            WishApply::Wrote(Ok(json!({
                "rewards": [{ "id": 6001, "count": 20, "name": "点券" }],
                "activity": { "id": "2026092401", "active": true }
            })))
        ));
        assert!(other.note.is_none());
        session.busy = true;
        assert!(apply(&mut session, &command, WishApply::Wrote(Err("请求超时".into()))));
        assert!(session.board.is_none());
        assert_eq!(session.note.as_deref(), None);
        assert_eq!(session.failure.as_deref(), Some("请求超时"));
    }

    #[test]
    fn missing_flags_do_not_become_drawable() {
        let board = board_from(&json!({
            "id": "2026092401",
            "active": true,
            "serverTime": 1,
            "startTime": 0,
            "endTime": 5_000
        }))
        .unwrap();
        assert!(!board.can_draw);
        assert!(!board.can_claim);
        assert!(board.pending.is_none());
        assert_eq!(board.day, "-");
        let mut session = session_with(board);
        assert!(press(&mut session, WishIntent::Draw, 1, true).is_none());
    }
}
