use serde_json::Value;

use crate::pages::{DisplayRow, Page};

use super::entity::entity_row;
use super::format::{detail_at, humanize_key, row_section, state_at, string_at, value_string};

pub(super) fn append_entity_rows(
    value: &Value,
    section: &str,
    label: &str,
    rows: &mut Vec<DisplayRow>,
) {
    match value {
        Value::Array(values) => {
            for (index, entry) in values.iter().enumerate() {
                if entry.is_object() {
                    rows.push(entity_row(entry, &format!("{section}/{index}"), label));
                } else if !entry.is_null() {
                    rows.push(DisplayRow {
                        id: format!("{section}/{index}"),
                        section: section.split('/').next().unwrap_or(section).into(),
                        title: value_string(entry),
                        detail: label.into(),
                        state: String::new(),
                        record: entry.clone(),
                    });
                }
            }
        }
        Value::Object(object) => {
            let mut found = false;
            for (key, nested) in object {
                if nested.is_array() || nested.is_object() {
                    let start = rows.len();
                    append_entity_rows(nested, &format!("{section}/{key}"), label, rows);
                    found |= rows.len() > start;
                }
            }
            if !found && !object.is_empty() {
                rows.push(entity_row(value, section, label));
            }
        }
        _ => {}
    }
}

pub(super) fn append_value_rows(
    value: &Value,
    page: Page,
    prefix: &str,
    rows: &mut Vec<DisplayRow>,
) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                if item.is_object() {
                    rows.push(DisplayRow {
                        id: string_at(
                            item,
                            &["id", "gid", "seedId", "offerId"],
                            &format!("{}-{index}", prefix),
                        ),
                        section: row_section(page, prefix),
                        title: string_at(
                            item,
                            &["name", "title", "nick", "desc", "msg", "id"],
                            "条目",
                        ),
                        detail: detail_at(item),
                        state: state_at(item),
                        record: item.clone(),
                    });
                } else if !item.is_null() {
                    rows.push(DisplayRow {
                        id: format!("{}-{index}", prefix),
                        section: row_section(page, prefix),
                        title: value_string(item),
                        detail: prefix.to_string(),
                        state: String::new(),
                        record: item.clone(),
                    });
                }
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                if item.is_null() {
                    continue;
                }
                if item.is_object() || item.is_array() {
                    append_value_rows(item, page, &format!("{prefix}{key}/"), rows);
                } else {
                    let detail = if prefix.is_empty() {
                        String::new()
                    } else {
                        prefix.trim_end_matches('/').replace('/', " · ")
                    };
                    rows.push(DisplayRow {
                        id: format!("{prefix}{key}"),
                        section: row_section(page, prefix),
                        title: humanize_key(key),
                        detail: if detail.is_empty() {
                            value_string(item)
                        } else {
                            format!("{detail} · {}", value_string(item))
                        },
                        state: String::new(),
                        record: item.clone(),
                    });
                }
            }
        }
        _ => {
            if !value.is_null() {
                rows.push(DisplayRow {
                    id: prefix.to_string(),
                    section: row_section(page, prefix),
                    title: humanize_key(prefix.trim_end_matches('/')),
                    detail: value_string(value),
                    state: String::new(),
                    record: value.clone(),
                });
            }
        }
    }
    if page == Page::Dashboard && rows.len() > 81 {
        rows.truncate(81);
    }
}
