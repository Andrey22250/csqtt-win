//! Deploy and uninstall operations over SSH (port of `DeployOperations.kt` lines 200–751).
//!
//! Executes remote installation or uninstallation of the CSQTT server software
//! on a target Linux VPS over SSH, handling architecture detection, asset uploading,
//! command streaming, error classification, and automatic reconnects.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ops_policy::{self, DeployOutputLevel, DeployOutputLine, ServerArchitecture};
use crate::ssh::{ExecResult, HostKeyCheck, SshAuth, SshSession};

// ---------------------------------------------------------------------------
// Request, Action, and Outcome Types
// ---------------------------------------------------------------------------

/// Parameters for remote CSQTT installation or removal on a target VPS.
///
/// Mirrors the parameters accepted by Kotlin `performDeploy` and `performUninstall`,
/// mapping SSH credentials to [`SshAuth`] and keeping options in `snake_case`.
#[derive(Debug, Clone)]
pub struct DeployRequest {
    /// Target VPS hostname or IPv4/IPv6 address.
    pub host: String,
    /// SSH port on the target VPS (default 22).
    pub port: u16,
    /// SSH login user (e.g. `"root"`).
    pub user: String,
    /// SSH authentication method (password or OpenSSH/PEM private key).
    pub auth: SshAuth,
    /// Optional sudo password for privilege escalation if connecting as a non-root user.
    pub sudo_password: Option<String>,
    /// Tunnel main password (16 alphanumeric ASCII characters).
    pub main_pass: String,
    /// Administrative login username for the web management panel.
    pub web_login: String,
    /// Administrative login password for the web management panel.
    pub web_pass: String,
    /// CSQTT tunnel peer UDP port (default 46000).
    pub peer_port: u16,
    /// CSQTT web management panel HTTPS port (default 46002).
    pub web_port: u16,
    /// Optional TLS certificate text.
    pub certificate: String,
    /// Whether to deploy CSQTT as a Docker container instead of a systemd service.
    pub install_in_docker: bool,
    /// Optional secret URL prefix/UUID for the web management panel.
    pub secret_path: String,
}

impl DeployRequest {
    /// Returns the main tunnel password.
    pub fn main_password(&self) -> &str {
        &self.main_pass
    }

    /// Returns whether Docker deployment mode is enabled.
    pub fn docker_install(&self) -> bool {
        self.install_in_docker
    }
}

impl Default for DeployRequest {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: ops_policy::DEFAULT_SSH_PORT as u16,
            user: "root".to_string(),
            auth: SshAuth::Password(String::new()),
            sudo_password: None,
            main_pass: String::new(),
            web_login: String::new(),
            web_pass: String::new(),
            peer_port: ops_policy::DEFAULT_SERVER_PEER_PORT as u16,
            web_port: ops_policy::DEFAULT_SERVER_WEB_PORT as u16,
            certificate: String::new(),
            install_in_docker: false,
            secret_path: String::new(),
        }
    }
}

impl PartialEq for DeployRequest {
    fn eq(&self, other: &Self) -> bool {
        let auth_eq = match (&self.auth, &other.auth) {
            (SshAuth::Password(p1), SshAuth::Password(p2)) => p1 == p2,
            (
                SshAuth::Key {
                    private_key: k1,
                    passphrase: pass1,
                },
                SshAuth::Key {
                    private_key: k2,
                    passphrase: pass2,
                },
            ) => k1 == k2 && pass1 == pass2,
            _ => false,
        };
        auth_eq
            && self.host == other.host
            && self.port == other.port
            && self.user == other.user
            && self.sudo_password == other.sudo_password
            && self.main_pass == other.main_pass
            && self.web_login == other.web_login
            && self.web_pass == other.web_pass
            && self.peer_port == other.peer_port
            && self.web_port == other.web_port
            && self.certificate == other.certificate
            && self.install_in_docker == other.install_in_docker
            && self.secret_path == other.secret_path
    }
}

/// Operation to perform on the remote server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployAction {
    /// Full server deployment (`performDeploy`).
    Deploy,
    /// Server removal (`performUninstall`).
    Uninstall,
}

/// Outcome of a successful remote operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployOutcome {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Final status message (e.g. `"Установка завершена · служба CSQTT активна"`).
    pub message: String,
    /// Detected target VPS architecture (populated on `DeployAction::Deploy`).
    pub architecture: Option<ServerArchitecture>,
    /// Main password to synchronize into client connection settings after deploy.
    pub password_to_synchronize: Option<String>,
    /// Complete HTTPS URL of the deployed web management panel.
    pub panel_url: Option<String>,
    /// Full raw stdout and stderr output from the remote installer command.
    pub raw_output: String,
}

