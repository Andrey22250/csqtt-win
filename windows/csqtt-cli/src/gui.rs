//! `csqtt gui`: the main window. The UI is HTML/CSS/JS from `csqtt-cli/gui/` (embedded into
//! the exe, unpacked to `%LOCALAPPDATA%\csqtt-win\gui` and opened in WebView2 by file URL);
//! this module is the bridge between the page and the service.
//!
//! Page -> host: `chrome.webview.postMessage({id, method, params})`.
//! Host -> page: `window.__csqttHost.recv([msg, ...])`, where msg is a reply
//! `{id, ok, result | error}` or an event `{event, data}`. See `gui/bridge.js` for the
//! page-side API and the list of methods and events.
//!
//! Threads: the UI thread runs `webview::run`; one worker thread runs a tokio runtime with
//! the service subscription (status, logs, captcha) and the host calls. The worker hands
//! finished JSON messages to the UI thread through a channel drained on every tick.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use csqtt_ipc::pipe::Client;
use csqtt_ipc::{Request, Response};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::webview::{self, Action, Options, WebCtl, WebEvent, WebViewError};

/// Window title; also used to find and raise an already running window.
pub const WINDOW_TITLE: &str = "csqtt-win";
const INSTANCE_MUTEX: &str = crate::svc_control::GUI_MUTEX;
/// Subcommands of our own exe the page may start through `proc_start`.
const ALLOWED_SUBCOMMANDS: &[&str] = &["vk-login", "vk-logout", "deploy", "undeploy", "update-check"];
const TICK: Duration = Duration::from_millis(16);
const MAX_MESSAGES_PER_TICK: usize = 400;
const IPC_TIMEOUT: Duration = Duration::from_secs(20);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

macro_rules! asset {
    ($name:literal) => {
        ($name, include_str!(concat!("../gui/", $name)))
    };
}

/// Every file of the UI. A new file must be added here and to `index.html`.
const ASSETS: &[(&str, &str)] = &[
    asset!("index.html"),
    asset!("app.css"),
    asset!("bridge.js"),
    asset!("mock.js"),
    asset!("icons.js"),
    asset!("components.js"),
    asset!("app.js"),
    asset!("connection.js"),
    asset!("logs.js"),
    asset!("settings.js"),
    asset!("exceptions.js"),
    asset!("deploy.js"),
    asset!("info.js"),
    asset!("dialogs.js"),
];

struct HostCall {
    id: u64,
    method: String,
    params: Value,
}

/// A named kernel object held for as long as this value lives. `claim` returns `None` when
/// another process already holds the same name.
pub(crate) struct NamedClaim(isize);

// SAFETY: a kernel handle may be closed from any thread.
unsafe impl Send for NamedClaim {}
unsafe impl Sync for NamedClaim {}

impl NamedClaim {
    pub(crate) fn claim(name: &str) -> Option<NamedClaim> {
        use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::core::PCWSTR;
        let wide = webview::wide(name);
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) }.ok()?;
        // SAFETY: reads the thread's last error set by CreateMutexW just above.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: `handle` was just returned by CreateMutexW and is closed once.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return None;
        }
        Some(NamedClaim(handle.0 as isize))
    }
}

impl Drop for NamedClaim {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        // SAFETY: the handle came from CreateMutexW and is closed exactly once here.
        unsafe {
            let _ = CloseHandle(HANDLE(self.0 as *mut core::ffi::c_void));
        }
    }
}

/// Name of the claim that decides which process (tray or window) solves one captcha.
pub(crate) fn captcha_claim_name(session_token: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in session_token.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("Local\\csqtt-win-captcha-{hash:016x}")
}

