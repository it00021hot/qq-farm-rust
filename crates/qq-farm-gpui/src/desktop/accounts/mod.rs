mod state;
pub(crate) use state::AccountsState;

mod auth_status;
mod columns;
mod delete_view;
mod feedback;
mod filters;
mod form;
mod helpers;
mod lifecycle;
mod list;
mod messages;
mod refresh;
mod row_actions;
mod search;
mod search_selectors;
mod sheet;
mod sidebar;
mod table;
mod wx_login;

#[derive(Clone, Debug, Default)]
pub(crate) struct AccountDirectory {
    pub(crate) records: Vec<(String, String, bool)>,
    pub(crate) avatars: std::collections::HashMap<String, String>,
}

pub(crate) use columns::{
    default_account_columns, move_account_column, AccountColumnId, AccountColumnPin,
    AccountColumnSetting,
};

pub(crate) use helpers::{
    account_last_online_timestamp, account_login_url_hint, account_value, format_account_timestamp,
    parse_account_login_input, upsert_and_maybe_start_account,
};
pub(crate) use lifecycle::AccountLifecycleState;
pub(super) use messages::{account_status_text, localized_account_message};

pub(super) fn read_accounts(app: &qq_farm_app::session::AppContext) -> AccountDirectory {
    let accounts = qq_farm_app::accounts::list_accounts_enriched(app, None);
    account_directory_from_json(accounts.get("accounts"))
}

fn account_directory_from_json(accounts: Option<&serde_json::Value>) -> AccountDirectory {
    accounts
        .and_then(serde_json::Value::as_array)
        .map(|accounts| {
            let mut directory = AccountDirectory::default();
            for account in accounts {
                let Some(id) = account.get("id").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                let id = id.to_string();
                let name = account
                    .get("name")
                    .or_else(|| account.get("nick"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("QQ Farm")
                    .to_string();
                let running =
                    account.get("running").and_then(serde_json::Value::as_bool).unwrap_or(false);
                if let Some(avatar) = account
                    .get("avatar")
                    .and_then(serde_json::Value::as_str)
                    .filter(|avatar| !avatar.trim().is_empty())
                {
                    directory.avatars.insert(id.clone(), avatar.to_string());
                }
                directory.records.push((id, name, running));
            }
            directory
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::account_directory_from_json;

    #[test]
    fn account_directory_keeps_saved_avatars_for_dashboard_fallbacks() {
        let directory = account_directory_from_json(Some(&serde_json::json!([
            {"id":"1", "name":"Farm", "running":true, "avatar":"https://example.invalid/a.png"},
            {"id":"2", "name":"Other", "avatar":" "},
            {"name":"Invalid record", "avatar":"https://example.invalid/ignored.png"}
        ])));

        assert_eq!(
            directory.records,
            vec![("1".into(), "Farm".into(), true), ("2".into(), "Other".into(), false)]
        );
        assert_eq!(
            directory.avatars.get("1").map(String::as_str),
            Some("https://example.invalid/a.png")
        );
        assert!(!directory.avatars.contains_key("2"));
    }
}
