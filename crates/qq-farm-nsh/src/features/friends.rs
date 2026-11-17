use chrono::{DateTime, Datelike, FixedOffset, Timelike};
use nsh_core::prelude::*;
use qq_farm_app::dto::{FriendPlantSummary, FriendSummary};
use serde_json::Value;

use super::account_gate::{account_gate, gate_notice, AccountGate};
use super::accounts::AccountRow;
use super::confirm;
use super::outcome::{op_detail, outcome_line, OpDetail, Outcome};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, field, notice, BadgeTone, ButtonVariant};

#[derive(Clone, Copy)]
pub(crate) struct FriendPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FriendAction {
    Steal,
    Help,
    Water,
    Weed,
    Bug,
    Fertilize,
    Bad,
    Delete,
}

impl FriendAction {
    const ALL: [Self; 8] = [
        Self::Steal,
        Self::Help,
        Self::Water,
        Self::Weed,
        Self::Bug,
        Self::Fertilize,
        Self::Bad,
        Self::Delete,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::Steal => "steal",
            Self::Help => "help",
            Self::Water => "water",
            Self::Weed => "weed",
            Self::Bug => "bug",
            Self::Fertilize => "fertilize",
            Self::Bad => "bad",
            Self::Delete => "delete",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Steal => "friends.steal",
            Self::Help => "friends.help",
            Self::Water => "friends.water",
            Self::Weed => "friends.weed",
            Self::Bug => "friends.bug",
            Self::Fertilize => "friends.fertilize",
            Self::Bad => "friends.bad",
            Self::Delete => "friends.delete",
        }
    }

    const fn variant(self) -> ButtonVariant {
        match self {
            Self::Steal => ButtonVariant::Primary,
            Self::Bad | Self::Delete => ButtonVariant::Danger,
            _ => ButtonVariant::Secondary,
        }
    }
}

#[derive(Clone, Copy)]
struct BlockList {
    local: Signal<Option<Vec<i64>>>,
    local_account: Signal<String>,
    pending: Signal<bool>,
    generation: Signal<u64>,
    blocked: Resource<Vec<i64>, String>,
}

pub(crate) fn page(page: FriendPage) -> impl IntoElement {
    let generation = signal(0_u64);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let outcome = signal(None::<Outcome>);
    let block = BlockList {
        local: signal(None),
        local_account: signal(String::new()),
        pending: signal(false),
        generation: signal(0_u64),
        blocked: resource(
            move || ready_account(page),
            |account, _| async move { load_blocked(account).await },
        ),
    };
    let friend_query = signal(String::new());
    let friend_page = signal(0_usize);
    let query_seen = signal(String::new());
    effect(move || {
        let current = friend_query.get();
        if query_seen.get_untracked() != current {
            query_seen.set(current);
            friend_page.set(0);
        }
    });
    let open_lands = signal(None::<i64>);
    let lands_account = signal(String::new());
    effect(move || {
        let account = page.account_id.get().unwrap_or_default();
        if lands_account.get_untracked() != account {
            lands_account.set(account);
            open_lands.set(None);
        }
    });
    let friend_lands = resource(
        move || {
            let account = ready_account(page)?;
            open_lands.get().filter(|gid| *gid > 0).map(|gid| (account, gid))
        },
        |(account, gid), _| async move {
            let context = session::session().context.clone();
            session::call(async move {
                qq_farm_app::friend::friend_lands(&context, &account, gid).await
            })
            .await
        },
    );
    let visit_filter = signal(VisitFilter::All);
    let visits = resource(
        move || ready_account(page),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::friend::interact_records(&context, &id).await })
                .await
        },
    );
    let pick = super::interaction_items::interaction_pick(page.account_id, page.rows);
    pick.reset_when_scope_changes(open_lands);
    let friends = resource(
        move || ready_account(page).map(|id| (id, generation.get())),
        |(id, generation), _| async move {
            let context = session::session().context.clone();
            session::call(async move {
                qq_farm_app::friend::list_friends(&context, &id, generation > 0).await
            })
            .await
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("friends.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(super::accounts::failure_line(failure))
        .child(when(
            move || outcome.get().is_some(),
            move || notice(move || outcome.get().as_ref().map(outcome_line).unwrap_or_default()),
        ))
        .child(gate_notice(page.account_id, page.rows, "friends.needRunning"))
        .child(when(
            move || ready(page) && friends.loading() && friends.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(move || ready(page) && friends.error().is_some(), move || load_error(friends)))
        .child(when(
            move || ready(page) && friends.get().is_some(),
            move || {
                friend_body(
                    page,
                    generation,
                    pending,
                    failure,
                    outcome,
                    friends,
                    block,
                    open_lands,
                    friend_lands,
                    friend_query,
                    friend_page,
                    pick,
                )
            },
        ))
        .child(when(move || ready(page), move || visitors_section(visits, visit_filter)))
        .child(when(
            move || ready(page) && qq_account(page),
            move || super::known_friends::section(page.account_id, friends, generation),
        ))
}

fn ready(page: FriendPage) -> bool {
    account_gate(page.account_id, page.rows) == AccountGate::Ready
}

fn ready_account(page: FriendPage) -> Option<String> {
    ready(page).then(|| page.account_id.get()).flatten()
}

fn qq_account(page: FriendPage) -> bool {
    let Some(id) = page.account_id.get() else {
        return false;
    };
    page.rows.get().is_some_and(|rows| rows.iter().any(|row| row.id == id && row.platform == "qq"))
}

fn load_error(friends: Resource<Vec<FriendSummary>, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            friends.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || friends.refetch()),
        )
}

fn friend_body(
    page: FriendPage,
    generation: Signal<u64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    friends: Resource<Vec<FriendSummary>, String>,
    block: BlockList,
    open_lands: Signal<Option<i64>>,
    friend_lands: Resource<Value, String>,
    query: Signal<String>,
    friend_page: Signal<usize>,
    pick: super::interaction_items::InteractionPick,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || block.blocked.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        block.blocked.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || block.blocked.refetch()),
                    )
            },
        ))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    field(
                        move || query.get(),
                        move || text_key("friends.search").to_string(),
                        move |text| query.set(text.to_owned()),
                    )
                    .flex_1(),
                )
                .child(
                    button(move || text_key("friends.refresh"), ButtonVariant::Secondary)
                        .disabled(move || pending.get().is_some())
                        .on_press(move || {
                            generation.set(generation.get_untracked().saturating_add(1))
                        }),
                ),
        )
        .child(when(
            move || friends.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("friends.none")),
        ))
        .child(when(
            move || {
                let lines = friend_lines(friends);
                !lines.is_empty()
                    && friend_window(&lines, &query.get(), friend_page.get()).lines.is_empty()
            },
            move || notice(move || text_key("friends.noMatch")),
        ))
        .child(each(
            move || friend_window(&friend_lines(friends), &query.get(), friend_page.get()).lines,
            |friend: &FriendLine| friend.gid.to_string(),
            move |friend, _| {
                render_friend(
                    friend.get_untracked(),
                    page,
                    open_lands,
                    friend_lands,
                    pending,
                    failure,
                    outcome,
                    friends,
                    block,
                    pick,
                )
            },
        ))
        .child(when(
            move || {
                friend_window(&friend_lines(friends), &query.get(), friend_page.get()).pages > 1
            },
            move || friend_pager(friends, query, friend_page),
        ))
}

