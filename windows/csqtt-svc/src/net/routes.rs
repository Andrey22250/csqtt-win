//! Routes and physical interface discovery, with a journal for crash cleanup.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context;
use windows_sys::Win32::Foundation::{
    ERROR_NOT_FOUND, ERROR_OBJECT_ALREADY_EXISTS, HANDLE, NO_ERROR,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    CancelMibChangeNotify2, CreateIpForwardEntry2, DeleteIpForwardEntry2, FreeMibTable,
    GetBestRoute2, GetIpForwardTable2, GetIpInterfaceEntry, InitializeIpForwardEntry,
    InitializeIpInterfaceEntry, NotifyIpInterfaceChange, NotifyRouteChange2,
    MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE,
};
use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, IN6_ADDR, IN6_ADDR_0, IN_ADDR, IN_ADDR_0, IN_ADDR_0_0,
    MIB_IPPROTO_NETMGMT, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_IN6_0, SOCKADDR_INET,
};

static JOURNAL_TMP_ID: AtomicUsize = AtomicUsize::new(0);

/// RAII guard to free MIB tables allocated by IP Helper API functions.
struct MibTableGuard(*mut MIB_IPFORWARD_TABLE2);

impl Drop for MibTableGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: The table was allocated by GetIpForwardTable2 and must be freed with FreeMibTable.
            unsafe { FreeMibTable(self.0 as *const core::ffi::c_void) };
        }
    }
}

/// Convert an [`IpAddr`] to a Windows [`SOCKADDR_INET`] union.
pub fn ip_to_sockaddr_inet(addr: IpAddr) -> SOCKADDR_INET {
    match addr {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            SOCKADDR_INET {
                Ipv4: SOCKADDR_IN {
                    sin_family: AF_INET,
                    sin_port: 0,
                    sin_addr: IN_ADDR {
                        S_un: IN_ADDR_0 {
                            S_un_b: IN_ADDR_0_0 {
                                s_b1: octets[0],
                                s_b2: octets[1],
                                s_b3: octets[2],
                                s_b4: octets[3],
                            },
                        },
                    },
                    sin_zero: [0; 8],
                },
            }
        }
        IpAddr::V6(v6) => SOCKADDR_INET {
            Ipv6: SOCKADDR_IN6 {
                sin6_family: AF_INET6,
                sin6_port: 0,
                sin6_flowinfo: 0,
                sin6_addr: IN6_ADDR {
                    u: IN6_ADDR_0 {
                        Byte: v6.octets(),
                    },
                },
                Anonymous: SOCKADDR_IN6_0 { sin6_scope_id: 0 },
            },
        },
    }
}

/// Convert a Windows [`SOCKADDR_INET`] union to [`IpAddr`].
/// Returns `None` if the address family is unsupported.
pub fn sockaddr_inet_to_ip(sock: &SOCKADDR_INET) -> Option<IpAddr> {
    // SAFETY: Reading active union variant selected by `si_family`.
    unsafe {
        match sock.si_family {
            AF_INET => {
                let bytes = [
                    sock.Ipv4.sin_addr.S_un.S_un_b.s_b1,
                    sock.Ipv4.sin_addr.S_un.S_un_b.s_b2,
                    sock.Ipv4.sin_addr.S_un.S_un_b.s_b3,
                    sock.Ipv4.sin_addr.S_un.S_un_b.s_b4,
                ];
                Some(IpAddr::V4(Ipv4Addr::from(bytes)))
            }
            AF_INET6 => {
                let bytes = sock.Ipv6.sin6_addr.u.Byte;
                Some(IpAddr::V6(Ipv6Addr::from(bytes)))
            }
            _ => None,
        }
    }
}

