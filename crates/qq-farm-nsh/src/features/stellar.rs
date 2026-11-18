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
    Solar,
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
    pub needs_refresh: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct StellarBoard {
    level: String,
    progress: String,
    nodes: Vec<PassNode>,
    goods: Vec<ShopGood>,
    groups: Vec<StarGroup>,
    rules: Vec<String>,
    catalog_status: String,
    terms: Vec<SolarRow>,
    claimable: bool,
    lightable: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct PassNode {
    level: String,
    state: String,
    rewards: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ShopGood {
    id: i64,
    name: String,
    category: String,
    owned: bool,
    cost: String,
    unit_cost: i64,
    max_count: i64,
    balance_known: bool,
    balance: Option<i64>,
}

#[derive(Clone, PartialEq, Eq)]
struct StarGroup {
    name: String,
    state: String,
    rewards: String,
}

#[derive(Clone, PartialEq, Eq)]
struct SolarRow {
    id: i64,
    name: String,
    can_claim: bool,
    status: String,
    rewards: String,
}

#[derive(Clone)]
pub(crate) struct StellarCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: StellarIntent,
    pub goods_id: i64,
    pub count: i64,
    pub term_id: i64,
}

pub(crate) fn board_from(value: &Value) -> Option<StellarBoard> {
    let season = value.get("season")?;
    let nodes = season
        .pointer("/pass/nodes")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(pass_node).collect::<Vec<_>>())
        .unwrap_or_default();
    let goods = value
        .pointer("/shop/goods")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(shop_good).collect::<Vec<_>>())
        .unwrap_or_default();
    let groups = value
        .pointer("/constellation/groups")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(star_group).collect::<Vec<_>>())
        .unwrap_or_default();
    let terms = value
        .pointer("/solarTerms/terms")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(solar_row).collect::<Vec<_>>())
        .unwrap_or_default();
    let rules = value
        .pointer("/constellation/rules")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| row.as_str()).map(str::to_owned).collect())
        .unwrap_or_default();
    Some(StellarBoard {
        level: season
            .pointer("/pass/level")
            .and_then(Value::as_i64)
            .map(|level| level.to_string())
            .unwrap_or_else(|| "-".into()),
        progress: format!(
            "{}/{}",
            season.pointer("/pass/progress").and_then(Value::as_i64).unwrap_or(0),
            season.pointer("/pass/progressMax").and_then(Value::as_i64).unwrap_or(0)
        ),
        claimable: nodes.iter().any(|node| node.state == "可领取"),
        lightable: groups.iter().any(|group| group.state == "可点亮" || group.state == "待同步"),
        nodes,
        goods,
        groups,
        rules,
        catalog_status: value
            .pointer("/constellation/catalogStatus")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        terms,
    })
}

