use nsh_core::prelude::*;
use serde_json::Value;

use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

#[derive(Clone, Copy)]
pub(crate) struct AnalyticsPage {
    pub account_id: Signal<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RankRow {
    id: i64,
    seed_id: i64,
    name: String,
    exp_per_hour: String,
    gold_per_hour: String,
    grow: String,
}

pub(crate) fn page(page: AnalyticsPage) -> impl IntoElement {
    let query = signal(String::new());
    let local = signal(None::<Vec<i64>>);
    let local_account = signal(String::new());
    let pending = signal(false);
    let failure = signal(None::<String>);
    let generation = signal(0_u64);
    let rankings = resource(
        || Some(()),
        |(), _| async {
            let value = session::session()
                .runtime
                .spawn_blocking(|| qq_farm_app::farm::analytics(None))
                .await
                .map_err(|error| error.to_string())?;
            Ok(rank_rows(&value))
        },
    );
    let blacklist = resource(
        move || selected_account(page.account_id),
        |account, _| async move { load_blacklist(account).await },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(
            text(move || text_key("analytics.title")).text_lg().font_weight(FontWeight::SEMIBOLD),
        )
        .child(blacklist_section(
            page,
            query,
            local,
            local_account,
            pending,
            failure,
            generation,
            rankings,
            blacklist,
        ))
        .child(when(
            move || rankings.loading() && rankings.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(move || rankings.error().is_some(), move || load_error(rankings)))
        .child(when(
            move || rankings.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("analytics.empty")),
        ))
        .child(each(
            move || rankings.get().unwrap_or_default(),
            |row: &RankRow| row.id.to_string(),
            move |row, _| rank_line(row.get_untracked()),
        ))
}

fn load_error(rankings: Resource<Vec<RankRow>, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            rankings.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || rankings.refetch()),
        )
}

fn blacklist_section(
    page: AnalyticsPage,
    query: Signal<String>,
    local: Signal<Option<Vec<i64>>>,
    local_account: Signal<String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    generation: Signal<u64>,
    rankings: Resource<Vec<RankRow>, String>,
    blacklist: Resource<Vec<i64>, String>,
) -> View {
    view()
        .gap_2()
        .child(text(move || text_key("analytics.blacklist")).font_weight(FontWeight::SEMIBOLD))
        .child(text(move || text_key("analytics.blacklistHint")).text_sm())
        .child(when(
            move || selected_account(page.account_id).is_none(),
            move || notice(move || text_key("analytics.needAccount")),
        ))
        .child(when(
            move || selected_account(page.account_id).is_some(),
            move || {
                blacklist_editor(
                    page,
                    query,
                    local,
                    local_account,
                    pending,
                    failure,
                    generation,
                    rankings,
                    blacklist,
                )
            },
        ))
}

fn blacklist_editor(
    page: AnalyticsPage,
    query: Signal<String>,
    local: Signal<Option<Vec<i64>>>,
    local_account: Signal<String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    generation: Signal<u64>,
    rankings: Resource<Vec<RankRow>, String>,
    blacklist: Resource<Vec<i64>, String>,
) -> View {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
        .child(when(
            move || blacklist.loading() && blacklist.get().is_none() && local.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || blacklist.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        blacklist.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || blacklist.refetch()),
                    )
            },
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    button(move || text_key("analytics.addAll"), ButtonVariant::Secondary)
                        .disabled(move || {
                            pending.get() || rankings.get().is_none_or(|rows| rows.is_empty())
                        })
                        .on_press(move || {
                            let next = add_ranking_seeds(
                                &shown_ids(page.account_id, blacklist, local_account, local),
                                &rankings.get().unwrap_or_default(),
                            );
                            commit(page, next, local, local_account, pending, failure, generation);
                        }),
                )
                .child(
                    button(move || text_key("analytics.clear"), ButtonVariant::Secondary)
                        .disabled(move || {
                            pending.get()
                                || shown_ids(page.account_id, blacklist, local_account, local)
                                    .is_empty()
                        })
                        .on_press(move || {
                            commit(
                                page,
                                Vec::new(),
                                local,
                                local_account,
                                pending,
                                failure,
                                generation,
                            );
                        }),
                ),
        )
        .child(field_search(query))
        .child(each(
            move || {
                crop_matches(
                    &rankings.get().unwrap_or_default(),
                    &query.get(),
                    &shown_ids(page.account_id, blacklist, local_account, local),
                )
            },
            |row: &RankRow| row.seed_id.to_string(),
            move |row, _| {
                let row = row.get_untracked();
                let seed_id = row.seed_id;
                let label = SharedString::from(format!("{} #{seed_id}", row.name));
                button(label, ButtonVariant::Quiet).on_press(move || {
                    let next = toggle_id(
                        &shown_ids(page.account_id, blacklist, local_account, local),
                        seed_id,
                        true,
                    );
                    commit(page, next, local, local_account, pending, failure, generation);
                    query.set(String::new());
                })
            },
        ))
        .child(when(
            move || shown_ids(page.account_id, blacklist, local_account, local).is_empty(),
            move || notice(move || text_key("analytics.blacklistEmpty")),
        ))
        .child(each(
            move || shown_ids(page.account_id, blacklist, local_account, local),
            |seed_id: &i64| seed_id.to_string(),
            move |seed_id, _| {
                let seed_id = seed_id.get_untracked();
                blocked_row(seed_id, rankings, pending, move || {
                    let next = toggle_id(
                        &shown_ids(page.account_id, blacklist, local_account, local),
                        seed_id,
                        false,
                    );
                    commit(page, next, local, local_account, pending, failure, generation);
                })
            },
        ))
}

