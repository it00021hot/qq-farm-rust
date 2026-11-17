use nsh_core::file_dialog::{FileFilter, OpenDialog};
use nsh_core::prelude::*;
use qq_farm_app::config::{
    delete_game_config, FruitConfigWrite, GameConfigKind, ItemConfigWrite, SeedConfigWrite,
};
use serde_json::Value;

use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const SEED_PHASES: &str = "种子:2400;发芽:2400;小叶子:2400;大叶子:2400;开花:2400;成熟:0;";
const ITEM_TYPES: &[(i64, &str)] = &[
    (1, "config.type1"),
    (2, "config.type2"),
    (3, "config.type3"),
    (4, "config.type4"),
    (7, "config.type7"),
    (8, "config.type8"),
    (9, "config.type9"),
    (10, "config.type10"),
    (11, "config.type11"),
    (12, "config.type12"),
    (13, "config.type13"),
    (14, "config.type14"),
    (15, "config.type15"),
    (16, "config.type16"),
    (17, "config.type17"),
    (18, "config.type18"),
    (23, "config.type23"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum CatalogKind {
    Seeds,
    Fruits,
    Items,
}

impl CatalogKind {
    const ALL: [Self; 3] = [Self::Seeds, Self::Fruits, Self::Items];

    const fn id(self) -> &'static str {
        match self {
            Self::Seeds => "seeds",
            Self::Fruits => "fruits",
            Self::Items => "items",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Seeds => "config.seeds",
            Self::Fruits => "config.fruits",
            Self::Items => "config.items",
        }
    }
}

#[derive(Clone, Copy)]
enum Field {
    Id,
    Name,
    Price,
    PriceId,
    Level,
    Seasons,
    Exp,
    Size,
    FruitCount,
    Phases,
    Rarity,
    MaxCount,
    CanUse,
    Desc,
    Effect,
    Interaction,
    Asset,
    Image,
}

#[derive(Clone, Default, PartialEq)]
struct ConfigDraft {
    id: String,
    name: String,
    price: String,
    price_id: String,
    level: String,
    seasons: String,
    exp: String,
    size: String,
    fruit_count: String,
    grow_phases: String,
    rarity: String,
    max_count: String,
    can_use: String,
    desc: String,
    effect: String,
    interaction: String,
    asset: String,
    image: String,
    item_type: i64,
    plant_id: i64,
    plant_name: String,
}

#[derive(Clone, PartialEq, Eq)]
struct CatalogRow {
    id: i64,
    name: String,
    price: i64,
}

#[derive(Debug)]
enum Prepared {
    Seed(SeedConfigWrite),
    Fruit(FruitConfigWrite),
    Item(ItemConfigWrite),
}

enum ImageFailure {
    Format,
    Read(String),
}

pub(crate) fn page() -> impl IntoElement {
    let kind = signal(CatalogKind::Seeds);
    let query = signal(String::new());
    let plant_query = signal(String::new());
    let selected = signal(None::<i64>);
    let draft = signal(new_draft(CatalogKind::Seeds));
    let pending = signal(false);
    let failure = signal(None::<String>);
    let catalog =
        resource(move || Some(kind.get()), |kind, _| async move { load_catalog(kind).await });
    let plants = resource(|| Some(()), |_, _| async move { load_plants().await });
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("config.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(kind_switch(kind, selected, draft, plant_query))
        .child(field(
            move || query.get(),
            move || text_key("config.search").to_string(),
            move |text: &str| query.set(text.to_owned()),
        ))
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
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
        .child(editor(kind, selected, draft, pending, failure, catalog, plants, plant_query))
        .child(when(
            move || catalog.get().is_some() && visible_values(catalog, query).is_empty(),
            move || notice(move || text_key("config.empty")),
        ))
        .child(each(
            move || visible_values(catalog, query),
            |row: &Value| catalog_row_from(row).map(|item| item.id.to_string()).unwrap_or_default(),
            move |row, _| open_row(row.get_untracked(), kind, selected, draft),
        ))
}

fn kind_switch(
    kind: Signal<CatalogKind>,
    selected: Signal<Option<i64>>,
    draft: Signal<ConfigDraft>,
    plant_query: Signal<String>,
) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || CatalogKind::ALL.to_vec(),
        |item: &CatalogKind| item.id().to_owned(),
        move |item, _| {
            let item = item.get_untracked();
            button(move || text_key(item.label_key()), ButtonVariant::Quiet)
                .selected(move || kind.get() == item)
                .on_press(move || {
                    kind.set(item);
                    selected.set(None);
                    draft.set(new_draft(item));
                    plant_query.set(String::new());
                })
        },
    ))
}