pub(crate) fn press(
    session: &mut StellarSession,
    intent: StellarIntent,
    goods_id: i64,
    count: i64,
    term_id: i64,
    running: bool,
) -> Option<StellarCommand> {
    if !running
        || session.account_id.is_empty()
        || session.busy
        || session.needs_refresh && intent != StellarIntent::Reload
    {
        return None;
    }
    let board = session.board.as_ref();
    match intent {
        StellarIntent::Reload => {}
        StellarIntent::Claim => {
            if !board.is_some_and(|board| board.claimable) {
                return None;
            }
        }
        StellarIntent::Light => {
            if !board.is_some_and(|board| board.lightable) {
                return None;
            }
        }
        StellarIntent::Exchange => {
            let good = board?.goods.iter().find(|good| good.id == goods_id)?;
            if count <= 0 || !good.balance_known || good.unit_cost <= 0 || count > good.max_count {
                return None;
            }
        }
        StellarIntent::Solar => {
            let term = board?.terms.iter().find(|term| term.id == term_id)?;
            if !term.can_claim {
                return None;
            }
        }
    }
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
        term_id,
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
            let refresh = value.get("refreshRequired").and_then(Value::as_bool) == Some(true);
            if let Some(snapshot) =
                value.get("snapshot").filter(|snapshot| snapshot.get("season").is_some())
            {
                session.board = board_from(snapshot);
            } else if command.intent == StellarIntent::Reload {
                session.board = board_from(&value);
            } else if let Some(board) = session.board.as_mut() {
                if let Some(rows) = value.pointer("/shop/goods").and_then(Value::as_array) {
                    board.goods = rows.iter().filter_map(shop_good).collect();
                }
                if let Some(rows) = value.pointer("/constellation/groups").and_then(Value::as_array)
                {
                    board.groups = rows.iter().filter_map(star_group).collect();
                    board.lightable = board
                        .groups
                        .iter()
                        .any(|group| group.state == "可点亮" || group.state == "待同步");
                }
            }
            if refresh {
                session.needs_refresh = true;
                if let Some(board) = session.board.as_mut() {
                    board.claimable = false;
                    board.lightable = false;
                    for term in &mut board.terms {
                        term.can_claim = false;
                    }
                }
                session.note = Some("请刷新确认最新进度，刷新不会再次提交".into());
            } else {
                session.needs_refresh = false;
                session.note = None;
            }
            session.failure = None;
        }
        Err(message) => {
            if command.intent != StellarIntent::Reload {
                session.needs_refresh = true;
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
        StellarIntent::Solar => {
            qq_farm_app::stellar::claim_solar(context, &command.account_id, command.term_id).await
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
        if let Some(command) = press(&mut owned, StellarIntent::Reload, 0, 0, 0, true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

pub(crate) fn panel(session: Signal<StellarSession>, page: ActivityPage) -> impl IntoElement {
    let goods_id = signal(0_i64);
    let count = signal(1_i64);
    view()
        .gap_3()
        .child(text(move || text_key("activities.stellar")).text_lg())
        .child(when(
            move || session.get().note.is_some(),
            move || {
                notice(move || SharedString::from(session.get().note.clone().unwrap_or_default()))
            },
        ))
        .child(when(
            move || session.get().board.is_none() && !session.get().busy,
            move || {
                button(move || text_key("activities.wishReload"), ButtonVariant::Secondary)
                    .on_press(move || fire(session, page, StellarIntent::Reload, 0, 0, 0))
            },
        ))
        .child(when(
            move || session.get().board.is_some(),
            move || body(session, page, goods_id, count),
        ))
}

fn body(
    session: Signal<StellarSession>,
    page: ActivityPage,
    goods_id: Signal<i64>,
    count: Signal<i64>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || {
            let board = session.get().board.clone().unwrap_or_else(empty_board);
            SharedString::from(format!("游记等级 {} · 进度 {}", board.level, board.progress))
        }))
        .child(each(
            move || session.get().board.clone().unwrap_or_else(empty_board).nodes,
            |node: &PassNode| node.level.clone(),
            move |node, _| {
                let node = node.get_untracked();
                text(format!("{} {} {}", node.level, node.state, node.rewards)).text_sm()
            },
        ))
        .child(
            button(move || text_key("activities.stellarClaim"), ButtonVariant::Primary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || !current.board.as_ref().is_some_and(|board| board.claimable)
                })
                .on_press(move || fire(session, page, StellarIntent::Claim, 0, 0, 0)),
        )
        .child(text(move || text_key("activities.stellarShop")))
        .child(each(
            move || session.get().board.clone().unwrap_or_else(empty_board).goods,
            |good: &ShopGood| good.id,
            move |good, _| {
                let good = good.get_untracked();
                let id = good.id;
                let owned = if good.owned { "已拥有" } else { "未拥有" };
                let balance = good
                    .balance
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "余额未知".into());
                let line = format!(
                    "{} {} {} {} 余额{balance} 最多{}",
                    good.category, good.name, owned, good.cost, good.max_count
                );
                button(move || SharedString::from(line.clone()), ButtonVariant::Quiet)
                    .selected(move || goods_id.get() == id)
                    .on_press(move || goods_id.set(id))
            },
        ))
        .child(text(move || {
            let chosen = count.get();
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            let Some(good) = board.goods.iter().find(|good| good.id == goods_id.get()) else {
                return SharedString::from(format!("数量 {chosen}"));
            };
            SharedString::from(format!(
                "{} {} ×{} · 数量 {chosen}",
                text_key("activities.stellarCost"),
                good.cost,
                good.unit_cost.saturating_mul(chosen.max(0))
            ))
        }))
        .child(button(move || SharedString::from("-"), ButtonVariant::Secondary).on_press(
            move || {
                count.update(|value| *value = (*value - 1).max(1));
            },
        ))
        .child(button(move || SharedString::from("+"), ButtonVariant::Secondary).on_press(
            move || {
                let max = session
                    .get_untracked()
                    .board
                    .as_ref()
                    .and_then(|board| {
                        board.goods.iter().find(|good| good.id == goods_id.get_untracked())
                    })
                    .map(|good| good.max_count)
                    .unwrap_or(1);
                count.update(|value| *value = (*value + 1).min(max.max(1)));
            },
        ))
        .child(
            button(move || text_key("activities.stellarExchange"), ButtonVariant::Primary)
                .disabled(move || session.get().busy || session.get().needs_refresh)
                .on_press(move || {
                    let id = goods_id.get_untracked();
                    let chosen = count.get_untracked();
                    confirm(
                        "activities.stellarExchange",
                        "activities.stellarExchange",
                        "exchange",
                        "activities.stellarExchange",
                        move |accepted| {
                            if accepted {
                                fire(session, page, StellarIntent::Exchange, id, chosen, 0);
                            }
                        },
                    );
                }),
        )
        .child(text(move || {
            let status = session
                .get()
                .board
                .as_ref()
                .map(|board| board.catalog_status.clone())
                .unwrap_or_default();
            if status == "unsupported" {
                text_key("activities.stellarUnsupported")
            } else {
                text_key("activities.stellarStars")
            }
        }))
        .child(each(
            move || session.get().board.clone().unwrap_or_else(empty_board).rules,
            |rule: &String| rule.clone(),
            move |rule, _| text(rule.get_untracked().clone()).text_sm(),
        ))
        .child(each(
            move || session.get().board.clone().unwrap_or_else(empty_board).groups,
            |group: &StarGroup| group.name.clone(),
            move |group, _| {
                let group = group.get_untracked();
                text(format!("{} {} {}", group.name, group.state, group.rewards)).text_sm()
            },
        ))
        .child(
            button(move || text_key("activities.stellarLight"), ButtonVariant::Primary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || !current.board.as_ref().is_some_and(|board| board.lightable)
                })
                .on_press(move || fire(session, page, StellarIntent::Light, 0, 0, 0)),
        )
        .child(text(move || text_key("activities.stellarSolarV1")))
        .child(each(
            move || session.get().board.clone().unwrap_or_else(empty_board).terms,
            |term: &SolarRow| term.id,
            move |term, _| {
                let term = term.get_untracked();
                let id = term.id;
                let can_claim = term.can_claim;
                view()
                    .gap_1()
                    .child(
                        text(format!("{} {} {}", term.name, term.status, term.rewards)).text_sm(),
                    )
                    .child(
                        button(
                            move || text_key("activities.stellarSolarClaim"),
                            ButtonVariant::Secondary,
                        )
                        .disabled(move || {
                            session.get().busy || session.get().needs_refresh || !can_claim
                        })
                        .on_press(move || fire(session, page, StellarIntent::Solar, 0, 0, id)),
                    )
            },
        ))
}

