//! Port of TurnSubnetRegistry.kt.

use std::sync::LazyLock;

/// Representation of a known TURN subnet and its IP addresses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TurnSubnet {
    pub id: String,
    pub title: String,
    pub subnet_cidr: String,
    pub count: i32,
    pub ips: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl TurnSubnet {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        subnet_cidr: impl Into<String>,
        count: i32,
        ips: Vec<String>,
        aliases: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subnet_cidr: subnet_cidr.into(),
            count,
            ips,
            aliases,
        }
    }
}

/// Static registry of known TURN subnets.
pub static SUBNETS: LazyLock<Vec<TurnSubnet>> = LazyLock::new(|| {
    vec![
        TurnSubnet {
            id: "95_163_34_0_24".to_string(),
            title: "95.163.34.0/24".to_string(),
            subnet_cidr: "95.163.34.0/24".to_string(),
            count: 64,
            ips: vec![
                "95.163.34.48".to_string(),
                "95.163.34.128".to_string(),
                "95.163.34.129".to_string(),
                "95.163.34.130".to_string(),
                "95.163.34.131".to_string(),
                "95.163.34.132".to_string(),
                "95.163.34.133".to_string(),
                "95.163.34.134".to_string(),
                "95.163.34.135".to_string(),
                "95.163.34.136".to_string(),
                "95.163.34.137".to_string(),
                "95.163.34.138".to_string(),
                "95.163.34.139".to_string(),
                "95.163.34.140".to_string(),
                "95.163.34.141".to_string(),
                "95.163.34.142".to_string(),
                "95.163.34.143".to_string(),
                "95.163.34.144".to_string(),
                "95.163.34.145".to_string(),
                "95.163.34.146".to_string(),
                "95.163.34.147".to_string(),
                "95.163.34.148".to_string(),
                "95.163.34.149".to_string(),
                "95.163.34.150".to_string(),
                "95.163.34.151".to_string(),
                "95.163.34.152".to_string(),
                "95.163.34.153".to_string(),
                "95.163.34.154".to_string(),
                "95.163.34.155".to_string(),
                "95.163.34.156".to_string(),
                "95.163.34.157".to_string(),
                "95.163.34.158".to_string(),
                "95.163.34.159".to_string(),
                "95.163.34.160".to_string(),
                "95.163.34.161".to_string(),
                "95.163.34.162".to_string(),
                "95.163.34.163".to_string(),
                "95.163.34.164".to_string(),
                "95.163.34.165".to_string(),
                "95.163.34.166".to_string(),
                "95.163.34.167".to_string(),
                "95.163.34.168".to_string(),
                "95.163.34.169".to_string(),
                "95.163.34.170".to_string(),
                "95.163.34.171".to_string(),
                "95.163.34.172".to_string(),
                "95.163.34.173".to_string(),
                "95.163.34.174".to_string(),
                "95.163.34.175".to_string(),
                "95.163.34.176".to_string(),
                "95.163.34.177".to_string(),
                "95.163.34.178".to_string(),
                "95.163.34.179".to_string(),
                "95.163.34.180".to_string(),
                "95.163.34.181".to_string(),
                "95.163.34.182".to_string(),
                "95.163.34.183".to_string(),
                "95.163.34.184".to_string(),
                "95.163.34.185".to_string(),
                "95.163.34.186".to_string(),
                "95.163.34.187".to_string(),
                "95.163.34.188".to_string(),
                "95.163.34.189".to_string(),
                "95.163.34.190".to_string(),
            ],
            aliases: vec![],
        },
        TurnSubnet {
            id: "91_231_135_0_24".to_string(),
            title: "91.231.135.0/24".to_string(),
            subnet_cidr: "91.231.135.0/24".to_string(),
            count: 64,
            ips: vec![
                "91.231.135.85".to_string(),
                "91.231.135.86".to_string(),
                "91.231.135.87".to_string(),
                "91.231.135.88".to_string(),
                "91.231.135.89".to_string(),
                "91.231.135.90".to_string(),
                "91.231.135.91".to_string(),
                "91.231.135.92".to_string(),
                "91.231.135.93".to_string(),
                "91.231.135.94".to_string(),
                "91.231.135.95".to_string(),
                "91.231.135.128".to_string(),
                "91.231.135.129".to_string(),
                "91.231.135.130".to_string(),
                "91.231.135.131".to_string(),
                "91.231.135.132".to_string(),
                "91.231.135.133".to_string(),
                "91.231.135.134".to_string(),
                "91.231.135.135".to_string(),
                "91.231.135.136".to_string(),
                "91.231.135.137".to_string(),
                "91.231.135.138".to_string(),
                "91.231.135.139".to_string(),
                "91.231.135.140".to_string(),
                "91.231.135.141".to_string(),
                "91.231.135.142".to_string(),
                "91.231.135.143".to_string(),
                "91.231.135.144".to_string(),
                "91.231.135.145".to_string(),
                "91.231.135.146".to_string(),
                "91.231.135.147".to_string(),
                "91.231.135.148".to_string(),
                "91.231.135.149".to_string(),
                "91.231.135.150".to_string(),
                "91.231.135.151".to_string(),
                "91.231.135.152".to_string(),
                "91.231.135.153".to_string(),
                "91.231.135.154".to_string(),
                "91.231.135.155".to_string(),
                "91.231.135.167".to_string(),
                "91.231.135.168".to_string(),
                "91.231.135.169".to_string(),
                "91.231.135.170".to_string(),
                "91.231.135.171".to_string(),
                "91.231.135.172".to_string(),
                "91.231.135.173".to_string(),
                "91.231.135.174".to_string(),
                "91.231.135.175".to_string(),
                "91.231.135.176".to_string(),
                "91.231.135.177".to_string(),
                "91.231.135.178".to_string(),
                "91.231.135.179".to_string(),
                "91.231.135.180".to_string(),
                "91.231.135.181".to_string(),
                "91.231.135.182".to_string(),
                "91.231.135.183".to_string(),
                "91.231.135.184".to_string(),
                "91.231.135.185".to_string(),
                "91.231.135.186".to_string(),
                "91.231.135.187".to_string(),
                "91.231.135.188".to_string(),
                "91.231.135.189".to_string(),
                "91.231.135.190".to_string(),
                "91.231.135.191".to_string(),
            ],
            aliases: vec!["91_231_132_0_22".to_string()],
        },
        TurnSubnet {
            id: "90_156_232_0_21".to_string(),
            title: "90.156.232.0/21".to_string(),
            subnet_cidr: "90.156.232.0/21".to_string(),
            count: 51,
            ips: vec![
                "90.156.234.253".to_string(),
                "90.156.234.254".to_string(),
                "90.156.234.255".to_string(),
                "90.156.236.64".to_string(),
                "90.156.236.65".to_string(),
                "90.156.236.66".to_string(),
                "90.156.236.67".to_string(),
                "90.156.236.68".to_string(),
                "90.156.236.69".to_string(),
                "90.156.236.70".to_string(),
                "90.156.236.71".to_string(),
                "90.156.236.72".to_string(),
                "90.156.236.73".to_string(),
                "90.156.236.74".to_string(),
                "90.156.236.75".to_string(),
                "90.156.236.76".to_string(),
                "90.156.236.77".to_string(),
                "90.156.236.78".to_string(),
                "90.156.236.79".to_string(),
                "90.156.236.80".to_string(),
                "90.156.236.81".to_string(),
                "90.156.236.82".to_string(),
                "90.156.236.83".to_string(),
                "90.156.236.84".to_string(),
                "90.156.236.85".to_string(),
                "90.156.236.86".to_string(),
                "90.156.236.87".to_string(),
                "90.156.236.88".to_string(),
                "90.156.236.89".to_string(),
                "90.156.236.90".to_string(),
                "90.156.236.91".to_string(),
                "90.156.236.92".to_string(),
                "90.156.236.93".to_string(),
                "90.156.236.94".to_string(),
                "90.156.236.95".to_string(),
                "90.156.236.96".to_string(),
                "90.156.236.97".to_string(),
                "90.156.236.98".to_string(),
                "90.156.236.99".to_string(),
                "90.156.236.100".to_string(),
                "90.156.236.101".to_string(),
                "90.156.236.102".to_string(),
                "90.156.236.103".to_string(),
                "90.156.236.104".to_string(),
                "90.156.236.105".to_string(),
                "90.156.236.106".to_string(),
                "90.156.236.107".to_string(),
                "90.156.236.108".to_string(),
                "90.156.236.114".to_string(),
                "90.156.236.115".to_string(),
                "90.156.236.117".to_string(),
            ],
            aliases: vec!["90_156_236_0_24".to_string()],
        },
        TurnSubnet {
            id: "193_203_43_0_24".to_string(),
            title: "193.203.43.0/24".to_string(),
            subnet_cidr: "193.203.43.0/24".to_string(),
            count: 48,
            ips: vec![
                "193.203.43.0".to_string(),
                "193.203.43.1".to_string(),
                "193.203.43.2".to_string(),
                "193.203.43.3".to_string(),
                "193.203.43.4".to_string(),
                "193.203.43.6".to_string(),
                "193.203.43.7".to_string(),
                "193.203.43.8".to_string(),
                "193.203.43.9".to_string(),
                "193.203.43.10".to_string(),
                "193.203.43.11".to_string(),
                "193.203.43.12".to_string(),
                "193.203.43.13".to_string(),
                "193.203.43.14".to_string(),
                "193.203.43.15".to_string(),
                "193.203.43.16".to_string(),
                "193.203.43.17".to_string(),
                "193.203.43.18".to_string(),
                "193.203.43.19".to_string(),
                "193.203.43.21".to_string(),
                "193.203.43.22".to_string(),
                "193.203.43.23".to_string(),
                "193.203.43.24".to_string(),
                "193.203.43.25".to_string(),
                "193.203.43.26".to_string(),
                "193.203.43.27".to_string(),
                "193.203.43.28".to_string(),
                "193.203.43.29".to_string(),
                "193.203.43.30".to_string(),
                "193.203.43.31".to_string(),
                "193.203.43.32".to_string(),
                "193.203.43.33".to_string(),
                "193.203.43.34".to_string(),
                "193.203.43.35".to_string(),
                "193.203.43.36".to_string(),
                "193.203.43.37".to_string(),
                "193.203.43.38".to_string(),
                "193.203.43.39".to_string(),
                "193.203.43.40".to_string(),
                "193.203.43.41".to_string(),
                "193.203.43.42".to_string(),
                "193.203.43.43".to_string(),
                "193.203.43.44".to_string(),
                "193.203.43.45".to_string(),
                "193.203.43.46".to_string(),
                "193.203.43.47".to_string(),
                "193.203.43.48".to_string(),
            ],
            aliases: vec!["193_203_40_0_22".to_string()],
        },
        TurnSubnet {
            id: "185_180_200_0_22".to_string(),
            title: "185.180.200.0/22".to_string(),
            subnet_cidr: "185.180.200.0/22".to_string(),
            count: 2,
            ips: vec![
                "185.180.202.14".to_string(),
                "185.180.202.15".to_string(),
            ],
            aliases: vec![],
        },
    ]
});