impl DeployOutcome {
    /// Convenience accessor for the password to synchronize.
    pub fn password_to_sync(&self) -> Option<&str> {
        self.password_to_synchronize.as_deref()
    }

    /// Returns true if the operation succeeded.
    pub fn is_success(&self) -> bool {
        self.success
    }
}

impl Default for DeployOutcome {
    fn default() -> Self {
        Self {
            success: true,
            message: String::new(),
            architecture: None,
            password_to_synchronize: None,
            panel_url: None,
            raw_output: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Pure Helpers (Command building, asset resolution, payload generation)
// ---------------------------------------------------------------------------

/// Reads an asset file from `assets_dir`, returning a clear Russian error if missing or empty.
pub fn resolve_asset_file(assets_dir: &Path, file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = assets_dir.join(file_name);
    match std::fs::read(&path) {
        Ok(data) if !data.is_empty() => Ok(data),
        _ => anyhow::bail!("Файл {file_name} отсутствует или пуст в assets"),
    }
}

/// Computes the complete web panel URL for the deployed server.
pub fn panel_url(req: &DeployRequest) -> String {
    let h = if req.host.trim().is_empty() {
        "IP"
    } else {
        req.host.trim()
    };
    let p = req.web_port;
    let u = if !req.secret_path.trim().is_empty() {
        let trimmed = req.secret_path.trim();
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

/// Builds the environment variable prefix for the deployment script execution.
///
/// Matches Kotlin:
/// `env [CSQTT_WEB_SECRET_PATH=<val> ]CSQTT_PEER_PORT=<peer_port> CSQTT_SSH_PORT=<port> CSQTT_WEB_PORT=<web_port> CSQTT_PUBLIC_IP=<host> CSQTT_DEPLOY_MODE=<mode>`
pub fn build_deploy_environment(req: &DeployRequest) -> String {
    let secret_env = if !req.secret_path.trim().is_empty() {
        format!(
            "CSQTT_WEB_SECRET_PATH={} ",
            ops_policy::deploy_environment_value(req.secret_path.trim(), false)
        )
    } else {
        String::new()
    };
    format!(
        "env {secret_env}CSQTT_PEER_PORT={} CSQTT_SSH_PORT={} CSQTT_WEB_PORT={} CSQTT_PUBLIC_IP={} CSQTT_DEPLOY_MODE={}",
        req.peer_port,
        req.port,
        req.web_port,
        req.host.trim(),
        ops_policy::deploy_mode(req.install_in_docker)
    )
}

/// Builds the complete remote root command to execute the deploy installation.
pub fn build_deploy_command(req: &DeployRequest) -> String {
    let env_prefix = build_deploy_environment(req);
    let cmd = format!("{env_prefix} bash /tmp/deploy.sh install");
    ops_policy::root_command(&cmd, req.sudo_password.as_deref())
}

/// Builds the complete remote root command to execute the uninstall.
pub fn build_uninstall_command(req: &DeployRequest) -> String {
    let cmd = format!(
        "env CSQTT_PEER_PORT={} CSQTT_SSH_PORT={} bash /tmp/deploy.sh uninstall",
        req.peer_port, req.port
    );
    ops_policy::root_command(&cmd, req.sudo_password.as_deref())
}

/// Generates the contents of `csqtt.env` to be uploaded to `/tmp/.csqtt-upload-web.env`.
pub fn generate_env_content(req: &DeployRequest) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "CSQTT_WEB_USER={}\n",
        ops_policy::deploy_environment_value(&req.web_login, req.install_in_docker)
    ));
    out.push_str(&format!(
        "CSQTT_WEB_PASS={}\n",
        ops_policy::deploy_environment_value(&req.web_pass, req.install_in_docker)
    ));
    out.push_str(&format!(
        "CSQTT_PUBLIC_IP={}\n",
        ops_policy::deploy_environment_value(req.host.trim(), req.install_in_docker)
    ));
    if !req.secret_path.trim().is_empty() {
        out.push_str(&format!(
            "CSQTT_WEB_SECRET_PATH={}\n",
            ops_policy::deploy_environment_value(req.secret_path.trim(), req.install_in_docker)
        ));
    }
    out
}

/// Generates the contents of `csqtt-deploy.json` to be uploaded to `/tmp/.csqtt-upload-overrides.json`.
pub fn generate_overrides_content(req: &DeployRequest) -> String {
    serde_json::json!({
        "main_password": req.main_pass
    })
    .to_string()
}

/// Standard Base64 encoder used for the chunked stream upload fallback path.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// SSH Runner with Reconnection and Upload Fallback
// ---------------------------------------------------------------------------

/// Active runner managing the SSH session, automatic reconnection, and upload fallbacks.
struct DeployRunner<'a> {
    session: Option<SshSession>,
    host: String,
    port: u16,
    user: String,
    auth: SshAuth,
    sudo_pass: Option<String>,
    check: Arc<dyn Fn(&str, u16, &str, &str) -> bool + Send + Sync>,
    on_output: &'a mut (dyn FnMut(DeployOutputLine) + Send),
}

impl<'a> DeployRunner<'a> {
    /// Connects a new SSH session using the configured host, port, user, auth, and host key check.
    async fn connect_session(
        host: &str,
        port: u16,
        user: &str,
        auth: &SshAuth,
        check: &Arc<dyn Fn(&str, u16, &str, &str) -> bool + Send + Sync>,
    ) -> anyhow::Result<SshSession> {
        let c = Arc::clone(check);
        let check_box: HostKeyCheck = Box::new(move |h, p, kt, fp| (c)(h, p, kt, fp));
        SshSession::connect(host, port, user, auth.clone(), check_box).await
    }

