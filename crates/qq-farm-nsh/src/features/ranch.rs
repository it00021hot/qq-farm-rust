use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::{account_gate, gate_notice, AccountGate};
use super::accounts::AccountRow;
use super::confirm;
use crate::i18n::text_key;
use crate::purchase_count::{purchase_max_count, PurchaseCost};
use crate::session;
use crate::ui::{badge, button, field, notice, BadgeTone, ButtonVariant};

#[derive(Clone, PartialEq, Eq)]
struct AnimalLine {
    id: i32,
    ranch_id: String,
    name: String,
    stage: String,
    can_harvest: bool,
    can_produce: bool,
    can_feed: bool,
    can_release: bool,
    limited_can: bool,
    slot: i32,
}

#[derive(Clone, PartialEq, Eq)]
struct FacilityLine {
    id: String,
    level: i64,
    module_type: i64,
    grass_room: i64,
    hazard_count: i64,
    kind: i64,
    open_room: i64,
    name: String,
    pen_count: i64,
    pen_capacity: i64,
    upgrade: String,
    animals: Vec<AnimalLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct CubLine {
    id: i64,
    name: String,
    kind: i64,
    count: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct ProductLine {
    uid: i64,
    name: String,
    count: i64,
    price: String,
}

#[derive(Clone, PartialEq, Eq)]
struct CanLine {
    uid: i64,
    name: String,
    count: i64,
    organic: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct GoodLine {
    id: i64,
    name: String,
    price: i64,
    balance: i64,
    max_count: i64,
    remaining: Option<i64>,
    available: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct ShopBoard {
    goods: Vec<GoodLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct RanchBoard {
    unlocked: bool,
    reason: String,
    grass: i64,
    normal_seconds: i64,
    organic_seconds: i64,
    cubs: Vec<CubLine>,
    products: Vec<ProductLine>,
    cans: Vec<CanLine>,
    facilities: Vec<FacilityLine>,
    pending_ops: Vec<PendingLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct PendingLine {
    id: String,
    label: String,
    costs: String,
    message: String,
}

pub(crate) struct RanchPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

pub(crate) fn page(page: RanchPage) -> impl IntoElement {
    let RanchPage { account_id, rows } = page;
    let pending = signal(false);
    let failure = signal(None::<String>);
    let grass_counts = signal(Vec::<(String, i64)>::new());
    let place_counts = signal(Vec::<(String, i64)>::new());
    let can_use_counts = signal(Vec::<(i64, i64)>::new());
    let board = resource(
        move || ready_id(account_id, rows).map(|id| (id, 0)),
        |id, _| async move { load_board(&id.0).await },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("ranch.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(gate_notice(account_id, rows, "ranch.needRunning"))
        .child(when(
            move || account_gate(account_id, rows) == AccountGate::Ready,
            move || {
                board_body(
                    board,
                    pending,
                    failure,
                    account_id,
                    grass_counts,
                    place_counts,
                    can_use_counts,
                )
            },
        ))
}

fn board_body(
    board: Resource<RanchBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    grass_counts: Signal<Vec<(String, i64)>>,
    place_counts: Signal<Vec<(String, i64)>>,
    can_use_counts: Signal<Vec<(i64, i64)>>,
) -> View {
    view()
        .gap_3()
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
        .child(when(
            move || board.loading() && board.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || board.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        board.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || board.refetch()),
                    )
            },
        ))
        .child(when(
            move || board.get().is_some_and(|board| !board.unlocked),
            move || notice(move || locked_reason(board)),
        ))
        .child(when(
            move || board.get().is_some_and(|board| board.unlocked),
            move || {
                unlocked_body(
                    board,
                    pending,
                    failure,
                    account_id,
                    grass_counts,
                    place_counts,
                    can_use_counts,
                )
            },
        ))
}

fn unlocked_body(
    board: Resource<RanchBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    grass_counts: Signal<Vec<(String, i64)>>,
    place_counts: Signal<Vec<(String, i64)>>,
    can_use_counts: Signal<Vec<(i64, i64)>>,
) -> View {
    view()
        .gap_3()
        .child(text(move || grass_label(board)).text_sm())
        .child(pending_shelf(pending, failure, account_id, board))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("ranch.harvestAll"), ButtonVariant::Primary)
                        .disabled(move || pending.get())
                        .on_press(move || harvest(None, None, pending, failure, account_id, board)),
                )
                .child(
                    button(move || text_key("ranch.produceAll"), ButtonVariant::Secondary)
                        .disabled(move || pending.get())
                        .on_press(move || produce(None, None, pending, failure, account_id, board)),
                )
                .child(daily_share_button(pending, failure, account_id, board))
                .child(when(
                    move || {
                        board.get().is_some_and(|board| {
                            board.facilities.iter().any(|facility| facility.hazard_count > 0)
                        })
                    },
                    move || {
                        button(move || text_key("ranch.clean"), ButtonVariant::Secondary)
                            .disabled(move || pending.get())
                            .on_press(move || clean(pending, failure, account_id, board))
                    },
                ))
                .child(
                    button(move || text_key("ranch.feedAllNormal"), ButtonVariant::Secondary)
                        .disabled(move || {
                            pending.get()
                                || !board.get().is_some_and(|board| feed_all_ready(&board, false))
                        })
                        .on_press(move || feed(None, None, 2, pending, failure, account_id, board)),
                )
                .child(
                    button(move || text_key("ranch.feedAllOrganic"), ButtonVariant::Secondary)
                        .disabled(move || {
                            pending.get()
                                || !board.get().is_some_and(|board| feed_all_ready(&board, true))
                        })
                        .on_press(move || feed(None, None, 1, pending, failure, account_id, board)),
                )
                .child(
                    button(move || text_key("common.retry"), ButtonVariant::Secondary)
                        .disabled(move || pending.get())
                        .on_press(move || board.refetch()),
                ),
        )
        .child(when(
            move || board.get().is_some_and(|board| board.facilities.is_empty()),
            move || notice(move || text_key("ranch.empty")),
        ))
        .child(each(
            move || pen_groups(board.get().map(|board| board.facilities).unwrap_or_default()),
            |group: &PenGroup| group.module_type,
            move |group, _| {
                let current = group.get_untracked();
                let title = SharedString::from(current.name.clone());
                let summary = format!("{} / {}", current.count, current.capacity);
                let empty = current.capacity > 0 && current.count == 0;
                view()
                    .gap_2()
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(text(title).text_sm().font_weight(FontWeight::SEMIBOLD))
                            .child(
                                text(summary)
                                    .text_sm()
                                    .text_color(move || nsh_base::theme().muted_foreground),
                            ),
                    )
                    .child(when(move || empty, move || notice(move || text_key("ranch.penEmpty"))))
                    .child(each(
                        move || group.get().facilities.clone(),
                        |facility: &FacilityLine| facility.id.clone(),
                        move |facility, _| {
                            let cubs = board.get().map(|board| board.cubs).unwrap_or_default();
                            facility_line(
                                facility.get_untracked(),
                                cubs,
                                pending,
                                failure,
                                account_id,
                                board,
                                grass_counts,
                                place_counts,
                            )
                        },
                    ))
            },
        ))
        .child(product_shelf(pending, failure, account_id, board))
        .child(can_shelf(can_use_counts, pending, failure, account_id, board))
        .child(shop_shelf(account_id, pending, failure, board))
        .child(can_mall_shelf(account_id, pending, failure, board))
        .child(market_shelf(account_id, pending, failure, board))
        .child(task_shelf(account_id, pending, failure, board))
        .child(book_shelf(account_id, pending, failure, board))
        .child(friend_shelf(account_id, pending, failure, board))
}

fn pending_shelf(
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view().gap_2().child(each(
        move || board.get().map(|board| board.pending_ops).unwrap_or_default(),
        |row: &PendingLine| row.id.clone(),
        move |row, _| {
            let current = row.get_untracked();
            let operation_id = current.id.clone();
            let label = current.label.clone();
            let costs = current.costs.clone();
            let message = current.message.clone();
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(text(move || {
                    if costs.is_empty() {
                        SharedString::from(format!("{label}\n{message}"))
                    } else {
                        SharedString::from(format!("{label}  {costs}\n{message}"))
                    }
                }))
                .child(
                    button(move || text_key("ranch.acknowledge"), ButtonVariant::Secondary)
                        .disabled(move || pending.get())
                        .on_press(move || {
                            acknowledge_pending(
                                operation_id.clone(),
                                pending,
                                failure,
                                account_id,
                                board,
                            )
                        }),
                )
        },
    ))
}

fn acknowledge_pending(
    operation_id: String,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmAck.title",
        "ranch.confirmAck.body",
        "acknowledge",
        "ranch.acknowledge",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, move |_context, id| async move {
                qq_farm_app::ranch::acknowledge(&id, &operation_id)
            });
        },
    );
}

fn can_shelf(
    can_use_counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view().gap_2().child(each(
        move || board.get().map(|board| board.cans).unwrap_or_default(),
        |can: &CanLine| can.uid,
        move |can, _| {
            let can = can.get_untracked();
            let uid = can.uid;
            let owned = can.count;
            let name = SharedString::from(can.name);
            let organic = can.organic;
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(text(name).text_sm())
                .child(text(move || SharedString::from(owned.to_string())).text_sm())
                .child(
                    text(move || {
                        text_key(if organic { "ranch.feedOrganic" } else { "ranch.feedNormal" })
                    })
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground),
                )
                .child(when(
                    move || can_use_limit(owned) > 1,
                    move || shop_count_stepper(uid, can_use_limit(owned), can_use_counts, pending),
                ))
                .child(
                    button(move || text_key("ranch.useCan"), ButtonVariant::Quiet)
                        .disabled(move || pending.get() || can_use_limit(owned) < 1)
                        .on_press(move || {
                            let count =
                                chosen_shop_count(can_use_counts, uid, can_use_limit(owned));
                            use_can(uid, count, pending, failure, account_id, board);
                        }),
                )
        },
    ))
}

fn product_shelf(
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || board.get().is_some_and(|board| !board.products.is_empty()),
            move || {
                button(move || text_key("ranch.sellAll"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || sell_all(pending, failure, account_id, board))
            },
        ))
        .child(each(
            move || board.get().map(|board| board.products).unwrap_or_default(),
            |product: &ProductLine| product.uid,
            move |product, _| {
                let product = product.get_untracked();
                let uid = product.uid;
                let count = product.count;
                let name = SharedString::from(product.name);
                let price = SharedString::from(product.price);
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(text(name).text_sm())
                    .child(text(move || SharedString::from(count.to_string())).text_sm())
                    .child(
                        text(price)
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground),
                    )
                    .child(
                        button(move || text_key("ranch.sell"), ButtonVariant::Quiet)
                            .disabled(move || pending.get())
                            .on_press(move || {
                                sell_stack(uid, count, pending, failure, account_id, board)
                            }),
                    )
            },
        ))
}