/// Convert a [`RouteSpec`] into an initialized [`MIB_IPFORWARD_ROW2`] structure.
fn route_spec_to_forward_row(route: &RouteSpec) -> MIB_IPFORWARD_ROW2 {
    let mut row: MIB_IPFORWARD_ROW2 = unsafe { core::mem::zeroed() };
    // SAFETY: InitializeIpForwardEntry sets default fields for a forward entry.
    unsafe { InitializeIpForwardEntry(&mut row) };

    row.DestinationPrefix.Prefix = ip_to_sockaddr_inet(route.dest);
    row.DestinationPrefix.PrefixLength = route.prefix;

    row.InterfaceLuid = NET_LUID_LH { Value: route.luid };
    row.InterfaceIndex = route.if_index;

    row.NextHop = match route.next_hop {
        Some(hop) => ip_to_sockaddr_inet(hop),
        None => {
            if route.dest.is_ipv4() {
                ip_to_sockaddr_inet(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            } else {
                ip_to_sockaddr_inet(IpAddr::V6(Ipv6Addr::UNSPECIFIED))
            }
        }
    };

    row.Metric = route.metric;
    row.Protocol = MIB_IPPROTO_NETMGMT;

    row
}

/// Atomically write a list of [`RouteSpec`]s to disk as pretty JSON (write tmp + rename).
pub fn write_journal(path: &Path, routes: &[RouteSpec]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
    }
    let json = serde_json::to_string_pretty(routes)
        .context("failed to serialize route journal to JSON")?;

    let count = JOURNAL_TMP_ID.fetch_add(1, Ordering::Relaxed);
    let tmp_path = match path.file_name() {
        Some(name) => {
            let mut tmp_name = name.to_os_string();
            tmp_name.push(format!(".tmp.{}.{}", std::process::id(), count));
            path.with_file_name(tmp_name)
        }
        None => path.with_extension(format!("tmp.{}.{}", std::process::id(), count)),
    };

    if let Err(e) = std::fs::write(&tmp_path, json.as_bytes()) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e).with_context(|| format!("failed to write temporary journal {}", tmp_path.display()));
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e).with_context(|| format!("failed to rename {} to {}", tmp_path.display(), path.display()));
    }

    Ok(())
}

/// Read a list of [`RouteSpec`]s from a JSON journal file.
pub fn read_journal(path: &Path) -> anyhow::Result<Vec<RouteSpec>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read journal {}", path.display()))?;
    let routes: Vec<RouteSpec> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse journal JSON from {}", path.display()))?;
    Ok(routes)
}

/// The interface Windows would use to reach a destination without the tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRoute {
    pub if_index: u32,
    pub luid: u64,
    pub gateway: Option<IpAddr>,
}