fn editor(
    kind: Signal<CatalogKind>,
    selected: Signal<Option<i64>>,
    draft: Signal<ConfigDraft>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    catalog: Resource<Vec<Value>, String>,
    plants: Resource<Vec<Value>, String>,
    plant_query: Signal<String>,
) -> View {
    view()
        .gap_3()
        .child(
            text(move || {
                text_key(if selected.get().is_some() { "config.edit" } else { "config.create" })
            })
            .font_weight(FontWeight::SEMIBOLD),
        )
        .child(text(move || text_key("config.hint")).text_sm())
        .child(labeled_field("config.id", draft, Field::Id))
        .child(labeled_field("config.name", draft, Field::Name))
        .child(labeled_field("config.price", draft, Field::Price))
        .child(labeled_field("config.priceId", draft, Field::PriceId))
        .child(labeled_field("config.level", draft, Field::Level))
        .child(when(move || kind.get() == CatalogKind::Seeds, move || seed_fields(draft)))
        .child(when(
            move || kind.get() == CatalogKind::Fruits,
            move || fruit_fields(draft, plants, plant_query),
        ))
        .child(when(move || kind.get() == CatalogKind::Items, move || item_fields(draft)))
        .child(when(move || kind.get() != CatalogKind::Seeds, move || shared_details(draft)))
        .child(image_row(draft, pending, failure))
        .child(actions(kind, selected, draft, pending, failure, catalog, plant_query))
}

fn seed_fields(draft: Signal<ConfigDraft>) -> View {
    view()
        .gap_2()
        .child(labeled_field("config.seasons", draft, Field::Seasons))
        .child(labeled_field("config.exp", draft, Field::Exp))
        .child(labeled_field("config.size", draft, Field::Size))
        .child(labeled_field("config.fruitCount", draft, Field::FruitCount))
        .child(labeled_field("config.phases", draft, Field::Phases))
}

fn fruit_fields(
    draft: Signal<ConfigDraft>,
    plants: Resource<Vec<Value>, String>,
    plant_query: Signal<String>,
) -> View {
    view()
        .gap_2()
        .child(labeled_field("config.fruitCount", draft, Field::FruitCount))
        .child(plant_picker(draft, plants, plant_query))
}

fn item_fields(draft: Signal<ConfigDraft>) -> View {
    view()
        .gap_2()
        .child(text(move || text_key("config.itemType")).text_sm())
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || ITEM_TYPES.to_vec(),
            |item: &(i64, &str)| item.0.to_string(),
            move |item, _| {
                let (value, label) = item.get_untracked();
                button(move || text_key(label), ButtonVariant::Quiet)
                    .selected(move || draft.get().item_type == value)
                    .on_press(move || {
                        let mut next = draft.get();
                        next.item_type = value;
                        draft.set(next);
                    })
            },
        )))
        .child(labeled_field("config.interaction", draft, Field::Interaction))
        .child(labeled_field("config.canUse", draft, Field::CanUse))
}

fn shared_details(draft: Signal<ConfigDraft>) -> View {
    view()
        .gap_2()
        .child(labeled_field("config.rarity", draft, Field::Rarity))
        .child(labeled_field("config.maxCount", draft, Field::MaxCount))
        .child(labeled_field("config.desc", draft, Field::Desc))
        .child(labeled_field("config.effect", draft, Field::Effect))
        .child(labeled_field("config.asset", draft, Field::Asset))
}

fn plant_picker(
    draft: Signal<ConfigDraft>,
    plants: Resource<Vec<Value>, String>,
    plant_query: Signal<String>,
) -> View {
    view()
        .gap_2()
        .child(text(move || text_key("config.plant")).text_sm())
        .child(text(move || plant_label(&draft.get())).text_sm())
        .child(field(
            move || plant_query.get(),
            move || text_key("config.plantSearch").to_string(),
            move |text: &str| plant_query.set(text.to_owned()),
        ))
        .child(each(
            move || matched_plants(plants, plant_query),
            |plant: &(i64, String)| plant.0.to_string(),
            move |plant, _| {
                let (id, name) = plant.get_untracked();
                let label = SharedString::from(format!("#{id} {name}"));
                button(label, ButtonVariant::Quiet).on_press(move || {
                    let mut next = draft.get();
                    next.plant_id = id;
                    next.plant_name = name.clone();
                    draft.set(next);
                    plant_query.set(String::new());
                })
            },
        ))
}

