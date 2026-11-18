use std::time::Duration;

use nsh_core::prelude::*;
use serde_json::{json, Value};

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

const SHARE_ACTIVITIES: [&str; 2] = ["2026092501", "2026110104"];
const WEATHER_VISIT_WATCH: Duration = Duration::from_millis(400);

#[derive(Clone, Copy)]
pub(crate) struct ActivityPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shelf {
    Solar,
    Share,
    Tea,
    Muge,
    Ranch,
    Weather,
}

impl Shelf {
    const ALL: [Self; 6] =
        [Self::Solar, Self::Share, Self::Tea, Self::Muge, Self::Ranch, Self::Weather];

    const fn id(self) -> &'static str {
        match self {
            Self::Solar => "solar",
            Self::Share => "share",
            Self::Tea => "tea",
            Self::Muge => "muge",
            Self::Ranch => "ranch",
            Self::Weather => "weather",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Solar => "activities.solar",
            Self::Share => "activities.share",
            Self::Tea => "activities.tea",
            Self::Muge => "activities.muge",
            Self::Ranch => "activities.ranch",
            Self::Weather => "activities.weather",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct SolarEvent {
    id: i64,
    name: String,
    description: String,
    can_claim: bool,
    question: String,
    option_index: Option<i64>,
}

#[derive(Clone, PartialEq, Eq)]
struct SeasonLine {
    id: i64,
    name: String,
    progress: String,
    can_claim: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct SolarBoard {
    events: Vec<SolarEvent>,
    seasons: Vec<SeasonLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct ShareLine {
    id: String,
    title: String,
    score: i64,
    daily: String,
    daily_reward: i64,
    first_share_reward: i64,
    pool: String,
    can_daily: bool,
    can_share: bool,
    can_milestones: bool,
    milestones: Vec<ShareTier>,
}

#[derive(Clone, PartialEq, Eq)]
struct ShareTier {
    id: String,
    threshold: i64,
    state_key: &'static str,
    rewards: String,
}

#[derive(Clone, PartialEq, Eq)]
struct ShareLog {
    key: String,
    kind: i64,
    score: i64,
    when: String,
    actor: String,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaShop {
    name: String,
    level: i64,
    revenue: i64,
    settling: bool,
    can_claim: bool,
    can_upgrade: bool,
    sales_item: i64,
    sales_stock: i64,
    boost_cups: i64,
    opened: bool,
    suggestions: Vec<String>,
    goods: Vec<TeaGood>,
    recipes: Vec<TeaRecipe>,
    fruits: Vec<TeaFruit>,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaRecipe {
    id: i64,
    name: String,
    stock: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaFruit {
    uid: String,
    name: String,
    count: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaHireBoard {
    wage: i64,
    settling: bool,
    durations: Vec<TeaDuration>,
    candidates: Vec<TeaCandidate>,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaDuration {
    id: i64,
    hours: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaCandidate {
    gid: String,
    name: String,
    talent: String,
    available: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaFund {
    unlocked: bool,
    costs: Vec<TeaCost>,
    tiers: Vec<TeaFundTier>,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaCost {
    id: i64,
    count: i64,
    name: String,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaFundTier {
    id: i64,
    level: i64,
    state: i64,
    rewards: String,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaWork {
    done: bool,
    now: i64,
    shift: Option<TeaShift>,
    slips: Vec<TeaSlip>,
    actions: Vec<TeaInteract>,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaShift {
    id: String,
    employer: String,
    shop: String,
    end_at: i64,
    used: i64,
    limit: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaSlip {
    id: String,
    employer: String,
    wage: i64,
    claimable: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaInteract {
    id: i64,
    name: String,
    description: String,
    ready: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaSettings {
    name: String,
    rename_at: i64,
    now: i64,
    hire_disabled: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaSales {
    day: i64,
    today: i64,
    token: i64,
    coin: i64,
    cups: i64,
    hires: Vec<TeaHireLog>,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaHireLog {
    id: String,
    name: String,
    talent: String,
    wage: i64,
    when: String,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaRank {
    page: i64,
    mine: i64,
    has_more: bool,
    rows: Vec<TeaRankRow>,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaRankRow {
    rank: i64,
    shop: String,
    owner: String,
    revenue: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaGood {
    id: String,
    name: String,
    price: String,
    limit_key: Option<&'static str>,
    remaining: Option<i64>,
    limit_max: i64,
    available: bool,
    costs: Vec<(i64, i64)>,
    max_count: i64,
}

pub(crate) fn page(page: ActivityPage) -> impl IntoElement {
    let shelf = signal(Shelf::Solar);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let solar = resource(
        move || (shelf.get() == Shelf::Solar).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            let value =
                session::call(
                    async move { qq_farm_app::solar_calendar::snapshot(&context, &id).await },
                )
                .await?;
            Ok(solar_board(&value))
        },
    );
    let shares = resource(
        move || (shelf.get() == Shelf::Share).then(|| running_account(page)).flatten(),
        |id, _| async move { load_shares(id).await },
    );
    let log_activity = signal(String::new());
    let log_tab = signal(1_i64);
    let share_logs = resource(
        move || {
            let activity = log_activity.get();
            if activity.is_empty() || shelf.get() != Shelf::Share {
                return None;
            }
            running_account(page).map(|id| (id, activity, log_tab.get()))
        },
        |(id, activity, tab), _| async move { load_share_logs(id, activity, tab).await },
    );
    let tea_counts = signal(Vec::<(String, i64)>::new());
    let tea_boost = signal(1_i64);
    let tea = resource(
        move || (shelf.get() == Shelf::Tea).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            let shop =
                session::call({
                    let context = context.clone();
                    let id = id.clone();
                    async move {
                        qq_farm_app::milk_tea::snapshot(&context, &id, "shop", &json!({})).await
                    }
                })
                .await?;
            let exchange = session::call({
                let context = context.clone();
                let id = id.clone();
                async move {
                    qq_farm_app::milk_tea::snapshot(&context, &id, "exchange", &json!({})).await
                }
            })
            .await?;
            let workshop = session::call(async move {
                qq_farm_app::milk_tea::snapshot(&context, &id, "workshop", &json!({})).await
            })
            .await?;
            Ok(tea_shop(&shop, &exchange, &workshop))
        },
    );
    let hire_open = signal(false);
    let hire_duration = signal(1_i64);
    let hire = resource(
        move || {
            if !hire_open.get() || shelf.get() != Shelf::Tea {
                return None;
            }
            running_account(page).map(|id| (id, hire_duration.get()))
        },
        |(id, duration), _| async move { load_tea_hire(id, duration).await },
    );
    let fund_open = signal(false);
    let fund = resource(
        move || {
            if !fund_open.get() || shelf.get() != Shelf::Tea {
                return None;
            }
            running_account(page)
        },
        |id, _| async move { load_tea_fund(id).await },
    );
    let work_open = signal(false);
    let work = resource(
        move || {
            if !work_open.get() || shelf.get() != Shelf::Tea {
                return None;
            }
            running_account(page)
        },
        |id, _| async move { load_tea_work(id).await },
    );
    let settings_open = signal(false);
    let settings_name = signal(String::new());
    let open_name = signal(String::new());
    let settings = resource(
        move || {
            if !settings_open.get() || shelf.get() != Shelf::Tea {
                return None;
            }
            running_account(page)
        },
        |id, _| async move { load_tea_settings(id).await },
    );
    let sales_open = signal(false);
    let sales_day = signal(0_i64);
    let sales = resource(
        move || {
            if !sales_open.get() || shelf.get() != Shelf::Tea {
                return None;
            }
            running_account(page).map(|id| (id, sales_day.get()))
        },
        |(id, day), _| async move { load_tea_sales(id, day).await },
    );
    let rank_open = signal(false);
    let rank_page = signal(1_i64);
    let rank = resource(
        move || {
            if !rank_open.get() || shelf.get() != Shelf::Tea {
                return None;
            }
            running_account(page).map(|id| (id, rank_page.get().max(1)))
        },
        |(id, page_number), _| async move { load_tea_rank(id, page_number).await },
    );
    let ranch_directory = resource(
        move || (shelf.get() == Shelf::Ranch).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(
                async move { qq_farm_app::ranch_activities::snapshot(&context, &id).await },
            )
            .await
        },
    );
    let muge_board = resource(
        move || (shelf.get() == Shelf::Muge).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            let value =
                session::call(async move { qq_farm_app::muge::snapshot(&context, &id).await })
                    .await?;
            Ok(super::muge::board_from(&value))
        },
    );
    let weather = resource(
        move || (shelf.get() == Shelf::Weather).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::weather::snapshot(&context, &id).await }).await
        },
    );
    let weather_gid = signal(0_i64);
    let weather_picks = signal(Vec::<i64>::new());
    let weather_scans = signal(None::<Value>);
    let weather_session = signal(0_u64);
    let weather_generation = signal(0_u64);
    let weather_watch = signal(false);
    let weather_account = page.account_id;
    effect(move || {
        let _current = weather_account.get();
        weather_gid.set(0);
        weather_picks.set(Vec::new());
        weather_scans.set(None);
    });
    set_interval(WEATHER_VISIT_WATCH, move || {
        if shelf.get_untracked() != Shelf::Weather || weather_watch.get_untracked() {
            return;
        }
        let Some(id) = running_account(page) else {
            return;
        };
        weather_watch.set(true);
        let session = weather_session;
        let generation = weather_generation;
        let watch = weather_watch;
        spawn(async move {
            let context = session::session().context.clone();
            match session::call(async move { qq_farm_app::weather::visit_epoch(&context, &id) })
                .await
            {
                Ok((next_session, next_generation)) => {
                    session.set(next_session);
                    generation.set(next_generation);
                }
                Err(_) => session.set(0),
            }
            watch.set(false);
        });
    });
    let weather_friends = resource(
        move || (shelf.get() == Shelf::Weather).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            let rows = session::call(async move {
                qq_farm_app::friend::list_friends(&context, &id, false).await
            })
            .await?;
            Ok(super::weather::weather_friends(rows))
        },
    );
    let weather_visit = resource(
        move || {
            let gid = weather_gid.get();
            if shelf.get() != Shelf::Weather || gid <= 0 {
                return None;
            }
            running_account(page).map(|id| (id, gid))
        },
        |(id, gid), _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::weather::inspect(&context, &id, gid).await })
                .await
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(
            text(move || text_key("activities.title")).text_lg().font_weight(FontWeight::SEMIBOLD),
        )
        .child(shelf_switch(shelf))
        .child(failure_line(failure))
        .child(gate_notice(page.account_id, page.rows, "activities.needRunning"))
        .child(when(
            move || shelf.get() == Shelf::Solar && running(page),
            move || loaded(solar, pending, failure, page, Shelf::Solar),
        ))
        .child(when(
            move || shelf.get() == Shelf::Share && running(page),
            move || share_loaded(shares, share_logs, log_activity, log_tab, pending, failure, page),
        ))
        .child(when(
            move || shelf.get() == Shelf::Tea && running(page),
            move || {
                tea_loaded(
                    tea,
                    hire,
                    hire_open,
                    hire_duration,
                    pending,
                    failure,
                    page,
                    tea_counts,
                    tea_boost,
                    fund,
                    fund_open,
                    work,
                    work_open,
                    settings,
                    settings_open,
                    settings_name,
                    sales,
                    sales_open,
                    sales_day,
                    rank,
                    rank_open,
                    rank_page,
                    open_name,
                )
            },
        ))
        .child(when(
            move || shelf.get() == Shelf::Muge && running(page),
            move || super::muge::loaded(muge_board, pending, failure, page),
        ))
        .child(when(
            move || shelf.get() == Shelf::Ranch && running(page),
            move || ranch_loaded(ranch_directory),
        ))
        .child(when(
            move || shelf.get() == Shelf::Weather && running(page),
            move || {
                super::weather::loaded(
                    weather,
                    pending,
                    failure,
                    page,
                    weather_friends,
                    weather_visit,
                    weather_gid,
                    weather_picks,
                    weather_scans,
                    weather_session,
                    weather_generation,
                )
            },
        ))
}

fn ranch_loaded(directory: Resource<Value, String>) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || directory.loading() && directory.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || directory.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        directory.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || directory.refetch()),
                    )
            },
        ))
        .child(when(
            move || directory.get().is_some_and(|page| ranch_rows(&page).is_empty()),
            move || notice(move || text_key("activities.empty")),
        ))
        .child(each(
            move || directory.get().map(|page| ranch_rows(&page)).unwrap_or_default(),
            |row: &RanchActivity| row.id.clone(),
            move |row, _| {
                let row = row.get_untracked();
                let status_key = row.status_key;
                view()
                    .gap_1()
                    .child(text(row.name).text_sm().font_weight(FontWeight::MEDIUM))
                    .child(text(move || text_key(status_key)).text_sm())
                    .child(
                        text(row.when)
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground),
                    )
            },
        ))
}

#[derive(Clone, PartialEq, Eq)]
struct RanchActivity {
    id: String,
    name: String,
    status_key: &'static str,
    when: String,
}

fn ranch_rows(value: &Value) -> Vec<RanchActivity> {
    value
        .get("activities")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(ranch_row).collect())
        .unwrap_or_default()
}

fn ranch_row(value: &Value) -> Option<RanchActivity> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let status = value.get("status").and_then(Value::as_str).unwrap_or("");
    Some(RanchActivity {
        id: id.to_owned(),
        name: if name.is_empty() { format!("活动 {id}") } else { name.to_owned() },
        status_key: match status {
            "upcoming" => "activities.upcoming",
            "ended" => "activities.ended",
            _ => "activities.active",
        },
        when: value.get("when").and_then(Value::as_str).unwrap_or("").to_owned(),
    })
}

fn shelf_switch(shelf: Signal<Shelf>) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || Shelf::ALL.to_vec(),
        |item: &Shelf| item.id().to_owned(),
        move |item, _| {
            let item = item.get_untracked();
            button(move || text_key(item.label_key()), ButtonVariant::Quiet)
                .selected(move || shelf.get() == item)
                .on_press(move || shelf.set(item))
        },
    ))
}

fn running(page: ActivityPage) -> bool {
    page.account_id
        .get()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| account_is_running(&rows, &id))
}

fn running_account(page: ActivityPage) -> Option<String> {
    running(page).then(|| page.account_id.get()).flatten()
}

fn loaded(
    solar: Resource<SolarBoard, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    _shelf: Shelf,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || solar.loading() && solar.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(move || solar.error().is_some(), move || retry(move || solar.refetch(), solar)))
        .child(when(
            move || solar.get().is_some(),
            move || solar_body(solar, pending, failure, page),
        ))
}

