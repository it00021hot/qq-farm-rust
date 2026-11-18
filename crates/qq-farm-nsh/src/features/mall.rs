use nsh_core::prelude::*;
use qq_farm_core::services::commerce::ExpectedPrice;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const COMMON_SLOT: i32 = 1;
const MEMBER_SLOT: i32 = 4;

#[derive(Clone, Copy)]
pub(crate) struct MallPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shelf {
    Common,
    Member,
    Mystery,
}

impl Shelf {
    const ALL: [Self; 3] = [Self::Common, Self::Member, Self::Mystery];

    const fn id(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Member => "member",
            Self::Mystery => "mystery",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Common => "mall.common",
            Self::Member => "mall.member",
            Self::Mystery => "mall.mystery",
        }
    }

    const fn slot(self) -> Option<i32> {
        match self {
            Self::Common => Some(COMMON_SLOT),
            Self::Member => Some(MEMBER_SLOT),
            Self::Mystery => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct GoodsLine {
    id: i64,
    name: String,
    contents: String,
    price: String,
    price_id: i64,
    price_count: i64,
    purchasable: bool,
    status_key: Option<&'static str>,
    limit_key: Option<&'static str>,
    limit_bought: i64,
    limit_max: i64,
    balance: Option<i64>,
    limit_remaining: Option<i64>,
    original: i64,
    discount: String,
    discount_seconds: i64,
    free: bool,
    discounted: bool,
    haystack: String,
    short: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct MysteryOffer {
    id: i64,
    reward: String,
    price: String,
    discount_percent: i64,
    original: i64,
    balance: Option<i64>,
    leaves: String,
    purchasable: bool,
    block_key: &'static str,
}

pub(crate) fn page(page: MallPage) -> impl IntoElement {
    let shelf = signal(Shelf::Common);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let summary = signal(None::<String>);
    let catalog = resource(
        move || shelf.get().slot().and_then(|slot| running_account(page).map(|id| (id, slot))),
        |(id, slot), _| async move {
            let context = session::session().context.clone();
            session::call(async move {
                qq_farm_app::commerce::mall_catalog(&context, &id, Some(slot), Some(0)).await
            })
            .await
        },
    );
    let counts = signal(Vec::<(i64, i64)>::new());
    let goods_filter = signal("all".to_owned());
    let goods_query = signal(String::new());
    let mystery = resource(
        move || (shelf.get() == Shelf::Mystery).then(|| running_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::commerce::mystery_shop(&context, &id).await })
                .await
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("mall.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(shelf_switch(shelf))
        .child(failure_line(failure))
        .child(when(
            move || summary.get().is_some(),
            move || notice(move || summary.get().map(SharedString::from).unwrap_or_default()),
        ))
        .child(gate_notice(page.account_id, page.rows, "mall.needRunning"))
        .child(when(
            move || {
                shelf.get() != Shelf::Mystery
                    && running(page)
                    && catalog.loading()
                    && catalog.get().is_none()
            },
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || shelf.get() != Shelf::Mystery && running(page) && catalog.error().is_some(),
            move || load_error(catalog),
        ))
        .child(when(
            move || shelf.get() != Shelf::Mystery && running(page) && catalog.get().is_some(),
            move || {
                goods_body(
                    page,
                    shelf,
                    pending,
                    failure,
                    summary,
                    catalog,
                    counts,
                    goods_filter,
                    goods_query,
                )
            },
        ))
        .child(when(
            move || {
                shelf.get() == Shelf::Mystery
                    && running(page)
                    && mystery.loading()
                    && mystery.get().is_none()
            },
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || shelf.get() == Shelf::Mystery && running(page) && mystery.error().is_some(),
            move || mystery_error(mystery),
        ))
        .child(when(
            move || shelf.get() == Shelf::Mystery && running(page) && mystery.get().is_some(),
            move || mystery_body(page, pending, failure, summary, mystery),
        ))
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

fn running(page: MallPage) -> bool {
    page.account_id
        .get()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| account_is_running(&rows, &id))
}

fn running_account(page: MallPage) -> Option<String> {
    running(page).then(|| page.account_id.get()).flatten()
}

fn load_error(catalog: Resource<Value, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            catalog.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || catalog.refetch()),
        )
}

fn mystery_error(mystery: Resource<Value, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            mystery.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || mystery.refetch()),
        )
}

