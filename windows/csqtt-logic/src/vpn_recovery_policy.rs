//! Port of VpnRecoveryPolicy.kt

use serde::{Deserialize, Serialize};

/// Categories of VPN failure encountered during tunnel setup or operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VpnFailureKind {
    ServiceDestroyed,
    TunFdDeliveryFailed,
    TunEstablishFailed,
    PermissionRevoked,
}

impl VpnFailureKind {
    /// All enum variants, mirroring Kotlin's `VpnFailureKind.entries`.
    pub const ALL: [Self; 4] = [
        Self::ServiceDestroyed,
        Self::TunFdDeliveryFailed,
        Self::TunEstablishFailed,
        Self::PermissionRevoked,
    ];

    /// Diagnostic error code string associated with the failure kind.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ServiceDestroyed => "vpn_service_destroyed",
            Self::TunFdDeliveryFailed => "tun_fd_delivery_failed",
            Self::TunEstablishFailed => "tun_establish_failed",
            Self::PermissionRevoked => "vpn_permission_revoked",
        }
    }

    /// Whether this VPN failure kind is recoverable without terminating the tunnel request.
    pub const fn recoverable(&self) -> bool {
        match self {
            Self::ServiceDestroyed | Self::TunFdDeliveryFailed => true,
            Self::TunEstablishFailed | Self::PermissionRevoked => false,
        }
    }
}

/// Policy handling recovery and diagnostics for VPN failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpnRecoveryPolicy;

impl VpnRecoveryPolicy {
    /// Returns true if the failure kind allows the tunnel to remain requested.
    pub fn keeps_tunnel_requested(kind: VpnFailureKind) -> bool {
        kind.recoverable()
    }

    /// Generates the user-facing diagnostic error message for a given failure kind and detail.
    pub fn message(kind: VpnFailureKind, detail: &str) -> String {
        match kind {
            VpnFailureKind::ServiceDestroyed => {
                format!("VPN-служба Android была пересоздана: {detail}")
            }
            VpnFailureKind::TunFdDeliveryFailed => {
                format!("Rust-клиент не принял TUN-интерфейс: {detail}")
            }
            VpnFailureKind::TunEstablishFailed => {
                format!("Не удалось создать VPN-интерфейс: {detail}")
            }
            VpnFailureKind::PermissionRevoked => {
                format!("Разрешение Android VPN отозвано: {detail}")
            }
        }
    }
}

/// Standalone helper forwarding to [`VpnRecoveryPolicy::keeps_tunnel_requested`].
pub fn keeps_tunnel_requested(kind: VpnFailureKind) -> bool {
    VpnRecoveryPolicy::keeps_tunnel_requested(kind)
}

/// Standalone helper forwarding to [`VpnRecoveryPolicy::message`].
pub fn message(kind: VpnFailureKind, detail: &str) -> String {
    VpnRecoveryPolicy::message(kind, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unexpected_service_destruction_keeps_the_tunnel_requested() {
        assert!(VpnRecoveryPolicy::keeps_tunnel_requested(
            VpnFailureKind::ServiceDestroyed
        ));
    }

    #[test]
    fn tun_fd_delivery_failure_keeps_the_tunnel_requested() {
        assert!(VpnRecoveryPolicy::keeps_tunnel_requested(
            VpnFailureKind::TunFdDeliveryFailed
        ));
    }

    #[test]
    fn permission_revocation_stops_the_tunnel() {
        assert!(!VpnRecoveryPolicy::keeps_tunnel_requested(
            VpnFailureKind::PermissionRevoked
        ));
    }

    #[test]
    fn invalid_tun_establishment_stops_the_tunnel() {
        assert!(!VpnRecoveryPolicy::keeps_tunnel_requested(
            VpnFailureKind::TunEstablishFailed
        ));
    }

    #[test]
    fn every_diagnostic_has_a_stable_nonempty_code_and_message() {
        for kind in VpnFailureKind::ALL {
            assert!(!kind.code().trim().is_empty());
            assert!(!VpnRecoveryPolicy::message(kind, "detail").trim().is_empty());
        }
    }
}
