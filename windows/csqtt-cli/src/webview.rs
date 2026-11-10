//! Minimal WebView2 host (webview2-com 0.38 + windows 0.61): one top-level window with one
//! WebView2 controller, driven by a callback. Used by captcha.rs and vk_auth.rs.
//!
//! Threading: `run` must be called on a thread that may become an STA thread (it calls
//! CoInitializeEx(COINIT_APARTMENTTHREADED)); it creates the window, pumps its own message
//! loop and returns when the handler returns `Action::Finish`, the user closes the window,
//! or the timeout fires. Each call is independent (new window, new controller).
//! User data folder: %LOCALAPPDATA%\csqtt-win\webview2 (`profile` selects a subfolder, so
//! the captcha and VK login cookie jars stay separate).

#![allow(dead_code)]

use std::time::Duration;

/// What the page / WebView2 reported.
#[derive(Debug, Clone, PartialEq)]
pub enum WebEvent {
    /// WebView2 is ready and the first navigation to `Options::url` has been started.
    Ready,
    /// NavigationStarting (also fired for redirects). The handler may answer `Action::Cancel...`.
    NavigationStarting { url: String },
    /// NavigationCompleted for the top-level document.
    NavigationCompleted { url: String, success: bool, http_status: i32 },
    /// A request matching one of `Options::intercept_filters` (WebResourceRequested).
    ResourceRequested { url: String, method: String },
    /// `window.chrome.webview.postMessage(x)` from page or injected script: x as JSON text
    /// (strings arrive as JSON strings, e.g. "\"ok\"").
    WebMessage { json: String },
    /// Result of an `Action::ExecuteScript` / `WebCtl::execute_script` call, tagged by `id`.
    ScriptResult { id: u32, json: String },
    /// Periodic tick every `Options::tick` (None = no ticks).
    Tick,
    /// The user closed the window. `run` returns `Err(Closed)` right after the handler.
    Closed,
}

/// Handler reply. For `ResourceRequested` and `NavigationStarting`, `Block` cancels the
/// navigation / answers the request with an empty 403; for other events it is ignored.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Continue,
    Block,
    /// Stop the loop and return `Ok(value)` from `run`.
    Finish(String),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub title: String,
    pub url: String,
    pub width: i32,
    pub height: i32,
    /// false = window stays hidden (auto captcha); `WebCtl::show` can reveal it later.
    pub visible: bool,
    pub user_agent: Option<String>,
    /// Scripts added with AddScriptToExecuteOnDocumentCreated before the first navigation.
    pub init_scripts: Vec<String>,
    /// URI wildcard filters for WebResourceRequested (e.g. "*://api.vk.ru/*"), all contexts.
    pub intercept_filters: Vec<String>,
    /// Cookie jar / cache subfolder name under the user data folder.
    pub profile: String,
    pub timeout: Duration,
    pub tick: Option<Duration>,
}

/// Errors returned by `run`.
#[derive(Debug)]
pub enum WebViewError {
    /// WebView2 Runtime not installed / environment creation failed (message in Russian).
    Runtime(String),
    Timeout,
    Closed,
    Other(String),
}

impl std::fmt::Display for WebViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebViewError::Runtime(m) => write!(f, "WebView2: {m}"),
            WebViewError::Timeout => write!(f, "timeout"),
            WebViewError::Closed => write!(f, "cancelled"),
            WebViewError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for WebViewError {}

/// Control handle passed to the handler (valid only during the callback).
pub trait WebCtl {
    fn navigate(&mut self, url: &str);
    /// ExecuteScript; the result comes back later as `WebEvent::ScriptResult { id, .. }`.
    fn execute_script(&mut self, id: u32, script: &str);
    fn show(&mut self);
    fn set_title(&mut self, title: &str);
    /// "name=value; name2=value2" for `url` from the cookie manager (synchronous wait
    /// pumping messages; empty string on failure).
    fn cookies_for(&mut self, url: &str) -> String;
    /// Delete all cookies of this profile.
    fn clear_cookies(&mut self);
    fn current_url(&self) -> String;
}

