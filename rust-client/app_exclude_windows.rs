use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

/// Maximum entries in flow cache before clearing.
const FLOW_CACHE_CAPACITY: usize = 4096;
/// TTL for flow verdict cache.
const FLOW_CACHE_TTL: Duration = Duration::from_secs(30);
/// Maximum entries in PID to image path cache before clearing.
const PID_CACHE_CAPACITY: usize = 4096;
/// TTL for PID to path cache.
const PID_CACHE_TTL: Duration = Duration::from_secs(60);

/// Extract local port from DWORD where the lower 16 bits are in network byte order.
pub fn port_from_dw(dw_port: u32) -> u16 {
    u16::from_be(dw_port as u16)
}

/// Check if a row IP equals the target IP or is unspecified (0.0.0.0 or ::).
pub fn matches_ip_addr(row_ip: IpAddr, src_ip: IpAddr) -> bool {
    row_ip.is_unspecified() || row_ip == src_ip
}

/// Parse '|'-separated string into trimmed non-empty application rules.
pub fn parse_excluded_apps(input: &str) -> Vec<String> {
    input
        .split('|')
        .map(|token| token.trim().trim_matches('"').trim())
        .filter(|token| !token.is_empty())
        .map(String::from)
        .collect()
}

/// Extract file name from a path slice.
pub fn extract_file_name(path: &str) -> &str {
    path.rsplit(|c| c == '\\' || c == '/').next().unwrap_or(path)
}

/// Normalize path slashes and strip optional `\\?\` prefix.
pub fn normalize_path(path: &str) -> String {
    let mut s = path.trim();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest;
    }
    s.replace('/', "\\")
}

/// Check if an image path matches an exclusion rule.
/// If rule contains a backslash or slash, it matches against the full path.
/// Otherwise, it matches against the file name.
pub fn matches_rule(image_path: &str, rule: &str) -> bool {
    let rule_trimmed = rule.trim().trim_matches('"').trim();
    if rule_trimmed.is_empty() {
        return false;
    }
    if rule_trimmed.contains('\\') || rule_trimmed.contains('/') {
        let norm_image = normalize_path(image_path);
        let norm_rule = normalize_path(rule_trimmed);
        norm_image.eq_ignore_ascii_case(&norm_rule)
    } else {
        let file_name = extract_file_name(image_path);
        file_name.eq_ignore_ascii_case(rule_trimmed)
    }
}

/// Per-app split tunnelling excluder for Windows.
pub struct AppExcluder {
    rules: Vec<String>,
    flow_cache: RwLock<HashMap<(u8, SocketAddr), (bool, Instant)>>,
    pid_cache: RwLock<HashMap<u32, (Option<String>, Instant)>>,
}

