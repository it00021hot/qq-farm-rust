use super::super::*;
use crate::desktop::land_grid::visible_lands;
use gpui_kit::component::WindowExt;
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

/// 一键施肥候选数（对齐 bot `isFertilizeCandidate`：已解锁、非副地块、
/// 生长中且未成熟；只统计可见布局地块，与巡田操作按钮一致）。
pub(crate) fn fertilize_candidate_count(raw: &serde_json::Value) -> usize {
    let lands = raw
        .pointer("/lands/lands")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    visible_lands(&lands)
        .iter()
        .filter(|land| {
            js_truthy(land.get("unlocked"))
                && !js_truthy(land.get("occupiedByMaster"))
                && land.get("status").and_then(serde_json::Value::as_str) == Some("growing")
                && js_number(land.get("matureInSec")) > 0.
        })
        .count()
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
        controls.child(self.fertilize_all_button(cx))
    }

    fn fertilize_all_button(&self, cx: &mut Context<Self>) -> Button {
        let language = &self.preferences.language;
        let candidates = fertilize_candidate_count(&self.page_raw);
        Button::new("fertilize-all")
            .label(crate::i18n::text(language, "一键施肥", "Fertilize All"))
            .outline()
            .loading(self.farm.lands.fertilize_all_pending)
            .disabled(
                self.farm.lands.operating
                    || self.farm.lands.fertilize_all_pending
                    || self.selected_account.is_none()
                    || candidates == 0,
            )
            .on_click(cx.listener(|this, _, window, cx| this.open_fertilize_all_dialog(window, cx)))
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

    /// 一键施肥确认框（对齐 bot：确认 + 化肥类型选择；GPUI 用两个类型按钮代替下拉）。
    pub(in crate::desktop::personal) fn open_fertilize_all_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.farm.lands.fertilize_all_pending || self.farm.lands.operating {
            return;
        }
        let language = self.preferences.language.clone();
        let candidates = fertilize_candidate_count(&self.page_raw);
        if candidates == 0 {
            self.show_farm_operation_notice(
                crate::i18n::text(&language, "当前没有可施肥的作物", "No crops to fertilize")
                    .to_string(),
                true,
                cx,
            );
            return;
        }
        let english = language == "en-US";
        let count_text = if english {
            format!("{candidates} crops can be fertilized")
        } else {
            format!("可施肥 {candidates} 株作物")
        };
        let desktop = cx.entity();
        window.open_dialog(cx, move |dialog, _, app| {
            let pending = desktop.read(app).farm.lands.fertilize_all_pending;
            let normal_entity = desktop.clone();
            let organic_entity = desktop.clone();
            dialog
                .title(crate::i18n::text(&language, "一键施肥", "Fertilize All"))
                .w(px(440.))
                .child(
                    div()
                        .v_flex()
                        .gap_3()
                        .child(div().text_sm().child(crate::i18n::text(
                            &language,
                            "将为所有生长中的作物施用一次所选化肥。",
                            "Apply the selected fertilizer once to every growing crop.",
                        )))
                        .child({
                            let label = div()
                                .id("fertilize-all-count")
                                .aria_label(count_text.clone())
                                .text_sm()
                                .child(count_text.clone());
                            #[cfg(test)]
                            let label = label.test_support();
                            label
                        }),
                )
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("fertilize-all-cancel")
                                .label(crate::i18n::text(&language, "取消", "Cancel"))
                                .ghost()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("fertilize-all-normal")
                                .label(crate::i18n::text(
                                    &language,
                                    "普通化肥",
                                    "Normal fertilizer",
                                ))
                                .outline()
                                .loading(pending)
                                .disabled(pending)
                                .on_click(move |_, window, cx| {
                                    normal_entity.update(cx, |this, cx| {
                                        this.start_fertilize_all("normal", cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("fertilize-all-organic")
                                .label(crate::i18n::text(
                                    &language,
                                    "有机化肥",
                                    "Organic fertilizer",
                                ))
                                .primary()
                                .loading(pending)
                                .disabled(pending)
                                .on_click(move |_, window, cx| {
                                    organic_entity.update(cx, |this, cx| {
                                        this.start_fertilize_all("organic", cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
        cx.notify();
    }

    /// 执行一键施肥：单次批量请求，成功后就地刷新土地。
    pub(in crate::desktop::personal) fn start_fertilize_all(
        &mut self,
        fertilizer_type: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.farm.lands.fertilize_all_pending {
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
        self.farm.lands.fertilize_all_generation =
            self.farm.lands.fertilize_all_generation.wrapping_add(1);
        let generation = self.farm.lands.fertilize_all_generation;
        self.farm.lands.fertilize_all_pending = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::farm::fertilize_all(&app, &account_id, fertilizer_type).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.lands.fertilize_all_generation != generation {
                    return;
                }
                if this.selected_account.as_deref() != Some(account_id_guard.as_str()) {
                    this.farm.lands.fertilize_all_pending = false;
                    cx.notify();
                    return;
                }
                this.farm.lands.fertilize_all_pending = false;
                let language = this.preferences.language.clone();
                match result {
                    Ok(Ok(value)) => {
                        let success = value
                            .get("successCount")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0);
                        let skipped = value
                            .get("skippedCount")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0);
                        let remaining = value
                            .get("fertilizerRemainingSec")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0);
                        if success == 0 {
                            this.show_farm_operation_notice(
                                crate::i18n::text(
                                    &language,
                                    "当前没有可施肥的作物",
                                    "No crops to fertilize",
                                )
                                .to_string(),
                                true,
                                cx,
                            );
                        } else {
                            let mut message = if language == "en-US" {
                                format!("Fertilized {success} crops")
                            } else {
                                format!("已为 {success} 株作物施肥")
                            };
                            if skipped > 0 {
                                message += &if language == "en-US" {
                                    format!(", {skipped} skipped")
                                } else {
                                    format!("，{skipped} 株未施肥")
                                };
                            }
                            if remaining > 0 {
                                message += &if language == "en-US" {
                                    format!(
                                        ", fertilizer left: {}h {}m",
                                        remaining / 3600,
                                        (remaining % 3600) / 60
                                    )
                                } else {
                                    format!(
                                        "，化肥剩余 {}小时{}分钟",
                                        remaining / 3600,
                                        (remaining % 3600) / 60
                                    )
                                };
                            }
                            this.show_farm_operation_notice(message, true, cx);
                        }
                        if this.page == Page::Personal && this.page_section == "lands" {
                            this.refresh_personal_lands(cx);
                        }
                    }
                    Ok(Err(error)) => {
                        this.show_farm_operation_notice(error.to_string(), false, cx);
                    }
                    Err(error) => {
                        this.show_farm_operation_notice(
                            crate::i18n::background_task_error(&language, error),
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
        self.farm.lands.fertilize_all_generation =
            self.farm.lands.fertilize_all_generation.wrapping_add(1);
        self.farm.lands.fertilize_all_pending = false;
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
        let message = message.into();
        self.show_toast(message.clone(), success, cx);
        self.farm.lands.operation_notice_generation =
            self.farm.lands.operation_notice_generation.wrapping_add(1);
        let generation = self.farm.lands.operation_notice_generation;
        self.farm.lands.operation_notice = Some((message, success));
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
