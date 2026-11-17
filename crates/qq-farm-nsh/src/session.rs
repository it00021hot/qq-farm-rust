use std::sync::{Arc, OnceLock};

use qq_farm_app::session::AppContext;
use tokio::runtime::Handle;

pub(crate) struct Session {
    pub context: Arc<AppContext>,
    pub runtime: Handle,
}

static SESSION: OnceLock<Session> = OnceLock::new();

pub(crate) fn install(session: Session) {
    SESSION.set(session).ok();
}

pub(crate) fn session() -> &'static Session {
    SESSION.get().expect("session installed before the window")
}

pub(crate) async fn call<T: Send + 'static>(
    task: impl std::future::Future<Output = qq_farm_app::AppResult<T>> + Send + 'static,
) -> Result<T, String> {
    match session().runtime.spawn(task).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error.to_string()),
        Err(error) => Err(error.to_string()),
    }
}
