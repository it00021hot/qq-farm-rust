use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QingIntent {
    Reload,
    Seed,
    Start,
    Continue,
    Settle,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct QingSession {
    pub account_id: String,
    pub generation: u64,
    pub busy: bool,
    pub board: Option<QingBoard>,
    pub failure: Option<String>,
    pub note: Option<String>,
    pub attempted: bool,
    pub needs_refresh: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct QingBoard {
    name: String,
    balance: String,
    balance_known: bool,
    ingredients: Vec<QingItem>,
    started: bool,
    round: String,
    max_rounds: i64,
    guaranteed: String,
    quotes: Vec<String>,
    pending_rounds: i64,
    rules_title: String,
    rules: Vec<String>,
    end_time: String,
    seed_claimed: bool,
    seed_enabled: bool,
    start_enabled: bool,
    continue_enabled: bool,
    settle_enabled: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct QingItem {
    uid: i64,
    name: String,
    count: i64,
    mutants: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct QingPick {
    pub uid: i64,
    pub count: i64,
}

#[derive(Clone)]
pub(crate) struct QingCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: QingIntent,
    pub ingredients: Vec<QingPick>,
}

pub(crate) fn board_from(value: &Value) -> Option<QingBoard> {
    if value.get("available").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let ingredients = value
        .get("ingredients")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(item_from).collect::<Vec<_>>())
        .unwrap_or_default();
    let quotes = value
        .get("quoteTotals")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .enumerate()
                .map(|(index, total)| {
                    let price = value
                        .get("quotePrices")
                        .and_then(Value::as_array)
                        .and_then(|prices| prices.get(index))
                        .and_then(Value::as_str)
                        .unwrap_or("0");
                    format!("第 {} 轮 {} 单价 {price}", index + 1, total.as_str().unwrap_or("0"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let max_rounds = value.get("maxRounds").and_then(Value::as_i64).unwrap_or(3);
    Some(QingBoard {
        name: value.get("name").and_then(Value::as_str).unwrap_or("青酿换万金").to_owned(),
        balance: if value.get("balanceKnown").and_then(Value::as_bool) == Some(true) {
            value.get("balance").and_then(Value::as_i64).unwrap_or(0).to_string()
        } else {
            "余额未知".into()
        },
        balance_known: value.get("balanceKnown").and_then(Value::as_bool) == Some(true),
        ingredients,
        started: value.get("started").and_then(Value::as_bool) == Some(true),
        round: value.get("currentRound").and_then(Value::as_i64).unwrap_or(0).to_string(),
        max_rounds,
        guaranteed: value.get("guaranteedPrice").and_then(Value::as_str).unwrap_or("0").to_owned(),
        pending_rounds: (max_rounds - quotes.len() as i64).max(0),
        quotes,
        rules_title: value.pointer("/rules/title").and_then(Value::as_str).unwrap_or("").to_owned(),
        end_time: end_time_text(value),
        seed_claimed: value.pointer("/dailySeed/claimed").and_then(Value::as_bool) == Some(true),
        rules: value
            .pointer("/rules/paragraphs")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_str()).map(str::to_owned).collect())
            .unwrap_or_default(),
        seed_enabled: value.pointer("/actions/claimSeed/enabled").and_then(Value::as_bool)
            == Some(true),
        start_enabled: value.pointer("/actions/start/enabled").and_then(Value::as_bool)
            == Some(true),
        continue_enabled: value.pointer("/actions/continue/enabled").and_then(Value::as_bool)
            == Some(true),
        settle_enabled: value.pointer("/actions/settle/enabled").and_then(Value::as_bool)
            == Some(true),
    })
}

pub(crate) fn start_refusal(board: Option<&QingBoard>, picks: &[QingPick]) -> Option<&'static str> {
    let Some(board) = board else {
        return Some("activities.qingmeiEmpty");
    };
    if !board.balance_known {
        return Some("activities.qingmeiBalanceUnknown");
    }
    if picks.is_empty() || !board.start_enabled {
        return Some("activities.qingmeiPick");
    }
    let mut seen = Vec::new();
    for pick in picks {
        if !seen.iter().any(|uid| uid == &pick.uid) {
            seen.push(pick.uid);
        } else {
            return Some("activities.qingmeiDuplicate");
        }
        if pick.count <= 0 {
            return Some("activities.qingmeiBadCount");
        }
        let owned =
            board.ingredients.iter().find(|item| item.uid == pick.uid).map(|item| item.count);
        if owned.is_none() || pick.count > owned.unwrap_or(0) {
            return Some("activities.qingmeiShort");
        }
    }
    None
}

pub(crate) fn press(
    session: &mut QingSession,
    intent: QingIntent,
    picks: &[QingPick],
    running: bool,
) -> Option<QingCommand> {
    if !running
        || session.account_id.trim().is_empty()
        || session.busy
        || session.needs_refresh && intent != QingIntent::Reload
    {
        return None;
    }
    match intent {
        QingIntent::Reload => {}
        QingIntent::Seed => {
            if !session.board.as_ref().is_some_and(|board| board.seed_enabled) {
                return None;
            }
        }
        QingIntent::Start => {
            if start_refusal(session.board.as_ref(), picks).is_some() {
                return None;
            }
        }
        QingIntent::Continue => {
            if !session.board.as_ref().is_some_and(|board| board.continue_enabled) {
                return None;
            }
        }
        QingIntent::Settle => {
            if !session.board.as_ref().is_some_and(|board| board.settle_enabled) {
                return None;
            }
        }
    }
    session.busy = true;
    session.attempted = true;
    session.generation += 1;
    session.failure = None;
    Some(QingCommand {
        account_id: session.account_id.clone(),
        generation: session.generation,
        intent,
        ingredients: picks.to_vec(),
    })
}

pub(crate) fn reset_account(session: &mut QingSession, account_id: String) {
    if session.account_id == account_id {
        return;
    }
    *session =
        QingSession { account_id, generation: session.generation + 1, ..QingSession::default() };
}

pub(crate) fn apply(
    session: &mut QingSession,
    command: &QingCommand,
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
            } else if command.intent == QingIntent::Reload {
                session.board = board_from(&value);
            }
            if refresh {
                session.needs_refresh = true;
                if let Some(board) = session.board.as_mut() {
                    board.seed_enabled = false;
                    board.start_enabled = false;
                    board.continue_enabled = false;
                    board.settle_enabled = false;
                }
            } else {
                session.needs_refresh = false;
            }
            let success = success_line(&value);
            session.note = match (success, refresh) {
                (Some(success), true) => {
                    Some(format!("{success}。请刷新确认最新进度，刷新不会再次提交"))
                }
                (Some(success), false) => Some(success),
                (None, true) => Some("请刷新确认最新进度，刷新不会再次提交".into()),
                (None, false) => None,
            };
            session.failure = None;
        }
        Err(message) => {
            if command.intent != QingIntent::Reload {
                session.needs_refresh = true;
            }
            session.failure = Some(message);
        }
    }
    true
}

