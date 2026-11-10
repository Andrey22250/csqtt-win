//! Client update check against GitHub releases for the fork, and asset download.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Default repository for client updates.
///
/// In upstream csqtt (AppUpdate.kt), this was "amurcanov/csqtt".
/// This constant must be set to the GitHub repository of the Windows fork (e.g. "OWNER/csqtt-win").
pub const DEFAULT_REPO: &str = "OWNER/csqtt-win";

/// Information describing an available application update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub tag: String,
    pub notes: String,
    pub asset_url: String,
    pub asset_name: String,
    pub html_url: String,
}

/// Checks for the latest release on GitHub for `repo` (e.g. "OWNER/csqtt-win")
/// and returns `Ok(Some(UpdateInfo))` if a newer release with Windows assets exists.
///
/// Returns `Ok(None)` if the current version is up to date or no newer Windows release is found.
pub fn check_latest(repo: &str, current_version: &str) -> anyhow::Result<Option<UpdateInfo>> {
    let clean_repo = repo.trim().trim_matches('/');
    let url = format!("https://api.github.com/repos/{clean_repo}/releases/latest");

    let response = ureq::get(&url)
        .config()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .header("User-Agent", "csqtt-win")
        .header("Accept", "application/vnd.github+json")
        .call();

    let mut resp = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(err) => {
            return Err(anyhow::Error::new(err)
                .context(format!("Не удалось проверить обновления в репозитории {clean_repo}")));
        }
    };

    let body = resp
        .body_mut()
        .read_to_string()
        .with_context(|| "Не удалось прочитать ответ GitHub API")?;

    parse_release_json(&body, current_version)
}

/// Parses a GitHub release JSON string and compares it with `current_version`.
///
/// Returns `Ok(Some(UpdateInfo))` if the release is newer and contains a Windows
/// asset ending with ".msi" or "-windows-x64.zip" (preferring ".msi").
/// Returns `Ok(None)` if the release is not newer or no matching asset is found.
pub fn parse_release_json(json: &str, current_version: &str) -> anyhow::Result<Option<UpdateInfo>> {
    let value: serde_json::Value = serde_json::from_str(json)
        .with_context(|| "Неверный формат JSON ответа")?;

    let tag_name = match value.get("tag_name").and_then(|t| t.as_str()) {
        Some(t) => t.trim(),
        None => return Ok(None),
    };

    if tag_name.is_empty() {
        return Ok(None);
    }

    // Compare versions using pure logic from csqtt-logic::app_update
    if !csqtt_logic::app_update::is_newer_version(current_version, tag_name) {
        return Ok(None);
    }

    // Pick the asset whose name ends with "-windows-x64.zip" or ".msi" (prefer .msi)
    let mut msi_asset: Option<(String, String)> = None;
    let mut zip_asset: Option<(String, String)> = None;

    if let Some(assets) = value.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            let name = match asset.get("name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => continue,
            };
            let download_url = match asset
                .get("browser_download_url")
                .or_else(|| asset.get("url"))
                .and_then(|u| u.as_str())
            {
                Some(u) => u,
                None => continue,
            };

            let name_lower = name.to_ascii_lowercase();
            if name_lower.ends_with(".msi") {
                if msi_asset.is_none() {
                    msi_asset = Some((name.to_string(), download_url.to_string()));
                }
            } else if name_lower.ends_with("-windows-x64.zip") {
                if zip_asset.is_none() {
                    zip_asset = Some((name.to_string(), download_url.to_string()));
                }
            }
        }
    }

    let (asset_name, asset_url) = match msi_asset.or(zip_asset) {
        Some(pair) => pair,
        None => return Ok(None),
    };

    let tag = csqtt_logic::app_update::normalize_version_tag(tag_name);
    let version = tag.trim_start_matches(|c| c == 'v' || c == 'V').to_string();

    let notes = value
        .get("body")
        .or_else(|| value.get("notes"))
        .and_then(|b| b.as_str())
        .unwrap_or("")
        .to_string();

    let html_url = value
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    Ok(Some(UpdateInfo {
        version,
        tag,
        notes,
        asset_url,
        asset_name,
        html_url,
    }))
}

