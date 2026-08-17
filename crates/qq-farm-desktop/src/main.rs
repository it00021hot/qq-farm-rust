//! QQ Farm Tauri 桌面端入口。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    qq_farm_desktop_lib::run();
}
