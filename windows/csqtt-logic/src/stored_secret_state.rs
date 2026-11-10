//! Port of StoredSecretState.kt.

use serde::{Deserialize, Serialize};

/// State of a stored secret such as passwords or private keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoredSecretState {
    Readable,
    Missing,
    Unreadable,
}

/// A stored secret along with its readability state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoredSecret {
    pub value: String,
    pub state: StoredSecretState,
}

impl StoredSecret {
    /// Creates a new stored secret with the specified value and state.
    pub fn new(value: impl Into<String>, state: StoredSecretState) -> Self {
        Self {
            value: value.into(),
            state,
        }
    }
}

/// Resolves a stored secret from encrypted storage or legacy plaintext fallback.
pub fn resolve_stored_secret<F>(
    encrypted_value: Option<&str>,
    legacy_value: Option<&str>,
    decrypt: F,
) -> StoredSecret
where
    F: FnOnce(&str) -> Option<String>,
{
    let is_encrypted_blank = match encrypted_value {
        Some(s) => s.trim().is_empty(),
        None => true,
    };

    if is_encrypted_blank {
        return if let Some(legacy) = legacy_value.filter(|s| !s.trim().is_empty()) {
            StoredSecret::new(legacy, StoredSecretState::Readable)
        } else {
            StoredSecret::new("", StoredSecretState::Missing)
        };
    }

    let encrypted = encrypted_value.unwrap();
    if let Some(decrypted) = decrypt(encrypted) {
        return StoredSecret::new(decrypted, StoredSecretState::Readable);
    }

    if let Some(legacy) = legacy_value.filter(|s| !s.trim().is_empty()) {
        StoredSecret::new(legacy, StoredSecretState::Readable)
    } else {
        StoredSecret::new("", StoredSecretState::Unreadable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