pub(crate) async fn dispatch_qingmei(
    context: &qq_farm_app::AppContext,
    command: &QingCommand,
) -> qq_farm_app::AppResult<Value> {
    let picks = command.ingredients.iter().map(|pick| (pick.uid, pick.count)).collect::<Vec<_>>();
    match command.intent {
        QingIntent::Reload => qq_farm_app::qingmei::snapshot(context, &command.account_id).await,
        QingIntent::Seed => qq_farm_app::qingmei::claim_seed(context, &command.account_id).await,
        QingIntent::Start => {
            qq_farm_app::qingmei::start(context, &command.account_id, &picks).await
        }
        QingIntent::Continue => {
            qq_farm_app::qingmei::continue_brew(context, &command.account_id).await
        }
        QingIntent::Settle => qq_farm_app::qingmei::settle(context, &command.account_id).await,
    }
}

pub(crate) fn watch(account: impl Fn() -> Option<String> + 'static, session: Signal<QingSession>) {
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
        if let Some(command) = press(&mut owned, QingIntent::Reload, &[], true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

pub(crate) fn panel(session: Signal<QingSession>, page: ActivityPage) -> impl IntoElement {
    let picks = signal(Vec::<QingPick>::new());
    view()
        .gap_3()
        .child(text(move || text_key("activities.qingmei")).text_lg())
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
                view().gap_2().child(text(move || text_key("activities.qingmeiEmpty"))).child(
                    button(move || text_key("activities.wishReload"), ButtonVariant::Secondary)
                        .on_press(move || fire(session, page, QingIntent::Reload, Vec::new())),
                )
            },
        ))
        .child(when(move || session.get().board.is_some(), move || body(session, page, picks)))
}

