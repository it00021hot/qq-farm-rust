use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QixiIntent {
    Reload,
    Claim,
    Gift,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct QixiSession {
    pub account_id: String,
    pub generation: u64,
    pub busy: bool,
    pub board: Option<QixiBoard>,
    pub failure: Option<String>,
    pub note: Option<String>,
    pub attempted: bool,
    pub needs_refresh: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct QixiBoard {
    name: String,
    window: String,
    active: bool,
    feather: String,
    sachet: String,
    received: String,
    dew: String,
    sent: String,
    send_limit: String,
    receive_limit: String,
    rules_title: String,
    rules: Vec<String>,
    stages: Vec<QixiStage>,
    exchanges: Vec<String>,
    claim_enabled: bool,
    gift_enabled: bool,
    dew_hint: String,
}

#[derive(Clone, PartialEq, Eq)]
struct QixiStage {
    stage: String,
    state: String,
    cost: String,
    rewards: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct QixiFriend {
    pub gid: i64,
    pub name: String,
    pub level: i64,
}

#[derive(Clone)]
pub(crate) struct QixiCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: QixiIntent,
    pub friend_gid: i64,
}

pub(crate) fn board_from(value: &Value) -> Option<QixiBoard> {
    let stages = value
        .pointer("/bridge/stages")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(stage_from).collect::<Vec<_>>())
        .unwrap_or_default();
    let exchanges = value
        .pointer("/gift/exchanges")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().map(exchange_line).collect::<Vec<_>>())
        .unwrap_or_default();
    let rules = value
        .pointer("/rules/paragraphs")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| row.as_str()).map(str::to_owned).collect())
        .unwrap_or_default();
    Some(QixiBoard {
        name: value.get("name").and_then(Value::as_str).unwrap_or("鹊桥寄情").to_owned(),
        window: value.get("window").and_then(Value::as_str).unwrap_or("").to_owned(),
        active: value.get("active").and_then(Value::as_bool) == Some(true),
        feather: balance_text(value, "feather"),
        sachet: balance_text(value, "sachet"),
        received: balance_text(value, "receivedSachet"),
        dew: balance_text(value, "dew"),
        sent: value.pointer("/gift/sentCount").and_then(Value::as_i64).unwrap_or(0).to_string(),
        send_limit: value
            .pointer("/gift/sendLimit")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .to_string(),
        receive_limit: value
            .pointer("/gift/receiveLimit")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .to_string(),
        rules_title: value.pointer("/rules/title").and_then(Value::as_str).unwrap_or("").to_owned(),
        rules,
        stages,
        exchanges,
        claim_enabled: value.pointer("/actions/bridge/enabled").and_then(Value::as_bool)
            == Some(true),
        gift_enabled: value.pointer("/actions/gift/enabled").and_then(Value::as_bool) == Some(true),
        dew_hint: value.get("dewHint").and_then(Value::as_str).unwrap_or("").to_owned(),
    })
}

pub(crate) fn press(
    session: &mut QixiSession,
    intent: QixiIntent,
    friend_gid: i64,
    running: bool,
) -> Option<QixiCommand> {
    if !running
        || session.account_id.trim().is_empty()
        || session.busy
        || session.needs_refresh && intent != QixiIntent::Reload
    {
        return None;
    }
    match intent {
        QixiIntent::Reload => {}
        QixiIntent::Claim => {
            if !session.board.as_ref().is_some_and(|board| board.claim_enabled) {
                return None;
            }
        }
        QixiIntent::Gift => {
            let board = session.board.as_ref()?;
            if !board.active || !board.gift_enabled || friend_gid <= 0 {
                return None;
            }
        }
    }
    session.busy = true;
    session.attempted = true;
    session.generation += 1;
    session.failure = None;
    Some(QixiCommand {
        account_id: session.account_id.clone(),
        generation: session.generation,
        intent,
        friend_gid,
    })
}

pub(crate) fn reset_account(session: &mut QixiSession, account_id: String) {
    if session.account_id == account_id {
        return;
    }
    *session =
        QixiSession { account_id, generation: session.generation + 1, ..QixiSession::default() };
}