#[derive(Clone, PartialEq, Eq)]
struct FriendLine {
    gid: i64,
    name: String,
    level: i64,
    gold: i64,
    pet_state: String,
    pet_name: String,
    marks: Vec<PlantMark>,
}

fn friend_lines(friends: Resource<Vec<FriendSummary>, String>) -> Vec<FriendLine> {
    friends.get().map(|rows| rows.iter().map(friend_line).collect()).unwrap_or_default()
}

fn friend_line(friend: &FriendSummary) -> FriendLine {
    FriendLine {
        gid: friend.gid,
        name: friend_name(friend),
        level: friend.level,
        gold: friend.gold.max(0),
        pet_state: friend.pet_state.clone(),
        pet_name: friend.pet.as_ref().map(|pet| pet.name.trim().to_owned()).unwrap_or_default(),
        marks: plant_marks(friend.plant.as_ref()),
    }
}

const FRIEND_PAGE_SIZE: usize = 25;

struct FriendWindow {
    lines: Vec<FriendLine>,
    page: usize,
    pages: usize,
}

fn friend_window(friends: &[FriendLine], query: &str, page: usize) -> FriendWindow {
    let mut matched: Vec<FriendLine> =
        friends.iter().filter(|friend| friend_matches(friend, query)).cloned().collect();
    matched.sort_by(|left, right| right.level.cmp(&left.level).then(left.gid.cmp(&right.gid)));
    let pages = matched.len().div_ceil(FRIEND_PAGE_SIZE).max(1);
    let page = page.min(pages - 1);
    let start = page * FRIEND_PAGE_SIZE;
    FriendWindow {
        lines: matched.into_iter().skip(start).take(FRIEND_PAGE_SIZE).collect(),
        page,
        pages,
    }
}

fn friend_matches(friend: &FriendLine, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || friend.name.to_lowercase().contains(&query)
        || friend.gid.to_string().contains(&query)
}

fn shows_pet(state: &str) -> bool {
    matches!(state, "protect" | "other" | "unknown")
}

fn pet_label(state: &str, name: &str) -> SharedString {
    match state {
        "protect" if name.is_empty() => text_key("friends.petProtect"),
        "other" if name.is_empty() => text_key("friends.petNone"),
        "unknown" => text_key("friends.petUnknown"),
        "protect" | "other" => SharedString::from(name.to_owned()),
        _ => SharedString::from(""),
    }
}

fn friend_pager(
    friends: Resource<Vec<FriendSummary>, String>,
    query: Signal<String>,
    friend_page: Signal<usize>,
) -> View {
    let window = move || friend_window(&friend_lines(friends), &query.get(), friend_page.get());
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || text_key("friends.prev"), ButtonVariant::Secondary)
                .disabled(move || window().page == 0)
                .on_press(move || friend_page.set(window().page.saturating_sub(1))),
        )
        .child(
            text(move || {
                let window = window();
                SharedString::from(
                    text_key("friends.page")
                        .to_string()
                        .replace("{page}", &(window.page + 1).to_string())
                        .replace("{pages}", &window.pages.to_string()),
                )
            })
            .text_sm(),
        )
        .child(
            button(move || text_key("friends.next"), ButtonVariant::Secondary)
                .disabled(move || {
                    let window = window();
                    window.page + 1 >= window.pages
                })
                .on_press(move || {
                    let window = window();
                    if window.page + 1 < window.pages {
                        friend_page.set(window.page + 1);
                    }
                }),
        )
}

fn render_friend(
    friend: FriendLine,
    page: FriendPage,
    open_lands: Signal<Option<i64>>,
    friend_lands: Resource<Value, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    friends: Resource<Vec<FriendSummary>, String>,
    block: BlockList,
    pick: super::interaction_items::InteractionPick,
) -> View {
    let gid = friend.gid;
    let name = friend.name;
    let level = friend.level;
    let gold = friend.gold;
    let pet_state = friend.pet_state;
    let pet_name = friend.pet_name;
    let marks = friend.marks;
    view()
        .gap_2()
        .py_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(text(SharedString::from(name)).text_sm().font_weight(FontWeight::MEDIUM))
                .child(when(
                    move || is_blocked(&shown_ids(page, block), gid),
                    move || badge(move || text_key("friends.blocked"), BadgeTone::Neutral),
                ))
                .child(
                    text(move || {
                        SharedString::from(format!("{} {level}", text_key("friends.level")))
                    })
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground),
                )
                .child(when(
                    move || gold > 0,
                    move || {
                        text(move || {
                            SharedString::from(format!(
                                "{} {}",
                                text_key("friends.gold"),
                                friend_gold(gold, locale().as_str() == "en-US"),
                            ))
                        })
                        .text_sm()
                        .text_color(move || nsh_base::theme().muted_foreground)
                    },
                ))
                .child(when(
                    {
                        let pet_state = pet_state.clone();
                        move || shows_pet(&pet_state)
                    },
                    {
                        let pet_state = pet_state.clone();
                        let pet_name = pet_name.clone();
                        move || {
                            let pet_state = pet_state.clone();
                            let pet_name = pet_name.clone();
                            badge(move || pet_label(&pet_state, &pet_name), BadgeTone::Neutral)
                        }
                    },
                ))
                .child(each(
                    move || marks.clone(),
                    |mark: &PlantMark| mark.key.to_owned(),
                    |mark, _| {
                        let mark = mark.get_untracked();
                        badge(
                            move || {
                                SharedString::from(format!("{} {}", text_key(mark.key), mark.count))
                            },
                            BadgeTone::Neutral,
                        )
                    },
                )),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(each(
                    || FriendAction::ALL.to_vec(),
                    |action: &FriendAction| action.id().to_owned(),
                    move |action, _| {
                        action_button(
                            action.get_untracked(),
                            gid,
                            page,
                            pending,
                            failure,
                            outcome,
                            friends,
                        )
                    },
                ))
                .child(block_button(gid, page, failure, block))
                .child(
                    button(
                        move || {
                            text_key(if open_lands.get() == Some(gid) {
                                "friends.landsHide"
                            } else {
                                "friends.lands"
                            })
                        },
                        ButtonVariant::Quiet,
                    )
                    .selected(move || open_lands.get() == Some(gid))
                    .on_press(move || {
                        if open_lands.get() == Some(gid) {
                            open_lands.set(None);
                        } else {
                            open_lands.set(Some(gid));
                        }
                    }),
                ),
        )
        .child(when(
            move || open_lands.get() == Some(gid),
            move || friend_land_panel(friend_lands, page, gid, pending, failure, pick),
        ))
}

