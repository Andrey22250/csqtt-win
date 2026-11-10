//! Port of UiUtils.kt (CsqttLink parser, formatter, encryption/decryption).

use std::collections::{BTreeMap, HashSet};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{
    alphabet,
    engine::{
        general_purpose::{GeneralPurpose, GeneralPurposeConfig, URL_SAFE_NO_PAD},
        DecodePaddingMode,
    },
    Engine as _,
};
use hmac::{Hmac, Mac};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Decoded parameters of a csqtt connection link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsqttLink {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub hashes: Vec<String>,
    #[serde(default)]
    pub vk_access_token: String,
}

impl Default for CsqttLink {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 0,
            password: String::new(),
            hashes: Vec::new(),
            vk_access_token: String::new(),
        }
    }
}

impl CsqttLink {
    /// Creates a new CsqttLink.
    pub fn new(
        host: impl Into<String>,
        port: u16,
        password: impl Into<String>,
        hashes: Vec<String>,
        vk_access_token: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            password: password.into(),
            hashes,
            vk_access_token: vk_access_token.into(),
        }
    }

    /// Formats the peer address as host:port, wrapping IPv6 addresses in brackets.
    pub fn peer_address(&self) -> String {
        peer_address(self)
    }
}

/// Formats the peer address as host:port, wrapping IPv6 addresses in brackets.
pub fn peer_address(link: &CsqttLink) -> String {
    let formatted_host = if link.host.contains(':') && !link.host.starts_with('[') {
        format!("[{}]", link.host)
    } else {
        link.host.clone()
    };
    format!("{formatted_host}:{}", link.port)
}

/// Seed used to derive the link payload encryption key (matches Kotlin).
pub const CSQTT_CIPHER_KEY_SEED: &str = "csqtt-chiper-key";

/// Derives the 32-byte AES-256 key via SHA-256 over the seed string.
pub fn cipher_key() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CSQTT_CIPHER_KEY_SEED.as_bytes());
    hasher.finalize().into()
}

const URL_SAFE_INDIFFERENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

const STANDARD_INDIFFERENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// Decodes base64 string, trying URL-safe alphabet first, then standard alphabet.
/// Whitespace characters (\n, \r, ' ') are ignored as in Kotlin.
pub fn decode_base64_safe(input: &str) -> Option<Vec<u8>> {
    let clean: String = input
        .chars()
        .filter(|&c| c != '\n' && c != '\r' && c != ' ')
        .collect();
    let clean = clean.trim();
    URL_SAFE_INDIFFERENT
        .decode(clean.as_bytes())
        .or_else(|_| STANDARD_INDIFFERENT.decode(clean.as_bytes()))
        .ok()
}

/// Decrypts AES-GCM-256 encrypted payload bytes from Base64 string.
/// Minimum length is 28 bytes (12 bytes IV + 16 bytes auth tag).
pub fn decrypt_csqtt_payload(raw_payload: &str) -> Option<String> {
    let bytes = decode_base64_safe(raw_payload)?;
    if bytes.len() >= 28 {
        let key = cipher_key();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(&bytes[..12]);
        let ciphertext_and_tag = &bytes[12..];
        if let Ok(plaintext) = cipher.decrypt(nonce, ciphertext_and_tag) {
            if let Ok(s) = String::from_utf8(plaintext) {
                return Some(s);
            }
        }
    }
    None
}

/// Encrypts plaintext payload using AES-256-GCM.
/// Derives a deterministic 12-byte IV via HmacSHA256 to produce byte-identical output to Kotlin.
pub fn encrypt_csqtt_payload(plaintext: &str) -> String {
    let key = cipher_key();
    let iv: [u8; 12] = {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&key).expect("HMAC can take key of any size");
        mac.update(plaintext.as_bytes());
        let hmac_bytes = mac.finalize().into_bytes();
        let mut iv_bytes = [0u8; 12];
        iv_bytes.copy_from_slice(&hmac_bytes[..12]);
        iv_bytes
    };

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&iv);
    let ciphertext_and_tag = match cipher.encrypt(nonce, plaintext.as_bytes()) {
        Ok(ct) => ct,
        Err(_) => return String::new(),
    };

    let mut combined = Vec::with_capacity(12 + ciphertext_and_tag.len());
    combined.extend_from_slice(&iv);
    combined.extend_from_slice(&ciphertext_and_tag);

    URL_SAFE_NO_PAD.encode(&combined)
}