impl AppExcluder {
    pub fn new(rules: Vec<String>) -> Self {
        Self {
            rules,
            flow_cache: RwLock::new(HashMap::new()),
            pid_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Check if the connection originating from `src` with `protocol` (6 for TCP, 17 for UDP)
    /// belongs to an excluded application.
    pub fn is_excluded(&self, protocol: u8, src: SocketAddr) -> bool {
        if self.rules.is_empty() {
            return false;
        }
        if protocol != 6 && protocol != 17 {
            return false;
        }

        let now = Instant::now();

        // 1. Check flow cache (30s TTL, bounded 4096)
        let cached = self
            .flow_cache
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(protocol, src))
            .and_then(|&(verdict, ts)| {
                if now.duration_since(ts) < FLOW_CACHE_TTL {
                    Some(verdict)
                } else {
                    None
                }
            });

        if let Some(verdict) = cached {
            return verdict;
        }

        // 2. Query PID from extended TCP/UDP table
        let pid = self.find_pid(protocol, src);

        // 3. Query process path and check against rules
        let verdict = if let Some(pid) = pid {
            if let Some(path) = self.get_process_path(pid) {
                self.is_path_excluded(&path)
            } else {
                false
            }
        } else {
            false
        };

        // 4. Update flow cache
        let mut cache = self
            .flow_cache
            .write()
            .unwrap_or_else(|p| p.into_inner());
        if cache.len() >= FLOW_CACHE_CAPACITY {
            cache.clear();
        }
        cache.insert((protocol, src), (verdict, now));

        verdict
    }

    /// Check if an image path matches any configured rule.
    pub fn is_path_excluded(&self, path: &str) -> bool {
        for rule in &self.rules {
            if matches_rule(path, rule) {
                return true;
            }
        }
        false
    }

    fn find_pid(&self, protocol: u8, src: SocketAddr) -> Option<u32> {
        match (protocol, src) {
            (6, SocketAddr::V4(v4)) => find_tcp4_pid(v4),
            (6, SocketAddr::V6(v6)) => find_tcp6_pid(v6),
            (17, SocketAddr::V4(v4)) => find_udp4_pid(v4),
            (17, SocketAddr::V6(v6)) => find_udp6_pid(v6),
            _ => None,
        }
    }

    fn get_process_path(&self, pid: u32) -> Option<String> {
        let now = Instant::now();
        let cached = self
            .pid_cache
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(&pid)
            .and_then(|(path_opt, ts)| {
                if now.duration_since(*ts) < PID_CACHE_TTL {
                    Some(path_opt.clone())
                } else {
                    None
                }
            });

        if let Some(path_opt) = cached {
            return path_opt;
        }

        let path = query_process_image_path(pid);
        let mut cache = self
            .pid_cache
            .write()
            .unwrap_or_else(|p| p.into_inner());
        if cache.len() >= PID_CACHE_CAPACITY {
            cache.clear();
        }
        cache.insert(pid, (path.clone(), now));
        path
    }
}

/// Retrieve the static AppExcluder instance if CSQTT_EXCLUDED_APPS is set and non-empty.
pub fn excluder() -> Option<&'static AppExcluder> {
    static EXCLUDER: OnceLock<Option<AppExcluder>> = OnceLock::new();
    EXCLUDER
        .get_or_init(|| {
            let val = std::env::var("CSQTT_EXCLUDED_APPS").ok()?;
            if val.trim().is_empty() {
                return None;
            }
            let rules = parse_excluded_apps(&val);
            if rules.is_empty() {
                None
            } else {
                Some(AppExcluder::new(rules))
            }
        })
        .as_ref()
}

