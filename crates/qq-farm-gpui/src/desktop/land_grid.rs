use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::desktop) struct LandPlacement {
    pub(in crate::desktop) col: usize,
    pub(in crate::desktop) row: usize,
    pub(in crate::desktop) col_span: usize,
    pub(in crate::desktop) row_span: usize,
}

fn number(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
        .unwrap_or_default()
}

fn land_ids(land: &Value) -> Vec<i64> {
    let ids = land
        .get("occupiedLandIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| number(Some(value)))
        .filter(|id| *id > 0)
        .collect::<Vec<_>>();
    if ids.is_empty() {
        vec![number(land.get("id")).max(1)]
    } else {
        ids
    }
}

fn is_merged(land: &Value) -> bool {
    let mut ids = land_ids(land);
    ids.sort_unstable();
    ids.dedup();
    !land.get("occupiedByMaster").and_then(Value::as_bool).unwrap_or(false)
        && (ids.len() > 1 || number(land.get("plantSize")) > 1)
}

pub(in crate::desktop) fn visible_lands(lands: &[Value]) -> Vec<&Value> {
    let mut covered = std::collections::HashSet::new();
    for land in lands {
        if !is_merged(land) {
            continue;
        }
        let master = number(land.get("id"));
        covered.extend(land_ids(land).into_iter().filter(|id| *id != master));
    }
    lands
        .iter()
        .filter(|land| {
            !land.get("occupiedByMaster").and_then(Value::as_bool).unwrap_or(false)
                && !covered.contains(&number(land.get("id")))
        })
        .collect()
}

pub(in crate::desktop) fn land_placement(land: &Value) -> LandPlacement {
    let ids = land_ids(land);
    let coordinates = ids
        .iter()
        .map(|id| {
            let safe = (*id).max(1) as usize - 1;
            (safe % 4 + 1, safe / 4 + 1)
        })
        .collect::<Vec<_>>();
    let min_col = coordinates.iter().map(|(col, _)| *col).min().unwrap_or(1);
    let max_col = coordinates.iter().map(|(col, _)| *col).max().unwrap_or(min_col);
    let min_row = coordinates.iter().map(|(_, row)| *row).min().unwrap_or(1);
    let max_row = coordinates.iter().map(|(_, row)| *row).max().unwrap_or(min_row);
    LandPlacement {
        col: min_col,
        row: min_row,
        col_span: max_col - min_col + 1,
        row_span: max_row - min_row + 1,
    }
}

pub(in crate::desktop) fn land_id_label(land: &Value) -> String {
    let id = number(land.get("id"));
    if !is_merged(land) {
        return format!("#{id}");
    }
    let mut ids = land_ids(land);
    ids.sort_unstable();
    ids.dedup();
    if ids.len() <= 1 {
        format!("#{id}")
    } else {
        format!("#{}-{}", ids[0], ids[ids.len() - 1])
    }
}
