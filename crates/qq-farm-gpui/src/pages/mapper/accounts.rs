use super::common::append_value_rows;
use crate::pages::{DisplayRow, Page};
use serde_json::Value;
pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    append_value_rows(raw, Page::Accounts, "", &mut rows);
    rows
}
