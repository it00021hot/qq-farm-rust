use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use super::confirm::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StellarIntent {
    Reload,
    Claim,
    Exchange,
    Light,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct StellarSession {
    pub account_id: String,
    pub generation: u64,
    pub busy: bool,
    pub board: Option<StellarBoard>,
    pub failure: Option<String>,
    pub note: Option<String>,
    pub attempted: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct StellarBoard {
    claimable: bool,
    lightable: bool,
    goods_id: i64,
    goods_name: String,
    cost_name: String,
    unit_cost: i64,
    max_count: i64,
    balance_known: bool,
    day: String,
}

#[derive(Clone)]
pub(crate) struct StellarCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: StellarIntent,
    pub goods_id: i64,
    pub count: i64,
}

pub(crate) fn board_from(value: &Value) -> Option<StellarBoard> {
    let season = value.get("season")?;
    let claimable = season.pointer("/pass/nodes").and_then(Value::as_array).is_some_and(|nodes| {
        nodes.iter().any(|node| node.get("claimable").and_then(Value::as_bool) == Some(true))
    });
    let goods =
        value.pointer("/shop/goods").and_then(Value::as_array).and_then(|rows| rows.first());
    let groups = value.pointer("/constellation/groups").and_then(Value::as_array);
    let lightable = groups.is_some_and(|groups| {
        groups.iter().any(|group| {
            matches!(
                group.get("visualState").and_then(Value::as_str),
                Some("lightable" | "claimableUnknown")
            )
        })
    });
    Some(StellarBoard {
        claimable,
        lightable,
        goods_id: goods
            .and_then(|goods| goods.get("id"))
            .and_then(Value::as_str)
            .and_then(|id| id.parse().ok())
            .unwrap_or(0),
        goods_name: goods
            .and_then(|goods| goods.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        cost_name: goods
            .and_then(|goods| goods.pointer("/cost/name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        unit_cost: goods
            .and_then(|goods| goods.pointer("/cost/count"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        max_count: goods
            .and_then(|goods| goods.get("maxExchangeCount"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        balance_known: goods.and_then(|goods| goods.get("balanceKnown")).and_then(Value::as_bool)
            == Some(true),
        day: value
            .pointer("/constellation/currentDay")
            .and_then(Value::as_i64)
            .map(|day| day.to_string())
            .unwrap_or_else(|| "-".into()),
    })
}

pub(crate) fn press(
    session: &mut StellarSession,
    intent: StellarIntent,
    count: i64,
    running: bool,
) -> Option<StellarCommand> {
    if !running || session.account_id.is_empty() || session.busy {
        return None;
    }
    let board = session.board.as_ref();
    let (goods_id, count) = match intent {
        StellarIntent::Reload => (0, 0),
        StellarIntent::Claim => {
            if !board.is_some_and(|board| board.claimable) {
                return None;
            }
            (0, 0)
        }
        StellarIntent::Light => {
            if !board.is_some_and(|board| board.lightable) {
                return None;
            }
            (0, 0)
        }
        StellarIntent::Exchange => {
            let board = board?;
            if count <= 0
                || !board.balance_known
                || board.goods_id <= 0
                || board.unit_cost <= 0
                || count > board.max_count
            {
                return None;
            }
            (board.goods_id, count)
        }
    };
    session.busy = true;
    session.attempted = true;
    session.generation += 1;
    session.failure = None;
    Some(StellarCommand {
        account_id: session.account_id.clone(),
        generation: session.generation,
        intent,
        goods_id,
        count,
    })
}

pub(crate) fn reset_account(session: &mut StellarSession, account_id: String) {
    if session.account_id == account_id {
        return;
    }
    *session = StellarSession {
        account_id,
        generation: session.generation + 1,
        ..StellarSession::default()
    };
}

pub(crate) fn apply(
    session: &mut StellarSession,
    command: &StellarCommand,
    result: Result<Value, String>,
) -> bool {
    if session.account_id != command.account_id || session.generation != command.generation {
        return false;
    }
    session.busy = false;
    match result {
        Ok(value) => {
            if command.intent == StellarIntent::Reload {
                session.board = board_from(&value);
            } else if value.get("refreshRequired").and_then(Value::as_bool) == Some(true) {
                session.note = Some("请刷新确认最新进度".into());
            } else if let Some(board) = value.get("season").and_then(|_| board_from(&serde_json::json!({"season": value.get("season"), "shop": value.get("shop"), "constellation": value.get("constellation")}))) {
                session.board = Some(board);
            }
            session.failure = None;
        }
        Err(message) => {
            if command.intent != StellarIntent::Reload {
                session.board = None;
            }
            session.failure = Some(message);
        }
    }
    true
}

pub(crate) async fn dispatch_stellar(
    context: &qq_farm_app::AppContext,
    command: &StellarCommand,
) -> qq_farm_app::AppResult<Value> {
    match command.intent {
        StellarIntent::Reload => qq_farm_app::stellar::snapshot(context, &command.account_id).await,
        StellarIntent::Claim => {
            qq_farm_app::stellar::claim_pass(context, &command.account_id).await
        }
        StellarIntent::Light => qq_farm_app::stellar::light(context, &command.account_id).await,
        StellarIntent::Exchange => {
            qq_farm_app::stellar::exchange(
                context,
                &command.account_id,
                command.goods_id,
                command.count,
            )
            .await
        }
    }
}

pub(crate) fn watch(
    account: impl Fn() -> Option<String> + 'static,
    session: Signal<StellarSession>,
) {
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
        if let Some(command) = press(&mut owned, StellarIntent::Reload, 0, true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

pub(crate) fn panel(
    session: Signal<StellarSession>,
    page: ActivityPage,
    open_solar: impl Fn() + 'static,
) -> impl IntoElement {
    let count = signal(1_i64);
    view()
        .gap_3()
        .child(text(move || text_key("activities.stellar")).text_lg())
        .child(
            button(move || text_key("activities.stellarSolar"), ButtonVariant::Secondary)
                .on_press(open_solar),
        )
        .child(when(
            move || session.get().board.is_none() && !session.get().busy,
            move || {
                view().gap_2().child(notice(move || text_key("activities.stellarEmpty"))).child(
                    button(move || text_key("activities.wishReload"), ButtonVariant::Secondary)
                        .on_press(move || fire(session, page, StellarIntent::Reload, 0)),
                )
            },
        ))
        .child(when(move || session.get().board.is_some(), move || body(session, page, count)))
}

fn body(
    session: Signal<StellarSession>,
    page: ActivityPage,
    count: Signal<i64>,
) -> impl IntoElement {
    let board = session.get().board.clone();
    let claimable = board.as_ref().is_some_and(|board| board.claimable);
    let lightable = board.as_ref().is_some_and(|board| board.lightable);
    let cost = board
        .as_ref()
        .map(|board| {
            format!(
                "{} {} ×{}",
                text_key("activities.stellarCost"),
                board.cost_name,
                board.unit_cost.saturating_mul(count.get())
            )
        })
        .unwrap_or_default();
    view()
        .gap_2()
        .child(text(board.as_ref().map(|board| format!("第 {} 日", board.day)).unwrap_or_default()))
        .child(
            button(move || text_key("activities.stellarClaim"), ButtonVariant::Primary)
                .disabled(move || session.get().busy || !claimable)
                .on_press(move || fire(session, page, StellarIntent::Claim, 0)),
        )
        .child(
            button(move || text_key("activities.stellarLight"), ButtonVariant::Primary)
                .disabled(move || session.get().busy || !lightable)
                .on_press(move || fire(session, page, StellarIntent::Light, 0)),
        )
        .child(text(cost).text_sm())
        .child(
            button(move || text_key("activities.stellarExchange"), ButtonVariant::Primary)
                .disabled(move || {
                    let Some(board) = session.get().board else { return true };
                    session.get().busy
                        || count.get() <= 0
                        || !board.balance_known
                        || count.get() > board.max_count
                })
                .on_press(move || {
                    let count = count.get_untracked();
                    confirm(
                        "activities.stellarExchange",
                        "activities.stellarExchange",
                        "exchange",
                        "activities.stellarExchange",
                        move |accepted| {
                            if accepted {
                                fire(session, page, StellarIntent::Exchange, count);
                            }
                        },
                    );
                }),
        )
}

fn fire(session: Signal<StellarSession>, page: ActivityPage, intent: StellarIntent, count: i64) {
    let running = page
        .account_id
        .get_untracked()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| super::accounts::account_is_running(&rows, &id));
    let mut owned = session.get_untracked();
    let Some(command) = press(&mut owned, intent, count, running) else {
        return;
    };
    session.set(owned);
    launch(session, command);
}

fn launch(session: Signal<StellarSession>, command: StellarCommand) {
    spawn(async move {
        let context = session::session().context.clone();
        let intent = command.intent;
        let result = {
            let context = context.clone();
            let command = command.clone();
            session::call(async move { dispatch_stellar(&context, &command).await }).await
        };
        let mut owned = session.get_untracked();
        let _accepted = apply(
            &mut owned,
            &command,
            result.map_err(|error| error.to_string()).map(|value| {
                if intent == StellarIntent::Reload {
                    value
                } else {
                    value
                }
            }),
        );
        session.set(owned);
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn session() -> StellarSession {
        StellarSession {
            account_id: "acc".into(),
            board: Some(StellarBoard {
                claimable: false,
                lightable: true,
                goods_id: 4,
                goods_name: "种子".into(),
                cost_name: "星砂".into(),
                unit_cost: 3,
                max_count: 1,
                balance_known: true,
                day: "2".into(),
            }),
            ..StellarSession::default()
        }
    }

    #[test]
    fn exchange_cancel_quantity_and_unknown_balance_do_not_make_a_command() {
        let mut owned = session();
        assert!(press(&mut owned, StellarIntent::Exchange, 0, true).is_none());
        assert!(press(&mut owned, StellarIntent::Exchange, 2, true).is_none());
        owned.board.as_mut().unwrap().balance_known = false;
        assert!(press(&mut owned, StellarIntent::Exchange, 1, true).is_none());
        assert_eq!(owned.generation, 0);
        owned.board.as_mut().unwrap().balance_known = true;
        let command = press(&mut owned, StellarIntent::Exchange, 1, true).unwrap();
        assert_eq!(command.goods_id, 4);
        assert_eq!(command.count, 1);
    }

    #[test]
    fn stellar_directory_key_is_stable() {
        let board = board_from(&json!({
            "season": { "pass": { "nodes": [{ "claimable": true }] } },
            "constellation": { "currentDay": null, "groups": [{ "visualState": "claimableUnknown" }] },
            "shop": { "goods": [{ "id": "4", "name": "种子", "cost": { "name": "星砂", "count": 3 }, "maxExchangeCount": 1, "balanceKnown": true }] }
        })).unwrap();
        assert!(board.claimable);
        assert!(board.lightable);
        assert_eq!(board.day, "-");
    }
}
