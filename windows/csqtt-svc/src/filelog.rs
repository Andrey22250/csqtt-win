//! Rotating file logger for the host service and core process.
//!
//! Provides rotating file logging with automatic redaction of sensitive
//! information (passwords, tokens, VK credentials).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Context;

/// Rotating file logger. Thread-safe (`Send + Sync`) and cloneable (`Clone`).
#[derive(Clone)]
pub struct FileLog {
    inner: Arc<Mutex<FileLogInner>>,
}

struct FileLogInner {
    dir: PathBuf,
    base_name: String,
    main_path: PathBuf,
    max_bytes: u64,
    keep: usize,
    current_size: u64,
    file: Option<File>,
}

impl FileLog {
    /// Opens or creates the main log file `<dir>/<base_name>.log`.
    ///
    /// If the directory does not exist, it will be created. Log output is appended
    /// to `<base_name>.log`. When a write would cause the file size to exceed `max_bytes`,
    /// the logs are rotated: `<base>.log` -> `<base>.1.log` ... up to `keep` files,
    /// with the oldest file beyond `keep` deleted.
    pub fn open(dir: &Path, base_name: &str, max_bytes: u64, keep: usize) -> anyhow::Result<Self> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create log directory {}", dir.display()))?;

        let main_path = dir.join(format!("{base_name}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&main_path)
            .with_context(|| format!("failed to open log file {}", main_path.display()))?;

        let current_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        Ok(Self {
            inner: Arc::new(Mutex::new(FileLogInner {
                dir: dir.to_path_buf(),
                base_name: base_name.to_string(),
                main_path,
                max_bytes,
                keep,
                current_size,
                file: Some(file),
            })),
        })
    }

    /// Writes a line to the log file with a timestamp prefix `YYYY-MM-DD HH:MM:SS.mmm `.
    ///
    /// Values after sensitive keys (such as `password=`, `token=`, VK tokens) are automatically
    /// redacted. Never panics and swallows I/O errors.
    pub fn write_line(&self, line: &str) {
        let ts = format_utc_timestamp(std::time::SystemTime::now());
        let redacted = redact(line.trim_end_matches(['\r', '\n']));
        let formatted = format!("{ts}{redacted}\n");

        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let _ = inner.write_entry(formatted.as_bytes());
    }

    /// Spawns an asynchronous task that reads events from the manager broadcast receiver
    /// and logs them to the file.
    ///
    /// - `ManagerEvent::Log` lines are written directly.
    /// - `ManagerEvent::Status` changes are written as `== статус: ...`.
    /// - `ManagerEvent::Stats` events are ignored.
    pub fn spawn_manager_logger(
        &self,
        mut events: tokio::sync::broadcast::Receiver<crate::manager::ManagerEvent>,
    ) -> tokio::task::JoinHandle<()> {
        let logger = self.clone();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(crate::manager::ManagerEvent::Log(line)) | Ok(crate::manager::ManagerEvent::CoreLine(line)) => {
                        logger.write_line(&line);
                    }
                    Ok(crate::manager::ManagerEvent::Status(status)) => {
                        logger.write_line(&format!("== статус: {status:?}"));
                    }
                    Ok(crate::manager::ManagerEvent::Stats { .. }) | Ok(crate::manager::ManagerEvent::Keyed(_)) => {
                        // Ignored by design
                    }
                    Ok(crate::manager::ManagerEvent::CaptchaRequest { redirect_uri, .. }) => {
                        logger.write_line(&format!("== капча: {redirect_uri}"));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Receiver lagged behind, continue processing next events
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        })
    }
}

impl FileLogInner {
    fn rotate(&mut self) -> std::io::Result<()> {
        // Drop file handle before renaming on Windows to avoid sharing violations
        drop(self.file.take());

        if self.keep > 0 {
            // Delete oldest if it exists: <base>.<keep>.log
            let oldest = self.dir.join(format!("{}.{}.log", self.base_name, self.keep));
            if oldest.exists() {
                let _ = std::fs::remove_file(&oldest);
            }

            // Shift existing <base>.<i-1>.log -> <base>.<i>.log for i in keep down to 2
            for i in (2..=self.keep).rev() {
                let src = self.dir.join(format!("{}.{}.log", self.base_name, i - 1));
                let dst = self.dir.join(format!("{}.{}.log", self.base_name, i));
                if src.exists() {
                    if dst.exists() {
                        let _ = std::fs::remove_file(&dst);
                    }
                    let _ = std::fs::rename(&src, &dst);
                }
            }

            // Rename main log -> <base>.1.log
            let log_1 = self.dir.join(format!("{}.1.log", self.base_name));
            if self.main_path.exists() {
                if log_1.exists() {
                    let _ = std::fs::remove_file(&log_1);
                }
                let _ = std::fs::rename(&self.main_path, &log_1);
            }
        } else {
            if self.main_path.exists() {
                let _ = std::fs::remove_file(&self.main_path);
            }
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.main_path)?;

        self.current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(file);
        Ok(())
    }

