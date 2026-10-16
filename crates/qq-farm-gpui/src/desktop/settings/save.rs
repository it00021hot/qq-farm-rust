use super::super::*;

fn automation_requires_fertilizer_check(snapshot: &serde_json::Value) -> bool {
    let automation = snapshot.get("automation").unwrap_or(snapshot);
    ["fertilizer_buy_organic", "fertilizer_buy_normal"]
        .into_iter()
        .any(|key| automation.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false))
}

fn should_run_fertilizer_check(page_section: &str, snapshot: &serde_json::Value) -> bool {
    page_section == "automation" && automation_requires_fertilizer_check(snapshot)
}

fn fertilizer_check_error(result: Result<serde_json::Value, String>) -> Option<String> {
    match result {
        Ok(result) => result
            .get("error")
            .and_then(serde_json::Value::as_str)
            .filter(|error| !error.is_empty())
            .map(str::to_owned),
        Err(error) => Some(error),
    }
}

const STRATEGY_FIELDS: &[&str] = &[
    "intervals",
    "plantingStrategy",
    "preferredSeedId",
    "bagSeedPriority",
    "bagSeedFallbackStrategy",
    "bagSeedMultiLandReservationEnabled",
    "plantOrderRandom",
    "plantDelaySeconds",
    "stealDelaySeconds",
    "plantBlacklist",
    "friendQuietHours",
];

const AUTOMATION_FIELDS: &[&str] = &[
    "friendAutoAccept",
    "autoAcceptFriendMinLevel",
    "autoAcceptRequireOwnLevel",
    "autoAcceptHarvestStealEnabled",
    "autoAcceptHarvestStealHarvest",
    "autoAcceptHarvestStealSteal",
    "fertilizerBuyOrganicCount",
    "fertilizerBuyOrganicThresholdHours",
    "fertilizerBuyNormalCount",
    "fertilizerBuyNormalThresholdHours",
];

const AUTOMATION_OPTIONS: &[&str] = &[
    "farm",
    "farm_push",
    "land_upgrade",
    "friend",
    "friend_steal",
    "friend_help",
    "friend_bad",
    "friend_help_exp_limit",
    "task",
    "sell",
    "fertilizer",
    "fertilizer_gift",
    "fertilizer_buy_organic",
    "fertilizer_buy_normal",
    "fertilizer_multi_season",
    "fertilizer_land_types",
    "fertilizer_smart_seconds",
    "skip_own_weed_bug",
    "mystery_shop_auto_buy",
    "mystery_shop_arrival_notify",
    "mystery_shop_purchase_notify",
    "mystery_shop_allow_gold",
    "mystery_shop_allow_coupon",
    "mystery_shop_allow_gold_bean",
    "mystery_shop_allow_diamond",
    "friend_auto_accept",
];

fn fields_from(
    source: &serde_json::Value,
    fields: &[&str],
) -> serde_json::Map<String, serde_json::Value> {
    fields
        .iter()
        .filter_map(|key| source.get(*key).cloned().map(|value| ((*key).into(), value)))
        .collect()
}

fn section_save_snapshot(section: &str, draft: &serde_json::Value) -> Option<serde_json::Value> {
    let mut snapshot = serde_json::Map::new();
    match section {
        "strategy" => {
            for (key, value) in fields_from(draft, STRATEGY_FIELDS) {
                snapshot.insert(key, value);
            }
        }
        "automation" => {
            let automation = fields_from(
                draft.get("automation").unwrap_or(&serde_json::Value::Null),
                AUTOMATION_OPTIONS,
            );
            if !automation.is_empty() {
                snapshot.insert("automation".into(), serde_json::Value::Object(automation));
            }
            for (key, value) in fields_from(draft, AUTOMATION_FIELDS) {
                snapshot.insert(key, value);
            }
        }
        _ => return None,
    }
    (!snapshot.is_empty()).then_some(serde_json::Value::Object(snapshot))
}

impl FarmDesktop {
    pub(in crate::desktop) fn clear_settings_notice(&mut self) {
        self.settings.save_notice_generation = self.settings.save_notice_generation.wrapping_add(1);
        self.settings.save_notice = None;
        self.settings.save_notice_warning = false;
    }

