use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, field, notice, BadgeTone, ButtonVariant};

#[derive(Clone, Copy)]
pub(crate) struct CollectionPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

#[derive(Clone, PartialEq, Eq)]
struct Book {
    score: i64,
    progress: String,
    buffs: String,
    can_claim: bool,
    display_id: i64,
    series: Vec<SeriesLine>,
    milestones: Vec<MilestoneLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct MilestoneLine {
    level: i64,
    score: i64,
    state_key: &'static str,
    detail: String,
}

#[derive(Clone, PartialEq, Eq)]
struct SeriesLine {
    id: i64,
    name: String,
    rarity: String,
    detail: String,
    progress: String,
    pieces: Vec<Piece>,
}

#[derive(Clone, PartialEq, Eq)]
struct Piece {
    id: i64,
    name: String,
    description: String,
    stars: String,
    star: i64,
    own: i64,
    score: i64,
    status_key: &'static str,
    block_key: &'static str,
    can_upgrade: bool,
    displayed: bool,
    fresh: bool,
    price: String,
    costs: Vec<(i64, i64)>,
    display_id: i64,
}

pub(crate) fn page(page: CollectionPage) -> impl IntoElement {
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let mode = signal("book".to_owned());
    let tab = signal("normal".to_owned());
    let query = signal(String::new());
    let book_filter = signal("all".to_owned());
    let book_query = signal(String::new());
    let rewards_open = signal(false);
    let catalog = resource(
        || Some(()),
        |(), _| async move { Ok(catalog_from(&qq_farm_app::collections::mutation_catalog())) },
    );
    let book = resource(
        move || running_account(page),
        |id, _| async move {
            let context = session::session().context.clone();
            let value =
                session::call(
                    async move { qq_farm_app::collections::snapshot(&context, &id).await },
                )
                .await?;
            Ok(book_from(&value))
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(
            text(move || text_key("collections.title")).text_lg().font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("collections.book"), ButtonVariant::Quiet)
                        .selected(move || mode.get() == "book")
                        .on_press(move || mode.set("book".to_owned())),
                )
                .child(
                    button(move || text_key("collections.catalog"), ButtonVariant::Quiet)
                        .selected(move || mode.get() == "catalog")
                        .on_press(move || mode.set("catalog".to_owned())),
                ),
        )
        .child(when(
            move || mode.get() == "book",
            move || {
                view()
                    .gap_3()
                    .child(failure_line(failure))
                    .child(gate_notice(page.account_id, page.rows, "collections.needRunning"))
                    .child(when(
                        move || running(page),
                        move || {
                            book_body(
                                book,
                                pending,
                                failure,
                                page,
                                book_filter,
                                book_query,
                                rewards_open,
                            )
                        },
                    ))
            },
        ))
        .child(when(move || mode.get() == "catalog", move || catalog_body(catalog, tab, query)))
}

fn book_body(
    book: Resource<Book, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    filter: Signal<String>,
    query: Signal<String>,
    rewards_open: Signal<bool>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || book.loading() && book.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || book.error().is_some(),
            move || notice(move || SharedString::from(book.error().unwrap_or_default())),
        ))
        .child(when(
            move || book.get().is_some(),
            move || loaded(book, pending, failure, page, filter, query, rewards_open),
        ))
}

