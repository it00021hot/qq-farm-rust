use crate::desktop::{
    CancelOverlay, CheckForUpdates, FarmDesktop, FocusSearch, NextPage, OpenSettings, PreviousPage,
    QuitApp, RefreshPage, SaveChanges, ToggleSidebar,
};
use gpui_kit::{Action, AnyWindowHandle, App, Entity, KeyBinding, Menu, MenuItem};

#[derive(Debug, PartialEq, Eq)]
struct CommandLabels {
    view_menu: &'static str,
    refresh: &'static str,
    save: &'static str,
    check_updates: &'static str,
    quit: &'static str,
    search: &'static str,
    toggle_sidebar: &'static str,
    open_settings: &'static str,
    next_page: &'static str,
    previous_page: &'static str,
}

fn command_labels(language: &str) -> CommandLabels {
    let text = crate::i18n::text;
    CommandLabels {
        view_menu: text(language, "视图", "View"),
        refresh: text(language, "刷新", "Refresh"),
        save: text(language, "保存", "Save"),
        check_updates: text(language, "检查更新…", "Check for Updates…"),
        quit: text(language, "退出 QQ 农场", "Quit QQ Farm"),
        search: text(language, "搜索", "Search"),
        toggle_sidebar: text(language, "切换侧栏", "Toggle Sidebar"),
        open_settings: text(language, "打开设置", "Open Settings"),
        next_page: text(language, "下一个页面", "Next Page"),
        previous_page: text(language, "上一个页面", "Previous Page"),
    }
}

pub(crate) fn configure_commands(cx: &mut App, language: &str) {
    cx.bind_keys([
        KeyBinding::new("secondary-r", RefreshPage, None),
        KeyBinding::new("secondary-f", FocusSearch, None),
        KeyBinding::new("secondary-,", OpenSettings, None),
        KeyBinding::new("secondary-s", SaveChanges, None),
        KeyBinding::new("secondary-q", QuitApp, None),
        KeyBinding::new("secondary-b", ToggleSidebar, None),
        KeyBinding::new("escape", CancelOverlay, None),
        KeyBinding::new("alt-down", NextPage, None),
        KeyBinding::new("alt-up", PreviousPage, None),
    ]);
    set_application_menus(cx, language);
}

pub(crate) fn set_application_menus(cx: &mut App, language: &str) {
    let labels = command_labels(language);
    cx.set_global(crate::i18n::ActiveLanguage(language.to_owned()));
    cx.set_menus([
        Menu::new("QQ Farm").items([
            MenuItem::action(labels.refresh, RefreshPage),
            MenuItem::action(labels.save, SaveChanges),
            MenuItem::action(labels.check_updates, CheckForUpdates),
            MenuItem::separator(),
            MenuItem::action(labels.quit, QuitApp),
        ]),
        Menu::new(labels.view_menu).items([
            MenuItem::action(labels.search, FocusSearch),
            MenuItem::action(labels.toggle_sidebar, ToggleSidebar),
            MenuItem::action(labels.open_settings, OpenSettings),
            MenuItem::separator(),
            MenuItem::action(labels.next_page, NextPage),
            MenuItem::action(labels.previous_page, PreviousPage),
        ]),
    ]);
}

pub(crate) fn register_window_commands(
    view: &Entity<FarmDesktop>,
    window_handle: AnyWindowHandle,
    cx: &mut App,
) {
    register_action::<SaveChanges>(view, window_handle, cx);
    register_action::<FocusSearch>(view, window_handle, cx);
    register_action::<CancelOverlay>(view, window_handle, cx);
    register_action::<QuitApp>(view, window_handle, cx);
    register_action::<RefreshPage>(view, window_handle, cx);
    register_action::<OpenSettings>(view, window_handle, cx);
    register_action::<CheckForUpdates>(view, window_handle, cx);
    register_action::<NextPage>(view, window_handle, cx);
    register_action::<PreviousPage>(view, window_handle, cx);
    register_action::<ToggleSidebar>(view, window_handle, cx);
}

fn register_action<A: Action>(
    view: &Entity<FarmDesktop>,
    window_handle: AnyWindowHandle,
    cx: &mut App,
) {
    let view = view.downgrade();
    cx.on_action(move |action: &A, cx| {
        // 键盘分发期间当前窗口已被借出，延后回到窗口以兼容原生 sheet 的焦点树。
        let view = view.clone();
        let action = action.boxed_clone();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = view
                    .update(cx, |this, cx| this.dispatch_ui_command(action.as_ref(), window, cx));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::command_labels;

    #[test]
    fn application_menu_labels_follow_the_selected_language() {
        let chinese = command_labels("zh-CN");
        assert_eq!(chinese.view_menu, "视图");
        assert_eq!(chinese.check_updates, "检查更新…");
        assert_eq!(chinese.toggle_sidebar, "切换侧栏");

        let english = command_labels("en-US");
        assert_eq!(english.view_menu, "View");
        assert_eq!(english.check_updates, "Check for Updates…");
        assert_eq!(english.toggle_sidebar, "Toggle Sidebar");
        assert_eq!(english.quit, "Quit QQ Farm");
    }

    #[gpui_kit::test]
    fn active_language_is_shared_with_global_asset_rendering(cx: &mut gpui_kit::TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| super::set_application_menus(cx, "en-US"));
        cx.update(|cx| {
            assert_eq!(cx.global::<crate::i18n::ActiveLanguage>().0.as_str(), "en-US");
        });
    }
}
