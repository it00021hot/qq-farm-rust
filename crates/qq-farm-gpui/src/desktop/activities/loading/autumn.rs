use super::*;

fn activity_key(mode: &str) -> &'static str {
    if mode == "happy" {
        "happy"
    } else {
        "wish"
    }
}

pub(super) async fn load(
    app: &AppContext,
    account_id: &str,
    mode: &str,
) -> Result<super::ActivityPayload, qq_farm_app::AppError> {
    let key = activity_key(mode);
    let value = qq_farm_app::activity::autumn(app, account_id, key).await?;
    Ok(super::ActivityPayload { value: serde_json::json!({key: value}), pet_diary_friends: None })
}

#[cfg(test)]
mod tests {
    use super::activity_key;

    #[test]
    fn autumn_loader_fetches_only_the_selected_vue_gameplay() {
        assert_eq!(activity_key("happy"), "happy");
        assert_eq!(activity_key("wish"), "wish");
        assert_eq!(activity_key("unknown"), "wish");
    }
}
