use nsh_core::prelude::*;
use serde_json::{json, Value};

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

const SHARE_ACTIVITIES: [&str; 2] = ["2026092501", "2026110104"];

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
}

impl Shelf {
    const ALL: [Self; 5] = [Self::Solar, Self::Share, Self::Tea, Self::Muge, Self::Ranch];

    const fn id(self) -> &'static str {
        match self {
            Self::Solar => "solar",
            Self::Share => "share",
            Self::Tea => "tea",
            Self::Muge => "muge",
            Self::Ranch => "ranch",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Solar => "activities.solar",
            Self::Share => "activities.share",
            Self::Tea => "activities.tea",
            Self::Muge => "activities.muge",
            Self::Ranch => "activities.ranch",
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
    can_daily: bool,
    can_share: bool,
    can_milestones: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TeaShop {
    name: String,
    level: i64,
    revenue: i64,
    settling: bool,
    can_claim: bool,
    goods: Vec<TeaGood>,
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
    let tea_counts = signal(Vec::<(String, i64)>::new());
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
            let exchange = session::call(async move {
                qq_farm_app::milk_tea::snapshot(&context, &id, "exchange", &json!({})).await
            })
            .await?;
            Ok(tea_shop(&shop, &exchange))
        },
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
            move || share_loaded(shares, pending, failure, page),
        ))
        .child(when(
            move || shelf.get() == Shelf::Tea && running(page),
            move || tea_loaded(tea, pending, failure, page, tea_counts),
        ))
        .child(when(
            move || shelf.get() == Shelf::Muge && running(page),
            move || super::muge::loaded(muge_board, pending, failure, page),
        ))
        .child(when(
            move || shelf.get() == Shelf::Ranch && running(page),
            move || ranch_loaded(ranch_directory),
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
                            share_line(share.get_untracked(), pending, failure, page, shares)
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
) -> View {
    let title = SharedString::from(format!("{} {}", share.title, share.score));
    let daily = SharedString::from(share.daily.clone());
    let id = share.id.clone();
    let can_daily = share.can_daily;
    let can_share = share.can_share;
    let can_milestones = share.can_milestones;
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
}

fn tea_loaded(
    tea: Resource<TeaShop, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    counts: Signal<Vec<(String, i64)>>,
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
            move || tea.get().is_some(),
            move || tea_body(tea, pending, failure, page, counts),
        ))
}

fn tea_body(
    tea: Resource<TeaShop, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    counts: Signal<Vec<(String, i64)>>,
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
        .child(each(
            move || tea.get().map(|shop| shop.goods).unwrap_or_default(),
            |good: &TeaGood| good.id.clone(),
            move |good, _| tea_good(good.get_untracked(), pending, failure, page, tea, counts),
        ))
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
        can_daily: value.get("canClaimDaily").and_then(Value::as_bool).unwrap_or(false),
        can_share: value.get("canShare").and_then(Value::as_bool).unwrap_or(false),
        can_milestones: value.get("canClaimMilestones").and_then(Value::as_bool).unwrap_or(false),
    }
}

fn tea_shop(shop_page: &Value, exchange_page: &Value) -> TeaShop {
    let summary = shop_page.get("summary");
    let shop = shop_page.get("shop");
    let name = summary.and_then(|item| item.get("shopName")).and_then(Value::as_str).unwrap_or("");
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
        level: shop.and_then(|item| item.get("level")).and_then(Value::as_i64).unwrap_or(0),
        revenue: shop
            .and_then(|item| item.get("totalRevenue"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        settling: shop
            .and_then(|item| item.get("isSettling"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        can_claim: pending,
        goods: exchange_page
            .get("goods")
            .and_then(Value::as_array)
            .map(|rows| {
                let balances = exchange_page.get("balances").unwrap_or(&Value::Null);
                rows.iter().filter_map(|row| tea_good_from(row, balances)).collect()
            })
            .unwrap_or_default(),
    }
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
                "canClaimMilestones": true
            }),
        );
        assert_eq!(share.daily, "1/3");
        assert!(share.can_daily);
        assert!(!share.can_share);
        let tea = tea_shop(
            &json!({
                "summary": { "shopName": "青梅" },
                "shop": {
                    "level": 2,
                    "totalRevenue": 80,
                    "isSettling": false,
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
        );
        assert_eq!(tea.name, "青梅");
        assert_eq!(tea.level, 2);
        assert!(tea.can_claim);
        assert_eq!(tea.goods[0].limit_key, Some("mall.limitDaily"));
        assert_eq!(tea.goods[0].max_count, 2);
        assert_eq!(tea.goods[0].price, "金币 10");
        assert_eq!(tea.goods[0].costs, vec![(1001, 10)]);
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
