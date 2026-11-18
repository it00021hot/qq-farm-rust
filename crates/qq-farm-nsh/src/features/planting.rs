use std::collections::{HashMap, HashSet};

use nsh_core::prelude::*;
use serde_json::{json, Value};

use super::settings::{save, SettingsPage, LAND_TYPES};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const STRATEGIES: &[(&str, &str)] = &[
    ("preferred", "settings.plantPreferred"),
    ("level", "settings.plantLevel"),
    ("max_exp", "settings.plantExp"),
    ("max_fert_exp", "settings.plantFertExp"),
    ("max_profit", "settings.plantProfit"),
    ("max_fert_profit", "settings.plantFertProfit"),
    ("bag_priority", "settings.plantBag"),
];

const FALLBACKS: &[(&str, &str)] = &[
    ("level", "settings.plantLevel"),
    ("max_exp", "settings.plantExp"),
    ("max_fert_exp", "settings.plantFertExp"),
    ("max_profit", "settings.plantProfit"),
    ("max_fert_profit", "settings.plantFertProfit"),
    ("preferred", "settings.plantPreferred"),
];

const MATCH_LIMIT: usize = 8;

#[derive(Clone, PartialEq, Eq)]
struct SeedLine {
    id: i64,
    name: String,
    count: i64,
    level: i64,
    size: i64,
    available: bool,
}

pub(crate) fn section(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> impl IntoElement {
    let query = signal(String::new());
    let catalog = resource(|| Some(()), |(), _| async move { load_catalog().await });
    let bag =
        resource(move || bag_account(page, settings), |id, _| async move { load_bag(&id).await });
    let ranks = resource(
        move || {
            preview_sort(&resolved_strategy(&settings.get().unwrap_or(Value::Null)))
                .map(str::to_owned)
        },
        |sort, _| async move { load_rank_ids(sort).await },
    );
    view()
        .gap_2()
        .child(
            text(move || text_key("settings.planting")).text_sm().font_weight(FontWeight::SEMIBOLD),
        )
        .child(strategy_row(page, pending, failure, note, settings))
        .child(when(
            move || preview_line(settings, catalog, ranks).is_some(),
            move || {
                text(move || {
                    SharedString::from(preview_line(settings, catalog, ranks).unwrap_or_default())
                })
                .text_sm()
            },
        ))
        .child(when(
            move || wants_preferred_seed(&settings.get().unwrap_or(Value::Null)),
            move || preferred_seed(page, pending, failure, note, settings, query, catalog),
        ))
        .child(when(
            move || planting_strategy(&settings.get().unwrap_or(Value::Null)) == "bag_priority",
            move || bag_panel(page, pending, failure, note, settings, bag, catalog),
        ))
}

fn strategy_row(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || STRATEGIES.to_vec(),
        |choice: &(&str, &str)| choice.0.to_owned(),
        move |choice, _| {
            let (id, label) = choice.get_untracked();
            button(move || text_key(label), ButtonVariant::Quiet)
                .selected(move || planting_strategy(&settings.get().unwrap_or(Value::Null)) == id)
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    save(page, pending, failure, note, settings, json!({ "plantingStrategy": id }))
                })
        },
    ))
}

fn preferred_seed(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    query: Signal<String>,
    catalog: Resource<Vec<SeedLine>, String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(text(move || text_key("settings.preferredSeed")).text_sm())
        .child(
            text(move || {
                SharedString::from(selected_seed_name(
                    &settings.get().unwrap_or(Value::Null),
                    &catalog.get().unwrap_or_default(),
                ))
            })
            .text_sm(),
        )
        .child(field(
            move || query.get(),
            move || text_key("settings.seedSearch").to_string(),
            move |text: &str| query.set(text.to_owned()),
        ))
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            move || seed_matches(&catalog.get().unwrap_or_default(), &query.get()),
            |seed: &SeedLine| seed.id.to_string(),
            move |seed, _| {
                let line = seed.get_untracked();
                let id = line.id;
                let name = line.name.clone();
                button(move || SharedString::from(name.clone()), ButtonVariant::Quiet)
                    .selected(move || preferred_id(&settings.get().unwrap_or(Value::Null)) == id)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        save(
                            page,
                            pending,
                            failure,
                            note,
                            settings,
                            json!({ "preferredSeedId": id }),
                        )
                    })
            },
        )))
}