    /// Establishes the initial connection or reconnects if disconnected.
    async fn connect(&mut self) -> anyhow::Result<()> {
        let session =
            Self::connect_session(&self.host, self.port, &self.user, &self.auth, &self.check)
                .await?;
        self.session = Some(session);
        Ok(())
    }

    /// Reconnects once per stage on disconnect, mirroring Kotlin `reconnectFor(stage)`.
    async fn reconnect_for(&mut self, stage: &str) -> bool {
        (self.on_output)(DeployOutputLine::new(
            format!("SSH-соединение восстановление: {stage}"),
            DeployOutputLevel::Log,
        ));
        if let Some(old) = self.session.take() {
            let _ = old.close().await;
        }
        match Self::connect_session(&self.host, self.port, &self.user, &self.auth, &self.check).await
        {
            Ok(new_session) => {
                self.session = Some(new_session);
                (self.on_output)(DeployOutputLine::new(
                    "SSH-соединение восстановлено",
                    DeployOutputLevel::Ok,
                ));
                true
            }
            Err(e) => {
                let friendly = ops_policy::friendly_deploy_error(&e.to_string());
                (self.on_output)(DeployOutputLine::new(
                    format!("Ошибка повторного подключения: {friendly}"),
                    DeployOutputLevel::Err,
                ));
                false
            }
        }
    }

    /// Executes a remote command with a per-stage reconnect attempt if disconnected.
    ///
    /// Every raw stdout/stderr line is passed through `is_benign_stderr`, `parse_deploy_progress`,
    /// and `parse_deploy_output_line` like Kotlin.
    async fn exec_with_retry(
        &mut self,
        command: &str,
        stage: &str,
        timeout_duration: Duration,
    ) -> anyhow::Result<ExecResult> {
        if self.session.is_none() && !self.reconnect_for(stage).await {
            anyhow::bail!("SSH client disconnected before stage: {stage}");
        }

        // Attempt 1
        let res = {
            let DeployRunner {
                session,
                on_output,
                ..
            } = self;
            let mut line_handler = |raw_line: &str| {
                if ops_policy::is_benign_stderr(raw_line) {
                    return;
                }
                if let Some((_fraction, step)) = ops_policy::parse_deploy_progress(raw_line) {
                    on_output(DeployOutputLine::new(
                        format!("Этап: {step}"),
                        DeployOutputLevel::Log,
                    ));
                    return;
                }
                if let Some(parsed) = ops_policy::parse_deploy_output_line(raw_line) {
                    on_output(parsed);
                }
            };

            let sess = session
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("SSH session unavailable"))?;
            tokio::time::timeout(timeout_duration, sess.exec(command, &mut line_handler)).await
        };

