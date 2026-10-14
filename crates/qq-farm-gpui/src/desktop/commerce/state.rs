use crate::desktop::FarmDesktop;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use std::collections::HashMap;

pub(crate) struct CommerceState {
    pub(crate) mall_slot_type: i32,
    pub(crate) catalog_account_id: Option<String>,
    pub(crate) catalog_snapshots: HashMap<i32, serde_json::Value>,
    pub(crate) purchase_count: Entity<InputState>,
    pub(crate) pending_purchase: Option<serde_json::Value>,
    pub(crate) purchase_pending: bool,
    pub(crate) purchase_generation: u64,
    pub(crate) purchase_dialog_open: bool,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) notice_generation: u64,
}

impl CommerceState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let purchase_count = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(crate::i18n::text(language, "数量", "Quantity"));
            state.set_value("1", window, cx);
            state.set_min(Some(1.), window, cx);
            state.set_max(Some(99.), window, cx);
            state
        });
        Self {
            mall_slot_type: 1,
            catalog_account_id: None,
            catalog_snapshots: HashMap::new(),
            purchase_count,
            pending_purchase: None,
            purchase_pending: false,
            purchase_generation: 0,
            purchase_dialog_open: false,
            notice: None,
            notice_generation: 0,
        }
    }

    pub(crate) fn invalidate_purchase(&mut self) {
        self.purchase_generation = self.purchase_generation.wrapping_add(1);
        self.purchase_pending = false;
        self.pending_purchase = None;
        self.purchase_dialog_open = false;
        self.notice_generation = self.notice_generation.wrapping_add(1);
        self.notice = None;
    }

    pub(crate) fn cached_catalog(
        &self,
        account_id: &str,
        slot_type: i32,
    ) -> Option<&serde_json::Value> {
        (self.catalog_account_id.as_deref() == Some(account_id))
            .then(|| self.catalog_snapshots.get(&slot_type))
            .flatten()
    }

    pub(crate) fn store_catalog(
        &mut self,
        account_id: &str,
        slot_type: i32,
        snapshot: serde_json::Value,
    ) {
        if self.catalog_account_id.as_deref() != Some(account_id) {
            self.catalog_account_id = Some(account_id.to_string());
            self.catalog_snapshots.clear();
        }
        self.catalog_snapshots.insert(slot_type, snapshot);
    }

    pub(crate) fn clear_catalogs(&mut self) {
        self.catalog_account_id = None;
        self.catalog_snapshots.clear();
    }
}