fn bag_panel(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
    catalog: Resource<Vec<SeedLine>, String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            button(move || text_key("settings.multiLand"), ButtonVariant::Quiet)
                .selected(move || multi_land(&settings.get().unwrap_or(Value::Null)))
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    let next = !multi_land(&settings.get().unwrap_or(Value::Null));
                    save(
                        page,
                        pending,
                        failure,
                        note,
                        settings,
                        json!({ "bagSeedMultiLandReservationEnabled": next }),
                    )
                }),
        )
        .child(text(move || text_key("settings.fallback")).text_sm())
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || FALLBACKS.to_vec(),
            |choice: &(&str, &str)| choice.0.to_owned(),
            move |choice, _| {
                let (id, label) = choice.get_untracked();
                button(move || text_key(label), ButtonVariant::Quiet)
                    .selected(move || {
                        fallback_strategy(&settings.get().unwrap_or(Value::Null)) == id
                    })
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        save_fallback(page, pending, failure, note, settings, bag, id)
                    })
            },
        )))
        .child(text(move || text_key("settings.bagHint")).text_sm())
        .child(text(move || text_key("settings.landHint")).text_sm())
        .child(
            button(move || text_key("settings.bagReset"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || bag.get().is_none())
                .on_press(move || save_reset(page, pending, failure, note, settings, bag)),
        )
        .child(when(
            move || bag.loading() && bag.get().is_none(),
            move || notice(move || text_key("settings.bagLoading")),
        ))
        .child(when(
            move || bag.error().is_some(),
            move || {
                crate::ui::failure(move || bag.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(
            move || bag.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("settings.bagEmpty")),
        ))
        .child(each(
            move || display_lines(bag, settings),
            |seed: &SeedLine| seed.id.to_string(),
            move |seed, index| {
                seed_row(seed.get_untracked(), index, page, pending, failure, note, settings, bag)
            },
        ))
        .child(when(
            move || !orphan_lines(settings, bag, catalog).is_empty(),
            move || orphan_panel(page, pending, failure, note, settings, bag, catalog),
        ))
}

fn seed_row(
    seed: SeedLine,
    index: ReadSignal<usize>,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
) -> View {
    let id = seed.id;
    let name = seed.name.clone();
    let detail = seed_detail(&seed);
    let size = seed.size;
    view()
        .gap_1()
        .py_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(text(move || SharedString::from((index.get() + 1).to_string())).text_sm())
                .child(text(SharedString::from(name)).text_sm().flex_1())
                .child(text(SharedString::from(format!("{size}x{size}"))).text_sm())
                .child(text(SharedString::from(detail)).text_sm())
                .child(
                    button(move || SharedString::from("↑"), ButtonVariant::Quiet)
                        .disabled(move || pending.get().is_some() || index.get() == 0)
                        .on_press(move || {
                            nudge_saved(page, pending, failure, note, settings, bag, id, -1)
                        }),
                )
                .child(
                    button(move || SharedString::from("↓"), ButtonVariant::Quiet)
                        .disabled(move || {
                            pending.get().is_some()
                                || bag.get().is_some_and(|rows| index.get() + 1 >= rows.len())
                        })
                        .on_press(move || {
                            nudge_saved(page, pending, failure, note, settings, bag, id, 1)
                        }),
                ),
        )
        .child(
            text(move || {
                SharedString::from(land_summary(&seed_lands(
                    &settings.get().unwrap_or(Value::Null),
                    id,
                )))
            })
            .text_sm(),
        )
        .child(land_buttons(id, page, pending, failure, note, settings))
}

fn land_buttons(
    seed_id: i64,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || LAND_TYPES.to_vec(),
        |land: &(&str, &str)| land.0.to_owned(),
        move |land, _| {
            let (land_id, label) = land.get_untracked();
            button(move || text_key(label), ButtonVariant::Quiet)
                .selected(move || {
                    seed_lands(&settings.get().unwrap_or(Value::Null), seed_id)
                        .iter()
                        .any(|item| item == land_id)
                })
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    save_lands(
                        page,
                        pending,
                        failure,
                        note,
                        settings,
                        toggle_land(
                            &land_map(&settings.get().unwrap_or(Value::Null)),
                            seed_id,
                            land_id,
                        ),
                    )
                })
        },
    ))
}

