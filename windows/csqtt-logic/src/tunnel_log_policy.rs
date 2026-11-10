//! Port of TunnelLogPolicy.kt.

/// Severity / classification level of a tunnel log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Ok,
    Dbg,
    Net,
    Ping,
    Turn,
    Log,
    Err,
}

/// Policy decisions on log filtering and classification.
pub struct TunnelLogPolicy;

impl TunnelLogPolicy {
    /// Checks whether an entry is permitted to be logged when inactive logging is enabled.
    pub fn is_allowed_when_inactive_logging(
        key: &str,
        is_error: bool,
        level: Option<LogLevel>,
    ) -> bool {
        is_allowed_when_inactive_logging(key, is_error, level)
    }

    /// Checks whether a log message represents an internal recoverable condition that should be hidden.
    pub fn is_internal_recovery(line: &str) -> bool {
        is_internal_recovery(line)
    }

    /// Checks whether a log message represents a fatal TURN stream failure.
    pub fn is_turn_stream_failure(line: &str) -> bool {
        is_turn_stream_failure(line)
    }
}

/// Checks whether an entry is permitted to be logged when inactive logging is enabled.
pub fn is_allowed_when_inactive_logging(
    key: &str,
    is_error: bool,
    level: Option<LogLevel>,
) -> bool {
    is_error || level == Some(LogLevel::Err) || key == "ready" || key == "stats"
}

/// Checks whether a log message represents an internal recoverable condition that should be hidden.
pub fn is_internal_recovery(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.contains("fatal_") {
        return false;
    }
    lower.contains("getconf")
        || lower.contains("[сеть][retry]")
        || lower.contains("[turn][retry]")
        || lower.contains("[handshake][retry]")
        || lower.contains("peer_liveness_timeout")
        || lower.contains("486")
        || lower.contains("437")
        || (lower.contains("turn") && lower.contains("timeout"))
        || (lower.contains("allocation") && lower.contains("quota"))
        || (lower.contains("allocation") && lower.contains("mismatch"))
        || (lower.contains("[кред") && lower.contains("повторяем через"))
}

/// Checks whether a log message represents a fatal TURN stream failure.
pub fn is_turn_stream_failure(line: &str) -> bool {
    if is_internal_recovery(line) {
        return false;
    }
    let lower = line.to_lowercase();
    lower.contains("[turn][down]")
        || (lower.contains("[воркер")
            && lower.contains("[turn]")
            && (lower.contains("ошибка")
                || lower.contains("failed")
                || lower.contains("exhausted")))
}

/// Determines whether a character is an emoji/sticker to strip from user-visible logs.
/// Characters '✓' and '✗' are explicitly preserved.
fn is_log_sticker(c: char) -> bool {
    if c == '✓' || c == '✗' {
        return false;
    }
    if c == '\u{FE0F}' || c == '\u{200D}' {
        return true;
    }
    let u = c as u32;
    matches!(
        u,
        0x02C2..=0x02DF
            | 0x02E5..=0x02FF
            | 0x203C..=0x2049
            | 0x2122
            | 0x2139
            | 0x2190..=0x21FF
            | 0x2300..=0x23FF
            | 0x2460..=0x24FF
            | 0x25A0..=0x27BF
            | 0x2900..=0x297F
            | 0x2B00..=0x2BFF
            | 0x3030
            | 0x303D
            | 0x3297
            | 0x3299
            | 0x1F000..=0x1FAFF
    )
}

/// Removes empty tags matching `\[\s*\]` from the message.
fn remove_empty_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut j = i + 1;
            let mut all_ws = true;
            let mut found_close = false;
            while j < chars.len() {
                if chars[j] == ']' {
                    found_close = true;
                    break;
                } else if !chars[j].is_whitespace() {
                    all_ws = false;
                    break;
                }
                j += 1;
            }
            if found_close && all_ws {
                i = j + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Replaces consecutive horizontal whitespace characters with a single space.
fn collapse_horizontal_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_h_ws = false;
    for c in s.chars() {
        if c.is_whitespace() && c != '\r' && c != '\n' {
            if !in_h_ws {
                result.push(' ');
                in_h_ws = true;
            }
        } else {
            in_h_ws = false;
            result.push(c);
        }
    }
    result
}

