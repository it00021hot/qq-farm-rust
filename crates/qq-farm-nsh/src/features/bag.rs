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
const ITEM_MUTANT_FRUIT: i64 = 17;

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
            move || bag_body(account_id, filter, pending, failure, summary, bag, use_counts),
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
) -> impl IntoElement {
    view()
        .gap_3()
        .child(filters(filter))
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || bag.refetch()),
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
) -> View {
    let name = SharedString::from(line.name.clone());
    let count = SharedString::from(line.count.to_string());
    let kind = line.kind;
    let stack = line.count;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().font_weight(FontWeight::MEDIUM).flex_1())
        .child(text(count).text_sm())
        .child(badge(move || text_key(kind.label_key()), BadgeTone::Neutral))
        .child(when(
            {
                let locked = line.locked;
                move || locked
            },
            || badge(|| text_key("bag.locked"), BadgeTone::Neutral),
        ))
        .child(when(move || kind == BagKind::Tool && stack > 1, {
            let step_line = line.clone();
            move || use_stepper(step_line.clone(), use_counts, pending)
        }))
        .child(when(move || kind == BagKind::Tool, {
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
        }))
        .child(when(
            {
                let lockable = line.lockable;
                move || lockable
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
                move || sellable
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
        kind,
    }
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
        bag_lines, chosen_count, is_balance_item, is_dog_food, BagFilter, BagKind, ITEM_FRUIT,
        ITEM_SEED,
    };
    use qq_farm_core::services::warehouse::BagItemView;

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
}
