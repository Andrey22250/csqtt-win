//! Hand the Wintun adapter to the core over its named pipe (Windows replacement of the TUN fd
//! handoff in TunVpnService.kt).
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

/// Connect to \\.\pipe\<pipe_name> (retry up to `attempts` times every 500 ms while the pipe
/// does not exist or is busy), write `adapter_name` + '\n', read one byte with `ack_timeout`.
/// [1] = Ok, [0] or EOF = error.
pub async fn hand_over(
    pipe_name: &str,
    adapter_name: &str,
    attempts: u32,
    ack_timeout: Duration,
) -> anyhow::Result<()> {
    if attempts == 0 {
        anyhow::bail!("pipe connection attempts must be greater than 0");
    }

    let pipe_path = format!(r"\\.\pipe\{pipe_name}");
    let mut client = None;

    for attempt in 1..=attempts {
        match ClientOptions::new().open(&pipe_path) {
            Ok(c) => {
                client = Some(c);
                break;
            }
            Err(e) => {
                let raw_err = e.raw_os_error();
                let is_retryable = raw_err
                    == Some(windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND as i32)
                    || raw_err == Some(windows_sys::Win32::Foundation::ERROR_PIPE_BUSY as i32)
                    || e.kind() == std::io::ErrorKind::NotFound;

                if is_retryable && attempt < attempts {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else if is_retryable {
                    anyhow::bail!(
                        "timed out connecting to named pipe '{}' after {} attempts: {}",
                        pipe_path,
                        attempts,
                        e
                    );
                } else {
                    anyhow::bail!(
                        "failed to connect to named pipe '{}' on attempt {}: {}",
                        pipe_path,
                        attempt,
                        e
                    );
                }
            }
        }
    }

    let mut client = client.ok_or_else(|| {
        anyhow::anyhow!("could not connect to named pipe '{}'", pipe_path)
    })?;

    let message = format!("{adapter_name}\n");
    client
        .write_all(message.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("failed to write adapter name to named pipe '{}': {e}", pipe_path))?;
    client
        .flush()
        .await
        .map_err(|e| anyhow::anyhow!("failed to flush named pipe '{}': {e}", pipe_path))?;

    let mut ack = [0u8; 1];
    let read_result = tokio::time::timeout(ack_timeout, client.read_exact(&mut ack)).await;
    match read_result {
        Ok(Ok(_)) => {
            if ack[0] == 1 {
                Ok(())
            } else {
                anyhow::bail!(
                    "core rejected adapter handoff on pipe '{}' (ack byte was {}, expected 1)",
                    pipe_path,
                    ack[0]
                );
            }
        }
        Ok(Err(e)) => anyhow::bail!(
            "failed to read ack byte from named pipe '{}' (unexpected EOF or I/O error): {e}",
            pipe_path
        ),
        Err(_) => anyhow::bail!(
            "timed out after {:?} waiting for ack byte from core on named pipe '{}'",
            ack_timeout,
            pipe_path
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::windows::named_pipe::ServerOptions;

    #[tokio::test]
    async fn test_hand_over_success() {
        let pipe_name = format!("csqtt-test-handoff-ok-{}", std::process::id());
        let pipe_path = format!(r"\\.\pipe\{pipe_name}");
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_path)
            .expect("create named pipe server");

        let server_task = tokio::spawn(async move {
            server.connect().await.expect("server connect client");
            let mut reader = BufReader::new(server);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("read line from client");
            assert_eq!(line, "csqtt0\n");
            let mut server = reader.into_inner();
            server.write_all(&[1]).await.expect("write ack 1");
            server.flush().await.expect("flush ack 1");
        });

        hand_over(&pipe_name, "csqtt0", 5, Duration::from_secs(2))
            .await
            .expect("hand_over should succeed");

        server_task.await.expect("server task completed");
    }

    #[tokio::test]
    async fn test_hand_over_rejected() {
        let pipe_name = format!("csqtt-test-handoff-fail-{}", std::process::id());
        let pipe_path = format!(r"\\.\pipe\{pipe_name}");
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_path)
            .expect("create named pipe server");

        let server_task = tokio::spawn(async move {
            server.connect().await.expect("server connect client");
            let mut reader = BufReader::new(server);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("read line from client");
            assert_eq!(line, "csqtt0\n");
            let mut server = reader.into_inner();
            server.write_all(&[0]).await.expect("write ack 0");
            server.flush().await.expect("flush ack 0");
        });

        let res = hand_over(&pipe_name, "csqtt0", 5, Duration::from_secs(2)).await;
        assert!(res.is_err(), "expected error on ack [0]");

        server_task.await.expect("server task completed");
    }
}
