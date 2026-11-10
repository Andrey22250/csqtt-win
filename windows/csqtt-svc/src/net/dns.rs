//! Tunnel DNS and DNS-leak protection.
use std::net::IpAddr;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceLuidToGuid, SetInterfaceDnsSettings, DNS_INTERFACE_SETTINGS,
    DNS_INTERFACE_SETTINGS_VERSION1, DNS_SETTING_IPV6, DNS_SETTING_NAMESERVER,
};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE,
    KEY_WRITE, REG_DWORD, REG_MULTI_SZ, REG_OPTION_NON_VOLATILE, REG_SZ,
};

/// Format DNS servers joined by ';' for NRPT GenericDNSServers.
pub fn format_nrpt_servers(servers: &[IpAddr]) -> String {
    servers
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(";")
}

/// Encode a slice of string slices as a UTF-16 REG_MULTI_SZ buffer (null-separated, double null terminated).
pub fn encode_multi_sz(strings: &[&str]) -> Vec<u16> {
    let mut out = Vec::new();
    for s in strings {
        out.extend(s.encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

/// Convert a string slice to a null-terminated UTF-16 wide vector.
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Set the DNS servers of the tunnel interface (SetInterfaceDnsSettings via its GUID).
pub fn set_interface_dns(luid: u64, servers: &[IpAddr]) -> anyhow::Result<()> {
    let luid_raw = NET_LUID_LH { Value: luid };
    let mut guid: windows_sys::core::GUID = unsafe { std::mem::zeroed() };
    // SAFETY: ConvertInterfaceLuidToGuid writes the interface GUID for the given LUID.
    let ret = unsafe { ConvertInterfaceLuidToGuid(&luid_raw, &mut guid) };
    if ret != ERROR_SUCCESS {
        return Err(anyhow::anyhow!(
            "ConvertInterfaceLuidToGuid failed for LUID {luid:#x} with error code {ret}"
        ));
    }

    let v4_servers: Vec<String> = servers
        .iter()
        .filter_map(|s| match s {
            IpAddr::V4(v4) => Some(v4.to_string()),
            _ => None,
        })
        .collect();

    let v6_servers: Vec<String> = servers
        .iter()
        .filter_map(|s| match s {
            IpAddr::V6(v6) => Some(v6.to_string()),
            _ => None,
        })
        .collect();

    // Call for IPv4 nameservers (or empty string to clear settings when servers slice is empty).
    if !v4_servers.is_empty() || servers.is_empty() {
        let v4_csv = v4_servers.join(",");
        let mut v4_wide = to_wide_null(&v4_csv);
        let mut settings: DNS_INTERFACE_SETTINGS = unsafe { std::mem::zeroed() };
        settings.Version = DNS_INTERFACE_SETTINGS_VERSION1;
        settings.Flags = DNS_SETTING_NAMESERVER as u64;
        settings.NameServer = v4_wide.as_mut_ptr();

        // SAFETY: SetInterfaceDnsSettings applies DNS settings to the interface GUID.
        let ret = unsafe { SetInterfaceDnsSettings(guid, &settings) };
        if ret != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "SetInterfaceDnsSettings (IPv4) failed with error code {ret}"
            ));
        }
    }

    // Call for IPv6 nameservers (second call with DNS_SETTING_IPV6 flag).
    if !v6_servers.is_empty() {
        let v6_csv = v6_servers.join(",");
        let mut v6_wide = to_wide_null(&v6_csv);
        let mut settings: DNS_INTERFACE_SETTINGS = unsafe { std::mem::zeroed() };
        settings.Version = DNS_INTERFACE_SETTINGS_VERSION1;
        settings.Flags = (DNS_SETTING_NAMESERVER | DNS_SETTING_IPV6) as u64;
        settings.NameServer = v6_wide.as_mut_ptr();

        // SAFETY: SetInterfaceDnsSettings applies IPv6 DNS settings to the interface GUID.
        let ret = unsafe { SetInterfaceDnsSettings(guid, &settings) };
        if ret != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "SetInterfaceDnsSettings (IPv6) failed with error code {ret}"
            ));
        }
    }

    Ok(())
}

/// RAII guard to ensure an open registry HKEY is closed upon drop.
struct RegKeyGuard(HKEY);

impl Drop for RegKeyGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: Closing open registry key handle.
            unsafe { RegCloseKey(self.0) };
        }
    }
}

/// Helper to set a registry value with RegSetValueExW.
fn reg_set_value(hkey: HKEY, name: &str, dwtype: u32, data: &[u8]) -> anyhow::Result<()> {
    let name_wide = to_wide_null(name);
    // SAFETY: RegSetValueExW sets the named value under open hkey.
    let ret = unsafe {
        RegSetValueExW(
            hkey,
            name_wide.as_ptr(),
            0,
            dwtype,
            data.as_ptr(),
            data.len() as u32,
        )
    };
    if ret != ERROR_SUCCESS {
        return Err(anyhow::anyhow!(
            "RegSetValueExW failed for value '{name}' with error code {ret}"
        ));
    }
    Ok(())
}