fn image_row(
    draft: Signal<ConfigDraft>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) -> View {
    let mut row = view().gap_2().child(labeled_field("config.image", draft, Field::Image));
    if is_desktop() {
        row = row.child(
            button(move || text_key("config.chooseImage"), ButtonVariant::Secondary)
                .disabled(move || pending.get())
                .on_press(move || choose_image(draft, failure)),
        );
    }
    row
}

fn actions(
    kind: Signal<CatalogKind>,
    selected: Signal<Option<i64>>,
    draft: Signal<ConfigDraft>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    catalog: Resource<Vec<Value>, String>,
    plant_query: Signal<String>,
) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(
            button(move || text_key("config.save"), ButtonVariant::Primary)
                .disabled(move || pending.get() || draft.get().name.trim().is_empty())
                .on_press(move || save_draft(kind, selected, draft, pending, failure, catalog)),
        )
        .child(
            button(move || text_key("config.create"), ButtonVariant::Secondary)
                .disabled(move || pending.get())
                .on_press(move || {
                    selected.set(None);
                    draft.set(new_draft(kind.get()));
                    plant_query.set(String::new());
                    failure.set(None);
                }),
        )
        .child(
            button(move || text_key("config.delete"), ButtonVariant::Secondary)
                .disabled(move || pending.get() || selected.get().is_none())
                .on_press(move || ask_delete(kind, selected, draft, pending, failure, catalog)),
        )
}

fn labeled_field(label: &'static str, draft: Signal<ConfigDraft>, slot: Field) -> View {
    view().gap_1().child(text(move || text_key(label)).text_sm()).child(field(
        move || read_field(&draft.get(), slot),
        move || text_key(label).to_string(),
        move |text: &str| {
            let mut next = draft.get();
            write_field(&mut next, slot, text.to_owned());
            draft.set(next);
        },
    ))
}

