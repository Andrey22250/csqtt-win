//! Host state machine: the Windows counterpart of TunnelManager.kt + TunVpnService.kt.
//!
//! One task owns everything (adapter, routes, core process), so there are no locks:
//! requests, core output, process exit, network changes and timers are all events of
//! a single `select!` loop. Behaviour and timings follow the Android host
//! (see work notes `manager-spec.md`); Windows-specific parts:
//! - the service owns the Wintun adapter and hands its name to the core over a named pipe;
//! - the core's sockets are pinned to the physical interface (CSQTT_BYPASS_IFINDEX /
//!   CSQTT_BYPASS_LOCAL_IP), so a change of the physical interface restarts the core;
//! - routes 0/1 + 128/1 (+ IPv6 halves) and an NRPT rule are kept across core restarts
//!   (kill-switch, like Android keeps its VPN interface) and removed on stop.

use csqtt_logic::tunnel_log_policy::LogLevel;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use csqtt_logic::tunnel_event_parser::{self, Event};
use csqtt_logic::worker_recovery_policy::WorkerRecoveryPolicy;
use tokio::sync::{broadcast, mpsc};

use crate::core::args::{self, CoreCommand, CoreParams};
use crate::core::handoff;
use crate::core::process::{CoreExit, CoreProcess};
use crate::net::adapter::TunAdapter;
use crate::net::routes::{self, NetworkWatcher, PhysicalRoute, RouteTable};
use crate::net::{dns, ipconfig};
use crate::vk_auto_calls::VkAutoCalls;

const WORKER_ZERO_CONFIRMATION: Duration = Duration::from_millis(4_000);
const PANEL_SERVER_RESTART_DELAY: Duration = Duration::from_millis(4_000);
const CRASH_RESTART_STABLE: Duration = Duration::from_millis(30_000);
const CRASH_RESTART_BASE_MS: u64 = 2_000;
const CRASH_RESTART_MAX_MS: u64 = 60_000;
const TUN_LOSS_REBUILD_WINDOW: Duration = Duration::from_millis(60_000);
const NETWORK_LOSS_GRACE: Duration = Duration::from_millis(2_000);
const NETWORK_SWAP_SETTLE: Duration = Duration::from_millis(4_000);
const RETAINED_CONFIG_DELAY: Duration = Duration::from_millis(250);
const HANDOFF_ATTEMPTS: u32 = 60;
const HANDOFF_ACK_TIMEOUT: Duration = Duration::from_millis(3_000);
const TUNNEL_METRIC: u32 = 5;
/// Destination used to find the physical interface (Yandex DNS, the Android fallback DNS).
const PROBE_DEST: IpAddr = IpAddr::V4(Ipv4Addr::new(77, 88, 8, 8));
const TIMESTAMP_LEN: usize = "2006/01/02 15:04:05".len();

/// Static configuration of the manager.
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub core_exe: PathBuf,
    pub wintun_dir: PathBuf,
    pub routes_journal: PathBuf,
    pub adapter_name: String,
    pub tun_pipe: String,
    pub mtu: u32,
}

