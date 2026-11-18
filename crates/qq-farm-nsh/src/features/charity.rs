use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum CharityIntent {
    Reload,
    Seeds,
    Donate,
    ConfirmDonate,
    CancelDonate,
    Daily,
    Progress(String),
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct CharitySession {
    pub account_id: String,
    pub generation: u64,
    pub busy: bool,
    pub board: Option<CharityBoard>,
    pub failure: Option<String>,
    pub note: Option<String>,
    pub attempted: bool,
    pub needs_refresh: bool,
    pub confirming_donate: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CharityBoard {
    name: String,
    end_time: String,
    active: bool,
    love: String,
    donated: String,
    seed_status: String,
    seed_reward: String,
    seed_claimed: bool,
    seed_enabled: bool,
    donate_text: String,
    donate_enabled: bool,
    daily_status: String,
    daily_reward: String,
    daily_fund: String,
    daily_claimed: bool,
    daily_enabled: bool,
    global_percent: String,
    global_progress: String,
    global_reward: String,
    milestones: Vec<CharityMilestone>,
    settlement: String,
    settlement_reward: String,
    settlement_status: String,
    rules_title: String,
    rules: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct CharityMilestone {
    target: String,
    reward: String,
    claimable: bool,
    status: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CharityCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: CharityIntent,
}

fn board_from(value: &Value) -> Option<CharityBoard> {
    if value.get("available").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let milestones = value
        .get("progressRewards")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let claimable = row.get("claimable").and_then(Value::as_bool) == Some(true);
                    let claimed = row.get("claimed").and_then(Value::as_bool) == Some(true);
                    let reached = row.get("reached").and_then(Value::as_bool) == Some(true);
                    CharityMilestone {
                        target: row.get("target").and_then(Value::as_str).unwrap_or("0").to_owned(),
                        reward: reward_text(row.get("reward")),
                        claimable,
                        status: if claimed {
                            "奖励已领取".to_owned()
                        } else if claimable {
                            "领取奖励".to_owned()
                        } else if reached {
                            "奖励已达成".to_owned()
                        } else {
                            "尚未达成".to_owned()
                        },
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Some(CharityBoard {
        name: value.get("name").and_then(Value::as_str).unwrap_or("公益小红花").to_owned(),
        end_time: value
            .get("endTime")
            .and_then(Value::as_str)
            .filter(|text| *text != "0")
            .unwrap_or("未知")
            .to_owned(),
        active: value.get("active").and_then(Value::as_bool) == Some(true),
        love: value.get("loveBalanceText").and_then(Value::as_str).unwrap_or("0").to_owned(),
        donated: value.get("donatedLoveText").and_then(Value::as_str).unwrap_or("0").to_owned(),
        seed_status: value.get("seedStatus").and_then(Value::as_str).unwrap_or("").to_owned(),
        seed_reward: reward_text(value.pointer("/seedReward/reward")),
        seed_claimed: value.pointer("/seedReward/claimed").and_then(Value::as_bool) == Some(true),
        seed_enabled: value.pointer("/actions/claimSeeds/enabled").and_then(Value::as_bool)
            == Some(true),
        donate_text: value.get("loveBalanceText").and_then(Value::as_str).unwrap_or("0").to_owned(),
        donate_enabled: value.pointer("/actions/donateLove/enabled").and_then(Value::as_bool)
            == Some(true),
        daily_status: value.get("dailyStatus").and_then(Value::as_str).unwrap_or("").to_owned(),
        daily_reward: reward_text(value.pointer("/dailyGift/reward")),
        daily_fund: value
            .pointer("/dailyGift/publicFund/dateText")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        daily_claimed: value.pointer("/dailyGift/claimed").and_then(Value::as_bool) == Some(true),
        daily_enabled: value.pointer("/actions/claimDailyGift/enabled").and_then(Value::as_bool)
            == Some(true),
        global_percent: value
            .get("globalPercent")
            .and_then(Value::as_str)
            .unwrap_or("0.00")
            .to_owned(),
        global_progress: format!(
            "{} / {} 爱心",
            value.pointer("/globalProgress/donatedText").and_then(Value::as_str).unwrap_or("0"),
            value.pointer("/globalProgress/targetText").and_then(Value::as_str).unwrap_or("0")
        ),
        global_reward: reward_text(value.pointer("/globalProgress/reward")),
        milestones,
        settlement: value
            .pointer("/settlement/requiredLoveText")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_owned(),
        settlement_reward: reward_text(value.pointer("/settlement/reward")),
        settlement_status: value
            .get("settlementStatus")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        rules_title: value.pointer("/rules/title").and_then(Value::as_str).unwrap_or("").to_owned(),
        rules: value
            .pointer("/rules/paragraphs")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_str()).map(str::to_owned).collect())
            .unwrap_or_default(),
    })
}

fn reward_text(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "奖励未下发".to_owned();
    };
    let name = value.get("name").and_then(Value::as_str).unwrap_or("奖励未下发");
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    if name.is_empty() || name == "奖励未下发" {
        "奖励未下发".to_owned()
    } else {
        format!("{name}×{count}")
    }
}

pub(crate) fn press(
    session: &mut CharitySession,
    intent: CharityIntent,
    running: bool,
) -> Option<CharityCommand> {
    if !running || session.account_id.trim().is_empty() || session.busy {
        return None;
    }
    if session.needs_refresh && intent != CharityIntent::Reload {
        return None;
    }
    let allowed = match &intent {
        CharityIntent::Reload | CharityIntent::CancelDonate => true,
        CharityIntent::Seeds => session.board.as_ref().is_some_and(|board| board.seed_enabled),
        CharityIntent::Donate => session.board.as_ref().is_some_and(|board| board.donate_enabled),
        CharityIntent::ConfirmDonate => {
            session.confirming_donate
                && session.board.as_ref().is_some_and(|board| board.donate_enabled)
        }
        CharityIntent::Daily => session.board.as_ref().is_some_and(|board| board.daily_enabled),
        CharityIntent::Progress(target) => session.board.as_ref().is_some_and(|board| {
            board.milestones.iter().any(|item| item.target == *target && item.claimable)
        }),
    };
    if !allowed {
        return None;
    }
    match intent {
        CharityIntent::Donate => {
            session.confirming_donate = true;
            None
        }
        CharityIntent::CancelDonate => {
            session.confirming_donate = false;
            None
        }
        other => {
            session.confirming_donate = false;
            session.busy = true;
            session.attempted = true;
            Some(CharityCommand {
                account_id: session.account_id.clone(),
                generation: session.generation,
                intent: other,
            })
        }
    }
}

pub(crate) fn apply(
    session: &mut CharitySession,
    command: &CharityCommand,
    result: Result<Value, String>,
) -> bool {
    if session.account_id != command.account_id || session.generation != command.generation {
        return false;
    }
    session.busy = false;
    match result {
        Ok(value) => {
            let refresh = value.get("refreshRequired").and_then(Value::as_bool) == Some(true);
            if let Some(snapshot) = value
                .get("snapshot")
                .filter(|item| item.get("available").and_then(Value::as_bool) != Some(false))
            {
                if let Some(board) = board_from(snapshot) {
                    session.board = Some(board);
                }
            } else if command.intent == CharityIntent::Reload {
                session.board = board_from(&value);
            }
            if refresh {
                session.needs_refresh = true;
                if let Some(board) = session.board.as_mut() {
                    board.seed_enabled = false;
                    board.donate_enabled = false;
                    board.daily_enabled = false;
                    for milestone in &mut board.milestones {
                        milestone.claimable = false;
                    }
                }
            } else {
                session.needs_refresh = false;
            }
            session.note = match (success_line(&value), refresh) {
                (Some(success), true) => {
                    Some(format!("{success}。请刷新确认最新进度，刷新不会再次提交"))
                }
                (Some(success), false) => Some(success),
                (None, true) => Some("请刷新确认最新进度，刷新不会再次提交".into()),
                (None, false) => None,
            };
            session.failure = None;
            session.confirming_donate = false;
        }
        Err(message) => {
            if command.intent != CharityIntent::Reload {
                session.needs_refresh = true;
            }
            session.failure = Some(message);
        }
    }
    true
}

fn success_line(value: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(message) =
        value.get("message").and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty())
    {
        parts.push(message.to_owned());
    }
    if let Some(rows) = value.get("rewards").and_then(Value::as_array) {
        if !rows.is_empty() {
            let text = rows
                .iter()
                .map(|row| {
                    format!(
                        "{}×{}",
                        row.get("name").and_then(Value::as_str).unwrap_or("奖励"),
                        row.get("count").and_then(Value::as_i64).unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>()
                .join("、");
            if !parts.join("，").contains(&text) {
                parts.push(format!("获得 {text}"));
            }
        }
    }
    if let Some(donated) = value.get("donated").and_then(Value::as_str) {
        if !parts.join("，").contains(donated) {
            parts.push(format!("捐赠 {donated} 份"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("，"))
    }
}

pub(crate) async fn dispatch_charity(
    context: &qq_farm_app::AppContext,
    command: &CharityCommand,
) -> qq_farm_app::AppResult<Value> {
    match &command.intent {
        CharityIntent::Reload => qq_farm_app::charity::snapshot(context, &command.account_id).await,
        CharityIntent::Seeds => {
            qq_farm_app::charity::claim_seeds(context, &command.account_id).await
        }
        CharityIntent::ConfirmDonate => {
            qq_farm_app::charity::donate(context, &command.account_id).await
        }
        CharityIntent::Daily => {
            qq_farm_app::charity::claim_daily_gift(context, &command.account_id).await
        }
        CharityIntent::Progress(target) => {
            let target = target.parse::<i64>().map_err(|_| {
                qq_farm_app::AppError::BadRequest("target 必须是正十进制整数".into())
            })?;
            qq_farm_app::charity::claim_progress(context, &command.account_id, target).await
        }
        CharityIntent::Donate | CharityIntent::CancelDonate => {
            Err(qq_farm_app::AppError::BadRequest("捐赠尚未确认".into()))
        }
    }
}

pub(crate) fn watch(
    account: impl Fn() -> Option<String> + 'static,
    session: Signal<CharitySession>,
) {
    effect(move || {
        let next = account().unwrap_or_default();
        let current = session.get_untracked();
        if current.account_id != next {
            let mut owned = current;
            owned.account_id = next.clone();
            owned.generation += 1;
            owned.board = None;
            owned.busy = false;
            owned.note = None;
            owned.failure = None;
            owned.attempted = false;
            owned.needs_refresh = false;
            owned.confirming_donate = false;
            session.set(owned);
        }
        let current = session.get_untracked();
        if next.is_empty() || current.busy || current.board.is_some() || current.attempted {
            return;
        }
        let mut owned = current;
        if let Some(command) = press(&mut owned, CharityIntent::Reload, true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

pub(crate) fn panel(session: Signal<CharitySession>, page: ActivityPage) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || text_key("activities.charity")).text_lg())
        .child(when(
            move || session.get().note.is_some(),
            move || {
                notice(move || SharedString::from(session.get().note.clone().unwrap_or_default()))
            },
        ))
        .child(when(
            move || session.get().failure.is_some(),
            move || {
                notice(move || {
                    SharedString::from(session.get().failure.clone().unwrap_or_default())
                })
            },
        ))
        .child(when(
            move || session.get().board.is_none() && !session.get().busy,
            move || {
                view().gap_2().child(text(move || text_key("activities.charityEmpty"))).child(
                    button(move || text_key("activities.wishReload"), ButtonVariant::Secondary)
                        .on_press(move || fire(session, page, CharityIntent::Reload)),
                )
            },
        ))
        .child(when(move || session.get().board.is_some(), move || body(session, page)))
}

fn body(session: Signal<CharitySession>, page: ActivityPage) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "{} · 结束 {} · {} · 当前爱心 {} · 累计捐赠 {}",
                board.name,
                board.end_time,
                if board.active { "进行中" } else { "未在活动时间" },
                board.love,
                board.donated
            ))
        }))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "全服 {}% · {} · 目标奖励 {}",
                board.global_percent, board.global_progress, board.global_reward
            ))
        }))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!("每日种子 {} · {}", board.seed_status, board.seed_reward))
        }))
        .child(
            button(
                move || {
                    if session.get().board.as_ref().is_some_and(|board| board.seed_claimed) {
                        text_key("activities.charitySeedClaimed")
                    } else {
                        text_key("activities.charitySeed")
                    }
                },
                ButtonVariant::Primary,
            )
            .disabled(move || !can_press(session, CharityIntent::Seeds))
            .on_press(move || fire(session, page, CharityIntent::Seeds)),
        )
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "本次将捐赠全部 {} 份。协议会一次性清空当前爱心余额",
                board.donate_text
            ))
        }))
        .child(when(
            move || session.get().confirming_donate,
            move || {
                view()
                    .gap_2()
                    .child(
                        button(
                            move || text_key("activities.charityDonateCancel"),
                            ButtonVariant::Secondary,
                        )
                        .on_press(move || fire(session, page, CharityIntent::CancelDonate)),
                    )
                    .child(
                        button(
                            move || text_key("activities.charityDonateConfirm"),
                            ButtonVariant::Primary,
                        )
                        .disabled(move || !can_press(session, CharityIntent::ConfirmDonate))
                        .on_press(move || fire(session, page, CharityIntent::ConfirmDonate)),
                    )
            },
        ))
        .child(when(
            move || !session.get().confirming_donate,
            move || {
                button(
                    move || {
                        let amount = session
                            .get()
                            .board
                            .map(|board| board.donate_text)
                            .unwrap_or_else(|| "0".into());
                        SharedString::from(format!(
                            "{} {amount}",
                            text_key("activities.charityDonate")
                        ))
                    },
                    ButtonVariant::Primary,
                )
                .disabled(move || !can_press(session, CharityIntent::Donate))
                .on_press(move || fire(session, page, CharityIntent::Donate))
            },
        ))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            let fund = if board.daily_fund.is_empty() {
                String::new()
            } else {
                format!(" · 公益记录 {}", board.daily_fund)
            };
            SharedString::from(format!(
                "每日公益礼包 {} · {}{fund}",
                board.daily_status, board.daily_reward
            ))
        }))
        .child(
            button(
                move || {
                    if session.get().board.as_ref().is_some_and(|board| board.daily_claimed) {
                        text_key("activities.charityGiftClaimed")
                    } else {
                        text_key("activities.charityGift")
                    }
                },
                ButtonVariant::Secondary,
            )
            .disabled(move || !can_press(session, CharityIntent::Daily))
            .on_press(move || fire(session, page, CharityIntent::Daily)),
        )
        .child(each(
            move || session.get().board.map(|board| board.milestones).unwrap_or_default(),
            |item: &CharityMilestone| item.target.clone(),
            move |item, _| {
                let milestone = item.get_untracked();
                let target = milestone.target.clone();
                let line = format!(
                    "{} 爱心 · {} · {}",
                    milestone.target, milestone.reward, milestone.status
                );
                let claimable = milestone.claimable;
                view().gap_1().child(text(line)).child(when(
                    move || claimable,
                    move || {
                        let disabled_target = target.clone();
                        let pressed_target = target.clone();
                        button(
                            move || text_key("activities.charityProgress"),
                            ButtonVariant::Secondary,
                        )
                        .disabled(move || {
                            !can_press(session, CharityIntent::Progress(disabled_target.clone()))
                        })
                        .on_press(move || {
                            fire(session, page, CharityIntent::Progress(pressed_target.clone()))
                        })
                    },
                ))
            },
        ))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "活动结算奖励 累计捐赠 {} 份爱心 · {} · {}",
                board.settlement, board.settlement_reward, board.settlement_status
            ))
        }))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            if board.rules.is_empty() {
                SharedString::from("")
            } else if board.rules_title.is_empty() {
                text_key("activities.charityRules")
            } else {
                SharedString::from(board.rules_title)
            }
        }))
        .child(each(
            move || session.get().board.map(|board| board.rules).unwrap_or_default(),
            |rule: &String| rule.clone(),
            move |rule, _| text(rule.get_untracked().clone()).text_sm(),
        ))
}

