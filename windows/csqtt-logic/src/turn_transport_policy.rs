//! Port of TurnTransportPolicy.kt.

/// Checks whether entering TCP transport mode requires confirmation.
pub fn should_confirm_tcp_transport(
    current_transport: &str,
    requested_transport: &str,
    warning_acknowledged: bool,
) -> bool {
    !warning_acknowledged
        && current_transport != crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS
        && requested_transport == crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_warning_appears_only_when_entering_tcp_without_acknowledgement() {
        assert_eq!(
            true,
            should_confirm_tcp_transport(
                crate::constants::tunnel::DEFAULT_TURN_TRANSPORT,
                crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS,
                false,
            ),
        );
        assert_eq!(
            false,
            should_confirm_tcp_transport(
                crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS,
                crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS,
                false,
            ),
        );
        assert_eq!(
            false,
            should_confirm_tcp_transport(
                crate::constants::tunnel::DEFAULT_TURN_TRANSPORT,
                crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS,
                true,
            ),
        );
    }
}