        match res {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) | Err(_) => {
                // If attempt 1 failed (or timed out / disconnected), attempt reconnect once like Kotlin ensureConnected
                if self.reconnect_for(stage).await {
                    let DeployRunner {
                        session,
                        on_output,
                        ..
                    } = self;
                    let mut line_handler = |raw_line: &str| {
                        if ops_policy::is_benign_stderr(raw_line) {
                            return;
                        }
                        if let Some((_fraction, step)) = ops_policy::parse_deploy_progress(raw_line)
                        {
                            on_output(DeployOutputLine::new(
                                format!("Этап: {step}"),
                                DeployOutputLevel::Log,
                            ));
                            return;
                        }
                        if let Some(parsed) = ops_policy::parse_deploy_output_line(raw_line) {
                            on_output(parsed);
                        }
                    };

                    let sess = session
                        .as_mut()
                        .ok_or_else(|| anyhow::anyhow!("SSH session unavailable after reconnect"))?;
                    match tokio::time::timeout(timeout_duration, sess.exec(command, &mut line_handler)).await {
                        Ok(Ok(r)) => Ok(r),
                        Ok(Err(e)) => Err(e),
                        Err(_) => anyhow::bail!("Истекло время ожидания SSH/SFTP"),
                    }
                } else {
                    anyhow::bail!("SSH connection lost during {stage}");
                }
            }
        }
    }

    /// Uploads a file to `remote_path` with permissions `mode`.
    ///
    /// NOTE: Russh does not provide SFTP or SCP in `ssh.rs`. Instead, [`SshSession::upload`]
    /// executes `cat > path && chmod <mode> path` over an SSH session channel with stdin streaming.
    /// This mirrors Kotlin's chunked stream upload fallback directly.
    ///
    /// If primary channel upload fails, this function falls back to chunked base64 stream
    /// upload via remote exec with `root_command`, and reconnects once per stage if disconnected
    /// (matching Kotlin's `UPLOAD_RECONNECT_ATTEMPTS = 1`).
    async fn upload_with_retry(
        &mut self,
        data: &[u8],
        remote_path: &str,
        mode: u32,
        file_name: &str,
    ) -> anyhow::Result<()> {
        let label = ops_policy::deploy_asset_label(file_name);
        let _stage = format!("загрузка {label}");

        (self.on_output)(DeployOutputLine::new(
            format!(
                "Загрузка: {label} ({})",
                ops_policy::readable_file_size(data.len() as u64)
            ),
            DeployOutputLevel::Log,
        ));

        let mut upload_err = None;
        let mut succeeded = false;

        // Primary upload attempt: SshSession::upload (exec cat > ... && chmod)
        if let Some(ref mut session) = self.session {
            match session.upload(data, remote_path, mode).await {
                Ok(()) => {
                    succeeded = true;
                }
                Err(e) => {
                    upload_err = Some(e.to_string());
                }
            }
        }

        // Secondary upload attempt: Kotlin chunked stream upload fallback via remote exec + cat/base64
        if !succeeded && self.session.is_some() {
            if let Some(ref mut session) = self.session {
                match upload_fallback_stream(
                    session,
                    data,
                    remote_path,
                    mode,
                    self.sudo_pass.as_deref(),
                )
                .await
                {
                    Ok(()) => {
                        succeeded = true;
                    }
                    Err(e) => {
                        upload_err = Some(format!("Fallback exec+cat failed: {e}"));
                    }
                }
            }
        }

        // If disconnected or failed, reconnect once like Kotlin uploadInternal(..., reconnectsLeft - 1)
        if !succeeded {
            let retry_stage = format!("повторная загрузка {label}");
            if self.reconnect_for(&retry_stage).await {
                if let Some(ref mut session) = self.session {
                    // Retry primary upload
                    match session.upload(data, remote_path, mode).await {
                        Ok(()) => {
                            succeeded = true;
                        }
                        Err(_) => {
                            // Retry fallback stream
                            if upload_fallback_stream(
                                session,
                                data,
                                remote_path,
                                mode,
                                self.sudo_pass.as_deref(),
                            )
                            .await
                            .is_ok()
                            {
                                succeeded = true;
                            }
                        }
                    }
                }
            }
        }

        if succeeded {
            (self.on_output)(DeployOutputLine::new(
                format!("Загружено: {label}"),
                DeployOutputLevel::Ok,
            ));
            Ok(())
        } else {
            let err_msg = upload_err.unwrap_or_else(|| "Неизвестная ошибка загрузки".to_string());
            (self.on_output)(DeployOutputLine::new(
                format!("Не удалось загрузить {label}: {err_msg}"),
                DeployOutputLevel::Err,
            ));
            anyhow::bail!("SFTP/SCP upload failed for {file_name}: {err_msg}");
        }
    }
}