fn goods_body(
    page: MallPage,
    shelf: Signal<Shelf>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    catalog: Resource<Value, String>,
    counts: Signal<Vec<(i64, i64)>>,
    filter: Signal<String>,
    query: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || catalog.refetch()),
        )
        .child(when(
            move || {
                shelf_note(&catalog.get().unwrap_or(Value::Null), shelf.get() == Shelf::Member)
                    != ShelfNote::Quiet
            },
            move || {
                text(move || {
                    shelf_copy(shelf.get() == Shelf::Member, &catalog.get().unwrap_or(Value::Null))
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
                .child(goods_filter_button(filter, "all", "mall.filterAll"))
                .child(goods_filter_button(filter, "free", "mall.filterFree"))
                .child(goods_filter_button(filter, "discount", "mall.filterDiscount"))
                .child(goods_filter_button(filter, "fertilizer", "mall.filterFertilizer"))
                .child(goods_filter_button(filter, "pet", "mall.filterPet")),
        )
        .child(when(
            move || !currency_line(&catalog.get().unwrap_or(Value::Null)).is_empty(),
            move || {
                text(move || {
                    SharedString::from(currency_line(&catalog.get().unwrap_or(Value::Null)))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(field(
            move || query.get(),
            move || text_key("mall.search").to_string(),
            move |text| query.set(text.to_owned()),
        ))
        .child(when(
            move || goods_lines(&catalog.get().unwrap_or(Value::Null)).is_empty(),
            move || notice(move || text_key("mall.empty")),
        ))
        .child(when(
            move || {
                let rows = goods_lines(&catalog.get().unwrap_or(Value::Null));
                !rows.is_empty()
                    && !rows.iter().any(|goods| goods_matches(goods, &filter.get(), &query.get()))
            },
            move || notice(move || text_key("mall.noMatch")),
        ))
        .child(each(
            move || {
                let selected = filter.get();
                let keyword = query.get();
                goods_lines(&catalog.get().unwrap_or(Value::Null))
                    .into_iter()
                    .filter(|goods| goods_matches(goods, &selected, &keyword))
                    .collect::<Vec<_>>()
            },
            |goods: &GoodsLine| goods.id.to_string(),
            move |goods, _| {
                goods_line(
                    goods.get_untracked(),
                    shelf.get_untracked().slot().unwrap_or(COMMON_SLOT),
                    page,
                    pending,
                    failure,
                    summary,
                    catalog,
                    counts,
                )
            },
        ))
}

fn goods_line(
    goods: GoodsLine,
    slot: i32,
    page: MallPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    catalog: Resource<Value, String>,
    counts: Signal<Vec<(i64, i64)>>,
) -> View {
    let name = SharedString::from(goods.name.clone());
    let contents = SharedString::from(goods.contents.clone());
    let contents_shown = !contents.is_empty();
    let price = SharedString::from(goods.price.clone());
    let status_key = goods.status_key;
    let purchasable = goods.purchasable;
    let limit_key = goods.limit_key;
    let limit_bought = goods.limit_bought;
    let limit_max = goods.limit_max;
    let original = goods.original;
    let discount = SharedString::from(goods.discount.clone());
    let discount_shown = !discount.is_empty();
    let discount_seconds = goods.discount_seconds;
    let short = goods.short;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().font_weight(FontWeight::MEDIUM).flex_1())
        .child(when(move || contents_shown, {
            let contents = contents;
            move || {
                text(contents.clone())
                    .text_sm()
                    .truncate()
                    .text_color(move || nsh_base::theme().muted_foreground)
            }
        }))
        .child(text(price).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || original > 0,
            move || {
                text(move || {
                    SharedString::from(
                        text_key("mall.original").replace("{count}", &original.to_string()),
                    )
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(move || discount_shown, {
            let discount = discount;
            move || text(discount.clone()).text_sm()
        }))
        .child(when(
            move || discount_seconds > 0,
            move || {
                text(move || SharedString::from(discount_left(discount_seconds)))
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(move || short, move || text(move || text_key("mall.short")).text_sm()))
        .child(count_stepper(goods.id, goods.max_count(), counts, pending))
        .child(when(
            move || limit_key.is_some(),
            move || {
                text(move || {
                    let label = text_key(limit_key.unwrap_or("mall.limit"));
                    SharedString::from(format!("{label} {limit_bought}/{limit_max}"))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            move || status_key.is_some(),
            move || text(move || text_key(status_key.unwrap_or("mall.unavailable"))).text_sm(),
        ))
        .child(when(move || purchasable, {
            let goods = goods.clone();
            move || {
                let key = goods.id.to_string();
                let buying = goods.clone();
                button(
                    move || {
                        if pending.get().as_deref() == Some(key.as_str()) {
                            text_key("farm.working")
                        } else {
                            text_key("mall.buy")
                        }
                    },
                    ButtonVariant::Primary,
                )
                .disabled(move || pending.get().is_some() || short)
                .on_press(move || {
                    if short {
                        return;
                    }
                    buy_goods(
                        buying.clone(),
                        slot,
                        page,
                        pending,
                        failure,
                        summary,
                        catalog,
                        counts,
                    )
                })
            }
        }))
}

fn buy_goods(
    goods: GoodsLine,
    slot: i32,
    page: MallPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    catalog: Resource<Value, String>,
    counts: Signal<Vec<(i64, i64)>>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(account_id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(goods.id.to_string()));
    failure.set(None);
    summary.set(None);
    confirm::confirm(
        "mall.confirmBuy.title",
        "mall.confirmBuy.body",
        "buy",
        "mall.buy",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            let quantity = chosen_count(counts, goods.id, goods.max_count());
            spawn(async move {
                let context = session::session().context.clone();
                let goods_id = goods.id;
                let expected = Some(ExpectedPrice { id: goods.price_id, count: goods.price_count });
                let result = session::call(async move {
                    qq_farm_app::commerce::purchase_mall(
                        &context,
                        &account_id,
                        goods_id,
                        quantity,
                        Some(slot),
                        expected,
                    )
                    .await
                })
                .await;
                match result {
                    Ok(_) => summary.set(Some(text_key("bag.done").to_string())),
                    Err(message) => failure.set(Some(message)),
                }
                pending.set(None);
                catalog.refetch();
            });
        },
    );
}

fn mystery_body(
    page: MallPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    mystery: Resource<Value, String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || mystery.refetch()),
        )
        .child(when(
            move || mystery_offer(&mystery.get().unwrap_or(Value::Null)).is_none(),
            move || notice(move || text_key("mall.mysteryAway")),
        ))
        .child(each(
            move || {
                mystery_offer(&mystery.get().unwrap_or(Value::Null)).into_iter().collect::<Vec<_>>()
            },
            |offer: &MysteryOffer| offer.id.to_string(),
            move |offer, _| {
                let offer = offer.get_untracked();
                let reward = SharedString::from(offer.reward.clone());
                let price = SharedString::from(offer.price.clone());
                let key = offer.id.to_string();
                let purchasable = offer.purchasable;
                let discount_percent = offer.discount_percent;
                let original = offer.original;
                let balance = offer.balance;
                let leaves = SharedString::from(offer.leaves.clone());
                let leaves_shown = !leaves.is_empty();
                let block_key = offer.block_key;
                view()
                    .gap_1()
                    .py_2()
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(text(reward).text_sm().font_weight(FontWeight::MEDIUM).flex_1())
                            .child(text(price).text_sm())
                            .child(
                                button(
                                    move || {
                                        if pending.get().as_deref() == Some(key.as_str()) {
                                            text_key("farm.working")
                                        } else {
                                            text_key("mall.buy")
                                        }
                                    },
                                    ButtonVariant::Primary,
                                )
                                .disabled(move || pending.get().is_some() || !purchasable)
                                .on_press(move || {
                                    if purchasable {
                                        buy_mystery(
                                            offer.clone(),
                                            page,
                                            pending,
                                            failure,
                                            summary,
                                            mystery,
                                        );
                                    }
                                }),
                            ),
                    )
                    .child(when(
                        move || discount_percent > 0,
                        move || {
                            text(move || {
                                SharedString::from(
                                    text_key("mall.discount")
                                        .replace("{fold}", &discount_fold(discount_percent))
                                        .replace("{percent}", &discount_percent.to_string()),
                                )
                            })
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                        },
                    ))
                    .child(when(
                        move || original > 0 && discount_percent > 0,
                        move || {
                            text(move || {
                                SharedString::from(
                                    text_key("mall.original")
                                        .replace("{count}", &original.to_string()),
                                )
                            })
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                        },
                    ))
                    .child(when(
                        move || balance.is_some(),
                        move || {
                            text(move || {
                                SharedString::from(
                                    text_key("mall.balance")
                                        .replace("{count}", &balance.unwrap_or(0).to_string()),
                                )
                            })
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                        },
                    ))
                    .child(when(move || leaves_shown, {
                        let leaves = leaves;
                        move || {
                            let leaves = leaves.clone();
                            text(move || {
                                SharedString::from(
                                    text_key("mall.leaves").replace("{time}", leaves.as_ref()),
                                )
                            })
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                        }
                    }))
                    .child(when(
                        move || !block_key.is_empty(),
                        move || {
                            text(move || text_key(block_key))
                                .text_sm()
                                .text_color(move || nsh_base::theme().muted_foreground)
                        },
                    ))
            },
        ))
}

fn buy_mystery(
    offer: MysteryOffer,
    page: MallPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    mystery: Resource<Value, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(account_id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(offer.id.to_string()));
    failure.set(None);
    summary.set(None);
    confirm::confirm(
        "mall.confirmBuy.title",
        "mall.confirmBuy.body",
        "buy",
        "mall.buy",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            spawn(async move {
                let context = session::session().context.clone();
                let npc_id = offer.id.to_string();
                let result = session::call(async move {
                    qq_farm_app::commerce::purchase_mystery(&context, &account_id, &npc_id).await
                })
                .await;
                match result {
                    Ok(_) => summary.set(Some(text_key("bag.done").to_string())),
                    Err(message) => failure.set(Some(message)),
                }
                pending.set(None);
                mystery.refetch();
            });
        },
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShelfNote {
    Quiet,
    Refresh(i64),
    Svip(i64),
    NotSvip,
}

fn shelf_note(value: &Value, member: bool) -> ShelfNote {
    if member {
        if let Some(membership) = value.get("membership").filter(|item| !item.is_null()) {
            let days = membership.get("remainingDays").and_then(Value::as_i64).unwrap_or(0).max(0);
            return if membership.get("isSvip").and_then(Value::as_bool).unwrap_or(false) {
                ShelfNote::Svip(days)
            } else {
                ShelfNote::NotSvip
            };
        }
    }
    let seconds = value.get("refreshCountdown").and_then(Value::as_i64).unwrap_or(0);
    if seconds > 0 {
        ShelfNote::Refresh(seconds)
    } else {
        ShelfNote::Quiet
    }
}

fn shelf_copy(member: bool, value: &Value) -> String {
    match shelf_note(value, member) {
        ShelfNote::Quiet => String::new(),
        ShelfNote::NotSvip => text_key("mall.notSvip").to_string(),
        ShelfNote::Svip(days) => text_key("mall.svip").replace("{days}", &days.to_string()),
        ShelfNote::Refresh(seconds) => {
            text_key("mall.afterRefresh").replace("{time}", &span_text(seconds))
        }
    }
}

fn span_text(seconds: i64) -> String {
    let (days, hours, minutes) = discount_parts(seconds);
    if days > 0 {
        text_key("mall.spanDays")
            .replace("{days}", &days.to_string())
            .replace("{hours}", &hours.to_string())
    } else if hours > 0 {
        text_key("mall.spanHours")
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &minutes.to_string())
    } else {
        text_key("mall.spanMinutes").replace("{minutes}", &minutes.max(1).to_string())
    }
}

fn goods_lines(value: &Value) -> Vec<GoodsLine> {
    let now = value.get("serverTime").and_then(Value::as_i64).unwrap_or(0);
    value
        .get("goods")
        .and_then(Value::as_array)
        .map(|goods| goods.iter().filter_map(|row| goods_from(row, now)).collect())
        .unwrap_or_default()
}

fn goods_filter_button(
    filter: Signal<String>,
    id: &'static str,
    label: &'static str,
) -> impl IntoElement {
    button(move || text_key(label), ButtonVariant::Quiet)
        .selected(move || filter.get() == id)
        .on_press(move || filter.set(id.to_owned()))
}

fn reward_contents(value: &Value) -> String {
    value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("").trim();
                    if name.is_empty() {
                        return None;
                    }
                    let count = row.get("count").and_then(Value::as_i64).unwrap_or(0);
                    Some(format!("{name} ×{count}"))
                })
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

fn reward_names(value: &Value) -> String {
    let mut names = vec![value.get("name").and_then(Value::as_str).unwrap_or("").to_owned()];
    if let Some(rows) = value.get("rewards").and_then(Value::as_array) {
        for row in rows {
            if let Some(name) = row.get("name").and_then(Value::as_str) {
                names.push(name.to_owned());
            }
        }
    }
    names.join(" ")
}

fn discount_seconds(end: i64, now_ms: i64) -> i64 {
    if end <= 0 || now_ms <= 0 {
        return 0;
    }
    let end_ms = if end > 1_000_000_000_000 { end } else { end * 1000 };
    (end_ms - now_ms).max(0) / 1000
}

fn discount_parts(seconds: i64) -> (i64, i64, i64) {
    let seconds = seconds.max(0);
    (seconds / 86_400, (seconds % 86_400) / 3600, (seconds % 3600) / 60)
}

fn discount_left(seconds: i64) -> String {
    let (days, hours, minutes) = discount_parts(seconds);
    if days > 0 {
        text_key("mall.leftDays")
            .replace("{days}", &days.to_string())
            .replace("{hours}", &hours.to_string())
    } else if hours > 0 {
        text_key("mall.leftHours")
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &minutes.to_string())
    } else {
        text_key("mall.leftMinutes").replace("{minutes}", &minutes.max(1).to_string())
    }
}

fn goods_matches(goods: &GoodsLine, filter: &str, query: &str) -> bool {
    if !query_matches(goods, query) {
        return false;
    }
    match filter {
        "free" => goods.free,
        "discount" => goods.discounted || !goods.discount.is_empty(),
        "fertilizer" => goods.haystack.contains("化肥"),
        "pet" => goods.haystack.contains("狗粮"),
        _ => true,
    }
}

fn query_matches(goods: &GoodsLine, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || goods.haystack.to_lowercase().contains(&query)
        || goods.id.to_string().contains(&query)
}

fn currency_line(value: &Value) -> String {
    let mut rows = value
        .get("currencies")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    if !row.get("balanceKnown").and_then(Value::as_bool).unwrap_or(false) {
                        return None;
                    }
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("").trim();
                    if name.is_empty() {
                        return None;
                    }
                    let amount = row.get("balance").and_then(Value::as_i64).unwrap_or(0);
                    let id = row.get("id").and_then(Value::as_i64).unwrap_or(0);
                    Some((id, format!("{name} {amount}")))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    rows.sort_by_key(|(id, _)| *id);
    rows.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join(" · ")
}

fn goods_from(value: &Value, now_ms: i64) -> Option<GoodsLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let price = value.get("price");
    let free = value.get("isFree").and_then(Value::as_bool).unwrap_or(false);
    let status = value.get("purchaseStatus").and_then(Value::as_str).unwrap_or("");
    let price_count = price.and_then(|item| item.get("count")).and_then(Value::as_i64).unwrap_or(0);
    Some(GoodsLine {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        contents: reward_contents(value),
        price: if free { String::new() } else { price_text(price) },
        price_id: price.and_then(|item| item.get("id")).and_then(Value::as_i64).unwrap_or(0),
        price_count,
        purchasable: value.get("purchasable").and_then(Value::as_bool).unwrap_or(false),
        status_key: status_key(status).or(free.then_some("mall.free")),
        limit_key: value.get("limit").and_then(limit_key),
        limit_bought: value.pointer("/limit/bought").and_then(Value::as_i64).unwrap_or(0),
        limit_max: value.pointer("/limit/max").and_then(Value::as_i64).unwrap_or(0),
        balance: price.and_then(|item| item.get("balance")).and_then(Value::as_i64),
        original: value.get("originalPrice").and_then(Value::as_i64).unwrap_or(0).max(0),
        discount: value.get("discountText").and_then(Value::as_str).unwrap_or("").trim().to_owned(),
        discount_seconds: discount_seconds(
            value.get("discountEndTime").and_then(Value::as_i64).unwrap_or(0),
            now_ms,
        ),
        free,
        discounted: value.get("isDiscounted").and_then(Value::as_bool).unwrap_or(false),
        haystack: reward_names(value),
        short: price
            .and_then(|item| item.get("balance"))
            .and_then(Value::as_i64)
            .is_some_and(|held| price_count > 0 && held < price_count),
        limit_remaining: value.get("limit").and_then(|limit| {
            limit.get("remaining").and_then(Value::as_i64).or_else(|| {
                let max = limit.get("max").and_then(Value::as_i64)?;
                let bought = limit.get("bought").and_then(Value::as_i64).unwrap_or(0);
                Some(max.saturating_sub(bought))
            })
        }),
    })
}

impl GoodsLine {
    fn max_count(&self) -> i64 {
        let costs = if self.price_count > 0 {
            vec![crate::purchase_count::PurchaseCost {
                count: self.price_count,
                balance: self.balance,
            }]
        } else {
            Vec::new()
        };
        crate::purchase_count::purchase_max_count(self.limit_remaining, &costs, 9999)
    }
}

fn chosen_count(counts: Signal<Vec<(i64, i64)>>, id: i64, max_count: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(goods_id, _)| *goods_id == id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    stored.clamp(1, max_count.max(1))
}

fn set_count(counts: Signal<Vec<(i64, i64)>>, id: i64, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(goods_id, _)| *goods_id == id) {
        row.1 = count;
    } else {
        rows.push((id, count));
    }
    counts.set(rows);
}

fn count_stepper(
    id: i64,
    max_count: i64,
    counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<Option<String>>,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some() || chosen_count(counts, id, max_count) <= 1
                })
                .on_press(move || {
                    let next = chosen_count(counts, id, max_count).saturating_sub(1).max(1);
                    set_count(counts, id, next);
                }),
        )
        .child(
            text(move || SharedString::from(chosen_count(counts, id, max_count).to_string()))
                .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some() || chosen_count(counts, id, max_count) >= max_count
                })
                .on_press(move || {
                    let next = chosen_count(counts, id, max_count).saturating_add(1).min(max_count);
                    set_count(counts, id, next);
                }),
        )
}

fn limit_key(limit: &Value) -> Option<&'static str> {
    Some(match limit.get("type").and_then(Value::as_i64)? {
        1 => "mall.limitDaily",
        2 => "mall.limitWeekly",
        3 => "mall.limitMonthly",
        4 => "mall.limitForever",
        _ => "mall.limit",
    })
}

fn price_text(price: Option<&Value>) -> String {
    let Some(price) = price else {
        return String::new();
    };
    let name = price.get("name").and_then(Value::as_str).unwrap_or("");
    let count = price.get("count").and_then(Value::as_i64).unwrap_or(0);
    if name.is_empty() {
        count.to_string()
    } else {
        format!("{name} {count}")
    }
}

fn status_key(status: &str) -> Option<&'static str> {
    match status {
        "owned" => Some("mall.owned"),
        "sold_out" => Some("mall.soldOut"),
        "svip_required" => Some("mall.memberOnly"),
        "ad_required" => Some("mall.ad"),
        "share_required" => Some("mall.share"),
        "unavailable" => Some("mall.unavailable"),
        _ => None,
    }
}