fn action_button(
    action: FriendAction,
    gid: i64,
    page: FriendPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    friends: Resource<Vec<FriendSummary>, String>,
) -> nsh_base::ButtonElement {
    button(
        move || {
            if pending.get().as_deref() == Some(pending_key(gid, action).as_str()) {
                text_key("friends.working")
            } else {
                text_key(action.label_key())
            }
        },
        action.variant(),
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || {
        press_friend(FriendCommand { action, gid, page, pending, failure, outcome, friends })
    })
}

struct FriendCommand {
    action: FriendAction,
    gid: i64,
    page: FriendPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    friends: Resource<Vec<FriendSummary>, String>,
}

fn pending_key(gid: i64, action: FriendAction) -> String {
    format!("{gid}:{}", action.id())
}

fn press_friend(command: FriendCommand) {
    if command.pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = command.page.account_id.get_untracked() else {
        return;
    };
    let key = pending_key(command.gid, command.action);
    command.pending.set(Some(key));
    command.failure.set(None);
    command.outcome.set(None);
    match command.action {
        FriendAction::Bad => confirm::confirm(
            "friends.confirmBad.title",
            "friends.confirmBad.body",
            "bad",
            "friends.bad",
            move |accepted| accept_friend(command, id, accepted),
        ),
        FriendAction::Delete => confirm::confirm(
            "friends.confirmDelete.title",
            "friends.confirmDelete.body",
            "delete",
            "friends.delete",
            move |accepted| accept_friend(command, id, accepted),
        ),
        _ => {
            spawn(async move { finish_friend(command, id).await });
        }
    }
}

fn accept_friend(command: FriendCommand, id: String, accepted: bool) {
    if !accepted {
        command.pending.set(None);
        return;
    }
    spawn(async move { finish_friend(command, id).await });
}

async fn finish_friend(command: FriendCommand, id: String) {
    match run_friend(command.action, id, command.gid).await {
        Ok(detail) => {
            command.outcome.set(Some(Outcome { label_key: command.action.label_key(), detail }))
        }
        Err(message) => command.failure.set(Some(message)),
    }
    command.pending.set(None);
    command.friends.refetch();
}

async fn run_friend(action: FriendAction, id: String, gid: i64) -> Result<OpDetail, String> {
    let context = session::session().context.clone();
    match action {
        FriendAction::Delete => {
            session::call(
                async move { qq_farm_app::friend::delete_friend(&context, &id, gid).await },
            )
            .await?;
            Ok(OpDetail::Done)
        }
        other => {
            let op = other.id();
            let value = session::call(async move {
                qq_farm_app::friend::friend_op(&context, &id, gid, op).await
            })
            .await?;
            Ok(op_detail(&value))
        }
    }
}

fn block_button(
    gid: i64,
    page: FriendPage,
    failure: Signal<Option<String>>,
    block: BlockList,
) -> nsh_base::ButtonElement {
    button(
        move || {
            text_key(if is_blocked(&shown_ids(page, block), gid) {
                "friends.unblock"
            } else {
                "friends.block"
            })
        },
        ButtonVariant::Secondary,
    )
    .selected(move || is_blocked(&shown_ids(page, block), gid))
    .disabled(move || block.pending.get() || gid <= 0)
    .on_press(move || ask_block(gid, page, failure, block))
}

fn ask_block(gid: i64, page: FriendPage, failure: Signal<Option<String>>, block: BlockList) {
    if gid <= 0 || block.pending.get_untracked() {
        return;
    }
    let blocked = is_blocked(&shown_ids(page, block), gid);
    let (title, body, action, label) = if blocked {
        (
            "friends.confirmUnblock.title",
            "friends.confirmUnblock.body",
            "unblock",
            "friends.unblock",
        )
    } else {
        ("friends.confirmBlock.title", "friends.confirmBlock.body", "block", "friends.block")
    };
    confirm::confirm(title, body, action, label, move |accepted| {
        if accepted {
            commit_block(gid, page, failure, block);
        }
    });
}

fn commit_block(gid: i64, page: FriendPage, failure: Signal<Option<String>>, block: BlockList) {
    if block.pending.get_untracked() {
        return;
    }
    let Some(account) = page.account_id.get_untracked().filter(|id| !id.is_empty()) else {
        return;
    };
    let ticket = block.generation.get_untracked().wrapping_add(1);
    block.generation.set(ticket);
    block.pending.set(true);
    failure.set(None);
    let saved_account = account.clone();
    spawn(async move {
        let saved = session::session()
            .runtime
            .spawn_blocking(move || qq_farm_app::friend::toggle_friend_blacklist(&account, gid))
            .await
            .map_err(|error| error.to_string());
        if block.generation.get_untracked() != ticket {
            block.pending.set(false);
            return;
        }
        block.pending.set(false);
        match saved {
            Ok(value) => {
                block.local_account.set(saved_account);
                block.local.set(Some(blocked_ids(&value)));
            }
            Err(message) => failure.set(Some(message)),
        }
    });
}

fn shown_ids(page: FriendPage, block: BlockList) -> Vec<i64> {
    let current = page.account_id.get().unwrap_or_default();
    if block.local_account.get() == current {
        if let Some(ids) = block.local.get() {
            return ids;
        }
    }
    block.blocked.get().unwrap_or_default()
}

async fn load_blocked(account: String) -> Result<Vec<i64>, String> {
    let value = session::session()
        .runtime
        .spawn_blocking(move || qq_farm_app::friend::friend_blacklist(&account))
        .await
        .map_err(|error| error.to_string())?;
    Ok(blocked_ids(&value))
}

