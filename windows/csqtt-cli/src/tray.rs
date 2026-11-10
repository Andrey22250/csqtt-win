//! Tray icon + global hotkey for csqtt-win.
//!
//! Plain Win32 implementation (no extra crates):
//! - Hidden top-level window (`RegisterClassW` + `CreateWindowExW`, never shown)
//! - Stock icon (`LoadIconW(null, IDI_APPLICATION)`) in notification area (`Shell_NotifyIconW`)
//! - Tooltip: "csqtt-win: <state>" or "csqtt-win: служба не запущена"
//! - Right-click context menu: Подключить / Отключить / Показать логи / Выход
//! - Left double-click toggles connect/disconnect
//! - Global hotkey Ctrl+Alt+V (`RegisterHotKey`) toggles connect/disconnect
//! - Balloon notifications (`NIF_INFO`) on state changes Connected/Failed and on Captcha requests
//! - Background tokio runtime communicating with `csqtt-svc` via `csqtt_ipc::pipe::Client`
//! - On exit: `NIM_DELETE`, `UnregisterHotKey`, `DestroyWindow`, `PostQuitMessage`

use std::os::windows::process::CommandExt;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, VK_V,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_INFO,
    NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, NOTIFY_ICON_INFOTIP_FLAGS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage, UnregisterClassW,
    IDI_APPLICATION, MF_DISABLED, MF_ENABLED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    MSG, TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY,
    WM_HOTKEY, WM_LBUTTONDBLCLK, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};

const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1;

const WM_APP_STATUS_UPDATE: u32 = WM_APP + 1;
const WM_APP_CAPTCHA: u32 = WM_APP + 2;
const WM_TRAY_CALLBACK: u32 = WM_APP + 10;

