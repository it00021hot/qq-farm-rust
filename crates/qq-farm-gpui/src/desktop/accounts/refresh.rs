use crate::desktop::{account_value, FarmDesktop};
use std::collections::HashSet;

impl FarmDesktop {
    pub(in crate::desktop) fn reconcile_account_selection(&mut self) {
        let valid_ids: HashSet<String> = self
            .page_raw
            .get("accounts")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .map(|record| account_value(record, &["id"]))
            .collect();
        self.accounts.selected.retain(|id| valid_ids.contains(id));
        let available_pages = valid_ids.len().div_ceil(self.accounts.page_size.max(1)).max(1);
        self.accounts.page = self.accounts.page.min(available_pages - 1);
    }
}
