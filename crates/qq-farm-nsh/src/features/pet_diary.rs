use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, PartialEq, Eq)]
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
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PetBoard {
    title: String,
    end_time: String,
    active: bool,
    rules_title: String,
    rules: Vec<String>,
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
    can_refresh_free: bool,
    can_refresh_paid: bool,
    paid_count: i64,
    charm_choices: Vec<i64>,
    treasures: Vec<String>,
    can_open: bool,
    compensation: String,
    shop: Vec<(i64, String, bool)>,
    solar: Vec<(i64, String, bool)>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PetCommand {
    pub account_id: String,
    pub generation: u64,
    pub intent: PetIntent,
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
                            "手记 {order} {}",
                            if claimed {
                                "已领取"
                            } else if unlocked {
                                "可领取"
                            } else {
                                "未解锁"
                            }
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
                    Some((id, format!("{name} {costs}"), exchangeable))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(PetBoard {
        title: value.get("title").and_then(Value::as_str).unwrap_or("萌宠成长日记").to_owned(),
        end_time: value.get("endTimeText").and_then(Value::as_str).unwrap_or("未知").to_owned(),
        active: value.get("active").and_then(Value::as_bool) == Some(true),
        rules_title: value.get("rulesTitle").and_then(Value::as_str).unwrap_or("").to_owned(),
        rules: value
            .get("rules")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
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
            "{}/{}",
            nurture.get("growth").and_then(Value::as_i64).unwrap_or(0),
            nurture.get("adultGrowth").and_then(Value::as_i64).unwrap_or(0)
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
        can_refresh_free: value.pointer("/charms/canRefreshFree").and_then(Value::as_bool)
            == Some(true),
        can_refresh_paid: value.pointer("/charms/canRefreshPaid").and_then(Value::as_bool)
            == Some(true),
        paid_count: value.pointer("/charms/paidRefreshCount").and_then(Value::as_i64).unwrap_or(0),
        charm_choices: value
            .pointer("/charms/pool")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|row| row.as_i64()).collect())
            .unwrap_or_default(),
        treasures: value
            .get("treasures")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        format!(
                            "{} {}",
                            row.get("id").and_then(Value::as_str).unwrap_or(""),
                            row.get("status").and_then(Value::as_str).unwrap_or("")
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
                    board.shop.iter().any(|goods| goods.0 == *goods_id && goods.2)
                })
        }
        PetIntent::Solar(id) => {
            board.is_some_and(|board| board.solar.iter().any(|term| term.0 == *id && term.2))
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

pub(crate) fn panel(session: Signal<PetSession>, page: ActivityPage) -> impl IntoElement {
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
            |row: &(i64, String, bool)| row.0,
            move |row, _| {
                let row = row.get_untracked();
                let id = row.0;
                view().gap_1().child(text(row.1.clone())).child(
                    button(move || text_key("activities.petExchange"), ButtonVariant::Primary)
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
                                    count: session.get_untracked().exchange_count.max(1),
                                },
                            )
                        }),
                )
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
            "shop": [{ "id": "8", "name": "种子", "exchangeable": true, "costs": [{ "name": "元气糕", "count": 2, "known": true }] }],
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
}