fn orphan_panel(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
    catalog: Resource<Vec<SeedLine>, String>,
) -> View {
    view().gap_2().child(text(move || text_key("settings.landOrphans")).text_sm()).child(each(
        move || orphan_lines(settings, bag, catalog),
        |seed: &SeedLine| seed.id.to_string(),
        move |seed, _| {
            let seed = seed.get_untracked();
            let id = seed.id;
            let name = SharedString::from(seed.name);
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(text(name).text_sm().flex_1())
                .child(
                    text(move || {
                        SharedString::from(land_summary(&seed_lands(
                            &settings.get().unwrap_or(Value::Null),
                            id,
                        )))
                    })
                    .text_sm(),
                )
                .child(
                    button(move || text_key("settings.landClear"), ButtonVariant::Secondary)
                        .disabled(move || pending.get().is_some())
                        .on_press(move || {
                            save_lands(
                                page,
                                pending,
                                failure,
                                note,
                                settings,
                                clear_land(&land_map(&settings.get().unwrap_or(Value::Null)), id),
                            )
                        }),
                )
        },
    ))
}

fn save_fallback(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
    strategy: &'static str,
) {
    let current = settings.get().unwrap_or(Value::Null);
    let patch = match bag.get() {
        Some(seeds) => json!({
            "bagSeedFallbackStrategy": strategy,
            "bagSeedPriority": reset_priority(&current, &seeds, strategy)
        }),
        None => json!({ "bagSeedFallbackStrategy": strategy }),
    };
    save(page, pending, failure, note, settings, patch);
}

fn save_reset(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
) {
    let Some(seeds) = bag.get() else { return };
    let current = settings.get().unwrap_or(Value::Null);
    let strategy = fallback_strategy(&current);
    save(
        page,
        pending,
        failure,
        note,
        settings,
        json!({ "bagSeedPriority": reset_priority(&current, &seeds, &strategy) }),
    );
}

fn nudge_saved(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
    seed_id: i64,
    delta: i64,
) {
    let Some(seeds) = bag.get() else { return };
    let current = settings.get().unwrap_or(Value::Null);
    let Some(next) = nudge_bag_seed(priority_ids(&current), &seed_ids(&seeds), seed_id, delta)
    else {
        return;
    };
    save(page, pending, failure, note, settings, json!({ "bagSeedPriority": next }));
}

fn reset_priority(settings: &Value, seeds: &[SeedLine], strategy: &str) -> Vec<i64> {
    let preferred = preferred_id(settings);
    let ranks = rank_order(strategy);
    let ordered = sorted_seed_ids(seeds, strategy, preferred, &ranks);
    merge_visible_bag_seed_order(priority_ids(settings), &seed_ids(seeds), &ordered)
}

fn rank_order(strategy: &str) -> HashMap<i64, usize> {
    let sort = match strategy {
        "max_exp" => Some("exp"),
        "max_fert_exp" => Some("fert"),
        "max_profit" => Some("profit"),
        "max_fert_profit" => Some("fert_profit"),
        _ => None,
    };
    let Some(sort) = sort else { return HashMap::new() };
    let rows = qq_farm_app::farm::analytics(Some(sort));
    rows.as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, row)| row.get("seedId").and_then(Value::as_i64).map(|id| (id, index)))
        .collect()
}

async fn load_rank_ids(sort: String) -> Result<Vec<i64>, String> {
    let value = session::session()
        .runtime
        .spawn_blocking(move || qq_farm_app::farm::analytics(Some(sort.as_str())))
        .await
        .map_err(|error| error.to_string())?;
    Ok(rank_seed_ids(&value))
}

fn rank_seed_ids(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| json_i64(row, &["seedId", "seed_id"]).filter(|id| *id > 0))
                .collect()
        })
        .unwrap_or_default()
}