fn retry(refresh: impl Fn() + 'static, resource: Resource<SolarBoard, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            resource.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(button(move || text_key("common.retry"), ButtonVariant::Secondary).on_press(refresh))
}

fn solar_body(
    solar: Resource<SolarBoard, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(refresh_button(pending, move || solar.refetch()))
        .child(when(
            move || {
                solar.get().is_some_and(|board| board.events.is_empty() && board.seasons.is_empty())
            },
            move || notice(move || text_key("activities.empty")),
        ))
        .child(each(
            move || solar.get().map(|board| board.events).unwrap_or_default(),
            |event: &SolarEvent| event.id.to_string(),
            move |event, _| solar_event(event.get_untracked(), pending, failure, page, solar),
        ))
        .child(each(
            move || solar.get().map(|board| board.seasons).unwrap_or_default(),
            |season: &SeasonLine| season.id.to_string(),
            move |season, _| solar_season(season.get_untracked(), pending, failure, page, solar),
        ))
}

fn solar_event(
    event: SolarEvent,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    solar: Resource<SolarBoard, String>,
) -> View {
    let name = SharedString::from(event.name.clone());
    let description = event.description.clone();
    let claim_id = event.id;
    let answer = event.clone();
    let can_claim = event.can_claim;
    let can_answer = event.option_index.is_some();
    let copy = description.clone();
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(view().flex_1().gap_1().child(text(name).text_sm()).child(when(
            move || !description.is_empty(),
            move || {
                text(copy.clone()).text_sm().text_color(move || nsh_base::theme().muted_foreground)
            },
        )))
        .child(when(move || can_claim, {
            let key = format!("claim:{claim_id}");
            move || {
                action_button(&key, "activities.claim", pending, move || {
                    run_solar("claim", json!({ "id": claim_id }), pending, failure, page, solar)
                })
            }
        }))
        .child(when(
            move || can_answer,
            move || {
                let key = format!("answer:{}", answer.id);
                let input = json!({
                    "id": answer.id,
                    "expectedQuestion": answer.question.clone(),
                    "optionIndex": answer.option_index,
                });
                let submit = input.clone();
                action_button(&key, "activities.answer", pending, move || {
                    run_solar("answer", submit.clone(), pending, failure, page, solar)
                })
            },
        ))
}

fn solar_season(
    season: SeasonLine,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    solar: Resource<SolarBoard, String>,
) -> View {
    let name = SharedString::from(season.name);
    let progress = SharedString::from(season.progress);
    let can_claim = season.can_claim;
    let id = season.id;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().flex_1())
        .child(text(progress).text_sm())
        .child(when(move || can_claim, {
            let key = format!("season:{id}");
            move || {
                action_button(&key, "activities.claim", pending, move || {
                    run_solar("season", json!({ "id": id }), pending, failure, page, solar)
                })
            }
        }))
}

fn share_loaded(
    shares: Resource<Vec<ShareLine>, String>,
    share_logs: Resource<Vec<ShareLog>, String>,
    log_activity: Signal<String>,
    log_tab: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || shares.loading() && shares.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || shares.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        shares.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || shares.refetch()),
                    )
            },
        ))
        .child(when(
            move || shares.get().is_some(),
            move || {
                view()
                    .gap_3()
                    .child(refresh_button(pending, move || shares.refetch()))
                    .child(when(
                        move || shares.get().is_some_and(|rows| rows.is_empty()),
                        move || notice(move || text_key("activities.empty")),
                    ))
                    .child(each(
                        move || shares.get().unwrap_or_default(),
                        |share: &ShareLine| share.id.clone(),
                        move |share, _| {
                            share_line(
                                share.get_untracked(),
                                pending,
                                failure,
                                page,
                                shares,
                                share_logs,
                                log_activity,
                                log_tab,
                            )
                        },
                    ))
            },
        ))
}

fn share_line(
    share: ShareLine,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    shares: Resource<Vec<ShareLine>, String>,
    share_logs: Resource<Vec<ShareLog>, String>,
    log_activity: Signal<String>,
    log_tab: Signal<i64>,
) -> View {
    let title = SharedString::from(format!("{} {}", share.title, share.score));
    let daily = SharedString::from(share.daily.clone());
    let pool = SharedString::from(share.pool.clone());
    let daily_reward = share.daily_reward;
    let first_share_reward = share.first_share_reward;
    let tiers = share.milestones.clone();
    let id = share.id.clone();
    let can_daily = share.can_daily;
    let can_share = share.can_share;
    let can_milestones = share.can_milestones;
    let logs_id = share.id.clone();
    let logs_open_id = share.id.clone();
    view()
        .gap_2()
        .py_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(text(title).text_sm().flex_1())
                .child(text(daily).text_sm()),
        )
        .child(
            text(move || {
                let pool = pool.clone();
                SharedString::from(format!(
                    "{} +{daily_reward} · {} {first_share_reward} · {} {pool}",
                    text_key("activities.shareDailyReward"),
                    text_key("activities.shareFirst"),
                    text_key("activities.sharePool"),
                ))
            })
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(each(
            move || tiers.clone(),
            |tier: &ShareTier| tier.id.clone(),
            move |tier, _| {
                let tier = tier.get_untracked();
                let state_key = tier.state_key;
                let rewards = SharedString::from(tier.rewards);
                let threshold = tier.threshold;
                text(move || {
                    let rewards = rewards.clone();
                    let state = text_key(state_key);
                    let points = text_key("activities.sharePoints");
                    SharedString::from(if rewards.is_empty() {
                        format!("{threshold} {points} · {state}")
                    } else {
                        format!("{threshold} {points} · {rewards} · {state}")
                    })
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(when(move || can_daily, {
                    let activity = id.clone();
                    move || {
                        let key = format!("daily:{activity}");
                        let activity = activity.clone();
                        action_button(&key, "activities.daily", pending, move || {
                            run_share(&activity, "daily", pending, failure, page, shares)
                        })
                    }
                }))
                .child(when(move || can_share, {
                    let activity = id.clone();
                    move || {
                        let key = format!("share:{activity}");
                        let activity = activity.clone();
                        action_button(&key, "activities.shareAction", pending, move || {
                            run_share(&activity, "share", pending, failure, page, shares)
                        })
                    }
                }))
                .child(when(move || can_milestones, {
                    let activity = id;
                    move || {
                        let key = format!("milestones:{activity}");
                        let activity = activity.clone();
                        action_button(&key, "activities.milestones", pending, move || {
                            run_share(&activity, "milestones", pending, failure, page, shares)
                        })
                    }
                })),
        )
        .child(
            button(move || text_key("activities.logs"), ButtonVariant::Quiet)
                .selected(move || log_activity.get() == logs_id)
                .on_press({
                    let open_id = logs_open_id.clone();
                    let set_id = logs_open_id.clone();
                    move || {
                        if log_activity.get_untracked() == open_id {
                            log_activity.set(String::new());
                        } else {
                            log_activity.set(set_id.clone());
                        }
                    }
                }),
        )
        .child(when(
            {
                let logs_open_id = logs_open_id.clone();
                move || log_activity.get() == logs_open_id
            },
            move || share_log_panel(share_logs, log_tab),
        ))
}

fn tea_loaded(
    tea: Resource<TeaShop, String>,
    hire: Resource<TeaHireBoard, String>,
    hire_open: Signal<bool>,
    hire_duration: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    counts: Signal<Vec<(String, i64)>>,
    boost_count: Signal<i64>,
    fund: Resource<TeaFund, String>,
    fund_open: Signal<bool>,
    work: Resource<TeaWork, String>,
    work_open: Signal<bool>,
    settings: Resource<TeaSettings, String>,
    settings_open: Signal<bool>,
    settings_name: Signal<String>,
    sales: Resource<TeaSales, String>,
    sales_open: Signal<bool>,
    sales_day: Signal<i64>,
    rank: Resource<TeaRank, String>,
    rank_open: Signal<bool>,
    rank_page: Signal<i64>,
    open_name: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || tea.loading() && tea.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || tea.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        tea.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || tea.refetch()),
                    )
            },
        ))
        .child(when(
            move || tea.get().is_some_and(|shop| !shop.opened),
            move || tea_open_panel(tea, open_name, pending, failure, page),
        ))
        .child(when(
            move || tea.get().is_some_and(|shop| shop.opened),
            move || {
                tea_body(
                    tea,
                    hire,
                    hire_open,
                    hire_duration,
                    pending,
                    failure,
                    page,
                    counts,
                    boost_count,
                    fund,
                    fund_open,
                    work,
                    work_open,
                    settings,
                    settings_open,
                    settings_name,
                    sales,
                    sales_open,
                    sales_day,
                    rank,
                    rank_open,
                    rank_page,
                )
            },
        ))
}

