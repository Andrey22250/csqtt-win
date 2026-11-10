//! VK login in WebView2 and access-token extraction (port of VkAuthWebViewManager.kt,
//! VkTokenScraper.kt, ui/dialogs/VkAuthDialog.kt logic).

#![allow(dead_code)]

use std::time::{Duration, Instant};

use crate::webview::{self, Action, Options, WebCtl, WebEvent, WebViewError};

// Constants copied verbatim from Kotlin (CsqttConstants.VkAutoHash and VkTokenScraper.kt).
pub const VK_APP_ID: i32 = 7793118;
pub const VK_CLIENT_ID: &str = "7793118";
pub const VK_OAUTH_SCOPE: i32 = 1073737727;
pub const VK_OAUTH_REDIRECT_URI: &str = "https://oauth.vk.ru/blank.html";
pub const VK_LOGIN_URL: &str = "https://vk.ru/";

/// OAuth authorization URL for user consent / fallback / repeat authorization.
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

/// Scraper authorization endpoint URL (from VkTokenScraper.kt line 11).
///
/// In the Android app, `VkTokenScraper` performed background HTTP requests with
/// `HttpURLConnection` using cookies extracted from the WebView. Because `csqtt-cli`
/// has no HTTP client dependency (such as reqwest), we execute the `VkTokenScraper`
/// request directly inside the WebView2 instance. Navigating the WebView to this
/// endpoint automatically reuses the session cookies stored in the "vk" profile,
/// and redirects are captured via `NavigationStarting` and `NavigationCompleted`.
pub const VK_TOKEN_SCRAPER_URL: &str = concat!(
    "https://oauth.vk.com/authorize",
    "?client_id=7793118",
    "&display=mobile",
    "&redirect_uri=https%3A%2F%2Foauth.vk.ru%2Fblank.html",
    "&response_type=token",
    "&scope=1073737727",
    "&v=5.199",
    "&revoke=1"
);

/// Domains to inspect for login session cookies (`remixsid`).
pub const VK_COOKIE_DOMAINS: [&str; 8] = [
    "https://vk.com/",
    "https://vk.ru/",
    "https://id.vk.ru/",
    "https://id.vk.com/",
    "https://login.vk.com/",
    "https://login.vk.ru/",
    "https://m.vk.com/",
    "https://m.vk.ru/",
];

pub const AUTH_TIMEOUT: Duration = Duration::from_secs(600); // 10 minutes
pub const AUTH_TICK_INTERVAL: Duration = Duration::from_secs(1); // 1 second
pub const AUTH_SILENT_RETRY_DELAY_MS: u64 = 1_500; // 1.5 seconds