fn can_press(session: Signal<CharitySession>, intent: CharityIntent) -> bool {
    let current = session.get();
    if current.busy || current.needs_refresh || current.account_id.trim().is_empty() {
        return false;
    }
    let Some(board) = current.board.as_ref() else {
        return matches!(intent, CharityIntent::Reload);
    };
    match intent {
        CharityIntent::Reload | CharityIntent::CancelDonate => true,
        CharityIntent::Seeds => board.seed_enabled,
        CharityIntent::Donate => board.donate_enabled && !current.confirming_donate,
        CharityIntent::ConfirmDonate => current.confirming_donate && board.donate_enabled,
        CharityIntent::Daily => board.daily_enabled,
        CharityIntent::Progress(target) => {
            board.milestones.iter().any(|item| item.target == target && item.claimable)
        }
    }
}

fn fire(session: Signal<CharitySession>, page: ActivityPage, intent: CharityIntent) {
    let running = page
        .account_id
        .get_untracked()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| super::accounts::account_is_running(&rows, &id));
    let mut owned = session.get_untracked();
    let Some(command) = press(&mut owned, intent, running) else {
        session.set(owned);
        return;
    };
    session.set(owned);
    launch(session, command);
}

fn launch(session: Signal<CharitySession>, command: CharityCommand) {
    spawn(async move {
        let context = session::session().context.clone();
        let result = {
            let context = context.clone();
            let command = command.clone();
            session::call(async move { dispatch_charity(&context, &command).await }).await
        };
        let mut owned = session.get_untracked();
        let _accepted = apply(&mut owned, &command, result);
        session.set(owned);
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn loaded() -> CharitySession {
        let board = board_from(&json!({
            "available": true,
            "name": "公益小红花",
            "endTime": "200",
            "active": true,
            "loveBalanceText": "4",
            "donatedLoveText": "62",
            "seedStatus": "可以领取",
            "seedReward": { "claimed": false, "reward": { "name": "种子", "count": 6 } },
            "dailyStatus": "可以领取",
            "dailyGift": { "claimed": false, "reward": { "name": "礼包", "count": 2 }, "publicFund": { "dateText": "2026-09-01" } },
            "globalPercent": "1.00",
            "globalProgress": { "donatedText": "1", "targetText": "100", "reward": { "name": "肥料", "count": 1 } },
            "progressRewards": [
                { "target": "30", "claimed": true, "reached": true, "claimable": false, "reward": { "name": "头像框", "count": 1 } },
                { "target": "60", "claimed": false, "reached": true, "claimable": true, "reward": { "name": "金币", "count": 50 } }
            ],
            "settlement": { "requiredLoveText": "30", "reward": { "name": "结算礼包", "count": 1 } },
            "settlementStatus": "已获得结算资格",
            "rules": { "title": "说明", "paragraphs": ["捐赠"] },
            "actions": {
                "claimSeeds": { "enabled": true },
                "donateLove": { "enabled": true },
                "claimDailyGift": { "enabled": true }
            }
        }))
        .unwrap();
        CharitySession { account_id: "acc".into(), board: Some(board), ..CharitySession::default() }
    }

    #[test]
    fn the_board_keeps_every_section_and_donate_waits_for_confirmation() {
        let board = loaded().board.unwrap();
        assert_eq!(board.end_time, "200");
        assert_eq!(board.rules_title, "说明");
        assert_eq!(board.rules, vec!["捐赠".to_string()]);
        assert_eq!(board.milestones.len(), 2);
        assert!(board.milestones[1].claimable);
        assert_eq!(board.global_reward, "肥料×1");
        assert_eq!(board.settlement_status, "已获得结算资格");
        assert_eq!(board.daily_fund, "2026-09-01");
        let mut session = loaded();
        assert!(press(&mut session, CharityIntent::Donate, true).is_none());
        assert!(session.confirming_donate);
        press(&mut session, CharityIntent::CancelDonate, true);
        assert!(!session.confirming_donate);
        assert!(press(&mut session, CharityIntent::Donate, true).is_none());
        let command = press(&mut session, CharityIntent::ConfirmDonate, true).unwrap();
        assert!(matches!(command.intent, CharityIntent::ConfirmDonate));
        let mut closed = loaded();
        closed.board.as_mut().unwrap().donate_enabled = false;
        closed.board.as_mut().unwrap().seed_enabled = false;
        assert!(press(&mut closed, CharityIntent::Donate, true).is_none());
        assert!(!closed.confirming_donate);
        assert!(press(&mut closed, CharityIntent::Seeds, true).is_none());
    }

    #[test]
    fn settlement_feedback_survives_refresh_failure() {
        let mut session = loaded();
        let command = press(&mut session, CharityIntent::Seeds, true).unwrap();
        assert!(apply(
            &mut session,
            &command,
            Ok(json!({
                "message": "小红花种子领取成功",
                "rewards": [{ "name": "种子", "count": 6 }],
                "refreshRequired": true,
                "snapshot": {
                    "available": true,
                    "name": "公益小红花",
                    "endTime": "200",
                    "active": true,
                    "loveBalanceText": "4",
                    "donatedLoveText": "62",
                    "rules": { "title": "说明", "paragraphs": ["捐赠"] },
                    "actions": { "claimSeeds": { "enabled": true }, "donateLove": { "enabled": true } }
                }
            }))
        ));
        let note = session.note.clone().unwrap_or_default();
        assert!(note.contains("小红花种子领取成功"), "{note}");
        assert!(note.contains("种子×6"), "{note}");
        assert!(note.contains("刷新不会再次提交"), "{note}");
        assert!(!session.board.as_ref().unwrap().seed_enabled);
        assert!(press(&mut session, CharityIntent::Seeds, true).is_none());
    }

    #[tokio::test]
    async fn dispatch_charity_sends_the_seed_operate() {
        use std::sync::{Arc, Mutex};

        use prost::Message as _;
        use qq_farm_core::models::types::AccountConfigSnapshot;
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityContent, ActivityData, ActivityListReply, ActivityOperateReply, ActivityWindow,
            CharityRedFlowerData, CharityRedFlowerOperateRequest,
        };
        use qq_farm_core::runtime::runtime_state::AccountStoreLike;

        struct EmptyStore;
        impl AccountStoreLike for EmptyStore {
            fn get_config_snapshot(&self, _id: &str) -> AccountConfigSnapshot {
                AccountConfigSnapshot::default()
            }
            fn get_automation(&self, _id: &str) -> serde_json::Value {
                serde_json::Value::Null
            }
            fn get_preferred_seed(&self, _id: &str) -> i64 {
                0
            }
        }

        let _data = qq_farm_core::config::paths::TestDataDir::enter();
        let bodies = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let log = Arc::clone(&bodies);
        let gateway = Arc::new(qq_farm_core::network::gateway::Gateway::new(
            qq_farm_core::network::gateway::GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(qq_farm_core::network::encryptor::NoopEncryptor),
        ));
        gateway.mark_online();
        gateway.install_reply_script(move |_service, method, body| {
            if method == "Operate" {
                log.lock().expect("log").push(body.to_vec());
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_090_901,
                    operate_type: 35,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(ActivityListReply {
                activities: vec![ActivityData {
                    activity: Some(ActivityContent {
                        activity_id: 2_026_090_901,
                        name: "公益小红花".into(),
                        ..ActivityContent::default()
                    }),
                    charity_red_flower: Some(CharityRedFlowerData {
                        love_balance: 4,
                        seed_reward_status: 2,
                        ..CharityRedFlowerData::default()
                    }),
                    ..ActivityData::default()
                }],
                activity_windows: vec![ActivityWindow {
                    id: 2_026_090_901,
                    begin_time: 0,
                    end_time: 0,
                    ..ActivityWindow::default()
                }],
                ..ActivityListReply::default()
            }
            .encode_to_vec())
        });
        let runtime = Arc::new(qq_farm_core::runtime::engine::RuntimeEngine::assemble_with(
            qq_farm_core::runtime::engine::EngineConfig::default(),
            Arc::new(qq_farm_core::runtime::runtime_state::RuntimeState::new(
                Arc::new(EmptyStore),
                Vec::new(),
            )),
            None,
        ));
        let (tx, _) = tokio::sync::broadcast::channel(8);
        runtime.register_worker_loop(
            "acc",
            Arc::new(qq_farm_core::runtime::worker_loop::WorkerLoop::new(
                qq_farm_core::models::account::AccountSession::new("acc", "code", "name"),
                qq_farm_core::runtime::worker_loop::WorkerLoopConfig::default(),
                Arc::clone(&gateway),
                tx,
                Arc::new(qq_farm_core::services::farm::FarmService::new(Arc::clone(&gateway))),
                Arc::new(qq_farm_core::services::friend::scheduler::FriendService::new(
                    Arc::clone(&gateway),
                    5,
                )),
                Arc::new(qq_farm_core::services::email::EmailService::new(
                    Arc::clone(&gateway),
                    "acc",
                )),
                Arc::new(qq_farm_core::services::share::ShareService::new(Arc::clone(&gateway))),
                Arc::new(qq_farm_core::services::monthcard::MonthCardService::new(Arc::clone(
                    &gateway,
                ))),
                Arc::new(qq_farm_core::services::qqvip::QQVipService::new(
                    Arc::clone(&gateway),
                    Arc::new(qq_farm_core::services::mall::MallService::new(Arc::clone(&gateway))),
                )),
                Arc::new(qq_farm_core::services::mall::MallService::new(Arc::clone(&gateway))),
                Arc::new(qq_farm_core::services::task::TaskService::new(Arc::clone(&gateway))),
                Arc::new(qq_farm_core::services::warehouse::WarehouseService::new(Arc::clone(
                    &gateway,
                ))),
                Arc::new(qq_farm_core::services::mystery_shop::MysteryShopService::new(gateway)),
            )),
        );
        let context = qq_farm_app::AppContext::new(runtime);
        let mut session = loaded();
        let command = press(&mut session, CharityIntent::Seeds, true).unwrap();
        dispatch_charity(&context, &command).await.unwrap();
        let request =
            CharityRedFlowerOperateRequest::decode(bodies.lock().expect("log")[0].as_slice())
                .unwrap();
        assert_eq!(request.operate_type, 35);
        assert!(request.claim_seed.is_some());
        assert_eq!(bodies.lock().expect("log").len(), 1);
    }
}