async fn load_catalog() -> Result<Vec<SeedLine>, String> {
    let value = session::session()
        .runtime
        .spawn_blocking(qq_farm_app::farm::seeds_catalog)
        .await
        .map_err(|error| error.to_string())?;
    Ok(catalog_lines(&value))
}

async fn load_bag(account_id: &str) -> Result<Vec<SeedLine>, String> {
    let id = account_id.to_owned();
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::farm::bag_seeds(&context, &id).await }).await?;
    Ok(bag_lines(&value))
}

fn bag_account(page: SettingsPage, settings: Resource<Value, String>) -> Option<String> {
    let value = settings.get()?;
    (planting_strategy(&value) == "bag_priority").then(|| page.account_id.get())?
}

fn display_lines(
    bag: Resource<Vec<SeedLine>, String>,
    settings: Resource<Value, String>,
) -> Vec<SeedLine> {
    let Some(seeds) = bag.get() else { return Vec::new() };
    let priority = settings.get().map(|value| priority_ids(&value)).unwrap_or_default();
    sort_lines(seeds, &priority)
}

fn planting_strategy(value: &Value) -> String {
    value
        .get("plantingStrategy")
        .or_else(|| value.get("strategy"))
        .and_then(Value::as_str)
        .filter(|strategy| !strategy.is_empty())
        .unwrap_or("level")
        .to_owned()
}

fn fallback_strategy(value: &Value) -> String {
    value
        .get("bagSeedFallbackStrategy")
        .and_then(Value::as_str)
        .filter(|strategy| !strategy.is_empty())
        .unwrap_or("preferred")
        .to_owned()
}

fn resolved_strategy(value: &Value) -> String {
    let strategy = planting_strategy(value);
    if strategy == "bag_priority" {
        fallback_strategy(value)
    } else {
        strategy
    }
}

fn preview_sort(strategy: &str) -> Option<&'static str> {
    match strategy {
        "max_exp" => Some("exp"),
        "max_fert_exp" => Some("fert"),
        "max_profit" => Some("profit"),
        "max_fert_profit" => Some("fert_profit"),
        _ => None,
    }
}

enum Preview {
    Seed { level: i64, name: String },
    NoneAvailable,
    NoneMatching,
}

fn preview_line(
    settings: Resource<Value, String>,
    catalog: Resource<Vec<SeedLine>, String>,
    ranks: Resource<Vec<i64>, String>,
) -> Option<String> {
    let error = ranks.error();
    preview_message(
        &settings.get().unwrap_or(Value::Null),
        catalog.get().as_deref(),
        ranks.get().as_deref(),
        ranks.loading(),
        error.as_deref(),
    )
}

fn preview_message(
    settings: &Value,
    catalog: Option<&[SeedLine]>,
    ranks: Option<&[i64]>,
    ranks_loading: bool,
    ranks_error: Option<&str>,
) -> Option<String> {
    let strategy = resolved_strategy(settings);
    if strategy == "preferred" {
        return None;
    }
    let seeds = catalog?;
    if seeds.is_empty() {
        return None;
    }
    if preview_sort(&strategy).is_some() {
        if let Some(error) = ranks_error {
            return Some(error.to_owned());
        }
        if ranks.is_none() && ranks_loading {
            return Some(text_key("common.loading").to_string());
        }
    }
    preview_choice(&strategy, seeds, ranks.unwrap_or(&[])).map(|choice| preview_label(&choice))
}

fn preview_choice(strategy: &str, seeds: &[SeedLine], ranks: &[i64]) -> Option<Preview> {
    if strategy == "preferred" || seeds.is_empty() {
        return None;
    }
    let available: Vec<&SeedLine> =
        seeds.iter().filter(|seed| seed.available && seed.id > 0).collect();
    if available.is_empty() {
        return Some(Preview::NoneAvailable);
    }
    if strategy == "level" {
        return available
            .into_iter()
            .max_by(|left, right| left.level.cmp(&right.level).then(right.id.cmp(&left.id)))
            .map(|seed| Preview::Seed { level: seed.level, name: seed.name.clone() });
    }
    ranks
        .iter()
        .find_map(|id| {
            available
                .iter()
                .find(|seed| seed.id == *id)
                .map(|seed| Preview::Seed { level: seed.level, name: seed.name.clone() })
        })
        .or(Some(Preview::NoneMatching))
}