    fn write_entry(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let entry_len = bytes.len() as u64;

        if self.max_bytes > 0
            && self.current_size > 0
            && (self.current_size + entry_len > self.max_bytes)
        {
            let _ = self.rotate();
        }

        if self.file.is_none() {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.main_path)?;
            self.current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
            self.file = Some(file);
        }

        if let Some(ref mut file) = self.file {
            file.write_all(bytes)?;
            file.flush()?;
            self.current_size += entry_len;
        }

        Ok(())
    }
}

/// Helper to check if byte slice starts with a prefix ignoring ASCII case.
fn starts_with_ignore_ascii_case(slice: &[u8], prefix: &[u8]) -> bool {
    if slice.len() < prefix.len() {
        return false;
    }
    slice[..prefix.len()]
        .iter()
        .zip(prefix.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
}

/// Formats UTC timestamp `YYYY-MM-DD HH:MM:SS.mmm ` computed by hand from `SystemTime`.
fn format_utc_timestamp(now: std::time::SystemTime) -> String {
    let dur = match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => std::time::Duration::ZERO,
    };
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();

    let days = (total_secs / 86400) as i64;
    let day_secs = (total_secs % 86400) as u32;

    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // Howard Hinnant's algorithm to convert days since 1970-01-01 to (year, month, day)
    let z = days + 719468;
    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} ", y, m, d, hour, minute, second, millis)
}

