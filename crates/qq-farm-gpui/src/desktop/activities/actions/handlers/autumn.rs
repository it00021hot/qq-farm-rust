use super::super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
    autumn_choice: Option<String>,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "autumn-daily" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "happy",
                "daily",
                &serde_json::json!({}),
            )
            .await
        }
        "autumn-happy-daily" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "happy",
                "daily",
                &serde_json::json!({}),
            )
            .await
        }
        "autumn-happy-share" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "happy",
                "share",
                &serde_json::json!({}),
            )
            .await
        }
        "autumn-happy-milestones" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "happy",
                "milestones",
                &serde_json::json!({}),
            )
            .await
        }
        "autumn-happy-logs-mine" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "happy",
                "logs",
                &serde_json::json!({"tab": 0}),
            )
            .await
        }
        "autumn-happy-logs-others" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "happy",
                "logs",
                &serde_json::json!({"tab": 1}),
            )
            .await
        }
        "autumn-draw" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "wish",
                "draw",
                &serde_json::json!({"chooseId": autumn_choice.unwrap_or_default()}),
            )
            .await
        }
        "autumn-claim" => {
            qq_farm_app::activity::operate_autumn(
                &app,
                &account_id,
                "wish",
                "claim",
                &serde_json::json!({}),
            )
            .await
        }
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