fn preview_label(choice: &Preview) -> String {
    match choice {
        Preview::Seed { level, name } => {
            format!("{}：Lv{level} {name}", text_key("settings.preview"))
        }
        Preview::NoneAvailable => text_key("settings.previewNone").to_string(),
        Preview::NoneMatching => text_key("settings.previewMiss").to_string(),
    }
}

fn wants_preferred_seed(value: &Value) -> bool {
    let strategy = planting_strategy(value);
    strategy == "preferred"
        || (strategy == "bag_priority" && fallback_strategy(value) == "preferred")
}

fn preferred_id(value: &Value) -> i64 {
    value
        .get("preferredSeedId")
        .or_else(|| value.get("preferredSeed"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn multi_land(value: &Value) -> bool {
    value.get("bagSeedMultiLandReservationEnabled").and_then(Value::as_bool).unwrap_or(false)
}

fn priority_ids(value: &Value) -> Vec<i64> {
    value
        .get("bagSeedPriority")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .filter(|id| *id > 0)
        .collect()
}

fn seed_ids(seeds: &[SeedLine]) -> Vec<i64> {
    seeds.iter().map(|seed| seed.id).filter(|id| *id > 0).collect()
}

fn selected_seed_name(settings: &Value, catalog: &[SeedLine]) -> String {
    let id = preferred_id(settings);
    if id <= 0 {
        return text_key("settings.automatic").to_string();
    }
    catalog
        .iter()
        .find(|seed| seed.id == id)
        .map(|seed| seed.name.clone())
        .unwrap_or_else(|| format!("#{id}"))
}

fn seed_matches(catalog: &[SeedLine], query: &str) -> Vec<SeedLine> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    catalog
        .iter()
        .filter(|seed| {
            seed.name.to_lowercase().contains(&query) || seed.id.to_string().contains(&query)
        })
        .take(MATCH_LIMIT)
        .cloned()
        .collect()
}

fn seed_detail(seed: &SeedLine) -> String {
    if locale().as_str().starts_with("zh") {
        format!("库存 {} · Lv{} · ID {}", seed.count, seed.level, seed.id)
    } else {
        format!("In stock {} · Lv{} · ID {}", seed.count, seed.level, seed.id)
    }
}

fn sort_lines(mut seeds: Vec<SeedLine>, priority: &[i64]) -> Vec<SeedLine> {
    seeds.sort_by_key(|seed| priority.iter().position(|id| *id == seed.id).unwrap_or(usize::MAX));
    seeds
}

fn sorted_seed_ids(
    seeds: &[SeedLine],
    strategy: &str,
    preferred: i64,
    ranks: &HashMap<i64, usize>,
) -> Vec<i64> {
    let mut seeds = seeds.to_vec();
    seeds.sort_by_key(|seed| match strategy {
        "preferred" => (i64::from(seed.id != preferred), -seed.level, seed.id),
        "level" => (0, -seed.level, seed.id),
        _ => {
            let rank = ranks.get(&seed.id).copied().map(|rank| rank as i64).unwrap_or(i64::MAX);
            (rank, -seed.level, seed.id)
        }
    });
    seed_ids(&seeds)
}

fn json_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(Value::as_i64).or_else(|| {
            value.get(*key).and_then(Value::as_str).and_then(|text| text.trim().parse().ok())
        })
    })
}

fn json_flag(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(_) | Value::Object(_)) => true,
        _ => false,
    }
}

fn catalog_lines(value: &Value) -> Vec<SeedLine> {
    value.as_array().map(|rows| rows.iter().filter_map(catalog_line).collect()).unwrap_or_default()
}

fn catalog_line(value: &Value) -> Option<SeedLine> {
    let id = value.get("seed_id").or_else(|| value.get("seedId")).and_then(Value::as_i64)?;
    (id > 0).then(|| SeedLine {
        id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        count: 0,
        level: json_i64(
            value,
            &["requiredLevel", "required_level", "landLevelNeed", "land_level_need"],
        )
        .unwrap_or(0),
        size: 1,
        available: !json_flag(value.get("locked")) && !json_flag(value.get("soldOut")),
    })
}

