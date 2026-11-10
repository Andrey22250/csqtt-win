//! Port of VpnConfiguration pure logic.

use serde::{Deserialize, Serialize};

/// Default MTU for the VPN tunnel interface.
pub const DEFAULT_MTU: i32 = crate::constants::vpn::DEFAULT_MTU;

/// Configuration parameters for the VPN tunnel interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnConfiguration {
    pub mtu: i32,
}

impl Default for VpnConfiguration {
    fn default() -> Self {
        Self { mtu: DEFAULT_MTU }
    }
}

impl VpnConfiguration {
    pub const DEFAULT_MTU: i32 = DEFAULT_MTU;

    /// Creates a new VPN configuration with the specified MTU.
    pub fn new(mtu: i32) -> Self {
        Self { mtu }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mtu_is_1500() {
        assert_eq!(1500, crate::constants::vpn::DEFAULT_MTU);
        assert_eq!(1500, VpnConfiguration::DEFAULT_MTU);
        assert_eq!(1500, DEFAULT_MTU);
    }
}