/// Opens the main window and blocks until it is closed.
pub fn run_gui() -> anyhow::Result<()> {
    let _instance = match NamedClaim::claim(INSTANCE_MUTEX) {
        Some(claim) => claim,
        None => {
            raise_existing_window();
            return Ok(());
        }
    };
    anyhow::ensure!(
        webview::runtime_available(),
        "Не найден Microsoft Edge WebView2 Runtime. Установите его: https://go.microsoft.com/fwlink/p/?LinkId=2124703"
    );
    let dir = prepare_assets()?;
    let url = file_url(&dir.join("index.html"));
    // The service lives while the app is open: start it now (no UAC, see svc_control).
    std::thread::spawn(crate::svc_control::start_service);

    let (call_tx, call_rx) = tokio::sync::mpsc::unbounded_channel::<HostCall>();
    let (out_tx, out_rx) = std_mpsc::channel::<String>();
    std::thread::Builder::new()
        .name("csqtt-gui-worker".into())
        .spawn(move || worker_main(call_rx, out_tx))
        .context("не удалось запустить рабочий поток")?;

    let options = Options {
        title: WINDOW_TITLE.to_string(),
        url,
        width: 460,
        height: 920,
        visible: true,
        user_agent: None,
        init_scripts: Vec::new(),
        intercept_filters: Vec::new(),
        profile: "gui".to_string(),
        timeout: Duration::from_millis(u64::from(u32::MAX)),
        tick: Some(TICK),
    };

    let mut page_ready = false;
    let mut script_id: u32 = 0;
    let mut handler = |event: WebEvent, ctl: &mut dyn WebCtl| -> Action {
        match event {
            WebEvent::NavigationStarting { url } => {
                if is_external_url(&url) {
                    open_external(&url);
                    return Action::Block;
                }
                page_ready = false;
            }
            WebEvent::WebMessage { json } => match parse_page_message(&json) {
                Some(PageMessage::Ready) => page_ready = true,
                Some(PageMessage::Call(call)) => {
                    let _ = call_tx.send(call);
                }
                None => {}
            },
            WebEvent::Tick if page_ready => {
                let mut batch: Vec<String> = Vec::new();
                while batch.len() < MAX_MESSAGES_PER_TICK {
                    match out_rx.try_recv() {
                        Ok(message) => batch.push(message),
                        Err(_) => break,
                    }
                }
                if !batch.is_empty() {
                    script_id = script_id.wrapping_add(1);
                    ctl.execute_script(script_id, &recv_script(&batch));
                }
            }
            _ => {}
        }
        Action::Continue
    };

    let result = webview::run(options, &mut handler);
    // Window closed: stop the service unless the tray keeps the app running.
    crate::svc_control::stop_if_last(crate::svc_control::TRAY_MUTEX);
    match result {
        Ok(_) | Err(WebViewError::Closed) | Err(WebViewError::Timeout) => Ok(()),
        Err(err) => Err(anyhow::anyhow!("{err}")),
    }
}

enum PageMessage {
    Ready,
    Call(HostCall),
}

/// Parses `{id, method, params}`; `{method: "ready"}` marks the page as able to receive.
fn parse_page_message(json: &str) -> Option<PageMessage> {
    let value: Value = serde_json::from_str(json).ok()?;
    let method = value.get("method")?.as_str()?.to_string();
    if method == "ready" {
        return Some(PageMessage::Ready);
    }
    let id = value.get("id")?.as_u64()?;
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    Some(PageMessage::Call(HostCall { id, method, params }))
}

/// One script that delivers a batch of ready JSON messages to the page.
fn recv_script(batch: &[String]) -> String {
    let mut script = String::from("window.__csqttHost&&window.__csqttHost.recv([");
    for (index, message) in batch.iter().enumerate() {
        if index > 0 {
            script.push(',');
        }
        // U+2028/2029 are legal in JSON but were line terminators in older JS string rules.
        for ch in message.chars() {
            match ch {
                '\u{2028}' => script.push_str("\\u2028"),
                '\u{2029}' => script.push_str("\\u2029"),
                other => script.push(other),
            }
        }
    }
    script.push_str("])");
    script
}

