use chrono::{Local, TimeZone};
use nsh_base::alert_dialog;
use nsh_core::prelude::*;
use qq_farm_core::constants::{
    COLLECTION_NORMAL_ID, COLLECTION_RARE_ID, COUPON_ITEM_ID, NORMAL_CONTAINER_ID,
    ORGANIC_CONTAINER_ID,
};
use qq_farm_core::services::warehouse::{BagDetail, BagItemView};

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, notice, BadgeTone, ButtonVariant};

const LEGACY_GOLD_ITEM_ID: i64 = 1;
const GOLD_ITEM_ID: i64 = 1001;
const EXP_ITEM_ID: i64 = 1101;
const ITEM_SEED: i64 = 5;
const ITEM_FRUIT: i64 = 6;
const ITEM_TOOL: i64 = 11;
const ITEM_PACK: i64 = 33;
const ITEM_MUTANT_FRUIT: i64 = 17;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PickMode {
    Off,
    Sell,
    Lock,
    Unlock,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BagFilter {
    All,
    Fruit,
    Seed,
    Tool,
    Other,
}

impl BagFilter {
    const ALL: [Self; 5] = [Self::All, Self::Fruit, Self::Seed, Self::Tool, Self::Other];

    const fn id(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Fruit => "fruit",
            Self::Seed => "seed",
            Self::Tool => "tool",
            Self::Other => "other",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::All => "bag.all",
            Self::Fruit => "bag.fruit",
            Self::Seed => "bag.seed",
            Self::Tool => "bag.tool",
            Self::Other => "bag.other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BagKind {
    Fruit,
    Seed,
    Tool,
    Other,
}

impl BagKind {
    const fn label_key(self) -> &'static str {
        match self {
            Self::Fruit => "bag.fruit",
            Self::Seed => "bag.seed",
            Self::Tool => "bag.tool",
            Self::Other => "bag.other",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct BagLine {
    key: String,
    id: i64,
    uid: i64,
    name: String,
    count: i64,
    kind: BagKind,
    sellable: bool,
    lockable: bool,
    locked: bool,
    usable: bool,
    price_id: i64,
    price: i64,
    gift: Option<GiftNote>,
    sell_wait: Option<SellWait>,
    level: i64,
    mutants: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SellWait {
    ActivityEnded,
    ActivityOpen,
    OutsideActivity,
    AfterExpire,
    Other,
}

#[derive(Clone, PartialEq, Eq, Default)]
struct GiftNote {
    sender: String,
    sent_at: i64,
    message_id: i64,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let filter = signal(BagFilter::All);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let summary = signal(None::<String>);
    let use_counts = signal(Vec::<(String, i64)>::new());
    let choosing = signal(PickMode::Off);
    let picked = signal(Vec::<String>::new());
    let bag = resource(
        move || running_account(account_id, rows),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::farm::bag(&context, &id).await }).await
        },
    );
    view()
        .gap_3()
        .child(failure_line(failure))
        .child(when(
            move || summary.get().is_some(),
            move || notice(move || summary.get().map(SharedString::from).unwrap_or_default()),
        ))
        .child(gate_notice(account_id, rows, "farm.needRunning"))
        .child(when(
            move || running(account_id, rows) && bag.loading() && bag.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || running(account_id, rows) && bag.error().is_some(),
            move || load_error(bag),
        ))
        .child(when(
            move || running(account_id, rows) && bag.get().is_some(),
            move || {
                bag_body(
                    account_id, filter, pending, failure, summary, bag, use_counts, choosing,
                    picked,
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

fn load_error(bag: Resource<BagDetail, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || bag.error().map(SharedString::from).unwrap_or_default()))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || bag.refetch()),
        )
}

fn bag_body(
    account_id: Signal<Option<String>>,
    filter: Signal<BagFilter>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
    use_counts: Signal<Vec<(String, i64)>>,
    choosing: Signal<PickMode>,
    picked: Signal<Vec<String>>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(filters(filter))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                        .disabled(move || pending.get().is_some())
                        .on_press(move || bag.refetch()),
                )
                .child(pick_modes(bag, filter, choosing, picked, pending))
                .child(when(
                    move || {
                        !mode_picks(&visible_lines(bag, filter), &picked.get(), choosing.get())
                            .is_empty()
                    },
                    move || {
                        apply_picked(
                            account_id, filter, pending, failure, summary, bag, picked, choosing,
                        )
                    },
                )),
        )
        .child(when(
            move || visible_lines(bag, filter).is_empty(),
            move || notice(move || text_key("bag.empty")),
        ))
        .child(each(
            move || visible_lines(bag, filter),
            |line: &BagLine| line.key.clone(),
            move |line, _| {
                bag_line(
                    line.get_untracked(),
                    account_id,
                    pending,
                    failure,
                    summary,
                    bag,
                    use_counts,
                    choosing,
                    picked,
                )
            },
        ))
}

fn filters(filter: Signal<BagFilter>) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || BagFilter::ALL.to_vec(),
        |item: &BagFilter| item.id().to_owned(),
        move |item, _| {
            let item = item.get_untracked();
            button(move || text_key(item.label_key()), ButtonVariant::Quiet)
                .selected(move || filter.get() == item)
                .on_press(move || filter.set(item))
        },
    ))
}