/// Run one WebView2 window until the handler finishes, the window is closed or timeout.
pub fn run(
    options: Options,
    handler: &mut dyn FnMut(WebEvent, &mut dyn WebCtl) -> Action,
) -> Result<String, WebViewError> {
    imp::run(options, handler)
}

/// True when the WebView2 Runtime is installed (GetAvailableCoreWebView2BrowserVersionString).
pub fn runtime_available() -> bool {
    imp::runtime_available()
}

/// `%LOCALAPPDATA%\csqtt-win\webview2\<profile>` (profile sanitized to a safe folder name).
pub fn user_data_folder(local_app_data: &std::path::Path, profile: &str) -> std::path::PathBuf {
    let safe: String = profile
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe = if safe.is_empty() { "default".to_string() } else { safe };
    local_app_data.join("csqtt-win").join("webview2").join(safe)
}

/// NUL-terminated UTF-16.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::*;
    use std::result::Result;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::{Interface, PCWSTR, PWSTR, w};

    const TIMER_TICK: usize = 1;
    const TIMER_TIMEOUT: usize = 2;

    type Handler = dyn FnMut(WebEvent, &mut dyn WebCtl) -> Action;

    /// Per-run state shared by the window proc and the COM callbacks (UI thread only).
    struct Shared {
        hwnd: HWND,
        queue: RefCell<VecDeque<WebEvent>>,
        controller: RefCell<Option<ICoreWebView2Controller>>,
        webview: RefCell<Option<ICoreWebView2>>,
        environment: RefCell<Option<ICoreWebView2Environment>>,
        /// Last known cookies per URL (see `cookies_for`) and the URLs with a request in flight.
        cookies: RefCell<std::collections::HashMap<String, String>>,
        cookies_pending: RefCell<std::collections::HashSet<String>>,
        /// Raw pointer to the caller's handler; valid for the whole `run` call.
        handler: Cell<Option<*mut Handler>>,
        in_handler: Cell<bool>,
        finished: RefCell<Option<String>>,
        timed_out: Cell<bool>,
    }

    thread_local! {
        static CURRENT: RefCell<Option<Rc<Shared>>> = const { RefCell::new(None) };
    }

    fn current() -> Option<Rc<Shared>> {
        CURRENT.with(|c| c.borrow().clone())
    }

    fn push(shared: &Shared, event: WebEvent) {
        shared.queue.borrow_mut().push_back(event);
    }

    fn take_pwstr(p: PWSTR) -> String {
        if p.is_null() {
            return String::new();
        }
        CoTaskMemPWSTR::from(p).to_string()
    }

    fn webview_source(webview: &ICoreWebView2) -> String {
        let mut p = PWSTR::null();
        // SAFETY: valid out pointer; the string is freed by CoTaskMemPWSTR.
        if unsafe { webview.Source(&mut p) }.is_ok() { take_pwstr(p) } else { String::new() }
    }

    /// Calls the user handler unless it is already running (re-entrancy guard). In that case
    /// the event is queued and `Continue` is returned.
    fn call_handler(shared: &Rc<Shared>, event: WebEvent) -> Action {
        let Some(ptr) = shared.handler.get() else { return Action::Continue };
        if shared.in_handler.get() || shared.finished.borrow().is_some() {
            push(shared, event);
            return Action::Continue;
        }
        shared.in_handler.set(true);
        let mut ctl = Ctl { shared: shared.clone() };
        // SAFETY: the pointer refers to the `handler` borrowed by `run`, which outlives every
        // callback (callbacks only fire while `run` pumps messages); the in_handler flag
        // prevents creating a second &mut while one is live.
        let action = unsafe { (*ptr)(event, &mut ctl) };
        shared.in_handler.set(false);
        if let Action::Finish(v) = &action {
            shared.finished.borrow_mut().get_or_insert_with(|| v.clone());
        }
        action
    }

    struct Ctl {
        shared: Rc<Shared>,
    }

    impl WebCtl for Ctl {
        fn navigate(&mut self, url: &str) {
            if let Some(wv) = self.shared.webview.borrow().as_ref() {
                let w = wide(url);
                // SAFETY: NUL-terminated buffer alive for the call.
                let _ = unsafe { wv.Navigate(PCWSTR(w.as_ptr())) };
            }
        }

        fn execute_script(&mut self, id: u32, script: &str) {
            let Some(wv) = self.shared.webview.borrow().clone() else { return };
            let shared = self.shared.clone();
            let handler = ExecuteScriptCompletedHandler::create(Box::new(move |_hr, json: String| {
                push(&shared, WebEvent::ScriptResult { id, json });
                Ok(())
            }));
            let w = wide(script);
            // SAFETY: NUL-terminated buffer alive for the call.
            let _ = unsafe { wv.ExecuteScript(PCWSTR(w.as_ptr()), &handler) };
        }

        fn show(&mut self) {
            // SAFETY: plain window calls on our own window.
            unsafe {
                let _ = ShowWindow(self.shared.hwnd, SW_SHOW);
                let _ = SetForegroundWindow(self.shared.hwnd);
            }
        }

        fn set_title(&mut self, title: &str) {
            let w = wide(title);
            // SAFETY: NUL-terminated buffer alive for the call.
            let _ = unsafe { SetWindowTextW(self.shared.hwnd, PCWSTR(w.as_ptr())) };
        }

        /// Non-blocking: returns the last known cookies of `url` (empty before the first answer)
        /// and starts a fresh request whose answer the next call sees. Waiting for the answer
        /// here (a nested message loop inside the handler) starved the window: it stayed black.
        fn cookies_for(&mut self, url: &str) -> String {
            let cached = self.shared.cookies.borrow().get(url).cloned().unwrap_or_default();
            if !self.shared.cookies_pending.borrow_mut().insert(url.to_string()) {
                return cached;
            }
            let Some(wv) = self.shared.webview.borrow().clone() else { return cached };
            let Ok(wv2) = wv.cast::<ICoreWebView2_2>() else { return cached };
            // SAFETY: COM call on a live interface.
            let Ok(manager) = (unsafe { wv2.CookieManager() }) else { return cached };
            let shared = self.shared.clone();
            let key = url.to_string();
            let handler = GetCookiesCompletedHandler::create(Box::new(move |_hr, list: Option<ICoreWebView2CookieList>| {
                let mut out = Vec::new();
                if let Some(list) = list {
                    let mut count = 0u32;
                    // SAFETY: COM calls with valid out pointers.
                    unsafe {
                        let _ = list.Count(&mut count);
                        for i in 0..count {
                            if let Ok(cookie) = list.GetValueAtIndex(i) {
                                let (mut n, mut v) = (PWSTR::null(), PWSTR::null());
                                let _ = cookie.Name(&mut n);
                                let _ = cookie.Value(&mut v);
                                out.push(format!("{}={}", take_pwstr(n), take_pwstr(v)));
                            }
                        }
                    }
                }
                shared.cookies.borrow_mut().insert(key.clone(), out.join("; "));
                shared.cookies_pending.borrow_mut().remove(&key);
                Ok(())
            }));
            let w = wide(url);
            // SAFETY: NUL-terminated buffer alive for the call.
            if unsafe { manager.GetCookies(PCWSTR(w.as_ptr()), &handler) }.is_err() {
                self.shared.cookies_pending.borrow_mut().remove(url);
            }
            cached
        }

        fn clear_cookies(&mut self) {
            let Some(wv) = self.shared.webview.borrow().clone() else { return };
            if let Ok(wv2) = wv.cast::<ICoreWebView2_2>() {
                // SAFETY: COM calls on live interfaces.
                unsafe {
                    if let Ok(manager) = wv2.CookieManager() {
                        let _ = manager.DeleteAllCookies();
                    }
                }
            }
            self.shared.cookies.borrow_mut().clear();
        }

        fn current_url(&self) -> String {
            self.shared.webview.borrow().as_ref().map(webview_source).unwrap_or_default()
        }
    }

    /// Per-monitor DPI awareness for the whole process: without it Windows stretches the
    /// window bitmap on scaled displays and the text is blurry. Ignored if already set.
    fn enable_dpi_awareness() {
        use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
        // SAFETY: process-wide setting with a constant context; failure is harmless.
        let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    }

    extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let shared = current().filter(|s| s.hwnd == hwnd);
        match (msg, shared) {
            (WM_SIZE, Some(s)) => {
                if let Some(c) = s.controller.borrow().as_ref() {
                    let mut rect = RECT::default();
                    // SAFETY: valid window and out pointer.
                    unsafe {
                        let _ = GetClientRect(hwnd, &mut rect);
                        let _ = c.SetBounds(rect);
                    }
                }
                LRESULT(0)
            }
            (WM_DPICHANGED, Some(_)) => {
                // Moved to a monitor with another scale: take the size Windows suggests.
                // SAFETY: for WM_DPICHANGED lparam points to a RECT valid during the call.
                unsafe {
                    let r = &*(lparam.0 as *const RECT);
                    let _ = SetWindowPos(hwnd, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
                }
                LRESULT(0)
            }
            (WM_CLOSE, Some(s)) => {
                // Do not destroy here: `run` reports Closed and cleans up.
                push(&s, WebEvent::Closed);
                LRESULT(0)
            }
            (WM_TIMER, Some(s)) => {
                match wparam.0 {
                    TIMER_TICK => push(&s, WebEvent::Tick),
                    TIMER_TIMEOUT => s.timed_out.set(true),
                    _ => {}
                }
                LRESULT(0)
            }
            // SAFETY: default processing for everything else.
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    fn create_window(options: &Options) -> Result<HWND, WebViewError> {
        let class = w!("csqtt_webview2_host");
        // SAFETY: registering a class / creating a window with valid static strings.
        unsafe {
            let hinstance = GetModuleHandleW(None).map(|h| HINSTANCE(h.0)).ok();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                lpszClassName: class,
                hInstance: hinstance.unwrap_or_default(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                // Window colour instead of black while the page is still loading.
                hbrBackground: windows::Win32::Graphics::Gdi::GetSysColorBrush(windows::Win32::Graphics::Gdi::COLOR_WINDOW),
                ..Default::default()
            };
            // Fails harmlessly with "class already exists" on later calls.
            RegisterClassW(&wc);
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            // Options give the size in logical pixels (96 DPI); scale to the screen and keep it
            // inside the screen.
            let dpi = windows::Win32::UI::HiDpi::GetDpiForSystem().max(96) as i32;
            let w = (options.width.max(200) * dpi / 96).min(sw);
            let h = (options.height.max(200) * dpi / 96).min(sh - 48 * dpi / 96);
            let title = wide(&options.title);
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW,
                ((sw - w) / 2).max(0),
                ((sh - h) / 2).max(0),
                w,
                h,
                None,
                None,
                hinstance,
                None,
            )
            .map_err(|e| WebViewError::Other(format!("CreateWindowExW: {e}")))
        }
    }

    fn create_environment(folder: &std::path::Path) -> Result<ICoreWebView2Environment, WebViewError> {
        let folder_w = wide(&folder.to_string_lossy());
        let (tx, rx) = std::sync::mpsc::channel();
        CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: folder_w lives until the closure returns; the call copies the path.
                unsafe {
                    CreateCoreWebView2EnvironmentWithOptions(
                        PCWSTR::null(),
                        PCWSTR(folder_w.as_ptr()),
                        None::<&ICoreWebView2EnvironmentOptions>,
                        &handler,
                    )
                }
                .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |hr, env| {
                hr?;
                let _ = tx.send(env);
                Ok(())
            }),
        )
        .map_err(|e| WebViewError::Runtime(format!("не установлен WebView2 Runtime ({e:?})")))?;
        rx.recv()
            .ok()
            .flatten()
            .ok_or_else(|| WebViewError::Runtime("не установлен WebView2 Runtime".into()))
    }

    fn create_controller(
        env: &ICoreWebView2Environment,
        hwnd: HWND,
    ) -> Result<ICoreWebView2Controller, WebViewError> {
        let (tx, rx) = std::sync::mpsc::channel();
        let env = env.clone();
        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: COM call on a live environment with our window.
                unsafe { env.CreateCoreWebView2Controller(hwnd, &handler) }
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |hr, controller| {
                hr?;
                let _ = tx.send(controller);
                Ok(())
            }),
        )
        .map_err(|e| WebViewError::Other(format!("WebView2 controller: {e:?}")))?;
        rx.recv().ok().flatten().ok_or_else(|| WebViewError::Other("WebView2 controller: null".into()))
    }

    fn add_init_script(webview: &ICoreWebView2, script: &str) -> Result<(), WebViewError> {
        let webview = webview.clone();
        let w = wide(script);
        AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: NUL-terminated buffer alive for the call.
                unsafe { webview.AddScriptToExecuteOnDocumentCreated(PCWSTR(w.as_ptr()), &handler) }
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(|hr, _id| hr),
        )
        .map_err(|e| WebViewError::Other(format!("AddScriptToExecuteOnDocumentCreated: {e:?}")))
    }

    fn install_events(shared: &Rc<Shared>, webview: &ICoreWebView2) -> windows::core::Result<()> {
        let mut token = 0i64;
        // SAFETY (whole block): registering event handlers on a live webview; handlers run on
        // this thread while `run` pumps messages.
        unsafe {
            let s = shared.clone();
            webview.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_wv, args| {
                    if let Some(args) = args {
                        let mut p = PWSTR::null();
                        let url = if args.Uri(&mut p).is_ok() { take_pwstr(p) } else { String::new() };
                        if call_handler(&s, WebEvent::NavigationStarting { url }) == Action::Block {
                            let _ = args.SetCancel(true);
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;

            let s = shared.clone();
            webview.add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |wv, args| {
                    let url = wv.as_ref().map(webview_source).unwrap_or_default();
                    let mut success = false;
                    let mut http_status = 0i32;
                    if let Some(args) = args {
                        let mut ok = windows::core::BOOL::default();
                        if args.IsSuccess(&mut ok).is_ok() {
                            success = ok.as_bool();
                        }
                        if let Ok(args2) = args.cast::<ICoreWebView2NavigationCompletedEventArgs2>() {
                            let _ = args2.HttpStatusCode(&mut http_status);
                        }
                    }
                    push(&s, WebEvent::NavigationCompleted { url, success, http_status });
                    Ok(())
                })),
                &mut token,
            )?;

            let s = shared.clone();
            webview.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_wv, args| {
                    if let Some(args) = args {
                        let mut p = PWSTR::null();
                        if args.WebMessageAsJson(&mut p).is_ok() {
                            push(&s, WebEvent::WebMessage { json: take_pwstr(p) });
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;

            let s = shared.clone();
            webview.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_wv, args| {
                    let Some(args) = args else { return Ok(()) };
                    let Ok(request) = args.Request() else { return Ok(()) };
                    let (mut u, mut m) = (PWSTR::null(), PWSTR::null());
                    let url = if request.Uri(&mut u).is_ok() { take_pwstr(u) } else { String::new() };
                    let method = if request.Method(&mut m).is_ok() { take_pwstr(m) } else { String::new() };
                    if call_handler(&s, WebEvent::ResourceRequested { url, method }) == Action::Block {
                        let env = s.environment.borrow().clone();
                        if let Some(env) = env
                            && let Ok(resp) = env.CreateWebResourceResponse(
                                None::<&windows::Win32::System::Com::IStream>,
                                403,
                                w!("Forbidden"),
                                w!(""),
                            )
                        {
                            let _ = args.SetResponse(&resp);
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    pub(super) fn run(options: Options, handler: &mut (dyn FnMut(WebEvent, &mut dyn WebCtl) -> Action + '_)) -> Result<String, WebViewError> {
        // SAFETY: COM init for this thread; balanced below when it succeeded.
        let com_inited = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        enable_dpi_awareness();
        let result = run_inner(options, handler);
        if com_inited {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
        result
    }

    fn run_inner(options: Options, handler: &mut (dyn FnMut(WebEvent, &mut dyn WebCtl) -> Action + '_)) -> Result<String, WebViewError> {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let folder = user_data_folder(&base, &options.profile);
        std::fs::create_dir_all(&folder)
            .map_err(|e| WebViewError::Other(format!("{}: {e}", folder.display())))?;

        let hwnd = create_window(&options)?;
        // SAFETY: erase the handler lifetime; it is only used while this function runs and
        // the pointer is cleared (CURRENT reset) before returning.
        let handler_ptr: *mut Handler = unsafe {
            std::mem::transmute::<&mut (dyn FnMut(WebEvent, &mut dyn WebCtl) -> Action + '_), &'static mut Handler>(handler)
        };
        let shared = Rc::new(Shared {
            hwnd,
            queue: RefCell::new(VecDeque::new()),
            controller: RefCell::new(None),
            webview: RefCell::new(None),
            environment: RefCell::new(None),
            cookies: RefCell::new(std::collections::HashMap::new()),
            cookies_pending: RefCell::new(std::collections::HashSet::new()),
            handler: Cell::new(Some(handler_ptr)),
            in_handler: Cell::new(false),
            finished: RefCell::new(None),
            timed_out: Cell::new(false),
        });
        CURRENT.with(|c| *c.borrow_mut() = Some(shared.clone()));

        let result = drive(&shared, &options, &folder);

        // Cleanup: no callbacks may reach the handler after this point.
        shared.handler.set(None);
        if let Some(c) = shared.controller.borrow_mut().take() {
            // SAFETY: closing our own controller.
            let _ = unsafe { c.Close() };
        }
        shared.webview.borrow_mut().take();
        shared.environment.borrow_mut().take();
        // SAFETY: our own window and timers.
        unsafe {
            let _ = KillTimer(Some(hwnd), TIMER_TICK);
            let _ = KillTimer(Some(hwnd), TIMER_TIMEOUT);
            let _ = DestroyWindow(hwnd);
        }
        CURRENT.with(|c| *c.borrow_mut() = None);
        result
    }

    fn drive(shared: &Rc<Shared>, options: &Options, folder: &std::path::Path) -> Result<String, WebViewError> {
        let env = create_environment(folder)?;
        let controller = create_controller(&env, shared.hwnd)?;
        // SAFETY: COM calls on live interfaces and our own window.
        let webview = unsafe {
            let mut rect = RECT::default();
            let _ = GetClientRect(shared.hwnd, &mut rect);
            controller.SetBounds(rect).map_err(|e| WebViewError::Other(e.to_string()))?;
            controller.SetIsVisible(true).map_err(|e| WebViewError::Other(e.to_string()))?;
            controller.CoreWebView2().map_err(|e| WebViewError::Other(e.to_string()))?
        };
        *shared.environment.borrow_mut() = Some(env);
        *shared.controller.borrow_mut() = Some(controller);
        *shared.webview.borrow_mut() = Some(webview.clone());

        if let Some(ua) = &options.user_agent {
            // SAFETY: COM calls on live interfaces; buffer alive for the call.
            unsafe {
                if let Ok(settings) = webview.Settings()
                    && let Ok(s2) = settings.cast::<ICoreWebView2Settings2>()
                {
                    let w = wide(ua);
                    let _ = s2.SetUserAgent(PCWSTR(w.as_ptr()));
                }
            }
        }
        for script in &options.init_scripts {
            add_init_script(&webview, script)?;
        }
        for filter in &options.intercept_filters {
            let w = wide(filter);
            // SAFETY: buffer alive for the call.
            unsafe { webview.AddWebResourceRequestedFilter(PCWSTR(w.as_ptr()), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL) }
                .map_err(|e| WebViewError::Other(format!("AddWebResourceRequestedFilter: {e}")))?;
        }
        install_events(shared, &webview).map_err(|e| WebViewError::Other(format!("WebView2 events: {e}")))?;

        // SAFETY: our own window and timers.
        unsafe {
            if options.visible {
                let _ = ShowWindow(shared.hwnd, SW_SHOW);
                let _ = SetForegroundWindow(shared.hwnd);
            }
            if let Some(tick) = options.tick {
                SetTimer(Some(shared.hwnd), TIMER_TICK, tick.as_millis().clamp(10, u32::MAX as u128) as u32, None);
            }
            SetTimer(
                Some(shared.hwnd),
                TIMER_TIMEOUT,
                options.timeout.as_millis().clamp(10, u32::MAX as u128) as u32,
                None,
            );
            let url = wide(&options.url);
            webview
                .Navigate(PCWSTR(url.as_ptr()))
                .map_err(|e| WebViewError::Other(format!("Navigate: {e}")))?;
        }
        push(shared, WebEvent::Ready);

        loop {
            // Drain queued events.
            loop {
                if let Some(v) = shared.finished.borrow_mut().take() {
                    return Ok(v);
                }
                let Some(event) = shared.queue.borrow_mut().pop_front() else { break };
                let closed = event == WebEvent::Closed;
                if let Action::Finish(v) = call_handler(shared, event) {
                    shared.finished.borrow_mut().take();
                    return Ok(v);
                }
                if closed {
                    return Err(WebViewError::Closed);
                }
            }
            if shared.timed_out.get() {
                return Err(WebViewError::Timeout);
            }
            let mut msg = MSG::default();
            // SAFETY: standard message loop on this thread.
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if ret.0 <= 0 {
                return Err(WebViewError::Other("message loop ended".into()));
            }
            // SAFETY: dispatching the message we just received.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub(super) fn runtime_available() -> bool {
        let mut p = PWSTR::null();
        // SAFETY: valid out pointer; string freed by CoTaskMemPWSTR.
        match unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut p) } {
            Ok(()) => !take_pwstr(p).is_empty(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_data_folder_is_sanitized() {
        let base = std::path::Path::new(r"C:\Users\u\AppData\Local");
        assert_eq!(
            user_data_folder(base, "captcha"),
            std::path::PathBuf::from(r"C:\Users\u\AppData\Local\csqtt-win\webview2\captcha")
        );
        assert!(user_data_folder(base, r"..\x").ends_with("___x"));
        assert!(user_data_folder(base, "").ends_with("default"));
    }

    /// Live smoke test (needs a desktop session and the WebView2 Runtime):
    /// cargo test -p csqtt-cli webview_live -- --ignored
    #[test]
    #[ignore = "opens a hidden WebView2 window"]
    fn webview_live_smoke() {
        if let Ok(dir) = std::env::var("CSQTT_WV_TEST_DIR") {
            // SAFETY: single-threaded test setup before any other env access.
            unsafe { std::env::set_var("LOCALAPPDATA", dir) };
        }
        assert!(runtime_available(), "WebView2 Runtime missing");
        let html = "data:text/html,<html><body>x<script>chrome.webview.postMessage('hi')</script></body></html>";
        let mut steps = Vec::new();
        let result = run(
            Options {
                title: "test".into(),
                url: html.into(),
                width: 400,
                height: 300,
                visible: false,
                user_agent: Some("csqtt-test-agent".into()),
                init_scripts: vec!["window.__init = 42;".into()],
                intercept_filters: vec![],
                profile: "test".into(),
                timeout: Duration::from_secs(20),
                tick: None,
            },
            &mut |event, ctl| {
                steps.push(format!("{event:?}"));
                match event {
                    WebEvent::WebMessage { json } if json == "\"hi\"" => {
                        ctl.execute_script(7, "window.__init + ':' + navigator.userAgent");
                        Action::Continue
                    }
                    WebEvent::ScriptResult { id: 7, json } => Action::Finish(json),
                    _ => Action::Continue,
                }
            },
        );
        eprintln!("events: {steps:?}");
        assert_eq!(result.unwrap(), "\"42:csqtt-test-agent\"");
    }

    #[test]
    fn wide_is_nul_terminated() {
        assert_eq!(wide("ab"), vec![97, 98, 0]);
        assert_eq!(wide(""), vec![0]);
    }
}
