use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, notice, BadgeTone, ButtonVariant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shelf {
    Crop,
    Mutant,
}

impl Shelf {
    const ALL: [Self; 2] = [Self::Crop, Self::Mutant];

    const fn id(self) -> &'static str {
        match self {
            Self::Crop => "crop",
            Self::Mutant => "mutant",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Crop => "illustrated.crop",
            Self::Mutant => "illustrated.mutant",
        }
    }

    const fn kind(self) -> i32 {
        match self {
            Self::Crop => 1,
            Self::Mutant => 2,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct IllustratedView {
    crop: BookView,
    mutant: BookView,
}

#[derive(Clone, PartialEq, Eq)]
struct BookView {
    level: i64,
    progress: i64,
    next: i64,
    claimable: bool,
    items: Vec<IllItem>,
}

#[derive(Clone, PartialEq, Eq)]
struct IllItem {
    seed_id: i64,
    name: String,
    unlocked: bool,
    progress: i64,
    goods_id: i64,
    price: i64,
    max_count: i64,
    purchasable: bool,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let shelf = signal(Shelf::Crop);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let counts = signal(Vec::<(i64, i64)>::new());
    let books = resource(
        move || running_account(account_id, rows),
        |id, _| async move {
            let context = session::session().context.clone();
            let value = session::call(async move {
                qq_farm_app::friend::illustrated_snapshot(&context, &id).await
            })
            .await?;
            Ok(illustrated_view(&value))
        },
    );
    view()
        .gap_3()
        .child(failure_line(failure))
        .child(gate_notice(account_id, rows, "farm.needRunning"))
        .child(when(
            move || running(account_id, rows) && books.loading() && books.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || running(account_id, rows) && books.error().is_some(),
            move || load_error(books),
        ))
        .child(when(
            move || running(account_id, rows) && books.get().is_some(),
            move || book_body(account_id, shelf, pending, failure, counts, books),
        ))
}

fn running(account_id: Signal<Option<String>>, rows: Resource<Vec<AccountRow>, String>) -> bool {
    account_id.get().zip(rows.get()).is_some_and(|(id, rows)| account_is_running(&rows, &id))
}

fn running_account(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> Option<String> {
    running(account_id, rows).then(|| account_id.get()).flatten()
}

fn load_error(books: Resource<IllustratedView, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            books.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || books.refetch()),
        )
}

fn book_body(
    account_id: Signal<Option<String>>,
    shelf: Signal<Shelf>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    counts: Signal<Vec<(i64, i64)>>,
    books: Resource<IllustratedView, String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || Shelf::ALL.to_vec(),
            |item: &Shelf| item.id().to_owned(),
            move |item, _| {
                let item = item.get_untracked();
                button(move || text_key(item.label_key()), ButtonVariant::Quiet)
                    .selected(move || shelf.get() == item)
                    .on_press(move || shelf.set(item))
            },
        )))
        .child(
            text(move || {
                let book = selected_book(books, shelf);
                SharedString::from(format!(
                    "{} {} · {}/{}",
                    text_key("farm.illustrated"),
                    book.level,
                    book.progress,
                    book.next
                ))
            })
            .text_sm(),
        )
        .child(when(
            move || selected_book(books, shelf).claimable,
            move || {
                button(move || text_key("illustrated.claim"), ButtonVariant::Primary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || claim_book(account_id, shelf, pending, failure, books))
            },
        ))
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || books.refetch()),
        )
        .child(when(
            move || selected_book(books, shelf).items.is_empty(),
            move || notice(move || text_key("illustrated.empty")),
        ))
        .child(each(
            move || selected_book(books, shelf).items,
            |item: &IllItem| item.seed_id.to_string(),
            move |item, _| {
                item_line(item.get_untracked(), account_id, pending, failure, counts, books)
            },
        ))
}