fn tea_open_panel(
    tea: Resource<TeaShop, String>,
    draft: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .gap_2()
        .child(
            text(move || text_key("activities.openTitle"))
                .text_sm()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(
            text(move || text_key("activities.openHint"))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(crate::ui::field(
            move || draft.get(),
            move || text_key("activities.open").to_string(),
            move |text| draft.set(text.to_owned()),
        ))
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            move || tea.get().map(|shop| shop.suggestions).unwrap_or_default(),
            |name: &String| name.clone(),
            move |name, _| {
                let name = name.get_untracked();
                let label = SharedString::from(name.clone());
                let selected = name.clone();
                button(move || label.clone(), ButtonVariant::Quiet)
                    .selected(move || draft.get().trim() == selected)
                    .on_press({
                        let name = name.clone();
                        move || draft.set(name.clone())
                    })
            },
        )))
        .child(
            button(move || text_key("activities.open"), ButtonVariant::Primary)
                .disabled(move || {
                    pending.get().is_some() || !rename_allowed("", &draft.get(), false)
                })
                .on_press(move || {
                    let name = draft.get().trim().to_owned();
                    super::confirm::confirm(
                        "activities.confirmOpen.title",
                        "activities.confirmOpen.body",
                        "create",
                        "activities.open",
                        move |accepted| {
                            if accepted {
                                run_create(name.clone(), pending, failure, page, tea);
                            }
                        },
                    );
                }),
        )
}

fn run_create(
    name: String,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) {
    if !rename_allowed("", &name, false) {
        return;
    }
    let Some(id) = begin("create", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(&context, &id, "create", &json!({ "name": name })).await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
    });
}

fn tea_body(
    tea: Resource<TeaShop, String>,
    hire: Resource<TeaHireBoard, String>,
    hire_open: Signal<bool>,
    hire_duration: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    counts: Signal<Vec<(String, i64)>>,
    boost_count: Signal<i64>,
    fund: Resource<TeaFund, String>,
    fund_open: Signal<bool>,
    work: Resource<TeaWork, String>,
    work_open: Signal<bool>,
    settings: Resource<TeaSettings, String>,
    settings_open: Signal<bool>,
    settings_name: Signal<String>,
    sales: Resource<TeaSales, String>,
    sales_open: Signal<bool>,
    sales_day: Signal<i64>,
    rank: Resource<TeaRank, String>,
    rank_open: Signal<bool>,
    rank_page: Signal<i64>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(refresh_button(pending, move || tea.refetch()))
        .child(text(move || tea_name(tea)).text_sm().font_weight(FontWeight::MEDIUM))
        .child(text(move || tea_metrics(tea)).text_sm())
        .child(when(
            move || tea.get().is_some_and(|shop| shop.settling),
            move || notice(move || text_key("activities.settling")),
        ))
        .child(when(
            move || tea.get().is_some_and(|shop| shop.can_claim),
            move || {
                action_button("claim", "activities.cashier", pending, move || {
                    run_tea(pending, failure, page, tea)
                })
            },
        ))
        .child(when(
            move || tea.get().is_some_and(|shop| shop.can_upgrade),
            move || {
                action_button("upgrade", "activities.upgrade", pending, move || {
                    let level = tea.get().map(|shop| shop.level).unwrap_or(0);
                    super::confirm::confirm(
                        "activities.confirmUpgrade.title",
                        "activities.confirmUpgrade.body",
                        "upgrade",
                        "activities.upgrade",
                        move |accepted| {
                            if accepted {
                                run_upgrade(level, pending, failure, page, tea);
                            }
                        },
                    );
                })
            },
        ))
        .child(when(
            move || {
                tea.get().is_some_and(|shop| {
                    shop.sales_item > 0 && shop.sales_stock > 0 && !shop.settling
                })
            },
            move || tea_boost_row(tea, boost_count, pending, failure, page),
        ))
        .child(each(
            move || tea.get().map(|shop| shop.goods).unwrap_or_default(),
            |good: &TeaGood| good.id.clone(),
            move |good, _| tea_good(good.get_untracked(), pending, failure, page, tea, counts),
        ))
        .child(
            text(move || text_key("activities.workshop")).text_sm().font_weight(FontWeight::MEDIUM),
        )
        .child(each(
            move || tea.get().map(|shop| shop.recipes).unwrap_or_default(),
            |recipe: &TeaRecipe| recipe.id.to_string(),
            move |recipe, _| {
                let recipe = recipe.get_untracked();
                let name = SharedString::from(recipe.name);
                let stock = recipe.stock;
                text(move || {
                    let name = name.clone();
                    SharedString::from(format!(
                        "{name} · {} {stock}",
                        text_key("activities.stockCups")
                    ))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(each(
            move || tea.get().map(|shop| shop.fruits).unwrap_or_default(),
            |fruit: &TeaFruit| fruit.uid.clone(),
            move |fruit, _| tea_fruit(fruit.get_untracked(), pending, failure, page, tea),
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    button(move || text_key("activities.hireFriends"), ButtonVariant::Quiet)
                        .selected(move || hire_open.get())
                        .disabled(move || pending.get().is_some())
                        .on_press(move || hire_open.set(!hire_open.get_untracked())),
                )
                .child(
                    button(move || text_key("activities.fund"), ButtonVariant::Quiet)
                        .selected(move || fund_open.get())
                        .disabled(move || pending.get().is_some())
                        .on_press(move || fund_open.set(!fund_open.get_untracked())),
                )
                .child(
                    button(move || text_key("activities.work"), ButtonVariant::Quiet)
                        .selected(move || work_open.get())
                        .disabled(move || pending.get().is_some())
                        .on_press(move || work_open.set(!work_open.get_untracked())),
                )
                .child(
                    button(move || text_key("activities.settings"), ButtonVariant::Quiet)
                        .selected(move || settings_open.get())
                        .disabled(move || pending.get().is_some())
                        .on_press(move || settings_open.set(!settings_open.get_untracked())),
                )
                .child(
                    button(move || text_key("activities.sales"), ButtonVariant::Quiet)
                        .selected(move || sales_open.get())
                        .disabled(move || pending.get().is_some())
                        .on_press(move || sales_open.set(!sales_open.get_untracked())),
                )
                .child(
                    button(move || text_key("activities.rank"), ButtonVariant::Quiet)
                        .selected(move || rank_open.get())
                        .disabled(move || pending.get().is_some())
                        .on_press(move || rank_open.set(!rank_open.get_untracked())),
                ),
        )
        .child(when(
            move || hire_open.get(),
            move || tea_hire_panel(hire, hire_duration, pending, failure, page, tea),
        ))
        .child(when(move || fund_open.get(), move || tea_fund_panel(fund, pending, failure, page)))
        .child(when(move || work_open.get(), move || tea_work_panel(work, pending, failure, page)))
        .child(when(
            move || settings_open.get(),
            move || tea_settings_panel(settings, settings_name, pending, failure, page, tea),
        ))
        .child(when(move || sales_open.get(), move || tea_sales_panel(sales, sales_day)))
        .child(when(move || rank_open.get(), move || tea_rank_panel(rank, rank_page)))
}

fn tea_good(
    current: TeaGood,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
    counts: Signal<Vec<(String, i64)>>,
) -> impl IntoElement {
    let name = current.name.clone();
    let price = current.price.clone();
    let limit_key = current.limit_key;
    let shown_remaining = current.remaining.unwrap_or(0);
    let limit_max = current.limit_max;
    let available = current.available;
    let max_count = current.max_count;
    let goods_id = current.id.clone();
    let offer = current.clone();
    view()
        .gap_1()
        .child(text(name).text_sm())
        .child(text(price).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || limit_key.is_some(),
            move || {
                text(move || {
                    let label = text_key(limit_key.unwrap_or("mall.limit"));
                    if limit_max > 0 {
                        SharedString::from(format!("{label} {shown_remaining}/{limit_max}"))
                    } else {
                        label
                    }
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            move || available && max_count > 1,
            move || tea_count_stepper(goods_id.clone(), max_count, counts, pending),
        ))
        .child(when(move || available, {
            let offer = offer.clone();
            move || {
                let offer = offer.clone();
                button(move || text_key("activities.exchange"), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        let offer = offer.clone();
                        super::confirm::confirm(
                            "activities.confirmExchange.title",
                            "activities.confirmExchange.body",
                            "exchange",
                            "activities.exchange",
                            move |accepted| {
                                if accepted {
                                    run_exchange(offer, pending, failure, page, tea, counts);
                                }
                            },
                        );
                    })
            }
        }))
}

fn tea_name(tea: Resource<TeaShop, String>) -> SharedString {
    let name = tea.get().map(|shop| shop.name).unwrap_or_default();
    if name.is_empty() {
        text_key("activities.tea")
    } else {
        SharedString::from(name)
    }
}

fn tea_metrics(tea: Resource<TeaShop, String>) -> SharedString {
    let shop = tea.get();
    let level = shop.as_ref().map(|shop| shop.level).unwrap_or(0);
    let revenue = shop.as_ref().map(|shop| shop.revenue).unwrap_or(0);
    SharedString::from(format!(
        "{} {level} · {} {revenue}",
        text_key("activities.level"),
        text_key("activities.revenue")
    ))
}

fn refresh_button(
    pending: Signal<Option<String>>,
    refresh: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
        .disabled(move || pending.get().is_some())
        .on_press(refresh)
}

fn action_button(
    key: &str,
    label: &'static str,
    pending: Signal<Option<String>>,
    press: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let key = key.to_owned();
    button(
        move || {
            if pending.get().as_deref() == Some(key.as_str()) {
                text_key("farm.working")
            } else {
                text_key(label)
            }
        },
        ButtonVariant::Primary,
    )
    .disabled(move || pending.get().is_some())
    .on_press(press)
}

fn run_solar(
    action: &'static str,
    input: Value,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    solar: Resource<SolarBoard, String>,
) {
    let key = format!("{action}:{}", input.get("id").and_then(Value::as_i64).unwrap_or(0));
    let Some(id) = begin(&key, page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::solar_calendar::operate(&context, &id, action, &input).await
        })
        .await;
        finish(result, pending, failure);
        solar.refetch();
    });
}

fn run_share(
    activity_id: &str,
    action: &'static str,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    shares: Resource<Vec<ShareLine>, String>,
) {
    let key = format!("{action}:{activity_id}");
    let activity_id = activity_id.to_owned();
    let Some(id) = begin(&key, page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::share_reward::operate(&context, &id, &activity_id, action, &json!({}))
                .await
        })
        .await;
        finish(result, pending, failure);
        shares.refetch();
    });
}

fn run_exchange(
    good: TeaGood,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
    counts: Signal<Vec<(String, i64)>>,
) {
    let key = format!("exchange:{}", good.id);
    let Some(id) = begin(&key, page, pending, failure) else {
        return;
    };
    let goods_id = good.id.clone();
    let quantity = tea_chosen(counts, &good.id, good.max_count.max(1));
    let costs = good
        .costs
        .iter()
        .map(|(item_id, count)| json!({ "id": item_id, "count": count }))
        .collect::<Vec<_>>();
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "exchange",
                &json!({ "goodsId": goods_id, "count": quantity, "expectedCosts": costs }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
    });
}

