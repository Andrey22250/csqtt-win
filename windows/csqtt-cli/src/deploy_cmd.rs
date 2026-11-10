//! Deploy, undeploy, and update-check commands for csqtt CLI.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;

/// Supported password alphabet from Android (UiUtils.kt lines 219–227):
/// "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
pub const PASSWORD_ALPHABET: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Fills the buffer with cryptographically secure random bytes.
#[cfg(windows)]
pub fn fill_random_bytes(buf: &mut [u8]) -> anyhow::Result<()> {
    use windows_sys::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let status = unsafe {
        BCryptGenRandom(
            core::ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        anyhow::bail!("BCryptGenRandom failed with NTSTATUS {:#x}", status);
    }
}

/// Fallback for non-Windows platforms.
#[cfg(not(windows))]
pub fn fill_random_bytes(buf: &mut [u8]) -> anyhow::Result<()> {
    use std::io::Read;
    let mut file = std::fs::File::open("/dev/urandom")?;
    file.read_exact(buf)?;
    Ok(())
}

/// Generates a 16-character alphanumeric password matching Android's `generate16CharPassword`.
/// Discards byte values >= 248 (62 * 4) to ensure an unbiased, uniform distribution.
pub fn generate_password_with<F>(mut fill_bytes: F) -> anyhow::Result<String>
where
    F: FnMut(&mut [u8]) -> anyhow::Result<()>,
{
    let mut password = String::with_capacity(16);
    let mut buf = [0u8; 32];
    while password.len() < 16 {
        fill_bytes(&mut buf)?;
        for &b in &buf {
            if b < 248 {
                password.push(PASSWORD_ALPHABET[(b % 62) as usize] as char);
                if password.len() == 16 {
                    break;
                }
            }
        }
    }
    Ok(password)
}

/// Generates a 16-character alphanumeric password using BCryptGenRandom.
pub fn generate_password() -> anyhow::Result<String> {
    generate_password_with(fill_random_bytes)
}

// ---------------------------------------------------------------------------
// TOFU (Trust On First Use) Host Key Storage
// ---------------------------------------------------------------------------

/// Host key entry stored in `%LOCALAPPDATA%\csqtt-win\known_hosts.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHostEntry {
    pub key_type: String,
    pub fingerprint: String,
}

impl KnownHostEntry {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "type": self.key_type,
            "fingerprint": self.fingerprint,
        })
    }

    pub fn from_json(val: &serde_json::Value) -> Option<Self> {
        let obj = val.as_object()?;
        let key_type = obj.get("type")?.as_str()?.to_string();
        let fingerprint = obj.get("fingerprint")?.as_str()?.to_string();
        Some(Self {
            key_type,
            fingerprint,
        })
    }
}

pub type KnownHostsMap = HashMap<String, KnownHostEntry>;

/// TOFU host verification decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TofuDecision {
    AcceptKnown,
    Mismatch {
        expected_type: String,
        expected_fingerprint: String,
    },
    Unknown,
}

/// Pure TOFU logic checking whether host:port key matches the stored entry.
pub fn check_known_host(
    map: &KnownHostsMap,
    host: &str,
    port: u16,
    key_type: &str,
    fingerprint: &str,
) -> TofuDecision {
    let key = format!("{}:{}", host.trim().to_lowercase(), port);
    match map.get(&key) {
        Some(entry) => {
            if entry.fingerprint == fingerprint && entry.key_type == key_type {
                TofuDecision::AcceptKnown
            } else {
                TofuDecision::Mismatch {
                    expected_type: entry.key_type.clone(),
                    expected_fingerprint: entry.fingerprint.clone(),
                }
            }
        }
        None => TofuDecision::Unknown,
    }
}

/// Path to `%LOCALAPPDATA%\csqtt-win\known_hosts.json`.
pub fn known_hosts_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Local"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("csqtt-win").join("known_hosts.json")
}

/// Loads known hosts map from the specified file.
pub fn load_known_hosts(path: &Path) -> KnownHostsMap {
    let mut map = KnownHostsMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(obj) = val.as_object() {
                for (k, v) in obj {
                    if let Some(entry) = KnownHostEntry::from_json(v) {
                        map.insert(k.clone(), entry);
                    }
                }
            }
        }
    }
    map
}

/// Persists known hosts map to disk.
pub fn save_known_hosts(path: &Path, map: &KnownHostsMap) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut obj = serde_json::Map::new();
    for (k, v) in map {
        obj.insert(k.clone(), v.to_json());
    }
    let val = serde_json::Value::Object(obj);
    let text = serde_json::to_string_pretty(&val)?;
    std::fs::write(path, text)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

