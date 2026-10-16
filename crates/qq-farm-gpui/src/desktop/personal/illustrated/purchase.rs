use gpui_kit::component::input::NumberInput;
use gpui_kit::component::WindowExt;

use super::super::super::*;

/// 可直接购买：有商品 ID 且未锁定/未售罄/非仅本地目录（对齐 bot IllustratedPanel 的按钮门控）。
pub(super) fn can_buy(item: &serde_json::Value) -> bool {
    let seed = item.get("shopSeed");
    let goods_id =
        seed.and_then(|s| s.get("goodsId")).and_then(serde_json::Value::as_i64).unwrap_or(0);
    let flag = |key: &str| seed.and_then(|s| s.get(key)).and_then(serde_json::Value::as_bool);
    goods_id > 0
        && flag("locked") != Some(true)
        && flag("soldOut") != Some(true)
        && flag("unknownMeta") != Some(true)
}

/// 不可购买原因（对齐 bot seedAvailability 文案语义）；无可买信息时返回 None。
pub(super) fn seed_availability_text(item: &serde_json::Value, language: &str) -> Option<String> {
    let english = language == "en-US";
    let seed = item.get("shopSeed").filter(|seed| !seed.is_null())?;
    Some(if seed.get("unknownMeta").and_then(serde_json::Value::as_bool) == Some(true) {
        if english {
            "From events/other sources".into()
        } else {
            "活动/其他途径获取".into()
        }
    } else if seed.get("locked").and_then(serde_json::Value::as_bool) == Some(true) {
        if seed.get("requiresUnlockCard").and_then(serde_json::Value::as_bool) == Some(true) {
            if english {
                "Requires unlock card".into()
            } else {
                "需要种子解锁卡".into()
            }
        } else if let Some(level) =
            seed.get("requiredLevel").and_then(serde_json::Value::as_i64).filter(|l| *l > 0)
        {
            if english {
                format!("Unlocks at Lv.{level}")
            } else {
                format!("{level}级解锁")
            }
        } else if english {
            "Other unlock conditions".into()
        } else {
            "需要其他解锁条件".into()
        }
    } else if seed.get("soldOut").and_then(serde_json::Value::as_bool) == Some(true) {
        if english {
            "Purchase limit reached".into()
        } else {
            "已达到限购数量".into()
        }
    } else if english {
        "Cannot buy now".into()
    } else {
        "当前无法购买".into()
    })
}

/// 购买批次上限：优先剩余限购，否则 9999（对齐 bot purchaseLimit）
fn purchase_limit(item: &serde_json::Value) -> f64 {
    item.pointer("/shopSeed/remainingPurchaseCount")
        .and_then(serde_json::Value::as_i64)
        .filter(|n| *n > 0)
        .map_or(9_999., |n| n as f64)
}

impl FarmDesktop {
    /// 图鉴页操作提示（购买成功/失败）
    pub(super) fn show_illustrated_notice(
        &mut self,
        message: impl Into<String>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        let message = message.into();
        self.show_toast(message.clone(), success, cx);
        self.farm.illustrated.notice_generation =
            self.farm.illustrated.notice_generation.wrapping_add(1);
        self.farm.illustrated.notice = Some((message, success));
        cx.notify();
    }