pub(crate) fn apply(
    session: &mut QixiSession,
    command: &QixiCommand,
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
                value.get("snapshot").filter(|snapshot| snapshot.get("bridge").is_some())
            {
                session.board = board_from(snapshot);
            } else if command.intent == QixiIntent::Reload {
                session.board = board_from(&value);
            }
            if refresh {
                session.needs_refresh = true;
                if let Some(board) = session.board.as_mut() {
                    board.claim_enabled = false;
                    board.gift_enabled = false;
                }
                session.note = Some("请刷新确认最新进度，刷新不会再次提交".into());
            } else {
                session.needs_refresh = false;
                session.note = None;
            }
            session.failure = None;
        }
        Err(message) => {
            if command.intent != QixiIntent::Reload {
                session.needs_refresh = true;
            }
            session.failure = Some(message);
        }
    }
    true
}

pub(crate) async fn dispatch_qixi(
    context: &qq_farm_app::AppContext,
    command: &QixiCommand,
) -> qq_farm_app::AppResult<Value> {
    match command.intent {
        QixiIntent::Reload => qq_farm_app::qixi::snapshot(context, &command.account_id).await,
        QixiIntent::Claim => qq_farm_app::qixi::claim(context, &command.account_id).await,
        QixiIntent::Gift => {
            qq_farm_app::qixi::gift(context, &command.account_id, command.friend_gid).await
        }
    }
}

pub(crate) fn watch(account: impl Fn() -> Option<String> + 'static, session: Signal<QixiSession>) {
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
        if let Some(command) = press(&mut owned, QixiIntent::Reload, 0, true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

pub(crate) fn panel(
    session: Signal<QixiSession>,
    page: ActivityPage,
    friends: Resource<Vec<QixiFriend>, String>,
) -> impl IntoElement {
    let picked = signal(0_i64);
    let search = signal(String::new());
    view()
        .gap_3()
        .child(text(move || text_key("activities.qixi")).text_lg())
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
                button(move || text_key("activities.wishReload"), ButtonVariant::Secondary)
                    .on_press(move || fire(session, page, QixiIntent::Reload, 0))
            },
        ))
        .child(when(
            move || session.get().board.is_some(),
            move || body(session, page, friends, picked, search),
        ))
}

fn body(
    session: Signal<QixiSession>,
    page: ActivityPage,
    friends: Resource<Vec<QixiFriend>, String>,
    picked: Signal<i64>,
    search: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || {
            let board = session.get().board.clone();
            let Some(board) = board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "{} · {} · 鹊羽 {} · 香囊 {} · 获赠 {} · 灵露 {}",
                board.name,
                window_text(&board.window),
                board.feather,
                board.sachet,
                board.received,
                board.dew
            ))
        }))
        .child(text(move || {
            session.get().board.as_ref().map(|board| board.dew_hint.clone()).unwrap_or_default()
        }))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "已赠 {} / 赠送上限 {} / 获赠上限 {}",
                board.sent, board.send_limit, board.receive_limit
            ))
        }))
        .child(each(
            move || session.get().board.clone().map(|board| board.stages).unwrap_or_default(),
            |stage: &QixiStage| stage.stage.clone(),
            move |stage, _| {
                let stage = stage.get_untracked();
                text(format!(
                    "第 {} 阶段 {} {} {}",
                    stage.stage, stage.state, stage.cost, stage.rewards
                ))
                .text_sm()
            },
        ))
        .child(
            button(move || text_key("activities.qixiClaim"), ButtonVariant::Primary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || !current.board.as_ref().is_some_and(|board| board.claim_enabled)
                })
                .on_press(move || fire(session, page, QixiIntent::Claim, 0)),
        )
        .child(when(
            move || session.get().board.as_ref().is_some_and(|board| board.active),
            move || gift_section(session, page, friends, picked, search),
        ))
        .child(text(move || {
            session
                .get()
                .board
                .as_ref()
                .map(|board| board.rules_title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| text_key("activities.qixiRules").to_string())
        }))
        .child(each(
            move || session.get().board.clone().map(|board| board.rules).unwrap_or_default(),
            |rule: &String| rule.clone(),
            move |rule, _| text(rule.get_untracked().clone()).text_sm(),
        ))
        .child(each(
            move || session.get().board.clone().map(|board| board.exchanges).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
}

fn gift_section(
    session: Signal<QixiSession>,
    page: ActivityPage,
    friends: Resource<Vec<QixiFriend>, String>,
    picked: Signal<i64>,
    search: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(text(move || text_key("activities.qixiGift")))
        .child(text(move || text_key("activities.qixiFixed")).text_sm())
        .child(field(
            move || search.get(),
            move || text_key("activities.qixiSearch").to_string(),
            move |value| search.set(value.to_owned()),
        ))
        .child(
            button(move || text_key("activities.qixiFriends"), ButtonVariant::Secondary)
                .on_press(move || friends.refetch()),
        )
        .child(when(
            move || friends.loading() && friends.get().is_none(),
            move || notice(move || text_key("activities.qixiFriendsLoading")),
        ))
        .child(each(
            move || visible_friends(friends.get().unwrap_or_default(), &search.get()),
            |friend: &QixiFriend| friend.gid,
            move |friend, _| {
                let friend = friend.get_untracked();
                let gid = friend.gid;
                let line = format!("{} GID {} Lv.{}", friend.name, friend.gid, friend.level);
                button(move || SharedString::from(line.clone()), ButtonVariant::Quiet)
                    .selected(move || picked.get() == gid)
                    .on_press(move || picked.set(gid))
            },
        ))
        .child(
            button(move || text_key("activities.qixiSend"), ButtonVariant::Primary)
                .disabled(move || {
                    let current = session.get();
                    current.busy
                        || current.needs_refresh
                        || picked.get() <= 0
                        || !current
                            .board
                            .as_ref()
                            .is_some_and(|board| board.active && board.gift_enabled)
                })
                .on_press(move || fire(session, page, QixiIntent::Gift, picked.get_untracked())),
        )
}

fn fire(session: Signal<QixiSession>, page: ActivityPage, intent: QixiIntent, friend_gid: i64) {
    let running = page
        .account_id
        .get_untracked()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| super::accounts::account_is_running(&rows, &id));
    let mut owned = session.get_untracked();
    let Some(command) = press(&mut owned, intent, friend_gid, running) else {
        return;
    };
    session.set(owned);
    launch(session, command);
}