/// Downloads the file from `url` to `dest` streaming via a temporary `.part` file,
/// invoking `on_progress(downloaded_bytes, total_bytes)` periodically.
///
/// Renames the `.part` file to `dest` upon successful completion.
pub fn download(
    url: &str,
    dest: &Path,
    on_progress: &mut dyn FnMut(u64, Option<u64>),
) -> anyhow::Result<()> {
    let mut part_os_str = dest.as_os_str().to_os_string();
    part_os_str.push(".part");
    let part_path = std::path::PathBuf::from(part_os_str);

    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Не удалось создать директорию: {}", parent.display()))?;
        }
    }

    let download_res = (|| -> anyhow::Result<()> {
        let response = ureq::get(url)
            .header("User-Agent", "csqtt-win")
            .call()
            .with_context(|| format!("Не удалось загрузить файл с {url}"))?;

        let total_bytes = response
            .body()
            .content_length()
            .or_else(|| {
                response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
            });

        let mut reader = response.into_body().into_reader();
        let mut part_file = std::fs::File::create(&part_path)
            .with_context(|| format!("Не удалось создать временный файл: {}", part_path.display()))?;

        let mut downloaded: u64 = 0;
        let mut buffer = [0u8; 65536];

        on_progress(0, total_bytes);

        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .with_context(|| "Ошибка чтения данных из сети")?;
            if bytes_read == 0 {
                break;
            }
            part_file
                .write_all(&buffer[..bytes_read])
                .with_context(|| "Ошибка записи во временный файл")?;
            downloaded += bytes_read as u64;
            on_progress(downloaded, total_bytes);
        }

        part_file
            .flush()
            .with_context(|| "Ошибка сброса буфера файла на диск")?;
        drop(part_file);

        if let Some(expected) = total_bytes {
            if downloaded != expected {
                anyhow::bail!(
                    "Неполная загрузка: получено {} байт, ожидалось {}",
                    downloaded,
                    expected
                );
            }
        }

        if dest.exists() {
            let _ = std::fs::remove_file(dest);
        }

        std::fs::rename(&part_path, dest).with_context(|| {
            format!(
                "Не удалось переименовать {} в {}",
                part_path.display(),
                dest.display()
            )
        })?;

        Ok(())
    })();

    if download_res.is_err() {
        let _ = std::fs::remove_file(&part_path);
    }

    download_res
}

#[cfg(test)]
mod tests {
    use super::*;

