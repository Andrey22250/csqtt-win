//! Port of VkModePolicy.kt.

/// Selection of VK authentication and hash modes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VkModeSelection {
    pub auth_mode: String,
    pub hash_mode: String,
}

impl VkModeSelection {
    pub fn new(auth_mode: impl Into<String>, hash_mode: impl Into<String>) -> Self {
        Self {
            auth_mode: auth_mode.into(),
            hash_mode: hash_mode.into(),
        }
    }
}

/// Normalizes requested VK authentication and hash modes into a valid pair.
pub fn normalize_vk_mode_selection(
    requested_auth_mode: Option<&str>,
    requested_hash_mode: Option<&str>,
    _has_vk_token: bool,
) -> VkModeSelection {
    let (auth, hash) = normalize_raw(requested_auth_mode, requested_hash_mode);
    VkModeSelection {
        auth_mode: auth.to_string(),
        hash_mode: hash.to_string(),
    }
}

/// Determines the VK hash mode for a given requested auth mode.
pub fn vk_hash_mode_for_auth_mode(
    requested_auth_mode: &str,
    current_hash_mode: Option<&str>,
    has_vk_token: bool,
) -> &'static str {
    match requested_auth_mode {
        crate::constants::vk_auth::MODE_AUTO_JS => crate::constants::vk_auto_hash::MODE_AUTO_JS,
        crate::constants::vk_auth::MODE_CALLS => {
            if current_hash_mode == Some(crate::constants::vk_auto_hash::MODE_AUTO_JS) {
                if has_vk_token {
                    crate::constants::vk_auto_hash::MODE_AUTO_API
                } else {
                    crate::constants::vk_auto_hash::MODE_MANUAL
                }
            } else {
                normalize_raw(Some(requested_auth_mode), current_hash_mode).1
            }
        }
        _ => normalize_raw(Some(requested_auth_mode), current_hash_mode).1,
    }
}

/// Determines the VK auth mode for a given requested hash mode.
pub fn vk_auth_mode_for_hash_mode(
    requested_hash_mode: &str,
    current_auth_mode: Option<&str>,
) -> &'static str {
    match requested_hash_mode {
        crate::constants::vk_auto_hash::MODE_AUTO_JS => crate::constants::vk_auth::MODE_AUTO_JS,
        crate::constants::vk_auto_hash::MODE_AUTO_API => crate::constants::vk_auth::MODE_CALLS,
        _ => {
            if current_auth_mode == Some(crate::constants::vk_auth::MODE_AUTO_JS) {
                crate::constants::vk_auth::MODE_CALLS
            } else {
                normalize_raw(current_auth_mode, Some(requested_hash_mode)).0
            }
        }
    }
}

/// Helper that computes normalized modes as &'static str constants.
fn normalize_raw(
    requested_auth_mode: Option<&str>,
    requested_hash_mode: Option<&str>,
) -> (&'static str, &'static str) {
    let auth_mode = match requested_auth_mode {
        Some(crate::constants::vk_auth::MODE_CAPTCHA) => crate::constants::vk_auth::MODE_CAPTCHA,
        Some(crate::constants::vk_auth::MODE_AUTO_JS) => crate::constants::vk_auth::MODE_AUTO_JS,
        _ => crate::constants::vk_auth::MODE_CALLS,
    };
    let hash_mode = match requested_hash_mode {
        Some("auto") | Some(crate::constants::vk_auto_hash::MODE_AUTO_API) => {
            crate::constants::vk_auto_hash::MODE_AUTO_API
        }
        Some(crate::constants::vk_auto_hash::MODE_AUTO_JS) => {
            crate::constants::vk_auto_hash::MODE_AUTO_JS
        }
        _ => crate::constants::vk_auto_hash::MODE_MANUAL,
    };
    if auth_mode == crate::constants::vk_auth::MODE_AUTO_JS
        || hash_mode == crate::constants::vk_auto_hash::MODE_AUTO_JS
    {
        (
            crate::constants::vk_auth::MODE_AUTO_JS,
            crate::constants::vk_auto_hash::MODE_AUTO_JS,
        )
    } else {
        (auth_mode, hash_mode)
    }
}

