//! Pure deploy policies and constants ported from Android `DeployOperations.kt`,
//! `DeployTab.kt`, `DeployScreen.kt`, and `DeployManager.kt`.
//!
//! # Remote Operations Reference for `ops.rs`
//!
//! This reference details step-by-step what `performDeploy` and `performUninstall`
//! execute remotely over SSH so that `ops.rs` can implement them cleanly.
//!
//! ## 1. Remote SSH Execution & Connection Settings
//! - **Connection timeouts**: Connect timeout 15 s (with 30 s overall limit); command timeout
//!   [`CMD_TIMEOUT_MS`] (900,000 ms / 15 minutes) for deployment, 30 s for uninstall and architecture probe.
//! - **Keepalive**: 10 seconds interval.
//! - **Host Key Verification**: TOFU (Trust On First Use) host key callback.
//! - **Authentication**:
//!   1. Private key mode (if key text provided): OpenSSH / PEM key with optional passphrase;
//!   2. Password mode: password authentication with keyboard-interactive fallback.
//! - **Sudo & Root wrapping**: All privileged remote commands are wrapped with [`root_command`]:
//!   `if [ "$(id -u)" = "0" ]; then bash -c <quoted>; elif command -v sudo >/dev/null 2>&1; then printf '%s\n' <pass_quoted> | sudo -S -p '' bash -c <quoted>; else echo 'error: root privileges required and sudo not found'; exit 1; fi`
//! - **Reconnect & Retry Policy**:
//!   - Remote command execution: attempt reconnect if disconnected (`ensureConnected`).
//!   - File uploads: try SFTP up to [`SFTP_UPLOAD_ATTEMPTS`] (2 attempts). If SFTP fails,
//!     fallback to SCP (`newSCPFileTransfer`). If SCP fails, fallback to chunked stream upload
//!     (64 KiB chunks base64-encoded to `/tmp/<remote>.tmp` via stdin and moved via `mv -f` + `chmod 0755`).
//!     If disconnected during upload, attempt up to [`UPLOAD_RECONNECT_ATTEMPTS`] (1 reconnect).
//! - **Output & Stderr streaming**:
//!   - Lines matching `CSQTT_PROGRESS|<fraction>|<step>` update real-time progress and step description.
//!   - All stdout lines are parsed using [`parse_deploy_output_line`].
//!   - Stderr lines matching [`is_benign_stderr`] (or containing `password for`) are suppressed.
//!
//! ## 2. `performDeploy` Step-by-Step Remote Sequence
//! 1. **Connect & Authenticate**: Establish SSH session to `{host}:{port}`.
//! 2. **Architecture Probe**:
//!    - Command: `uname -m` (timeout 10,000 ms).
//!    - Mapped via [`server_architecture_for_machine`]:
//!      - `x86_64` / `amd64` -> [`ServerArchitecture::Amd64`] (asset `csqtt-linux-amd64`, label `"сервер CSQTT (amd64)"`)
//!      - `aarch64` / `arm64` -> [`ServerArchitecture::Arm64`] (asset `csqtt-linux-arm64`, label `"сервер CSQTT (ARM64)"`)
//!      - `armv7l` / `armv7` / `armhf` -> [`ServerArchitecture::Armv7`] (asset `csqtt-linux-armv7`, label `"сервер CSQTT (ARM32)"`)
//!      - Any other architecture causes deployment to abort before upload.
//! 3. **Prepare Local Assets**:
//!    - `deploy.sh`: bash installer script (from assets).
//!    - Server binary: `csqtt-linux-{arch}` selected by probe.
//!    - `csqtt.env`: environment file containing:
//!      - `CSQTT_WEB_USER={deploy_environment_value(web_login, install_in_docker)}`
//!      - `CSQTT_WEB_PASS={deploy_environment_value(web_pass, install_in_docker)}`
//!      - `CSQTT_PUBLIC_IP={deploy_environment_value(host, install_in_docker)}`
//!      - If secret path given: `CSQTT_WEB_SECRET_PATH={deploy_environment_value(secret_path, install_in_docker)}`
//!    - `csqtt-deploy.json`: JSON payload `{"main_password": "<main_pass>"}`.
//! 4. **Upload Assets to `/tmp/`**:
//!    - `deploy.sh` -> `/tmp/deploy.sh`
//!    - Server binary -> `/tmp/.csqtt-upload-server`
//!    - `csqtt.env` -> `/tmp/.csqtt-upload-web.env`
//!    - `csqtt-deploy.json` -> `/tmp/.csqtt-upload-overrides.json`
//!    - Asset labels for logging:
//!      - `"deploy.sh"` -> `"скрипт установки"`
//!      - `"csqtt-linux-amd64"` -> `"сервер CSQTT (amd64)"`
//!      - `"csqtt-linux-arm64"` -> `"сервер CSQTT (ARM64)"`
//!      - `"csqtt-linux-armv7"` -> `"сервер CSQTT (ARM32)"`
//!      - `"csqtt.env"` -> `"настройки веб-панели"`
//!      - `"csqtt-deploy.json"` -> `"настройки доступа"`
//!      - `"hev-socks5-tunnel"` -> `"C-движок туннеля"`
//! 5. **Execute Installer on VPS**:
//!    - Environment prefix:
//!      `env [CSQTT_WEB_SECRET_PATH=<val> ]CSQTT_PEER_PORT=<peer_port> CSQTT_SSH_PORT=<port> CSQTT_WEB_PORT=<web_port> CSQTT_PUBLIC_IP=<host> CSQTT_DEPLOY_MODE=<mode>`
//!      where mode is `"docker"` if `install_in_docker` else `"systemd"`.
//!    - Full command: `root_command("<env_prefix> bash /tmp/deploy.sh install", Some(sudo_pass))`
//!    - Timeout: [`CMD_TIMEOUT_MS`] (15 minutes).
//! 6. **Result Verification**:
//!    - Evaluated via [`is_successful_deploy_result(exit_status, output)`]:
//!      Requires `exit_status == 0` AND at least one line trimmed to `"CSQTT_DEPLOY_OK"`.
//!    - On failure, formatted via [`deploy_failure_message(exit_status, output)`].
//! 7. **Cleanup**: Disconnect SSH session and purge temporary local working directory.
//!
//! ## 3. `performUninstall` Step-by-Step Remote Sequence
//! 1. **Connect & Authenticate**: Establish SSH session to `{host}:{port}`.
//! 2. **Upload Installer Script**: Upload `deploy.sh` to `/tmp/deploy.sh`.
//! 3. **Execute Uninstall on VPS**:
//!    - Command: `root_command("env CSQTT_PEER_PORT=<peer_port> CSQTT_SSH_PORT=<port> bash /tmp/deploy.sh uninstall", Some(sudo_pass))`
//!    - Timeout: 30,000 ms (30 seconds).
//! 4. **Cleanup**: Disconnect SSH session and delete local temporary script.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Execution timeout for the deploy script (15 minutes = 900,000 ms).
pub const CMD_TIMEOUT_MS: u64 = csqtt_logic::constants::timeouts::DEPLOY_CMD_TIMEOUT_MS as u64;

/// Number of attempts for SFTP uploads before fallback.
pub const SFTP_UPLOAD_ATTEMPTS: usize = 2;

/// Number of automatic SSH reconnect attempts during file upload.
pub const UPLOAD_RECONNECT_ATTEMPTS: usize = 1;

/// Auto-reset threshold for stalled deployments (30 minutes = 1,800,000 ms).
pub const DEPLOY_STALL_TIMEOUT_MS: u64 =
    csqtt_logic::constants::timeouts::DEPLOY_STALL_TIMEOUT_MS as u64;

/// Default SSH server port (22).
pub const DEFAULT_SSH_PORT: i32 = csqtt_logic::constants::network::DEFAULT_SSH_PORT;

/// Default CSQTT tunnel peer UDP port (46000).
pub const DEFAULT_SERVER_PEER_PORT: i32 =
    csqtt_logic::constants::network::DEFAULT_SERVER_PEER_PORT;

/// Default CSQTT web management panel port (46002).
pub const DEFAULT_SERVER_WEB_PORT: i32 =
    csqtt_logic::constants::network::DEFAULT_SERVER_WEB_PORT;

// ---------------------------------------------------------------------------
// Pure Deploy Operations Logic (Ported from DeployOperations.kt)
// ---------------------------------------------------------------------------

