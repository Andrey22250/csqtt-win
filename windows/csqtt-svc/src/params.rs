//! Stored settings -> core parameters, and config import.
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use csqtt_logic::connection_source::ConnectionSource;
use csqtt_logic::constants;
use csqtt_logic::csqtt_link::parse_csqtt_link;
use csqtt_logic::settings::{
    import_config_json, normalize_obfs_mode, normalize_srtp_mode, normalize_turn_transport,
    normalize_vk_auth_mode, SecretKey, Settings,
};
use csqtt_logic::tunnel_session_id::new_tunnel_session_id;
use csqtt_logic::vk_hash_validator::{VkHashValidationCodec, VkHashValidationStatus};
use csqtt_logic::worker_count_policy::WorkerCountPolicy;

use crate::core::args::CoreParams;

/// Read a secret by its storage name (SecretKey::storage_name()); None if absent.
pub type SecretLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Filters and normalizes active VK hashes, removing invalid ones according to validation results.
fn resolve_active_hashes(
    raw: &str,
    invalid_hashes: &BTreeMap<String, VkHashValidationStatus>,
) -> Vec<String> {
    let mut raw_list = Vec::new();
    let mut seen = HashSet::new();
    for part in raw.split(|c: char| c == ',' || c.is_whitespace()) {
        let trimmed = part.trim();
        if trimmed.len() >= 16 && seen.insert(trimmed.to_string()) {
            raw_list.push(trimmed.to_string());
        }
    }
    let filtered = VkHashValidationCodec::active(&raw_list, invalid_hashes);
    if !filtered.is_empty() {
        filtered
    } else {
        raw_list
    }
}