fn visible_lines(bag: Resource<BagDetail, String>, filter: Signal<BagFilter>) -> Vec<BagLine> {
    bag.get().map(|detail| bag_lines(&detail.items, filter.get())).unwrap_or_default()
}

fn bag_line(
    line: BagLine,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
    use_counts: Signal<Vec<(String, i64)>>,
    choosing: Signal<PickMode>,
    picked: Signal<Vec<String>>,
) -> View {
    let name = SharedString::from(line.name.clone());
    let count = SharedString::from(line.count.to_string());
    let kind = line.kind;
    let stack = line.count;
    let gift = line.gift.clone();
    let sell_wait = line.sell_wait;
    let level = line.level;
    let mutants = SharedString::from(line.mutants.clone());
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(
            view()
                .flex_col()
                .gap_1()
                .flex_1()
                .child(text(name).text_sm().font_weight(FontWeight::MEDIUM))
                .child(when(
                    {
                        let shown = gift.clone();
                        move || shown.is_some()
                    },
                    {
                        let shown = gift;
                        move || gift_lines(shown.clone().unwrap_or_default())
                    },
                ))
                .child(when(
                    move || sell_wait.is_some(),
                    move || {
                        text(move || text_key(sell_wait_key(sell_wait)))
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                    },
                ))
                .child(when(
                    {
                        let shown = mutants.clone();
                        move || !shown.is_empty()
                    },
                    {
                        let shown = mutants;
                        move || {
                            let shown = shown.clone();
                            text(move || {
                                SharedString::from(format!("{} · {shown}", text_key("bag.mutant")))
                            })
                            .text_sm()
                            .text_color(move || nsh_base::theme().muted_foreground)
                        }
                    },
                )),
        )
        .child(text(count).text_sm())
        .child(when(
            move || level > 0,
            move || {
                text(SharedString::from(format!("Lv{level}")))
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            {
                let shown_price = line.price;
                move || shown_price > 0
            },
            {
                let price_id = line.price_id;
                let price = line.price;
                move || {
                    text(move || SharedString::from(price_text(price_id, price)))
                        .text_sm()
                        .text_color(move || nsh_base::theme().muted_foreground)
                }
            },
        ))
        .child(badge(move || text_key(kind.label_key()), BadgeTone::Neutral))
        .child(when(
            {
                let locked = line.locked;
                move || locked
            },
            || badge(|| text_key("bag.locked"), BadgeTone::Neutral),
        ))
        .child(when(
            {
                let usable = line.usable;
                move || usable && stack > 1
            },
            {
                let step_line = line.clone();
                move || use_stepper(step_line.clone(), use_counts, pending)
            },
        ))
        .child(when(
            {
                let usable = line.usable;
                move || usable
            },
            {
                let use_line = line.clone();
                move || {
                    command_button(
                        "bag.use",
                        "use",
                        ButtonVariant::Primary,
                        use_line.clone(),
                        account_id,
                        pending,
                        failure,
                        summary,
                        bag,
                        use_counts,
                    )
                }
            },
        ))
        .child(when(
            {
                let lockable = line.lockable;
                move || choosing.get() == PickMode::Off && lockable
            },
            {
                let lock_line = line.clone();
                move || {
                    let locked = lock_line.locked;
                    command_button(
                        if locked { "bag.unlock" } else { "bag.lock" },
                        if locked { "unlock" } else { "lock" },
                        ButtonVariant::Secondary,
                        lock_line.clone(),
                        account_id,
                        pending,
                        failure,
                        summary,
                        bag,
                        use_counts,
                    )
                }
            },
        ))
        .child(when(
            {
                let sellable = line.sellable;
                let lockable = line.lockable;
                let locked = line.locked;
                move || eligible(choosing.get(), sellable, lockable, locked)
            },
            {
                let key = line.key.clone();
                move || pick_button(key.clone(), picked, pending)
            },
        ))
        .child(when(
            {
                let sellable = line.sellable;
                move || choosing.get() == PickMode::Off && sellable
            },
            {
                let sell_line = line;
                move || {
                    command_button(
                        "bag.sell",
                        "sell",
                        ButtonVariant::Danger,
                        sell_line.clone(),
                        account_id,
                        pending,
                        failure,
                        summary,
                        bag,
                        use_counts,
                    )
                }
            },
        ))
}

fn command_button(
    label_key: &'static str,
    action: &'static str,
    variant: ButtonVariant,
    line: BagLine,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
    use_counts: Signal<Vec<(String, i64)>>,
) -> nsh_base::ButtonElement {
    let key = action_key(&line, action);
    button(
        move || {
            if pending.get().as_deref() == Some(key.as_str()) {
                text_key("farm.working")
            } else {
                text_key(label_key)
            }
        },
        variant,
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || {
        let count = if action == "sell" {
            line.count
        } else {
            chosen_use(use_counts, &line.key, line.count.max(1))
        };
        press_bag(action, line.clone(), count, account_id, pending, failure, summary, bag);
    })
}

fn chosen_count(stored: i64, max_count: i64) -> i64 {
    stored.clamp(1, max_count.max(1))
}

fn chosen_use(counts: Signal<Vec<(String, i64)>>, key: &str, max_count: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(item_key, _)| item_key == key)
        .map(|(_, count)| count)
        .unwrap_or(1);
    chosen_count(stored, max_count)
}