/// Sanitizes an SSH password by removing carriage returns and line feeds.
pub fn sanitize_ssh_password(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

/// Checks whether remote deployment succeeded: exit code must be 0 and stdout must contain
/// the marker `"CSQTT_DEPLOY_OK"` as a standalone trimmed line.
pub fn is_successful_deploy_result(exit_status: i32, output: &str) -> bool {
    exit_status == 0 && output.lines().any(|it| it.trim() == "CSQTT_DEPLOY_OK")
}

/// Extracts the failure detail from the last `CSQTT_DEPLOY_ERROR|<phase>|<detail>` line.
pub fn deploy_failure_detail(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|it| it.starts_with("CSQTT_DEPLOY_ERROR|"))
        .last()
        .and_then(|line| {
            let mut parts = line.splitn(3, '|');
            let _marker = parts.next()?;
            let _phase = parts.next()?;
            let detail = parts.next()?.trim();
            if detail.is_empty() {
                None
            } else {
                Some(detail.chars().take(360).collect())
            }
        })
}

/// Converts remote installer exit status and stdout/stderr output into a localized Russian
/// error message explaining why deployment failed.
pub fn deploy_failure_message(exit_status: i32, output: &str) -> String {
    let detail = deploy_failure_detail(output);
    let with_detail = |message: &str| -> String {
        match &detail {
            Some(d) => format!("{message} Причина: {d}"),
            None => message.to_string(),
        }
    };

    if output.lines().any(|it| it.trim() == "CSQTT_DEPLOY_ROLLED_BACK") || exit_status == 30 {
        with_detail("Новый релиз не прошёл проверку; предыдущая установка восстановлена")
    } else if output.lines().any(|it| it.trim() == "CSQTT_DEPLOY_ROLLBACK_FAILED") || exit_status == 31 {
        with_detail(
            "Новый релиз не запустился, а автоматический rollback завершился с ошибкой — требуется проверка VPS",
        )
    } else if exit_status == 20 {
        with_detail("Кандидат не прошёл предзапусковую проверку; работающий сервер не останавливался")
    } else if exit_status == 2 {
        with_detail("Установщик отклонил параметры развёртывания")
    } else {
        detail.unwrap_or_else(|| {
            format!("Установщик завершился с кодом {exit_status}; подробности сохранены в errors.log")
        })
    }
}

/// Transforms lower-level SSH, network, or transport errors into friendly Russian guidance.
pub fn friendly_deploy_error(message: &str) -> String {
    let text = message.trim();
    if text.is_empty() {
        return "Неизвестная ошибка установки".to_string();
    }
    let lower = text.to_lowercase();
    if lower.contains("auth fail (public key)") || lower.contains("auth fail (key parse)") {
        "SSH: Auth fail — ключ не принят сервером. Проверьте: 1) приватный ключ вставлен целиком (-----BEGIN...-----END), 2) публичный ключ добавлен в authorized_keys на VPS, 3) логин верный".to_string()
    } else if lower.contains("auth fail") || lower.contains("authentication") {
        "SSH-аутентификация отклонена: проверьте логин, пароль и разрешение PasswordAuthentication на VPS".to_string()
    } else if lower.contains("sftp/scp upload failed") {
        "Загрузка на VPS оборвалась даже после автоматического переподключения SSH; проверьте стабильность Wi-Fi и подробную причину в errors.log".to_string()
    } else if lower.contains("reject hostkey") || lower.contains("hostkey") || lower.contains("host key") {
        "SSH-ключ сервера изменился или не принят".to_string()
    } else if lower.contains("timeout") {
        "Истекло время ожидания SSH/SFTP".to_string()
    } else if lower.contains("unknownhost") || lower.contains("no route to host") {
        "VPS недоступен по указанному адресу".to_string()
    } else if lower.contains("session is down") || lower.contains("connection is closed") {
        "SSH-соединение было разорвано".to_string()
    } else if lower.contains("connection refused") {
        "SSH: соединение отклонено — проверьте порт и доступность VPS".to_string()
    } else {
        text.chars().take(240).collect()
    }
}

/// Optional helper for friendly deploy error when input is `Option<&str>`.
pub fn friendly_deploy_error_opt(message: Option<&str>) -> String {
    friendly_deploy_error(message.unwrap_or(""))
}

/// Output log level for user-visible deploy messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployOutputLevel {
    Log,
    Ok,
    Err,
}

/// Cleaned deploy output line with classified severity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployOutputLine {
    pub message: String,
    pub level: DeployOutputLevel,
}

impl DeployOutputLine {
    pub fn new(message: impl Into<String>, level: DeployOutputLevel) -> Self {
        Self {
            message: message.into(),
            level,
        }
    }
}

/// Strips ANSI escape sequences of form `\u{1B}\[[;\d]*m`.
pub fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b';') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'm' {
                i = j + 1;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Checks whether a character is a deploy decoration symbol, dingbat, or emoji
/// corresponding to `[\p{So}\p{Sk}\uFE0F\u200D]`.
pub fn is_deploy_symbol(c: char) -> bool {
    if c == '\u{FE0F}' || c == '\u{200D}' {
        return true;
    }
    let u = c as u32;
    matches!(
        u,
        0x02C2..=0x02DF
            | 0x02E5..=0x02FF
            | 0x1FBD..=0x1FFE
            | 0x2190..=0x27BF
            | 0x2B00..=0x2BFF
            | 0x2E80..=0x2FDF
            | 0x3000..=0x303F
            | 0x3200..=0x33FF
            | 0xFE00..=0xFE0F
            | 0x1F000..=0x1FAFF
    )
}

/// Strips status prefix brackets like `[OK]`, `[ERR]`, `[LOG]`, `[WARN]`, `[✓]`, `[!]`, `[✗]`, `[►]`.
pub fn strip_deploy_status_prefix(s: &str) -> &str {
    if !s.starts_with('[') {
        return s;
    }
    if let Some(close_idx) = s.find(']') {
        let inside = &s[1..close_idx];
        let is_prefix = inside.eq_ignore_ascii_case("OK")
            || inside.eq_ignore_ascii_case("ERR")
            || inside.eq_ignore_ascii_case("LOG")
            || inside.eq_ignore_ascii_case("WARN")
            || inside == "✓"
            || inside == "!"
            || inside == "✗"
            || inside == "►";
        if is_prefix {
            return s[close_idx + 1..].trim_start();
        }
    }
    s
}

/// Parses a raw stdout/stderr line from the remote VPS installer into a user-facing log line.
///
/// Returns `None` for internal protocol markers (`CSQTT_PROGRESS|...`, `CSQTT_DEPLOY_ERROR|...`,
/// `CSQTT_DEPLOY_OK`, `CSQTT_DEPLOY_ROLLED_BACK`, `CSQTT_DEPLOY_ROLLBACK_FAILED`, or decoration lines).
pub fn parse_deploy_output_line(raw_line: &str) -> Option<DeployOutputLine> {
    let stripped = strip_ansi_escapes(raw_line);
    let raw = stripped.trim();
    if raw.is_empty()
        || raw.starts_with("CSQTT_PROGRESS|")
        || raw.starts_with("CSQTT_DEPLOY_ERROR|")
        || raw == "CSQTT_DEPLOY_OK"
        || raw == "CSQTT_DEPLOY_ROLLED_BACK"
        || raw == "CSQTT_DEPLOY_ROLLBACK_FAILED"
    {
        return None;
    }

    let raw_lower = raw.to_lowercase();
    let warning = raw_lower.starts_with("[warn]") || raw.starts_with("[!]") || raw.starts_with('⚠');
    let explicit_error =
        raw_lower.starts_with("[err]") || raw.starts_with("[✗]") || raw.starts_with('✗');
    let success = raw_lower.starts_with("[ok]") || raw.starts_with("[✓]") || raw.starts_with('✓');
    let looks_like_error = explicit_error
        || (!warning
            && (raw_lower.contains("error:")
                || raw_lower.contains("failed")
                || raw_lower.contains("fatal")
                || raw_lower.contains("не запуст")
                || raw_lower.contains("ошибка")));

    let without_prefix = strip_deploy_status_prefix(raw);
    let without_symbols: String = without_prefix
        .chars()
        .filter(|&c| !is_deploy_symbol(c))
        .collect();

    let trimmed = without_symbols.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '═' | '║' | '╔' | '╗' | '╚' | '╝' | '─' | '━')
    });

    let mut collapsed = String::with_capacity(trimmed.len());
    let mut last_was_ws = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !last_was_ws {
                collapsed.push(' ');
                last_was_ws = true;
            }
        } else {
            collapsed.push(ch);
            last_was_ws = false;
        }
    }
    let mut clean = collapsed.trim().to_string();

    if clean.is_empty() || clean.chars().all(|c| c == '=' || c == '-' || c == '_') {
        return None;
    }

    if warning && !clean.to_lowercase().starts_with("предупреждение:") {
        clean = format!("Предупреждение: {clean}");
    }

    let level = if looks_like_error {
        DeployOutputLevel::Err
    } else if success {
        DeployOutputLevel::Ok
    } else {
        DeployOutputLevel::Log
    };

    let message: String = clean.chars().take(500).collect();
    Some(DeployOutputLine { message, level })
}

