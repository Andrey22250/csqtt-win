//! Keyed log mechanism ported from Android TunnelManager.kt.
//!
//! Provides `LogBook` for managing prioritized, keyed log entries and
//! `CoreLineClassifier` for parsing native process stdout lines into keyed log updates.

use crate::tunnel_log_policy::{without_log_stickers, LogLevel, TunnelLogPolicy};

/// Maximum number of log entries kept in `LogBook`.
pub const MAX_LOG_ENTRIES: usize = 100;

/// Maximum length of a log entry message in characters.
pub const MAX_LOG_MESSAGE_LENGTH: usize = 4_096;

/// A single keyed log entry stored in `LogBook`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub key: String,
    pub message: String,
    pub count: u32,
    pub priority: i32,
    pub is_error: bool,
    pub level: Option<LogLevel>,
}

/// A classified update ready to be applied to `LogBook`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogUpdate {
    pub key: String,
    pub message: String,
    pub priority: i32,
    pub is_error: bool,
    pub level: Option<LogLevel>,
}

impl LogUpdate {
    pub fn new(
        key: impl Into<String>,
        message: impl Into<String>,
        priority: i32,
        is_error: bool,
        level: Option<LogLevel>,
    ) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
            priority,
            is_error,
            level,
        }
    }
}

/// Keyed logbook storing prioritized log entries.
#[derive(Debug, Clone, Default)]
pub struct LogBook {
    logs: Vec<LogEntry>,
}

impl LogBook {
    pub fn new() -> Self {
        Self { logs: Vec::new() }
    }

    /// Returns a slice of current log entries sorted by priority, is_error, and key.
    pub fn entries(&self) -> &[LogEntry] {
        &self.logs
    }

    /// Clears all entries from the logbook.
    /// Drops entries hidden when active logging is off (errors, ready and stats stay).
    pub fn retain_inactive_allowed(&mut self) {
        self.logs.retain(|e| TunnelLogPolicy::is_allowed_when_inactive_logging(&e.key, e.is_error, e.level));
    }

    pub fn clear(&mut self) {
        self.logs.clear();
    }

    /// Updates or inserts a log entry by key.
    ///
    /// Applies active logging policy, sticker stripping, length limit, count accumulation,
    /// priority re-sorting, and capacity trimming.
    /// Returns `true` if the book was modified.
    pub fn update(
        &mut self,
        key: &str,
        message: &str,
        priority: i32,
        is_error: bool,
        level: Option<LogLevel>,
        logging_enabled: bool,
    ) -> bool {
        if !logging_enabled && !TunnelLogPolicy::is_allowed_when_inactive_logging(key, is_error, level) {
            return false;
        }

        let clean_message = without_log_stickers(message);
        let clean_message: String = clean_message.chars().take(MAX_LOG_MESSAGE_LENGTH).collect();
        if clean_message.is_empty() {
            return false;
        }

        if let Some(index) = self.logs.iter().position(|e| e.key == key) {
            let old_count = self.logs[index].count;
            let new_count = old_count.saturating_add(1);

            if self.logs[index].priority == priority {
                self.logs[index].count = new_count;
                self.logs[index].message = clean_message;
                self.logs[index].is_error = is_error;
                self.logs[index].level = level;
            } else {
                self.logs.remove(index);
                self.insert_sorted(LogEntry {
                    key: key.to_string(),
                    message: clean_message,
                    count: new_count,
                    priority,
                    is_error,
                    level,
                });
            }
        } else {
            self.insert_sorted(LogEntry {
                key: key.to_string(),
                message: clean_message,
                count: 1,
                priority,
                is_error,
                level,
            });
        }

        while self.logs.len() > MAX_LOG_ENTRIES {
            self.logs.remove(0);
        }

        true
    }

    /// Helper to update the logbook from a `LogUpdate`.
    pub fn update_from(&mut self, update: &LogUpdate, logging_enabled: bool) -> bool {
        self.update(
            &update.key,
            &update.message,
            update.priority,
            update.is_error,
            update.level,
            logging_enabled,
        )
    }

    fn insert_sorted(&mut self, entry: LogEntry) {
        // Kotlin comparator:
        // compareBy<LogEntry>({ it.priority }, { if (it.isError) 1 else 0 }, { it.key })
        let pos = self.logs.iter().position(|item| {
            let ord = entry
                .priority
                .cmp(&item.priority)
                .then_with(|| (entry.is_error as u8).cmp(&(item.is_error as u8)))
                .then_with(|| entry.key.cmp(&item.key));
            ord == std::cmp::Ordering::Less
        });

        match pos {
            Some(i) => self.logs.insert(i, entry),
            None => self.logs.push(entry),
        }
    }
}

