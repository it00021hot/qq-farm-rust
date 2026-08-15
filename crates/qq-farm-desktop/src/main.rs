//! qq-farm-desktop 占位入口。
//!
//! GPUI 依赖将在后续阶段引入；当前仅验证 qq-farm-app 链接。

fn main() {
    let _ = qq_farm_app::AppContext::new;
    println!("qq-farm-desktop placeholder — depends on qq-farm-app");
}