/// URL-decodes a percent-encoded string with '+' decoded as space.
pub fn url_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return None;
                }
                let h1 = hex_val(bytes[i + 1])?;
                let h2 = hex_val(bytes[i + 2])?;
                out.push((h1 << 4) | h2);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Decodes a URL query component, preserving literal '+' characters (as in Kotlin).
pub fn decode_query_component(value: &str) -> Option<String> {
    let escaped = value.replace('+', "%2B");
    url_decode(&escaped)
}

/// Strips VK join call domain prefixes, query parameters, fragments, and trailing slashes.
pub fn strip_vk_url_static(input: &str) -> String {
    let mut s = input.trim();
    let lower = s.to_ascii_lowercase();
    const PREFIXES: [&str; 12] = [
        "https://vk.com/call/join/",
        "http://vk.com/call/join/",
        "https://m.vk.com/call/join/",
        "http://m.vk.com/call/join/",
        "m.vk.com/call/join/",
        "vk.com/call/join/",
        "https://vk.ru/call/join/",
        "http://vk.ru/call/join/",
        "https://m.vk.ru/call/join/",
        "http://m.vk.ru/call/join/",
        "m.vk.ru/call/join/",
        "vk.ru/call/join/",
    ];
    for prefix in &PREFIXES {
        if lower.starts_with(prefix) {
            s = &s[prefix.len()..];
            break;
        }
    }
    if let Some(idx) = s.find('?') {
        s = &s[..idx];
    }
    if let Some(idx) = s.find('#') {
        s = &s[..idx];
    }
    s.trim_end_matches('/').to_string()
}

/// Generates a random 16-character alphanumeric password using the system RNG.
pub fn generate_16_char_password() -> String {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    let mut s = String::with_capacity(16);
    for _ in 0..16 {
        let idx = rng.random_range(0..CHARS.len());
        s.push(CHARS[idx] as char);
    }
    s
}

/// Alias for `generate_16_char_password`.
pub fn generate16_char_password() -> String {
    generate_16_char_password()
}

