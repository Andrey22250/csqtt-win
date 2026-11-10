//! Port of TunnelEventParser.kt.

pub const CALL_NOT_FOUND_CODE: i32 = 951;
pub const MIN_SERVER_WORKERS: i32 = 3;

/// Events emitted by the native tunnel process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    Started {
        message: String,
    },
    Stopped {
        message: String,
    },
    Process {
        pid: i32,
    },
    Ready {
        worker: i32,
        message: String,
    },
    ActiveZero {
        message: String,
    },
    CallUnavailable {
        hash: String,
        code: i32,
    },
    NetworkSuspect,
    ServerRestart,
    Config {
        config: String,
    },
    Stats {
        active: i32,
        bytes_up: i64,
        bytes_down: i64,
    },
    Error {
        code: String,
        message: String,
        fatal: bool,
    },
    CaptchaRequest {
        mode: String,
        redirect_uri: String,
        session_token: String,
    },
    CaptchaDone {
        success: bool,
        error: Option<String>,
    },
    Progress {
        kind: String,
    },
    Dtls {
        worker: i32,
        message: String,
    },
    Routing {
        routing: String,
    },
    Workers {
        workers: i32,
    },
    TunLost,
}

/// Parser for structured events emitted by the native tunnel client.
pub struct TunnelEventParser;

impl TunnelEventParser {
    pub const CALL_NOT_FOUND_CODE: i32 = CALL_NOT_FOUND_CODE;
    pub const MIN_SERVER_WORKERS: i32 = MIN_SERVER_WORKERS;

    /// Parses a single line into a tunnel `Event`, if valid.
    pub fn parse(line: &str) -> Option<Event> {
        parse(line)
    }
}

/// Helper to replicate `org.json.JSONObject.optString` lenient behavior.
fn opt_string(map: &serde_json::Map<String, serde_json::Value>, key: &str, default: &str) -> String {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => default.to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(other) => other.to_string(),
    }
}

/// Helper to replicate `org.json.JSONObject.optInt` lenient behavior.
fn opt_int(map: &serde_json::Map<String, serde_json::Value>, key: &str, default: i32) -> i32 {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => default,
        Some(serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i as i32
            } else if let Some(u) = n.as_u64() {
                u as i32
            } else if let Some(f) = n.as_f64() {
                f as i32
            } else {
                default
            }
        }
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            trimmed.parse::<i32>().unwrap_or_else(|_| {
                trimmed.parse::<f64>().map(|f| f as i32).unwrap_or(default)
            })
        }
        _ => default,
    }
}

/// Helper to replicate `org.json.JSONObject.optLong` lenient behavior.
fn opt_long(map: &serde_json::Map<String, serde_json::Value>, key: &str, default: i64) -> i64 {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => default,
        Some(serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i
            } else if let Some(u) = n.as_u64() {
                u as i64
            } else if let Some(f) = n.as_f64() {
                f as i64
            } else {
                default
            }
        }
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            trimmed.parse::<i64>().unwrap_or_else(|_| {
                trimmed.parse::<f64>().map(|f| f as i64).unwrap_or(default)
            })
        }
        _ => default,
    }
}

/// Helper to replicate `org.json.JSONObject.optBoolean` lenient behavior.
fn opt_bool(map: &serde_json::Map<String, serde_json::Value>, key: &str, default: bool) -> bool {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => default,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => {
            if s.eq_ignore_ascii_case("true") {
                true
            } else if s.eq_ignore_ascii_case("false") {
                false
            } else {
                default
            }
        }
        _ => default,
    }
}

