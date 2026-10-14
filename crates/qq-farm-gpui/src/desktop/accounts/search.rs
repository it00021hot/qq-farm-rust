use serde_json::Value;

pub(super) fn account_matches_keyword(record: &Value, keyword: &str) -> bool {
    if keyword.is_empty() {
        return true;
    }

    let name = string_field(record.get("name"));
    let qq = string_field(record.get("qq"));
    let remark = record
        .get("remark")
        .filter(|value| !value.is_null())
        .and_then(Value::as_str)
        .unwrap_or(&name);
    let keyword = keyword.to_lowercase();

    [name.as_str(), qq.as_str(), remark].iter().any(|field| field.to_lowercase().contains(&keyword))
}

fn string_field(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::account_matches_keyword;
    use serde_json::json;

    #[test]
    fn keyword_matches_only_vue_account_name_qq_and_remark_fields() {
        let record = json!({
            "name": "Saved Account",
            "qq": "123456",
            "uin": "654321",
            "nick": "Runtime Nickname",
            "remark": "Farm Notes"
        });

        assert!(account_matches_keyword(&record, "saved"));
        assert!(account_matches_keyword(&record, "123456"));
        assert!(account_matches_keyword(&record, "farm notes"));
        assert!(!account_matches_keyword(&record, "Runtime Nickname"));
        assert!(!account_matches_keyword(&record, "654321"));
    }

    #[test]
    fn null_remark_falls_back_to_account_name_and_whitespace_is_not_trimmed() {
        let record = json!({ "name": "Farm Account", "remark": null });

        assert!(account_matches_keyword(&record, "Farm Account"));
        assert!(account_matches_keyword(&record, " "));
        assert!(!account_matches_keyword(&record, "  "));
        assert!(account_matches_keyword(&record, ""));
    }
}