/// Java/Kotlin String.hashCode() implementation for exact key reproduction.
fn java_hash_code_take(s: &str, n: usize) -> i32 {
    let mut h: i32 = 0;
    for c in s.encode_utf16().take(n) {
        h = h.wrapping_mul(31).wrapping_add(c as i32);
    }
    h
}

fn java_hash_code(s: &str) -> i32 {
    java_hash_code_take(s, usize::MAX)
}

/// Substring after first occurrence of delimiter, or the original string if not found.
fn substring_after<'a>(s: &'a str, delimiter: &str) -> &'a str {
    match s.find(delimiter) {
        Some(idx) => &s[idx + delimiter.len()..],
        None => s,
    }
}

/// Removes timestamp prefix matching:
// Original regex: ^\d{4}/\d{2}/\d{2}\s\d{2}:\d{2}:\d{2}(\.\d+)?\s
fn strip_timestamp_prefix(line: &str) -> &str {
    let bytes = line.as_bytes();
    if bytes.len() <= 19 || !bytes[0].is_ascii_digit() || bytes[4] != b'/' {
        return line;
    }
    // Check YYYY/MM/DD
    if !bytes[1].is_ascii_digit()
        || !bytes[2].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
        || bytes[7] != b'/'
        || !bytes[5].is_ascii_digit()
        || !bytes[6].is_ascii_digit()
        || !bytes[8].is_ascii_digit()
        || !bytes[9].is_ascii_digit()
    {
        return line;
    }
    // Check whitespace between date and time
    if !bytes[10].is_ascii_whitespace() {
        return line;
    }
    // Check HH:MM:SS
    if !bytes[11].is_ascii_digit()
        || !bytes[12].is_ascii_digit()
        || bytes[13] != b':'
        || !bytes[14].is_ascii_digit()
        || !bytes[15].is_ascii_digit()
        || bytes[16] != b':'
        || !bytes[17].is_ascii_digit()
        || !bytes[18].is_ascii_digit()
    {
        return line;
    }
    let mut idx = 19;
    // Optional fractional seconds: (\.\d+)?
    if idx < bytes.len() && bytes[idx] == b'.' {
        idx += 1;
        let frac_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == frac_start {
            return line;
        }
    }
    // Trailing whitespace
    if idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        &line[idx + 1..]
    } else {
        line
    }
}

