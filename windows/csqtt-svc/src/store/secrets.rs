//! Windows DPAPI secret protection and persistent SecretStore.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context;
use csqtt_logic::stored_secret_state::{resolve_stored_secret, StoredSecret, StoredSecretState};

use crate::store::settings_file::{load_json, save_json};

/// Constant optional entropy for DPAPI protection scope.
const ENTROPY: &[u8] = b"csqtt-win";

/// Protects plaintext data with DPAPI using machine scope and optional entropy.
///
/// Uses `CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN` so the service
/// running as SYSTEM can read secrets across service restarts and reinstalls.
pub fn protect(plain: &[u8]) -> anyhow::Result<Vec<u8>> {
    let data_in = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        // Non-null even for an empty slice: DPAPI rejects a null pbData.
        pbData: plain.as_ptr() as *mut u8,
    };
    let entropy_blob = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut data_out = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: core::ptr::null_mut(),
    };

    // SAFETY: CryptProtectData is called with valid pointers to data and entropy.
    // Allocated output buffer is guaranteed to be freed via LocalFree before return.
    let ok = unsafe {
        windows_sys::Win32::Security::Cryptography::CryptProtectData(
            &data_in,
            core::ptr::null(),
            &entropy_blob,
            core::ptr::null(),
            core::ptr::null(),
            windows_sys::Win32::Security::Cryptography::CRYPTPROTECT_LOCAL_MACHINE
                | windows_sys::Win32::Security::Cryptography::CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )
    };

    if ok == 0 {
        return Err(std::io::Error::last_os_error()).context("CryptProtectData failed");
    }

    let slice = if data_out.cbData > 0 && !data_out.pbData.is_null() {
        // SAFETY: CryptProtectData succeeded and returned a buffer of size cbData.
        unsafe { core::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) }
    } else {
        &[]
    };
    let result = slice.to_vec();

    // SAFETY: data_out.pbData was allocated by CryptProtectData and must be freed with LocalFree.
    if !data_out.pbData.is_null() {
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(data_out.pbData as _);
        }
    }

    Ok(result)
}

/// Unprotects DPAPI ciphertext data using the machine scope and optional entropy.
pub fn unprotect(cipher: &[u8]) -> anyhow::Result<Vec<u8>> {
    if cipher.is_empty() {
        anyhow::bail!("cannot unprotect empty ciphertext");
    }

    let data_in = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: cipher.len() as u32,
        pbData: cipher.as_ptr() as *mut u8,
    };
    let entropy_blob = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut data_out = windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: core::ptr::null_mut(),
    };

    // SAFETY: CryptUnprotectData is called with valid pointers to ciphertext and entropy.
    // Allocated output buffer is guaranteed to be freed via LocalFree before return.
    let ok = unsafe {
        windows_sys::Win32::Security::Cryptography::CryptUnprotectData(
            &data_in,
            core::ptr::null_mut(),
            &entropy_blob,
            core::ptr::null(),
            core::ptr::null(),
            windows_sys::Win32::Security::Cryptography::CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )
    };

    if ok == 0 {
        return Err(std::io::Error::last_os_error()).context("CryptUnprotectData failed");
    }

    let slice = if data_out.cbData > 0 && !data_out.pbData.is_null() {
        // SAFETY: CryptUnprotectData succeeded and returned a buffer of size cbData.
        unsafe { core::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) }
    } else {
        &[]
    };
    let result = slice.to_vec();

    // SAFETY: data_out.pbData was allocated by CryptUnprotectData and must be freed with LocalFree.
    if !data_out.pbData.is_null() {
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(data_out.pbData as _);
        }
    }

    Ok(result)
}

/// Encodes byte slice to lowercase hex string.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let _ = write!(&mut out, "{:02x}", b);
    }
    out
}

/// Decodes hex string to byte vector.
pub fn from_hex(s: &str) -> anyhow::Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        anyhow::bail!("invalid hex string length: {}", s.len());
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let b = u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|e| anyhow::anyhow!("invalid hex byte at offset {}: {}", i, e))?;
        bytes.push(b);
    }
    Ok(bytes)
}

/// Encrypted secret store backed by a JSON file storing `name -> hex(DPAPI blob)`.
#[derive(Debug, Clone)]
pub struct SecretStore {
    path: PathBuf,
}