/// Registry for looking up TURN subnets and their IPs.
pub struct TurnSubnetRegistry;

impl TurnSubnetRegistry {
    /// Returns all registered subnets.
    pub fn subnets() -> &'static [TurnSubnet] {
        &SUBNETS
    }

    /// Finds a subnet by its id or one of its aliases.
    pub fn get_subnet(id: Option<&str>) -> Option<&'static TurnSubnet> {
        let id = id?;
        SUBNETS
            .iter()
            .find(|subnet| subnet.id == id || subnet.aliases.iter().any(|alias| alias == id))
    }

    /// Returns the list of IP addresses belonging to the requested subnet id.
    pub fn get_ips_for_subnet(id: Option<&str>) -> Vec<String> {
        Self::get_subnet(id)
            .map(|subnet| subnet.ips.clone())
            .unwrap_or_default()
    }

    /// Returns a comma-separated list of IP addresses for the requested subnet id.
    pub fn get_joined_ips_for_subnet(id: Option<&str>) -> String {
        Self::get_ips_for_subnet(id).join(",")
    }
}

/// Returns all registered subnets.
pub fn subnets() -> &'static [TurnSubnet] {
    TurnSubnetRegistry::subnets()
}

/// Finds a subnet by its id or one of its aliases.
pub fn get_subnet(id: Option<&str>) -> Option<&'static TurnSubnet> {
    TurnSubnetRegistry::get_subnet(id)
}

