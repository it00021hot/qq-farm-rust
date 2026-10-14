use super::super::*;
use super::search::account_matches_keyword;

impl FarmDesktop {
    fn filtered_account_records(&self) -> Vec<serde_json::Value> {
        let records = self
            .page_raw
            .get("accounts")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        filter_account_records(
            records,
            &self.accounts.keyword_applied,
            self.accounts.platform_filter.as_deref(),
            self.accounts.running_filter,
            self.accounts.auth_filter,
        )
    }

    pub(in crate::desktop) fn selected_visible_account_ids(&self) -> Vec<String> {
        let records = self.filtered_account_records();
        let start = self.accounts.page.saturating_mul(self.accounts.page_size);
        records
            .iter()
            .skip(start)
            .take(self.accounts.page_size)
            .map(|record| account_value(record, &["id"]))
            .filter(|id| self.accounts.selected.contains(id))
            .collect()
    }

    pub(in crate::desktop) fn confirm_account_batch_delete(&mut self, cx: &mut Context<Self>) {
        if self.loading || !self.accounts.pending_batch_delete {
            return;
        }
        self.accounts.pending_batch_delete = false;
        let ids = self.selected_visible_account_ids();
        if !ids.is_empty() {
            self.delete_accounts(ids, cx);
        }
    }

    pub(in crate::desktop) fn reset_account_filters(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Vue's search-card Reset restores the form model only. Results keep the
        // last applied query until the user presses Search again.
        self.accounts.keyword.update(cx, |state, cx| state.set_value("", window, cx));
        self.accounts.platform_draft = None;
        self.accounts.running_draft = None;
        self.accounts.auth_draft = None;
        self.sync_account_filter_selectors(window, cx);
        cx.notify();
    }

    pub(in crate::desktop) fn apply_account_filters(&mut self, cx: &mut Context<Self>) {
        self.accounts.keyword_applied = self.accounts.keyword.read(cx).value().to_string();
        self.accounts.platform_filter = self.accounts.platform_draft.clone();
        self.accounts.running_filter = self.accounts.running_draft;
        self.accounts.auth_filter = self.accounts.auth_draft;
        self.accounts.page = 0;
        cx.notify();
    }
    pub(in crate::desktop) fn render_accounts_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut filtered = self.filtered_account_records();
        let total = filtered.len();
        let page_size = self.accounts.page_size;
        let page_count = total.div_ceil(page_size).max(1);
        let page = self.accounts.page.min(page_count - 1);
        let start = page * page_size;
        let page_records: Vec<_> =
            filtered.drain(start.min(total)..(start + page_size).min(total)).collect();
        let page_ids: Vec<String> = page_records
            .iter()
            .map(|record| account_value(record, &["id"]))
            .filter(|id| !id.is_empty())
            .collect();
        let all_selected =
            !page_ids.is_empty() && page_ids.iter().all(|id| self.accounts.selected.contains(id));
        let start_index = start + 1;
        let end_index = (start + page_size).min(total);
        div()
            .relative()
            .v_flex()
            .gap_4()
            .flex_1()
            .min_h_0()
            .child(self.render_account_filters(cx))
            .when_some(self.accounts.success_feedback.as_ref(), |panel, message| {
                let feedback = div()
                    .id("account-success-feedback")
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .px_4()
                    .py_3()
                    .text_color(cx.theme().success)
                    .child(message.clone());
                #[cfg(test)]
                let feedback = feedback.test_support();
                panel.child(feedback)
            })
            .child(self.render_account_table(
                window,
                cx,
                page_records,
                page_ids,
                all_selected,
                total,
                start_index,
                end_index,
                page,
                page_count,
            ))
    }
}

fn filter_account_records(
    records: Vec<serde_json::Value>,
    keyword: &str,
    platform_filter: Option<&str>,
    running_filter: Option<i64>,
    auth_filter: Option<bool>,
) -> Vec<serde_json::Value> {
    records
        .into_iter()
        .filter(|record| {
            let platform = account_value(record, &["platform"]);
            let run_status = if record.get("running").and_then(serde_json::Value::as_bool)
                == Some(true)
                || record.get("runStatus").and_then(serde_json::Value::as_i64) == Some(1)
            {
                1
            } else {
                0
            };
            let authorized =
                record.get("wxAuthorized").and_then(serde_json::Value::as_bool).unwrap_or(false);
            account_matches_keyword(record, keyword)
                && platform_filter.is_none_or(|value| value == platform)
                && running_filter.is_none_or(|value| value == run_status)
                && auth_filter.is_none_or(|value| value == authorized)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::filter_account_records;
    use serde_json::json;

    #[test]
    fn account_filters_preserve_the_source_order_like_vue_pagination() {
        let records = vec![
            json!({"id":"10","name":"first","platform":"qq","running":false,"wxAuthorized":true}),
            json!({"id":"legacy-b","name":"second","platform":"wx","running":false,"wxAuthorized":false}),
            json!({"id":"2","name":"third","platform":"qq","running":true,"wxAuthorized":true}),
        ];

        let filtered = filter_account_records(records, "", None, Some(0), None);
        let ids = filtered
            .iter()
            .filter_map(|record| record.get("id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["10", "legacy-b"]);
    }

    #[test]
    fn error_run_status_is_offered_but_matches_no_normalized_account_row() {
        let records = vec![
            json!({"id":"stopped", "running":false}),
            json!({"id":"running", "running":true}),
            // The Vue adapter maps anything except running=1 to stopped=0.
            json!({"id":"error-like", "running":false, "runStatus":2}),
        ];

        assert!(filter_account_records(records, "", None, Some(2), None).is_empty());
    }
}