fn bag_lines(value: &Value) -> Vec<SeedLine> {
    value.as_array().map(|rows| rows.iter().filter_map(bag_line).collect()).unwrap_or_default()
}

fn bag_line(value: &Value) -> Option<SeedLine> {
    let id = value.get("seedId").and_then(Value::as_i64).filter(|id| *id > 0)?;
    Some(SeedLine {
        id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        count: value.get("count").and_then(Value::as_i64).unwrap_or(0),
        level: value.get("requiredLevel").and_then(Value::as_i64).unwrap_or(0),
        size: value.get("plantSize").and_then(Value::as_i64).unwrap_or(1),
        available: true,
    })
}

fn nudge_bag_seed(
    existing: Vec<i64>,
    visible: &[i64],
    seed_id: i64,
    delta: i64,
) -> Option<Vec<i64>> {
    let visible_set = visible.iter().copied().collect::<HashSet<_>>();
    let mut visible_order = unique_visible(&existing, &visible_set);
    for id in visible {
        if !visible_order.contains(id) {
            visible_order.push(*id);
        }
    }
    let index = visible_order.iter().position(|id| *id == seed_id)?;
    let target = index as i64 + delta;
    if !(0..visible_order.len() as i64).contains(&target) {
        return None;
    }
    visible_order.swap(index, target as usize);
    Some(merge_visible_bag_seed_order(existing, visible, &visible_order))
}

fn merge_visible_bag_seed_order(
    existing: Vec<i64>,
    visible: &[i64],
    visible_order: &[i64],
) -> Vec<i64> {
    let visible_set = visible.iter().copied().collect::<HashSet<_>>();
    let mut ordered_visible = unique_visible(visible_order, &visible_set);
    for id in visible {
        if !ordered_visible.contains(id) {
            ordered_visible.push(*id);
        }
    }
    let mut merged = Vec::new();
    let mut visible_index = 0;
    for id in existing.into_iter().filter(|id| *id > 0) {
        if visible_set.contains(&id) {
            if let Some(replacement) = ordered_visible.get(visible_index) {
                if !merged.contains(replacement) {
                    merged.push(*replacement);
                }
            }
            visible_index += 1;
        } else if !merged.contains(&id) {
            merged.push(id);
        }
    }
    while let Some(id) = ordered_visible.get(visible_index) {
        if !merged.contains(id) {
            merged.push(*id);
        }
        visible_index += 1;
    }
    merged
}

fn save_lands(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    note: Signal<Option<String>>,
    settings: Resource<Value, String>,
    map: HashMap<i64, Vec<String>>,
) {
    save(page, pending, failure, note, settings, land_patch(&map));
}

fn seed_lands(value: &Value, seed_id: i64) -> Vec<String> {
    land_map(value).get(&seed_id).cloned().unwrap_or_default()
}

fn land_map(value: &Value) -> HashMap<i64, Vec<String>> {
    let Some(object) = value.get("bagSeedLandTypes").and_then(Value::as_object) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for (key, types) in object {
        let Ok(id) = key.parse::<i64>() else { continue };
        let Some(rows) = types.as_array() else { continue };
        let selected = ordered_lands(
            &rows.iter().filter_map(|item| item.as_str().map(str::to_owned)).collect::<Vec<_>>(),
        );
        if id > 0 && !selected.is_empty() && selected.len() < LAND_TYPES.len() {
            map.insert(id, selected);
        }
    }
    map
}

fn toggle_land(
    map: &HashMap<i64, Vec<String>>,
    seed_id: i64,
    land: &str,
) -> HashMap<i64, Vec<String>> {
    if seed_id <= 0 || !LAND_TYPES.iter().any(|(id, _)| *id == land) {
        return map.clone();
    }
    let mut next = map.clone();
    let mut selected = next.get(&seed_id).cloned().unwrap_or_default();
    if let Some(index) = selected.iter().position(|item| item == land) {
        selected.remove(index);
    } else {
        selected.push(land.to_owned());
    }
    let selected = ordered_lands(&selected);
    if selected.is_empty() || selected.len() == LAND_TYPES.len() {
        next.remove(&seed_id);
    } else {
        next.insert(seed_id, selected);
    }
    next
}

