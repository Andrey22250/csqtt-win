use anyhow::Result;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio_util::sync::CancellationToken;

const WINTUN_RING_CAPACITY: u32 = 0x400_000;

#[derive(Clone)]
pub struct TunSession {
    pub session: Arc<wintun::Session>,
}

unsafe impl Send for TunSession {}
unsafe impl Sync for TunSession {}

impl std::ops::Deref for TunSession {
    type Target = wintun::Session;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl std::fmt::Debug for TunSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunSession").finish_non_exhaustive()
    }
}

pub(crate) fn format_pipe_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Pipe name cannot be empty");
    }
    if trimmed.starts_with(r"\\.\pipe\") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!(r"\\.\pipe\{trimmed}"))
    }
}

pub(crate) fn parse_adapter_line(line: &[u8]) -> Result<String> {
    let s = std::str::from_utf8(line)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in adapter name: {e}"))?;
    let trimmed = s.trim_end_matches(|c| c == '\r' || c == '\n').trim();
    if trimmed.is_empty() {
        anyhow::bail!("Wintun adapter name is empty");
    }
    Ok(trimmed.to_string())
}

pub(crate) fn load_wintun() -> Result<wintun::Wintun> {
    static WINTUN: OnceLock<wintun::Wintun> = OnceLock::new();
    if let Some(w) = WINTUN.get() {
        return Ok(w.clone());
    }

    let loaded = (|| {
        if let Ok(mut exe) = std::env::current_exe() {
            exe.pop();
            let dll_path = exe.join("wintun.dll");
            if let Ok(w) = unsafe { wintun::load_from_path(&dll_path) } {
                return Ok(w);
            }
        }
        unsafe { wintun::load() }
    })();

    match loaded {
        Ok(w) => {
            let _ = WINTUN.set(w.clone());
            Ok(w)
        }
        Err(err) => {
            anyhow::bail!("Failed to load wintun.dll: {err}");
        }
    }
}

pub struct FdReceiver {
    pipe_name: String,
    server: tokio::sync::Mutex<Option<NamedPipeServer>>,
    has_created_first: AtomicBool,
}

impl FdReceiver {
    pub fn bind(name: &str) -> Result<Self> {
        let pipe_name = format_pipe_name(name)?;
        let has_runtime = tokio::runtime::Handle::try_current().is_ok();
        let (server, created_first) = if has_runtime {
            let s = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(&pipe_name)
                .map_err(|e| anyhow::anyhow!("Failed to create named pipe {pipe_name}: {e}"))?;
            (Some(s), true)
        } else {
            (None, false)
        };

        Ok(Self {
            pipe_name,
            server: tokio::sync::Mutex::new(server),
            has_created_first: AtomicBool::new(created_first),
        })
    }

    pub async fn receive(&self, cancel: &CancellationToken) -> Result<TunSession> {
        let mut guard = self.server.lock().await;

        loop {
            if cancel.is_cancelled() {
                anyhow::bail!("TUN receive cancelled");
            }

            let mut server = match guard.take() {
                Some(s) => s,
                None => {
                    let is_first = !self.has_created_first.swap(true, Ordering::SeqCst);
                    ServerOptions::new()
                        .first_pipe_instance(is_first)
                        .create(&self.pipe_name)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to create named pipe {}: {e}", self.pipe_name)
                        })?
                }
            };

