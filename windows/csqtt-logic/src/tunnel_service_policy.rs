//! Port of TunnelServicePolicy.kt

/// Policy governing when the background tunnel service should stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TunnelServicePolicy;

impl TunnelServicePolicy {
    /// Startup grace period in milliseconds before giving up on an unstarted tunnel.
    pub const STARTUP_GRACE_MS: i64 = 60_000;

    /// Evaluates whether the tunnel service should terminate.
    pub fn should_stop(
        was_ever_running: bool,
        running: bool,
        elapsed_ms: i64,
    ) -> bool {
        !running && (was_ever_running || elapsed_ms >= Self::STARTUP_GRACE_MS)
    }
}

/// Standalone constant mirroring [`TunnelServicePolicy::STARTUP_GRACE_MS`].
pub const STARTUP_GRACE_MS: i64 = TunnelServicePolicy::STARTUP_GRACE_MS;

/// Standalone helper forwarding to [`TunnelServicePolicy::should_stop`].
pub fn should_stop(
    was_ever_running: bool,
    running: bool,
    elapsed_ms: i64,
) -> bool {
    TunnelServicePolicy::should_stop(was_ever_running, running, elapsed_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_does_not_stop_before_grace_period() {
        assert!(!TunnelServicePolicy::should_stop(
            false,
            false,
            TunnelServicePolicy::STARTUP_GRACE_MS - 1,
        ));
    }

    #[test]
    fn startup_failure_stops_after_grace_period() {
        assert!(TunnelServicePolicy::should_stop(
            false,
            false,
            TunnelServicePolicy::STARTUP_GRACE_MS,
        ));
    }

    #[test]
    fn stopped_running_tunnel_stops_immediately() {
        assert!(TunnelServicePolicy::should_stop(
            true,
            false,
            1_000,
        ));
    }

    #[test]
    fn running_tunnel_keeps_service_alive() {
        assert!(!TunnelServicePolicy::should_stop(
            true,
            true,
            TunnelServicePolicy::STARTUP_GRACE_MS,
        ));
    }
}