fn tea_fruit(
    fruit: TeaFruit,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) -> impl IntoElement {
    let name = SharedString::from(fruit.name);
    let count = fruit.count;
    let uid = fruit.uid;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(text(name).text_sm().flex_1())
        .child(text(move || SharedString::from(count.to_string())).text_sm())
        .child(
            button(move || text_key("activities.craft"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || count <= 0)
                .on_press({
                    let uid = uid.clone();
                    move || {
                        let uid = uid.clone();
                        super::confirm::confirm(
                            "activities.confirmCraft.title",
                            "activities.confirmCraft.body",
                            "craft",
                            "activities.craft",
                            move |accepted| {
                                if accepted {
                                    run_craft(uid.clone(), count, pending, failure, page, tea);
                                }
                            },
                        );
                    }
                }),
        )
}

fn run_craft(
    uid: String,
    count: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) {
    if count <= 0 {
        return;
    }
    let key = format!("craft:{uid}");
    let Some(id) = begin(&key, page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "craft",
                &json!({ "fruits": [{ "uid": uid, "count": count }] }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
    });
}

const TEA_TOKEN: i64 = 1041;

async fn load_tea_hire(id: String, duration: i64) -> Result<TeaHireBoard, String> {
    let context = session::session().context.clone();
    let value = session::call(async move {
        qq_farm_app::milk_tea::snapshot(
            &context,
            &id,
            "hireList",
            &json!({ "durationId": duration }),
        )
        .await
    })
    .await?;
    Ok(tea_hire_board(&value))
}

fn tea_hire_panel(
    hire: Resource<TeaHireBoard, String>,
    duration: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || hire.loading() && hire.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || hire.error().is_some(),
            move || {
                crate::ui::failure(move || hire.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(
            move || hire.get().is_some(),
            move || tea_hire_body(hire, duration, pending, failure, page, tea),
        ))
}

fn tea_hire_body(
    hire: Resource<TeaHireBoard, String>,
    duration: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) -> View {
    view()
        .gap_2()
        .child(
            text(move || {
                let wage = hire.get().map(|board| board.wage).unwrap_or(0);
                SharedString::from(format!("{} {wage}", text_key("activities.hireWage")))
            })
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            move || hire.get().map(|board| board.durations).unwrap_or_default(),
            |item: &TeaDuration| item.id.to_string(),
            move |item, _| {
                let item = item.get_untracked();
                let id = item.id;
                let hours = item.hours;
                button(
                    move || SharedString::from(format!("{hours} {}", text_key("activities.hours"))),
                    ButtonVariant::Quiet,
                )
                .selected(move || duration.get() == id)
                .on_press(move || duration.set(id))
            },
        )))
        .child(when(
            move || hire.get().is_some_and(|board| board.candidates.is_empty()),
            move || notice(move || text_key("activities.hireEmpty")),
        ))
        .child(each(
            move || hire.get().map(|board| board.candidates).unwrap_or_default(),
            |candidate: &TeaCandidate| candidate.gid.clone(),
            move |candidate, _| {
                tea_candidate_row(candidate.get_untracked(), hire, pending, failure, page, tea)
            },
        ))
}

fn tea_candidate_row(
    candidate: TeaCandidate,
    hire: Resource<TeaHireBoard, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) -> impl IntoElement {
    let name = SharedString::from(candidate.name);
    let talent = SharedString::from(candidate.talent);
    let gid = candidate.gid;
    let gid_ready = gid.clone();
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(text(name).text_sm().flex_1())
        .child(text(talent).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || {
                hire.get().is_some_and(|board| {
                    !board.settling
                        && board
                            .candidates
                            .iter()
                            .any(|item| item.gid == gid_ready && item.available)
                })
            },
            {
                let gid = gid.clone();
                move || {
                    let gid = gid.clone();
                    button(move || text_key("activities.hire"), ButtonVariant::Secondary)
                        .disabled(move || pending.get().is_some())
                        .on_press(move || {
                            let gid = gid.clone();
                            let wage = hire.get().map(|board| board.wage).unwrap_or(0);
                            super::confirm::confirm(
                                "activities.confirmHire.title",
                                "activities.confirmHire.body",
                                "hire",
                                "activities.hire",
                                move |accepted| {
                                    if accepted {
                                        run_hire(
                                            gid.clone(),
                                            wage,
                                            pending,
                                            failure,
                                            page,
                                            tea,
                                            hire,
                                        );
                                    }
                                },
                            );
                        })
                }
            },
        ))
}

fn run_hire(
    gid: String,
    wage: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
    hire: Resource<TeaHireBoard, String>,
) {
    if gid.is_empty() || wage < 0 {
        return;
    }
    let key = format!("hire:{gid}");
    let Some(id) = begin(&key, page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "hire",
                &json!({
                    "targetGid": gid,
                    "expectedCosts": [{ "id": TEA_TOKEN, "count": wage }],
                }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
        hire.refetch();
    });
}

fn tea_hire_board(value: &Value) -> TeaHireBoard {
    let hire = value.get("hire").unwrap_or(&Value::Null);
    let now = value.get("serverTime").and_then(Value::as_i64).unwrap_or(0);
    let mut candidates = tea_candidates(hire.get("friends"), now);
    candidates.extend(tea_candidates(hire.get("robots"), now));
    TeaHireBoard {
        wage: hire.get("wageTotal").and_then(Value::as_i64).unwrap_or(0).max(0),
        settling: value.get("settling").and_then(Value::as_bool).unwrap_or(false),
        durations: value
            .get("durations")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(tea_duration).collect())
            .unwrap_or_default(),
        candidates,
    }
}

fn tea_duration(value: &Value) -> Option<TeaDuration> {
    let id = value.get("durationId").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let seconds = value.get("durationSec").and_then(Value::as_i64).unwrap_or(0).max(0);
    Some(TeaDuration { id, hours: seconds / 3600 })
}

fn tea_candidates(value: Option<&Value>, now: i64) -> Vec<TeaCandidate> {
    value
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| tea_candidate_from(row, now)).collect())
        .unwrap_or_default()
}

fn tea_candidate_from(value: &Value, now: i64) -> Option<TeaCandidate> {
    let gid = value
        .pointer("/friend/gid")
        .and_then(|item| {
            item.as_i64().map(|gid| gid.to_string()).or_else(|| item.as_str().map(str::to_owned))
        })
        .filter(|gid| !gid.is_empty() && gid != "0")?;
    let name = value.pointer("/friend/name").and_then(Value::as_str).unwrap_or("");
    let ready = value.get("rehireAvailableAt").and_then(Value::as_i64).unwrap_or(0) <= now;
    Some(TeaCandidate {
        gid,
        name: if name.is_empty() { "好友".to_owned() } else { name.to_owned() },
        talent: value.get("talentName").and_then(Value::as_str).unwrap_or("").to_owned(),
        available: value.get("state").and_then(Value::as_i64).unwrap_or(-1) == 0 && ready,
    })
}

async fn load_tea_fund(id: String) -> Result<TeaFund, String> {
    let context = session::session().context.clone();
    let value = session::call(async move {
        qq_farm_app::milk_tea::snapshot(&context, &id, "fund", &json!({})).await
    })
    .await?;
    Ok(tea_fund(&value))
}

fn tea_fund_panel(
    fund: Resource<TeaFund, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || fund.loading() && fund.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || fund.error().is_some(),
            move || {
                crate::ui::failure(move || fund.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(
            move || fund.get().is_some(),
            move || tea_fund_body(fund, pending, failure, page),
        ))
}

fn tea_fund_body(
    fund: Resource<TeaFund, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || fund.get().is_some_and(|board| !board.unlocked),
            move || {
                button(
                    move || {
                        let costs = fund.get().map(|board| board.costs).unwrap_or_default();
                        if costs.is_empty() {
                            text_key("activities.fundUnlock")
                        } else {
                            let price = costs
                                .iter()
                                .map(|cost| {
                                    let name = if cost.name.is_empty() {
                                        cost.id.to_string()
                                    } else {
                                        cost.name.clone()
                                    };
                                    format!("{name} {}", cost.count)
                                })
                                .collect::<Vec<_>>()
                                .join("、");
                            SharedString::from(format!(
                                "{} {price}",
                                text_key("activities.fundUnlock")
                            ))
                        }
                    },
                    ButtonVariant::Secondary,
                )
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    let costs = fund.get().map(|board| board.costs).unwrap_or_default();
                    super::confirm::confirm(
                        "activities.confirmFund.title",
                        "activities.confirmFund.body",
                        "fund",
                        "activities.fundUnlock",
                        move |accepted| {
                            if accepted {
                                run_fund_unlock(costs.clone(), pending, failure, page, fund);
                            }
                        },
                    );
                })
            },
        ))
        .child(when(
            move || fund.get().is_some_and(|board| fund_claimable(&board)),
            move || {
                action_button("fundClaim", "activities.fundClaim", pending, move || {
                    run_fund_claim(pending, failure, page, fund)
                })
            },
        ))
        .child(each(
            move || fund.get().map(|board| board.tiers).unwrap_or_default(),
            |tier: &TeaFundTier| tier.id.to_string(),
            move |tier, _| {
                let tier = tier.get_untracked();
                let level = tier.level;
                let state = tier.state;
                let rewards = SharedString::from(tier.rewards);
                text(move || {
                    let unlocked = fund.get().is_some_and(|board| board.unlocked);
                    let rewards = rewards.clone();
                    let state_label = text_key(fund_state_key(unlocked, state));
                    if rewards.is_empty() {
                        SharedString::from(format!(
                            "{} {level} · {state_label}",
                            text_key("activities.fundLevel")
                        ))
                    } else {
                        SharedString::from(format!(
                            "{} {level} · {state_label} · {rewards}",
                            text_key("activities.fundLevel")
                        ))
                    }
                })
                .text_sm()
            },
        ))
}

fn fund_claimable(board: &TeaFund) -> bool {
    board.unlocked && board.tiers.iter().any(|tier| tier.state == 2)
}

fn fund_state_key(unlocked: bool, state: i64) -> &'static str {
    if state == 3 {
        "activities.tierClaimed"
    } else if !unlocked {
        "activities.fundLocked"
    } else if state == 2 {
        "activities.tierReady"
    } else {
        "activities.tierLocked"
    }
}

fn run_fund_unlock(
    costs: Vec<TeaCost>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    fund: Resource<TeaFund, String>,
) {
    let Some(id) = begin("fund", page, pending, failure) else {
        return;
    };
    let expected: Vec<Value> =
        costs.iter().map(|cost| json!({ "id": cost.id, "count": cost.count })).collect();
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "unlock",
                &json!({ "expectedCosts": expected }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        fund.refetch();
    });
}

fn run_fund_claim(
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    fund: Resource<TeaFund, String>,
) {
    let Some(id) = begin("fundClaim", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(&context, &id, "fundClaim", &json!({})).await
        })
        .await;
        finish(result, pending, failure);
        fund.refetch();
    });
}

fn tea_fund(value: &Value) -> TeaFund {
    let fund = value.get("fund").unwrap_or(&Value::Null);
    TeaFund {
        unlocked: fund.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        costs: fund
            .get("costs")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(tea_cost).collect())
            .unwrap_or_default(),
        tiers: fund
            .get("tiers")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(tea_fund_tier).collect())
            .unwrap_or_default(),
    }
}

fn tea_cost(value: &Value) -> Option<TeaCost> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    if count < 0 {
        return None;
    }
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    Some(TeaCost { id, count, name: name.to_owned() })
}

