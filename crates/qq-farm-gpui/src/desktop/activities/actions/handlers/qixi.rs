use super::super::super::super::*;

pub(super) async fn execute(
    app: Arc<AppContext>,
    account_id: String,
    action: &'static str,
    qixi_gift: Option<(i64, i64)>,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    match action {
        "qixi-bridge" => qq_farm_app::activity::claim_qixi_bridge(&app, &account_id).await,
        "qixi-gift" => {
            let (gid, count) = qixi_gift.unwrap_or_default();
            qq_farm_app::activity::gift_qixi_sachet(&app, &account_id, gid, count).await
        }
        _ => Err(qq_farm_app::AppError::BadRequest("未知活动操作".into())),
    }
}
