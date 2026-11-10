//! Atomic JSON persistence with corrupt-file recovery.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Context;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Loads JSON from a file.
///
/// If the file does not exist, returns [`T::default()`].
/// If the file is corrupt (cannot be parsed as valid JSON for type `T`),
/// renames the corrupt file to `<path>.bad-<unix_time>`, logs a warning via [`eprintln!`],
/// and returns [`T::default()`].
pub fn load_json<T: serde::de::DeserializeOwned + Default>(path: impl AsRef<Path>) -> anyhow::Result<T> {
    let path = path.as_ref();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(e) => return Err(e).with_context(|| format!("failed to read file {}", path.display())),
    };

    match serde_json::from_slice::<T>(&bytes) {
        Ok(value) => Ok(value),
        Err(err) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let bad_path = match path.file_name() {
                Some(name) => {
                    let mut bad_name = name.to_os_string();
                    bad_name.push(format!(".bad-{}", now));
                    path.with_file_name(bad_name)
                }
                None => path.with_extension(format!("bad-{}", now)),
            };

            eprintln!(
                "Corrupt JSON in {}: {}. Renaming to {} and returning default values.",
                path.display(),
                err,
                bad_path.display()
            );

            if let Err(rename_err) = std::fs::rename(path, &bad_path) {
                eprintln!(
                    "Failed to rename corrupt file {} to {}: {}",
                    path.display(),
                    bad_path.display(),
                    rename_err
                );
            }

            Ok(T::default())
        }
    }
}

/// Atomically saves a serializable value to disk as pretty JSON.
///
/// Writes to a temporary file (`<path>.tmp.<pid>.<seq>`), flushes, invokes `sync_all`,
/// and replaces the destination file via [`std::fs::rename`].
pub fn save_json<T: serde::Serialize>(path: impl AsRef<Path>, value: &T) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
    }

    let json_bytes = serde_json::to_vec_pretty(value)
        .context("failed to serialize value to JSON")?;

    let count = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = match path.file_name() {
        Some(name) => {
            let mut tmp_name = name.to_os_string();
            tmp_name.push(format!(".tmp.{}.{}", std::process::id(), count));
            path.with_file_name(tmp_name)
        }
        None => path.with_extension(format!("tmp.{}.{}", std::process::id(), count)),
    };

    let write_res = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(&json_bytes)?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e).with_context(|| format!("failed to write temporary file {}", tmp_path.display()));
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e).with_context(|| {
            format!("failed to rename {} to {}", tmp_path.display(), path.display())
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    struct SampleConfig {
        host: String,
        port: u16,
        enabled: bool,
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = std::env::temp_dir().join(format!("csqtt_missing_test_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let cfg: SampleConfig = load_json(&tmp).unwrap();
        assert_eq!(cfg, SampleConfig::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("csqtt_roundtrip_test_{}.json", std::process::id()));
        let expected = SampleConfig {
            host: "vpn.example.com".to_string(),
            port: 46000,
            enabled: true,
        };

        save_json(&tmp, &expected).unwrap();
        let loaded: SampleConfig = load_json(&tmp).unwrap();
        assert_eq!(loaded, expected);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn corrupt_file_is_renamed_and_returns_default() {
        let dir = std::env::temp_dir().join(format!("csqtt_corrupt_dir_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("config.json");

        std::fs::write(&file_path, b"{ not valid json syntax ...").unwrap();

        let loaded: SampleConfig = load_json(&file_path).unwrap();
        assert_eq!(loaded, SampleConfig::default());
        assert!(!file_path.exists());

        let mut bad_found = false;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("config.json.bad-") {
                    bad_found = true;
                    let content = std::fs::read(entry.path()).unwrap();
                    assert_eq!(content, b"{ not valid json syntax ...");
                }
            }
        }
        assert!(bad_found, "expected corrupt file to be renamed to .bad-<time>");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_save_overwrites_existing_file() {
        let tmp = std::env::temp_dir().join(format!("csqtt_overwrite_test_{}.json", std::process::id()));
        let first = SampleConfig {
            host: "first.com".to_string(),
            port: 1,
            enabled: false,
        };
        let second = SampleConfig {
            host: "second.com".to_string(),
            port: 2,
            enabled: true,
        };

        save_json(&tmp, &first).unwrap();
        let loaded1: SampleConfig = load_json(&tmp).unwrap();
        assert_eq!(loaded1, first);

        save_json(&tmp, &second).unwrap();
        let loaded2: SampleConfig = load_json(&tmp).unwrap();
        assert_eq!(loaded2, second);

        let _ = std::fs::remove_file(&tmp);
    }
}
