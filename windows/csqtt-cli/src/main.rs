//! csqtt: CLI and tray for csqtt-win.
mod tray;
mod webview;
mod captcha;
mod vk_auth;
mod deploy_cmd;
mod gui;
mod svc_control;

use std::path::{Path, PathBuf};

use csqtt_ipc::pipe::Client;
use csqtt_ipc::{Request, Response, StatusInfo, TunnelState};
use csqtt_logic::settings::{SecretKey, Settings};

/// Supported CLI commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Connect,
    Disconnect,
    Status,
    Logs { follow: bool, tail: usize },
    Import { source: String },
    Export { include_secrets: bool, file: Option<PathBuf> },
    SettingsGet { key: Option<String> },
    SettingsSet { key: String, value: String },
    SecretSet { key: String, value: String },
    AppsList,
    AppsAdd { exe: String },
    AppsRemove { exe: String },
    VkLogin,
    VkLogout,
    CaptchaWindow { payload: String },
    Tray,
    Gui,
    Help,
    Deploy(deploy_cmd::DeployArgs),
    Undeploy(deploy_cmd::UndeployArgs),
    UpdateCheck,
}

/// Parses command line arguments (excluding the executable name) into a [`Command`].
pub fn parse_command<T: AsRef<str>>(args: &[T]) -> anyhow::Result<Command> {
    if args.is_empty() {
        return Ok(Command::Help);
    }

    match args[0].as_ref() {
        "connect" => Ok(Command::Connect),
        "disconnect" => Ok(Command::Disconnect),
        "status" => Ok(Command::Status),
        "logs" => {
            let mut follow = false;
            let mut tail = 100;
            let mut i = 1;
            while i < args.len() {
                let arg = args[i].as_ref();
                if arg == "-f" || arg == "--follow" {
                    follow = true;
                    i += 1;
                } else if arg == "-n" {
                    i += 1;
                    if i >= args.len() {
                        anyhow::bail!("Параметр -n требует указания количества строк");
                    }
                    tail = args[i]
                        .as_ref()
                        .parse::<usize>()
                        .map_err(|_| anyhow::anyhow!("Некорректное значение для -n: '{}'", args[i].as_ref()))?;
                    i += 1;
                } else if let Some(rest) = arg.strip_prefix("-n") {
                    tail = rest
                        .parse::<usize>()
                        .map_err(|_| anyhow::anyhow!("Некорректное значение для -n: '{}'", rest))?;
                    i += 1;
                } else {
                    anyhow::bail!("Неизвестный параметр команды logs: '{}'", arg);
                }
            }
            Ok(Command::Logs { follow, tail })
        }
        "import" => {
            if args.len() < 2 {
                anyhow::bail!("Использование: csqtt import <csqtt://...|путь_к_файлу>");
            }
            Ok(Command::Import {
                source: args[1].as_ref().to_string(),
            })
        }
        "export" => {
            let mut include_secrets = false;
            let mut file = None;
            for arg in &args[1..] {
                let s = arg.as_ref();
                if s == "--secrets" || s == "-s" {
                    include_secrets = true;
                } else if !s.starts_with('-') && file.is_none() {
                    file = Some(PathBuf::from(s));
                } else {
                    anyhow::bail!("Неожиданный аргумент команды export: '{}'", s);
                }
            }
            Ok(Command::Export {
                include_secrets,
                file,
            })
        }
        "settings" => {
            if args.len() < 2 {
                anyhow::bail!("Использование: csqtt settings get [ключ] | settings set <ключ> <значение>");
            }
            match args[1].as_ref() {
                "get" => {
                    if args.len() == 2 {
                        Ok(Command::SettingsGet { key: None })
                    } else if args.len() == 3 {
                        Ok(Command::SettingsGet {
                            key: Some(args[2].as_ref().to_string()),
                        })
                    } else {
                        anyhow::bail!("Неожиданный аргумент для settings get: '{}'", args[3].as_ref());
                    }
                }
                "set" => {
                    if args.len() < 4 {
                        anyhow::bail!("Использование: csqtt settings set <ключ> <значение>");
                    }
                    let key = args[2].as_ref().to_string();
                    let value = args[3..].iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ");
                    Ok(Command::SettingsSet { key, value })
                }
                other => {
                    anyhow::bail!("Неизвестное действие settings: '{other}'. Ожидается 'get' или 'set'");
                }
            }
        }
        "secret" => {
            if args.len() < 2 || args[1].as_ref() != "set" {
                anyhow::bail!("Использование: csqtt secret set <ключ> <значение>");
            }
            if args.len() < 4 {
                anyhow::bail!("Использование: csqtt secret set <ключ> <значение>");
            }
            let key = args[2].as_ref().to_string();
            let value = args[3..].iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ");
            Ok(Command::SecretSet { key, value })
        }
        "apps" => {
            if args.len() < 2 {
                anyhow::bail!("Использование: csqtt apps list | apps add <exe> | apps remove <exe>");
            }
            match args[1].as_ref() {
                "list" => {
                    if args.len() > 2 {
                        anyhow::bail!("Неожиданный аргумент для apps list: '{}'", args[2].as_ref());
                    }
                    Ok(Command::AppsList)
                }
                "add" => {
                    if args.len() < 3 {
                        anyhow::bail!("Использование: csqtt apps add <exe>");
                    }
                    let exe = args[2..].iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ");
                    Ok(Command::AppsAdd { exe })
                }
                "remove" => {
                    if args.len() < 3 {
                        anyhow::bail!("Использование: csqtt apps remove <exe>");
                    }
                    let exe = args[2..].iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ");
                    Ok(Command::AppsRemove { exe })
                }
                other => {
                    anyhow::bail!("Неизвестное действие apps: '{other}'. Ожидается 'list', 'add' или 'remove'");
                }
            }
        }
        "vk-login" => {
            if args.len() > 1 {
                anyhow::bail!("Неожиданный аргумент для vk-login: '{}'", args[1].as_ref());
            }
            Ok(Command::VkLogin)
        }
        "vk-logout" => {
            if args.len() > 1 {
                anyhow::bail!("Неожиданный аргумент для vk-logout: '{}'", args[1].as_ref());
            }
            Ok(Command::VkLogout)
        }
        "captcha-window" => {
            if args.len() < 2 {
                anyhow::bail!("Использование: csqtt captcha-window <base64url-json>");
            }
            if args.len() > 2 {
                anyhow::bail!("Неожиданный аргумент для captcha-window: '{}'", args[2].as_ref());
            }
            Ok(Command::CaptchaWindow {
                payload: args[1].as_ref().to_string(),
            })
        }
        "deploy" => {
            let d_args = deploy_cmd::parse_deploy_args(&args[1..])?;
            Ok(Command::Deploy(d_args))
        }
        "undeploy" => {
            let u_args = deploy_cmd::parse_undeploy_args(&args[1..])?;
            Ok(Command::Undeploy(u_args))
        }
        "update-check" => {
            if args.len() > 1 {
                anyhow::bail!("Неожиданный аргумент для update-check: '{}'", args[1].as_ref());
            }
            Ok(Command::UpdateCheck)
        }
        "tray" => Ok(Command::Tray),
        "gui" => Ok(Command::Gui),
        "help" | "-h" | "--help" => Ok(Command::Help),
        other => anyhow::bail!("Неизвестная команда: '{other}'. Используйте 'csqtt help' для списка команд."),
    }
}

