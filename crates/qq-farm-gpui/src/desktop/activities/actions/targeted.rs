use super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
    target: String,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "solar-term" => qq_farm_app::activity::claim_solar_term(&app, &account_id, &target).await,
        "star-sand" => {
            qq_farm_app::activity::exchange_star_sand(
                &app,
                &account_id,
                &serde_json::Value::String(target),
                &serde_json::json!(1),
            )
            .await
        }
        "pet-diary-exchange" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "exchange",
                &serde_json::json!({"goodsId": target, "count": 1}),
            )
            .await
        }
        "charity-progress" => {
            qq_farm_app::activity::claim_charity_progress_reward(&app, &account_id, &target).await
        }
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