/// Best route to `dest` (GetBestRoute2), ignoring the interface `exclude_luid` (the tunnel).
pub fn best_physical_route(dest: IpAddr, exclude_luid: Option<u64>) -> anyhow::Result<PhysicalRoute> {
    let dest_sock = ip_to_sockaddr_inet(dest);
    let mut best_row: MIB_IPFORWARD_ROW2 = unsafe { core::mem::zeroed() };
    let mut best_source: SOCKADDR_INET = unsafe { core::mem::zeroed() };

    // SAFETY: GetBestRoute2 queries the IP routing table for the best route to destination.
    let err = unsafe {
        GetBestRoute2(
            core::ptr::null(),
            0,
            core::ptr::null(),
            &dest_sock,
            0,
            &mut best_row,
            &mut best_source,
        )
    };
    if err != NO_ERROR {
        anyhow::bail!("GetBestRoute2 failed with error code {}", err);
    }

    // SAFETY: Reading 64-bit value from the NET_LUID_LH union.
    let chosen_luid = unsafe { best_row.InterfaceLuid.Value };
    if exclude_luid == Some(chosen_luid) {
        let family = match dest {
            IpAddr::V4(_) => AF_INET,
            IpAddr::V6(_) => AF_INET6,
        };

        let mut table: *mut MIB_IPFORWARD_TABLE2 = core::ptr::null_mut();
        // SAFETY: GetIpForwardTable2 allocates a table of forward rows for the specified address family.
        let table_err = unsafe { GetIpForwardTable2(family, &mut table) };
        if table_err != NO_ERROR {
            anyhow::bail!("GetIpForwardTable2 failed with error code {}", table_err);
        }
        let _guard = MibTableGuard(table);

        let rows = if table.is_null() {
            &[]
        } else {
            // SAFETY: Dereferencing table to read NumEntries and obtain Table slice.
            let num_entries = unsafe { (*table).NumEntries } as usize;
            if num_entries == 0 {
                &[]
            } else {
                // SAFETY: (*table).Table contains num_entries valid MIB_IPFORWARD_ROW2 elements.
                unsafe { core::slice::from_raw_parts((*table).Table.as_ptr(), num_entries) }
            }
        };

        let mut best_candidate: Option<(&MIB_IPFORWARD_ROW2, u64)> = None;

        for row in rows {
            // Only scan default routes (prefix length 0).
            if row.DestinationPrefix.PrefixLength != 0 {
                continue;
            }

            // SAFETY: Reading 64-bit value from the NET_LUID_LH union.
            let row_luid = unsafe { row.InterfaceLuid.Value };
            if Some(row_luid) == exclude_luid {
                continue;
            }

            let mut if_row: MIB_IPINTERFACE_ROW = unsafe { core::mem::zeroed() };
            // SAFETY: InitializeIpInterfaceEntry initializes default fields.
            unsafe { InitializeIpInterfaceEntry(&mut if_row) };
            if_row.Family = family;
            if_row.InterfaceLuid = row.InterfaceLuid;
            if_row.InterfaceIndex = row.InterfaceIndex;

            // SAFETY: GetIpInterfaceEntry queries interface properties including interface metric.
            let if_err = unsafe { GetIpInterfaceEntry(&mut if_row) };
            let if_metric = if if_err == NO_ERROR { if_row.Metric } else { 0 };
            let total_metric = (row.Metric as u64) + (if_metric as u64);

            match best_candidate {
                None => best_candidate = Some((row, total_metric)),
                Some((_, current_min)) if total_metric < current_min => {
                    best_candidate = Some((row, total_metric));
                }
                _ => {}
            }
        }

        let (chosen_row, _) = best_candidate.ok_or_else(|| {
            anyhow::anyhow!(
                "no default route found on physical interfaces excluding LUID {:?}",
                exclude_luid
            )
        })?;

        let if_index = chosen_row.InterfaceIndex;
        // SAFETY: Reading 64-bit value from the NET_LUID_LH union.
        let luid = unsafe { chosen_row.InterfaceLuid.Value };
        let gateway = sockaddr_inet_to_ip(&chosen_row.NextHop).filter(|ip| !ip.is_unspecified());

        Ok(PhysicalRoute {
            if_index,
            luid,
            gateway,
        })
    } else {
        let if_index = best_row.InterfaceIndex;
        let luid = chosen_luid;
        let gateway = sockaddr_inet_to_ip(&best_row.NextHop).filter(|ip| !ip.is_unspecified());

        Ok(PhysicalRoute {
            if_index,
            luid,
            gateway,
        })
    }
}

/// Source address Windows would use to reach `dest` through interface `luid`
/// (GetBestRoute2 constrained to that interface). Used for the core's CSQTT_BYPASS_LOCAL_IP.
pub fn source_address(luid: u64, dest: IpAddr) -> anyhow::Result<IpAddr> {
    let dest_sock = ip_to_sockaddr_inet(dest);
    let luid_raw = NET_LUID_LH { Value: luid };
    let mut best_row: MIB_IPFORWARD_ROW2 = unsafe { core::mem::zeroed() };
    let mut best_source: SOCKADDR_INET = unsafe { core::mem::zeroed() };
    // SAFETY: all pointers reference live locals for the duration of the call.
    let err = unsafe {
        GetBestRoute2(&luid_raw, 0, core::ptr::null(), &dest_sock, 0, &mut best_row, &mut best_source)
    };
    if err != NO_ERROR {
        anyhow::bail!("GetBestRoute2(luid {luid:#x}) failed with error code {err}");
    }
    sockaddr_inet_to_ip(&best_source)
        .filter(|ip| !ip.is_unspecified())
        .ok_or_else(|| anyhow::anyhow!("no source address on LUID {luid:#x} for {dest}"))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RouteSpec {
    pub dest: IpAddr,
    pub prefix: u8,
    pub luid: u64,
    pub if_index: u32,
    /// None = on-link.
    pub next_hop: Option<IpAddr>,
    pub metric: u32,
}

/// 0.0.0.0/1, 128.0.0.0/1, ::/1, 8000::/1 on-link via the tunnel interface.
pub fn tunnel_split_routes(luid: u64, if_index: u32) -> Vec<RouteSpec> {
    vec![
        RouteSpec {
            dest: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            prefix: 1,
            luid,
            if_index,
            next_hop: None,
            metric: 0,
        },
        RouteSpec {
            dest: IpAddr::V4(Ipv4Addr::new(128, 0, 0, 0)),
            prefix: 1,
            luid,
            if_index,
            next_hop: None,
            metric: 0,
        },
        RouteSpec {
            dest: IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
            prefix: 1,
            luid,
            if_index,
            next_hop: None,
            metric: 0,
        },
        RouteSpec {
            dest: IpAddr::V6(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0)),
            prefix: 1,
            luid,
            if_index,
            next_hop: None,
            metric: 0,
        },
    ]
}