/// Parsed arguments for `csqtt deploy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key: Option<PathBuf>,
    pub passphrase_env: Option<String>,
    pub sudo: bool,
    pub docker: bool,
    pub peer_port: u16,
    pub web_port: u16,
    pub web_login: Option<String>,
    pub secret_path: String,
    pub assets: Option<PathBuf>,
}

/// Parsed arguments for `csqtt undeploy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndeployArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key: Option<PathBuf>,
    pub passphrase_env: Option<String>,
    pub sudo: bool,
    pub peer_port: u16,
    pub assets: Option<PathBuf>,
}

fn parse_port_val(flag: &str, val: &str) -> anyhow::Result<u16> {
    let p = val
        .trim()
        .parse::<u16>()
        .map_err(|_| anyhow::anyhow!("Некорректное значение порта для {flag}: '{val}'"))?;
    if p == 0 {
        anyhow::bail!("Порт для {flag} должен быть в диапазоне 1..=65535");
    }
    Ok(p)
}

/// Parses command line arguments for `csqtt deploy`.
pub fn parse_deploy_args<T: AsRef<str>>(args: &[T]) -> anyhow::Result<DeployArgs> {
    let mut host: Option<String> = None;
    let mut port: u16 = csqtt_deploy::ops_policy::DEFAULT_SSH_PORT as u16;
    let mut user: String = "root".to_string();
    let mut key: Option<PathBuf> = None;
    let mut passphrase_env: Option<String> = None;
    let mut sudo: bool = false;
    let mut docker: bool = false;
    let mut peer_port: u16 = csqtt_deploy::ops_policy::DEFAULT_SERVER_PEER_PORT as u16;
    let mut web_port: u16 = csqtt_deploy::ops_policy::DEFAULT_SERVER_WEB_PORT as u16;
    let mut web_login: Option<String> = None;
    let mut secret_path: String = String::new();
    let mut assets: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_ref();
        if arg == "--host" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --host требует указания значения");
            }
            host = Some(args[i].as_ref().to_string());
        } else if let Some(val) = arg.strip_prefix("--host=") {
            host = Some(val.to_string());
        } else if arg == "--port" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --port требует указания значения");
            }
            port = parse_port_val("--port", args[i].as_ref())?;
        } else if let Some(val) = arg.strip_prefix("--port=") {
            port = parse_port_val("--port", val)?;
        } else if arg == "--user" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --user требует указания значения");
            }
            user = args[i].as_ref().to_string();
        } else if let Some(val) = arg.strip_prefix("--user=") {
            user = val.to_string();
        } else if arg == "--key" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --key требует указания пути к файлу");
            }
            key = Some(PathBuf::from(args[i].as_ref()));
        } else if let Some(val) = arg.strip_prefix("--key=") {
            key = Some(PathBuf::from(val));
        } else if arg == "--passphrase-env" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --passphrase-env требует имени переменной окружения");
            }
            passphrase_env = Some(args[i].as_ref().to_string());
        } else if let Some(val) = arg.strip_prefix("--passphrase-env=") {
            passphrase_env = Some(val.to_string());
        } else if arg == "--sudo" {
            sudo = true;
        } else if arg == "--docker" {
            docker = true;
        } else if arg == "--peer-port" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --peer-port требует указания значения");
            }
            peer_port = parse_port_val("--peer-port", args[i].as_ref())?;
        } else if let Some(val) = arg.strip_prefix("--peer-port=") {
            peer_port = parse_port_val("--peer-port", val)?;
        } else if arg == "--web-port" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --web-port требует указания значения");
            }
            web_port = parse_port_val("--web-port", args[i].as_ref())?;
        } else if let Some(val) = arg.strip_prefix("--web-port=") {
            web_port = parse_port_val("--web-port", val)?;
        } else if arg == "--web-login" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --web-login требует указания логина");
            }
            web_login = Some(args[i].as_ref().to_string());
        } else if let Some(val) = arg.strip_prefix("--web-login=") {
            web_login = Some(val.to_string());
        } else if arg == "--secret-path" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --secret-path требует указания пути");
            }
            secret_path = args[i].as_ref().to_string();
        } else if let Some(val) = arg.strip_prefix("--secret-path=") {
            secret_path = val.to_string();
        } else if arg == "--assets" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --assets требует указания пути к директории");
            }
            assets = Some(PathBuf::from(args[i].as_ref()));
        } else if let Some(val) = arg.strip_prefix("--assets=") {
            assets = Some(PathBuf::from(val));
        } else {
            anyhow::bail!("Неизвестный параметр команды deploy: '{arg}'");
        }
        i += 1;
    }

    let host = host.ok_or_else(|| anyhow::anyhow!("Параметр --host обязателен для команды deploy"))?;
    if host.trim().is_empty() {
        anyhow::bail!("Параметр --host не может быть пустым");
    }

    Ok(DeployArgs {
        host: host.trim().to_string(),
        port,
        user,
        key,
        passphrase_env,
        sudo,
        docker,
        peer_port,
        web_port,
        web_login,
        secret_path,
        assets,
    })
}

