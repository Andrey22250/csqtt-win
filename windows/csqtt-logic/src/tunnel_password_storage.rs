//! Port of TunnelPasswordStorage.kt.

use crate::stored_secret_state::{resolve_stored_secret, StoredSecret, StoredSecretState};

/// Resolves tunnel password across persistent local value, legacy plaintext, and retired encrypted storage.
pub fn resolve_tunnel_password_storage<F>(
    persistent_value: Option<&str>,
    legacy_value: Option<&str>,
    encrypted_value: Option<&str>,
    decrypt: F,
) -> StoredSecret
where
    F: FnOnce(&str) -> Option<String>,
{
    if let Some(persistent) = persistent_value.filter(|s| !s.trim().is_empty()) {
        return StoredSecret::new(persistent, StoredSecretState::Readable);
    }
    resolve_stored_secret(encrypted_value, legacy_value, decrypt)
}

/// Checks whether an unreadable retired password can be safely reset.
pub fn should_reset_unreadable_tunnel_password<F>(
    persistent_value: Option<&str>,
    legacy_value: Option<&str>,
    encrypted_value: Option<&str>,
    decrypt: F,
) -> bool
where
    F: FnOnce(&str) -> Option<String>,
{
    resolve_tunnel_password_storage(persistent_value, legacy_value, encrypted_value, decrypt).state
        == StoredSecretState::Unreadable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_tunnel_password_takes_priority_over_retired_formats() {
        assert_eq!(
            StoredSecret::new("local", StoredSecretState::Readable),
            resolve_tunnel_password_storage(
                Some("local"),
                Some("legacy"),
                Some("v1:encrypted"),
                |_| Some("encrypted".to_string()),
            ),
        );
    }

    #[test]
    fn legacy_tunnel_password_remains_available_during_migration() {
        assert_eq!(
            StoredSecret::new("legacy", StoredSecretState::Readable),
            resolve_tunnel_password_storage(
                None,
                Some("legacy"),
                None,
                |_| panic!("decrypt must not run"),
            ),
        );
    }

    #[test]
    fn old_keystore_value_can_be_copied_into_local_storage() {
        assert_eq!(
            StoredSecret::new("encrypted", StoredSecretState::Readable),
            resolve_tunnel_password_storage(
                None,
                None,
                Some("v1:encrypted"),
                |_| Some("encrypted".to_string()),
            ),
        );
    }

    #[test]
    fn unreadable_retired_keystore_value_remains_distinguishable() {
        assert_eq!(
            StoredSecret::new("", StoredSecretState::Unreadable),
            resolve_tunnel_password_storage(
                None,
                None,
                Some("v1:broken"),
                |_| None,
            ),
        );
    }

    #[test]
    fn only_an_unreadable_retired_value_is_eligible_for_reset() {
        assert_eq!(
            true,
            should_reset_unreadable_tunnel_password(None, None, Some("v1:broken"), |_| None),
        );
        assert_eq!(
            false,
            should_reset_unreadable_tunnel_password(Some("local"), None, Some("v1:broken"), |_| None),
        );
        assert_eq!(
            false,
            should_reset_unreadable_tunnel_password(None, Some("legacy"), Some("v1:broken"), |_| None),
        );
        assert_eq!(
            false,
            should_reset_unreadable_tunnel_password(None, None, None, |_| panic!("decrypt must not run")),
        );
    }

    #[test]
    fn missing_tunnel_password_remains_missing() {
        assert_eq!(
            StoredSecret::new("", StoredSecretState::Missing),
            resolve_tunnel_password_storage(
                None,
                None,
                None,
                |_| panic!("decrypt must not run"),
            ),
        );
    }
}
