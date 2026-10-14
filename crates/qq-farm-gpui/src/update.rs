use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseNotice {
    pub tag_name: String,
    pub html_url: String,
    pub name: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseCheckError {
    Client(String),
    Request(String),
    Http(String),
    InvalidResponse(String),
}

impl ReleaseCheckError {
    pub(crate) fn localized_message(&self, language: &str) -> String {
        let (chinese, english, detail) = match self {
            Self::Client(detail) => {
                ("无法创建更新请求", "Could not prepare the update request", detail)
            }
            Self::Request(detail) => {
                ("无法检查发布版本", "Could not check the latest release", detail)
            }
            Self::Http(detail) => ("发布版本查询失败", "Release lookup failed", detail),
            Self::InvalidResponse(detail) => {
                ("发布信息格式无效", "The release response is invalid", detail)
            }
        };
        crate::i18n::error_with_detail(language, chinese, english, detail)
    }
}

impl ReleaseNotice {
    pub fn has_platform_package(&self) -> bool {
        let expected = package_asset_name();
        self.assets.iter().any(|asset| asset.name == expected)
    }

    pub fn is_newer(&self) -> bool {
        let tag = self.tag_name.trim_start_matches(['v', 'V']);
        match (semver::Version::parse(env!("CARGO_PKG_VERSION")), semver::Version::parse(tag)) {
            (Ok(current), Ok(latest)) => latest > current,
            _ => false,
        }
    }
}

pub async fn check_latest() -> Result<Option<ReleaseNotice>, ReleaseCheckError> {
    let repository = env!("CARGO_PKG_REPOSITORY");
    let endpoint = format!(
        "https://api.github.com/repos/{}/releases/latest",
        repository.trim_start_matches("https://github.com/")
    );
    let response = reqwest::Client::builder()
        .user_agent(concat!("qq-farm-gpui/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|error| ReleaseCheckError::Client(error.to_string()))?
        .get(endpoint)
        .send()
        .await
        .map_err(|error| ReleaseCheckError::Request(error.to_string()))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response =
        response.error_for_status().map_err(|error| ReleaseCheckError::Http(error.to_string()))?;
    let release = response
        .json::<ReleaseNotice>()
        .await
        .map_err(|error| ReleaseCheckError::InvalidResponse(error.to_string()))?;
    Ok((release.is_newer() && release.has_platform_package()).then_some(release))
}

pub fn package_asset_name() -> &'static str {
    package_asset_name_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn package_asset_name_for(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("macos", "aarch64") => "qq-farm-gpui-macos-aarch64.app.zip",
        ("macos", "x86_64") => "qq-farm-gpui-macos-x86_64.app.zip",
        ("windows", "x86_64") => "qq-farm-gpui-windows-x86_64.zip",
        ("linux", "x86_64") => "qq-farm-gpui-linux-x86_64.tar.gz",
        ("linux", "aarch64") => "qq-farm-gpui-linux-aarch64.tar.gz",
        _ => "qq-farm-gpui-unsupported-platform",
    }
}

#[cfg(test)]
mod tests {
    use super::{package_asset_name_for, ReleaseAsset, ReleaseCheckError, ReleaseNotice};

    #[test]
    fn updater_asset_names_match_every_packaged_target() {
        for ((os, arch), expected) in [
            (("macos", "aarch64"), "qq-farm-gpui-macos-aarch64.app.zip"),
            (("macos", "x86_64"), "qq-farm-gpui-macos-x86_64.app.zip"),
            (("windows", "x86_64"), "qq-farm-gpui-windows-x86_64.zip"),
            (("linux", "x86_64"), "qq-farm-gpui-linux-x86_64.tar.gz"),
            (("linux", "aarch64"), "qq-farm-gpui-linux-aarch64.tar.gz"),
        ] {
            assert_eq!(package_asset_name_for(os, arch), expected, "{os}/{arch}");
        }
        assert_eq!(
            package_asset_name_for("windows", "aarch64"),
            "qq-farm-gpui-unsupported-platform"
        );
    }

    #[test]
    fn only_matching_gpui_release_artifacts_are_eligible_for_update_notices() {
        let release = ReleaseNotice {
            tag_name: "v99.0.0".into(),
            html_url: "https://github.com/example/repo/releases/tag/v99.0.0".into(),
            name: None,
            assets: vec![ReleaseAsset { name: "tauri-installer.exe".into() }],
        };
        assert!(!release.has_platform_package());
        assert!(release.is_newer());
    }

    #[test]
    fn update_lookup_failures_follow_the_selected_language() {
        let failures = [
            (
                ReleaseCheckError::Client("bad certificate".into()),
                "无法创建更新请求：bad certificate",
                "Could not prepare the update request: bad certificate",
            ),
            (
                ReleaseCheckError::Request("connection refused".into()),
                "无法检查发布版本：connection refused",
                "Could not check the latest release: connection refused",
            ),
            (
                ReleaseCheckError::Http("503 Service Unavailable".into()),
                "发布版本查询失败：503 Service Unavailable",
                "Release lookup failed: 503 Service Unavailable",
            ),
            (
                ReleaseCheckError::InvalidResponse("missing tag".into()),
                "发布信息格式无效：missing tag",
                "The release response is invalid: missing tag",
            ),
        ];
        for (failure, chinese, english) in failures {
            assert_eq!(failure.localized_message("zh-CN"), chinese);
            assert_eq!(failure.localized_message("en-US"), english);
        }
    }
}