fn fire(
    session: Signal<StellarSession>,
    page: ActivityPage,
    intent: StellarIntent,
    goods_id: i64,
    count: i64,
    term_id: i64,
) {
    let running = page
        .account_id
        .get_untracked()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| super::accounts::account_is_running(&rows, &id));
    let mut owned = session.get_untracked();
    let Some(command) = press(&mut owned, intent, goods_id, count, term_id, running) else {
        return;
    };
    session.set(owned);
    launch(session, command);
}

fn launch(session: Signal<StellarSession>, command: StellarCommand) {
    spawn(async move {
        let context = session::session().context.clone();
        let result = {
            let context = context.clone();
            let command = command.clone();
            session::call(async move { dispatch_stellar(&context, &command).await }).await
        };
        let mut owned = session.get_untracked();
        let _accepted = apply(&mut owned, &command, result);
        session.set(owned);
    });
}

fn pass_node(value: &Value) -> Option<PassNode> {
    let level = value.get("level").and_then(Value::as_i64).unwrap_or(0).to_string();
    let state = if value.get("claimable").and_then(Value::as_bool) == Some(true) {
        "可领取"
    } else if value.get("claimed").and_then(Value::as_bool) == Some(true) {
        "已领取"
    } else if value.get("locked").and_then(Value::as_bool) == Some(true) {
        "未开放"
    } else {
        "进行中"
    };
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| reward_line(rows))
        .unwrap_or_default();
    Some(PassNode { level, state: state.into(), rewards })
}

