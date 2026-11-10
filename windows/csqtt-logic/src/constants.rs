//! Port of Constants.kt.

/// General application constants and intent actions.
pub mod general {
    pub const APP_NAME: &str = "CSQTT";
    pub const PACKAGE_VK: &str = "com.vkontakte.android";
    pub const PACKAGE_VK_CALLS: &str = "com.vk.calls";
    pub const CSQTT_EVENT_PREFIX: &str = "__CSQTT_EVENT__|";
    pub const ACTION_RESTART_WHEN_VK_REACHABLE: &str = "com.csqtt.client.RESTART_WHEN_VK_REACHABLE";
    pub const ACTION_VERIFY_VK_REACHABILITY: &str = "com.csqtt.client.VERIFY_VK_REACHABILITY";
    pub const ACTION_RECOVER_VK_CALL: &str = "com.csqtt.client.RECOVER_VK_CALL";
    pub const EXTRA_UNAVAILABLE_VK_HASH: &str = "unavailable_vk_hash";
}

/// Profile index limits.
pub mod profiles {
    pub const MIN_INDEX: i32 = 0;
    pub const MAX_INDEX: i32 = 3;
}

/// Network defaults and host definitions.
pub mod network {
    pub const LOCAL_LISTEN_HOST: &str = "127.0.0.1";
    pub const DEFAULT_LOCAL_PORT: i32 = 0;
    pub const DEFAULT_SERVER_PEER_PORT: i32 = 46000;
    pub const DEFAULT_SSH_PORT: i32 = 22;
    pub const DEFAULT_SERVER_WEB_PORT: i32 = 46002;
}

/// Tunnel binary and runtime defaults.
pub mod tunnel {
    pub const BINARY_NAME: &str = "libclient.so";
    pub const PROCESS_ENV_EVENTS: &str = "CSQTT_EVENTS";

    pub const DEFAULT_FINGERPRINT: &str = "firefox";
    pub const DEFAULT_CLIENT_IDS: &str = "8202606,6287487";
    pub const DEFAULT_OBFS_MODE: &str = "unified";
    pub const DEFAULT_TURN_TRANSPORT: &str = "udp";
    pub const TURN_TRANSPORT_TCP_TLS: &str = "tcp_tls";

    pub const WORKERS_PER_GROUP: i32 = 10;
    pub const GROUPS_PER_VK_HASH: i32 = 1;
    pub const MAX_VK_HASHES: i32 = 7;
    pub const DEFAULT_MAX_WORKERS: i32 = 100;
    pub const MAX_WORKERS: i32 = 100;
}

/// VK authorization mode identifiers.
pub mod vk_auth {
    pub const MODE_CALLS: &str = "vkcalls";
    pub const MODE_CAPTCHA: &str = "legacy";
    pub const MODE_AUTO_JS: &str = "auto_js";
}

/// SRTP modes for media transport.
pub mod srtp_mode {
    pub const WRAP: &str = "wrap-srtp";
    pub const DTLS: &str = "dtls-srtp";
    pub const AUTO: &str = "auto";
    pub const DEFAULT: &str = AUTO;
}

/// Timeouts across various operations in milliseconds.
pub mod timeouts {
    pub const CAPTCHA_AUTO_TIMEOUT_MS: i64 = 10_000;
    pub const CAPTCHA_MANUAL_TIMEOUT_MS: i64 = 60_000;
    pub const CAPTCHA_WV_CREATE_TIMEOUT_MS: i64 = 3_000;

    pub const WATCHDOG_INTERVAL_MS: i64 = 5_000;
    pub const LOG_READER_RESET_INTERVAL_MS: i64 = 60_000;

    pub const VPN_PERMISSION_COOLDOWN_MS: i64 = 300;

    pub const DEPLOY_CMD_TIMEOUT_MS: i64 = 900_000;
    pub const DEPLOY_STALL_TIMEOUT_MS: i64 = 30 * 60 * 1000;

    pub const UPDATE_RATE_LIMIT_COOLDOWN_MS: i64 = 30 * 60 * 1000;
}

/// Error counting thresholds.
pub mod thresholds {
    pub const FLOOD_CONTROL_THRESHOLD: i32 = 5;
    pub const IP_MISMATCH_THRESHOLD: i32 = 5;
    pub const CONNECTION_REFUSED_THRESHOLD: i32 = 400;
    pub const HASH_ERROR_THRESHOLD: i32 = 10;
}

/// Update check intervals and URLs.
pub mod update {
    pub const DEFAULT_CHECK_INTERVAL_HOURS: i32 = 12;
    pub const CHECK_NEVER: i32 = -1;
    pub const ACTION_POSTPONED: &str = "postponed";
    pub const ACTION_UPDATE: &str = "update";

    pub const GITHUB_RELEASES_URL: &str =
        "https://api.github.com/repos/amurcanov/csqtt/releases?per_page=30";
    pub const GITHUB_LATEST_RELEASE_URL: &str =
        "https://api.github.com/repos/amurcanov/csqtt/releases/latest";
    pub const GITHUB_LATEST_RELEASE_WEB_URL: &str =
        "https://github.com/amurcanov/csqtt/releases/latest";
    pub const GITHUB_RELEASE_TAG_URL_PREFIX: &str =
        "https://github.com/amurcanov/csqtt/releases/tag/";
    pub const GITHUB_TAGS_URL: &str =
        "https://api.github.com/repos/amurcanov/csqtt/tags?per_page=100";
    pub const GITHUB_TAG_TREE_URL_PREFIX: &str =
        "https://github.com/amurcanov/csqtt/tree/";
}

