use super::selectors::{self, GameConfigFilterSelect, GameConfigItemTypeSelect};
use crate::desktop::FarmDesktop;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use std::collections::{HashMap, HashSet};

pub(crate) struct GameConfigState {
    pub(crate) id: Entity<InputState>,
    pub(crate) name: Entity<InputState>,
    pub(crate) level: Entity<InputState>,
    pub(crate) desc: Entity<TextareaState>,
    pub(crate) asset: Entity<InputState>,
    pub(crate) image_path: Entity<InputState>,
    pub(crate) extra_fields: HashMap<String, Entity<InputState>>,
    pub(crate) extra_textareas: HashMap<String, Entity<TextareaState>>,
    pub(crate) plants: Vec<serde_json::Value>,
    pub(crate) item_types: Vec<(i64, String)>,
    pub(crate) form_open: bool,
    pub(crate) kind: String,
    pub(crate) record: serde_json::Value,
    pub(crate) form_error: Option<String>,
    pub(crate) pending_delete: Option<(String, i64)>,
    pub(crate) pending_batch_delete: bool,
    pub(crate) selected_ids: HashSet<(String, i64)>,
    pub(crate) success_feedback: Option<String>,
    pub(crate) feedback_generation: u64,
    pub(crate) columns: HashMap<String, Vec<super::columns::GameConfigColumnSetting>>,
    pub(crate) column_picker_open: bool,
    pub(crate) seasons: Option<i64>,
    pub(crate) rarity: Option<i64>,
    pub(crate) item_type: Option<i64>,
    pub(crate) season_selector: Entity<GameConfigFilterSelect>,
    pub(crate) rarity_selector: Entity<GameConfigFilterSelect>,
    pub(crate) item_type_selector: Entity<GameConfigFilterSelect>,
    pub(crate) item_type_form_selector: Entity<GameConfigItemTypeSelect>,
    pub(crate) plant_form_selector: Entity<selectors::GameConfigPlantSelect>,
    pub(crate) plant_id: Option<i64>,
    pub(crate) page: usize,
    pub(crate) page_size: usize,
    pub(crate) search_open: bool,
    pub(crate) applied_keyword: String,
    pub(crate) applied_seasons: Option<i64>,
    pub(crate) applied_rarity: Option<i64>,
    pub(crate) applied_item_type: Option<i64>,
}

impl GameConfigState {
    pub(crate) fn reset_for_page_navigation(&mut self) {
        self.form_open = false;
        self.form_error = None;
        self.pending_delete = None;
        self.pending_batch_delete = false;
        self.success_feedback = None;
        self.feedback_generation = self.feedback_generation.wrapping_add(1);
        self.selected_ids.clear();
        self.seasons = None;
        self.rarity = None;
        self.item_type = None;
        self.page = 0;
        self.page_size = 10;
        self.search_open = false;
        self.applied_keyword.clear();
        self.applied_seasons = None;
        self.applied_rarity = None;
        self.applied_item_type = None;
        self.column_picker_open = false;
        for section in ["seeds", "fruits", "items"] {
            self.columns
                .insert(section.to_string(), super::columns::default_game_config_columns(section));
        }
    }