    pub(in crate::desktop) fn show_settings_notice(
        &mut self,
        chinese: &str,
        english: &str,
        warning: bool,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        let message = crate::i18n::text(&self.preferences.language, chinese, english).to_string();
        self.show_toast(message.clone(), !warning, cx);
        self.settings.save_notice_generation = self.settings.save_notice_generation.wrapping_add(1);
        let generation = self.settings.save_notice_generation;
        self.settings.save_notice = Some(message);
        self.settings.save_notice_warning = warning;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.settings.save_notice_generation == generation {
                    this.settings.save_notice = None;
                    this.settings.save_notice_warning = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn complete_settings_save(
        &mut self,
        section: &str,
        result: Result<Option<String>, String>,
        show_feedback: bool,
        cx: &mut Context<Self>,
    ) {
        match section {
            "strategy" => self.settings.strategy_saving = false,
            "automation" => self.settings.automation_saving = false,
            _ => return,
        }
        if !show_feedback {
            cx.notify();
            return;
        }
        match result {
            Ok(Some(_)) => self.show_settings_notice(
                "设置已保存，但化肥检查失败。",
                "Settings saved, but the fertilizer check failed.",
                true,
                cx,
            ),
            Ok(None) if section == "strategy" => {
                self.show_settings_notice("策略设置已保存", "Strategy settings saved", false, cx)
            }
            Ok(None) => {
                self.show_settings_notice("自动控制已保存", "Automation settings saved", false, cx)
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(in crate::desktop) fn save_settings(&mut self, cx: &mut Context<Self>) {
        let section = self.page_section.clone();
        if self.loading || self.settings.section_save_pending(&section) {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let Some(snapshot) = section_save_snapshot(&self.page_section, &self.settings.draft) else {
            return;
        };
        let check_fertilizer = should_run_fertilizer_check(&self.page_section, &snapshot);
        let account_id_guard = account_id.clone();
        let page_section_guard = section.clone();
        let request_revision = self.request_revision;
        self.clear_settings_notice();
        self.settings.set_section_save_pending(&section, true);
        self.error = None;
        cx.notify();
        if self.test_mode && !check_fertilizer {
            let result =
                qq_farm_app::settings::save_settings(&self.app, &account_id_guard, snapshot)
                    .map(|_| None)
                    .map_err(|error| error.to_string());
            self.complete_settings_save(&page_section_guard, result, true, cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::settings::save_settings(&app, &account_id, snapshot)
                        .map_err(|error| error.to_string())?;
                    let fertilizer_error = if check_fertilizer {
                        fertilizer_check_error(
                            qq_farm_app::farm::fertilizer_check_and_buy(&app, &account_id)
                                .await
                                .map_err(|error| error.to_string()),
                        )
                    } else {
                        None
                    };
                    Ok::<_, String>(fertilizer_error)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let show_feedback = this.request_revision == request_revision
                    && this.selected_account.as_deref() == Some(account_id_guard.as_str())
                    && this.page == Page::Settings
                    && this.page_section == page_section_guard;
                let result = match result {
                    Ok(result) => result,
                    Err(error) => {
                        Err(crate::i18n::background_task_error(&this.preferences.language, error))
                    }
                };
                this.complete_settings_save(&page_section_guard, result, show_feedback, cx);
                // Vue keeps edits in other settings sections as unsaved local state after saving
                // the current section; refreshing the whole page would discard those drafts.
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        automation_requires_fertilizer_check, fertilizer_check_error, section_save_snapshot,
        should_run_fertilizer_check,
    };

    #[test]
    fn fertilizer_check_runs_only_when_vue_purchase_automation_is_enabled() {
        assert!(!automation_requires_fertilizer_check(&serde_json::json!({
            "automation": {"fertilizer_buy_organic": false, "fertilizer_buy_normal": false}
        })));
        assert!(automation_requires_fertilizer_check(&serde_json::json!({
            "automation": {"fertilizer_buy_organic": true}
        })));
        assert!(automation_requires_fertilizer_check(&serde_json::json!({
            "fertilizer_buy_normal": true
        })));
    }

    #[test]
    fn fertilizer_check_runs_only_after_saving_the_automation_section() {
        let snapshot = serde_json::json!({
            "automation": {"fertilizer_buy_organic": true}
        });

        assert!(!should_run_fertilizer_check("strategy", &snapshot));
        assert!(should_run_fertilizer_check("automation", &snapshot));
        assert!(!should_run_fertilizer_check("offlineReminder", &snapshot));
    }

    #[test]
    fn fertilizer_check_reads_the_vue_error_field_and_transport_errors() {
        assert_eq!(
            fertilizer_check_error(Ok(serde_json::json!({ "error": "账号未运行" }))),
            Some("账号未运行".into())
        );
        assert_eq!(fertilizer_check_error(Ok(serde_json::json!({ "error": null }))), None);
        assert_eq!(fertilizer_check_error(Err("请求失败".into())), Some("请求失败".into()));
    }

    #[test]
    fn settings_save_payload_contains_only_the_vue_section_fields() {
        let draft = serde_json::json!({
            "intervals": {"farm": 15, "farmMin": 10},
            "plantingStrategy": "max_profit",
            "plantBlacklist": [1, 2],
            "friendQuietHours": {"enabled": true, "start": "22:00", "end": "06:00", "continueFarm": true},
            "automation": {"farm": false, "friend": true, "friend_help_protect_dog_ignore_exp_limit": true},
            "autoAcceptFriendMinLevel": 12,
            "offlineReminder": {"provider": "ding_talk"},
            "systemConfig": {"serverUrl": "wss://example.invalid"}
        });

        let strategy = section_save_snapshot("strategy", &draft).unwrap();
        assert_eq!(strategy["plantingStrategy"], "max_profit");
        assert_eq!(strategy["plantBlacklist"], serde_json::json!([1, 2]));
        assert!(strategy.get("automation").is_none());
        assert!(strategy.get("offlineReminder").is_none());
        assert!(strategy.get("systemConfig").is_none());

        let automation = section_save_snapshot("automation", &draft).unwrap();
        assert_eq!(automation["automation"], serde_json::json!({"farm": false, "friend": true}));
        assert_eq!(automation["autoAcceptFriendMinLevel"], 12);
        assert!(automation.get("plantingStrategy").is_none());
        assert!(automation.get("offlineReminder").is_none());
        assert!(section_save_snapshot("offlineReminder", &draft).is_none());
    }
}