const NRPT_KEY: &str =
    r"SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig\{5c2f7a36-9d41-4e0b-b1a8-63c7d2e4f901}";

/// NRPT rule for namespace "." -> `servers`, so every query goes to the tunnel DNS
/// (stops leaks via smart multi-homed resolution). Registry key under
/// HKLM\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig\csqtt-win .
///
/// Note: Windows Dnscache service monitors DnsPolicyConfig and applies NRPT policy
/// changes dynamically without restarting the Dnscache service.
pub fn nrpt_install(servers: &[IpAddr]) -> anyhow::Result<()> {
    let subkey_wide = to_wide_null(NRPT_KEY);

    let mut hkey: HKEY = std::ptr::null_mut();
    // SAFETY: RegCreateKeyExW opens or creates the DnsPolicyConfig\csqtt-win subkey.
    let ret = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey_wide.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut hkey,
            std::ptr::null_mut(),
        )
    };
    if ret != ERROR_SUCCESS {
        return Err(anyhow::anyhow!(
            "RegCreateKeyExW failed for '{NRPT_KEY}' with error code {ret}"
        ));
    }
    let guard = RegKeyGuard(hkey);

    // 1. Name: REG_MULTI_SZ "."
    let name_multi_sz = encode_multi_sz(&["."]);
    // SAFETY: Slicing &[u16] as &[u8] for byte-level registry API.
    let name_bytes = unsafe {
        std::slice::from_raw_parts(
            name_multi_sz.as_ptr() as *const u8,
            name_multi_sz.len() * std::mem::size_of::<u16>(),
        )
    };
    reg_set_value(guard.0, "Name", REG_MULTI_SZ, name_bytes)?;

    // 2. GenericDNSServers: REG_SZ joined by ';'
    let dns_str = format_nrpt_servers(servers);
    let dns_wide = to_wide_null(&dns_str);
    // SAFETY: Slicing &[u16] as &[u8] for byte-level registry API.
    let dns_bytes = unsafe {
        std::slice::from_raw_parts(
            dns_wide.as_ptr() as *const u8,
            dns_wide.len() * std::mem::size_of::<u16>(),
        )
    };
    reg_set_value(guard.0, "GenericDNSServers", REG_SZ, dns_bytes)?;

    // 3. ConfigOptions: REG_DWORD 8
    let config_options: u32 = 8;
    let config_bytes = config_options.to_ne_bytes();
    reg_set_value(guard.0, "ConfigOptions", REG_DWORD, &config_bytes)?;

    // 4. Version: REG_DWORD 2
    let version: u32 = 2;
    let version_bytes = version.to_ne_bytes();
    reg_set_value(guard.0, "Version", REG_DWORD, &version_bytes)?;

    // 5. IPSECCARestriction: REG_SZ ""
    let empty_wide = to_wide_null("");
    // SAFETY: Slicing &[u16] as &[u8] for byte-level registry API.
    let empty_bytes = unsafe {
        std::slice::from_raw_parts(
            empty_wide.as_ptr() as *const u8,
            empty_wide.len() * std::mem::size_of::<u16>(),
        )
    };
    reg_set_value(guard.0, "IPSECCARestriction", REG_SZ, empty_bytes)?;

    Ok(())
}

/// Remove the csqtt-win NRPT rule. Ok even if absent.
pub fn nrpt_remove() -> anyhow::Result<()> {
    let subkey_wide = to_wide_null(NRPT_KEY);

    // SAFETY: RegDeleteTreeW deletes the subkey and its values recursively.
    let ret = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, subkey_wide.as_ptr()) };
    if ret == ERROR_SUCCESS || ret == ERROR_FILE_NOT_FOUND || ret == ERROR_NOT_FOUND {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "RegDeleteTreeW failed for '{NRPT_KEY}' with error code {ret}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_format_nrpt_servers() {
        let servers: Vec<IpAddr> = vec![
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
        ];
        let formatted = format_nrpt_servers(&servers);
        assert_eq!(formatted, "1.1.1.1;8.8.8.8;2001:4860:4860::8888");

        let empty: Vec<IpAddr> = vec![];
        assert_eq!(format_nrpt_servers(&empty), "");
    }

    #[test]
    fn test_encode_multi_sz() {
        let encoded = encode_multi_sz(&["."]);
        assert_eq!(encoded, vec![b'.' as u16, 0, 0]);

        let multi = encode_multi_sz(&["foo", "bar"]);
        assert_eq!(
            multi,
            vec![
                b'f' as u16, b'o' as u16, b'o' as u16, 0,
                b'b' as u16, b'a' as u16, b'r' as u16, 0,
                0
            ]
        );
    }
}
