//! 桌面进程状态：内嵌 `AppContext` + LocalOwner ACL。

use std::sync::Arc;

use qq_farm_app::accounts::AclPolicy;
use qq_farm_app::session::AppContext;

/// Tauri `Managed` 状态。
#[derive(Clone)]
pub struct DesktopState {
    pub app: Arc<AppContext>,
    pub acl: AclPolicy,
}

impl DesktopState {
    #[must_use]
    pub fn new(app: Arc<AppContext>) -> Self {
        Self {
            app,
            acl: AclPolicy::LocalOwner,
        }
    }
}