/// Routes added by csqtt-win. Every add is journaled to disk (JSON) before returning,
/// so a crashed service can remove them on next start.
pub struct RouteTable {
    journal: PathBuf,
    added: Vec<RouteSpec>,
}

impl RouteTable {
    pub fn new(journal: &Path) -> Self {
        Self {
            journal: journal.to_path_buf(),
            added: Vec::new(),
        }
    }

    /// CreateIpForwardEntry2. ERROR_OBJECT_ALREADY_EXISTS counts as success (and is journaled).
    pub fn add(&mut self, route: RouteSpec) -> anyhow::Result<()> {
        let row = route_spec_to_forward_row(&route);
        // SAFETY: CreateIpForwardEntry2 adds a route to the system table.
        let err = unsafe { CreateIpForwardEntry2(&row) };
        if err != NO_ERROR && err != ERROR_OBJECT_ALREADY_EXISTS {
            anyhow::bail!("CreateIpForwardEntry2 failed with error code {}", err);
        }
        self.added.push(route);
        write_journal(&self.journal, &self.added)?;
        Ok(())
    }

    /// Remove one route previously added.
    pub fn remove(&mut self, route: &RouteSpec) -> anyhow::Result<()> {
        let row = route_spec_to_forward_row(route);
        // SAFETY: DeleteIpForwardEntry2 removes the route from the system table.
        let err = unsafe { DeleteIpForwardEntry2(&row) };
        if err != NO_ERROR && err != ERROR_NOT_FOUND {
            anyhow::bail!("DeleteIpForwardEntry2 failed with error code {}", err);
        }
        if let Some(pos) = self.added.iter().position(|r| r == route) {
            self.added.remove(pos);
        }
        write_journal(&self.journal, &self.added)?;
        Ok(())
    }

    /// Remove everything added (reverse order), ignore ERROR_NOT_FOUND, then delete the journal.
    pub fn remove_all(&mut self) -> anyhow::Result<()> {
        let mut first_err = None;
        for route in self.added.iter().rev() {
            let row = route_spec_to_forward_row(route);
            // SAFETY: DeleteIpForwardEntry2 removes the route from the system table.
            let err = unsafe { DeleteIpForwardEntry2(&row) };
            if err != NO_ERROR && err != ERROR_NOT_FOUND {
                if first_err.is_none() {
                    first_err = Some(anyhow::anyhow!("DeleteIpForwardEntry2 failed with error code {}", err));
                }
            }
        }
        self.added.clear();
        if self.journal.exists() {
            if let Err(e) = std::fs::remove_file(&self.journal) {
                if first_err.is_none() {
                    first_err = Some(e.into());
                }
            }
        }
        if let Some(err) = first_err {
            return Err(err);
        }
        Ok(())
    }

    pub fn added(&self) -> &[RouteSpec] {
        &self.added
    }