fn set_use(counts: Signal<Vec<(String, i64)>>, key: &str, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(item_key, _)| item_key == key) {
        row.1 = count;
    } else {
        rows.push((key.to_owned(), count));
    }
    counts.set(rows);
}

fn use_stepper(
    line: BagLine,
    counts: Signal<Vec<(String, i64)>>,
    pending: Signal<Option<String>>,
) -> impl IntoElement {
    let key = line.key;
    let max_count = line.count.max(1);
    let minus = key.clone();
    let label = key.clone();
    let plus = key;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child({
            let disabled_key = minus.clone();
            let press_key = minus;
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some() || chosen_use(counts, &disabled_key, max_count) <= 1
                })
                .on_press(move || {
                    let next = chosen_use(counts, &press_key, max_count).saturating_sub(1).max(1);
                    set_use(counts, &press_key, next);
                })
        })
        .child(
            text(move || SharedString::from(chosen_use(counts, &label, max_count).to_string()))
                .text_sm(),
        )
        .child({
            let disabled_key = plus.clone();
            let press_key = plus;
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    pending.get().is_some()
                        || chosen_use(counts, &disabled_key, max_count) >= max_count
                })
                .on_press(move || {
                    let next =
                        chosen_use(counts, &press_key, max_count).saturating_add(1).min(max_count);
                    set_use(counts, &press_key, next);
                })
        })
}

fn pick_modes(
    bag: Resource<BagDetail, String>,
    filter: Signal<BagFilter>,
    choosing: Signal<PickMode>,
    picked: Signal<Vec<String>>,
    pending: Signal<Option<String>>,
) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(when(
            move || visible_lines(bag, filter).iter().any(|line| line.sellable),
            move || mode_button("bag.chooseSell", PickMode::Sell, choosing, picked, pending),
        ))
        .child(when(
            move || visible_lines(bag, filter).iter().any(|line| line.lockable && !line.locked),
            move || mode_button("bag.chooseLock", PickMode::Lock, choosing, picked, pending),
        ))
        .child(when(
            move || visible_lines(bag, filter).iter().any(|line| line.lockable && line.locked),
            move || mode_button("bag.chooseUnlock", PickMode::Unlock, choosing, picked, pending),
        ))
}

