//! IP address, MTU and metric on an interface (IP Helper API).
use std::net::Ipv4Addr;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    CreateUnicastIpAddressEntry, DeleteUnicastIpAddressEntry, FreeMibTable, GetIpInterfaceEntry,
    GetUnicastIpAddressTable, InitializeIpInterfaceEntry, InitializeUnicastIpAddressEntry,
    SetIpInterfaceEntry, MIB_IPINTERFACE_ROW, MIB_UNICASTIPADDRESS_ROW,
    MIB_UNICASTIPADDRESS_TABLE,
};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, IpDadStatePreferred};

/// Validate IPv4 prefix length (0..=32).
pub fn validate_ipv4_prefix(prefix: u8) -> anyhow::Result<()> {
    if prefix > 32 {
        anyhow::bail!("invalid IPv4 prefix length /{prefix}, must be <= 32");
    }
    Ok(())
}

/// Replace all unicast IPv4 addresses on `luid` with `ip/prefix`.
pub fn set_ipv4(luid: u64, ip: Ipv4Addr, prefix: u8) -> anyhow::Result<()> {
    validate_ipv4_prefix(prefix)?;

    // Step 1: Query all unicast IPv4 addresses and delete entries matching this LUID.
    let mut table_ptr: *mut MIB_UNICASTIPADDRESS_TABLE = std::ptr::null_mut();
    // SAFETY: GetUnicastIpAddressTable allocates and populates the table for AF_INET.
    let ret = unsafe { GetUnicastIpAddressTable(AF_INET, &mut table_ptr) };
    if ret != ERROR_SUCCESS {
        return Err(anyhow::anyhow!(
            "GetUnicastIpAddressTable failed with error code {ret}"
        ));
    }

    if !table_ptr.is_null() {
        // SAFETY: table_ptr points to a valid MIB_UNICASTIPADDRESS_TABLE.
        let num_entries = unsafe { (*table_ptr).NumEntries as usize };
        if num_entries > 0 {
            // SAFETY: Table contains at least num_entries elements of MIB_UNICASTIPADDRESS_ROW.
            let entries =
                unsafe { std::slice::from_raw_parts((*table_ptr).Table.as_ptr(), num_entries) };
            for entry in entries {
                // SAFETY: Reading the 64-bit integer representation of NET_LUID_LH union.
                if unsafe { entry.InterfaceLuid.Value } == luid {
                    // SAFETY: entry is a valid row from the table to delete.
                    let del_ret = unsafe { DeleteUnicastIpAddressEntry(entry) };
                    if del_ret != ERROR_SUCCESS && del_ret != ERROR_NOT_FOUND {
                        // SAFETY: Free table before returning on error.
                        unsafe { FreeMibTable(table_ptr as *const std::ffi::c_void) };
                        return Err(anyhow::anyhow!(
                            "DeleteUnicastIpAddressEntry failed with error code {del_ret}"
                        ));
                    }
                }
            }
        }
        // SAFETY: Free the table allocated by GetUnicastIpAddressTable.
        unsafe { FreeMibTable(table_ptr as *const std::ffi::c_void) };
    }

    // Step 2: Initialize a new unicast address row and populate its fields.
    let mut new_row: MIB_UNICASTIPADDRESS_ROW = unsafe { std::mem::zeroed() };
    // SAFETY: InitializeUnicastIpAddressEntry sets up default values in new_row.
    unsafe { InitializeUnicastIpAddressEntry(&mut new_row) };

    new_row.InterfaceLuid = NET_LUID_LH { Value: luid };
    new_row.Address.si_family = AF_INET;
    new_row.Address.Ipv4.sin_family = AF_INET;
    new_row.Address.Ipv4.sin_addr.S_un.S_addr = u32::from_ne_bytes(ip.octets());
    new_row.OnLinkPrefixLength = prefix;
    new_row.DadState = IpDadStatePreferred;

    // SAFETY: CreateUnicastIpAddressEntry creates the address entry for the specified LUID.
    let create_ret = unsafe { CreateUnicastIpAddressEntry(&new_row) };
    if create_ret != ERROR_SUCCESS {
        return Err(anyhow::anyhow!(
            "CreateUnicastIpAddressEntry failed for {ip}/{prefix} on LUID {luid:#x} with error code {create_ret}"
        ));
    }

    Ok(())
}

/// Helper to query and update IP interface properties for AF_INET and AF_INET6.
fn update_ip_interface<F>(luid: u64, mut mutate: F) -> anyhow::Result<()>
where
    F: FnMut(&mut MIB_IPINTERFACE_ROW),
{
    let families = [AF_INET, AF_INET6];

    for family in families {
        let mut row: MIB_IPINTERFACE_ROW = unsafe { std::mem::zeroed() };
        // SAFETY: InitializeIpInterfaceEntry sets default field values.
        unsafe { InitializeIpInterfaceEntry(&mut row) };
        row.InterfaceLuid = NET_LUID_LH { Value: luid };
        row.Family = family;

        // SAFETY: GetIpInterfaceEntry retrieves interface configuration for the given LUID and family.
        let ret = unsafe { GetIpInterfaceEntry(&mut row) };
        if ret != ERROR_SUCCESS {
            // IPv6 might be disabled on the interface; ignore not found errors for AF_INET6.
            if family == AF_INET6 && (ret == ERROR_NOT_FOUND || ret == ERROR_FILE_NOT_FOUND) {
                continue;
            }
            return Err(anyhow::anyhow!(
                "GetIpInterfaceEntry failed for family {family} on LUID {luid:#x} with error code {ret}"
            ));
        }

        mutate(&mut row);

        // Win32 requirement: SitePrefixLength must be 0 for AF_INET before SetIpInterfaceEntry.
        if family == AF_INET {
            row.SitePrefixLength = 0;
        }

        // SAFETY: SetIpInterfaceEntry applies the modified configuration.
        let ret = unsafe { SetIpInterfaceEntry(&mut row) };
        if ret != ERROR_SUCCESS {
            if family == AF_INET6 && (ret == ERROR_NOT_FOUND || ret == ERROR_FILE_NOT_FOUND) {
                continue;
            }
            return Err(anyhow::anyhow!(
                "SetIpInterfaceEntry failed for family {family} on LUID {luid:#x} with error code {ret}"
            ));
        }
    }
    Ok(())
}

/// Set NlMtu for IPv4 and IPv6 on `luid`.
pub fn set_mtu(luid: u64, mtu: u32) -> anyhow::Result<()> {
    update_ip_interface(luid, |row| {
        row.NlMtu = mtu;
    })
}

/// Disable automatic metric and set the interface metric (IPv4 and IPv6).
pub fn set_metric(luid: u64, metric: u32) -> anyhow::Result<()> {
    update_ip_interface(luid, |row| {
        row.UseAutomaticMetric = false;
        row.Metric = metric;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_validation() {
        assert!(validate_ipv4_prefix(0).is_ok());
        assert!(validate_ipv4_prefix(24).is_ok());
        assert!(validate_ipv4_prefix(32).is_ok());
        assert!(validate_ipv4_prefix(33).is_err());
    }

    #[test]
    fn test_ipv4_octets_conversion() {
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let s_addr = u32::from_ne_bytes(ip.octets());
        let back = Ipv4Addr::from(s_addr.to_ne_bytes());
        assert_eq!(ip, back);
    }
}
