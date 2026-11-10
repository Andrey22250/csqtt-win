//! Port of TunnelAuthSnapshot in SettingsStore.kt.

use serde::{Deserialize, Serialize};
use crate::stored_secret_state::StoredSecretState;

/// In-memory snapshot of tunnel authentication configuration for an active profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TunnelAuthSnapshot {
    pub profile: i32,
    pub connection_password: String,
    pub connection_password_state: StoredSecretState,
    pub csqtt_link: String,
    pub csqtt_link_mode: bool,
}

impl Default for TunnelAuthSnapshot {
    fn default() -> Self {
        Self {
            profile: -1,
            connection_password: String::new(),
            connection_password_state: StoredSecretState::Missing,
            csqtt_link: String::new(),
            csqtt_link_mode: false,
        }
    }
}

impl TunnelAuthSnapshot {
    /// Creates a new snapshot with all fields explicitly set.
    pub fn new(
        profile: i32,
        connection_password: impl Into<String>,
        connection_password_state: StoredSecretState,
        csqtt_link: impl Into<String>,
        csqtt_link_mode: bool,
    ) -> Self {
        Self {
            profile,
            connection_password: connection_password.into(),
            connection_password_state,
            csqtt_link: csqtt_link.into(),
            csqtt_link_mode,
        }
    }

    /// Whether this snapshot corresponds to a loaded profile (profile >= 0).
    pub fn is_loaded(&self) -> bool {
        self.profile >= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_is_a_loading_sentinel() {
        assert!(!TunnelAuthSnapshot::default().is_loaded());
    }

    #[test]
    fn profile_snapshot_is_loaded_even_when_password_is_genuinely_empty() {
        let snapshot = TunnelAuthSnapshot {
            profile: 0,
            connection_password: String::new(),
            ..Default::default()
        };
        assert!(snapshot.is_loaded());
    }

    #[test]
    fn snapshot_keeps_unreadable_secret_distinct_from_a_missing_value() {
        let snapshot = TunnelAuthSnapshot {
            profile: 0,
            connection_password: String::new(),
            connection_password_state: StoredSecretState::Unreadable,
            ..Default::default()
        };

        assert!(snapshot.is_loaded());
        assert_eq!(StoredSecretState::Unreadable, snapshot.connection_password_state);
    }

    #[test]
    fn participant_snapshot_keeps_link_and_mode_in_one_state() {
        let snapshot = TunnelAuthSnapshot {
            profile: 0,
            csqtt_link: "csqtt://connect?peer=example.test".to_string(),
            csqtt_link_mode: true,
            ..Default::default()
        };

        assert!(snapshot.csqtt_link_mode);
        assert_eq!("csqtt://connect?peer=example.test", snapshot.csqtt_link);
    }
}
