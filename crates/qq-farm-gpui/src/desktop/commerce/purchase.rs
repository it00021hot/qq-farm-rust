use std::time::Duration;

use gpui_kit::component::WindowExt;

use super::super::*;

fn bounded_purchase_count(value: &str) -> i64 {
    value.trim().parse::<i64>().unwrap_or(1).clamp(1, 99)
}

impl FarmDesktop {
    pub(super) fn confirm_purchase(&mut self, cx: &mut Context<Self>) {
        if self.commerce.purchase_pending {
            return;
        }
        let Some(goods) = self.commerce.pending_purchase.clone() else { return };
        let Some(account_id) = self.selected_account.clone() else { return };
        let account_id_check = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let page = self.page;
        let page_section = self.page_section.clone();
        let count = bounded_purchase_count(&self.commerce.purchase_count.read(cx).value());
        let request_count = count;
        let slot_type = self.commerce.mall_slot_type;
        self.commerce.purchase_generation = self.commerce.purchase_generation.wrapping_add(1);
        let generation = self.commerce.purchase_generation;
        self.commerce.purchase_pending = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    match page {
                        Page::MysteryShop | Page::Mall if slot_type == -1 => {
                            let id = goods
                                .get("offerId")
                                .or_else(|| goods.get("id"))
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string)
                                .or_else(|| {
                                    goods
                                        .get("offerId")
                                        .or_else(|| goods.get("id"))
                                        .and_then(serde_json::Value::as_i64)
                                        .map(|id| id.to_string())
                                })
                                .ok_or_else(|| "神秘商人商品编号缺失".to_string())?;
                            qq_farm_app::commerce::purchase_mystery(&app, &account_id, &id)
                                .await
                                .map_err(|error| error.to_string())
                        }
                        Page::Mall => {
                            let id = goods
                                .get("id")
                                .and_then(serde_json::Value::as_i64)
                                .ok_or_else(|| "商品编号缺失".to_string())?;
                            let expected = goods
                                .get("price")
                                .cloned()
                                .and_then(|price| serde_json::from_value(price).ok());
                            qq_farm_app::commerce::purchase_mall(
                                &app,
                                &account_id,
                                id,
                                request_count,
                                Some(slot_type),
                                expected,
                            )
                            .await
                            .map_err(|error| error.to_string())
                        }
                        _ => Err("当前页面不支持购买".into()),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.commerce.purchase_generation != generation {
                    return;
                }
                this.commerce.purchase_pending = false;
                if this.selected_account.as_deref() != Some(account_id_check.as_str())
                    || this.page != page
                    || this.page_section != page_section
                    || (page == Page::Mall && this.commerce.mall_slot_type != slot_type)
                {
                    this.commerce.pending_purchase = None;
                    if this.commerce.purchase_dialog_open {
                        this.commerce.purchase_dialog_open = false;
                        if let Some(handle) = this.window_handle {
                            let _ = handle.update(cx, |_, window, cx| window.close_dialog(cx));
                        }
                    }
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(_)) => {
                        this.commerce.pending_purchase = None;
                        if this.preferences.language == "en-US" {
                            this.show_commerce_notice("Purchase successful", true, cx);
                        } else {
                            this.show_commerce_notice("购买成功", true, cx);
                        }
                        if this.commerce.purchase_dialog_open {
                            this.commerce.purchase_dialog_open = false;
                            if let Some(handle) = this.window_handle {
                                let _ = handle.update(cx, |_, window, cx| window.close_dialog(cx));
                            }
                        }
                        this.refresh(cx);
                    }
                    Ok(Err(error)) => {
                        this.error = Some(error);
                        cx.notify();
                    }
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn show_commerce_notice(
        &mut self,
        message: impl Into<String>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        let message = message.into();
        self.show_toast(message.clone(), success, cx);
        self.commerce.notice_generation = self.commerce.notice_generation.wrapping_add(1);
        let generation = self.commerce.notice_generation;
        self.commerce.notice = Some((message, success));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.commerce.notice_generation == generation {
                    this.commerce.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_purchase_count;

    #[test]
    fn purchase_count_matches_the_vue_one_to_ninety_nine_range() {
        assert_eq!(bounded_purchase_count("1"), 1);
        assert_eq!(bounded_purchase_count("99"), 99);
        assert_eq!(bounded_purchase_count("100"), 99);
        assert_eq!(bounded_purchase_count("0"), 1);
        assert_eq!(bounded_purchase_count("invalid"), 1);
    }
}
