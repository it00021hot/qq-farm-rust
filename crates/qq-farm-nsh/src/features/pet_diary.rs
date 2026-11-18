use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PetIntent {
    Reload,
    Feed,
    Draw,
    Initialize,
    ClaimDog,
    Seeds,
    Story(i64),
    RefreshFree,
    RefreshPaid(i64),
    Equip(i64),
    OpenTreasure,
    Compensation,
    Exchange { goods_id: i64, count: i64 },
    Solar(i64),
    Friend(i64),
    InteractLog,
    PlunderLog,
    Battle { gid: i64, treasure_id: String, challenge_id: i64 },
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct PetSession {
    pub account_id: String,
    pub generation: u64,
    pub busy: bool,
    pub board: Option<PetBoard>,
    pub failure: Option<String>,
    pub note: Option<String>,
    pub attempted: bool,
    pub needs_refresh: bool,
    pub exchange_count: i64,
    pub section: String,
    pub confirming_goods: Option<i64>,
    pub friend_gid: Option<i64>,
    pub selected_treasure: String,
    pub selected_challenge: i64,
    pub friend_treasures: Vec<String>,
    pub logs: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PetBoard {
    title: String,
    end_time: String,
    active: bool,
    bag_known: bool,
    rules_title: String,
    rules: Vec<String>,
    treasure_rules_title: String,
    treasure_rules: Vec<String>,
    warnings: Vec<String>,
    balances: Vec<String>,
    growth: String,
    feed: String,
    can_feed: bool,
    can_draw: bool,
    can_initialize: bool,
    can_dog: bool,
    can_seeds: bool,
    stories: Vec<(i64, String, bool)>,
    charms: String,
    charm_lines: Vec<String>,
    plants: Vec<String>,
    can_refresh_free: bool,
    can_refresh_paid: bool,
    paid_count: i64,
    charm_choices: Vec<i64>,
    treasures: Vec<String>,
    can_open: bool,
    compensation: String,
    shop: Vec<(i64, String, bool, i64)>,
    solar: Vec<(i64, String, bool)>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PetCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: PetIntent,
}

fn text_lines(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| row.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn board_from(value: &Value) -> Option<PetBoard> {
    if value.get("available").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let nurture = value.get("nurture")?;
    let stories = value
        .get("stories")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let order = row.get("order").and_then(Value::as_i64).unwrap_or(0);
                    let claimed = row.get("claimed").and_then(Value::as_bool) == Some(true);
                    let unlocked = row.get("unlocked").and_then(Value::as_bool) == Some(true);
                    (
                        order,
                        format!(
                            "手记 {order} {} {} {}",
                            if claimed {
                                "已领取"
                            } else if unlocked {
                                "可领取"
                            } else {
                                "未解锁"
                            },
                            row.get("photo").and_then(Value::as_str).unwrap_or(""),
                            row.get("note").and_then(Value::as_str).unwrap_or("")
                        ),
                        unlocked && !claimed,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let shop = value
        .get("shop")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let id = row.get("id").and_then(Value::as_str)?.parse().ok()?;
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("商品");
                    let exchangeable =
                        row.get("exchangeable").and_then(Value::as_bool) == Some(true);
                    let costs = row
                        .get("costs")
                        .and_then(Value::as_array)
                        .map(|costs| {
                            costs
                                .iter()
                                .map(|cost| {
                                    if cost.get("known").and_then(Value::as_bool) == Some(false) {
                                        "消耗未知".to_owned()
                                    } else {
                                        format!(
                                            "{}×{}",
                                            cost.get("name")
                                                .and_then(Value::as_str)
                                                .unwrap_or("物品"),
                                            cost.get("count").and_then(Value::as_i64).unwrap_or(0)
                                        )
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("、")
                        })
                        .unwrap_or_else(|| "消耗未知".into());
                    let remaining = row
                        .get("remaining")
                        .and_then(Value::as_str)
                        .and_then(|text| text.parse::<i64>().ok());
                    let affordable = row
                        .get("costs")
                        .and_then(Value::as_array)
                        .and_then(|costs| {
                            costs
                                .iter()
                                .map(|cost| {
                                    if cost.get("known").and_then(Value::as_bool) == Some(false) {
                                        return None;
                                    }
                                    let need =
                                        cost.get("count").and_then(Value::as_i64).unwrap_or(0);
                                    let owned = cost
                                        .get("owned")
                                        .and_then(Value::as_str)
                                        .and_then(|text| text.parse::<i64>().ok())
                                        .unwrap_or(0);
                                    if need <= 0 {
                                        None
                                    } else {
                                        Some(owned / need)
                                    }
                                })
                                .collect::<Option<Vec<_>>>()
                                .and_then(|rows| rows.into_iter().min())
                        })
                        .unwrap_or(0);
                    let max_count =
                        remaining.map(|limit| limit.min(affordable)).unwrap_or(affordable).max(0);
                    Some((id, format!("{name} {costs}"), exchangeable, max_count))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(PetBoard {
        title: value.get("title").and_then(Value::as_str).unwrap_or("萌宠成长日记").to_owned(),
        end_time: value.get("endTimeText").and_then(Value::as_str).unwrap_or("未知").to_owned(),
        active: value.get("active").and_then(Value::as_bool) == Some(true),
        bag_known: value.get("bagKnown").and_then(Value::as_bool) != Some(false),
        rules_title: value.get("rulesTitle").and_then(Value::as_str).unwrap_or("").to_owned(),
        rules: text_lines(value.get("rules")),
        treasure_rules_title: value
            .get("treasureRulesTitle")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        treasure_rules: text_lines(value.get("treasureRules")),
        warnings: value
            .get("warnings")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        balances: value
            .get("balances")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        let name = row.get("name").and_then(Value::as_str).unwrap_or("物品");
                        if row.get("known").and_then(Value::as_bool) == Some(false) {
                            format!("{name} 余额未知")
                        } else {
                            format!(
                                "{name} {}",
                                row.get("count").and_then(Value::as_str).unwrap_or("0")
                            )
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        growth: format!(
            "{}/{} · 幸运星 {} · {}",
            nurture.get("growth").and_then(Value::as_i64).unwrap_or(0),
            nurture.get("adultGrowth").and_then(Value::as_i64).unwrap_or(0),
            value.pointer("/hunt/luckyStarTotal").and_then(Value::as_str).unwrap_or("0"),
            if value.get("skipBattle").and_then(Value::as_bool) == Some(true) {
                "已跳过战斗动画"
            } else {
                "播放战斗动画"
            }
        ),
        feed: format!(
            "{}/{}",
            nurture.get("feedCount").and_then(Value::as_i64).unwrap_or(0),
            nurture.get("feedLimit").and_then(Value::as_i64).unwrap_or(0)
        ),
        can_feed: nurture.get("canFeed").and_then(Value::as_bool) == Some(true),
        can_draw: value.pointer("/hunt/canDraw").and_then(Value::as_bool) == Some(true),
        can_initialize: nurture.get("initialized").and_then(Value::as_bool) != Some(true),
        can_dog: nurture.get("adult").and_then(Value::as_bool) == Some(true)
            && nurture.get("dogGranted").and_then(Value::as_bool) != Some(true),
        can_seeds: value.pointer("/seeds/canClaim").and_then(Value::as_bool) == Some(true),
        stories,
        charms: value
            .pointer("/charms/refreshNote")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        charm_lines: value
            .pointer("/charms/pool")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        format!(
                            "{} {} 剩余 {:?}",
                            row.get("name").and_then(Value::as_str).unwrap_or("锦囊"),
                            row.get("shortDescription").and_then(Value::as_str).unwrap_or(""),
                            row.get("remaining")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        plants: value
            .get("plants")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        format!(
                            "{} {}",
                            row.get("name").and_then(Value::as_str).unwrap_or("作物"),
                            if row.get("known").and_then(Value::as_bool) == Some(false) {
                                "未知".to_owned()
                            } else {
                                row.get("count").and_then(Value::as_str).unwrap_or("0").to_owned()
                            }
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        can_refresh_free: value.pointer("/charms/canRefreshFree").and_then(Value::as_bool)
            == Some(true),
        can_refresh_paid: value.pointer("/charms/canRefreshPaid").and_then(Value::as_bool)
            == Some(true),
        paid_count: value.pointer("/charms/paidRefreshCount").and_then(Value::as_i64).unwrap_or(0),
        charm_choices: value
            .pointer("/charms/poolIds")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_i64()).collect())
            .unwrap_or_default(),
        treasures: value
            .get("treasures")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        let previews = row
                            .get("previews")
                            .and_then(Value::as_array)
                            .map(|rows| {
                                rows.iter()
                                    .map(|preview| {
                                        format!(
                                            "{}{}",
                                            preview
                                                .get("challengeName")
                                                .and_then(Value::as_str)
                                                .unwrap_or(""),
                                            if preview.get("canStart").and_then(Value::as_bool)
                                                == Some(true)
                                            {
                                                "可挑战"
                                            } else {
                                                "不可挑战"
                                            }
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .unwrap_or_default();
                        format!(
                            "{} {} ×{} {}–{} 被夺 {}/{} {} {}",
                            row.get("id").and_then(Value::as_str).unwrap_or(""),
                            row.pointer("/item/name").and_then(Value::as_str).unwrap_or(""),
                            row.get("count").and_then(Value::as_i64).unwrap_or(0),
                            row.get("startTimeText").and_then(Value::as_str).unwrap_or(""),
                            row.get("endTimeText").and_then(Value::as_str).unwrap_or(""),
                            row.get("plunderCount").and_then(Value::as_i64).unwrap_or(0),
                            row.get("maxPlunderCount").and_then(Value::as_i64).unwrap_or(0),
                            row.get("status").and_then(Value::as_str).unwrap_or(""),
                            previews
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        can_open: value.get("treasures").and_then(Value::as_array).is_some_and(|rows| {
            rows.iter().any(|row| row.get("status").and_then(Value::as_str) == Some("待领取"))
        }),
        compensation: value
            .get("compensationCount")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_owned(),
        shop,
        solar: value
            .get("solarTerms")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        let id = row.get("id").and_then(Value::as_str)?.parse().ok()?;
                        let name = row.get("name").and_then(Value::as_str).unwrap_or("节令");
                        let can = row.get("canClaim").and_then(Value::as_bool) == Some(true);
                        Some((
                            id,
                            format!(
                                "{name} {}",
                                row.get("endTimeText").and_then(Value::as_str).unwrap_or("")
                            ),
                            can,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub(crate) fn press(
    session: &mut PetSession,
    intent: PetIntent,
    running: bool,
) -> Option<PetCommand> {
    if !running || session.account_id.trim().is_empty() || session.busy {
        return None;
    }
    if session.needs_refresh && intent != PetIntent::Reload {
        return None;
    }
    let board = session.board.as_ref();
    let allowed = match &intent {
        PetIntent::Reload => true,
        PetIntent::Feed => board.is_some_and(|board| board.can_feed),
        PetIntent::Draw => board.is_some_and(|board| board.can_draw),
        PetIntent::Initialize => board.is_some_and(|board| board.can_initialize),
        PetIntent::ClaimDog => board.is_some_and(|board| board.can_dog),
        PetIntent::Seeds => board.is_some_and(|board| board.can_seeds),
        PetIntent::Story(order) => board
            .is_some_and(|board| board.stories.iter().any(|story| story.0 == *order && story.2)),
        PetIntent::RefreshFree => board.is_some_and(|board| board.can_refresh_free),
        PetIntent::RefreshPaid(_) => board.is_some_and(|board| board.can_refresh_paid),
        PetIntent::Equip(id) => board.is_some_and(|board| board.charm_choices.contains(id)),
        PetIntent::OpenTreasure => board.is_some_and(|board| board.can_open),
        PetIntent::Compensation => board.is_some_and(|board| board.compensation != "0"),
        PetIntent::Exchange { goods_id, count } => {
            *count > 0
                && board.is_some_and(|board| {
                    board
                        .shop
                        .iter()
                        .any(|goods| goods.0 == *goods_id && goods.2 && *count <= goods.3)
                })
        }
        PetIntent::Solar(id) => {
            board.is_some_and(|board| board.solar.iter().any(|term| term.0 == *id && term.2))
        }
        PetIntent::Friend(gid) => *gid > 0,
        PetIntent::InteractLog | PetIntent::PlunderLog => board.is_some(),
        PetIntent::Battle { gid, treasure_id, challenge_id } => {
            *gid > 0
                && !treasure_id.is_empty()
                && [80101, 80102, 80103].contains(challenge_id)
                && board.is_some_and(|board| board.bag_known)
        }
    };
    if !allowed {
        return None;
    }
    session.busy = true;
    session.attempted = true;
    session.generation += 1;
    session.failure = None;
    Some(PetCommand {
        account_id: session.account_id.clone(),
        generation: session.generation,
        intent,
    })
}

pub(crate) fn reset_account(session: &mut PetSession, account_id: String) {
    if session.account_id == account_id {
        return;
    }
    *session = PetSession {
        account_id,
        generation: session.generation + 1,
        exchange_count: 1,
        ..PetSession::default()
    };
}

pub(crate) fn apply(
    session: &mut PetSession,
    command: &PetCommand,
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
            } else if command.intent == PetIntent::Reload {
                session.board = board_from(&value);
            }
            if refresh {
                session.needs_refresh = true;
                if let Some(board) = session.board.as_mut() {
                    board.can_feed = false;
                    board.can_draw = false;
                    board.can_initialize = false;
                    board.can_dog = false;
                    board.can_seeds = false;
                    board.can_open = false;
                    board.can_refresh_free = false;
                    board.can_refresh_paid = false;
                }
            } else {
                session.needs_refresh = false;
            }
            let mut note = value.get("message").and_then(Value::as_str).map(str::to_owned);
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
                    note = Some(match note {
                        Some(message) if !message.contains(&text) => {
                            format!("{message}，获得 {text}")
                        }
                        Some(message) => message,
                        None => format!("获得 {text}"),
                    });
                }
            }
            if refresh {
                let refresh_text = value
                    .get("refreshError")
                    .and_then(Value::as_str)
                    .unwrap_or("操作已成功，刷新失败");
                note = Some(match note {
                    Some(message) => {
                        format!("{message}。{refresh_text}。请刷新确认最新进度，刷新不会再次提交")
                    }
                    None => format!("{refresh_text}。请刷新确认最新进度，刷新不会再次提交"),
                });
            }
            session.note = note;
            if let Some(rows) = value.get("logs").and_then(Value::as_array) {
                session.logs = rows.iter().map(interact_line).collect();
            }
            if let Some(rows) = value.get("plunderLogs").and_then(Value::as_array) {
                session.logs = rows.iter().map(plunder_line).collect();
            }
            if let Some(friend) = value.get("friend") {
                session.friend_gid =
                    friend.get("gid").and_then(Value::as_str).and_then(|gid| gid.parse().ok());
                session.friend_treasures = friend
                    .get("treasures")
                    .and_then(Value::as_array)
                    .map(|rows| {
                        rows.iter()
                            .map(|row| {
                                row.get("id").and_then(Value::as_str).unwrap_or("").to_owned()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            session.failure = None;
        }
        Err(message) => {
            if command.intent != PetIntent::Reload {
                session.needs_refresh = true;
            }
            session.failure = Some(message);
        }
    }
    true
}

pub(crate) async fn dispatch_pet(
    context: &qq_farm_app::AppContext,
    command: &PetCommand,
) -> qq_farm_app::AppResult<Value> {
    let id = command.account_id.as_str();
    match command.intent {
        PetIntent::Reload => qq_farm_app::pet_diary::snapshot(context, id).await,
        PetIntent::Feed => qq_farm_app::pet_diary::feed(context, id).await,
        PetIntent::Draw => qq_farm_app::pet_diary::draw(context, id).await,
        PetIntent::Initialize => qq_farm_app::pet_diary::initialize(context, id).await,
        PetIntent::ClaimDog => qq_farm_app::pet_diary::claim_dog(context, id).await,
        PetIntent::Seeds => qq_farm_app::pet_diary::claim_seeds(context, id).await,
        PetIntent::Story(order) => qq_farm_app::pet_diary::claim_story(context, id, order).await,
        PetIntent::RefreshFree => {
            qq_farm_app::pet_diary::refresh_charm(context, id, "free", None).await
        }
        PetIntent::RefreshPaid(expected) => {
            qq_farm_app::pet_diary::refresh_charm(context, id, "tickets", Some(expected)).await
        }
        PetIntent::Equip(charm) => qq_farm_app::pet_diary::equip_charm(context, id, charm).await,
        PetIntent::OpenTreasure => qq_farm_app::pet_diary::open_treasure(context, id).await,
        PetIntent::Compensation => qq_farm_app::pet_diary::claim_compensation(context, id).await,
        PetIntent::Exchange { goods_id, count } => {
            qq_farm_app::pet_diary::exchange(context, id, goods_id, count).await
        }
        PetIntent::Solar(term) => qq_farm_app::pet_diary::claim_solar(context, id, term).await,
        PetIntent::Friend(gid) => qq_farm_app::pet_diary::friend(context, id, gid).await,
        PetIntent::InteractLog => qq_farm_app::pet_diary::interact_log(context, id).await,
        PetIntent::PlunderLog => qq_farm_app::pet_diary::plunder_log(context, id).await,
        PetIntent::Battle { gid, ref treasure_id, challenge_id } => {
            qq_farm_app::pet_diary::battle(context, id, gid, treasure_id, challenge_id).await
        }
    }
}

pub(crate) fn watch(account: impl Fn() -> Option<String> + 'static, session: Signal<PetSession>) {
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
        if let Some(command) = press(&mut owned, PetIntent::Reload, true) {
            session.set(owned);
            launch(session, command);
        }
    });
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PetFriend {
    pub gid: i64,
    pub name: String,
}

pub(crate) fn panel(
    session: Signal<PetSession>,
    page: ActivityPage,
    friends: Resource<Vec<PetFriend>, String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || text_key("activities.petDiary")).text_lg())
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
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!(
                "{} · 结束 {} · {}",
                board.title,
                board.end_time,
                if board.active { "进行中" } else { "未在活动时间" }
            ))
        }))
        .child(text(move || {
            session
                .get()
                .board
                .map(|board| {
                    if board.rules_title.is_empty() {
                        text_key("activities.petRules")
                    } else {
                        SharedString::from(board.rules_title)
                    }
                })
                .unwrap_or_default()
        }))
        .child(each(
            move || session.get().board.map(|board| board.rules).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(text(move || {
            session
                .get()
                .board
                .map(|board| SharedString::from(board.treasure_rules_title))
                .unwrap_or_default()
        }))
        .child(each(
            move || session.get().board.map(|board| board.treasure_rules).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(each(
            move || {
                [
                    "activities.petSectionNurture",
                    "activities.petSectionTreasure",
                    "activities.petSectionStory",
                    "activities.petSectionShop",
                    "activities.petSectionSolar",
                    "activities.petSectionLog",
                    "activities.petSectionBattle",
                ]
            },
            |name: &&str| *name,
            move |name, _| {
                let name = name.get_untracked();
                button(move || text_key(name), ButtonVariant::Quiet)
                    .selected(move || session.get().section == name)
                    .on_press(move || choose(session, name))
            },
        ))
        .child(each(
            move || session.get().board.map(|board| board.balances).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(text(move || {
            let Some(board) = session.get().board else {
                return SharedString::from("");
            };
            SharedString::from(format!("成长 {} · 今日投喂 {}", board.growth, board.feed))
        }))
        .child(
            button(move || text_key("activities.petFeed"), ButtonVariant::Primary)
                .disabled(move || !enabled(session, PetIntent::Feed))
                .on_press(move || fire(session, page, PetIntent::Feed)),
        )
        .child(
            button(move || text_key("activities.petDraw"), ButtonVariant::Primary)
                .disabled(move || !enabled(session, PetIntent::Draw))
                .on_press(move || fire(session, page, PetIntent::Draw)),
        )
        .child(
            button(move || text_key("activities.petInit"), ButtonVariant::Secondary)
                .disabled(move || !enabled(session, PetIntent::Initialize))
                .on_press(move || fire(session, page, PetIntent::Initialize)),
        )
        .child(
            button(move || text_key("activities.petDog"), ButtonVariant::Secondary)
                .disabled(move || !enabled(session, PetIntent::ClaimDog))
                .on_press(move || fire(session, page, PetIntent::ClaimDog)),
        )
        .child(
            button(move || text_key("activities.petSeeds"), ButtonVariant::Secondary)
                .disabled(move || !enabled(session, PetIntent::Seeds))
                .on_press(move || fire(session, page, PetIntent::Seeds)),
        )
        .child(text(move || {
            SharedString::from(session.get().board.map(|board| board.charms).unwrap_or_default())
        }))
        .child(each(
            move || session.get().board.map(|board| board.charm_choices).unwrap_or_default(),
            |id: &i64| *id,
            move |id, _| {
                let charm = id.get_untracked();
                button(
                    move || SharedString::from(format!("选择锦囊 {charm}")),
                    ButtonVariant::Secondary,
                )
                .disabled(move || !enabled(session, PetIntent::Equip(charm)))
                .on_press(move || fire(session, page, PetIntent::Equip(charm)))
            },
        ))
        .child(
            button(move || text_key("activities.petRefreshFree"), ButtonVariant::Secondary)
                .disabled(move || !enabled(session, PetIntent::RefreshFree))
                .on_press(move || fire(session, page, PetIntent::RefreshFree)),
        )
        .child(
            button(move || text_key("activities.petRefreshPaid"), ButtonVariant::Secondary)
                .disabled(move || {
                    let expected = session.get().board.map(|board| board.paid_count).unwrap_or(0);
                    !enabled(session, PetIntent::RefreshPaid(expected))
                })
                .on_press(move || {
                    let expected =
                        session.get_untracked().board.map(|board| board.paid_count).unwrap_or(0);
                    fire(session, page, PetIntent::RefreshPaid(expected))
                }),
        )
        .child(each(
            move || session.get().board.map(|board| board.stories).unwrap_or_default(),
            |row: &(i64, String, bool)| row.0,
            move |row, _| {
                let row = row.get_untracked();
                let order = row.0;
                view().gap_1().child(text(row.1.clone())).child(
                    button(move || text_key("activities.petStory"), ButtonVariant::Secondary)
                        .disabled(move || !enabled(session, PetIntent::Story(order)))
                        .on_press(move || fire(session, page, PetIntent::Story(order))),
                )
            },
        ))
        .child(each(
            move || session.get().board.map(|board| board.treasures).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(
            button(move || text_key("activities.petOpen"), ButtonVariant::Secondary)
                .disabled(move || !enabled(session, PetIntent::OpenTreasure))
                .on_press(move || fire(session, page, PetIntent::OpenTreasure)),
        )
        .child(text(move || {
            SharedString::from(format!(
                "夺宝补偿 {}",
                session.get().board.map(|board| board.compensation).unwrap_or_else(|| "0".into())
            ))
        }))
        .child(
            button(move || text_key("activities.petCompensation"), ButtonVariant::Secondary)
                .disabled(move || !enabled(session, PetIntent::Compensation))
                .on_press(move || fire(session, page, PetIntent::Compensation)),
        )
        .child(each(
            move || session.get().board.map(|board| board.shop).unwrap_or_default(),
            |row: &(i64, String, bool, i64)| row.0,
            move |row, _| {
                let row = row.get_untracked();
                let id = row.0;
                view()
                    .gap_1()
                    .child(text(row.1.clone()))
                    .child(
                        button(move || SharedString::from("+"), ButtonVariant::Quiet)
                            .on_press(move || adjust_count(session, 1)),
                    )
                    .child(text(move || {
                        SharedString::from(session.get().exchange_count.max(1).to_string())
                    }))
                    .child(
                        button(move || SharedString::from("-"), ButtonVariant::Quiet)
                            .on_press(move || adjust_count(session, -1)),
                    )
                    .child(when(
                        move || session.get().confirming_goods == Some(id),
                        move || {
                            view()
                                .gap_1()
                                .child(
                                    button(
                                        move || text_key("activities.charityDonateCancel"),
                                        ButtonVariant::Secondary,
                                    )
                                    .on_press(move || cancel_exchange(session)),
                                )
                                .child(
                                    button(
                                        move || text_key("activities.petExchange"),
                                        ButtonVariant::Primary,
                                    )
                                    .disabled(move || {
                                        !enabled(
                                            session,
                                            PetIntent::Exchange {
                                                goods_id: id,
                                                count: session.get().exchange_count.max(1),
                                            },
                                        )
                                    })
                                    .on_press(move || {
                                        fire(
                                            session,
                                            page,
                                            PetIntent::Exchange {
                                                goods_id: id,
                                                count: session
                                                    .get_untracked()
                                                    .exchange_count
                                                    .max(1),
                                            },
                                        )
                                    }),
                                )
                        },
                    ))
                    .child(when(
                        move || session.get().confirming_goods != Some(id),
                        move || {
                            button(
                                move || text_key("activities.petExchange"),
                                ButtonVariant::Secondary,
                            )
                            .on_press(move || prepare_exchange(session, id))
                        },
                    ))
            },
        ))
        .child(each(
            move || session.get().board.map(|board| board.solar).unwrap_or_default(),
            |row: &(i64, String, bool)| row.0,
            move |row, _| {
                let row = row.get_untracked();
                let id = row.0;
                view().gap_1().child(text(row.1.clone())).child(
                    button(move || text_key("activities.petSolar"), ButtonVariant::Secondary)
                        .disabled(move || !enabled(session, PetIntent::Solar(id)))
                        .on_press(move || fire(session, page, PetIntent::Solar(id))),
                )
            },
        ))
        .child(each(
            move || session.get().board.map(|board| board.warnings).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(each(
            move || session.get().board.map(|board| board.plants).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(each(
            move || session.get().board.map(|board| board.charm_lines).unwrap_or_default(),
            |line: &String| line.clone(),
            move |line, _| text(line.get_untracked().clone()).text_sm(),
        ))
        .child(when(
            move || session.get().section == "activities.petSectionLog",
            move || {
                view()
                    .gap_2()
                    .child(
                        button(
                            move || text_key("activities.petInteractLog"),
                            ButtonVariant::Secondary,
                        )
                        .on_press(move || fire(session, page, PetIntent::InteractLog)),
                    )
                    .child(
                        button(
                            move || text_key("activities.petPlunderLog"),
                            ButtonVariant::Secondary,
                        )
                        .on_press(move || fire(session, page, PetIntent::PlunderLog)),
                    )
                    .child(each(
                        move || session.get().logs,
                        |line: &String| line.clone(),
                        move |line, _| text(line.get_untracked().clone()).text_sm(),
                    ))
            },
        ))
        .child(when(
            move || session.get().section == "activities.petSectionBattle",
            move || {
                view()
                    .gap_2()
                    .child(each(
                        move || friends.get().unwrap_or_default(),
                        |friend: &PetFriend| friend.gid,
                        move |friend, _| {
                            let friend = friend.get_untracked();
                            let gid = friend.gid;
                            button(
                                move || SharedString::from(format!("{} {gid}", friend.name)),
                                ButtonVariant::Quiet,
                            )
                            .selected(move || session.get().friend_gid == Some(gid))
                            .on_press(move || fire(session, page, PetIntent::Friend(gid)))
                        },
                    ))
                    .child(each(
                        move || session.get().friend_treasures,
                        |id: &String| id.clone(),
                        move |id, _| {
                            let treasure = id.get_untracked();
                            let label = treasure.clone();
                            let picked = treasure;
                            button(move || SharedString::from(label.clone()), ButtonVariant::Quiet)
                                .on_press(move || pick_treasure(session, picked.clone()))
                        },
                    ))
                    .child(each(
                        move || [80101_i64, 80102, 80103],
                        |id: &i64| *id,
                        move |id, _| {
                            let id = id.get_untracked();
                            button(
                                move || {
                                    SharedString::from(format!(
                                        "{} {id}",
                                        text_key("activities.petChallenge")
                                    ))
                                },
                                ButtonVariant::Quiet,
                            )
                            .selected(move || session.get().selected_challenge == id)
                            .on_press(move || pick_challenge(session, id))
                        },
                    ))
                    .child(
                        button(move || text_key("activities.petBattle"), ButtonVariant::Primary)
                            .on_press(move || {
                                let current = session.get_untracked();
                                let Some(gid) = current.friend_gid else { return };
                                if current.selected_treasure.is_empty()
                                    || current.selected_challenge == 0
                                {
                                    return;
                                }
                                fire(
                                    session,
                                    page,
                                    PetIntent::Battle {
                                        gid,
                                        treasure_id: current.selected_treasure,
                                        challenge_id: current.selected_challenge,
                                    },
                                );
                            }),
                    )
            },
        ))
}

fn choose(session: Signal<PetSession>, name: &str) {
    let mut owned = session.get_untracked();
    owned.section = name.to_owned();
    session.set(owned);
}

fn shop_cap(session: &PetSession, goods_id: Option<i64>) -> i64 {
    let Some(board) = session.board.as_ref() else {
        return 0;
    };
    if let Some(id) = goods_id {
        return board.shop.iter().find(|goods| goods.0 == id).map(|goods| goods.3).unwrap_or(0);
    }
    board.shop.iter().filter(|goods| goods.2).map(|goods| goods.3).max().unwrap_or(0)
}

fn adjust_count(session: Signal<PetSession>, delta: i64) {
    let mut owned = session.get_untracked();
    let cap = shop_cap(
        &owned,
        owned.confirming_goods.or_else(|| {
            owned
                .board
                .as_ref()
                .and_then(|board| board.shop.iter().find(|goods| goods.2).map(|goods| goods.0))
        }),
    );
    owned.exchange_count =
        if cap <= 0 { 0 } else { (owned.exchange_count.max(1) + delta).clamp(1, cap) };
    session.set(owned);
}

fn prepare_exchange(session: Signal<PetSession>, goods_id: i64) {
    let mut owned = session.get_untracked();
    owned.confirming_goods = Some(goods_id);
    let cap = shop_cap(&owned, Some(goods_id));
    owned.exchange_count = if cap <= 0 { 0 } else { owned.exchange_count.max(1).clamp(1, cap) };
    session.set(owned);
}

fn pick_treasure(session: Signal<PetSession>, id: String) {
    let mut owned = session.get_untracked();
    owned.selected_treasure = id;
    session.set(owned);
}

fn pick_challenge(session: Signal<PetSession>, id: i64) {
    let mut owned = session.get_untracked();
    owned.selected_challenge = id;
    session.set(owned);
}

fn cancel_exchange(session: Signal<PetSession>) {
    let mut owned = session.get_untracked();
    owned.confirming_goods = None;
    session.set(owned);
}

fn interact_line(row: &Value) -> String {
    format!(
        "{} 类型 {} 消耗 {} 奖励 {}",
        row.get("timeText").and_then(Value::as_str).unwrap_or(""),
        row.get("type").and_then(Value::as_i64).unwrap_or(0),
        row.get("costs").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0),
        row.get("rewards").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0)
    )
}

fn plunder_line(row: &Value) -> String {
    format!(
        "{} {} {} 挑战书 {} 损失 {} 注入 {}",
        row.get("timeText").and_then(Value::as_str).unwrap_or(""),
        row.get("name").and_then(Value::as_str).unwrap_or(""),
        if row.get("won").and_then(Value::as_bool) == Some(true) {
            "对方获胜"
        } else {
            "对方未获胜"
        },
        row.get("challengeName").and_then(Value::as_str).unwrap_or(""),
        row.get("lost").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0),
        row.get("injected").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0)
    )
}

fn enabled(session: Signal<PetSession>, intent: PetIntent) -> bool {
    let mut owned = session.get();
    press(&mut owned, intent, true).is_some()
}

fn fire(session: Signal<PetSession>, page: ActivityPage, intent: PetIntent) {
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

fn launch(session: Signal<PetSession>, command: PetCommand) {
    spawn(async move {
        let context = session::session().context.clone();
        let result = {
            let context = context.clone();
            let command = command.clone();
            session::call(async move { dispatch_pet(&context, &command).await }).await
        };
        let mut owned = session.get_untracked();
        let _accepted = apply(&mut owned, &command, result);
        session.set(owned);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn loaded() -> PetSession {
        let board = board_from(&json!({
            "available": true,
            "title": "萌宠成长日记",
            "endTimeText": "2026-09-01 08:00",
            "active": true,
            "rulesTitle": "说明",
            "rules": ["投喂"],
            "balances": [{ "name": "元气糕", "count": "800", "known": true }],
            "nurture": { "growth": 10, "adultGrowth": 7000, "feedCount": 0, "feedLimit": 16, "canFeed": true, "initialized": false, "adult": false, "dogGranted": false },
            "hunt": { "canDraw": false },
            "seeds": { "canClaim": true },
            "stories": [{ "order": 2, "unlocked": true, "claimed": false }],
            "charms": { "refreshNote": "每日免费 1 次", "canRefreshFree": false, "pool": [] },
            "treasures": [{ "id": "t1", "status": "待领取" }],
            "compensationCount": "1",
            "shop": [{ "id": "8", "name": "种子", "exchangeable": true, "remaining": "5", "costs": [{ "name": "元气糕", "count": 2, "known": true, "owned": "800" }] }],
            "solarTerms": [{ "id": "3", "name": "白露", "canClaim": true, "endTimeText": "2026-09-08 08:00" }]
        })).unwrap();
        PetSession {
            account_id: "acc".into(),
            board: Some(board),
            exchange_count: 1,
            ..PetSession::default()
        }
    }

    #[test]
    fn the_board_lists_the_sections_and_a_late_reply_keeps_the_newer_command() {
        let board = loaded().board.unwrap();
        assert_eq!(board.end_time, "2026-09-01 08:00");
        assert_eq!(board.rules, vec!["投喂".to_string()]);
        assert_eq!(board.balances[0], "元气糕 800");
        assert!(board.can_feed && board.can_seeds && board.can_open);
        let mut session = loaded();
        assert_eq!(session.generation, 0);
        let first = press(&mut session, PetIntent::Feed, true).unwrap();
        assert_eq!(first.generation, 1);
        assert!(apply(
            &mut session,
            &first,
            Ok(json!({
                "message": "操作成功",
                "rewards": [{ "name": "元气糕", "count": 1 }],
                "refreshRequired": true,
                "refreshError": "操作已成功，刷新失败"
            }))
        ));
        let note = session.note.clone().unwrap_or_default();
        assert!(
            note.contains("操作成功") && note.contains("元气糕×1") && note.contains("刷新失败"),
            "{note}"
        );
        assert!(session.needs_refresh);
        assert!(press(&mut session, PetIntent::Feed, true).is_none());
        let reload = press(&mut session, PetIntent::Reload, true).unwrap();
        assert_ne!(reload.generation, first.generation);
        assert!(!apply(&mut session, &first, Ok(json!({"message": "旧回包"}))));
        assert!(session.busy);
        assert!(!session.note.as_deref().unwrap_or("").contains("旧回包"));
    }

    #[test]
    fn quantity_cancel_friend_and_logs_do_not_send_until_the_matching_press() {
        let mut session = loaded();
        session.section = "activities.petSectionBattle".into();
        session.exchange_count = 1;
        session.confirming_goods = Some(8);
        session.confirming_goods = None;
        session.exchange_count = 3;
        let quantity = session.exchange_count;
        assert_eq!(session.generation, 0);
        assert!(press(
            &mut session,
            PetIntent::Battle { gid: 0, treasure_id: "box".into(), challenge_id: 80101 },
            true
        )
        .is_none());
        assert!(press(
            &mut session,
            PetIntent::Battle { gid: 9, treasure_id: String::new(), challenge_id: 80101 },
            true
        )
        .is_none());
        assert!(press(
            &mut session,
            PetIntent::Battle { gid: 9, treasure_id: "box".into(), challenge_id: 80000 },
            true
        )
        .is_none());
        assert_eq!(session.generation, 0);
        let exchange =
            press(&mut session, PetIntent::Exchange { goods_id: 8, count: quantity }, true)
                .unwrap();
        assert_eq!(exchange.intent, PetIntent::Exchange { goods_id: 8, count: 3 });
        assert!(apply(&mut session, &exchange, Ok(json!({ "message": "操作成功" }))));
        let friend = press(&mut session, PetIntent::Friend(9), true).unwrap();
        assert!(apply(
            &mut session,
            &friend,
            Ok(json!({
                "message": "操作成功",
                "friend": { "gid": "9", "treasures": [{ "id": "box" }] }
            }))
        ));
        assert_eq!(session.friend_gid, Some(9));
        assert_eq!(session.friend_treasures, vec!["box".to_string()]);
        let logs = press(&mut session, PetIntent::InteractLog, true).unwrap();
        assert!(apply(
            &mut session,
            &logs,
            Ok(json!({
                "logs": [{ "timeText": "08:00", "type": 3, "costs": [{ "count": 1 }], "rewards": [] }]
            }))
        ));
        assert!(session.logs[0].contains("08:00"));
        let battle = press(
            &mut session,
            PetIntent::Battle { gid: 9, treasure_id: "box".into(), challenge_id: 80102 },
            true,
        )
        .unwrap();
        assert_eq!(
            battle.intent,
            PetIntent::Battle { gid: 9, treasure_id: "box".into(), challenge_id: 80102 }
        );
    }

    #[tokio::test]
    async fn dispatch_sends_the_pressed_feed_quantity_friend_battle_and_logs() {
        use std::sync::{Arc, Mutex};

        use prost::Message as _;
        use qq_farm_core::models::types::AccountConfigSnapshot;
        use qq_farm_core::proto::generated::corepb::{Item as CoreItem, ItemBag};
        use qq_farm_core::proto::generated::gamepb::activitypb::{
            ActivityBodyPetTreasureHunt, PetDiaryActivityBodyShop, PetDiaryActivityData,
            PetDiaryActivityGroup, PetDiaryActivityHead, PetDiaryGetGroupReply,
            PetDiaryOperateReply, PetDiaryOperateRequest, PetDiaryShopBuyRsp,
            PetDiaryShopGoodsInfo, PetDiaryShopItemInfo, PetTreasureHuntBattlePreview,
            PetTreasureHuntFeed, PetTreasureHuntFeedRsp, PetTreasureHuntFriendInfo,
            PetTreasureHuntGetFriendActivityInfoRsp, PetTreasureHuntGetLogRsp,
            PetTreasureHuntGetPlunderedLogRsp, PetTreasureHuntHunt, PetTreasureHuntLogEntry,
            PetTreasureHuntNurture, PetTreasureHuntPlunderedLogEntry,
            PetTreasureHuntStartBattleRsp, PetTreasureHuntTreasure,
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
            if method == "Bag" {
                return Ok(BagReply {
                    item_bag: Some(ItemBag {
                        items: vec![
                            CoreItem { id: 1028, count: 800, ..CoreItem::default() },
                            CoreItem { id: 80101, count: 1, ..CoreItem::default() },
                        ],
                        ..ItemBag::default()
                    }),
                }
                .encode_to_vec());
            }
            if method == "GetSolarTerms" {
                return Ok(qq_farm_core::proto::generated::gamepb::solartermspb::GetSolarTermsReply::default().encode_to_vec());
            }
            if method == "Operate" {
                log.lock().expect("log").push(body.to_vec());
                let request = PetDiaryOperateRequest::decode(body).unwrap();
                let mut reply = PetDiaryOperateReply {
                    activity_id: request.activity_id,
                    operate_type: request.operate_type,
                    ..PetDiaryOperateReply::default()
                };
                match request.operate_type {
                    29 => reply.pet_treasure_hunt_feed = Some(PetTreasureHuntFeedRsp::default()),
                    7 => {
                        reply.data = Some(PetDiaryActivityData {
                            head: Some(PetDiaryActivityHead {
                                id: 2_026_090_103,
                                start_time: 1,
                                end_time: qq_farm_core::utils::time::get_server_time_secs() + 100,
                                ..PetDiaryActivityHead::default()
                            }),
                            shop: Some(PetDiaryActivityBodyShop {
                                goods: vec![PetDiaryShopGoodsInfo {
                                    id: 8,
                                    cost: vec![PetDiaryShopItemInfo { id: 1028, count: 1 }],
                                    purchase_limit: 5,
                                    name: "种子".into(),
                                    ..PetDiaryShopGoodsInfo::default()
                                }],
                            }),
                            ..PetDiaryActivityData::default()
                        })
                    }
                    47 => {
                        let gid = request
                            .pet_treasure_hunt_get_friend_activity_info
                            .as_ref()
                            .map(|item| item.friend_gid)
                            .unwrap_or(0);
                        reply.pet_treasure_hunt_get_friend_activity_info =
                            Some(PetTreasureHuntGetFriendActivityInfoRsp {
                                gid,
                                info: Some(PetTreasureHuntFriendInfo {
                                    treasures: vec![PetTreasureHuntTreasure {
                                        id: "box".into(),
                                        status: 2,
                                        battle_previews: vec![PetTreasureHuntBattlePreview {
                                            challenge_item_id: 80101,
                                            can_start: true,
                                            ..PetTreasureHuntBattlePreview::default()
                                        }],
                                        ..PetTreasureHuntTreasure::default()
                                    }],
                                    ..PetTreasureHuntFriendInfo::default()
                                }),
                            })
                    }
                    43 => {
                        reply.pet_treasure_hunt_start_battle =
                            Some(PetTreasureHuntStartBattleRsp { won: true, ..Default::default() })
                    }
                    1 => reply.shop_buy = Some(PetDiaryShopBuyRsp::default()),
                    31 => {
                        reply.pet_treasure_hunt_get_log = Some(PetTreasureHuntGetLogRsp {
                            logs: vec![PetTreasureHuntLogEntry {
                                ts: 10,
                                r#type: 3,
                                ..PetTreasureHuntLogEntry::default()
                            }],
                        })
                    }
                    44 => {
                        reply.pet_treasure_hunt_get_plundered_log =
                            Some(PetTreasureHuntGetPlunderedLogRsp {
                                logs: vec![PetTreasureHuntPlunderedLogEntry {
                                    ts: 11,
                                    attacker_won: false,
                                    challenge_item_id: 80103,
                                    ..PetTreasureHuntPlunderedLogEntry::default()
                                }],
                            })
                    }
                    _ => {}
                }
                return Ok(reply.encode_to_vec());
            }
            let end = qq_farm_core::utils::time::get_server_time_secs() + 3600;
            Ok(PetDiaryGetGroupReply {
                group: Some(PetDiaryActivityGroup {
                    head: Some(PetDiaryActivityHead {
                        id: 2_026_090_100,
                        start_time: 1,
                        end_time: end,
                        ..PetDiaryActivityHead::default()
                    }),
                    children: vec![PetDiaryActivityData {
                        head: Some(PetDiaryActivityHead {
                            id: 2_026_090_101,
                            start_time: 1,
                            end_time: end,
                            ..PetDiaryActivityHead::default()
                        }),
                        pet_treasure_hunt: Some(ActivityBodyPetTreasureHunt {
                            nurture: Some(PetTreasureHuntNurture { stage: 1, ..Default::default() }),
                            feed: Some(PetTreasureHuntFeed { feed_count: 0 }),
                            hunt: Some(PetTreasureHuntHunt {
                                can_play_plunder: true,
                                ..PetTreasureHuntHunt::default()
                            }),
                            ..ActivityBodyPetTreasureHunt::default()
                        }),
                        ..PetDiaryActivityData::default()
                    }],
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
        let feed = press(&mut session, PetIntent::Feed, true).unwrap();
        dispatch_pet(&context, &feed).await.unwrap();
        session.busy = false;
        session.exchange_count = 3;
        let exchange =
            press(&mut session, PetIntent::Exchange { goods_id: 8, count: 3 }, true).unwrap();
        dispatch_pet(&context, &exchange).await.unwrap();
        session.busy = false;
        let friend = press(&mut session, PetIntent::Friend(9), true).unwrap();
        let friend_result = dispatch_pet(&context, &friend).await.unwrap();
        assert_eq!(friend_result["friend"]["gid"], "9");
        session.busy = false;
        let battle = press(
            &mut session,
            PetIntent::Battle { gid: 9, treasure_id: "box".into(), challenge_id: 80101 },
            true,
        )
        .unwrap();
        let battle_result = dispatch_pet(&context, &battle).await.unwrap();
        assert_eq!(battle_result["message"], "夺宝成功");
        session.busy = false;
        let interact = press(&mut session, PetIntent::InteractLog, true).unwrap();
        let interact_result = dispatch_pet(&context, &interact).await.unwrap();
        assert_eq!(interact_result["logs"][0]["type"], 3);
        session.busy = false;
        let plunder = press(&mut session, PetIntent::PlunderLog, true).unwrap();
        let plunder_result = dispatch_pet(&context, &plunder).await.unwrap();
        assert_eq!(plunder_result["plunderLogs"][0]["challengeId"], "80103");
        let requests = seen
            .lock()
            .expect("seen")
            .iter()
            .map(|body| PetDiaryOperateRequest::decode(body.as_slice()).unwrap())
            .collect::<Vec<_>>();
        let feed_request = requests.iter().find(|item| item.operate_type == 29).unwrap();
        assert!(feed_request.pet_treasure_hunt_feed.is_some());
        let buy = requests.iter().find(|item| item.operate_type == 1).unwrap();
        assert_eq!(buy.shop_buy.as_ref().unwrap().count, 3);
        let friend_request = requests.iter().find(|item| item.operate_type == 47).unwrap();
        assert_eq!(
            friend_request.pet_treasure_hunt_get_friend_activity_info.as_ref().unwrap().friend_gid,
            9
        );
        let battle_request = requests.iter().find(|item| item.operate_type == 43).unwrap();
        let body = battle_request.pet_treasure_hunt_start_battle.as_ref().unwrap();
        assert_eq!(
            (body.defender_gid, body.treasure_id.as_str(), body.challenge_item_id),
            (9, "box", 80101)
        );
        assert!(requests
            .iter()
            .any(|item| item.operate_type == 31 && item.pet_treasure_hunt_get_log.is_some()));
        assert!(requests
            .iter()
            .any(|item| item.operate_type == 44
                && item.pet_treasure_hunt_get_plundered_log.is_some()));
    }
}