fn find_tcp4_pid(src: SocketAddrV4) -> Option<u32> {
    let mut size: u32 = 0;
    // SAFETY: GetExtendedTcpTable called with null buffer to query required size.
    let mut ret = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        )
    };
    for _ in 0..5 {
        if ret != ERROR_INSUFFICIENT_BUFFER && ret != ERROR_SUCCESS {
            return None;
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        // SAFETY: Buffer is allocated with `size` bytes.
        ret = unsafe {
            GetExtendedTcpTable(
                buf.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if ret == ERROR_SUCCESS {
            return parse_tcp4_table(&buf, src);
        }
    }
    None
}

pub fn parse_tcp4_table(buf: &[u8], src: SocketAddrV4) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    let num_entries = u32::from_ne_bytes(buf[0..4].try_into().ok()?) as usize;
    let entry_size = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
    let target_port = src.port();
    let target_ip = *src.ip();
    let mut fallback_pid = None;

    for i in 0..num_entries {
        let offset = 4 + i.checked_mul(entry_size)?;
        if offset + entry_size > buf.len() {
            break;
        }
        // SAFETY: Offset and entry_size are strictly within buf bounds. read_unaligned handles alignment.
        let row: MIB_TCPROW_OWNER_PID = unsafe {
            std::ptr::read_unaligned(buf.as_ptr().add(offset).cast())
        };
        if port_from_dw(row.dwLocalPort) == target_port {
            let row_ip = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
            if row_ip == target_ip {
                return Some(row.dwOwningPid);
            }
            if row_ip.is_unspecified() && fallback_pid.is_none() {
                fallback_pid = Some(row.dwOwningPid);
            }
        }
    }
    fallback_pid
}

fn find_tcp6_pid(src: SocketAddrV6) -> Option<u32> {
    let mut size: u32 = 0;
    // SAFETY: GetExtendedTcpTable called with null buffer to query required size.
    let mut ret = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET6 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        )
    };
    for _ in 0..5 {
        if ret != ERROR_INSUFFICIENT_BUFFER && ret != ERROR_SUCCESS {
            return None;
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        // SAFETY: Buffer is allocated with `size` bytes.
        ret = unsafe {
            GetExtendedTcpTable(
                buf.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET6 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if ret == ERROR_SUCCESS {
            return parse_tcp6_table(&buf, src);
        }
    }
    None
}

pub fn parse_tcp6_table(buf: &[u8], src: SocketAddrV6) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    let num_entries = u32::from_ne_bytes(buf[0..4].try_into().ok()?) as usize;
    let entry_size = std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>();
    let target_port = src.port();
    let target_ip = *src.ip();
    let mut fallback_pid = None;

    for i in 0..num_entries {
        let offset = 4 + i.checked_mul(entry_size)?;
        if offset + entry_size > buf.len() {
            break;
        }
        // SAFETY: Offset and entry_size are strictly within buf bounds. read_unaligned handles alignment.
        let row: MIB_TCP6ROW_OWNER_PID = unsafe {
            std::ptr::read_unaligned(buf.as_ptr().add(offset).cast())
        };
        if port_from_dw(row.dwLocalPort) == target_port {
            let row_ip = Ipv6Addr::from(row.ucLocalAddr);
            if row_ip == target_ip {
                return Some(row.dwOwningPid);
            }
            if row_ip.is_unspecified() && fallback_pid.is_none() {
                fallback_pid = Some(row.dwOwningPid);
            }
        }
    }
    fallback_pid
}

fn find_udp4_pid(src: SocketAddrV4) -> Option<u32> {
    let mut size: u32 = 0;
    // SAFETY: GetExtendedUdpTable called with null buffer to query required size.
    let mut ret = unsafe {
        GetExtendedUdpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET as u32,
            UDP_TABLE_OWNER_PID,
            0,
        )
    };
    for _ in 0..5 {
        if ret != ERROR_INSUFFICIENT_BUFFER && ret != ERROR_SUCCESS {
            return None;
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        // SAFETY: Buffer is allocated with `size` bytes.
        ret = unsafe {
            GetExtendedUdpTable(
                buf.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET as u32,
                UDP_TABLE_OWNER_PID,
                0,
            )
        };
        if ret == ERROR_SUCCESS {
            return parse_udp4_table(&buf, src);
        }
    }
    None
}

pub fn parse_udp4_table(buf: &[u8], src: SocketAddrV4) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    let num_entries = u32::from_ne_bytes(buf[0..4].try_into().ok()?) as usize;
    let entry_size = std::mem::size_of::<MIB_UDPROW_OWNER_PID>();
    let target_port = src.port();
    let target_ip = *src.ip();
    let mut fallback_pid = None;

    for i in 0..num_entries {
        let offset = 4 + i.checked_mul(entry_size)?;
        if offset + entry_size > buf.len() {
            break;
        }
        // SAFETY: Offset and entry_size are strictly within buf bounds. read_unaligned handles alignment.
        let row: MIB_UDPROW_OWNER_PID = unsafe {
            std::ptr::read_unaligned(buf.as_ptr().add(offset).cast())
        };
        if port_from_dw(row.dwLocalPort) == target_port {
            let row_ip = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
            if row_ip == target_ip {
                return Some(row.dwOwningPid);
            }
            if row_ip.is_unspecified() && fallback_pid.is_none() {
                fallback_pid = Some(row.dwOwningPid);
            }
        }
    }
    fallback_pid
}

fn find_udp6_pid(src: SocketAddrV6) -> Option<u32> {
    let mut size: u32 = 0;
    // SAFETY: GetExtendedUdpTable called with null buffer to query required size.
    let mut ret = unsafe {
        GetExtendedUdpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET6 as u32,
            UDP_TABLE_OWNER_PID,
            0,
        )
    };
    for _ in 0..5 {
        if ret != ERROR_INSUFFICIENT_BUFFER && ret != ERROR_SUCCESS {
            return None;
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        // SAFETY: Buffer is allocated with `size` bytes.
        ret = unsafe {
            GetExtendedUdpTable(
                buf.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET6 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            )
        };
        if ret == ERROR_SUCCESS {
            return parse_udp6_table(&buf, src);
        }
    }
    None
}

