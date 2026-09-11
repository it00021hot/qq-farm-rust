use serde::Deserialize;

use crate::commands::application::UpdateResult;

#[cfg(mobile)]
const RELEASE_API: &str = "https://api.github.com/repos/it00021hot/qq-farm-rust/releases/latest";
const RELEASE_BASE: &str = "https://github.com/it00021hot/qq-farm-rust/releases/tag/";

#[derive(Deserialize)]
struct Asset {
    name: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

fn parse_release(release: Release, current: &str) -> Result<UpdateResult, String> {
    if release.draft || release.prerelease || !release.html_url.starts_with(RELEASE_BASE) {
        return Err("Invalid release metadata".into());
    }
    let version = semver::Version::parse(release.tag_name.trim_start_matches('v'))
        .map_err(|e| format!("Invalid release version: {e}"))?;
    let current = semver::Version::parse(current).map_err(|e| e.to_string())?;
    Ok(UpdateResult::Release {
        available: version.cmp_precedence(&current).is_gt(),
        version: version.to_string(),
        has_apk: release
            .assets
            .iter()
            .any(|asset| asset.name.to_ascii_lowercase().ends_with(".apk")),
        release_url: release.html_url,
    })
}

#[cfg(mobile)]
pub async fn check(current: &str) -> Result<UpdateResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("qq-farm-rust/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let release = client
        .get(RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Release>()
        .await
        .map_err(|e| e.to_string())?;
    parse_release(release, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, apk: bool) -> Release {
        Release {
            tag_name: tag.into(),
            html_url: format!("{RELEASE_BASE}{tag}"),
            draft: false,
            prerelease: false,
            assets: if apk { vec![Asset { name: "QQ.Farm.apk".into() }] } else { vec![] },
        }
    }

    #[test]
    fn compares_semantic_versions_and_reports_missing_apk() {
        let UpdateResult::Release { available, has_apk, .. } =
            parse_release(release("v0.10.0", false), "0.9.0").unwrap()
        else {
            panic!("expected release")
        };
        assert!(available);
        assert!(!has_apk);
        for current in ["0.10.0", "0.11.0", "0.10.0+local"] {
            let UpdateResult::Release { available, has_apk, .. } =
                parse_release(release("v0.10.0", true), current).unwrap()
            else {
                panic!("expected release")
            };
            assert!(!available);
            assert!(has_apk);
        }
    }

    #[test]
    fn rejects_invalid_release_metadata() {
        assert!(parse_release(release("broken", true), "0.1.0").is_err());
        let mut invalid = release("v1.0.0", true);
        invalid.prerelease = true;
        assert!(parse_release(invalid, "0.1.0").is_err());
        let mut invalid = release("v1.0.0", true);
        invalid.html_url = "https://example.com/download".into();
        assert!(parse_release(invalid, "0.1.0").is_err());
    }
}