fn loaded(
    book: Resource<Book, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    filter: Signal<String>,
    query: Signal<String>,
    rewards_open: Signal<bool>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(text(move || {
            let current = book.get();
            let score = current.as_ref().map(|item| item.score).unwrap_or(0);
            let progress = current.map(|item| item.progress).unwrap_or_default();
            SharedString::from(format!("{} {score}  {progress}", text_key("collections.score")))
        }))
        .child(when(
            move || book.get().is_some_and(|item| !item.buffs.is_empty()),
            move || {
                text(move || {
                    book.get().map(|item| SharedString::from(item.buffs)).unwrap_or_default()
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            move || book.get().is_some_and(|item| item.can_claim),
            move || {
                button(move || text_key("collections.claim"), ButtonVariant::Primary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        super::confirm::confirm(
                            "collections.confirm.title",
                            "collections.confirm.body",
                            "claim",
                            "collections.claim",
                            move |accepted| {
                                if accepted {
                                    run_claim(pending, failure, page, book);
                                }
                            },
                        );
                    })
            },
        ))
        .child(
            button(move || text_key("collections.rewards"), ButtonVariant::Quiet)
                .selected(move || rewards_open.get())
                .on_press(move || rewards_open.set(!rewards_open.get_untracked())),
        )
        .child(when(move || rewards_open.get(), move || milestone_list(book)))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(book_filter_button(filter, "all", "collections.all"))
                .child(book_filter_button(filter, "active", "collections.active"))
                .child(book_filter_button(filter, "pending", "collections.pending"))
                .child(book_filter_button(filter, "missing", "collections.missing")),
        )
        .child(field(
            move || query.get(),
            move || text_key("collections.bookSearch").to_string(),
            move |text| query.set(text.to_owned()),
        ))
        .child(when(
            move || {
                book.get().is_some_and(|item| !item.series.is_empty())
                    && visible_book(
                        &book.get().map(|item| item.series).unwrap_or_default(),
                        &filter.get(),
                        &query.get(),
                    )
                    .is_empty()
            },
            move || notice(move || text_key("collections.noMatch")),
        ))
        .child(each(
            move || {
                visible_book(
                    &book.get().map(|item| item.series).unwrap_or_default(),
                    &filter.get(),
                    &query.get(),
                )
            },
            |line: &SeriesLine| line.id.to_string(),
            move |line, _| series_block(line.get_untracked(), pending, failure, page, book),
        ))
}

fn book_filter_button(
    filter: Signal<String>,
    id: &'static str,
    label: &'static str,
) -> impl IntoElement {
    button(move || text_key(label), ButtonVariant::Quiet)
        .selected(move || filter.get() == id)
        .on_press(move || filter.set(id.to_owned()))
}

fn series_block(
    line: SeriesLine,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    book: Resource<Book, String>,
) -> impl IntoElement {
    let name = SharedString::from(line.name);
    let rarity = SharedString::from(line.rarity);
    let detail = SharedString::from(line.detail);
    let rarity_shown = !rarity.is_empty();
    let detail_shown = !detail.is_empty();
    let progress = SharedString::from(line.progress);
    let pieces = line.pieces;
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(text(name).text_sm().font_weight(FontWeight::SEMIBOLD))
                .child(when(move || rarity_shown, {
                    let rarity = rarity.clone();
                    move || {
                        text(rarity.clone())
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                    }
                }))
                .child(
                    text(progress).text_sm().text_color(move || nsh_base::theme().muted_foreground),
                ),
        )
        .child(when(move || detail_shown, {
            let detail = detail.clone();
            move || {
                text(detail.clone())
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            }
        }))
        .child(each(
            move || pieces.clone(),
            |piece: &Piece| piece.id.to_string(),
            move |piece, _| piece_row(piece.get_untracked(), pending, failure, page, book),
        ))
}

