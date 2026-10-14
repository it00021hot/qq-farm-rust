#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AccountAuthStatus {
    Authorized,
    Unauthorized,
    RescanRecommended,
}

impl AccountAuthStatus {
    pub(super) fn from_record(record: &serde_json::Value) -> Self {
        let authorized =
            record.get("wxAuthorized").and_then(serde_json::Value::as_bool).unwrap_or(false);
        if !authorized {
            return Self::Unauthorized;
        }

        let rescan_recommended =
            record.get("wxRescanRecommended").and_then(serde_json::Value::as_bool).unwrap_or(false);
        if rescan_recommended {
            Self::RescanRecommended
        } else {
            Self::Authorized
        }
    }

    pub(super) fn label(self, language: &str) -> &'static str {
        match (self, language == "en-US") {
            (Self::Authorized, true) => "Authorized",
            (Self::Unauthorized, true) => "Unauthorized",
            (Self::RescanRecommended, true) => "Rescan suggested",
            (Self::Authorized, false) => "已授权",
            (Self::Unauthorized, false) => "未授权",
            (Self::RescanRecommended, false) => "建议重扫",
        }
    }

    pub(super) fn tooltip(self, language: &str) -> Option<&'static str> {
        if self != Self::RescanRecommended {
            return None;
        }

        Some(if language == "en-US" {
            "Refresh token has been used for about 25 days; rescan before it expires"
        } else {
            "RefreshToken 已使用约 25 天，建议在失效前重新扫码"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AccountAuthStatus;
    use serde_json::json;

    #[test]
    fn authorization_takes_precedence_over_rescan_recommendation() {
        assert_eq!(
            AccountAuthStatus::from_record(&json!({
                "wxAuthorized": false,
                "wxRescanRecommended": true
            })),
            AccountAuthStatus::Unauthorized
        );
    }

    #[test]
    fn rescan_status_matches_vue_labels_and_tooltips_in_both_locales() {
        let status = AccountAuthStatus::from_record(&json!({
            "wxAuthorized": true,
            "wxRescanRecommended": true
        }));

        assert_eq!(status.label("zh-CN"), "建议重扫");
        assert_eq!(
            status.tooltip("zh-CN"),
            Some("RefreshToken 已使用约 25 天，建议在失效前重新扫码")
        );
        assert_eq!(status.label("en-US"), "Rescan suggested");
        assert_eq!(
            status.tooltip("en-US"),
            Some("Refresh token has been used for about 25 days; rescan before it expires")
        );
    }

    #[test]
    fn ordinary_authorization_status_has_no_tooltip() {
        assert_eq!(
            AccountAuthStatus::from_record(&json!({ "wxAuthorized": true })),
            AccountAuthStatus::Authorized
        );
        assert_eq!(AccountAuthStatus::from_record(&json!({})), AccountAuthStatus::Unauthorized);
        assert_eq!(AccountAuthStatus::Authorized.tooltip("zh-CN"), None);
    }
}
