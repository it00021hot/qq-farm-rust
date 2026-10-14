use super::super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "charity" => qq_farm_app::activity::claim_charity_daily_gift(&app, &account_id).await,
        "charity-seeds" => qq_farm_app::activity::claim_charity_seeds(&app, &account_id).await,
        "charity-donate" => qq_farm_app::activity::donate_charity_love(&app, &account_id).await,
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
