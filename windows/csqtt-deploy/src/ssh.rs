//! Async SSH client on russh.

use std::sync::Arc;
use std::time::Duration;

use russh::ChannelMsg;
use russh::client::{self, Handle, Handler, KeyboardInteractiveAuthResponse};
use russh::keys::{decode_secret_key, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};

/// How to authenticate.
#[derive(Debug, Clone)]
pub enum SshAuth {
    Password(String),
    /// OpenSSH/PEM private key text and optional passphrase.
    Key { private_key: String, passphrase: Option<String> },
}

/// Host key decision callback: (host, port, key type, SHA256 fingerprint "SHA256:base64") -> accept?
pub type HostKeyCheck = Box<dyn Fn(&str, u16, &str, &str) -> bool + Send + Sync>;

/// Result of a remote command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub exit_code: Option<u32>,
    pub output: String,
}

/// Client handler that delegates host key checking to `HostKeyCheck`.
struct ClientHandler {
    host: String,
    port: u16,
    check: HostKeyCheck,
}

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let public_key = server_public_key.public_key();
        let algorithm = public_key.algorithm();
        let key_type = algorithm.as_str();
        let fingerprint = public_key.fingerprint(HashAlg::Sha256).to_string();
        let accepted = (self.check)(&self.host, self.port, key_type, &fingerprint);
        Ok(accepted)
    }
}

pub struct SshSession {
    handle: Handle<ClientHandler>,
}

impl SshSession {
    /// Connect (timeout 15 s), verify the host key via `check`, authenticate
    /// (password also tried as keyboard-interactive, like sshj AuthKeyboardInteractive).
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        auth: SshAuth,
        check: HostKeyCheck,
    ) -> anyhow::Result<Self> {
        tokio::time::timeout(
            Duration::from_secs(15),
            Self::connect_inner(host, port, user, auth, check),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Connection timeout (15 s)"))?
    }

    async fn connect_inner(
        host: &str,
        port: u16,
        user: &str,
        auth: SshAuth,
        check: HostKeyCheck,
    ) -> anyhow::Result<Self> {
        let config = Arc::new(client::Config::default());
        let handler = ClientHandler {
            host: host.to_string(),
            port,
            check,
        };

        let mut handle = client::connect(config, (host, port), handler).await?;

        match auth {
            SshAuth::Password(password) => {
                let auth_res = handle.authenticate_password(user, &password).await?;
                if !auth_res.success() {
                    let mut resp = handle
                        .authenticate_keyboard_interactive_start(user, None::<String>)
                        .await?;
                    loop {
                        match resp {
                            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                                let responses = vec![password.clone(); prompts.len()];
                                resp = handle
                                    .authenticate_keyboard_interactive_respond(responses)
                                    .await?;
                            }
                            KeyboardInteractiveAuthResponse::Success => break,
                            KeyboardInteractiveAuthResponse::Failure { .. } => {
                                anyhow::bail!("Auth fail (password)");
                            }
                        }
                    }
                }
            }
            SshAuth::Key {
                private_key,
                passphrase,
            } => {
                let key = decode_secret_key(&private_key, passphrase.as_deref())
                    .map_err(|e| anyhow::anyhow!("Auth fail (key parse): {e}"))?;
                let hash_alg = handle.best_supported_rsa_hash().await?.flatten();
                let key_with_hash = PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);
                let auth_res = handle.authenticate_publickey(user, key_with_hash).await?;
                if !auth_res.success() {
                    anyhow::bail!("Auth fail (public key)");
                }
            }
        }

        Ok(Self { handle })
    }

    /// Run a command; every output line (stdout+stderr) is passed to `on_line` as it arrives.
    pub async fn exec(
        &mut self,
        command: &str,
        on_line: &mut (dyn FnMut(&str) + Send),
    ) -> anyhow::Result<ExecResult> {
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, command.as_bytes().to_vec()).await?;

        let mut exit_code = None;
        let mut output = String::new();
        let mut splitter = LineSplitter::new();

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => {
                    for line in splitter.feed(data) {
                        on_line(&line);
                        output.push_str(&line);
                        output.push('\n');
                    }
                }
                ChannelMsg::ExtendedData { ref data, .. } => {
                    for line in splitter.feed(data) {
                        on_line(&line);
                        output.push_str(&line);
                        output.push('\n');
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = Some(exit_status);
                }
                ChannelMsg::Close => {
                    break;
                }
                ChannelMsg::Eof => {
                    if exit_code.is_some() {
                        break;
                    }
                }
                _ => {}
            }
        }

        for line in splitter.flush() {
            on_line(&line);
            output.push_str(&line);
            output.push('\n');
        }

        Ok(ExecResult { exit_code, output })
    }

    /// Write `data` to `remote_path` (via `cat > path` with stdin), then chmod `mode` (e.g. 0o755).
    pub async fn upload(&mut self, data: &[u8], remote_path: &str, mode: u32) -> anyhow::Result<()> {
        let quoted = quote_path(remote_path)?;
        let command = format!("cat > {quoted} && chmod {:o} {quoted}", mode);
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;
        channel.data(data).await?;
        channel.eof().await?;

        let mut exit_code = None;
        let mut stderr = String::new();
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::ExtendedData { ref data, .. } => {
                    stderr.push_str(&String::from_utf8_lossy(data));
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = Some(exit_status);
                }
                ChannelMsg::Close => {
                    break;
                }
                ChannelMsg::Eof => {
                    if exit_code.is_some() {
                        break;
                    }
                }
                _ => {}
            }
        }

        if exit_code != Some(0) {
            anyhow::bail!(
                "Upload failed with exit code {:?}: {}",
                exit_code,
                stderr.trim()
            );
        }

        Ok(())
    }

    pub async fn close(self) -> anyhow::Result<()> {
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        Ok(())
    }
}