const IDM_OPEN_GUI: usize = 1000;
const IDM_CONNECT: usize = 1001;
const IDM_DISCONNECT: usize = 1002;
const IDM_LOGS: usize = 1003;
const IDM_EXIT: usize = 1004;
const IDM_VK_LOGIN: usize = 1005;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const DETACHED_PROCESS: u32 = 0x00000008;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Connect,
    Disconnect,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestAction {
    Connect,
    Disconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalloonKind {
    Connected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuEnablement {
    pub can_connect: bool,
    pub can_disconnect: bool,
    pub can_logs: bool,
    pub can_exit: bool,
}

struct TrayContext {
    hwnd: AtomicIsize,
    tx_cmd: tokio::sync::mpsc::UnboundedSender<TrayCommand>,
    status: Mutex<Option<csqtt_ipc::StatusInfo>>,
    prev_state: Mutex<Option<csqtt_ipc::TunnelState>>,
    exit_requested: AtomicBool,
    shutdown_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

static CONTEXT: OnceLock<TrayContext> = OnceLock::new();

/// Human-readable Russian state description for `TunnelState` or disconnected service.
pub fn state_display(state: Option<&csqtt_ipc::TunnelState>) -> &'static str {
    match state {
        Some(csqtt_ipc::TunnelState::Stopped) => "отключено",
        Some(csqtt_ipc::TunnelState::Starting) => "подключение...",
        Some(csqtt_ipc::TunnelState::Connected) => "подключено",
        Some(csqtt_ipc::TunnelState::Reconnecting) => "переподключение...",
        Some(csqtt_ipc::TunnelState::Paused) => "приостановлено",
        Some(csqtt_ipc::TunnelState::Failed) => "ошибка",
        None => "служба не запущена",
    }
}

/// Formats the tray icon tooltip according to the specification.
pub fn format_tooltip(state: Option<&csqtt_ipc::TunnelState>) -> String {
    format!("csqtt-win: {}", state_display(state))
}

/// Determines which context menu items should be enabled.
pub fn menu_item_enablement(state: Option<&csqtt_ipc::TunnelState>) -> MenuEnablement {
    match state {
        None => MenuEnablement {
            can_connect: false,
            can_disconnect: false,
            can_logs: true,
            can_exit: true,
        },
        Some(
            csqtt_ipc::TunnelState::Stopped
            | csqtt_ipc::TunnelState::Failed
            | csqtt_ipc::TunnelState::Paused,
        ) => MenuEnablement {
            can_connect: true,
            can_disconnect: false,
            can_logs: true,
            can_exit: true,
        },
        Some(
            csqtt_ipc::TunnelState::Connected
            | csqtt_ipc::TunnelState::Starting
            | csqtt_ipc::TunnelState::Reconnecting,
        ) => MenuEnablement {
            can_connect: false,
            can_disconnect: true,
            can_logs: true,
            can_exit: true,
        },
    }
}

/// Computes the action corresponding to a toggle (double-click or Ctrl+Alt+V).
pub fn action_for_toggle(state: Option<&csqtt_ipc::TunnelState>) -> Option<RequestAction> {
    match state {
        None => None,
        Some(
            csqtt_ipc::TunnelState::Connected
            | csqtt_ipc::TunnelState::Starting
            | csqtt_ipc::TunnelState::Reconnecting,
        ) => Some(RequestAction::Disconnect),
        Some(
            csqtt_ipc::TunnelState::Stopped
            | csqtt_ipc::TunnelState::Failed
            | csqtt_ipc::TunnelState::Paused,
        ) => Some(RequestAction::Connect),
    }
}

/// Checks whether a balloon notification should be shown on a state transition.
pub fn should_show_balloon_for_transition(
    prev: Option<&csqtt_ipc::TunnelState>,
    curr: &csqtt_ipc::TunnelState,
) -> Option<BalloonKind> {
    match curr {
        csqtt_ipc::TunnelState::Connected if prev != Some(&csqtt_ipc::TunnelState::Connected) => {
            Some(BalloonKind::Connected)
        }
        csqtt_ipc::TunnelState::Failed if prev != Some(&csqtt_ipc::TunnelState::Failed) => {
            Some(BalloonKind::Failed)
        }
        _ => None,
    }
}

/// Copies a string as null-terminated UTF-16 code units into a fixed-size buffer.
pub fn copy_str_to_u16_buf<const N: usize>(src: &str, buf: &mut [u16; N]) {
    let mut i = 0;
    for c in src.encode_utf16() {
        if i + 1 >= N {
            break;
        }
        buf[i] = c;
        i += 1;
    }
    buf[i] = 0;
}

fn get_current_tunnel_state() -> Option<csqtt_ipc::TunnelState> {
    CONTEXT.get().and_then(|ctx| {
        ctx.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|s| s.state.clone())
    })
}

fn set_service_status(status: Option<csqtt_ipc::StatusInfo>) {
    if let Some(ctx) = CONTEXT.get() {
        *ctx.status.lock().unwrap_or_else(|e| e.into_inner()) = status;
    }
}

fn notify_window() {
    if let Some(ctx) = CONTEXT.get() {
        let hwnd = ctx.hwnd.load(Ordering::Acquire);
        if hwnd != 0 {
            // SAFETY: PostMessageW is thread-safe and hwnd is a valid HWND or 0.
            unsafe {
                PostMessageW(hwnd as HWND, WM_APP_STATUS_UPDATE, 0, 0);
            }
        }
    }
}

fn notify_captcha_balloon() {
    if let Some(ctx) = CONTEXT.get() {
        let hwnd = ctx.hwnd.load(Ordering::Acquire);
        if hwnd != 0 {
            // SAFETY: PostMessageW is thread-safe and hwnd is a valid HWND or 0.
            unsafe {
                PostMessageW(hwnd as HWND, WM_APP_CAPTCHA, 0, 0);
            }
        }
    }
}

fn send_tray_command(cmd: TrayCommand) {
    if let Some(ctx) = CONTEXT.get() {
        let _ = ctx.tx_cmd.send(cmd);
    }
}

/// Executes a one-shot command over named pipe on the background runtime.
async fn execute_command(cmd: TrayCommand) {
    let req = match cmd {
        TrayCommand::Connect => csqtt_ipc::Request::Connect,
        TrayCommand::Disconnect => csqtt_ipc::Request::Disconnect,
        TrayCommand::Toggle => {
            let current = get_current_tunnel_state();
            match action_for_toggle(current.as_ref()) {
                Some(RequestAction::Connect) => csqtt_ipc::Request::Connect,
                Some(RequestAction::Disconnect) => csqtt_ipc::Request::Disconnect,
                None => return,
            }
        }
    };

    if let Ok(mut client) = csqtt_ipc::pipe::Client::connect().await {
        let _ = client.send(&req).await;
        let _ = client.recv().await;
    }
}

unsafe fn add_tray_icon(
    hwnd: HWND,
    icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
    tooltip: &str,
) {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY_CALLBACK;
    nid.hIcon = icon;
    copy_str_to_u16_buf(tooltip, &mut nid.szTip);
    // SAFETY: nid is a valid NOTIFYICONDATAW struct.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

unsafe fn update_tooltip(hwnd: HWND, tooltip: &str) {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_TIP;
    copy_str_to_u16_buf(tooltip, &mut nid.szTip);
    // SAFETY: nid is a valid NOTIFYICONDATAW struct.
    unsafe {
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

unsafe fn show_balloon(hwnd: HWND, title: &str, text: &str, info_flags: NOTIFY_ICON_INFOTIP_FLAGS) {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = info_flags;
    copy_str_to_u16_buf(text, &mut nid.szInfo);
    copy_str_to_u16_buf(title, &mut nid.szInfoTitle);
    nid.Anonymous.uTimeout = 10_000;
    // SAFETY: nid is a valid NOTIFYICONDATAW struct.
    unsafe {
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

unsafe fn delete_tray_icon(hwnd: HWND) {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    // SAFETY: nid is a valid NOTIFYICONDATAW struct.
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe fn append_menu_item(
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    id: usize,
    flags: u32,
    text: &str,
) {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: menu is a valid HMENU and wide is null-terminated UTF-16.
    unsafe {
        AppendMenuW(menu, flags, id, wide.as_ptr());
    }
}

unsafe fn append_menu_separator(menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU) {
    // SAFETY: menu is a valid HMENU.
    unsafe {
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    }
}

unsafe fn show_context_menu(hwnd: HWND) {
    let mut pt = POINT { x: 0, y: 0 };
    // SAFETY: pt is a valid mutable pointer, hwnd is valid.
    unsafe {
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }

        let curr_state = get_current_tunnel_state();
        let enablement = menu_item_enablement(curr_state.as_ref());

        append_menu_item(menu, IDM_OPEN_GUI, MF_STRING | MF_ENABLED, "Открыть окно");
        append_menu_separator(menu);

        let connect_flags = MF_STRING
            | if enablement.can_connect {
                MF_ENABLED
            } else {
                MF_GRAYED | MF_DISABLED
            };
        append_menu_item(menu, IDM_CONNECT, connect_flags, "Подключить");

        let disconnect_flags = MF_STRING
            | if enablement.can_disconnect {
                MF_ENABLED
            } else {
                MF_GRAYED | MF_DISABLED
            };
        append_menu_item(menu, IDM_DISCONNECT, disconnect_flags, "Отключить");

        append_menu_separator(menu);

        append_menu_item(menu, IDM_VK_LOGIN, MF_STRING | MF_ENABLED, "Войти в VK");

        append_menu_separator(menu);

        append_menu_item(menu, IDM_LOGS, MF_STRING | MF_ENABLED, "Показать логи");

        append_menu_separator(menu);

        append_menu_item(menu, IDM_EXIT, MF_STRING | MF_ENABLED, "Выход");

        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );

        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}

fn handle_status_update(hwnd: HWND) {
    let ctx = match CONTEXT.get() {
        Some(ctx) => ctx,
        None => return,
    };

    let status_opt = ctx.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let current_state = status_opt.as_ref().map(|s| &s.state);
    let tooltip = format_tooltip(current_state);

    // SAFETY: hwnd is valid and tooltip string is properly formatted.
    unsafe {
        update_tooltip(hwnd, &tooltip);
    }

    let mut prev_guard = ctx.prev_state.lock().unwrap_or_else(|e| e.into_inner());
    let prev = prev_guard.clone();

    if let Some(curr) = current_state {
        if let Some(kind) = should_show_balloon_for_transition(prev.as_ref(), curr) {
            match kind {
                BalloonKind::Connected => {
                    let msg = if let Some(ref st) = status_opt {
                        if !st.client_ip.is_empty() {
                            format!("Подключено ({})", st.client_ip)
                        } else {
                            "Подключено".to_string()
                        }
                    } else {
                        "Подключено".to_string()
                    };
                    // SAFETY: hwnd is valid.
                    unsafe {
                        show_balloon(hwnd, "csqtt-win", &msg, NIIF_INFO);
                    }
                }
                BalloonKind::Failed => {
                    let msg = if let Some(ref st) = status_opt {
                        if !st.message.is_empty() {
                            format!("Ошибка подключения: {}", st.message)
                        } else {
                            "Ошибка подключения".to_string()
                        }
                    } else {
                        "Ошибка подключения".to_string()
                    };
                    // SAFETY: hwnd is valid.
                    unsafe {
                        show_balloon(hwnd, "csqtt-win", &msg, NIIF_ERROR);
                    }
                }
            }
        }
        *prev_guard = Some(curr.clone());
    } else {
        *prev_guard = None;
    }
}

fn exit_tray(hwnd: HWND) {
    if let Some(ctx) = CONTEXT.get() {
        if ctx.exit_requested.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(guard) = ctx.shutdown_tx.lock() {
            if let Some(ref tx) = *guard {
                let _ = tx.send(true);
            }
        }
    }

    // SAFETY: Exit sequence removes the notification icon, unregisters hotkey,
    // destroys the window, and posts quit message to end the loop.
    unsafe {
        delete_tray_icon(hwnd);
        UnregisterHotKey(hwnd, HOTKEY_ID);
        DestroyWindow(hwnd);
        PostQuitMessage(0);
    }
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            let event = (lparam & 0xffff) as u32;
            match event {
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    // SAFETY: hwnd is a valid message window.
                    unsafe {
                        show_context_menu(hwnd);
                    }
                }
                WM_LBUTTONDBLCLK => {
                    send_tray_command(TrayCommand::Toggle);
                }
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            let cmd_id = (wparam & 0xffff) as usize;
            match cmd_id {
                IDM_OPEN_GUI => {
                    let exe = std::env::current_exe()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "csqtt".to_string());
                    let _ = std::process::Command::new(exe)
                        .arg("gui")
                        .creation_flags(DETACHED_PROCESS)
                        .spawn();
                }
                IDM_CONNECT => {
                    send_tray_command(TrayCommand::Connect);
                }
                IDM_DISCONNECT => {
                    send_tray_command(TrayCommand::Disconnect);
                }
                IDM_VK_LOGIN => {
                    let exe = std::env::current_exe()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "csqtt".to_string());
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", &exe, "vk-login"])
                        .spawn();
                }
                IDM_LOGS => {
                    let _ = std::process::Command::new("cmd")
                        .args(["/k", "csqtt", "logs", "-f"])
                        .spawn();
                }
                IDM_EXIT => {
                    exit_tray(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_HOTKEY => {
            if wparam == HOTKEY_ID as usize {
                send_tray_command(TrayCommand::Toggle);
            }
            0
        }
        WM_APP_STATUS_UPDATE => {
            handle_status_update(hwnd);
            0
        }
        WM_APP_CAPTCHA => {
            // SAFETY: hwnd is valid.
            unsafe {
                show_balloon(
                    hwnd,
                    "csqtt-win",
                    "Требуется капча",
                    NIIF_WARNING,
                );
            }
            0
        }
        WM_DESTROY => {
            exit_tray(hwnd);
            0
        }
        _ => {
            // SAFETY: Forward unhandled message to standard default window proc.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

/// Extracts the token from a `CAPTCHA_RESULT <token-or-error>` line.
pub fn parse_captcha_result_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n').trim_start();
    if let Some(rest) = trimmed.strip_prefix("CAPTCHA_RESULT") {
        let token = rest.strip_prefix(' ').unwrap_or(rest).trim();
        Some(token)
    } else {
        None
    }
}

async fn get_captcha_solve_method() -> String {
    if let Ok(mut client) = csqtt_ipc::pipe::Client::connect().await {
        if client.send(&csqtt_ipc::Request::GetSettings).await.is_ok() {
            if let Ok(Some(csqtt_ipc::Response::Settings { settings })) = client.recv().await {
                if let Ok(s) = serde_json::from_value::<csqtt_logic::settings::Settings>(settings) {
                    let m = s.current_profile().captcha_solve_method.trim();
                    if !m.is_empty() {
                        return m.to_string();
                    }
                }
            }
        }
    }
    "auto".to_string()
}

pub(crate) async fn solve_captcha_task(
    mode: String,
    redirect_uri: String,
    session_token: String,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) {
    let _claim = match crate::gui::NamedClaim::claim(&crate::gui::captcha_claim_name(&session_token)) {
        Some(c) => c,
        None => return,
    };
    let timeout_duration = std::time::Duration::from_secs(300);
    let result = tokio::time::timeout(timeout_duration, async {
        let solve_method = get_captcha_solve_method().await;

        let payload_json = serde_json::json!({
            "mode": mode,
            "redirect_uri": redirect_uri,
            "session_token": session_token,
            "solve_method": solve_method,
        });

        let b64 = crate::base64url_encode(payload_json.to_string().as_bytes());

        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => return format!("error:current_exe failed: {e}"),
        };

        let mut cmd = tokio::process::Command::new(current_exe);
        cmd.args(["captcha-window", &b64]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.kill_on_drop(true);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return format!("error:spawn captcha-window failed: {e}"),
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return "error:child stdout missing".to_string(),
        };

        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    return "error:captcha-window closed without result".to_string();
                }
                Ok(_) => {
                    if let Some(token) = parse_captcha_result_line(&line) {
                        return token.to_string();
                    }
                }
                Err(e) => {
                    return format!("error:reading stdout: {e}");
                }
            }
        }
    }).await;

    let token = match result {
        Ok(t) => t,
        Err(_) => "error:timeout".to_string(),
    };

    let _ = tx.send(token);
}

/// Runs the tray message loop until "Exit" is chosen. Blocking.
#[allow(dead_code)]
/// Runs the tray icon: one per session; starts the service and, on exit, stops it unless the
/// window is still open (see svc_control).
pub fn run_tray() -> anyhow::Result<()> {
    let Some(_instance) = crate::gui::NamedClaim::claim(crate::svc_control::TRAY_MUTEX) else {
        return Ok(());
    };
    std::thread::spawn(crate::svc_control::start_service);
    let result = run_tray_inner();
    crate::svc_control::stop_if_last(crate::svc_control::GUI_MUTEX);
    result
}

fn run_tray_inner() -> anyhow::Result<()> {
    let (tx_cmd, mut rx_cmd) = tokio::sync::mpsc::unbounded_channel::<TrayCommand>();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let ctx = CONTEXT.get_or_init(|| TrayContext {
        hwnd: AtomicIsize::new(0),
        tx_cmd,
        status: Mutex::new(None),
        prev_state: Mutex::new(None),
        exit_requested: AtomicBool::new(false),
        shutdown_tx: Mutex::new(Some(shutdown_tx.clone())),
    });

    ctx.exit_requested.store(false, Ordering::Release);
    *ctx.status.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *ctx.prev_state.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *ctx.shutdown_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(shutdown_tx.clone());

    // SAFETY: NULL parameter retrieves module handle of current process.
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let class_name = windows_sys::core::w!("csqtt_tray_window_class");
    let window_name = windows_sys::core::w!("csqtt_tray_window");

    let wnd_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(tray_window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name,
    };

    // SAFETY: wnd_class points to a valid struct with null-terminated class name.
    let atom = unsafe { RegisterClassW(&wnd_class) };
    if atom == 0 {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        // 1410 is ERROR_CLASS_ALREADY_EXISTS. If already registered, proceed.
        if err != 1410 {
            anyhow::bail!("RegisterClassW failed: error code {err}");
        }
    }

    // SAFETY: class_name is registered. A plain hidden top-level window (never shown):
    // message-only windows miss broadcasts and SetForegroundWindow for the tray menu.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name,
            window_name,
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        anyhow::bail!("CreateWindowExW failed: error code {err}");
    }

    ctx.hwnd.store(hwnd as isize, Ordering::Release);

    // Register global hotkey Ctrl+Alt+V
    // SAFETY: hwnd is a valid window handle.
    unsafe {
        RegisterHotKey(hwnd, HOTKEY_ID, MOD_CONTROL | MOD_ALT, VK_V as u32);
    }

    // Load stock application icon
    // SAFETY: NULL hInstance loads standard Windows IDI_APPLICATION icon.
    let stock_icon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };

    // Add tray icon with initial tooltip
    let initial_tooltip = format_tooltip(None);
    // SAFETY: hwnd is valid window handle, stock_icon is valid.
    unsafe {
        add_tray_icon(hwnd, stock_icon, &initial_tooltip);
    }

    // Spawn background thread with tokio runtime
    let mut bg_shutdown_rx = shutdown_rx.clone();
    let bg_thread = std::thread::Builder::new()
        .name("csqtt-tray-ipc".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("csqtt tray: failed to build tokio runtime: {e}");
                    return;
                }
            };

            rt.block_on(async move {
                // Command dispatcher task
                let mut cmd_shutdown_rx = bg_shutdown_rx.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = cmd_shutdown_rx.changed() => break,
                            cmd_opt = rx_cmd.recv() => {
                                match cmd_opt {
                                    Some(cmd) => {
                                        tokio::spawn(async move {
                                            execute_command(cmd).await;
                                        });
                                    }
                                    None => break,
                                }
                            }
                        }
                    }
                });

                // Subscriber loop
                loop {
                    if *bg_shutdown_rx.borrow() {
                        break;
                    }

                    let connect_res = tokio::select! {
                        _ = bg_shutdown_rx.changed() => break,
                        res = csqtt_ipc::pipe::Client::connect() => res,
                    };

                    match connect_res {
                        Ok(mut client) => {
                            let sub_res = tokio::select! {
                                _ = bg_shutdown_rx.changed() => break,
                                res = client.send(&csqtt_ipc::Request::Subscribe) => res,
                            };

                            if sub_res.is_err() {
                                set_service_status(None);
                                notify_window();
                                tokio::select! {
                                    _ = bg_shutdown_rx.changed() => break,
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                                }
                                continue;
                            }

                            // Subscribed! Read responses
                            let (tx_captcha, mut rx_captcha) =
                                tokio::sync::mpsc::unbounded_channel::<String>();
                            let mut disconnected = false;
                            while !disconnected {
                                tokio::select! {
                                    _ = bg_shutdown_rx.changed() => {
                                        return;
                                    }
                                    Some(token) = rx_captcha.recv() => {
                                        let answer = csqtt_ipc::Request::CaptchaResult { token };
                                        if client.send(&answer).await.is_err() {
                                            disconnected = true;
                                        }
                                    }
                                    resp = client.recv() => {
                                        match resp {
                                            Ok(Some(response)) => {
                                                match response {
                                                    csqtt_ipc::Response::Status { status } => {
                                                        set_service_status(Some(status));
                                                        notify_window();
                                                    }
                                                    csqtt_ipc::Response::Captcha { mode, redirect_uri, session_token } => {
                                                        notify_captcha_balloon();
                                                        let tx = tx_captcha.clone();
                                                        tokio::spawn(async move {
                                                            solve_captcha_task(mode, redirect_uri, session_token, tx).await;
                                                        });
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            Ok(None) | Err(_) => {
                                                disconnected = true;
                                            }
                                        }
                                    }
                                }
                            }

                            set_service_status(None);
                            notify_window();
                            tokio::select! {
                                _ = bg_shutdown_rx.changed() => break,
                                _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                            }
                        }
                        Err(_) => {
                            set_service_status(None);
                            notify_window();
                            tokio::select! {
                                _ = bg_shutdown_rx.changed() => break,
                                _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                            }
                        }
                    }
                }
            });
        })?;

    // Win32 message loop
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: msg is a valid pointer. NULL hWnd retrieves messages for current thread.
        let ret = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if ret <= 0 {
            break;
        }
        // SAFETY: msg is a valid pointer to retrieved message.
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup after message loop terminates
    let _ = shutdown_tx.send(true);
    let _ = bg_thread.join();

    // SAFETY: class_name is a valid null-terminated UTF-16 string; hinstance is valid module handle.
    unsafe {
        UnregisterClassW(class_name, hinstance);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use csqtt_ipc::TunnelState;

    #[test]
    fn test_format_tooltip() {
        assert_eq!(format_tooltip(None), "csqtt-win: служба не запущена");
        assert_eq!(
            format_tooltip(Some(&TunnelState::Stopped)),
            "csqtt-win: отключено"
        );
        assert_eq!(
            format_tooltip(Some(&TunnelState::Starting)),
            "csqtt-win: подключение..."
        );
        assert_eq!(
            format_tooltip(Some(&TunnelState::Connected)),
            "csqtt-win: подключено"
        );
        assert_eq!(
            format_tooltip(Some(&TunnelState::Reconnecting)),
            "csqtt-win: переподключение..."
        );
        assert_eq!(
            format_tooltip(Some(&TunnelState::Paused)),
            "csqtt-win: приостановлено"
        );
        assert_eq!(
            format_tooltip(Some(&TunnelState::Failed)),
            "csqtt-win: ошибка"
        );
    }

    #[test]
    fn test_menu_enablement() {
        let none = menu_item_enablement(None);
        assert!(!none.can_connect);
        assert!(!none.can_disconnect);
        assert!(none.can_logs);
        assert!(none.can_exit);

        let stopped = menu_item_enablement(Some(&TunnelState::Stopped));
        assert!(stopped.can_connect);
        assert!(!stopped.can_disconnect);
        assert!(stopped.can_logs);
        assert!(stopped.can_exit);

        let failed = menu_item_enablement(Some(&TunnelState::Failed));
        assert!(failed.can_connect);
        assert!(!failed.can_disconnect);

        let paused = menu_item_enablement(Some(&TunnelState::Paused));
        assert!(paused.can_connect);
        assert!(!paused.can_disconnect);

        let connected = menu_item_enablement(Some(&TunnelState::Connected));
        assert!(!connected.can_connect);
        assert!(connected.can_disconnect);

        let starting = menu_item_enablement(Some(&TunnelState::Starting));
        assert!(!starting.can_connect);
        assert!(starting.can_disconnect);

        let reconnecting = menu_item_enablement(Some(&TunnelState::Reconnecting));
        assert!(!reconnecting.can_connect);
        assert!(reconnecting.can_disconnect);
    }

    #[test]
    fn test_action_for_toggle() {
        assert_eq!(action_for_toggle(None), None);
        assert_eq!(
            action_for_toggle(Some(&TunnelState::Stopped)),
            Some(RequestAction::Connect)
        );
        assert_eq!(
            action_for_toggle(Some(&TunnelState::Failed)),
            Some(RequestAction::Connect)
        );
        assert_eq!(
            action_for_toggle(Some(&TunnelState::Paused)),
            Some(RequestAction::Connect)
        );
        assert_eq!(
            action_for_toggle(Some(&TunnelState::Connected)),
            Some(RequestAction::Disconnect)
        );
        assert_eq!(
            action_for_toggle(Some(&TunnelState::Starting)),
            Some(RequestAction::Disconnect)
        );
        assert_eq!(
            action_for_toggle(Some(&TunnelState::Reconnecting)),
            Some(RequestAction::Disconnect)
        );
    }

    #[test]
    fn test_balloon_transitions() {
        assert_eq!(
            should_show_balloon_for_transition(None, &TunnelState::Connected),
            Some(BalloonKind::Connected)
        );
        assert_eq!(
            should_show_balloon_for_transition(
                Some(&TunnelState::Starting),
                &TunnelState::Connected
            ),
            Some(BalloonKind::Connected)
        );
        assert_eq!(
            should_show_balloon_for_transition(
                Some(&TunnelState::Connected),
                &TunnelState::Connected
            ),
            None
        );
        assert_eq!(
            should_show_balloon_for_transition(
                Some(&TunnelState::Connected),
                &TunnelState::Reconnecting
            ),
            None
        );
        assert_eq!(
            should_show_balloon_for_transition(
                Some(&TunnelState::Reconnecting),
                &TunnelState::Failed
            ),
            Some(BalloonKind::Failed)
        );
        assert_eq!(
            should_show_balloon_for_transition(Some(&TunnelState::Failed), &TunnelState::Failed),
            None
        );
        assert_eq!(
            should_show_balloon_for_transition(Some(&TunnelState::Failed), &TunnelState::Stopped),
            None
        );
    }

    #[test]
    fn test_copy_str_to_u16_buf() {
        let mut buf = [0u16; 8];
        copy_str_to_u16_buf("abc", &mut buf);
        let s = String::from_utf16_lossy(&buf[..3]);
        assert_eq!(s, "abc");
        assert_eq!(buf[3], 0);

        let mut small_buf = [0u16; 4];
        copy_str_to_u16_buf("longstring", &mut small_buf);
        assert_eq!(small_buf[3], 0);
        let s_truncated = String::from_utf16_lossy(&small_buf[..3]);
        assert_eq!(s_truncated, "lon");
    }

    #[test]
    fn test_parse_captcha_result_line() {
        assert_eq!(parse_captcha_result_line("CAPTCHA_RESULT abc"), Some("abc"));
        assert_eq!(
            parse_captcha_result_line("CAPTCHA_RESULT token_123\r\n"),
            Some("token_123")
        );
        assert_eq!(
            parse_captcha_result_line("CAPTCHA_RESULT error:cancelled\n"),
            Some("error:cancelled")
        );
        assert_eq!(parse_captcha_result_line("CAPTCHA_RESULT "), Some(""));
        assert_eq!(parse_captcha_result_line("CAPTCHA_RESULT\n"), Some(""));
        assert_eq!(parse_captcha_result_line("[КАПЧА WBV] step 1"), None);
        assert_eq!(parse_captcha_result_line("random stdout line"), None);
    }
}