/// Cleans status stickers and empty tags from visible log messages.
pub fn without_log_stickers(message: &str) -> String {
    let step1: String = message.chars().filter(|&c| !is_log_sticker(c)).collect();
    let step2 = remove_empty_tags(&step1);
    let step3 = collapse_horizontal_whitespace(&step2);
    step3.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_status_stickers_from_visible_logs() {
        assert_eq!(
            "VPN TUN: разрешение Android VPN было отозвано",
            without_log_stickers("❌ VPN TUN: разрешение Android VPN было отозвано")
        );
        assert_eq!("Поток готов ✓", without_log_stickers("Поток готов ✓"));
        assert_eq!("✗ Ошибка VPN", without_log_stickers("✗ Ошибка VPN"));
        assert_eq!("Сервис запущен", without_log_stickers("[✅] Сервис запущен"));
        assert_eq!(
            "[СЕТЬ] Переподключение",
            without_log_stickers("[СЕТЬ] ⚠ Переподключение")
        );
    }

    #[test]
    fn inactive_logging_keeps_only_ready_stats_and_errors() {
        assert!(TunnelLogPolicy::is_allowed_when_inactive_logging(
            "ready",
            false,
            Some(LogLevel::Ok)
        ));
        assert!(TunnelLogPolicy::is_allowed_when_inactive_logging(
            "stats",
            false,
            Some(LogLevel::Net)
        ));
        assert!(TunnelLogPolicy::is_allowed_when_inactive_logging(
            "turn_error",
            true,
            Some(LogLevel::Err)
        ));
        assert!(TunnelLogPolicy::is_allowed_when_inactive_logging(
            "general_error",
            true,
            None
        ));
        assert!(!TunnelLogPolicy::is_allowed_when_inactive_logging(
            "turn_refresh_status",
            false,
            Some(LogLevel::Log)
        ));
        assert!(!TunnelLogPolicy::is_allowed_when_inactive_logging(
            "wrap_status",
            false,
            Some(LogLevel::Ok)
        ));
        assert!(!TunnelLogPolicy::is_allowed_when_inactive_logging(
            "vk_js_call_progress",
            false,
            Some(LogLevel::Ok)
        ));
    }

    #[test]
    fn hides_every_getconf_diagnostic() {
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[GETCONF] Sending 72 bytes"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[ВОРКЕР #8] GETCONF timeout 750ms"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "GETCONF чтение ответа конфига: timeout"
        ));
        assert!(!TunnelLogPolicy::is_internal_recovery(
            "GETCONF: FATAL_AUTH: неверный пароль подключения"
        ));
        assert!(!TunnelLogPolicy::is_internal_recovery(
            "GETCONF: FATAL_PROTOCOL: клиент и сервер используют разные версии протокола"
        ));
    }

    #[test]
    fn hides_turn_retries_and_internal_recovery() {
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[TURN][RETRY] Ошибка обновления permission: timeout"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[TURN][RETRY] Ошибка транзакции ChannelBind: timeout"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[TURN][RETRY] Квота Allocate у креда #1; STUN error 486"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[TURN][RETRY] Allocation mismatch у креда #1; STUN error 437"
        ));
        assert!(!TunnelLogPolicy::is_internal_recovery(
            "[TURN][DOWN] Permission истекает, переподнимаем поток"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[ВОРКЕР #17] PEER_LIVENESS_TIMEOUT: 8s без валидных пакетов"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[ВОРКЕР #2] [TURN][RETRY] Попытка 46: TURN liveness probe failed"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[ВОРКЕР #2] [СЕТЬ][RETRY] Временная ошибка сокета"
        ));
        assert!(TunnelLogPolicy::is_internal_recovery(
            "[ВОРКЕР #1] Ошибка (попытка 1): turn allocate: transaction timeout"
        ));
    }

    #[test]
    fn surfaces_only_turn_failures_that_end_a_stream() {
        assert!(TunnelLogPolicy::is_turn_stream_failure(
            "[TURN][DOWN] Allocation истекает, переподнимаем поток"
        ));
        assert!(TunnelLogPolicy::is_turn_stream_failure(
            "[ВОРКЕР #4] [TURN] Ошибка allocation/кредов: socket closed"
        ));
        assert!(!TunnelLogPolicy::is_turn_stream_failure(
            "[ГРУППА #1] [TURN] Креды обновлены, TURN urls=3"
        ));
    }
}