#[derive(Clone, PartialEq, Eq)]
struct MarketLine {
    id: i64,
    name: String,
    price: i64,
    price_name: String,
    balance: i64,
    max_count: i64,
    remaining: Option<i64>,
    available: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct MarketBoard {
    goods: Vec<MarketLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct TaskLine {
    id: i64,
    name: String,
    progress: i64,
    total: i64,
    can_claim: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TaskBoard {
    tasks: Vec<TaskLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct BookAnimal {
    id: i64,
    name: String,
    unlocked: bool,
    weight: String,
    tier: String,
    giant_unlocked: bool,
    harvest: i64,
    fresh: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct BookMutation {
    name: String,
    detail: String,
}

#[derive(Clone, PartialEq, Eq)]
struct BookLevel {
    id: i64,
    score: i64,
    state_key: &'static str,
    detail: String,
}

#[derive(Clone, PartialEq, Eq)]
struct BookBoard {
    level: i64,
    progress: i64,
    next_progress: i64,
    can_claim: bool,
    score: i64,
    giant_claim: bool,
    reward_text: String,
    animals: Vec<BookAnimal>,
    mutations: Vec<BookMutation>,
    levels: Vec<BookLevel>,
}

fn book_shelf(
    account_id: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let open = signal(false);
    let book_filter = signal("all".to_owned());
    let book_query = signal(String::new());
    let rewards_open = signal(false);
    let catalog = resource(
        move || open.get().then(|| account_id.get()).flatten().map(|id| (id, 0_i64)),
        |(id, _), _| async move { load_book(&id).await },
    );
    view()
        .gap_2()
        .child(
            button(move || text_key("ranch.book"), ButtonVariant::Quiet)
                .selected(move || open.get())
                .on_press(move || open.set(!open.get_untracked())),
        )
        .child(when(
            move || open.get(),
            move || {
                book_body(
                    catalog,
                    pending,
                    failure,
                    account_id,
                    board,
                    book_filter,
                    book_query,
                    rewards_open,
                )
            },
        ))
}

fn book_body(
    catalog: Resource<BookBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
    filter: Signal<String>,
    query: Signal<String>,
    rewards_open: Signal<bool>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || catalog.loading() && catalog.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || catalog.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        catalog.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || catalog.refetch()),
                    )
            },
        ))
        .child(when(
            move || catalog.get().is_some_and(|book| book.animals.is_empty()),
            move || notice(move || text_key("ranch.bookEmpty")),
        ))
        .child(when(
            move || catalog.get().is_some(),
            move || book_summary(catalog, pending, failure, account_id, board),
        ))
        .child(when(
            move || {
                catalog.get().is_some_and(|book| book.can_claim && !book.reward_text.is_empty())
            },
            move || {
                text(move || {
                    catalog
                        .get()
                        .map(|book| SharedString::from(book.reward_text))
                        .unwrap_or_default()
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(
            button(move || text_key("ranch.bookRewards"), ButtonVariant::Quiet)
                .selected(move || rewards_open.get())
                .on_press(move || rewards_open.set(!rewards_open.get_untracked())),
        )
        .child(when(move || rewards_open.get(), move || book_level_list(catalog)))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(book_filter_button(filter, "all", "ranch.bookAll"))
                .child(book_filter_button(filter, "owned", "ranch.bookOwned"))
                .child(book_filter_button(filter, "missing", "ranch.bookMissing")),
        )
        .child(field(
            move || query.get(),
            move || text_key("ranch.bookSearch").to_string(),
            move |text| query.set(text.to_owned()),
        ))
        .child(when(
            move || {
                let animals = catalog.get().map(|book| book.animals).unwrap_or_default();
                !animals.is_empty()
                    && visible_animals(&animals, &filter.get(), &query.get()).is_empty()
            },
            move || notice(move || text_key("ranch.bookNoMatch")),
        ))
        .child(each(
            move || catalog.get().map(|book| book.mutations).unwrap_or_default(),
            |row: &BookMutation| row.name.clone(),
            move |row, _| {
                let row = row.get_untracked();
                let name = SharedString::from(row.name);
                let detail = SharedString::from(row.detail);
                view().flex_row().items_center().gap_3().child(text(name).text_sm()).child(
                    text(detail).text_sm().text_color(move || nsh_base::theme().muted_foreground),
                )
            },
        ))
        .child(each(
            move || {
                visible_animals(
                    &catalog.get().map(|book| book.animals).unwrap_or_default(),
                    &filter.get(),
                    &query.get(),
                )
            },
            |animal: &BookAnimal| animal.id,
            move |animal, _| {
                let animal = animal.get_untracked();
                let name = SharedString::from(animal.name);
                let collected = animal.unlocked;
                let giant_unlocked = animal.giant_unlocked;
                let harvest = animal.harvest;
                let fresh = animal.fresh;
                let tier = animal.tier.clone();
                let weight = animal.weight.clone();
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(text(name).text_sm())
                    .child(when(
                        move || fresh,
                        move || badge(move || text_key("ranch.fresh"), BadgeTone::Success),
                    ))
                    .child(
                        text(move || {
                            let status = text_key(if collected {
                                "ranch.bookOwned"
                            } else {
                                "ranch.bookMissing"
                            });
                            if giant_unlocked && !weight.is_empty() {
                                let record = if tier.is_empty() {
                                    format!("{weight} kg")
                                } else {
                                    format!("{tier} · {weight} kg")
                                };
                                SharedString::from(format!("{status} · {record}"))
                            } else if harvest > 0 {
                                SharedString::from(format!(
                                    "{status} · {}",
                                    text_key("ranch.bookHarvested")
                                        .replace("{count}", &harvest.to_string())
                                ))
                            } else {
                                status
                            }
                        })
                        .text_sm()
                        .text_color(move || nsh_base::theme().muted_foreground),
                    )
            },
        ))
}

fn book_summary(
    catalog: Resource<BookBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            text(move || {
                SharedString::from(
                    catalog
                        .get()
                        .map(|book| {
                            format!("{} {}/{}", book.level, book.progress, book.next_progress)
                        })
                        .unwrap_or_default(),
                )
            })
            .text_sm(),
        )
        .child(when(
            move || catalog.get().is_some_and(|book| book.can_claim),
            move || {
                button(move || text_key("ranch.claimBook"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || claim_book(catalog, pending, failure, account_id, board))
            },
        ))
        .child(
            text(move || {
                SharedString::from(
                    catalog.get().map(|book| book.score.to_string()).unwrap_or_default(),
                )
            })
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(when(
            move || catalog.get().is_some_and(|book| book.giant_claim),
            move || {
                button(move || text_key("ranch.claimGiant"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || claim_giant(catalog, pending, failure, account_id, board))
            },
        ))
}

fn task_shelf(
    account_id: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let open = signal(false);
    let catalog = resource(
        move || open.get().then(|| account_id.get()).flatten().map(|id| (id, 0_i64)),
        |(id, _), _| async move { load_tasks(&id).await },
    );
    view()
        .gap_2()
        .child(
            button(move || text_key("ranch.tasks"), ButtonVariant::Quiet)
                .selected(move || open.get())
                .on_press(move || open.set(!open.get_untracked())),
        )
        .child(when(
            move || open.get(),
            move || task_body(catalog, pending, failure, account_id, board),
        ))
}

fn task_body(
    catalog: Resource<TaskBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || catalog.loading() && catalog.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || catalog.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        catalog.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || catalog.refetch()),
                    )
            },
        ))
        .child(when(
            move || catalog.get().is_some_and(|tasks| tasks.tasks.is_empty()),
            move || notice(move || text_key("ranch.taskEmpty")),
        ))
        .child(each(
            move || catalog.get().map(|tasks| tasks.tasks).unwrap_or_default(),
            |task: &TaskLine| task.id,
            move |task, _| {
                let task = task.get_untracked();
                let task_id = task.id;
                let name = SharedString::from(task.name);
                let progress = SharedString::from(format!("{}/{}", task.progress, task.total));
                let can_claim = task.can_claim;
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(text(name).text_sm())
                    .child(
                        text(progress)
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground),
                    )
                    .child(when(
                        move || can_claim,
                        move || {
                            button(move || text_key("ranch.claim"), ButtonVariant::Secondary)
                                .disabled(move || pending.get())
                                .on_press(move || {
                                    claim_task(
                                        task_id, catalog, pending, failure, account_id, board,
                                    )
                                })
                        },
                    ))
            },
        ))
}

fn market_shelf(
    account_id: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let open = signal(false);
    let market_counts = signal(Vec::<(i64, i64)>::new());
    let catalog = resource(
        move || open.get().then(|| account_id.get()).flatten().map(|id| (id, 0_i64)),
        |(id, _), _| async move { load_market(&id).await },
    );
    view()
        .gap_2()
        .child(
            button(move || text_key("ranch.market"), ButtonVariant::Quiet)
                .selected(move || open.get())
                .on_press(move || open.set(!open.get_untracked())),
        )
        .child(when(
            move || open.get(),
            move || market_body(catalog, market_counts, pending, failure, account_id, board),
        ))
}

fn market_body(
    catalog: Resource<MarketBoard, String>,
    market_counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || catalog.loading() && catalog.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || catalog.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        catalog.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || catalog.refetch()),
                    )
            },
        ))
        .child(when(
            move || catalog.get().is_some_and(|market| market.goods.is_empty()),
            move || notice(move || text_key("ranch.shopEmpty")),
        ))
        .child(each(
            move || catalog.get().map(|market| market.goods).unwrap_or_default(),
            |good: &MarketLine| good.id,
            move |good, _| {
                let good = good.get_untracked();
                let goods_id = good.id;
                let name = SharedString::from(good.name);
                let price = SharedString::from(format!("{} {}", good.price_name, good.price));
                let unit_price = good.price;
                let balance = good.balance;
                let max_count = good.max_count;
                let remaining = good.remaining;
                let available = good.available;
                let limit = shop_buy_limit(max_count, remaining, unit_price, balance);
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(text(name).text_sm())
                    .child(
                        text(price)
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground),
                    )
                    .child(when(
                        move || available && limit > 1,
                        move || shop_count_stepper(goods_id, limit, market_counts, pending),
                    ))
                    .child(when(
                        move || available,
                        move || {
                            button(move || text_key("ranch.exchange"), ButtonVariant::Secondary)
                                .disabled(move || pending.get())
                                .on_press(move || {
                                    let count = chosen_shop_count(market_counts, goods_id, limit);
                                    exchange(
                                        goods_id, count, catalog, pending, failure, account_id,
                                        board,
                                    )
                                })
                        },
                    ))
            },
        ))
}