fn launch(session: Signal<QixiSession>, command: QixiCommand) {
    spawn(async move {
        let context = session::session().context.clone();
        let result = {
            let context = context.clone();
            let command = command.clone();
            session::call(async move { dispatch_qixi(&context, &command).await }).await
        };
        let mut owned = session.get_untracked();
        let _accepted = apply(&mut owned, &command, result);
        session.set(owned);
    });
}

fn visible_friends(friends: Vec<QixiFriend>, search: &str) -> Vec<QixiFriend> {
    let keyword = search.trim().to_lowercase();
    friends
        .into_iter()
        .filter(|friend| friend.gid > 0)
        .filter(|friend| {
            keyword.is_empty()
                || friend.name.to_lowercase().contains(&keyword)
                || friend.gid.to_string().contains(&keyword)
        })
        .take(60)
        .collect()
}

fn stage_from(value: &Value) -> Option<QixiStage> {
    let cost = value.get("cost")?;
    Some(QixiStage {
        stage: value.get("stage").and_then(Value::as_i64).unwrap_or(0).to_string(),
        state: value.get("state").and_then(Value::as_str).unwrap_or("").to_owned(),
        cost: format!(
            "{}×{}",
            cost.get("name").and_then(Value::as_str).unwrap_or(""),
            cost.get("count").and_then(Value::as_i64).unwrap_or(0)
        ),
        rewards: value
            .get("rewards")
            .and_then(Value::as_array)
            .map(|rows| {
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
            })
            .unwrap_or_default(),
    })
}

fn exchange_line(value: &Value) -> String {
    let cost = value.get("costItems").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0);
    let receive =
        value.get("receiveItems").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0);
    format!("香囊交换 消耗{cost}项 获得{receive}项")
}

fn balance_text(value: &Value, key: &str) -> String {
    if value.pointer("/balances/known").and_then(Value::as_bool) != Some(true) {
        return "--".into();
    }
    value.pointer(&format!("/balances/{key}")).and_then(Value::as_i64).unwrap_or(0).to_string()
}