fn is_external_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Opens an http(s) link in the default browser. Anything else is ignored.
fn open_external(url: &str) {
    if !is_external_url(url) {
        return;
    }
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;
    let verb = webview::wide("open");
    let target = webview::wide(url);
    // SAFETY: both strings are NUL-terminated UTF-16 and outlive the call.
    unsafe {
        ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(target.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

fn raise_existing_window() {
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow};
    use windows::core::PCWSTR;
    let title = webview::wide(WINDOW_TITLE);
    // SAFETY: `title` is NUL-terminated UTF-16; the window handle is used only if found.
    unsafe {
        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

/// Folder with the UI files: `CSQTT_GUI_DIR` (for development) or the unpacked embedded copy.
fn prepare_assets() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CSQTT_GUI_DIR") {
        let dir = PathBuf::from(dir);
        anyhow::ensure!(dir.join("index.html").is_file(), "в CSQTT_GUI_DIR нет index.html");
        return Ok(dir);
    }
    let local = std::env::var_os("LOCALAPPDATA").context("переменная LOCALAPPDATA не задана")?;
    let dir = PathBuf::from(local).join("csqtt-win").join("gui");
    std::fs::create_dir_all(&dir).with_context(|| format!("не удалось создать {}", dir.display()))?;
    for (name, text) in ASSETS {
        let path = dir.join(name);
        let same = std::fs::read(&path).map(|old| old == text.as_bytes()).unwrap_or(false);
        if !same {
            std::fs::write(&path, text).with_context(|| format!("не удалось записать {}", path.display()))?;
        }
    }
    Ok(dir)
}

/// `C:\a b\я.html` -> `file:///C:/a%20b/%D1%8F.html`.
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file:///");
    for byte in text.trim_start_matches('/').bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => url.push(byte as char),
            other => url.push_str(&format!("%{other:02X}")),
        }
    }
    url
}

// ---------------------------------------------------------------------------------------
// Worker thread
// ---------------------------------------------------------------------------------------

type Out = std_mpsc::Sender<String>;

struct Proc {
    stdin: Option<tokio::process::ChildStdin>,
    kill: Arc<tokio::sync::Notify>,
}

type Procs = Arc<Mutex<HashMap<u64, Proc>>>;

fn emit(out: &Out, event: &str, data: Value) {
    let _ = out.send(json!({ "event": event, "data": data }).to_string());
}

fn worker_main(mut calls: tokio::sync::mpsc::UnboundedReceiver<HostCall>, out: Out) {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            emit(&out, "fatal", json!({ "message": format!("tokio: {err}") }));
            return;
        }
    };
    runtime.block_on(async move {
        let procs: Procs = Arc::new(Mutex::new(HashMap::new()));
        tokio::spawn(subscription_loop(out.clone()));
        while let Some(call) = calls.recv().await {
            let out = out.clone();
            let procs = procs.clone();
            tokio::spawn(async move {
                let id = call.id;
                let reply = match dispatch(call, &out, &procs).await {
                    Ok(result) => json!({ "id": id, "ok": true, "result": result }),
                    Err(err) => json!({ "id": id, "ok": false, "error": format!("{err:#}") }),
                };
                let _ = out.send(reply.to_string());
            });
        }
    });
}