impl SecretStore {
    /// Creates a new secret store backed by the specified file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Path to the backing JSON file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load_map(&self) -> anyhow::Result<HashMap<String, String>> {
        load_json(&self.path)
    }

    fn save_map(&self, map: &HashMap<String, String>) -> anyhow::Result<()> {
        save_json(&self.path, map)
    }

    /// Retrieves and decrypts the secret with the specified name.
    ///
    /// Returns `Ok(None)` if the secret is missing.
    /// Returns an error if the encrypted blob is corrupt or decryption fails.
    pub fn get(&self, name: &str) -> anyhow::Result<Option<String>> {
        let map = self.load_map()?;
        let hex_val = match map.get(name) {
            Some(v) => v,
            None => return Ok(None),
        };
        let cipher = from_hex(hex_val)
            .with_context(|| format!("failed to decode hex secret for '{}'", name))?;
        let plain_bytes = unprotect(&cipher)
            .with_context(|| format!("failed to unprotect secret for '{}'", name))?;
        let plain_str = String::from_utf8(plain_bytes)
            .with_context(|| format!("secret '{}' is not valid UTF-8", name))?;
        Ok(Some(plain_str))
    }

    /// Encrypts and stores a secret under the specified name.
    pub fn set(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let cipher = protect(value.as_bytes())
            .with_context(|| format!("failed to protect secret for '{}'", name))?;
        let hex_val = to_hex(&cipher);
        let mut map = self.load_map().unwrap_or_default();
        map.insert(name.to_string(), hex_val);
        self.save_map(&map)
            .with_context(|| format!("failed to save secret store to {}", self.path.display()))?;
        Ok(())
    }

    /// Removes a secret by name from the store.
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let mut map = match self.load_map() {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };
        if map.remove(name).is_some() {
            self.save_map(&map)
                .with_context(|| format!("failed to save secret store to {}", self.path.display()))?;
        }
        Ok(())
    }

    /// Resolves a secret respecting legacy fallback and unreadable states,
    /// matching the pure decision logic of `StoredSecretState.kt`.
    pub fn resolve_secret(&self, name: &str, legacy_value: Option<&str>) -> StoredSecret {
        let map = match self.load_map() {
            Ok(m) => m,
            Err(_) => return StoredSecret::new("", StoredSecretState::Unreadable),
        };
        let encrypted = map.get(name).map(|s| s.as_str());
        resolve_stored_secret(encrypted, legacy_value, |hex_str| {
            let cipher = from_hex(hex_str).ok()?;
            let plain_bytes = unprotect(&cipher).ok()?;
            String::from_utf8(plain_bytes).ok()
        })
    }

    /// Gets a stored secret with explicit status (Readable, Missing, Unreadable).
    pub fn get_secret(&self, name: &str) -> StoredSecret {
        self.resolve_secret(name, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_protect_unprotect_roundtrip() {
        let original = b"csqtt-service-test-secret-12345";
        let protected = protect(original).expect("protect should succeed");
        assert_ne!(protected, original);

        let unprotected = unprotect(&protected).expect("unprotect should succeed");
        assert_eq!(unprotected, original);
    }

    #[test]
    fn dpapi_protect_unprotect_empty() {
        let original = b"";
        let protected = protect(original).expect("protect empty should succeed");
        let unprotected = unprotect(&protected).expect("unprotect empty should succeed");
        assert_eq!(unprotected, original);
    }

    #[test]
    fn secret_store_set_get_remove() {
        let tmp = std::env::temp_dir().join(format!("csqtt_secrets_test_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let store = SecretStore::new(&tmp);
        assert_eq!(store.get("ssh_key").unwrap(), None);

        store.set("ssh_key", "my-private-key-data").unwrap();
        assert_eq!(
            store.get("ssh_key").unwrap(),
            Some("my-private-key-data".to_string())
        );

        store.remove("ssh_key").unwrap();
        assert_eq!(store.get("ssh_key").unwrap(), None);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn secret_store_missing_returns_none() {
        let tmp = std::env::temp_dir().join(format!("csqtt_secrets_missing_{}.json", std::process::id()));
        let store = SecretStore::new(&tmp);
        assert_eq!(store.get("nonexistent").unwrap(), None);
    }

    // Kotlin StoredSecretStateTest ports (same test names in snake_case, same asserts).

    #[test]
    fn missing_values_remain_missing() {
        assert_eq!(
            StoredSecret::new("", StoredSecretState::Missing),
            resolve_stored_secret(None, None, |_| panic!("decrypt must not run")),
        );
    }

    #[test]
    fn legacy_plaintext_remains_readable() {
        assert_eq!(
            StoredSecret::new("legacy", StoredSecretState::Readable),
            resolve_stored_secret(None, Some("legacy"), |_| panic!("decrypt must not run")),
        );
    }

    #[test]
    fn successfully_decrypted_value_is_preferred_over_legacy_plaintext() {
        assert_eq!(
            StoredSecret::new("encrypted", StoredSecretState::Readable),
            resolve_stored_secret(Some("v1:data"), Some("legacy"), |_| Some("encrypted".to_string())),
        );
    }

    #[test]
    fn unreadable_encrypted_secret_is_explicit_when_no_fallback_exists() {
        assert_eq!(
            StoredSecret::new("", StoredSecretState::Unreadable),
            resolve_stored_secret(Some("v1:broken"), None, |_| None),
        );
    }

    #[test]
    fn legacy_plaintext_recovers_an_unreadable_encrypted_secret() {
        assert_eq!(
            StoredSecret::new("legacy", StoredSecretState::Readable),
            resolve_stored_secret(Some("v1:broken"), Some("legacy"), |_| None),
        );
    }

    #[test]
    fn readable_empty_encrypted_value_is_not_reported_as_a_keystore_failure() {
        assert_eq!(
            StoredSecret::new("", StoredSecretState::Readable),
            resolve_stored_secret(Some("v1:empty"), None, |_| Some(String::new())),
        );
    }
}
