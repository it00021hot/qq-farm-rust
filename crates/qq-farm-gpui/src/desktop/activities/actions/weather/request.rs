use qq_farm_app::session::AppContext;

#[derive(Clone, Copy)]
pub(super) enum WeatherOperation {
    Bottle { action: &'static str, gid: Option<i64> },
    Research(i64),
}

impl WeatherOperation {
    pub(super) fn action(self) -> &'static str {
        match self {
            Self::Bottle { action, .. } => action,
            Self::Research(_) => "research",
        }
    }
    pub(super) fn key(self) -> String {
        match self {
            Self::Bottle { action, .. } => action.into(),
            Self::Research(id) => format!("research:{id}"),
        }
    }
    pub(super) async fn execute(
        self,
        app: &AppContext,
        account_id: &str,
    ) -> qq_farm_app::AppResult<serde_json::Value> {
        match self {
            Self::Bottle { action: "summon", .. } => {
                qq_farm_app::weather::summon(app, account_id).await
            }
            Self::Bottle { action: "exchange", .. } => {
                qq_farm_app::weather::exchange_collector(app, account_id).await
            }
            Self::Bottle { action: "scan", gid: Some(gid) } => {
                qq_farm_app::weather::scan_friends(app, account_id, &[gid]).await
            }
            Self::Bottle { action: "collect", gid: Some(gid) } => {
                qq_farm_app::weather::collect(app, account_id, gid).await
            }
            Self::Bottle { action: "frog", gid: Some(gid) } => {
                qq_farm_app::weather::mischief_frog(app, account_id, gid).await
            }
            Self::Bottle { action: "cloud", gid: Some(gid) } => {
                qq_farm_app::weather::mischief_cloud(app, account_id, gid, None).await
            }
            Self::Research(id) => qq_farm_app::weather::advance_research(app, account_id, id).await,
            _ => Err(qq_farm_app::AppError::BadRequest("未知天气操作或缺少好友".into())),
        }
    }
}