#[derive(Clone, PartialEq, Eq)]
struct RanchFriend {
    gid: i64,
    name: String,
}

#[derive(Clone, PartialEq, Eq)]
struct StrokeAnimal {
    ranch_id: i64,
    id: i32,
    name: String,
}

#[derive(Clone, PartialEq, Eq)]
struct FriendPenLine {
    id: i64,
    name: String,
    can_mosquito: bool,
    animals: Vec<StrokeAnimal>,
}

#[derive(Clone, PartialEq, Eq)]
struct FriendRanchView {
    name: String,
    can_steal: bool,
    cleanable: i64,
    pens: Vec<FriendPenLine>,
}

fn daily_share_button(
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> If {
    let share = resource(
        move || account_id.get().map(|id| (id, 0_i64)),
        |(id, _), _| async move { load_daily_share(&id).await },
    );
    when(
        move || share.get() == Some(true),
        move || {
            button(move || text_key("ranch.dailyShare"), ButtonVariant::Secondary)
                .disabled(move || pending.get())
                .on_press(move || claim_share(share, pending, failure, account_id, board))
        },
    )
}

fn claim_share(
    share: Resource<bool, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmShare.title",
        "ranch.confirmShare.body",
        "dailyShare",
        "ranch.dailyShare",
        move |accepted| {
            if !accepted || pending.get_untracked() {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(async move {
                    qq_farm_app::ranch::claim_daily_share(&context, &id).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                share.refetch();
            });
        },
    );
}

fn friend_shelf(
    account_id: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let open = signal(false);
    let host = signal(0_i64);
    let friends = resource(
        move || open.get().then(|| account_id.get()).flatten().map(|id| (id, 0_i64)),
        |(id, _), _| async move { load_ranch_friends(&id).await },
    );
    let ranch = resource(
        move || {
            let gid = host.get();
            if gid == 0 {
                return None;
            }
            account_id.get().map(|id| (id, gid))
        },
        |(id, gid), _| async move { load_friend_ranch(&id, gid).await },
    );
    view()
        .gap_2()
        .child(
            button(move || text_key("ranch.friends"), ButtonVariant::Quiet)
                .selected(move || open.get())
                .on_press(move || open.set(!open.get_untracked())),
        )
        .child(when(
            move || open.get(),
            move || friend_body(friends, ranch, host, pending, failure, account_id, board),
        ))
}

fn friend_body(
    friends: Resource<Vec<RanchFriend>, String>,
    ranch: Resource<FriendRanchView, String>,
    host: Signal<i64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || friends.loading() && friends.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || friends.error().is_some() || ranch.error().is_some(),
            move || {
                let message = friends.error().or_else(|| ranch.error()).unwrap_or_default();
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || SharedString::from(message.clone())))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || {
                                friends.refetch();
                                ranch.refetch();
                            }),
                    )
            },
        ))
        .child(each(
            move || friends.get().unwrap_or_default(),
            |friend: &RanchFriend| friend.gid,
            move |friend, _| {
                let friend = friend.get_untracked();
                let gid = friend.gid;
                let name = SharedString::from(friend.name);
                button(move || name.clone(), ButtonVariant::Quiet)
                    .selected(move || host.get() == gid)
                    .on_press(move || {
                        if host.get_untracked() == gid {
                            host.set(0);
                        } else {
                            host.set(gid);
                        }
                    })
            },
        ))
        .child(when(
            move || host.get() != 0 && ranch.get().is_some(),
            move || friend_actions(ranch, host, pending, failure, account_id, board),
        ))
}

fn friend_actions(
    ranch: Resource<FriendRanchView, String>,
    host: Signal<i64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(
            text(move || {
                SharedString::from(ranch.get().map(|ranch| ranch.name.clone()).unwrap_or_default())
            })
            .text_sm(),
        )
        .child(when(
            move || ranch.get().is_some_and(|ranch| ranch.can_steal),
            move || {
                button(move || text_key("ranch.steal"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        confirm_friend(
                            "ranch.steal",
                            ranch,
                            host,
                            pending,
                            failure,
                            account_id,
                            board,
                            |context, id, gid| async move {
                                qq_farm_app::ranch::steal_friend(&context, &id, gid).await
                            },
                        )
                    })
            },
        ))
        .child(when(
            move || ranch.get().is_some_and(|ranch| ranch.cleanable > 0),
            move || {
                button(move || text_key("ranch.cleanFriend"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        confirm_friend(
                            "ranch.cleanFriend",
                            ranch,
                            host,
                            pending,
                            failure,
                            account_id,
                            board,
                            |context, id, gid| async move {
                                qq_farm_app::ranch::clean_friend(&context, &id, gid).await
                            },
                        )
                    })
            },
        ))
        .child(each(
            move || ranch.get().map(|ranch| ranch.pens).unwrap_or_default(),
            |pen: &FriendPenLine| pen.id,
            move |pen, _| {
                let pen = pen.get_untracked();
                let ranch_id = pen.id;
                let can_mosquito = pen.can_mosquito;
                let animals = pen.animals;
                view()
                    .gap_2()
                    .child(text(SharedString::from(pen.name)).text_sm())
                    .child(when(
                        move || can_mosquito,
                        move || {
                            button(move || text_key("ranch.mosquito"), ButtonVariant::Secondary)
                                .disabled(move || pending.get())
                                .on_press(move || {
                                    confirm_friend(
                                        "ranch.mosquito",
                                        ranch,
                                        host,
                                        pending,
                                        failure,
                                        account_id,
                                        board,
                                        move |context, id, gid| async move {
                                            qq_farm_app::ranch::mosquito_friend(
                                                &context, &id, gid, ranch_id,
                                            )
                                            .await
                                        },
                                    )
                                })
                        },
                    ))
                    .child(each(
                        move || animals.clone(),
                        |animal: &StrokeAnimal| animal.id,
                        move |animal, _| {
                            let animal = animal.get_untracked();
                            let animal_id = animal.id;
                            let name = SharedString::from(animal.name);
                            button(move || name.clone(), ButtonVariant::Secondary)
                                .disabled(move || pending.get())
                                .on_press(move || {
                                    confirm_friend(
                                        "ranch.stroke",
                                        ranch,
                                        host,
                                        pending,
                                        failure,
                                        account_id,
                                        board,
                                        move |context, id, gid| async move {
                                            qq_farm_app::ranch::stroke_friend(
                                                &context, &id, gid, ranch_id, animal_id,
                                            )
                                            .await
                                        },
                                    )
                                })
                        },
                    ))
            },
        ))
}

fn confirm_friend<F, Fut>(
    label: &'static str,
    ranch: Resource<FriendRanchView, String>,
    host: Signal<i64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
    claim: F,
) where
    F: FnOnce(std::sync::Arc<qq_farm_app::session::AppContext>, String, i64) -> Fut + 'static,
    Fut: std::future::Future<Output = qq_farm_app::AppResult<()>> + Send + 'static,
{
    if pending.get_untracked() {
        return;
    }
    let gid = host.get_untracked();
    confirm::confirm(
        "ranch.confirmFriend.title",
        "ranch.confirmFriend.body",
        "friendRanch",
        label,
        move |accepted| {
            if !accepted || pending.get_untracked() || gid == 0 {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(claim(context, id, gid)).await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                ranch.refetch();
            });
        },
    );
}

fn can_mall_shelf(
    account_id: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let slot = signal(0_i64);
    let can_counts = signal(Vec::<(i64, i64)>::new());
    let catalog = resource(
        move || {
            let slot = slot.get();
            if slot != 6 && slot != 7 {
                return None;
            }
            account_id.get().map(|id| (id, slot))
        },
        |(id, slot), _| async move { load_can_shop(&id, slot).await },
    );
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(shelf_button(slot, 6, "ranch.canRegular"))
                .child(shelf_button(slot, 7, "ranch.canLimited")),
        )
        .child(when(
            move || slot.get() == 6 || slot.get() == 7,
            move || can_mall_body(catalog, slot, can_counts, pending, failure, account_id, board),
        ))
}

fn can_mall_body(
    catalog: Resource<ShopBoard, String>,
    slot: Signal<i64>,
    can_counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || catalog.loading() && catalog.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || catalog.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        catalog.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || catalog.refetch()),
                    )
            },
        ))
        .child(when(
            move || catalog.get().is_some_and(|shop| shop.goods.is_empty()),
            move || notice(move || text_key("ranch.shopEmpty")),
        ))
        .child(each(
            move || catalog.get().map(|shop| shop.goods).unwrap_or_default(),
            |good: &GoodLine| good.id,
            move |good, _| {
                let good = good.get_untracked();
                let name = SharedString::from(good.name);
                let price = good.price;
                let goods_id = good.id;
                let available = good.available;
                let limit =
                    shop_buy_limit(good.max_count, good.remaining, good.price, good.balance);
                let label = if price == 0 { "ranch.claim" } else { "ranch.buy" };
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(text(name).text_sm())
                    .child(text(move || SharedString::from(price.to_string())).text_sm())
                    .child(when(
                        move || available && limit > 1,
                        move || shop_count_stepper(goods_id, limit, can_counts, pending),
                    ))
                    .child(when(
                        move || available,
                        move || {
                            button(move || text_key(label), ButtonVariant::Secondary)
                                .disabled(move || pending.get())
                                .on_press(move || {
                                    let count = chosen_shop_count(can_counts, goods_id, limit);
                                    buy_can_goods(
                                        slot, goods_id, count, label, catalog, pending, failure,
                                        account_id, board,
                                    )
                                })
                        },
                    ))
            },
        ))
}

