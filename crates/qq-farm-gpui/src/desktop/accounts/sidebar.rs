use super::super::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

impl FarmDesktop {
    /// Renders the sidebar's bottom account switcher, mirroring the Tauri
    /// shell: one compact control that names the current account and switches
    /// between saved accounts.
    pub(in crate::desktop) fn render_sidebar_account_switcher(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = &self.preferences.language;
        let current_name = self
            .accounts
            .records
            .iter()
            .find(|(id, _, _)| self.selected_account.as_deref() == Some(id.as_str()))
            .map(|(_, name, _)| name.clone());
        let trigger_label = current_name.clone().unwrap_or_else(|| {
            crate::i18n::text(language, "请选择账号", "Select account").to_owned()
        });
        let accessibility_label = match current_name {
            Some(name) => crate::i18n::text(
                language,
                &format!("当前账号：{name}"),
                &format!("Current account: {name}"),
            )
            .to_owned(),
            None => crate::i18n::text(language, "当前账号", "Current account").to_owned(),
        };
        let empty_label = crate::i18n::text(language, "尚未添加账号", "No accounts yet").to_owned();
        let records = self.accounts.records.clone();
        let selected = self.selected_account.clone();
        let desktop = cx.entity().downgrade();

        Button::new("sidebar-account-switcher")
            .label(trigger_label)
            .accessibility_label(accessibility_label)
            .dropdown_caret(true)
            .ghost()
            .w_full()
            .disabled(self.quitting)
            .dropdown_menu(move |mut menu, _, _| {
                if records.is_empty() {
                    return menu.item(PopupMenuItem::new(empty_label.clone()).disabled(true));
                }
                for (id, name, _) in records.iter().cloned() {
                    let desktop = desktop.clone();
                    let checked = selected.as_deref() == Some(id.as_str());
                    menu = menu.item(PopupMenuItem::new(name).checked(checked).on_click(
                        move |_, window, cx| {
                            let account_id = id.clone();
                            let _ = desktop
                                .update(cx, |this, cx| this.set_account(account_id, window, cx));
                        },
                    ));
                }
                menu
            })
    }
}
