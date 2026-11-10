//! Port of AppUpdate.kt.

use serde::{Deserialize, Serialize};

pub const UPDATE_CHECK_NEVER: i32 = crate::constants::update::CHECK_NEVER;
pub const DEFAULT_UPDATE_CHECK_INTERVAL_HOURS: i32 = crate::constants::update::DEFAULT_CHECK_INTERVAL_HOURS;
pub const UPDATE_DIALOG_ACTION_POSTPONED: &str = crate::constants::update::ACTION_POSTPONED;
pub const UPDATE_DIALOG_ACTION_UPDATE: &str = crate::constants::update::ACTION_UPDATE;

/// Source of an application update version information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteVersionSource {
    Release,
    Tag,
}

/// Information describing an available application release or tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppReleaseInfo {
    pub version_tag: String,
    pub release_url: String,
    pub source: RemoteVersionSource,
}

/// Converts update check interval from hours to milliseconds. Returns None if hours <= 0.
pub fn update_interval_hours_to_millis(hours: i32) -> Option<i64> {
    if hours <= 0 {
        None
    } else {
        Some(hours as i64 * 60 * 60 * 1000)
    }
}

/// Normalizes a version tag ensuring it has a leading 'v' prefix.
pub fn normalize_version_tag(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with('v') || trimmed.starts_with('V') {
        trimmed.to_string()
    } else {
        format!("v{trimmed}")
    }
}

/// Parses numeric dot-separated version segments (matching `\d+(?:\.\d+)*`).
pub fn version_parts(version: &str) -> Vec<i32> {
    let trimmed = version.trim();
    let start = match trimmed.find(|c: char| c.is_ascii_digit()) {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let bytes = trimmed.as_bytes();
    let mut i = start;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    loop {
        if i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            break;
        }
    }

    trimmed[start..i]
        .split('.')
        .filter_map(|part| part.parse::<i32>().ok())
        .collect()
}

/// Compares two version strings numerically. Returns true if `remote` is strictly newer than `local`.
pub fn is_newer_version(local: &str, remote: &str) -> bool {
    let local_parts = version_parts(local);
    let remote_parts = version_parts(remote);
    if remote_parts.is_empty() {
        return false;
    }

    let max_len = local_parts.len().max(remote_parts.len());
    for i in 0..max_len {
        let local_part = *local_parts.get(i).unwrap_or(&0);
        let remote_part = *remote_parts.get(i).unwrap_or(&0);
        if remote_part > local_part {
            return true;
        }
        if remote_part < local_part {
            return false;
        }
    }
    false
}

/// Extracts the normalized release version tag from a GitHub release URL.
pub fn extract_tag_from_release_url(release_url: &str) -> Option<String> {
    let marker = "/releases/tag/";
    let index = release_url.find(marker)?;
    let rest = &release_url[index + marker.len()..];
    let candidate = rest
        .split('?')
        .next()?
        .split('#')
        .next()?
        .split('/')
        .next()?
        .trim();
    if candidate.is_empty() {
        None
    } else {
        Some(normalize_version_tag(candidate))
    }
}

/// Parses a single GitHub release JSON object into `AppReleaseInfo`.
pub fn parse_release_json(json_str: &str) -> Option<AppReleaseInfo> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    parse_release_value(&value)
}

fn parse_release_value(value: &serde_json::Value) -> Option<AppReleaseInfo> {
    let tag_name = value.get("tag_name")?.as_str()?;
    let html_url = value.get("html_url")?.as_str()?.trim();
    let version_tag = normalize_version_tag(tag_name);
    if version_tag.is_empty() || html_url.is_empty() {
        return None;
    }
    Some(AppReleaseInfo {
        version_tag,
        release_url: html_url.to_string(),
        source: RemoteVersionSource::Release,
    })
}

/// Parses a list of GitHub releases, selecting the newest stable (non-draft, non-prerelease) release.
pub fn parse_latest_stable_release_from_list(json_str: &str) -> Option<AppReleaseInfo> {
    let array: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let items = array.as_array()?;

    let mut best_release: Option<AppReleaseInfo> = None;
    for item in items {
        if item.get("draft").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        if item.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let release = match parse_release_value(item) {
            Some(r) => r,
            None => continue,
        };
        let is_better = match &best_release {
            None => true,
            Some(best) => is_newer_version(&best.version_tag, &release.version_tag),
        };
        if is_better {
            best_release = Some(release);
        }
    }
    best_release
}

