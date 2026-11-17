use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, Copy)]
pub(crate) struct CollectionPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

#[derive(Clone, PartialEq, Eq)]
struct Book {
    score: i64,
    progress: String,
    can_claim: bool,
    display_id: i64,
    series: Vec<SeriesLine>,
    pieces: Vec<Piece>,
}

#[derive(Clone, PartialEq, Eq)]
struct SeriesLine {
    id: i64,
    name: String,
    progress: String,
}

#[derive(Clone, PartialEq, Eq)]
struct Piece {
    id: i64,
    name: String,
    stars: String,
    star: i64,
    can_upgrade: bool,
    displayed: bool,
    price: String,
    costs: Vec<(i64, i64)>,
    display_id: i64,
}

pub(crate) fn page(page: CollectionPage) -> impl IntoElement {
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
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
        .child(failure_line(failure))
        .child(gate_notice(page.account_id, page.rows, "collections.needRunning"))
        .child(when(move || running(page), move || book_body(book, pending, failure, page)))
}

fn book_body(
    book: Resource<Book, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
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
        .child(when(move || book.get().is_some(), move || loaded(book, pending, failure, page)))
}

fn loaded(
    book: Resource<Book, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: CollectionPage,
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
        .child(each(
            move || book.get().map(|item| item.pieces).unwrap_or_default(),
            |piece: &Piece| piece.id.to_string(),
            move |piece, _| piece_row(piece.get_untracked(), pending, failure, page, book),
        ))
        .child(each(
            move || book.get().map(|item| item.series).unwrap_or_default(),
            |line: &SeriesLine| line.id.to_string(),
            move |line, _| {
                let line = line.get_untracked();
                view().gap_1().child(text(line.name).text_sm()).child(
                    text(line.progress)
                        .text_sm()
                        .text_color(move || nsh_base::theme().muted_foreground),
                )
            },
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
    let stars = piece.stars.clone();
    let price = piece.price.clone();
    let price_label = price.clone();
    let activated = piece.star > 0;
    let can_upgrade = piece.can_upgrade;
    let displayed = piece.displayed;
    let offer = piece.clone();
    view()
        .gap_1()
        .child(text(name).text_sm())
        .child(text(stars).text_sm().text_color(move || nsh_base::theme().muted_foreground))
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
                button(move || text_key("collections.upgrade"), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        let offer = offer.clone();
                        super::confirm::confirm(
                            "collections.confirmUpgrade.title",
                            "collections.confirmUpgrade.body",
                            "upgrade",
                            "collections.upgrade",
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
    let series =
        series_rows.map(|rows| rows.iter().filter_map(series_from).collect()).unwrap_or_default();
    let display_id = value.pointer("/display/itemId").and_then(Value::as_i64).unwrap_or(0);
    let pieces = series_rows
        .map(|rows| rows.iter().flat_map(|row| piece_rows(row, display_id)).collect())
        .unwrap_or_default();
    let activated = value.get("activatedCount").and_then(Value::as_i64).unwrap_or(0);
    let total = value.get("totalCount").and_then(Value::as_i64).unwrap_or(0);
    Book {
        score: value.get("score").and_then(Value::as_i64).unwrap_or(0),
        progress: format!("{activated}/{total}"),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
        display_id,
        series,
        pieces,
    }
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
    if !activated && !can_upgrade {
        return None;
    }
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
        stars: format!("{star}/5"),
        star,
        can_upgrade,
        displayed: value.get("displayed").and_then(Value::as_bool).unwrap_or(false),
        price,
        costs,
        display_id,
    })
}

fn series_from(value: &Value) -> Option<SeriesLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    let activated = value.get("activatedCount").and_then(Value::as_i64).unwrap_or(0);
    let total = value.get("totalCount").and_then(Value::as_i64).unwrap_or(0);
    Some(SeriesLine {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        progress: format!("{activated}/{total}"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::book_from;

    #[test]
    fn book_reads_score_claim_and_series() {
        let book = book_from(&json!({
            "score": 400,
            "activatedCount": 1,
            "totalCount": 6,
            "canClaim": true,
            "display": { "itemId": 710001 },
            "series": [{
                "id": 1001,
                "name": "氧气精灵",
                "activatedCount": 1,
                "totalCount": 6,
                "items": [{
                    "id": 710001,
                    "name": "氧气",
                    "starLevel": 1,
                    "activated": true,
                    "canUpgrade": true,
                    "displayed": true,
                    "nextStarCosts": [{ "itemId": 710001, "count": 2, "name": "氧气" }]
                }]
            }]
        }));
        assert_eq!(book.score, 400);
        assert_eq!(book.progress, "1/6");
        assert!(book.can_claim);
        assert_eq!(book.display_id, 710001);
        assert_eq!(book.series[0].name, "氧气精灵");
        assert_eq!(book.series[0].progress, "1/6");
        assert_eq!(book.pieces[0].costs, vec![(710001, 2)]);
        assert_eq!(book.pieces[0].price, "氧气 2");
    }
}