fn blocked_ids(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .map(|rows| rows.iter().filter_map(|item| item.as_i64().filter(|id| *id > 0)).collect())
        .unwrap_or_default()
}

fn is_blocked(ids: &[i64], gid: i64) -> bool {
    gid > 0 && ids.contains(&gid)
}

#[cfg(test)]
fn next_blacklist(current: &[i64], gid: i64) -> Vec<i64> {
    if gid <= 0 {
        return current.to_vec();
    }
    if current.contains(&gid) {
        current.iter().copied().filter(|id| *id != gid).collect()
    } else {
        let mut next = current.to_vec();
        next.push(gid);
        next
    }
}

fn friend_land_panel(
    lands: Resource<Value, String>,
    page: FriendPage,
    gid: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    pick: super::interaction_items::InteractionPick,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(super::interaction_items::expanded_tools(
            page.account_id,
            gid,
            pending,
            failure,
            lands,
            pick,
        ))
        .child(when(
            move || lands.loading() && lands.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || lands.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        lands.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || lands.refetch()),
                    )
            },
        ))
        .child(when(
            move || lands.get().is_some_and(|value| friend_farm(&value).plots.is_empty()),
            move || notice(move || text_key("friends.noLands")),
        ))
        .child(when(
            move || lands.get().is_some_and(|value| friend_farm(&value).career.is_some()),
            move || {
                text(move || {
                    lands
                        .get()
                        .and_then(|value| friend_farm(&value).career)
                        .map(|career| {
                            let (harvest, steal, ratio) = career_numbers(
                                career.harvest,
                                career.steal,
                                locale().as_str() == "en-US",
                            );
                            SharedString::from(format!(
                                "{} {harvest} · {} {steal} · {} {ratio}",
                                text_key("friends.careerHarvest"),
                                text_key("friends.careerSteal"),
                                text_key("friends.careerRatio"),
                            ))
                        })
                        .unwrap_or_default()
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(each(
            move || lands.get().map(|value| friend_farm(&value).plots).unwrap_or_default(),
            |plot: &FriendPlot| plot.id.to_string(),
            move |plot, _| render_plot(plot.get_untracked(), pick, pending, failure),
        ))
}

fn render_plot(
    plot: FriendPlot,
    pick: super::interaction_items::InteractionPick,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
) -> View {
    let status = plot.status_key;
    let marks = plot.marks;
    let title = SharedString::from(plot.title);
    let soil = plot.soil;
    let season = SharedString::from(plot.season);
    let joint = SharedString::from(plot.joint);
    let mutants = SharedString::from(plot.mutants);
    let effects = SharedString::from(plot.effects);
    let fert_left = plot.fert_left;
    let resonance = SharedString::from(plot.resonance);
    let button_title = title.clone();
    let target = plot.target.clone();
    let button_target = target.clone();
    let press_target = target.clone();
    let label_target = target.clone();
    let id = plot.id;
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            text(move || text_key(soil))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(when(
            move || !super::interaction_items::choosing(pick),
            move || text(title.clone()).text_sm(),
        ))
        .child(fact_line("friends.mutant", mutants))
        .child(fact_line("friends.season", season))
        .child(fact_line("friends.joint", joint))
        .child(fact_line("friends.effect", effects))
        .child(fact_line(
            "friends.fertLeft",
            if fert_left > 0 {
                SharedString::from(fert_left.to_string())
            } else {
                SharedString::from("")
            },
        ))
        .child(fact_line(
            "friends.resonance",
            if resonance.is_empty() {
                SharedString::from("")
            } else {
                SharedString::from(format!("+{resonance}%"))
            },
        ))
        .child(when(
            move || super::interaction_items::choosing(pick),
            move || {
                let button_target = button_target.clone();
                let press_target = press_target.clone();
                let button_title = button_title.clone();
                button(move || button_title.clone(), ButtonVariant::Quiet)
                    .selected(move || super::interaction_items::land_selected(pick, id))
                    .disabled(move || {
                        pending.get().is_some()
                            || super::interaction_items::selection_label(pick, &button_target, id)
                                .is_some()
                    })
                    .on_press(move || {
                        super::interaction_items::press_land(pick, &press_target, id, failure)
                    })
            },
        ))
        .child(when(
            {
                let label_target = label_target.clone();
                move || super::interaction_items::selection_label(pick, &label_target, id).is_some()
            },
            move || {
                let label_target = label_target.clone();
                badge(
                    move || {
                        text_key(
                            super::interaction_items::selection_label(pick, &label_target, id)
                                .unwrap_or("interaction.unavailable"),
                        )
                    },
                    BadgeTone::Neutral,
                )
            },
        ))
        .child(when(
            move || status.is_some(),
            move || {
                badge(move || text_key(status.unwrap_or("friends.landGrowing")), BadgeTone::Neutral)
            },
        ))
        .child(each(
            move || marks.clone(),
            |mark: &&'static str| (*mark).to_owned(),
            move |mark, _| {
                let mark = mark.get_untracked();
                badge(move || text_key(mark), BadgeTone::Neutral)
            },
        ))
}

#[derive(Clone, PartialEq, Eq)]
struct FriendPlot {
    id: i64,
    title: String,
    soil: &'static str,
    mutants: String,
    season: String,
    joint: String,
    effects: String,
    fert_left: i64,
    resonance: String,
    status_key: Option<&'static str>,
    marks: Vec<&'static str>,
    target: super::interaction_items::LandTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CareerLine {
    harvest: i64,
    steal: i64,
}

struct FriendFarm {
    career: Option<CareerLine>,
    plots: Vec<FriendPlot>,
}

fn friend_farm(value: &Value) -> FriendFarm {
    let plots = value
        .get("lands")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(friend_plot).collect())
        .unwrap_or_default();
    FriendFarm { career: career_line(value.get("career")), plots }
}

fn friend_plot(value: &Value) -> Option<FriendPlot> {
    if value.get("occupiedByMaster").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("plantName").and_then(Value::as_str).unwrap_or("").trim();
    let phase = value.get("phaseName").and_then(Value::as_str).unwrap_or("").trim();
    let remain = remain_label(value.get("matureInSec").and_then(Value::as_i64).unwrap_or(0));
    let mut title = format!("#{id}");
    if !name.is_empty() {
        title.push(' ');
        title.push_str(name);
    }
    if !phase.is_empty() {
        title.push_str(" · ");
        title.push_str(phase);
    }
    if !remain.is_empty() {
        title.push_str(" · ");
        title.push_str(&remain);
    }
    let mut marks = Vec::new();
    if value.get("needWater").and_then(Value::as_bool).unwrap_or(false) {
        marks.push("friends.dry");
    }
    if value.get("needWeed").and_then(Value::as_bool).unwrap_or(false) {
        marks.push("friends.weeds");
    }
    if value.get("needBug").and_then(Value::as_bool).unwrap_or(false) {
        marks.push("friends.bugs");
    }
    Some(FriendPlot {
        id,
        title,
        soil: soil_key(value.get("level").and_then(Value::as_i64).unwrap_or(0)),
        mutants: plot_mutants(value),
        season: plot_season(value),
        joint: plot_joint(value),
        effects: plot_effects(value),
        fert_left: plot_fert_left(value),
        resonance: plot_resonance(value),
        status_key: status_key(value.get("status").and_then(Value::as_str).unwrap_or("")),
        marks,
        target: super::interaction_items::land_target(value),
    })
}

fn fact_line(label_key: &'static str, detail: SharedString) -> impl IntoElement {
    let gate = detail.clone();
    when(
        move || !gate.is_empty(),
        move || {
            let shown = detail.clone();
            text(move || SharedString::from(format!("{} · {shown}", text_key(label_key))))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
        },
    )
}

fn plot_planted(value: &Value) -> bool {
    let status = value.get("status").and_then(Value::as_str).unwrap_or("");
    let planted = !value.get("plantName").and_then(Value::as_str).unwrap_or("").trim().is_empty();
    planted && !matches!(status, "empty" | "locked")
}

fn soil_key(level: i64) -> &'static str {
    match level {
        2 => "settings.landRed",
        3 => "settings.landBlack",
        4 => "settings.landGold",
        5 => "settings.landPurple",
        6 => "settings.landBlue",
        _ => "settings.landNormal",
    }
}

fn plot_season(value: &Value) -> String {
    if !plot_planted(value) {
        return String::new();
    }
    let Some(total) = value.get("totalSeason").and_then(Value::as_i64).filter(|total| *total > 1)
    else {
        return String::new();
    };
    let current = value.get("currentSeason").and_then(Value::as_i64).unwrap_or(1).clamp(1, total);
    format!("{current}/{total}")
}

fn plot_joint(value: &Value) -> String {
    if !plot_planted(value) {
        return String::new();
    }
    let size = value.get("plantSize").and_then(Value::as_i64).unwrap_or(1).max(1);
    if size > 1 {
        format!("{size}×{size}")
    } else {
        String::new()
    }
}

fn plot_mutants(value: &Value) -> String {
    if !plot_planted(value) {
        return String::new();
    }
    let effects = value.get("mutantEffects").and_then(Value::as_array);
    if let Some(effects) = effects.filter(|rows| !rows.is_empty()) {
        return effects
            .iter()
            .filter_map(|effect| effect.get("name").and_then(Value::as_str))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(" + ");
    }
    value
        .get("mutantConfigIds")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_i64)
                .filter(|id| *id > 0)
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(" + ")
        })
        .unwrap_or_default()
}