    pub(crate) fn reset_for_category_change(&mut self, section: &str) {
        self.pending_delete = None;
        self.pending_batch_delete = false;
        self.success_feedback = None;
        self.feedback_generation = self.feedback_generation.wrapping_add(1);
        self.selected_ids.clear();
        self.seasons = None;
        self.rarity = None;
        self.item_type = None;
        self.page = 0;
        self.applied_keyword.clear();
        self.applied_seasons = None;
        self.applied_rarity = None;
        self.applied_item_type = None;
        self.column_picker_open = false;
        self.columns
            .insert(section.to_string(), super::columns::default_game_config_columns(section));
    }

    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let season_options = selectors::season_options(language);
        let season_selector = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(season_options),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &season_selector,
            |this, _, event: &SelectEvent<SearchableVec<selectors::GameConfigFilterOption>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.game_config.seasons = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        let rarity_options = selectors::rarity_options(language);
        let rarity_selector = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(rarity_options),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &rarity_selector,
            |this, _, event: &SelectEvent<SearchableVec<selectors::GameConfigFilterOption>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.game_config.rarity = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        let item_type_options = selectors::item_type_options(&serde_json::Value::Null, language);
        let item_type_selector = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(item_type_options),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &item_type_selector,
            |this, _, event: &SelectEvent<SearchableVec<selectors::GameConfigFilterOption>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.game_config.item_type = *value;
                    cx.notify();
                }
            },
        )
        .detach();

        let item_type_form_selector = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<selectors::GameConfigItemTypeOption>::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(
            &item_type_form_selector,
            |_, _, event: &SelectEvent<SearchableVec<selectors::GameConfigItemTypeOption>>, cx| {
                if matches!(event, SelectEvent::Confirm(Some(_))) {
                    cx.notify();
                }
            },
        )
        .detach();

        let plant_form_selector = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<selectors::GameConfigPlantOption>::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(
            &plant_form_selector,
            |this, _, event: &SelectEvent<SearchableVec<selectors::GameConfigPlantOption>>, cx| {
                if let SelectEvent::Confirm(Some(plant_id)) = event {
                    this.game_config.plant_id = Some(*plant_id);
                    let name = this
                        .game_config
                        .plants
                        .iter()
                        .find(|plant| super::form::plant_id(plant) == Some(*plant_id))
                        .and_then(|plant| plant.get("name"))
                        .and_then(serde_json::Value::as_str)
                        .map(|name| name.split(" (").next().unwrap_or(name).to_string());
                    if let Some(name) =
                        name.filter(|_| this.game_config.name.read(cx).value().trim().is_empty())
                    {
                        let name_state = this.game_config.name.clone();
                        if let Some(handle) = this.window_handle {
                            let _ = handle.update(cx, |_, window, cx| {
                                name_state
                                    .update(cx, |state, cx| state.set_value(&name, window, cx));
                            });
                        }
                    }
                    cx.notify();
                }
            },
        )
        .detach();

        let id = cx.new(|cx| {
            let mut state =
                InputState::new(window, cx).placeholder(crate::i18n::text(language, "编号", "ID"));
            state.set_min(Some(1.), window, cx);
            state
        });
        let name = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::text(language, "名称", "Name"))
        });
        let level = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(crate::i18n::text(
                language,
                "等级要求",
                "Required level",
            ));
            state.set_min(Some(0.), window, cx);
            state
        });
        let desc = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(crate::i18n::text(language, "描述", "Description"))
                .auto_grow(2, 4)
        });
        let asset = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::text(
                language,
                "资源名",
                "Asset name",
            ))
        });
        let image_path = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::text(
                language,
                "图片文件路径（可选）",
                "Image path (optional)",
            ))
        });
        let extra_fields = crate::desktop::game_config::form::EXTRA_FIELDS
            .iter()
            .filter(|(key, _)| !matches!(*key, "growPhases" | "effectDesc"))
            .map(|(key, placeholder)| {
                (
                    key.to_string(),
                    cx.new(|cx| {
                        let mut state = InputState::new(window, cx)
                            .placeholder(super::labels::form_field(language, placeholder));
                        if matches!(*key, "price" | "fruitCount" | "exp" | "maxCount") {
                            state.set_min(Some(0.), window, cx);
                        }
                        state
                    }),
                )
            })
            .collect();
        let extra_textareas = [("growPhases", "生长阶段"), ("effectDesc", "效果描述")]
            .into_iter()
            .map(|(key, placeholder)| {
                (
                    key.to_string(),
                    cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .placeholder(super::labels::form_field(language, placeholder))
                            .auto_grow(2, 4)
                    }),
                )
            })
            .collect();
        let state = Self {
            id,
            name,
            level,
            desc,
            asset,
            image_path,
            extra_fields,
            extra_textareas,
            plants: Vec::new(),
            item_types: Vec::new(),
            form_open: false,
            kind: "seed".into(),
            record: serde_json::Value::Null,
            form_error: None,
            pending_delete: None,
            pending_batch_delete: false,
            selected_ids: HashSet::new(),
            success_feedback: None,
            feedback_generation: 0,
            columns: ["seeds", "fruits", "items"]
                .into_iter()
                .map(|section| {
                    (section.to_string(), super::columns::default_game_config_columns(section))
                })
                .collect(),
            column_picker_open: false,
            seasons: None,
            rarity: None,
            item_type: None,
            season_selector,
            rarity_selector,
            item_type_selector,
            item_type_form_selector,
            plant_form_selector,
            plant_id: None,
            page: 0,
            page_size: 10,
            search_open: false,
            applied_keyword: String::new(),
            applied_seasons: None,
            applied_rarity: None,
            applied_item_type: None,
        };
        cx.observe(&state.desc, |_, _, cx| cx.notify()).detach();
        for input in state.extra_fields.values() {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        for input in state.extra_textareas.values() {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        state
    }
}