/// Parses a list of GitHub tags, selecting the newest tag.
pub fn parse_latest_tag_from_list(json_str: &str) -> Option<AppReleaseInfo> {
    let array: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let items = array.as_array()?;

    let mut best_tag: Option<AppReleaseInfo> = None;
    for item in items {
        let name = match item.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let tag_name = normalize_version_tag(name);
        if tag_name.is_empty() {
            continue;
        }
        let tag = AppReleaseInfo {
            version_tag: tag_name.clone(),
            release_url: format!("{}{}", crate::constants::update::GITHUB_TAG_TREE_URL_PREFIX, tag_name),
            source: RemoteVersionSource::Tag,
        };
        let is_better = match &best_tag {
            None => true,
            Some(best) => is_newer_version(&best.version_tag, &tag.version_tag),
        };
        if is_better {
            best_tag = Some(tag);
        }
    }
    best_tag
}

/// Chooses the preferred release info between a candidate release and candidate tag.
pub fn select_preferred_release(
    latest_release: Option<AppReleaseInfo>,
    latest_tag: Option<AppReleaseInfo>,
) -> Option<AppReleaseInfo> {
    match (latest_release, latest_tag) {
        (None, tag) => tag,
        (rel, None) => rel,
        (Some(rel), Some(tag)) => {
            if is_newer_version(&rel.version_tag, &tag.version_tag) {
                Some(tag)
            } else {
                Some(rel)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer_version("1.9.9", "2.0.5"));
        assert!(is_newer_version("2.0", "2.0.1"));
        assert!(!is_newer_version("2.0.5", "2.0"));
        assert!(!is_newer_version("2.1.0", "2.0.9"));
        assert!(!is_newer_version("2.0.5", "invalid"));
    }

    #[test]
    fn test_update_interval_hours_to_millis() {
        assert_eq!(None, update_interval_hours_to_millis(0));
        assert_eq!(None, update_interval_hours_to_millis(-1));
        assert_eq!(Some(3_600_000), update_interval_hours_to_millis(1));
        assert_eq!(Some(43_200_000), update_interval_hours_to_millis(12));
    }

    #[test]
    fn test_normalize_version_tag() {
        assert_eq!("", normalize_version_tag(""));
        assert_eq!("v2.0.5", normalize_version_tag("2.0.5"));
        assert_eq!("v2.0.5", normalize_version_tag("v2.0.5"));
        assert_eq!("v2.0.5", normalize_version_tag("  v2.0.5  "));
    }

    #[test]
    fn test_extract_tag_from_release_url() {
        assert_eq!(
            Some("v2.0.5".to_string()),
            extract_tag_from_release_url("https://github.com/amurcanov/csqtt/releases/tag/v2.0.5"),
        );
        assert_eq!(
            Some("v2.0.5".to_string()),
            extract_tag_from_release_url("https://github.com/amurcanov/csqtt/releases/tag/2.0.5?foo=bar#baz"),
        );
        assert_eq!(
            None,
            extract_tag_from_release_url("https://github.com/amurcanov/csqtt/releases/latest"),
        );
    }

    #[test]
    fn test_parse_release_json() {
        let json = r#"{"tag_name": "v2.0.5", "html_url": "https://github.com/amurcanov/csqtt/releases/tag/v2.0.5"}"#;
        let info = parse_release_json(json).expect("should parse release");
        assert_eq!("v2.0.5", info.version_tag);
        assert_eq!("https://github.com/amurcanov/csqtt/releases/tag/v2.0.5", info.release_url);
        assert_eq!(RemoteVersionSource::Release, info.source);
    }

    #[test]
    fn test_parse_latest_stable_release_from_list() {
        let json = r#"[
            {"tag_name": "v2.0.6", "html_url": "url6", "prerelease": true},
            {"tag_name": "v2.0.5", "html_url": "url5", "draft": false, "prerelease": false},
            {"tag_name": "v2.0.4", "html_url": "url4", "draft": false, "prerelease": false}
        ]"#;
        let info = parse_latest_stable_release_from_list(json).expect("should parse list");
        assert_eq!("v2.0.5", info.version_tag);
        assert_eq!("url5", info.release_url);
    }

    #[test]
    fn test_parse_latest_tag_from_list() {
        let json = r#"[
            {"name": "2.0.1"},
            {"name": "2.0.5"},
            {"name": "2.0.3"}
        ]"#;
        let info = parse_latest_tag_from_list(json).expect("should parse tags");
        assert_eq!("v2.0.5", info.version_tag);
        assert_eq!(RemoteVersionSource::Tag, info.source);
    }
}