fn plot_effects(value: &Value) -> String {
    if !plot_planted(value) {
        return String::new();
    }
    let Some(effects) = value.get("interactionEffects").and_then(Value::as_array) else {
        return String::new();
    };
    let mut seen = Vec::new();
    let mut names = Vec::new();
    for effect in effects {
        if !effect.get("confirmed").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let id = effect
            .get("itemId")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
            .map(|id| id.to_string())
            .or_else(|| {
                effect
                    .get("itemId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty() && *text != "0")
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_default();
        if id.is_empty() || seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        let name = effect.get("itemName").and_then(Value::as_str).unwrap_or("").trim();
        names.push(if name.is_empty() { format!("#{id}") } else { name.to_owned() });
    }
    names.join(" · ")
}

fn plot_fert_left(value: &Value) -> i64 {
    if plot_planted(value) && value.get("status").and_then(Value::as_str) == Some("growing") {
        value.get("leftInorcFertTimes").and_then(Value::as_i64).unwrap_or(0).max(0)
    } else {
        0
    }
}

fn plot_resonance(value: &Value) -> String {
    if !plot_planted(value) {
        return String::new();
    }
    let bonus = value.get("purpleCrystalResonanceExpBonus").and_then(Value::as_i64).unwrap_or(0);
    if bonus <= 0 {
        return String::new();
    }
    let whole = bonus / 100;
    let fraction = bonus % 100;
    if fraction == 0 {
        whole.to_string()
    } else if fraction % 10 == 0 {
        format!("{whole}.{}", fraction / 10)
    } else {
        format!("{whole}.{fraction:02}")
    }
}

fn status_key(status: &str) -> Option<&'static str> {
    match status {
        "stealable" => Some("friends.landStealable"),
        "harvested" => Some("friends.landHarvested"),
        "growing" => Some("friends.landGrowing"),
        "empty" => Some("friends.landEmpty"),
        "dead" => Some("friends.landDead"),
        "locked" => Some("friends.landLocked"),
        _ => None,
    }
}

fn remain_label(seconds: i64) -> String {
    if seconds <= 0 {
        return String::new();
    }
    let minutes = seconds / 60;
    let rest = seconds % 60;
    if minutes == 0 {
        format!("{rest}s")
    } else if rest == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {rest}s")
    }
}

fn friend_gold(gold: i64, english: bool) -> String {
    let gold = gold.max(0);
    if english {
        if gold >= 1_000_000_000 {
            return compact(gold as f64 / 1_000_000_000.0, 2, "B");
        }
        if gold >= 1_000_000 {
            return compact(gold as f64 / 1_000_000.0, 2, "M");
        }
        if gold >= 1_000 {
            return compact(gold as f64 / 1_000.0, 1, "K");
        }
    } else if gold >= 100_000_000 {
        return compact(gold as f64 / 100_000_000.0, 2, "亿");
    } else if gold >= 10_000 {
        return compact(gold as f64 / 10_000.0, 1, "万");
    }
    group_digits(gold)
}

fn compact(value: f64, digits: usize, suffix: &str) -> String {
    format!("{}{suffix}", trim_zeros(&format!("{value:.digits$}")))
}

fn trim_zeros(value: &str) -> String {
    let trimmed = value.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn group_digits(value: i64) -> String {
    let text = value.to_string();
    let mut grouped = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

fn career_numbers(harvest: i64, steal: i64, english: bool) -> (String, String, String) {
    (
        super::farm::career_count(harvest, english),
        super::farm::career_count(steal, english),
        super::farm::steal_ratio(harvest, steal),
    )
}

fn career_line(value: Option<&Value>) -> Option<CareerLine> {
    let value = value?.as_object()?;
    Some(CareerLine {
        harvest: value.get("harvest").and_then(Value::as_i64).unwrap_or(0),
        steal: value.get("steal").and_then(Value::as_i64).unwrap_or(0),
    })
}

fn friend_name(friend: &FriendSummary) -> String {
    let name = friend.name.trim();
    if name.is_empty() {
        format!("GID:{}", friend.gid)
    } else {
        name.to_owned()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PlantMark {
    key: &'static str,
    count: i64,
}

fn plant_marks(plant: Option<&FriendPlantSummary>) -> Vec<PlantMark> {
    let Some(plant) = plant else {
        return Vec::new();
    };
    let mut marks = Vec::new();
    if plant.steal_num > 0 {
        marks.push(PlantMark { key: "friends.stealable", count: plant.steal_num });
    }
    if plant.dry_num > 0 {
        marks.push(PlantMark { key: "friends.dry", count: plant.dry_num });
    }
    if plant.weed_num > 0 {
        marks.push(PlantMark { key: "friends.weeds", count: plant.weed_num });
    }
    if plant.insect_num > 0 {
        marks.push(PlantMark { key: "friends.bugs", count: plant.insect_num });
    }
    marks
}

const VISIT_LIMIT: usize = 50;
const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitFilter {
    All,
    Steal,
    Help,
    Bad,
}

impl VisitFilter {
    const ALL: [Self; 4] = [Self::All, Self::Steal, Self::Help, Self::Bad];

    const fn label_key(self) -> &'static str {
        match self {
            Self::All => "friends.visitAll",
            Self::Steal => "friends.visitSteal",
            Self::Help => "friends.visitHelp",
            Self::Bad => "friends.visitBad",
        }
    }

    const fn action_type(self) -> Option<i64> {
        match self {
            Self::All => None,
            Self::Steal => Some(1),
            Self::Help => Some(2),
            Self::Bad => Some(3),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VisitWhen {
    Missing,
    JustNow,
    Minutes(i64),
    Today { hour: u32, minute: u32 },
    ThisYear { month: u32, day: u32, hour: u32, minute: u32 },
    Date { year: i32, month: u32, day: u32, hour: u32, minute: u32 },
}

#[derive(Clone, PartialEq, Eq)]
struct VisitLine {
    key: String,
    name: String,
    meta: String,
    action: String,
    detail: String,
    when: VisitWhen,
}

struct VisitPage {
    lines: Vec<VisitLine>,
    hidden: usize,
}

fn visitors_section(
    visits: Resource<Value, String>,
    filter: Signal<VisitFilter>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            text(move || text_key("friends.visitors")).text_sm().font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(each(
                    || VisitFilter::ALL.to_vec(),
                    |item: &VisitFilter| item.label_key().to_owned(),
                    move |item, _| {
                        let item = item.get_untracked();
                        button(move || text_key(item.label_key()), ButtonVariant::Quiet)
                            .selected(move || filter.get() == item)
                            .on_press(move || filter.set(item))
                    },
                ))
                .child(
                    button(move || text_key("friends.refresh"), ButtonVariant::Secondary)
                        .on_press(move || visits.refetch()),
                ),
        )
        .child(when(
            move || visits.loading() && visits.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || visits.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        visits.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || visits.refetch()),
                    )
            },
        ))
        .child(when(
            move || {
                visits.get().is_some_and(|value| {
                    shown_visits(&value, filter.get(), local_now()).lines.is_empty()
                })
            },
            move || notice(move || text_key("friends.noVisitors")),
        ))
        .child(each(
            move || {
                shown_visits(&visits.get().unwrap_or(Value::Null), filter.get(), local_now()).lines
            },
            |visit: &VisitLine| visit.key.clone(),
            move |visit, _| render_visit(visit.get_untracked()),
        ))
        .child(when(
            move || {
                visits
                    .get()
                    .is_some_and(|value| shown_visits(&value, filter.get(), local_now()).hidden > 0)
            },
            move || notice(move || text_key("friends.visitMore")),
        ))
}

fn render_visit(visit: VisitLine) -> View {
    let when = visit.when;
    let action = visit.action.clone();
    let detail = visit.detail.clone();
    view()
        .gap_1()
        .py_2()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(text(visit.name).text_sm().font_weight(FontWeight::MEDIUM))
                .child(badge(
                    move || {
                        if action.is_empty() {
                            text_key("friends.visitOther")
                        } else {
                            SharedString::from(action.clone())
                        }
                    },
                    BadgeTone::Neutral,
                ))
                .child(
                    text(visit.meta)
                        .text_sm()
                        .text_color(move || nsh_base::theme().muted_foreground),
                ),
        )
        .child(
            view()
                .flex_row()
                .justify_between()
                .gap_3()
                .child(
                    text(move || {
                        if detail.is_empty() {
                            text_key("friends.visitOther")
                        } else {
                            SharedString::from(detail.clone())
                        }
                    })
                    .text_sm()
                    .flex_1(),
                )
                .child(
                    text(move || time_label(when))
                        .text_sm()
                        .text_color(move || nsh_base::theme().muted_foreground),
                ),
        )
}

fn local_now() -> DateTime<FixedOffset> {
    chrono::Local::now().fixed_offset()
}

fn shown_visits(value: &Value, filter: VisitFilter, now: DateTime<FixedOffset>) -> VisitPage {
    let matched: Vec<&Value> =
        value.as_array().into_iter().flatten().filter(|row| visit_matches(row, filter)).collect();
    let hidden = matched.len().saturating_sub(VISIT_LIMIT);
    let lines = matched
        .into_iter()
        .take(VISIT_LIMIT)
        .enumerate()
        .map(|(index, row)| visit_line(row, index, now))
        .collect();
    VisitPage { lines, hidden }
}

fn visit_matches(row: &Value, filter: VisitFilter) -> bool {
    filter.action_type().is_none_or(|action| json_i64(row, "actionType") == action)
}

fn visit_line(row: &Value, index: usize, now: DateTime<FixedOffset>) -> VisitLine {
    let gid = json_i64(row, "visitorGid");
    let level = json_i64(row, "level");
    let nick = json_text(row, "nick");
    let action = json_text(row, "actionLabel");
    let detail = json_text(row, "actionDetail");
    let key = json_text(row, "key");
    let server_ms = json_i64(row, "serverTimeMs");
    let server_ms =
        if server_ms > 0 { server_ms } else { json_i64(row, "serverTimeSec").saturating_mul(1000) };
    VisitLine {
        key: if key.is_empty() { format!("{gid}-{server_ms}-{index}") } else { key },
        name: if nick.is_empty() { format!("GID:{gid}") } else { nick },
        meta: visit_meta(level, gid),
        action,
        detail: if detail.is_empty() { json_text(row, "actionLabel") } else { detail },
        when: visit_when(server_ms, now),
    }
}

fn visit_meta(level: i64, gid: i64) -> String {
    match (level > 0, gid > 0) {
        (true, true) => format!("Lv.{level} · GID {gid}"),
        (true, false) => format!("Lv.{level}"),
        (false, true) => format!("GID {gid}"),
        (false, false) => String::new(),
    }
}

fn visit_when(server_ms: i64, now: DateTime<FixedOffset>) -> VisitWhen {
    if server_ms <= 0 {
        return VisitWhen::Missing;
    }
    let Some(instant) = DateTime::from_timestamp_millis(server_ms) else {
        return VisitWhen::Missing;
    };
    let at = instant.with_timezone(&now.timezone());
    let diff = now.timestamp_millis().saturating_sub(server_ms);
    if (0..MINUTE_MS).contains(&diff) {
        return VisitWhen::JustNow;
    }
    if (MINUTE_MS..HOUR_MS).contains(&diff) {
        return VisitWhen::Minutes(diff / MINUTE_MS);
    }
    let hour = at.hour();
    let minute = at.minute();
    if at.date_naive() == now.date_naive() {
        return VisitWhen::Today { hour, minute };
    }
    if at.year() == now.year() {
        return VisitWhen::ThisYear { month: at.month(), day: at.day(), hour, minute };
    }
    VisitWhen::Date { year: at.year(), month: at.month(), day: at.day(), hour, minute }
}

fn time_label(when: VisitWhen) -> SharedString {
    match when {
        VisitWhen::Missing => "--".into(),
        VisitWhen::JustNow => text_key("friends.justNow"),
        VisitWhen::Minutes(count) => format!("{count} {}", text_key("friends.minutesAgo")).into(),
        VisitWhen::Today { hour, minute } => {
            format!("{} {hour:02}:{minute:02}", text_key("friends.today")).into()
        }
        VisitWhen::ThisYear { month, day, hour, minute } => {
            format!("{month}-{day} {hour:02}:{minute:02}").into()
        }
        VisitWhen::Date { year, month, day, hour, minute } => {
            format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02}").into()
        }
    }
}

fn json_i64(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn json_text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use qq_farm_app::dto::{FriendPlantSummary, FriendSummary};
    use serde_json::json;

    use super::{
        blocked_ids, career_numbers, friend_farm, friend_gold, friend_name, is_blocked,
        next_blacklist, plant_marks, shown_visits, visit_when, VisitFilter, VisitWhen,
    };

    #[test]
    fn friend_lands_skip_slave_plots_and_keep_career() {
        let farm = friend_farm(&json!({
            "lands": [
                {
                    "id": 1,
                    "plantName": "萝卜",
                    "phaseName": "成熟",
                    "status": "stealable",
                    "needWater": true,
                    "occupiedByMaster": false
                },
                {
                    "id": 2,
                    "plantName": "白菜",
                    "status": "growing",
                    "matureInSec": 90,
                    "occupiedByMaster": true
                },
                { "id": 3, "status": "empty" },
                { "id": 0, "status": "dead" }
            ],
            "career": { "harvest": 10, "steal": 2 }
        }));
        assert_eq!(farm.plots.len(), 2);
        assert_eq!(farm.plots[0].title, "#1 萝卜 · 成熟");
        assert_eq!(farm.plots[0].status_key, Some("friends.landStealable"));
        assert_eq!(farm.plots[0].marks, vec!["friends.dry"]);
        assert_eq!(farm.plots[1].title, "#3");
        assert_eq!(farm.plots[1].status_key, Some("friends.landEmpty"));
        assert_eq!(farm.career, Some(super::CareerLine { harvest: 10, steal: 2 }));
        assert_eq!(
            career_numbers(12_340, 2_000, false),
            ("1.23万".to_owned(), "2000".to_owned(), "16.2%".to_owned())
        );
        assert_eq!(friend_gold(12_340, false), "1.2万");
        assert_eq!(friend_gold(123_456_789, false), "1.23亿");
        assert_eq!(friend_gold(9_999, false), "9,999");
        assert_eq!(friend_gold(12_340, true), "12.3K");
        assert_eq!(friend_gold(0, false), "0");
        assert!(friend_farm(&json!({})).plots.is_empty());
        let mutated = friend_farm(&json!({
            "lands": [
                {
                    "id": 4,
                    "plantName": "萝卜",
                    "status": "growing",
                    "level": 2,
                    "currentSeason": 2,
                    "totalSeason": 3,
                    "plantSize": 2,
                    "mutantEffects": [{"name": "冰冻"}, {"name": ""}, {"name": "黄金"}],
                    "interactionEffects": [
                        {"itemId": 11, "itemName": "青蛙瓶", "confirmed": true},
                        {"itemId": 11, "itemName": "青蛙瓶", "confirmed": true}
                    ],
                    "leftInorcFertTimes": 2,
                    "purpleCrystalResonanceExpBonus": 250
                },
                {
                    "id": 5,
                    "plantName": "白菜",
                    "status": "growing",
                    "mutantConfigIds": [7]
                },
                {
                    "id": 6,
                    "status": "empty",
                    "mutantEffects": [{"name": "冰冻"}]
                }
            ]
        }));
        assert_eq!(mutated.plots[0].mutants, "冰冻 + 黄金");
        assert_eq!(mutated.plots[0].soil, "settings.landRed");
        assert_eq!(mutated.plots[0].season, "2/3");
        assert_eq!(mutated.plots[0].joint, "2×2");
        assert_eq!(mutated.plots[0].effects, "青蛙瓶");
        assert_eq!(mutated.plots[0].fert_left, 2);
        assert_eq!(mutated.plots[0].resonance, "2.5");
        assert_eq!(mutated.plots[1].fert_left, 0);
        assert_eq!(mutated.plots[1].mutants, "#7");
        assert!(mutated.plots[2].mutants.is_empty());
        assert_eq!(super::remain_label(90), "1m 30s");
        assert!(super::remain_label(0).is_empty());
    }

    #[test]
    fn friends_sort_by_level_search_and_page() {
        let lines: Vec<super::FriendLine> = (1..=26)
            .map(|gid| super::FriendLine {
                gid,
                name: if gid == 3 { "萝卜".to_owned() } else { format!("好友{gid}") },
                level: gid,
                gold: 0,
                pet_state: String::new(),
                pet_name: String::new(),
                marks: Vec::new(),
            })
            .collect();
        let first = super::friend_window(&lines, "", 0);
        assert_eq!(first.pages, 2);
        assert_eq!(first.lines.len(), 25);
        assert_eq!(first.lines[0].gid, 26);
        assert_eq!(first.lines[24].gid, 2);
        let second = super::friend_window(&lines, "", 5);
        assert_eq!(second.page, 1);
        assert_eq!(second.lines[0].gid, 1);
        let found = super::friend_window(&lines, "萝卜", 0);
        assert_eq!(found.lines.len(), 1);
        assert_eq!(found.lines[0].gid, 3);
        let by_gid = super::friend_window(&lines, "26", 0);
        assert_eq!(by_gid.lines[0].gid, 26);
        assert!(super::friend_window(&lines, "没有", 0).lines.is_empty());
        assert!(!super::shows_pet(""));
        assert!(super::shows_pet("protect"));
        assert!(super::shows_pet("unknown"));
        let protect = super::friend_line(&FriendSummary {
            gid: 8,
            name: "甲".to_owned(),
            avatar_url: String::new(),
            level: 4,
            gold: 0,
            plant: None,
            pet_state: "protect".to_owned(),
            pet: serde_json::from_value(json!({ "id": 1, "name": " 大黄 ", "image": "" })).ok(),
        });
        assert_eq!(protect.pet_name, "大黄");
        let none = super::friend_line(&FriendSummary {
            gid: 9,
            name: "乙".to_owned(),
            avatar_url: String::new(),
            level: 1,
            gold: 0,
            plant: None,
            pet_state: "other".to_owned(),
            pet: None,
        });
        assert!(none.pet_name.is_empty());
    }

    #[test]
    fn blank_name_uses_gid() {
        let friend = FriendSummary {
            gid: 9,
            name: "  ".to_owned(),
            avatar_url: String::new(),
            level: 1,
            gold: 0,
            plant: None,
            pet_state: String::new(),
            pet: None,
        };
        assert_eq!(friend_name(&friend), "GID:9");
    }

    #[test]
    fn plant_marks_keep_positive_counts() {
        let plant = FriendPlantSummary { steal_num: 2, dry_num: 0, weed_num: 1, insect_num: 0 };
        let marks = plant_marks(Some(&plant));
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].key, "friends.stealable");
        assert_eq!(marks[0].count, 2);
        assert_eq!(marks[1].key, "friends.weeds");
    }

    #[test]
    fn blacklist_toggle_adds_and_removes_a_friend() {
        assert_eq!(blocked_ids(&serde_json::json!([4, 0, "9"])), vec![4]);
        assert!(is_blocked(&[4], 4));
        assert!(!is_blocked(&[4], 0));
        assert_eq!(next_blacklist(&[4, 8], 4), vec![8]);
        assert_eq!(next_blacklist(&[4], 8), vec![4, 8]);
        assert_eq!(next_blacklist(&[4], 0), vec![4]);
    }

    fn clock() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::FixedOffset::east_opt(8 * 3600)
            .expect("offset")
            .with_ymd_and_hms(2026, 11, 18, 12, 30, 0)
            .unwrap()
    }

    #[test]
    fn visitors_filter_by_action_and_keep_the_latest_fifty() {
        let mut rows = Vec::new();
        for index in 0..51 {
            rows.push(json!({
                "key": format!("steal-{index}"),
                "actionType": 1,
                "nick": "甲",
                "visitorGid": 7,
                "actionLabel": "偷取作物",
                "actionDetail": "偷了萝卜",
                "serverTimeMs": 1
            }));
        }
        rows.push(json!({
            "key": "help-1",
            "actionType": 2,
            "nick": " ",
            "visitorGid": 8,
            "level": 3,
            "actionLabel": "帮忙",
            "serverTimeSec": 20
        }));
        let value = serde_json::Value::Array(rows);
        let now = clock();
        let steal = shown_visits(&value, VisitFilter::Steal, now);
        assert_eq!(steal.lines.len(), 50);
        assert_eq!(steal.hidden, 1);
        let help = shown_visits(&value, VisitFilter::Help, now);
        assert_eq!(help.lines.len(), 1);
        assert_eq!(help.hidden, 0);
        assert_eq!(help.lines[0].name, "GID:8");
        assert_eq!(help.lines[0].meta, "Lv.3 · GID 8");
        assert_eq!(help.lines[0].detail, "帮忙");
        assert!(shown_visits(&json!([]), VisitFilter::All, now).lines.is_empty());
        assert!(shown_visits(&json!({}), VisitFilter::All, now).lines.is_empty());
    }

    #[test]
    fn visit_time_follows_the_recent_window_then_the_calendar() {
        let now = clock();
        let now_ms = now.timestamp_millis();
        let zone = chrono::FixedOffset::east_opt(8 * 3600).expect("offset");
        assert_eq!(visit_when(0, now), VisitWhen::Missing);
        assert_eq!(visit_when(now_ms - 59_999, now), VisitWhen::JustNow);
        assert_eq!(visit_when(now_ms - 60_000, now), VisitWhen::Minutes(1));
        assert_eq!(visit_when(now_ms - 5 * 60_000, now), VisitWhen::Minutes(5));
        assert_eq!(visit_when(now_ms - 3_600_000, now), VisitWhen::Today { hour: 11, minute: 30 });
        let morning = zone.with_ymd_and_hms(2026, 11, 18, 9, 5, 0).unwrap();
        assert_eq!(
            visit_when(morning.timestamp_millis(), now),
            VisitWhen::Today { hour: 9, minute: 5 }
        );
        let january = zone.with_ymd_and_hms(2026, 1, 2, 8, 0, 0).unwrap();
        assert_eq!(
            visit_when(january.timestamp_millis(), now),
            VisitWhen::ThisYear { month: 1, day: 2, hour: 8, minute: 0 }
        );
        let last_year = zone.with_ymd_and_hms(2025, 12, 31, 23, 15, 0).unwrap();
        assert_eq!(
            visit_when(last_year.timestamp_millis(), now),
            VisitWhen::Date { year: 2025, month: 12, day: 31, hour: 23, minute: 15 }
        );
    }
}