fn shop_shelf(
    account_id: Signal<Option<String>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let shelf = signal(0_i64);
    let place = signal(false);
    let shop_counts = signal(Vec::<(i64, i64)>::new());
    let catalog = resource(
        move || {
            let shop_id = shelf.get();
            if shop_id != 5 && shop_id != 6 {
                return None;
            }
            account_id.get().map(|id| (id, shop_id))
        },
        |(id, shop_id), _| async move { load_shop(&id, shop_id).await },
    );
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(shelf_button(shelf, 5, "ranch.cubs"))
                .child(shelf_button(shelf, 6, "ranch.feedShop")),
        )
        .child(when(
            move || shelf.get() == 5 || shelf.get() == 6,
            move || {
                shop_body(catalog, shelf, place, shop_counts, pending, failure, account_id, board)
            },
        ))
}

fn shelf_button(shelf: Signal<i64>, shop_id: i64, label: &'static str) -> nsh_base::ButtonElement {
    button(move || text_key(label), ButtonVariant::Quiet)
        .selected(move || shelf.get() == shop_id)
        .on_press(move || {
            if shelf.get_untracked() == shop_id {
                shelf.set(0);
            } else {
                shelf.set(shop_id);
            }
        })
}

fn shop_body(
    catalog: Resource<ShopBoard, String>,
    shelf: Signal<i64>,
    place: Signal<bool>,
    shop_counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || shelf.get() == 5,
            move || {
                button(move || text_key("ranch.autoPlace"), ButtonVariant::Quiet)
                    .selected(move || place.get())
                    .on_press(move || place.set(!place.get_untracked()))
            },
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
                    .child(crate::ui::failure(move || {
                        catalog.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || catalog.refetch()),
                    )
            },
        ))
        .child(when(
            move || catalog.get().is_some_and(|shop| shop.goods.is_empty()),
            move || notice(move || text_key("ranch.shopEmpty")),
        ))
        .child(each(
            move || catalog.get().map(|shop| shop.goods).unwrap_or_default(),
            |good: &GoodLine| good.id,
            move |good, _| {
                good_line(
                    good.get_untracked(),
                    shelf,
                    place,
                    shop_counts,
                    catalog,
                    pending,
                    failure,
                    account_id,
                    board,
                )
            },
        ))
}

fn good_line(
    good: GoodLine,
    shelf: Signal<i64>,
    place: Signal<bool>,
    shop_counts: Signal<Vec<(i64, i64)>>,
    catalog: Resource<ShopBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let name = SharedString::from(good.name);
    let price = good.price;
    let goods_id = good.id;
    let available = good.available;
    let limit = shop_buy_limit(good.max_count, good.remaining, good.price, good.balance);
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(text(name).text_sm())
        .child(text(move || SharedString::from(price.to_string())).text_sm())
        .child(when(
            move || available && limit > 1,
            move || shop_count_stepper(goods_id, limit, shop_counts, pending),
        ))
        .child(when(
            move || available,
            move || {
                button(move || text_key("ranch.buy"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        let count = chosen_shop_count(shop_counts, goods_id, limit);
                        buy(
                            shelf, place, goods_id, count, catalog, pending, failure, account_id,
                            board,
                        )
                    })
            },
        ))
}

fn shop_buy_limit(max_count: i64, remaining: Option<i64>, price: i64, balance: i64) -> i64 {
    let cap = if max_count > 0 { max_count } else { 1 };
    let costs = if price > 0 {
        vec![PurchaseCost { count: price, balance: Some(balance) }]
    } else {
        Vec::new()
    };
    purchase_max_count(remaining, &costs, cap)
}

fn chosen_shop_count(counts: Signal<Vec<(i64, i64)>>, id: i64, limit: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(goods_id, _)| *goods_id == id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    clamp_grass_count(stored, limit)
}

fn set_shop_count(counts: Signal<Vec<(i64, i64)>>, id: i64, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(goods_id, _)| *goods_id == id) {
        row.1 = count;
    } else {
        rows.push((id, count));
    }
    counts.set(rows);
}

fn shop_count_stepper(
    id: i64,
    limit: i64,
    counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<bool>,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || pending.get() || chosen_shop_count(counts, id, limit) <= 1)
                .on_press(move || {
                    let next = chosen_shop_count(counts, id, limit).saturating_sub(1).max(1);
                    set_shop_count(counts, id, next);
                }),
        )
        .child(
            text(move || SharedString::from(chosen_shop_count(counts, id, limit).to_string()))
                .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || pending.get() || chosen_shop_count(counts, id, limit) >= limit)
                .on_press(move || {
                    let next = chosen_shop_count(counts, id, limit).saturating_add(1).min(limit);
                    set_shop_count(counts, id, next);
                }),
        )
}

fn facility_line(
    facility: FacilityLine,
    cubs: Vec<CubLine>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
    grass_counts: Signal<Vec<(String, i64)>>,
    place_counts: Signal<Vec<(String, i64)>>,
) -> View {
    let title = SharedString::from(format!("{} · {}", facility.id, facility.level));
    let animals = facility.animals.clone();
    let facility_id = facility.id.parse::<i64>().ok();
    let is_machine = facility.module_type == 8;
    let kind = facility.kind;
    let open_room = facility.open_room;
    let place_facility = facility.id.clone();
    let can_start = facility.upgrade == "start";
    let can_finish = facility.upgrade == "finish";
    view()
        .gap_2()
        .py_2()
        .child(text(title).text_sm().font_weight(FontWeight::MEDIUM))
        .child(when(
            {
                let facility_id = facility.id.clone();
                move || is_machine && facility_fill_limit(board, &facility_id) > 0
            },
            {
                let facility_id_key = facility.id.clone();
                move || {
                    grass_fill_row(
                        facility_id_key.clone(),
                        facility_id,
                        grass_counts,
                        pending,
                        failure,
                        account_id,
                        board,
                    )
                }
            },
        ))
        .child(when(
            move || can_start,
            move || {
                button(move || text_key("ranch.upgrade"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        start_upgrade(facility_id, pending, failure, account_id, board)
                    })
            },
        ))
        .child(when(
            move || can_finish,
            move || {
                button(move || text_key("ranch.finishUpgrade"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        finish_upgrade(facility_id, pending, failure, account_id, board)
                    })
            },
        ))
        .child(each(
            move || {
                if open_room <= 0 {
                    return Vec::new();
                }
                cubs.iter().filter(|cub| cub.kind == kind).cloned().collect()
            },
            |cub: &CubLine| cub.id,
            move |cub, _| {
                let cub = cub.get_untracked();
                place_cub_row(
                    place_facility.clone(),
                    facility_id,
                    cub,
                    place_counts,
                    pending,
                    failure,
                    account_id,
                    board,
                )
            },
        ))
        .child(each(
            move || animals.clone(),
            |animal: &AnimalLine| format!("{}:{}", animal.ranch_id, animal.id),
            move |animal, _| {
                animal_line(animal.get_untracked(), pending, failure, account_id, board)
            },
        ))
}

fn animal_line(
    animal: AnimalLine,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let name = SharedString::from(animal.name.clone());
    let stage = SharedString::from(animal.stage.clone());
    let harvest_ranch = animal.ranch_id.clone();
    let produce_ranch = animal.ranch_id.clone();
    let normal_ranch = animal.ranch_id.clone();
    let release_ranch = animal.ranch_id.clone();
    let organic_ranch = animal.ranch_id;
    let animal_id = animal.id;
    let can_harvest = animal.can_harvest;
    let can_produce = animal.can_produce;
    let can_feed = animal.can_feed;
    let can_release = animal.can_release;
    let normal_feed = animal.can_feed && !animal.limited_can;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(text(name).text_sm())
        .child(text(stage).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(when(
            move || can_harvest,
            move || {
                let ranch_id = harvest_ranch.clone();
                button(move || text_key("ranch.harvest"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        harvest(
                            ranch_id.parse().ok(),
                            Some(animal_id),
                            pending,
                            failure,
                            account_id,
                            board,
                        )
                    })
            },
        ))
        .child(when(
            move || can_produce,
            move || {
                let ranch_id = produce_ranch.clone();
                button(move || text_key("ranch.produce"), ButtonVariant::Secondary)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        produce(
                            ranch_id.parse().ok(),
                            Some(animal_id),
                            pending,
                            failure,
                            account_id,
                            board,
                        )
                    })
            },
        ))
        .child(when(
            move || normal_feed,
            move || {
                let ranch_id = normal_ranch.clone();
                button(move || text_key("ranch.feedNormal"), ButtonVariant::Quiet)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        feed(
                            ranch_id.parse().ok(),
                            Some(animal_id),
                            2,
                            pending,
                            failure,
                            account_id,
                            board,
                        )
                    })
            },
        ))
        .child(when(
            move || can_feed,
            move || {
                let ranch_id = organic_ranch.clone();
                button(move || text_key("ranch.feedOrganic"), ButtonVariant::Quiet)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        feed(
                            ranch_id.parse().ok(),
                            Some(animal_id),
                            1,
                            pending,
                            failure,
                            account_id,
                            board,
                        )
                    })
            },
        ))
        .child(when(
            move || can_release,
            move || {
                let ranch_id = release_ranch.clone();
                button(move || text_key("ranch.release"), ButtonVariant::Quiet)
                    .disabled(move || pending.get())
                    .on_press(move || {
                        release_animal(
                            ranch_id.parse().ok(),
                            animal_id,
                            pending,
                            failure,
                            account_id,
                            board,
                        )
                    })
            },
        ))
}

fn start_upgrade(
    facility_id: Option<i64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    let Some(facility_id) = facility_id else {
        return;
    };
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmUpgrade.title",
        "ranch.confirmUpgrade.body",
        "upgrade",
        "ranch.upgrade",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, move |context, id| async move {
                qq_farm_app::ranch::start_upgrade(&context, &id, facility_id).await
            });
        },
    );
}

fn finish_upgrade(
    facility_id: Option<i64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    let Some(facility_id) = facility_id else {
        return;
    };
    run_action(pending, failure, account_id, board, move |context, id| async move {
        qq_farm_app::ranch::finish_upgrade(&context, &id, facility_id).await
    });
}

fn release_animal(
    ranch_id: Option<i64>,
    animal_inst_id: i32,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    let Some(ranch_id) = ranch_id else {
        return;
    };
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmRelease.title",
        "ranch.confirmRelease.body",
        "release",
        "ranch.release",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, move |context, id| async move {
                qq_farm_app::ranch::release(&context, &id, ranch_id, animal_inst_id).await
            });
        },
    );
}

fn clean(
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    run_action(pending, failure, account_id, board, |context, id| async move {
        qq_farm_app::ranch::clean(&context, &id).await
    });
}