            let connect_result = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    *guard = Some(server);
                    anyhow::bail!("TUN receive cancelled");
                }
                res = server.connect() => res,
            };

            if let Err(e) = connect_result {
                crate::log_error!(
                    "[ОШИБКА] Ошибка подключения к Named Pipe {}: {e}",
                    self.pipe_name
                );
                match ServerOptions::new().create(&self.pipe_name) {
                    Ok(next) => *guard = Some(next),
                    Err(err) => crate::log_error!(
                        "[ОШИБКА] Не удалось пересоздать Named Pipe: {err}"
                    ),
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                continue;
            }

            match ServerOptions::new().create(&self.pipe_name) {
                Ok(next_server) => {
                    *guard = Some(next_server);
                }
                Err(err) => {
                    crate::log_error!(
                        "[ОШИБКА] Не удалось создать следующий экземпляр Named Pipe: {err}"
                    );
                }
            }

            let mut line_bytes = Vec::new();
            let mut buf = [0u8; 1];
            let mut read_failed = false;

            loop {
                let read_res = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        anyhow::bail!("TUN receive cancelled");
                    }
                    res = server.read_exact(&mut buf) => res,
                };

                match read_res {
                    Ok(0) => {
                        read_failed = true;
                        break;
                    }
                    Ok(_) => {
                        if buf[0] == b'\n' {
                            break;
                        }
                        line_bytes.push(buf[0]);
                        if line_bytes.len() > 1024 {
                            read_failed = true;
                            break;
                        }
                    }
                    Err(err) => {
                        crate::log_error!(
                            "[ОШИБКА] Ошибка чтения имени адаптера из Named Pipe: {err}"
                        );
                        read_failed = true;
                        break;
                    }
                }
            }

            if read_failed {
                let _ = server.write_all(&[0]).await;
                let _ = server.flush().await;
                crate::log_error!("[ОШИБКА] Не удалось прочитать корректное имя Wintun адаптера");
                continue;
            }

            let adapter_name = match parse_adapter_line(&line_bytes) {
                Ok(name) => name,
                Err(err) => {
                    let _ = server.write_all(&[0]).await;
                    let _ = server.flush().await;
                    crate::log_error!("[ОШИБКА] Некорректное имя Wintun адаптера: {err}");
                    continue;
                }
            };

            let wintun = match load_wintun() {
                Ok(w) => w,
                Err(err) => {
                    let _ = server.write_all(&[0]).await;
                    let _ = server.flush().await;
                    crate::log_error!("[ОШИБКА] Не удалось загрузить wintun.dll: {err}");
                    continue;
                }
            };

            let adapter = match wintun::Adapter::open(&wintun, &adapter_name) {
                Ok(a) => a,
                Err(err) => {
                    let _ = server.write_all(&[0]).await;
                    let _ = server.flush().await;
                    crate::log_error!(
                        "[ОШИБКА] Не удалось открыть Wintun адаптер \"{adapter_name}\": {err}"
                    );
                    continue;
                }
            };

            let session = match adapter.start_session(WINTUN_RING_CAPACITY) {
                Ok(s) => Arc::new(s),
                Err(err) => {
                    let _ = server.write_all(&[0]).await;
                    let _ = server.flush().await;
                    crate::log_error!(
                        "[ОШИБКА] Не удалось запустить сессию Wintun (ring={WINTUN_RING_CAPACITY:#x}): {err}"
                    );
                    continue;
                }
            };

            if let Err(err) = server.write_all(&[1]).await {
                crate::log_error!("[ОШИБКА] Не удалось отправить ACK в Named Pipe: {err}");
                continue;
            }
            let _ = server.flush().await;

            return Ok(TunSession { session });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_name_formatting() {
        assert_eq!(
            format_pipe_name("csqtt_tun").unwrap(),
            r"\\.\pipe\csqtt_tun"
        );
        assert_eq!(
            format_pipe_name(r"\\.\pipe\already_prefixed").unwrap(),
            r"\\.\pipe\already_prefixed"
        );
        assert!(format_pipe_name("").is_err());
        assert!(format_pipe_name("   ").is_err());
    }

    #[test]
    fn test_parse_adapter_line() {
        assert_eq!(parse_adapter_line(b"Demo\n").unwrap(), "Demo");
        assert_eq!(parse_adapter_line(b"Demo\r\n").unwrap(), "Demo");
        assert_eq!(
            parse_adapter_line(b"  My Adapter 1  \n").unwrap(),
            "My Adapter 1"
        );
        assert!(parse_adapter_line(b"\n").is_err());
        assert!(parse_adapter_line(b"\r\n").is_err());
        assert!(parse_adapter_line(&[0xff, 0xfe, b'\n']).is_err());
    }

    #[tokio::test]
    async fn test_pipe_protocol_roundtrip() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::windows::named_pipe::ClientOptions;

        let pipe_name = format!(
            r"\\.\pipe\csqtt-test-pipe-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        );

        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .unwrap();

        let client_pipe = pipe_name.clone();
        let client_task = tokio::spawn(async move {
            let mut client = ClientOptions::new().open(&client_pipe).unwrap();
            client.write_all(b"TestAdapter\n").await.unwrap();
            let mut ack = [0u8; 1];
            client.read_exact(&mut ack).await.unwrap();
            ack[0]
        });

        server.connect().await.unwrap();
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            server.read_exact(&mut byte).await.unwrap();
            if byte[0] == b'\n' {
                break;
            }
            buf.push(byte[0]);
        }

        let name = parse_adapter_line(&buf).unwrap();
        assert_eq!(name, "TestAdapter");
        server.write_all(&[1]).await.unwrap();

        let ack = client_task.await.unwrap();
        assert_eq!(ack, 1);
    }
}