/// Widget intent actions.
pub mod widget {
    pub const ACTION_TOGGLE: &str = "com.csqtt.client.ACTION_WIDGET_TOGGLE";
}

/// Captcha detection and viewport sizes.
pub mod captcha {
    pub const ERROR_SLIDER_DETECTED: &str = "slider_detected";
    pub const AUTO_VIEWPORT_WIDTHS: [i32; 7] = [356, 358, 360, 362, 364, 366, 368];
    pub const AUTO_VIEWPORT_HEIGHTS: [i32; 7] = [376, 378, 380, 382, 384, 386, 388];
}

/// VK auto-hash authentication and API constants.
pub mod vk_auto_hash {
    pub const MODE_MANUAL: &str = "manual";
    pub const MODE_AUTO_API: &str = "auto_api";
    pub const MODE_AUTO_JS: &str = "auto_js";
    pub const MAX_OAUTH_HOPS: i32 = 15;

    pub const VK_APP_ID: i32 = 7793118;
    pub const VK_OAUTH_SCOPE: i32 = 1073737727;
    pub const VK_OAUTH_REDIRECT_URI: &str = "https://oauth.vk.ru/blank.html";
    pub const VK_OAUTH_AUTH_URL: &str = concat!(
        "https://oauth.vk.ru/authorize",
        "?client_id=7793118",
        "&scope=1073737727",
        "&redirect_uri=https%3A%2F%2Foauth.vk.ru%2Fblank.html",
        "&display=page",
        "&response_type=token",
        "&revoke=1",
        "&v=5.199"
    );
    pub const VK_LOGIN_URL: &str = "https://vk.ru/";
    pub const API_METHOD_BASE_URL: &str = "https://api.vk.ru/method/";
    pub const API_VERSION: &str = "5.199";

    pub const AUTH_TIMEOUT_MS: i64 = 300_000;
    pub const AUTH_MONITOR_INTERVAL_MS: i64 = 200;
    pub const AUTH_SILENT_RETRY_DELAY_MS: i64 = 1_500;

    pub const AUTO_CALL_SMALL_DELAY_MS: i64 = 380;
    pub const AUTO_CALL_LARGE_DELAY_MS: i64 = 380;
    pub const AUTO_CALL_MAX_ATTEMPTS: i32 = 3;
    pub const FINISH_CALL_STEP_MS: i64 = 150;
    pub const FINISH_CALLS_TIMEOUT_MS: i64 = 8_000;
}

/// VPN MTU and localized messages.
pub mod vpn {
    pub const DEFAULT_MTU: i32 = 1500;
    pub const EMPTY_WHITELIST_MESSAGE: &str =
        "В режиме БС выберите хотя бы одно приложение";
}

/// UI themes and color palettes.
pub mod theme {
    pub const MODE_SYSTEM: &str = "system";
    pub const MODE_DARK: &str = "dark";
    pub const MODE_LIGHT: &str = "light";
    pub const PALETTE_INDIGO: &str = "indigo";
    pub const PALETTE_ESPRESSO: &str = "espresso";
    pub const PALETTE_FOREST: &str = "forest";
}

/// External project and contact links.
pub mod links {
    pub const RELEASES: &str = "https://github.com/amurcanov/csqtt/releases";
    pub const ISSUES: &str = "https://github.com/amurcanov/csqtt/issues/new";
    pub const DEVELOPER_PROFILE: &str = "https://github.com/amurcanov";
    pub const REPOSITORY: &str = "https://github.com/amurcanov/csqtt";
    pub const TELEGRAM: &str = "https://t.me/amurcanov_telegram";
    pub const TELEGRAM_HANDLE: &str = "t.me/amurcanov_telegram";
}

/// Regular expression pattern strings.
pub mod patterns {
    pub const VERSION_NUMBER: &str = r"\d+(?:\.\d+)*";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_values() {
        assert_eq!(general::APP_NAME, "CSQTT");
        assert_eq!(general::CSQTT_EVENT_PREFIX, "__CSQTT_EVENT__|");
        assert_eq!(network::DEFAULT_SERVER_PEER_PORT, 46000);
        assert_eq!(tunnel::WORKERS_PER_GROUP, 10);
        assert_eq!(tunnel::MAX_WORKERS, 100);
        assert_eq!(timeouts::CAPTCHA_AUTO_TIMEOUT_MS, 10000);
        assert_eq!(timeouts::DEPLOY_STALL_TIMEOUT_MS, 1800000);
        assert_eq!(timeouts::UPDATE_RATE_LIMIT_COOLDOWN_MS, 1800000);
        assert_eq!(thresholds::FLOOD_CONTROL_THRESHOLD, 5);
        assert_eq!(update::DEFAULT_CHECK_INTERVAL_HOURS, 12);
        assert_eq!(captcha::AUTO_VIEWPORT_WIDTHS.len(), 7);
        assert_eq!(captcha::AUTO_VIEWPORT_HEIGHTS.len(), 7);
        assert_eq!(vk_auto_hash::VK_APP_ID, 7793118);
        assert_eq!(vpn::DEFAULT_MTU, 1500);
        assert_eq!(theme::MODE_SYSTEM, "system");
        assert_eq!(patterns::VERSION_NUMBER, r"\d+(?:\.\d+)*");
    }

    #[test]
    fn test_vk_oauth_auth_url_matches() {
        let expected = "https://oauth.vk.ru/authorize?client_id=7793118&scope=1073737727&redirect_uri=https%3A%2F%2Foauth.vk.ru%2Fblank.html&display=page&response_type=token&revoke=1&v=5.199";
        assert_eq!(vk_auto_hash::VK_OAUTH_AUTH_URL, expected);
    }
}
