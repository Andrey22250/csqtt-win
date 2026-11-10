use std::net::SocketAddr;
use std::os::windows::io::AsRawSocket;
use std::sync::OnceLock;
use tokio_util::sync::CancellationToken;
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Networking::WinSock::{
    IPPROTO_IP, IPPROTO_IPV6, IP_UNICAST_IF, IPV6_UNICAST_IF, SIO_UDP_CONNRESET, SOCKET,
    WSAIoctl, setsockopt,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
    TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

/// Parse the bypass interface index from string slice.
/// Returns None if value is absent, empty, 0, or unparseable.
pub fn parse_bypass_ifindex(val: Option<&str>) -> Option<u32> {
    match val {
        Some(s) => match s.trim().parse::<u32>() {
            Ok(idx) if idx != 0 => Some(idx),
            _ => None,
        },
        None => None,
    }
}

/// Read physical interface index from CSQTT_BYPASS_IFINDEX env var once and cache it.
pub fn bypass_ifindex() -> Option<u32> {
    static IFINDEX: OnceLock<Option<u32>> = OnceLock::new();
    *IFINDEX.get_or_init(|| {
        let val = std::env::var("CSQTT_BYPASS_IFINDEX").ok();
        parse_bypass_ifindex(val.as_deref())
    })
}

/// Local IP of the physical interface (env CSQTT_BYPASS_LOCAL_IP), used to pin primp HTTP
/// clients: Windows uses the strong host model, so a bound source address keeps traffic
/// on its interface even when the tunnel holds the more specific routes.
pub fn bypass_local_ip() -> Option<std::net::IpAddr> {
    static LOCAL_IP: OnceLock<Option<std::net::IpAddr>> = OnceLock::new();
    *LOCAL_IP.get_or_init(|| {
        std::env::var("CSQTT_BYPASS_LOCAL_IP")
            .ok()
            .and_then(|value| value.trim().parse().ok())
    })
}


/// Protect a socket with an explicit interface index.
pub fn protect_socket_ifindex<S: AsRawSocket>(
    s: &S,
    ipv6: bool,
    ifindex: u32,
) -> std::io::Result<()> {
    let raw = s.as_raw_socket() as SOCKET;
    let ret = if ipv6 {
        // IPV6_UNICAST_IF takes interface index in host byte order
        let val: u32 = ifindex;
        unsafe {
            setsockopt(
                raw,
                IPPROTO_IPV6,
                IPV6_UNICAST_IF,
                &val as *const _ as *const u8,
                std::mem::size_of_val(&val) as i32,
            )
        }
    } else {
        // IP_UNICAST_IF takes interface index in network byte order
        let val: u32 = ifindex.to_be();
        unsafe {
            setsockopt(
                raw,
                IPPROTO_IP,
                IP_UNICAST_IF,
                &val as *const _ as *const u8,
                std::mem::size_of_val(&val) as i32,
            )
        }
    };
    if ret != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Bind socket to physical interface to bypass VPN routing loop if CSQTT_BYPASS_IFINDEX is set.
pub fn protect_socket<S: AsRawSocket>(s: &S, ipv6: bool) -> std::io::Result<()> {
    if let Some(ifindex) = bypass_ifindex() {
        protect_socket_ifindex(s, ipv6, ifindex)
    } else {
        Ok(())
    }
}

/// Build a TcpSocket, protect it against routing loop, and connect to remote address.
pub async fn connect_tcp(addr: SocketAddr) -> std::io::Result<tokio::net::TcpStream> {
    let socket = if addr.is_ipv6() {
        tokio::net::TcpSocket::new_v6()?
    } else {
        tokio::net::TcpSocket::new_v4()?
    };
    protect_socket(&socket, addr.is_ipv6())?;
    socket.connect(addr).await
}

/// Disable WSAECONNRESET on UDP socket caused by incoming ICMP port unreachable.
pub fn disable_udp_connreset<S: AsRawSocket>(s: &S) -> std::io::Result<()> {
    let raw = s.as_raw_socket() as SOCKET;
    let mut enable: windows_sys::core::BOOL = 0; // FALSE
    let mut bytes_returned: u32 = 0;
    let ret = unsafe {
        WSAIoctl(
            raw,
            SIO_UDP_CONNRESET,
            &mut enable as *mut _ as *const core::ffi::c_void,
            std::mem::size_of_val(&enable) as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
            None,
        )
    };
    if ret != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Disable UDP connreset trap and protect socket against routing loop.
pub fn prepare_udp<S: AsRawSocket>(s: &S, ipv6: bool) -> std::io::Result<()> {
    disable_udp_connreset(s)?;
    protect_socket(s, ipv6)?;
    Ok(())
}

/// Watch parent process and trigger cancel when it exits.
pub async fn watch_parent(cancel: CancellationToken) {
    if cancel.is_cancelled() {
        return;
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        crate::log_error!("[WATCHDOG] Failed to create process snapshot for parent watchdog");
        cancel.cancelled().await;
        return;
    }

    let my_pid = unsafe { GetCurrentProcessId() };
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut parent_pid = None;
    unsafe {
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == my_pid {
                    parent_pid = Some(entry.th32ParentProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }

    let Some(ppid) = parent_pid else {
        crate::log_error!("[WATCHDOG] Parent PID not found for current process ({my_pid})");
        cancel.cancelled().await;
        return;
    };

    if ppid == 0 {
        crate::log_error!("[WATCHDOG] Parent PID is 0");
        cancel.cancelled().await;
        return;
    }

    let parent_handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, ppid) };
    if parent_handle.is_null() {
        crate::log_error!(
            "[WATCHDOG] Failed to open parent process {ppid}: {}",
            std::io::Error::last_os_error()
        );
        cancel.cancelled().await;
        return;
    }

    let handle_val = parent_handle as usize;
    let cancel_child = cancel.clone();

    let _ = tokio::task::spawn_blocking(move || {
        let handle = handle_val as HANDLE;
        loop {
            if cancel_child.is_cancelled() {
                break;
            }
            let res = unsafe { WaitForSingleObject(handle, 1000) };
            match res {
                WAIT_OBJECT_0 => {
                    cancel_child.cancel();
                    break;
                }
                WAIT_TIMEOUT => {
                    continue;
                }
                _ => {
                    break;
                }
            }
        }
        unsafe {
            CloseHandle(handle);
        }
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    #[test]
    fn test_bypass_ifindex_parsing() {
        assert_eq!(parse_bypass_ifindex(None), None);
        assert_eq!(parse_bypass_ifindex(Some("")), None);
        assert_eq!(parse_bypass_ifindex(Some("   ")), None);
        assert_eq!(parse_bypass_ifindex(Some("0")), None);
        assert_eq!(parse_bypass_ifindex(Some("  0  ")), None);
        assert_eq!(parse_bypass_ifindex(Some("1")), Some(1));
        assert_eq!(parse_bypass_ifindex(Some(" 42 \n")), Some(42));
        assert_eq!(parse_bypass_ifindex(Some("4294967295")), Some(u32::MAX));
        assert_eq!(parse_bypass_ifindex(Some("-1")), None);
        assert_eq!(parse_bypass_ifindex(Some("invalid")), None);
        assert_eq!(parse_bypass_ifindex(Some("12abc")), None);
    }

    #[test]
    fn test_protect_socket_loopback_udp() {
        let sock = UdpSocket::bind("127.0.0.1:0").expect("bind loopback socket");
        // Test protect_socket_ifindex with ifindex 1: should either succeed (if ifindex 1 exists)
        // or return a clean OS error (e.g. WSAEINVAL, WSAENOPROTOOPT, etc.).
        let res = protect_socket_ifindex(&sock, false, 1);
        if let Err(e) = res {
            assert!(
                e.raw_os_error().is_some(),
                "protect_socket error should have raw OS error code: {e}"
            );
        }

        // Test protect_socket: does not fail if CSQTT_BYPASS_IFINDEX is unset or set to valid
        let res_default = protect_socket(&sock, false);
        if let Err(e) = res_default {
            assert!(
                e.raw_os_error().is_some(),
                "protect_socket error should have raw OS error code: {e}"
            );
        }

        // Test disable_udp_connreset on loopback UDP socket
        let res_reset = disable_udp_connreset(&sock);
        assert!(
            res_reset.is_ok(),
            "disable_udp_connreset on UDP socket should succeed: {:?}",
            res_reset.err()
        );
    }
}
