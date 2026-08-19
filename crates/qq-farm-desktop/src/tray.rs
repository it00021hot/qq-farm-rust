//! 系统托盘：关窗进托盘、左键显隐、菜单与 Wails 对齐（无「在浏览器中打开」）。

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::shell;

const TRAY_ICON: &[u8] = include_bytes!("../icons/trayicon.png");
const TRAY_TEMPLATE: &[u8] = include_bytes!("../icons/trayicon-template.png");

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, shell::ID_SHOW_MAIN, "显示主窗口", true, None::<&str>)?;
    let open_data =
        MenuItem::with_id(app, shell::ID_OPEN_DATA_DIR, "打开数据目录", true, None::<&str>)?;
    let check_update =
        MenuItem::with_id(app, shell::ID_CHECK_UPDATE, "检查更新", true, None::<&str>)?;
    // macOS 上原生 AboutMetadata 的 id 固定为 "about"；这里用独立 id，避免与原生 about 冲突
    //（也避免你之前看到的 “about 两次”/“点了没反应”）。
    let about = MenuItem::with_id(app, shell::ID_ABOUT_TRAY, "关于", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, shell::ID_QUIT, "退出", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&open_data)
        .item(&sep1)
        .item(&check_update)
        .item(&about)
        .item(&sep2)
        .item(&quit)
        .build()?;

    let icon_bytes = if cfg!(target_os = "macos") { TRAY_TEMPLATE } else { TRAY_ICON };
    let icon = Image::from_bytes(icon_bytes)?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("QQ Farm")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| shell::handle_menu_event(app, event.id()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                shell::toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