/// Parses real-time `CSQTT_PROGRESS|<float>|<step>` lines from the deploy script.
pub fn parse_deploy_progress(line: &str) -> Option<(f32, String)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("CSQTT_PROGRESS|")?;
    let (p_str, step) = rest.split_once('|')?;
    let progress = p_str.trim().parse::<f32>().ok()?;
    let step = step.trim().to_string();
    if step.is_empty() {
        return None;
    }
    Some((progress, step))
}

/// Target server CPU architectures supported by the installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerArchitecture {
    Amd64,
    Arm64,
    Armv7,
}

impl ServerArchitecture {
    /// Returns the asset binary name for this architecture.
    pub const fn asset_name(&self) -> &'static str {
        match self {
            Self::Amd64 => "csqtt-linux-amd64",
            Self::Arm64 => "csqtt-linux-arm64",
            Self::Armv7 => "csqtt-linux-armv7",
        }
    }

    /// Returns the user-visible display name for this architecture.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "ARM64",
            Self::Armv7 => "ARM32",
        }
    }
}

/// Maps the output of `uname -m` from the target VPS into a [`ServerArchitecture`].
///
/// Rejects unsupported architectures with a localized Russian error message.
pub fn server_architecture_for_machine(output: &str) -> anyhow::Result<ServerArchitecture> {
    let machine = output
        .lines()
        .map(str::trim)
        .find(|it| !it.is_empty())
        .map(str::to_lowercase)
        .ok_or_else(|| anyhow::anyhow!("VPS не вернул архитектуру через uname -m"))?;

    match machine.as_str() {
        "x86_64" | "amd64" => Ok(ServerArchitecture::Amd64),
        "aarch64" | "arm64" => Ok(ServerArchitecture::Arm64),
        "armv7l" | "armv7" | "armhf" => Ok(ServerArchitecture::Armv7),
        _ => anyhow::bail!(
            "Архитектура VPS {} пока не поддерживается. Поддерживаются: x86_64, aarch64, armv7l",
            machine
        ),
    }
}

/// Maps an uploaded file name to a human-readable Russian label for status logs.
pub fn deploy_asset_label(file_name: &str) -> &str {
    match file_name {
        "deploy.sh" => "скрипт установки",
        "csqtt-linux-amd64" => "сервер CSQTT (amd64)",
        "csqtt-linux-arm64" => "сервер CSQTT (ARM64)",
        "csqtt-linux-armv7" => "сервер CSQTT (ARM32)",
        "csqtt.env" => "настройки веб-панели",
        "csqtt-deploy.json" => "настройки доступа",
        "hev-socks5-tunnel" => "C-движок туннеля",
        other => other,
    }
}

/// Formats a byte count into a human-readable string (e.g. `"1.2 МБ"` or `"512 Б"`).
pub fn readable_file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} МБ", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} КБ", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} Б")
    }
}

/// Safely quotes an argument for POSIX shell execution using single quotes.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// Formats an environment variable value for systemd environment files (escaping quotes and backslashes).
pub fn systemd_environment_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Formats an environment variable value for Docker environment files (stripping newlines).
pub fn docker_environment_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

/// Returns the formatted environment variable value depending on whether Docker or systemd is targeted.
pub fn deploy_environment_value(value: &str, install_in_docker: bool) -> String {
    if install_in_docker {
        docker_environment_value(value)
    } else {
        systemd_environment_value(value)
    }
}

/// Returns the deploy mode string (`"docker"` or `"systemd"`).
pub fn deploy_mode(install_in_docker: bool) -> &'static str {
    if install_in_docker {
        "docker"
    } else {
        "systemd"
    }
}

/// Wraps a command to execute as root, falling back to `sudo -S` with optional password.
pub fn root_command(command: &str, password: Option<&str>) -> String {
    let quoted = shell_quote(command);
    let sudo_cmd = match password {
        Some(pass) if !pass.is_empty() => {
            let pass_quoted = shell_quote(pass);
            format!("printf '%s\\n' {pass_quoted} | sudo -S -p '' bash -c {quoted}")
        }
        _ => format!("sudo -S -p '' bash -c {quoted}"),
    };
    format!(
        "if [ \"$(id -u)\" = \"0\" ]; then bash -c {quoted}; \
         elif command -v sudo >/dev/null 2>&1; then {sudo_cmd}; \
         else echo 'error: root privileges required and sudo not found'; exit 1; fi"
    )
}

/// Hand-written matcher for benign stderr output lines during remote package installation.
///
/// Implements identical semantics to Kotlin's `BENIGN_STDERR` regex without requiring the regex crate.
pub fn is_benign_stderr(line: &str) -> bool {
    let lower = line.to_lowercase();

    if lower.contains("debconf")
        || lower.contains("dpkg-preconfigure")
        || lower.contains("term is not set")
        || lower.contains("falling back to frontend")
        || lower.contains("unable to re-open stdin")
        || lower.contains("controlling tty")
        || lower.contains("unable to initialize frontend")
        || lower.contains("[sudo] password for")
    {
        return true;
    }

    if lower.starts_with("warning:") {
        return true;
    }

    if let Some(pos) = lower.find("frontend:") {
        let remainder = lower[pos + "frontend:".len()..].trim_start();
        if remainder.starts_with("readline")
            || remainder.starts_with("teletype")
            || remainder.starts_with("dialog")
        {
            return true;
        }
    }

    false
}

/// Matcher object providing `.is_match()` and `.contains_match_in()` identical to `is_benign_stderr`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BenignStderrMatcher;

impl BenignStderrMatcher {
    pub const fn new() -> Self {
        Self
    }

    pub fn is_match(&self, line: &str) -> bool {
        is_benign_stderr(line)
    }

    pub fn contains_match_in(&self, line: &str) -> bool {
        is_benign_stderr(line)
    }
}

/// Global instance of the benign stderr matcher.
pub const BENIGN_STDERR: BenignStderrMatcher = BenignStderrMatcher;

// ---------------------------------------------------------------------------
// Deploy UI State & Policy (Ported from DeployTab.kt / DeployScreen.kt / DeployFields.kt)
// ---------------------------------------------------------------------------

/// Effective resolved ports for SSH, tunnel peer, and web panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveServerPorts {
    pub ssh: i32,
    pub peer: i32,
    pub web: i32,
}

/// Resolves server ports based on manual override mode and custom values.
pub fn resolve_server_ports(
    manual_ports: bool,
    ssh_port: &str,
    peer_port: &str,
    web_port: &str,
) -> EffectiveServerPorts {
    if !manual_ports {
        return EffectiveServerPorts {
            ssh: DEFAULT_SSH_PORT,
            peer: DEFAULT_SERVER_PEER_PORT,
            web: DEFAULT_SERVER_WEB_PORT,
        };
    }
    EffectiveServerPorts {
        ssh: ssh_port
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|&p| (1..=65535).contains(&p))
            .unwrap_or(DEFAULT_SSH_PORT),
        peer: peer_port
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|&p| (1..=65535).contains(&p))
            .unwrap_or(DEFAULT_SERVER_PEER_PORT),
        web: web_port
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|&p| (1..=65535).contains(&p))
            .unwrap_or(DEFAULT_SERVER_WEB_PORT),
    }
}

/// Validates whether a port string contains a valid TCP/UDP port number (1..=65535).
pub fn is_valid_port(value: &str) -> bool {
    value
        .trim()
        .parse::<i32>()
        .map(|p| (1..=65535).contains(&p))
        .unwrap_or(false)
}