fn mode_button(
    label_key: &'static str,
    next: PickMode,
    choosing: Signal<PickMode>,
    picked: Signal<Vec<String>>,
    pending: Signal<Option<String>>,
) -> nsh_base::ButtonElement {
    button(move || text_key(label_key), ButtonVariant::Secondary)
        .selected(move || choosing.get() == next)
        .disabled(move || pending.get().is_some())
        .on_press(move || {
            if choosing.get_untracked() == next {
                choosing.set(PickMode::Off);
            } else {
                choosing.set(next);
            }
            picked.set(Vec::new());
        })
}

fn eligible(mode: PickMode, sellable: bool, lockable: bool, locked: bool) -> bool {
    match mode {
        PickMode::Sell => sellable,
        PickMode::Lock => lockable && !locked,
        PickMode::Unlock => lockable && locked,
        PickMode::Off => false,
    }
}

fn pick_button(
    key: String,
    picked: Signal<Vec<String>>,
    pending: Signal<Option<String>>,
) -> nsh_base::ButtonElement {
    let selected_key = key.clone();
    let label_key = key.clone();
    button(
        move || {
            text_key(if picked.get().iter().any(|item| item == &label_key) {
                "bag.picked"
            } else {
                "bag.pick"
            })
        },
        ButtonVariant::Secondary,
    )
    .selected(move || picked.get().iter().any(|item| item == &selected_key))
    .disabled(move || pending.get().is_some())
    .on_press(move || picked.set(toggle_key(&picked.get_untracked(), &key)))
}

fn apply_picked(
    account_id: Signal<Option<String>>,
    filter: Signal<BagFilter>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
    picked: Signal<Vec<String>>,
    choosing: Signal<PickMode>,
) -> nsh_base::ButtonElement {
    button(
        move || {
            let mode = choosing.get();
            let count = mode_picks(&visible_lines(bag, filter), &picked.get(), mode).len();
            let label = match mode {
                PickMode::Lock => "bag.lockSelected",
                PickMode::Unlock => "bag.unlockSelected",
                _ => "bag.sellSelected",
            };
            SharedString::from(text_key(label).to_string().replace("{count}", &count.to_string()))
        },
        ButtonVariant::Danger,
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || {
        confirm_picked(account_id, filter, pending, failure, summary, bag, picked, choosing)
    })
}

fn confirm_picked(
    account_id: Signal<Option<String>>,
    filter: Signal<BagFilter>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
    picked: Signal<Vec<String>>,
    choosing: Signal<PickMode>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else {
        return;
    };
    let mode = choosing.get_untracked();
    let picks = mode_picks(&visible_lines(bag, filter), &picked.get_untracked(), mode);
    if picks.is_empty() {
        return;
    }
    let count = picks.len();
    let (title, body_key, action, label) = match mode {
        PickMode::Lock => {
            ("bag.confirmBatchLock.title", "bag.confirmBatchLock.body", "lock", "bag.lock")
        }
        PickMode::Unlock => {
            ("bag.confirmBatchUnlock.title", "bag.confirmBatchUnlock.body", "unlock", "bag.unlock")
        }
        _ => ("bag.confirmBatch.title", "bag.confirmBatch.body", "sell", "bag.sell"),
    };
    pending.set(Some(format!("batch-{action}")));
    failure.set(None);
    summary.set(None);
    let body = SharedString::from(text_key(body_key).replace("{count}", &count.to_string()));
    let answer = alert_dialog()
        .title(text_key(title))
        .description(body)
        .cancel("cancel", text_key("accounts.cancel"))
        .destructive(action, text_key(label))
        .close_on_escape(true)
        .show();
    spawn(async move {
        let accepted = answer.await.is_ok_and(|result| result.action.as_deref() == Some(action));
        if !accepted {
            pending.set(None);
            return;
        }
        finish_picked(picks, mode, id, pending, failure, summary, bag, picked, choosing).await;
    });
}