/// Build CoreParams from settings + secrets the way TunnelService.kt builds TunnelParams.
/// Err(text) with the same user-facing message Android shows when something required is missing.
pub fn core_params_from_settings(
    settings: &Settings,
    secrets: SecretLookup<'_>,
    data_dir: &Path,
) -> Result<CoreParams, String> {
    let profile = settings.current_profile();
    let invalid_hashes = VkHashValidationCodec::decode(&profile.vk_hash_check_results);

    let (peer, connection_password, source_hashes, hashes_from_link, token_from_link, link_token) =
        if profile.csqtt_link_mode {
            let link = parse_csqtt_link(&profile.csqtt_link)
                .ok_or_else(|| "нужна корректная ссылка CSQTT".to_string())?;

            let link_hashes = resolve_active_hashes(&link.hashes.join(","), &invalid_hashes);
            let has_token = !link.vk_access_token.is_empty();
            let hashes_str = if has_token {
                String::new()
            } else if !link_hashes.is_empty() {
                link_hashes.join(",")
            } else {
                resolve_active_hashes(&profile.vk_hashes, &invalid_hashes).join(",")
            };

            (
                link.peer_address(),
                link.password,
                hashes_str,
                !link_hashes.is_empty(),
                has_token,
                link.vk_access_token,
            )
        } else {
            let base_peer = profile.peer.trim();
            if base_peer.is_empty()
                || base_peer == "0.0.0.0"
                || base_peer == "127.0.0.1"
                || base_peer.eq_ignore_ascii_case("localhost")
                || base_peer == "::1"
            {
                return Err("не указан корректный адрес сервера".to_string());
            }

            let port = if profile.manual_ports_enabled {
                if !(1..=65535).contains(&profile.server_peer_port) {
                    return Err("указан некорректный порт сервера".to_string());
                }
                profile.server_peer_port
            } else {
                constants::network::DEFAULT_SERVER_PEER_PORT
            };
            let peer = ConnectionSource::format_peer(base_peer, port);

            let password = secrets(SecretKey::ConnectionPassword.storage_name())
                .or_else(|| secrets(SecretKey::ConnectionPassword.config_name()))
                .unwrap_or_default();
            if password.is_empty() {
                return Err("не указан пароль подключения".to_string());
            }

            let hashes_str = resolve_active_hashes(&profile.vk_hashes, &invalid_hashes).join(",");

            (peer, password, hashes_str, false, false, String::new())
        };

    let requested_auth = normalize_vk_auth_mode(&profile.vk_auth_mode);
    let configured_hash = profile.vk_hash_mode.as_str();

    let vk_auth_mode = if hashes_from_link
        && requested_auth == constants::vk_auth::MODE_AUTO_JS
    {
        constants::vk_auth::MODE_CALLS.to_string()
    } else if token_from_link {
        if configured_hash == constants::vk_auto_hash::MODE_AUTO_JS {
            constants::vk_auto_hash::MODE_AUTO_JS.to_string()
        } else if requested_auth == constants::vk_auth::MODE_AUTO_JS {
            constants::vk_auth::MODE_CALLS.to_string()
        } else {
            requested_auth.to_string()
        }
    } else {
        requested_auth.to_string()
    };

    let account_auto_js = vk_auth_mode == constants::vk_auth::MODE_AUTO_JS;
    // TunnelService.kt restoredHashes: link hashes as-is, otherwise resolveAutoHashes with
    // forcedMode = auto_js (account) / auto_api (token from link) / stored vkHashMode.
    let hash_mode = if hashes_from_link && !token_from_link && !account_auto_js {
        constants::vk_auto_hash::MODE_MANUAL
    } else if account_auto_js {
        constants::vk_auto_hash::MODE_AUTO_JS
    } else if token_from_link {
        constants::vk_auto_hash::MODE_AUTO_API
    } else if configured_hash == constants::vk_auto_hash::MODE_AUTO_JS
        || configured_hash == constants::vk_auto_hash::MODE_AUTO_API
    {
        configured_hash
    } else {
        constants::vk_auto_hash::MODE_MANUAL
    };

    let (vk_hashes, vk_hash_mode, vk_access_token, allow_hash_redistribution) =
        if hash_mode == constants::vk_auto_hash::MODE_MANUAL {
            let parsed_hashes = resolve_active_hashes(&source_hashes, &invalid_hashes);
            if parsed_hashes.is_empty() {
                return Err("нет действующих VK хешей или VK токена".to_string());
            }
            (parsed_hashes, hash_mode.to_string(), String::new(), false)
        } else {
            let token = if token_from_link {
                link_token
            } else {
                secrets(SecretKey::VkAccessToken.storage_name())
                    .or_else(|| secrets(SecretKey::VkAccessToken.config_name()))
                    .unwrap_or_default()
            };
            if token.trim().is_empty() {
                return Err("Ошибка: VK access token не указан".to_string());
            }
            // auto_js: the core creates calls itself. auto_api: the service creates the calls
            // with the token before starting the core (manager.rs, VkAutoCallsManager port);
            // redistribution is decided there.
            let redistribute = hash_mode == constants::vk_auto_hash::MODE_AUTO_JS;
            (Vec::new(), hash_mode.to_string(), token, redistribute)
        };

    let domain_white = profile.domain_is_whitelist;
    let domain_list_raw = if domain_white {
        &profile.direct_domains_white
    } else if !profile.direct_domains_black.is_empty() {
        &profile.direct_domains_black
    } else {
        &profile.direct_domains
    };

    let domains: Vec<&str> = domain_list_raw
        .split(|c: char| c == ',' || c == '\n' || c == '\r')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .collect();

    let user_txt_path = data_dir.join("user.txt");
    let direct_domains_file = if !domains.is_empty() {
        let _ = std::fs::create_dir_all(data_dir);
        let content = domains.join("\n");
        std::fs::write(&user_txt_path, content.as_bytes())
            .map_err(|e| format!("failed to write user.txt: {e}"))?;
        user_txt_path
    } else {
        if user_txt_path.exists() {
            let _ = std::fs::remove_file(&user_txt_path);
        }
        PathBuf::new()
    };

    let domains_txt_path = data_dir.join("domains.txt");
    let template_domains_file = if domains_txt_path.exists()
        && std::fs::metadata(&domains_txt_path).map(|m| m.len() > 0).unwrap_or(false)
    {
        domains_txt_path
    } else {
        PathBuf::new()
    };

    let template_domains = if domain_white { false } else { profile.template_domains_enabled };
    let exclude_ru_zones = if domain_white { false } else { profile.exclude_ru_zones_enabled };
    // TunnelService.kt: enabled flag OR any effective domain source.
    let domain_routing_enabled = profile.domain_routing_enabled
        || template_domains
        || !domain_list_raw.trim().is_empty()
        || exclude_ru_zones;

    let ip_white = profile.ip_is_whitelist;
    let ip_list_raw = if ip_white {
        &profile.direct_ips_white
    } else {
        &profile.direct_ips_black
    };
    let direct_ips = ip_list_raw.trim().to_string();
    // TunnelService.kt: ipRoutingEnabled || ipList.isNotBlank().
    let ip_routing_enabled = profile.ip_routing_enabled || !direct_ips.is_empty();

    let workers_per_hash = u32::try_from(
        WorkerCountPolicy::normalize(profile.workers_per_hash).max(1),
    )
    .unwrap_or(18);

    // TunnelService.kt always passes port = 0 (ephemeral local listener).
    let port = 0;

    let captcha_mode = if profile.captcha_mode.trim().is_empty() {
        "auto".to_string()
    } else {
        profile.captcha_mode.trim().to_string()
    };

    let fingerprint = if profile.selected_fingerprint.trim().is_empty() {
        constants::tunnel::DEFAULT_FINGERPRINT.to_string()
    } else {
        profile.selected_fingerprint.trim().to_string()
    };

    let client_ids = if profile.active_client_ids.trim().is_empty() {
        constants::tunnel::DEFAULT_CLIENT_IDS.to_string()
    } else {
        profile.active_client_ids.trim().to_string()
    };

    let obfs_mode = normalize_obfs_mode(&profile.obfs_mode).to_string();
    let turn_transport = normalize_turn_transport(&profile.turn_transport).to_string();
    let srtp_mode = normalize_srtp_mode(&profile.srtp_mode).to_string();

    let generation_id = u64::try_from(profile.connection_generation.max(0)).unwrap_or(0);
    let session_salt = new_tunnel_session_id();

    Ok(CoreParams {
        vk_access_token,
        peer,
        vk_hashes,
        workers_per_hash,
        port,
        connection_password,
        vk_auth_mode,
        captcha_mode,
        fingerprint,
        client_ids,
        obfs_mode,
        turn_transport,
        srtp_mode,
        generation_id,
        session_salt,
        allow_hash_redistribution,
        vk_hash_mode,
        custom_turn_ips: profile.custom_turn_ips.clone(),
        domain_routing_enabled,
        domain_is_whitelist: domain_white,
        exclude_ru_zones,
        template_domains,
        template_domains_file,
        direct_domains_file,
        ip_routing_enabled,
        ip_is_whitelist: ip_white,
        direct_ips,
        // Windows: the profile's excluded_apps holds exe names / paths (Android: package names).
        excluded_apps: profile
            .excluded_apps
            .split(|c: char| matches!(c, ',' | '|' | '\n' | '\r'))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

/// Result of importing a csqtt:// link or an Android config export.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportOutcome {
    pub settings: Settings,
    /// (storage name, value) pairs to write into the SecretStore.
    pub secrets: Vec<(String, String)>,
}

/// Accepts a csqtt:// link (switches to link mode like Android) or a config JSON export.
pub fn import_text(text: &str, base: &Settings) -> Result<ImportOutcome, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("текст для импорта пуст".to_string());
    }

    if trimmed.to_ascii_lowercase().starts_with("csqtt://") {
        let _link = parse_csqtt_link(trimmed)
            .ok_or_else(|| "Неверная ссылка CSQTT".to_string())?;
        let mut settings = base.clone();
        let profile = settings.current_profile_mut();
        profile.csqtt_link = trimmed.to_string();
        profile.csqtt_link_mode = true;
        Ok(ImportOutcome {
            settings,
            secrets: Vec::new(),
        })
    } else {
        let imported = import_config_json(trimmed, base)?;
        let secrets = imported
            .secrets
            .into_iter()
            .map(|(k, v)| (k.storage_name().to_string(), v))
            .collect();
        Ok(ImportOutcome {
            settings: imported.settings,
            secrets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csqtt_logic::csqtt_link::format_csqtt_link;
    use csqtt_logic::settings::export_config_json;

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_temp_dir(tag: &str) -> (PathBuf, TempDirGuard) {
        let path = std::env::temp_dir().join(format!("csqtt_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        (path.clone(), TempDirGuard(path))
    }

    #[test]
    fn test_manual_mode_settings_to_expected_core_params() {
        let (data_dir, _guard) = make_temp_dir("manual_mode");
        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "203.0.113.10".to_string();
        profile.server_peer_port = 46000;
        profile.manual_ports_enabled = false;
        profile.vk_hashes = "hash1111111111111111,hash2222222222222222".to_string();
        profile.workers_per_hash = 20;
        profile.selected_fingerprint = "firefox".to_string();
        profile.active_client_ids = "8202606,6287487".to_string();
        profile.obfs_mode = "unified".to_string();
        profile.turn_transport = "udp".to_string();
        profile.srtp_mode = "auto".to_string();
        profile.direct_domains = "example.com,test.org".to_string();
        profile.domain_routing_enabled = true;
        profile.csqtt_link_mode = false;

        let secrets_fn = |name: &str| {
            if name == SecretKey::ConnectionPassword.storage_name() {
                Some("mypassword".to_string())
            } else {
                None
            }
        };

        let params = core_params_from_settings(&settings, &secrets_fn, &data_dir)
            .expect("should produce CoreParams");

        assert_eq!(params.peer, "203.0.113.10:46000");
        assert_eq!(params.connection_password, "mypassword");
        assert_eq!(
            params.vk_hashes,
            vec!["hash1111111111111111".to_string(), "hash2222222222222222".to_string()]
        );
        assert_eq!(params.workers_per_hash, 20);
        assert_eq!(params.fingerprint, "firefox");
        assert_eq!(params.client_ids, "8202606,6287487");
        assert_eq!(params.obfs_mode, "unified");
        assert_eq!(params.turn_transport, "udp");
        assert_eq!(params.srtp_mode, "auto");
        assert_eq!(params.vk_hash_mode, "manual");
        assert_eq!(params.allow_hash_redistribution, false);
        assert_eq!(params.domain_routing_enabled, true);
        assert_eq!(params.direct_domains_file, data_dir.join("user.txt"));
        assert_eq!(params.session_salt.len(), 32);

        let user_txt_content = std::fs::read_to_string(data_dir.join("user.txt"))
            .expect("user.txt should exist");
        assert_eq!(user_txt_content, "example.com\ntest.org");
    }

    #[test]
    fn test_missing_password_returns_err() {
        let (data_dir, _guard) = make_temp_dir("missing_password");
        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "203.0.113.10".to_string();
        profile.vk_hashes = "hash1111111111111111".to_string();

        let no_secrets = |_: &str| None;
        let res = core_params_from_settings(&settings, &no_secrets, &data_dir);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "не указан пароль подключения");
    }

    #[test]
    fn test_missing_peer_returns_err() {
        let (data_dir, _guard) = make_temp_dir("missing_peer");
        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "".to_string();
        profile.vk_hashes = "hash1111111111111111".to_string();

        let secrets_fn = |name: &str| {
            if name == SecretKey::ConnectionPassword.storage_name() {
                Some("pass".to_string())
            } else {
                None
            }
        };

        let res = core_params_from_settings(&settings, &secrets_fn, &data_dir);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "не указан корректный адрес сервера");
    }

    #[test]
    fn test_missing_hashes_returns_err() {
        let (data_dir, _guard) = make_temp_dir("missing_hashes");
        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "203.0.113.10".to_string();
        profile.vk_hashes = "".to_string();

        let secrets_fn = |name: &str| {
            if name == SecretKey::ConnectionPassword.storage_name() {
                Some("pass".to_string())
            } else {
                None
            }
        };

        let res = core_params_from_settings(&settings, &secrets_fn, &data_dir);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "нет действующих VK хешей или VK токена");
    }

    #[test]
    fn test_link_import_and_core_params() {
        let (data_dir, _guard) = make_temp_dir("link_import");
        let link_text = format_csqtt_link(
            "203.0.113.5",
            46000,
            "linksecretpass123",
            &["abcdefghijklmnop1", "abcdefghijklmnop2"],
            "",
        );

        let outcome = import_text(&link_text, &Settings::default()).expect("import link must succeed");
        assert!(outcome.settings.current_profile().csqtt_link_mode);
        assert_eq!(outcome.settings.current_profile().csqtt_link, link_text);
        assert!(outcome.secrets.is_empty());

        let no_secrets = |_: &str| None;
        let params = core_params_from_settings(&outcome.settings, &no_secrets, &data_dir)
            .expect("core_params_from_settings with link should succeed");

        assert_eq!(params.peer, "203.0.113.5:46000");
        assert_eq!(params.connection_password, "linksecretpass123");
        assert_eq!(
            params.vk_hashes,
            vec!["abcdefghijklmnop1".to_string(), "abcdefghijklmnop2".to_string()]
        );
        assert_eq!(params.vk_hash_mode, "manual");
        assert_eq!(params.allow_hash_redistribution, false);
    }

    #[test]
    fn test_link_with_token_auto_api_or_auto_js() {
        let (data_dir, _guard) = make_temp_dir("link_token");
        let link_text = format_csqtt_link(
            "203.0.113.5",
            46000,
            "linksecretpass123",
            &[] as &[&str],
            "vk1.a.test_token_12345678",
        );

        let outcome = import_text(&link_text, &Settings::default()).expect("import link must succeed");
        let no_secrets = |_: &str| None;
        let params = core_params_from_settings(&outcome.settings, &no_secrets, &data_dir)
            .expect("core_params_from_settings with token link should succeed");

        assert_eq!(params.peer, "203.0.113.5:46000");
        assert_eq!(params.connection_password, "linksecretpass123");
        assert!(params.vk_hashes.is_empty());
        // TunnelService.kt: a token from the link without auto_js hash mode -> auto_api.
        assert_eq!(params.vk_hash_mode, "auto_api");
        assert_eq!(params.vk_access_token, "vk1.a.test_token_12345678");
        assert!(!params.allow_hash_redistribution);

        // With the stored hash mode auto_js the account auth switches to auto_js.
        let mut settings = outcome.settings.clone();
        settings.current_profile_mut().vk_hash_mode = "auto_js".to_string();
        let params = core_params_from_settings(&settings, &no_secrets, &data_dir).unwrap();
        assert_eq!(params.vk_hash_mode, "auto_js");
        assert_eq!(params.vk_auth_mode, "auto_js");
        assert!(params.allow_hash_redistribution);
    }

    #[test]
    fn test_auto_js_mode_missing_token_returns_err() {
        let (data_dir, _guard) = make_temp_dir("auto_js_missing_token");
        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "203.0.113.10".to_string();
        profile.vk_hash_mode = "auto_js".to_string();

        let secrets_fn = |name: &str| {
            if name == SecretKey::ConnectionPassword.storage_name() {
                Some("pass".to_string())
            } else {
                None
            }
        };

        let res = core_params_from_settings(&settings, &secrets_fn, &data_dir);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Ошибка: VK access token не указан");
    }

    #[test]
    fn test_json_import_round_trip_with_export_config_json() {
        let (data_dir, _guard) = make_temp_dir("json_round_trip");
        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "192.0.2.1".to_string();
        profile.server_peer_port = 50000;
        profile.manual_ports_enabled = true;
        profile.obfs_mode = "unified".to_string();
        profile.turn_transport = "tcp_tls".to_string();
        profile.srtp_mode = "dtls-srtp".to_string();
        profile.vk_hashes = "abcdefghijklmnop1,abcdefghijklmnop2".to_string();
        profile.direct_domains_black = "foo.com,bar.org".to_string();
        profile.domain_routing_enabled = true;

        let secrets_fn = |k: SecretKey| match k {
            SecretKey::ConnectionPassword => Some("mypassword123".to_string()),
            SecretKey::VkAccessToken => Some("mytoken456".to_string()),
            _ => None,
        };

        let json = export_config_json(&settings, &secrets_fn);
        let outcome = import_text(&json, &Settings::default()).expect("import JSON must succeed");

        assert_eq!(outcome.settings.current_profile().peer, "192.0.2.1");
        assert_eq!(outcome.settings.current_profile().server_peer_port, 50000);
        assert_eq!(outcome.settings.current_profile().manual_ports_enabled, true);
        assert_eq!(outcome.settings.current_profile().obfs_mode, "unified");
        assert_eq!(outcome.settings.current_profile().turn_transport, "tcp_tls");
        assert_eq!(outcome.settings.current_profile().srtp_mode, "dtls-srtp");
        assert_eq!(
            outcome.settings.current_profile().vk_hashes,
            "abcdefghijklmnop1,abcdefghijklmnop2"
        );
        assert_eq!(
            outcome.settings.current_profile().direct_domains_black,
            "foo.com,bar.org"
        );
        assert_eq!(
            outcome.settings.current_profile().domain_routing_enabled,
            true
        );

        let conn_pw = outcome
            .secrets
            .iter()
            .find(|(k, _)| k == SecretKey::ConnectionPassword.storage_name());
        assert_eq!(conn_pw.map(|(_, v)| v.as_str()), Some("mypassword123"));

        let vk_tok = outcome
            .secrets
            .iter()
            .find(|(k, _)| k == SecretKey::VkAccessToken.storage_name());
        assert_eq!(vk_tok.map(|(_, v)| v.as_str()), Some("mytoken456"));

        let secrets_map: std::collections::HashMap<_, _> = outcome.secrets.into_iter().collect();
        let lookup = |name: &str| secrets_map.get(name).cloned();
        let params = core_params_from_settings(&outcome.settings, &lookup, &data_dir)
            .expect("core_params_from_settings after JSON import must succeed");

        assert_eq!(params.peer, "192.0.2.1:50000");
        assert_eq!(params.connection_password, "mypassword123");
        assert_eq!(
            params.vk_hashes,
            vec!["abcdefghijklmnop1".to_string(), "abcdefghijklmnop2".to_string()]
        );
        assert_eq!(params.obfs_mode, "unified");
        assert_eq!(params.turn_transport, "tcp_tls");
        assert_eq!(params.srtp_mode, "dtls-srtp");
        assert_eq!(params.domain_routing_enabled, true);
        assert_eq!(params.direct_domains_file, data_dir.join("user.txt"));

        let user_content = std::fs::read_to_string(data_dir.join("user.txt")).unwrap();
        assert_eq!(user_content, "foo.com\nbar.org");
    }

    #[test]
    fn test_template_domains_file_detection() {
        let (data_dir, _guard) = make_temp_dir("template_domains");
        let domains_txt = data_dir.join("domains.txt");
        std::fs::write(&domains_txt, b"template.example.com\n").unwrap();

        let mut settings = Settings::default();
        let profile = settings.current_profile_mut();
        profile.peer = "192.0.2.1".to_string();
        profile.template_domains_enabled = true;
        profile.domain_routing_enabled = true;
        profile.vk_hashes = "abcdefghijklmnop1".to_string();

        let lookup = |name: &str| {
            if name == SecretKey::ConnectionPassword.storage_name() {
                Some("pass".to_string())
            } else {
                None
            }
        };

        let params = core_params_from_settings(&settings, &lookup, &data_dir).expect("should succeed");
        assert_eq!(params.template_domains, true);
        assert_eq!(params.template_domains_file, domains_txt);
        assert!(params.direct_domains_file.as_os_str().is_empty());

        // When empty domains.txt, template_domains_file should be empty
        std::fs::write(&domains_txt, b"").unwrap();
        let params_empty =
            core_params_from_settings(&settings, &lookup, &data_dir).expect("should succeed");
        assert_eq!(params_empty.template_domains, true);
        assert!(params_empty.template_domains_file.as_os_str().is_empty());
    }

    #[test]
    fn test_import_invalid_link_returns_err() {
        let res = import_text("csqtt://invalid_payload", &Settings::default());
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Неверная ссылка CSQTT");
    }

    #[test]
    fn test_import_invalid_json_returns_err() {
        let res = import_text("{not json}", &Settings::default());
        assert!(res.is_err());
    }
}
