//! Named pipe transport for the csqtt-win protocol.
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};
use tokio::time::{sleep, Instant};
use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_NOT_CONNECTED, HLOCAL, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

use crate::{Request, Response};

/// RAII wrapper for a Windows `SECURITY_DESCRIPTOR` allocated from SDDL.
struct PipeSecurityDescriptor(PSECURITY_DESCRIPTOR);

// SAFETY: `PipeSecurityDescriptor` points to a self-contained, read-only security
// descriptor in memory created from a static SDDL string. It can be safely transferred
// and referenced across thread boundaries.
unsafe impl Send for PipeSecurityDescriptor {}
unsafe impl Sync for PipeSecurityDescriptor {}

impl Drop for PipeSecurityDescriptor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `self.0` was allocated by `ConvertStringSecurityDescriptorToSecurityDescriptorW`
            // and must be released via `LocalFree`.
            unsafe {
                LocalFree(self.0 as HLOCAL);
            }
        }
    }
}

impl PipeSecurityDescriptor {
    fn from_sddl(sddl: &str) -> anyhow::Result<Self> {
        let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut sd_ptr: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let mut sd_size: u32 = 0;

        // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW receives a valid
        // null-terminated UTF-16 string and writes a pointer to the allocated security
        // descriptor into `sd_ptr` on success.
        let success = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut sd_ptr,
                &mut sd_size,
            )
        };

        if success == 0 {
            return Err(std::io::Error::last_os_error().into());
        }

        Ok(Self(sd_ptr))
    }
}

/// Helper to create a `NamedPipeServer` instance with the specified security descriptor.
fn create_pipe_instance(
    name: &str,
    first: bool,
    sd_ptr: PSECURITY_DESCRIPTOR,
) -> anyhow::Result<NamedPipeServer> {
    let mut sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_ptr,
        bInheritHandle: 0,
    };

    // SAFETY: `sa` is a valid stack-allocated SECURITY_ATTRIBUTES struct, and `sd_ptr`
    // points to a valid security descriptor allocated from SDDL.
    let server = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(name, &mut sa as *mut _ as *mut c_void)?
    };

    Ok(server)
}