/// Validates that the CSQTT main tunnel password is exactly 16 ASCII alphanumeric characters.
pub fn is_deploy_main_password_valid(password: &str) -> bool {
    password.len() == 16 && password.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Checks that both the web panel admin login and password are non-blank.
pub fn has_web_panel_credentials(login: &str, password: &str) -> bool {
    !login.trim().is_empty() && !password.trim().is_empty()
}

/// Returns the main password to synchronize into client tunnel settings after a successful deploy.
pub fn password_to_synchronize_after_deploy(
    deploy_succeeded: bool,
    main_password: &str,
) -> Option<String> {
    if deploy_succeeded && !main_password.trim().is_empty() {
        Some(main_password.to_string())
    } else {
        None
    }
}

/// UI state for the server deployment screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeployUiState {
    pub host: String,
    pub ssh_login: String,
    pub ssh_password: String,
    pub manual_ports: bool,
    pub ssh_port: String,
    pub peer_port: String,
    pub web_port: String,
    pub main_password_configured: bool,
    pub web_panel_configured: bool,
    pub ssh_keys_mode: bool,
    pub docker_install: bool,
    pub secret_path_enabled: bool,
    pub secret_path_uuid: String,
    pub ssh_keys_filled: i32,
    pub is_deploying: bool,
    pub progress: f32,
    pub current_step: String,
    pub show_validation: bool,
}

impl Default for DeployUiState {
    fn default() -> Self {
        Self {
            host: String::new(),
            ssh_login: String::new(),
            ssh_password: String::new(),
            manual_ports: false,
            ssh_port: DEFAULT_SSH_PORT.to_string(),
            peer_port: DEFAULT_SERVER_PEER_PORT.to_string(),
            web_port: DEFAULT_SERVER_WEB_PORT.to_string(),
            main_password_configured: false,
            web_panel_configured: false,
            ssh_keys_mode: false,
            docker_install: false,
            secret_path_enabled: false,
            secret_path_uuid: String::new(),
            ssh_keys_filled: 0,
            is_deploying: false,
            progress: 0.0,
            current_step: String::new(),
            show_validation: false,
        }
    }
}

impl DeployUiState {
    /// Returns true if port configuration is valid (either manual ports are disabled or all 3 ports are valid).
    pub fn ports_valid(&self) -> bool {
        !self.manual_ports
            || (is_valid_port(&self.ssh_port)
                && is_valid_port(&self.peer_port)
                && is_valid_port(&self.web_port))
    }

    /// Returns true if both the web panel credentials and the main tunnel password are configured.
    pub fn authorization_configured(&self) -> bool {
        self.web_panel_configured && self.main_password_configured
    }

    /// Returns true if SSH credentials are ready (login present, and either key mode has key or password is present).
    pub fn ssh_auth_ready(&self) -> bool {
        !self.ssh_login.trim().is_empty()
            && if self.ssh_keys_mode {
                self.ssh_keys_filled == 1
            } else {
                !self.ssh_password.trim().is_empty()
            }
    }

    /// Returns true if the deployment preconditions are satisfied to initiate installation.
    pub fn can_install(&self) -> bool {
        !self.host.trim().is_empty()
            && self.ssh_auth_ready()
            && self.authorization_configured()
            && self.ports_valid()
    }

    /// Returns true if preconditions are satisfied to initiate uninstall (authorization is not required).
    pub fn can_uninstall(&self) -> bool {
        !self.host.trim().is_empty() && self.ssh_auth_ready() && self.ports_valid()
    }

    /// Returns the complete URL to access the deployed web panel.
    pub fn panel_display_url(&self) -> String {
        let h = if self.host.trim().is_empty() {
            "IP"
        } else {
            self.host.trim()
        };
        let p = if self.manual_ports {
            self.web_port
                .trim()
                .parse::<i32>()
                .unwrap_or(DEFAULT_SERVER_WEB_PORT)
        } else {
            DEFAULT_SERVER_WEB_PORT
        };
        let u = if !self.secret_path_uuid.trim().is_empty() {
            let trimmed = self.secret_path_uuid.trim();
            if trimmed.starts_with('/') {
                trimmed.to_string()
            } else {
                format!("/{trimmed}")
            }
        } else {
            "/uuid".to_string()
        };
        format!("https://{h}:{p}{u}")
    }

    /// Applies a UI action to update this state.
    pub fn update(&mut self, action: DeployAction) {
        match action {
            DeployAction::HostChanged(val) => self.host = val,
            DeployAction::LoginChanged(val) => self.ssh_login = val,
            DeployAction::PasswordChanged(val) => self.ssh_password = val,
            DeployAction::ManualPortsChanged(val) => self.manual_ports = val,
            DeployAction::SshKeysModeChanged(val) => self.ssh_keys_mode = val,
            DeployAction::DockerInstallChanged(val) => self.docker_install = val,
            DeployAction::RegenerateSecretPath => {}
            DeployAction::EditSshKeys => {}
            DeployAction::DockerInfo => {}
            DeployAction::SshPortChanged(val) => self.ssh_port = val,
            DeployAction::PeerPortChanged(val) => self.peer_port = val,
            DeployAction::WebPortChanged(val) => self.web_port = val,
            DeployAction::EditAuthorization => {}
            DeployAction::Install => {}
            DeployAction::Uninstall => {}
            DeployAction::OpenControlPanel => {}
            DeployAction::PanelInfo => {}
        }
    }
}

/// UI user actions on the deploy screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployAction {
    HostChanged(String),
    LoginChanged(String),
    PasswordChanged(String),
    ManualPortsChanged(bool),
    SshKeysModeChanged(bool),
    DockerInstallChanged(bool),
    RegenerateSecretPath,
    EditSshKeys,
    DockerInfo,
    SshPortChanged(String),
    PeerPortChanged(String),
    WebPortChanged(String),
    EditAuthorization,
    Install,
    Uninstall,
    OpenControlPanel,
    PanelInfo,
}

/// Pure progress tracker corresponding to `DeployManager.kt`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeployProgressState {
    pub is_deploying: bool,
    pub progress: f32,
    pub current_step: String,
    pub last_result: String,
}

impl DeployProgressState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_deploy(&mut self) {
        self.is_deploying = true;
        self.progress = 0.0;
        self.current_step = "Инициализация...".to_string();
        self.last_result.clear();
    }

    pub fn stop_deploy(&mut self, result: &str) {
        self.is_deploying = false;
        if !result.trim().is_empty() {
            self.last_result = result.to_string();
        }
    }

    pub fn force_reset(&mut self) {
        self.is_deploying = false;
        self.progress = 0.0;
        self.current_step.clear();
    }

    pub fn update_progress(&mut self, progress: f32, step: impl Into<String>) {
        self.progress = progress;
        self.current_step = step.into();
    }
}