/// Redacts sensitive information from log lines.
///
/// Masks values after:
/// - `password=`
/// - `token=`
/// - `access_token=`
/// - `-password `
/// - `CSQTT_PASSWORD=`
/// - anything that looks like `vk1.a.<...>` tokens
pub fn redact(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut result = String::with_capacity(line.len());
    let mut i = 0;

    // Ordered so that longer / more specific prefixes match before substrings
    // (e.g. `csqtt_password=` before `password=`, `access_token=` before `token=`).
    const PREFIXES: &[&[u8]] = &[
        b"csqtt_password=",
        b"access_token=",
        b"-password ",
        b"password=",
        b"token=",
        b"vk1.a.",
    ];

    while i < bytes.len() {
        let mut matched: Option<&[u8]> = None;
        for &p in PREFIXES {
            if starts_with_ignore_ascii_case(&bytes[i..], p) {
                matched = Some(p);
                break;
            }
        }

        if let Some(prefix) = matched {
            let prefix_len = prefix.len();
            // Preserve the original prefix casing
            result.push_str(&line[i..i + prefix_len]);
            let mut val_idx = i + prefix_len;

            if prefix == b"-password " {
                while val_idx < bytes.len() && bytes[val_idx] == b' ' {
                    result.push(' ');
                    val_idx += 1;
                }
            }

            if val_idx < bytes.len() && (bytes[val_idx] == b'"' || bytes[val_idx] == b'\'') {
                let quote = bytes[val_idx];
                result.push(quote as char);
                val_idx += 1;
                while val_idx < bytes.len() && bytes[val_idx] != quote {
                    val_idx += 1;
                }
                result.push_str("***");
                if val_idx < bytes.len() {
                    result.push(quote as char);
                    val_idx += 1;
                }
                i = val_idx;
            } else {
                while val_idx < bytes.len() {
                    let b = bytes[val_idx];
                    if b.is_ascii_whitespace()
                        || b == b'&'
                        || b == b','
                        || b == b';'
                        || b == b'"'
                        || b == b'\''
                        || b == b')'
                        || b == b']'
                        || b == b'}'
                        || b == b'>'
                        || b == b'<'
                    {
                        break;
                    }
                    val_idx += 1;
                }
                result.push_str("***");
                i = val_idx;
            }
        } else {
            if let Some(ch) = line[i..].chars().next() {
                result.push(ch);
                i += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_passwords_and_tokens() {
        assert_eq!(redact("password=secret123"), "password=***");
        assert_eq!(redact("PASSWORD=SECRET123"), "PASSWORD=***");
        assert_eq!(redact("CSQTT_PASSWORD=env_secret"), "CSQTT_PASSWORD=***");
        assert_eq!(redact("-password my_secret_pass"), "-password ***");
        assert_eq!(redact("token=some_token_here"), "token=***");
        assert_eq!(redact("access_token=acc_token_here"), "access_token=***");
    }

    #[test]
    fn test_redact_vk1_tokens() {
        assert_eq!(redact("vk1.a.secret_payload_12345"), "vk1.a.***");
        assert_eq!(
            redact("got token vk1.a.abc-XYZ_1234 from api"),
            "got token vk1.a.*** from api"
        );
        assert_eq!(
            redact("https://oauth.vk.com/blank.html#access_token=vk1.a.ABC&expires_in=0"),
            "https://oauth.vk.com/blank.html#access_token=***&expires_in=0"
        );
    }

    #[test]
    fn test_redact_quoted_and_complex_lines() {
        assert_eq!(
            redact("-password \"secret with spaces\" -peer 1.2.3.4"),
            "-password \"***\" -peer 1.2.3.4"
        );
        assert_eq!(
            redact("foo password='secret' token=abc123"),
            "foo password='***' token=***"
        );
        assert_eq!(
            redact("connection_password=test123, other=val"),
            "connection_password=***, other=val"
        );
    }

    #[test]
    fn test_redact_innocuous_lines() {
        let line = "[КЛИЕНТ] Подключено к серверу 203.0.113.10:46000 (воркеров: 5)";
        assert_eq!(redact(line), line);
    }

    #[test]
    fn test_filelog_rotation() {
        let temp_dir = std::env::temp_dir().join(format!("csqtt_filelog_rot_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        // max_bytes = 100, keep = 3
        let log = FileLog::open(&temp_dir, "test_svc", 100, 3).expect("open should succeed");

        // Write several lines. Each entry has ~25 bytes timestamp/newline + line content.
        // Line of 50 chars -> ~75 bytes.
        // 2 lines will exceed 100 bytes and trigger rotation.
        for i in 1..=10 {
            log.write_line(&format!("Log entry number {:02} with padding data...", i));
        }

        // Verify that main log exists
        let main_log = temp_dir.join("test_svc.log");
        assert!(main_log.exists(), "test_svc.log must exist");

        // Verify rotated files exist up to keep = 3
        assert!(temp_dir.join("test_svc.1.log").exists(), "test_svc.1.log must exist");
        assert!(temp_dir.join("test_svc.2.log").exists(), "test_svc.2.log must exist");
        assert!(temp_dir.join("test_svc.3.log").exists(), "test_svc.3.log must exist");

        // test_svc.4.log must NOT exist because keep = 3
        assert!(!temp_dir.join("test_svc.4.log").exists(), "test_svc.4.log must not exist");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filelog_redaction_on_write() {
        let temp_dir = std::env::temp_dir().join(format!("csqtt_filelog_redact_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let log = FileLog::open(&temp_dir, "redact_test", 10_000, 2).expect("open should succeed");
        log.write_line("Connecting with password=super_secret_vpn_pwd and vk1.a.TOPSECRET");

        let content = std::fs::read_to_string(temp_dir.join("redact_test.log")).unwrap();
        assert!(content.contains("password=***"));
        assert!(content.contains("vk1.a.***"));
        assert!(!content.contains("super_secret_vpn_pwd"));
        assert!(!content.contains("TOPSECRET"));

        // Verify timestamp prefix format YYYY-MM-DD HH:MM:SS.mmm
        let first_line = content.lines().next().unwrap();
        assert!(first_line.len() >= 24);
        assert_eq!(&first_line[4..5], "-");
        assert_eq!(&first_line[7..8], "-");
        assert_eq!(&first_line[10..11], " ");
        assert_eq!(&first_line[13..14], ":");
        assert_eq!(&first_line[16..17], ":");
        assert_eq!(&first_line[19..20], ".");
        assert_eq!(&first_line[23..24], " ");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_spawn_manager_logger() {
        let temp_dir = std::env::temp_dir().join(format!("csqtt_filelog_mgr_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let log = FileLog::open(&temp_dir, "mgr_test", 10_000, 2).expect("open should succeed");
        let (tx, rx) = tokio::sync::broadcast::channel(16);

        let handle = log.spawn_manager_logger(rx);

        tx.send(crate::manager::ManagerEvent::Log("Core started".into())).unwrap();
        tx.send(crate::manager::ManagerEvent::Status(crate::manager::Status::Starting)).unwrap();
        tx.send(crate::manager::ManagerEvent::Stats { active: 5, bytes_up: 1024, bytes_down: 2048 }).unwrap();

        // Drop tx to close broadcast channel and terminate logger task
        drop(tx);
        let _ = handle.await;

        let content = std::fs::read_to_string(temp_dir.join("mgr_test.log")).unwrap();
        assert!(content.contains("Core started"));
        assert!(content.contains("== статус: Starting"));
        // Stats should be ignored
        assert!(!content.contains("bytes_up"));
        assert!(!content.contains("1024"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