/// Parses '+' separated list of VK join hashes (1 to 6 distinct valid hashes >= 16 chars).
pub fn parse_link_hashes(raw_hashes: &str) -> Vec<String> {
    let parts: Vec<&str> = raw_hashes.split('+').collect();
    if parts.len() < 1 || parts.len() > 6 || parts.iter().any(|p| p.is_empty()) {
        return Vec::new();
    }
    let mut hashes = Vec::with_capacity(parts.len());
    for encoded in parts {
        let stripped = decode_query_component(encoded)
            .map(|s| strip_vk_url_static(&s))
            .unwrap_or_default();
        hashes.push(stripped);
    }
    if hashes.iter().any(|h| h.len() < 16 || h.chars().any(|c| c.is_whitespace())) {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    for h in &hashes {
        if !seen.insert(h.clone()) {
            return Vec::new();
        }
    }
    hashes
}

/// Parses and validates VK access token (<= 4096 non-whitespace characters).
pub fn parse_link_vk_access_token(raw_token: &str) -> Option<String> {
    let token = decode_query_component(raw_token)
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    if !token.is_empty() && token.len() <= 4096 && !token.chars().any(|c| c.is_whitespace()) {
        Some(token)
    } else {
        None
    }
}

const KNOWN_KEYS: [&str; 8] = [
    "v",
    "host",
    "peer",
    "password",
    "client",
    "hashes",
    "token",
    "workers",
];

fn match_regex_query(sanitized: &str) -> Option<BTreeMap<String, String>> {
    let bytes = sanitized.as_bytes();
    let len = bytes.len();
    let mut matches = Vec::new();
    let mut i = 0;

    while i < len {
        let lookbehind_ok = if i == 0 {
            true
        } else {
            let prev = bytes[i - 1];
            prev == b'&' || prev == b';' || prev.is_ascii_alphanumeric() || prev == b'_'
        };

        if !lookbehind_ok {
            i += 1;
            continue;
        }

        let mut found_key: Option<(&'static str, usize)> = None;
        for &k in &KNOWN_KEYS {
            let k_len = k.len();
            if i + k_len < len
                && sanitized[i..i + k_len].eq_ignore_ascii_case(k)
                && bytes[i + k_len] == b'='
            {
                found_key = Some((k, k_len));
                break;
            }
        }

        let Some((matched_key, k_len)) = found_key else {
            i += 1;
            continue;
        };

        let val_start = i + k_len + 1;
        let mut val_end = None;
        let mut j = val_start;
        while j <= len {
            let lookahead_matches = if j == len {
                true
            } else {
                KNOWN_KEYS.iter().any(|k| {
                    let kl = k.len();
                    j + kl < len
                        && sanitized[j..j + kl].eq_ignore_ascii_case(k)
                        && bytes[j + kl] == b'='
                })
            };

            if lookahead_matches {
                val_end = Some(j);
                break;
            }

            if j < len && (bytes[j] == b'&' || bytes[j] == b'?') {
                break;
            }

            j += 1;
        }

        if let Some(end) = val_end {
            matches.push((matched_key, sanitized[val_start..end].to_string()));
            i = end;
        } else {
            i += 1;
        }
    }

    if matches.is_empty() {
        return None;
    }

    let mut extracted = BTreeMap::new();
    for (key, value) in matches {
        let decoded_key = decode_query_component(key)?.to_ascii_lowercase();
        if extracted.contains_key(&decoded_key) {
            return None;
        }
        extracted.insert(decoded_key, value);
    }

    if extracted.contains_key("v")
        && extracted.contains_key("host")
        && extracted.contains_key("peer")
    {
        Some(extracted)
    } else {
        None
    }
}

/// Parses raw query string supporting standard &/; delimiters and concatenated parameters.
pub fn parse_raw_query(raw_query: &str) -> Option<BTreeMap<String, String>> {
    if raw_query.is_empty() {
        return None;
    }
    let sanitized = raw_query.replace("&amp;", "&");
    let mut result = BTreeMap::new();

    if sanitized.contains('&') || sanitized.contains(';') {
        for part in sanitized.split(['&', ';']) {
            if part.trim().is_empty() {
                continue;
            }
            let Some(separator) = part.find('=') else {
                continue;
            };
            if separator == 0 {
                continue;
            }
            let raw_key = &part[..separator];
            let Some(key) = decode_query_component(raw_key) else {
                continue;
            };
            let value = &part[separator + 1..];
            if !key.trim().is_empty() {
                if result.contains_key(&key) {
                    return None;
                }
                result.insert(key, value.to_string());
            }
        }
        if !result.is_empty()
            && result.contains_key("v")
            && result.contains_key("host")
            && result.contains_key("peer")
        {
            return Some(result);
        }
    }

    if let Some(extracted) = match_regex_query(&sanitized) {
        return Some(extracted);
    }

    if !result.is_empty() {
        Some(result)
    } else {
        None
    }
}

fn parse_csqtt_uri_query(candidate: &str) -> Option<&str> {
    if candidate.chars().any(|c| c.is_ascii_control() || c == ' ') {
        return None;
    }
    let lower = candidate.to_ascii_lowercase();
    if !lower.starts_with("csqtt://") {
        return None;
    }
    let after_scheme = &candidate[8..];
    let before_fragment = if let Some((before, _)) = after_scheme.split_once('#') {
        before
    } else {
        after_scheme
    };
    let (authority_and_path, raw_query) = before_fragment.split_once('?')?;
    if authority_and_path.contains('/') || authority_and_path.contains('@') {
        return None;
    }
    if let Some((_, port_str)) = authority_and_path.rsplit_once(':') {
        if !port_str.contains(']') && (port_str.parse::<u16>().is_ok() || !port_str.is_empty()) {
            return None;
        }
    }
    Some(raw_query)
}

fn parse_csqtt_v2_query(raw_query: &str) -> Option<CsqttLink> {
    let parameters = parse_raw_query(raw_query)?;
    if parameters.get("v").and_then(|v| decode_query_component(v)).as_deref() != Some("2") {
        return None;
    }
    let host = parameters
        .get("host")
        .and_then(|h| decode_query_component(h))
        .filter(|h| !h.trim().is_empty())?;
    if host == "0.0.0.0"
        || host == "127.0.0.1"
        || host.eq_ignore_ascii_case("localhost")
        || host == "::1"
    {
        return None;
    }
    let port = parameters
        .get("peer")
        .and_then(|p| decode_query_component(p))
        .and_then(|p| p.parse::<u16>().ok())
        .filter(|&p| p >= 1)?;
    let password = parameters
        .get("password")
        .and_then(|p| decode_query_component(p))
        .map(|p| p.trim().to_string())
        .unwrap_or_default();
    if host.chars().any(|c| c.is_whitespace())
        || (!password.is_empty() && password.chars().any(|c| c.is_whitespace()))
    {
        return None;
    }
    let hashes = if let Some(raw_hashes) = parameters.get("hashes") {
        let parsed = parse_link_hashes(raw_hashes);
        if parsed.is_empty() {
            return None;
        }
        parsed
    } else {
        Vec::new()
    };
    let vk_access_token = if let Some(raw_token) = parameters.get("token") {
        let parsed = parse_link_vk_access_token(raw_token);
        if parsed.is_none() {
            return None;
        }
        parsed.unwrap()
    } else {
        String::new()
    };
    if !hashes.is_empty() && !vk_access_token.is_empty() {
        return None;
    }
    Some(CsqttLink {
        host,
        port,
        password,
        hashes,
        vk_access_token,
    })
}

/// Parses csqtt v2 URI candidate or query string.
pub fn parse_csqtt_v2(candidate_or_query: &str) -> Option<CsqttLink> {
    if let Some(query) = parse_csqtt_uri_query(candidate_or_query) {
        parse_csqtt_v2_query(query)
    } else {
        parse_csqtt_v2_query(candidate_or_query)
    }
}

/// Parses an encrypted csqtt:// link into a CsqttLink structure.
pub fn parse_csqtt_link(raw: &str) -> Option<CsqttLink> {
    let trimmed = raw.trim();
    let lower_trimmed = trimmed.to_ascii_lowercase();
    if !lower_trimmed.starts_with("csqtt://") {
        return None;
    }
    let payload = trimmed[8..].trim();
    let decoded = decrypt_csqtt_payload(payload)?;
    if decoded.contains("host=") || decoded.contains("connect?") {
        let lower_decoded = decoded.to_ascii_lowercase();
        let candidate = if lower_decoded.starts_with("csqtt://connect?") {
            decoded.to_string()
        } else if lower_decoded.starts_with("connect?") {
            format!("csqtt://{decoded}")
        } else if lower_decoded.starts_with("csqtt://") {
            format!("csqtt://connect?{}", &decoded[8..])
        } else {
            format!("csqtt://connect?{decoded}")
        };

        if let Some(raw_query) = parse_csqtt_uri_query(&candidate) {
            if let Some(parsed) = parse_csqtt_v2_query(raw_query) {
                return Some(parsed);
            }
        }
    }
    None
}

/// Formats connection parameters into an encrypted csqtt:// link.
pub fn format_csqtt_link(
    host: &str,
    peer: u16,
    password: &str,
    hashes: &[impl AsRef<str>],
    token: &str,
) -> String {
    let mut query_parts = vec![
        "v=2".to_string(),
        format!("host={host}"),
        format!("peer={peer}"),
        format!("password={password}"),
    ];
    if !hashes.is_empty() {
        let joined = hashes
            .iter()
            .map(|h| h.as_ref())
            .collect::<Vec<_>>()
            .join("+");
        query_parts.push(format!("hashes={joined}"));
    } else if !token.is_empty() {
        query_parts.push(format!("token={token}"));
    }
    let plain = format!("connect?{}", query_parts.join("&"));
    format!("csqtt://{}", encrypt_csqtt_payload(&plain))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(plain: &str) -> String {
        format!("csqtt://{}", encrypt_csqtt_payload(plain))
    }

    #[test]
    fn parses_v2_without_hashes() {
        let link = parse_csqtt_link(&enc("connect?v=2&host=203.0.113.7&peer=46000&password=p%40ss"))
            .expect("link should parse");
        assert_eq!("203.0.113.7", link.host);
        assert_eq!(46000, link.port);
        assert_eq!("p@ss", link.password);
        assert_eq!(Vec::<String>::new(), link.hashes);
    }

    #[test]
    fn parses_v2_concatenated_parameters() {
        let link = parse_csqtt_link(&enc("connect?v=2host=203.0.113.88peer=46000password=dummy_secret_pass"))
            .expect("link should parse");
        assert_eq!("203.0.113.88", link.host);
        assert_eq!(46000, link.port);
        assert_eq!("dummy_secret_pass", link.password);
        assert_eq!(Vec::<String>::new(), link.hashes);
    }

    #[test]
    fn parses_v2_with_six_plus_separated_hashes() {
        let hashes: Vec<String> = (1..=6).map(|i| format!("abcdefghijklmnop{i}")).collect();
        let query = format!(
            "connect?v=2&host=vps.example&peer=46000&password=secret&hashes={}",
            hashes.join("+")
        );
        let link = parse_csqtt_link(&enc(&query)).expect("link should parse");
        assert_eq!(hashes, link.hashes);
    }

    #[test]
    fn parses_v2_with_vk_token() {
        let link = parse_csqtt_link(&enc(
            "connect?v=2&host=vps.example&peer=46000&password=secret&token=vk1.aBc-_%2B9",
        ))
        .expect("link should parse");
        assert_eq!("vk1.aBc-_+9", link.vk_access_token);
        assert_eq!(Vec::<String>::new(), link.hashes);
    }

    #[test]
    fn preserves_encoded_plus_inside_hash() {
        let link = parse_csqtt_link(&enc(
            "connect?v=2&host=vps.example&peer=46000&password=secret&hashes=abcdefghijklmno%2Bp",
        ))
        .expect("link should parse");
        assert_eq!(vec!["abcdefghijklmno+p".to_string()], link.hashes);
    }

    #[test]
    fn formats_and_parses_clean_v2_link_with_password() {
        let formatted = format_csqtt_link(
            "203.0.113.7",
            46000,
            "SecretPass123456",
            &["abcdefghijklmnop1"],
            "",
        );
        let link = parse_csqtt_link(&formatted).expect("link should parse");
        assert_eq!("203.0.113.7", link.host);
        assert_eq!(46000, link.port);
        assert_eq!("SecretPass123456", link.password);
        assert_eq!(vec!["abcdefghijklmnop1".to_string()], link.hashes);
    }

    #[test]
    fn generate_16_char_password_produces_valid_password() {
        let pass = generate_16_char_password();
        assert_eq!(16, pass.len());
        assert!(pass.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn rejects_legacy_unencrypted_links() {
        assert!(parse_csqtt_link("csqtt://secret@203.0.113.7:46000").is_none());
        assert!(parse_csqtt_link("csqtt://connect?v=2&host=203.0.113.7&peer=46000&password=secret").is_none());
        let raw_payload = "connect?v=2&host=203.0.113.7&peer=46000&password=secret";
        let base64 = URL_SAFE_NO_PAD.encode(raw_payload.as_bytes());
        assert!(parse_csqtt_link(&format!("csqtt://{base64}")).is_none());
    }

    #[test]
    fn formats_ipv6_peer_with_brackets() {
        let link = parse_csqtt_link(&enc(
            "connect?v=2&host=2001%3Adb8%3A%3A1&peer=46000&password=secret",
        ))
        .expect("link should parse");
        assert_eq!("[2001:db8::1]:46000", link.peer_address());
    }

    #[test]
    fn rejects_malformed_v2_links() {
        assert!(parse_csqtt_link(&enc("connect?v=3&host=1.2.3.4&peer=46000&password=secret")).is_none());
        assert!(parse_csqtt_link(&enc("connect?v=2&host=1.2.3.4&peer=0&password=secret")).is_none());
        assert!(parse_csqtt_link(&enc("connect?v=2&host=1.2.3.4&peer=46000&password=secret&hashes=")).is_none());
        assert!(parse_csqtt_link(&enc("connect?v=2&host=1.2.3.4&peer=46000&password=secret&hashes=short")).is_none());
        assert!(parse_csqtt_link(&enc("connect?v=2&host=1.2.3.4&peer=46000&password=secret&token=bad%20token")).is_none());
        assert!(parse_csqtt_link(&enc("connect?v=2&host=1.2.3.4&peer=46000&password=secret&token=one&token=two")).is_none());
        assert!(parse_csqtt_link(&enc("connect?v=2&host=1.2.3.4&peer=46000&password=secret&hashes=abcdefghijklmnop&token=token")).is_none());
        let seven_hashes = (1..=7)
            .map(|i| format!("abcdefghijklmnop{i}"))
            .collect::<Vec<_>>()
            .join("+");
        assert!(parse_csqtt_link(&enc(&format!(
            "connect?v=2&host=1.2.3.4&peer=46000&password=secret&hashes={seven_hashes}"
        ))).is_none());
    }

    #[test]
    fn ignores_workers_in_old_links() {
        let link = parse_csqtt_link(&enc("connect?v=2&host=203.0.113.7&peer=46000&workers=36"))
            .expect("link should parse");
        assert_eq!("203.0.113.7", link.host);

        let malformed = parse_csqtt_link(&enc(
            "connect?v=2&host=203.0.113.7&peer=46000&workers=not-a-number",
        ))
        .expect("malformed workers link should parse");
        assert_eq!("203.0.113.7", malformed.host);
    }

    #[test]
    fn parses_encrypted_link() {
        let raw_payload = "connect?v=2&host=203.0.113.7&peer=46000&workers=36";
        let encrypted_base64 = encrypt_csqtt_payload(raw_payload);
        let encrypted_link = format!("csqtt://{encrypted_base64}");
        let link = parse_csqtt_link(&encrypted_link).expect("link should parse");
        assert_eq!("203.0.113.7", link.host);
        assert_eq!(46000, link.port);
    }

    #[test]
    fn parses_rust_encrypted_link() {
        let rust_link = "csqtt://GNANVH0fhD6MisDFiWP_P0Bm_FHr-frawgfwur3Zx4ipjJx8gk-M6ZZbnDr_2clwPmzO-QS8AkiHLaMXyYO1RHyPeehXPqTc6jJi_iw72fRQJ9AR918ne5XsBRg";
        let link = parse_csqtt_link(rust_link).expect("link should parse");
        assert_eq!("1.2.3.4", link.host);
        assert_eq!(46000, link.port);
    }

    #[test]
    fn deterministic_link_encryption() {
        let raw_payload = "connect?v=2&host=203.0.113.7&peer=46000";
        let enc1 = encrypt_csqtt_payload(raw_payload);
        let enc2 = encrypt_csqtt_payload(raw_payload);
        assert_eq!(enc1, enc2);
    }

    #[test]
    fn parses_v2_owner_link_without_password() {
        let link = parse_csqtt_link(&enc("connect?v=2&host=203.0.113.7&peer=46000"))
            .expect("link should parse");
        assert_eq!("203.0.113.7", link.host);
        assert_eq!(46000, link.port);
        assert_eq!("", link.password);
    }

    #[test]
    fn parses_v2_with_hashes_without_password() {
        let link = parse_csqtt_link(&enc(
            "connect?v=2&host=203.0.113.7&peer=46000&hashes=abcdefghijklmnop1",
        ))
        .expect("link should parse");
        assert_eq!("203.0.113.7", link.host);
        assert_eq!(46000, link.port);
        assert_eq!("", link.password);
        assert_eq!(vec!["abcdefghijklmnop1".to_string()], link.hashes);
    }

    #[test]
    fn tampered_encrypted_link_fails_to_decrypt() {
        let valid_encrypted = encrypt_csqtt_payload("connect?v=2&host=203.0.113.7&peer=46000");
        let mut decoded = decode_base64_safe(&valid_encrypted).unwrap();
        let last = decoded.len() - 1;
        decoded[last] ^= 1;
        let tampered_base64 = URL_SAFE_NO_PAD.encode(&decoded);
        assert!(parse_csqtt_link(&format!("csqtt://{tampered_base64}")).is_none());
    }

    #[test]
    fn truncated_encrypted_link_fails_to_decrypt() {
        let b64 = URL_SAFE_NO_PAD.encode([0u8; 10]);
        assert!(parse_csqtt_link(&format!("csqtt://{b64}")).is_none());
    }

    #[test]
    fn format_parse_round_trip() {
        let link = CsqttLink {
            host: "198.51.100.25".to_string(),
            port: 51820,
            password: "GeneratedPassword123".to_string(),
            hashes: vec![
                "abcdefghijklmnop1".to_string(),
                "abcdefghijklmnop2".to_string(),
            ],
            vk_access_token: String::new(),
        };
        let formatted = format_csqtt_link(
            &link.host,
            link.port,
            &link.password,
            &link.hashes,
            &link.vk_access_token,
        );
        let parsed = parse_csqtt_link(&formatted).expect("round-trip link should parse");
        assert_eq!(link, parsed);

        let link_token = CsqttLink {
            host: "198.51.100.25".to_string(),
            port: 51820,
            password: "GeneratedPassword123".to_string(),
            hashes: Vec::new(),
            vk_access_token: "vk1.aBc-_+9".to_string(),
        };
        let formatted_token = format_csqtt_link(
            &link_token.host,
            link_token.port,
            &link_token.password,
            &link_token.hashes,
            &link_token.vk_access_token,
        );
        let parsed_token =
            parse_csqtt_link(&formatted_token).expect("round-trip link with token should parse");
        assert_eq!(link_token, parsed_token);
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let plain = "connect?v=2&host=203.0.113.7&peer=46000&password=secret";
        let encrypted = encrypt_csqtt_payload(plain);
        let decrypted = decrypt_csqtt_payload(&encrypted).expect("decryption should succeed");
        assert_eq!(plain, decrypted);
    }
}