fn body(
    session: Signal<QingSession>,
    page: ActivityPage,
    picks: Signal<Vec<QingPick>>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "{} · 结束 {} · 可用青梅 {} · 保底 {}",
                board.name, board.end_time, board.balance, board.guaranteed
            ))
        }))
        .child(
            button(
                move || {
                    if session.get().board.as_ref().is_some_and(|board| board.seed_claimed) {
                        text_key("activities.qingmeiSeedClaimed")
                    } else {
                        text_key("activities.qingmeiSeed")
                    }
                },
                ButtonVariant::Primary,
            )
            .disabled(move || {
                let current = session.get();
                current.busy
                    || current.needs_refresh
                    || !current.board.as_ref().is_some_and(|board| board.seed_enabled)
            })
            .on_press(move || fire(session, page, QingIntent::Seed, Vec::new())),
        )
        .child(when(
            move || session.get().board.as_ref().is_some_and(|board| !board.started),
            move || setup(session, page, picks),
        ))
        .child(when(
            move || session.get().board.as_ref().is_some_and(|board| board.started),
            move || brewing(session, page),
        ))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            if board.rules.is_empty() {
                SharedString::from("")
            } else if board.rules_title.is_empty() {
                text_key("activities.qingmeiRules")
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

fn setup(
    session: Signal<QingSession>,
    page: ActivityPage,
    picks: Signal<Vec<QingPick>>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(each(
            move || session.get().board.map(|board| board.ingredients).unwrap_or_default(),
            |item: &QingItem| item.uid,
            move |item, _| {
                let item = item.get_untracked();
                let uid = item.uid;
                let owned = item.count;
                let line =
                    format!("{} UID {} ×{} {}", item.name, item.uid, item.count, item.mutants);
                view()
                    .gap_1()
                    .child(
                        button(move || SharedString::from(line.clone()), ButtonVariant::Quiet)
                            .selected(move || picks.get().iter().any(|pick| pick.uid == uid))
                            .on_press(move || toggle(picks, uid, owned)),
                    )
                    .child(
                        button(move || SharedString::from("+"), ButtonVariant::Secondary)
                            .on_press(move || adjust(picks, uid, 1, owned)),
                    )
                    .child(
                        button(move || SharedString::from("-"), ButtonVariant::Secondary)
                            .on_press(move || adjust(picks, uid, -1, owned)),
                    )
            },
        ))
        .child(text(move || {
            start_refusal(session.get().board.as_ref(), &picks.get())
                .map(text_key)
                .unwrap_or_default()
        }))
        .child(
            button(move || text_key("activities.qingmeiStart"), ButtonVariant::Primary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || start_refusal(current.board.as_ref(), &picks.get()).is_some()
                })
                .on_press(move || fire(session, page, QingIntent::Start, picks.get_untracked())),
        )
}

fn brewing(session: Signal<QingSession>, page: ActivityPage) -> impl IntoElement {
    view()
        .gap_2()
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!("第 {}/{} 轮", board.round, board.max_rounds))
        }))
        .child(each(
            move || session.get().board.map(|board| board.quotes).unwrap_or_default(),
            |quote: &String| quote.clone(),
            move |quote, _| text(quote.get_untracked().clone()).text_sm(),
        ))
        .child(text(move || {
            let pending = session.get().board.map(|board| board.pending_rounds).unwrap_or(0);
            SharedString::from(format!("待酿造 {pending} 轮"))
        }))
        .child(when(
            move || {
                session
                    .get()
                    .board
                    .as_ref()
                    .is_some_and(|board| board.started && board.quotes.is_empty())
            },
            move || text(move || text_key("activities.qingmeiFirstQuote")).text_sm(),
        ))
        .child(
            button(move || text_key("activities.qingmeiContinue"), ButtonVariant::Primary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || !current.board.as_ref().is_some_and(|board| board.continue_enabled)
                })
                .on_press(move || fire(session, page, QingIntent::Continue, Vec::new())),
        )
        .child(
            button(move || text_key("activities.qingmeiSettle"), ButtonVariant::Secondary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || !current.board.as_ref().is_some_and(|board| board.settle_enabled)
                })
                .on_press(move || fire(session, page, QingIntent::Settle, Vec::new())),
        )
}