fn window_text(window: &str) -> String {
    text_key(match window {
        "upcoming" => "activities.qixiUpcoming",
        "ended" => "activities.qixiEnded",
        "open" => "activities.qixiOpen",
        _ => "activities.qixiTimeUnknown",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn loaded() -> QixiSession {
        let board = board_from(&json!({
            "name": "鹊桥寄情",
            "window": "open",
            "active": true,
            "balances": { "known": true, "feather": 40, "sachet": 2, "receivedSachet": 1, "dew": 0 },
            "dewHint": "土地上使用",
            "rules": { "title": "说明", "paragraphs": ["筑桥", "香囊"] },
            "bridge": { "stages": [
                { "stage": 1, "state": "已完成", "cost": { "name": "鹊羽", "count": 10 }, "rewards": [{ "name": "露", "count": 1 }] },
                { "stage": 2, "state": "待领取", "cost": { "name": "鹊羽", "count": 20 }, "rewards": [{ "name": "囊", "count": 1 }, { "name": "露", "count": 2 }] },
                { "stage": 3, "state": "未达成", "cost": { "name": "鹊羽", "count": 30 }, "rewards": [{ "name": "礼", "count": 3 }] }
            ] },
            "gift": { "sentCount": 2, "sendLimit": 5, "receiveLimit": 7, "exchanges": [{ "costItems": [{}], "receiveItems": [{}] }] },
            "actions": { "bridge": { "enabled": true }, "gift": { "enabled": true } }
        }))
        .unwrap();
        QixiSession { account_id: "acc".into(), board: Some(board), ..QixiSession::default() }
    }

    #[test]
    fn every_stage_is_shown_and_an_unselected_friend_does_not_press() {
        let board = loaded().board.unwrap();
        assert_eq!(board.stages.len(), 3);
        assert_eq!(board.stages[1].rewards, "囊×1、露×2");
        assert_eq!(board.rules, vec!["筑桥".to_string(), "香囊".to_string()]);
        assert_eq!(board.rules_title, "说明");
        let mut session = loaded();
        assert!(press(&mut session, QixiIntent::Gift, 0, true).is_none());
        let command = press(&mut session, QixiIntent::Gift, 88, true).unwrap();
        assert_eq!(command.friend_gid, 88);
    }

    #[test]
    fn refresh_required_disables_the_old_actions_and_a_late_apply_is_dropped() {
        let mut session = loaded();
        let command = press(&mut session, QixiIntent::Claim, 0, true).unwrap();
        assert!(apply(
            &mut session,
            &command,
            Ok(json!({
                "rewards": [{ "id": "1", "count": 1 }],
                "refreshRequired": true
            }))
        ));
        assert!(session.needs_refresh);
        assert!(!session.board.as_ref().unwrap().claim_enabled);
        assert!(press(&mut session, QixiIntent::Gift, 88, true).is_none());
        let mut other = session.clone();
        reset_account(&mut other, "other".into());
        assert!(!apply(&mut other, &command, Ok(json!({ "bridge": { "stages": [] } }))));
    }

    #[tokio::test]
    async fn dispatch_qixi_sends_the_pressed_friend() {
        use std::sync::{Arc, Mutex};

        use prost::Message as _;
        use qq_farm_core::models::types::AccountConfigSnapshot;
        use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityContent, ActivityData, ActivityOperateReply, GetGroupReply,
            GiftQixiSachetRequest,
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
            if method == "Operate" {
                log.lock().expect("log").push(body.to_vec());
                return Ok(ActivityOperateReply {
                    activity_id: 2_026_081_802,
                    operate_type: 26,
                    ..ActivityOperateReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![CoreItem { id: 1025, count: 2, ..CoreItem::default() }],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec());
            }
            Ok(GetGroupReply {
                group: Some(ActivityData {
                    children: vec![
                        ActivityData {
                            activity: Some(ActivityContent {
                                activity_id: 2_026_081_801,
                                begin_time: 0,
                                end_time: 0,
                                field_23: 1,
                                ..ActivityContent::default()
                            }),
                            ..ActivityData::default()
                        },
                        ActivityData {
                            activity: Some(ActivityContent {
                                activity_id: 2_026_081_802,
                                begin_time: 0,
                                end_time: 0,
                                ..ActivityContent::default()
                            }),
                            ..ActivityData::default()
                        },
                    ],
                    ..ActivityData::default()
                }),
            }
            .encode_to_vec())
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
        assert!(press(&mut session, QixiIntent::Gift, 0, true).is_none());
        let command = press(&mut session, QixiIntent::Gift, 88, true).unwrap();
        dispatch_qixi(&context, &command).await.unwrap();
        let request =
            GiftQixiSachetRequest::decode(seen.lock().expect("seen")[0].as_slice()).unwrap();
        assert_eq!(request.activity_id, 2_026_081_802);
        assert_eq!(request.operate_type, 26);
        let params = request.params.unwrap();
        assert_eq!(params.target_gid, 88);
        assert_eq!(params.msg_text_id, 15);
    }
}