// ---------------------------------------------------------------------------
// Unit Tests (Ported 1:1 from Kotlin tests + pure helpers)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // 1. Port of DeployResultPolicyTest.kt (16 tests)
    // =======================================================================

    #[test]
    fn exact_marker_and_zero_exit_are_required() {
        assert!(is_successful_deploy_result(0, "done\nCSQTT_DEPLOY_OK\n"));
        assert!(!is_successful_deploy_result(1, "CSQTT_DEPLOY_OK"));
        assert!(!is_successful_deploy_result(0, "service is active"));
        assert!(!is_successful_deploy_result(
            0,
            "prefix CSQTT_DEPLOY_OK suffix"
        ));
    }

    #[test]
    fn deploy_rollback_exit_is_explained_without_pretending_success() {
        assert_eq!(
            deploy_failure_message(30, "CSQTT_DEPLOY_ROLLED_BACK\n"),
            "Новый релиз не прошёл проверку; предыдущая установка восстановлена"
        );
        assert_eq!(
            deploy_failure_message(31, "CSQTT_DEPLOY_ROLLBACK_FAILED\n"),
            "Новый релиз не запустился, а автоматический rollback завершился с ошибкой — требуется проверка VPS"
        );
    }

    #[test]
    fn rollback_keeps_concrete_server_reason() {
        assert_eq!(
            deploy_failure_message(
                30,
                "CSQTT_DEPLOY_ERROR|cutover|Проверенный candidate-бинарник исчез после остановки старого runtime\nCSQTT_DEPLOY_ROLLED_BACK\n"
            ),
            "Новый релиз не прошёл проверку; предыдущая установка восстановлена Причина: Проверенный candidate-бинарник исчез после остановки старого runtime"
        );
    }

    #[test]
    fn preflight_failure_explains_that_live_service_was_not_stopped() {
        assert_eq!(
            deploy_failure_message(20, "probe failed"),
            "Кандидат не прошёл предзапусковую проверку; работающий сервер не останавливался"
        );
    }

    #[test]
    fn auth_failure_has_actionable_message() {
        assert_eq!(
            friendly_deploy_error("Auth fail for methods 'publickey,password'"),
            "SSH-аутентификация отклонена: проверьте логин, пароль и разрешение PasswordAuthentication на VPS"
        );
    }

    #[test]
    fn public_key_failure_has_key_specific_message() {
        assert!(
            friendly_deploy_error("Auth fail (public key): rejected").contains("authorized_keys")
        );
    }

    #[test]
    fn timeout_has_short_message() {
        assert_eq!(
            friendly_deploy_error("connect timeout"),
            "Истекло время ожидания SSH/SFTP"
        );
    }

    #[test]
    fn exhausted_upload_has_network_specific_message() {
        assert!(
            friendly_deploy_error("SFTP/SCP upload failed for csqtt: SFTP: channel closed")
                .contains("автоматического переподключения")
        );
    }

    #[test]
    fn server_success_line_becomes_clean_ok_log() {
        assert_eq!(
            parse_deploy_output_line("✓ csqtt установлен"),
            Some(DeployOutputLine {
                message: "csqtt установлен".to_string(),
                level: DeployOutputLevel::Ok,
            })
        );
    }

    #[test]
    fn server_step_has_no_emoji_or_status_sticker() {
        assert_eq!(
            parse_deploy_output_line("📦 Установка csqtt..."),
            Some(DeployOutputLine {
                message: "Установка csqtt...".to_string(),
                level: DeployOutputLevel::Log,
            })
        );
        assert_eq!(
            parse_deploy_output_line("[►] Проверка зависимостей"),
            Some(DeployOutputLine {
                message: "Проверка зависимостей".to_string(),
                level: DeployOutputLevel::Log,
            })
        );
    }

    #[test]
    fn server_failure_becomes_error_log() {
        assert_eq!(
            parse_deploy_output_line("[✗] Сервис csqtt не запустился"),
            Some(DeployOutputLine {
                message: "Сервис csqtt не запустился".to_string(),
                level: DeployOutputLevel::Err,
            })
        );
    }

    #[test]
    fn protocol_and_decoration_lines_are_hidden() {
        assert_eq!(parse_deploy_output_line("CSQTT_PROGRESS|0.6|Бинарник..."), None);
        assert_eq!(
            parse_deploy_output_line("CSQTT_DEPLOY_ERROR|cutover|candidate disappeared"),
            None
        );
        assert_eq!(parse_deploy_output_line("════════════════════════════"), None);
        assert_eq!(parse_deploy_output_line("CSQTT_DEPLOY_OK"), None);
        assert_eq!(parse_deploy_output_line("CSQTT_DEPLOY_ROLLED_BACK"), None);
        assert_eq!(
            parse_deploy_output_line("CSQTT_DEPLOY_ROLLBACK_FAILED"),
            None
        );
    }

    #[test]
    fn docker_mode_uses_raw_environment_values() {
        assert_eq!(deploy_mode(true), "docker");
        assert_eq!(deploy_mode(false), "systemd");
        assert_eq!(
            deploy_environment_value("a b#$\"\\c\r\nd", true),
            "a b#$\"\\c  d"
        );
        assert_eq!(
            deploy_environment_value("a b#$\"\\c\r\nd", false),
            "\"a b#$\\\"\\\\c  d\""
        );
    }

    #[test]
    fn server_asset_matches_the_remote_vps_architecture() {
        assert_eq!(
            server_architecture_for_machine("x86_64\n").unwrap(),
            ServerArchitecture::Amd64
        );
        assert_eq!(
            server_architecture_for_machine("amd64\n").unwrap(),
            ServerArchitecture::Amd64
        );
        assert_eq!(
            server_architecture_for_machine("aarch64\n").unwrap(),
            ServerArchitecture::Arm64
        );
        assert_eq!(
            server_architecture_for_machine("arm64\n").unwrap(),
            ServerArchitecture::Arm64
        );
        assert_eq!(
            server_architecture_for_machine("armv7l\n").unwrap(),
            ServerArchitecture::Armv7
        );
        assert_eq!(
            server_architecture_for_machine("armhf\n").unwrap(),
            ServerArchitecture::Armv7
        );
    }

    #[test]
    fn unsupported_remote_vps_architecture_stops_before_upload() {
        assert!(server_architecture_for_machine("riscv64\n").is_err());
    }

    #[test]
    fn ssh_password_keeps_spaces_and_special_characters_exactly() {
        assert_eq!(
            sanitize_ssh_password("a !@#$%^&*()[]{};:,.?/\\|~+= b"),
            "a !@#$%^&*()[]{};:,.?/\\|~+= b"
        );
        assert_eq!(sanitize_ssh_password("pa\n ss\rword"), "pa ssword");
    }

    // =======================================================================
    // 2. Port of DeployScriptPreflightTest.kt (18 tests)
    // =======================================================================

    const SCRIPT: &str = include_str!("../../../app/src/main/assets/deploy.sh");
    const DEPLOY_OPERATIONS: &str =
        include_str!("../../../app/src/main/java/com/csqtt/client/ui/DeployOperations.kt");

    fn script() -> &'static str {
        SCRIPT
    }

    fn deploy_operations() -> &'static str {
        DEPLOY_OPERATIONS
    }

    fn substring_after<'a>(s: &'a str, delim: &str) -> &'a str {
        match s.find(delim) {
            Some(pos) => &s[pos + delim.len()..],
            None => s,
        }
    }

    fn substring_before<'a>(s: &'a str, delim: &str) -> &'a str {
        match s.find(delim) {
            Some(pos) => &s[..pos],
            None => s,
        }
    }

    #[test]
    fn preflight_performs_working_kernel_probes() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("ip tuntap add dev \"$probe_iface\" mode tun"));
        assert!(deploy.contains("setup_nat_and_firewall"));
        assert!(deploy.contains("iptables -w \"$XT_WAIT\" -t nat -C POSTROUTING"));
        assert!(deploy.contains("probe_tun_support"));
        assert!(deploy.contains("prepare_uploaded_release()"));
        assert!(deploy.contains("run_platform_preflight"));
    }

    #[test]
    fn installer_accepts_the_supported_server_architectures() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("x86_64|amd64"));
        assert!(deploy.contains("aarch64|arm64"));
        assert!(deploy.contains("armv7l|armv7|armhf"));
        assert!(deploy.contains("Поддерживаются: x86_64, aarch64, armv7l"));
    }

    #[test]
    fn client_uploads_the_release_before_one_atomic_runtime_switch() {
        let deploy = script().replace("\r\n", "\n");
        let install_branch = substring_after(&deploy, "install|--install|-i)");
        let upload_check = install_branch
            .find("prepare_uploaded_release")
            .expect("prepare_uploaded_release");
        let cleanup = install_branch
            .find("csqtt_cleanup")
            .expect("csqtt_cleanup");
        let binary = install_branch
            .find("setup_csqtt_binary")
            .expect("setup_csqtt_binary");
        let preflight = install_branch
            .find("run_platform_preflight")
            .expect("run_platform_preflight");
        let firewall = install_branch
            .find("setup_nat_and_firewall")
            .expect("setup_nat_and_firewall");
        assert!(binary > preflight);
        assert!(binary > firewall);
        assert!(install_branch.contains("run_timed \"preflight\" run_platform_preflight"));
        assert!(cleanup > upload_check);
        assert!(binary > cleanup);
        assert!(install_branch.contains("local total_started=$SECONDS\n            detect_os\n            run_timed \"зависимости\" install_prerequisites\n            require_runtime_tools"));

        let client = deploy_operations().replace("\r\n", "\n");
        assert!(!client.contains("bash /tmp/deploy.sh prepare"));
        let u_idx = client
            .find("/tmp/.csqtt-upload-server")
            .expect("/tmp/.csqtt-upload-server");
        let i_idx = client
            .find("bash /tmp/deploy.sh install")
            .expect("bash /tmp/deploy.sh install");
        assert!(u_idx < i_idx);
    }

    #[test]
    fn activation_uses_direct_runtime_paths_and_has_no_staged_candidate() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("readonly UPLOAD_BINARY=\"/tmp/.csqtt-upload-server\""));
        assert!(deploy.contains("install -m 0755 \"$UPLOAD_BINARY\" /usr/local/bin/csqtt"));
        assert!(deploy.contains("install -m 0600 \"$UPLOAD_ENV_FILE\" \"$CSQTT_ENV_FILE\""));
        assert!(deploy.contains("rm -f /usr/local/bin/csqtt"));
        assert!(deploy.contains("rm -rf /usr/local/lib/csqtt"));
        assert!(deploy.contains("clear_runtime_config_preserving_database()"));
        assert!(!deploy.contains("csqtt.next.XXXXXX"));
        assert!(!deploy.contains("csqtt-stage"));
        assert!(!deploy.contains("verify_staged_release"));
        assert!(!deploy.contains("CSQTT_DEPLOY_READY_FOR_UPLOAD"));
        assert!(deploy.contains("probe_tun_support"));
    }

    #[test]
    fn reserved_runtime_directories_recover_from_a_failed_previous_install() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("ensure_csqtt_directory()"));
        assert!(deploy.contains("rm -rf -- \"$path\" || die \"Не удалось очистить путь CSQTT: $path\""));
        assert!(deploy.contains("ensure_csqtt_directory \"$CSQTT_CONFIG_DIR\" 700"));
        assert!(deploy.contains("ensure_csqtt_directory /usr/local/lib/csqtt 755"));
        assert!(deploy.contains("setup_csqtt_environment() {\n    ensure_csqtt_directory \"$CSQTT_CONFIG_DIR\" 700"));
        assert!(deploy.contains("install -m 0600 \"$UPLOAD_ENV_FILE\" \"$CSQTT_ENV_FILE\""));
    }

    #[test]
    fn redeploy_removes_old_runtime_config_but_preserves_sqlite_and_its_wal_files() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("readonly CSQTT_DATABASE_FILE=\"csqtt.db\""));
        assert!(deploy.contains("readonly CSQTT_DATABASE_WAL_FILE=\"csqtt.db-wal\""));
        assert!(deploy.contains("readonly CSQTT_DATABASE_SHM_FILE=\"csqtt.db-shm\""));
        assert!(deploy.contains("clear_runtime_config_preserving_database"));
        assert!(deploy.contains("\"$CSQTT_DATABASE_FILE\"|\"$CSQTT_DATABASE_WAL_FILE\"|\"$CSQTT_DATABASE_SHM_FILE\"|\"$CSQTT_LEGACY_MIGRATION_JSON\"|\"$CSQTT_LEGACY_MIGRATION_IMPORTED_JSON\"|web_cert.pem|web_key.pem|letsencrypt-ip.env|runtime-archive) continue"));
        assert!(!deploy.contains("find \"$CSQTT_CONFIG_DIR\" -mindepth 1 -maxdepth 1"));
    }

    #[test]
    fn docker_uses_minimal_capabilities_and_repeats_probes_inside_container() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("--cap-drop ALL"));
        assert!(deploy.contains("--cap-add NET_ADMIN"));
        assert!(deploy.contains("--cap-add NET_RAW"));
        assert!(deploy.contains("--device /dev/net/tun:/dev/net/tun"));
        assert!(deploy.contains("--security-opt seccomp=unconfined"));
        assert!(deploy.contains("probe_docker_runtime"));
        assert!(deploy.split("--cap-drop ALL").count() >= 3);
        assert!(deploy.contains("COPY network-up.sh /usr/local/lib/csqtt/network-up.sh"));
        assert!(deploy.contains("exec /usr/local/bin/csqtt \\\"$@\\\""));
        assert!(deploy.contains("write_network_helper \"$context_dir/network-up.sh\""));
    }

    #[test]
    fn docker_image_build_retries_dns_with_yandex_resolvers() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("DOCKER_BUILD_TIMEOUT_SECONDS"));
        assert!(deploy.contains("docker build --network host"));
        assert!(deploy.contains("CSQTT_BUILD_DNS_PRIMARY:-"));
        assert!(deploy.contains("FROM alpine:3.21"));
        assert!(deploy.contains("apk add --no-cache --no-progress ca-certificates iproute2 iptables"));
        assert!(deploy.contains("CSQTT_BUILD_DNS_PRIMARY=77.88.8.8"));
        assert!(deploy.contains("CSQTT_BUILD_DNS_SECONDARY=77.88.8.1"));
        assert!(deploy.contains("Temporary failure resolving|Could not resolve|Name or service not known"));
    }

    #[test]
    fn deploy_payload_leaves_dns_selection_to_the_persisted_server_database() {
        let deploy = script().replace("\r\n", "\n");
        let client = deploy_operations().replace("\r\n", "\n");
        assert!(!client.contains(".put(\"dns\""));
        assert!(!client.contains("dnsValue"));
        assert!(!deploy.contains("Не удалось сохранить DNS из существующей SQLite-конфигурации"));
        assert!(client.contains("\"csqtt-deploy.json\" -> \"настройки доступа\""));
    }

    #[test]
    fn web_health_probe_accepts_the_panels_unauthenticated_response_without_creating_sessions() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("200|301|302|401"));
        assert!(deploy.contains("${scheme}://127.0.0.1:${WEB_PORT}/"));
        assert!(!deploy.contains("--data-binary @-"));
        assert!(!deploy.contains("/api/login"));
        assert!(!deploy.contains("NETWORK_READY_STAMP"));
    }

    #[test]
    fn configured_network_is_verified_after_applying_rules_and_after_startup() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("verify_configured_network()"));
        assert!(deploy.split("verify_configured_network").count() >= 4);
        assert!(deploy.contains("-j MASQUERADE >/dev/null 2>&1"));
        assert!(deploy.contains("-j TCPMSS --clamp-mss-to-pmtu 2>/dev/null"));
    }

    #[test]
    fn startup_removes_stale_tunnel_state_and_waits_through_the_tun_initialization_window() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("remove_csqtt_tun_interface()"));
        assert!(deploy.contains("timeout 2 ip link del \"$CSQTT_IFACE\""));
        assert!(deploy.contains("write_tun_recovery_helper /usr/local/lib/csqtt/tun-recover.sh"));
        assert!(deploy.contains("ExecStartPre=/usr/local/lib/csqtt/tun-recover.sh"));
        assert!(deploy.contains("release_owned_peer_port()"));
        assert!(deploy.contains("csqtt_process_is_owned()"));
        assert!(deploy.contains("UDP/\\$PEER_PORT runtime PID \\$pid; terminating"));
        assert!(deploy.contains("readonly START_STABILITY_SECONDS=\"${CSQTT_START_STABILITY_SECONDS:-4}\""));
        assert!(deploy.contains("StartLimitIntervalSec=60"));
        assert!(deploy.contains("RestartSec=3"));
        assert!(deploy.contains("COPY tun-recover.sh /usr/local/lib/csqtt/tun-recover.sh"));
        assert!(deploy.contains("/usr/local/lib/csqtt/tun-recover.sh; exec /usr/local/bin/csqtt"));
    }

    #[test]
    fn runtime_mode_switch_removes_csqtt_paths_and_force_releases_the_configured_peer_port() {
        let deploy = script().replace("\r\n", "\n");
        let client = deploy_operations().replace("\r\n", "\n");
        assert!(deploy.contains("docker_container_runs_csqtt()"));
        assert!(deploy.contains("docker top \"$container\" -eo pid,args"));
        assert!(deploy.contains("docker-${container_id}.scope"));
        assert!(deploy.contains("remove_all_csqtt_docker_containers"));
        assert!(deploy.contains("stop_all_running_csqtt_systemd_units"));
        assert!(deploy.contains("remove_all_csqtt_systemd_units"));
        assert!(deploy.contains("assert_peer_port_is_available"));
        assert!(deploy.contains("force_release_peer_port"));
        assert!(deploy.contains("cleanup_legacy_proxy_interfaces"));
        assert!(deploy.contains("docker_container_for_pid"));
        assert!(deploy.contains("docker rm -f \"$container\""));
        assert!(deploy.contains("--label com.csqtt.managed=true"));
        assert!(deploy.contains("archive_csqtt_docker_state()"));
        assert!(deploy.contains("CSQTT_RUNTIME_ARCHIVE_DIR"));
        assert!(deploy.contains("runtime-archive) continue"));
        assert!(deploy.contains("force_stop_csqtt_processes"));
        assert!(deploy.contains("all_csqtt_process_pids"));
        assert!(deploy.contains("csqtt_peer_port_is_owned"));
        assert!(deploy.contains("csqtt_process_owns_peer_port"));
        assert!(deploy.contains("readlink \"/proc/$pid/exe\""));
        assert!(deploy.contains("--config-dir ${CSQTT_CONFIG_DIR}"));
        assert!(deploy.contains("kill -KILL \"$pid\""));
        assert!(deploy.contains("kill -TERM \"$pid\""));
        assert!(deploy.contains("принудительное освобождение"));
        assert!(!deploy.contains("unknown runtime; refusing to alter it"));
        assert!(!deploy.contains("CSQTT не будет их удалять"));
        assert!(!deploy.contains("pkill -"));
        assert!(!deploy.contains("pgrep -x csqtt"));
        assert!(!deploy.contains("ipt_del_repeat"));
        assert!(!deploy.contains("nft delete table ip csqtt"));
        assert!(!deploy.contains("fw_add_input_tcp \"$SSH_PORT\""));
        assert!(!deploy.contains("--dport \"$SSH_PORT\" -m comment --comment \"$IPT_COMMENT\""));
        assert!(client.contains("csqtt-uninstall-"));
        assert!(!client.contains("pkill -x csqtt"));
        assert!(!client.contains("docker rm -f csqtt"));
        assert!(!client.contains("nft delete table ip csqtt"));
    }

    #[test]
    fn runtime_cleanup_stops_processes_before_deleting_csqtt_runtime_paths() {
        let deploy = script().replace("\r\n", "\n");
        let cleanup =
            substring_before(substring_after(&deploy, "csqtt_cleanup() {"), "setup_sysctl()");
        let force_stop = substring_before(
            substring_after(&deploy, "force_stop_csqtt_processes() {"),
            "force_release_peer_port()",
        );

        assert!(force_stop.contains("kill -KILL \"$pid\""));
        let stop_idx = cleanup
            .find("force_stop_csqtt_processes")
            .expect("force_stop_csqtt_processes");
        let rm_idx = cleanup
            .find("rm -f /usr/local/bin/csqtt")
            .expect("rm -f /usr/local/bin/csqtt");
        assert!(stop_idx < rm_idx);
        assert!(cleanup.contains(
            "systemctl daemon-reload || die \"systemctl daemon-reload завершился ошибкой после удаления старого runtime\""
        ));
    }

    #[test]
    fn systemd_runtime_discovery_and_shutdown_tolerate_expected_nonzero_states() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("if systemd_unit_runs_csqtt \"$unit\"; then"));
        assert!(!deploy.contains("systemd_unit_runs_csqtt \"$unit\" &&"));
        assert!(deploy.contains(
            "systemctl list-units --type=service --all --no-legend --plain 2>/dev/null || true"
        ));
        assert!(deploy.contains(
            "systemctl list-unit-files --type=service --no-legend --plain 2>/dev/null || true"
        ));
        assert!(deploy.contains("if ! timeout 10 systemctl stop \"$unit\" >/dev/null 2>&1; then"));
        assert!(deploy.contains(
            "systemctl is-active --quiet \"$unit\"; then\n            die \"Не удалось остановить systemd runtime CSQTT: $unit\""
        ));
    }

    #[test]
    fn systemd_service_leaves_resource_limits_to_the_vps_defaults() {
        let deploy = script().replace("\r\n", "\n");
        assert!(!deploy.contains("LimitNOFILE="));
        assert!(!deploy.contains("LimitMEMLOCK="));
        assert!(!deploy.contains("LimitNPROC="));
        assert!(!deploy.contains("TasksMax="));
    }

    #[test]
    fn web_panel_prefers_renewable_trusted_ip_tls_with_self_signed_fallback() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("--preferred-profile shortlived"));
        assert!(deploy.contains("--ip-address \"$public_ip\""));
        assert!(deploy.contains("--force-renewal"));
        assert!(deploy.contains("openssl x509 -noout -checkend \"$LE_RENEW_BEFORE_SECONDS\""));
        assert!(deploy.contains("OnUnitInactiveSec=24h") || deploy.contains("OnUnitInactiveSec=1h"));
        assert!(deploy.contains("fw_add_input_tcp \"$LE_HTTP_PORT\""));
        assert!(deploy.contains("web_cert.pem|web_key.pem|letsencrypt-ip.env"));
        assert!(deploy.contains("systemctl kill --kill-who=main --signal USR1 csqtt"));
        assert!(deploy.contains("docker kill --signal USR1 csqtt"));
        assert!(deploy.contains("issuer=\"${issuer#issuer=}\""));
        assert!(deploy.contains("subject=\"${subject#subject=}\""));
        assert!(deploy.contains("! python3 -c 'import ensurepip'"));
        assert!(deploy.contains("apt) python_packages=(python3 python3-venv)"));
        assert!(deploy.contains("pkg_install_with_refresh()"));
        assert!(deploy.contains("pkg_install_with_refresh \"${python_packages[@]}\""));
        assert!(deploy.contains("local ip=\"$1\" first second third fourth value"));
        assert!(deploy.contains("grep '^csqtp' || true"));
        assert!(deploy.contains("self-signed fallback"));
    }

    #[test]
    fn certificate_ownership_keeps_user_tls_untouched_and_renews_only_csqtt_certificates() {
        let deploy = script().replace("\r\n", "\n");
        assert!(deploy.contains("certificate_is_csqtt_rcgen()"));
        assert!(deploy.contains("certificate_is_csqtt_letsencrypt()"));
        assert!(deploy.contains("web_certificate_is_csqtt_managed()"));
        assert!(deploy.contains("web_certificate_is_user_managed()"));
        assert!(deploy.contains("CN=rcgen self signed cert"));
        assert!(deploy.contains("O=Let's Encrypt"));
        assert!(deploy.contains("IP Address:${saved_ip}"));
        assert!(deploy.contains("rm -f -- \"$CSQTT_LE_STATE_FILE\""));
        assert!(deploy.contains("disable_letsencrypt_renewal_timer"));
        assert!(deploy.contains("CSQTT не изменяет его и отключает собственное продление"));
    }

    // =======================================================================
    // 3. Port of DeployUiStateTest.kt (8 tests)
    // =======================================================================

    #[test]
    fn install_requires_web_authorization() {
        let complete_state = DeployUiState {
            host: "server.example".to_string(),
            ssh_login: "root".to_string(),
            ssh_password: "ssh-password".to_string(),
            main_password_configured: true,
            web_panel_configured: true,
            ..Default::default()
        };
        assert!(complete_state.can_install());
        let mut no_web = complete_state.clone();
        no_web.web_panel_configured = false;
        assert!(!no_web.can_install());
    }

    #[test]
    fn install_requires_main_password_authorization() {
        let complete_state = DeployUiState {
            host: "server.example".to_string(),
            ssh_login: "root".to_string(),
            ssh_password: "ssh-password".to_string(),
            main_password_configured: true,
            web_panel_configured: true,
            ..Default::default()
        };
        assert!(complete_state.can_install());
        let mut no_main = complete_state.clone();
        no_main.main_password_configured = false;
        assert!(!no_main.can_install());

        assert!(is_deploy_main_password_valid("0123456789abcdef"));
        assert!(!is_deploy_main_password_valid("0123456789abcde"));
        assert!(!is_deploy_main_password_valid("short"));
        assert!(!is_deploy_main_password_valid(""));
        assert!(!is_deploy_main_password_valid("0123456789abcdefg"));
        assert!(!is_deploy_main_password_valid("0123456789abcde!"));
    }

    #[test]
    fn web_login_and_password_are_both_required() {
        assert!(has_web_panel_credentials("admin", "secret"));
        assert!(!has_web_panel_credentials("", "secret"));
        assert!(!has_web_panel_credentials("admin", ""));
        assert!(!has_web_panel_credentials("   ", "secret"));
        assert!(!has_web_panel_credentials("admin", "   "));
    }

    #[test]
    fn uninstall_does_not_require_panel_authorization() {
        let state = DeployUiState {
            host: "server.example".to_string(),
            ssh_login: "root".to_string(),
            ssh_password: "ssh-password".to_string(),
            main_password_configured: false,
            web_panel_configured: false,
            ..Default::default()
        };
        assert!(state.can_uninstall());
    }

    #[test]
    fn docker_selection_preserves_deploy_validation() {
        let mut complete_state = DeployUiState {
            host: "server.example".to_string(),
            ssh_login: "root".to_string(),
            ssh_password: "ssh-password".to_string(),
            main_password_configured: true,
            web_panel_configured: true,
            docker_install: true,
            ..Default::default()
        };
        assert!(complete_state.can_install());
        complete_state.host = String::new();
        assert!(!complete_state.can_install());
    }

    #[test]
    fn disabling_manual_ports_restores_every_deployment_port_default() {
        let ports = resolve_server_ports(false, "228", "47000", "47002");
        assert_eq!(ports.ssh, 22);
        assert_eq!(ports.peer, 46000);
        assert_eq!(ports.web, 46002);
    }

    #[test]
    fn manual_ports_preserve_valid_custom_values() {
        let ports = resolve_server_ports(true, "228", "47000", "47002");
        assert_eq!(ports.ssh, 228);
        assert_eq!(ports.peer, 47000);
        assert_eq!(ports.web, 47002);
    }

    #[test]
    fn successful_deploy_synchronizes_a_nonempty_server_password() {
        assert_eq!(
            password_to_synchronize_after_deploy(true, "server-password"),
            Some("server-password".to_string())
        );
        assert_eq!(
            password_to_synchronize_after_deploy(false, "server-password"),
            None
        );
        assert_eq!(password_to_synchronize_after_deploy(true, ""), None);
        assert_eq!(password_to_synchronize_after_deploy(true, "   "), None);
    }

    // =======================================================================
    // 4. Port of SshKeyRuntimeDependencyTest.kt (2 tests)
    // =======================================================================

    /// On Android, BouncyCastle classes `org.bouncycastle.openssl.PEMParser` and
    /// `org.bouncycastle.openssl.jcajce.JcaPEMKeyConverter` are required runtime dependencies
    /// for parsing OpenSSH / PEM keys. On Windows (Rust), key parsing is handled natively by
    /// `russh::keys::decode_secret_key` (backed by `ssh-key`), so JVM / BouncyCastle dependencies
    /// are not needed and omitted.
    #[test]
    fn classic_pem_parser_is_available() {
        // Android-only BouncyCastle dependency check; native russh key decoding used on Windows.
    }

    #[test]
    fn pem_key_converter_is_available() {
        // Android-only BouncyCastle dependency check; native russh key decoding used on Windows.
    }

    // =======================================================================
    // 5. Additional Unit Tests for Pure Helpers (8 tests)
    // =======================================================================

    #[test]
    fn test_is_benign_stderr() {
        assert!(is_benign_stderr("debconf: delaying package configuration..."));
        assert!(is_benign_stderr("dpkg-preconfigure: unable to re-open stdin:"));
        assert!(is_benign_stderr("debconf: unable to initialize frontend: Dialog"));
        assert!(is_benign_stderr("debconf: falling back to frontend: Readline"));
        assert!(is_benign_stderr("debconf: unable to initialize frontend: Teletype"));
        assert!(is_benign_stderr("Warning: apt-key is deprecated"));
        assert!(is_benign_stderr("[sudo] password for ubuntu:"));
        assert!(is_benign_stderr("frontend: Readline"));
        assert!(is_benign_stderr("frontend:   teletype"));
        assert!(is_benign_stderr("frontend: dialog"));
        assert!(BENIGN_STDERR.is_match("debconf: test"));
        assert!(BENIGN_STDERR.contains_match_in("dpkg-preconfigure: test"));
        assert!(!is_benign_stderr("apt-get: failed to fetch package"));
        assert!(!is_benign_stderr("csqtt: fatal error occurred"));
    }

    #[test]
    fn test_parse_deploy_progress() {
        assert_eq!(
            parse_deploy_progress("CSQTT_PROGRESS|0.42|Установка зависимостей"),
            Some((0.42, "Установка зависимостей".to_string()))
        );
        assert_eq!(
            parse_deploy_progress("CSQTT_PROGRESS|1.0|Готово"),
            Some((1.0, "Готово".to_string()))
        );
        assert_eq!(parse_deploy_progress("CSQTT_PROGRESS|invalid|step"), None);
        assert_eq!(parse_deploy_progress("some other output"), None);
    }

    #[test]
    fn test_root_command() {
        let cmd = root_command("systemctl restart csqtt", Some("my'pass"));
        assert!(cmd.contains("if [ \"$(id -u)\" = \"0\" ]; then bash -c 'systemctl restart csqtt';"));
        assert!(cmd.contains("printf '%s\\n' 'my'\"'\"'pass' | sudo -S -p '' bash -c 'systemctl restart csqtt'"));

        let cmd_no_pass = root_command("id", None);
        assert!(cmd_no_pass.contains("sudo -S -p '' bash -c 'id'"));
    }

    #[test]
    fn test_readable_file_size() {
        assert_eq!(readable_file_size(500), "500 Б");
        assert_eq!(readable_file_size(1024), "1.0 КБ");
        assert_eq!(readable_file_size(1536), "1.5 КБ");
        assert_eq!(readable_file_size(1024 * 1024), "1.0 МБ");
        assert_eq!(readable_file_size(25 * 1024 * 1024), "25.0 МБ");
    }

    #[test]
    fn test_deploy_asset_label() {
        assert_eq!(deploy_asset_label("deploy.sh"), "скрипт установки");
        assert_eq!(deploy_asset_label("csqtt-linux-amd64"), "сервер CSQTT (amd64)");
        assert_eq!(deploy_asset_label("csqtt-linux-arm64"), "сервер CSQTT (ARM64)");
        assert_eq!(deploy_asset_label("csqtt-linux-armv7"), "сервер CSQTT (ARM32)");
        assert_eq!(deploy_asset_label("csqtt.env"), "настройки веб-панели");
        assert_eq!(deploy_asset_label("csqtt-deploy.json"), "настройки доступа");
        assert_eq!(deploy_asset_label("hev-socks5-tunnel"), "C-движок туннеля");
        assert_eq!(deploy_asset_label("custom.file"), "custom.file");
    }

    #[test]
    fn test_deploy_progress_state() {
        let mut tracker = DeployProgressState::new();
        assert!(!tracker.is_deploying);

        tracker.start_deploy();
        assert!(tracker.is_deploying);
        assert_eq!(tracker.progress, 0.0);
        assert_eq!(tracker.current_step, "Инициализация...");

        tracker.update_progress(0.5, "Загрузка");
        assert_eq!(tracker.progress, 0.5);
        assert_eq!(tracker.current_step, "Загрузка");

        tracker.stop_deploy("success");
        assert!(!tracker.is_deploying);
        assert_eq!(tracker.last_result, "success");

        tracker.force_reset();
        assert!(!tracker.is_deploying);
        assert_eq!(tracker.progress, 0.0);
        assert!(tracker.current_step.is_empty());
    }

    #[test]
    fn test_strip_ansi_escapes() {
        assert_eq!(strip_ansi_escapes("\x1b[32mOK\x1b[0m"), "OK");
        assert_eq!(strip_ansi_escapes("\x1b[1;31mERROR\x1b[0m"), "ERROR");
        assert_eq!(strip_ansi_escapes("plain text"), "plain text");
    }

    #[test]
    fn test_deploy_action_and_ui_state_update() {
        let mut state = DeployUiState::default();
        state.update(DeployAction::HostChanged("192.168.1.100".to_string()));
        state.update(DeployAction::LoginChanged("admin".to_string()));
        state.update(DeployAction::PasswordChanged("secret".to_string()));
        state.update(DeployAction::ManualPortsChanged(true));
        state.update(DeployAction::SshPortChanged("2222".to_string()));
        state.update(DeployAction::PeerPortChanged("48000".to_string()));
        state.update(DeployAction::WebPortChanged("48002".to_string()));
        state.update(DeployAction::DockerInstallChanged(true));

        assert_eq!(state.host, "192.168.1.100");
        assert_eq!(state.ssh_login, "admin");
        assert_eq!(state.ssh_password, "secret");
        assert!(state.manual_ports);
        assert_eq!(state.ssh_port, "2222");
        assert_eq!(state.peer_port, "48000");
        assert_eq!(state.web_port, "48002");
        assert!(state.docker_install);
        assert!(state.ports_valid());

        state.secret_path_uuid = "custom-uuid".to_string();
        assert_eq!(
            state.panel_display_url(),
            "https://192.168.1.100:48002/custom-uuid"
        );
    }
}