/// Replaces parenthesized tags and surrounding whitespace with a single space.
// Original regex: \s*\([^)]+\)\s*
fn remove_parenthesized_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '(' {
            let mut close_idx = None;
            for j in (i + 1)..chars.len() {
                if chars[j] == ')' {
                    close_idx = Some(j);
                    break;
                }
            }
            if let Some(j) = close_idx {
                while result.ends_with(char::is_whitespace) {
                    result.pop();
                }
                result.push(' ');
                let mut next = j + 1;
                while next < chars.len() && chars[next].is_whitespace() {
                    next += 1;
                }
                i = next;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result.trim().to_string()
}

/// State-tracking line classifier reproducing TunnelManager.kt line-by-line classification.
#[derive(Debug, Clone, Default)]
pub struct CoreLineClassifier {
    wrap_auth_timeout_count: u32,
    active_workers: u32,
}

impl CoreLineClassifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the number of active workers currently reported.
    pub fn set_active_workers(&mut self, n: u32) {
        self.active_workers = n;
    }

    /// Returns the current timeout counter for handshake verification.
    pub fn wrap_auth_timeout_count(&self) -> u32 {
        self.wrap_auth_timeout_count
    }

    /// Classifies an incoming process log line, returning any keyed updates.
    ///
    /// Preserves branch order, keys, priorities, messages, and error states from TunnelManager.kt.
    /// Filtering by `logging_enabled` is deferred to `LogBook::update`.
    pub fn classify(&mut self, line: &str) -> Vec<LogUpdate> {
        let stripped = strip_timestamp_prefix(line);
        let line_trim = stripped.trim();

        if line_trim.is_empty() {
            return Vec::new();
        }

        // TODO(port): CSQTT_EVENT_PREFIX and TunnelEventParser not available in context.txt; event handling is a non-logging side effect and skipped.

        if TunnelLogPolicy::is_internal_recovery(line_trim) {
            return Vec::new();
        }

        let lower = line_trim.to_lowercase();

        if lower.contains("wrap_auth_timeout") {
            if self.active_workers > 0 {
                self.wrap_auth_timeout_count = 0;
                return vec![LogUpdate::new(
                    "wrap_timeout_recovered",
                    format!(
                        "[WRAP] Один поток не прошёл handshake, активных={}; повторяем",
                        self.active_workers
                    ),
                    50,
                    true,
                    None,
                )];
            } else {
                self.wrap_auth_timeout_count = self.wrap_auth_timeout_count.saturating_add(1);
                return vec![LogUpdate::new(
                    "wrap_timeout_wait",
                    format!(
                        "[WRAP] Handshake не подтвердился, проверяем пароль/сеть ({})",
                        self.wrap_auth_timeout_count
                    ),
                    50,
                    true,
                    None,
                )];
            }
        }

        if line_trim.contains("FATAL_AUTH") || line_trim.contains("AUTH_RETRY") {
            let reason = if line_trim.contains("неверный пароль") {
                "Неверный пароль подключения"
            } else if line_trim.contains("истёк") {
                "Срок действия пароля истёк"
            } else if line_trim.contains("деактивирован") {
                "Аккаунт деактивирован"
            } else {
                "Ошибка авторизации"
            };
            return vec![LogUpdate::new(
                "circuit_breaker",
                format!("[СТОП] \u{1F512} {reason}. Воркеры остановлены."),
                -1,
                true,
                None,
            )];
        }

        if line_trim.contains("FATAL_PROTOCOL") {
            return vec![LogUpdate::new(
                "circuit_breaker",
                "[СТОП] 🔁 Клиент и сервер используют разные версии протокола. Выполните деплой из этой версии приложения. Воркеры остановлены.",
                -1,
                true,
                None,
            )];
        }

        if line_trim.starts_with("CAPTCHA_SOLVE|") {
            // TODO(port): CAPTCHA_SOLVE side effects (handleCaptchaSolve, writeCaptchaResult) not available in context.txt and skipped as non-logging.
            return Vec::new();
        }

        let is_error = lower.contains("ошибка")
            || lower.contains("error")
            || lower.contains("fail")
            || lower.contains("timeout")
            || lower.contains("refused")
            || lower.contains("fatal_");

        if line_trim.contains("[КАПЧА] AUTO:") {
            let raw_text = substring_after(line_trim, "[КАПЧА] AUTO:").trim();
            let text = remove_parenthesized_tags(raw_text);
            let text_lower = text.to_lowercase();
            let is_err = text_lower.contains("ошибка")
                || text_lower.contains("timeout")
                || text_lower.contains("не решил");
            let stable_key = if text.contains("старт") {
                "captcha_auto_1".to_string()
            } else if (text.contains("Go v2") || text.contains("Rust v2")) && text.contains("2 попыт") {
                "captcha_auto_2".to_string()
            } else if text.contains("WBV Auto попытка") {
                "captcha_auto_3".to_string()
            } else if text.contains("финальная") {
                "captcha_auto_4".to_string()
            } else if text.contains("ручной WebView") {
                "captcha_auto_5".to_string()
            } else if text.contains("решил") || text.contains("решила") {
                "captcha_auto_done".to_string()
            } else {
                format!("captcha_auto_{}", java_hash_code_take(&text, 18))
            };
            return vec![LogUpdate::new(
                stable_key,
                format!("[КАПЧА AUTO] {text}"),
                60,
                is_err,
                None,
            )];
        }

        if line_trim.contains("[КАПЧА] RJS:") {
            let raw_text = substring_after(line_trim, "[КАПЧА] RJS:").trim();
            let text = remove_parenthesized_tags(raw_text);
            let stable_key = if text.contains("Загрузка") || text.contains("fetch") {
                "captcha_rjs_1".to_string()
            } else if text.contains("PoW") {
                "captcha_rjs_2".to_string()
            } else if text.contains("осматривает") || text.contains("человек") {
                "captcha_rjs_3".to_string()
            } else if text.contains("captchaNotRobot") || text.contains("Отправка") {
                "captcha_rjs_4".to_string()
            } else if text.contains("endSession") {
                "captcha_rjs_5".to_string()
            } else if text.contains("решена") {
                "captcha_rjs_6".to_string()
            } else {
                format!("captcha_rjs_{}", java_hash_code_take(&text, 15))
            };
            return vec![LogUpdate::new(
                stable_key,
                format!("[КАПЧА RJS] {text}"),
                60,
                false,
                None,
            )];
        }

        if line_trim.contains("[КАПЧА] WBV:") {
            let raw_text = substring_after(line_trim, "[КАПЧА] WBV:").trim();
            let text = remove_parenthesized_tags(raw_text);
            let is_err = text.contains("Ошибка");
            let stable_key = if text.contains("Запрос") {
                "captcha_wv_step_2"
            } else if text.contains("Токен") {
                "captcha_wv_step_5"
            } else if is_err {
                "captcha_wv_err"
            } else {
                "captcha_wv_go_other"
            };
            return vec![LogUpdate::new(
                stable_key,
                format!("[КАПЧА WBV] {text}"),
                60,
                is_err,
                None,
            )];
        }

        if line_trim.contains("Решаю VK Smart Captcha") {
            return vec![LogUpdate::new(
                "captcha_start",
                "[КАПЧА] Решение капчи...",
                60,
                false,
                None,
            )];
        }

        if line_trim.contains("Smart Captcha решена") {
            return vec![LogUpdate::new(
                "captcha_done",
                "[КАПЧА] Капча решена ✓",
                60,
                false,
                None,
            )];
        }

        if line_trim.contains("капча не решена") || line_trim.contains("ошибка решения капчи") {
            return vec![LogUpdate::new(
                "captcha_failed",
                "[КАПЧА] Ошибка решения капчи",
                99,
                true,
                Some(LogLevel::Err),
            )];
        }

        if line_trim == "[VK JS] Звонок создан"
            || line_trim.contains("Звонок VK создан")
            || line_trim.contains("Звонок создан")
        {
            return vec![LogUpdate::new(
                "vk_js_call_progress",
                "[VK JS] Звонок VK создан ✓",
                16,
                false,
                Some(LogLevel::Ok),
            )];
        }

        if line_trim.starts_with("[VK JS] Создатель вышел из звонков") {
            return vec![LogUpdate::new(
                "vk_js_creator_left",
                "[VK JS] Владелец вышел из звонка ✓",
                19,
                false,
                Some(LogLevel::Ok),
            )];
        }

        if line_trim.starts_with("[VK JS] Звонки завершены") {
            return vec![LogUpdate::new(
                "vk_js_call_finished",
                "[VK JS] Звонок завершён ✓",
                19,
                false,
                Some(LogLevel::Ok),
            )];
        }

        if line_trim.starts_with("[VK JS] Создатель удерживает звонок") {
            return vec![LogUpdate::new(
                "vk_js_creator_held",
                "[VK JS] Владелец удерживает звонок до отключения",
                18,
                false,
                Some(LogLevel::Ok),
            )];
        }

        if line_trim.starts_with("[VK JS] Креды аккаунта") && line_trim.contains("недоступны") {
            return vec![LogUpdate::new(
                "vk_js_account_credentials_progress",
                "[VK JS] Креды аккаунта недоступны",
                17,
                true,
                Some(LogLevel::Err),
            )];
        }

        if line_trim.starts_with("[VK JS] Креды аккаунта") {
            return vec![LogUpdate::new(
                "vk_js_account_credentials_progress",
                "[VK JS] Креды аккаунта готовы ✓",
                17,
                false,
                Some(LogLevel::Ok),
            )];
        }

        if line_trim.starts_with("[VK JS] Переход на Авто") {
            let text = substring_after(line_trim, "[VK JS]").trim();
            let failed = text.contains("не выполнен");
            let suffix = if failed { "" } else { " ✓" };
            return vec![LogUpdate::new(
                "vk_js_account_fallback_progress",
                format!("[VK JS] {text}{suffix}"),
                17,
                failed,
                Some(if failed { LogLevel::Err } else { LogLevel::Ok }),
            )];
        }

        if line_trim.contains("[WRAP]") {
            return vec![LogUpdate::new(
                "wrap_status",
                "[WRAP] Ключ вычислен ✓",
                10,
                false,
                Some(LogLevel::Ok),
            )];
        }

        if line_trim.starts_with("[ПОТОКИ]") {
            let text = substring_after(line_trim, "[ПОТОКИ]").trim();
            let text_lower = text.to_lowercase();
            let is_repair = text_lower.contains("не пришёл")
                || text_lower.contains("частично")
                || text_lower.contains("перезапуск");

            let (stable_key, priority) = if text_lower.starts_with("отправлен ping") {
                ("stream_probe_ping".to_string(), 56)
            } else if text_lower.starts_with("ack получен") || text_lower.starts_with("все активные") {
                ("stream_probe_ack".to_string(), 57)
            } else if text_lower.contains("channelbind") {
                ("stream_probe_rebind".to_string(), 58)
            } else if text_lower.contains("проверяю") {
                ("stream_probe_all".to_string(), 58)
            } else if text_lower.contains("перезапуск") {
                ("stream_probe_restart".to_string(), 59)
            } else {
                (
                    format!("stream_probe_{}", java_hash_code_take(text, 24)),
                    59,
                )
            };

            return vec![LogUpdate::new(
                stable_key,
                format!("[ПОТОКИ] {text}"),
                priority,
                is_repair,
                Some(if is_repair { LogLevel::Err } else { LogLevel::Log }),
            )];
        }

        if line_trim.contains("[TURN]") {
            let text = substring_after(line_trim, "[TURN]").trim();
            let text_lower = text.to_lowercase();
            if text_lower.contains("используются turn ip") {
                return Vec::new();
            }

            let turn_error = TunnelLogPolicy::is_turn_stream_failure(line_trim) || is_error;
            let maybe_key_prio = if turn_error {
                Some((format!("turn_error_{}", java_hash_code(text)), 99))
            } else if text_lower.contains("createpermission") {
                Some(("turn_permission_status".to_string(), 52))
            } else if text_lower.contains("channelbind") || text_lower.contains("канал") {
                Some(("turn_channel_status".to_string(), 53))
            } else if text_lower.contains("готова к передаче") {
                Some(("turn_ready_status".to_string(), 54))
            } else if text_lower.contains("refresh") {
                Some(("turn_refresh_status".to_string(), 55))
            } else {
                None
            };

            if let Some((stable_key, priority)) = maybe_key_prio {
                return vec![LogUpdate::new(
                    stable_key,
                    format!("[TURN] {text}"),
                    priority,
                    turn_error,
                    Some(if turn_error { LogLevel::Err } else { LogLevel::Log }),
                )];
            } else {
                return Vec::new();
            }
        }

        if line_trim.contains("Рукопожатие...") {
            return vec![LogUpdate::new(
                "peer_handshake",
                "Рукопожатие...",
                65,
                false,
                None,
            )];
        }

        if is_error {
            let error_key = if lower.contains("lookup login.vk.ru") {
                "err_vk_dns".to_string()
            } else if line_trim.contains("connection refused") {
                "err_conn_refused".to_string()
            } else if line_trim.contains("timeout") {
                "err_timeout".to_string()
            } else if line_trim.contains("кредов") {
                "err_creds".to_string()
            } else if line_trim.contains("PEER") {
                "err_peer".to_string()
            } else {
                format!("general_error_{}", java_hash_code_take(line_trim, 15))
            };

            let error_message = if error_key == "err_vk_dns" {
                "[СЕТЬ] DNS до VK недоступен: login.vk.ru"
            } else {
                line_trim
            };

            return vec![LogUpdate::new(
                error_key,
                error_message,
                99,
                true,
                None,
            )];
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_place_update_with_count() {
        let mut book = LogBook::new();
        assert!(book.update("k1", "Initial message", 50, false, None, true));
        assert_eq!(book.entries().len(), 1);
        assert_eq!(book.entries()[0].count, 1);
        assert_eq!(book.entries()[0].message, "Initial message");

        assert!(book.update("k1", "Updated message", 50, false, None, true));
        assert_eq!(book.entries().len(), 1);
        assert_eq!(book.entries()[0].count, 2);
        assert_eq!(book.entries()[0].message, "Updated message");
    }

    #[test]
    fn test_priority_change_resort() {
        let mut book = LogBook::new();
        book.update("k_low", "Low priority", 80, false, None, true);
        book.update("k_high", "High priority", 20, false, None, true);

        assert_eq!(book.entries()[0].key, "k_high");
        assert_eq!(book.entries()[1].key, "k_low");

        // Change priority of k_low to 10 (higher priority than k_high)
        book.update("k_low", "Now highest priority", 10, false, None, true);
        assert_eq!(book.entries().len(), 2);
        assert_eq!(book.entries()[0].key, "k_low");
        assert_eq!(book.entries()[0].count, 2);
        assert_eq!(book.entries()[0].priority, 10);
        assert_eq!(book.entries()[1].key, "k_high");
    }

    #[test]
    fn test_max_entries_trimming() {
        let mut book = LogBook::new();
        for i in 0..110 {
            book.update(
                &format!("key_{i:03}"),
                &format!("Message {i}"),
                i as i32,
                false,
                None,
                true,
            );
        }
        assert_eq!(book.entries().len(), MAX_LOG_ENTRIES);
        // The first 10 lowest priority entries (indexes 0..9 with lowest priority values at the front)
        // are removed from front as more are added beyond MAX_LOG_ENTRIES.
        assert_eq!(book.entries()[0].key, "key_010");
        assert_eq!(book.entries().last().unwrap().key, "key_109");
    }

    #[test]
    fn test_logging_enabled_false_filtering() {
        let mut book = LogBook::new();
        // Disallowed when inactive
        let res = book.update("wrap_status", "[WRAP] Test", 10, false, Some(LogLevel::Ok), false);
        assert!(!res);
        assert!(book.entries().is_empty());

        // Allowed when inactive ("ready" or is_error or level=Err)
        let res2 = book.update("ready", "Ready OK", 10, false, Some(LogLevel::Ok), false);
        assert!(res2);
        assert_eq!(book.entries().len(), 1);

        let res3 = book.update("err_key", "Fatal error", 99, true, None, false);
        assert!(res3);
        assert_eq!(book.entries().len(), 2);
    }

    #[test]
    fn test_classify_wrap_timeout_wait_and_recovered() {
        let mut classifier = CoreLineClassifier::new();
        classifier.set_active_workers(0);

        let updates1 = classifier.classify("Some WRAP_AUTH_TIMEOUT error");
        assert_eq!(updates1.len(), 1);
        assert_eq!(updates1[0].key, "wrap_timeout_wait");
        assert_eq!(
            updates1[0].message,
            "[WRAP] Handshake не подтвердился, проверяем пароль/сеть (1)"
        );
        assert_eq!(updates1[0].priority, 50);
        assert!(updates1[0].is_error);

        let updates2 = classifier.classify("Another WRAP_AUTH_TIMEOUT");
        assert_eq!(updates2[0].key, "wrap_timeout_wait");
        assert_eq!(
            updates2[0].message,
            "[WRAP] Handshake не подтвердился, проверяем пароль/сеть (2)"
        );

        classifier.set_active_workers(3);
        let updates3 = classifier.classify("Third WRAP_AUTH_TIMEOUT");
        assert_eq!(updates3.len(), 1);
        assert_eq!(updates3[0].key, "wrap_timeout_recovered");
        assert_eq!(
            updates3[0].message,
            "[WRAP] Один поток не прошёл handshake, активных=3; повторяем"
        );
        assert_eq!(updates3[0].priority, 50);
        assert!(updates3[0].is_error);
    }

    #[test]
    fn test_classify_fatal_auth_password() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("FATAL_AUTH: неверный пароль к серверу");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].key, "circuit_breaker");
        assert_eq!(
            updates[0].message,
            "[СТОП] 🔒 Неверный пароль подключения. Воркеры остановлены."
        );
        assert_eq!(updates[0].priority, -1);
        assert!(updates[0].is_error);
    }

    #[test]
    fn test_classify_fatal_protocol() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("Произошла ошибка FATAL_PROTOCOL в потоке");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].key, "circuit_breaker");
        assert_eq!(
            updates[0].message,
            "[СТОП] 🔁 Клиент и сервер используют разные версии протокола. Выполните деплой из этой версии приложения. Воркеры остановлены."
        );
        assert_eq!(updates[0].priority, -1);
        assert!(updates[0].is_error);
    }

    #[test]
    fn test_classify_captcha_auto_start() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("[КАПЧА] AUTO: старт (попытка 1)");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].key, "captcha_auto_1");
        assert_eq!(updates[0].message, "[КАПЧА AUTO] старт");
        assert_eq!(updates[0].priority, 60);
        assert!(!updates[0].is_error);
    }

    #[test]
    fn test_classify_vk_js_call_progress() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("[VK JS] Звонок создан");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].key, "vk_js_call_progress");
        assert_eq!(updates[0].message, "[VK JS] Звонок VK создан ✓");
        assert_eq!(updates[0].priority, 16);
        assert!(!updates[0].is_error);
        assert_eq!(updates[0].level, Some(LogLevel::Ok));
    }

    #[test]
    fn test_classify_vk_js_fallback() {
        let mut classifier = CoreLineClassifier::new();
        let ok_updates = classifier.classify("[VK JS] Переход на Авто выполнен");
        assert_eq!(ok_updates.len(), 1);
        assert_eq!(ok_updates[0].key, "vk_js_account_fallback_progress");
        assert_eq!(ok_updates[0].message, "[VK JS] Переход на Авто выполнен ✓");
        assert_eq!(ok_updates[0].priority, 17);
        assert!(!ok_updates[0].is_error);
        assert_eq!(ok_updates[0].level, Some(LogLevel::Ok));

        let fail_updates = classifier.classify("[VK JS] Переход на Авто не выполнен");
        assert_eq!(fail_updates.len(), 1);
        assert_eq!(fail_updates[0].key, "vk_js_account_fallback_progress");
        assert_eq!(fail_updates[0].message, "[VK JS] Переход на Авто не выполнен");
        assert_eq!(fail_updates[0].priority, 17);
        assert!(fail_updates[0].is_error);
        assert_eq!(fail_updates[0].level, Some(LogLevel::Err));
    }

    #[test]
    fn test_classify_streams_ping_and_repair() {
        let mut classifier = CoreLineClassifier::new();
        let ping_updates = classifier.classify("[ПОТОКИ] Отправлен ping 1");
        assert_eq!(ping_updates.len(), 1);
        assert_eq!(ping_updates[0].key, "stream_probe_ping");
        assert_eq!(ping_updates[0].message, "[ПОТОКИ] Отправлен ping 1");
        assert_eq!(ping_updates[0].priority, 56);
        assert!(!ping_updates[0].is_error);
        assert_eq!(ping_updates[0].level, Some(LogLevel::Log));

        let restart_updates = classifier.classify("[ПОТОКИ] Перезапуск потока #2");
        assert_eq!(restart_updates.len(), 1);
        assert_eq!(restart_updates[0].key, "stream_probe_restart");
        assert_eq!(restart_updates[0].message, "[ПОТОКИ] Перезапуск потока #2");
        assert_eq!(restart_updates[0].priority, 59);
        assert!(restart_updates[0].is_error);
        assert_eq!(restart_updates[0].level, Some(LogLevel::Err));
    }

    #[test]
    fn test_classify_turn_status_and_ignore() {
        let mut classifier = CoreLineClassifier::new();
        let perm_updates = classifier.classify("[TURN] CreatePermission OK");
        assert_eq!(perm_updates.len(), 1);
        assert_eq!(perm_updates[0].key, "turn_permission_status");
        assert_eq!(perm_updates[0].message, "[TURN] CreatePermission OK");
        assert_eq!(perm_updates[0].priority, 52);
        assert!(!perm_updates[0].is_error);
        assert_eq!(perm_updates[0].level, Some(LogLevel::Log));

        let ignored_updates = classifier.classify("[TURN] Используются TURN IP 127.0.0.1:3478");
        assert!(ignored_updates.is_empty());
    }

    #[test]
    fn test_classify_internal_recovery_ignored() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("[GETCONF] Sending 72 bytes");
        assert!(updates.is_empty());

        let updates2 = classifier.classify("[TURN][RETRY] Ошибка обновления permission: timeout");
        assert!(updates2.is_empty());
    }

    #[test]
    fn test_classify_general_error_vk_dns() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("lookup login.vk.ru failed: timeout");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].key, "err_vk_dns");
        assert_eq!(
            updates[0].message,
            "[СЕТЬ] DNS до VK недоступен: login.vk.ru"
        );
        assert_eq!(updates[0].priority, 99);
        assert!(updates[0].is_error);
    }

    #[test]
    fn test_timestamp_prefix_stripping() {
        let mut classifier = CoreLineClassifier::new();
        let updates = classifier.classify("2026/11/10 12:34:56.789 [WRAP] Ключ вычислен");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].key, "wrap_status");
        assert_eq!(updates[0].message, "[WRAP] Ключ вычислен ✓");
        assert_eq!(updates[0].priority, 10);
    }
}