    /// Startup cleanup: remove routes listed in a leftover journal, delete it. Returns how many were removed.
    pub fn cleanup_stale(journal: &Path) -> anyhow::Result<usize> {
        if !journal.exists() {
            return Ok(0);
        }
        let routes = read_journal(journal)?;
        let mut removed = 0;
        let mut first_err = None;
        for route in routes.iter().rev() {
            let row = route_spec_to_forward_row(route);
            // SAFETY: DeleteIpForwardEntry2 removes the stale route entry.
            let err = unsafe { DeleteIpForwardEntry2(&row) };
            if err == NO_ERROR {
                removed += 1;
            } else if err != ERROR_NOT_FOUND {
                if first_err.is_none() {
                    first_err = Some(anyhow::anyhow!("DeleteIpForwardEntry2 failed with error code {}", err));
                }
            }
        }
        if let Err(e) = std::fs::remove_file(journal) {
            if first_err.is_none() {
                first_err = Some(e.into());
            }
        }
        if let Some(err) = first_err {
            return Err(err);
        }
        Ok(removed)
    }
}

/// Watches route/interface changes (NotifyRouteChange2 + NotifyIpInterfaceChange) and reports them
/// debounced (~1 s) on a tokio channel. Dropping it unregisters the callbacks.
pub struct NetworkWatcher {
    route_handle: HANDLE,
    if_handle: HANDLE,
    context: *mut core::ffi::c_void,
    debounce_task: Option<tokio::task::JoinHandle<()>>,
}

// SAFETY: Notification handles and context can be safely transferred across threads and dropped.
unsafe impl Send for NetworkWatcher {}
unsafe impl Sync for NetworkWatcher {}

unsafe extern "system" fn route_change_callback(
    callercontext: *const core::ffi::c_void,
    _row: *const MIB_IPFORWARD_ROW2,
    _notificationtype: MIB_NOTIFICATION_TYPE,
) {
    if !callercontext.is_null() {
        // SAFETY: callercontext is a valid pointer to UnboundedSender<()> while callbacks are active.
        let sender = unsafe { &*(callercontext as *const tokio::sync::mpsc::UnboundedSender<()>) };
        let _ = sender.send(());
    }
}

unsafe extern "system" fn interface_change_callback(
    callercontext: *const core::ffi::c_void,
    _row: *const MIB_IPINTERFACE_ROW,
    _notificationtype: MIB_NOTIFICATION_TYPE,
) {
    if !callercontext.is_null() {
        // SAFETY: callercontext is a valid pointer to UnboundedSender<()> while callbacks are active.
        let sender = unsafe { &*(callercontext as *const tokio::sync::mpsc::UnboundedSender<()>) };
        let _ = sender.send(());
    }
}

impl NetworkWatcher {
    /// Start watching route and interface changes.
    ///
    /// Change notifications are debounced (~1 s after changes settle) and forwarded to `tx`.
    /// Dropping the returned [`NetworkWatcher`] unregisters the callbacks and cancels the background task.
    ///
    /// # Panics / Errors
    /// This function must be called inside an active Tokio runtime because it calls `tokio::spawn`.
    pub fn start(tx: tokio::sync::mpsc::UnboundedSender<()>) -> anyhow::Result<Self> {
        let (internal_tx, mut internal_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

        let debounce_task = tokio::spawn(async move {
            let debounce_duration = std::time::Duration::from_secs(1);
            loop {
                // Wait for the first notification.
                if internal_rx.recv().await.is_none() {
                    break;
                }

                // Wait until changes settle (no new events for 1s).
                let sleep = tokio::time::sleep(debounce_duration);
                tokio::pin!(sleep);

                loop {
                    tokio::select! {
                        res = internal_rx.recv() => {
                            match res {
                                Some(()) => {
                                    // Reset settling timer on new event.
                                    sleep.as_mut().reset(tokio::time::Instant::now() + debounce_duration);
                                }
                                None => {
                                    // Internal channel closed.
                                    break;
                                }
                            }
                        }
                        _ = &mut sleep => {
                            // Changes settled for 1 second.
                            break;
                        }
                    }
                }

                // Forward debounced event to user channel.
                if tx.send(()).is_err() {
                    break;
                }
            }
        });

        let context = Box::into_raw(Box::new(internal_tx)) as *mut core::ffi::c_void;
        let mut route_handle: HANDLE = core::ptr::null_mut();
        let mut if_handle: HANDLE = core::ptr::null_mut();

        // SAFETY: Registering route change notification with IP Helper API.
        let err1 = unsafe {
            NotifyRouteChange2(
                AF_UNSPEC,
                Some(route_change_callback),
                context,
                false,
                &mut route_handle,
            )
        };
        if err1 != NO_ERROR {
            debounce_task.abort();
            // SAFETY: Reclaim leaked context on failure.
            unsafe {
                let _ = Box::from_raw(context as *mut tokio::sync::mpsc::UnboundedSender<()>);
            }
            anyhow::bail!("NotifyRouteChange2 failed with error code {}", err1);
        }

        // SAFETY: Registering interface change notification with IP Helper API.
        let err2 = unsafe {
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(interface_change_callback),
                context,
                false,
                &mut if_handle,
            )
        };
        if err2 != NO_ERROR {
            debounce_task.abort();
            // SAFETY: Cancel the first notification handle before freeing context.
            unsafe {
                CancelMibChangeNotify2(route_handle);
                let _ = Box::from_raw(context as *mut tokio::sync::mpsc::UnboundedSender<()>);
            }
            anyhow::bail!("NotifyIpInterfaceChange failed with error code {}", err2);
        }

        Ok(Self {
            route_handle,
            if_handle,
            context,
            debounce_task: Some(debounce_task),
        })
    }
}

