//! macOS 原生菜单栏（Windows/Linux 无窗口菜单，动作在托盘）。

use tauri::menu::{AboutMetadata, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::AppHandle;

use crate::shell;

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let about = AboutMetadata {
        name: Some("QQ农场智能助手".into()),
        version: Some(app.package_info().version.to_string()),
        comments: Some("QQ农场智能助手桌面端".into()),
        copyright: Some("© 2026 QQFarm".into()),
        ..Default::default()
    };

    let app_menu = SubmenuBuilder::new(app, "QQ农场智能助手")
        .about(Some(about))
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window").minimize().maximize().build()?;

    let open_data = MenuItem::with_id(app, shell::ID_OPEN_DATA_DIR, "打开数据目录", true, None::<&str>)?;
    let check_update = MenuItem::with_id(app, shell::ID_CHECK_UPDATE, "检查更新", true, None::<&str>)?;
    let app_actions = SubmenuBuilder::new(app, "应用")
        .item(&open_data)
        .item(&check_update)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&app_menu)
        .item(&edit_menu)
        .item(&window_menu)
        .item(&app_actions)
        .build()?;
    app.set_menu(menu)?;
    Ok(())
}