/// Parses a setting value as JSON, falling back to a string on error.
pub fn parse_setting_value(raw: &str) -> serde_json::Value {
    match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => serde_json::Value::String(raw.to_string()),
    }
}

/// Formats a single setting value for CLI output.
pub fn format_setting_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// Translates tunnel state to Russian display text.
fn format_state(state: &TunnelState) -> &'static str {
    match state {
        TunnelState::Stopped => "Остановлен",
        TunnelState::Starting => "Запуск",
        TunnelState::Connected => "Подключен",
        TunnelState::Reconnecting => "Переподключение",
        TunnelState::Paused => "Приостановлен",
        TunnelState::Failed => "Ошибка",
    }
}

/// Formats tunnel status into human-readable multi-line Russian text.
pub fn format_status(info: &StatusInfo) -> String {
    let mb_up = (info.bytes_up.max(0) as f64) / (1024.0 * 1024.0);
    let mb_down = (info.bytes_down.max(0) as f64) / (1024.0 * 1024.0);
    let state_name = format_state(&info.state);
    let state_line = if info.message.is_empty() {
        format!("Состояние:   {state_name}")
    } else {
        format!("Состояние:   {} ({})", state_name, info.message)
    };
    let ip_str = if info.client_ip.is_empty() { "-" } else { &info.client_ip };
    let dns_str = if info.dns.is_empty() { "-" } else { &info.dns };

    format!(
        "{state_line}\n\
         IP:          {ip_str}\n\
         DNS:         {dns_str}\n\
         Воркеры:     {}\n\
         Трафик:      ↑ {:.2} МБ / ↓ {:.2} МБ",
        info.active_workers,
        mb_up,
        mb_down,
    )
}

/// Connects to the csqtt-svc IPC pipe.
async fn connect_client() -> anyhow::Result<Client> {
    Client::connect().await.map_err(|e| {
        anyhow::anyhow!("Не удалось подключиться к службе csqtt-svc: {e}")
    })
}

/// Sends a request and expects a single response, converting `Response::Error` into `anyhow::bail!`.
async fn send_and_expect_response(client: &mut Client, req: &Request) -> anyhow::Result<Response> {
    client.send(req).await.map_err(|e| {
        anyhow::anyhow!("Не удалось отправить запрос службе: {e}")
    })?;
    let resp = client.recv().await.map_err(|e| {
        anyhow::anyhow!("Не удалось получить ответ от службы: {e}")
    })?;
    match resp {
        Some(Response::Error { message }) => anyhow::bail!("{message}"),
        Some(r) => Ok(r),
        None => anyhow::bail!("Служба csqtt-svc закрыла соединение без ответа"),
    }
}

async fn handle_connect() -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::Connect).await?;
    match resp {
        Response::Ok => println!("Подключение запущено"),
        Response::Status { status } => println!("{}", format_status(&status)),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

async fn handle_disconnect() -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::Disconnect).await?;
    match resp {
        Response::Ok => println!("Отключено"),
        Response::Status { status } => println!("{}", format_status(&status)),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

async fn handle_status() -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::Status).await?;
    match resp {
        Response::Status { status } => {
            println!("{}", format_status(&status));
        }
        other => {
            anyhow::bail!("Неожиданный ответ службы на запрос статуса: {other:?}");
        }
    }
    Ok(())
}

