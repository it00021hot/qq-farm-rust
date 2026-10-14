use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn set_bag_seed_priority(
        &mut self,
        seed_id: i64,
        delta: i64,
        cx: &mut Context<Self>,
    ) {
        let visible = self
            .page_raw
            .get("bagSeeds")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|seed| seed.get("seedId").and_then(serde_json::Value::as_i64))
            .collect::<Vec<_>>();
        let visible_set = visible.iter().copied().collect::<HashSet<_>>();
        let existing = self
            .settings
            .draft
            .get("bagSeedPriority")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_i64)
            .filter(|id| *id > 0)
            .fold(Vec::new(), |mut ids, id| {
                if !ids.contains(&id) {
                    ids.push(id);
                }
                ids
            });
        let mut visible_order =
            existing.iter().copied().filter(|id| visible_set.contains(id)).collect::<Vec<_>>();
        for id in &visible {
            if !visible_order.contains(id) {
                visible_order.push(*id);
            }
        }
        let Some(index) = visible_order.iter().position(|id| *id == seed_id) else { return };
        let target = index as i64 + delta;
        if !(0..visible_order.len() as i64).contains(&target) {
            return;
        }
        visible_order.swap(index, target as usize);

        let merged = merge_visible_bag_seed_order(existing, &visible, &visible_order);
        self.set_setting_draft_value(
            "bagSeedPriority",
            serde_json::Value::Array(merged.into_iter().map(serde_json::Value::from).collect()),
            cx,
        );
    }

    pub(in crate::desktop) fn drop_bag_seed_priority(
        &mut self,
        source_id: i64,
        target_id: i64,
        cx: &mut Context<Self>,
    ) {
        let visible = self
            .page_raw
            .get("bagSeeds")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|seed| seed.get("seedId").and_then(serde_json::Value::as_i64))
            .collect::<Vec<_>>();
        let existing = self
            .settings
            .draft
            .get("bagSeedPriority")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_i64)
            .collect::<Vec<_>>();
        let Some(merged) =
            move_visible_bag_seed_to_target(existing, &visible, source_id, target_id)
        else {
            return;
        };
        self.set_setting_draft_value(
            "bagSeedPriority",
            serde_json::Value::Array(merged.into_iter().map(serde_json::Value::from).collect()),
            cx,
        );
    }

    pub(in crate::desktop) fn set_bag_seed_fallback_strategy(
        &mut self,
        strategy: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.set_setting_draft_value("bagSeedFallbackStrategy", serde_json::json!(strategy), cx);
        self.reset_bag_seed_priority(cx);
        self.refresh_strategy_preview(cx);
    }

    pub(in crate::desktop) fn reset_bag_seed_priority(&mut self, cx: &mut Context<Self>) {
        let mut seeds = self
            .page_raw
            .get("bagSeeds")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let strategy = self
            .settings
            .draft
            .get("bagSeedFallbackStrategy")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("preferred");
        let preferred = self
            .settings
            .draft
            .get("preferredSeedId")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        let sort_by = match strategy {
            "max_exp" => Some("exp"),
            "max_fert_exp" => Some("fert"),
            "max_profit" => Some("profit"),
            "max_fert_profit" => Some("fert_profit"),
            _ => None,
        };
        let rank_order = sort_by
            .map(|sort| qq_farm_app::farm::analytics(Some(sort)))
            .and_then(|rows| rows.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                row.get("seedId").and_then(serde_json::Value::as_i64).map(|id| (id, index))
            })
            .collect::<HashMap<_, _>>();
        seeds.sort_by_key(|seed| {
            let id = seed.get("seedId").and_then(serde_json::Value::as_i64).unwrap_or_default();
            let level =
                seed.get("requiredLevel").and_then(serde_json::Value::as_i64).unwrap_or_default();
            match strategy {
                "preferred" => (i64::from(id != preferred), -level, id),
                "level" => (0, -level, id),
                _ => {
                    let rank = rank_order.get(&id).copied().unwrap_or(usize::MAX);
                    (rank as i64, -level, id)
                }
            }
        });
        let visible = seeds
            .iter()
            .filter_map(|seed| seed.get("seedId").and_then(serde_json::Value::as_i64))
            .collect::<Vec<_>>();
        let existing = self
            .settings
            .draft
            .get("bagSeedPriority")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_i64)
            .collect::<Vec<_>>();
        let merged = merge_visible_bag_seed_order(existing, &visible, &visible);
        self.set_setting_draft_value(
            "bagSeedPriority",
            serde_json::Value::Array(merged.into_iter().map(serde_json::Value::from).collect()),
            cx,
        );
    }
}
fn merge_visible_bag_seed_order(
    existing: Vec<i64>,
    visible: &[i64],
    visible_order: &[i64],
) -> Vec<i64> {
    let visible_set = visible.iter().copied().collect::<HashSet<_>>();
    let mut ordered_visible = visible_order
        .iter()
        .copied()
        .filter(|id| visible_set.contains(id))
        .fold(Vec::new(), |mut ids, id| {
            if !ids.contains(&id) {
                ids.push(id);
            }
            ids
        });
    for id in visible {
        if !ordered_visible.contains(id) {
            ordered_visible.push(*id);
        }
    }

    let mut merged = Vec::new();
    let mut visible_index = 0;
    for id in existing.into_iter().filter(|id| *id > 0) {
        if visible_set.contains(&id) {
            if let Some(replacement) = ordered_visible.get(visible_index) {
                if !merged.contains(replacement) {
                    merged.push(*replacement);
                }
            }
            visible_index += 1;
        } else if !merged.contains(&id) {
            merged.push(id);
        }
    }
    while let Some(id) = ordered_visible.get(visible_index) {
        if !merged.contains(id) {
            merged.push(*id);
        }
        visible_index += 1;
    }
    merged
}

