use qq_farm_app::session::AppContext;

use super::{page_data, Page, PageData};

pub async fn load_page_for_mall_slot(
    app: &AppContext,
    page: Page,
    account_id: &str,
    mall_slot_type: i32,
    page_section: &str,
) -> Result<PageData, String> {
    if account_id.is_empty()
        && !matches!(page, Page::Accounts | Page::GameConfig | Page::Analytics | Page::Settings)
    {
        return Ok(page_data(page, serde_json::Value::Null));
    }
    let raw = match page {
        Page::Dashboard => {
            let status = qq_farm_app::farm::panel_status_with_progress(app, account_id);
            let logs = qq_farm_app::farm::engine_global_logs(app, Some(account_id), 1000);
            let (bag, diamond) = if status.run_status == 1 {
                let (bag, diamond) = tokio::join!(
                    qq_farm_app::farm::bag(app, account_id),
                    qq_farm_app::farm::diamond_balance(app, account_id),
                );
                (bag.ok().and_then(|value| serde_json::to_value(value).ok()), diamond.ok())
            } else {
                (None, None)
            };
            serde_json::json!({
                "status": status,
                "logs": logs,
                "bag": bag,
                "diamond": diamond,
            })
        }
        Page::Personal => personal::load(app, account_id, page_section).await,
        Page::Activities => serde_json::Value::Null,
        Page::SolarCalendar => qq_farm_app::solar_calendar::snapshot(app, account_id)
            .await
            .map_err(|error| error.to_string())?,
        Page::ShareReward => qq_farm_app::share_reward::snapshot(
            app,
            account_id,
            crate::desktop::SHARE_REWARD_ACTIVITY_ID,
        )
        .await
        .map_err(|error| error.to_string())?,
        // 奶茶活动页由 GPUI 侧专用加载（refresh 早分支），这里仅兜底
        Page::MilkTea => serde_json::Value::Null,
        Page::Friends => {
            let load = FriendPageLoad::for_section(page_section);
            let (friends, records, interaction_items) = tokio::join!(
                qq_farm_app::friend::list_friends(app, account_id, false),
                async {
                    if load.records {
                        Some(qq_farm_app::friend::interact_records(app, account_id).await)
                    } else {
                        None
                    }
                },
                async {
                    if load.interaction_items {
                        Some(qq_farm_app::friend::friend_interaction_items(app, account_id).await)
                    } else {
                        None
                    }
                },
            );
            let friends = friends.map_err(|error| error.to_string())?;
            let mut raw = serde_json::json!({
                "friends": friends,
                "blacklist": qq_farm_app::friend::friend_blacklist(account_id),
            });
            if let Some(records) = records {
                match records {
                    Ok(records) => {
                        raw["records"] =
                            serde_json::to_value(records).unwrap_or(serde_json::Value::Null);
                        raw["recordsError"] = serde_json::Value::Null;
                    }
                    Err(error) => {
                        raw["records"] = serde_json::json!([]);
                        raw["recordsError"] = serde_json::json!(error.to_string());
                    }
                }
            }
            if let Some(items) = interaction_items {
                match items {
                    Ok(items) => {
                        raw["interactionItems"] =
                            serde_json::to_value(items).unwrap_or(serde_json::Value::Null);
                        raw["interactionItemsError"] = serde_json::Value::Null;
                    }
                    Err(error) => {
                        raw["interactionItems"] = serde_json::json!([]);
                        raw["interactionItemsError"] = serde_json::json!(error.to_string());
                    }
                }
            }
            raw
        }
        Page::Analytics => {
            let status = qq_farm_app::farm::panel_status_with_progress(app, account_id);
            serde_json::json!({
                "rankings": qq_farm_app::farm::analytics(None),
                "plantBlacklist": qq_farm_app::farm::plant_blacklist(account_id),
                "configuration": qq_farm_app::settings::settings_panel(account_id, "local"),
                "accountLevel": status.level,
            })
        }
        Page::Mall => {
            if mall_slot_type == -1 {
                let mut shop = qq_farm_app::commerce::mystery_shop(app, account_id)
                    .await
                    .map_err(|error| error.to_string())?;
                if let Some(object) = shop.as_object_mut() {
                    object.insert("slotType".into(), serde_json::json!(-1));
                }
                shop
            } else {
                let (catalog, balance) = tokio::try_join!(
                    qq_farm_app::commerce::mall_catalog(
                        app,
                        account_id,
                        Some(mall_slot_type),
                        Some(0)
                    ),
                    qq_farm_app::farm::diamond_balance(app, account_id),
                )
                .map_err(|error| error.to_string())?;
                serde_json::json!({"catalog": catalog, "diamond": balance, "slotType": mall_slot_type})
            }
        }
        Page::MysteryShop => {
            let mut shop = qq_farm_app::commerce::mystery_shop(app, account_id)
                .await
                .map_err(|error| error.to_string())?;
            if let Some(object) = shop.as_object_mut() {
                object.insert("slotType".into(), serde_json::json!(-1));
            }
            shop
        }
        Page::Settings => {
            let mut settings = qq_farm_app::settings::settings_panel(account_id, "local");
            normalize_settings_snapshot(&mut settings);
            if let Some(object) = settings.as_object_mut() {
                let preferred_seed =
                    object.remove("preferredSeed").unwrap_or_else(|| serde_json::json!(0));
                object.insert("preferredSeedId".into(), preferred_seed);
                let system_config = qq_farm_app::admin::get_system_config();
                object.insert(
                    "systemConfig".into(),
                    if system_config.is_null() {
                        serde_json::json!(qq_farm_core::config::get_default_system_config())
                    } else {
                        system_config
                    },
                );
                object.insert("devicePresets".into(), qq_farm_app::admin::device_presets());
                object.insert("seedOptions".into(), qq_farm_app::farm::seeds_catalog());
                let bag_seed_result = if !account_id.is_empty()
                    && object.get("plantingStrategy").and_then(serde_json::Value::as_str)
                        == Some("bag_priority")
                {
                    Some(qq_farm_app::farm::bag_seeds(app, account_id).await)
                } else {
                    None
                };
                match bag_seed_result {
                    Some(Ok(bag_seeds)) => {
                        object.insert("bagSeeds".into(), bag_seeds);
                    }
                    Some(Err(error)) => {
                        object.insert("bagSeeds".into(), serde_json::Value::Null);
                        object.insert("bagSeedsError".into(), serde_json::json!(error.to_string()));
                    }
                    None => {
                        object.insert("bagSeeds".into(), serde_json::Value::Null);
                    }
                }
                object.insert(
                    "qqBotBinding".into(),
                    qq_farm_app::qq_bot_bind::qq_bot_bind_status(Some("local")),
                );
            }
            settings
        }
        Page::GameConfig => serde_json::json!({
            "seeds": qq_farm_app::config::list_seeds(),
            "fruits": qq_farm_app::config::list_fruits(),
            "items": qq_farm_app::config::list_items(),
            "itemTypes": qq_farm_app::config::item_types(),
        }),
        Page::Accounts => qq_farm_app::accounts::list_accounts_enriched(app, None),
    };
    Ok(page_data(page, raw))
}