fn mystery_offer(value: &Value) -> Option<MysteryOffer> {
    if !value.get("active").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let npc = value.get("npc")?;
    let id = npc.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let reward_count =
        npc.get("reward").and_then(|item| item.get("count")).and_then(Value::as_i64).unwrap_or(0);
    let price = npc.get("price");
    let price_count = price.and_then(|item| item.get("count")).and_then(Value::as_i64).unwrap_or(0);
    let balance = price.and_then(|item| item.get("balance")).filter(|item| !item.is_null());
    let balance = balance.and_then(|item| {
        item.as_i64().or_else(|| item.as_str().and_then(|text| text.trim().parse().ok()))
    });
    let discount_percent = npc.get("discountPercent").and_then(Value::as_i64).unwrap_or(0).max(0);
    let original = npc.get("originalPrice").and_then(Value::as_i64).unwrap_or(0).max(0);
    let block_key = if reward_count <= 0 {
        "mall.unavailable"
    } else if balance.is_some_and(|held| held < price_count) {
        "mall.mysteryShort"
    } else {
        ""
    };
    Some(MysteryOffer {
        id,
        reward: price_text(npc.get("reward")),
        price: price_text(price),
        discount_percent,
        original,
        balance,
        leaves: mystery_clock(value.get("expireTime").and_then(Value::as_i64).unwrap_or(0)),
        purchasable: block_key.is_empty(),
        block_key,
    })
}

