use super::*;
use gpui_kit::component::WindowExt;

impl FarmDesktop {
    pub(crate) fn dispatch_ui_command(
        &mut self,
        action: &dyn Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.quitting {
            return;
        }
        let action = action.as_any();
        if action.is::<SaveChanges>() {
            self.save_current_changes(window, cx);
        } else if action.is::<FocusSearch>() {
            self.focus_page_search(window, cx);
        } else if action.is::<CancelOverlay>() {
            self.cancel_overlay(window, cx);
        } else if action.is::<QuitApp>() {
            self.request_quit(cx);
        } else if action.is::<RefreshPage>() {
            self.refresh(cx);
        } else if action.is::<CheckForUpdates>() {
            self.check_update(cx);
        } else if action.is::<OpenSettings>() {
            self.set_page(Page::Settings, cx);
            self.sync_search_placeholder(window, cx);
        } else if action.is::<NextPage>() {
            self.step_page(1, cx);
            self.sync_search_placeholder(window, cx);
        } else if action.is::<PreviousPage>() {
            self.step_page(-1, cx);
            self.sync_search_placeholder(window, cx);
        } else if action.is::<ToggleSidebar>() {
            self.sidebar_collapsed = !self.sidebar_collapsed;
            self.preferences.sidebar_collapsed = self.sidebar_collapsed;
            self.persist_ui_preferences();
            cx.notify();
        }
    }

    pub(crate) fn focus_page_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.page, self.page_section.as_str()) {
            (Page::Accounts, _) => {
                // The account page has its own collapsible search form and hides the shared
                // toolbar search. Open the form before moving keyboard focus into it.
                self.accounts.search_open = true;
                let search = self.accounts.keyword.clone();
                window.defer(cx, move |window, cx| {
                    search.read(cx).focus_handle(cx).focus(window, cx);
                });
            }
            (Page::GameConfig, _) => {
                // Game-config search lives in a collapsed page panel, not the toolbar.
                self.game_config.search_open = true;
                let search = self.search.clone();
                window.defer(cx, move |window, cx| {
                    search.read(cx).focus_handle(cx).focus(window, cx);
                });
            }
            (Page::Analytics, "plantBlacklist") => {
                self.analytics.blacklist_selector.update(cx, |selector, cx| {
                    selector.focus(window, cx);
                });
            }
            (Page::Analytics, "strategy") => {
                let level = self.analytics.level.clone();
                window.defer(cx, move |window, cx| {
                    level.read(cx).focus_handle(cx).focus(window, cx);
                });
            }
            _ => {}
        }
        cx.notify();
    }

    pub(crate) fn cancel_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            window.close_dialog(cx);
            self.commerce.pending_purchase = None;
            self.cancel_account_delete_confirmation(cx);
            self.cancel_game_config_delete_confirmation(cx);
        } else if self.game_config.pending_delete.is_some() {
            self.cancel_game_config_delete_confirmation(cx);
        } else if self.accounts.column_picker_open {
            self.accounts.column_picker_open = false;
        } else if window.has_active_sheet(cx) {
            window.close_sheet(cx);
            self.close_account_form();
        } else {
            self.close_account_form();
            self.cancel_account_delete_confirmation(cx);
            self.cancel_game_config_delete_confirmation(cx);
            self.commerce.pending_purchase = None;
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn save_current_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        if self.accounts.form_open {
            if self.accounts.login_tab == "code" {
                self.save_account(cx);
            }
        } else if self.game_config.form_open && self.page == Page::GameConfig {
            self.save_game_config_form(window, cx);
        } else if self.page == Page::Settings {
            if self.settings.edit_path.is_some() {
                self.save_setting_edit(cx);
            } else {
                match self.page_section.as_str() {
                    "systemConfig" => self.save_system_config(cx),
                    "offlineReminder" => self.save_offline_reminder(cx),
                    "ui" => {}
                    _ => self.save_settings(cx),
                }
            }
        }
    }
}
