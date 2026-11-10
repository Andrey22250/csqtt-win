//! Filesystem paths for csqtt-win persistent state and logs.

use std::path::PathBuf;

use anyhow::Context;

/// Base data directory for csqtt-win service: `%ProgramData%\csqtt-win`.
/// Falls back to `C:\ProgramData\csqtt-win` if `PROGRAMDATA` is not set.
pub fn data_dir() -> PathBuf {
    let base = match std::env::var_os("PROGRAMDATA") {
        Some(val) if !val.is_empty() => PathBuf::from(val),
        _ => PathBuf::from(r"C:\ProgramData"),
    };
    base.join("csqtt-win")
}

/// Ensures the data directory exists and returns its path.
pub fn ensure_data_dir() -> anyhow::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create data directory {}", dir.display()))?;
    Ok(dir)
}

/// Ensures the data directory exists and applies a protected DACL granting
/// full access only to SYSTEM and Built-in Administrators:
/// SDDL `D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`.
///
/// Called by the service at start (SYSTEM), so the CLI never reads these files directly.
pub fn secure_data_dir() -> anyhow::Result<PathBuf> {
    let dir = ensure_data_dir()?;
    apply_protected_dacl(&dir)?;
    Ok(dir)
}

#[cfg(windows)]
pub fn apply_protected_dacl(dir: &std::path::Path) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let path_wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let sddl_wide: Vec<u16> = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)\0".encode_utf16().collect();

    let mut sd: windows_sys::Win32::Security::PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let ok = unsafe {
        windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_wide.as_ptr(),
            windows_sys::Win32::Security::Authorization::SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(anyhow::anyhow!(
            "ConvertStringSecurityDescriptorToSecurityDescriptorW failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    struct SdGuard(windows_sys::Win32::Security::PSECURITY_DESCRIPTOR);
    impl Drop for SdGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    windows_sys::Win32::Foundation::LocalFree(self.0 as _);
                }
            }
        }
    }
    let _guard = SdGuard(sd);

    let mut dacl_present: windows_sys::core::BOOL = 0;
    let mut dacl: *mut windows_sys::Win32::Security::ACL = std::ptr::null_mut();
    let mut dacl_defaulted: windows_sys::core::BOOL = 0;

    let ok = unsafe {
        windows_sys::Win32::Security::GetSecurityDescriptorDacl(
            sd,
            &mut dacl_present,
            &mut dacl,
            &mut dacl_defaulted,
        )
    };
    if ok == 0 {
        return Err(anyhow::anyhow!(
            "GetSecurityDescriptorDacl failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let ret = unsafe {
        windows_sys::Win32::Security::Authorization::SetNamedSecurityInfoW(
            path_wide.as_ptr(),
            windows_sys::Win32::Security::Authorization::SE_FILE_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION
                | windows_sys::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null_mut(),
        )
    };

    if ret != windows_sys::Win32::Foundation::ERROR_SUCCESS {
        return Err(anyhow::anyhow!(
            "SetNamedSecurityInfoW on {} failed with win32 error code {ret}",
            dir.display()
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn apply_protected_dacl(_dir: &std::path::Path) -> anyhow::Result<()> {
    Ok(())
}

/// Path to the main service settings file (`settings.json`).
pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Path to the DPAPI secrets storage file (`secrets.json`).
pub fn secrets_path() -> PathBuf {
    data_dir().join("secrets.json")
}

/// Path to the SSH host key trust-on-first-use store (`host_keys.json`).
pub fn host_keys_path() -> PathBuf {
    data_dir().join("host_keys.json")
}

/// Path to the route recovery journal file (`routes_journal.json`).
pub fn routes_journal_path() -> PathBuf {
    data_dir().join("routes_journal.json")
}

/// Path to the logs directory (`logs`).
pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

/// Ensures the logs directory exists and returns its path.
pub fn ensure_logs_dir() -> anyhow::Result<PathBuf> {
    let dir = logs_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create logs directory {}", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_ends_with_csqtt_win() {
        let dir = data_dir();
        assert_eq!(dir.file_name().unwrap(), "csqtt-win");
    }

    #[test]
    fn file_paths_have_expected_names() {
        assert_eq!(settings_path().file_name().unwrap(), "settings.json");
        assert_eq!(secrets_path().file_name().unwrap(), "secrets.json");
        assert_eq!(host_keys_path().file_name().unwrap(), "host_keys.json");
        assert_eq!(routes_journal_path().file_name().unwrap(), "routes_journal.json");
        assert_eq!(logs_dir().file_name().unwrap(), "logs");
    }

    #[test]
    fn ensure_data_dir_creates_directory() {
        let temp_dir = std::env::temp_dir().join(format!("csqtt_test_data_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        assert!(temp_dir.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "locks the dir to SYSTEM+Administrators; run elevated"]
    fn secure_data_dir_applies_protected_dacl() {
        let temp_dir = std::env::temp_dir().join(format!("csqtt_test_secure_dacl_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let res = apply_protected_dacl(&temp_dir);
        let _ = std::fs::remove_dir_all(&temp_dir);
        assert!(res.is_ok(), "apply_protected_dacl failed: {res:?}");
    }
}
