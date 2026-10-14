use super::super::*;

/// The shared toolbar search uses the page-specific placeholder Vue shows for
/// its own search fields; pages without one keep the generic wording.
pub(in crate::desktop) fn search_placeholder_for(page: Page, language: &str) -> &'static str {
    match page {
        Page::Mall => crate::i18n::text(language, "搜索商品名称 / ID", "Search item name / ID"),
        Page::Friends => crate::i18n::text(language, "搜索昵称 / GID", "Search nickname / GID"),
        Page::GameConfig => crate::i18n::text(language, "请输入关键词搜索", "Please enter keyword"),
        _ => crate::i18n::text(language, "搜索当前页面", "Search this page"),
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn sync_search_placeholder(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = search_placeholder_for(self.page, &self.preferences.language);
        self.search.update(cx, |state, cx| state.set_placeholder(placeholder, window, cx));
    }

    pub(in crate::desktop) fn persist_ui_preferences(&self) {
        let directory = self._lease.path().parent().expect("engine lease data directory");
        if let Err(error) = crate::runtime::save_ui_preferences(directory, &self.preferences) {
            tracing::warn!(%error, "could not save GPUI UI preferences");
        }
    }

    pub(in crate::desktop) fn set_appearance(
        &mut self,
        appearance: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::component::theme::{Theme, ThemeMode};

        self.preferences.appearance = appearance.into();
        let mode = match appearance {
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            _ => ThemeMode::from(window.appearance()),
        };
        Theme::change(mode, Some(window), cx);
        self.persist_ui_preferences();
        cx.notify();
    }

    pub(in crate::desktop) fn set_language(
        &mut self,
        language: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.language = language.into();
        if let Some(tray) = &self._tray {
            tray.set_language(language);
        }
        self.search.update(cx, |state, cx| {
            state.set_placeholder(search_placeholder_for(self.page, language), window, cx)
        });
        self.accounts.keyword.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(
                    language,
                    "请输入账号名称、QQ 号或备注",
                    "Search account name, QQ number, or notes",
                ),
                window,
                cx,
            )
        });
        self.accounts.name.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(
                    language,
                    "留空默认「账号ID」",
                    "Optional, defaults to account#{id}",
                ),
                window,
                cx,
            )
        });
        self.accounts.code.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(
                    language,
                    "可粘贴裸 Code，或完整 URL（含 platform/os/ver/code）",
                    "Paste bare code or full URL (platform/os/ver/code)",
                ),
                window,
                cx,
            )
        });
        self.accounts.remark.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "账号备注", "Account notes"),
                window,
                cx,
            )
        });
        self.dashboard.log_keyword.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "关键词", "Keyword"), window, cx)
        });
        self.friends.interaction_gid.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "好友 GID", "Friend GID"), window, cx)
        });
        self.friends.interaction_land_ids.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "例如： 1,2,3", "e.g. 1,2,3"),
                window,
                cx,
            )
        });
        self.farm.interaction.land_ids.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "例如： 1,2,3", "e.g. 1,2,3"),
                window,
                cx,
            )
        });
        for input in self.farm.pets.food_inputs.values() {
            input.update(cx, |state, cx| {
                state.set_placeholder(crate::i18n::text(language, "数量", "Quantity"), window, cx)
            });
        }
        self.analytics.level.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "参考等级", "Reference level"),
                window,
                cx,
            )
        });
        self.commerce.purchase_count.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "数量", "Quantity"), window, cx)
        });
        self.game_config.id.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "编号", "ID"), window, cx)
        });
        self.game_config.name.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "名称", "Name"), window, cx)
        });
        self.game_config.level.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "等级要求", "Required level"),
                window,
                cx,
            )
        });
        self.game_config.desc.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "描述", "Description"), window, cx)
        });
        self.game_config.asset.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "资源名", "Asset name"), window, cx)
        });
        self.game_config.image_path.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "图片文件路径（可选）", "Image path (optional)"),
                window,
                cx,
            )
        });
        for (key, placeholder) in crate::desktop::game_config::form::EXTRA_FIELDS {
            if let Some(input) = self.game_config.extra_fields.get(*key) {
                let placeholder =
                    crate::desktop::game_config::labels::form_field(language, placeholder);
                input.update(cx, |state, cx| state.set_placeholder(placeholder, window, cx));
            }
        }
        for (key, placeholder) in [("growPhases", "生长阶段"), ("effectDesc", "效果描述")] {
            if let Some(input) = self.game_config.extra_textareas.get(key) {
                let placeholder =
                    crate::desktop::game_config::labels::form_field(language, placeholder);
                input.update(cx, |state, cx| state.set_placeholder(placeholder, window, cx));
            }
        }
        self.activities.weather.search.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "搜索好友名称或 GID", "Search friends by name or GID"),
                window,
                cx,
            )
        });
        self.activities.qixi_friend_search.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "搜索好友", "Search friends"),
                window,
                cx,
            )
        });
        self.activities.qixi_gift_count.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "赠送数量", "Gift quantity"),
                window,
                cx,
            )
        });
        self.activities.pet_diary_exchange_count.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "数量", "Quantity"), window, cx)
        });
        self.activities.pet_diary_goods_id.update(cx, |state, cx| {
            state.set_placeholder(crate::i18n::text(language, "商品编号", "Item ID"), window, cx)
        });
        self.settings.offline_endpoint.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(
                    language,
                    "https://oapi.dingtalk.com/robot/send?access_token=…（与 Access Token 二选一）",
                    "https://oapi.dingtalk.com/robot/send?access_token=... (either this or the access token)",
                ),
                window,
                cx,
            )
        });
        self.settings.offline_token.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(
                    language,
                    "与 Webhook 地址二选一",
                    "Either the webhook URL or this",
                ),
                window,
                cx,
            )
        });
        self.settings.offline_secret.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(
                    language,
                    "可选；安全设置选择加签时填写 SEC 开头密钥",
                    "Optional; SEC-prefixed secret when the bot uses signed mode",
                ),
                window,
                cx,
            )
        });
        self.settings.edit_value.update(cx, |state, cx| {
            state.set_placeholder(
                crate::i18n::text(language, "设置值", "Setting value"),
                window,
                cx,
            )
        });
        self.sync_account_filter_selectors(window, cx);
        self.sync_pet_diary_friend_selector(window, cx);
        self.update_setting_selector_language(window, cx);
        self.sync_game_config_selectors(window, cx);
        self.persist_ui_preferences();
        cx.notify();
        let language = language.to_owned();
        cx.defer(move |cx| crate::application::set_application_menus(cx, &language));
    }

    pub(in crate::desktop) fn step_page(&mut self, delta: isize, cx: &mut Context<Self>) {
        let current = Page::ALL.iter().position(|page| *page == self.page).unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(Page::ALL.len() as isize) as usize;
        self.set_page(Page::ALL[next], cx);
    }
}