async fn finish_picked(
    picks: Vec<BagLine>,
    mode: PickMode,
    account_id: String,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
    picked: Signal<Vec<String>>,
    choosing: Signal<PickMode>,
) {
    let context = session::session().context.clone();
    let result = session::call(async move {
        match mode {
            PickMode::Lock | PickMode::Unlock => {
                let uids: Vec<i64> =
                    picks.iter().map(|line| line.uid).filter(|uid| *uid > 0).collect();
                qq_farm_app::farm::bag_set_locked(
                    &context,
                    &account_id,
                    &uids,
                    mode == PickMode::Lock,
                )
                .await
            }
            _ => {
                let items: Vec<(i64, i64, i64)> =
                    picks.iter().map(|line| (line.id, line.count, line.uid)).collect();
                qq_farm_app::farm::bag_sell(&context, &account_id, &items).await
            }
        }
    })
    .await;
    match result {
        Ok(value) => {
            let text = value.get("summary").and_then(|item| item.as_str()).unwrap_or("");
            summary.set(Some(if text.is_empty() {
                text_key("bag.done").to_string()
            } else {
                text.to_owned()
            }));
            picked.set(Vec::new());
            choosing.set(PickMode::Off);
        }
        Err(message) => failure.set(Some(message)),
    }
    pending.set(None);
    bag.refetch();
}