fn open_row(
    value: Value,
    kind: Signal<CatalogKind>,
    selected: Signal<Option<i64>>,
    draft: Signal<ConfigDraft>,
) -> View {
    let Some(item) = catalog_row_from(&value) else {
        return view();
    };
    let id = item.id;
    let title = SharedString::from(item.name);
    let price = SharedString::from(item.price.to_string());
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(
            button(title, ButtonVariant::Quiet)
                .flex_1()
                .justify_start()
                .selected(move || selected.get() == Some(id))
                .on_press(move || {
                    selected.set(Some(id));
                    draft.set(draft_from(kind.get(), &value));
                }),
        )
        .child(
            text(SharedString::from(id.to_string()))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(text(price).text_sm())
}

fn choose_image(draft: Signal<ConfigDraft>, failure: Signal<Option<String>>) {
    let title = text_key("config.chooseImage").to_string();
    let dialog = OpenDialog::files()
        .title(title.clone())
        .button_label(title)
        .filter(FileFilter::new("Image", ["png", "jpg", "jpeg", "webp"]));
    spawn(async move {
        match nsh_core::element::pick_paths(dialog).await {
            Ok(Some(paths)) => {
                if let Some(path) = paths.into_iter().next() {
                    let mut next = draft.get();
                    next.image = path.to_string_lossy().into_owned();
                    draft.set(next);
                }
            }
            Ok(None) => {}
            Err(error) => failure.set(Some(error)),
        }
    });
}

fn save_draft(
    kind: Signal<CatalogKind>,
    selected: Signal<Option<i64>>,
    draft: Signal<ConfigDraft>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    catalog: Resource<Vec<Value>, String>,
) {
    if pending.get_untracked() {
        return;
    }
    let current = draft.get_untracked();
    let kind = kind.get_untracked();
    let prepared = match prepare(kind, &current) {
        Ok(prepared) => prepared,
        Err(key) => {
            failure.set(Some(text_key(key).to_string()));
            return;
        }
    };
    let image_path = current.image.clone();
    let saved_id = current.id.trim().parse::<i64>().ok().filter(|id| *id > 0);
    let format_error = text_key("config.imageFormat").to_string();
    let read_error = text_key("config.imageRead").to_string();
    pending.set(true);
    failure.set(None);
    spawn(async move {
        let result = session::session()
            .runtime
            .spawn_blocking(move || {
                write_prepared(prepared, &image_path, &format_error, &read_error)
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|saved| saved);
        pending.set(false);
        if let Err(message) = result {
            failure.set(Some(message));
            return;
        }
        if let Some(id) = saved_id {
            selected.set(Some(id));
        }
        catalog.refetch();
    });
}

fn ask_delete(
    kind: Signal<CatalogKind>,
    selected: Signal<Option<i64>>,
    draft: Signal<ConfigDraft>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    catalog: Resource<Vec<Value>, String>,
) {
    let Some(id) = selected.get_untracked() else {
        return;
    };
    let kind = kind.get_untracked();
    let plant_id = draft.get_untracked().plant_id;
    confirm::confirm(
        "config.delete.title",
        "config.delete.body",
        "delete",
        "config.delete",
        move |accepted| {
            if !accepted || pending.get_untracked() {
                return;
            }
            pending.set(true);
            failure.set(None);
            spawn(async move {
                let result = session::session()
                    .runtime
                    .spawn_blocking(move || remove_config(kind, id, plant_id))
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|removed| removed.map_err(|error| error.to_string()));
                pending.set(false);
                if let Err(message) = result {
                    failure.set(Some(message));
                    return;
                }
                selected.set(None);
                draft.set(new_draft(kind));
                catalog.refetch();
            });
        },
    );
}

fn write_prepared(
    prepared: Prepared,
    image_path: &str,
    format_error: &str,
    read_error: &str,
) -> Result<(), String> {
    let image = match read_image(image_path) {
        Ok(image) => image,
        Err(ImageFailure::Format) => return Err(format_error.to_owned()),
        Err(ImageFailure::Read(detail)) => return Err(format!("{read_error}{detail}")),
    };
    let saved = match prepared {
        Prepared::Seed(write) => qq_farm_app::config::upsert_seed_config(write, image.as_ref()),
        Prepared::Fruit(write) => qq_farm_app::config::upsert_fruit_config(write, image.as_ref()),
        Prepared::Item(write) => qq_farm_app::config::upsert_item_config(write, image.as_ref()),
    };
    saved.map_err(|error| error.to_string())
}

fn remove_config(kind: CatalogKind, id: i64, plant_id: i64) -> qq_farm_app::AppResult<()> {
    for (target, target_id) in delete_targets(kind, id, plant_id) {
        delete_game_config(target, target_id)?;
    }
    Ok(())
}

fn new_draft(kind: CatalogKind) -> ConfigDraft {
    let mut draft = ConfigDraft::default();
    match kind {
        CatalogKind::Seeds => {
            draft.level = "1".into();
            draft.seasons = "1".into();
            draft.fruit_count = "200".into();
            draft.grow_phases = SEED_PHASES.into();
            draft.price = "0".into();
            draft.price_id = "0".into();
            draft.exp = "0".into();
            draft.size = "0".into();
        }
        CatalogKind::Fruits => {
            draft.level = "0".into();
            draft.fruit_count = "200".into();
            draft.price = "0".into();
            draft.price_id = "0".into();
            draft.rarity = "0".into();
            draft.max_count = "9999".into();
            draft.item_type = 6;
        }
        CatalogKind::Items => {
            draft.level = "0".into();
            draft.price = "0".into();
            draft.price_id = "0".into();
            draft.rarity = "0".into();
            draft.max_count = "9999".into();
            draft.can_use = "0".into();
            draft.item_type = 7;
        }
    }
    draft
}

fn draft_from(kind: CatalogKind, value: &Value) -> ConfigDraft {
    let mut draft = ConfigDraft {
        item_type: match kind {
            CatalogKind::Fruits => 6,
            CatalogKind::Items => 7,
            CatalogKind::Seeds => 5,
        },
        ..ConfigDraft::default()
    };
    let id_keys: &[&str] = match kind {
        CatalogKind::Seeds => &["seedId", "seed_id", "id"],
        CatalogKind::Fruits | CatalogKind::Items => &["id"],
    };
    set_text(&mut draft.id, value, id_keys);
    set_text(&mut draft.name, value, &["name"]);
    set_text(&mut draft.price, value, &["price"]);
    set_text(&mut draft.price_id, value, &["priceId", "price_id"]);
    set_text(&mut draft.level, value, &["level", "requiredLevel", "landLevelNeed"]);
    set_text(&mut draft.seasons, value, &["seasons"]);
    set_text(&mut draft.exp, value, &["exp"]);
    set_text(&mut draft.size, value, &["size"]);
    set_text(&mut draft.fruit_count, value, &["fruitCount", "harvestCount"]);
    set_text(&mut draft.grow_phases, value, &["growPhases", "grow_phases"]);
    set_text(&mut draft.rarity, value, &["rarity"]);
    set_text(&mut draft.max_count, value, &["maxCount", "max_count"]);
    set_text(&mut draft.can_use, value, &["canUse", "can_use"]);
    set_text(&mut draft.desc, value, &["desc", "description"]);
    set_text(&mut draft.effect, value, &["effectDesc", "effect_desc"]);
    set_text(&mut draft.interaction, value, &["interactionType", "interaction_type"]);
    set_text(&mut draft.asset, value, &["assetName", "harvestAnimation"]);
    if let Some(id) = value.get("plantId").or_else(|| value.get("plant_id")).and_then(Value::as_i64)
    {
        draft.plant_id = id;
    }
    if let Some(name) =
        value.get("plantName").or_else(|| value.get("plant_name")).and_then(Value::as_str)
    {
        draft.plant_name = name.to_owned();
    }
    if let Some(item_type) =
        value.get("type").or_else(|| value.get("itemType")).and_then(Value::as_i64)
    {
        draft.item_type = item_type;
    }
    draft
}

fn prepare(kind: CatalogKind, draft: &ConfigDraft) -> Result<Prepared, &'static str> {
    match kind {
        CatalogKind::Seeds => Ok(Prepared::Seed(seed_write(draft)?)),
        CatalogKind::Fruits => Ok(Prepared::Fruit(fruit_write(draft)?)),
        CatalogKind::Items => Ok(Prepared::Item(item_write(draft)?)),
    }
}