fn piece_row(
    piece: Piece,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    book: Resource<Book, String>,
) -> impl IntoElement {
    let name = piece.name.clone();
    let description = piece.description.clone();
    let description_shown = !description.is_empty();
    let stars = piece.stars.clone();
    let price = piece.price.clone();
    let price_label = price.clone();
    let activated = piece.star > 0;
    let fresh = piece.fresh;
    let own = piece.own;
    let score = piece.score;
    let status_key = piece.status_key;
    let block_key = piece.block_key;
    let can_upgrade = piece.can_upgrade;
    let action_key = if activated { "collections.upgrade" } else { "collections.activate" };
    let displayed = piece.displayed;
    let offer = piece.clone();
    view()
        .gap_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(text(name).text_sm())
                .child(when(
                    move || fresh,
                    move || badge(move || text_key("collections.fresh"), BadgeTone::Success),
                ))
                .child(badge(
                    move || {
                        let label = text_key(status_key);
                        if own > 0 && status_key == "collections.pending" {
                            SharedString::from(format!(
                                "{label} · {}",
                                text_key("collections.held").replace("{count}", &own.to_string())
                            ))
                        } else {
                            label
                        }
                    },
                    if activated { BadgeTone::Success } else { BadgeTone::Neutral },
                )),
        )
        .child(when(move || description_shown, {
            let description = description;
            move || {
                text(description.clone())
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            }
        }))
        .child(when(move || activated, {
            let stars = stars.clone();
            move || {
                let stars = stars.clone();
                text(move || {
                    let shown = stars.clone();
                    if score > 0 {
                        SharedString::from(format!(
                            "{shown} · {}",
                            text_key("collections.points").replace("{score}", &score.to_string())
                        ))
                    } else {
                        SharedString::from(shown)
                    }
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
        .child(when(
            move || !price.is_empty(),
            move || {
                text(price_label.clone())
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(move || can_upgrade, {
            let offer = offer.clone();
            move || {
                let offer = offer.clone();
                button(move || text_key(action_key), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        let offer = offer.clone();
                        super::confirm::confirm(
                            "collections.confirmUpgrade.title",
                            "collections.confirmUpgrade.body",
                            "upgrade",
                            action_key,
                            move |accepted| {
                                if accepted {
                                    run_upgrade(offer, pending, failure, page, book);
                                }
                            },
                        );
                    })
            }
        }))
        .child(when(move || activated, {
            let offer = offer.clone();
            move || {
                let offer = offer.clone();
                let label = if displayed { "collections.hide" } else { "collections.display" };
                button(move || text_key(label), ButtonVariant::Quiet)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        let offer = offer.clone();
                        super::confirm::confirm(
                            "collections.confirmDisplay.title",
                            "collections.confirmDisplay.body",
                            "display",
                            label,
                            move |accepted| {
                                if accepted {
                                    run_display(offer, !displayed, pending, failure, page, book);
                                }
                            },
                        );
                    })
            }
        }))
}

fn run_upgrade(
    piece: Piece,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    book: Resource<Book, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(format!("upgrade:{}", piece.id)));
    failure.set(None);
    let goods_id = piece.id;
    let star = piece.star;
    let costs = piece
        .costs
        .iter()
        .map(|(item_id, count)| serde_json::json!({ "itemId": item_id, "count": count }))
        .collect::<Vec<_>>();
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::collections::upgrade(
                &context,
                &id,
                &serde_json::json!({
                    "collectionId": goods_id,
                    "expectedStarLevel": star,
                    "expectedCosts": costs,
                }),
            )
            .await
        })
        .await;
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(None);
        book.refetch();
    });
}

fn run_display(
    piece: Piece,
    displayed: bool,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    book: Resource<Book, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(format!("display:{}", piece.id)));
    failure.set(None);
    let goods_id = piece.id;
    let expected = piece.display_id;
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::collections::display(
                &context,
                &id,
                &serde_json::json!({
                    "collectionId": goods_id,
                    "displayed": displayed,
                    "expectedDisplayItemId": expected,
                }),
            )
            .await
        })
        .await;
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(None);
        book.refetch();
    });
}

fn run_claim(
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
    book: Resource<Book, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some("claim".to_owned()));
    failure.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result =
            session::call(async move { qq_farm_app::collections::claim(&context, &id).await })
                .await;
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(None);
        book.refetch();
    });
}

fn running(page: CollectionPage) -> bool {
    page.account_id
        .get()
        .zip(page.rows.get())
        .is_some_and(|(id, rows)| account_is_running(&rows, &id))
}

fn running_account(page: CollectionPage) -> Option<String> {
    running(page).then(|| page.account_id.get()).flatten()
}

fn book_from(value: &Value) -> Book {
    let series_rows = value.get("series").and_then(Value::as_array);
    let display_id = value.pointer("/display/itemId").and_then(Value::as_i64).unwrap_or(0);
    let series = series_rows
        .map(|rows| rows.iter().filter_map(|row| series_from(row, display_id)).collect())
        .unwrap_or_default();
    let activated = value.get("activatedCount").and_then(Value::as_i64).unwrap_or(0);
    let total = value.get("totalCount").and_then(Value::as_i64).unwrap_or(0);
    Book {
        score: value.get("score").and_then(Value::as_i64).unwrap_or(0),
        progress: format!("{activated}/{total}"),
        buffs: joined_text(value.get("currentBuffs"), "description"),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
        display_id,
        series,
        milestones: value
            .get("milestones")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(milestone_from).collect())
            .unwrap_or_default(),
    }
}

