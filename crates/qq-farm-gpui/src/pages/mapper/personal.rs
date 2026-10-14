use super::common::append_entity_rows;
use crate::pages::DisplayRow;
use serde_json::Value;
pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    if let Some(object) = raw.as_object() {
        for (section, label) in [
            ("lands", "土地"),
            ("bag", "仓库"),
            ("dailyGifts", "日常任务"),
            ("pets", "宠物"),
            ("illustrated", "图鉴"),
            ("interactionItems", "互动道具"),
        ] {
            if let Some(value) = object.get(section) {
                append_entity_rows(value, section, label, &mut rows);
            }
        }
    }
    rows
}