fn tea_fund_tier(value: &Value) -> Option<TeaFundTier> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let level = value
        .get("conditions")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("target"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|reward| {
                    let name = reward.get("name").and_then(Value::as_str).unwrap_or("");
                    let count = reward.get("count").and_then(Value::as_i64).unwrap_or(0);
                    if name.is_empty() || count <= 0 {
                        None
                    } else {
                        Some(format!("{name} ×{count}"))
                    }
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    Some(TeaFundTier {
        id,
        level,
        state: value.get("state").and_then(Value::as_i64).unwrap_or(0),
        rewards,
    })
}

async fn load_tea_work(id: String) -> Result<TeaWork, String> {
    let context = session::session().context.clone();
    let value = session::call(async move {
        qq_farm_app::milk_tea::snapshot(&context, &id, "work", &json!({})).await
    })
    .await?;
    Ok(tea_work(&value))
}

fn tea_work_panel(
    work: Resource<TeaWork, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || work.loading() && work.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || work.error().is_some(),
            move || {
                crate::ui::failure(move || work.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(
            move || work.get().is_some(),
            move || tea_work_body(work, pending, failure, page),
        ))
}

fn tea_work_body(
    work: Resource<TeaWork, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || work.get().is_some_and(|board| board.shift.is_none() && board.slips.is_empty()),
            move || notice(move || text_key("activities.workEmpty")),
        ))
        .child(text(move || tea_shift_line(work)).text_sm())
        .child(when(
            move || work.get().is_some_and(|board| board.slips.iter().any(|slip| slip.claimable)),
            move || {
                action_button("wages", "activities.wages", pending, move || {
                    run_wages(pending, failure, page, work)
                })
            },
        ))
        .child(each(
            move || work.get().map(|board| board.slips).unwrap_or_default(),
            |slip: &TeaSlip| slip.id.clone(),
            move |slip, _| {
                let slip = slip.get_untracked();
                let employer = SharedString::from(slip.employer);
                let wage = slip.wage;
                text(move || {
                    let employer = employer.clone();
                    SharedString::from(format!(
                        "{employer} · {} {wage}",
                        text_key("activities.hireWage")
                    ))
                })
                .text_sm()
            },
        ))
        .child(each(
            move || work.get().map(|board| board.actions).unwrap_or_default(),
            |action: &TeaInteract| action.id.to_string(),
            move |action, _| tea_interact_row(action.get_untracked(), work, pending, failure, page),
        ))
}

fn tea_shift_line(work: Resource<TeaWork, String>) -> SharedString {
    let Some(board) = work.get() else {
        return SharedString::from("");
    };
    let Some(shift) = board.shift else {
        return SharedString::from("");
    };
    let left = if board.done { 0 } else { shift.limit.saturating_sub(shift.used).max(0) };
    let who = if shift.shop.is_empty() || shift.shop == shift.employer {
        shift.employer
    } else {
        format!("{} · {}", shift.employer, shift.shop)
    };
    SharedString::from(format!(
        "{who} · {} {left}/{}",
        text_key("activities.workChances"),
        shift.limit
    ))
}

fn tea_interact_row(
    action: TeaInteract,
    work: Resource<TeaWork, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> impl IntoElement {
    let name = SharedString::from(action.name);
    let description = SharedString::from(action.description);
    let kind = action.id;
    let ready = action.ready;
    view()
        .gap_1()
        .child(text(description).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || ready && work.get().is_some_and(|board| shift_open(&board)),
            move || {
                let name = name.clone();
                button(move || name.clone(), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        let shift = work
                            .get()
                            .and_then(|board| board.shift.map(|shift| shift.id))
                            .unwrap_or_default();
                        super::confirm::confirm(
                            "activities.confirmInteract.title",
                            "activities.confirmInteract.body",
                            "interact",
                            "activities.work",
                            move |accepted| {
                                if accepted {
                                    run_interact(kind, shift.clone(), pending, failure, page, work);
                                }
                            },
                        );
                    })
            },
        ))
}

fn shift_open(board: &TeaWork) -> bool {
    board.shift.as_ref().is_some_and(|shift| {
        !board.done && !shift.id.is_empty() && shift.end_at > board.now && shift.used < shift.limit
    })
}

fn run_wages(
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    work: Resource<TeaWork, String>,
) {
    let Some(id) = begin("wages", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(&context, &id, "wages", &json!({})).await
        })
        .await;
        finish(result, pending, failure);
        work.refetch();
    });
}

fn run_interact(
    kind: i64,
    shift: String,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    work: Resource<TeaWork, String>,
) {
    if kind <= 0 || shift.is_empty() {
        return;
    }
    let Some(id) = begin("interact", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "interact",
                &json!({ "interactionType": kind, "expectedShiftId": shift }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        work.refetch();
    });
}

fn tea_work(value: &Value) -> TeaWork {
    let work = value.get("work").unwrap_or(&Value::Null);
    TeaWork {
        done: work.get("interactDone").and_then(Value::as_bool).unwrap_or(false),
        now: value.get("serverTime").and_then(Value::as_i64).unwrap_or(0),
        shift: work.get("myShift").and_then(tea_shift),
        slips: work
            .get("slips")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter().enumerate().filter_map(|(index, row)| tea_slip(index, row)).collect()
            })
            .unwrap_or_default(),
        actions: value
            .get("interactions")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(tea_interact).collect())
            .unwrap_or_default(),
    }
}

fn tea_shift(value: &Value) -> Option<TeaShift> {
    if value.is_null() {
        return None;
    }
    let id = json_id(value.get("shiftId"));
    if id.is_empty() || id == "0" {
        return None;
    }
    let employer = value.pointer("/employer/name").and_then(Value::as_str).unwrap_or("");
    let shop = value.get("shopName").and_then(Value::as_str).unwrap_or("");
    Some(TeaShift {
        id,
        employer: if employer.is_empty() { shop.to_owned() } else { employer.to_owned() },
        shop: shop.to_owned(),
        end_at: value.get("endAt").and_then(Value::as_i64).unwrap_or(0),
        used: value.get("interactCount").and_then(Value::as_i64).unwrap_or(0).max(0),
        limit: value.get("interactLimit").and_then(Value::as_i64).unwrap_or(0).max(0),
    })
}

fn tea_slip(index: usize, value: &Value) -> Option<TeaSlip> {
    let wage = value.pointer("/wages/finalWage").and_then(Value::as_i64).unwrap_or(0).max(0);
    let employer = value.pointer("/employer/name").and_then(Value::as_str).unwrap_or("");
    let shift = json_id(value.get("shiftId"));
    Some(TeaSlip {
        id: if shift.is_empty() { index.to_string() } else { format!("{shift}:{index}") },
        employer: if employer.is_empty() { "好友".to_owned() } else { employer.to_owned() },
        wage,
        claimable: value.get("wageStatus").and_then(Value::as_i64) == Some(1),
    })
}