fn milestone_from(value: &Value) -> Option<MilestoneLine> {
    let level = value.get("level").and_then(Value::as_i64).filter(|level| *level > 0)?;
    let score = value.get("score").and_then(Value::as_i64).unwrap_or(0);
    let claimed = value.get("claimed").and_then(Value::as_bool).unwrap_or(false);
    let reached = value.get("reached").and_then(Value::as_bool).unwrap_or(false);
    let buffs = value
        .get("buffs")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get("description").and_then(Value::as_str))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default();
    let rewards = value
        .get("rewards")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("");
                    if name.is_empty() {
                        return None;
                    }
                    let count = row.get("count").and_then(Value::as_i64).unwrap_or(0);
                    Some(format!("{name} ×{count}"))
                })
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default();
    let detail = [buffs, rewards].into_iter().filter(|text| !text.is_empty()).collect::<Vec<_>>();
    Some(MilestoneLine {
        level,
        score,
        state_key: if claimed {
            "collections.claimed"
        } else if reached {
            "collections.ready"
        } else {
            "collections.unreached"
        },
        detail: detail.join(" · "),
    })
}

fn milestone_list(book: Resource<Book, String>) -> impl IntoElement {
    view().gap_2().child(each(
        move || book.get().map(|item| item.milestones).unwrap_or_default(),
        |line: &MilestoneLine| line.level.to_string(),
        move |line, _| {
            let line = line.get_untracked();
            let score = line.score;
            let state_key = line.state_key;
            let ready = state_key == "collections.ready";
            let detail = SharedString::from(line.detail);
            let shown = !detail.is_empty();
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .py_1()
                .child(
                    text(move || {
                        SharedString::from(
                            text_key("collections.points").replace("{score}", &score.to_string()),
                        )
                    })
                    .text_sm(),
                )
                .child(badge(
                    move || text_key(state_key),
                    if ready { BadgeTone::Success } else { BadgeTone::Neutral },
                ))
                .child(when(move || shown, {
                    let detail = detail;
                    move || {
                        text(detail.clone())
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                    }
                }))
        },
    ))
}

fn piece_rows(series: &Value, display_id: i64) -> Vec<Piece> {
    series
        .get("items")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(|row| piece_from(row, display_id)).collect())
        .unwrap_or_default()
}

