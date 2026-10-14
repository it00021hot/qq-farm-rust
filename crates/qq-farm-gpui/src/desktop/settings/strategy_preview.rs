use super::super::{FarmDesktop, Page};
use gpui_kit::Context;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StrategyPreview {
    Loading,
    Seed { required_level: i64, name: String },
    NoAvailableSeeds,
    NoMatchingSeed,
}

impl StrategyPreview {
    pub(crate) fn label(&self, language: &str) -> String {
        match self {
            Self::Loading => localized(language, "加载中...", "Loading..."),
            Self::Seed { required_level, name } => format!("Lv{required_level} {name}"),
            Self::NoAvailableSeeds => localized(language, "暂无可用种子", "No available seeds"),
            Self::NoMatchingSeed => localized(language, "暂无匹配种子", "No matching seed"),
        }
    }
}

pub(crate) fn strategy_preview_sort_by(draft: &Value) -> Option<&'static str> {
    let strategy = draft
        .get("plantingStrategy")
        .or_else(|| draft.get("strategy"))
        .and_then(Value::as_str)
        .unwrap_or("max_exp");
    let fallback = draft
        .get("bagSeedFallbackStrategy")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("preferred");
    let resolved = if strategy == "bag_priority" { fallback } else { strategy };
    match resolved {
        "max_exp" => Some("exp"),
        "max_fert_exp" => Some("fert"),
        "max_profit" => Some("profit"),
        "max_fert_profit" => Some("fert_profit"),
        _ => None,
    }
}

pub(crate) fn compute_strategy_preview(
    draft: &Value,
    raw: &Value,
    rankings: Option<&Value>,
) -> Option<StrategyPreview> {
    let strategy = draft
        .get("plantingStrategy")
        .or_else(|| draft.get("strategy"))
        .and_then(Value::as_str)
        .unwrap_or("max_exp");
    let fallback = draft
        .get("bagSeedFallbackStrategy")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("preferred");
    let resolved = if strategy == "bag_priority" { fallback } else { strategy };

    // Vue replaces the preview with the searchable preferred-seed selector in these cases.
    if resolved == "preferred" {
        return None;
    }

    let seeds = raw.get("seedOptions")?.as_array()?;
    if seeds.is_empty() {
        // Vue leaves the preview label unset while seed options are loading.
        return None;
    }
    let available = seeds
        .iter()
        .filter_map(|seed| {
            let id = number(seed.get("seedId").or_else(|| seed.get("seed_id")))?;
            (!js_truthy(seed.get("locked")) && !js_truthy(seed.get("soldOut")))
                .then_some((id, seed))
        })
        .collect::<Vec<_>>();
    if available.is_empty() {
        return Some(StrategyPreview::NoAvailableSeeds);
    }

    if resolved == "level" {
        return available
            .into_iter()
            .max_by(|(left_id, left), (right_id, right)| {
                seed_level(left).cmp(&seed_level(right)).then_with(|| right_id.cmp(left_id))
            })
            .map(|(_, seed)| preview_seed(seed));
    }

    let rankings = rankings.and_then(Value::as_array)?;
    rankings
        .iter()
        .filter_map(|row| number(row.get("seedId").or_else(|| row.get("seed_id"))))
        .find_map(|seed_id| {
            available
                .iter()
                .find(|(available_id, _)| *available_id == seed_id)
                .map(|(_, seed)| preview_seed(seed))
        })
        .or(Some(StrategyPreview::NoMatchingSeed))
}

fn preview_seed(seed: &Value) -> StrategyPreview {
    StrategyPreview::Seed {
        required_level: seed_level(seed),
        name: seed.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
    }
}

fn seed_level(seed: &Value) -> i64 {
    seed.get("requiredLevel")
        .or_else(|| seed.get("required_level"))
        .or_else(|| seed.get("landLevelNeed"))
        .or_else(|| seed.get("land_level_need"))
        .and_then(|value| number(Some(value)))
        .unwrap_or_default()
}

fn number(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
            .or_else(|| value.as_f64().map(|number| number as i64))
            .or_else(|| value.as_str().and_then(|number| number.trim().parse().ok()))
    })
}

fn js_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|value| value != 0.0),
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(_) | Value::Object(_)) => true,
    }
}

fn localized(language: &str, chinese: &str, english: &str) -> String {
    crate::i18n::text(language, chinese, english).to_string()
}

