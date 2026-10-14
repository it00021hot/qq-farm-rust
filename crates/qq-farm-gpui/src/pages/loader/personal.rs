use qq_farm_app::{AppContext, AppResult};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PersonalSection {
    Lands,
    Bag,
    DailyGifts,
    Pets,
    Illustrated,
    InteractionItems,
}

impl PersonalSection {
    fn from_key(key: &str) -> Option<Self> {
        match key {
            "lands" => Some(Self::Lands),
            "bag" => Some(Self::Bag),
            "dailyGifts" => Some(Self::DailyGifts),
            "pets" => Some(Self::Pets),
            "illustrated" => Some(Self::Illustrated),
            "interactionItems" => Some(Self::InteractionItems),
            _ => None,
        }
    }
}

pub(super) async fn load(app: &AppContext, account_id: &str, section: &str) -> Value {
    let connected = qq_farm_app::farm::panel_status_with_progress(app, account_id).run_status == 1;
    if !connected {
        return serde_json::json!({ "connected": false });
    }
    // Vue 的 NTabPane 默认按需挂载，只请求当前可见的业务分段。
    let section = PersonalSection::from_key(section).unwrap_or(PersonalSection::Lands);
    let mut errors = Map::new();
    let mut raw = Map::new();
    raw.insert("connected".into(), Value::Bool(true));
    match section {
        PersonalSection::Lands => {
            let lands = qq_farm_app::farm::lands(app, account_id).await;
            raw.insert("lands".into(), section_value(lands, "lands", &mut errors));
        }
        PersonalSection::Bag => {
            let bag = qq_farm_app::farm::bag(app, account_id).await;
            raw.insert("bag".into(), section_value(bag, "bag", &mut errors));
        }
        PersonalSection::DailyGifts => {
            let gifts = qq_farm_app::farm::daily_gift_overview(app, account_id).await;
            raw.insert("dailyGifts".into(), section_value(gifts, "dailyGifts", &mut errors));
        }
        PersonalSection::Pets => {
            let (pets, pet_gift_status) = tokio::join!(
                qq_farm_app::pets::pet_info(app, account_id),
                qq_farm_app::pets::dog_skill_gifts_status(app, account_id),
            );
            raw.insert("pets".into(), section_value(pets, "pets", &mut errors));
            raw.insert(
                "petGiftStatus".into(),
                section_value(pet_gift_status, "petGiftStatus", &mut errors),
            );
        }
        PersonalSection::Illustrated => {
            let illustrated = qq_farm_app::friend::illustrated_snapshot(app, account_id).await;
            raw.insert(
                "illustrated".into(),
                section_value(illustrated, "illustrated", &mut errors),
            );
        }
        PersonalSection::InteractionItems => {
            let interaction_items =
                qq_farm_app::friend::farm_interaction_items(app, account_id).await;
            raw.insert(
                "interactionItems".into(),
                section_value(interaction_items, "interactionItems", &mut errors),
            );
        }
    }
    raw.insert("sectionErrors".into(), Value::Object(errors));
    Value::Object(raw)
}

fn section_value<T: Serialize>(
    result: AppResult<T>,
    section: &str,
    errors: &mut Map<String, Value>,
) -> Value {
    match result {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => value,
            Err(error) => {
                errors.insert(section.into(), Value::String(error.to_string()));
                Value::Null
            }
        },
        Err(error) => {
            errors.insert(section.into(), Value::String(error.to_string()));
            Value::Null
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_section_keys_cover_the_vue_tabs_one_at_a_time() {
        let sections = [
            ("lands", PersonalSection::Lands),
            ("bag", PersonalSection::Bag),
            ("dailyGifts", PersonalSection::DailyGifts),
            ("pets", PersonalSection::Pets),
            ("illustrated", PersonalSection::Illustrated),
            ("interactionItems", PersonalSection::InteractionItems),
        ];
        for (key, section) in sections {
            assert_eq!(PersonalSection::from_key(key), Some(section));
        }
        assert_eq!(PersonalSection::from_key("unknown"), None);
    }

    #[test]
    fn failed_personal_capability_keeps_other_snapshots_available() {
        let mut errors = Map::new();
        let lands =
            section_value(Ok(serde_json::json!({"lands": [{"id": 1}]})), "lands", &mut errors);
        let bag = section_value::<Value>(
            Err(qq_farm_app::AppError::BadRequest("仓库暂不可用".into())),
            "bag",
            &mut errors,
        );
        assert_eq!(lands["lands"][0]["id"], 1);
        assert!(bag.is_null());
        assert!(errors.contains_key("bag"));
        assert!(!errors.contains_key("lands"));
    }
}