fn grass_fill_limit(stock: i64, room: i64) -> i64 {
    stock.max(0).min(room.max(0)).min(i64::from(i32::MAX))
}

fn clamp_grass_count(stored: i64, limit: i64) -> i64 {
    if limit < 1 {
        return 0;
    }
    stored.max(1).min(limit)
}

fn facility_fill_limit(board: Resource<RanchBoard, String>, facility_id: &str) -> i64 {
    board
        .get()
        .map(|board| {
            let room = board
                .facilities
                .iter()
                .find(|facility| facility.id == facility_id)
                .map(|facility| facility.grass_room)
                .unwrap_or(0);
            grass_fill_limit(board.grass, room)
        })
        .unwrap_or(0)
}

fn chosen_grass_count(counts: Signal<Vec<(String, i64)>>, facility_id: &str, limit: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(id, _)| id == facility_id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    clamp_grass_count(stored, limit)
}

fn set_grass_count(counts: Signal<Vec<(String, i64)>>, facility_id: &str, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(id, _)| id == facility_id) {
        row.1 = count;
    } else {
        rows.push((facility_id.to_owned(), count));
    }
    counts.set(rows);
}

fn grass_fill_row(
    facility_key: String,
    facility_id: Option<i64>,
    grass_counts: Signal<Vec<(String, i64)>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let stepper_key = facility_key.clone();
    let button_key = facility_key;
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(grass_stepper(stepper_key, grass_counts, pending, board))
        .child(
            button(move || text_key("ranch.addGrass"), ButtonVariant::Secondary)
                .disabled(move || pending.get())
                .on_press(move || {
                    let limit = facility_fill_limit(board, &button_key);
                    let count = chosen_grass_count(grass_counts, &button_key, limit);
                    add_grass(facility_id, count, pending, failure, account_id, board);
                }),
        )
}

fn grass_stepper(
    facility_id: String,
    counts: Signal<Vec<(String, i64)>>,
    pending: Signal<bool>,
    board: Resource<RanchBoard, String>,
) -> impl IntoElement {
    let less_disabled = facility_id.clone();
    let less_press = facility_id.clone();
    let more_disabled = facility_id.clone();
    let more_press = facility_id.clone();
    let label_id = facility_id;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || {
                    let limit = facility_fill_limit(board, &less_disabled);
                    pending.get() || chosen_grass_count(counts, &less_disabled, limit) <= 1
                })
                .on_press(move || {
                    let limit = facility_fill_limit(board, &less_press);
                    let next =
                        chosen_grass_count(counts, &less_press, limit).saturating_sub(1).max(1);
                    set_grass_count(counts, &less_press, next);
                }),
        )
        .child(
            text(move || {
                let limit = facility_fill_limit(board, &label_id);
                SharedString::from(chosen_grass_count(counts, &label_id, limit).to_string())
            })
            .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    let limit = facility_fill_limit(board, &more_disabled);
                    pending.get() || chosen_grass_count(counts, &more_disabled, limit) >= limit
                })
                .on_press(move || {
                    let limit = facility_fill_limit(board, &more_press);
                    let next =
                        chosen_grass_count(counts, &more_press, limit).saturating_add(1).min(limit);
                    set_grass_count(counts, &more_press, next);
                }),
        )
}

fn add_grass(
    facility_id: Option<i64>,
    count: i64,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    let Some(facility_id) = facility_id else {
        return;
    };
    let Some(count) = i32::try_from(count).ok().filter(|count| *count > 0) else {
        return;
    };
    run_action(pending, failure, account_id, board, move |context, id| async move {
        qq_farm_app::ranch::add_grass(&context, &id, facility_id, count).await
    });
}

fn use_can(
    uid: i64,
    count: i64,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() || count <= 0 {
        return;
    }
    confirm::confirm(
        "ranch.confirmCan.title",
        "ranch.confirmCan.body",
        "use-can",
        "ranch.useCan",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, move |context, id| async move {
                qq_farm_app::ranch::use_can(&context, &id, uid, count).await
            });
        },
    );
}

fn can_use_limit(owned: i64) -> i64 {
    owned.max(0).min(i64::from(i32::MAX))
}

fn claim_book(
    catalog: Resource<BookBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    confirm_book(
        "claimBook",
        "ranch.claimBook",
        catalog,
        pending,
        failure,
        account_id,
        board,
        |context, id| async move { qq_farm_app::ranch::claim_book(&context, &id).await },
    );
}

fn claim_giant(
    catalog: Resource<BookBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    confirm_book(
        "claimGiant",
        "ranch.claimGiant",
        catalog,
        pending,
        failure,
        account_id,
        board,
        |context, id| async move { qq_farm_app::ranch::claim_giant_book(&context, &id).await },
    );
}

fn confirm_book<F, Fut>(
    action_id: &'static str,
    action_label: &'static str,
    catalog: Resource<BookBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
    claim: F,
) where
    F: FnOnce(std::sync::Arc<qq_farm_app::session::AppContext>, String) -> Fut + 'static,
    Fut: std::future::Future<Output = qq_farm_app::AppResult<()>> + Send + 'static,
{
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmBook.title",
        "ranch.confirmBook.body",
        action_id,
        action_label,
        move |accepted| {
            if !accepted || pending.get_untracked() {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(claim(context, id)).await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                catalog.refetch();
            });
        },
    );
}

fn claim_task(
    task_id: i64,
    catalog: Resource<TaskBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmClaim.title",
        "ranch.confirmClaim.body",
        "claimTask",
        "ranch.claim",
        move |accepted| {
            if !accepted {
                return;
            }
            if pending.get_untracked() {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(async move {
                    qq_farm_app::ranch::claim_task(&context, &id, task_id).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                catalog.refetch();
            });
        },
    );
}

fn exchange(
    goods_id: i64,
    count: i64,
    catalog: Resource<MarketBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() || count <= 0 {
        return;
    }
    confirm::confirm(
        "ranch.confirmExchange.title",
        "ranch.confirmExchange.body",
        "exchange",
        "ranch.exchange",
        move |accepted| {
            if !accepted {
                return;
            }
            if pending.get_untracked() {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(async move {
                    qq_farm_app::ranch::exchange(&context, &id, goods_id, count).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                catalog.refetch();
            });
        },
    );
}

fn sell_all(
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() {
        return;
    }
    confirm::confirm(
        "ranch.confirmSell.title",
        "ranch.confirmSell.body",
        "sell",
        "ranch.sell",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, |context, id| async move {
                qq_farm_app::ranch::sell_all(&context, &id).await
            });
        },
    );
}

fn sell_stack(
    uid: i64,
    count: i64,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() || count <= 0 {
        return;
    }
    confirm::confirm(
        "ranch.confirmSell.title",
        "ranch.confirmSell.body",
        "sell",
        "ranch.sell",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, move |context, id| async move {
                qq_farm_app::ranch::sell_stack(&context, &id, uid, count).await
            });
        },
    );
}

fn place_limit(stock: i64, room: i64) -> i64 {
    stock.max(0).min(room.max(0)).min(i64::from(i32::MAX))
}

fn place_count_key(facility_id: &str, item_id: i64) -> String {
    format!("{facility_id}:{item_id}")
}

fn cub_place_limit(board: Resource<RanchBoard, String>, facility_id: &str, item_id: i64) -> i64 {
    board
        .get()
        .map(|board| {
            let room = board
                .facilities
                .iter()
                .find(|facility| facility.id == facility_id)
                .map(|facility| facility.open_room)
                .unwrap_or(0);
            let stock =
                board.cubs.iter().find(|cub| cub.id == item_id).map(|cub| cub.count).unwrap_or(0);
            place_limit(stock, room)
        })
        .unwrap_or(0)
}

fn chosen_place_count(counts: Signal<Vec<(String, i64)>>, key: &str, limit: i64) -> i64 {
    let stored =
        counts.get().into_iter().find(|(id, _)| id == key).map(|(_, count)| count).unwrap_or(1);
    clamp_grass_count(stored, limit)
}

fn set_place_count(counts: Signal<Vec<(String, i64)>>, key: &str, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(id, _)| id == key) {
        row.1 = count;
    } else {
        rows.push((key.to_owned(), count));
    }
    counts.set(rows);
}

fn place_cub_row(
    facility_key: String,
    ranch_id: Option<i64>,
    cub: CubLine,
    place_counts: Signal<Vec<(String, i64)>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) -> View {
    let item_id = cub.id;
    let name = SharedString::from(cub.name);
    let stepper_key = place_count_key(&facility_key, item_id);
    let button_key = stepper_key.clone();
    let button_facility = facility_key.clone();
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(place_stepper(stepper_key, facility_key, item_id, place_counts, pending, board))
        .child(
            button(
                move || SharedString::from(format!("{} {name}", text_key("ranch.place"))),
                ButtonVariant::Quiet,
            )
            .disabled(move || pending.get())
            .on_press(move || {
                let limit = cub_place_limit(board, &button_facility, item_id);
                let count = chosen_place_count(place_counts, &button_key, limit);
                place_cub(ranch_id, item_id, count, pending, failure, account_id, board);
            }),
        )
}

fn place_stepper(
    key: String,
    facility_id: String,
    item_id: i64,
    counts: Signal<Vec<(String, i64)>>,
    pending: Signal<bool>,
    board: Resource<RanchBoard, String>,
) -> impl IntoElement {
    let less_disabled = key.clone();
    let less_press = key.clone();
    let more_disabled = key.clone();
    let more_press = key.clone();
    let label_key = key;
    let less_disabled_facility = facility_id.clone();
    let less_press_facility = facility_id.clone();
    let more_disabled_facility = facility_id.clone();
    let more_press_facility = facility_id.clone();
    let label_facility = facility_id;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || {
                    let limit = cub_place_limit(board, &less_disabled_facility, item_id);
                    pending.get() || chosen_place_count(counts, &less_disabled, limit) <= 1
                })
                .on_press(move || {
                    let limit = cub_place_limit(board, &less_press_facility, item_id);
                    let next =
                        chosen_place_count(counts, &less_press, limit).saturating_sub(1).max(1);
                    set_place_count(counts, &less_press, next);
                }),
        )
        .child(
            text(move || {
                let limit = cub_place_limit(board, &label_facility, item_id);
                SharedString::from(chosen_place_count(counts, &label_key, limit).to_string())
            })
            .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    let limit = cub_place_limit(board, &more_disabled_facility, item_id);
                    pending.get() || chosen_place_count(counts, &more_disabled, limit) >= limit
                })
                .on_press(move || {
                    let limit = cub_place_limit(board, &more_press_facility, item_id);
                    let next =
                        chosen_place_count(counts, &more_press, limit).saturating_add(1).min(limit);
                    set_place_count(counts, &more_press, next);
                }),
        )
}