fn mode_picks(lines: &[BagLine], picked: &[String], mode: PickMode) -> Vec<BagLine> {
    lines
        .iter()
        .filter(|line| {
            picked.iter().any(|key| key == &line.key)
                && eligible(mode, line.sellable, line.lockable, line.locked)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
fn sell_picks(lines: &[BagLine], picked: &[String]) -> Vec<(i64, i64, i64)> {
    lines
        .iter()
        .filter(|line| line.sellable && picked.iter().any(|key| key == &line.key))
        .map(|line| (line.id, line.count, line.uid))
        .collect()
}

fn toggle_key(picked: &[String], key: &str) -> Vec<String> {
    if picked.iter().any(|item| item == key) {
        picked.iter().filter(|item| item.as_str() != key).cloned().collect()
    } else {
        let mut next = picked.to_vec();
        next.push(key.to_owned());
        next
    }
}

fn action_key(line: &BagLine, action: &str) -> String {
    format!("{}:{}:{action}", line.id, line.uid)
}

fn press_bag(
    action: &'static str,
    line: BagLine,
    count: i64,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else {
        return;
    };
    pending.set(Some(action_key(&line, action)));
    failure.set(None);
    summary.set(None);
    if matches!(action, "sell" | "lock" | "unlock") {
        let (title, body, confirm_key) = match action {
            "lock" => ("bag.confirmLock.title", "bag.confirmLock.body", "bag.lock"),
            "unlock" => ("bag.confirmUnlock.title", "bag.confirmUnlock.body", "bag.unlock"),
            _ => ("bag.confirmSell.title", "bag.confirmSell.body", "bag.sell"),
        };
        confirm::confirm(title, body, action, confirm_key, move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            spawn(async move {
                finish_bag(action, line, count, id, pending, failure, summary, bag).await
            });
        });
        return;
    }
    spawn(async move { finish_bag(action, line, count, id, pending, failure, summary, bag).await });
}

async fn finish_bag(
    action: &'static str,
    line: BagLine,
    count: i64,
    account_id: String,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    bag: Resource<BagDetail, String>,
) {
    let context = session::session().context.clone();
    let result = session::call(async move {
        if action == "sell" {
            qq_farm_app::farm::bag_sell(&context, &account_id, &[(line.id, line.count, line.uid)])
                .await
        } else if action == "lock" || action == "unlock" {
            qq_farm_app::farm::bag_set_locked(&context, &account_id, &[line.uid], action == "lock")
                .await
        } else {
            if is_dog_food(line.id) {
                qq_farm_app::pets::pet_food_use(&context, &account_id, line.id, count, line.uid)
                    .await
            } else {
                qq_farm_app::farm::bag_use(&context, &account_id, line.id, count, line.uid).await
            }
        }
    })
    .await;
    match result {
        Ok(value) => {
            let text = value.get("summary").and_then(|item| item.as_str()).unwrap_or("");
            summary.set(Some(if text.is_empty() {
                text_key("bag.done").to_string()
            } else {
                text.to_owned()
            }));
        }
        Err(message) => failure.set(Some(message)),
    }
    pending.set(None);
    bag.refetch();
}

fn bag_lines(items: &[BagItemView], filter: BagFilter) -> Vec<BagLine> {
    items
        .iter()
        .filter(|item| !is_balance_item(item.id))
        .map(bag_line_from)
        .filter(|line| filter == BagFilter::All || matches_filter(line.kind, filter))
        .collect()
}

fn bag_line_from(item: &BagItemView) -> BagLine {
    let kind = kind_of(item.item_type);
    let name =
        if item.name.trim().is_empty() { format!("#{}", item.id) } else { item.name.clone() };
    BagLine {
        key: if item.key.is_empty() {
            format!("{}:{}", item.id, item.uid)
        } else {
            item.key.clone()
        },
        id: item.id,
        uid: item.uid,
        name,
        count: item.count,
        sellable: kind == BagKind::Fruit && item.sellable && !item.locked,
        lockable: item.uid > 0
            && matches!(item.item_type, ITEM_SEED | ITEM_FRUIT | ITEM_MUTANT_FRUIT),
        locked: item.locked,
        usable: can_use(item),
        price_id: item.price_id,
        price: item.price,
        gift: gift_note(item),
        sell_wait: sell_wait(item),
        level: item.level,
        mutants: mutant_label(item),
        kind,
    }
}

fn mutant_label(item: &BagItemView) -> String {
    let names = item
        .mutant_effects
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if !names.is_empty() {
        return names.join(" + ");
    }
    item.mutant_types
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn sell_wait(item: &BagItemView) -> Option<SellWait> {
    if item.sell_status != "conditional" {
        return None;
    }
    let condition = item.sell_condition.as_deref().unwrap_or("");
    let kind = condition.split(';').next().unwrap_or("").split(':').next().unwrap_or("").trim();
    Some(match kind {
        "活动结束后" => SellWait::ActivityEnded,
        "活动结束前" => SellWait::ActivityOpen,
        "活动区间外" => SellWait::OutsideActivity,
        "道具过期后" => SellWait::AfterExpire,
        _ => SellWait::Other,
    })
}

fn sell_wait_key(wait: Option<SellWait>) -> &'static str {
    match wait {
        Some(SellWait::ActivityEnded) => "bag.waitActivity",
        Some(SellWait::ActivityOpen) => "bag.waitOpen",
        Some(SellWait::OutsideActivity) => "bag.waitOutside",
        Some(SellWait::AfterExpire) => "bag.waitExpire",
        _ => "bag.waitCondition",
    }
}

fn gift_note(item: &BagItemView) -> Option<GiftNote> {
    if !item.viewable {
        return None;
    }
    let source = item.source.as_ref()?;
    Some(GiftNote {
        sender: source.sender_name.clone(),
        sent_at: source.sent_at,
        message_id: source.message_text_id,
    })
}

fn price_text(price_id: i64, price: i64) -> String {
    let unit = match price_id {
        1002 => "dashboard.coupon",
        1005 => "dashboard.bean",
        _ => "dashboard.gold",
    };
    format!("{price} {}", text_key(unit))
}

fn recorded_sent(sent_at: i64) -> Option<String> {
    if sent_at <= 0 {
        return None;
    }
    Local.timestamp_opt(sent_at, 0).single().map(|time| time.format("%Y-%m-%d %H:%M").to_string())
}

fn gift_lines(note: GiftNote) -> View {
    let sender = if note.sender.is_empty() {
        text_key("bag.senderUnknown").to_string()
    } else {
        note.sender
    };
    let sent =
        recorded_sent(note.sent_at).unwrap_or_else(|| text_key("bag.timeMissing").to_string());
    let message = if note.message_id > 0 {
        text_key("bag.messageId").replace("{id}", &note.message_id.to_string())
    } else {
        text_key("bag.messageMissing").to_string()
    };
    view()
        .gap_1()
        .child(muted_fact("bag.sender", sender))
        .child(muted_fact("bag.sent", sent))
        .child(muted_fact("bag.message", message))
}

fn muted_fact(label_key: &'static str, value: String) -> impl IntoElement {
    text(move || SharedString::from(format!("{} {}", text_key(label_key), value.clone())))
        .text_sm()
        .text_color(move || nsh_base::theme().muted_foreground)
}

fn can_use(item: &BagItemView) -> bool {
    if item.locked || item.count <= 0 {
        return false;
    }
    item.item_type == ITEM_TOOL
        || item.item_type == ITEM_PACK
        || qq_farm_core::services::warehouse::is_fertilizer_related_item_id(item.id)
}

fn kind_of(item_type: i64) -> BagKind {
    match item_type {
        ITEM_SEED => BagKind::Seed,
        ITEM_FRUIT | ITEM_MUTANT_FRUIT => BagKind::Fruit,
        ITEM_TOOL => BagKind::Tool,
        _ => BagKind::Other,
    }
}

fn matches_filter(kind: BagKind, filter: BagFilter) -> bool {
    matches!(
        (kind, filter),
        (BagKind::Fruit, BagFilter::Fruit)
            | (BagKind::Seed, BagFilter::Seed)
            | (BagKind::Tool, BagFilter::Tool)
            | (BagKind::Other, BagFilter::Other)
    )
}

fn is_dog_food(id: i64) -> bool {
    matches!(id, 90004 | 90005 | 90006)
}

fn is_balance_item(id: i64) -> bool {
    matches!(
        id,
        LEGACY_GOLD_ITEM_ID
            | GOLD_ITEM_ID
            | EXP_ITEM_ID
            | COUPON_ITEM_ID
            | NORMAL_CONTAINER_ID
            | ORGANIC_CONTAINER_ID
            | COLLECTION_NORMAL_ID
            | COLLECTION_RARE_ID
    )
}

#[cfg(test)]
mod tests {
    use super::{
        bag_line_from, bag_lines, can_use, chosen_count, gift_note, is_balance_item, is_dog_food,
        mode_picks, recorded_sent, sell_picks, sell_wait, toggle_key, BagFilter, BagKind, PickMode,
        SellWait, ITEM_FRUIT, ITEM_PACK, ITEM_SEED,
    };
    use qq_farm_core::services::warehouse::{BagItemView, ItemSource};

    fn item(id: i64, item_type: i64, sellable: bool) -> BagItemView {
        BagItemView {
            key: format!("{id}"),
            id,
            count: 3,
            name: format!("物品{id}"),
            item_type,
            sellable,
            ..BagItemView::default()
        }
    }

    #[test]
    fn balances_and_containers_stay_hidden() {
        let items = vec![
            item(1, 0, false),
            item(1001, 0, false),
            item(1101, 0, false),
            item(1002, 0, false),
            item(1011, 0, false),
            item(200, ITEM_FRUIT, true),
        ];
        let lines = bag_lines(&items, BagFilter::All);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].id, 200);
        assert!(lines[0].sellable);
        assert!(is_balance_item(3001));
    }

    #[test]
    fn filters_keep_the_matching_kind() {
        let items = vec![item(5, 5, false), item(11, 11, false), item(6, 6, false)];
        assert_eq!(bag_lines(&items, BagFilter::Seed)[0].kind, BagKind::Seed);
        assert_eq!(bag_lines(&items, BagFilter::Tool)[0].kind, BagKind::Tool);
        assert!(!bag_lines(&items, BagFilter::Fruit)[0].sellable);
    }

    #[test]
    fn use_count_stays_inside_the_stack() {
        assert_eq!(chosen_count(0, 4), 1);
        assert_eq!(chosen_count(9, 4), 4);
        assert_eq!(chosen_count(2, 0), 1);
        assert!(is_dog_food(90005));
        assert!(!is_dog_food(90001));
    }

    #[test]
    fn unsellable_fruit_has_no_sell_action() {
        let lines = bag_lines(&[item(6, 17, false)], BagFilter::Fruit);
        assert_eq!(lines[0].kind, BagKind::Fruit);
        assert!(!lines[0].sellable);
    }

    #[test]
    fn locked_fruit_cannot_be_sold_and_seeds_can_be_locked() {
        let mut fruit = item(200, ITEM_FRUIT, true);
        fruit.uid = 9;
        fruit.locked = true;
        let mut seed = item(3, ITEM_SEED, false);
        seed.uid = 4;
        let lines = bag_lines(&[fruit, seed], BagFilter::All);
        assert!(lines[0].locked);
        assert!(lines[0].lockable);
        assert!(!lines[0].sellable);
        assert!(!lines[1].locked);
        assert!(lines[1].lockable);
    }

    #[test]
    fn gift_rows_keep_the_sender_and_skip_unmarked_items() {
        let mut gifted = item(1026, 19, true);
        gifted.viewable = true;
        gifted.source =
            Some(ItemSource { sender_name: "甲".to_owned(), sent_at: 0, message_text_id: 7 });
        let note = gift_note(&gifted).expect("gift");
        assert_eq!(note.sender, "甲");
        assert_eq!(note.message_id, 7);
        assert!(recorded_sent(0).is_none());
        assert_eq!(recorded_sent(1_700_000_000).expect("clock").len(), 16);
        gifted.viewable = false;
        assert!(gift_note(&gifted).is_none());
    }

    #[test]
    fn fertilizer_and_packs_can_be_used_until_they_are_locked() {
        let mut fertilizer = item(80_001, 0, false);
        assert!(can_use(&fertilizer));
        fertilizer.locked = true;
        assert!(!can_use(&fertilizer));
        assert!(can_use(&item(1, ITEM_PACK, false)));
        assert!(!can_use(&item(6, ITEM_FRUIT, true)));
        assert!(!can_use(&item(1011, 0, false)));
    }

    #[test]
    fn batch_sell_keeps_only_the_selected_sellable_rows() {
        let mut fruit = item(6, ITEM_FRUIT, true);
        fruit.uid = 1;
        fruit.count = 4;
        let mut locked = item(7, ITEM_FRUIT, true);
        locked.uid = 2;
        locked.locked = true;
        let lines = bag_lines(&[fruit, locked], BagFilter::All);
        assert_eq!(sell_picks(&lines, &["6".to_owned(), "7".to_owned()]), vec![(6, 4, 1)]);
        assert!(sell_picks(&lines, &[]).is_empty());
        assert_eq!(toggle_key(&["6".to_owned()], "6"), Vec::<String>::new());
        assert_eq!(toggle_key(&[], "6"), vec!["6".to_owned()]);
        let mut seed = item(3, ITEM_SEED, false);
        seed.uid = 4;
        let locked_fruit = lines.into_iter().find(|line| line.uid == 2).expect("locked");
        let open_seed = bag_lines(&[seed], BagFilter::All).into_iter().next().expect("seed");
        let rows = vec![open_seed, locked_fruit];
        let lock = mode_picks(&rows, &["3".to_owned(), "7".to_owned()], PickMode::Lock);
        assert_eq!(lock.iter().map(|line| line.uid).collect::<Vec<_>>(), vec![4]);
        let unlock = mode_picks(&rows, &["3".to_owned(), "7".to_owned()], PickMode::Unlock);
        assert_eq!(unlock.iter().map(|line| line.uid).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn conditional_items_wait_for_the_activity_or_expiry() {
        let mut waiting = item(1026, 19, false);
        waiting.sell_status = "conditional".to_owned();
        waiting.sell_condition = Some("活动结束后:2026081800".to_owned());
        assert_eq!(sell_wait(&waiting), Some(SellWait::ActivityEnded));
        waiting.sell_condition = Some("道具过期后".to_owned());
        assert_eq!(sell_wait(&waiting), Some(SellWait::AfterExpire));
        waiting.sell_status = "available".to_owned();
        assert!(sell_wait(&waiting).is_none());
    }

    #[test]
    fn fruit_row_keeps_level_and_mutant_names() {
        let mut fruit = item(6, ITEM_FRUIT, true);
        fruit.level = 3;
        fruit.mutant_effects = vec!["冰冻".to_owned(), " ".to_owned(), "黄金".to_owned()];
        let named = bag_line_from(&fruit);
        assert_eq!(named.level, 3);
        assert_eq!(named.mutants, "冰冻 + 黄金");
        fruit.mutant_effects.clear();
        fruit.mutant_types = vec![0, 5];
        fruit.level = 0;
        let numbered = bag_line_from(&fruit);
        assert_eq!(numbered.level, 0);
        assert_eq!(numbered.mutants, "#5");
        fruit.mutant_types.clear();
        assert!(bag_line_from(&fruit).mutants.is_empty());
    }

    #[test]
    fn fruit_row_keeps_the_server_price() {
        let mut fruit = item(200, ITEM_FRUIT, true);
        fruit.price_id = 1005;
        fruit.price = 15;
        let lines = bag_lines(&[fruit], BagFilter::Fruit);
        assert_eq!((lines[0].price_id, lines[0].price), (1005, 15));
    }
}
