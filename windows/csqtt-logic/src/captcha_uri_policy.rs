//! Port of CaptchaUriPolicy.kt.

/// Policy checking whether a captcha URI points to an allowed domain over HTTPS.
pub struct CaptchaUriPolicy;

impl CaptchaUriPolicy {
    pub const ALLOWED_DOMAINS: [&'static str; 4] = ["vk.com", "vk.ru", "ok.ru", "okcdn.ru"];

    /// Checks whether the given URI string is allowed.
    pub fn is_allowed(value: &str) -> bool {
        is_allowed(value)
    }
}

/// Checks whether the given URI string is allowed under CaptchaUriPolicy.
pub fn is_allowed(value: &str) -> bool {
    let Some(host) = extract_https_host(value) else {
        return false;
    };
    CaptchaUriPolicy::ALLOWED_DOMAINS.iter().any(|&domain| {
        host == domain || (host.ends_with(domain) && host[..host.len() - domain.len()].ends_with('.'))
    })
}

/// Parses an HTTPS URI by hand using std only and extracts the lowercase host (without trailing dots).
fn extract_https_host(value: &str) -> Option<String> {
    let colon_idx = value.find(':')?;
    let scheme = &value[..colon_idx];
    if !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let after_colon = &value[colon_idx + 1..];
    let after_slashes = after_colon.strip_prefix("//")?;

    let authority_end = after_slashes
        .find(|c| c == '/' || c == '?' || c == '#')
        .unwrap_or(after_slashes.len());
    let authority = &after_slashes[..authority_end];
    if authority.is_empty() {
        return None;
    }

    let host_port = match authority.rfind('@') {
        Some(at_idx) => &authority[at_idx + 1..],
        None => authority,
    };
    if host_port.is_empty() {
        return None;
    }

    let raw_host = if host_port.starts_with('[') {
        let close_bracket = host_port.find(']')?;
        let after_bracket = &host_port[close_bracket + 1..];
        if !after_bracket.is_empty() {
            let port_str = after_bracket.strip_prefix(':')?;
            if !port_str.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
        }
        &host_port[..=close_bracket]
    } else {
        match host_port.find(':') {
            Some(colon_pos) => {
                let port_str = &host_port[colon_pos + 1..];
                if port_str.is_empty() || !port_str.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                &host_port[..colon_pos]
            }
            None => host_port,
        }
    };

    if raw_host.is_empty()
        || raw_host
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }

    let cleaned = raw_host.to_ascii_lowercase();
    let trimmed = cleaned.trim_end_matches('.');
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_trusted_https_hosts() {
        assert!(CaptchaUriPolicy::is_allowed("https://vk.com/captcha"));
        assert!(CaptchaUriPolicy::is_allowed("https://id.vk.ru/path?q=1"));
        assert!(CaptchaUriPolicy::is_allowed("https://calls.okcdn.ru/fb.do"));
    }

    #[test]
    fn rejects_untrusted_or_insecure_hosts() {
        assert!(!CaptchaUriPolicy::is_allowed("http://vk.com/captcha"));
        assert!(!CaptchaUriPolicy::is_allowed("https://vk.com.attacker.example/captcha"));
        assert!(!CaptchaUriPolicy::is_allowed("https://evilvk.com/captcha"));
        assert!(!CaptchaUriPolicy::is_allowed("not a uri"));
    }
}