fn piece_from(value: &Value, display_id: i64) -> Option<Piece> {
    let activated = value.get("activated").and_then(Value::as_bool).unwrap_or(false);
    let can_upgrade = value.get("canUpgrade").and_then(Value::as_bool).unwrap_or(false);
    let fresh = value.get("isNew").and_then(Value::as_bool).unwrap_or(false);
    let own = value.get("ownCount").and_then(Value::as_i64).unwrap_or(0).max(0);
    let unlocked = value.get("unlocked").and_then(Value::as_bool).unwrap_or(true);
    let score = value.get("score").and_then(Value::as_i64).unwrap_or(0).max(0);
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let star = value.get("starLevel").and_then(Value::as_i64).unwrap_or(0);
    let costs = value
        .get("nextStarCosts")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|cost| {
                    Some((
                        cost.get("itemId").and_then(Value::as_i64)?,
                        cost.get("count").and_then(Value::as_i64).unwrap_or(0),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let price = value
        .get("nextStarCosts")
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
    Some(Piece {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        description: text_of(value, "description"),
        stars: format!("{star}/5"),
        star,
        own,
        score,
        status_key: collection_status(activated, own),
        block_key: upgrade_block(star, own, unlocked, can_upgrade),
        can_upgrade,
        displayed: value.get("displayed").and_then(Value::as_bool).unwrap_or(false),
        fresh,
        price,
        costs,
        display_id,
    })
}

fn series_from(value: &Value, display_id: i64) -> Option<SeriesLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let activated = value.get("activatedCount").and_then(Value::as_i64).unwrap_or(0);
    let total = value.get("totalCount").and_then(Value::as_i64).unwrap_or(0);
    let rarity = text_of(value, "rarityName");
    let description = text_of(value, "description");
    let rules = value
        .get("mutationRules")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().map(rule_text).filter(|text| !text.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default()
        .join(" · ");
    let detail = match (description.is_empty(), rules.is_empty()) {
        (true, true) => String::new(),
        (false, true) => description,
        (true, false) => rules,
        (false, false) => format!("{description} · {rules}"),
    };
    Some(SeriesLine {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        rarity,
        detail,
        progress: format!("{activated}/{total}"),
        pieces: piece_rows(value, display_id),
    })
}

fn joined_text(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get(key).and_then(Value::as_str))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

fn upgrade_block(star: i64, own: i64, unlocked: bool, can_upgrade: bool) -> &'static str {
    if can_upgrade || star >= 5 {
        return "";
    }
    if !unlocked {
        "collections.needUnlock"
    } else if own > 0 {
        "collections.needCopies"
    } else {
        ""
    }
}

fn collection_status(activated: bool, own: i64) -> &'static str {
    if activated {
        "collections.active"
    } else if own > 0 {
        "collections.pending"
    } else {
        "collections.missing"
    }
}

fn visible_book(series: &[SeriesLine], filter: &str, query: &str) -> Vec<SeriesLine> {
    let query = query.trim();
    series
        .iter()
        .filter_map(|line| {
            let matches_series = query.is_empty() || line.name.contains(query);
            let pieces = line
                .pieces
                .iter()
                .filter(|piece| {
                    piece_matches(piece, filter) && (matches_series || piece.name.contains(query))
                })
                .cloned()
                .collect::<Vec<_>>();
            if pieces.is_empty() {
                None
            } else {
                Some(SeriesLine { pieces, ..line.clone() })
            }
        })
        .collect()
}

fn piece_matches(piece: &Piece, filter: &str) -> bool {
    match filter {
        "active" => piece.status_key == "collections.active",
        "pending" => piece.status_key == "collections.pending",
        "missing" => piece.status_key == "collections.missing",
        _ => true,
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Catalog {
    normal: Vec<CatalogEffect>,
    decoration: Vec<CatalogEffect>,
    series: Vec<CatalogSeries>,
}

#[derive(Clone, PartialEq, Eq)]
struct CatalogEffect {
    id: i64,
    title: String,
    detail: String,
}

#[derive(Clone, PartialEq, Eq)]
struct CatalogSeries {
    id: i64,
    title: String,
    detail: String,
    items: String,
}

fn catalog_body(
    catalog: Resource<Catalog, String>,
    tab: Signal<String>,
    query: Signal<String>,
) -> View {
    view()
        .gap_3()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(catalog_tab(tab.clone(), "normal", "collections.normal"))
                .child(catalog_tab(tab.clone(), "decoration", "collections.decoration"))
                .child(catalog_tab(tab.clone(), "collections", "collections.mutations")),
        )
        .child(field(
            move || query.get(),
            move || text_key("collections.search").to_string(),
            move |text| query.set(text.to_owned()),
        ))
        .child(when(
            move || catalog.loading() && catalog.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || catalog.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(notice(move || {
                        catalog.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || catalog.refetch()),
                    )
            },
        ))
        .child(when(move || tab.get() != "collections", move || effect_list(catalog, tab, query)))
        .child(when(move || tab.get() == "collections", move || series_list(catalog, query)))
}

fn catalog_tab(tab: Signal<String>, id: &'static str, label: &'static str) -> impl IntoElement {
    button(move || text_key(label), ButtonVariant::Quiet)
        .selected(move || tab.get() == id)
        .on_press(move || tab.set(id.to_owned()))
}

fn effect_list(
    catalog: Resource<Catalog, String>,
    tab: Signal<String>,
    query: Signal<String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || {
                catalog.get().is_some_and(|catalog| {
                    visible_effects(&catalog, &tab.get(), &query.get()).is_empty()
                })
            },
            move || notice(move || text_key("collections.catalogEmpty")),
        ))
        .child(each(
            move || {
                catalog
                    .get()
                    .map(|catalog| visible_effects(&catalog, &tab.get(), &query.get()))
                    .unwrap_or_default()
            },
            |effect: &CatalogEffect| effect.id.to_string(),
            move |effect, _| {
                let effect = effect.get_untracked();
                let title = SharedString::from(effect.title);
                let detail = SharedString::from(effect.detail);
                view().gap_1().py_2().child(text(title).text_sm()).child(
                    text(detail).text_sm().text_color(move || nsh_base::theme().muted_foreground),
                )
            },
        ))
}

fn series_list(catalog: Resource<Catalog, String>, query: Signal<String>) -> View {
    view()
        .gap_2()
        .child(when(
            move || {
                catalog
                    .get()
                    .is_some_and(|catalog| visible_series(&catalog, &query.get()).is_empty())
            },
            move || notice(move || text_key("collections.catalogEmpty")),
        ))
        .child(each(
            move || {
                catalog
                    .get()
                    .map(|catalog| visible_series(&catalog, &query.get()))
                    .unwrap_or_default()
            },
            |series: &CatalogSeries| series.id.to_string(),
            move |series, _| {
                let series = series.get_untracked();
                let title = SharedString::from(series.title);
                let detail = SharedString::from(series.detail);
                let items = SharedString::from(series.items);
                view()
                    .gap_1()
                    .py_2()
                    .child(text(title).text_sm())
                    .child(
                        text(detail)
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground),
                    )
                    .child(
                        text(items)
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground),
                    )
            },
        ))
}

