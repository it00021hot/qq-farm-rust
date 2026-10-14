use super::*;

pub(super) async fn load(
    app: &AppContext,
    account_id: &str,
    include_friends: bool,
) -> Result<super::ActivityPayload, qq_farm_app::AppError> {
    if include_friends {
        let (diary, friends) = tokio::join!(
            qq_farm_app::activity::pet_diary(app, account_id),
            qq_farm_app::friend::list_friends(app, account_id, false),
        );
        let pet_diary_friends = friends.ok().and_then(|friends| serde_json::to_value(friends).ok());
        diary.map(|value| super::ActivityPayload { value, pet_diary_friends })
    } else {
        qq_farm_app::activity::pet_diary(app, account_id)
            .await
            .map(|value| super::ActivityPayload { value, pet_diary_friends: None })
    }
}