fn selected_book(books: Resource<IllustratedView, String>, shelf: Signal<Shelf>) -> BookView {
    let empty = BookView { level: 0, progress: 0, next: 0, claimable: false, items: Vec::new() };
    books
        .get()
        .map(|view| match shelf.get() {
            Shelf::Crop => view.crop,
            Shelf::Mutant => view.mutant,
        })
        .unwrap_or(empty)
}

fn item_line(
    item: IllItem,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    counts: Signal<Vec<(i64, i64)>>,
    books: Resource<IllustratedView, String>,
) -> View {
    let name = SharedString::from(item.name);
    let progress = item.progress;
    let unlocked = item.unlocked;
    let seed_id = item.seed_id;
    let goods_id = item.goods_id;
    let price = item.price;
    let max_count = item.max_count;
    let purchasable = item.purchasable;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().flex_1())
        .child(text(SharedString::from(progress.to_string())).text_sm())
        .child(badge(
            move || text_key(if unlocked { "illustrated.unlocked" } else { "illustrated.locked" }),
            if unlocked { BadgeTone::Success } else { BadgeTone::Neutral },
        ))
        .child(when(
            move || purchasable && max_count > 1,
            move || count_stepper(goods_id, max_count, counts, pending),
        ))
        .child(when(move || purchasable, {
            let price_label = SharedString::from(format!("{price}"));
            move || {
                let price_label = price_label.clone();
                button(
                    move || {
                        SharedString::from(format!(
                            "{} {}",
                            text_key("illustrated.buy"),
                            price_label
                        ))
                    },
                    ButtonVariant::Secondary,
                )
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    let count = chosen_count(counts, goods_id, max_count);
                    buy_seed(account_id, pending, failure, books, goods_id, count, seed_id);
                })
            }
        }))
}

fn count_stepper(
    goods_id: i64,
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
                    pending.get().is_some() || chosen_count(counts, goods_id, max_count) <= 1
                })
                .on_press(move || {
                    let next = chosen_count(counts, goods_id, max_count).saturating_sub(1).max(1);
                    set_count(counts, goods_id, next);
                }),
        )
        .child(
            text(move || SharedString::from(chosen_count(counts, goods_id, max_count).to_string()))
                .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some()
                        || chosen_count(counts, goods_id, max_count) >= max_count
                })
                .on_press(move || {
                    let next =
                        chosen_count(counts, goods_id, max_count).saturating_add(1).min(max_count);
                    set_count(counts, goods_id, next);
                }),
        )
}

fn chosen_count(counts: Signal<Vec<(i64, i64)>>, goods_id: i64, max_count: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(id, _)| *id == goods_id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    stored.clamp(1, max_count.max(1))
}

fn set_count(counts: Signal<Vec<(i64, i64)>>, goods_id: i64, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(id, _)| *id == goods_id) {
        row.1 = count;
    } else {
        rows.push((goods_id, count));
    }
    counts.set(rows);
}

fn claim_book(
    account_id: Signal<Option<String>>,
    shelf: Signal<Shelf>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    books: Resource<IllustratedView, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else { return };
    let kind = shelf.get_untracked().kind();
    pending.set(Some(format!("claim:{kind}")));
    failure.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::friend::illustrated_claim(&context, &id, kind).await
        })
        .await;
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(None);
        books.refetch();
    });
}

