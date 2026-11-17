use nsh_core::prelude::*;
use qq_farm_core::services::commerce::ExpectedPrice;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

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
}

#[derive(Clone, PartialEq, Eq)]
struct MysteryOffer {
    id: i64,
    reward: String,
    price: String,
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
            move || goods_body(page, shelf, pending, failure, summary, catalog, counts),
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
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || catalog.refetch()),
        )
        .child(when(
            move || goods_lines(&catalog.get().unwrap_or(Value::Null)).is_empty(),
            move || notice(move || text_key("mall.empty")),
        ))
        .child(each(
            move || goods_lines(&catalog.get().unwrap_or(Value::Null)),
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
    let price = SharedString::from(goods.price.clone());
    let status_key = goods.status_key;
    let purchasable = goods.purchasable;
    let limit_key = goods.limit_key;
    let limit_bought = goods.limit_bought;
    let limit_max = goods.limit_max;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().font_weight(FontWeight::MEDIUM).flex_1())
        .child(text(price).text_sm().text_color(move || nsh_base::theme().muted_foreground))
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
                .disabled(move || pending.get().is_some())
                .on_press(move || {
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
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .py_2()
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
                        .disabled(move || pending.get().is_some())
                        .on_press(move || {
                            buy_mystery(offer.clone(), page, pending, failure, summary, mystery)
                        }),
                    )
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

fn goods_lines(value: &Value) -> Vec<GoodsLine> {
    value
        .get("goods")
        .and_then(Value::as_array)
        .map(|goods| goods.iter().filter_map(goods_from).collect())
        .unwrap_or_default()
}

fn goods_from(value: &Value) -> Option<GoodsLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let price = value.get("price");
    let free = value.get("isFree").and_then(Value::as_bool).unwrap_or(false);
    let status = value.get("purchaseStatus").and_then(Value::as_str).unwrap_or("");
    Some(GoodsLine {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        price: if free { String::new() } else { price_text(price) },
        price_id: price.and_then(|item| item.get("id")).and_then(Value::as_i64).unwrap_or(0),
        price_count: price.and_then(|item| item.get("count")).and_then(Value::as_i64).unwrap_or(0),
        purchasable: value.get("purchasable").and_then(Value::as_bool).unwrap_or(false),
        status_key: status_key(status).or(free.then_some("mall.free")),
        limit_key: value.get("limit").and_then(limit_key),
        limit_bought: value.pointer("/limit/bought").and_then(Value::as_i64).unwrap_or(0),
        limit_max: value.pointer("/limit/max").and_then(Value::as_i64).unwrap_or(0),
        balance: price.and_then(|item| item.get("balance")).and_then(Value::as_i64),
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
    Some(MysteryOffer {
        id,
        reward: price_text(npc.get("reward")),
        price: price_text(npc.get("price")),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{goods_lines, mystery_offer};

    #[test]
    fn catalog_keeps_price_and_purchase_state() {
        let lines = goods_lines(&json!({
            "goods": [
                { "id": 0, "name": "drop" },
                {
                    "id": 8,
                    "name": "化肥",
                    "purchasable": true,
                    "purchaseStatus": "available",
                    "price": { "id": 1001, "count": 10, "name": "金币", "balance": 35 },
                    "limit": { "type": 1, "bought": 1, "max": 10, "remaining": 10 }
                },
                { "id": 9, "name": "", "isFree": true, "purchasable": true, "purchaseStatus": "available" }
            ]
        }));
        assert_eq!(lines.len(), 2);
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
    }
}