/// Helper to replicate Kotlin `JSONObject.strictString` behavior: returns `Some(&str)` only if key exists and is a String.
fn strict_string<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    match map.get(key) {
        Some(serde_json::Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Parses a line starting with `__CSQTT_EVENT__|` into an `Event`.
pub fn parse(line: &str) -> Option<Event> {
    let without_prefix = line.strip_prefix(crate::constants::general::CSQTT_EVENT_PREFIX)?;
    let type_end = without_prefix.find('|')?;
    let event_type = &without_prefix[..type_end];
    let payload_str = &without_prefix[type_end + 1..];

    let Ok(serde_json::Value::Object(payload)) = serde_json::from_str(payload_str) else {
        return None;
    };

    match event_type {
        "STARTED" => Some(Event::Started {
            message: opt_string(&payload, "message", ""),
        }),
        "STOPPED" => Some(Event::Stopped {
            message: opt_string(&payload, "message", ""),
        }),
        "PROCESS" => {
            let pid = opt_int(&payload, "pid", 0);
            if pid > 0 {
                Some(Event::Process { pid })
            } else {
                None
            }
        }
        "READY" => Some(Event::Ready {
            worker: opt_int(&payload, "worker", 0).clamp(0, crate::constants::tunnel::MAX_WORKERS),
            message: opt_string(&payload, "message", ""),
        }),
        "DTLS" => Some(Event::Dtls {
            worker: opt_int(&payload, "worker", 0).clamp(0, crate::constants::tunnel::MAX_WORKERS),
            message: opt_string(&payload, "message", ""),
        }),
        "ACTIVE_ZERO" => Some(Event::ActiveZero {
            message: opt_string(&payload, "message", ""),
        }),
        "CALL_UNAVAILABLE" => {
            let hash = opt_string(&payload, "hash", "").trim().to_string();
            if hash.is_empty() {
                None
            } else {
                let code = opt_int(&payload, "code", CALL_NOT_FOUND_CODE).max(0);
                Some(Event::CallUnavailable { hash, code })
            }
        }
        "NETWORK_SUSPECT" => Some(Event::NetworkSuspect),
        "SERVER_RESTART" => {
            if opt_string(&payload, "source", "") == "panel" {
                Some(Event::ServerRestart)
            } else {
                None
            }
        }
        "CONFIG" => Some(Event::Config {
            config: opt_string(&payload, "config", ""),
        }),
        "ROUTING" => Some(Event::Routing {
            routing: opt_string(&payload, "routing", ""),
        }),
        "TUN_LOST" => Some(Event::TunLost),
        "WORKERS" => {
            let workers = opt_int(&payload, "workers", -1);
            if workers == 0
                || (MIN_SERVER_WORKERS..=crate::constants::tunnel::MAX_WORKERS).contains(&workers)
            {
                Some(Event::Workers { workers })
            } else {
                None
            }
        }
        "STATS" => Some(Event::Stats {
            active: opt_int(&payload, "active", 0).max(0),
            bytes_up: opt_long(&payload, "bytes_up", 0).max(0),
            bytes_down: opt_long(&payload, "bytes_down", 0).max(0),
        }),
        "ERROR" => Some(Event::Error {
            code: opt_string(&payload, "code", ""),
            message: opt_string(&payload, "message", ""),
            fatal: opt_bool(&payload, "fatal", false),
        }),
        "CAPTCHA_REQUEST" => Some(Event::CaptchaRequest {
            mode: opt_string(&payload, "mode", "auto"),
            redirect_uri: opt_string(&payload, "redirect_uri", ""),
            session_token: opt_string(&payload, "session_token", ""),
        }),
        "CAPTCHA_DONE" => {
            let success = opt_bool(&payload, "success", false);
            let err_str = opt_string(&payload, "error", "");
            let error = if err_str.trim().is_empty() {
                None
            } else {
                Some(err_str)
            };
            Some(Event::CaptchaDone { success, error })
        }
        "PROGRESS" => {
            let kind = strict_string(&payload, "kind")?;
            if kind != "credentials" {
                return None;
            }
            Some(Event::Progress {
                kind: kind.to_string(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stats() {
        let event = TunnelEventParser::parse(
            "__CSQTT_EVENT__|STATS|{\"active\":3,\"bytes_up\":120,\"bytes_down\":450}",
        )
        .expect("expected Stats event");

        let Event::Stats {
            active,
            bytes_up,
            bytes_down,
        } = event
        else {
            panic!("unexpected event type: {:?}", event);
        };

        assert_eq!(3, active);
        assert_eq!(120, bytes_up);
        assert_eq!(450, bytes_down);
    }

    #[test]
    fn parses_fatal_error() {
        let event = TunnelEventParser::parse(
            "__CSQTT_EVENT__|ERROR|{\"code\":\"AUTH\",\"message\":\"denied\",\"fatal\":true}",
        )
        .expect("expected Error event");

        let Event::Error {
            code,
            message,
            fatal,
        } = event
        else {
            panic!("unexpected event type: {:?}", event);
        };

        assert_eq!("AUTH", code);
        assert_eq!("denied", message);
        assert!(fatal);
    }

    #[test]
    fn parses_workers() {
        let event = TunnelEventParser::parse("__CSQTT_EVENT__|WORKERS|{\"workers\":36}")
            .expect("expected Workers event");
        let Event::Workers { workers } = event else {
            panic!("unexpected event type: {:?}", event);
        };
        assert_eq!(36, workers);

        let event4 = TunnelEventParser::parse("__CSQTT_EVENT__|WORKERS|{\"workers\":4}")
            .expect("expected Workers event");
        let Event::Workers { workers: workers4 } = event4 else {
            panic!("unexpected event type: {:?}", event4);
        };
        assert_eq!(4, workers4);

        let event100 = TunnelEventParser::parse("__CSQTT_EVENT__|WORKERS|{\"workers\":100}")
            .expect("expected Workers event");
        let Event::Workers { workers: workers100 } = event100 else {
            panic!("unexpected event type: {:?}", event100);
        };
        assert_eq!(100, workers100);

        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|WORKERS|{\"workers\":2}").is_none());
        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|WORKERS|{\"workers\":101}").is_none());
    }

    #[test]
    fn rust_lifecycle_object_payloads_match_kotlin_contract() {
        assert!(matches!(
            TunnelEventParser::parse("__CSQTT_EVENT__|READY|{}"),
            Some(Event::Ready { .. })
        ));
        assert!(matches!(
            TunnelEventParser::parse("__CSQTT_EVENT__|STOPPED|{}"),
            Some(Event::Stopped { .. })
        ));
        assert!(matches!(
            TunnelEventParser::parse("__CSQTT_EVENT__|ACTIVE_ZERO|{}"),
            Some(Event::ActiveZero { .. })
        ));
        assert!(matches!(
            TunnelEventParser::parse("__CSQTT_EVENT__|NETWORK_SUSPECT|{}"),
            Some(Event::NetworkSuspect)
        ));
        assert!(matches!(
            TunnelEventParser::parse("__CSQTT_EVENT__|DTLS|{}"),
            Some(Event::Dtls { .. })
        ));
    }

    #[test]
    fn server_restart_requires_panel_source() {
        assert!(matches!(
            TunnelEventParser::parse("__CSQTT_EVENT__|SERVER_RESTART|{\"source\":\"panel\"}"),
            Some(Event::ServerRestart)
        ));
        assert!(TunnelEventParser::parse(
            "__CSQTT_EVENT__|SERVER_RESTART|{\"source\":\"network\"}"
        )
        .is_none());
        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|SERVER_RESTART|{}").is_none());
    }

    #[test]
    fn parses_credential_progress() {
        let progress = TunnelEventParser::parse(
            "__CSQTT_EVENT__|PROGRESS|{\"kind\":\"credentials\"}",
        )
        .expect("expected Progress event");
        let Event::Progress { kind } = progress else {
            panic!("unexpected event type: {:?}", progress);
        };
        assert_eq!("credentials", kind);
    }

    #[test]
    fn parses_call_unavailable_without_turning_it_into_a_log_error() {
        let event = TunnelEventParser::parse(
            "__CSQTT_EVENT__|CALL_UNAVAILABLE|{\"hash\":\"dead-hash\",\"code\":951}",
        )
        .expect("expected CallUnavailable event");
        let Event::CallUnavailable { hash, code } = event else {
            panic!("unexpected event type: {:?}", event);
        };
        assert_eq!("dead-hash", hash);
        assert_eq!(951, code);
        assert!(
            TunnelEventParser::parse("__CSQTT_EVENT__|CALL_UNAVAILABLE|{\"code\":951}").is_none()
        );
    }

    #[test]
    fn detailed_perf_events_are_ignored() {
        assert!(TunnelEventParser::parse(
            "__CSQTT_EVENT__|PERF_DETAIL|{\"rows\":[{\"name\":\"Crypto/obfs\"}]}"
        )
        .is_none());
    }

    #[test]
    fn rejects_unknown_progress_kinds() {
        assert!(
            TunnelEventParser::parse("__CSQTT_EVENT__|PROGRESS|{\"kind\":\"unknown\"}").is_none()
        );
    }

    #[test]
    fn handles_malformed_and_unknown_events() {
        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|CAPTCHA_DONE|invalid").is_none());
        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|READY|").is_none());
        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|READY|null").is_none());
        assert!(TunnelEventParser::parse("regular log line").is_none());
        assert!(TunnelEventParser::parse("__CSQTT_EVENT__|UNKNOWN|{}").is_none());
    }

    #[test]
    fn malformed_protocol_line_cannot_forge_ready_stopped_or_fatal_events() {
        for event_type in &["READY", "STOPPED", "ERROR", "STATS"] {
            for payload in &["", "null", "[]", "{", "garbage", "\0"] {
                assert!(
                    TunnelEventParser::parse(&format!("__CSQTT_EVENT__|{event_type}|{payload}"))
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn negative_stats_are_clamped_before_lifecycle_consumption() {
        let event = TunnelEventParser::parse(
            "__CSQTT_EVENT__|STATS|{\"active\":-9,\"bytes_up\":-1,\"bytes_down\":-2}",
        )
        .expect("expected Stats event");

        let Event::Stats {
            active,
            bytes_up,
            bytes_down,
        } = event
        else {
            panic!("unexpected event type: {:?}", event);
        };

        assert_eq!(0, active);
        assert_eq!(0, bytes_up);
        assert_eq!(0, bytes_down);
    }

    #[test]
    fn deterministic_protocol_pipe_chaos_never_throws_or_creates_negative_state() {
        let cases = std::env::var("CSQTT_ANDROID_PARSER_CASES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(|v| v.max(1))
            .unwrap_or(50_000);
        let mut random = (std::env::var("CSQTT_SOAK_SEED")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(104_729))
            ^ 0x6a09_e667_f3bc_c909_i64;
        let types = [
            "READY",
            "STOPPED",
            "ERROR",
            "STATS",
            "CONFIG",
            "CAPTCHA_DONE",
            "UNKNOWN",
        ];
        let mut covered = [false; 4];
        for index in 0..cases {
            random = next_random(random);
            let payload = if index % 16 == 0 {
                covered[2] = true;
                format!(
                    "{{\"active\":{},\"bytes_up\":{},\"bytes_down\":{}}}",
                    random as i32,
                    random >> 1,
                    random >> 2
                )
            } else {
                covered[3] = true;
                let len = ((random as u64 >> 8) % 128) as usize;
                let mut s = String::with_capacity(len);
                let mut value = random;
                for _ in 0..len {
                    value = next_random(value);
                    s.push(((value as u64 >> 16) & 0x7f) as u8 as char);
                }
                s
            };
            let line = if index % 3 == 0 {
                covered[0] = true;
                format!("__CSQTT_EVENT__|{}|{payload}", types[index % types.len()])
            } else {
                covered[1] = true;
                payload
            };
            let event = TunnelEventParser::parse(&line);
            if let Some(Event::Stats {
                active,
                bytes_up,
                bytes_down,
            }) = event
            {
                assert!(active >= 0);
                assert!(bytes_up >= 0);
                assert!(bytes_down >= 0);
            }
        }
        assert!(covered.iter().all(|&it| it));
    }

    fn next_random(value: i64) -> i64 {
        let mut next = value;
        next ^= next.wrapping_shl(13);
        next ^= ((next as u64).wrapping_shr(7)) as i64;
        next ^= next.wrapping_shl(17);
        next
    }
}
