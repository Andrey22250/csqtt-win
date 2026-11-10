//! Port of dnsProfileName in TunnelManager.kt.

/// Returns the display name for known DNS server configurations, or "Пользовательский DNS" if unknown.
pub fn dns_profile_name(dns: &str) -> &'static str {
    match dns.trim() {
        "77.88.8.8,77.88.8.1" => "Yandex DNS",
        "1.1.1.1,1.0.0.1" => "Cloudflare DNS",
        "8.8.8.8,8.8.4.4" => "Google DNS",
        "94.140.14.14,94.140.15.14" => "AdGuard DNS",
        "111.88.96.54,111.88.96.55" | "111.88.96.50,111.88.96.51" => "Xbox DNS",
        "83.220.169.155,212.109.195.93" => "Comms DNS",
        "193.233.112.67,193.233.112.88" | "95.182.120.241,37.230.192.51" => "Geohide DNS",
        "9.9.9.9,149.112.112.112" => "Quad9 DNS",
        "208.67.222.222,208.67.220.220" => "OpenDNS",
        "95.189.32.74,195.208.5.1" => "Rostelecom DNS",
        "45.90.28.181,45.90.30.181" => "NextDNS",
        "195.208.6.1,195.208.7.1" => "BI.ZONE DNS",
        "195.208.4.1,195.208.5.1" => "НСДИ DNS",
        _ => "Пользовательский DNS",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_server_dns_profiles_have_stable_display_names() {
        assert_eq!("Yandex DNS", dns_profile_name("77.88.8.8,77.88.8.1"));
        assert_eq!("Xbox DNS", dns_profile_name("111.88.96.54,111.88.96.55"));
        assert_eq!("Xbox DNS", dns_profile_name("111.88.96.50,111.88.96.51"));
        assert_eq!("Geohide DNS", dns_profile_name("193.233.112.67,193.233.112.88"));
        assert_eq!("Geohide DNS", dns_profile_name("95.182.120.241,37.230.192.51"));
        assert_eq!("BI.ZONE DNS", dns_profile_name("195.208.6.1,195.208.7.1"));
        assert_eq!("НСДИ DNS", dns_profile_name("195.208.4.1,195.208.5.1"));
    }

    #[test]
    fn unknown_dns_pair_remains_explicitly_custom() {
        assert_eq!("Пользовательский DNS", dns_profile_name("203.0.113.1,203.0.113.2"));
    }
}