fn visible_effects(catalog: &Catalog, tab: &str, query: &str) -> Vec<CatalogEffect> {
    let rows = if tab == "decoration" { &catalog.decoration } else { &catalog.normal };
    rows.iter()
        .filter(|row| contains_query(query, &format!("{} {}", row.title, row.detail)))
        .cloned()
        .collect()
}

fn visible_series(catalog: &Catalog, query: &str) -> Vec<CatalogSeries> {
    catalog
        .series
        .iter()
        .filter(|row| contains_query(query, &format!("{} {} {}", row.title, row.detail, row.items)))
        .cloned()
        .collect()
}

fn contains_query(query: &str, text: &str) -> bool {
    let query = query.trim();
    query.is_empty() || text.contains(query)
}

fn catalog_from(value: &Value) -> Catalog {
    Catalog {
        normal: effect_rows(value.get("normal")),
        decoration: effect_rows(value.get("decoration")),
        series: value
            .get("collections")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(series_row).collect())
            .unwrap_or_default(),
    }
}

fn effect_rows(value: Option<&Value>) -> Vec<CatalogEffect> {
    value
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(effect_row).collect())
        .unwrap_or_default()
}

fn effect_row(value: &Value) -> Option<CatalogEffect> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = text_of(value, "name");
    let effect = text_of(value, "effect");
    let description = text_of(value, "description");
    let odds = odds_text(value);
    let title = if effect.is_empty() { name.clone() } else { format!("{name} · {effect}") };
    let detail = if odds.is_empty() { description } else { format!("{description} · {odds}") };
    Some(CatalogEffect { id, title, detail })
}

fn series_row(value: &Value) -> Option<CatalogSeries> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = text_of(value, "name");
    let rarity = text_of(value, "rarityName");
    let description = text_of(value, "description");
    let rules = value
        .get("mutationRules")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().map(rule_text).filter(|text| !text.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default()
        .join(" · ");
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|item| text_of(item, "name"))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .join("、");
    let title = if rarity.is_empty() { name } else { format!("{name} · {rarity}") };
    Some(CatalogSeries {
        id,
        title,
        detail: if rules.is_empty() { description } else { format!("{description} · {rules}") },
        items,
    })
}

fn rule_text(value: &Value) -> String {
    let name = text_of(value, "name");
    if name.is_empty() {
        return String::new();
    }
    let probability = value.get("probability").and_then(Value::as_f64).unwrap_or(0.0);
    let guarantee = value.get("guarantee").and_then(Value::as_i64).unwrap_or(0);
    format!("{name} {}% · {guarantee}", percent_text(probability))
}

fn odds_text(value: &Value) -> String {
    value
        .get("probabilities")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let percent = row.get("percent").and_then(Value::as_f64)?;
                    let rarity = row.get("rarity").and_then(Value::as_i64).unwrap_or(0);
                    let label = match rarity {
                        1 => "普通",
                        2 => "稀有",
                        3 => "珍品",
                        4 => "天工",
                        _ => "",
                    };
                    let number = percent_text(percent);
                    Some(if label.is_empty() {
                        format!("{number}%")
                    } else {
                        format!("{label} {number}%")
                    })
                })
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

fn percent_text(value: f64) -> String {
    let scaled = (value * 10.0).round() / 10.0;
    if (scaled - scaled.round()).abs() < 0.05 {
        format!("{}", scaled.round() as i64)
    } else {
        format!("{scaled:.1}")
    }
}

