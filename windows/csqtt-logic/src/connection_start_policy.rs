//! Port of ConnectionStartPolicy.kt

/// Reasons preventing a tunnel connection from starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionStartBlocker {
    Link,
    Server,
    Port,
    Password,
    StoredSecret,
    Hashes,
}

impl ConnectionStartBlocker {
    /// User-facing descriptive message explaining why connection cannot start.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Link => "нужна корректная ссылка CSQTT",
            Self::Server => "не указан корректный адрес сервера",
            Self::Port => "указан некорректный порт сервера",
            Self::Password => "не указан пароль подключения",
            Self::StoredSecret => "не удалось прочитать сохранённый пароль подключения; введите его заново",
            Self::Hashes => "нет действующих VK хешей или VK токена",
        }
    }
}

impl std::fmt::Display for ConnectionStartBlocker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Validation rules that determine whether a tunnel connection can be initiated.
pub struct ConnectionStartPolicy;

impl ConnectionStartPolicy {
    /// Checks whether ready VK credentials exist under the active connection mode.
    pub fn has_ready_vk_credentials(
        link_mode: bool,
        link_hash_count: i32,
        link_token_present: bool,
        auto_hash_mode: bool,
        stored_token_present: bool,
        manual_hash_count: i32,
    ) -> bool {
        if link_mode {
            link_hash_count > 0
                || link_token_present
                || manual_hash_count > 0
                || (auto_hash_mode && stored_token_present)
        } else if auto_hash_mode {
            stored_token_present
        } else {
            manual_hash_count > 0
        }
    }

    /// Evaluates connection preconditions and returns the first blocker encountered, if any.
    pub fn blocker(
        link_mode: bool,
        link_valid: bool,
        peer_valid: bool,
        peer_port_valid: bool,
        connection_password_set: bool,
        connection_password_readable: bool,
        hashes_ready: bool,
    ) -> Option<ConnectionStartBlocker> {
        let _ = (connection_password_set, connection_password_readable);
        if !hashes_ready {
            return Some(ConnectionStartBlocker::Hashes);
        }
        if link_mode {
            return if link_valid {
                None
            } else {
                Some(ConnectionStartBlocker::Link)
            };
        }
        if !peer_valid {
            return Some(ConnectionStartBlocker::Server);
        }
        if !peer_port_valid {
            return Some(ConnectionStartBlocker::Port);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_from_participant_link_is_ready_without_stored_owner_token() {
        assert_eq!(
            true,
            ConnectionStartPolicy::has_ready_vk_credentials(
                true,  // link_mode
                0,     // link_hash_count
                true,  // link_token_present
                true,  // auto_hash_mode
                false, // stored_token_present
                0,     // manual_hash_count
            ),
        );
    }

    #[test]
    fn participant_link_uses_explicitly_entered_hashes_when_link_has_no_vk_credentials() {
        assert_eq!(
            true,
            ConnectionStartPolicy::has_ready_vk_credentials(
                true,  // link_mode
                0,     // link_hash_count
                false, // link_token_present
                false, // auto_hash_mode
                false, // stored_token_present
                1,     // manual_hash_count
            ),
        );
    }

    #[test]
    fn participant_link_without_credentials_is_blocked_until_user_enters_them() {
        assert_eq!(
            false,
            ConnectionStartPolicy::has_ready_vk_credentials(
                true,  // link_mode
                0,     // link_hash_count
                false, // link_token_present
                false, // auto_hash_mode
                false, // stored_token_present
                0,     // manual_hash_count
            ),
        );
    }

    #[test]
    fn participant_may_connect_through_a_valid_link_with_manually_saved_hashes() {
        assert!(ConnectionStartPolicy::blocker(
            true,  // link_mode
            true,  // link_valid
            false, // peer_valid
            false, // peer_port_valid
            false, // connection_password_set
            true,  // connection_password_readable
            true,  // hashes_ready
        )
        .is_none());
    }

    #[test]
    fn missing_manual_participant_hashes_is_reported_before_attempting_native_startup() {
        assert_eq!(
            Some(ConnectionStartBlocker::Hashes),
            ConnectionStartPolicy::blocker(
                true,  // link_mode
                true,  // link_valid
                false, // peer_valid
                false, // peer_port_valid
                false, // connection_password_set
                true,  // connection_password_readable
                false, // hashes_ready
            ),
        );
    }

    #[test]
    fn owner_validation_identifies_the_first_missing_setting() {
        assert_eq!(
            Some(ConnectionStartBlocker::Server),
            ConnectionStartPolicy::blocker(
                false, // link_mode
                false, // link_valid
                false, // peer_valid
                true,  // peer_port_valid
                true,  // connection_password_set
                true,  // connection_password_readable
                true,  // hashes_ready
            ),
        );
        assert!(ConnectionStartPolicy::blocker(
            false, // link_mode
            false, // link_valid
            true,  // peer_valid
            true,  // peer_port_valid
            false, // connection_password_set
            true,  // connection_password_readable
            true,  // hashes_ready
        )
        .is_none());
    }

    #[test]
    fn unreadable_stored_secret_does_not_block_passwordless_connection() {
        assert!(ConnectionStartPolicy::blocker(
            false, // link_mode
            false, // link_valid
            true,  // peer_valid
            true,  // peer_port_valid
            false, // connection_password_set
            false, // connection_password_readable
            true,  // hashes_ready
        )
        .is_none());
    }

    #[test]
    fn link_mode_does_not_depend_on_the_local_encrypted_password() {
        assert!(ConnectionStartPolicy::blocker(
            true,  // link_mode
            true,  // link_valid
            false, // peer_valid
            false, // peer_port_valid
            false, // connection_password_set
            false, // connection_password_readable
            true,  // hashes_ready
        )
        .is_none());
    }
}
