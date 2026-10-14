use super::super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "pet-diary-feed" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "feed",
                &serde_json::json!({}),
            )
            .await
        }
        "pet-diary-initialize" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "initialize",
                &serde_json::json!({}),
            )
            .await
        }
        "pet-diary-draw" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "draw",
                &serde_json::json!({}),
            )
            .await
        }
        "pet-diary-seeds" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "seeds",
                &serde_json::json!({}),
            )
            .await
        }
        "pet-diary-claim-dog" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "claimDog",
                &serde_json::json!({}),
            )
            .await
        }
        "pet-diary-open-treasure" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "openTreasure",
                &serde_json::json!({}),
            )
            .await
        }
        "pet-diary-compensation" => {
            qq_farm_app::activity::operate_pet_diary(
                &app,
                &account_id,
                "compensation",
                &serde_json::json!({}),
            )
            .await
        }
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