/// Fallback chunked stream upload ported from Kotlin `DeploySSHClient.uploadInternal` lines 374–406.
///
/// Encodes bytes into 64 KiB base64 chunks and appends them to `/tmp/<remote>.tmp` using
/// `printf '%s' '<base64>' | base64 -d >> '<temp_remote>'` with root permissions,
/// then atomically moves and chmods the file.
async fn upload_fallback_stream(
    session: &mut SshSession,
    data: &[u8],
    remote_path: &str,
    mode: u32,
    sudo_pass: Option<&str>,
) -> anyhow::Result<()> {
    let temp_remote = format!("{remote_path}.tmp");
    let rm_cmd = ops_policy::root_command(&format!("rm -f '{temp_remote}'"), sudo_pass);
    let _ = session.exec(&rm_cmd, &mut |_| {}).await;

    const CHUNK_SIZE: usize = 64 * 1024;
    for chunk in data.chunks(CHUNK_SIZE) {
        let b64 = base64_encode(chunk);
        let append_cmd = ops_policy::root_command(
            &format!("printf '%s' '{b64}' | base64 -d >> '{temp_remote}'"),
            sudo_pass,
        );
        let res = session.exec(&append_cmd, &mut |_| {}).await?;
        if res.exit_code != Some(0) {
            anyhow::bail!("Chunk upload failed with exit status {:?}", res.exit_code);
        }
    }

    let mv_cmd = ops_policy::root_command(
        &format!(
            "mv -f '{temp_remote}' '{remote_path}' && chmod {:o} '{remote_path}'",
            mode
        ),
        sudo_pass,
    );
    let mv_res = session.exec(&mv_cmd, &mut |_| {}).await?;
    if mv_res.exit_code != Some(0) {
        anyhow::bail!(
            "Move remote file failed with exit status {:?}",
            mv_res.exit_code
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Main Operation Entry Point
// ---------------------------------------------------------------------------

/// Runs the specified deploy or uninstall action against the remote VPS.
///
/// Server binaries and script assets are loaded from `assets_dir` (`deploy.sh` and
/// `csqtt-linux-{arch}`). Missing or empty assets produce a clear Russian error naming the file.
/// All output lines are classified via [`ops_policy::parse_deploy_output_line`], benign
/// stderr lines are suppressed, and timeouts adhere to the constants in [`ops_policy`].
pub async fn run(
    action: DeployAction,
    req: &DeployRequest,
    assets_dir: &Path,
    check: HostKeyCheck,
    on_output: &mut (dyn FnMut(DeployOutputLine) + Send),
) -> anyhow::Result<DeployOutcome> {
    if req.host.trim().is_empty() {
        anyhow::bail!("SSH host is empty");
    }
    if req.user.trim().is_empty() {
        anyhow::bail!("SSH login is empty");
    }
    if req.port == 0 {
        anyhow::bail!("Invalid SSH port: 0");
    }

    let check_arc: Arc<dyn Fn(&str, u16, &str, &str) -> bool + Send + Sync> = Arc::from(check);

    let mut runner = DeployRunner {
        session: None,
        host: req.host.clone(),
        port: req.port,
        user: req.user.clone(),
        auth: req.auth.clone(),
        sudo_pass: req.sudo_password.clone(),
        check: check_arc,
        on_output,
    };

    let result = match action {
        DeployAction::Deploy => run_deploy(&mut runner, req, assets_dir).await,
        DeployAction::Uninstall => run_uninstall(&mut runner, req, assets_dir).await,
    };

    // Cleanly close SSH connection on exit
    if let Some(session) = runner.session.take() {
        let _ = session.close().await;
    }

    result
}

/// Executes full deployment sequence corresponding to Kotlin `performDeploy`.
async fn run_deploy(
    runner: &mut DeployRunner<'_>,
    req: &DeployRequest,
    assets_dir: &Path,
) -> anyhow::Result<DeployOutcome> {
    (runner.on_output)(DeployOutputLine::new(
        "SSH/SFTP используют текущий системный маршрут",
        DeployOutputLevel::Log,
    ));
    (runner.on_output)(DeployOutputLine::new(
        "Подключение к VPS по SSH",
        DeployOutputLevel::Log,
    ));

    if let Err(e) = runner.connect().await {
        let friendly = ops_policy::friendly_deploy_error(&e.to_string());
        (runner.on_output)(DeployOutputLine::new(&friendly, DeployOutputLevel::Err));
        anyhow::bail!("{friendly}");
    }
    (runner.on_output)(DeployOutputLine::new(
        "SSH-соединение установлено",
        DeployOutputLevel::Ok,
    ));

    // 1. Probe CPU architecture via `uname -m` (timeout 10 s)
    let arch_res = runner
        .exec_with_retry("uname -m", "определение архитектуры", Duration::from_millis(10_000))
        .await?;

    if arch_res.exit_code != Some(0) {
        let trimmed: String = arch_res.output.trim().chars().take(160).collect();
        anyhow::bail!("Не удалось определить архитектуру VPS: {trimmed}");
    }
    let server_arch = ops_policy::server_architecture_for_machine(&arch_res.output)?;
    (runner.on_output)(DeployOutputLine::new(
        format!("Архитектура VPS: {}", server_arch.display_name()),
        DeployOutputLevel::Log,
    ));

    // 2. Prepare local assets
    (runner.on_output)(DeployOutputLine::new(
        "Подготовка файлов установки",
        DeployOutputLevel::Log,
    ));

    let script_bytes = resolve_asset_file(assets_dir, "deploy.sh")?;
    let binary_bytes = resolve_asset_file(assets_dir, server_arch.asset_name())?;
    let env_content = generate_env_content(req);
    let overrides_content = generate_overrides_content(req);

    (runner.on_output)(DeployOutputLine::new(
        "Файлы установки подготовлены",
        DeployOutputLevel::Ok,
    ));

    // 3. Upload assets to /tmp/
    runner
        .upload_with_retry(&script_bytes, "/tmp/deploy.sh", 0o755, "deploy.sh")
        .await?;
    runner
        .upload_with_retry(
            &binary_bytes,
            "/tmp/.csqtt-upload-server",
            0o755,
            server_arch.asset_name(),
        )
        .await?;
    runner
        .upload_with_retry(
            env_content.as_bytes(),
            "/tmp/.csqtt-upload-web.env",
            0o600,
            "csqtt.env",
        )
        .await?;
    runner
        .upload_with_retry(
            overrides_content.as_bytes(),
            "/tmp/.csqtt-upload-overrides.json",
            0o600,
            "csqtt-deploy.json",
        )
        .await?;

    // 4. Run installer script on VPS (timeout 15 minutes)
    (runner.on_output)(DeployOutputLine::new(
        "Запуск установщика на VPS",
        DeployOutputLevel::Log,
    ));

    let deploy_cmd = build_deploy_command(req);
    let deploy_res = runner
        .exec_with_retry(
            &deploy_cmd,
            "установка CSQTT",
            Duration::from_millis(ops_policy::CMD_TIMEOUT_MS),
        )
        .await?;

    let exit_status = deploy_res.exit_code.unwrap_or(u32::MAX) as i32;
    if ops_policy::is_successful_deploy_result(exit_status, &deploy_res.output) {
        let success_message = if req.install_in_docker {
            "Установка завершена · Docker-контейнер CSQTT активен".to_string()
        } else {
            "Установка завершена · служба CSQTT активна".to_string()
        };
        (runner.on_output)(DeployOutputLine::new(
            &success_message,
            DeployOutputLevel::Ok,
        ));
        let password_to_sync =
            ops_policy::password_to_synchronize_after_deploy(true, &req.main_pass);
        let panel = panel_url(req);
        Ok(DeployOutcome {
            success: true,
            message: success_message,
            architecture: Some(server_arch),
            password_to_synchronize: password_to_sync,
            panel_url: Some(panel),
            raw_output: deploy_res.output,
        })
    } else {
        let failure_message = ops_policy::deploy_failure_message(exit_status, &deploy_res.output);
        (runner.on_output)(DeployOutputLine::new(
            &failure_message,
            DeployOutputLevel::Err,
        ));
        anyhow::bail!("{failure_message}");
    }
}

/// Executes uninstall sequence corresponding to Kotlin `performUninstall`.
async fn run_uninstall(
    runner: &mut DeployRunner<'_>,
    req: &DeployRequest,
    assets_dir: &Path,
) -> anyhow::Result<DeployOutcome> {
    (runner.on_output)(DeployOutputLine::new(
        "SSH/SFTP используют текущий системный маршрут",
        DeployOutputLevel::Log,
    ));
    (runner.on_output)(DeployOutputLine::new(
        "Подключение к VPS по SSH",
        DeployOutputLevel::Log,
    ));

    if let Err(e) = runner.connect().await {
        let friendly = ops_policy::friendly_deploy_error(&e.to_string());
        (runner.on_output)(DeployOutputLine::new(&friendly, DeployOutputLevel::Err));
        anyhow::bail!("{friendly}");
    }
    (runner.on_output)(DeployOutputLine::new(
        "SSH-соединение установлено",
        DeployOutputLevel::Ok,
    ));

    // 1. Prepare and upload deploy.sh for uninstall
    (runner.on_output)(DeployOutputLine::new(
        "Загрузка штатного установщика CSQTT",
        DeployOutputLevel::Log,
    ));
    let script_bytes = resolve_asset_file(assets_dir, "deploy.sh")?;
    runner
        .upload_with_retry(&script_bytes, "/tmp/deploy.sh", 0o755, "deploy.sh")
        .await?;

    // 2. Execute uninstall on VPS (timeout 30 s)
    (runner.on_output)(DeployOutputLine::new(
        "Запуск серверного удаления",
        DeployOutputLevel::Log,
    ));

    let uninstall_cmd = build_uninstall_command(req);
    let uninstall_res = runner
        .exec_with_retry(
            &uninstall_cmd,
            "удаление CSQTT",
            Duration::from_millis(30_000),
        )
        .await?;

    let exit_status = uninstall_res.exit_code.unwrap_or(u32::MAX) as i32;
    if exit_status == 0 {
        let success_message = "Удаление CSQTT завершено".to_string();
        (runner.on_output)(DeployOutputLine::new(
            &success_message,
            DeployOutputLevel::Ok,
        ));
        Ok(DeployOutcome {
            success: true,
            message: success_message,
            architecture: None,
            password_to_synchronize: None,
            panel_url: None,
            raw_output: uninstall_res.output,
        })
    } else {
        let failure_message = format!("Удаление завершилось с кодом {exit_status}");
        (runner.on_output)(DeployOutputLine::new(
            &failure_message,
            DeployOutputLevel::Err,
        ));
        anyhow::bail!("{failure_message}");
    }
}

// ---------------------------------------------------------------------------
// Unit Tests (Pure helpers, command building, asset resolution)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_build_deploy_environment_systemd() {
        let req = DeployRequest {
            host: "192.168.1.100".to_string(),
            port: 22,
            peer_port: 46000,
            web_port: 46002,
            install_in_docker: false,
            secret_path: String::new(),
            ..Default::default()
        };
        let env = build_deploy_environment(&req);
        assert_eq!(
            env,
            "env CSQTT_PEER_PORT=46000 CSQTT_SSH_PORT=22 CSQTT_WEB_PORT=46002 CSQTT_PUBLIC_IP=192.168.1.100 CSQTT_DEPLOY_MODE=systemd"
        );
    }

    #[test]
    fn test_build_deploy_environment_docker_with_secret() {
        let req = DeployRequest {
            host: "10.0.0.5".to_string(),
            port: 2222,
            peer_port: 48000,
            web_port: 48002,
            install_in_docker: true,
            secret_path: "admin-secret-path".to_string(),
            ..Default::default()
        };
        let env = build_deploy_environment(&req);
        assert!(env.contains("CSQTT_WEB_SECRET_PATH=\"admin-secret-path\" "));
        assert!(env.contains("CSQTT_PEER_PORT=48000"));
        assert!(env.contains("CSQTT_SSH_PORT=2222"));
        assert!(env.contains("CSQTT_WEB_PORT=48002"));
        assert!(env.contains("CSQTT_PUBLIC_IP=10.0.0.5"));
        assert!(env.contains("CSQTT_DEPLOY_MODE=docker"));
    }

    #[test]
    fn test_build_deploy_command() {
        let req = DeployRequest {
            host: "server.test".to_string(),
            sudo_password: Some("secret'pass".to_string()),
            ..Default::default()
        };
        let cmd = build_deploy_command(&req);
        assert!(cmd.contains("bash /tmp/deploy.sh install"));
        assert!(cmd.contains("if [ \"$(id -u)\" = \"0\" ]; then bash -c '"));
        assert!(cmd.contains("printf '%s\\n' 'secret'\"'\"'pass' | sudo -S -p '' bash -c '"));
    }

    #[test]
    fn test_build_uninstall_command() {
        let req = DeployRequest {
            peer_port: 47000,
            port: 2200,
            sudo_password: None,
            ..Default::default()
        };
        let cmd = build_uninstall_command(&req);
        assert!(cmd.contains("env CSQTT_PEER_PORT=47000 CSQTT_SSH_PORT=2200 bash /tmp/deploy.sh uninstall"));
        assert!(cmd.contains("sudo -S -p '' bash -c '"));
    }

    #[test]
    fn test_generate_env_content() {
        let req = DeployRequest {
            host: "vpn.example.com".to_string(),
            web_login: "admin".to_string(),
            web_pass: "mypassword".to_string(),
            install_in_docker: false,
            secret_path: "secret-uuid".to_string(),
            ..Default::default()
        };
        let env_file = generate_env_content(&req);
        assert!(env_file.contains("CSQTT_WEB_USER=\"admin\"\n"));
        assert!(env_file.contains("CSQTT_WEB_PASS=\"mypassword\"\n"));
        assert!(env_file.contains("CSQTT_PUBLIC_IP=\"vpn.example.com\"\n"));
        assert!(env_file.contains("CSQTT_WEB_SECRET_PATH=\"secret-uuid\"\n"));

        let req_no_secret = DeployRequest {
            host: "vpn.example.com".to_string(),
            web_login: "admin".to_string(),
            web_pass: "mypassword".to_string(),
            install_in_docker: true,
            secret_path: "".to_string(),
            ..Default::default()
        };
        let env_docker = generate_env_content(&req_no_secret);
        assert!(env_docker.contains("CSQTT_WEB_USER=admin\n"));
        assert!(!env_docker.contains("CSQTT_WEB_SECRET_PATH"));
    }

    #[test]
    fn test_generate_overrides_content() {
        let req = DeployRequest {
            main_pass: "0123456789abcdef".to_string(),
            ..Default::default()
        };
        let overrides = generate_overrides_content(&req);
        let parsed: serde_json::Value = serde_json::from_str(&overrides).unwrap();
        assert_eq!(parsed["main_password"], "0123456789abcdef");
    }

    #[test]
    fn test_panel_url() {
        let req1 = DeployRequest {
            host: "1.2.3.4".to_string(),
            web_port: 46002,
            secret_path: "custom-path".to_string(),
            ..Default::default()
        };
        assert_eq!(panel_url(&req1), "https://1.2.3.4:46002/custom-path");

        let req2 = DeployRequest {
            host: "1.2.3.4".to_string(),
            web_port: 8443,
            secret_path: "/already-slash".to_string(),
            ..Default::default()
        };
        assert_eq!(panel_url(&req2), "https://1.2.3.4:8443/already-slash");

        let req3 = DeployRequest {
            host: "".to_string(),
            web_port: 46002,
            secret_path: "".to_string(),
            ..Default::default()
        };
        assert_eq!(panel_url(&req3), "https://IP:46002/uuid");
    }

    #[test]
    fn test_resolve_asset_file() {
        let temp_dir = std::env::temp_dir().join(format!("csqtt-ops-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let valid_file = temp_dir.join("deploy.sh");
        std::fs::write(&valid_file, b"#!/bin/bash\necho ok").unwrap();

        let empty_file = temp_dir.join("empty.sh");
        std::fs::write(&empty_file, b"").unwrap();

        let resolved = resolve_asset_file(&temp_dir, "deploy.sh").unwrap();
        assert_eq!(resolved, b"#!/bin/bash\necho ok");

        let empty_err = resolve_asset_file(&temp_dir, "empty.sh").unwrap_err();
        assert!(empty_err.to_string().contains("Файл empty.sh отсутствует или пуст в assets"));

        let missing_err = resolve_asset_file(&temp_dir, "missing.bin").unwrap_err();
        assert!(missing_err.to_string().contains("Файл missing.bin отсутствует или пуст в assets"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_deploy_request_defaults_and_helpers() {
        let req = DeployRequest::default();
        assert_eq!(req.port, 22);
        assert_eq!(req.peer_port, 46000);
        assert_eq!(req.web_port, 46002);
        assert_eq!(req.user, "root");
        assert!(!req.docker_install());
        assert_eq!(req.main_password(), "");

        let mut req2 = req.clone();
        assert_eq!(req, req2);
        req2.host = "other".to_string();
        assert_ne!(req, req2);
    }

    #[test]
    fn test_deploy_outcome_helpers() {
        let outcome = DeployOutcome {
            success: true,
            message: "Установка завершена".to_string(),
            architecture: Some(ServerArchitecture::Amd64),
            password_to_synchronize: Some("0123456789abcdef".to_string()),
            panel_url: Some("https://1.2.3.4:46002/uuid".to_string()),
            raw_output: "CSQTT_DEPLOY_OK\n".to_string(),
        };
        assert!(outcome.is_success());
        assert_eq!(outcome.password_to_sync(), Some("0123456789abcdef"));
    }
}