fn place_cub(
    ranch_id: Option<i64>,
    item_id: i64,
    count: i64,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    let Some(ranch_id) = ranch_id else {
        return;
    };
    if pending.get_untracked() || count <= 0 {
        return;
    }
    confirm::confirm(
        "ranch.confirmPlace.title",
        "ranch.confirmPlace.body",
        "place",
        "ranch.place",
        move |accepted| {
            if !accepted {
                return;
            }
            run_action(pending, failure, account_id, board, move |context, id| async move {
                qq_farm_app::ranch::place(&context, &id, ranch_id, item_id, count).await
            });
        },
    );
}

fn produce(
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    run_action(pending, failure, account_id, board, move |context, id| async move {
        qq_farm_app::ranch::produce(&context, &id, ranch_id, animal_id).await
    });
}

fn feed(
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    bucket_type: i32,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    run_action(pending, failure, account_id, board, move |context, id| async move {
        qq_farm_app::ranch::feed(&context, &id, ranch_id, animal_id, bucket_type).await
    });
}

fn harvest(
    ranch_id: Option<i64>,
    animal_id: Option<i32>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    run_action(pending, failure, account_id, board, move |context, id| async move {
        qq_farm_app::ranch::harvest(&context, &id, ranch_id, animal_id).await
    });
}

fn buy_can_goods(
    slot: Signal<i64>,
    goods_id: i64,
    count: i64,
    label: &'static str,
    catalog: Resource<ShopBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() || count <= 0 {
        return;
    }
    let slot_id = slot.get_untracked();
    confirm::confirm(
        "ranch.confirmCan.title",
        "ranch.confirmCan.body",
        "buyCan",
        label,
        move |accepted| {
            if !accepted || pending.get_untracked() {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            let Ok(slot) = i32::try_from(slot_id) else {
                failure.set(Some("未知牧场罐头商城".to_owned()));
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(async move {
                    qq_farm_app::ranch::buy_can(&context, &id, slot, goods_id, count).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                catalog.refetch();
            });
        },
    );
}

fn buy(
    shelf: Signal<i64>,
    place: Signal<bool>,
    goods_id: i64,
    count: i64,
    catalog: Resource<ShopBoard, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
) {
    if pending.get_untracked() || count <= 0 {
        return;
    }
    let shop_id = shelf.get_untracked();
    let auto_place = shop_id == 5 && place.get_untracked();
    confirm::confirm(
        "ranch.confirmBuy.title",
        "ranch.confirmBuy.body",
        "buy",
        "ranch.buy",
        move |accepted| {
            if !accepted {
                return;
            }
            if pending.get_untracked() {
                return;
            }
            let Some(id) = account_id.get_untracked() else {
                return;
            };
            pending.set(true);
            failure.set(None);
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::call(async move {
                    if auto_place {
                        qq_farm_app::ranch::buy_and_place(&context, &id, shop_id, goods_id, count)
                            .await
                    } else {
                        qq_farm_app::ranch::buy_goods(&context, &id, shop_id, goods_id, count).await
                    }
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(false);
                board.refetch();
                catalog.refetch();
            });
        },
    );
}

fn run_action<F, Fut>(
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    account_id: Signal<Option<String>>,
    board: Resource<RanchBoard, String>,
    action: F,
) where
    F: FnOnce(std::sync::Arc<qq_farm_app::AppContext>, String) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = qq_farm_app::AppResult<()>> + Send + 'static,
{
    if pending.get_untracked() {
        return;
    }
    let Some(id) = account_id.get_untracked() else {
        return;
    };
    pending.set(true);
    failure.set(None);
    let context = session::session().context.clone();
    spawn(async move {
        let result = session::call(action(context, id)).await;
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(false);
        board.refetch();
    });
}

fn ready_id(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> Option<String> {
    (account_gate(account_id, rows) == AccountGate::Ready).then(|| account_id.get()).flatten()
}

async fn load_daily_share(account_id: &str) -> Result<bool, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::daily_share(&context, &id).await }).await?;
    Ok(daily_share_can_claim(&value))
}

fn daily_share_can_claim(value: &Value) -> bool {
    value.get("canClaim").and_then(Value::as_bool).unwrap_or(false)
}

async fn load_ranch_friends(account_id: &str) -> Result<Vec<RanchFriend>, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let friends =
        session::call(async move { qq_farm_app::friend::list_friends(&context, &id, false).await })
            .await?;
    Ok(friends
        .into_iter()
        .filter(|friend| friend.gid > 0)
        .map(|friend| RanchFriend { gid: friend.gid, name: friend.name })
        .collect())
}

async fn load_friend_ranch(account_id: &str, host: i64) -> Result<FriendRanchView, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::friend_ranch(&context, &id, host).await })
            .await?;
    Ok(friend_ranch_from(&value))
}

async fn load_book(account_id: &str) -> Result<BookBoard, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value = session::call(async move { qq_farm_app::ranch::book(&context, &id).await }).await?;
    Ok(book_board_from(&value))
}

async fn load_tasks(account_id: &str) -> Result<TaskBoard, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::tasks(&context, &id).await }).await?;
    Ok(task_board_from(&value))
}

async fn load_market(account_id: &str) -> Result<MarketBoard, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::market(&context, &id).await }).await?;
    Ok(market_from(&value))
}

async fn load_can_shop(account_id: &str, slot: i64) -> Result<ShopBoard, String> {
    let slot = i32::try_from(slot).map_err(|_| "未知牧场罐头商城".to_owned())?;
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::can_shop(&context, &id, slot).await })
            .await?;
    Ok(shop_from(&value))
}

async fn load_shop(account_id: &str, shop_id: i64) -> Result<ShopBoard, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::shop(&context, &id, shop_id).await })
            .await?;
    Ok(shop_from(&value))
}

async fn load_board(account_id: &str) -> Result<RanchBoard, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::ranch::snapshot(&context, &id).await }).await?;
    Ok(board_from(&value))
}

fn friend_ranch_from(value: &Value) -> FriendRanchView {
    FriendRanchView {
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        can_steal: value.get("canSteal").and_then(Value::as_bool).unwrap_or(false),
        cleanable: value.get("cleanable").and_then(Value::as_i64).unwrap_or(0),
        pens: value
            .get("pens")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(friend_pen_from).collect())
            .unwrap_or_default(),
    }
}