fn tea_interact(value: &Value) -> Option<TeaInteract> {
    let id = value.get("interactionId").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        return None;
    }
    Some(TeaInteract {
        id,
        name: name.to_owned(),
        description: value.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
        ready: value.get("ready").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn json_id(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

async fn load_tea_settings(id: String) -> Result<TeaSettings, String> {
    let context = session::session().context.clone();
    let value = session::call(async move {
        qq_farm_app::milk_tea::snapshot(&context, &id, "settings", &json!({})).await
    })
    .await?;
    Ok(tea_settings(&value))
}

fn tea_settings_panel(
    settings: Resource<TeaSettings, String>,
    draft: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || settings.loading() && settings.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || settings.error().is_some(),
            move || {
                crate::ui::failure(move || {
                    settings.error().map(SharedString::from).unwrap_or_default()
                })
            },
        ))
        .child(when(
            move || settings.get().is_some(),
            move || tea_settings_body(settings, draft, pending, failure, page, tea),
        ))
}

fn tea_settings_body(
    settings: Resource<TeaSettings, String>,
    draft: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) -> View {
    view()
        .gap_2()
        .child(view().flex_row().items_center().gap_2().child(crate::ui::field(
            move || draft.get(),
            move || text_key("activities.rename").to_string(),
            move |text| draft.set(text.to_owned()),
        )))
        .child(when(
            move || settings.get().is_some_and(|board| board.rename_at > board.now),
            move || {
                text(move || {
                    let when = settings
                        .get()
                        .map(|board| share_log_time(board.rename_at))
                        .unwrap_or_default();
                    SharedString::from(format!("{} {when}", text_key("activities.renameCooling")))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(
            button(move || text_key("activities.rename"), ButtonVariant::Secondary)
                .disabled(move || {
                    let board = settings.get();
                    pending.get().is_some()
                        || !board.is_some_and(|board| {
                            rename_allowed(&board.name, &draft.get(), board.rename_at > board.now)
                        })
                })
                .on_press(move || {
                    let name = draft.get().trim().to_owned();
                    super::confirm::confirm(
                        "activities.confirmRename.title",
                        "activities.confirmRename.body",
                        "rename",
                        "activities.rename",
                        move |accepted| {
                            if accepted {
                                run_rename(name.clone(), pending, failure, page, tea, settings);
                            }
                        },
                    );
                }),
        )
        .child(
            text(move || text_key("activities.hireOffHint"))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(
            button(
                move || {
                    if settings.get().is_some_and(|board| board.hire_disabled) {
                        text_key("activities.hireOffSave")
                    } else {
                        text_key("activities.hireOffOn")
                    }
                },
                ButtonVariant::Secondary,
            )
            .disabled(move || pending.get().is_some() || settings.get().is_none())
            .on_press(move || {
                let disabled = !settings.get().is_some_and(|board| board.hire_disabled);
                super::confirm::confirm(
                    "activities.confirmHireOff.title",
                    "activities.confirmHireOff.body",
                    "hireSetting",
                    "activities.hireOff",
                    move |accepted| {
                        if accepted {
                            run_hire_setting(disabled, pending, failure, page, settings);
                        }
                    },
                );
            }),
        )
}

fn rename_allowed(current: &str, draft: &str, cooling: bool) -> bool {
    if cooling {
        return false;
    }
    let name = draft.trim();
    let length = name.chars().count();
    (2..=8).contains(&length) && name != current
}

fn run_rename(
    name: String,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
    settings: Resource<TeaSettings, String>,
) {
    if !rename_allowed("", &name, false) {
        return;
    }
    let Some(id) = begin("rename", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(&context, &id, "rename", &json!({ "name": name })).await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
        settings.refetch();
    });
}

fn run_hire_setting(
    disabled: bool,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    settings: Resource<TeaSettings, String>,
) {
    let Some(id) = begin("hireSetting", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "hireSetting",
                &json!({ "disabled": disabled }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        settings.refetch();
    });
}

fn tea_settings(value: &Value) -> TeaSettings {
    let summary = value.get("summary").unwrap_or(&Value::Null);
    TeaSettings {
        name: summary.get("shopName").and_then(Value::as_str).unwrap_or("").to_owned(),
        rename_at: summary.get("renameCdUntilAt").and_then(Value::as_i64).unwrap_or(0),
        now: value.get("serverTime").and_then(Value::as_i64).unwrap_or(0),
        hire_disabled: summary.get("hireDisabled").and_then(Value::as_bool).unwrap_or(false),
    }
}

fn tea_today(now: i64) -> i64 {
    ((now + 8 * 3_600) / 86_400) * 86_400 - 8 * 3_600
}

async fn load_tea_sales(id: String, day: i64) -> Result<TeaSales, String> {
    let context = session::session().context.clone();
    let input = if day > 0 { json!({ "day": day }) } else { json!({}) };
    let value = session::call(async move {
        qq_farm_app::milk_tea::snapshot(&context, &id, "logs", &input).await
    })
    .await?;
    Ok(tea_sales(&value))
}

async fn load_tea_rank(id: String, page_number: i64) -> Result<TeaRank, String> {
    let context = session::session().context.clone();
    let value = session::call(async move {
        qq_farm_app::milk_tea::snapshot(
            &context,
            &id,
            "rank",
            &json!({ "pageNumber": page_number.max(1) }),
        )
        .await
    })
    .await?;
    Ok(tea_rank(&value))
}

fn tea_sales_panel(sales: Resource<TeaSales, String>, day: Signal<i64>) -> View {
    view()
        .gap_2()
        .child(when(
            move || sales.loading() && sales.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || sales.error().is_some(),
            move || {
                crate::ui::failure(move || {
                    sales.error().map(SharedString::from).unwrap_or_default()
                })
            },
        ))
        .child(when(move || sales.get().is_some(), move || tea_sales_body(sales, day)))
}

fn tea_sales_body(sales: Resource<TeaSales, String>, day: Signal<i64>) -> View {
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("activities.salesPrev"), ButtonVariant::Quiet)
                        .disabled(move || {
                            sales.get().is_none_or(|board| board.day.saturating_sub(86_400) < 1)
                        })
                        .on_press(move || {
                            if let Some(board) = sales.get() {
                                day.set(board.day.saturating_sub(86_400));
                            }
                        }),
                )
                .child(
                    button(move || text_key("activities.salesNext"), ButtonVariant::Quiet)
                        .disabled(move || sales.get().is_none_or(|board| board.day >= board.today))
                        .on_press(move || {
                            if let Some(board) = sales.get() {
                                let next = board.day.saturating_add(86_400);
                                day.set(if next >= board.today { 0 } else { next });
                            }
                        }),
                ),
        )
        .child(
            text(move || {
                let board = sales.get();
                let token = board.as_ref().map(|board| board.token).unwrap_or(0);
                let coin = board.as_ref().map(|board| board.coin).unwrap_or(0);
                let cups = board.as_ref().map(|board| board.cups).unwrap_or(0);
                SharedString::from(format!(
                    "{} {token} · {} {coin} · {} {cups}",
                    text_key("activities.salesToken"),
                    text_key("activities.salesCoin"),
                    text_key("activities.salesCups")
                ))
            })
            .text_sm(),
        )
        .child(when(
            move || sales.get().is_some_and(|board| board.hires.is_empty()),
            move || notice(move || text_key("activities.salesEmpty")),
        ))
        .child(each(
            move || sales.get().map(|board| board.hires).unwrap_or_default(),
            |hire: &TeaHireLog| hire.id.clone(),
            move |hire, _| {
                let hire = hire.get_untracked();
                let name = SharedString::from(hire.name);
                let talent = SharedString::from(hire.talent);
                let when = SharedString::from(hire.when);
                let wage = hire.wage;
                text(move || {
                    let name = name.clone();
                    let talent = talent.clone();
                    let when = when.clone();
                    SharedString::from(format!(
                        "{name} · {talent} · {when} · {} {wage}",
                        text_key("activities.hireWage")
                    ))
                })
                .text_sm()
            },
        ))
}

fn tea_rank_panel(rank: Resource<TeaRank, String>, page_number: Signal<i64>) -> View {
    view()
        .gap_2()
        .child(when(
            move || rank.loading() && rank.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || rank.error().is_some(),
            move || {
                crate::ui::failure(move || rank.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(move || rank.get().is_some(), move || tea_rank_body(rank, page_number)))
}

fn tea_rank_body(rank: Resource<TeaRank, String>, page_number: Signal<i64>) -> View {
    view()
        .gap_2()
        .child(
            text(move || {
                let mine = rank.get().map(|board| board.mine).unwrap_or(0);
                SharedString::from(format!("{} {mine}", text_key("activities.rankMine")))
            })
            .text_sm(),
        )
        .child(when(
            move || rank.get().is_some_and(|board| board.rows.is_empty()),
            move || notice(move || text_key("activities.rankEmpty")),
        ))
        .child(each(
            move || rank.get().map(|board| board.rows).unwrap_or_default(),
            |row: &TeaRankRow| row.rank.to_string(),
            move |row, _| {
                let row = row.get_untracked();
                let place = row.rank;
                let shop = SharedString::from(row.shop);
                let owner = SharedString::from(row.owner);
                let revenue = row.revenue;
                text(move || {
                    let shop = shop.clone();
                    let owner = owner.clone();
                    SharedString::from(format!(
                        "{place} · {shop} · {owner} · {} {revenue}",
                        text_key("activities.revenue")
                    ))
                })
                .text_sm()
            },
        ))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("activities.rankPrev"), ButtonVariant::Quiet)
                        .disabled(move || rank.get().is_none_or(|board| board.page <= 1))
                        .on_press(move || {
                            let current = rank.get().map(|board| board.page).unwrap_or(1);
                            page_number.set(current.saturating_sub(1).max(1));
                        }),
                )
                .child(
                    button(move || text_key("activities.rankNext"), ButtonVariant::Quiet)
                        .disabled(move || rank.get().is_none_or(|board| !board.has_more))
                        .on_press(move || {
                            let current = rank.get().map(|board| board.page).unwrap_or(1);
                            page_number.set(current.saturating_add(1));
                        }),
                ),
        )
}

fn tea_sales(value: &Value) -> TeaSales {
    let logs = value.get("logs").unwrap_or(&Value::Null);
    let now = value.get("serverTime").and_then(Value::as_i64).unwrap_or(0);
    TeaSales {
        day: logs.get("day").and_then(Value::as_i64).unwrap_or(0),
        today: tea_today(now),
        token: logs.get("token").and_then(Value::as_i64).unwrap_or(0).max(0),
        coin: logs.get("coin").and_then(Value::as_i64).unwrap_or(0).max(0),
        cups: logs.get("cups").and_then(Value::as_i64).unwrap_or(0).max(0),
        hires: logs
            .get("hires")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .enumerate()
                    .filter_map(|(index, row)| tea_hire_log(index, row))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn tea_hire_log(index: usize, value: &Value) -> Option<TeaHireLog> {
    let shift = json_id(value.get("shiftId"));
    let name = value.pointer("/friend/name").and_then(Value::as_str).unwrap_or("");
    let hired = value.get("hireAt").and_then(Value::as_i64).unwrap_or(0);
    Some(TeaHireLog {
        id: if shift.is_empty() { index.to_string() } else { format!("{shift}:{index}") },
        name: if name.is_empty() { "好友".to_owned() } else { name.to_owned() },
        talent: value.get("talentName").and_then(Value::as_str).unwrap_or("").to_owned(),
        wage: value.pointer("/wages/finalWage").and_then(Value::as_i64).unwrap_or(0).max(0),
        when: if hired > 0 { share_log_time(hired) } else { String::new() },
    })
}

fn tea_rank(value: &Value) -> TeaRank {
    let rank = value.get("rank").unwrap_or(&Value::Null);
    TeaRank {
        page: rank.get("page").and_then(Value::as_i64).unwrap_or(1).max(1),
        mine: rank.get("myRank").and_then(Value::as_i64).unwrap_or(0).max(0),
        has_more: rank.get("hasMore").and_then(Value::as_bool).unwrap_or(false),
        rows: rank
            .get("ranks")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(tea_rank_row).collect())
            .unwrap_or_default(),
    }
}

fn tea_rank_row(value: &Value) -> Option<TeaRankRow> {
    let rank = value.get("rank").and_then(Value::as_i64).filter(|rank| *rank > 0)?;
    let shop = value.get("shopName").and_then(Value::as_str).unwrap_or("");
    let owner = value.pointer("/owner/name").and_then(Value::as_str).unwrap_or("");
    Some(TeaRankRow {
        rank,
        shop: if shop.is_empty() { "店铺".to_owned() } else { shop.to_owned() },
        owner: if owner.is_empty() { "好友".to_owned() } else { owner.to_owned() },
        revenue: value.get("totalRevenue").and_then(Value::as_i64).unwrap_or(0).max(0),
    })
}

fn tea_boost_row(
    tea: Resource<TeaShop, String>,
    count: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(when(
            move || tea.get().is_some_and(|shop| shop.sales_stock > 1),
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        button(move || SharedString::from("−"), ButtonVariant::Quiet)
                            .disabled(move || {
                                pending.get().is_some() || chosen_boost(count, tea) <= 1
                            })
                            .on_press(move || {
                                let next = chosen_boost(count, tea).saturating_sub(1).max(1);
                                count.set(next);
                            }),
                    )
                    .child(
                        text(move || SharedString::from(chosen_boost(count, tea).to_string()))
                            .text_sm(),
                    )
                    .child(
                        button(move || SharedString::from("+"), ButtonVariant::Quiet)
                            .disabled(move || {
                                let limit = boost_limit(tea);
                                pending.get().is_some() || chosen_boost(count, tea) >= limit
                            })
                            .on_press(move || {
                                let next = chosen_boost(count, tea)
                                    .saturating_add(1)
                                    .min(boost_limit(tea));
                                count.set(next);
                            }),
                    )
            },
        ))
        .child(
            text(move || {
                let cups = tea.get().map(|shop| shop.boost_cups).unwrap_or(0);
                SharedString::from(format!("{} {cups}", text_key("activities.boostCups")))
            })
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(
            button(move || text_key("activities.boost"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || chosen_boost(count, tea) <= 0)
                .on_press(move || {
                    let copies = chosen_boost(count, tea);
                    let item = tea.get().map(|shop| shop.sales_item).unwrap_or(0);
                    super::confirm::confirm(
                        "activities.confirmBoost.title",
                        "activities.confirmBoost.body",
                        "boost",
                        "activities.boost",
                        move |accepted| {
                            if accepted {
                                run_boost(item, copies, pending, failure, page, tea);
                            }
                        },
                    );
                }),
        )
}

fn boost_limit(tea: Resource<TeaShop, String>) -> i64 {
    tea.get().map(|shop| shop.sales_stock.max(0).min(9_999)).unwrap_or(0)
}

fn chosen_boost(count: Signal<i64>, tea: Resource<TeaShop, String>) -> i64 {
    let limit = boost_limit(tea);
    if limit < 1 {
        0
    } else {
        count.get().max(1).min(limit)
    }
}

fn run_upgrade(
    level: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) {
    if level <= 0 {
        return;
    }
    let Some(id) = begin("upgrade", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "upgrade",
                &json!({ "expectedLevel": level }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
    });
}

fn run_boost(
    item: i64,
    count: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) {
    if item <= 0 || count <= 0 {
        return;
    }
    let Some(id) = begin("boost", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(
                &context,
                &id,
                "boost",
                &json!({
                    "count": count,
                    "expectedCosts": [{ "id": item, "count": 1 }],
                }),
            )
            .await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
    });
}

fn run_tea(
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    tea: Resource<TeaShop, String>,
) {
    let Some(id) = begin("claim", page, pending, failure) else {
        return;
    };
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::milk_tea::operate(&context, &id, "claim", &json!({})).await
        })
        .await;
        finish(result, pending, failure);
        tea.refetch();
    });
}

fn begin(
    key: &str,
    page: ActivityPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
) -> Option<String> {
    if pending.get_untracked().is_some() {
        return None;
    }
    let id = page.account_id.get_untracked()?;
    pending.set(Some(key.to_owned()));
    failure.set(None);
    Some(id)
}

fn finish(
    result: Result<Value, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
) {
    if let Err(message) = result {
        failure.set(Some(message));
    }
    pending.set(None);
}

async fn load_shares(id: String) -> Result<Vec<ShareLine>, String> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for activity_id in SHARE_ACTIVITIES {
        let context = session::session().context.clone();
        let account_id = id.clone();
        match session::call(async move {
            qq_farm_app::share_reward::snapshot(&context, &account_id, activity_id).await
        })
        .await
        {
            Ok(value) => lines.push(share_from(activity_id, &value)),
            Err(error) => errors.push(error),
        }
    }
    if lines.is_empty() {
        Err(errors.join(" "))
    } else {
        Ok(lines)
    }
}