fn field_search(query: Signal<String>) -> impl IntoElement {
    crate::ui::field(
        move || query.get(),
        move || text_key("analytics.search").to_string(),
        move |text: &str| query.set(text.to_owned()),
    )
}

fn blocked_row(
    seed_id: i64,
    rankings: Resource<Vec<RankRow>, String>,
    pending: Signal<bool>,
    remove: impl Fn() + 'static,
) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(
            text(move || {
                SharedString::from(crop_name(&rankings.get().unwrap_or_default(), seed_id))
            })
            .text_sm()
            .flex_1(),
        )
        .child(
            text(SharedString::from(format!("#{seed_id}")))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(
            button(move || text_key("analytics.remove"), ButtonVariant::Secondary)
                .disabled(move || pending.get())
                .on_press(remove),
        )
}

fn commit(
    page: AnalyticsPage,
    ids: Vec<i64>,
    local: Signal<Option<Vec<i64>>>,
    local_account: Signal<String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    generation: Signal<u64>,
) {
    if pending.get_untracked() {
        return;
    }
    let Some(account) = selected_account(page.account_id) else {
        return;
    };
    let ids = normalize_ids(ids);
    let ticket = generation.get_untracked().wrapping_add(1);
    generation.set(ticket);
    pending.set(true);
    failure.set(None);
    let saved_account = account.clone();
    spawn(async move {
        let saved = session::session()
            .runtime
            .spawn_blocking(move || qq_farm_app::farm::set_plant_blacklist(&account, ids))
            .await
            .map_err(|error| error.to_string());
        if generation.get_untracked() != ticket {
            pending.set(false);
            return;
        }
        pending.set(false);
        match saved {
            Ok(ids) => {
                local_account.set(saved_account);
                local.set(Some(ids));
            }
            Err(message) => failure.set(Some(message)),
        }
    });
}

fn selected_account(account_id: Signal<Option<String>>) -> Option<String> {
    account_id.get().filter(|id| !id.is_empty())
}

fn shown_ids(
    account_id: Signal<Option<String>>,
    blacklist: Resource<Vec<i64>, String>,
    local_account: Signal<String>,
    local: Signal<Option<Vec<i64>>>,
) -> Vec<i64> {
    let current = selected_account(account_id).unwrap_or_default();
    if local_account.get() == current {
        if let Some(ids) = local.get() {
            return ids;
        }
    }
    blacklist.get().unwrap_or_default()
}

async fn load_blacklist(account: String) -> Result<Vec<i64>, String> {
    let value = session::session()
        .runtime
        .spawn_blocking(move || qq_farm_app::farm::plant_blacklist(&account))
        .await
        .map_err(|error| error.to_string())?;
    Ok(id_list(&value))
}

fn rank_line(row: RankRow) -> View {
    let name = SharedString::from(row.name);
    let exp = SharedString::from(row.exp_per_hour);
    let gold = SharedString::from(row.gold_per_hour);
    let grow = SharedString::from(row.grow);
    view()
        .flex_row()
        .items_center()
        .gap_4()
        .py_2()
        .child(text(name).text_sm().font_weight(FontWeight::MEDIUM).flex_1())
        .child(metric("analytics.exp", exp))
        .child(metric("analytics.gold", gold))
        .child(metric("analytics.grow", grow))
}

fn metric(label: &'static str, value: SharedString) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            text(move || text_key(label))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(text(value).text_sm())
}

fn rank_rows(value: &Value) -> Vec<RankRow> {
    value.as_array().map(|rows| rows.iter().filter_map(rank_row).collect()).unwrap_or_default()
}

fn rank_row(value: &Value) -> Option<RankRow> {
    let id = value.get("id").and_then(Value::as_i64)?;
    let name = value.get("name").and_then(Value::as_str).filter(|name| !name.is_empty())?;
    Some(RankRow {
        id,
        seed_id: value.get("seedId").and_then(Value::as_i64).filter(|id| *id > 0).unwrap_or(id),
        name: name.to_owned(),
        exp_per_hour: one_decimal(value.get("expPerHour").and_then(Value::as_f64).unwrap_or(0.0)),
        gold_per_hour: one_decimal(value.get("goldPerHour").and_then(Value::as_f64).unwrap_or(0.0)),
        grow: value.get("growTimeStr").and_then(Value::as_str).unwrap_or("").to_owned(),
    })
}

