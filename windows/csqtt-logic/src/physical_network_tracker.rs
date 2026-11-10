//! Port of PhysicalNetworkTracker.kt

use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::Hash;

/// Transition state of underlying physical network interfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicalNetworkTransition {
    None,
    Available,
    Changed,
    Unavailable,
}

impl PhysicalNetworkTransition {
    pub const NONE: Self = Self::None;
    pub const AVAILABLE: Self = Self::Available;
    pub const CHANGED: Self = Self::Changed;
    pub const UNAVAILABLE: Self = Self::Unavailable;
}

/// Reason for physical network suspension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicalNetworkPauseReason {
    AirplaneMode,
    Offline,
}

impl PhysicalNetworkPauseReason {
    pub const AIRPLANE_MODE: Self = Self::AirplaneMode;
    pub const OFFLINE: Self = Self::Offline;
}

/// Derives the network pause reason based on airplane mode state.
pub fn physical_network_pause_reason(airplane_mode_on: bool) -> PhysicalNetworkPauseReason {
    if airplane_mode_on {
        PhysicalNetworkPauseReason::AirplaneMode
    } else {
        PhysicalNetworkPauseReason::Offline
    }
}

/// Determines whether a network interface meets the criteria for being an eligible physical network.
pub fn is_eligible_physical_network(
    has_internet: bool,
    not_vpn: bool,
    wifi: bool,
    cellular: bool,
) -> bool {
    has_internet && not_vpn && (wifi || cellular)
}

/// Tracks active physical network identifiers and detects availability transitions.
#[derive(Debug, Clone)]
pub struct PhysicalNetworkTracker<T> {
    active: HashSet<T>,
}

impl<T> Default for PhysicalNetworkTracker<T> {
    fn default() -> Self {
        Self {
            active: HashSet::new(),
        }
    }
}

impl<T: Eq + Hash> PhysicalNetworkTracker<T> {
    /// Creates an empty tracker with no active networks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the state of a given network handle and reports any transition.
    pub fn update(&mut self, network: T, usable: bool) -> PhysicalNetworkTransition {
        let was_available = !self.active.is_empty();
        let changed = if usable {
            self.active.insert(network)
        } else {
            self.active.remove(&network)
        };
        let is_available = !self.active.is_empty();

        if !was_available && is_available {
            PhysicalNetworkTransition::Available
        } else if was_available && !is_available {
            PhysicalNetworkTransition::Unavailable
        } else if changed && was_available && is_available {
            PhysicalNetworkTransition::Changed
        } else {
            PhysicalNetworkTransition::None
        }
    }

    /// Clears all tracked active networks.
    pub fn clear(&mut self) {
        self.active.clear();
    }

    /// Returns true if at least one usable physical network is active.
    pub fn has_usable_network(&self) -> bool {
        !self.active.is_empty()
    }

    /// Returns true if the specified network is currently tracked as usable.
    pub fn is_usable<Q>(&self, network: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: ?Sized + Eq + Hash,
    {
        self.active.contains(network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_callbacks_produce_one_availability_transition() {
        let mut tracker = PhysicalNetworkTracker::<&str>::new();
        assert_eq!(
            PhysicalNetworkTransition::Available,
            tracker.update("mobile", true)
        );
        for _ in 0..100 {
            assert_eq!(
                PhysicalNetworkTransition::None,
                tracker.update("mobile", true)
            );
        }
    }

    #[test]
    fn overlapping_physical_networks_signal_transport_change() {
        let mut tracker = PhysicalNetworkTracker::<&str>::new();
        assert_eq!(
            PhysicalNetworkTransition::Available,
            tracker.update("mobile", true)
        );
        assert_eq!(
            PhysicalNetworkTransition::Changed,
            tracker.update("wifi", true)
        );
        assert_eq!(
            PhysicalNetworkTransition::Changed,
            tracker.update("mobile", false)
        );
        assert_eq!(
            PhysicalNetworkTransition::Unavailable,
            tracker.update("wifi", false)
        );
    }

    #[test]
    fn rapid_airplane_cycles_remain_exactly_paired() {
        let mut tracker = PhysicalNetworkTracker::<String>::new();
        for it in 0..1_000 {
            let item = format!("mobile-{it}");
            assert_eq!(
                PhysicalNetworkTransition::Available,
                tracker.update(item.clone(), true)
            );
            assert_eq!(
                PhysicalNetworkTransition::None,
                tracker.update(item.clone(), true)
            );
            assert_eq!(
                PhysicalNetworkTransition::Unavailable,
                tracker.update(item.clone(), false)
            );
            assert_eq!(
                PhysicalNetworkTransition::None,
                tracker.update(item, false)
            );
        }
    }

    #[test]
    fn pause_reason_uses_the_actual_airplane_mode_state() {
        assert_eq!(
            PhysicalNetworkPauseReason::AirplaneMode,
            physical_network_pause_reason(true)
        );
        assert_eq!(
            PhysicalNetworkPauseReason::Offline,
            physical_network_pause_reason(false)
        );
    }

    #[test]
    fn usable_state_is_stable_across_duplicate_and_clear_events() {
        let mut tracker = PhysicalNetworkTracker::<&str>::new();
        assert_eq!(false, tracker.has_usable_network());
        tracker.update("mobile", true);
        tracker.update("mobile", true);
        assert_eq!(true, tracker.has_usable_network());
        tracker.clear();
        assert_eq!(false, tracker.has_usable_network());
    }

    #[test]
    fn vpn_and_non_physical_transports_are_not_eligible_physical_networks() {
        assert_eq!(
            true,
            is_eligible_physical_network(true, true, true, false)
        );
        assert_eq!(
            true,
            is_eligible_physical_network(true, true, false, true)
        );
        assert_eq!(
            false,
            is_eligible_physical_network(true, false, true, false)
        );
        assert_eq!(
            false,
            is_eligible_physical_network(true, true, false, false)
        );
    }
}