async fn load_share_logs(id: String, activity: String, tab: i64) -> Result<Vec<ShareLog>, String> {
    let context = session::session().context.clone();
    let value = session::call(async move {
        qq_farm_app::share_reward::operate(&context, &id, &activity, "logs", &json!({ "tab": tab }))
            .await
    })
    .await?;
    Ok(share_log_rows(&value))
}

fn share_log_panel(logs: Resource<Vec<ShareLog>, String>, tab: Signal<i64>) -> View {
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(share_log_tab(tab, 1, "activities.logMine"))
                .child(share_log_tab(tab, 0, "activities.logTheirs")),
        )
        .child(when(
            move || logs.loading() && logs.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || logs.error().is_some(),
            move || {
                crate::ui::failure(move || logs.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(
            move || logs.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("activities.logEmpty")),
        ))
        .child(each(
            move || logs.get().unwrap_or_default(),
            |log: &ShareLog| log.key.clone(),
            move |log, _| {
                let log = log.get_untracked();
                let kind = log.kind;
                let actor = SharedString::from(log.actor);
                let when = SharedString::from(log.when);
                let score = log.score;
                text(move || {
                    let actor = actor.clone();
                    let when = when.clone();
                    SharedString::from(format!(
                        "{} · {when} · +{score}",
                        share_log_label(kind, &actor)
                    ))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
}

fn share_log_tab(tab: Signal<i64>, value: i64, label: &'static str) -> impl IntoElement {
    button(move || text_key(label), ButtonVariant::Quiet)
        .selected(move || tab.get() == value)
        .on_press(move || tab.set(value))
}

fn share_log_rows(value: &Value) -> Vec<ShareLog> {
    value
        .pointer("/result/logs")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(share_log_from).collect())
        .unwrap_or_default()
}

fn share_log_from(value: &Value) -> Option<ShareLog> {
    let kind = json_i64(value, "kind");
    let seq = value
        .get("seq")
        .map(|item| {
            item.as_i64()
                .map(|number| number.to_string())
                .unwrap_or_else(|| item.as_str().unwrap_or("").to_owned())
        })
        .filter(|seq| !seq.is_empty())?;
    let actor = value.pointer("/actor/name").and_then(Value::as_str).unwrap_or("").to_owned();
    Some(ShareLog {
        key: format!("{seq}:{kind}"),
        kind,
        score: json_i64(value, "score").max(0),
        when: share_log_time(json_i64(value, "createdAt")),
        actor,
    })
}

fn json_i64(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(json_number).unwrap_or(0)
}

fn json_number(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn share_log_time(seconds: i64) -> String {
    chrono::FixedOffset::east_opt(8 * 3600)
        .and_then(|zone| {
            chrono::DateTime::from_timestamp(seconds, 0)
                .map(|time| time.with_timezone(&zone).format("%m-%d %H:%M").to_string())
        })
        .unwrap_or_default()
}

fn share_log_label(kind: i64, actor: &str) -> String {
    let name = if actor.is_empty() {
        text_key("activities.logFriend").to_string()
    } else {
        actor.to_owned()
    };
    match kind {
        3 => text_key("activities.logShare").to_string(),
        4 => text_key("activities.logDaily").to_string(),
        1 => format!(
            "{}{name}{}",
            text_key("activities.logTookPrefix"),
            text_key("activities.logTookSuffix")
        ),
        2 => format!("{name}{}", text_key("activities.logGiven")),
        _ => text_key("activities.logOther").to_string(),
    }
}

fn solar_board(value: &Value) -> SolarBoard {
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .map(|events| events.iter().filter_map(solar_event_from).collect())
        .unwrap_or_default();
    let seasons = value
        .get("seasons")
        .and_then(Value::as_array)
        .map(|seasons| seasons.iter().filter_map(season_from).collect())
        .unwrap_or_default();
    SolarBoard { events, seasons }
}

fn solar_event_from(value: &Value) -> Option<SolarEvent> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    Some(SolarEvent {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        description: value.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
        question: value.get("question").and_then(Value::as_str).unwrap_or("").to_owned(),
        option_index: value
            .get("answerIndex")
            .and_then(Value::as_i64)
            .filter(|_| value.get("canAnswer").and_then(Value::as_bool).unwrap_or(false)),
    })
}

fn season_from(value: &Value) -> Option<SeasonLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let lit = value.get("litCount").and_then(Value::as_i64).unwrap_or(0);
    let total = value.get("totalCount").and_then(Value::as_i64).unwrap_or(0);
    Some(SeasonLine {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        progress: format!("{lit}/{total}"),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn share_from(id: &str, value: &Value) -> ShareLine {
    let claimed = value.get("claimedCount").and_then(Value::as_i64).unwrap_or(0);
    let limit = value.get("claimLimit").and_then(Value::as_i64).unwrap_or(0);
    let title = value.get("title").and_then(Value::as_str).unwrap_or("");
    ShareLine {
        id: id.to_owned(),
        title: if title.is_empty() { id.to_owned() } else { title.to_owned() },
        score: value.get("score").and_then(Value::as_i64).unwrap_or(0),
        daily: format!("{claimed}/{limit}"),
        daily_reward: value.get("dailyReward").and_then(Value::as_i64).unwrap_or(0).max(0),
        first_share_reward: value
            .get("firstShareReward")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0),
        pool: format!(
            "{}/{}",
            value.get("poolClaimedCount").and_then(Value::as_i64).unwrap_or(0).max(0),
            value.get("poolClaimLimit").and_then(Value::as_i64).unwrap_or(0).max(0),
        ),
        can_daily: value.get("canClaimDaily").and_then(Value::as_bool).unwrap_or(false),
        can_share: value.get("canShare").and_then(Value::as_bool).unwrap_or(false),
        can_milestones: value.get("canClaimMilestones").and_then(Value::as_bool).unwrap_or(false),
        milestones: value
            .get("milestones")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(share_tier).collect())
            .unwrap_or_default(),
    }
}

fn share_tier(value: &Value) -> Option<ShareTier> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|item| {
                    let name =
                        item.get("name").and_then(Value::as_str).filter(|name| !name.is_empty())?;
                    let count = item.get("count").and_then(Value::as_i64).unwrap_or(0).max(0);
                    Some(format!("{name} ×{count}"))
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    Some(ShareTier {
        id: id.to_owned(),
        threshold: value.get("threshold").and_then(Value::as_i64).unwrap_or(0).max(0),
        state_key: share_tier_state(value.get("state").and_then(Value::as_i64).unwrap_or(0)),
        rewards,
    })
}

fn share_tier_state(state: i64) -> &'static str {
    match state {
        2 => "activities.tierReady",
        3 => "activities.tierClaimed",
        _ => "activities.tierLocked",
    }
}

fn tea_shop(shop_page: &Value, exchange_page: &Value, workshop_page: &Value) -> TeaShop {
    let summary = shop_page.get("summary");
    let shop = shop_page.get("shop");
    let name = summary.and_then(|item| item.get("shopName")).and_then(Value::as_str).unwrap_or("");
    let shop_level = shop.and_then(|item| item.get("level")).and_then(Value::as_i64).unwrap_or(0);
    let summary_level =
        summary.and_then(|item| item.get("level")).and_then(Value::as_i64).unwrap_or(0);
    let sales_item =
        shop.and_then(|item| item.get("salesItemId")).and_then(Value::as_i64).unwrap_or(0);
    let sales_stock = shop_page
        .get("balances")
        .and_then(|balances| balances.get(sales_item.to_string()))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let pending = shop
        .and_then(|item| item.get("pendingRewards"))
        .and_then(Value::as_array)
        .is_some_and(|rewards| {
            rewards
                .iter()
                .any(|reward| reward.get("count").and_then(Value::as_i64).unwrap_or(0) > 0)
        });
    TeaShop {
        name: name.to_owned(),
        level: if summary_level > 0 { summary_level } else { shop_level },
        revenue: shop
            .and_then(|item| item.get("totalRevenue"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        settling: shop
            .and_then(|item| item.get("isSettling"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        can_claim: pending,
        can_upgrade: summary
            .and_then(|item| item.get("canUpgrade"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        sales_item,
        sales_stock,
        boost_cups: shop
            .and_then(|item| item.get("boostCupsPerUse"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0),
        opened: summary
            .and_then(|item| item.get("shopOpenAt"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            > 0,
        suggestions: shop_page
            .get("recommendedNames")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.as_str())
                    .map(str::trim)
                    .filter(|name| rename_allowed("", name, false))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        goods: exchange_page
            .get("goods")
            .and_then(Value::as_array)
            .map(|rows| {
                let balances = exchange_page.get("balances").unwrap_or(&Value::Null);
                rows.iter().filter_map(|row| tea_good_from(row, balances)).collect()
            })
            .unwrap_or_default(),
        recipes: tea_recipes(workshop_page),
        fruits: tea_fruits(workshop_page),
    }
}

fn tea_recipes(value: &Value) -> Vec<TeaRecipe> {
    value
        .get("recipes")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(tea_recipe).collect())
        .unwrap_or_default()
}

fn tea_recipe(value: &Value) -> Option<TeaRecipe> {
    if value.get("unlocked").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let name = value.get("recipeName").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        return None;
    }
    Some(TeaRecipe {
        id: value.get("recipeId").and_then(Value::as_i64).unwrap_or(0),
        name: name.to_owned(),
        stock: value.get("stockCups").and_then(Value::as_i64).unwrap_or(0).max(0),
    })
}

fn tea_fruits(value: &Value) -> Vec<TeaFruit> {
    value
        .get("fruits")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(tea_fruit_from).collect())
        .unwrap_or_default()
}

fn tea_fruit_from(value: &Value) -> Option<TeaFruit> {
    if value.get("locked").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let uid = value.get("uid").and_then(Value::as_str).filter(|uid| !uid.is_empty())?;
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    if count <= 0 {
        return None;
    }
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    Some(TeaFruit {
        uid: uid.to_owned(),
        name: if name.is_empty() { format!("果实 {uid}") } else { name.to_owned() },
        count,
    })
}

fn tea_good_from(value: &Value, balances: &Value) -> Option<TeaGood> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let costs = value
        .get("costs")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|cost| {
                    Some((
                        cost.get("id").and_then(Value::as_i64)?,
                        cost.get("count").and_then(Value::as_i64).unwrap_or(0),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let price = value
        .get("costs")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|cost| {
                    let name = cost.get("name").and_then(Value::as_str).unwrap_or("");
                    let count = cost.get("count").and_then(Value::as_i64).unwrap_or(0);
                    format!("{name} {count}")
                })
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default();
    let period = value.get("limitPeriod").and_then(Value::as_str).unwrap_or("");
    let remaining = value.get("remaining").and_then(Value::as_i64);
    let priced = costs
        .iter()
        .map(|(item_id, count)| crate::purchase_count::PurchaseCost {
            count: *count,
            balance: balances.get(item_id.to_string()).and_then(Value::as_i64),
        })
        .collect::<Vec<_>>();
    Some(TeaGood {
        id: id.to_owned(),
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        price,
        limit_key: tea_limit_key(period),
        remaining,
        limit_max: value.get("purchaseLimit").and_then(Value::as_i64).unwrap_or(0),
        available: value.get("available").and_then(Value::as_bool).unwrap_or(false),
        costs,
        max_count: crate::purchase_count::exchange_max(remaining, &priced),
    })
}

fn tea_chosen(counts: Signal<Vec<(String, i64)>>, id: &str, max_count: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(goods_id, _)| goods_id == id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    stored.clamp(1, max_count.max(1))
}

fn tea_set(counts: Signal<Vec<(String, i64)>>, id: &str, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(goods_id, _)| goods_id == id) {
        row.1 = count;
    } else {
        rows.push((id.to_owned(), count));
    }
    counts.set(rows);
}

fn tea_count_stepper(
    id: String,
    max_count: i64,
    counts: Signal<Vec<(String, i64)>>,
    pending: Signal<Option<String>>,
) -> impl IntoElement {
    let less_label = id.clone();
    let less_press = id.clone();
    let more_label = id.clone();
    let more_press = id.clone();
    let shown = id;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some() || tea_chosen(counts, &less_label, max_count) <= 1
                })
                .on_press(move || {
                    let next = tea_chosen(counts, &less_press, max_count).saturating_sub(1).max(1);
                    tea_set(counts, &less_press, next);
                }),
        )
        .child(
            text(move || SharedString::from(tea_chosen(counts, &shown, max_count).to_string()))
                .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some()
                        || tea_chosen(counts, &more_label, max_count) >= max_count
                })
                .on_press(move || {
                    let next =
                        tea_chosen(counts, &more_press, max_count).saturating_add(1).min(max_count);
                    tea_set(counts, &more_press, next);
                }),
        )
}

fn tea_limit_key(period: &str) -> Option<&'static str> {
    Some(match period {
        "daily" => "mall.limitDaily",
        "weekly" => "mall.limitWeekly",
        "monthly" => "mall.limitMonthly",
        "permanent" => "mall.limitForever",
        "unknown" => "mall.limit",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ranch_rows, share_from, solar_board, tea_shop};

    #[test]
    fn solar_keeps_claimable_events_and_seasons() {
        let board = solar_board(&json!({
            "events": [
                { "id": 401, "name": "重阳", "canClaim": true, "canAnswer": true, "question": "问", "answerIndex": 0 },
                { "id": 0, "name": "drop" }
            ],
            "seasons": [{ "id": 5, "name": "秋", "litCount": 2, "totalCount": 6, "canClaim": true }]
        }));
        assert_eq!(board.events.len(), 1);
        assert!(board.events[0].can_claim);
        assert_eq!(board.events[0].option_index, Some(0));
        assert_eq!(board.seasons[0].progress, "2/6");
        assert!(board.seasons[0].can_claim);
    }

    #[test]
    fn closed_quiz_has_no_answer() {
        let board = solar_board(&json!({
            "events": [{ "id": 1, "name": "立冬", "canClaim": false, "canAnswer": false, "answerIndex": 1 }]
        }));
        assert!(board.events[0].option_index.is_none());
    }

    #[test]
    fn share_and_tea_read_claim_gates() {
        let share = share_from(
            "2026092501",
            &json!({
                "title": "快乐不独享",
                "score": 12,
                "claimedCount": 1,
                "claimLimit": 3,
                "canClaimDaily": true,
                "canShare": false,
                "canClaimMilestones": true,
                "dailyReward": 5,
                "firstShareReward": 10,
                "poolClaimedCount": 2,
                "poolClaimLimit": 8,
                "milestones": [
                    { "id": "1", "threshold": 20, "state": 2, "rewards": [{ "name": "金币", "count": 100 }] },
                    { "id": "2", "threshold": 50, "state": 3, "rewards": [{ "name": "", "count": 1 }] },
                    { "id": "", "threshold": 1, "state": 0 }
                ]
            }),
        );
        assert_eq!(share.daily, "1/3");
        assert!(share.can_daily);
        assert!(!share.can_share);
        assert_eq!(share.daily_reward, 5);
        assert_eq!(share.first_share_reward, 10);
        assert_eq!(share.pool, "2/8");
        assert_eq!(share.milestones.len(), 2);
        assert_eq!(share.milestones[0].state_key, "activities.tierReady");
        assert_eq!(share.milestones[0].rewards, "金币 ×100");
        assert_eq!(share.milestones[1].state_key, "activities.tierClaimed");
        assert!(share.milestones[1].rewards.is_empty());
        let logs = super::share_log_rows(&json!({
            "result": {
                "logs": [
                    { "seq": "9", "kind": 1, "score": "4", "createdAt": 1700000000, "actor": { "name": "阿宁" } },
                    { "seq": "", "kind": 3, "score": 1 }
                ]
            }
        }));
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].kind, 1);
        assert_eq!(logs[0].score, 4);
        assert_eq!(logs[0].actor, "阿宁");
        assert_eq!(logs[0].when, "11-15 06:13");
        let tea = tea_shop(
            &json!({
                "summary": {
                    "shopName": "青梅",
                    "level": 2,
                    "canUpgrade": true,
                    "shopOpenAt": 10
                },
                "recommendedNames": ["青梅奶茶", "一", ""],
                "balances": { "1042": 5 },
                "shop": {
                    "level": 2,
                    "totalRevenue": 80,
                    "isSettling": false,
                    "salesItemId": 1042,
                    "boostCupsPerUse": 3,
                    "pendingRewards": [{ "count": 4 }]
                }
            }),
            &json!({
                "balances": { "1001": 35 },
                "goods": [{
                    "id": "9",
                    "name": "种子",
                    "available": true,
                    "remaining": 2,
                    "purchaseLimit": 2,
                    "limitPeriod": "daily",
                    "costs": [{ "id": 1001, "count": 10, "name": "金币" }]
                }]
            }),
            &json!({}),
        );
        assert_eq!(tea.name, "青梅");
        assert_eq!(tea.level, 2);
        assert!(tea.can_claim);
        assert!(tea.can_upgrade);
        assert!(tea.opened);
        assert_eq!(tea.suggestions, vec!["青梅奶茶".to_owned()]);
        assert_eq!(tea.sales_item, 1042);
        assert_eq!(tea.sales_stock, 5);
        assert_eq!(tea.boost_cups, 3);
        assert_eq!(tea.goods[0].limit_key, Some("mall.limitDaily"));
        assert_eq!(tea.goods[0].max_count, 2);
        assert_eq!(tea.goods[0].price, "金币 10");
        assert_eq!(tea.goods[0].costs, vec![(1001, 10)]);
        let workshop = tea_shop(
            &json!({}),
            &json!({}),
            &json!({
                "recipes": [
                    { "recipeId": 3, "recipeName": "青梅", "unlocked": true, "stockCups": 2 },
                    { "recipeId": 4, "recipeName": "未解锁", "unlocked": false, "stockCups": 1 }
                ],
                "fruits": [
                    { "uid": "8", "name": "梅子", "count": 6, "locked": false },
                    { "uid": "9", "name": "锁定", "count": 1, "locked": true },
                    { "uid": "10", "name": "空", "count": 0, "locked": false }
                ]
            }),
        );
        assert_eq!(workshop.recipes.len(), 1);
        assert_eq!(workshop.recipes[0].stock, 2);
        assert_eq!(workshop.fruits.len(), 1);
        assert_eq!(workshop.fruits[0].uid, "8");
        assert_eq!(workshop.fruits[0].count, 6);
        let hire = super::tea_hire_board(&json!({
            "serverTime": 100,
            "settling": false,
            "durations": [{ "durationId": 1, "durationSec": 14400 }],
            "hire": {
                "wageTotal": 12,
                "friends": [{
                    "friend": { "gid": 7, "name": "阿宁" },
                    "talentName": "稳",
                    "state": 0,
                    "rehireAvailableAt": 90
                }],
                "robots": [{
                    "friend": { "gid": 8, "name": "忙" },
                    "talentName": "快",
                    "state": 1,
                    "rehireAvailableAt": 0
                }]
            }
        }));
        assert_eq!(hire.wage, 12);
        assert_eq!(hire.durations[0].hours, 4);
        assert_eq!(hire.candidates.len(), 2);
        assert!(hire.candidates[0].available);
        assert!(!hire.candidates[1].available);
        let fund = super::tea_fund(&json!({
            "fund": {
                "unlocked": true,
                "costs": [{ "id": 1041, "count": 20, "name": "奶茶券" }],
                "tiers": [
                    {
                        "id": 1,
                        "state": 2,
                        "conditions": [{ "target": 3 }],
                        "rewards": [{ "name": "金币", "count": 50 }]
                    },
                    { "id": 2, "state": 3, "conditions": [{ "target": 5 }], "rewards": [] }
                ]
            }
        }));
        assert!(fund.unlocked);
        assert!(super::fund_claimable(&fund));
        assert_eq!(fund.costs[0].count, 20);
        assert_eq!(fund.tiers[0].level, 3);
        assert_eq!(fund.tiers[0].rewards, "金币 ×50");
        assert_eq!(super::fund_state_key(false, 2), "activities.fundLocked");
        assert_eq!(super::fund_state_key(true, 2), "activities.tierReady");
        let work = super::tea_work(&json!({
            "serverTime": 200,
            "work": {
                "interactDone": false,
                "myShift": {
                    "shiftId": 7,
                    "shopName": "青梅",
                    "employer": { "name": "阿宁" },
                    "endAt": 500,
                    "interactCount": 1,
                    "interactLimit": 3
                },
                "slips": [{
                    "shiftId": 6,
                    "employer": { "name": "阿宁" },
                    "wageStatus": 1,
                    "wages": { "finalWage": 12 }
                }]
            },
            "interactions": [
                { "interactionId": 1, "name": "招呼", "description": "打个招呼", "ready": true },
                { "interactionId": 4, "name": "", "ready": false }
            ]
        }));
        assert!(super::shift_open(&work));
        assert_eq!(work.shift.unwrap().id, "7");
        assert!(work.slips[0].claimable);
        assert_eq!(work.slips[0].wage, 12);
        assert_eq!(work.actions.len(), 1);
        assert_eq!(work.actions[0].name, "招呼");
        let settings = super::tea_settings(&json!({
            "serverTime": 100,
            "summary": {
                "shopName": "青梅",
                "renameCdUntilAt": 300,
                "hireDisabled": true
            }
        }));
        assert!(settings.hire_disabled);
        assert!(!super::rename_allowed(&settings.name, "青梅", settings.rename_at > settings.now));
        assert!(!super::rename_allowed("青梅", "一", false));
        assert!(super::rename_allowed("青梅", "新店", false));
        assert_eq!(super::tea_today(86_400), 57_600);
        let sales = super::tea_sales(&json!({
            "serverTime": 86400,
            "logs": {
                "day": 57600,
                "token": 3,
                "coin": 40,
                "cups": 2,
                "hires": [{
                    "shiftId": 6,
                    "friend": { "name": "阿宁" },
                    "talentName": "稳",
                    "hireAt": 1700000000,
                    "wages": { "finalWage": 8 }
                }]
            }
        }));
        assert_eq!(sales.token, 3);
        assert_eq!(sales.today, sales.day);
        assert_eq!(sales.hires[0].wage, 8);
        assert_eq!(sales.hires[0].when, "11-15 06:13");
        let rank = super::tea_rank(&json!({
            "rank": {
                "page": 1,
                "myRank": 4,
                "hasMore": true,
                "ranks": [{
                    "rank": 1,
                    "shopName": "青梅",
                    "owner": { "name": "阿宁" },
                    "totalRevenue": 90
                }]
            }
        }));
        assert_eq!(rank.mine, 4);
        assert!(rank.has_more);
        assert_eq!(rank.rows[0].shop, "青梅");
        assert_eq!(rank.rows[0].revenue, 90);
    }

    #[test]
    fn ranch_directory_reads_status_without_scaling_time() {
        let rows = ranch_rows(&json!({
            "serverTime": 1700000050,
            "activities": [{
                "id": "100",
                "name": "牧草节",
                "startTime": 1700000000,
                "endTime": 1700090000,
                "status": "active",
                "when": "11-15 06:13 – 11-16 07:13"
            }]
        }));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status_key, "activities.active");
        assert_eq!(rows[0].when, "11-15 06:13 – 11-16 07:13");
    }
}