fn shop_good(value: &Value) -> Option<ShopGood> {
    let id = value.get("id").and_then(Value::as_str).and_then(|id| id.parse().ok())?;
    Some(ShopGood {
        id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        category: value.get("category").and_then(Value::as_str).unwrap_or("").to_owned(),
        owned: value.get("owned").and_then(Value::as_bool) == Some(true),
        cost: value.pointer("/cost/name").and_then(Value::as_str).unwrap_or("").to_owned(),
        unit_cost: value.pointer("/cost/count").and_then(Value::as_i64).unwrap_or(0),
        max_count: value.get("maxExchangeCount").and_then(Value::as_i64).unwrap_or(0),
        balance_known: value.get("balanceKnown").and_then(Value::as_bool) == Some(true),
        balance: value.get("balance").and_then(Value::as_i64),
    })
}

fn star_group(value: &Value) -> Option<StarGroup> {
    let state = match value.get("visualState").and_then(Value::as_str).unwrap_or("") {
        "lit" => "已点亮",
        "lightable" => "可点亮",
        "locked" => "未开放",
        "claimableUnknown" => "待同步",
        "claimedToday" => "今日已领取",
        _ => "未知",
    };
    Some(StarGroup {
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        state: state.into(),
        rewards: value
            .get("rewards")
            .and_then(Value::as_array)
            .map(|rows| reward_line(rows))
            .unwrap_or_default(),
    })
}

fn solar_row(value: &Value) -> Option<SolarRow> {
    Some(SolarRow {
        id: value.get("id").and_then(Value::as_str).and_then(|id| id.parse().ok())?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        can_claim: value.get("canClaim").and_then(Value::as_bool) == Some(true),
        status: value.get("statusCode").and_then(Value::as_str).unwrap_or("").to_owned(),
        rewards: value
            .get("rewards")
            .and_then(Value::as_array)
            .map(|rows| reward_line(rows))
            .unwrap_or_default(),
    })
}