/// Returns the list of IP addresses belonging to the requested subnet id.
pub fn get_ips_for_subnet(id: Option<&str>) -> Vec<String> {
    TurnSubnetRegistry::get_ips_for_subnet(id)
}

/// Returns a comma-separated list of IP addresses for the requested subnet id.
pub fn get_joined_ips_for_subnet(id: Option<&str>) -> String {
    TurnSubnetRegistry::get_joined_ips_for_subnet(id)
}

/// Finds a subnet by direct string slice.
pub fn get_subnet_by_id(id: &str) -> Option<&'static TurnSubnet> {
    TurnSubnetRegistry::get_subnet(Some(id))
}

/// Returns the list of IP addresses by direct string slice.
pub fn get_ips_for_subnet_id(id: &str) -> Vec<String> {
    TurnSubnetRegistry::get_ips_for_subnet(Some(id))
}

/// Returns a comma-separated list of IP addresses by direct string slice.
pub fn get_joined_ips_for_subnet_id(id: &str) -> String {
    TurnSubnetRegistry::get_joined_ips_for_subnet(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_subnet_by_primary_id() {
        let subnet = get_subnet(Some("95_163_34_0_24")).expect("subnet found");
        assert_eq!(subnet.id, "95_163_34_0_24");
        assert_eq!(subnet.title, "95.163.34.0/24");
        assert_eq!(subnet.subnet_cidr, "95.163.34.0/24");
        assert_eq!(subnet.count, 64);
        assert_eq!(subnet.ips.len(), 64);
        assert_eq!(subnet.ips[0], "95.163.34.48");
        assert_eq!(subnet.ips[63], "95.163.34.190");
    }

    #[test]
    fn get_subnet_by_alias() {
        let subnet = get_subnet(Some("91_231_132_0_22")).expect("subnet found via alias");
        assert_eq!(subnet.id, "91_231_135_0_24");
        assert_eq!(subnet.title, "91.231.135.0/24");
        assert_eq!(subnet.aliases, vec!["91_231_132_0_22".to_string()]);
    }

    #[test]
    fn get_subnet_unknown_or_none_returns_none() {
        assert!(get_subnet(Some("non_existent")).is_none());
        assert!(get_subnet(None).is_none());
    }

    #[test]
    fn get_ips_for_subnet_fallback() {
        assert_eq!(
            get_ips_for_subnet(Some("185_180_200_0_22")),
            vec![
                "185.180.202.14".to_string(),
                "185.180.202.15".to_string()
            ]
        );
        assert!(get_ips_for_subnet(Some("invalid")).is_empty());
        assert!(get_ips_for_subnet(None).is_empty());
    }

    #[test]
    fn get_joined_ips_for_subnet_format() {
        let joined = get_joined_ips_for_subnet(Some("185_180_200_0_22"));
        assert_eq!(joined, "185.180.202.14,185.180.202.15");
        assert_eq!(get_joined_ips_for_subnet(None), "");
        assert_eq!(get_joined_ips_for_subnet(Some("missing")), "");
    }

    #[test]
    fn all_subnets_registered_and_retrievable() {
        let all = subnets();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].id, "95_163_34_0_24");
        assert_eq!(all[1].id, "91_231_135_0_24");
        assert_eq!(all[2].id, "90_156_232_0_21");
        assert_eq!(all[3].id, "193_203_43_0_24");
        assert_eq!(all[4].id, "185_180_200_0_22");
    }
}