    /// 打开图鉴种子购买弹窗（对齐 bot IllustratedPanel 购买对话框）。
    pub(in crate::desktop) fn open_seed_purchase_confirmation(
        &mut self,
        item: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.farm.illustrated.purchase_pending {
            return;
        }
        self.farm
            .illustrated
            .purchase_count
            .update(cx, |state, cx| state.set_value("1", window, cx));
        let language = self.preferences.language.clone();
        let english = language == "en-US";
        let muted_foreground = cx.theme().muted_foreground;
        let name =
            item.get("name").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
        let seed = item.get("shopSeed").cloned().unwrap_or_default();
        let item_count =
            seed.get("itemCount").and_then(serde_json::Value::as_i64).unwrap_or(1).max(1);
        let price = seed.get("price").and_then(serde_json::Value::as_i64).unwrap_or(0);
        let limit = purchase_limit(&item);
        self.farm
            .illustrated
            .purchase_count
            .update(cx, |state, cx| state.set_max(Some(limit), window, cx));
        self.farm.illustrated.pending_purchase = Some(item);
        self.farm.illustrated.purchase_dialog_open = true;
        let purchase_count = self.farm.illustrated.purchase_count.clone();
        let desktop = cx.entity();
        let state_entity = desktop.clone();
        let close_entity = desktop.clone();
        let confirm_entity = desktop;
        window.open_dialog(cx, move |dialog, _, app| {
            let purchase_pending = state_entity.read(app).farm.illustrated.purchase_pending;
            let confirm_entity = confirm_entity.clone();
            let close_entity = close_entity.clone();
            let batch_line = if english {
                format!("{item_count} per pack · {price} gold")
            } else {
                format!("每批 {item_count} 颗 · {price} 金")
            };
            let count_value = purchase_count
                .read(app)
                .value()
                .trim()
                .parse::<i64>()
                .unwrap_or(1)
                .clamp(1, limit as i64);
            let total = price.saturating_mul(count_value);
            dialog
                .title(if english {
                    format!("Buy Seeds · {name}")
                } else {
                    format!("购买种子 · {name}")
                })
                .w(px(420.))
                .child(
                    div()
                        .v_flex()
                        .gap_4()
                        .child(div().text_xs().text_color(muted_foreground).child(batch_line))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_3()
                                .child(crate::i18n::text(&language, "购买批次", "Packs"))
                                .child(NumberInput::new(&purchase_count).w(px(148.))),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .text_sm()
                                .child(crate::i18n::text(&language, "合计", "Total"))
                                .child(div().font_medium().child(format!("{total}"))),
                        )
                        .child(div().text_xs().text_color(muted_foreground).child(
                            crate::i18n::text(
                                &language,
                                "服务端会再次检查余额和限购。",
                                "The server will check your balance and purchase limit again.",
                            ),
                        )),
                )
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("illustrated-purchase-cancel")
                                .label(crate::i18n::text(&language, "取消", "Cancel"))
                                .ghost()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("illustrated-purchase-confirm")
                                .label(crate::i18n::text(&language, "确认购买", "Confirm"))
                                .primary()
                                .loading(purchase_pending)
                                .disabled(purchase_pending)
                                .on_click(move |_, _window, cx| {
                                    confirm_entity
                                        .update(cx, |this, cx| this.confirm_seed_purchase(cx));
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    close_entity.update(cx, |this, cx| {
                        this.farm.illustrated.pending_purchase = None;
                        this.farm.illustrated.purchase_dialog_open = false;
                        cx.notify();
                    });
                })
        });
    }

    /// 确认购买（对齐 bot purchaseSeed 调用链：门控在服务端，这里只透传 goodsId/count）。
    pub(super) fn confirm_seed_purchase(&mut self, cx: &mut Context<Self>) {
        if self.farm.illustrated.purchase_pending {
            return;
        }
        let Some(item) = self.farm.illustrated.pending_purchase.clone() else { return };
        let Some(account_id) = self.selected_account.clone() else { return };
        let goods_id =
            item.pointer("/shopSeed/goodsId").and_then(serde_json::Value::as_i64).unwrap_or(0);
        if goods_id <= 0 {
            return;
        }
        let count = item
            .pointer("/shopSeed/remainingPurchaseCount")
            .and_then(serde_json::Value::as_i64)
            .filter(|n| *n > 0)
            .unwrap_or(9_999);
        let count = self
            .farm
            .illustrated
            .purchase_count
            .read(cx)
            .value()
            .trim()
            .parse::<i64>()
            .unwrap_or(1)
            .clamp(1, count);
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let page = self.page;
        let page_section = self.page_section.clone();
        self.farm.illustrated.purchase_generation =
            self.farm.illustrated.purchase_generation.wrapping_add(1);
        let generation = self.farm.illustrated.purchase_generation;
        self.farm.illustrated.purchase_pending = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::seed_purchase(&app, &account_id, goods_id, count)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.illustrated.purchase_generation != generation {
                    return;
                }
                this.farm.illustrated.purchase_pending = false;
                if this.selected_account.as_deref() != Some(account_id_check.as_str())
                    || this.page != page
                    || this.page_section != page_section
                {
                    this.farm.illustrated.pending_purchase = None;
                    if this.farm.illustrated.purchase_dialog_open {
                        this.farm.illustrated.purchase_dialog_open = false;
                        if let Some(handle) = this.window_handle {
                            let _ = handle.update(cx, |_, window, cx| window.close_dialog(cx));
                        }
                    }
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(_)) => {
                        let name =
                            item.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
                        this.show_illustrated_notice(
                            if this.preferences.language == "en-US" {
                                format!("Seeds purchased: {name}")
                            } else {
                                format!("种子购买成功：{name}")
                            },
                            true,
                            cx,
                        );
                        if this.farm.illustrated.purchase_dialog_open {
                            this.farm.illustrated.purchase_dialog_open = false;
                            if let Some(handle) = this.window_handle {
                                let _ = handle.update(cx, |_, window, cx| window.close_dialog(cx));
                            }
                        }
                        this.refresh(cx);
                    }
                    Ok(Err(error)) => {
                        this.show_illustrated_notice(error, false, cx);
                    }
                    Err(error) => {
                        this.show_illustrated_notice(
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
}
