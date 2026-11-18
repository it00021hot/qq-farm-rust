use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, field, notice, BadgeTone, ButtonVariant};

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
    current: Vec<BuffLine>,
    details: Vec<DetailLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct BuffLine {
    name: String,
    amount: String,
}

#[derive(Clone, PartialEq, Eq)]
struct DetailLine {
    id: String,
    text: String,
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
    group: String,
    sort: i64,
    status_key: &'static str,
    status_level: i64,
    fresh: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct ItemSection {
    id: String,
    title_key: &'static str,
    owned: i64,
    total: i64,
    items: Vec<IllItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressHint {
    Ready,
    Remain(i64),
    Max,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let shelf = signal(Shelf::Crop);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let counts = signal(Vec::<(i64, i64)>::new());
    let filter = signal("all".to_owned());
    let query = signal(String::new());
    let details_open = signal(false);
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
            move || {
                book_body(
                    account_id,
                    shelf,
                    pending,
                    failure,
                    counts,
                    books,
                    filter,
                    query,
                    details_open,
                )
            },
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
    filter: Signal<String>,
    query: Signal<String>,
    details_open: Signal<bool>,
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
                let hint = match progress_hint(&book) {
                    ProgressHint::Ready => text_key("illustrated.claimReady").to_string(),
                    ProgressHint::Remain(count) => {
                        text_key("illustrated.remain").replace("{count}", &count.to_string())
                    }
                    ProgressHint::Max => text_key("illustrated.maxLevel").to_string(),
                };
                SharedString::from(format!(
                    "{} {} · {}/{} · {hint}",
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
            move || shelf.get() == Shelf::Mutant && !selected_book(books, shelf).current.is_empty(),
            move || {
                view().gap_1().child(each(
                    move || selected_book(books, shelf).current,
                    |line: &BuffLine| format!("{} {}", line.name, line.amount),
                    move |line, _| {
                        let line = line.get_untracked();
                        let label = SharedString::from(format!("{} · {}", line.name, line.amount));
                        text(label).text_sm().text_color(move || nsh_base::theme().muted_foreground)
                    },
                ))
            },
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(filter_button(filter, "all", "illustrated.all"))
                .child(filter_button(filter, "owned", "illustrated.owned"))
                .child(filter_button(filter, "missing", "illustrated.missing"))
                .child(when(
                    move || shelf.get() == Shelf::Crop,
                    move || filter_button(filter, "buyable", "illustrated.buyable"),
                ))
                .child(
                    button(move || text_key("illustrated.details"), ButtonVariant::Quiet)
                        .selected(move || details_open.get())
                        .on_press(move || details_open.set(!details_open.get_untracked())),
                ),
        )
        .child(field(
            move || query.get(),
            move || text_key("illustrated.search").to_string(),
            move |text| query.set(text.to_owned()),
        ))
        .child(when(move || details_open.get(), move || detail_list(books, shelf)))
        .child(when(
            move || {
                let book = selected_book(books, shelf);
                book.items.is_empty()
            },
            move || notice(move || text_key("illustrated.empty")),
        ))
        .child(when(
            move || {
                let book = selected_book(books, shelf);
                !book.items.is_empty()
                    && visible_items(
                        &book,
                        &filter.get(),
                        &query.get(),
                        shelf.get() == Shelf::Mutant,
                    )
                    .is_empty()
            },
            move || notice(move || text_key("illustrated.noMatch")),
        ))
        .child(each(
            move || {
                item_sections(
                    &selected_book(books, shelf),
                    &filter.get(),
                    &query.get(),
                    shelf.get() == Shelf::Mutant,
                )
            },
            |section: &ItemSection| section.id.clone(),
            move |section, _| {
                let section = section.get_untracked();
                let title_key = section.title_key;
                let owned = section.owned;
                let total = section.total;
                let items = section.items;
                view()
                    .gap_1()
                    .child(when(
                        move || !title_key.is_empty(),
                        move || {
                            text(move || {
                                SharedString::from(format!(
                                    "{} {owned}/{total}",
                                    text_key(title_key)
                                ))
                            })
                            .text_sm()
                        },
                    ))
                    .child(each(
                        move || items.clone(),
                        |item: &IllItem| item.seed_id.to_string(),
                        move |item, _| {
                            item_line(
                                item.get_untracked(),
                                account_id,
                                pending,
                                failure,
                                counts,
                                books,
                            )
                        },
                    ))
            },
        ))
}

fn filter_button(
    filter: Signal<String>,
    id: &'static str,
    label: &'static str,
) -> impl IntoElement {
    button(move || text_key(label), ButtonVariant::Quiet)
        .selected(move || filter.get() == id)
        .on_press(move || filter.set(id.to_owned()))
}

fn detail_list(books: Resource<IllustratedView, String>, shelf: Signal<Shelf>) -> View {
    view()
        .gap_1()
        .child(when(
            move || selected_book(books, shelf).details.is_empty(),
            move || notice(move || text_key("illustrated.noDetails")),
        ))
        .child(each(
            move || selected_book(books, shelf).details,
            |line: &DetailLine| line.id.clone(),
            move |line, _| {
                let label = SharedString::from(line.get_untracked().text);
                text(label).text_sm().text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
}

fn visible_items(book: &BookView, filter: &str, query: &str, mutant: bool) -> Vec<IllItem> {
    let filter = if mutant && filter == "buyable" { "all" } else { filter };
    let query = query.trim();
    book.items
        .iter()
        .filter(|item| {
            if !query.is_empty() && !item.name.contains(query) {
                return false;
            }
            match filter {
                "owned" => item.unlocked,
                "missing" => !item.unlocked,
                "buyable" => item.purchasable,
                _ => true,
            }
        })
        .cloned()
        .collect()
}

fn item_sections(book: &BookView, filter: &str, query: &str, mutant: bool) -> Vec<ItemSection> {
    let visible = visible_items(book, filter, query, mutant);
    if !mutant {
        return if visible.is_empty() {
            Vec::new()
        } else {
            vec![ItemSection {
                id: "crop".to_owned(),
                title_key: "",
                owned: 0,
                total: 0,
                items: visible,
            }]
        };
    }
    [
        ("gold", "illustrated.goldFruit"),
        ("decoration", "illustrated.decorationFruit"),
        ("activity", "illustrated.activityFruit"),
    ]
    .into_iter()
    .filter_map(|(key, title_key)| {
        let all: Vec<_> = book.items.iter().filter(|item| item.group == key).cloned().collect();
        if all.is_empty() {
            return None;
        }
        let owned = all.iter().filter(|item| item.unlocked).count() as i64;
        let total = all.len() as i64;
        let mut items: Vec<_> = visible.iter().filter(|item| item.group == key).cloned().collect();
        items.sort_by_key(|item| item.sort);
        if items.is_empty() {
            return None;
        }
        Some(ItemSection { id: key.to_owned(), title_key, owned, total, items })
    })
    .collect()
}

fn progress_hint(book: &BookView) -> ProgressHint {
    if book.claimable {
        return ProgressHint::Ready;
    }
    let remain = book.next - book.progress;
    if book.next > 0 && remain > 0 {
        ProgressHint::Remain(remain)
    } else {
        ProgressHint::Max
    }
}

fn selected_book(books: Resource<IllustratedView, String>, shelf: Signal<Shelf>) -> BookView {
    let empty = empty_book();
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
    let seed_id = item.seed_id;
    let goods_id = item.goods_id;
    let price = item.price;
    let max_count = item.max_count;
    let purchasable = item.purchasable;
    let status_key = item.status_key;
    let status_level = item.status_level;
    let status_ready = matches!(status_key, "illustrated.buy" | "illustrated.collected");
    let fresh = item.fresh;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().flex_1())
        .child(when(
            move || fresh,
            move || badge(move || text_key("illustrated.fresh"), BadgeTone::Success),
        ))
        .child(text(SharedString::from(progress.to_string())).text_sm())
        .child(badge(
            move || {
                let label = text_key(status_key);
                if status_level > 0 {
                    SharedString::from(label.replace("{level}", &status_level.to_string()))
                } else {
                    label
                }
            },
            if status_ready { BadgeTone::Success } else { BadgeTone::Neutral },
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
        return empty_book();
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
        current: buff_lines(value.get("currentBuffs")),
        details: detail_lines(value),
    }
}

fn empty_book() -> BookView {
    BookView {
        level: 0,
        progress: 0,
        next: 0,
        claimable: false,
        items: Vec::new(),
        current: Vec::new(),
        details: Vec::new(),
    }
}

fn buff_lines(value: Option<&Value>) -> Vec<BuffLine> {
    value
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(buff_line).collect())
        .unwrap_or_default()
}

fn buff_line(value: &Value) -> Option<BuffLine> {
    let name = value.get("name").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if name.is_empty() {
        return None;
    }
    let amount = value.get("value").and_then(Value::as_i64).unwrap_or(0);
    let kind = value.get("valueType").and_then(Value::as_str).unwrap_or("");
    Some(BuffLine { name, amount: buff_amount(amount, kind) })
}

fn buff_amount(value: i64, kind: &str) -> String {
    if kind == "probability" {
        format!("触发概率 {}%", tenth_percent(value))
    } else {
        format!("数量 +{value}")
    }
}

fn tenth_percent(value: i64) -> String {
    if value % 10 == 0 {
        format!("{}", value / 10)
    } else {
        format!("{:.1}", value as f64 / 10.0)
    }
}

fn detail_lines(value: &Value) -> Vec<DetailLine> {
    let level = value.get("level").and_then(Value::as_i64).unwrap_or(0);
    let buffs = value.get("buffs").and_then(Value::as_array);
    if let Some(buffs) = buffs.filter(|rows| !rows.is_empty()) {
        return buffs
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let line = buff_line(row)?;
                let bonus_level = row.get("level").and_then(Value::as_i64).unwrap_or(0);
                let state = if bonus_level <= level { "已达成" } else { "未达成" };
                Some(DetailLine {
                    id: format!("buff:{index}"),
                    text: format!("Lv.{bonus_level} · {} · {} · {state}", line.name, line.amount),
                })
            })
            .collect();
    }
    value
        .get("levels")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter().enumerate().filter_map(|(index, row)| level_line(index, row)).collect()
        })
        .unwrap_or_default()
}

fn level_line(index: usize, value: &Value) -> Option<DetailLine> {
    let level = value.get("level").and_then(Value::as_i64).unwrap_or(0);
    let progress = value.get("progress").and_then(Value::as_i64).unwrap_or(0);
    let claimed = value.get("claimed").and_then(Value::as_bool).unwrap_or(false);
    let ready = value.get("canClaim").and_then(Value::as_bool).unwrap_or(false);
    let state = if claimed {
        "已领取"
    } else if ready {
        "可领取"
    } else {
        "未达成"
    };
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|reward| {
                    let name = reward.get("name").and_then(Value::as_str).unwrap_or("").trim();
                    if name.is_empty() {
                        return None;
                    }
                    let count = reward.get("count").and_then(Value::as_i64).unwrap_or(0);
                    Some(format!("{name} ×{count}"))
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    let text = if rewards.is_empty() {
        format!("{level} 级 · {progress} · {state}")
    } else {
        format!("{level} 级 · {progress} · {rewards} · {state}")
    };
    Some(DetailLine { id: format!("level:{index}"), text })
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
    let status = seed_status(value, purchasable);
    Some(IllItem {
        seed_id,
        name: if name.is_empty() { format!("#{seed_id}") } else { name },
        unlocked: value.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        progress: value.get("progress").and_then(Value::as_i64).unwrap_or(0),
        goods_id,
        price,
        max_count,
        purchasable,
        group: value.get("group").and_then(Value::as_str).unwrap_or("gold").to_owned(),
        sort: value.get("sort").and_then(Value::as_i64).unwrap_or(0),
        status_key: status.0,
        status_level: status.1,
        fresh: value.get("isNew").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn seed_status(value: &Value, purchasable: bool) -> (&'static str, i64) {
    if purchasable {
        return ("illustrated.buy", 0);
    }
    let shop = value.get("shopSeed").filter(|shop| !shop.is_null());
    let flag =
        |key: &str| shop.and_then(|shop| shop.get(key)).and_then(Value::as_bool) == Some(true);
    if flag("unknownMeta") {
        return ("illustrated.otherSource", 0);
    }
    if flag("requiresUnlockCard") {
        return ("illustrated.unlockCard", 0);
    }
    if flag("locked") {
        let level =
            shop.and_then(|shop| shop.get("requiredLevel")).and_then(Value::as_i64).unwrap_or(0);
        return if level > 0 {
            ("illustrated.levelLock", level)
        } else {
            ("illustrated.notYet", 0)
        };
    }
    if flag("soldOut") {
        return ("illustrated.soldOut", 0);
    }
    if value.get("unlocked").and_then(Value::as_bool).unwrap_or(false) {
        ("illustrated.collected", 0)
    } else {
        ("illustrated.missing", 0)
    }
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

    use super::{
        illustrated_view, item_sections, progress_hint, seed_purchase_max, seed_status,
        visible_items, ProgressHint,
    };

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
                ],
                "levels": [{
                    "level": 2,
                    "progress": 10,
                    "claimed": false,
                    "canClaim": true,
                    "rewards": [{ "name": "金币", "count": 5 }]
                }]
            },
            "mutant": {
                "level": 1,
                "items": [
                    { "seedId": 3, "name": "装扮果", "group": "decoration", "sort": 2, "unlocked": true, "isNew": true },
                    { "seedId": 2, "name": "黄金果", "group": "gold", "sort": 9, "unlocked": false },
                    { "seedId": 4, "name": "活动果", "group": "activity", "sort": 1, "unlocked": false }
                ],
                "currentBuffs": [{ "name": "经验", "value": 20, "valueType": "probability" }],
                "buffs": [{ "level": 1, "name": "产量", "value": 3, "valueType": "quantity" }]
            }
        }));
        assert!(view.crop.claimable);
        assert_eq!(view.crop.level, 3);
        assert_eq!(view.crop.next, 20);
        assert!(view.crop.items[0].purchasable);
        assert_eq!(view.crop.items[0].status_key, "illustrated.buy");
        assert_eq!(view.crop.items[1].status_key, "illustrated.otherSource");
        assert_eq!(
            seed_status(&json!({ "shopSeed": { "locked": true, "requiredLevel": 8 } }), false),
            ("illustrated.levelLock", 8)
        );
        assert_eq!(
            seed_status(&json!({ "shopSeed": { "requiresUnlockCard": true } }), false).0,
            "illustrated.unlockCard"
        );
        assert_eq!(
            seed_status(&json!({ "shopSeed": { "soldOut": true } }), false).0,
            "illustrated.soldOut"
        );
        assert_eq!(seed_status(&json!({ "unlocked": true }), false).0, "illustrated.collected");
        assert_eq!(view.crop.items[0].max_count, 4);
        assert!(!view.crop.items[1].purchasable);
        assert_eq!(view.mutant.items.len(), 3);
        assert_eq!(progress_hint(&view.crop), ProgressHint::Ready);
        assert_eq!(progress_hint(&view.mutant), ProgressHint::Max);
        let sections = item_sections(&view.mutant, "all", "", true);
        assert_eq!(
            sections.iter().map(|section| section.id.as_str()).collect::<Vec<_>>(),
            ["gold", "decoration", "activity"]
        );
        assert!(sections[1].items[0].fresh);
        assert!(!sections[0].items[0].fresh);
        assert_eq!(sections[1].owned, 1);
        assert_eq!(sections[1].total, 1);
        assert_eq!(item_sections(&view.mutant, "owned", "", true)[0].id, "decoration");
        assert_eq!(view.crop.details[0].text, "2 级 · 10 · 金币 ×5 · 可领取");
        assert_eq!(view.mutant.current[0].amount, "触发概率 2%");
        assert_eq!(view.mutant.details[0].text, "Lv.1 · 产量 · 数量 +3 · 已达成");
        assert_eq!(visible_items(&view.crop, "buyable", "萝", false).len(), 1);
        assert!(visible_items(&view.crop, "missing", "", false)
            .iter()
            .any(|item| item.name == "锁定"));
        assert!(visible_items(&view.crop, "buyable", "", true)
            .iter()
            .any(|item| item.name == "锁定"));
        assert_eq!(seed_purchase_max(None), 9999);
        assert_eq!(seed_purchase_max(Some(0)), 0);
        assert_eq!(seed_purchase_max(Some(20_000)), 9999);
    }
}
