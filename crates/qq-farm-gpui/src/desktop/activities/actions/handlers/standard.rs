use super::super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "battle-pass" => qq_farm_app::activity::claim_battle_pass(&app, &account_id).await,
        "constellation" => qq_farm_app::activity::light_constellation(&app, &account_id).await,
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
