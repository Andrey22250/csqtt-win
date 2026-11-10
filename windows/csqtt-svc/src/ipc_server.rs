//! IPC server: serves csqtt_ipc requests using the manager and the stores.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use csqtt_ipc::pipe::{Listener, ServerConn};
use csqtt_ipc::{
    LogBookEntry, Request, Response, StatusInfo, TunnelState, PROTOCOL_VERSION,
};
use csqtt_logic::log_book::{CoreLineClassifier, LogBook};
use csqtt_logic::settings::{export_config_json, SecretKey, Settings};

use crate::manager::{ManagerEvent, ManagerHandle, Request as MgrRequest, Status as MgrStatus};
use crate::store::{paths, secrets::SecretStore, settings_file};

pub const MAX_LOG_LINES: usize = 2000;

/// Events broadcast to connected streaming clients (Logs and Subscribe).
#[derive(Debug, Clone)]
pub enum ServerBroadcastEvent {
    Status(StatusInfo),
    Log(String),
    LogBook(Vec<LogBookEntry>),
    Captcha {
        mode: String,
        redirect_uri: String,
        session_token: String,
    },
}

/// Shared in-memory server state.
#[derive(Debug)]
pub struct ServerState {
    pub status: StatusInfo,
    pub logs: VecDeque<String>,
    pub book: LogBook,
    pub classifier: CoreLineClassifier,
    pub logging_enabled: bool,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            status: StatusInfo::default(),
            logs: VecDeque::with_capacity(MAX_LOG_LINES),
            book: LogBook::new(),
            classifier: CoreLineClassifier::new(),
            logging_enabled: settings_file::load_json::<Settings>(paths::settings_path())
                .map(|s| s.logging_enabled)
                .unwrap_or(true),
        }
    }
}

/// Updates StatusInfo with a manager Status event.
pub fn apply_status(info: &mut StatusInfo, status: &MgrStatus) {
    match status {
        MgrStatus::Stopped => {
            info.state = TunnelState::Stopped;
            info.message.clear();
            info.client_ip.clear();
            info.dns.clear();
            info.active_workers = 0;
        }
        MgrStatus::Starting => {
            info.state = TunnelState::Starting;
            info.message.clear();
            info.client_ip.clear();
            info.dns.clear();
        }
        MgrStatus::Connected { client_ip, dns } => {
            info.state = TunnelState::Connected;
            info.message.clear();
            info.client_ip = client_ip.clone();
            info.dns = dns.clone();
        }
        MgrStatus::Reconnecting { reason } => {
            info.state = TunnelState::Reconnecting;
            info.message = reason.clone();
        }
        MgrStatus::Paused => {
            info.state = TunnelState::Paused;
            info.message.clear();
        }
        MgrStatus::Failed { message } => {
            info.state = TunnelState::Failed;
            info.message = message.clone();
        }
    }
}

/// Appends a log line to the ring buffer, preserving at most MAX_LOG_LINES.
/// Feeds one log line into the keyed log book (Android TunnelManager logic). Lines the
/// classifier does not know (service messages, plain core output) get their own entry keyed
/// by the text, so an identical repeat bumps its counter instead of appending.
pub fn book_add_line(state: &mut ServerState, line: &str) -> bool {
    let updates = state.classifier.classify(line);
    let enabled = state.logging_enabled;
    if updates.is_empty() {
        let text = line.trim();
        if text.is_empty() {
            return false;
        }
        let lower = text.to_lowercase();
        let is_error = lower.contains("ошибка") || lower.contains("error") || lower.contains("fail");
        return state.book.update(&format!("line:{text}"), text, 90, is_error, None, enabled);
    }
    let mut changed = false;
    for u in &updates {
        changed |= state.book.update_from(u, enabled);
    }
    changed
}

pub fn book_entries(book: &LogBook) -> Vec<LogBookEntry> {
    book.entries()
        .iter()
        .map(|e| LogBookEntry {
            key: e.key.clone(),
            message: e.message.clone(),
            count: e.count,
            priority: e.priority,
            is_error: e.is_error,
        })
        .collect()
}