fn friend_pen_from(value: &Value) -> Option<FriendPenLine> {
    let ranch_id = value.get("id").and_then(Value::as_str)?.parse().ok()?;
    Some(FriendPenLine {
        id: ranch_id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        can_mosquito: value.get("canMosquito").and_then(Value::as_bool).unwrap_or(false),
        animals: value
            .get("animals")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|animal| {
                        Some(StrokeAnimal {
                            ranch_id,
                            id: i32::try_from(animal.get("id").and_then(Value::as_i64)?).ok()?,
                            name: animal
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn book_board_from(value: &Value) -> BookBoard {
    let levels = value.get("levels").and_then(Value::as_array);
    BookBoard {
        level: value.get("level").and_then(Value::as_i64).unwrap_or(0),
        progress: value.get("progress").and_then(Value::as_i64).unwrap_or(0),
        next_progress: value.get("nextProgress").and_then(Value::as_i64).unwrap_or(0),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
        score: value.get("score").and_then(Value::as_i64).unwrap_or(0),
        giant_claim: levels.is_some_and(|rows| {
            rows.iter().any(|row| row.get("canClaim").and_then(Value::as_bool) == Some(true))
        }),
        reward_text: reward_text(value.get("rewards")),
        animals: value
            .get("animals")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(book_animal_from).collect())
            .unwrap_or_default(),
        mutations: book_mutations(value),
        levels: levels
            .map(|rows| rows.iter().filter_map(book_level_from).collect())
            .unwrap_or_default(),
    }
}

fn reward_text(value: Option<&Value>) -> String {
    value
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

fn book_level_from(value: &Value) -> Option<BookLevel> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let claimed = value.get("claimed").and_then(Value::as_bool).unwrap_or(false);
    let ready = value.get("canClaim").and_then(Value::as_bool).unwrap_or(false);
    Some(BookLevel {
        id,
        score: value.get("score").and_then(Value::as_i64).unwrap_or(0).max(0),
        state_key: if claimed {
            "ranch.bookClaimed"
        } else if ready {
            "ranch.bookReady"
        } else {
            "ranch.bookUnreached"
        },
        detail: reward_text(value.get("rewards")),
    })
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

fn visible_animals(animals: &[BookAnimal], filter: &str, query: &str) -> Vec<BookAnimal> {
    let query = query.trim();
    animals
        .iter()
        .filter(|animal| {
            let status = match filter {
                "owned" => animal.unlocked,
                "missing" => !animal.unlocked,
                _ => true,
            };
            status && (query.is_empty() || animal.name.contains(query))
        })
        .cloned()
        .collect()
}

fn book_level_list(catalog: Resource<BookBoard, String>) -> impl IntoElement {
    view().gap_2().child(each(
        move || catalog.get().map(|book| book.levels).unwrap_or_default(),
        |level: &BookLevel| level.id.to_string(),
        move |level, _| {
            let level = level.get_untracked();
            let score = level.score;
            let state_key = level.state_key;
            let detail = SharedString::from(level.detail);
            let shown = !detail.is_empty();
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(text(move || SharedString::from(score.to_string())).text_sm())
                .child(badge(
                    move || text_key(state_key),
                    if state_key == "ranch.bookReady" {
                        BadgeTone::Success
                    } else {
                        BadgeTone::Neutral
                    },
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

fn book_mutations(value: &Value) -> Vec<BookMutation> {
    value
        .get("mutations")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("").trim();
                    if name.is_empty() {
                        return None;
                    }
                    let description =
                        row.get("description").and_then(Value::as_str).unwrap_or("").trim();
                    let probability = row.get("probability").and_then(Value::as_i64).unwrap_or(0);
                    let detail = if probability > 0 {
                        if description.is_empty() {
                            format!("{probability}%")
                        } else {
                            format!("{description} · {probability}%")
                        }
                    } else {
                        description.to_owned()
                    };
                    Some(BookMutation { name: name.to_owned(), detail })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn book_animal_from(value: &Value) -> Option<BookAnimal> {
    let giant = value.get("giant").filter(|giant| !giant.is_null());
    Some(BookAnimal {
        id: value.get("id").and_then(Value::as_i64)?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        unlocked: value.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        weight: giant
            .and_then(|giant| giant.get("weightKg"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        tier: giant
            .and_then(|giant| giant.get("tierName"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        giant_unlocked: giant
            .and_then(|giant| giant.get("unlocked"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        harvest: giant
            .and_then(|giant| giant.get("harvestCount"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0),
        fresh: value.get("isNew").and_then(Value::as_bool).unwrap_or(false)
            || giant.and_then(|giant| giant.get("isNew")).and_then(Value::as_bool).unwrap_or(false),
    })
}

fn task_board_from(value: &Value) -> TaskBoard {
    TaskBoard {
        tasks: value
            .get("tasks")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(task_line_from).collect())
            .unwrap_or_default(),
    }
}

fn task_line_from(value: &Value) -> Option<TaskLine> {
    Some(TaskLine {
        id: value.get("id").and_then(Value::as_str)?.parse().ok()?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        progress: value.get("progress").and_then(Value::as_i64).unwrap_or(0),
        total: value.get("total").and_then(Value::as_i64).unwrap_or(0),
        can_claim: value.get("canClaim").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn market_from(value: &Value) -> MarketBoard {
    MarketBoard {
        goods: value
            .get("goods")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(market_line_from).collect())
            .unwrap_or_default(),
    }
}

fn market_line_from(value: &Value) -> Option<MarketLine> {
    Some(MarketLine {
        id: value.get("id").and_then(Value::as_str)?.parse().ok()?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        price: value.get("price").and_then(Value::as_i64).unwrap_or(0),
        price_name: value.get("priceName").and_then(Value::as_str).unwrap_or("").to_owned(),
        balance: value.get("balance").and_then(Value::as_i64).unwrap_or(0),
        max_count: value.get("maxCount").and_then(Value::as_i64).unwrap_or(0),
        remaining: value.get("remaining").and_then(Value::as_i64),
        available: value.get("available").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn shop_from(value: &Value) -> ShopBoard {
    ShopBoard {
        goods: value
            .get("goods")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(good_from).collect())
            .unwrap_or_default(),
    }
}

fn good_from(value: &Value) -> Option<GoodLine> {
    Some(GoodLine {
        id: value.get("id").and_then(Value::as_str)?.parse().ok()?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        price: value.get("price").and_then(Value::as_i64).unwrap_or(0),
        balance: value.get("balance").and_then(Value::as_i64).unwrap_or(0),
        max_count: value.get("maxCount").and_then(Value::as_i64).unwrap_or(0),
        remaining: value.get("remaining").and_then(Value::as_i64),
        available: value.get("available").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn board_from(value: &Value) -> RanchBoard {
    RanchBoard {
        unlocked: value.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        reason: value.get("reason").and_then(Value::as_str).unwrap_or("").to_owned(),
        grass: value.get("grass").and_then(Value::as_i64).unwrap_or(0),
        normal_seconds: bucket_seconds(value, 7003),
        organic_seconds: bucket_seconds(value, 7004),
        cubs: value
            .get("cubs")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(cub_from).collect())
            .unwrap_or_default(),
        products: value
            .get("products")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(product_from).collect())
            .unwrap_or_default(),
        cans: value
            .get("cans")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(can_from).collect())
            .unwrap_or_default(),
        facilities: value
            .get("facilities")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(facility_from).collect())
            .unwrap_or_default(),
        pending_ops: value
            .get("pending")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(pending_from).collect())
            .unwrap_or_default(),
    }
}

fn pending_from(value: &Value) -> Option<PendingLine> {
    Some(PendingLine {
        id: value.get("id").and_then(Value::as_str)?.to_owned(),
        label: value.get("label").and_then(Value::as_str).unwrap_or("").to_owned(),
        costs: value
            .get("costs")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|cost| {
                        let name = cost.get("name").and_then(Value::as_str)?;
                        let amount = cost.get("amount").and_then(Value::as_str).unwrap_or("0");
                        Some(format!("{name} ×{amount}"))
                    })
                    .collect::<Vec<_>>()
                    .join("  ")
            })
            .unwrap_or_default(),
        message: value.get("message").and_then(Value::as_str).unwrap_or("").to_owned(),
    })
}

fn facility_from(value: &Value) -> Option<FacilityLine> {
    Some(FacilityLine {
        id: value.get("id").and_then(Value::as_str)?.to_owned(),
        level: value.get("level").and_then(Value::as_i64).unwrap_or(0),
        module_type: value.get("moduleType").and_then(Value::as_i64).unwrap_or(0),
        grass_room: value.get("capacity").and_then(Value::as_i64).unwrap_or(0)
            - value.get("grassStored").and_then(Value::as_i64).unwrap_or(0),
        hazard_count: value.get("hazardCount").and_then(Value::as_i64).unwrap_or(0),
        kind: value.get("husbandryKind").and_then(Value::as_i64).unwrap_or(0),
        open_room: value.get("openRoom").and_then(Value::as_i64).unwrap_or(0),
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        pen_count: value.get("penCount").and_then(Value::as_i64).unwrap_or(0),
        pen_capacity: value.get("penCapacity").and_then(Value::as_i64).unwrap_or(0),
        upgrade: value.get("upgrade").and_then(Value::as_str).unwrap_or("").to_owned(),
        animals: value
            .get("animals")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(animal_from).collect())
            .unwrap_or_default(),
    })
}

fn can_from(value: &Value) -> Option<CanLine> {
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    if count <= 0 {
        return None;
    }
    Some(CanLine {
        uid: value.get("uid").and_then(Value::as_str)?.parse().ok()?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        count,
        organic: value.get("organic").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn product_from(value: &Value) -> Option<ProductLine> {
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    if count <= 0 {
        return None;
    }
    Some(ProductLine {
        uid: value.get("uid").and_then(Value::as_str)?.parse().ok()?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        count,
        price: value.get("price").and_then(Value::as_str).unwrap_or("").to_owned(),
    })
}

fn cub_from(value: &Value) -> Option<CubLine> {
    let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
    if count <= 0 {
        return None;
    }
    Some(CubLine {
        id: value.get("id").and_then(Value::as_str)?.parse().ok()?,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        kind: value.get("kind").and_then(Value::as_i64).unwrap_or(0),
        count,
    })
}

fn animal_from(value: &Value) -> Option<AnimalLine> {
    Some(AnimalLine {
        id: i32::try_from(value.get("id").and_then(Value::as_i64)?).ok()?,
        ranch_id: value.get("ranchId").and_then(Value::as_str).unwrap_or("").to_owned(),
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        stage: value.get("stageName").and_then(Value::as_str).unwrap_or("").to_owned(),
        can_harvest: value.get("canHarvest").and_then(Value::as_bool).unwrap_or(false),
        can_produce: value.get("canProduce").and_then(Value::as_bool).unwrap_or(false),
        can_feed: value.get("canFeed").and_then(Value::as_bool).unwrap_or(false),
        can_release: value.get("canRelease").and_then(Value::as_bool).unwrap_or(false),
        limited_can: value.get("limitedCanUsed").and_then(Value::as_bool).unwrap_or(false),
        slot: i32::try_from(value.get("slot").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
    })
}

fn bucket_seconds(value: &Value, id: i64) -> i64 {
    value
        .get("buckets")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id").and_then(Value::as_i64) == Some(id)))
        .and_then(|row| row.get("seconds").and_then(Value::as_i64))
        .unwrap_or(0)
}

#[derive(Clone, PartialEq, Eq)]
struct PenGroup {
    module_type: i64,
    name: String,
    count: i64,
    capacity: i64,
    facilities: Vec<FacilityLine>,
}

fn facility_rank(module_type: i64) -> i64 {
    match module_type {
        6 => 0,
        7 => 1,
        8 => 2,
        other => 9 + other.max(0),
    }
}

fn pen_groups(mut facilities: Vec<FacilityLine>) -> Vec<PenGroup> {
    for facility in &mut facilities {
        facility
            .animals
            .sort_by(|left, right| left.slot.cmp(&right.slot).then(left.id.cmp(&right.id)));
    }
    facilities.sort_by(|left, right| {
        facility_rank(left.module_type)
            .cmp(&facility_rank(right.module_type))
            .then(left.id.cmp(&right.id))
    });
    let mut groups = Vec::new();
    for facility in facilities {
        if groups.last().is_some_and(|group: &PenGroup| group.module_type == facility.module_type) {
            let group = groups.last_mut().expect("分组刚放入");
            group.count += facility.pen_count;
            group.capacity += facility.pen_capacity;
            group.facilities.push(facility);
        } else {
            groups.push(PenGroup {
                module_type: facility.module_type,
                name: if facility.name.is_empty() {
                    facility_rank_name(facility.module_type)
                } else {
                    facility.name.clone()
                },
                count: facility.pen_count,
                capacity: facility.pen_capacity,
                facilities: vec![facility],
            });
        }
    }
    groups
}

fn facility_rank_name(module_type: i64) -> String {
    match module_type {
        6 => "窝",
        7 => "棚",
        8 => "牧草机",
        _ => "设施",
    }
    .to_owned()
}

fn feed_all_ready(board: &RanchBoard, organic: bool) -> bool {
    let seconds = if organic { board.organic_seconds } else { board.normal_seconds };
    seconds > 0
        && board
            .facilities
            .iter()
            .flat_map(|facility| facility.animals.iter())
            .any(|animal| animal.can_feed && (organic || !animal.limited_can))
}

fn locked_reason(board: Resource<RanchBoard, String>) -> SharedString {
    let reason = board.get().map(|board| board.reason).unwrap_or_default();
    if reason.is_empty() {
        text_key("ranch.locked")
    } else {
        SharedString::from(reason)
    }
}

fn grass_label(board: Resource<RanchBoard, String>) -> SharedString {
    let grass = board.get().map(|board| board.grass).unwrap_or(0);
    SharedString::from(format!("{} {grass}", text_key("ranch.grass")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        board_from, book_board_from, can_use_limit, clamp_grass_count, cub_from,
        daily_share_can_claim, friend_ranch_from, grass_fill_limit, market_from, place_limit,
        shop_buy_limit, shop_from, task_board_from, visible_animals,
    };

    #[test]
    fn grass_fill_stops_at_the_smaller_of_stock_and_room() {
        assert_eq!(grass_fill_limit(0, 10), 0);
        assert_eq!(grass_fill_limit(8, 0), 0);
        assert_eq!(grass_fill_limit(8, 3), 3);
        assert_eq!(grass_fill_limit(2, 10), 2);
        assert_eq!(clamp_grass_count(20, 3), 3);
        assert_eq!(clamp_grass_count(0, 3), 1);
        assert_eq!(clamp_grass_count(4, 0), 0);
        assert_eq!(place_limit(4, 2), 2);
        assert_eq!(place_limit(1, 6), 1);
        assert_eq!(place_limit(0, 3), 0);
        let cub =
            cub_from(&json!({ "id": "12", "name": "鸭", "count": 4, "kind": 1 })).expect("cub");
        assert_eq!(cub.count, 4);
        assert_eq!(shop_buy_limit(99, Some(4), 10, 1_000), 4);
        assert_eq!(shop_buy_limit(99, None, 10, 35), 3);
        assert_eq!(shop_buy_limit(0, None, 0, 0), 1);
        let good = super::good_from(&json!({
            "id": "7",
            "name": "牧草",
            "price": 8,
            "balance": 40,
            "maxCount": 99,
            "remaining": 2,
            "available": true
        }))
        .expect("good");
        assert_eq!(good.max_count, 99);
        assert_eq!(good.remaining, Some(2));
        assert_eq!(good.balance, 40);
        assert_eq!(shop_buy_limit(999, Some(2), 8, 40), 2);
        assert_eq!(shop_buy_limit(999, None, 0, 0), 999);
    }

    #[test]
    fn pens_are_grouped_coop_then_shed_and_animals_follow_slot() {
        let shed = facility(7, "2", 1, vec![(4, 2), (3, 1)]);
        let coop = facility(6, "9", 2, vec![(8, 2), (7, 1)]);
        let groups = super::pen_groups(vec![shed, coop]);
        assert_eq!(groups.iter().map(|group| group.module_type).collect::<Vec<_>>(), vec![6, 7]);
        assert_eq!(groups[0].count, 2);
        assert_eq!(
            groups[0].facilities[0].animals.iter().map(|animal| animal.id).collect::<Vec<_>>(),
            vec![7, 8]
        );
    }

    #[test]
    fn feed_all_stays_off_without_time_or_a_feedable_animal() {
        let mut board = super::board_from(&json!({
            "unlocked": true,
            "buckets": [{ "id": 7003, "seconds": 0 }, { "id": 7004, "seconds": 90 }],
            "facilities": [{
                "id": "9",
                "moduleType": 6,
                "animals": [{ "id": 3, "ranchId": "9", "canFeed": true, "limitedCanUsed": true }]
            }]
        }));
        assert!(!super::feed_all_ready(&board, false));
        assert!(super::feed_all_ready(&board, true));
        board.organic_seconds = 0;
        assert!(!super::feed_all_ready(&board, true));
    }

    fn facility(
        module_type: i64,
        id: &str,
        count: i64,
        animals: Vec<(i32, i32)>,
    ) -> super::FacilityLine {
        super::FacilityLine {
            id: id.to_owned(),
            level: 1,
            module_type,
            grass_room: 0,
            hazard_count: 0,
            kind: 0,
            open_room: 0,
            name: String::new(),
            pen_count: count,
            pen_capacity: 4,
            upgrade: String::new(),
            animals: animals
                .into_iter()
                .map(|(id, slot)| super::AnimalLine {
                    id,
                    ranch_id: "9".to_owned(),
                    name: String::new(),
                    stage: String::new(),
                    can_harvest: false,
                    can_produce: false,
                    can_feed: false,
                    can_release: false,
                    limited_can: false,
                    slot,
                })
                .collect(),
        }
    }

    #[test]
    fn daily_share_button_follows_can_claim() {
        assert!(daily_share_can_claim(&json!({ "canClaim": true })));
        assert!(!daily_share_can_claim(&json!({ "canClaim": false })));
        assert!(!daily_share_can_claim(&json!({})));
    }

    #[test]
    fn locked_board_keeps_the_reason() {
        let board = board_from(&json!({ "unlocked": false, "reason": "未开启" }));
        assert!(!board.unlocked);
        assert_eq!(board.reason, "未开启");
        assert!(board.facilities.is_empty());
        assert!(board.pending_ops.is_empty());
        let held = board_from(&json!({
            "unlocked": true,
            "pending": [{
                "id": "op-1",
                "label": "购买牧草 ×2",
                "message": "操作结果待核对",
                "costs": [{ "name": "金券", "amount": "8" }]
            }]
        }));
        assert_eq!(held.pending_ops[0].label, "购买牧草 ×2");
        assert_eq!(held.pending_ops[0].costs, "金券 ×8");
    }

    #[test]
    fn animals_keep_harvest_state() {
        let board = board_from(&json!({
            "unlocked": true,
            "grass": 12,
            "facilities": [{
                "id": "9",
                "level": 2,
                "animals": [{ "id": 3, "ranchId": "9", "name": "动物30", "stageName": "可收获", "canHarvest": true }]
            }]
        }));
        assert_eq!(board.grass, 12);
        assert!(board.facilities[0].animals[0].can_harvest);
    }

    #[test]
    fn cubs_keep_the_stall_kind() {
        let board = board_from(&json!({
            "unlocked": true,
            "cubs": [
                { "id": "60001", "name": "鸡", "count": 2, "kind": 1 },
                { "id": "1", "name": "空", "count": 0, "kind": 1 }
            ],
            "facilities": [{ "id": "9", "husbandryKind": 1, "openRoom": 3 }]
        }));
        assert_eq!(board.cubs.len(), 1);
        assert_eq!(board.cubs[0].kind, 1);
        assert_eq!(board.facilities[0].open_room, 3);
    }

    #[test]
    fn products_keep_the_shown_price() {
        let board = board_from(&json!({
            "products": [
                { "uid": "7", "name": "鸡", "count": 2, "price": "金券 3" },
                { "uid": "8", "name": "空", "count": 0, "price": "" }
            ]
        }));
        assert_eq!(board.products.len(), 1);
        assert_eq!(board.products[0].price, "金券 3");
    }

    #[test]
    fn cans_keep_the_bucket_kind() {
        let board = board_from(&json!({
            "cans": [
                { "uid": "4", "name": "罐头", "count": 2, "organic": false },
                { "uid": "5", "name": "空", "count": 0, "organic": true }
            ]
        }));
        assert_eq!(board.cans.len(), 1);
        assert!(!board.cans[0].organic);
        assert_eq!(board.cans[0].count, 2);
        assert_eq!(can_use_limit(2), 2);
        assert_eq!(can_use_limit(i64::from(i32::MAX) + 5), i64::from(i32::MAX));
    }

    #[test]
    fn shop_keeps_only_priced_goods() {
        let shop = shop_from(&json!({
            "goods": [
                { "id": "10002", "name": "鸡", "price": 2, "available": true },
                { "name": "缺编号" }
            ]
        }));
        assert_eq!(shop.goods.len(), 1);
        assert_eq!(shop.goods[0].name, "鸡");
        assert!(shop.goods[0].available);
    }

    #[test]
    fn friend_ranch_keeps_steal_and_stroke() {
        let ranch = friend_ranch_from(&json!({
            "name": "好友",
            "canSteal": true,
            "cleanable": 1,
            "pens": [
                { "id": "9", "name": "窝", "canMosquito": true, "animals": [{ "id": 3, "name": "鸡" }] },
                { "name": "缺编号" }
            ]
        }));
        assert!(ranch.can_steal);
        assert_eq!(ranch.cleanable, 1);
        assert_eq!(ranch.pens.len(), 1);
        assert!(ranch.pens[0].can_mosquito);
        assert_eq!(ranch.pens[0].animals[0].name, "鸡");
    }

    #[test]
    fn book_keeps_claimable_animals() {
        let book = book_board_from(&json!({
            "level": 2,
            "progress": 3,
            "nextProgress": 5,
            "canClaim": true,
            "score": 30,
            "levels": [{ "id": 1, "score": 10, "canClaim": true, "rewards": [{ "name": "金券", "count": 2 }] }, { "id": 2, "canClaim": false, "claimed": true }],
            "rewards": [{ "name": "金券", "count": 4 }],
            "animals": [
                { "id": 9, "name": "未知", "unlocked": false, "isNew": true, "giant": { "unlocked": false, "tierName": "巨无霸", "weightKg": "11.24", "harvestCount": 2 } },
                { "name": "缺编号" }
            ],
            "mutations": [{ "name": "巨大", "description": "饲养或好友照料", "probability": 10 }]
        }));
        assert!(book.can_claim);
        assert!(book.giant_claim);
        assert_eq!(book.animals.len(), 1);
        assert_eq!(book.animals[0].weight, "11.24");
        assert_eq!(book.animals[0].tier, "巨无霸");
        assert!(!book.animals[0].giant_unlocked);
        assert_eq!(book.animals[0].harvest, 2);
        assert!(book.animals[0].fresh);
        assert_eq!(book.reward_text, "金券 ×4");
        assert_eq!(book.levels[0].state_key, "ranch.bookReady");
        assert_eq!(book.levels[0].detail, "金券 ×2");
        assert_eq!(book.levels[1].state_key, "ranch.bookClaimed");
        assert_eq!(visible_animals(&book.animals, "missing", "").len(), 1);
        assert!(visible_animals(&book.animals, "owned", "").is_empty());
        assert!(visible_animals(&book.animals, "all", "鸡").is_empty());
        assert_eq!(book.mutations[0].name, "巨大");
        assert_eq!(book.mutations[0].detail, "饲养或好友照料 · 10%");
    }

    #[test]
    fn tasks_keep_claimable_rows() {
        let tasks = task_board_from(&json!({
            "tasks": [
                { "id": "21", "name": "每日", "progress": 2, "total": 2, "canClaim": true },
                { "name": "缺编号" }
            ]
        }));
        assert_eq!(tasks.tasks.len(), 1);
        assert!(tasks.tasks[0].can_claim);
        assert_eq!(tasks.tasks[0].name, "每日");
    }

    #[test]
    fn market_keeps_available_goods() {
        let market = market_from(&json!({
            "goods": [
                { "id": "3", "name": "鸡", "price": 1, "priceName": "金券", "available": true },
                { "name": "缺编号" }
            ]
        }));
        assert_eq!(market.goods.len(), 1);
        assert!(market.goods[0].available);
        assert_eq!(market.goods[0].price_name, "金券");
    }
}