impl ManagerConfig {
    pub fn new(core_exe: PathBuf, wintun_dir: PathBuf, routes_journal: PathBuf) -> Self {
        Self {
            core_exe,
            wintun_dir,
            routes_journal,
            adapter_name: "CSQTT".into(),
            tun_pipe: "csqtt_tun_uds".into(),
            mtu: 1500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Stopped,
    Starting,
    Connected { client_ip: String, dns: String },
    Reconnecting { reason: String },
    Paused,
    Failed { message: String },
}

#[derive(Debug, Clone)]
pub enum ManagerEvent {
    Status(Status),
    /// A message of the service itself.
    Log(String),
    /// A raw line of the core: kept in the raw log, shown only when the log book knows it.
    CoreLine(String),
    /// A keyed entry of the live log view (core events: ready, DTLS, ...).
    Keyed(csqtt_logic::log_book::LogUpdate),
    Stats { active: i32, bytes_up: i64, bytes_down: i64 },
    /// The core needs a captcha solved; answer with `Request::CaptchaResult`.
    CaptchaRequest { mode: String, redirect_uri: String, session_token: String },
}

#[derive(Debug)]
pub enum Request {
    Start(Box<CoreParams>),
    Stop { finish_vk_calls: bool },
    /// Token, or `error:<reason>`; written to the core as `CAPTCHA_RESULT|...`.
    CaptchaResult(String),
    /// Stop everything and end the manager task.
    Shutdown,
}

#[derive(Clone)]
pub struct ManagerHandle {
    requests: mpsc::UnboundedSender<Request>,
    events: broadcast::Sender<ManagerEvent>,
}

impl ManagerHandle {
    /// Returns false if the manager task has ended.
    pub fn send(&self, request: Request) -> bool {
        self.requests.send(request).is_ok()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ManagerEvent> {
        self.events.subscribe()
    }
}

/// Clean leftovers of a crashed previous run, then start the manager task.
/// Must be called inside a tokio runtime.
pub fn spawn(config: ManagerConfig) -> (ManagerHandle, tokio::task::JoinHandle<()>) {
    let (req_tx, req_rx) = mpsc::unbounded_channel();
    let (events, _) = broadcast::channel(512);
    let handle = ManagerHandle { requests: req_tx, events: events.clone() };
    let manager = Manager::new(config, events);
    let task = tokio::spawn(manager.run(req_rx));
    (handle, task)
}

/// `TUNCONF:<ip>:<dns>[:...]` as sent by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunConf {
    pub client_ip: Ipv4Addr,
    pub dns: Vec<IpAddr>,
}

impl TunConf {
    pub fn parse(config: &str) -> Option<Self> {
        let value = config.strip_prefix("TUNCONF:")?;
        let mut parts = value.splitn(3, ':');
        let client_ip = parts.next()?.trim().parse().ok()?;
        let dns: Vec<IpAddr> = parts
            .next()
            .unwrap_or_default()
            .split([',', ' ', ';'])
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        let dns = if dns.is_empty() { vec![PROBE_DEST] } else { dns };
        Some(Self { client_ip, dns })
    }

    fn dns_text(&self) -> String {
        self.dns.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
    }
}

/// Removes the Go-style `YYYY/MM/DD HH:MM:SS[.frac] ` prefix (TunnelManager.kt TIMESTAMP regex).
pub fn strip_timestamp(line: &str) -> &str {
    let b = line.as_bytes();
    if b.len() <= TIMESTAMP_LEN {
        return line;
    }
    let shape_ok = b[..TIMESTAMP_LEN].iter().enumerate().all(|(i, c)| match i {
        4 | 7 => *c == b'/',
        10 => *c == b' ',
        13 | 16 => *c == b':',
        _ => c.is_ascii_digit(),
    });
    if !shape_ok {
        return line;
    }
    let mut i = TIMESTAMP_LEN;
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    if b.get(i) == Some(&b' ') { &line[i + 1..] } else { line }
}

/// Crash restart delay (TunnelManager.kt: 2 s << (clamp(streak,1,6)-1), max 60 s).
pub fn crash_restart_delay(streak: u32) -> Duration {
    let shift = streak.clamp(1, 6) - 1;
    Duration::from_millis((CRASH_RESTART_BASE_MS << shift).min(CRASH_RESTART_MAX_MS))
}

/// `CAPTCHA_SOLVE|mode|uri|token` or `CAPTCHA_SOLVE|uri|token` (mode "selected").
pub fn parse_captcha_solve(line: &str) -> Option<(String, String, String)> {
    let payload = line.split_once("CAPTCHA_SOLVE|")?.1;
    let parts: Vec<&str> = payload.split('|').collect();
    match parts.as_slice() {
        [mode, uri, token, ..] => Some((mode.to_string(), uri.to_string(), token.to_string())),
        [uri, token] => Some(("selected".into(), uri.to_string(), token.to_string())),
        _ => None,
    }
}

fn critical_auth_reason(line: &str) -> Option<String> {
    // Order and texts as in TunnelManager.kt startLogReader.
    if line.contains("FATAL_AUTH") || line.contains("AUTH_RETRY") {
        let reason = if line.contains("неверный пароль") {
            "Неверный пароль подключения"
        } else if line.contains("истёк") {
            "Срок действия пароля истёк"
        } else if line.contains("деактивирован") {
            "Аккаунт деактивирован"
        } else {
            "Ошибка авторизации"
        };
        return Some(format!("🔒 {reason}. Воркеры остановлены."));
    }
    if line.contains("FATAL_PROTOCOL") {
        return Some(
            "🔁 Клиент и сервер используют разные версии протокола. Выполните деплой из этой версии приложения. Воркеры остановлены."
                .into(),
        );
    }
    None
}

enum Wake {
    Request(Option<Request>),
    Line(Option<String>),
    Exit(CoreExit),
    Handoff(u64, anyhow::Result<()>),
    Network,
    Timer,
}

struct Manager {
    cfg: ManagerConfig,
    events: broadcast::Sender<ManagerEvent>,
    status: Status,
    desired: Option<CoreParams>,
    adapter: Option<TunAdapter>,
    routes: RouteTable,
    applied: Option<TunConf>,
    config: Option<String>,
    physical: Option<PhysicalRoute>,
    proc: Option<CoreProcess>,
    lines: Option<mpsc::UnboundedReceiver<String>>,
    /// Bumped on every core start; stale handoff results are ignored.
    core_seq: u64,
    started_at: Option<Instant>,
    last_generation: i64,
    crash_streak: u32,
    active: i32,
    recovery: WorkerRecoveryPolicy,
    paused: bool,
    last_tun_loss: Option<Instant>,
    restart_at: Option<(Instant, String)>,
    handoff_at: Option<Instant>,
    zero_check_at: Option<(Instant, i32)>,
    panel_restart_at: Option<Instant>,
    net_loss_at: Option<Instant>,
    net_swap_at: Option<Instant>,
    handoff_tx: mpsc::UnboundedSender<(u64, anyhow::Result<()>)>,
    handoff_rx: mpsc::UnboundedReceiver<(u64, anyhow::Result<()>)>,
    net_rx: Option<mpsc::UnboundedReceiver<()>>,
    _watcher: Option<NetworkWatcher>,
    /// auto_api: calls created by the service (VkAutoCallsManager port).
    auto_calls: VkAutoCalls,
    /// Hashes and redistribution flag resolved for the current `desired` (auto_api only).
    auto_hashes: Option<(Vec<String>, bool)>,
}

impl Manager {
    fn new(cfg: ManagerConfig, events: broadcast::Sender<ManagerEvent>) -> Self {
        let (handoff_tx, handoff_rx) = mpsc::unbounded_channel();
        Self {
            routes: RouteTable::new(&cfg.routes_journal),
            cfg,
            events,
            status: Status::Stopped,
            desired: None,
            adapter: None,
            applied: None,
            config: None,
            physical: None,
            proc: None,
            lines: None,
            core_seq: 0,
            started_at: None,
            last_generation: 0,
            crash_streak: 0,
            active: 0,
            recovery: WorkerRecoveryPolicy::new(),
            paused: false,
            last_tun_loss: None,
            restart_at: None,
            handoff_at: None,
            zero_check_at: None,
            panel_restart_at: None,
            net_loss_at: None,
            net_swap_at: None,
            handoff_tx,
            handoff_rx,
            net_rx: None,
            _watcher: None,
            auto_calls: VkAutoCalls::new(),
            auto_hashes: None,
        }
    }

    fn keyed(&self, key: &str, message: String, priority: i32, is_error: bool, level: Option<LogLevel>) {
        let update = csqtt_logic::log_book::LogUpdate { key: key.to_string(), message, priority, is_error, level };
        let _ = self.events.send(ManagerEvent::Keyed(update));
    }

    fn log(&self, message: impl Into<String>) {
        let _ = self.events.send(ManagerEvent::Log(message.into()));
    }

    fn set_status(&mut self, status: Status) {
        if self.status != status {
            self.status = status.clone();
            let _ = self.events.send(ManagerEvent::Status(status));
        }
    }

    /// Startup cleanup ("служба при старте чистит хвосты").
    fn cleanup_leftovers(&mut self) {
        match RouteTable::cleanup_stale(&self.cfg.routes_journal) {
            Ok(0) => {}
            Ok(n) => self.log(format!("[СЛУЖБА] Удалено старых маршрутов: {n}")),
            Err(e) => self.log(format!("[СЛУЖБА] Очистка маршрутов: {e:#}")),
        }
        if let Err(e) = dns::nrpt_remove() {
            self.log(format!("[СЛУЖБА] Очистка NRPT: {e:#}"));
        }
        match TunAdapter::delete_stale(&self.cfg.adapter_name, &self.cfg.wintun_dir) {
            Ok(true) => self.log("[СЛУЖБА] Удалён старый адаптер Wintun"),
            Ok(false) => {}
            Err(e) => self.log(format!("[СЛУЖБА] Очистка адаптера: {e:#}")),
        }
    }

    async fn run(mut self, mut req_rx: mpsc::UnboundedReceiver<Request>) {
        self.cleanup_leftovers();
        let (net_tx, net_rx) = mpsc::unbounded_channel();
        match NetworkWatcher::start(net_tx) {
            Ok(w) => {
                self._watcher = Some(w);
                self.net_rx = Some(net_rx);
            }
            Err(e) => self.log(format!("[СЕТЬ] Не удалось подписаться на изменения сети: {e:#}")),
        }

        loop {
            let deadline = self.next_deadline();
            let wake = {
                let proc = &mut self.proc;
                let lines = &mut self.lines;
                let net_rx = &mut self.net_rx;
                let handoff_rx = &mut self.handoff_rx;
                tokio::select! {
                    r = req_rx.recv() => Wake::Request(r),
                    l = async { match lines { Some(rx) => rx.recv().await, None => std::future::pending().await } } => Wake::Line(l),
                    e = async { match proc { Some(p) => p.wait().await, None => std::future::pending().await } } => Wake::Exit(e),
                    Some((seq, res)) = handoff_rx.recv() => Wake::Handoff(seq, res),
                    Some(()) = async { match net_rx { Some(rx) => rx.recv().await, None => std::future::pending().await } } => Wake::Network,
                    _ = async { match deadline { Some(d) => tokio::time::sleep_until(d.into()).await, None => std::future::pending().await } } => Wake::Timer,
                }
            };
            match wake {
                Wake::Request(None) | Wake::Request(Some(Request::Shutdown)) => {
                    self.stop(false, Status::Stopped).await;
                    // Calls created by the service (auto_api) must not outlive it.
                    if self.auto_calls.has_active_calls() {
                        let events = self.events.clone();
                        let log = move |line: &str| {
                            let _ = events.send(ManagerEvent::Log(line.to_string()));
                        };
                        self.auto_calls.finish_all(&log).await;
                    }
                    break;
                }
                Wake::Request(Some(request)) => self.on_request(request).await,
                Wake::Line(Some(line)) => self.on_line(&line).await,
                Wake::Line(None) => self.lines = None,
                Wake::Exit(exit) => self.on_exit(exit).await,
                Wake::Handoff(seq, res) => self.on_handoff(seq, res),
                Wake::Network => self.on_network_change(),
                Wake::Timer => self.on_timer().await,
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        [
            self.restart_at.as_ref().map(|(t, _)| *t),
            self.handoff_at,
            self.zero_check_at.map(|(t, _)| t),
            self.panel_restart_at,
            self.net_loss_at,
            self.net_swap_at,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    async fn on_request(&mut self, request: Request) {
        match request {
            Request::Start(params) => {
                if self.desired.as_ref() == Some(&*params) && self.proc.is_some() {
                    return;
                }
                let switching = self.proc.is_some();
                if self.desired.as_ref() != Some(&*params) {
                    self.auto_hashes = None;
                }
                self.desired = Some(*params);
                self.crash_streak = 0;
                self.restart_at = None;
                if switching {
                    self.restart_core("Переключение параметров").await;
                } else {
                    self.set_status(Status::Starting);
                    self.start_core().await;
                }
            }
            Request::Stop { finish_vk_calls } => self.stop(finish_vk_calls, Status::Stopped).await,
            Request::CaptchaResult(result) => {
                let line = result.replace(['\r', '\n'], "");
                self.send_core(CoreCommand::CaptchaResult(line)).await;
            }
            Request::Shutdown => unreachable!("handled in run"),
        }
    }

    async fn send_core(&mut self, command: CoreCommand) {
        if let Some(proc) = self.proc.as_mut()
            && let Err(e) = proc.send(command).await
        {
            self.log(format!("[СЛУЖБА] Запись в stdin ядра: {e:#}"));
        }
    }

    fn ensure_adapter(&mut self) -> anyhow::Result<()> {
        if self.adapter.is_none() {
            self.adapter = Some(TunAdapter::create(&self.cfg.adapter_name, &self.cfg.wintun_dir)?);
        }
        Ok(())
    }

    fn tunnel_luid(&self) -> Option<u64> {
        self.adapter.as_ref().map(|a| a.luid)
    }

    fn refresh_physical(&mut self) -> Option<PhysicalRoute> {
        match routes::best_physical_route(PROBE_DEST, self.tunnel_luid()) {
            Ok(route) => Some(route),
            Err(e) => {
                self.log(format!("[СЕТЬ] Физический интерфейс не найден: {e:#}"));
                None
            }
        }
    }

    /// startProcessLocked: validate, build args/env, spawn, bootstrap, retained-config handoff.
    async fn start_core(&mut self) {
        let Some(mut params) = self.desired.clone() else { return };
        if params.vk_hash_mode == csqtt_logic::constants::vk_auto_hash::MODE_AUTO_API {
            match self.resolve_auto_api(&params).await {
                Some((hashes, redistribute)) => {
                    params.vk_hashes = hashes;
                    params.allow_hash_redistribution = redistribute;
                    // TunnelService.kt ResolvedVkHashes for auto_api carries no token.
                    params.vk_access_token.clear();
                }
                None => {
                    return self
                        .fail("[ПОДКЛЮЧЕНИЕ] В ссылке и настройках нет действующих VK-хэшей или VK-токена.")
                        .await;
                }
            }
        }
        let js_hash_mode = params.vk_hash_mode == "auto_js";
        let has_hashes = params.vk_hashes.iter().any(|h| !h.trim().is_empty());
        if !has_hashes && !js_hash_mode {
            return self.fail("Ошибка: Хеш не указан").await;
        }
        if js_hash_mode && params.vk_access_token.trim().is_empty() {
            return self.fail("Ошибка: VK access token не указан").await;
        }
        if !self.cfg.core_exe.exists() {
            return self.fail("Ошибка: Бинарный файл не найден").await;
        }
        if let Err(e) = self.ensure_adapter() {
            return self.fail(&format!("Ошибка: адаптер Wintun: {e:#}")).await;
        }

        self.physical = self.refresh_physical();
        let local_ip = self
            .physical
            .as_ref()
            .and_then(|p| routes::source_address(p.luid, PROBE_DEST).ok());

        let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.last_generation =
            csqtt_logic::tunnel_lifecycle_state::next_tunnel_generation_id(now_secs, self.last_generation);
        params.generation_id = self.last_generation as u64;
        params.session_salt = csqtt_logic::tunnel_session_id::new_tunnel_session_id();

        let argv = args::build_args(&params, &self.cfg.tun_pipe);
        let mut env = args::build_env(&params, self.physical.as_ref().map(|p| p.if_index));
        if let Some(ip) = local_ip {
            env.push(("CSQTT_BYPASS_LOCAL_IP".into(), ip.to_string()));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        let mut proc = match CoreProcess::spawn(&self.cfg.core_exe, &argv, &env, tx).await {
            Ok(p) => p,
            Err(e) => return self.schedule_crash_restart(&format!("запуск ядра: {e:#}")),
        };
        if js_hash_mode {
            let bootstrap = serde_json::json!({ "token": params.vk_access_token }).to_string();
            let encoded = base64::engine::general_purpose::STANDARD.encode(bootstrap.as_bytes());
            if let Err(e) = proc.send(CoreCommand::VkJsBootstrap(encoded)).await {
                proc.terminate(false).await;
                return self.fail(&format!("Ошибка: VK_JS_BOOTSTRAP: {e:#}")).await;
            }
        }
        self.core_seq += 1;
        self.log(format!("[СЛУЖБА] Ядро запущено (pid {})", proc.pid));
        self.proc = Some(proc);
        self.lines = Some(rx);
        self.started_at = Some(Instant::now());
        self.active = 0;
        self.recovery.reset();
        self.zero_check_at = None;
        self.panel_restart_at = None;
        // A retained TUNCONF means the network is already configured: hand the adapter over again.
        if self.applied.is_some() {
            self.handoff_at = Some(Instant::now() + RETAINED_CONFIG_DELAY);
        }
    }

    /// auto_api: create VK calls with the token (once per `desired`), like
    /// TunnelService.resolveAutoHashes -> VkAutoCallsManager.startAutoCalls.
    async fn resolve_auto_api(&mut self, params: &CoreParams) -> Option<(Vec<String>, bool)> {
        if let Some(resolved) = &self.auto_hashes {
            return Some(resolved.clone());
        }
        if params.vk_access_token.trim().is_empty() {
            return None;
        }
        let events = self.events.clone();
        let log = move |line: &str| {
            let _ = events.send(ManagerEvent::Log(line.to_string()));
        };
        if self.auto_calls.has_active_calls() {
            self.auto_calls.finish_all(&log).await;
        }
        let workers = i32::try_from(params.workers_per_hash).unwrap_or(i32::MAX);
        let mut invalid_token = false;
        let result = self.auto_calls.start(&params.vk_access_token, workers, &log, &mut invalid_token).await;
        if invalid_token {
            self.log("[VK] Токен VK недействителен — войдите заново (csqtt vk-login)");
        }
        match result {
            Ok(Some(started)) => {
                let hashes: Vec<String> = started
                    .hashes
                    .split(',')
                    .map(str::trim)
                    .filter(|h| !h.is_empty())
                    .map(str::to_string)
                    .collect();
                if hashes.is_empty() {
                    return None;
                }
                let resolved = (hashes, started.needs_worker_redistribution());
                self.auto_hashes = Some(resolved.clone());
                Some(resolved)
            }
            Ok(None) => None,
            Err(e) => {
                self.log(format!("[VK] Создание звонков: {e:#}"));
                None
            }
        }
    }

    /// TunnelManager.handleUnavailableVkCall: auto modes replace the hash (new calls / new core).
    async fn on_call_unavailable(&mut self, hash: &str, code: i32) {
        let Some(mode) = self.desired.as_ref().map(|p| p.vk_hash_mode.clone()) else { return };
        let trimmed = hash.trim();
        let display = if trimmed.chars().count() <= 10 {
            trimmed.to_string()
        } else {
            format!("{}…", trimmed.chars().take(10).collect::<String>())
        };
        let reason = if code == tunnel_event_parser::CALL_NOT_FOUND_CODE {
            "звонок не найден".to_string()
        } else {
            format!("код {code}")
        };
        if !csqtt_logic::vk_mode_policy::should_replace_unavailable_vk_hash(&mode) {
            self.log(format!("[VK] Хеш \"{display}\" недоступен · ошибка {code} — {reason}"));
            return;
        }
        self.log(format!("[VK] Хеш \"{display}\" устарел · ошибка {code} — {reason} · создаём новый ✓"));
        if mode == csqtt_logic::constants::vk_auto_hash::MODE_AUTO_API {
            self.auto_hashes = None;
        }
        self.restart_core("Замена VK-хеша").await;
    }

    async fn stop_core(&mut self, finish_vk_calls: bool) {
        self.handoff_at = None;
        if let Some(mut proc) = self.proc.take() {
            let exit = proc.terminate(finish_vk_calls).await;
            self.log(format!("[СЛУЖБА] Ядро остановлено: {exit:?}"));
        }
        // Flush remaining output into the log.
        if let Some(mut rx) = self.lines.take() {
            while let Ok(line) = rx.try_recv() {
                // Same path as live output: events are not log text, other lines are core lines.
                let line = strip_timestamp(&line);
                if tunnel_event_parser::parse(line).is_none() && !csqtt_logic::tunnel_log_policy::is_internal_recovery(line) {
                    let _ = self.events.send(ManagerEvent::CoreLine(line.to_string()));
                }
            }
        }
        self.active = 0;
    }

    /// requestTransportRestart / hot switch: kill and start immediately, keep the network.
    async fn restart_core(&mut self, reason: &str) {
        self.set_status(Status::Reconnecting { reason: reason.into() });
        self.log(format!("[СЛУЖБА] Перезапуск ядра: {reason}"));
        // auto_api: new calls are created through the VK API. The kill-switch routes stay up
        // during a restart, so the requests must go out while the old core still carries
        // traffic; after stop_core they would be routed into a dead tunnel.
        if self.proc.is_some()
            && self.auto_hashes.is_none()
            && let Some(params) = self.desired.clone()
            && params.vk_hash_mode == csqtt_logic::constants::vk_auto_hash::MODE_AUTO_API
        {
            let _ = self.resolve_auto_api(&params).await;
        }
        self.stop_core(false).await;
        self.start_core().await;
    }

    async fn stop(&mut self, finish_vk_calls: bool, final_status: Status) {
        self.desired = None;
        self.restart_at = None;
        self.zero_check_at = None;
        self.panel_restart_at = None;
        self.net_loss_at = None;
        self.net_swap_at = None;
        self.paused = false;
        self.stop_core(finish_vk_calls).await;
        self.auto_hashes = None;
        // Routes and NRPT go first: with the core stopped they lead into a dead tunnel, and
        // the VK API requests below (and everything else on the machine) would hang.
        self.teardown_network();
        if finish_vk_calls && self.auto_calls.has_active_calls() {
            let events = self.events.clone();
            let log = move |line: &str| {
                let _ = events.send(ManagerEvent::Log(line.to_string()));
            };
            self.auto_calls.finish_all(&log).await;
        }
        self.config = None;
        self.adapter = None; // dropping the created adapter deletes it
        self.set_status(final_status);
    }

    async fn fail(&mut self, message: &str) {
        self.log(message.to_string());
        self.stop(false, Status::Failed { message: message.into() }).await;
    }

    fn schedule_crash_restart(&mut self, reason: &str) {
        let stable = self.started_at.is_some_and(|t| t.elapsed() >= CRASH_RESTART_STABLE);
        self.crash_streak = if stable { 0 } else { self.crash_streak + 1 };
        let delay = crash_restart_delay(self.crash_streak);
        self.log(format!("[СЛУЖБА] {reason}; перезапуск через {} с", delay.as_secs()));
        self.set_status(Status::Reconnecting { reason: reason.into() });
        self.restart_at = Some((Instant::now() + delay, reason.into()));
    }

    async fn on_exit(&mut self, exit: CoreExit) {
        self.proc = None;
        if let Some(mut rx) = self.lines.take() {
            while let Ok(line) = rx.try_recv() {
                self.on_line(&line).await;
            }
        }
        self.handoff_at = None;
        if self.desired.is_none() {
            self.set_status(Status::Stopped);
            return;
        }
        let reason = match exit {
            CoreExit::Normal => "ядро завершилось".to_string(),
            CoreExit::Fatal => "ядро завершилось с ошибкой".to_string(),
            CoreExit::TunLost => "ядро потеряло TUN".to_string(),
            CoreExit::Other(code) => format!("ядро завершилось с кодом {code}"),
            CoreExit::Killed => "ядро убито".to_string(),
        };
        if exit == CoreExit::TunLost {
            // Rebuild the adapter before the next start (TunVpnService forceRebuild).
            self.rebuild_adapter();
        }
        self.schedule_crash_restart(&reason);
    }

    async fn on_line(&mut self, raw: &str) {
        let line = strip_timestamp(raw);
        if let Some(event) = tunnel_event_parser::parse(line) {
            return self.on_event(event).await;
        }
        if line.contains("CAPTCHA_SOLVE|") {
            match parse_captcha_solve(line) {
                Some((mode, redirect_uri, session_token)) => {
                    self.request_captcha(mode, redirect_uri, session_token).await
                }
                None => {
                    self.send_core(CoreCommand::CaptchaResult("error:invalid CAPTCHA_SOLVE format".into()))
                        .await
                }
            }
            return;
        }
        if let Some(reason) = critical_auth_reason(line) {
            return self.fail(&reason).await;
        }
        if !csqtt_logic::tunnel_log_policy::is_internal_recovery(line) {
            let _ = self.events.send(ManagerEvent::CoreLine(line.to_string()));
        }
    }

    async fn request_captcha(&mut self, mode: String, redirect_uri: String, session_token: String) {
        if !csqtt_logic::captcha_uri_policy::is_allowed(&redirect_uri) {
            self.log("[КАПЧА] Недопустимый адрес captcha");
            self.send_core(CoreCommand::CaptchaResult("error:untrusted captcha uri".into())).await;
            return;
        }
        if self.events.receiver_count() == 0 {
            self.send_core(CoreCommand::CaptchaResult("error:no captcha ui".into())).await;
            return;
        }
        let _ = self.events.send(ManagerEvent::CaptchaRequest { mode, redirect_uri, session_token });
    }

    async fn on_event(&mut self, event: Event) {
        match event {
            Event::Config { config } => {
                self.config = Some(config.clone());
                if config.starts_with("TUNCONF:") {
                    self.apply_network(&config);
                    self.handoff_at = Some(Instant::now());
                }
            }
            Event::Stats { active, bytes_up, bytes_down } => {
                self.active = active;
                if self.recovery.observe(active) {
                    self.zero_check_at = None;
                }
                let _ = self.events.send(ManagerEvent::Stats { active, bytes_up, bytes_down });
            }
            Event::ActiveZero { .. } => {
                self.active = 0;
                if let Some(target) = self.recovery.arm_at_zero() {
                    self.zero_check_at = Some((Instant::now() + WORKER_ZERO_CONFIRMATION, target));
                }
            }
            Event::ServerRestart => {
                self.log("[СЕРВЕР] Перезапуск сервера из панели, переподключаемся");
                self.panel_restart_at = Some(Instant::now() + PANEL_SERVER_RESTART_DELAY);
            }
            Event::TunLost => {
                let now = Instant::now();
                let escalate = self.last_tun_loss.is_some_and(|t| now.duration_since(t) < TUN_LOSS_REBUILD_WINDOW);
                self.last_tun_loss = Some(now);
                if escalate {
                    self.log("[TUN] Повторная потеря TUN, пересоздаём адаптер");
                    self.rebuild_adapter();
                    if let Some(config) = self.config.clone() {
                        self.apply_network(&config);
                    }
                } else {
                    self.log("[TUN] Потеря TUN, передаём адаптер заново");
                }
                self.handoff_at = Some(now);
            }
            Event::Error { message, fatal: true, .. } => self.fail(&message).await,
            Event::Error { code, message, .. } => self.log(format!("[ОШИБКА] {code}: {message}")),
            Event::CaptchaRequest { mode, redirect_uri, session_token } => {
                self.request_captcha(mode, redirect_uri, session_token).await
            }
            Event::NetworkSuspect => {
                self.log("[СЕТЬ] Подозрение на проблемы сети, проверяем интерфейс");
                self.on_network_change();
            }
            Event::CallUnavailable { hash, code } => self.on_call_unavailable(&hash, code).await,
            Event::Process { .. } => {}
            Event::Ready { .. } => self.keyed("ready", "[ВОРКЕР] Поток готов ✓".into(), 20, false, Some(LogLevel::Ok)),
            Event::Dtls { worker, message } => {
                // TunnelManager.handleTunnelEvent (Dtls)
                let suffix = message.trim();
                let is_error = suffix.contains("не состоялось") || suffix.to_lowercase().contains("ошибка");
                if is_error {
                    self.keyed(&format!("dtls_err_{worker}"), format!("[DTLS] Поток {worker}: {suffix}"), 30, true, Some(LogLevel::Err));
                } else {
                    let text = if suffix.contains("завершено") {
                        "[DTLS] Рукопожатие завершено, ключи SRTP установлены ✓".to_string()
                    } else if !suffix.is_empty() {
                        format!("[DTLS] {suffix}")
                    } else {
                        "[DTLS] Рукопожатие выполняется".to_string()
                    };
                    self.keyed("dtls_status", text, 30, false, Some(LogLevel::Ok));
                }
            }
            Event::Workers { workers } => self.log(format!("[СЕРВЕР] Назначено воркеров: {workers}")),
            Event::Routing { .. } => self.log("[СЕРВЕР] Получены правила маршрутизации"),
            Event::Progress { .. } | Event::Started { .. } | Event::Stopped { .. } | Event::CaptchaDone { .. } => {}
        }
    }

    fn rebuild_adapter(&mut self) {
        self.teardown_network();
        self.adapter = None;
        if let Err(e) = self.ensure_adapter() {
            self.log(format!("[TUN] Не удалось пересоздать адаптер: {e:#}"));
        }
    }

    /// TunVpnService.Builder: address /32, MTU, metric, DNS, split routes, NRPT.
    fn apply_network(&mut self, config: &str) {
        let Some(conf) = TunConf::parse(config) else {
            self.log(format!("[TUN] Некорректный TUNCONF: {config}"));
            return;
        };
        if self.applied.as_ref() == Some(&conf) {
            return;
        }
        if let Err(e) = self.ensure_adapter() {
            self.log(format!("[TUN] Адаптер Wintun: {e:#}"));
            return;
        }
        let Some((luid, if_index)) = self.adapter.as_ref().map(|a| (a.luid, a.if_index)) else { return };
        let mut errors = Vec::new();
        if let Err(e) = ipconfig::set_ipv4(luid, conf.client_ip, 32) {
            errors.push(format!("{e:#}"));
        }
        if let Err(e) = ipconfig::set_mtu(luid, self.cfg.mtu) {
            errors.push(format!("{e:#}"));
        }
        if let Err(e) = ipconfig::set_metric(luid, TUNNEL_METRIC) {
            errors.push(format!("{e:#}"));
        }
        if let Err(e) = dns::set_interface_dns(luid, &conf.dns) {
            errors.push(format!("{e:#}"));
        }
        if self.routes.added().is_empty() {
            for route in routes::tunnel_split_routes(luid, if_index) {
                let ipv6 = route.dest.is_ipv6();
                if let Err(e) = self.routes.add(route) {
                    // IPv6 halves are best effort: without them IPv6 may bypass the tunnel.
                    if ipv6 {
                        self.log(format!("[TUN] IPv6-маршрут не добавлен: {e:#}"));
                    } else {
                        errors.push(format!("{e:#}"));
                    }
                }
            }
        }
        if let Err(e) = dns::nrpt_install(&conf.dns) {
            errors.push(format!("{e:#}"));
        }
        flush_dns_cache();
        if errors.is_empty() {
            self.log(format!("[TUN] Интерфейс настроен: {}/32, DNS {}", conf.client_ip, conf.dns_text()));
        } else {
            self.log(format!("[TUN] Ошибки настройки интерфейса: {}", errors.join("; ")));
        }
        self.applied = Some(conf);
    }

    fn teardown_network(&mut self) {
        if let Err(e) = self.routes.remove_all() {
            self.log(format!("[TUN] Удаление маршрутов: {e:#}"));
        }
        if let Err(e) = dns::nrpt_remove() {
            self.log(format!("[TUN] Удаление NRPT: {e:#}"));
        }
        if self.applied.take().is_some() {
            flush_dns_cache();
        }
    }

    fn spawn_handoff(&mut self) {
        let Some(adapter) = self.adapter.as_ref() else { return };
        if self.proc.is_none() {
            return;
        }
        let seq = self.core_seq;
        let pipe = self.cfg.tun_pipe.clone();
        let name = adapter.name.clone();
        let tx = self.handoff_tx.clone();
        tokio::spawn(async move {
            let res = handoff::hand_over(&pipe, &name, HANDOFF_ATTEMPTS, HANDOFF_ACK_TIMEOUT).await;
            let _ = tx.send((seq, res));
        });
    }

    fn on_handoff(&mut self, seq: u64, res: anyhow::Result<()>) {
        if seq != self.core_seq || self.proc.is_none() {
            return;
        }
        match res {
            Ok(()) => {
                if let Some(conf) = self.applied.clone() {
                    self.set_status(Status::Connected {
                        client_ip: conf.client_ip.to_string(),
                        dns: conf.dns_text(),
                    });
                }
            }
            Err(e) => {
                // TunVpnService: fd delivery failed -> rebuild and retry (the core will
                // report TUN_LOST / exit 3 if it cannot recover; restart policy handles it).
                self.log(format!("[TUN] Передача адаптера ядру не удалась: {e:#}"));
                self.rebuild_adapter();
                if let Some(config) = self.config.clone() {
                    self.apply_network(&config);
                }
                self.handoff_at = Some(Instant::now() + Duration::from_secs(2));
            }
        }
    }

    fn on_network_change(&mut self) {
        if self.desired.is_none() {
            return;
        }
        let current = self.refresh_physical();
        match (&current, self.paused) {
            (None, false) => {
                if self.net_loss_at.is_none() {
                    self.net_loss_at = Some(Instant::now() + NETWORK_LOSS_GRACE);
                }
            }
            (None, true) => {}
            (Some(route), _) => {
                self.net_loss_at = None;
                let changed = self
                    .physical
                    .as_ref()
                    .is_none_or(|old| old.if_index != route.if_index || old.luid != route.luid);
                if changed {
                    if self.net_swap_at.is_none() {
                        self.net_swap_at = Some(Instant::now() + NETWORK_SWAP_SETTLE);
                    }
                } else if self.paused {
                    // Same interface is back: resume from the timer (async context).
                    self.net_swap_at = Some(Instant::now());
                }
            }
        }
    }

    async fn on_timer(&mut self) {
        let now = Instant::now();
        if self.restart_at.as_ref().is_some_and(|(t, _)| *t <= now) {
            let (_, reason) = self.restart_at.take().unwrap_or((now, String::new()));
            if self.desired.is_some() {
                if self.proc.is_some() {
                    self.restart_core(&reason).await;
                } else {
                    self.start_core().await;
                }
            }
        }
        if self.handoff_at.is_some_and(|t| t <= now) {
            self.handoff_at = None;
            self.spawn_handoff();
        }
        if let Some((t, target)) = self.zero_check_at
            && t <= now
        {
            self.zero_check_at = None;
            if self.recovery.should_recover(self.active, target) {
                self.restart_core("[NET] Потоки: 0 · обновление TURN").await;
            }
        }
        if self.panel_restart_at.is_some_and(|t| t <= now) {
            self.panel_restart_at = None;
            self.recovery.reset();
            self.restart_core("перезапуск сервера из панели").await;
        }
        if self.net_loss_at.is_some_and(|t| t <= now) {
            self.net_loss_at = None;
            if self.refresh_physical().is_none() && !self.paused && self.proc.is_some() {
                self.paused = true;
                self.send_core(CoreCommand::Pause).await;
                self.set_status(Status::Paused);
                self.log("[СЕТЬ] Нет сети, ядро на паузе");
            }
        }
        if self.net_swap_at.is_some_and(|t| t <= now) {
            self.net_swap_at = None;
            let current = self.refresh_physical();
            let changed = match (&current, &self.physical) {
                (Some(a), Some(b)) => a.if_index != b.if_index || a.luid != b.luid,
                (Some(_), None) => true,
                (None, _) => false,
            };
            if current.is_some() && changed {
                // The core's sockets are pinned to the old interface: restart it.
                self.paused = false;
                self.restart_core("смена сети").await;
            } else if current.is_some() && self.paused {
                self.paused = false;
                self.send_core(CoreCommand::Resume).await;
                self.log("[СЕТЬ] Сеть вернулась, ядро продолжает работу");
                if let Some(conf) = self.applied.clone() {
                    self.set_status(Status::Connected { client_ip: conf.client_ip.to_string(), dns: conf.dns_text() });
                }
            }
        }
    }
}

/// `ipconfig /flushdns` so cached answers from the physical DNS are not reused.
fn flush_dns_cache() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("ipconfig")
        .arg("/flushdns")
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunconf_parse() {
        let c = TunConf::parse("TUNCONF:10.66.67.2:1.1.1.1:stream-v2").unwrap();
        assert_eq!(c.client_ip, Ipv4Addr::new(10, 66, 67, 2));
        assert_eq!(c.dns, vec!["1.1.1.1".parse::<IpAddr>().unwrap()]);
        let c = TunConf::parse("TUNCONF:10.0.0.2:8.8.8.8,1.1.1.1").unwrap();
        assert_eq!(c.dns.len(), 2);
        let c = TunConf::parse("TUNCONF:10.0.0.2").unwrap();
        assert_eq!(c.dns, vec![PROBE_DEST]);
        assert!(TunConf::parse("TUNCONF:bad:1.1.1.1").is_none());
        assert!(TunConf::parse("NOCONF").is_none());
    }

    #[test]
    fn timestamp_is_stripped() {
        assert_eq!(strip_timestamp("2026/11/10 05:02:03 [КЛИЕНТ] ok"), "[КЛИЕНТ] ok");
        assert_eq!(strip_timestamp("2026/11/10 05:02:03.123456 x"), "x");
        assert_eq!(strip_timestamp("[КЛИЕНТ] no ts"), "[КЛИЕНТ] no ts");
        assert_eq!(strip_timestamp("2026-11-10 05:02:03 x"), "2026-11-10 05:02:03 x");
    }

    #[test]
    fn crash_backoff_matches_android() {
        let ms: Vec<u64> = (0..=7).map(|s| crash_restart_delay(s).as_millis() as u64).collect();
        assert_eq!(ms, vec![2_000, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000]);
    }

    #[test]
    fn captcha_solve_formats() {
        assert_eq!(
            parse_captcha_solve("CAPTCHA_SOLVE|auto|https://id.vk.ru/c|tok"),
            Some(("auto".into(), "https://id.vk.ru/c".into(), "tok".into()))
        );
        assert_eq!(
            parse_captcha_solve("x CAPTCHA_SOLVE|https://id.vk.ru/c|tok"),
            Some(("selected".into(), "https://id.vk.ru/c".into(), "tok".into()))
        );
        assert_eq!(parse_captcha_solve("CAPTCHA_SOLVE|only"), None);
    }

    #[test]
    fn critical_lines() {
        assert!(critical_auth_reason("FATAL_PROTOCOL v3").unwrap().contains("версии протокола"));
        assert!(critical_auth_reason("FATAL_AUTH неверный пароль").unwrap().contains("Неверный пароль"));
        assert!(critical_auth_reason("[КЛИЕНТ] ok").is_none());
    }
}