fn discount_fold(percent: i64) -> String {
    let fold = percent as f64 / 10.0;
    if percent % 10 == 0 {
        format!("{}", fold.round() as i64)
    } else {
        format!("{fold:.1}")
    }
}

fn mystery_clock(millis: i64) -> String {
    if millis <= 0 {
        return String::new();
    }
    chrono::FixedOffset::east_opt(8 * 3600)
        .and_then(|zone| {
            chrono::DateTime::from_timestamp_millis(millis)
                .map(|time| time.with_timezone(&zone).format("%m-%d %H:%M").to_string())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        currency_line, discount_fold, discount_parts, goods_lines, goods_matches, mystery_offer,
        shelf_note, ShelfNote,
    };

    #[test]
    fn catalog_keeps_price_and_purchase_state() {
        let lines = goods_lines(&json!({
            "serverTime": 1_000_000,
            "goods": [
                { "id": 0, "name": "drop" },
                {
                    "id": 8,
                    "name": "化肥",
                    "purchasable": true,
                    "purchaseStatus": "available",
                    "isDiscounted": true,
                    "discountText": "5折",
                    "originalPrice": 20,
                    "discountEndTime": 2000,
                    "price": { "id": 1001, "count": 10, "name": "金币", "balance": 35 },
                    "limit": { "type": 1, "bought": 1, "max": 10, "remaining": 10 }
                },
                { "id": 9, "name": "", "isFree": true, "purchasable": true, "purchaseStatus": "available" },
                {
                    "id": 10,
                    "name": "礼包",
                    "rewards": [{ "name": "狗粮", "count": 1 }],
                    "purchasable": true,
                    "price": { "id": 1002, "count": 40, "name": "点券", "balance": 10 }
                }
            ]
        }));
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].price, "金币 10");
        assert_eq!(lines[0].price_count, 10);
        assert_eq!(lines[0].max_count(), 3);
        assert!(lines[0].status_key.is_none());
        assert_eq!(lines[0].limit_key, Some("mall.limitDaily"));
        assert_eq!(lines[0].limit_bought, 1);
        assert_eq!(lines[0].limit_max, 10);
        assert_eq!(lines[1].limit_key, None);
        assert_eq!(lines[1].name, "#9");
        assert_eq!(lines[1].status_key, Some("mall.free"));
        assert_eq!(lines[0].contents, "");
        assert_eq!(lines[2].contents, "狗粮 ×1");
        assert_eq!(lines[0].discount, "5折");
        assert_eq!(lines[0].discount_seconds, 1000);
        assert_eq!(discount_parts(1000), (0, 0, 16));
        assert!(!lines[0].short);
        assert!(lines[2].short);
        assert!(goods_matches(&lines[0], "discount", ""));
        assert!(goods_matches(&lines[0], "fertilizer", "化肥"));
        assert!(!goods_matches(&lines[0], "fertilizer", "狗粮"));
        assert!(goods_matches(&lines[1], "free", ""));
        assert!(!goods_matches(&lines[1], "discount", ""));
        assert!(goods_matches(&lines[2], "pet", "10"));
        assert_eq!(
            currency_line(&json!({
                "currencies": [
                    { "id": 1002, "name": "点券", "balanceKnown": true, "balance": 10 },
                    { "id": 1001, "name": "金币", "balanceKnown": true, "balance": 35 },
                    { "id": 1003, "name": "钻石", "balanceKnown": false, "count": 9 }
                ]
            })),
            "金币 35 · 点券 10"
        );
        assert_eq!(
            shelf_note(&json!({ "refreshCountdown": 1000 }), false),
            ShelfNote::Refresh(1000)
        );
        assert_eq!(
            shelf_note(&json!({ "membership": { "isSvip": true, "remainingDays": 3 } }), true),
            ShelfNote::Svip(3)
        );
        assert_eq!(
            shelf_note(&json!({ "membership": { "isSvip": false }, "refreshCountdown": 9 }), true),
            ShelfNote::NotSvip
        );
        assert_eq!(shelf_note(&json!({}), false), ShelfNote::Quiet);
    }

    #[test]
    fn inactive_mystery_shop_has_no_offer() {
        assert!(mystery_offer(&json!({ "active": false, "npc": { "id": 1 } })).is_none());
        let offer = mystery_offer(&json!({
            "active": true,
            "npc": {
                "id": 3,
                "reward": { "name": "种子", "count": 2 },
                "price": { "name": "点券", "count": 5 }
            }
        }))
        .expect("offer");
        assert_eq!(offer.reward, "种子 2");
        assert_eq!(offer.price, "点券 5");
        assert!(offer.purchasable);
        let short = mystery_offer(&json!({
            "active": true,
            "expireTime": 1_790_003_600_000_i64,
            "npc": {
                "id": 4,
                "discountPercent": 25,
                "originalPrice": 40,
                "reward": { "name": "种子", "count": 2 },
                "price": { "name": "点券", "count": 30, "balance": 10 }
            }
        }))
        .expect("short");
        assert!(!short.purchasable);
        assert_eq!(short.block_key, "mall.mysteryShort");
        assert_eq!(short.discount_percent, 25);
        assert_eq!(discount_fold(25), "2.5");
        assert_eq!(discount_fold(20), "2");
        assert_eq!(short.original, 40);
        assert_eq!(short.balance, Some(10));
        assert!(!short.leaves.is_empty());
        let empty = mystery_offer(&json!({
            "active": true,
            "npc": {
                "id": 5,
                "reward": { "name": "种子", "count": 0 },
                "price": { "name": "点券", "count": 1, "balance": null }
            }
        }))
        .expect("empty");
        assert!(!empty.purchasable);
        assert_eq!(empty.block_key, "mall.unavailable");
    }
}