impl FarmDesktop {
    pub(in crate::desktop) fn refresh_strategy_preview(&mut self, cx: &mut Context<Self>) {
        self.settings.strategy_preview_generation =
            self.settings.strategy_preview_generation.wrapping_add(1);
        let generation = self.settings.strategy_preview_generation;
        let Some(account_id) = self.selected_account.clone() else {
            self.settings.strategy_preview = None;
            cx.notify();
            return;
        };
        if self.page != Page::Settings {
            return;
        }

        let draft = self.settings.draft.clone();
        let raw = self.page_raw.clone();
        let Some(sort_by) = strategy_preview_sort_by(&draft) else {
            self.settings.strategy_preview = compute_strategy_preview(&draft, &raw, None);
            cx.notify();
            return;
        };

        self.settings.strategy_preview = Some(StrategyPreview::Loading);
        cx.notify();
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.spawn(async move |this, cx| {
            let rankings = runtime
                .spawn(async move { qq_farm_app::farm::analytics(Some(sort_by)) })
                .await
                .ok();
            let preview = compute_strategy_preview(&draft, &raw, rankings.as_ref());
            let _ = this.update(cx, |this, cx| {
                if this.page != Page::Settings
                    || this.settings.strategy_preview_generation != generation
                    || this.selected_account.as_deref() != Some(account_id.as_str())
                {
                    return;
                }
                this.settings.strategy_preview = preview;
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_strategy_preview, strategy_preview_sort_by, StrategyPreview};

    #[test]
    fn level_preview_uses_highest_available_level_and_lowest_id_for_ties() {
        let draft = serde_json::json!({"plantingStrategy":"level"});
        let raw = serde_json::json!({
            "seedOptions": [
                {"seedId": 20, "name":"番茄", "requiredLevel": 8},
                {"seedId": 10, "name":"玉米", "requiredLevel": 8},
                {"seedId": 1, "name":"锁定种子", "requiredLevel": 99, "locked":true},
                {"seedId": 2, "name":"售罄种子", "requiredLevel": 100, "soldOut":true}
            ]
        });
        assert_eq!(
            compute_strategy_preview(&draft, &raw, None),
            Some(StrategyPreview::Seed { required_level: 8, name: "玉米".into() })
        );
    }

    #[test]
    fn analytics_preview_uses_the_first_ranked_seed_that_is_available() {
        let draft = serde_json::json!({"plantingStrategy":"max_profit"});
        let raw = serde_json::json!({
            "seedOptions": [
                {"seedId":10, "name":"玉米", "requiredLevel":3},
                {"seedId":20, "name":"番茄", "requiredLevel":6},
                {"seedId":30, "name":"南瓜", "requiredLevel":9, "soldOut":true}
            ]
        });
        let rankings = serde_json::json!([
            {"seedId":30}, {"seedId":"20"}, {"seedId":10}
        ]);
        assert_eq!(
            compute_strategy_preview(&draft, &raw, Some(&rankings)),
            Some(StrategyPreview::Seed { required_level: 6, name: "番茄".into() })
        );
        assert_eq!(strategy_preview_sort_by(&draft), Some("profit"));
    }

    #[test]
    fn preview_statuses_follow_available_seed_and_strategy_gates() {
        let available = serde_json::json!({
            "seedOptions": [{"seedId":1,"name":"玉米","requiredLevel":1}]
        });
        let unavailable = serde_json::json!({
            "seedOptions": [{"seedId":1,"name":"玉米","requiredLevel":1,"locked":true}]
        });
        assert_eq!(
            compute_strategy_preview(
                &serde_json::json!({"plantingStrategy":"level"}),
                &unavailable,
                None,
            ),
            Some(StrategyPreview::NoAvailableSeeds)
        );
        assert_eq!(
            compute_strategy_preview(
                &serde_json::json!({"plantingStrategy":"max_exp"}),
                &available,
                Some(&serde_json::json!([])),
            ),
            Some(StrategyPreview::NoMatchingSeed)
        );
        assert_eq!(
            compute_strategy_preview(
                &serde_json::json!({"plantingStrategy":"bag_priority","bagSeedFallbackStrategy":"preferred"}),
                &available,
                None,
            ),
            None
        );
        assert_eq!(
            strategy_preview_sort_by(&serde_json::json!({
                "plantingStrategy":"bag_priority",
                "bagSeedFallbackStrategy":"max_fert_profit"
            })),
            Some("fert_profit")
        );
    }
}