/// Keeps one subscribed connection to the service and turns its stream into page events.
async fn subscription_loop(out: Out) {
    loop {
        if let Ok(mut client) = Client::connect().await {
            if client.send(&Request::Subscribe).await.is_ok() {
                emit(&out, "service", json!({ "running": true }));
                let (token_tx, mut token_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                loop {
                    tokio::select! {
                        Some(token) = token_rx.recv() => {
                            emit(&out, "captcha", json!({ "active": false }));
                            if client.send(&Request::CaptchaResult { token }).await.is_err() {
                                break;
                            }
                        }
                        response = client.recv() => match response {
                            Ok(Some(Response::Status { status })) => emit(&out, "status", json!({ "status": status })),
                            Ok(Some(Response::Log { line })) => emit(&out, "log", json!({ "line": line })),
                            Ok(Some(Response::LogBook { entries })) => emit(&out, "logbook", json!({ "entries": entries })),
                            Ok(Some(Response::Captcha { mode, redirect_uri, session_token })) => {
                                emit(&out, "captcha", json!({ "active": true }));
                                tokio::spawn(crate::tray::solve_captcha_task(mode, redirect_uri, session_token, token_tx.clone()));
                            }
                            Ok(Some(_)) => {}
                            Ok(None) | Err(_) => break,
                        }
                    }
                }
            }
        }
        emit(&out, "service", json!({ "running": false }));
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn dispatch(call: HostCall, out: &Out, procs: &Procs) -> anyhow::Result<Value> {
    match call.method.as_str() {
        "ipc" => {
            let request = call.params.get("request").cloned().context("нет поля request")?;
            tokio::time::timeout(IPC_TIMEOUT, ipc_call(request))
                .await
                .map_err(|_| anyhow::anyhow!("служба не ответила вовремя"))?
        }
        "version" => Ok(json!({ "version": env!("CARGO_PKG_VERSION") })),
        "list_processes" => Ok(tokio::task::spawn_blocking(list_processes).await?),
        "pick_exe" => Ok(tokio::task::spawn_blocking(pick_exe).await?.map_or(Value::Null, Value::String)),
        "open_url" => {
            let url = call.params.get("url").and_then(Value::as_str).context("нет поля url")?;
            anyhow::ensure!(is_external_url(url), "разрешены только ссылки http и https");
            open_external(url);
            Ok(Value::Null)
        }
        "parse_link" => {
            let text = call.params.get("link").and_then(Value::as_str).unwrap_or("");
            Ok(link_summary(text))
        }
        "service_setup" => {
            let code = tokio::task::spawn_blocking(|| run_svc_elevated("setup")).await??;
            anyhow::ensure!(code == 0, "установка службы завершилась с кодом {code}");
            Ok(Value::Null)
        }
        "service_uninstall" => {
            // `uninstall` stops the service (tunnel, routes and DNS are torn down) and deletes
            // it; `cleanup` removes what a crashed service could have left behind.
            let code = tokio::task::spawn_blocking(|| run_svc_elevated("uninstall")).await??;
            anyhow::ensure!(code == 0, "удаление службы завершилось с кодом {code}");
            let _ = tokio::task::spawn_blocking(|| run_svc_elevated("cleanup")).await;
            Ok(Value::Null)
        }
        "proc_start" => proc_start(call.id, &call.params, out, procs).await,
        "proc_input" => {
            let proc = call.params.get("proc").and_then(Value::as_u64).context("нет поля proc")?;
            let text = call.params.get("text").and_then(Value::as_str).context("нет поля text")?.to_string();
            let stdin = procs.lock().unwrap().get_mut(&proc).and_then(|p| p.stdin.take());
            let mut stdin = stdin.context("процесс не найден или уже завершён")?;
            let result = stdin.write_all(text.as_bytes()).await;
            let _ = stdin.flush().await;
            if let Some(entry) = procs.lock().unwrap().get_mut(&proc) {
                entry.stdin = Some(stdin);
            }
            result?;
            Ok(Value::Null)
        }
        "proc_kill" => {
            let proc = call.params.get("proc").and_then(Value::as_u64).context("нет поля proc")?;
            if let Some(entry) = procs.lock().unwrap().get(&proc) {
                entry.kill.notify_one();
            }
            Ok(Value::Null)
        }
        other => anyhow::bail!("неизвестный метод: {other}"),
    }
}

/// Decodes a `csqtt://` link (base64 of an AES-GCM payload) with the same parser the service
/// uses. The page gets only what it shows: never the password or the token itself.
/// `null` when the link is invalid.
fn link_summary(text: &str) -> Value {
    match csqtt_logic::csqtt_link::parse_csqtt_link(text) {
        Some(link) => json!({
            "host": link.host,
            "port": link.port,
            "hashes": link.hashes.len(),
            "has_token": !link.vk_access_token.is_empty(),
        }),
        None => Value::Null,
    }
}

/// Runs `csqtt-svc.exe <action>` (next to this exe) with the UAC prompt, hidden, and waits for
/// it. Returns its exit code.
fn run_svc_elevated(action: &str) -> anyhow::Result<u32> {
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::PCWSTR;

    let exe = std::env::current_exe()?;
    let svc = exe.with_file_name("csqtt-svc.exe");
    anyhow::ensure!(svc.is_file(), "не найден {}: он должен лежать рядом с csqtt.exe", svc.display());
    let verb = webview::wide("runas");
    let file = webview::wide(&svc.to_string_lossy());
    let params = webview::wide(action);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` and the strings it points to live for the whole call.
    if unsafe { ShellExecuteExW(&mut info) }.is_err() {
        anyhow::bail!("действие отменено или нет прав администратора");
    }
    anyhow::ensure!(!info.hProcess.is_invalid(), "не удалось запустить csqtt-svc.exe");
    let mut code = 1u32;
    // SAFETY: hProcess was returned by ShellExecuteExW (SEE_MASK_NOCLOSEPROCESS) and is
    // closed exactly once here.
    unsafe {
        if WaitForSingleObject(info.hProcess, INFINITE) == WAIT_OBJECT_0 {
            let _ = GetExitCodeProcess(info.hProcess, &mut code);
        }
        let _ = CloseHandle(info.hProcess);
    }
    Ok(code)
}

/// One request to the service on a fresh connection. `logs` (without follow) collects the
/// tail into `{type: "logs", lines: [...]}`; everything else returns the single response.
async fn ipc_call(request: Value) -> anyhow::Result<Value> {
    let request: Request = serde_json::from_value(request).context("неверный запрос")?;
    let is_logs = match &request {
        Request::Subscribe | Request::Logs { follow: true, .. } | Request::CaptchaResult { .. } => {
            anyhow::bail!("этот запрос недоступен из окна")
        }
        Request::Logs { .. } => true,
        _ => false,
    };
    let mut client = Client::connect().await.map_err(|_| anyhow::anyhow!("Служба csqtt-win не запущена"))?;
    client.send(&request).await?;
    if is_logs {
        let mut lines: Vec<String> = Vec::new();
        loop {
            match client.recv().await? {
                Some(Response::Log { line }) => lines.push(line),
                Some(Response::Error { message }) => anyhow::bail!(message),
                Some(_) | None => break,
            }
        }
        return Ok(json!({ "type": "logs", "lines": lines }));
    }
    match client.recv().await? {
        Some(Response::Error { message }) => anyhow::bail!(message),
        Some(response) => Ok(serde_json::to_value(response)?),
        None => anyhow::bail!("служба закрыла соединение без ответа"),
    }
}

/// Starts `csqtt <args>` hidden, streams its output as `proc_output` events (`{proc, text}`)
/// and resolves with `{code}` when it exits. The process id on the page is the call id.
async fn proc_start(id: u64, params: &Value, out: &Out, procs: &Procs) -> anyhow::Result<Value> {
    let args: Vec<String> = params
        .get("args")
        .and_then(Value::as_array)
        .context("нет поля args")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let subcommand = args.first().map(String::as_str).unwrap_or("");
    anyhow::ensure!(ALLOWED_SUBCOMMANDS.contains(&subcommand), "команда не разрешена: {subcommand}");

    let mut command = tokio::process::Command::new(std::env::current_exe()?);
    command.args(&args);
    if let Some(env) = params.get("env").and_then(Value::as_object) {
        for (key, value) in env {
            if key.starts_with("CSQTT_") {
                if let Some(value) = value.as_str() {
                    command.env(key, value);
                }
            }
        }
    }
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.creation_flags(CREATE_NO_WINDOW);
    command.kill_on_drop(true);
    let mut child = command.spawn().context("не удалось запустить csqtt")?;

    let kill = Arc::new(tokio::sync::Notify::new());
    procs.lock().unwrap().insert(id, Proc { stdin: child.stdin.take(), kill: kill.clone() });
    let stdout = child.stdout.take().map(|stream| tokio::spawn(pump(stream, id, out.clone())));
    let stderr = child.stderr.take().map(|stream| tokio::spawn(pump(stream, id, out.clone())));

    let status = tokio::select! {
        status = child.wait() => status,
        _ = kill.notified() => {
            let _ = child.kill().await;
            child.wait().await
        }
    };
    procs.lock().unwrap().remove(&id);
    for task in [stdout, stderr].into_iter().flatten() {
        let _ = task.await;
    }
    Ok(json!({ "code": status?.code().unwrap_or(-1) }))
}

async fn pump<R: tokio::io::AsyncRead + Unpin>(mut stream: R, proc: u64, out: Out) {
    let mut pending: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                pending.extend_from_slice(&buffer[..read]);
                let text = take_utf8(&mut pending);
                if !text.is_empty() {
                    emit(&out, "proc_output", json!({ "proc": proc, "text": text }));
                }
            }
        }
    }
    if !pending.is_empty() {
        let text = String::from_utf8_lossy(&pending).into_owned();
        emit(&out, "proc_output", json!({ "proc": proc, "text": text }));
    }
}

/// Takes the longest valid UTF-8 prefix out of `pending`, leaving an unfinished character
/// (at most 3 bytes) for the next read. Invalid bytes are replaced.
fn take_utf8(pending: &mut Vec<u8>) -> String {
    match std::str::from_utf8(pending) {
        Ok(text) => {
            let text = text.to_string();
            pending.clear();
            text
        }
        Err(err) => {
            let valid = err.valid_up_to();
            if err.error_len().is_none() && pending.len() - valid <= 3 {
                let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
                pending.drain(..valid);
                text
            } else {
                let text = String::from_utf8_lossy(pending).into_owned();
                pending.clear();
                text
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_encodes_spaces_and_unicode() {
        assert_eq!(file_url(Path::new(r"C:\a b\я.html")), "file:///C:/a%20b/%D1%8F.html");
    }

    #[test]
    fn take_utf8_keeps_split_character() {
        let bytes = "жук".as_bytes();
        let mut pending = bytes[..3].to_vec();
        assert_eq!(take_utf8(&mut pending), "ж");
        assert_eq!(pending.len(), 1);
        pending.extend_from_slice(&bytes[3..]);
        assert_eq!(take_utf8(&mut pending), "ук");
        assert!(pending.is_empty());
        let mut bad = vec![b'a', 0xff, b'b'];
        assert_eq!(take_utf8(&mut bad), "a\u{fffd}b");
    }

    #[test]
    fn page_messages() {
        assert!(matches!(parse_page_message(r#"{"method":"ready"}"#), Some(PageMessage::Ready)));
        match parse_page_message(r#"{"id":7,"method":"ipc","params":{"request":{"cmd":"status"}}}"#) {
            Some(PageMessage::Call(call)) => {
                assert_eq!(call.id, 7);
                assert_eq!(call.method, "ipc");
                assert_eq!(call.params["request"]["cmd"], "status");
            }
            _ => panic!("expected a call"),
        }
        assert!(parse_page_message("\"text\"").is_none());
    }

    #[test]
    fn recv_script_joins_and_escapes() {
        let script = recv_script(&["{\"a\":1}".to_string(), "{\"b\":\"x\u{2028}y\"}".to_string()]);
        assert_eq!(script, "window.__csqttHost&&window.__csqttHost.recv([{\"a\":1},{\"b\":\"x\\u2028y\"}])");
    }

    #[test]
    fn link_summary_decodes_encrypted_links_without_secrets() {
        let payload = csqtt_logic::csqtt_link::encrypt_csqtt_payload("connect?v=2&host=203.0.113.7&peer=46000&password=secret");
        let summary = link_summary(&format!("csqtt://{payload}"));
        assert_eq!(summary["host"], "203.0.113.7");
        assert_eq!(summary["port"], 46000);
        assert!(!summary.to_string().contains("secret"));
        assert!(link_summary("csqtt://connect?host=1.2.3.4").is_null());
        assert!(link_summary("").is_null());
    }

    #[test]
    fn external_urls_and_claim_names() {
        assert!(is_external_url("https://vk.com"));
        assert!(!is_external_url("file:///C:/x.html"));
        assert!(!is_external_url("javascript:alert(1)"));
        assert_eq!(captcha_claim_name("a"), captcha_claim_name("a"));
        assert_ne!(captcha_claim_name("a"), captcha_claim_name("b"));
    }
}

/// Running processes for the app-exclusion picker: unique by path (or name when the path
/// is not readable, e.g. system processes), sorted by name.
fn list_processes() -> Value {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows::core::PWSTR;
    let mut seen = std::collections::BTreeMap::<String, Value>::new();
    // SAFETY: plain ToolHelp/OpenProcess calls; every handle is closed, buffers outlive the calls.
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return json!([]) };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            let mut path = String::new();
            if entry.th32ProcessID > 4
                && let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, entry.th32ProcessID)
            {
                let mut buf = [0u16; 1024];
                let mut size = buf.len() as u32;
                if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok() {
                    path = String::from_utf16_lossy(&buf[..size as usize]);
                }
                let _ = CloseHandle(h);
            }
            if name.to_lowercase().ends_with(".exe") {
                let key = if path.is_empty() { name.to_lowercase() } else { path.to_lowercase() };
                seen.entry(key).or_insert_with(|| json!({ "name": name, "path": path }));
            }
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    let mut list: Vec<Value> = seen.into_values().collect();
    list.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_lowercase());
    Value::Array(list)
}

/// Standard "Open file" dialog filtered to .exe; returns the chosen path.
fn pick_exe() -> Option<String> {
    use windows::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows::core::{PCWSTR, PWSTR};
    let filter: Vec<u16> = "Программы (*.exe)\0*.exe\0\0".encode_utf16().collect();
    let title = webview::wide("Выберите программу");
    let mut buf = [0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: ofn points at buffers that live until the call returns.
    let ok = unsafe { GetOpenFileNameW(&mut ofn) }.as_bool();
    if !ok {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    (len > 0).then(|| String::from_utf16_lossy(&buf[..len]))
}