fn buy_seed(
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    books: Resource<IllustratedView, String>,
    goods_id: i64,
    count: i64,
    seed_id: i64,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else { return };
    pending.set(Some(format!("buy:{seed_id}")));
    failure.set(None);
    confirm::confirm(
        "illustrated.confirmBuy.title",
        "illustrated.confirmBuy.body",
        "buy",
        "illustrated.buy",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            spawn(async move {
                let context = session::session().context.clone();
                let result = session::call(async move {
                    qq_farm_app::friend::seed_purchase(&context, &id, goods_id, count).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(None);
                books.refetch();
            });
        },
    );
}

fn illustrated_view(value: &Value) -> IllustratedView {
    IllustratedView { crop: book_from(value.get("crop")), mutant: book_from(value.get("mutant")) }
}

fn book_from(value: Option<&Value>) -> BookView {
    let Some(value) = value else {
        return BookView { level: 0, progress: 0, next: 0, claimable: false, items: Vec::new() };
    };
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(item_from).collect())
        .unwrap_or_default();
    BookView {
        level: value.get("level").and_then(Value::as_i64).unwrap_or(0),
        progress: value.get("progress").and_then(Value::as_i64).unwrap_or(0),
        next: value.get("nextLevelProgress").and_then(Value::as_i64).unwrap_or(0),
        claimable: value.get("claimable").and_then(Value::as_bool).unwrap_or(false),
        items,
    }
}

fn item_from(value: &Value) -> Option<IllItem> {
    let seed_id = value.get("seedId").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let shop = value.get("shopSeed").filter(|shop| !shop.is_null());
    let goods_id = shop.and_then(|shop| shop.get("goodsId")).and_then(Value::as_i64).unwrap_or(0);
    let price = shop.and_then(|shop| shop.get("price")).and_then(Value::as_i64).unwrap_or(0);
    let locked = shop.and_then(|shop| shop.get("locked")).and_then(Value::as_bool).unwrap_or(false);
    let sold_out =
        shop.and_then(|shop| shop.get("soldOut")).and_then(Value::as_bool).unwrap_or(false);
    let unknown =
        shop.and_then(|shop| shop.get("unknownMeta")).and_then(Value::as_bool).unwrap_or(false);
    let unlock_card = shop
        .and_then(|shop| shop.get("requiresUnlockCard"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let remaining =
        shop.and_then(|shop| shop.get("remainingPurchaseCount")).and_then(Value::as_i64);
    let max_count = seed_purchase_max(remaining);
    let purchasable = goods_id > 0
        && !locked
        && !sold_out
        && !unknown
        && !unlock_card
        && price > 0
        && max_count > 0;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
    Some(IllItem {
        seed_id,
        name: if name.is_empty() { format!("#{seed_id}") } else { name },
        unlocked: value.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        progress: value.get("progress").and_then(Value::as_i64).unwrap_or(0),
        goods_id,
        price,
        max_count,
        purchasable,
    })
}

fn seed_purchase_max(remaining: Option<i64>) -> i64 {
    match remaining {
        Some(count) if count > 0 => count.min(9999),
        Some(_) => 0,
        None => 9999,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{illustrated_view, seed_purchase_max};

    #[test]
    fn book_keeps_claim_and_seed_purchase_gate() {
        let view = illustrated_view(&json!({
            "crop": {
                "level": 3,
                "progress": 12,
                "nextLevelProgress": 20,
                "claimable": true,
                "items": [
                    {
                        "seedId": 8,
                        "name": "萝卜",
                        "unlocked": true,
                        "progress": 2,
                        "shopSeed": {
                            "goodsId": 80,
                            "price": 15,
                            "locked": false,
                            "soldOut": false,
                            "remainingPurchaseCount": 4
                        }
                    },
                    {
                        "seedId": 9,
                        "name": "锁定",
                        "shopSeed": { "goodsId": 0, "unknownMeta": true, "price": null }
                    }
                ]
            },
            "mutant": { "level": 1, "items": [] }
        }));
        assert!(view.crop.claimable);
        assert_eq!(view.crop.level, 3);
        assert_eq!(view.crop.next, 20);
        assert!(view.crop.items[0].purchasable);
        assert_eq!(view.crop.items[0].max_count, 4);
        assert!(!view.crop.items[1].purchasable);
        assert!(view.mutant.items.is_empty());
        assert_eq!(seed_purchase_max(None), 9999);
        assert_eq!(seed_purchase_max(Some(0)), 0);
        assert_eq!(seed_purchase_max(Some(20_000)), 9999);
    }
}
