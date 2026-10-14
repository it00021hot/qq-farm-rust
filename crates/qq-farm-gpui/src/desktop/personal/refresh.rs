use crate::desktop::{FarmDesktop, Page};
use crate::pages::{self, PageData};
use gpui_kit::Context;

fn merge_page_snapshot(
    previous: serde_json::Value,
    incoming: &serde_json::Value,
) -> serde_json::Value {
    if incoming.get("connected").and_then(serde_json::Value::as_bool) == Some(false) {
        return incoming.clone();
    }

    let mut merged = previous.as_object().cloned().unwrap_or_default();
    if let Some(incoming_object) = incoming.as_object() {
        merged.extend(incoming_object.clone());
        if let Some(errors) =
            incoming_object.get("sectionErrors").and_then(serde_json::Value::as_object)
        {
            for (section, error) in errors {
                if !error.is_null()
                    && incoming_object.get(section).is_some_and(serde_json::Value::is_null)
                {
                    if let Some(previous_value) = previous.get(section) {
                        merged.insert(section.clone(), previous_value.clone());
                    }
                }
            }
        }
    }
    serde_json::Value::Object(merged)
}

impl FarmDesktop {
    pub(in crate::desktop) fn apply_personal_snapshot(
        &mut self,
        incoming: PageData,
        cx: &mut Context<Self>,
    ) {
        self.page_raw = merge_page_snapshot(self.page_raw.clone(), &incoming.raw);
        let mapped = pages::page_data(Page::Personal, self.page_raw.clone());
        self.summary = mapped.summary;
        self.rows = mapped.rows;

        if self.page_section == "lands" {
            self.farm.lands.refreshing = false;
            self.farm.lands.error = None;
            self.start_personal_lands_clock(cx);
        }
        if let Some(handle) = self.window_handle {
            let _ = handle.update(cx, |_, window, cx| {
                self.sync_personal_interaction_picker(window, cx);
                self.sync_personal_pet_food_inputs(window, cx);
            });
        }
        self.farm.bag.selected_ids.clear();
        if self.page_section == "bag" {
            self.farm.bag.refreshing = false;
            self.farm.bag.error = None;
            self.start_personal_bag_clock(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::merge_page_snapshot;

    #[test]
    fn failed_personal_sections_keep_the_last_successful_data_until_recovery() {
        let previous = serde_json::json!({"lands":[1], "bag":[2]});
        let incoming = serde_json::json!({
            "lands": null,
            "sectionErrors": {"lands":"temporary error"}
        });
        let merged = merge_page_snapshot(previous, &incoming);
        assert_eq!(merged["lands"], serde_json::json!([1]));
        assert_eq!(merged["bag"], serde_json::json!([2]));
        assert_eq!(merged["sectionErrors"]["lands"], "temporary error");
    }

    #[test]
    fn disconnected_personal_page_replaces_the_stale_snapshot() {
        let previous = serde_json::json!({"connected":true, "lands":[1]});
        let incoming = serde_json::json!({"connected":false, "lands":null});
        assert_eq!(merge_page_snapshot(previous, &incoming), incoming);
    }
}