fn is_disconnect_err(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::BrokenPipe
        || e.kind() == std::io::ErrorKind::ConnectionReset
        || e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32)
        || e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as i32)
}
/// Reads one LF-terminated line. Cancellation-safe: bytes of an unfinished line stay in
/// `buf` (owned by the connection) and the next call continues from there. The length is
/// capped at MAX_LINE while reading, so a client cannot make the service buffer unbounded data.
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Option<Vec<u8>>> {
    loop {
        let available = match reader.fill_buf().await {
            Ok(a) => a,
            Err(e) if buf.is_empty() && is_disconnect_err(&e) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if available.is_empty() {
            // EOF: a trailing partial line is dropped like a disconnect.
            buf.clear();
            return Ok(None);
        }
        let (take, done) = match available.iter().position(|&b| b == 10) {
            Some(i) => (i + 1, true),
            None => (available.len(), false),
        };
        buf.extend_from_slice(&available[..take]);
        reader.consume(take);
        anyhow::ensure!(
            buf.len() <= crate::MAX_LINE,
            "превышен максимальный размер строки: {} байт",
            buf.len()
        );
        if done {
            return Ok(Some(std::mem::take(buf)));
        }
    }
}

/// Client side (CLI / tray). Connects to PIPE_NAME, retrying ERROR_PIPE_BUSY for up to 5 s.
pub struct Client {
    pipe: BufReader<NamedPipeClient>,
    /// Partial line kept across calls so `recv` is cancellation-safe inside `select!`.
    buf: Vec<u8>,
}

impl Client {
    pub async fn connect() -> anyhow::Result<Self> {
        Self::connect_named(crate::PIPE_NAME).await
    }

    pub(crate) async fn connect_named(name: &str) -> anyhow::Result<Self> {
        let start = Instant::now();
        let timeout = Duration::from_secs(5);
        let retry_delay = Duration::from_millis(100);

        loop {
            match ClientOptions::new().open(name) {
                Ok(client) => {
                    return Ok(Self {
                        pipe: BufReader::new(client),
                        buf: Vec::new(),
                    });
                }
                Err(e) => {
                    let raw = e.raw_os_error();
                    if raw == Some(ERROR_PIPE_BUSY as i32) {
                        if start.elapsed() >= timeout {
                            anyhow::bail!("Таймаут ожидания канала (канал занят): {}", e);
                        }
                        sleep(retry_delay).await;
                        continue;
                    }
                    if raw == Some(ERROR_FILE_NOT_FOUND as i32)
                        || e.kind() == std::io::ErrorKind::NotFound
                    {
                        anyhow::bail!("Служба csqtt-win не запущена");
                    }
                    return Err(e.into());
                }
            }
        }
    }

    pub async fn send(&mut self, request: &Request) -> anyhow::Result<()> {
        let bytes = crate::encode_line(request)?;
        self.pipe.write_all(&bytes).await?;
        self.pipe.flush().await?;
        Ok(())
    }

    /// Next response; Ok(None) when the server closed the pipe.
    pub async fn recv(&mut self) -> anyhow::Result<Option<Response>> {
        match read_line(&mut self.pipe, &mut self.buf).await? {
            Some(line) => Ok(Some(crate::decode_line(&line)?)),
            None => Ok(None),
        }
    }
}

/// Server side: one connected client.
pub struct ServerConn {
    pipe: BufReader<NamedPipeServer>,
    /// Partial line kept across calls so `recv` is cancellation-safe inside `select!`.
    buf: Vec<u8>,
}

impl ServerConn {
    /// Next request; Ok(None) when the client disconnected.
    pub async fn recv(&mut self) -> anyhow::Result<Option<Request>> {
        match read_line(&mut self.pipe, &mut self.buf).await? {
            Some(line) => Ok(Some(crate::decode_line(&line)?)),
            None => Ok(None),
        }
    }

    pub async fn send(&mut self, response: &Response) -> anyhow::Result<()> {
        let bytes = crate::encode_line(response)?;
        self.pipe.write_all(&bytes).await?;
        self.pipe.flush().await?;
        Ok(())
    }
}

/// Accepts clients on PIPE_NAME with the PIPE_SDDL security descriptor.
pub struct Listener {
    name: String,
    server: NamedPipeServer,
    _sd: Arc<PipeSecurityDescriptor>,
}

impl Listener {
    /// Creates the first pipe instance (first_pipe_instance = true).
    pub fn bind() -> anyhow::Result<Self> {
        Self::bind_named(crate::PIPE_NAME)
    }

    pub(crate) fn bind_named(name: &str) -> anyhow::Result<Self> {
        let sd = PipeSecurityDescriptor::from_sddl(crate::PIPE_SDDL)?;
        let sd_arc = Arc::new(sd);
        let server = create_pipe_instance(name, true, sd_arc.0)?;
        Ok(Self {
            name: name.to_string(),
            server,
            _sd: sd_arc,
        })
    }

    /// Waits for the next client; a new pipe instance is created before returning.
    pub async fn accept(&mut self) -> anyhow::Result<ServerConn> {
        self.server.connect().await?;
        let next_server = create_pipe_instance(&self.name, false, self._sd.0)?;
        let connected = std::mem::replace(&mut self.server, next_server);
        Ok(ServerConn {
            pipe: BufReader::new(connected),
            buf: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_line_survives_cancellation() {
        use tokio::io::AsyncWriteExt;
        let (mut tx, rx) = tokio::io::duplex(64);
        let mut reader = BufReader::new(rx);
        let mut buf = Vec::new();
        tx.write_all(b"{\"cmd\":").await.unwrap();
        // The read is cancelled half-way through the line (like a losing select! branch).
        let first = tokio::time::timeout(Duration::from_millis(50), read_line(&mut reader, &mut buf)).await;
        assert!(first.is_err());
        tx.write_all(b"\"status\"}\n").await.unwrap();
        let line = read_line(&mut reader, &mut buf).await.unwrap().unwrap();
        assert_eq!(crate::decode_line::<Request>(&line).unwrap(), Request::Status);
        drop(tx);
        assert!(read_line(&mut reader, &mut buf).await.unwrap().is_none());
    }
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_PIPE_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_test_pipe_name() -> String {
        let counter = TEST_PIPE_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!(r"\\.\pipe\csqtt-ipc-test-{}-{}", std::process::id(), counter)
    }

    #[tokio::test]
    async fn test_pipe_exchange_status_ok_and_disconnect() {
        let pipe_name = unique_test_pipe_name();
        let mut listener = Listener::bind_named(&pipe_name).expect("bind listener");

        let server_handle = tokio::spawn(async move {
            let mut conn = listener.accept().await.expect("listener accept");

            // Receive Request::Status
            let req = conn.recv().await.expect("server recv request");
            assert_eq!(req, Some(Request::Status));

            // Send Response::Ok
            conn.send(&Response::Ok).await.expect("server send response");

            // Client will disconnect; subsequent recv should return Ok(None)
            let eof = conn.recv().await.expect("server recv after disconnect");
            assert_eq!(eof, None);
        });

        let mut client = Client::connect_named(&pipe_name).await.expect("client connect");

        // Send Request::Status
        client.send(&Request::Status).await.expect("client send request");

        // Receive Response::Ok
        let resp = client.recv().await.expect("client recv response");
        assert_eq!(resp, Some(Response::Ok));

        // Close client connection
        drop(client);

        // Await server task completion
        server_handle.await.expect("server task finished successfully");
    }

    #[tokio::test]
    async fn test_client_connect_service_not_running() {
        let pipe_name = format!(r"\\.\pipe\csqtt-nonexistent-{}", std::process::id());
        let err = Client::connect_named(&pipe_name).await.err().expect("connect must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("Служба csqtt-win не запущена"),
            "expected 'Служба csqtt-win не запущена', got: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_multiple_sequential_connections() {
        let pipe_name = unique_test_pipe_name();
        let mut listener = Listener::bind_named(&pipe_name).expect("bind listener");

        let server_handle = tokio::spawn(async move {
            for _ in 0..2 {
                let mut conn = listener.accept().await.expect("accept");
                let req = conn.recv().await.expect("recv");
                assert_eq!(req, Some(Request::Status));
                conn.send(&Response::Ok).await.expect("send");
            }
        });

        for _ in 0..2 {
            let mut client = Client::connect_named(&pipe_name).await.expect("client connect");
            client.send(&Request::Status).await.expect("send");
            let resp = client.recv().await.expect("recv");
            assert_eq!(resp, Some(Response::Ok));
        }

        server_handle.await.expect("server handle");
    }
}
