//! Protocol between `csqtt-svc` (Windows service, SYSTEM) and `csqtt` (CLI / tray, user).
//!
//! Transport: named pipe [`PIPE_NAME`], one JSON object per line (UTF-8, '\n'), max
//! [`MAX_LINE`] bytes. The client sends a [`Request`]; the server answers with one or more
//! [`Response`]s. One-shot requests get exactly one response; `Logs { follow: false }` gets
//! the tail as `Log` lines followed by `Ok`. `Logs { follow: true }` and
//! `Subscribe` turn the connection into a stream that lasts until the client disconnects.
//! The pipe ACL lets SYSTEM and Administrators do anything and interactive users read/write,
//! so the CLI and tray work without UAC.

use serde::{Deserialize, Serialize};

pub mod pipe;

pub const PIPE_NAME: &str = r"\\.\pipe\csqtt-win";
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE: usize = 1 << 20;
/// SDDL of the pipe: SYSTEM + Administrators full, interactive users read/write.
pub const PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Hello { version: u32 },
    /// Connect with the stored settings.
    Connect,
    Disconnect,
    Status,
    /// Last `tail` log lines; with `follow` keep streaming new lines (and status changes).
    Logs { follow: bool, tail: usize },
    /// Stream status, stats, logs and captcha requests (used by the tray).
    Subscribe,
    /// Answer to a `Response::Captcha`: token or `error:<reason>`.
    CaptchaResult { token: String },
    /// A `csqtt://` link or the contents of an Android config export file.
    Import { text: String },
    Export { include_secrets: bool },
    GetSettings,
    /// Full settings object (as returned by GetSettings) to store.
    SetSettings { settings: serde_json::Value },
    SetSecret { key: String, value: String },
    /// Keyed log entries (the live log view): answered with `Response::LogBook`.
    LogBook,
    /// Clears the log book and the raw log buffer.
    ClearLogs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelState {
    Stopped,
    Starting,
    Connected,
    Reconnecting,
    Paused,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInfo {
    pub state: TunnelState,
    /// Reason / error text for Reconnecting and Failed.
    pub message: String,
    pub client_ip: String,
    pub dns: String,
    pub active_workers: i32,
    pub bytes_up: i64,
    pub bytes_down: i64,
}

impl Default for StatusInfo {
    fn default() -> Self {
        Self {
            state: TunnelState::Stopped,
            message: String::new(),
            client_ip: String::new(),
            dns: String::new(),
            active_workers: 0,
            bytes_up: 0,
            bytes_down: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Hello { version: u32, service_version: String },
    Status { status: StatusInfo },
    Log { line: String },
    Settings { settings: serde_json::Value },
    Exported { json: String },
    Captcha { mode: String, redirect_uri: String, session_token: String },
    /// Full keyed log list, sorted for display; also streamed to subscribers on change.
    LogBook { entries: Vec<LogBookEntry> },
}

/// One entry of the live log view: a repeated line updates its entry instead of appending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogBookEntry {
    pub key: String,
    pub message: String,
    pub count: u32,
    pub priority: i32,
    pub is_error: bool,
}

/// Serialize one message as a protocol line (JSON + '\n').
pub fn encode_line<T: Serialize>(message: &T) -> anyhow::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(message)?;
    anyhow::ensure!(bytes.len() < MAX_LINE, "message too large: {} bytes", bytes.len());
    bytes.push(b'\n');
    Ok(bytes)
}

/// Parse one protocol line (trailing '\r'/'\n' ignored).
pub fn decode_line<T: for<'de> Deserialize<'de>>(line: &[u8]) -> anyhow::Result<T> {
    let mut end = line.len();
    while end > 0 && (line[end - 1] == b'\n' || line[end - 1] == b'\r') {
        end -= 1;
    }
    Ok(serde_json::from_slice(&line[..end])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_format() {
        let line = encode_line(&Request::Logs { follow: true, tail: 50 }).unwrap();
        assert_eq!(line, b"{\"cmd\":\"logs\",\"follow\":true,\"tail\":50}\n");
        let back: Request = decode_line(&line).unwrap();
        assert_eq!(back, Request::Logs { follow: true, tail: 50 });
        assert_eq!(decode_line::<Request>(b"{\"cmd\":\"connect\"}\r\n").unwrap(), Request::Connect);
    }

    #[test]
    fn response_wire_format() {
        let line = encode_line(&Response::Status { status: StatusInfo::default() }).unwrap();
        let text = String::from_utf8(line.clone()).unwrap();
        assert!(text.starts_with("{\"type\":\"status\""));
        assert!(text.contains("\"state\":\"stopped\""));
        let back: Response = decode_line(&line).unwrap();
        assert_eq!(back, Response::Status { status: StatusInfo::default() });
    }
}
