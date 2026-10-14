//! 桌面进程状态：内嵌 `AppContext` + `LocalOwner` ACL。

use std::sync::Arc;

use qq_farm_app::accounts::AclPolicy;
use qq_farm_app::runtime_lease::EngineLease;
use qq_farm_app::session::AppContext;

/// Tauri `Managed` 状态。
#[derive(Clone)]
pub struct DesktopState {
    pub app: Arc<AppContext>,
    pub acl: AclPolicy,
    _lease: Option<Arc<EngineLease>>,
}

impl DesktopState {
    #[must_use]
    pub fn with_lease(app: Arc<AppContext>, lease: EngineLease) -> Self {
        Self { app, acl: AclPolicy::LocalOwner, _lease: Some(Arc::new(lease)) }
    }
}