fn text_of(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{book_from, catalog_from, contains_query, visible_book};

    #[test]
    fn book_reads_score_claim_and_series() {
        let book = book_from(&json!({
            "score": 400,
            "activatedCount": 1,
            "totalCount": 6,
            "canClaim": true,
            "display": { "itemId": 710001 },
            "milestones": [{
                "level": 1,
                "score": 400,
                "claimed": false,
                "reached": true,
                "buffs": [{ "description": "经验 +1%" }],
                "rewards": [{ "name": "礼包", "count": 1 }]
            }, {
                "level": 2,
                "score": 1000,
                "claimed": true,
                "reached": true
            }],
            "currentBuffs": [{ "name": "经验", "description": "经验 +1%" }],
            "series": [{
                "id": 1001,
                "name": "氧气精灵",
                "rarityName": "普通",
                "description": "种植普通作物",
                "mutationRules": [{ "name": "4小时·普通", "probability": 1.2, "guarantee": 120 }],
                "activatedCount": 1,
                "totalCount": 6,
                "items": [{
                    "id": 710001,
                    "name": "氧气",
                    "description": "可收进珍藏室",
                    "starLevel": 1,
                    "activated": true,
                    "canUpgrade": true,
                    "score": 12,
                    "displayed": true,
                    "nextStarCosts": [{ "itemId": 710001, "count": 2, "name": "氧气" }]
                }, {
                    "id": 710002,
                    "name": "新藏品",
                    "starLevel": 0,
                    "activated": false,
                    "isNew": true
                }, {
                    "id": 710003,
                    "name": "待激活",
                    "starLevel": 0,
                    "activated": false,
                    "ownCount": 2
                }, {
                    "id": 710004,
                    "name": "未获得",
                    "starLevel": 0
                }, {
                    "id": 710005,
                    "name": "未解锁",
                    "starLevel": 1,
                    "activated": true,
                    "unlocked": false,
                    "canUpgrade": false
                }]
            }]
        }));
        assert_eq!(book.score, 400);
        assert_eq!(book.progress, "1/6");
        assert!(book.can_claim);
        assert_eq!(book.milestones.len(), 2);
        assert_eq!(book.milestones[0].state_key, "collections.ready");
        assert_eq!(book.milestones[0].detail, "经验 +1% · 礼包 ×1");
        assert_eq!(book.milestones[1].state_key, "collections.claimed");
        assert_eq!(book.display_id, 710001);
        assert_eq!(book.buffs, "经验 +1%");
        assert_eq!(book.series[0].name, "氧气精灵");
        assert_eq!(book.series[0].rarity, "普通");
        assert!(book.series[0].detail.contains("种植普通作物"));
        assert!(book.series[0].detail.contains("1.2%"));
        assert_eq!(book.series[0].progress, "1/6");
        assert_eq!(book.series[0].pieces[0].costs, vec![(710001, 2)]);
        assert!(!book.series[0].pieces[0].fresh);
        assert_eq!(book.series[0].pieces[0].status_key, "collections.active");
        assert_eq!(book.series[0].pieces[0].score, 12);
        assert_eq!(book.series[0].pieces[0].block_key, "");
        assert_eq!(book.series[0].pieces[2].block_key, "collections.needCopies");
        assert_eq!(book.series[0].pieces[4].block_key, "collections.needUnlock");
        assert!(book.series[0].pieces[1].fresh);
        assert_eq!(book.series[0].pieces[1].status_key, "collections.missing");
        assert_eq!(book.series[0].pieces[2].status_key, "collections.pending");
        assert_eq!(book.series[0].pieces[2].own, 2);
        assert_eq!(visible_book(&book.series, "pending", "").len(), 1);
        assert_eq!(visible_book(&book.series, "pending", "")[0].pieces[0].name, "待激活");
        assert_eq!(visible_book(&book.series, "all", "未获得")[0].pieces.len(), 1);
        let catalog = catalog_from(&json!({
            "normal": [{
                "id": 1,
                "name": "冰冻",
                "effect": "售价 ×3",
                "description": "生长",
                "probabilities": [{ "percent": 3.2 }]
            }],
            "decoration": [],
            "collections": [{
                "id": 1001,
                "name": "氧气精灵",
                "rarityName": "普通",
                "description": "种植",
                "mutationRules": [{ "name": "4小时·普通", "probability": 1.2, "guarantee": 120 }],
                "items": [{ "name": "氧气" }]
            }]
        }));
        assert_eq!(catalog.normal[0].title, "冰冻 · 售价 ×3");
        assert!(catalog.normal[0].detail.contains("3.2%"));
        assert_eq!(catalog.series[0].items, "氧气");
        assert!(catalog.series[0].detail.contains("1.2%"));
        assert!(contains_query("冰", "冰冻"));
        assert!(!contains_query("装扮", "冰冻"));
        assert_eq!(book.series[0].pieces[0].price, "氧气 2");
        assert_eq!(book.series[0].pieces[0].description, "可收进珍藏室");
    }
}