fn one_decimal(value: f64) -> String {
    format!("{value:.1}")
}

fn id_list(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .map(|rows| rows.iter().filter_map(|item| item.as_i64().filter(|id| *id > 0)).collect())
        .unwrap_or_default()
}

fn normalize_ids(ids: Vec<i64>) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| *id > 0 && seen.insert(*id)).collect()
}

fn toggle_id(current: &[i64], seed_id: i64, add: bool) -> Vec<i64> {
    let mut next = current.to_vec();
    if add {
        if seed_id > 0 && !next.contains(&seed_id) {
            next.push(seed_id);
        }
    } else {
        next.retain(|id| *id != seed_id);
    }
    next
}

fn add_ranking_seeds(current: &[i64], rankings: &[RankRow]) -> Vec<i64> {
    let mut next = current.to_vec();
    for row in rankings {
        if row.seed_id > 0 && !next.contains(&row.seed_id) {
            next.push(row.seed_id);
        }
    }
    next
}

fn crop_matches(rankings: &[RankRow], query: &str, selected: &[i64]) -> Vec<RankRow> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    rankings
        .iter()
        .filter(|row| {
            !selected.contains(&row.seed_id)
                && (row.name.to_lowercase().contains(&query)
                    || row.seed_id.to_string().contains(&query))
        })
        .take(8)
        .cloned()
        .collect()
}

fn crop_name(rankings: &[RankRow], seed_id: i64) -> String {
    rankings
        .iter()
        .find(|row| row.seed_id == seed_id)
        .map(|row| row.name.clone())
        .unwrap_or_else(|| format!("#{seed_id}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        add_ranking_seeds, crop_matches, crop_name, id_list, rank_rows, toggle_id, RankRow,
    };

    #[test]
    fn missing_rankings_are_empty() {
        assert!(rank_rows(&json!({})).is_empty());
        assert!(rank_rows(&json!([])).is_empty());
    }

    #[test]
    fn a_ranking_keeps_name_and_rates() {
        let rows = rank_rows(&json!([{
            "id": 4,
            "name": "萝卜",
            "expPerHour": 12.34,
            "goldPerHour": 8,
            "growTimeStr": "2时"
        }]));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "萝卜");
        assert_eq!(rows[0].exp_per_hour, "12.3");
        assert_eq!(rows[0].gold_per_hour, "8.0");
        assert_eq!(rows[0].grow, "2时");
    }

    #[test]
    fn blank_names_are_skipped() {
        let rows = rank_rows(&json!([
            { "id": 1, "name": "" },
            { "name": "无名" },
            { "id": 2, "name": "白菜", "expPerHour": 1.0, "goldPerHour": 2.0, "growTimeStr": "1时" }
        ]));
        assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), [2]);
    }

    #[test]
    fn seed_id_is_kept_when_present_and_otherwise_follows_the_plant_id() {
        let rows = rank_rows(&json!([
            { "id": 4, "seedId": 40, "name": "萝卜" },
            { "id": 5, "name": "白菜" }
        ]));
        assert_eq!(rows[0].seed_id, 40);
        assert_eq!(rows[1].seed_id, 5);
    }

    #[test]
    fn blacklist_edits_keep_positive_ids_and_skip_selected_search_hits() {
        assert_eq!(id_list(&json!([1, 0, -2, "3"])), vec![1]);
        assert_eq!(toggle_id(&[1, 2], 2, false), vec![1]);
        assert_eq!(toggle_id(&[1], 1, true), vec![1]);
        assert_eq!(toggle_id(&[1], 4, true), vec![1, 4]);
        let rankings = vec![
            RankRow {
                id: 1,
                seed_id: 10,
                name: "萝卜".to_owned(),
                exp_per_hour: String::new(),
                gold_per_hour: String::new(),
                grow: String::new(),
            },
            RankRow {
                id: 2,
                seed_id: 20,
                name: "白菜".to_owned(),
                exp_per_hour: String::new(),
                gold_per_hour: String::new(),
                grow: String::new(),
            },
        ];
        assert_eq!(add_ranking_seeds(&[10], &rankings), vec![10, 20]);
        assert!(crop_matches(&rankings, "  ", &[]).is_empty());
        assert_eq!(crop_matches(&rankings, "白", &[10])[0].seed_id, 20);
        assert!(crop_matches(&rankings, "10", &[10]).is_empty());
        assert_eq!(crop_name(&rankings, 99), "#99");
        assert_eq!(crop_name(&rankings, 10), "萝卜");
    }
}
