//! Port of UpdateLaunchGuard.kt.

/// Builds the package version token string formatted as "version_code:last_update_time".
pub fn format_package_token(version_code: i64, last_update_time: i64) -> String {
    format!("{version_code}:{last_update_time}")
}

/// Evaluates whether saved state from a previous version should be discarded after an update.
pub fn should_discard_stale_saved_state(
    previous_token: Option<&str>,
    current_token: &str,
    package_was_updated: bool,
) -> bool {
    previous_token != Some(current_token) && (previous_token.is_some() || package_was_updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_install_keeps_its_normal_saved_state() {
        assert!(!should_discard_stale_saved_state(
            None,
            "205:1000",
            false,
        ));
    }

    #[test]
    fn new_package_token_clears_stale_state_once() {
        assert!(should_discard_stale_saved_state(
            Some("205:1000"),
            "206:2000",
            true,
        ));
        assert!(!should_discard_stale_saved_state(
            Some("206:2000"),
            "206:2000",
            true,
        ));
    }

    #[test]
    fn an_existing_update_with_a_cleared_marker_still_clears_stale_state() {
        assert!(should_discard_stale_saved_state(
            None,
            "206:2000",
            true,
        ));
    }
}
