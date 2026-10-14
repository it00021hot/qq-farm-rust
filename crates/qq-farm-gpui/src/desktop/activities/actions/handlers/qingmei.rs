use super::super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
    qingmei_input: Option<serde_json::Value>,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "qingmei" => qq_farm_app::activity::claim_qingmei_seed(&app, &account_id).await,
        "qingmei-start" => {
            qq_farm_app::activity::start_qingmei_brew(
                &app,
                &account_id,
                qingmei_input.unwrap_or_else(|| serde_json::json!({"ingredients": []})),
            )
            .await
        }
        "qingmei-continue" => qq_farm_app::activity::continue_qingmei_brew(&app, &account_id).await,
        "qingmei-settle" => qq_farm_app::activity::settle_qingmei_brew(&app, &account_id).await,
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