async fn handle_logs(follow: bool, tail: usize) -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    client.send(&Request::Logs { follow, tail }).await.map_err(|e| {
        anyhow::anyhow!("Не удалось отправить запрос журнала: {e}")
    })?;

    if follow {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
                res = client.recv() => {
                    match res {
                        Ok(Some(Response::Log { line })) => {
                            println!("{line}");
                        }
                        Ok(Some(Response::Error { message })) => {
                            anyhow::bail!("{message}");
                        }
                        Ok(Some(Response::Ok)) | Ok(None) => {
                            break;
                        }
                        Ok(Some(Response::Status { status })) => {
                            let msg = if status.message.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", status.message)
                            };
                            println!("[статус: {}{}]", format_state(&status.state), msg);
                        }
                        Ok(Some(_)) => {}
                        Err(e) => {
                            anyhow::bail!("Ошибка чтения журнала: {e}");
                        }
                    }
                }
            }
        }
    } else {
        while let Some(resp) = client.recv().await.map_err(|e| anyhow::anyhow!("Ошибка чтения журнала: {e}"))? {
            match resp {
                Response::Log { line } => {
                    println!("{line}");
                }
                Response::Error { message } => {
                    anyhow::bail!("{message}");
                }
                Response::Ok => {
                    break;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

async fn handle_import(source: String) -> anyhow::Result<()> {
    let path = Path::new(&source);
    let text = if path.is_file() {
        std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Не удалось прочитать файл '{}': {e}", path.display())
        })?
    } else {
        source
    };

    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::Import { text }).await?;
    match resp {
        Response::Ok => println!("Конфигурация успешно импортирована"),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

async fn handle_export(include_secrets: bool, file: Option<PathBuf>) -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::Export { include_secrets }).await?;
    match resp {
        Response::Exported { json } => {
            if let Some(file_path) = file {
                std::fs::write(&file_path, json.as_bytes()).map_err(|e| {
                    anyhow::anyhow!("Не удалось записать в файл '{}': {e}", file_path.display())
                })?;
                println!("Конфигурация успешно экспортирована в '{}'", file_path.display());
            } else {
                println!("{json}");
            }
        }
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос экспорта: {other:?}"),
    }
    Ok(())
}

/// Settings keys live either at the top level (app-wide) or in the current profile (most
/// connection options). Top level wins; then the active profile.
fn lookup_setting(settings: &serde_json::Value, key: &str) -> anyhow::Result<serde_json::Value> {
    if let Some(v) = settings.get(key) {
        return Ok(v.clone());
    }
    let parsed: csqtt_logic::settings::Settings = serde_json::from_value(settings.clone())
        .map_err(|e| anyhow::anyhow!("Не удалось разобрать настройки службы: {e}"))?;
    let profile = serde_json::to_value(parsed.current_profile())?;
    profile
        .get(key)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Ключ '{key}' не найден ни в настройках, ни в текущем профиле"))
}

/// Change one key at the top level or in the current profile; unknown keys are rejected
/// (the service would silently drop them).
fn apply_setting(settings: &mut serde_json::Value, key: &str, value: serde_json::Value) -> anyhow::Result<()> {
    let map = settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Настройки службы не являются JSON-объектом"))?;
    if key != "profiles" && map.contains_key(key) {
        map.insert(key.to_string(), value);
    } else {
        let mut parsed: csqtt_logic::settings::Settings = serde_json::from_value(settings.clone())
            .map_err(|e| anyhow::anyhow!("Не удалось разобрать настройки службы: {e}"))?;
        let mut profile = serde_json::to_value(parsed.current_profile())?;
        let pmap = profile.as_object_mut().ok_or_else(|| anyhow::anyhow!("профиль не является объектом"))?;
        if !pmap.contains_key(key) {
            anyhow::bail!("Неизвестный ключ настроек: '{key}'");
        }
        pmap.insert(key.to_string(), value);
        *parsed.current_profile_mut() = serde_json::from_value(profile)
            .map_err(|e| anyhow::anyhow!("Неверное значение для '{key}': {e}"))?;
        *settings = serde_json::to_value(&parsed)?;
    }
    // Validate the whole object the way the service will.
    serde_json::from_value::<csqtt_logic::settings::Settings>(settings.clone())
        .map_err(|e| anyhow::anyhow!("Неверное значение для '{key}': {e}"))?;
    Ok(())
}

async fn handle_settings_get(key: Option<String>) -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::GetSettings).await?;
    match resp {
        Response::Settings { settings } => {
            if let Some(k) = key {
                let val = lookup_setting(&settings, &k)?;
                println!("{}", format_setting_value(&val));
            } else {
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
        }
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос настроек: {other:?}"),
    }
    Ok(())
}

async fn handle_settings_set(key: String, value: String) -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::GetSettings).await?;
    let mut settings = match resp {
        Response::Settings { settings } => settings,
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос настроек: {other:?}"),
    };

    apply_setting(&mut settings, &key, parse_setting_value(&value))?;

    let resp = send_and_expect_response(&mut client, &Request::SetSettings { settings }).await?;
    match resp {
        Response::Ok => println!("Настройка '{key}' успешно обновлена"),
        Response::Error { message } => anyhow::bail!("{message}"),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

async fn handle_secret_set(key: String, value: String) -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::SetSecret { key: key.clone(), value }).await?;
    match resp {
        Response::Ok => println!("Секрет '{key}' успешно сохранён"),
        Response::Error { message } => anyhow::bail!("{message}"),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

const BASE64URL_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encodes raw bytes into an unpadded URL-safe base64 string (RFC 4648 §5).
pub fn base64url_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        result.push(BASE64URL_CHARS[(b0 >> 2) as usize] as char);
        result.push(BASE64URL_CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if i + 1 < data.len() {
            result.push(BASE64URL_CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if i + 2 < data.len() {
            result.push(BASE64URL_CHARS[(b2 & 0x3f) as usize] as char);
        }
        i += 3;
    }
    result
}

fn decode_base64url_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'-' | b'+' => Some(62),
        b'_' | b'/' => Some(63),
        _ => None,
    }
}

