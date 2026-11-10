//! Port of TunRecoveryPolicy.kt

/// Policy governing whether a running TUN interface can be reused or must be rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TunRecoveryPolicy;

impl TunRecoveryPolicy {
    /// Determines whether an existing TUN interface can be reused for a connection request.
    pub fn should_reuse(
        has_interface: bool,
        descriptor_valid: bool,
        service_destroyed: bool,
        force_rebuild: bool,
        active_client_ip: Option<&str>,
        active_dns: Option<&str>,
        requested_client_ip: &str,
        requested_dns: &str,
    ) -> bool {
        has_interface
            && descriptor_valid
            && !service_destroyed
            && !force_rebuild
            && active_client_ip == Some(requested_client_ip)
            && active_dns == Some(requested_dns)
    }

    /// Determines whether the TUN interface should be restored following a revocation.
    pub fn should_restore_interface(
        _permission_granted: bool,
        _stop_requested: bool,
        _service_destroyed: bool,
        _active_client_ip: Option<&str>,
        _active_dns: Option<&str>,
    ) -> bool {
        false
    }
}

/// Standalone helper forwarding to [`TunRecoveryPolicy::should_reuse`].
pub fn should_reuse(
    has_interface: bool,
    descriptor_valid: bool,
    service_destroyed: bool,
    force_rebuild: bool,
    active_client_ip: Option<&str>,
    active_dns: Option<&str>,
    requested_client_ip: &str,
    requested_dns: &str,
) -> bool {
    TunRecoveryPolicy::should_reuse(
        has_interface,
        descriptor_valid,
        service_destroyed,
        force_rebuild,
        active_client_ip,
        active_dns,
        requested_client_ip,
        requested_dns,
    )
}

/// Standalone helper forwarding to [`TunRecoveryPolicy::should_restore_interface`].
pub fn should_restore_interface(
    permission_granted: bool,
    stop_requested: bool,
    service_destroyed: bool,
    active_client_ip: Option<&str>,
    active_dns: Option<&str>,
) -> bool {
    TunRecoveryPolicy::should_restore_interface(
        permission_granted,
        stop_requested,
        service_destroyed,
        active_client_ip,
        active_dns,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DecideParams<'a> {
        has_interface: bool,
        descriptor_valid: bool,
        service_destroyed: bool,
        force_rebuild: bool,
        active_client_ip: Option<&'a str>,
        active_dns: Option<&'a str>,
        requested_client_ip: &'a str,
        requested_dns: &'a str,
    }

    impl<'a> Default for DecideParams<'a> {
        fn default() -> Self {
            Self {
                has_interface: true,
                descriptor_valid: true,
                service_destroyed: false,
                force_rebuild: false,
                active_client_ip: Some("10.66.66.2"),
                active_dns: Some("1.1.1.1"),
                requested_client_ip: "10.66.66.2",
                requested_dns: "1.1.1.1",
            }
        }
    }

    fn decide(params: DecideParams) -> bool {
        TunRecoveryPolicy::should_reuse(
            params.has_interface,
            params.descriptor_valid,
            params.service_destroyed,
            params.force_rebuild,
            params.active_client_ip,
            params.active_dns,
            params.requested_client_ip,
            params.requested_dns,
        )
    }

    #[test]
    fn identical_configuration_reuses_live_interface() {
        assert!(decide(DecideParams::default()));
    }

    #[test]
    fn missing_interface_requires_rebuild() {
        assert!(!decide(DecideParams {
            has_interface: false,
            ..Default::default()
        }));
    }

    #[test]
    fn invalid_descriptor_requires_rebuild() {
        assert!(!decide(DecideParams {
            descriptor_valid: false,
            ..Default::default()
        }));
    }

    #[test]
    fn destroyed_service_requires_rebuild() {
        assert!(!decide(DecideParams {
            service_destroyed: true,
            ..Default::default()
        }));
    }

    #[test]
    fn forced_reload_requires_rebuild() {
        assert!(!decide(DecideParams {
            force_rebuild: true,
            ..Default::default()
        }));
    }

    #[test]
    fn changed_address_requires_rebuild() {
        assert!(!decide(DecideParams {
            requested_client_ip: "10.66.66.3",
            ..Default::default()
        }));
    }

    #[test]
    fn changed_dns_requires_rebuild() {
        assert!(!decide(DecideParams {
            requested_dns: "8.8.8.8",
            ..Default::default()
        }));
    }

    #[test]
    fn absent_previous_address_requires_rebuild() {
        assert!(!decide(DecideParams {
            active_client_ip: None,
            ..Default::default()
        }));
    }

    #[test]
    fn absent_previous_dns_requires_rebuild() {
        assert!(!decide(DecideParams {
            active_dns: None,
            ..Default::default()
        }));
    }

    #[test]
    fn do_not_restore_interface_on_revoke_even_when_permission_granted() {
        assert!(!TunRecoveryPolicy::should_restore_interface(
            true,
            false,
            false,
            Some("10.66.66.2"),
            Some("1.1.1.1"),
        ));
    }

    #[test]
    fn do_not_restore_interface_when_stop_requested() {
        assert!(!TunRecoveryPolicy::should_restore_interface(
            true,
            true,
            false,
            Some("10.66.66.2"),
            Some("1.1.1.1"),
        ));
    }
}