pub fn push_log_line(logs: &mut VecDeque<String>, line: String) {
    if logs.len() >= MAX_LOG_LINES {
        logs.pop_front();
    }
    logs.push_back(line);
}

/// Extracts the last `tail` log lines from the ring buffer.
pub fn get_tail_logs(logs: &VecDeque<String>, tail: usize) -> Vec<String> {
    let total = logs.len();
    let skip = total.saturating_sub(tail);
    logs.iter().skip(skip).cloned().collect()
}

/// Fallback response for manager when no client is subscribed to solve a captcha.
pub fn no_captcha_ui_response() -> MgrRequest {
    MgrRequest::CaptchaResult("error:no captcha ui".to_string())
}

/// Main entry point for the IPC server. Runs until `shutdown` is triggered.
pub async fn serve(
    manager: ManagerHandle,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(ServerState::default()));
    let (event_tx, _) = broadcast::channel::<ServerBroadcastEvent>(1024);
    let subscribers = Arc::new(AtomicUsize::new(0));

    // Background task to consume manager events and update shared state / broadcast
    let manager_events_task = {
        let state = state.clone();
        let manager = manager.clone();
        let event_tx = event_tx.clone();
        let subscribers = subscribers.clone();
        let shutdown = shutdown.clone();
        let mut manager_events = manager.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    ev = manager_events.recv() => {
                        match ev {
                            Ok(ManagerEvent::Status(status)) => {
                                let mut guard = state.lock().await;
                                apply_status(&mut guard.status, &status);
                                let current = guard.status.clone();
                                drop(guard);
                                let _ = event_tx.send(ServerBroadcastEvent::Status(current));
                            }
                            Ok(ManagerEvent::Stats { active, bytes_up, bytes_down }) => {
                                let mut guard = state.lock().await;
                                guard.status.active_workers = active;
                                guard.classifier.set_active_workers(u32::try_from(active).unwrap_or(0));
                                guard.status.bytes_up = bytes_up;
                                guard.status.bytes_down = bytes_down;
                                let current = guard.status.clone();
                                drop(guard);
                                let _ = event_tx.send(ServerBroadcastEvent::Status(current));
                            }
                            Ok(ManagerEvent::Log(line)) => {
                                let mut guard = state.lock().await;
                                push_log_line(&mut guard.logs, line.clone());
                                let changed = book_add_line(&mut guard, &line);
                                let entries = changed.then(|| book_entries(&guard.book));
                                drop(guard);
                                let _ = event_tx.send(ServerBroadcastEvent::Log(line));
                                if let Some(entries) = entries {
                                    let _ = event_tx.send(ServerBroadcastEvent::LogBook(entries));
                                }
                            }
                            Ok(ManagerEvent::CaptchaRequest { mode, redirect_uri, session_token }) => {
                                if subscribers.load(Ordering::SeqCst) == 0 {
                                    manager.send(no_captcha_ui_response());
                                } else {
                                    let _ = event_tx.send(ServerBroadcastEvent::Captcha {
                                        mode,
                                        redirect_uri,
                                        session_token,
                                    });
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        })
    };

    let mut listener = Listener::bind()?;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            res = listener.accept() => {
                match res {
                    Ok(conn) => {
                        let client_state = state.clone();
                        let client_manager = manager.clone();
                        let client_event_tx = event_tx.clone();
                        let client_subscribers = subscribers.clone();
                        let client_shutdown = shutdown.clone();

                        tokio::spawn(async move {
                            handle_client(
                                conn,
                                client_state,
                                client_manager,
                                client_event_tx,
                                client_subscribers,
                                client_shutdown,
                            ).await;
                        });
                    }
                    Err(err) => {
                        eprintln!("listener accept error: {err}");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        }
    }

    manager_events_task.abort();
    let _ = manager_events_task.await;
    Ok(())
}

async fn handle_client(
    mut conn: ServerConn,
    state: Arc<Mutex<ServerState>>,
    manager: ManagerHandle,
    event_tx: broadcast::Sender<ServerBroadcastEvent>,
    subscribers: Arc<AtomicUsize>,
    shutdown: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            req = conn.recv() => {
                let req = match req {
                    Ok(Some(r)) => r,
                    Ok(None) => break,
                    Err(_) => break,
                };

                match req {
                    Request::Logs { follow, tail } => {
                        let lines = {
                            let guard = state.lock().await;
                            get_tail_logs(&guard.logs, tail)
                        };

                        let mut failed = false;
                        for line in lines {
                            if conn.send(&Response::Log { line }).await.is_err() {
                                failed = true;
                                break;
                            }
                        }

                        if failed {
                            break;
                        }

                        if follow {
                            run_logs_follow(&mut conn, &event_tx, &shutdown).await;
                            break;
                        }
                        // End of a one-shot tail: the client reads Log lines until Ok.
                        if conn.send(&Response::Ok).await.is_err() {
                            break;
                        }
                    }
                    Request::Subscribe => {
                        let initial_status = {
                            let guard = state.lock().await;
                            guard.status.clone()
                        };

                        if conn.send(&Response::Status { status: initial_status }).await.is_err() {
                            break;
                        }

                        run_subscribe(
                            &mut conn,
                            &event_tx,
                            &manager,
                            &subscribers,
                            &shutdown,
                            &state,
                        ).await;
                        break;
                    }
                    one_shot => {
                        let resp = handle_one_shot(one_shot, &state, &manager).await;
                        if conn.send(&resp).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
}

async fn run_logs_follow(
    conn: &mut ServerConn,
    event_tx: &broadcast::Sender<ServerBroadcastEvent>,
    shutdown: &CancellationToken,
) {
    let mut rx = event_tx.subscribe();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            res = rx.recv() => {
                match res {
                    Ok(ServerBroadcastEvent::Log(line)) => {
                        if conn.send(&Response::Log { line }).await.is_err() {
                            break;
                        }
                    }
                    Ok(ServerBroadcastEvent::Status(status)) => {
                        if conn.send(&Response::Status { status }).await.is_err() {
                            break;
                        }
                    }
                    Ok(ServerBroadcastEvent::Captcha { .. }) | Ok(ServerBroadcastEvent::LogBook(_)) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            req = conn.recv() => {
                match req {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }
}

struct SubscriberGuard(Arc<AtomicUsize>);
impl Drop for SubscriberGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn run_subscribe(
    conn: &mut ServerConn,
    event_tx: &broadcast::Sender<ServerBroadcastEvent>,
    manager: &ManagerHandle,
    subscribers: &Arc<AtomicUsize>,
    shutdown: &CancellationToken,
    state: &Arc<Mutex<ServerState>>,
) {
    subscribers.fetch_add(1, Ordering::SeqCst);
    let _guard = SubscriberGuard(subscribers.clone());

    let mut rx = event_tx.subscribe();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            res = rx.recv() => {
                match res {
                    Ok(ServerBroadcastEvent::Status(status)) => {
                        if conn.send(&Response::Status { status }).await.is_err() {
                            break;
                        }
                    }
                    Ok(ServerBroadcastEvent::Log(line)) => {
                        if conn.send(&Response::Log { line }).await.is_err() {
                            break;
                        }
                    }
                    Ok(ServerBroadcastEvent::LogBook(entries)) => {
                        if conn.send(&Response::LogBook { entries }).await.is_err() {
                            break;
                        }
                    }
                    Ok(ServerBroadcastEvent::Captcha { mode, redirect_uri, session_token }) => {
                        if conn.send(&Response::Captcha { mode, redirect_uri, session_token }).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            req = conn.recv() => {
                match req {
                    Ok(Some(Request::CaptchaResult { token })) => {
                        manager.send(MgrRequest::CaptchaResult(token));
                        if conn.send(&Response::Ok).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(other_req)) => {
                        let resp = handle_one_shot(other_req, state, manager).await;
                        if conn.send(&resp).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }
}

async fn handle_one_shot(
    req: Request,
    state: &Arc<Mutex<ServerState>>,
    manager: &ManagerHandle,
) -> Response {
    match req {
        Request::Hello { .. } => Response::Hello {
            version: PROTOCOL_VERSION,
            service_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        Request::Connect => {
            let settings: Settings = match settings_file::load_json(paths::settings_path()) {
                Ok(s) => s,
                Err(e) => {
                    return Response::Error {
                        message: format!("не удалось загрузить настройки: {e}"),
                    };
                }
            };

            let secret_store = SecretStore::new(paths::secrets_path());
            let secret_lookup = |key: &str| -> Option<String> {
                secret_store.get(key).ok().flatten()
            };
            let data_dir = paths::data_dir();

            match crate::params::core_params_from_settings(&settings, &secret_lookup, &data_dir) {
                Ok(params) => {
                    manager.send(MgrRequest::Start(Box::new(params)));
                    Response::Ok
                }
                Err(err_msg) => Response::Error { message: err_msg },
            }
        }
        Request::Disconnect => {
            manager.send(MgrRequest::Stop { finish_vk_calls: true });
            Response::Ok
        }
        Request::Status => {
            let status = state.lock().await.status.clone();
            Response::Status { status }
        }
        Request::CaptchaResult { token } => {
            manager.send(MgrRequest::CaptchaResult(token));
            Response::Ok
        }
        Request::Import { text } => {
            let base_settings: Settings = settings_file::load_json(paths::settings_path()).unwrap_or_default();
            match crate::params::import_text(&text, &base_settings) {
                Ok(outcome) => {
                    if let Err(e) = settings_file::save_json(paths::settings_path(), &outcome.settings) {
                        return Response::Error {
                            message: format!("не удалось сохранить настройки: {e}"),
                        };
                    }
                    let secret_store = SecretStore::new(paths::secrets_path());
                    for (k, v) in outcome.secrets {
                        if let Err(e) = secret_store.set(&k, &v) {
                            return Response::Error {
                                message: format!("не удалось сохранить секрет '{k}': {e}"),
                            };
                        }
                    }
                    Response::Ok
                }
                Err(err_msg) => Response::Error { message: err_msg },
            }
        }
        Request::Export { include_secrets } => {
            let settings: Settings = settings_file::load_json(paths::settings_path()).unwrap_or_default();
            let secret_store = SecretStore::new(paths::secrets_path());
            let secrets_closure = |key: SecretKey| -> Option<String> {
                if include_secrets {
                    secret_store.get(key.storage_name()).ok().flatten()
                } else {
                    None
                }
            };
            let json = export_config_json(&settings, &secrets_closure);
            Response::Exported { json }
        }
        Request::GetSettings => {
            let settings: Settings = settings_file::load_json(paths::settings_path()).unwrap_or_default();
            match serde_json::to_value(&settings) {
                Ok(val) => Response::Settings { settings: val },
                Err(e) => Response::Error {
                    message: format!("ошибка сериализации настроек: {e}"),
                },
            }
        }
        Request::SetSettings { settings } => {
            match serde_json::from_value::<Settings>(settings) {
                Ok(val) => {
                    state.lock().await.logging_enabled = val.logging_enabled;
                    if let Err(e) = settings_file::save_json(paths::settings_path(), &val) {
                        Response::Error {
                            message: format!("не удалось сохранить настройки: {e}"),
                        }
                    } else {
                        Response::Ok
                    }
                }
                Err(e) => Response::Error {
                    message: format!("неверный формат настроек: {e}"),
                },
            }
        }
        Request::SetSecret { key, value } => {
            let secret_store = SecretStore::new(paths::secrets_path());
            match secret_store.set(&key, &value) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error {
                    message: format!("не удалось сохранить секрет: {e}"),
                },
            }
        }
        Request::LogBook => {
            let guard = state.lock().await;
            Response::LogBook { entries: book_entries(&guard.book) }
        }
        Request::ClearLogs => {
            let mut guard = state.lock().await;
            guard.book.clear();
            guard.logs.clear();
            Response::Ok
        }
        Request::Logs { .. } | Request::Subscribe => Response::Ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping_variants() {
        let mut info = StatusInfo::default();

        apply_status(&mut info, &MgrStatus::Starting);
        assert_eq!(info.state, TunnelState::Starting);
        assert_eq!(info.client_ip, "");
        assert_eq!(info.dns, "");
        assert_eq!(info.message, "");

        apply_status(
            &mut info,
            &MgrStatus::Connected {
                client_ip: "10.0.0.2".into(),
                dns: "1.1.1.1,8.8.8.8".into(),
            },
        );
        assert_eq!(info.state, TunnelState::Connected);
        assert_eq!(info.client_ip, "10.0.0.2");
        assert_eq!(info.dns, "1.1.1.1,8.8.8.8");

        apply_status(
            &mut info,
            &MgrStatus::Reconnecting {
                reason: "peer timeout".into(),
            },
        );
        assert_eq!(info.state, TunnelState::Reconnecting);
        assert_eq!(info.message, "peer timeout");

        apply_status(&mut info, &MgrStatus::Paused);
        assert_eq!(info.state, TunnelState::Paused);

        apply_status(
            &mut info,
            &MgrStatus::Failed {
                message: "fatal auth error".into(),
            },
        );
        assert_eq!(info.state, TunnelState::Failed);
        assert_eq!(info.message, "fatal auth error");

        info.active_workers = 12;
        apply_status(&mut info, &MgrStatus::Stopped);
        assert_eq!(info.state, TunnelState::Stopped);
        assert_eq!(info.active_workers, 0);
        assert_eq!(info.client_ip, "");
        assert_eq!(info.dns, "");
        assert_eq!(info.message, "");
    }

    #[test]
    fn ring_buffer_truncation() {
        let mut logs = VecDeque::new();
        for i in 0..2500 {
            push_log_line(&mut logs, format!("line {i}"));
        }
        assert_eq!(logs.len(), MAX_LOG_LINES);
        assert_eq!(logs.front().unwrap(), "line 500");
        assert_eq!(logs.back().unwrap(), "line 2499");

        let tail_5 = get_tail_logs(&logs, 5);
        assert_eq!(tail_5.len(), 5);
        assert_eq!(tail_5[0], "line 2495");
        assert_eq!(tail_5[4], "line 2499");

        let tail_all = get_tail_logs(&logs, 5000);
        assert_eq!(tail_all.len(), MAX_LOG_LINES);
        assert_eq!(tail_all[0], "line 500");

        let tail_zero = get_tail_logs(&logs, 0);
        assert!(tail_zero.is_empty());
    }

    #[test]
    fn fallback_captcha_response() {
        let req = no_captcha_ui_response();
        match req {
            MgrRequest::CaptchaResult(err) => assert_eq!(err, "error:no captcha ui"),
            _ => panic!("unexpected request type"),
        }
    }

    #[test]
    fn hello_response_format() {
        let resp = Response::Hello {
            version: PROTOCOL_VERSION,
            service_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        match resp {
            Response::Hello { version, service_version } => {
                assert_eq!(version, PROTOCOL_VERSION);
                assert_eq!(service_version, "0.1.0");
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn export_secrets_selector() {
        let mut mock_store = std::collections::HashMap::new();
        mock_store.insert(
            SecretKey::ConnectionPassword.storage_name().to_string(),
            "my_secret_pass".to_string(),
        );

        let query_secrets = |include_secrets: bool, key: SecretKey| -> Option<String> {
            if include_secrets {
                mock_store.get(key.storage_name()).cloned()
            } else {
                None
            }
        };

        let secret_included = query_secrets(true, SecretKey::ConnectionPassword);
        assert_eq!(secret_included.as_deref(), Some("my_secret_pass"));

        let secret_excluded = query_secrets(false, SecretKey::ConnectionPassword);
        assert_eq!(secret_excluded, None);
    }

    #[test]
    fn set_settings_deserialization() {
        let default_settings = Settings::default();
        let valid_json = serde_json::to_value(&default_settings).unwrap();
        let parsed = serde_json::from_value::<Settings>(valid_json);
        assert!(parsed.is_ok());

        let invalid_json = serde_json::json!({
            "active_profile": "not_an_integer"
        });
        let failed = serde_json::from_value::<Settings>(invalid_json);
        assert!(failed.is_err());
    }
}
