use super::*;

pub(super) async fn load_friends(
    app: &AppContext,
    account_id: &str,
    force: bool,
) -> Result<serde_json::Value, qq_farm_app::AppError> {
    if force {
        qq_farm_app::friend::clear_friends_cache(app, account_id)?;
    }
    let friends = qq_farm_app::friend::list_friends(app, account_id, false).await?;
    serde_json::to_value(friends)
        .map_err(|error| qq_farm_app::AppError::Internal(error.to_string()))
}