fn reward_line(rows: &[Value]) -> String {
    rows.iter()
        .map(|row| {
            format!(
                "{}×{}",
                row.get("name").and_then(Value::as_str).unwrap_or(""),
                row.get("count").and_then(Value::as_i64).unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join("、")
}

fn empty_board() -> StellarBoard {
    StellarBoard {
        level: "-".into(),
        progress: "0/0".into(),
        nodes: Vec::new(),
        goods: Vec::new(),
        groups: Vec::new(),
        rules: Vec::new(),
        catalog_status: String::new(),
        terms: Vec::new(),
        claimable: false,
        lightable: false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use prost::Message as _;
    use qq_farm_core::models::types::AccountConfigSnapshot;
    use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
    use qq_farm_core::proto::generated::gamepb::activitypb::{
        ActivityData, ActivityItem, ActivityOperateReply, ExchangeShopRequest, StarSandGoods,
        StarSandGoodsList,
    };
    use qq_farm_core::proto::generated::gamepb::itempb::BagReply;
    use qq_farm_core::proto::generated::gamepb::seasonpb::{
        GetSeasonInfoReply, SeasonActivity, SeasonInfo,
    };
    use qq_farm_core::proto::generated::gamepb::solartermspb::{
        ClaimSolarTermsReply, ClaimSolarTermsRequest, GetSolarTermsReply, SolarTermInfo,
    };
    use qq_farm_core::runtime::runtime_state::AccountStoreLike;
    use serde_json::json;

    use super::*;

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

    fn loaded() -> StellarSession {
        let board = board_from(&json!({
            "season": { "pass": { "level": 2, "progress": 3, "progressMax": 10, "nodes": [
                { "level": 1, "claimed": true, "claimable": false, "locked": false, "rewards": [{ "name": "点券", "count": 5 }] },
                { "level": 2, "claimed": false, "claimable": true, "locked": false, "rewards": [{ "name": "种子", "count": 1 }] }
            ] } },
            "shop": { "goods": [
                { "id": "4", "name": "甲", "category": "种子", "owned": false, "cost": { "name": "星砂", "count": 3 }, "maxExchangeCount": 1, "balanceKnown": true },
                { "id": "5", "name": "乙", "category": "道具", "owned": true, "cost": { "name": "星砂", "count": 4 }, "maxExchangeCount": 0, "balanceKnown": true }
            ] },
            "constellation": { "catalogStatus": "supported", "rules": ["每日馈赠"], "groups": [
                { "name": "角宿", "visualState": "claimedToday" },
                { "name": "亢宿", "visualState": "lightable" }
            ] },
            "solarTerms": { "terms": [{ "id": "8", "name": "立春", "statusCode": "2", "canClaim": true }] }
        })).unwrap();
        StellarSession { account_id: "acc".into(), board: Some(board), ..StellarSession::default() }
    }

    #[test]
    fn lists_keep_every_good_and_cancel_does_not_press() {
        let board = loaded().board.unwrap();
        assert_eq!(board.goods.len(), 2);
        assert_eq!(board.nodes.len(), 2);
        assert_eq!(board.groups[0].state, "今日已领取");
        assert_eq!(board.rules, vec!["每日馈赠".to_string()]);
        assert!(board.terms[0].can_claim);
        let mut session = loaded();
        assert!(press(&mut session, StellarIntent::Exchange, 4, 0, 0, true).is_none());
        assert!(press(&mut session, StellarIntent::Exchange, 5, 1, 0, true).is_none());
        let command = press(&mut session, StellarIntent::Solar, 0, 0, 8, true).unwrap();
        assert_eq!(command.term_id, 8);
    }

    #[test]
    fn refresh_required_disables_another_write_and_a_late_apply_is_dropped() {
        let mut session = loaded();
        let command = press(&mut session, StellarIntent::Claim, 0, 0, 0, true).unwrap();
        assert!(apply(
            &mut session,
            &command,
            Ok(json!({ "refreshRequired": true, "rewards": [{ "id": 1, "count": 1 }] }))
        ));
        assert!(session.needs_refresh);
        assert!(!session.board.as_ref().unwrap().claimable);
        assert!(press(&mut session, StellarIntent::Exchange, 4, 1, 0, true).is_none());
        let mut other = session.clone();
        reset_account(&mut other, "other".into());
        assert!(!apply(&mut other, &command, Ok(json!({ "season": { "pass": { "nodes": [] } } }))));
    }

    #[test]
    fn a_write_without_a_top_level_season_refreshes_from_the_snapshot() {
        let mut session = loaded();
        let command = press(&mut session, StellarIntent::Exchange, 4, 1, 0, true).unwrap();
        assert!(apply(
            &mut session,
            &command,
            Ok(json!({
                "purchaseCount": 1,
                "snapshot": {
                    "season": { "pass": { "level": 2, "progress": 4, "progressMax": 10, "nodes": [] } },
                    "shop": { "goods": [
                        { "id": "4", "name": "甲", "category": "种子", "owned": true, "cost": { "name": "星砂", "count": 3 }, "balance": 6, "maxExchangeCount": 2, "balanceKnown": true },
                        { "id": "5", "name": "乙", "category": "道具", "owned": false, "cost": { "name": "星砂", "count": 4 }, "balance": 6, "maxExchangeCount": 1, "balanceKnown": true }
                    ] }
                }
            }))
        ));
        let board = session.board.unwrap();
        assert_eq!(board.progress, "4/10");
        assert_eq!(board.goods.len(), 2);
        assert_eq!(board.goods[0].balance, Some(6));
        assert!(!session.needs_refresh);
    }

    #[tokio::test]
    async fn dispatch_stellar_sends_the_pressed_exchange_and_solar_claim() {
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
        let seen = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
        let log = Arc::clone(&seen);
        gateway.install_reply_script(move |_service, method, body| {
            log.lock().expect("log").push((method.to_owned(), body.to_vec()));
            match method {
                "GetSeasonInfo" => Ok(GetSeasonInfoReply {
                    season_info: Some(SeasonInfo {
                        season_id: 7,
                        server_time: 1_700_000_000,
                        activities: vec![SeasonActivity {
                            activity_id: 900,
                            r#type: 3,
                            ..SeasonActivity::default()
                        }],
                        ..SeasonInfo::default()
                    }),
                }
                .encode_to_vec()),
                "Bag" => Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![CoreItem { id: 3, count: 10, ..CoreItem::default() }],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec()),
                "Operate" => {
                    let kind = qq_farm_core::proto::generated::gamepb::activitypb::QueryActivityRequest::decode(body)
                        .map(|request| request.operate_type)
                        .unwrap_or(7);
                    Ok(ActivityOperateReply {
                    activity_id: 900,
                    operate_type: kind,
                    data: Some(ActivityData {
                        catalog: Some(StarSandGoodsList {
                            goods: vec![StarSandGoods {
                                goods_id: 4,
                                cost: Some(ActivityItem { item_id: 3, count: 3 }),
                                ..StarSandGoods::default()
                            }],
                        }),
                        ..ActivityData::default()
                    }),
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec())
                }
                "GetSolarTerms" => Ok(GetSolarTermsReply {
                    terms: vec![SolarTermInfo { term_id: 8, status: 2, ..SolarTermInfo::default() }],
                    ..GetSolarTermsReply::default()
                }
                .encode_to_vec()),
                "ClaimSolarTerms" => Ok(ClaimSolarTermsReply::default().encode_to_vec()),
                _ => Err(qq_farm_core::network::error::NetworkError::Phase(method.into())),
            }
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
        session.board.as_mut().unwrap().goods[0].max_count = 3;
        let exchange = press(&mut session, StellarIntent::Exchange, 4, 2, 0, true).unwrap();
        assert_eq!(exchange.count, 2);
        dispatch_stellar(&context, &exchange).await.unwrap();
        session.busy = false;
        let solar = press(&mut session, StellarIntent::Solar, 0, 0, 8, true).unwrap();
        dispatch_stellar(&context, &solar).await.unwrap();
        let saved = seen.lock().expect("seen");
        let exchange_body = saved.iter().find(|(method, body)| {
            method == "Operate"
                && ExchangeShopRequest::decode(body.as_slice()).unwrap().operate_type == 1
        });
        let request = ExchangeShopRequest::decode(exchange_body.unwrap().1.as_slice()).unwrap();
        let params = request.exchange_shop_operate.unwrap();
        assert_eq!(params.goods_id, 4);
        assert_eq!(params.count, 2);
        let claim = saved.iter().find(|(method, _)| method == "ClaimSolarTerms").unwrap();
        assert_eq!(ClaimSolarTermsRequest::decode(claim.1.as_slice()).unwrap().term_id, 8);
    }
}