/// Helper to quote a remote path safely for shell execution.
/// Rejects paths containing single quotes.
pub(crate) fn quote_path(path: &str) -> anyhow::Result<String> {
    if path.is_empty() {
        anyhow::bail!("remote path is empty");
    }
    if path.contains('\'') {
        anyhow::bail!("remote path contains single quote: {path:?}");
    }
    Ok(format!("'{path}'"))
}

/// Incremental line splitter that handles partial lines, \n, \r\n, and \r.
#[derive(Default, Debug)]
pub(crate) struct LineSplitter {
    buf: Vec<u8>,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Feeds a chunk of incoming data and returns complete lines decoded as UTF-8 lossy.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        self.process(false)
    }

    /// Flushes any remaining bytes at EOF as the final line.
    pub fn flush(&mut self) -> Vec<String> {
        self.process(true)
    }

    fn process(&mut self, is_eof: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let mut i = 0;
        let mut start = 0;

        while i < self.buf.len() {
            if self.buf[i] == b'\r' {
                if i + 1 < self.buf.len() {
                    if self.buf[i + 1] == b'\n' {
                        let line = String::from_utf8_lossy(&self.buf[start..i]).into_owned();
                        lines.push(line);
                        start = i + 2;
                        i = start;
                    } else {
                        let line = String::from_utf8_lossy(&self.buf[start..i]).into_owned();
                        lines.push(line);
                        start = i + 1;
                        i = start;
                    }
                } else if is_eof {
                    let line = String::from_utf8_lossy(&self.buf[start..i]).into_owned();
                    lines.push(line);
                    start = i + 1;
                    i = start;
                } else {
                    // \r at end of chunk: wait for next chunk to determine if followed by \n
                    break;
                }
            } else if self.buf[i] == b'\n' {
                let line = String::from_utf8_lossy(&self.buf[start..i]).into_owned();
                lines.push(line);
                start = i + 1;
                i = start;
            } else {
                i += 1;
            }
        }

        if start > 0 {
            self.buf.drain(..start);
        }

        if is_eof && !self.buf.is_empty() {
            let line = String::from_utf8_lossy(&self.buf).into_owned();
            self.buf.clear();
            lines.push(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_path_simple() {
        assert_eq!(quote_path("/etc/csqtt.conf").unwrap(), "'/etc/csqtt.conf'");
    }

    #[test]
    fn test_quote_path_with_spaces() {
        assert_eq!(quote_path("/opt/my app/bin").unwrap(), "'/opt/my app/bin'");
    }

    #[test]
    fn test_quote_path_with_special_characters() {
        assert_eq!(quote_path("/tmp/$FOO;rm -rf /").unwrap(), "'/tmp/$FOO;rm -rf /'");
    }

    #[test]
    fn test_quote_path_rejects_single_quote() {
        assert!(quote_path("/path/with/'single_quote").is_err());
        assert!(quote_path("'").is_err());
        assert!(quote_path("foo'bar").is_err());
    }

    #[test]
    fn test_quote_path_rejects_empty() {
        assert!(quote_path("").is_err());
    }

    #[test]
    fn test_line_splitter_lf() {
        let mut splitter = LineSplitter::new();
        assert_eq!(splitter.feed(b"line1\nline2\n"), vec!["line1", "line2"]);
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_crlf() {
        let mut splitter = LineSplitter::new();
        assert_eq!(splitter.feed(b"line1\r\nline2\r\n"), vec!["line1", "line2"]);
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_cr() {
        let mut splitter = LineSplitter::new();
        assert_eq!(
            splitter.feed(b"progress 10%\rprogress 20%\r"),
            vec!["progress 10%"]
        );
        // A trailing CR waits for the next chunk (it may be the first half of CRLF).
        assert_eq!(splitter.flush(), vec!["progress 20%"]);
    }

    #[test]
    fn test_line_splitter_mixed() {
        let mut splitter = LineSplitter::new();
        assert_eq!(
            splitter.feed(b"line1\nline2\r\nline3\rline4\n"),
            vec!["line1", "line2", "line3", "line4"]
        );
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_partial_across_chunks() {
        let mut splitter = LineSplitter::new();
        assert!(splitter.feed(b"part").is_empty());
        assert!(splitter.feed(b"ial ").is_empty());
        assert_eq!(splitter.feed(b"line\n"), vec!["partial line"]);
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_cr_chunk_boundary_followed_by_lf() {
        let mut splitter = LineSplitter::new();
        assert!(splitter.feed(b"line1\r").is_empty());
        assert_eq!(splitter.feed(b"\nline2\n"), vec!["line1", "line2"]);
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_cr_chunk_boundary_followed_by_char() {
        let mut splitter = LineSplitter::new();
        assert!(splitter.feed(b"line1\r").is_empty());
        assert_eq!(splitter.feed(b"x\n"), vec!["line1", "x"]);
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_flush_without_trailing_newline() {
        let mut splitter = LineSplitter::new();
        assert_eq!(splitter.feed(b"line1\nline2"), vec!["line1"]);
        assert_eq!(splitter.flush(), vec!["line2"]);
    }

    #[test]
    fn test_line_splitter_flush_trailing_cr() {
        let mut splitter = LineSplitter::new();
        assert!(splitter.feed(b"line1\r").is_empty());
        assert_eq!(splitter.flush(), vec!["line1"]);
    }

    #[test]
    fn test_line_splitter_empty_chunks() {
        let mut splitter = LineSplitter::new();
        assert!(splitter.feed(b"").is_empty());
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_consecutive_newlines() {
        let mut splitter = LineSplitter::new();
        assert_eq!(splitter.feed(b"line1\n\nline2\n"), vec!["line1", "", "line2"]);
        assert!(splitter.flush().is_empty());
    }

    #[test]
    fn test_line_splitter_crlf_consecutive() {
        let mut splitter = LineSplitter::new();
        assert_eq!(
            splitter.feed(b"line1\r\n\r\nline2\r\n"),
            vec!["line1", "", "line2"]
        );
        assert!(splitter.flush().is_empty());
    }
}