fn clear_land(map: &HashMap<i64, Vec<String>>, seed_id: i64) -> HashMap<i64, Vec<String>> {
    let mut next = map.clone();
    next.remove(&seed_id);
    next
}

fn ordered_lands(selected: &[String]) -> Vec<String> {
    LAND_TYPES
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .filter(|id| selected.iter().any(|item| item == id))
        .collect()
}

fn land_patch(map: &HashMap<i64, Vec<String>>) -> Value {
    let mut object = serde_json::Map::new();
    let mut ids: Vec<i64> = map.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if let Some(types) = map.get(&id) {
            object.insert(id.to_string(), json!(types));
        }
    }
    json!({ "bagSeedLandTypes": object })
}

fn land_summary(types: &[String]) -> String {
    let selected = ordered_lands(types);
    if selected.is_empty() || selected.len() == LAND_TYPES.len() {
        return text_key("settings.landAny").to_string();
    }
    let names = LAND_TYPES
        .iter()
        .filter(|(id, _)| selected.iter().any(|item| item == *id))
        .map(|(_, label)| text_key(label).to_string())
        .collect::<Vec<_>>()
        .join("、");
    format!("{} {names}", text_key("settings.landOnly"))
}

fn orphan_lines(
    settings: Resource<Value, String>,
    bag: Resource<Vec<SeedLine>, String>,
    catalog: Resource<Vec<SeedLine>, String>,
) -> Vec<SeedLine> {
    let Some(visible_rows) = bag.get() else { return Vec::new() };
    let visible: HashSet<i64> = visible_rows.iter().map(|seed| seed.id).collect();
    let catalog_rows = catalog.get().unwrap_or_default();
    let mut ids: Vec<i64> = land_map(&settings.get().unwrap_or(Value::Null))
        .into_keys()
        .filter(|id| !visible.contains(id))
        .collect();
    ids.sort_unstable();
    ids.into_iter()
        .map(|id| SeedLine {
            id,
            name: catalog_rows
                .iter()
                .find(|seed| seed.id == id)
                .map(|seed| seed.name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("#{id}")),
            count: 0,
            level: 0,
            size: 0,
            available: true,
        })
        .collect()
}

fn unique_visible(ids: &[i64], visible: &HashSet<i64>) -> Vec<i64> {
    ids.iter().copied().filter(|id| visible.contains(id)).fold(Vec::new(), |mut kept, id| {
        if !kept.contains(&id) {
            kept.push(id);
        }
        kept
    })
}

#[cfg(test)]
mod tests {
    use super::{
        bag_lines, catalog_lines, clear_land, land_map, merge_visible_bag_seed_order,
        nudge_bag_seed, preview_choice, preview_sort, rank_seed_ids, resolved_strategy,
        seed_matches, sorted_seed_ids, toggle_land, Preview, SeedLine,
    };
    use std::collections::HashMap;

    fn line(id: i64, level: i64) -> SeedLine {
        SeedLine { id, name: format!("种子{id}"), count: 1, level, size: 1, available: true }
    }

    #[test]
    fn moving_a_visible_seed_keeps_hidden_priority() {
        assert_eq!(
            merge_visible_bag_seed_order(vec![10, 20, 30, 99], &[10, 20, 30], &[20, 10, 30]),
            vec![20, 10, 30, 99]
        );
        assert_eq!(
            nudge_bag_seed(vec![10, 20, 30, 99], &[10, 20, 30], 10, 1),
            Some(vec![20, 10, 30, 99])
        );
        assert_eq!(
            nudge_bag_seed(vec![10, 20, 30, 99], &[10, 20, 30], 10, 2),
            Some(vec![30, 20, 10, 99])
        );
        assert_eq!(nudge_bag_seed(vec![10, 20], &[10, 20], 10, -1), None);
        assert_eq!(nudge_bag_seed(vec![10], &[10], 99, 1), None);
    }