fn fire(
    session: Signal<QingSession>,
    page: ActivityPage,
    intent: QingIntent,
    picks: Vec<QingPick>,
) {
    let running = page
        .account_id
        .get_untracked()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| super::accounts::account_is_running(&rows, &id));
    let mut owned = session.get_untracked();
    let Some(command) = press(&mut owned, intent, &picks, running) else {
        return;
    };
    session.set(owned);
    launch(session, command);
}

fn launch(session: Signal<QingSession>, command: QingCommand) {
    spawn(async move {
        let context = session::session().context.clone();
        let result = {
            let context = context.clone();
            let command = command.clone();
            session::call(async move { dispatch_qingmei(&context, &command).await }).await
        };
        let mut owned = session.get_untracked();
        let _accepted = apply(&mut owned, &command, result);
        session.set(owned);
    });
}

fn toggle(picks: Signal<Vec<QingPick>>, uid: i64, owned: i64) {
    picks.update(|rows| {
        if let Some(index) = rows.iter().position(|pick| pick.uid == uid) {
            rows.remove(index);
        } else {
            rows.push(QingPick { uid, count: owned.max(1) });
        }
    });
}

fn adjust(picks: Signal<Vec<QingPick>>, uid: i64, delta: i64, owned: i64) {
    picks.update(|rows| {
        if let Some(pick) = rows.iter_mut().find(|pick| pick.uid == uid) {
            pick.count = (pick.count + delta).clamp(1, owned.max(1));
        }
    });
}

fn end_time_text(value: &Value) -> String {
    let raw = value.get("endTime").and_then(|item| {
        item.as_i64().map(|time| time.to_string()).or_else(|| {
            item.as_str().map(str::trim).filter(|text| !text.is_empty()).map(str::to_owned)
        })
    });
    match raw {
        Some(text) if text != "0" => text,
        _ => "未知".to_owned(),
    }
}