const SCRIPT_ID_CHECK_FEED: u32 = 1001;
const CHECK_FEED_JS: &str = "(function() { return !!(document.body && (document.body.innerText.includes('Мессенджер') || document.body.innerText.includes('Лента'))); })();";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct VkAuthPayload {
    pub access_token: String,
    pub user_id: String,
    pub expires_in: i64,
    /// Any other field Android's VkAuthPayload carries, as (name, value).
    pub extra: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkAuthPhase {
    Login,
    Token,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlClassification {
    /// Target URL has access_token parameter in fragment or query.
    AccessToken(String),
    /// Host is oauth.vk.com/ru or vkhost.github.io and has payload= in fragment.
    SilentPayload(String),
    /// Target URL has error parameter in fragment or query.
    OAuthError {
        error: String,
        description: Option<String>,
    },
    /// Redirect to main feed (/feed).
    LoginFeed,
    /// Other unclassified URL.
    Other,
}

/// Percent-decodes a URL-encoded string, decoding '+' as space.
pub fn url_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let raw = s.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'%' if i + 2 < raw.len() => {
                if let (Some(h1), Some(h2)) = (hex_digit(raw[i + 1]), hex_digit(raw[i + 2])) {
                    bytes.push((h1 << 4) | h2);
                    i += 3;
                } else {
                    bytes.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                bytes.push(b' ');
                i += 1;
            }
            b => {
                bytes.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Extracts the fragment (#...) from a URL without the leading '#'.
pub fn extract_fragment(url: &str) -> Option<&str> {
    url.find('#').map(|idx| &url[idx + 1..])
}

/// Extracts query parameters (?...) from a URL without the leading '?' and before any '#'.
pub fn extract_query(url: &str) -> Option<&str> {
    let q_start = url.find('?')?;
    let query_part = &url[q_start + 1..];
    if let Some(hash_pos) = query_part.find('#') {
        Some(&query_part[..hash_pos])
    } else {
        Some(query_part)
    }
}

/// Extracts the host from a URL.
pub fn extract_host(url: &str) -> Option<&str> {
    let without_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };
    let end = without_scheme
        .find(|c| c == '/' || c == '?' || c == '#' || c == ':')
        .unwrap_or(without_scheme.len());
    let host = &without_scheme[..end];
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

/// Extracts the path from a URL.
pub fn extract_path(url: &str) -> &str {
    let without_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };
    if let Some(slash_idx) = without_scheme.find('/') {
        let after_slash = &without_scheme[slash_idx..];
        let end = after_slash
            .find(|c| c == '?' || c == '#')
            .unwrap_or(after_slash.len());
        &after_slash[..end]
    } else {
        "/"
    }
}

/// Parses key-value pairs separated by '&' and '=' from a fragment or query string.
pub fn parse_fragment_params(fragment: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for part in fragment.split('&') {
        if part.is_empty() {
            continue;
        }
        if let Some(eq) = part.find('=') {
            if eq > 0 {
                let key = url_decode(&part[..eq]);
                let val = url_decode(&part[eq + 1..]);
                result.push((key, val));
            }
        }
    }
    result
}

/// Builds a VkAuthPayload from parsed key-value parameters.
/// Fields other than token, user_id, and expires_in are preserved in `extra`.
pub fn build_payload(params: &[(String, String)]) -> Result<VkAuthPayload, String> {
    let mut token = None;
    let mut user_id = String::new();
    let mut expires_in = 0i64;
    let mut extra = Vec::new();

    for (k, v) in params {
        match k.as_str() {
            "access_token" => token = Some(v.clone()),
            "user_id" => user_id = v.clone(),
            "expires_in" => expires_in = v.parse::<i64>().unwrap_or(0),
            _ => extra.push((k.clone(), v.clone())),
        }
    }

    let token = match token {
        Some(t) if !t.is_empty() => t,
        _ => return Err("Не удалось извлечь access_token из URL".to_string()),
    };

    Ok(VkAuthPayload {
        access_token: token,
        user_id,
        expires_in,
        extra,
    })
}

/// Classifies a URL encountered during navigation or monitor ticks.
pub fn classify_url(url: &str) -> UrlClassification {
    let fragment = extract_fragment(url).unwrap_or("");
    let query = extract_query(url).unwrap_or("");

    // 1. Check access token in fragment or query
    if fragment.contains("access_token=") {
        return UrlClassification::AccessToken(fragment.to_string());
    }
    if query.contains("access_token=") {
        return UrlClassification::AccessToken(query.to_string());
    }

    // 2. Check silent payload in fragment
    if fragment.starts_with("payload=") || fragment.contains("&payload=") {
        let host = extract_host(url).unwrap_or("");
        if host == "oauth.vk.com" || host == "oauth.vk.ru" || host == "vkhost.github.io" {
            let payload_str = if let Some(p) = fragment.strip_prefix("payload=") {
                p
            } else if let Some(idx) = fragment.find("&payload=") {
                &fragment[idx + 9..]
            } else {
                fragment
            };
            let payload_json_enc = payload_str.split('&').next().unwrap_or(payload_str);
            return UrlClassification::SilentPayload(payload_json_enc.to_string());
        }
    }

    // 3. Check OAuth error in fragment or query
    if fragment.contains("error=") || query.contains("error=") {
        let target = if fragment.contains("error=") { fragment } else { query };
        let params = parse_fragment_params(target);
        let error = params
            .iter()
            .find(|(k, _)| k == "error")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "unknown_error".to_string());
        let description = params
            .iter()
            .find(|(k, _)| k == "error_description")
            .map(|(_, v)| v.clone());
        return UrlClassification::OAuthError { error, description };
    }

    // 4. Check login feed redirect
    let path = extract_path(url);
    if path.starts_with("/feed") {
        return UrlClassification::LoginFeed;
    }

    UrlClassification::Other
}

/// Checks if any of the VK authentication domains have the `remixsid` cookie set.
fn is_vk_logged_in(ctl: &mut dyn WebCtl) -> bool {
    VK_COOKIE_DOMAINS.iter().any(|domain| {
        let cookies = ctl.cookies_for(domain);
        cookies.contains("remixsid")
    })
}

fn switch_to_token_phase(
    phase: &mut VkAuthPhase,
    ctl: &mut dyn WebCtl,
    on_step: &mut dyn FnMut(&str),
    last_url: &mut Option<String>,
) {
    if *phase == VkAuthPhase::Token {
        return;
    }
    *phase = VkAuthPhase::Token;
    on_step("Получение токена");
    ctl.set_title("Получение токена");
    *last_url = None;
    ctl.navigate(VK_TOKEN_SCRAPER_URL);
}

fn repeat_authorization(
    ctl: &mut dyn WebCtl,
    on_step: &mut dyn FnMut(&str),
    last_url: &mut Option<String>,
) {
    on_step("Повторная авторизация VK...");
    *last_url = None;
    ctl.navigate(VK_OAUTH_AUTH_URL);
}

fn handle_navigation_url(
    url: &str,
    phase: &mut VkAuthPhase,
    ctl: &mut dyn WebCtl,
    on_step: &mut dyn FnMut(&str),
    outcome: &mut Option<Result<VkAuthPayload, String>>,
    non_permanent_retried: &mut bool,
    silent_retried: &mut bool,
    silent_retry_at: &mut Option<Instant>,
    last_url: &mut Option<String>,
) -> Action {
    *last_url = Some(url.to_string());
    match classify_url(url) {
        UrlClassification::AccessToken(fragment_or_query) => {
            let params = parse_fragment_params(&fragment_or_query);
            match build_payload(&params) {
                Ok(payload) => {
                    if payload.expires_in == 0 {
                        *outcome = Some(Ok(payload));
                        Action::Finish("ok".to_string())
                    } else if !*non_permanent_retried {
                        *non_permanent_retried = true;
                        repeat_authorization(ctl, on_step, last_url);
                        Action::Continue
                    } else {
                        *outcome = Some(Err(
                            "VK выдал токен с ограниченным сроком действия. Попробуйте еще раз.".to_string(),
                        ));
                        Action::Finish("error".to_string())
                    }
                }
                Err(e) => {
                    *outcome = Some(Err(e));
                    Action::Finish("error".to_string())
                }
            }
        }
        UrlClassification::SilentPayload(payload_str) => {
            if !*silent_retried {
                let decoded = url_decode(&payload_str);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&decoded) {
                    let is_silent = val.get("type").and_then(|t| t.as_str()) == Some("silent_token");
                    let has_token = val
                        .get("token")
                        .and_then(|t| t.as_str())
                        .map(|t| !t.trim().is_empty())
                        .unwrap_or(false);
                    if is_silent && has_token {
                        *silent_retried = true;
                        *silent_retry_at = Some(
                            Instant::now() + Duration::from_millis(AUTH_SILENT_RETRY_DELAY_MS),
                        );
                    }
                }
            }
            Action::Continue
        }
        UrlClassification::OAuthError { error, description } => {
            let msg = description.unwrap_or(error);
            *outcome = Some(Err(msg));
            Action::Finish("error".to_string())
        }
        UrlClassification::LoginFeed => {
            if *phase == VkAuthPhase::Login {
                switch_to_token_phase(phase, ctl, on_step, last_url);
            }
            Action::Continue
        }
        UrlClassification::Other => {
            if *phase == VkAuthPhase::Login && is_vk_logged_in(ctl) {
                switch_to_token_phase(phase, ctl, on_step, last_url);
            }
            Action::Continue
        }
    }
}

/// Open the VK login window, let the user log in, obtain the token like Android does.
/// Blocking; Err(text in Russian) on cancel / error / timeout.
pub fn login(on_step: &mut dyn FnMut(&str)) -> Result<VkAuthPayload, String> {
    let options = Options {
        title: "Вход в VK".to_string(),
        url: VK_LOGIN_URL.to_string(),
        width: 480,
        height: 720,
        visible: true,
        // The WebView's own user agent. Android uses its real WebView UA here; the short
        // "Android 13; Mobile" string is only its fallback for a background request. Set on
        // the window it contradicted the Client Hints (Edge on Windows) and VK's anti-fraud
        // refused password logins with "слишком много попыток" on any account.
        user_agent: None,
        init_scripts: Vec::new(),
        intercept_filters: Vec::new(),
        profile: "vk".to_string(),
        timeout: AUTH_TIMEOUT,
        tick: Some(AUTH_TICK_INTERVAL),
    };

    let mut phase = VkAuthPhase::Login;
    let mut outcome: Option<Result<VkAuthPayload, String>> = None;
    let mut non_permanent_retried = false;
    let mut silent_retried = false;
    let mut silent_retry_at: Option<Instant> = None;
    let mut last_url: Option<String> = None;
    // A failed main-frame navigation is often just a redirect cancelled by the next one;
    // treat it as a network error only if nothing else starts loading for a while.
    let mut failed_at: Option<Instant> = None;

    let mut handler = |event: WebEvent, ctl: &mut dyn WebCtl| -> Action {
        match event {
            WebEvent::Ready => {
                if is_vk_logged_in(ctl) {
                    switch_to_token_phase(&mut phase, ctl, on_step, &mut last_url);
                } else {
                    on_step("Войдите любым способом, затем токен будет получен автоматически");
                }
                Action::Continue
            }
            WebEvent::NavigationStarting { url } => {
                failed_at = None;
                handle_navigation_url(
                    &url,
                    &mut phase,
                    ctl,
                    on_step,
                    &mut outcome,
                    &mut non_permanent_retried,
                    &mut silent_retried,
                    &mut silent_retry_at,
                    &mut last_url,
                )
            }
            WebEvent::NavigationCompleted { url, success, http_status: _ } => {
                if success {
                    failed_at = None;
                    handle_navigation_url(
                        &url,
                        &mut phase,
                        ctl,
                        on_step,
                        &mut outcome,
                        &mut non_permanent_retried,
                        &mut silent_retried,
                        &mut silent_retry_at,
                        &mut last_url,
                    )
                } else {
                    let action = handle_navigation_url(
                        &url,
                        &mut phase,
                        ctl,
                        on_step,
                        &mut outcome,
                        &mut non_permanent_retried,
                        &mut silent_retried,
                        &mut silent_retry_at,
                        &mut last_url,
                    );
                    if action != Action::Continue {
                        action
                    } else if !url.contains("blank.html") && outcome.is_none() {
                        failed_at.get_or_insert_with(Instant::now);
                        Action::Continue
                    } else {
                        Action::Continue
                    }
                }
            }
            WebEvent::ScriptResult { id, json } => {
                if id == SCRIPT_ID_CHECK_FEED && phase == VkAuthPhase::Login {
                    if json == "true" || json == "\"true\"" {
                        switch_to_token_phase(&mut phase, ctl, on_step, &mut last_url);
                    }
                }
                Action::Continue
            }
            WebEvent::Tick => {
                if let Some(at) = failed_at {
                    if at.elapsed() >= Duration::from_secs(6) && outcome.is_none() {
                        outcome = Some(Err(
                            "Не удалось открыть страницу авторизации VK — проверьте интернет".to_string(),
                        ));
                        return Action::Finish("error".to_string());
                    }
                }
                // Check if delayed silent retry timer expired
                if let Some(retry_at) = silent_retry_at {
                    if Instant::now() >= retry_at {
                        silent_retry_at = None;
                        repeat_authorization(ctl, on_step, &mut last_url);
                    }
                }

                // Check current URL for redirects / fragments
                let current = ctl.current_url();
                if !current.is_empty() && last_url.as_deref() != Some(&current) {
                    let action = handle_navigation_url(
                        &current,
                        &mut phase,
                        ctl,
                        on_step,
                        &mut outcome,
                        &mut non_permanent_retried,
                        &mut silent_retried,
                        &mut silent_retry_at,
                        &mut last_url,
                    );
                    if action != Action::Continue {
                        return action;
                    }
                }

                // Periodic monitor checks in Login phase
                if phase == VkAuthPhase::Login {
                    if is_vk_logged_in(ctl) {
                        switch_to_token_phase(&mut phase, ctl, on_step, &mut last_url);
                    } else {
                        let path = extract_path(&current);
                        if path.starts_with("/feed") {
                            switch_to_token_phase(&mut phase, ctl, on_step, &mut last_url);
                        } else {
                            ctl.execute_script(SCRIPT_ID_CHECK_FEED, CHECK_FEED_JS);
                        }
                    }
                }

                Action::Continue
            }
            WebEvent::Closed => {
                if outcome.is_none() {
                    outcome = Some(Err("Авторизация отменена пользователем".to_string()));
                }
                Action::Finish("closed".to_string())
            }
            WebEvent::ResourceRequested { .. } | WebEvent::WebMessage { .. } => {
                Action::Continue
            }
        }
    };

    match webview::run(options, &mut handler) {
        Ok(_) => outcome.unwrap_or_else(|| Err("Авторизация не завершена".to_string())),
        Err(WebViewError::Closed) => {
            outcome.unwrap_or_else(|| Err("Авторизация отменена пользователем".to_string()))
        }
        Err(WebViewError::Timeout) => {
            Err("Тайм-аут ожидания access_token".to_string())
        }
        Err(WebViewError::Runtime(m)) => {
            Err(format!("Ошибка среды WebView2: {m}"))
        }
        Err(WebViewError::Other(m)) => {
            outcome.unwrap_or_else(|| Err(m))
        }
    }
}

/// Forget the VK session (cookies of the VK login profile), like clearVkSessionCookies().
pub fn clear_session() -> Result<(), String> {
    let options = Options {
        title: String::new(),
        url: "about:blank".to_string(),
        width: 480,
        height: 720,
        visible: false,
        user_agent: None,
        init_scripts: Vec::new(),
        intercept_filters: Vec::new(),
        profile: "vk".to_string(),
        timeout: Duration::from_secs(30),
        tick: None,
    };

    let mut cleared = false;
    let mut handler = |event: WebEvent, ctl: &mut dyn WebCtl| -> Action {
        match event {
            WebEvent::Ready => {
                ctl.clear_cookies();
                cleared = true;
                Action::Finish("ok".to_string())
            }
            WebEvent::Closed => {
                Action::Finish("closed".to_string())
            }
            _ => Action::Continue,
        }
    };

    match webview::run(options, &mut handler) {
        Ok(_) if cleared => Ok(()),
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Не удалось очистить сессию VK: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_decode_ascii_and_spaces() {
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("foo%20bar%21"), "foo bar!");
        assert_eq!(url_decode("a%2Fb%3Fc%26d%3De"), "a/b?c&d=e");
    }

    #[test]
    fn test_url_decode_utf8_russian() {
        // UTF-8 for "Привет мир"
        assert_eq!(
            url_decode("%D0%9F%D1%80%D0%B8%D0%B2%D0%B5%D1%82+%D0%BC%D0%B8%D1%80"),
            "Привет мир"
        );
        // UTF-8 for "Ошибка VK"
        assert_eq!(
            url_decode("%D0%9E%D1%88%D0%B8%D0%B1%D0%BA%D0%B0+VK"),
            "Ошибка VK"
        );
    }

    #[test]
    fn test_url_decode_malformed_percent() {
        assert_eq!(url_decode("abc%2"), "abc%2");
        assert_eq!(url_decode("abc%zz"), "abc%zz");
        assert_eq!(url_decode("%"), "%");
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("https://oauth.vk.ru/blank.html#access_token=123"),
            Some("oauth.vk.ru")
        );
        assert_eq!(
            extract_host("https://oauth.vk.com:443/authorize?client_id=123"),
            Some("oauth.vk.com")
        );
        assert_eq!(
            extract_host("http://vkhost.github.io/#payload=test"),
            Some("vkhost.github.io")
        );
        assert_eq!(extract_host("vk.com/feed"), Some("vk.com"));
        assert_eq!(extract_host(""), None);
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("https://vk.ru/feed"), "/feed");
        assert_eq!(extract_path("https://vk.ru/feed?act=news"), "/feed");
        assert_eq!(
            extract_path("https://oauth.vk.ru/blank.html#access_token=123"),
            "/blank.html"
        );
        assert_eq!(extract_path("https://vk.ru"), "/");
        assert_eq!(extract_path("https://vk.ru/"), "/");
    }

    #[test]
    fn test_extract_fragment_and_query() {
        let url = "https://oauth.vk.ru/blank.html?client_id=7793118#access_token=token123&expires_in=0";
        assert_eq!(
            extract_fragment(url),
            Some("access_token=token123&expires_in=0")
        );
        assert_eq!(extract_query(url), Some("client_id=7793118"));

        let url_no_fragment = "https://oauth.vk.ru/authorize?client_id=7793118&v=5.199";
        assert_eq!(extract_fragment(url_no_fragment), None);
        assert_eq!(extract_query(url_no_fragment), Some("client_id=7793118&v=5.199"));

        let url_no_query = "https://oauth.vk.ru/blank.html#payload=test";
        assert_eq!(extract_fragment(url_no_query), Some("payload=test"));
        assert_eq!(extract_query(url_no_query), None);
    }

    #[test]
    fn test_parse_fragment_params_basic() {
        let frag = "access_token=vk1.a.xyz&expires_in=0&user_id=123456";
        let params = parse_fragment_params(frag);
        assert_eq!(
            params,
            vec![
                ("access_token".to_string(), "vk1.a.xyz".to_string()),
                ("expires_in".to_string(), "0".to_string()),
                ("user_id".to_string(), "123456".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_fragment_params_url_decoding() {
        let frag = "access_token=vk1.a.xyz%2Btest&email=user%40example.com&extra=foo%20bar";
        let params = parse_fragment_params(frag);
        assert_eq!(
            params,
            vec![
                ("access_token".to_string(), "vk1.a.xyz+test".to_string()),
                ("email".to_string(), "user@example.com".to_string()),
                ("extra".to_string(), "foo bar".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_fragment_params_special_cases() {
        assert!(parse_fragment_params("").is_empty());
        assert!(parse_fragment_params("&&&").is_empty());
        assert!(parse_fragment_params("no_equal_sign&another").is_empty());
        assert_eq!(
            parse_fragment_params("empty_val=&another=val"),
            vec![
                ("empty_val".to_string(), "".to_string()),
                ("another".to_string(), "val".to_string())
            ]
        );
        // Preserves subsequent '=' inside value
        assert_eq!(
            parse_fragment_params("data=a=b=c&next=1"),
            vec![
                ("data".to_string(), "a=b=c".to_string()),
                ("next".to_string(), "1".to_string())
            ]
        );
    }

    #[test]
    fn test_build_payload_permanent_token() {
        let params = vec![
            ("access_token".to_string(), "vk1.a.my_secret_token".to_string()),
            ("user_id".to_string(), "12345678".to_string()),
            ("expires_in".to_string(), "0".to_string()),
        ];
        let payload = build_payload(&params).expect("payload build should succeed");
        assert_eq!(payload.access_token, "vk1.a.my_secret_token");
        assert_eq!(payload.user_id, "12345678");
        assert_eq!(payload.expires_in, 0);
        assert!(payload.extra.is_empty());
    }

    #[test]
    fn test_build_payload_temporary_token() {
        let params = vec![
            ("access_token".to_string(), "vk1.a.temp_token".to_string()),
            ("user_id".to_string(), "87654321".to_string()),
            ("expires_in".to_string(), "86400".to_string()),
        ];
        let payload = build_payload(&params).expect("payload build should succeed");
        assert_eq!(payload.access_token, "vk1.a.temp_token");
        assert_eq!(payload.user_id, "87654321");
        assert_eq!(payload.expires_in, 86400);
    }

    #[test]
    fn test_build_payload_missing_token() {
        let params = vec![
            ("user_id".to_string(), "123".to_string()),
            ("expires_in".to_string(), "0".to_string()),
        ];
        let err = build_payload(&params).expect_err("should fail when access_token missing");
        assert_eq!(err, "Не удалось извлечь access_token из URL");

        let empty_token = vec![
            ("access_token".to_string(), "".to_string()),
            ("user_id".to_string(), "123".to_string()),
        ];
        let err2 = build_payload(&empty_token).expect_err("should fail when access_token empty");
        assert_eq!(err2, "Не удалось извлечь access_token из URL");
    }

    #[test]
    fn test_build_payload_defaults() {
        let params = vec![("access_token".to_string(), "token_only".to_string())];
        let payload = build_payload(&params).expect("should succeed with defaults");
        assert_eq!(payload.access_token, "token_only");
        assert_eq!(payload.user_id, "");
        assert_eq!(payload.expires_in, 0);
        assert!(payload.extra.is_empty());
    }

    #[test]
    fn test_build_payload_preserves_extra_fields() {
        let params = vec![
            ("access_token".to_string(), "tok".to_string()),
            ("user_id".to_string(), "1".to_string()),
            ("expires_in".to_string(), "0".to_string()),
            ("email".to_string(), "test@example.com".to_string()),
            ("state".to_string(), "xyz".to_string()),
            ("scope".to_string(), "1073737727".to_string()),
        ];
        let payload = build_payload(&params).expect("payload should build");
        assert_eq!(payload.access_token, "tok");
        assert_eq!(payload.user_id, "1");
        assert_eq!(payload.expires_in, 0);
        assert_eq!(
            payload.extra,
            vec![
                ("email".to_string(), "test@example.com".to_string()),
                ("state".to_string(), "xyz".to_string()),
                ("scope".to_string(), "1073737727".to_string()),
            ]
        );
    }

    #[test]
    fn test_classify_url_access_token_in_fragment() {
        let url = "https://oauth.vk.ru/blank.html#access_token=vk1.a.123&user_id=1&expires_in=0";
        match classify_url(url) {
            UrlClassification::AccessToken(fragment) => {
                assert!(fragment.contains("access_token=vk1.a.123"));
                assert!(fragment.contains("user_id=1"));
            }
            other => panic!("unexpected classification: {other:?}"),
        }
    }

    #[test]
    fn test_classify_url_access_token_in_query() {
        let url = "https://oauth.vk.ru/blank.html?access_token=vk1.a.456&user_id=2&expires_in=0";
        match classify_url(url) {
            UrlClassification::AccessToken(query) => {
                assert!(query.contains("access_token=vk1.a.456"));
            }
            other => panic!("unexpected classification: {other:?}"),
        }
    }

    #[test]
    fn test_classify_url_silent_payload() {
        let url = "https://oauth.vk.com/blank.html#payload=%7B%22type%22%3A%22silent_token%22%2C%22token%22%3A%22tok123%22%7D";
        match classify_url(url) {
            UrlClassification::SilentPayload(payload_str) => {
                assert_eq!(
                    payload_str,
                    "%7B%22type%22%3A%22silent_token%22%2C%22token%22%3A%22tok123%22%7D"
                );
                let decoded = url_decode(&payload_str);
                let val: serde_json::Value = serde_json::from_str(&decoded).unwrap();
                assert_eq!(val["type"], "silent_token");
                assert_eq!(val["token"], "tok123");
            }
            other => panic!("unexpected classification: {other:?}"),
        }

        // Works on oauth.vk.ru and vkhost.github.io
        let url_ru = "https://oauth.vk.ru/blank.html#payload=test_payload";
        assert_eq!(
            classify_url(url_ru),
            UrlClassification::SilentPayload("test_payload".to_string())
        );

        let url_gh = "https://vkhost.github.io/#payload=gh_payload";
        assert_eq!(
            classify_url(url_gh),
            UrlClassification::SilentPayload("gh_payload".to_string())
        );
    }

    #[test]
    fn test_classify_url_silent_payload_wrong_host() {
        let url = "https://attacker.example.com/blank.html#payload=evil";
        assert_eq!(classify_url(url), UrlClassification::Other);
    }

    #[test]
    fn test_classify_url_oauth_error() {
        let url = "https://oauth.vk.ru/blank.html#error=access_denied&error_description=%D0%94%D0%BE%D1%81%D1%82%D1%83%D0%BF+%D0%B7%D0%B0%D0%BF%D1%80%D0%B5%D1%89%D1%91%D0%BD";
        match classify_url(url) {
            UrlClassification::OAuthError { error, description } => {
                assert_eq!(error, "access_denied");
                assert_eq!(description, Some("Доступ запрещён".to_string()));
            }
            other => panic!("unexpected classification: {other:?}"),
        }
    }

    #[test]
    fn test_classify_url_oauth_error_without_description() {
        let url = "https://oauth.vk.ru/blank.html?error=invalid_request";
        match classify_url(url) {
            UrlClassification::OAuthError { error, description } => {
                assert_eq!(error, "invalid_request");
                assert_eq!(description, None);
            }
            other => panic!("unexpected classification: {other:?}"),
        }
    }

    #[test]
    fn test_classify_url_login_feed() {
        assert_eq!(classify_url("https://vk.ru/feed"), UrlClassification::LoginFeed);
        assert_eq!(
            classify_url("https://vk.com/feed?section=news"),
            UrlClassification::LoginFeed
        );
    }

    #[test]
    fn test_classify_url_other() {
        assert_eq!(classify_url("https://vk.ru/"), UrlClassification::Other);
        assert_eq!(classify_url("https://vk.ru/login"), UrlClassification::Other);
        assert_eq!(classify_url("https://id.vk.com/auth"), UrlClassification::Other);
    }
}
