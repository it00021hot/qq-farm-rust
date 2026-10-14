use crate::desktop::{FarmDesktop, Page};
use crate::pages;

pub(in crate::desktop) fn detail_gid_for_refresh(
    detail: Option<&(i64, serde_json::Value)>,
    detail_loading_gid: Option<i64>,
) -> Option<i64> {
    detail.map(|(gid, _)| *gid).or(detail_loading_gid)
}

fn merge_page_snapshot(
    previous: serde_json::Value,
    incoming: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = previous.as_object().cloned().unwrap_or_default();
    if let Some(incoming) = incoming.as_object() {
        merged.extend(incoming.clone());
    }
    serde_json::Value::Object(merged)
}

impl FarmDesktop {
    pub(in crate::desktop) fn apply_friends_snapshot(&mut self, incoming: serde_json::Value) {
        self.page_raw = merge_page_snapshot(self.page_raw.clone(), &incoming);
        let mapped = pages::page_data(Page::Friends, self.page_raw.clone());
        self.summary = mapped.summary;
        self.rows = mapped.rows;

        let available = self
            .page_raw
            .pointer("/interactionItems/items")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                item.get("itemId")
                    .or_else(|| item.get("id"))
                    .and_then(|id| id.as_i64().or_else(|| id.as_str()?.trim().parse::<i64>().ok()))
            })
            .collect::<Vec<_>>();
        if !self.friends.selected_interaction_item_id.is_some_and(|id| available.contains(&id)) {
            self.friends.selected_interaction_item_id = available.first().copied();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{detail_gid_for_refresh, merge_page_snapshot};

    #[test]
    fn friend_list_refresh_reloads_the_expanded_or_pending_detail() {
        assert_eq!(detail_gid_for_refresh(Some(&(12, serde_json::json!({}))), Some(13)), Some(12));
        assert_eq!(detail_gid_for_refresh(None, Some(13)), Some(13));
        assert_eq!(detail_gid_for_refresh(None, None), None);
    }

    #[test]
    fn lazy_friend_section_refresh_preserves_other_loaded_tabs() {
        let previous = serde_json::json!({"friends":[1], "records":[2]});
        let incoming = serde_json::json!({"friends":[3]});
        let merged = merge_page_snapshot(previous, &incoming);
        assert_eq!(merged["friends"], serde_json::json!([3]));
        assert_eq!(merged["records"], serde_json::json!([2]));
    }
}