impl Drop for NetworkWatcher {
    fn drop(&mut self) {
        if let Some(task) = self.debounce_task.take() {
            task.abort();
        }
        if !self.route_handle.is_null() {
            // SAFETY: CancelMibChangeNotify2 deregisters route change notification.
            unsafe { CancelMibChangeNotify2(self.route_handle) };
            self.route_handle = core::ptr::null_mut();
        }
        if !self.if_handle.is_null() {
            // SAFETY: CancelMibChangeNotify2 deregisters interface change notification.
            unsafe { CancelMibChangeNotify2(self.if_handle) };
            self.if_handle = core::ptr::null_mut();
        }
        if !self.context.is_null() {
            // SAFETY: After CancelMibChangeNotify2 has returned for both handles,
            // no callbacks can be invoked. It is safe to reclaim and drop the context.
            unsafe {
                let _ = Box::from_raw(self.context as *mut tokio::sync::mpsc::UnboundedSender<()>);
            }
            self.context = core::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_split_routes() {
        let luid = 0x1122_3344_5566_7788;
        let if_index = 42;
        let routes = tunnel_split_routes(luid, if_index);
        assert_eq!(routes.len(), 4);

        // IPv4 0.0.0.0/1 on-link metric 0
        assert_eq!(routes[0].dest, IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_eq!(routes[0].prefix, 1);
        assert_eq!(routes[0].luid, luid);
        assert_eq!(routes[0].if_index, if_index);
        assert_eq!(routes[0].next_hop, None);
        assert_eq!(routes[0].metric, 0);

        // IPv4 128.0.0.0/1 on-link metric 0
        assert_eq!(routes[1].dest, IpAddr::V4(Ipv4Addr::new(128, 0, 0, 0)));
        assert_eq!(routes[1].prefix, 1);
        assert_eq!(routes[1].luid, luid);
        assert_eq!(routes[1].if_index, if_index);
        assert_eq!(routes[1].next_hop, None);
        assert_eq!(routes[1].metric, 0);

        // IPv6 ::/1 on-link metric 0
        assert_eq!(routes[2].dest, IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)));
        assert_eq!(routes[2].prefix, 1);
        assert_eq!(routes[2].luid, luid);
        assert_eq!(routes[2].if_index, if_index);
        assert_eq!(routes[2].next_hop, None);
        assert_eq!(routes[2].metric, 0);

        // IPv6 8000::/1 on-link metric 0
        assert_eq!(routes[3].dest, IpAddr::V6(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0)));
        assert_eq!(routes[3].prefix, 1);
        assert_eq!(routes[3].luid, luid);
        assert_eq!(routes[3].if_index, if_index);
        assert_eq!(routes[3].next_hop, None);
        assert_eq!(routes[3].metric, 0);
    }

