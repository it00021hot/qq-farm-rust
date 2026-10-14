use super::labels::{land_badge, land_badge_with_color};
use super::*;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::TestSupportExt;

pub(super) fn render_mutation(
    land: &serde_json::Value,
    names: &[String],
    purple_percent: i64,
    language: &str,
    cx: &App,
) -> Div {
    let effects = land.get("mutantEffects").and_then(serde_json::Value::as_array);
    let tooltip = mutation_tooltip(land);
    let icons = effects
        .into_iter()
        .flatten()
        .filter_map(|effect| {
            let url = effect.get("iconUrl")?.as_str()?;
            if url.is_empty() {
                return None;
            }
            let record = serde_json::json!({ "image": url });
            render_record_asset_preview_sized(&record, 14., cx)
        })
        .collect::<Vec<_>>();

    h_flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child({
            let label = crate::i18n::text(
                language,
                &format!("变异: {}", names.join("+")),
                &format!("Mutant: {}", names.join("+")),
            )
            .to_string();
            h_flex()
                .id("farm-land-mutation-badge")
                .items_center()
                .gap_1()
                .rounded(cx.theme().radius)
                .bg(cx.theme().warning.opacity(0.16))
                .px_2()
                .py_1()
                .children(icons)
                .child(div().text_xs().text_color(cx.theme().warning).child(label))
                .when(!tooltip.is_empty(), |badge| {
                    badge.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                })
                .test_support()
        })
        .when(purple_percent > 0, |row| {
            row.child(land_badge_with_color(
                crate::i18n::text(
                    language,
                    &format!("紫晶共鸣 +{purple_percent}%"),
                    &format!("Purple crystal +{purple_percent}%"),
                )
                .to_string(),
                cx.theme().danger,
                cx,
            ))
        })
}

pub(super) fn render_interaction(
    names: &[String],
    tooltip: String,
    needs_cleanup: bool,
    language: &str,
    cx: &App,
) -> Div {
    h_flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .when(!names.is_empty() || needs_cleanup, |row| {
            let names = names.join("+");
            let label = if names.is_empty() {
                crate::i18n::text(language, "互动效果: --", "Interaction effects: --").to_string()
            } else {
                crate::i18n::text(
                    language,
                    &format!("互动效果: {names}"),
                    &format!("Interaction effects: {names}"),
                )
                .to_string()
            };
            let badge = land_badge(label, cx).id("farm-land-interaction-items");
            row.child(if tooltip.is_empty() {
                badge
            } else {
                badge.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            })
        })
        .when(needs_cleanup, |row| {
            let label = crate::i18n::text(language, "待清理", "Cleanup needed").to_string();
            let hint = crate::i18n::text(
                language,
                "好友道具残留，一键务农会顺带清理",
                "Friend item leftovers; one-click farming will clean them",
            )
            .to_string();
            row.child(
                land_badge_with_color(label, cx.theme().warning, cx)
                    .id("farm-land-interaction-cleanup")
                    .tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx)),
            )
        })
}

pub(super) fn mutant_names(land: &serde_json::Value) -> Vec<String> {
    let names = land
        .get("mutantEffects")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|effect| effect.get("name").and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !names.is_empty() {
        return names;
    }
    land.get("mutantConfigIds")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|id| id.as_i64().map_or_else(|| id.to_string(), |id| id.to_string()))
        .collect()
}

fn mutation_tooltip(land: &serde_json::Value) -> String {
    land.get("mutantEffects")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|effect| {
            effect
                .get("description")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .or_else(|| effect.get("name").and_then(serde_json::Value::as_str))
                .filter(|value| !value.is_empty())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn interaction_names(land: &serde_json::Value) -> Vec<String> {
    land.get("interactionEffects")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|effect| {
            effect
                .get("itemName")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map(|name| name.trim().to_string())
        })
        .collect()
}

pub(super) fn interaction_tooltip(land: &serde_json::Value) -> String {
    land.get("interactionEffects")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|effect| {
            let item_name = effect.get("itemName").filter(|value| is_truthy(value));
            item_name.or_else(|| effect.get("itemId")).map(js_string)
        })
        .collect::<Vec<_>>()
        .join("、")
}

fn is_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

fn js_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(values) => {
            values.iter().map(js_string).collect::<Vec<_>>().join(",")
        }
        serde_json::Value::Object(_) => "[object Object]".into(),
        other => other.to_string(),
    }
}

pub(in crate::desktop) fn care_flags(
    land: &serde_json::Value,
    language: &str,
    cx: &App,
) -> Vec<(&'static str, &'static str, Hsla)> {
    let needed = |key| land.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
    let mut flags = Vec::new();
    if needed("needWater") {
        flags.push(("💧", crate::i18n::text(language, "待浇水", "Needs water"), cx.theme().info));
    }
    if needed("needWeed") {
        flags.push((
            "🌿",
            crate::i18n::text(language, "待除草", "Needs weeding"),
            cx.theme().success,
        ));
    }
    if needed("needBug") {
        flags.push((
            "🐛",
            crate::i18n::text(language, "待除虫", "Needs pest control"),
            cx.theme().danger,
        ));
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::{interaction_names, interaction_tooltip, mutation_tooltip};
    use serde_json::json;

    #[test]
    fn interaction_names_match_the_visible_vue_badge_and_tooltip_keeps_id_fallbacks() {
        let land = json!({
            "interactionEffects": [
                {"itemName": "  浇水器  ", "itemId": 1},
                {"itemName": "", "itemId": 2},
                {"itemId": 3}
            ]
        });

        assert_eq!(interaction_names(&land), ["浇水器"]);
        assert_eq!(interaction_tooltip(&land), "  浇水器  、2、3");
    }

    #[test]
    fn mutation_tooltip_prefers_description_then_falls_back_to_name() {
        let land = json!({
            "mutantEffects": [
                {"description": "经验增加", "name": "增产"},
                {"description": "", "name": "甜蜜"},
                {"name": ""}
            ]
        });

        assert_eq!(mutation_tooltip(&land), "经验增加\n甜蜜");
    }
}
