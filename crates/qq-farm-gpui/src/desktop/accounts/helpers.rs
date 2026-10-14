pub(crate) fn account_value(record: &serde_json::Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| record.get(*key).and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn format_account_timestamp(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "-".into();
    }
    use chrono::TimeZone as _;
    chrono::Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}

#[cfg(test)]
mod timestamp_tests {
    use super::format_account_timestamp;

    #[test]
    fn missing_account_timestamp_uses_vue_hyphen_placeholder() {
        assert_eq!(format_account_timestamp(0), "-");
        assert_eq!(format_account_timestamp(-1), "-");
    }
}

pub(crate) fn account_last_online_timestamp(record: &serde_json::Value) -> i64 {
    ["lastOnlineAt", "updatedAt", "updated_at"]
        .into_iter()
        .find_map(|key| record.get(key).filter(|value| !value.is_null()))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
                .or_else(|| {
                    value.as_f64().filter(|number| number.is_finite()).map(|number| number as i64)
                })
                .or_else(|| {
                    value
                        .as_str()
                        .and_then(|number| number.parse::<f64>().ok())
                        .filter(|number| number.is_finite())
                        .map(|number| number as i64)
                })
        })
        .unwrap_or_default()
}

pub(crate) fn parse_account_login_input(raw: &str, fallback_platform: &str) -> (String, String) {
    let trimmed = raw.trim();
    let looks_like_url = trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ws://")
        || trimmed.starts_with("wss://")
        || trimmed.contains("code=")
        || trimmed.contains("platform=");
    if !looks_like_url {
        return (fallback_platform.to_string(), trimmed.to_string());
    }
    let normalized = if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ws://")
        || trimmed.starts_with("wss://")
    {
        trimmed.to_string()
    } else {
        format!("http://127.0.0.1/prod/ws?{}", trimmed.trim_start_matches(['?', '&']))
    };
    let parsed = reqwest::Url::parse(&normalized).ok();
    let code = parsed
        .as_ref()
        .and_then(|url| url.query_pairs().find(|(key, _)| key == "code"))
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| trimmed.to_string());
    let platform = parsed
        .as_ref()
        .and_then(|url| url.query_pairs().find(|(key, _)| key == "platform"))
        .map(|(_, value)| value.to_ascii_lowercase())
        .filter(|value| value == "qq" || value == "wx")
        .unwrap_or_else(|| fallback_platform.to_string());
    (platform, code)
}

fn account_login_url_hints(raw: &str) -> Option<qq_farm_core::utils::login_url::LoginClientHints> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let looks_like_url =
        ["http://", "https://", "ws://", "wss://"].iter().any(|prefix| lower.starts_with(prefix))
            || ["code=", "platform=", "os=", "ver="].iter().any(|key| {
                [format!("?{key}"), format!("&{key}")].iter().any(|part| lower.contains(part))
            });
    if !looks_like_url {
        return None;
    }

    // Vue wraps a bare query in the game URL before extracting client hints.
    let hint_source = if trimmed.contains("://") || trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("http://127.0.0.1/prod/ws?{}", trimmed.trim_start_matches(['?', '&']))
    };
    Some(qq_farm_core::utils::login_url::extract_client_hints(&hint_source))
}

pub(crate) fn account_login_detected_platform(raw: &str) -> Option<String> {
    let hints = account_login_url_hints(raw)?;
    let platform = qq_farm_core::utils::login_url::normalize_login_platform(&hints.platform);
    (!platform.is_empty()).then(|| platform.to_string())
}

pub(crate) fn account_login_url_hint(raw: &str, language: &str) -> Option<String> {
    let hints = account_login_url_hints(raw)?;
    let mut detail = Vec::new();
    match hints.platform.as_str() {
        "qq" => detail.push(format!("{} QQ", crate::i18n::text(language, "平台", "Platform"))),
        "wx" => detail.push(format!(
            "{} {}",
            crate::i18n::text(language, "平台", "Platform"),
            crate::i18n::text(language, "微信", "WeChat")
        )),
        _ => {}
    }
    if !hints.os.is_empty() {
        detail.push(format!("os {}", hints.os));
    }
    if !hints.ver.is_empty() {
        detail.push(format!("ver {}", hints.ver));
    }
    if detail.is_empty() {
        return None;
    }

    let (prefix, separator) = if language == "en-US" {
        ("Detected from URL", ": ")
    } else {
        ("已从 URL 识别", "：")
    };
    Some(format!("{prefix}{separator}{}", detail.join(" / ")))
}