    #[test]
    fn test_sockaddr_inet_conversion_ipv4() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let sock = ip_to_sockaddr_inet(ip);

        // SAFETY: Reading active union variant for AF_INET.
        unsafe {
            assert_eq!(sock.si_family, AF_INET);
            assert_eq!(sock.Ipv4.sin_family, AF_INET);
            assert_eq!(sock.Ipv4.sin_addr.S_un.S_un_b.s_b1, 192);
            assert_eq!(sock.Ipv4.sin_addr.S_un.S_un_b.s_b2, 168);
            assert_eq!(sock.Ipv4.sin_addr.S_un.S_un_b.s_b3, 1);
            assert_eq!(sock.Ipv4.sin_addr.S_un.S_un_b.s_b4, 100);
        }

        let converted_back = sockaddr_inet_to_ip(&sock);
        assert_eq!(converted_back, Some(ip));
    }

    #[test]
    fn test_sockaddr_inet_conversion_ipv6() {
        let v6 = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1);
        let ip = IpAddr::V6(v6);
        let sock = ip_to_sockaddr_inet(ip);

        // SAFETY: Reading active union variant for AF_INET6.
        unsafe {
            assert_eq!(sock.si_family, AF_INET6);
            assert_eq!(sock.Ipv6.sin6_family, AF_INET6);
            assert_eq!(sock.Ipv6.sin6_addr.u.Byte, v6.octets());
        }

        let converted_back = sockaddr_inet_to_ip(&sock);
        assert_eq!(converted_back, Some(ip));
    }

    #[test]
    fn test_sockaddr_inet_unspecified_and_unknown() {
        let v4_unspec = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        let sock_v4 = ip_to_sockaddr_inet(v4_unspec);
        let back_v4 = sockaddr_inet_to_ip(&sock_v4).unwrap();
        assert!(back_v4.is_unspecified());

        let v6_unspec = IpAddr::V6(Ipv6Addr::UNSPECIFIED);
        let sock_v6 = ip_to_sockaddr_inet(v6_unspec);
        let back_v6 = sockaddr_inet_to_ip(&sock_v6).unwrap();
        assert!(back_v6.is_unspecified());

        // Unknown family returns None
        let mut unknown_sock: SOCKADDR_INET = unsafe { core::mem::zeroed() };
        unknown_sock.si_family = AF_UNSPEC;
        assert_eq!(sockaddr_inet_to_ip(&unknown_sock), None);
    }

    #[test]
    fn test_journal_round_trip() {
        let temp_dir = std::env::temp_dir();
        let journal_path = temp_dir.join(format!(
            "csqtt_routes_test_{}_{}.json",
            std::process::id(),
            JOURNAL_TMP_ID.fetch_add(1, Ordering::Relaxed)
        ));

        let routes = vec![
            RouteSpec {
                dest: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)),
                prefix: 8,
                luid: 1001,
                if_index: 2,
                next_hop: None,
                metric: 10,
            },
            RouteSpec {
                dest: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)),
                prefix: 24,
                luid: 1002,
                if_index: 3,
                next_hop: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
                metric: 20,
            },
            RouteSpec {
                dest: IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0)),
                prefix: 64,
                luid: 1003,
                if_index: 4,
                next_hop: Some(IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))),
                metric: 30,
            },
        ];

        // Write routes to journal
        write_journal(&journal_path, &routes).expect("write_journal failed");
        assert!(journal_path.exists());

        // Read routes back
        let loaded = read_journal(&journal_path).expect("read_journal failed");
        assert_eq!(loaded, routes);

        // Cleanup
        let _ = std::fs::remove_file(&journal_path);
        assert!(!journal_path.exists());
    }

    #[test]
    fn test_route_table_cleanup_stale_missing_file() {
        let temp_dir = std::env::temp_dir();
        let journal_path = temp_dir.join(format!(
            "nonexistent_csqtt_journal_{}_{}.json",
            std::process::id(),
            JOURNAL_TMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&journal_path);

        let removed = RouteTable::cleanup_stale(&journal_path).expect("cleanup_stale on missing file failed");
        assert_eq!(removed, 0);
    }
}