/// Decodes a URL-safe base64 string (padded or unpadded) into raw bytes.
pub fn base64url_decode(input: &str) -> anyhow::Result<Vec<u8>> {
    let trimmed = input.trim().trim_end_matches('=');
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.len() % 4 == 1 {
        anyhow::bail!("Некорректная длина base64url данных");
    }

    let mut output = Vec::with_capacity(trimmed.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in trimmed.as_bytes() {
        let val = decode_base64url_char(b)
            .ok_or_else(|| anyhow::anyhow!("Недопустимый символ base64url: '{}'", b as char))?;
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

/// Parses comma-separated excluded apps with quote trimming and case-insensitive deduplication.
pub fn parse_excluded_apps(raw: &str) -> Vec<String> {
    let mut list = Vec::new();
    for item in raw.split(',') {
        let trimmed = item.trim().trim_matches('"').trim();
        if !trimmed.is_empty() {
            if !list.iter().any(|existing: &String| existing.eq_ignore_ascii_case(trimmed)) {
                list.push(trimmed.to_string());
            }
        }
    }
    list
}

fn handle_captcha_window(raw_payload: &str) {
    let decoded = match base64url_decode(raw_payload) {
        Ok(d) => d,
        Err(e) => {
            println!("CAPTCHA_RESULT error:{e}");
            std::process::exit(0);
        }
    };

    let val: serde_json::Value = match serde_json::from_slice(&decoded) {
        Ok(v) => v,
        Err(e) => {
            println!("CAPTCHA_RESULT error:invalid json: {e}");
            std::process::exit(0);
        }
    };

    let mode = val.get("mode").and_then(|v| v.as_str()).unwrap_or_default();
    let redirect_uri = val.get("redirect_uri").and_then(|v| v.as_str()).unwrap_or_default();
    let session_token = val.get("session_token").and_then(|v| v.as_str()).unwrap_or_default();
    let solve_method = val.get("solve_method").and_then(|v| v.as_str()).unwrap_or("auto");

    let result = captcha::solve(
        mode,
        redirect_uri,
        session_token,
        solve_method,
        &mut |step| {
            eprintln!("{step}");
        },
    );

    println!("CAPTCHA_RESULT {result}");
    std::process::exit(0);
}

async fn handle_apps_list() -> anyhow::Result<()> {
    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::GetSettings).await?;
    let settings_val = match resp {
        Response::Settings { settings } => settings,
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос настроек: {other:?}"),
    };

    let settings: Settings = serde_json::from_value(settings_val)
        .map_err(|e| anyhow::anyhow!("Не удалось разобрать настройки службы: {e}"))?;

    let apps = parse_excluded_apps(&settings.current_profile().excluded_apps);
    if apps.is_empty() {
        println!("Список исключённых приложений пуст");
    } else {
        for app in apps {
            println!("{app}");
        }
    }
    Ok(())
}

async fn handle_apps_add(exe: String) -> anyhow::Result<()> {
    let app_to_add = exe.trim().trim_matches('"').trim();
    if app_to_add.is_empty() {
        anyhow::bail!("Имя приложения не может быть пустым");
    }

    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::GetSettings).await?;
    let settings_val = match resp {
        Response::Settings { settings } => settings,
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос настроек: {other:?}"),
    };

    let mut settings: Settings = serde_json::from_value(settings_val)
        .map_err(|e| anyhow::anyhow!("Не удалось разобрать настройки службы: {e}"))?;

    let mut apps = parse_excluded_apps(&settings.current_profile().excluded_apps);
    if !apps.iter().any(|existing| existing.eq_ignore_ascii_case(app_to_add)) {
        apps.push(app_to_add.to_string());
    }

    settings.current_profile_mut().excluded_apps = apps.join(",");
    let new_val = serde_json::to_value(&settings)
        .map_err(|e| anyhow::anyhow!("Не удалось сериализовать настройки: {e}"))?;

    let resp = send_and_expect_response(&mut client, &Request::SetSettings { settings: new_val }).await?;
    match resp {
        Response::Ok => {
            println!("Приложение '{app_to_add}' добавлено в исключения (применится при следующем подключении)");
        }
        Response::Error { message } => anyhow::bail!("{message}"),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

async fn handle_apps_remove(exe: String) -> anyhow::Result<()> {
    let app_to_remove = exe.trim().trim_matches('"').trim();
    if app_to_remove.is_empty() {
        anyhow::bail!("Имя приложения не может быть пустым");
    }

    let mut client = connect_client().await?;
    let resp = send_and_expect_response(&mut client, &Request::GetSettings).await?;
    let settings_val = match resp {
        Response::Settings { settings } => settings,
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на запрос настроек: {other:?}"),
    };

    let mut settings: Settings = serde_json::from_value(settings_val)
        .map_err(|e| anyhow::anyhow!("Не удалось разобрать настройки службы: {e}"))?;

    let mut apps = parse_excluded_apps(&settings.current_profile().excluded_apps);
    let initial_len = apps.len();
    apps.retain(|existing| !existing.eq_ignore_ascii_case(app_to_remove));

    if apps.len() == initial_len {
        println!("Приложение '{app_to_remove}' не найдено в списке исключений");
        return Ok(());
    }

    settings.current_profile_mut().excluded_apps = apps.join(",");
    let new_val = serde_json::to_value(&settings)
        .map_err(|e| anyhow::anyhow!("Не удалось сериализовать настройки: {e}"))?;

    let resp = send_and_expect_response(&mut client, &Request::SetSettings { settings: new_val }).await?;
    match resp {
        Response::Ok => {
            println!("Приложение '{app_to_remove}' удалено из исключений (применится при следующем подключении)");
        }
        Response::Error { message } => anyhow::bail!("{message}"),
        other => println!("Ответ службы: {other:?}"),
    }
    Ok(())
}

async fn handle_vk_login() -> anyhow::Result<()> {
    let payload = vk_auth::login(&mut |step| {
        eprintln!("{step}");
    }).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut client = connect_client().await?;
    let key = SecretKey::VkAccessToken.storage_name().to_string();
    let resp = send_and_expect_response(
        &mut client,
        &Request::SetSecret {
            key,
            value: payload.access_token,
        },
    ).await?;

    match resp {
        Response::Ok => {
            println!("{}", payload.user_id);
            Ok(())
        }
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на установку секрета: {other:?}"),
    }
}

async fn handle_vk_logout() -> anyhow::Result<()> {
    vk_auth::clear_session().map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut client = connect_client().await?;
    let key = SecretKey::VkAccessToken.storage_name().to_string();
    let resp = send_and_expect_response(
        &mut client,
        &Request::SetSecret {
            key,
            value: String::new(),
        },
    ).await?;

    match resp {
        Response::Ok => {
            println!("Сессия VK успешно сброшена");
            Ok(())
        }
        Response::Error { message } => anyhow::bail!("{message}"),
        other => anyhow::bail!("Неожиданный ответ службы на сброс секрета: {other:?}"),
    }
}

/// Executes non-tray async commands inside a Tokio runtime.
async fn run_async_command(cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Connect => handle_connect().await,
        Command::Disconnect => handle_disconnect().await,
        Command::Status => handle_status().await,
        Command::Logs { follow, tail } => handle_logs(follow, tail).await,
        Command::Import { source } => handle_import(source).await,
        Command::Export { include_secrets, file } => handle_export(include_secrets, file).await,
        Command::SettingsGet { key } => handle_settings_get(key).await,
        Command::SettingsSet { key, value } => handle_settings_set(key, value).await,
        Command::SecretSet { key, value } => handle_secret_set(key, value).await,
        Command::AppsList => handle_apps_list().await,
        Command::AppsAdd { exe } => handle_apps_add(exe).await,
        Command::AppsRemove { exe } => handle_apps_remove(exe).await,
        Command::VkLogin => handle_vk_login().await,
        Command::VkLogout => handle_vk_logout().await,
        Command::Deploy(args) => deploy_cmd::handle_deploy(args).await,
        Command::Undeploy(args) => deploy_cmd::handle_undeploy(args).await,
        Command::UpdateCheck => deploy_cmd::handle_update_check().await,
        Command::Tray | Command::Gui | Command::Help | Command::CaptchaWindow { .. } => {
            unreachable!("handled before tokio runtime")
        }
    }
}

/// Prints Russian help text for all commands.
fn print_help() {
    println!(
        "csqtt - консольный клиент csqtt-win\n\n\
         Использование:\n  \
           csqtt <команда> [параметры...]\n\n\
         Команды:\n  \
           connect                        Подключиться с сохранёнными настройками\n  \
           disconnect                     Отключить VPN-соединение\n  \
           status                         Показать состояние (статус, IP, DNS, воркеры, трафик)\n  \
           logs [-f] [-n <строк>]         Просмотр журнала (по умолчанию 100 строк, -f потоковый вывод)\n  \
           import <csqtt://...|файл>      Импортировать конфигурацию из ссылки или файла\n  \
           export [--secrets] [файл]      Экспортировать конфигурацию в stdout или файл\n  \
           settings get [ключ]            Получить настройки (все в формате JSON или один ключ)\n  \
           settings set <ключ> <значение> Установить значение настройки (JSON или строка)\n  \
           secret set <ключ> <значение>   Установить значение защищённого секрета\n  \
           apps list                      Список исключённых приложений\n  \
           apps add <exe>                 Добавить приложение в исключения (применится при след. подключении)\n  \
           apps remove <exe>              Удалить приложение из исключений (применится при след. подключении)\n  \
           vk-login                       Авторизация в VK через окно браузера\n  \
           vk-logout                      Выход из VK и удаление сохранённого токена\n  \
           deploy --host H [опции]        Установка сервера CSQTT на VPS по SSH\n  \
                                          (пароль SSH из CSQTT_SSH_PASSWORD или stdin; ввод видим)\n  \
           undeploy --host H [опции]      Удаление сервера CSQTT с VPS по SSH\n  \
           update-check                   Проверить наличие обновлений клиента\n  \
           tray                           Запустить иконку в системном трее\n  \
           gui                            Открыть окно приложения\n  \
           help                           Показать эту справку"
    );
}

/// True when this process is the only one attached to its console, i.e. the console was
/// created for us (double click, shortcut) rather than inherited from a terminal.
fn owns_console() -> bool {
    let mut list = [0u32; 4];
    // SAFETY: valid buffer; returns the number of processes attached to the console.
    let count = unsafe { windows::Win32::System::Console::GetConsoleProcessList(&mut list) };
    count == 1
}

fn main() -> anyhow::Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    let cmd_args = if raw_args.is_empty() {
        &raw_args[..]
    } else {
        &raw_args[1..]
    };

    // Started by a double click (Explorer gave us a console of our own): open the window
    // instead of printing help into a console that closes at once.
    let own_console = owns_console();
    let cmd = if cmd_args.is_empty() && own_console {
        Command::Gui
    } else {
        match parse_command(cmd_args) {
            Ok(c) => c,
            Err(err) => {
                eprintln!("Ошибка: {err}");
                eprintln!("\nИспользуйте 'csqtt help' для просмотра доступных команд.");
                std::process::exit(1);
            }
        }
    };

    // The window and the tray need no console: drop ours so no black window hangs behind
    // them. A console shared with a terminal is kept (the user's shell owns it).
    if matches!(cmd, Command::Gui | Command::Tray) && own_console {
        // SAFETY: detaches this process from its console; no handles are used afterwards.
        let _ = unsafe { windows::Win32::System::Console::FreeConsole() };
    }

    let res = match cmd {
        Command::Tray => tray::run_tray(),
        Command::Gui => gui::run_gui(),
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::CaptchaWindow { payload } => {
            handle_captcha_window(&payload);
            Ok(())
        }
        async_cmd => {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(err) => {
                    eprintln!("Ошибка запуска среды выполнения tokio: {err}");
                    std::process::exit(1);
                }
            };
            rt.block_on(run_async_command(async_cmd))
        }
    };

    if let Err(err) = res {
        eprintln!("{err}");
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_lookup_and_apply_cover_profile_keys() {
        let mut v = serde_json::to_value(csqtt_logic::settings::Settings::default()).unwrap();
        assert_eq!(lookup_setting(&v, "captcha_mode").unwrap(), serde_json::json!("auto"));
        apply_setting(&mut v, "captcha_mode", serde_json::json!("manual")).unwrap();
        assert_eq!(lookup_setting(&v, "captcha_mode").unwrap(), serde_json::json!("manual"));
        apply_setting(&mut v, "logging_enabled", serde_json::json!(false)).unwrap();
        assert_eq!(v["logging_enabled"], serde_json::json!(false));
        assert!(apply_setting(&mut v, "no_such_key", serde_json::json!(1)).is_err());
        assert!(apply_setting(&mut v, "workers_per_hash", serde_json::json!("x")).is_err());
        assert!(lookup_setting(&v, "no_such_key").is_err());
    }
    use serde_json::json;

    #[test]
    fn test_parse_connect() {
        assert_eq!(parse_command(&["connect"]).unwrap(), Command::Connect);
    }

    #[test]
    fn test_parse_disconnect() {
        assert_eq!(parse_command(&["disconnect"]).unwrap(), Command::Disconnect);
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(parse_command(&["status"]).unwrap(), Command::Status);
    }

    #[test]
    fn test_parse_logs_default() {
        assert_eq!(
            parse_command(&["logs"]).unwrap(),
            Command::Logs { follow: false, tail: 100 }
        );
    }

    #[test]
    fn test_parse_logs_options() {
        assert_eq!(
            parse_command(&["logs", "-f"]).unwrap(),
            Command::Logs { follow: true, tail: 100 }
        );
        assert_eq!(
            parse_command(&["logs", "--follow"]).unwrap(),
            Command::Logs { follow: true, tail: 100 }
        );
        assert_eq!(
            parse_command(&["logs", "-n", "50"]).unwrap(),
            Command::Logs { follow: false, tail: 50 }
        );
        assert_eq!(
            parse_command(&["logs", "-n50"]).unwrap(),
            Command::Logs { follow: false, tail: 50 }
        );
        assert_eq!(
            parse_command(&["logs", "-f", "-n", "20"]).unwrap(),
            Command::Logs { follow: true, tail: 20 }
        );
        assert_eq!(
            parse_command(&["logs", "-n", "20", "-f"]).unwrap(),
            Command::Logs { follow: true, tail: 20 }
        );
        assert!(parse_command(&["logs", "-n"]).is_err());
        assert!(parse_command(&["logs", "-n", "invalid"]).is_err());
        assert!(parse_command(&["logs", "-x"]).is_err());
    }

    #[test]
    fn test_parse_import() {
        assert_eq!(
            parse_command(&["import", "csqtt://link"]).unwrap(),
            Command::Import { source: "csqtt://link".into() }
        );
        assert_eq!(
            parse_command(&["import", "config.json"]).unwrap(),
            Command::Import { source: "config.json".into() }
        );
        assert!(parse_command(&["import"]).is_err());
    }

    #[test]
    fn test_parse_export() {
        assert_eq!(
            parse_command(&["export"]).unwrap(),
            Command::Export { include_secrets: false, file: None }
        );
        assert_eq!(
            parse_command(&["export", "--secrets"]).unwrap(),
            Command::Export { include_secrets: true, file: None }
        );
        assert_eq!(
            parse_command(&["export", "-s"]).unwrap(),
            Command::Export { include_secrets: true, file: None }
        );
        assert_eq!(
            parse_command(&["export", "out.json"]).unwrap(),
            Command::Export { include_secrets: false, file: Some(PathBuf::from("out.json")) }
        );
        assert_eq!(
            parse_command(&["export", "--secrets", "out.json"]).unwrap(),
            Command::Export { include_secrets: true, file: Some(PathBuf::from("out.json")) }
        );
        assert_eq!(
            parse_command(&["export", "out.json", "--secrets"]).unwrap(),
            Command::Export { include_secrets: true, file: Some(PathBuf::from("out.json")) }
        );
        assert!(parse_command(&["export", "out1.json", "out2.json"]).is_err());
    }

    #[test]
    fn test_parse_settings() {
        assert_eq!(
            parse_command(&["settings", "get"]).unwrap(),
            Command::SettingsGet { key: None }
        );
        assert_eq!(
            parse_command(&["settings", "get", "peer"]).unwrap(),
            Command::SettingsGet { key: Some("peer".into()) }
        );
        assert!(parse_command(&["settings", "get", "peer", "extra"]).is_err());

        assert_eq!(
            parse_command(&["settings", "set", "workers_per_hash", "18"]).unwrap(),
            Command::SettingsSet { key: "workers_per_hash".into(), value: "18".into() }
        );
        assert_eq!(
            parse_command(&["settings", "set", "theme", "dark", "mode"]).unwrap(),
            Command::SettingsSet { key: "theme".into(), value: "dark mode".into() }
        );
        assert!(parse_command(&["settings", "set", "key"]).is_err());
        assert!(parse_command(&["settings", "set"]).is_err());
        assert!(parse_command(&["settings"]).is_err());
        assert!(parse_command(&["settings", "unknown"]).is_err());
    }

    #[test]
    fn test_parse_secret() {
        assert_eq!(
            parse_command(&["secret", "set", "conn_pass", "p@ssword"]).unwrap(),
            Command::SecretSet { key: "conn_pass".into(), value: "p@ssword".into() }
        );
        assert!(parse_command(&["secret", "set", "conn_pass"]).is_err());
        assert!(parse_command(&["secret", "set"]).is_err());
        assert!(parse_command(&["secret"]).is_err());
        assert!(parse_command(&["secret", "get"]).is_err());
    }

    #[test]
    fn test_parse_tray() {
        assert_eq!(parse_command(&["tray"]).unwrap(), Command::Tray);
    }

    #[test]
    fn test_parse_gui() {
        assert_eq!(parse_command(&["gui"]).unwrap(), Command::Gui);
    }

    #[test]
    fn test_parse_help() {
        assert_eq!(parse_command::<&str>(&[]).unwrap(), Command::Help);
        assert_eq!(parse_command(&["help"]).unwrap(), Command::Help);
        assert_eq!(parse_command(&["-h"]).unwrap(), Command::Help);
        assert_eq!(parse_command(&["--help"]).unwrap(), Command::Help);
    }

    #[test]
    fn test_parse_unknown() {
        assert!(parse_command(&["unknown_cmd"]).is_err());
    }

    #[test]
    fn test_parse_setting_value_types() {
        assert_eq!(parse_setting_value("123"), json!(123));
        assert_eq!(parse_setting_value("-50"), json!(-50));
        assert_eq!(parse_setting_value("true"), json!(true));
        assert_eq!(parse_setting_value("false"), json!(false));
        assert_eq!(parse_setting_value("null"), json!(null));
        assert_eq!(parse_setting_value("[1, 2, 3]"), json!([1, 2, 3]));
        assert_eq!(parse_setting_value("{\"k\":\"v\"}"), json!({"k": "v"}));
        assert_eq!(parse_setting_value("\"quoted\""), json!("quoted"));
        assert_eq!(parse_setting_value("plain string"), json!("plain string"));
        assert_eq!(parse_setting_value("10.0.0.1:46000"), json!("10.0.0.1:46000"));
        assert_eq!(parse_setting_value(""), json!(""));
    }

    #[test]
    fn test_format_setting_value_output() {
        assert_eq!(format_setting_value(&json!("hello")), "hello");
        assert_eq!(format_setting_value(&json!(123)), "123");
        assert_eq!(format_setting_value(&json!(true)), "true");
        assert_eq!(format_setting_value(&json!(null)), "null");
        assert!(format_setting_value(&json!([1, 2])).contains("1"));
    }

    #[test]
    fn test_format_status_output() {
        let status = StatusInfo {
            state: TunnelState::Connected,
            message: String::new(),
            client_ip: "10.0.0.2".into(),
            dns: "77.88.8.8".into(),
            active_workers: 18,
            bytes_up: 10 * 1024 * 1024,
            bytes_down: 25 * 1024 * 1024,
            connected_since_ms: None,
        };
        let text = format_status(&status);
        assert!(text.contains("Подключен"));
        assert!(text.contains("10.0.0.2"));
        assert!(text.contains("77.88.8.8"));
        assert!(text.contains("18"));
        assert!(text.contains("10.00 МБ"));
        assert!(text.contains("25.00 МБ"));
    }

    #[test]
    fn test_format_status_with_message() {
        let status = StatusInfo {
            state: TunnelState::Failed,
            message: "Неверный пароль".into(),
            client_ip: String::new(),
            dns: String::new(),
            active_workers: 0,
            bytes_up: 0,
            bytes_down: 0,
            connected_since_ms: None,
        };
        let text = format_status(&status);
        assert!(text.contains("Ошибка (Неверный пароль)"));
        assert!(text.contains("IP:          -"));
        assert!(text.contains("DNS:         -"));
    }

    #[test]
    fn test_parse_captcha_window() {
        assert_eq!(
            parse_command(&["captcha-window", "eyJtb2RlIjoiYXV0byJ9"]).unwrap(),
            Command::CaptchaWindow {
                payload: "eyJtb2RlIjoiYXV0byJ9".into()
            }
        );
        assert!(parse_command(&["captcha-window"]).is_err());
        assert!(parse_command(&["captcha-window", "arg1", "arg2"]).is_err());
    }

    #[test]
    fn test_parse_vk_commands() {
        assert_eq!(parse_command(&["vk-login"]).unwrap(), Command::VkLogin);
        assert!(parse_command(&["vk-login", "extra"]).is_err());
        assert_eq!(parse_command(&["vk-logout"]).unwrap(), Command::VkLogout);
        assert!(parse_command(&["vk-logout", "extra"]).is_err());
    }

    #[test]
    fn test_parse_apps() {
        assert_eq!(parse_command(&["apps", "list"]).unwrap(), Command::AppsList);
        assert!(parse_command(&["apps", "list", "extra"]).is_err());

        assert_eq!(
            parse_command(&["apps", "add", "telegram.exe"]).unwrap(),
            Command::AppsAdd { exe: "telegram.exe".into() }
        );
        assert_eq!(
            parse_command(&["apps", "add", "C:\\Program Files\\App.exe"]).unwrap(),
            Command::AppsAdd { exe: "C:\\Program Files\\App.exe".into() }
        );
        assert!(parse_command(&["apps", "add"]).is_err());

        assert_eq!(
            parse_command(&["apps", "remove", "telegram.exe"]).unwrap(),
            Command::AppsRemove { exe: "telegram.exe".into() }
        );
        assert!(parse_command(&["apps", "remove"]).is_err());
        assert!(parse_command(&["apps"]).is_err());
        assert!(parse_command(&["apps", "unknown"]).is_err());
    }

    #[test]
    fn test_base64url_rfc4648_vectors() {
        assert_eq!(base64url_encode(b""), "");
        assert_eq!(base64url_encode(b"f"), "Zg");
        assert_eq!(base64url_encode(b"fo"), "Zm8");
        assert_eq!(base64url_encode(b"foo"), "Zm9v");
        assert_eq!(base64url_encode(b"foob"), "Zm9vYg");
        assert_eq!(base64url_encode(b"fooba"), "Zm9vYmE");
        assert_eq!(base64url_encode(b"foobar"), "Zm9vYmFy");

        assert_eq!(base64url_decode("").unwrap(), b"");
        assert_eq!(base64url_decode("Zg").unwrap(), b"f");
        assert_eq!(base64url_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64url_decode("Zm8").unwrap(), b"fo");
        assert_eq!(base64url_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64url_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64url_decode("Zm9vYg").unwrap(), b"foob");
        assert_eq!(base64url_decode("Zm9vYmE").unwrap(), b"fooba");
        assert_eq!(base64url_decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn test_base64url_special_chars_and_errors() {
        let data = vec![251, 240, 239];
        let enc = base64url_encode(&data);
        assert!(enc.contains('-'));
        assert!(enc.contains('_'));
        let dec = base64url_decode(&enc).unwrap();
        assert_eq!(dec, data);

        assert_eq!(base64url_decode("-_8").unwrap(), base64url_decode("+/8").unwrap());
        assert!(base64url_decode("invalid!char").is_err());
        assert!(base64url_decode("Z").is_err());
    }

    #[test]
    fn test_base64url_json_roundtrip() {
        let json = r#"{"mode":"auto","redirect_uri":"https://oauth.vk.com/blank.html","session_token":"xyz"}"#;
        let enc = base64url_encode(json.as_bytes());
        let dec = base64url_decode(&enc).unwrap();
        assert_eq!(String::from_utf8(dec).unwrap(), json);
    }

    #[test]
    fn test_parse_excluded_apps() {
        assert_eq!(parse_excluded_apps(""), Vec::<String>::new());
        assert_eq!(parse_excluded_apps("   "), Vec::<String>::new());
        assert_eq!(
            parse_excluded_apps("telegram.exe, chrome.exe"),
            vec!["telegram.exe", "chrome.exe"]
        );
        assert_eq!(
            parse_excluded_apps("telegram.exe, TELEGRAM.EXE, Chrome.exe, telegram.exe"),
            vec!["telegram.exe", "Chrome.exe"]
        );
        assert_eq!(
            parse_excluded_apps(r#""C:\Program Files\App.exe", "telegram.exe""#),
            vec!["C:\\Program Files\\App.exe", "telegram.exe"]
        );
        assert_eq!(
            parse_excluded_apps(",,telegram.exe,  ,,"),
            vec!["telegram.exe"]
        );
    }

    #[test]
    fn test_parse_deploy_command() {
        assert_eq!(
            parse_command(&["deploy", "--host", "example.com"]).unwrap(),
            Command::Deploy(deploy_cmd::DeployArgs {
                host: "example.com".into(),
                port: 22,
                user: "root".into(),
                key: None,
                passphrase_env: None,
                sudo: false,
                docker: false,
                peer_port: 46000,
                web_port: 46002,
                web_login: None,
                secret_path: "".into(),
                assets: None,
            })
        );
    }

    #[test]
    fn test_parse_undeploy_command() {
        assert_eq!(
            parse_command(&["undeploy", "--host", "example.com", "--port", "2222"]).unwrap(),
            Command::Undeploy(deploy_cmd::UndeployArgs {
                host: "example.com".into(),
                port: 2222,
                user: "root".into(),
                key: None,
                passphrase_env: None,
                sudo: false,
                peer_port: 46000,
                assets: None,
            })
        );
    }

    #[test]
    fn test_parse_update_check_command() {
        assert_eq!(
            parse_command(&["update-check"]).unwrap(),
            Command::UpdateCheck
        );
        assert!(parse_command(&["update-check", "extra"]).is_err());
    }
}