fn seed_write(draft: &ConfigDraft) -> Result<SeedConfigWrite, &'static str> {
    Ok(SeedConfigWrite {
        seed_id: Some(parse_id(&draft.id)?),
        name: Some(required_name(&draft.name)?),
        grow_phases: Some(draft.grow_phases.trim().to_owned()),
        land_level_need: Some(parse_count(&draft.level)?),
        seasons: Some(parse_count(&draft.seasons)?),
        fruit_count: Some(parse_count(&draft.fruit_count)?),
        price: Some(parse_count(&draft.price)?),
        price_id: Some(parse_count(&draft.price_id)?),
        exp: Some(parse_count(&draft.exp)?),
        size: Some(parse_count(&draft.size)?),
    })
}

fn fruit_write(draft: &ConfigDraft) -> Result<FruitConfigWrite, &'static str> {
    if draft.plant_id <= 0 {
        return Err("config.needPlant");
    }
    Ok(FruitConfigWrite {
        id: parse_optional_id(&draft.id)?,
        plant_id: Some(draft.plant_id),
        name: Some(required_name(&draft.name)?),
        price: Some(parse_count(&draft.price)?),
        price_id: Some(parse_count(&draft.price_id)?),
        desc: Some(draft.desc.trim().to_owned()),
        effect_desc: Some(draft.effect.trim().to_owned()),
        rarity: Some(parse_count(&draft.rarity)?),
        max_count: Some(parse_count(&draft.max_count)?),
        level: Some(parse_count(&draft.level)?),
        fruit_count: Some(parse_count(&draft.fruit_count)?),
        asset_name: Some(draft.asset.trim().to_owned()),
    })
}

fn item_write(draft: &ConfigDraft) -> Result<ItemConfigWrite, &'static str> {
    if draft.item_type <= 0 {
        return Err("config.typeRequired");
    }
    Ok(ItemConfigWrite {
        id: Some(parse_id(&draft.id)?),
        item_type: Some(draft.item_type),
        name: Some(required_name(&draft.name)?),
        price: Some(parse_count(&draft.price)?),
        price_id: Some(parse_count(&draft.price_id)?),
        interaction_type: Some(draft.interaction.trim().to_owned()),
        can_use: Some(parse_count(&draft.can_use)?),
        desc: Some(draft.desc.trim().to_owned()),
        effect_desc: Some(draft.effect.trim().to_owned()),
        rarity: Some(parse_count(&draft.rarity)?),
        max_count: Some(parse_count(&draft.max_count)?),
        level: Some(parse_count(&draft.level)?),
        asset_name: Some(draft.asset.trim().to_owned()),
    })
}

