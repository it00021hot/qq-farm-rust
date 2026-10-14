use crate::desktop::FarmDesktop;
use gpui_kit::component::combobox::{ComboboxEvent, ComboboxState};
use gpui_kit::component::input::InputState;
use gpui_kit::component::select::SearchableVec;
use gpui_kit::{AppContext as _, Context, Entity, Window};

pub(crate) struct AnalyticsState {
    pub(crate) level: Entity<InputState>,
    pub(crate) blacklist_selector: Entity<super::PlantBlacklistSelector>,
    pub(crate) level_is_user_override: bool,
    /// Last value the application wrote or observed, so `observe` notifications
    /// that do not change the text never register as a user edit.
    pub(crate) level_last_value: String,
    pub(crate) blacklist_feedback: Option<String>,
    pub(crate) blacklist_feedback_is_error: bool,
    pub(crate) blacklist_feedback_generation: u64,
    pub(crate) blacklist_save_pending: bool,
    pub(crate) blacklist_save_generation: u64,
}

impl AnalyticsState {
    pub(in crate::desktop) fn new(
        app: &qq_farm_app::session::AppContext,
        account_id: Option<&str>,
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let initial_level = account_id
            .map(|id| qq_farm_app::farm::panel_status_with_progress(app, id).level)
            .unwrap_or(1)
            .clamp(1, 200)
            .to_string();
        let observer_initial_level = initial_level.clone();
        let level = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(crate::i18n::text(
                language,
                "参考等级",
                "Reference level",
            ));
            state.set_value(initial_level, window, cx);
            state.set_min(Some(1.), window, cx);
            state.set_max(Some(200.), window, cx);
            state
        });
        let blacklist_selector = cx.new(|cx| {
            ComboboxState::new(
                SearchableVec::new(super::plant_blacklist_options(
                    &serde_json::Value::Null,
                    language,
                )),
                Vec::new(),
                window,
                cx,
            )
            .multiple(true)
            .searchable(true)
        });
        cx.subscribe(&blacklist_selector, |this, _, event, cx| {
            if let ComboboxEvent::Change(seed_ids) = event {
                this.replace_plant_blacklist(seed_ids.clone(), cx);
            }
        })
        .detach();
        let state = Self {
            level,
            blacklist_selector,
            level_is_user_override: false,
            level_last_value: observer_initial_level,
            blacklist_feedback: None,
            blacklist_feedback_is_error: false,
            blacklist_feedback_generation: 0,
            blacklist_save_pending: false,
            blacklist_save_generation: 0,
        };
        cx.observe(&state.level, |this, input, cx| {
            let value = input.read(cx).value().to_string();
            if this.analytics.level_last_value == value {
                return;
            }
            // Application syncs record their value first, so any other text
            // change is a user edit that overrides the account reference level.
            this.analytics.level_last_value = value;
            this.analytics.level_is_user_override = true;
            cx.notify();
        })
        .detach();
        state
    }
}

impl FarmDesktop {
    /// Applies a reference level that comes from application data rather than
    /// from typing, so the level observer keeps treating it as an automatic
    /// value instead of a user override.
    pub(in crate::desktop) fn apply_reference_level(
        &mut self,
        level: i64,
        window: &mut Window,
        cx: &mut impl gpui_kit::AppContext,
    ) {
        let value = level.clamp(1, 200).to_string();
        self.analytics.level_is_user_override = false;
        self.analytics.level_last_value = value.clone();
        self.analytics.level.update(cx, |state, cx| state.set_value(value.as_str(), window, cx));
    }

    /// Vue's analytics page re-syncs the reference level from the account's
    /// live level on every load (`syncStrategyLevel`), overwriting earlier
    /// edits; keep the same contract when a page snapshot arrives.
    pub(in crate::desktop) fn sync_analytics_level(
        &mut self,
        window: &mut Window,
        cx: &mut impl gpui_kit::AppContext,
    ) {
        let Some(level) = self
            .page_raw
            .get("accountLevel")
            .and_then(serde_json::Value::as_i64)
            .filter(|level| *level > 0)
        else {
            return;
        };
        self.apply_reference_level(level, window, cx);
    }
}