/// Parses command line arguments for `csqtt undeploy`.
pub fn parse_undeploy_args<T: AsRef<str>>(args: &[T]) -> anyhow::Result<UndeployArgs> {
    let mut host: Option<String> = None;
    let mut port: u16 = csqtt_deploy::ops_policy::DEFAULT_SSH_PORT as u16;
    let mut user: String = "root".to_string();
    let mut key: Option<PathBuf> = None;
    let mut passphrase_env: Option<String> = None;
    let mut sudo: bool = false;
    let mut peer_port: u16 = csqtt_deploy::ops_policy::DEFAULT_SERVER_PEER_PORT as u16;
    let mut assets: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_ref();
        if arg == "--host" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --host требует указания значения");
            }
            host = Some(args[i].as_ref().to_string());
        } else if let Some(val) = arg.strip_prefix("--host=") {
            host = Some(val.to_string());
        } else if arg == "--port" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --port требует указания значения");
            }
            port = parse_port_val("--port", args[i].as_ref())?;
        } else if let Some(val) = arg.strip_prefix("--port=") {
            port = parse_port_val("--port", val)?;
        } else if arg == "--user" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --user требует указания значения");
            }
            user = args[i].as_ref().to_string();
        } else if let Some(val) = arg.strip_prefix("--user=") {
            user = val.to_string();
        } else if arg == "--key" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --key требует указания пути к файлу");
            }
            key = Some(PathBuf::from(args[i].as_ref()));
        } else if let Some(val) = arg.strip_prefix("--key=") {
            key = Some(PathBuf::from(val));
        } else if arg == "--passphrase-env" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --passphrase-env требует имени переменной окружения");
            }
            passphrase_env = Some(args[i].as_ref().to_string());
        } else if let Some(val) = arg.strip_prefix("--passphrase-env=") {
            passphrase_env = Some(val.to_string());
        } else if arg == "--sudo" {
            sudo = true;
        } else if arg == "--docker" {
            // Ignored on undeploy but accepted for compatibility
        } else if arg == "--peer-port" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --peer-port требует указания значения");
            }
            peer_port = parse_port_val("--peer-port", args[i].as_ref())?;
        } else if let Some(val) = arg.strip_prefix("--peer-port=") {
            peer_port = parse_port_val("--peer-port", val)?;
        } else if arg == "--assets" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("Параметр --assets требует указания пути к директории");
            }
            assets = Some(PathBuf::from(args[i].as_ref()));
        } else if let Some(val) = arg.strip_prefix("--assets=") {
            assets = Some(PathBuf::from(val));
        } else {
            anyhow::bail!("Неизвестный параметр команды undeploy: '{arg}'");
        }
        i += 1;
    }

    let host = host.ok_or_else(|| anyhow::anyhow!("Параметр --host обязателен для команды undeploy"))?;
    if host.trim().is_empty() {
        anyhow::bail!("Параметр --host не может быть пустым");
    }

    Ok(UndeployArgs {
        host: host.trim().to_string(),
        port,
        user,
        key,
        passphrase_env,
        sudo,
        peer_port,
        assets,
    })
}

// ---------------------------------------------------------------------------
// SSH Auth and Asset Resolution
// ---------------------------------------------------------------------------

fn resolve_ssh_auth(
    key_path: Option<&Path>,
    passphrase_env: Option<&str>,
) -> anyhow::Result<csqtt_deploy::ssh::SshAuth> {
    if let Some(path) = key_path {
        let private_key = std::fs::read_to_string(path)
            .with_context(|| format!("Не удалось прочитать приватный ключ '{}'", path.display()))?;
        let passphrase = passphrase_env.and_then(|var| std::env::var(var).ok());
        Ok(csqtt_deploy::ssh::SshAuth::Key {
            private_key,
            passphrase,
        })
    } else if let Ok(pass) = std::env::var("CSQTT_SSH_PASSWORD") {
        if !pass.is_empty() {
            return Ok(csqtt_deploy::ssh::SshAuth::Password(pass));
        }
        prompt_ssh_password()
    } else {
        prompt_ssh_password()
    }
}