fn delete_targets(kind: CatalogKind, id: i64, plant_id: i64) -> Vec<(GameConfigKind, i64)> {
    match kind {
        CatalogKind::Seeds => {
            let plant = if plant_id > 0 { plant_id } else { id };
            vec![(GameConfigKind::Plant, plant), (GameConfigKind::Item, id)]
        }
        CatalogKind::Fruits | CatalogKind::Items => vec![(GameConfigKind::Item, id)],
    }
}

fn read_image(path: &str) -> Result<Option<qq_farm_app::config::GameConfigImage>, ImageFailure> {
    let path = path.trim();
    if path.is_empty() {
        return Ok(None);
    }
    if !image_supported(path) {
        return Err(ImageFailure::Format);
    }
    let bytes = std::fs::read(path).map_err(|error| ImageFailure::Read(error.to_string()))?;
    use base64::Engine as _;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config-image.png")
        .to_owned();
    Ok(Some(qq_farm_app::config::GameConfigImage {
        name,
        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

fn image_supported(path: &str) -> bool {
    ["png", "jpg", "jpeg", "webp"]
        .iter()
        .any(|extension| path.to_lowercase().ends_with(&format!(".{extension}")))
}

fn parse_id(raw: &str) -> Result<i64, &'static str> {
    let id = parse_count(raw)?;
    if id <= 0 {
        Err("config.idRequired")
    } else {
        Ok(id)
    }
}

fn parse_optional_id(raw: &str) -> Result<Option<i64>, &'static str> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let id = parse_count(raw)?;
    Ok(if id == 0 { None } else { Some(id) })
}

fn required_name(raw: &str) -> Result<String, &'static str> {
    let name = raw.trim();
    if name.is_empty() {
        Err("config.nameRequired")
    } else {
        Ok(name.to_owned())
    }
}

fn parse_count(raw: &str) -> Result<i64, &'static str> {
    let raw = raw.trim();
    let value = if raw.is_empty() { 0 } else { raw.parse::<i64>().map_err(|_| "config.number")? };
    if value < 0 {
        Err("config.number")
    } else {
        Ok(value)
    }
}

fn read_field(draft: &ConfigDraft, slot: Field) -> String {
    match slot {
        Field::Id => draft.id.clone(),
        Field::Name => draft.name.clone(),
        Field::Price => draft.price.clone(),
        Field::PriceId => draft.price_id.clone(),
        Field::Level => draft.level.clone(),
        Field::Seasons => draft.seasons.clone(),
        Field::Exp => draft.exp.clone(),
        Field::Size => draft.size.clone(),
        Field::FruitCount => draft.fruit_count.clone(),
        Field::Phases => draft.grow_phases.clone(),
        Field::Rarity => draft.rarity.clone(),
        Field::MaxCount => draft.max_count.clone(),
        Field::CanUse => draft.can_use.clone(),
        Field::Desc => draft.desc.clone(),
        Field::Effect => draft.effect.clone(),
        Field::Interaction => draft.interaction.clone(),
        Field::Asset => draft.asset.clone(),
        Field::Image => draft.image.clone(),
    }
}

fn write_field(draft: &mut ConfigDraft, slot: Field, value: String) {
    match slot {
        Field::Id => draft.id = value,
        Field::Name => draft.name = value,
        Field::Price => draft.price = value,
        Field::PriceId => draft.price_id = value,
        Field::Level => draft.level = value,
        Field::Seasons => draft.seasons = value,
        Field::Exp => draft.exp = value,
        Field::Size => draft.size = value,
        Field::FruitCount => draft.fruit_count = value,
        Field::Phases => draft.grow_phases = value,
        Field::Rarity => draft.rarity = value,
        Field::MaxCount => draft.max_count = value,
        Field::CanUse => draft.can_use = value,
        Field::Desc => draft.desc = value,
        Field::Effect => draft.effect = value,
        Field::Interaction => draft.interaction = value,
        Field::Asset => draft.asset = value,
        Field::Image => draft.image = value,
    }
}