    #[test]
    fn reset_puts_the_preferred_seed_first_and_higher_levels_next() {
        let seeds = vec![line(10, 1), line(20, 4), line(30, 2)];
        assert_eq!(sorted_seed_ids(&seeds, "preferred", 30, &HashMap::new()), vec![30, 20, 10]);
        assert_eq!(sorted_seed_ids(&seeds, "level", 30, &HashMap::new()), vec![20, 30, 10]);
        let ranks = HashMap::from([(10, 0), (30, 1)]);
        assert_eq!(sorted_seed_ids(&seeds, "max_exp", 0, &ranks), vec![10, 30, 20]);
    }

    #[test]
    fn search_matches_name_or_id_and_skips_a_blank_query() {
        let catalog = catalog_lines(&serde_json::json!([
            { "seed_id": 12, "name": "萝卜" },
            { "seedId": 0, "name": "drop" },
            { "seed_id": 40, "name": "玉米" }
        ]));
        assert!(seed_matches(&catalog, "  ").is_empty());
        assert_eq!(seed_matches(&catalog, "萝")[0].id, 12);
        assert_eq!(seed_matches(&catalog, "40")[0].name, "玉米");
        let bag = bag_lines(
            &serde_json::json!([{ "seedId": 8, "name": "白菜", "count": 3, "requiredLevel": 2, "plantSize": 2 }]),
        );
        assert_eq!(bag[0].count, 3);
        assert_eq!(bag[0].size, 2);
    }

    #[test]
    fn land_limits_drop_empty_or_complete_selections_and_keep_other_seeds() {
        let current = serde_json::json!({
            "bagSeedLandTypes": {
                "10": ["normal", "gold"],
                "99": ["red"],
                "12": ["blue-crystal", "purple-gold", "gold", "black", "red", "normal"],
                "0": ["gold"]
            }
        });
        let map = land_map(&current);
        assert_eq!(
            map.get(&10).cloned().unwrap_or_default(),
            vec!["gold".to_owned(), "normal".to_owned()]
        );
        assert_eq!(map.get(&99).cloned().unwrap_or_default(), vec!["red".to_owned()]);
        assert!(!map.contains_key(&12));
        assert!(!map.contains_key(&0));
        let gold_only = toggle_land(&map, 10, "normal");
        assert_eq!(gold_only.get(&10).cloned().unwrap_or_default(), vec!["gold".to_owned()]);
        assert_eq!(gold_only.get(&99).cloned().unwrap_or_default(), vec!["red".to_owned()]);
        let mut all = gold_only;
        for land in ["blue-crystal", "purple-gold", "black", "red", "normal"] {
            all = toggle_land(&all, 10, land);
        }
        assert!(!all.contains_key(&10));
        assert!(clear_land(&map, 99).get(&10).is_some());
        assert!(clear_land(&map, 99).get(&99).is_none());
        assert!(land_map(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn preview_picks_the_highest_level_or_the_first_available_rank() {
        let seeds = vec![
            SeedLine {
                id: 20, name: "番茄".into(), count: 0, level: 8, size: 1, available: true
            },
            SeedLine {
                id: 10, name: "玉米".into(), count: 0, level: 8, size: 1, available: true
            },
            SeedLine {
                id: 1, name: "锁定".into(), count: 0, level: 99, size: 1, available: false
            },
        ];
        assert!(matches!(
            preview_choice("level", &seeds, &[]),
            Some(Preview::Seed { level: 8, name }) if name == "玉米"
        ));
        assert!(matches!(
            preview_choice("max_profit", &seeds, &[30, 20, 10]),
            Some(Preview::Seed { level: 8, name }) if name == "番茄"
        ));
        assert!(matches!(preview_choice("max_exp", &seeds, &[]), Some(Preview::NoneMatching)));
        assert!(matches!(
            preview_choice("level", &[seeds[2].clone()], &[]),
            Some(Preview::NoneAvailable)
        ));
        assert!(preview_choice("preferred", &seeds, &[]).is_none());
        assert_eq!(
            resolved_strategy(&serde_json::json!({
                "plantingStrategy": "bag_priority",
                "bagSeedFallbackStrategy": "max_fert_profit"
            })),
            "max_fert_profit"
        );
        assert_eq!(preview_sort("max_fert_profit"), Some("fert_profit"));
        assert_eq!(
            rank_seed_ids(&serde_json::json!([{"seedId": 30}, {"seedId": "20"}])),
            vec![30, 20]
        );
    }
}
