use super::super::super::*;

mod autumn;
mod charity;
mod pet_diary;
mod qingmei;
mod qixi;
mod standard;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
    qingmei_input: Option<serde_json::Value>,
    qixi_gift: Option<(i64, i64)>,
    autumn_choice: Option<String>,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "battle-pass" | "constellation" => standard::execute(app, account_id, action).await,
        "qingmei" | "qingmei-start" | "qingmei-continue" | "qingmei-settle" => {
            qingmei::execute(app, account_id, action, qingmei_input).await
        }
        "charity" | "charity-seeds" | "charity-donate" => {
            charity::execute(app, account_id, action).await
        }
        "qixi-bridge" | "qixi-gift" => qixi::execute(app, account_id, action, qixi_gift).await,
        "autumn-daily"
        | "autumn-happy-daily"
        | "autumn-happy-share"
        | "autumn-happy-milestones"
        | "autumn-happy-logs-mine"
        | "autumn-happy-logs-others"
        | "autumn-draw"
        | "autumn-claim" => autumn::execute(app, account_id, action, autumn_choice).await,
        "pet-diary-feed"
        | "pet-diary-initialize"
        | "pet-diary-draw"
        | "pet-diary-seeds"
        | "pet-diary-claim-dog"
        | "pet-diary-open-treasure"
        | "pet-diary-compensation" => pet_diary::execute(app, account_id, action).await,
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