pub(crate) fn account_id_from_upsert_response(
    response: &serde_json::Value,
    requested_id: Option<&str>,
    code: &str,
    platform: &str,
) -> Option<String> {
    if let Some(id) = requested_id.filter(|id| !id.trim().is_empty()) {
        return Some(id.to_string());
    }
    let accounts = response.get("accounts")?.as_array()?;
    let matching = accounts.iter().find(|account| {
        account.get("code").and_then(serde_json::Value::as_str) == Some(code)
            && account.get("platform").and_then(serde_json::Value::as_str) == Some(platform)
    });
    matching.and_then(account_id_value).or_else(|| {
        accounts
            .iter()
            .max_by(|left, right| {
                account_updated_timestamp(left)
                    .cmp(&account_updated_timestamp(right))
                    .then_with(|| account_id_number(left).cmp(&account_id_number(right)))
            })
            .and_then(account_id_value)
    })
}

fn account_updated_timestamp(account: &serde_json::Value) -> i64 {
    ["updatedAt", "updated_at"]
        .into_iter()
        .find_map(|key| account.get(key).and_then(serde_json::Value::as_i64))
        .unwrap_or_default()
}

fn account_id_number(account: &serde_json::Value) -> u64 {
    account
        .get("id")
        .and_then(|id| id.as_u64().or_else(|| id.as_str()?.parse::<u64>().ok()))
        .unwrap_or_default()
}

fn account_id_value(account: &serde_json::Value) -> Option<String> {
    let id = account.get("id")?;
    id.as_str()
        .map(str::to_owned)
        .or_else(|| id.as_i64().map(|value| value.to_string()))
        .or_else(|| id.as_u64().map(|value| value.to_string()))
}

pub(crate) async fn upsert_and_maybe_start_account(
    app: &qq_farm_app::session::AppContext,
    request: qq_farm_app::accounts::UpsertAccountRequest,
    ensure_running: bool,
) -> Result<Option<String>, String> {
    let requested_id = request.id.clone();
    let code = request.code.clone().unwrap_or_default();
    let platform = request.platform.clone().unwrap_or_else(|| "qq".into());
    let response = qq_farm_app::accounts::upsert_account(
        app,
        &qq_farm_app::accounts::AclPolicy::LocalOwner,
        request,
    )
    .await
    .map_err(|error| error.to_string())?;

    if !ensure_running {
        return Ok(None);
    }
    let Some(account_id) =
        account_id_from_upsert_response(&response, requested_id.as_deref(), &code, &platform)
    else {
        return Ok(Some("无法确认账号编号，未能完成启动检查".into()));
    };
    Ok(qq_farm_app::accounts::start_account(
        app,
        &qq_farm_app::accounts::AclPolicy::LocalOwner,
        &account_id,
    )
    .err()
    .map(|error| error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{account_id_from_upsert_response, account_login_url_hint};

    #[test]
    fn account_login_url_hints_match_vue_for_platform_and_client_fields() {
        assert_eq!(
            account_login_url_hint(
                "https://game.qq.com/prod/ws?code=abc&platform=wx&os=mac&ver=1.2",
                "zh-CN"
            )
            .as_deref(),
            Some("已从 URL 识别：平台 微信 / os mac / ver 1.2")
        );
        assert_eq!(
            account_login_url_hint("?code=abc&platform=qq&os=windows", "en-US").as_deref(),
            Some("Detected from URL: Platform QQ / os windows")
        );
        assert_eq!(account_login_url_hint("plain-code", "zh-CN"), None);
        assert_eq!(account_login_url_hint("?code=abc", "zh-CN"), None);
        assert_eq!(
            super::account_login_detected_platform("?code=abc&platform=WX"),
            Some("wx".into())
        );
    }

    #[test]
    fn wx_login_start_check_targets_the_saved_account() {
        let response = serde_json::json!({
            "accounts": [
                {"id": "2", "name": "old", "code": "other", "platform": "wx"},
                {"id": "9", "name": "new", "code": "fresh", "platform": "wx"}
            ]
        });
        assert_eq!(
            account_id_from_upsert_response(&response, None, "fresh", "wx").as_deref(),
            Some("9")
        );
        assert_eq!(
            account_id_from_upsert_response(&response, Some("2"), "fresh", "wx").as_deref(),
            Some("2")
        );
        assert_eq!(
            account_id_from_upsert_response(
                &serde_json::json!({"accounts": [
                    {"id": 9, "name": "new"}
                ]}),
                None,
                "missing",
                "wx"
            )
            .as_deref(),
            Some("9")
        );
    }
}