fn prompt_ssh_password() -> anyhow::Result<csqtt_deploy::ssh::SshAuth> {
    print!("Введите SSH-пароль (ввод видим): ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("Не удалось прочитать пароль SSH со стандартного ввода")?;
    let trimmed = line.trim_end_matches(&['\r', '\n'][..]).to_string();
    Ok(csqtt_deploy::ssh::SshAuth::Password(trimmed))
}

fn resolve_assets_dir(custom: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(dir) = custom {
        Ok(dir.to_path_buf())
    } else {
        let exe_dir = std::env::current_exe()?
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(exe_dir.join("server"))
    }
}

fn make_host_key_check() -> csqtt_deploy::ssh::HostKeyCheck {
    let hosts_path = known_hosts_path();
    let hosts_map = Arc::new(std::sync::Mutex::new(load_known_hosts(&hosts_path)));
    let hosts_map_cb = Arc::clone(&hosts_map);
    let hosts_path_cb = hosts_path;

    Box::new(move |host, port, key_type, fingerprint| {
        let decision = {
            let guard = hosts_map_cb.lock().unwrap();
            check_known_host(&guard, host, port, key_type, fingerprint)
        };

        match decision {
            TofuDecision::AcceptKnown => true,
            TofuDecision::Mismatch {
                expected_type,
                expected_fingerprint,
            } => {
                eprintln!();
                eprintln!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                eprintln!("@ ПРЕДУПРЕЖДЕНИЕ: КЛЮЧ УЗЛА ИЗМЕНИЛСЯ!                     @");
                eprintln!("@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
                eprintln!("Ключ удалённого сервера {host}:{port} не совпадает с сохранённым!");
                eprintln!("Сохранённый: {expected_fingerprint} ({expected_type})");
                eprintln!("Полученный:  {fingerprint} ({key_type})");
                eprintln!("Возможна атака типа «man-in-the-middle» (MITM) или замена ключа на сервере.");
                eprintln!("Подключение отклонено в целях безопасности.");
                false
            }
            TofuDecision::Unknown => {
                println!("Неизвестный ключ SSH для {host}:{port}:");
                println!("  Тип:       {key_type}");
                println!("  Отпечаток: {fingerprint}");
                print!("Доверять этому ключу? [y/N]: ");
                let _ = std::io::stdout().flush();
                let mut line = String::new();
                if std::io::stdin().read_line(&mut line).is_ok() {
                    let trimmed = line.trim();
                    if trimmed.eq_ignore_ascii_case("y")
                        || trimmed.eq_ignore_ascii_case("yes")
                        || trimmed.eq_ignore_ascii_case("д")
                        || trimmed.eq_ignore_ascii_case("да")
                    {
                        let mut guard = hosts_map_cb.lock().unwrap();
                        let key = format!("{}:{}", host.trim().to_lowercase(), port);
                        guard.insert(
                            key,
                            KnownHostEntry {
                                key_type: key_type.to_string(),
                                fingerprint: fingerprint.to_string(),
                            },
                        );
                        if let Err(e) = save_known_hosts(&hosts_path_cb, &guard) {
                            eprintln!("[ERR] Не удалось сохранить known_hosts: {e}");
                        }
                        return true;
                    }
                }
                println!("Ключ узла отклонён пользователем.");
                false
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Service Settings Synchronization
// ---------------------------------------------------------------------------

async fn sync_settings_to_service(
    host: &str,
    peer_port: u16,
    web_port: u16,
    password_to_sync: Option<&str>,
) -> anyhow::Result<()> {
    let mut client = match csqtt_ipc::pipe::Client::connect().await {
        Ok(c) => c,
        Err(_) => {
            println!();
            println!("Служба csqtt-win не запущена. Параметры подключения не были импортированы автоматически.");
            println!("Адрес сервера (peer): {host}");
            if let Some(pass) = password_to_sync {
                println!("Пароль подключения:  {pass}");
            }
            println!("\nПодсказка: запустите службу (csqtt-svc service) и примените параметры вручную:");
            println!("  csqtt settings set peer {host}");
            if let Some(pass) = password_to_sync {
                println!("  csqtt secret set connection_password_encrypted {pass}");
            }
            return Ok(());
        }
    };

    // 1. Get current settings from service
    client
        .send(&csqtt_ipc::Request::GetSettings)
        .await
        .map_err(|e| anyhow::anyhow!("Не удалось отправить запрос настроек службе: {e}"))?;
    let resp = client
        .recv()
        .await
        .map_err(|e| anyhow::anyhow!("Не удалось получить настройки от службы: {e}"))?;
    let settings_val = match resp {
        Some(csqtt_ipc::Response::Settings { settings }) => settings,
        Some(csqtt_ipc::Response::Error { message }) => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос настроек: {other:?}"),
    };

    let mut settings: csqtt_logic::settings::Settings = serde_json::from_value(settings_val)
        .map_err(|e| anyhow::anyhow!("Не удалось разобрать настройки службы: {e}"))?;

    let profile = settings.current_profile_mut();
    profile.peer = host.to_string();
    if peer_port != csqtt_deploy::ops_policy::DEFAULT_SERVER_PEER_PORT as u16 {
        profile.manual_ports_enabled = true;
        profile.server_peer_port = peer_port as i32;
    }
    if web_port != csqtt_deploy::ops_policy::DEFAULT_SERVER_WEB_PORT as u16 {
        profile.server_web_port = web_port as i32;
    }

    let new_val = serde_json::to_value(&settings)
        .map_err(|e| anyhow::anyhow!("Не удалось сериализовать настройки: {e}"))?;

    client
        .send(&csqtt_ipc::Request::SetSettings { settings: new_val })
        .await
        .map_err(|e| anyhow::anyhow!("Не удалось отправить новые настройки службе: {e}"))?;
    let resp = client
        .recv()
        .await
        .map_err(|e| anyhow::anyhow!("Не удалось сохранить настройки в службе: {e}"))?;
    match resp {
        Some(csqtt_ipc::Response::Ok) => {}
        Some(csqtt_ipc::Response::Error { message }) => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы: {other:?}"),
    }

    if let Some(pass) = password_to_sync {
        let key = csqtt_logic::settings::SecretKey::ConnectionPassword
            .storage_name()
            .to_string();
        client
            .send(&csqtt_ipc::Request::SetSecret {
                key,
                value: pass.to_string(),
            })
            .await
            .map_err(|e| anyhow::anyhow!("Не удалось отправить пароль службе: {e}"))?;
        let resp = client
            .recv()
            .await
            .map_err(|e| anyhow::anyhow!("Не удалось сохранить секрет в службе: {e}"))?;
        match resp {
            Some(csqtt_ipc::Response::Ok) => {}
            Some(csqtt_ipc::Response::Error { message }) => anyhow::bail!("{message}"),
            other => anyhow::bail!("Неожиданный ответ службы: {other:?}"),
        }
    }

    println!(
        "[OK] Параметры сервера успешно сохранены в текущий профиль службы (профиль {})",
        settings.active_profile
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command Handlers
// ---------------------------------------------------------------------------

/// Handles `csqtt deploy`.
pub async fn handle_deploy(args: DeployArgs) -> anyhow::Result<()> {
    let auth = resolve_ssh_auth(args.key.as_deref(), args.passphrase_env.as_deref())?;

    let sudo_password = if args.sudo {
        std::env::var("CSQTT_SUDO_PASSWORD").ok().or_else(|| match &auth {
            csqtt_deploy::ssh::SshAuth::Password(p) => Some(p.clone()),
            _ => None,
        })
    } else {
        None
    };

    let main_pass = std::env::var("CSQTT_MAIN_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty())
        .map(Ok)
        .unwrap_or_else(generate_password)?;

    let web_login = args
        .web_login
        .or_else(|| std::env::var("CSQTT_WEB_LOGIN").ok())
        .unwrap_or_else(|| "admin".to_string());

    let web_pass = std::env::var("CSQTT_WEB_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty())
        .map(Ok)
        .unwrap_or_else(generate_password)?;

    let assets_dir = resolve_assets_dir(args.assets.as_deref())?;

    let req = csqtt_deploy::ops::DeployRequest {
        host: args.host.clone(),
        port: args.port,
        user: args.user.clone(),
        auth,
        sudo_password,
        main_pass: main_pass.clone(),
        web_login,
        web_pass,
        peer_port: args.peer_port,
        web_port: args.web_port,
        certificate: String::new(),
        install_in_docker: args.docker,
        secret_path: args.secret_path.clone(),
    };

    let check_box = make_host_key_check();

    let mut on_output = |line: csqtt_deploy::ops_policy::DeployOutputLine| match line.level {
        csqtt_deploy::ops_policy::DeployOutputLevel::Ok => {
            println!("[OK] {}", line.message);
        }
        csqtt_deploy::ops_policy::DeployOutputLevel::Err => {
            eprintln!("[ERR] {}", line.message);
        }
        csqtt_deploy::ops_policy::DeployOutputLevel::Log => {
            println!("{}", line.message);
        }
    };

    let outcome = csqtt_deploy::ops::run(
        csqtt_deploy::ops::DeployAction::Deploy,
        &req,
        &assets_dir,
        check_box,
        &mut on_output,
    )
    .await?;

    println!();
    println!("[OK] {}", outcome.message);
    if let Some(arch) = outcome.architecture {
        println!("Архитектура сервера: {}", arch.display_name());
    }
    if let Some(ref url) = outcome.panel_url {
        println!("Панель управления:   {url}");
    }
    let password_to_sync = outcome.password_to_synchronize.as_deref().or(Some(&main_pass));
    if let Some(pass) = password_to_sync {
        println!("Пароль подключения:  {pass}");
    }

    sync_settings_to_service(
        &args.host,
        args.peer_port,
        args.web_port,
        password_to_sync,
    )
    .await?;

    Ok(())
}

/// Handles `csqtt undeploy`.
pub async fn handle_undeploy(args: UndeployArgs) -> anyhow::Result<()> {
    let auth = resolve_ssh_auth(args.key.as_deref(), args.passphrase_env.as_deref())?;

    let sudo_password = if args.sudo {
        std::env::var("CSQTT_SUDO_PASSWORD").ok().or_else(|| match &auth {
            csqtt_deploy::ssh::SshAuth::Password(p) => Some(p.clone()),
            _ => None,
        })
    } else {
        None
    };

    let assets_dir = resolve_assets_dir(args.assets.as_deref())?;

    let req = csqtt_deploy::ops::DeployRequest {
        host: args.host.clone(),
        port: args.port,
        user: args.user.clone(),
        auth,
        sudo_password,
        main_pass: String::new(),
        web_login: String::new(),
        web_pass: String::new(),
        peer_port: args.peer_port,
        web_port: csqtt_deploy::ops_policy::DEFAULT_SERVER_WEB_PORT as u16,
        certificate: String::new(),
        install_in_docker: false,
        secret_path: String::new(),
    };

    let check_box = make_host_key_check();

    let mut on_output = |line: csqtt_deploy::ops_policy::DeployOutputLine| match line.level {
        csqtt_deploy::ops_policy::DeployOutputLevel::Ok => {
            println!("[OK] {}", line.message);
        }
        csqtt_deploy::ops_policy::DeployOutputLevel::Err => {
            eprintln!("[ERR] {}", line.message);
        }
        csqtt_deploy::ops_policy::DeployOutputLevel::Log => {
            println!("{}", line.message);
        }
    };

    let outcome = csqtt_deploy::ops::run(
        csqtt_deploy::ops::DeployAction::Uninstall,
        &req,
        &assets_dir,
        check_box,
        &mut on_output,
    )
    .await?;

    println!();
    println!("[OK] {}", outcome.message);
    Ok(())
}

/// Handles `csqtt update-check`.
pub async fn handle_update_check() -> anyhow::Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let result = tokio::task::spawn_blocking(move || {
        csqtt_deploy::update::check_latest(csqtt_deploy::update::DEFAULT_REPO, current_version)
    })
    .await
    .map_err(|e| anyhow::anyhow!("Ошибка задачи проверки обновлений: {e}"))??;

    match result {
        Some(info) => {
            println!("Доступно обновление:");
            println!("  Текущая версия:  {}", env!("CARGO_PKG_VERSION"));
            println!("  Новая версия:    {}", info.version);
            println!("  Тег релиза:      {}", info.tag);
            println!("  Файл:            {}", info.asset_name);
            println!("  Ссылка:          {}", info.asset_url);
            println!("  Страница:        {}", info.html_url);
            if !info.notes.trim().is_empty() {
                println!("\nПримечания к выпуску:\n{}", info.notes.trim());
            }
        }
        None => {
            println!("Установлена актуальная версия CSQTT ({})", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_deploy_defaults() {
        let args = parse_deploy_args(&["--host", "10.0.0.1"]).unwrap();
        assert_eq!(args.host, "10.0.0.1");
        assert_eq!(args.port, 22);
        assert_eq!(args.user, "root");
        assert_eq!(args.key, None);
        assert_eq!(args.passphrase_env, None);
        assert!(!args.sudo);
        assert!(!args.docker);
        assert_eq!(args.peer_port, 46000);
        assert_eq!(args.web_port, 46002);
        assert_eq!(args.web_login, None);
        assert_eq!(args.secret_path, "");
        assert_eq!(args.assets, None);
    }

    #[test]
    fn test_parse_deploy_full() {
        let args = parse_deploy_args(&[
            "--host",
            "vps.example.com",
            "--port",
            "2222",
            "--user",
            "ubuntu",
            "--key",
            "id_ed25519",
            "--passphrase-env",
            "SSH_KEY_PASS",
            "--sudo",
            "--docker",
            "--peer-port",
            "50000",
            "--web-port",
            "50002",
            "--web-login",
            "admin_user",
            "--secret-path",
            "supersecret",
            "--assets",
            "custom_assets",
        ])
        .unwrap();

        assert_eq!(args.host, "vps.example.com");
        assert_eq!(args.port, 2222);
        assert_eq!(args.user, "ubuntu");
        assert_eq!(args.key, Some(PathBuf::from("id_ed25519")));
        assert_eq!(args.passphrase_env, Some("SSH_KEY_PASS".to_string()));
        assert!(args.sudo);
        assert!(args.docker);
        assert_eq!(args.peer_port, 50000);
        assert_eq!(args.web_port, 50002);
        assert_eq!(args.web_login, Some("admin_user".to_string()));
        assert_eq!(args.secret_path, "supersecret");
        assert_eq!(args.assets, Some(PathBuf::from("custom_assets")));
    }

    #[test]
    fn test_parse_deploy_equals_syntax() {
        let args = parse_deploy_args(&[
            "--host=192.168.1.100",
            "--port=2200",
            "--user=deployer",
            "--key=my_key",
            "--passphrase-env=MY_ENV",
            "--peer-port=47000",
            "--web-port=47002",
            "--web-login=root_web",
            "--secret-path=uuid123",
            "--assets=server_dir",
        ])
        .unwrap();

        assert_eq!(args.host, "192.168.1.100");
        assert_eq!(args.port, 2200);
        assert_eq!(args.user, "deployer");
        assert_eq!(args.key, Some(PathBuf::from("my_key")));
        assert_eq!(args.passphrase_env, Some("MY_ENV".to_string()));
        assert_eq!(args.peer_port, 47000);
        assert_eq!(args.web_port, 47002);
        assert_eq!(args.web_login, Some("root_web".to_string()));
        assert_eq!(args.secret_path, "uuid123");
        assert_eq!(args.assets, Some(PathBuf::from("server_dir")));
    }

    #[test]
    fn test_parse_deploy_missing_host() {
        assert!(parse_deploy_args::<&str>(&[]).is_err());
        assert!(parse_deploy_args(&["--port", "22"]).is_err());
        assert!(parse_deploy_args(&["--host"]).is_err());
        assert!(parse_deploy_args(&["--host", ""]).is_err());
    }

    #[test]
    fn test_parse_deploy_invalid_ports() {
        assert!(parse_deploy_args(&["--host", "1.1.1.1", "--port", "0"]).is_err());
        assert!(parse_deploy_args(&["--host", "1.1.1.1", "--port", "70000"]).is_err());
        assert!(parse_deploy_args(&["--host", "1.1.1.1", "--port", "abc"]).is_err());
        assert!(parse_deploy_args(&["--host", "1.1.1.1", "--peer-port", "0"]).is_err());
        assert!(parse_deploy_args(&["--host", "1.1.1.1", "--web-port", "0"]).is_err());
    }

    #[test]
    fn test_parse_deploy_unknown_arg() {
        assert!(parse_deploy_args(&["--host", "1.1.1.1", "--invalid"]).is_err());
    }

    #[test]
    fn test_parse_undeploy_defaults_and_full() {
        let args = parse_undeploy_args(&["--host", "10.0.0.2"]).unwrap();
        assert_eq!(args.host, "10.0.0.2");
        assert_eq!(args.port, 22);
        assert_eq!(args.user, "root");
        assert_eq!(args.key, None);
        assert!(!args.sudo);
        assert_eq!(args.peer_port, 46000);

        let full = parse_undeploy_args(&[
            "--host",
            "10.0.0.2",
            "--port",
            "222",
            "--user",
            "admin",
            "--key",
            "key.pem",
            "--passphrase-env",
            "VAR",
            "--sudo",
            "--peer-port",
            "48000",
            "--assets",
            "assets_dir",
        ])
        .unwrap();
        assert_eq!(full.host, "10.0.0.2");
        assert_eq!(full.port, 222);
        assert_eq!(full.user, "admin");
        assert_eq!(full.key, Some(PathBuf::from("key.pem")));
        assert_eq!(full.passphrase_env, Some("VAR".to_string()));
        assert!(full.sudo);
        assert_eq!(full.peer_port, 48000);
        assert_eq!(full.assets, Some(PathBuf::from("assets_dir")));
    }

    #[test]
    fn test_parse_undeploy_missing_host() {
        assert!(parse_undeploy_args::<&str>(&[]).is_err());
        assert!(parse_undeploy_args(&["--port", "22"]).is_err());
    }

    #[test]
    fn test_tofu_unknown_host() {
        let map = KnownHostsMap::new();
        let res = check_known_host(&map, "1.2.3.4", 22, "ssh-ed25519", "SHA256:abc");
        assert_eq!(res, TofuDecision::Unknown);
    }

    #[test]
    fn test_tofu_accept_known() {
        let mut map = KnownHostsMap::new();
        map.insert(
            "1.2.3.4:22".into(),
            KnownHostEntry {
                key_type: "ssh-ed25519".into(),
                fingerprint: "SHA256:abc".into(),
            },
        );

        let res = check_known_host(&map, "1.2.3.4", 22, "ssh-ed25519", "SHA256:abc");
        assert_eq!(res, TofuDecision::AcceptKnown);
    }

    #[test]
    fn test_tofu_fingerprint_mismatch() {
        let mut map = KnownHostsMap::new();
        map.insert(
            "1.2.3.4:22".into(),
            KnownHostEntry {
                key_type: "ssh-ed25519".into(),
                fingerprint: "SHA256:pinned".into(),
            },
        );

        let res = check_known_host(&map, "1.2.3.4", 22, "ssh-ed25519", "SHA256:different");
        assert_eq!(
            res,
            TofuDecision::Mismatch {
                expected_type: "ssh-ed25519".into(),
                expected_fingerprint: "SHA256:pinned".into(),
            }
        );
    }

    #[test]
    fn test_tofu_type_mismatch() {
        let mut map = KnownHostsMap::new();
        map.insert(
            "1.2.3.4:22".into(),
            KnownHostEntry {
                key_type: "ssh-ed25519".into(),
                fingerprint: "SHA256:pinned".into(),
            },
        );

        let res = check_known_host(&map, "1.2.3.4", 22, "rsa-sha2-512", "SHA256:pinned");
        assert_eq!(
            res,
            TofuDecision::Mismatch {
                expected_type: "ssh-ed25519".into(),
                expected_fingerprint: "SHA256:pinned".into(),
            }
        );
    }

    #[test]
    fn test_known_hosts_json_roundtrip() {
        let mut map = KnownHostsMap::new();
        map.insert(
            "server.com:22".into(),
            KnownHostEntry {
                key_type: "ssh-ed25519".into(),
                fingerprint: "SHA256:xyz123".into(),
            },
        );

        let entry = map.get("server.com:22").unwrap();
        let json_val = entry.to_json();
        assert_eq!(json_val["type"], "ssh-ed25519");
        assert_eq!(json_val["fingerprint"], "SHA256:xyz123");

        let reconstructed = KnownHostEntry::from_json(&json_val).unwrap();
        assert_eq!(reconstructed, *entry);
    }

    #[test]
    fn test_password_generator_format() {
        for _ in 0..50 {
            let pass = generate_password().expect("password generation should succeed");
            assert_eq!(pass.len(), 16);
            assert!(pass.bytes().all(|b| PASSWORD_ALPHABET.contains(&b)));
            assert!(csqtt_deploy::ops_policy::is_deploy_main_password_valid(&pass));
        }
    }

    #[test]
    fn test_password_generator_deterministic() {
        let mut counter = 0u8;
        let pass = generate_password_with(|buf| {
            for b in buf.iter_mut() {
                *b = counter;
                counter = counter.wrapping_add(1);
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(pass.len(), 16);
        assert!(csqtt_deploy::ops_policy::is_deploy_main_password_valid(&pass));
    }
}