pub fn parse_udp6_table(buf: &[u8], src: SocketAddrV6) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    let num_entries = u32::from_ne_bytes(buf[0..4].try_into().ok()?) as usize;
    let entry_size = std::mem::size_of::<MIB_UDP6ROW_OWNER_PID>();
    let target_port = src.port();
    let target_ip = *src.ip();
    let mut fallback_pid = None;

    for i in 0..num_entries {
        let offset = 4 + i.checked_mul(entry_size)?;
        if offset + entry_size > buf.len() {
            break;
        }
        // SAFETY: Offset and entry_size are strictly within buf bounds. read_unaligned handles alignment.
        let row: MIB_UDP6ROW_OWNER_PID = unsafe {
            std::ptr::read_unaligned(buf.as_ptr().add(offset).cast())
        };
        if port_from_dw(row.dwLocalPort) == target_port {
            let row_ip = Ipv6Addr::from(row.ucLocalAddr);
            if row_ip == target_ip {
                return Some(row.dwOwningPid);
            }
            if row_ip.is_unspecified() && fallback_pid.is_none() {
                fallback_pid = Some(row.dwOwningPid);
            }
        }
    }
    fallback_pid
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: Closing open handle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

fn query_process_image_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    // SAFETY: OpenProcess is called with PROCESS_QUERY_LIMITED_INFORMATION, bInheritHandle=0, and valid pid.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let guard = HandleGuard(handle);

    let mut buf = [0u16; 2048];
    let mut size = buf.len() as u32;
    // SAFETY: guard.0 is a valid open process handle, buf is allocated, size is in/out pointer.
    let ret = unsafe {
        QueryFullProcessImageNameW(guard.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size)
    };
    if ret != 0 && size > 0 && (size as usize) <= buf.len() {
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_from_dw() {
        assert_eq!(port_from_dw((80u16).to_be() as u32), 80);
        assert_eq!(port_from_dw((443u16).to_be() as u32), 443);
        assert_eq!(port_from_dw((5353u16).to_be() as u32), 5353);
        assert_eq!(
            port_from_dw(0xFFFF_0000 | ((8080u16).to_be() as u32)),
            8080
        );
    }

    #[test]
    fn test_matches_ip_addr() {
        let unspec_v4 = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        let ip_v4_a = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
        let ip_v4_b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        assert!(matches_ip_addr(unspec_v4, ip_v4_a));
        assert!(matches_ip_addr(ip_v4_a, ip_v4_a));
        assert!(!matches_ip_addr(ip_v4_a, ip_v4_b));

        let unspec_v6 = IpAddr::V6(Ipv6Addr::UNSPECIFIED);
        let ip_v6_a = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let ip_v6_b = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2));

        assert!(matches_ip_addr(unspec_v6, ip_v6_a));
        assert!(matches_ip_addr(ip_v6_a, ip_v6_a));
        assert!(!matches_ip_addr(ip_v6_a, ip_v6_b));
    }

    #[test]
    fn test_parse_excluded_apps() {
        assert!(parse_excluded_apps("").is_empty());
        assert!(parse_excluded_apps("   ").is_empty());
        assert_eq!(
            parse_excluded_apps("chrome.exe|firefox.exe"),
            vec!["chrome.exe", "firefox.exe"]
        );
        assert_eq!(
            parse_excluded_apps("  app1.exe | C:\\path\\app2.exe  | "),
            vec!["app1.exe", "C:\\path\\app2.exe"]
        );
        assert_eq!(
            parse_excluded_apps("\"quoted.exe\"|\"C:\\dir with space\\app.exe\""),
            vec!["quoted.exe", "C:\\dir with space\\app.exe"]
        );
    }

    #[test]
    fn test_extract_file_name() {
        assert_eq!(extract_file_name(r"C:\Program Files\App\app.exe"), "app.exe");
        assert_eq!(extract_file_name("C:/Program Files/App/app.exe"), "app.exe");
        assert_eq!(extract_file_name("app.exe"), "app.exe");
        assert_eq!(extract_file_name(""), "");
    }

    #[test]
    fn test_matches_rule_filename() {
        assert!(matches_rule(r"C:\Windows\System32\notepad.exe", "notepad.exe"));
        assert!(matches_rule(r"C:\Windows\System32\NOTEPAD.EXE", "notepad.exe"));
        assert!(matches_rule(r"C:\Windows\System32\notepad.exe", "NOTEPAD.EXE"));
        assert!(matches_rule("C:/Windows/System32/notepad.exe", "notepad.exe"));
        assert!(matches_rule("notepad.exe", "notepad.exe"));
        assert!(!matches_rule(r"C:\Windows\System32\calc.exe", "notepad.exe"));
        assert!(!matches_rule(r"C:\Windows\System32\notepad.exe.bak", "notepad.exe"));
    }

    #[test]
    fn test_matches_rule_fullpath() {
        assert!(matches_rule(
            r"C:\Program Files\App\app.exe",
            r"C:\Program Files\App\app.exe"
        ));
        assert!(matches_rule(
            r"C:\Program Files\App\app.exe",
            "c:/program files/app/app.exe"
        ));
        assert!(matches_rule(
            r"\\?\C:\Program Files\App\app.exe",
            r"C:\Program Files\App\app.exe"
        ));
        assert!(!matches_rule(
            r"C:\Other\app.exe",
            r"C:\Program Files\App\app.exe"
        ));
    }

    #[test]
    fn test_parse_tcp4_table_mock() {
        let mut buf = Vec::new();
        let num_entries: u32 = 2;
        buf.extend_from_slice(&num_entries.to_ne_bytes());

        // Row 1: port 80 on 0.0.0.0, PID 1000
        let mut row1 = [0u8; 24];
        row1[0..4].copy_from_slice(&5u32.to_ne_bytes()); // dwState
        row1[4..8].copy_from_slice(&0u32.to_ne_bytes()); // dwLocalAddr (0.0.0.0)
        row1[8..12].copy_from_slice(&((80u16).to_be() as u32).to_ne_bytes()); // dwLocalPort
        row1[20..24].copy_from_slice(&1000u32.to_ne_bytes()); // dwOwningPid
        buf.extend_from_slice(&row1);

        // Row 2: port 443 on 192.168.1.100, PID 2000
        let mut row2 = [0u8; 24];
        row2[0..4].copy_from_slice(&5u32.to_ne_bytes());
        row2[4..8].copy_from_slice(&Ipv4Addr::new(192, 168, 1, 100).octets());
        row2[8..12].copy_from_slice(&((443u16).to_be() as u32).to_ne_bytes());
        row2[20..24].copy_from_slice(&2000u32.to_ne_bytes());
        buf.extend_from_slice(&row2);

        // Matching port 80 against 0.0.0.0 listener should return fallback PID 1000
        let addr1 = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 50), 80);
        assert_eq!(parse_tcp4_table(&buf, addr1), Some(1000));

        // Matching port 443 on exact IP should return PID 2000
        let addr2 = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 100), 443);
        assert_eq!(parse_tcp4_table(&buf, addr2), Some(2000));

        // Mismatched IP for port 443
        let addr_mismatch_ip = SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 443);
        assert_eq!(parse_tcp4_table(&buf, addr_mismatch_ip), None);

        // Mismatched port
        let addr_mismatch_port = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 100), 8080);
        assert_eq!(parse_tcp4_table(&buf, addr_mismatch_port), None);
    }

    #[test]
    fn test_parse_udp4_table_mock() {
        let mut buf = Vec::new();
        let num_entries: u32 = 1;
        buf.extend_from_slice(&num_entries.to_ne_bytes());

        let mut row = [0u8; 12];
        row[0..4].copy_from_slice(&Ipv4Addr::new(192, 168, 1, 100).octets());
        row[4..8].copy_from_slice(&((5353u16).to_be() as u32).to_ne_bytes());
        row[8..12].copy_from_slice(&3000u32.to_ne_bytes());
        buf.extend_from_slice(&row);

        let addr = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 100), 5353);
        assert_eq!(parse_udp4_table(&buf, addr), Some(3000));

        let addr_diff = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 100), 5354);
        assert_eq!(parse_udp4_table(&buf, addr_diff), None);
    }
}