fn success_line(value: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(message) =
        value.get("message").and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty())
    {
        parts.push(message.to_owned());
    } else if value.get("alreadyClaimed").and_then(Value::as_bool) == Some(true) {
        parts.push("今日青梅种子已经领取，无需重复领取".to_owned());
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
    if let Some(gold) = value
        .pointer("/settlement/totalGold")
        .and_then(Value::as_str)
        .filter(|gold| !gold.is_empty())
    {
        if !parts.join("，").contains(gold) {
            parts.push(format!("结算 {gold} 金币"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("，"))
    }
}

fn item_from(value: &Value) -> Option<QingItem> {
    let uid = value.get("uid").and_then(Value::as_str).and_then(|uid| uid.parse().ok())?;
    let names = value
        .get("mutantTypeNames")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| row.as_str()).collect::<Vec<_>>().join("+"))
        .unwrap_or_default();
    Some(QingItem {
        uid,
        name: value.get("name").and_then(Value::as_str).unwrap_or("青梅").to_owned(),
        count: value.get("count").and_then(Value::as_i64).unwrap_or(0),
        mutants: names,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn loaded() -> QingSession {
        let board = board_from(&json!({
            "available": true,
            "name": "青酿换万金",
            "balanceKnown": true,
            "balance": 3,
            "guaranteedPrice": "6",
            "started": false,
            "currentRound": 0,
            "maxRounds": 3,
            "ingredients": [
                { "uid": "9", "name": "青梅", "count": 2, "mutantTypeNames": ["大"] },
                { "uid": "10", "name": "青梅", "count": 1, "mutantTypeNames": [] }
            ],
            "rules": { "title": "说明", "paragraphs": ["酿造"] },
            "actions": { "claimSeed": { "enabled": true }, "start": { "enabled": true }, "continue": { "enabled": false }, "settle": { "enabled": false } }
        }))
        .unwrap();
        QingSession { account_id: "acc".into(), board: Some(board), ..QingSession::default() }
    }

    #[test]
    fn both_ingredients_stay_and_unknown_balance_blocks_start() {
        let board = loaded().board.unwrap();
        assert_eq!(board.ingredients.len(), 2);
        assert_eq!(board.rules, vec!["酿造".to_string()]);
        let mut session = loaded();
        assert!(press(&mut session, QingIntent::Start, &[], true).is_none());
        let picks = vec![QingPick { uid: 9, count: 2 }, QingPick { uid: 10, count: 1 }];
        let command = press(&mut session, QingIntent::Start, &picks, true).unwrap();
        assert_eq!(command.ingredients[0].count, 2);
        let unknown = board_from(&json!({
            "available": true,
            "balanceKnown": false,
            "started": false,
            "ingredients": [{ "uid": "9", "count": 2 }],
            "actions": { "start": { "enabled": true } }
        }))
        .unwrap();
        assert_eq!(unknown.balance, "余额未知");
        assert_eq!(
            start_refusal(Some(&unknown), &[QingPick { uid: 9, count: 1 }]),
            Some("activities.qingmeiBalanceUnknown")
        );
        let mut blocked = QingSession {
            account_id: "acc".into(),
            board: Some(unknown),
            ..QingSession::default()
        };
        assert!(press(&mut blocked, QingIntent::Start, &[QingPick { uid: 9, count: 1 }], true)
            .is_none());
    }

    #[tokio::test]
    async fn dispatch_qingmei_sends_the_pressed_ingredients() {
        use std::sync::{Arc, Mutex};

        use prost::Message as _;
        use qq_farm_core::models::types::AccountConfigSnapshot;
        use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
        use qq_farm_core::proto::generated::gamepb::activitypb::ActivityData;
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityListReply, ActivityOperateReply, ActivityWindow, QingMeiBrewData,
            QueryActivityRequest, StartQingMeiBrewRequest,
        };
        use qq_farm_core::proto::generated::gamepb::itempb::BagReply;
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
        let runtime = Arc::new(qq_farm_core::runtime::engine::RuntimeEngine::assemble_with(
            qq_farm_core::runtime::engine::EngineConfig::default(),
            Arc::new(qq_farm_core::runtime::runtime_state::RuntimeState::new(
                Arc::new(EmptyStore),
                Vec::new(),
            )),
            None,
        ));
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
        let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let log = Arc::clone(&seen);
        gateway.install_reply_script(move |_service, method, body| {
            if method == "List" {
                return Ok(ActivityListReply {
                    activity_windows: vec![ActivityWindow {
                        id: 2_026_081_202,
                        begin_time: 0,
                        end_time: 0,
                        ..ActivityWindow::default()
                    }],
                    ..ActivityListReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![CoreItem {
                            id: 41_221,
                            count: 2,
                            uid: 9,
                            ..CoreItem::default()
                        }],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec());
            }
            if method == "Operate"
                && QueryActivityRequest::decode(body)
                    .map(|request| request.operate_type)
                    .unwrap_or(0)
                    == 14
            {
                log.lock().expect("log").push(body.to_vec());
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_202,
                    operate_type: 14,
                    data: Some(ActivityData {
                        qingmei_brew: Some(QingMeiBrewData {
                            base_gold: 1,
                            max_rounds: 3,
                            ..QingMeiBrewData::default()
                        }),
                        ..ActivityData::default()
                    }),
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(ActivityOperateReply::default().encode_to_vec())
        });
        let (tx, _) = tokio::sync::broadcast::channel(4);
        let gate = Arc::clone(&gateway);
        runtime.register_worker_loop(
            "acc",
            Arc::new(qq_farm_core::runtime::worker_loop::WorkerLoop::new(
                qq_farm_core::models::account::AccountSession::new("acc", "code", "name"),
                qq_farm_core::runtime::worker_loop::WorkerLoopConfig::default(),
                Arc::clone(&gate),
                tx,
                Arc::new(qq_farm_core::services::farm::FarmService::new(Arc::clone(&gate))),
                Arc::new(qq_farm_core::services::friend::scheduler::FriendService::new(
                    Arc::clone(&gate),
                    5,
                )),
                Arc::new(qq_farm_core::services::email::EmailService::new(
                    Arc::clone(&gate),
                    "acc",
                )),
                Arc::new(qq_farm_core::services::share::ShareService::new(Arc::clone(&gate))),
                Arc::new(qq_farm_core::services::monthcard::MonthCardService::new(Arc::clone(
                    &gate,
                ))),
                Arc::new(qq_farm_core::services::qqvip::QQVipService::new(
                    Arc::clone(&gate),
                    Arc::new(qq_farm_core::services::mall::MallService::new(Arc::clone(&gate))),
                )),
                Arc::new(qq_farm_core::services::mall::MallService::new(Arc::clone(&gate))),
                Arc::new(qq_farm_core::services::task::TaskService::new(Arc::clone(&gate))),
                Arc::new(qq_farm_core::services::warehouse::WarehouseService::new(Arc::clone(
                    &gate,
                ))),
                Arc::new(qq_farm_core::services::mystery_shop::MysteryShopService::new(gate)),
            )),
        );
        let context = qq_farm_app::AppContext::new(runtime);
        let mut session = loaded();
        assert!(press(&mut session, QingIntent::Start, &[], true).is_none());
        let command =
            press(&mut session, QingIntent::Start, &[QingPick { uid: 9, count: 2 }], true).unwrap();
        dispatch_qingmei(&context, &command).await.unwrap();
        let request =
            StartQingMeiBrewRequest::decode(seen.lock().expect("seen")[0].as_slice()).unwrap();
        let picked = request.params.unwrap().ingredients;
        assert_eq!(request.activity_id, 2_026_081_202);
        assert_eq!(request.operate_type, 14);
        assert_eq!(picked[0].uid, 9);
        assert_eq!(picked[0].count, 2);
    }

    #[test]
    fn refresh_required_disables_stale_actions() {
        let mut session = loaded();
        let command = press(&mut session, QingIntent::Seed, &[], true).unwrap();
        assert!(apply(
            &mut session,
            &command,
            Ok(json!({
                "rewards": [{ "name": "种子", "count": 2 }],
                "refreshRequired": true,
                "snapshot": {
                    "available": true,
                    "name": "青酿换万金",
                    "endTime": 200,
                    "balanceKnown": true,
                    "balance": 3,
                    "started": false,
                    "rules": { "title": "说明", "paragraphs": ["酿造"] },
                    "dailySeed": { "claimed": true },
                    "actions": { "claimSeed": { "enabled": false }, "start": { "enabled": true } }
                }
            }))
        ));
        let note = session.note.clone().unwrap_or_default();
        assert!(note.contains("种子×2"), "{note}");
        assert!(note.contains("刷新不会再次提交"), "{note}");
        assert_eq!(session.board.as_ref().unwrap().rules_title, "说明");
        assert_eq!(session.board.as_ref().unwrap().end_time, "200");
        assert!(session.board.as_ref().unwrap().seed_claimed);
        assert!(session.needs_refresh);
        assert!(!session.board.as_ref().unwrap().seed_enabled);
        assert!(press(&mut session, QingIntent::Start, &[QingPick { uid: 9, count: 1 }], true)
            .is_none());
        let mut selling = loaded();
        selling.board.as_mut().unwrap().settle_enabled = true;
        let sold = press(&mut selling, QingIntent::Settle, &[], true).unwrap();
        assert!(apply(
            &mut selling,
            &sold,
            Ok(json!({
                "rewards": [{ "name": "金币", "count": 1 }],
                "settlement": { "totalGold": "30" },
                "snapshot": {
                    "available": true,
                    "name": "青酿换万金",
                    "started": true,
                    "currentRound": 1,
                    "maxRounds": 3,
                    "quoteTotals": [],
                    "balanceKnown": true,
                    "balance": 0,
                    "rules": { "title": "说明", "paragraphs": ["酿造"] }
                }
            }))
        ));
        let sold_note = selling.note.clone().unwrap_or_default();
        assert!(sold_note.contains("金币×1"), "{sold_note}");
        assert!(sold_note.contains("结算 30 金币"), "{sold_note}");
        assert!(selling.board.as_ref().unwrap().started);
    }
}