fn plant_label(draft: &ConfigDraft) -> SharedString {
    if draft.plant_id <= 0 {
        return text_key("config.plant");
    }
    if draft.plant_name.is_empty() {
        SharedString::from(format!("#{}", draft.plant_id))
    } else {
        SharedString::from(format!("#{} {}", draft.plant_id, draft.plant_name))
    }
}

fn matched_plants(
    plants: Resource<Vec<Value>, String>,
    query: Signal<String>,
) -> Vec<(i64, String)> {
    plants.get().map(|rows| plant_matches(&rows, &query.get())).unwrap_or_default()
}

fn plant_matches(plants: &[Value], query: &str) -> Vec<(i64, String)> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    plants
        .iter()
        .filter_map(|plant| {
            let id = plant.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
            let name = plant.get("name").and_then(Value::as_str).unwrap_or("");
            let haystack = format!("{name} {id}").to_lowercase();
            if !haystack.contains(&query) {
                return None;
            }
            let label = if name.is_empty() { format!("#{id}") } else { name.to_owned() };
            Some((id, label))
        })
        .take(8)
        .collect()
}

fn set_text(target: &mut String, value: &Value, keys: &[&str]) {
    if let Some(text) = keys.iter().find_map(|key| value.get(*key).and_then(json_text)) {
        *target = text;
    }
}

fn json_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn visible_values(catalog: Resource<Vec<Value>, String>, query: Signal<String>) -> Vec<Value> {
    catalog.get().map(|rows| filter_values(&rows, &query.get())).unwrap_or_default()
}

fn filter_values(rows: &[Value], query: &str) -> Vec<Value> {
    let query = query.trim().to_lowercase();
    rows.iter()
        .filter(|row| {
            let Some(item) = catalog_row_from(row) else {
                return false;
            };
            query.is_empty()
                || item.name.to_lowercase().contains(&query)
                || item.id.to_string().contains(&query)
        })
        .cloned()
        .collect()
}