fn move_visible_bag_seed_to_target(
    existing: Vec<i64>,
    visible: &[i64],
    source_id: i64,
    target_id: i64,
) -> Option<Vec<i64>> {
    if source_id == target_id {
        return None;
    }
    let visible_set = visible.iter().copied().collect::<HashSet<_>>();
    let mut visible_order = existing.iter().copied().filter(|id| visible_set.contains(id)).fold(
        Vec::new(),
        |mut ids, id| {
            if !ids.contains(&id) {
                ids.push(id);
            }
            ids
        },
    );
    for id in visible {
        if !visible_order.contains(id) {
            visible_order.push(*id);
        }
    }
    let source_index = visible_order.iter().position(|id| *id == source_id)?;
    let target_index = visible_order.iter().position(|id| *id == target_id)?;
    let source = visible_order.remove(source_index);
    let insert_index = if source_index < target_index { target_index - 1 } else { target_index };
    visible_order.insert(insert_index, source);
    Some(merge_visible_bag_seed_order(existing, visible, &visible_order))
}

#[cfg(test)]
mod tests {
    use super::{merge_visible_bag_seed_order, move_visible_bag_seed_to_target};

    #[test]
    fn bag_seed_reordering_keeps_offline_priority_entries() {
        assert_eq!(
            merge_visible_bag_seed_order(vec![10, 20, 30, 99], &[10, 20, 30], &[20, 10, 30]),
            vec![20, 10, 30, 99]
        );
    }

    #[test]
    fn dropping_bag_seed_on_another_row_matches_vue_order_and_keeps_hidden_seeds() {
        let existing = vec![10, 20, 30, 99];
        let visible = [10, 20, 30];
        assert_eq!(
            move_visible_bag_seed_to_target(existing.clone(), &visible, 10, 30),
            Some(vec![20, 10, 30, 99])
        );
        assert_eq!(
            move_visible_bag_seed_to_target(existing.clone(), &visible, 30, 10),
            Some(vec![30, 10, 20, 99])
        );
        assert_eq!(move_visible_bag_seed_to_target(existing.clone(), &visible, 20, 20), None);
        assert_eq!(move_visible_bag_seed_to_target(existing, &visible, 99, 10), None);
    }
}