fn normalize_settings_snapshot(settings: &mut serde_json::Value) {
    let Some(quiet_hours) =
        settings.get_mut("friendQuietHours").and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let snake_case_value = quiet_hours.remove("continue_farm");
    if !quiet_hours.contains_key("continueFarm") {
        if let Some(value) = snake_case_value {
            quiet_hours.insert("continueFarm".into(), value);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FriendPageLoad {
    records: bool,
    interaction_items: bool,
}

impl FriendPageLoad {
    fn for_section(section: &str) -> Self {
        Self { records: section == "records", interaction_items: section == "interactionItems" }
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_settings_snapshot, FriendPageLoad};

    #[test]
    fn settings_snapshot_normalizes_quiet_hours_like_the_vue_api_adapter() {
        let mut snake_case = serde_json::json!({
            "friendQuietHours": {
                "enabled": true,
                "start": "22:30",
                "end": "06:15",
                "continue_farm": false
            }
        });
        normalize_settings_snapshot(&mut snake_case);
        assert_eq!(snake_case["friendQuietHours"]["continueFarm"], false);
        assert!(snake_case["friendQuietHours"].get("continue_farm").is_none());

        let mut both_names = serde_json::json!({
            "friendQuietHours": {"continue_farm": true, "continueFarm": false}
        });
        normalize_settings_snapshot(&mut both_names);
        assert_eq!(both_names["friendQuietHours"]["continueFarm"], false);
        assert!(both_names["friendQuietHours"].get("continue_farm").is_none());
    }

    #[test]
    fn friend_capabilities_are_loaded_only_when_their_vue_tabs_open() {
        assert_eq!(
            FriendPageLoad::for_section("friends"),
            FriendPageLoad { records: false, interaction_items: false }
        );
        assert_eq!(
            FriendPageLoad::for_section("blacklist"),
            FriendPageLoad { records: false, interaction_items: false }
        );
        assert_eq!(
            FriendPageLoad::for_section("records"),
            FriendPageLoad { records: true, interaction_items: false }
        );
        assert_eq!(
            FriendPageLoad::for_section("interactionItems"),
            FriendPageLoad { records: false, interaction_items: true }
        );
    }
}
mod personal;
