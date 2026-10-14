use super::super::*;
use crate::desktop::land_grid::visible_lands;
use std::time::Duration;

pub(crate) fn visible_farm_operations(
    raw: &serde_json::Value,
) -> Vec<(&'static str, &'static str)> {
    let lands = raw
        .pointer("/lands/lands")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let lands = visible_lands(&lands);
    let actionable = |land: &serde_json::Value| {
        js_truthy(land.get("unlocked"))
            && !js_truthy(land.get("occupiedByMaster"))
            && land.get("status").and_then(serde_json::Value::as_str) != Some("locked")
    };
    let harvest = lands.iter().any(|land| {
        if !actionable(land) {
            return false;
        }
        matches!(
            land.get("status").and_then(serde_json::Value::as_str),
            Some("harvestable" | "harvested")
        ) || (land.get("status").and_then(serde_json::Value::as_str) == Some("growing")
            && js_number(land.get("matureInSec")) <= 0.
            && js_truthy(land.get("plantName")))
    });
    let clear = lands.iter().any(|land| {
        actionable(land)
            && ["needWater", "needWeed", "needBug"].iter().any(|key| js_truthy(land.get(*key)))
    });
    let plant = lands.iter().any(|land| {
        actionable(land)
            && matches!(
                land.get("status").and_then(serde_json::Value::as_str),
                Some("empty" | "dead")
            )
    });
    let upgrade = lands.iter().any(|land| {
        !land.get("occupiedByMaster").and_then(serde_json::Value::as_bool).unwrap_or(false)
            && (js_truthy(land.get("couldUpgrade")) || js_truthy(land.get("couldUnlock")))
    });
    let mut operations = Vec::new();
    if harvest {
        operations.push(("harvest", "收获"));
    }
    if clear {
        operations.push(("clear", "一键务农"));
    }
    if plant {
        operations.push(("plant", "种植"));
    }
    if upgrade {
        operations.push(("upgrade", "升级土地"));
    }
    if harvest || clear || plant {
        operations.push(("all", "一键全收"));
    }
    operations
}

fn js_truthy(value: Option<&serde_json::Value>) -> bool {
    match value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::Number(value)) => value.as_f64().is_some_and(|value| value != 0.),
        Some(serde_json::Value::String(value)) => !value.is_empty(),
        Some(serde_json::Value::Array(_) | serde_json::Value::Object(_)) => true,
    }
}

fn js_number(value: Option<&serde_json::Value>) -> f64 {
    match value {
        None | Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false)) => 0.,
        Some(serde_json::Value::Bool(true)) => 1.,
        Some(serde_json::Value::Number(value)) => value.as_f64().unwrap_or(f64::NAN),
        Some(serde_json::Value::String(value)) if value.trim().is_empty() => 0.,
        Some(serde_json::Value::String(value)) => value.trim().parse().unwrap_or(f64::NAN),
        Some(serde_json::Value::Array(_) | serde_json::Value::Object(_)) => f64::NAN,
    }
}

impl FarmDesktop {
    pub(in crate::desktop::personal) fn append_farming_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let language = &self.preferences.language;
        for (key, label) in visible_farm_operations(&self.page_raw) {
            let label = match key {
                "harvest" => crate::i18n::text(language, label, "Harvest"),
                "clear" => crate::i18n::text(language, label, "Maintain"),
                "plant" => crate::i18n::text(language, label, "Plant"),
                "upgrade" => crate::i18n::text(language, label, "Upgrade Lands"),
                "all" => crate::i18n::text(language, label, "Run All"),
                _ => label,
            };
            controls = controls.child(
                Button::new(key.to_string())
                    .label(label)
                    .when(key == "harvest", |button| button.success())
                    .when(key == "clear", |button| button.info())
                    .when(key == "plant", |button| button.primary())
                    .when(key == "upgrade", |button| button.warning())
                    .when(key == "all", |button| button.danger())
                    .outline()
                    .loading(self.farm.lands.operating)
                    .disabled(self.farm.lands.operating || self.selected_account.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| this.run_farm_operation(key, cx))),
            );
        }
        controls
    }

    pub(in crate::desktop) fn run_farm_operation(
        &mut self,
        operation: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.farm.lands.operating {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先添加账号",
                    "Add an account first",
                )
                .into(),
            );
            cx.notify();
            return;
        };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let account_id_guard = account_id.clone();
        self.farm.lands.operation_generation = self.farm.lands.operation_generation.wrapping_add(1);
        let generation = self.farm.lands.operation_generation;
        self.farm.lands.operating = true;
        self.farm.lands.operation_notice_generation =
            self.farm.lands.operation_notice_generation.wrapping_add(1);
        self.farm.lands.operation_notice = None;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(
                    async move { qq_farm_app::farm::operate(&app, &account_id, operation).await },
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.lands.operation_generation != generation {
                    return;
                }
                if this.selected_account.as_deref() != Some(account_id_guard.as_str()) {
                    this.farm.lands.operating = false;
                    this.farm.lands.finish_operation_after_refresh = false;
                    cx.notify();
                    return;
                }
                let refresh_lands = matches!(&result, Ok(Ok(_)))
                    && this.page == Page::Personal
                    && this.page_section == "lands";
                if !refresh_lands {
                    this.farm.lands.operating = false;
                    this.farm.lands.finish_operation_after_refresh = false;
                }
                match result {
                    Ok(Ok(_)) => {
                        this.show_farm_operation_notice(
                            crate::i18n::text(
                                &this.preferences.language,
                                "操作已提交",
                                "Operation submitted",
                            ),
                            true,
                            cx,
                        );
                        if this.page == Page::Personal && this.page_section == "lands" {
                            this.farm.lands.finish_operation_after_refresh = true;
                            if this.loading {
                                this.refresh(cx);
                            } else {
                                this.refresh_personal_lands(cx);
                            }
                        } else if this.page == Page::Dashboard {
                            this.refresh(cx);
                        }
                    }
                    Ok(Err(error)) => {
                        this.show_farm_operation_notice(error.to_string(), false, cx);
                    }
                    Err(error) => {
                        this.show_farm_operation_notice(
                            crate::i18n::background_task_error(&this.preferences.language, error),
                            false,
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn invalidate_farm_operations(&mut self) {
        self.farm.lands.operation_generation = self.farm.lands.operation_generation.wrapping_add(1);
        self.farm.lands.operating = false;
        self.farm.lands.finish_operation_after_refresh = false;
        self.farm.lands.operation_notice_generation =
            self.farm.lands.operation_notice_generation.wrapping_add(1);
        self.farm.lands.operation_notice = None;
    }

    fn show_farm_operation_notice(
        &mut self,
        message: impl Into<String>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        self.farm.lands.operation_notice_generation =
            self.farm.lands.operation_notice_generation.wrapping_add(1);
        let generation = self.farm.lands.operation_notice_generation;
        self.farm.lands.operation_notice = Some((message.into(), success));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.lands.operation_notice_generation == generation {
                    this.farm.lands.operation_notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
