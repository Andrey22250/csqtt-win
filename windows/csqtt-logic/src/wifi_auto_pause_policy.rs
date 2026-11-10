//! Port of WifiAutoPausePolicy.kt

use serde::{Deserialize, Serialize};

/// Action to take regarding Wi-Fi auto-pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WifiAutoPauseAction {
    None,
    Pause,
    Resume,
}

impl WifiAutoPauseAction {
    pub const NONE: Self = Self::None;
    pub const PAUSE: Self = Self::Pause;
    pub const RESUME: Self = Self::Resume;
}

/// Evaluates what action should be taken based on current Wi-Fi state and pause status.
pub fn wifi_auto_pause_action(
    enabled: bool,
    wifi_connected: bool,
    desired_running: bool,
    paused: bool,
) -> WifiAutoPauseAction {
    if paused && (!enabled || !wifi_connected) {
        WifiAutoPauseAction::Resume
    } else if enabled && wifi_connected && desired_running && !paused {
        WifiAutoPauseAction::Pause
    } else {
        WifiAutoPauseAction::None
    }
}

/// Determines the effective Wi-Fi connection state, taking into account fallback when paused.
pub fn effective_wifi_auto_pause_connection(
    paused: bool,
    callback_wifi_connected: bool,
    default_physical_wifi_connected: Option<bool>,
) -> bool {
    if paused {
        default_physical_wifi_connected.unwrap_or(callback_wifi_connected)
    } else {
        callback_wifi_connected
    }
}

/// Policy container for Wi-Fi auto-pause logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WifiAutoPausePolicy;

impl WifiAutoPausePolicy {
    /// Evaluates what action should be taken based on Wi-Fi and tunnel state.
    pub fn action(
        enabled: bool,
        wifi_connected: bool,
        desired_running: bool,
        paused: bool,
    ) -> WifiAutoPauseAction {
        wifi_auto_pause_action(enabled, wifi_connected, desired_running, paused)
    }

    /// Determines effective Wi-Fi connectivity.
    pub fn effective_connection(
        paused: bool,
        callback_wifi_connected: bool,
        default_physical_wifi_connected: Option<bool>,
    ) -> bool {
        effective_wifi_auto_pause_connection(
            paused,
            callback_wifi_connected,
            default_physical_wifi_connected,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauses_only_for_an_enabled_requested_tunnel_on_wifi() {
        assert_eq!(
            WifiAutoPauseAction::Pause,
            wifi_auto_pause_action(true, true, true, false)
        );
        assert_eq!(
            WifiAutoPauseAction::None,
            wifi_auto_pause_action(true, true, false, false)
        );
    }

    #[test]
    fn leaves_a_wifi_pause_when_wifi_leaves_or_the_feature_is_disabled() {
        assert_eq!(
            WifiAutoPauseAction::Resume,
            wifi_auto_pause_action(true, false, true, true)
        );
        assert_eq!(
            WifiAutoPauseAction::Resume,
            wifi_auto_pause_action(false, true, true, true)
        );
    }

    #[test]
    fn default_cellular_network_overrides_stale_wifi_callback_only_while_paused() {
        assert_eq!(
            false,
            effective_wifi_auto_pause_connection(true, true, Some(false))
        );
        assert_eq!(
            true,
            effective_wifi_auto_pause_connection(true, true, None)
        );
        assert_eq!(
            true,
            effective_wifi_auto_pause_connection(false, true, Some(false))
        );
    }
}