/// Checks whether an unavailable VK hash should be replaced.
pub fn should_replace_unavailable_vk_hash(hash_mode: &str) -> bool {
    hash_mode != crate::constants::vk_auto_hash::MODE_MANUAL
}

/// Checks whether switching to auto JS mode requires confirmation.
pub fn should_confirm_auto_js_mode(
    current_mode: &str,
    requested_mode: &str,
    warning_acknowledged: bool,
) -> bool {
    !warning_acknowledged
        && current_mode != crate::constants::vk_auth::MODE_AUTO_JS
        && requested_mode == crate::constants::vk_auth::MODE_AUTO_JS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_vk_hash_selects_auto_vk_work_mode() {
        assert_eq!(
            crate::constants::vk_auth::MODE_AUTO_JS,
            vk_auth_mode_for_hash_mode(
                crate::constants::vk_auto_hash::MODE_AUTO_JS,
                Some(crate::constants::vk_auth::MODE_CALLS),
            ),
        );
    }

    #[test]
    fn auto_vk_work_mode_selects_auto_vk_hashes() {
        assert_eq!(
            crate::constants::vk_auto_hash::MODE_AUTO_JS,
            vk_hash_mode_for_auth_mode(
                crate::constants::vk_auth::MODE_AUTO_JS,
                Some(crate::constants::vk_auto_hash::MODE_AUTO_API),
                true,
            ),
        );
    }

    #[test]
    fn auto_api_hash_leaves_auto_vk_work_mode() {
        assert_eq!(
            crate::constants::vk_auth::MODE_CALLS,
            vk_auth_mode_for_hash_mode(
                crate::constants::vk_auto_hash::MODE_AUTO_API,
                Some(crate::constants::vk_auth::MODE_AUTO_JS),
            ),
        );
    }

    #[test]
    fn auto_work_mode_moves_auto_vk_hashes_to_available_non_vk_mode() {
        assert_eq!(
            crate::constants::vk_auto_hash::MODE_AUTO_API,
            vk_hash_mode_for_auth_mode(
                crate::constants::vk_auth::MODE_CALLS,
                Some(crate::constants::vk_auto_hash::MODE_AUTO_JS),
                true,
            ),
        );
        assert_eq!(
            crate::constants::vk_auto_hash::MODE_MANUAL,
            vk_hash_mode_for_auth_mode(
                crate::constants::vk_auth::MODE_CALLS,
                Some(crate::constants::vk_auto_hash::MODE_AUTO_JS),
                false,
            ),
        );
    }

    #[test]
    fn manual_hash_leaves_auto_vk_work_mode() {
        assert_eq!(
            crate::constants::vk_auth::MODE_CALLS,
            vk_auth_mode_for_hash_mode(
                crate::constants::vk_auto_hash::MODE_MANUAL,
                Some(crate::constants::vk_auth::MODE_AUTO_JS),
            ),
        );
    }

    #[test]
    fn unavailable_manual_hash_stays_in_current_process_while_auto_hash_is_replaced() {
        assert_eq!(
            false,
            should_replace_unavailable_vk_hash(crate::constants::vk_auto_hash::MODE_MANUAL),
        );
        assert_eq!(
            true,
            should_replace_unavailable_vk_hash(crate::constants::vk_auto_hash::MODE_AUTO_API),
        );
        assert_eq!(
            true,
            should_replace_unavailable_vk_hash(crate::constants::vk_auto_hash::MODE_AUTO_JS),
        );
    }

    #[test]
    fn auto_vk_warning_appears_only_when_entering_the_mode_without_acknowledgement() {
        assert_eq!(
            true,
            should_confirm_auto_js_mode(
                crate::constants::vk_auth::MODE_CALLS,
                crate::constants::vk_auth::MODE_AUTO_JS,
                false,
            ),
        );
        assert_eq!(
            false,
            should_confirm_auto_js_mode(
                crate::constants::vk_auth::MODE_AUTO_JS,
                crate::constants::vk_auth::MODE_AUTO_JS,
                false,
            ),
        );
        assert_eq!(
            false,
            should_confirm_auto_js_mode(
                crate::constants::vk_auth::MODE_CALLS,
                crate::constants::vk_auth::MODE_AUTO_JS,
                true,
            ),
        );
    }
}