    const REALISTIC_RELEASE_JSON: &str = r#"{
  "url": "https://api.github.com/repos/OWNER/csqtt-win/releases/12345678",
  "html_url": "https://github.com/OWNER/csqtt-win/releases/tag/v2.0.5",
  "id": 12345678,
  "tag_name": "v2.0.5",
  "target_commitish": "main",
  "name": "csqtt 2.0.5",
  "draft": false,
  "prerelease": false,
  "body": "Changelog:\n- Windows service improvements\n- Better stability",
  "assets": [
    {
      "name": "csqtt-2.0.5-windows-x64.zip",
      "browser_download_url": "https://github.com/OWNER/csqtt-win/releases/download/v2.0.5/csqtt-2.0.5-windows-x64.zip",
      "size": 15482910
    },
    {
      "name": "csqtt-2.0.5.msi",
      "browser_download_url": "https://github.com/OWNER/csqtt-win/releases/download/v2.0.5/csqtt-2.0.5.msi",
      "size": 16234500
    }
  ]
}"#;

    #[test]
    fn compares_versions_numerically() {
        assert!(csqtt_logic::app_update::is_newer_version("1.9.9", "2.0.5"));
        assert!(csqtt_logic::app_update::is_newer_version("2.0", "2.0.1"));
        assert!(!csqtt_logic::app_update::is_newer_version("2.0.5", "2.0"));
        assert!(!csqtt_logic::app_update::is_newer_version("2.1.0", "2.0.9"));
        assert!(!csqtt_logic::app_update::is_newer_version("2.0.5", "invalid"));
    }

    #[test]
    fn parse_release_json_newer() {
        let result = parse_release_json(REALISTIC_RELEASE_JSON, "2.0.4")
            .expect("should parse valid release JSON");
        let info = result.expect("should return UpdateInfo when remote is newer");
        assert_eq!(info.version, "2.0.5");
        assert_eq!(info.tag, "v2.0.5");
        assert_eq!(
            info.notes,
            "Changelog:\n- Windows service improvements\n- Better stability"
        );
        // .msi preferred over .zip
        assert_eq!(info.asset_name, "csqtt-2.0.5.msi");
        assert_eq!(
            info.asset_url,
            "https://github.com/OWNER/csqtt-win/releases/download/v2.0.5/csqtt-2.0.5.msi"
        );
        assert_eq!(
            info.html_url,
            "https://github.com/OWNER/csqtt-win/releases/tag/v2.0.5"
        );
    }

    #[test]
    fn parse_release_json_same() {
        let result = parse_release_json(REALISTIC_RELEASE_JSON, "2.0.5")
            .expect("should parse valid release JSON");
        assert_eq!(result, None);
    }

    #[test]
    fn parse_release_json_older() {
        let result = parse_release_json(REALISTIC_RELEASE_JSON, "2.0.6")
            .expect("should parse valid release JSON");
        assert_eq!(result, None);
    }

    #[test]
    fn parse_release_json_missing_asset() {
        let json_no_matching = r#"{
          "tag_name": "v2.0.5",
          "html_url": "https://github.com/OWNER/csqtt-win/releases/tag/v2.0.5",
          "body": "Android only release",
          "assets": [
            {
              "name": "csqtt-release.apk",
              "browser_download_url": "https://github.com/OWNER/csqtt-win/releases/download/v2.0.5/csqtt-release.apk"
            }
          ]
        }"#;
        let result = parse_release_json(json_no_matching, "2.0.4")
            .expect("should parse valid release JSON");
        assert_eq!(result, None);

        let json_empty_assets = r#"{
          "tag_name": "v2.0.5",
          "html_url": "https://github.com/OWNER/csqtt-win/releases/tag/v2.0.5",
          "assets": []
        }"#;
        let result_empty = parse_release_json(json_empty_assets, "2.0.4")
            .expect("should parse valid release JSON");
        assert_eq!(result_empty, None);
    }

    #[test]
    fn parse_release_json_zip_fallback() {
        let json_zip_only = r#"{
          "tag_name": "v2.0.5",
          "html_url": "https://github.com/OWNER/csqtt-win/releases/tag/v2.0.5",
          "body": "Zip release",
          "assets": [
            {
              "name": "csqtt-2.0.5-windows-x64.zip",
              "browser_download_url": "https://github.com/OWNER/csqtt-win/releases/download/v2.0.5/csqtt-2.0.5-windows-x64.zip"
            }
          ]
        }"#;
        let result = parse_release_json(json_zip_only, "2.0.4")
            .expect("should parse valid release JSON");
        let info = result.expect("should return UpdateInfo with zip asset");
        assert_eq!(info.asset_name, "csqtt-2.0.5-windows-x64.zip");
        assert_eq!(
            info.asset_url,
            "https://github.com/OWNER/csqtt-win/releases/download/v2.0.5/csqtt-2.0.5-windows-x64.zip"
        );
    }

    #[test]
    fn parse_release_json_tag_without_v() {
        let json = r#"{
          "tag_name": "2.0.5",
          "html_url": "https://github.com/OWNER/csqtt-win/releases/tag/2.0.5",
          "assets": [
            {
              "name": "csqtt-2.0.5.msi",
              "browser_download_url": "https://github.com/OWNER/csqtt-win/releases/download/2.0.5/csqtt-2.0.5.msi"
            }
          ]
        }"#;
        let result = parse_release_json(json, "2.0.4").expect("should parse");
        let info = result.expect("should be newer");
        assert_eq!(info.version, "2.0.5");
        assert_eq!(info.tag, "v2.0.5");
        assert_eq!(info.asset_name, "csqtt-2.0.5.msi");
    }

    #[test]
    fn parse_release_json_invalid_json() {
        let result = parse_release_json("{invalid json", "1.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_default_repo_constant() {
        assert_eq!(DEFAULT_REPO, "OWNER/csqtt-win");
    }
}