async fn load_catalog(kind: CatalogKind) -> Result<Vec<Value>, String> {
    let value = session::session()
        .runtime
        .spawn_blocking(move || match kind {
            CatalogKind::Seeds => qq_farm_app::config::list_seeds(),
            CatalogKind::Fruits => qq_farm_app::config::list_fruits(),
            CatalogKind::Items => qq_farm_app::config::list_items(),
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(value.as_array().cloned().unwrap_or_default())
}

async fn load_plants() -> Result<Vec<Value>, String> {
    let value = session::session()
        .runtime
        .spawn_blocking(qq_farm_app::config::list_plants)
        .await
        .map_err(|error| error.to_string())?;
    Ok(value.as_array().cloned().unwrap_or_default())
}

#[cfg(test)]
fn catalog_rows(value: &Value) -> Vec<CatalogRow> {
    value
        .as_array()
        .map(|rows| rows.iter().filter_map(catalog_row_from).collect())
        .unwrap_or_default()
}

fn catalog_row_from(value: &Value) -> Option<CatalogRow> {
    let id = value
        .get("seedId")
        .or_else(|| value.get("id"))
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    Some(CatalogRow {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name.to_owned() },
        price: value.get("price").and_then(Value::as_i64).unwrap_or(0),
    })
}

#[cfg(test)]
fn filter_rows(rows: &[CatalogRow], query: &str) -> Vec<CatalogRow> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return rows.to_vec();
    }
    rows.iter()
        .filter(|row| {
            row.name.to_lowercase().contains(&query) || row.id.to_string().contains(&query)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        catalog_rows, delete_targets, draft_from, filter_rows, image_supported, new_draft,
        plant_matches, prepare, CatalogKind, CatalogRow, GameConfigKind, Prepared,
    };

    #[test]
    fn rows_prefer_seed_id_and_skip_blanks() {
        let rows = catalog_rows(&json!([
            { "seedId": 12, "id": 99, "name": "萝卜", "price": 8 },
            { "id": 0, "name": "drop" },
            { "id": 4, "name": "", "price": 3 }
        ]));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, 12);
        assert_eq!(rows[0].price, 8);
        assert_eq!(rows[1].name, "#4");
    }

    #[test]
    fn filter_matches_name_or_id() {
        let rows = vec![
            CatalogRow { id: 12, name: "萝卜".to_owned(), price: 1 },
            CatalogRow { id: 40, name: "白菜".to_owned(), price: 1 },
        ];
        assert_eq!(filter_rows(&rows, "  ").len(), 2);
        assert_eq!(filter_rows(&rows, "白")[0].id, 40);
        assert_eq!(filter_rows(&rows, "12")[0].name, "萝卜");
    }

    #[test]
    fn seed_draft_keeps_sale_growth_and_plant() {
        let draft = draft_from(
            CatalogKind::Seeds,
            &json!({
                "seedId": 12,
                "name": "萝卜",
                "requiredLevel": 5,
                "plantId": 88,
                "price": 8,
                "priceId": 1,
                "seasons": 2,
                "exp": 9,
                "size": 1,
                "harvestCount": 3,
                "growPhases": "成熟:0;"
            }),
        );
        assert_eq!(draft.id, "12");
        assert_eq!(draft.level, "5");
        assert_eq!(draft.fruit_count, "3");
        assert_eq!(draft.plant_id, 88);
        assert_eq!(draft.grow_phases, "成熟:0;");
        let Prepared::Seed(write) = prepare(CatalogKind::Seeds, &draft).expect("seed") else {
            panic!("seed write");
        };
        assert_eq!(write.seed_id, Some(12));
        assert_eq!(write.land_level_need, Some(5));
        assert_eq!(write.fruit_count, Some(3));
        assert_eq!(write.price, Some(8));
        assert!(new_draft(CatalogKind::Seeds).grow_phases.contains("成熟"));
        assert!(draft_from(CatalogKind::Seeds, &json!({"seedId": 1, "name": "甲"}))
            .grow_phases
            .is_empty());
    }

    #[test]
    fn prepare_rejects_blank_name_bad_numbers_and_missing_links() {
        let mut seed = new_draft(CatalogKind::Seeds);
        seed.id = "4".into();
        assert_eq!(prepare(CatalogKind::Seeds, &seed).unwrap_err(), "config.nameRequired");
        seed.name = "甲".into();
        seed.price = "-1".into();
        assert_eq!(prepare(CatalogKind::Seeds, &seed).unwrap_err(), "config.number");
        seed.price = "1".into();
        seed.id = "0".into();
        assert_eq!(prepare(CatalogKind::Seeds, &seed).unwrap_err(), "config.idRequired");

        let mut fruit = new_draft(CatalogKind::Fruits);
        fruit.name = "果".into();
        assert_eq!(prepare(CatalogKind::Fruits, &fruit).unwrap_err(), "config.needPlant");
        fruit.plant_id = 9;
        let Prepared::Fruit(write) = prepare(CatalogKind::Fruits, &fruit).expect("fruit") else {
            panic!("fruit write");
        };
        assert_eq!(write.plant_id, Some(9));
        assert_eq!(write.id, None);

        let mut item = new_draft(CatalogKind::Items);
        item.name = "肥".into();
        item.item_type = 0;
        assert_eq!(prepare(CatalogKind::Items, &item).unwrap_err(), "config.typeRequired");
        item.item_type = 7;
        item.id = "15".into();
        let Prepared::Item(write) = prepare(CatalogKind::Items, &item).expect("item") else {
            panic!("item write");
        };
        assert_eq!(write.item_type, Some(7));
        assert_eq!(write.id, Some(15));
    }

    #[test]
    fn delete_targets_cover_seed_plant_and_item() {
        assert_eq!(
            delete_targets(CatalogKind::Seeds, 12, 88),
            vec![(GameConfigKind::Plant, 88), (GameConfigKind::Item, 12)]
        );
        assert_eq!(
            delete_targets(CatalogKind::Seeds, 12, 0),
            vec![(GameConfigKind::Plant, 12), (GameConfigKind::Item, 12)]
        );
        assert_eq!(delete_targets(CatalogKind::Fruits, 6, 0), vec![(GameConfigKind::Item, 6)]);
        assert!(image_supported("cover.PNG"));
        assert!(!image_supported("cover.gif"));
        assert_eq!(
            plant_matches(
                &[json!({"id": 3, "name": "萝卜"}), json!({"id": 8, "name": "白菜"})],
                "萝",
            ),
            vec![(3, "萝卜".to_owned())]
        );
        assert!(plant_matches(&[json!({"id": 3, "name": "萝卜"})], "  ").is_empty());
    }
}
