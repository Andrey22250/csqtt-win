//! Pure-logic port of SettingsStore.kt.
//!
//! Provides `Settings` and `Profile` representing all persisted preferences,
//! `SecretKey` identifying encrypted credentials stored outside settings,
//! `export_config_json` / `import_config_json` for exchange with the Android client,
//! and validation / normalisation helpers matching Kotlin behavior.

use serde::{Deserialize, Serialize};

/// Secret values encrypted and stored separately from general settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecretKey {
    /// Tunnel connection password.
    ConnectionPassword,
    /// VK OAuth access token.
    VkAccessToken,
    /// SSH deployment login password.
    DeployPassword,
    /// Remote deployment server master password.
    DeployMainPassword,
    /// Remote deployment web interface password.
    DeployWebPassword,
    /// SSH private key in PEM/OpenSSH format.
    SshPrivateKey,
    /// Passphrase for encrypted SSH private key.
    SshKeyPassphrase,
    /// SSH certificate for authentication.
    SshCertificate,
}

impl SecretKey {
    /// Stable encrypted storage key name used by Android Keystore / SecureStringStore.
    pub fn storage_name(&self) -> &'static str {
        match self {
            Self::ConnectionPassword => "connection_password_encrypted",
            Self::VkAccessToken => "vk_access_token_encrypted",
            Self::DeployPassword => "deploy_password_encrypted",
            Self::DeployMainPassword => "deploy_main_password_encrypted",
            Self::DeployWebPassword => "deploy_web_password_encrypted",
            Self::SshPrivateKey => "ssh_private_key_encrypted",
            Self::SshKeyPassphrase => "ssh_key_passphrase_encrypted",
            Self::SshCertificate => "ssh_certificate_encrypted",
        }
    }

    /// Configuration key name used in JSON exchange or legacy unencrypted storage.
    pub fn config_name(&self) -> &'static str {
        match self {
            Self::ConnectionPassword => "connection_password",
            Self::VkAccessToken => "vk_access_token",
            Self::DeployPassword => "deploy_password",
            Self::DeployMainPassword => "deploy_main_password",
            Self::DeployWebPassword => "deploy_web_password",
            Self::SshPrivateKey => "ssh_private_key",
            Self::SshKeyPassphrase => "ssh_key_passphrase",
            Self::SshCertificate => "ssh_certificate",
        }
    }
}

/// Profile-scoped preferences. Kotlin SettingsStore appends `_{profile}` to DataStore keys
/// for profiles 1..=3, using base keys without suffix for profile 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// DataStore key: `profile_name`
    pub profile_name: String,
    /// DataStore key: `peer`
    pub peer: String,
    /// DataStore key: `vk_hashes`
    pub vk_hashes: String,
    /// DataStore key: `secondary_vk_hash`
    pub secondary_vk_hash: String,
    /// DataStore key: `workers_per_hash`
    pub workers_per_hash: i32,
    /// DataStore key: `protocol`
    pub protocol: String,
    /// DataStore key: `listen_port`
    pub listen_port: i32,
    /// DataStore key: `manual_ports_enabled`
    pub manual_ports_enabled: bool,
    /// DataStore key: `server_peer_port`
    pub server_peer_port: i32,
    /// DataStore key: `sni`
    pub sni: String,
    /// DataStore key: `no_dns`
    pub no_dns: bool,
    /// DataStore key: `custom_turn_ips`
    pub custom_turn_ips: String,
    /// DataStore key: `selected_turn_subnet`
    pub selected_turn_subnet: String,
    /// DataStore key: `turn_subnet_ip_limit`
    pub turn_subnet_ip_limit: String,
    /// DataStore key: `user_agent`
    pub user_agent: String,
    /// DataStore key: `deploy_ip`
    pub deploy_ip: String,
    /// DataStore key: `deploy_login`
    pub deploy_login: String,
    /// DataStore key: `deploy_ssh_port`
    pub deploy_ssh_port: String,
    /// DataStore key: `ssh_keys_mode`
    pub ssh_keys_mode: bool,
    /// DataStore key: `docker_install`
    pub docker_install: bool,
    /// DataStore key: `deploy_secret_path_enabled`
    pub deploy_secret_path_enabled: bool,
    /// DataStore key: `deploy_secret_path_uuid`
    pub deploy_secret_path_uuid: String,
    /// DataStore key: `excluded_apps`
    pub excluded_apps: String,
    /// DataStore key: `whitelist_apps`
    pub whitelist_apps: String,
    /// DataStore key: `detailed_logs`
    pub detailed_logs: bool,
    /// DataStore key: `deploy_web_login`
    pub deploy_web_login: String,
    /// DataStore key: `server_web_port`
    pub server_web_port: i32,
    /// DataStore key: `proxy_mode`
    pub proxy_mode: String,
    /// DataStore key: `proxy_host`
    pub proxy_host: String,
    /// DataStore key: `proxy_port`
    pub proxy_port: i32,
    /// DataStore key: `vk_auth_mode`
    pub vk_auth_mode: String,
    /// DataStore key: `obfs_mode`
    pub obfs_mode: String,
    /// DataStore key: `turn_transport`
    pub turn_transport: String,
    /// DataStore key: `srtp_mode`
    pub srtp_mode: String,
    /// DataStore key: `captcha_mode`
    pub captcha_mode: String,
    /// DataStore key: `captcha_solve_method`
    pub captcha_solve_method: String,
    /// DataStore key: `captcha_wbv_solve_method`
    pub captcha_wbv_solve_method: String,
    /// DataStore key: `vk_hash_mode`
    pub vk_hash_mode: String,
    /// DataStore key: `vk_access_token_user_id`
    pub vk_access_token_user_id: String,
    /// DataStore key: `is_whitelist`
    pub is_whitelist: bool,
    /// DataStore key: `extra_workers`
    pub extra_workers: bool,
    /// DataStore key: `domain_routing_enabled`
    pub domain_routing_enabled: bool,
    /// DataStore key: `domain_is_whitelist`
    pub domain_is_whitelist: bool,
    /// DataStore key: `exclude_ru_zones_enabled`
    pub exclude_ru_zones_enabled: bool,
    /// DataStore key: `template_domains_enabled`
    pub template_domains_enabled: bool,
    /// DataStore key: `direct_domains`
    pub direct_domains: String,
    /// DataStore key: `direct_domains_black`
    pub direct_domains_black: String,
    /// DataStore key: `direct_domains_white`
    pub direct_domains_white: String,
    /// DataStore key: `ip_routing_enabled`
    pub ip_routing_enabled: bool,
    /// DataStore key: `ip_is_whitelist`
    pub ip_is_whitelist: bool,
    /// DataStore key: `direct_ips_black`
    pub direct_ips_black: String,
    /// DataStore key: `direct_ips_white`
    pub direct_ips_white: String,
    /// DataStore key: `selected_fingerprint`
    pub selected_fingerprint: String,
    /// DataStore key: `active_client_ids`
    pub active_client_ids: String,
    /// DataStore key: `vk_hash_check_results`
    pub vk_hash_check_results: String,
    /// DataStore key: `csqtt_link`
    pub csqtt_link: String,
    /// DataStore key: `csqtt_link_mode`
    pub csqtt_link_mode: bool,
    /// DataStore key: `participant_show_logs`
    pub participant_show_logs: bool,
    /// DataStore key: `connection_generation`
    pub connection_generation: i64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            profile_name: String::new(),
            peer: String::new(),
            vk_hashes: String::new(),
            secondary_vk_hash: String::new(),
            workers_per_hash: 18,
            protocol: "udp".to_string(),
            listen_port: crate::constants::network::DEFAULT_LOCAL_PORT,
            manual_ports_enabled: false,
            server_peer_port: crate::constants::network::DEFAULT_SERVER_PEER_PORT,
            sni: String::new(),
            no_dns: false,
            custom_turn_ips: String::new(),
            selected_turn_subnet: String::new(),
            turn_subnet_ip_limit: "ALL".to_string(),
            user_agent: String::new(),
            deploy_ip: String::new(),
            deploy_login: String::new(),
            deploy_ssh_port: String::new(),
            ssh_keys_mode: false,
            docker_install: false,
            deploy_secret_path_enabled: true,
            deploy_secret_path_uuid: String::new(),
            excluded_apps: String::new(),
            whitelist_apps: String::new(),
            detailed_logs: false,
            deploy_web_login: String::new(),
            server_web_port: crate::constants::network::DEFAULT_SERVER_WEB_PORT,
            proxy_mode: String::new(),
            proxy_host: String::new(),
            proxy_port: 0,
            vk_auth_mode: crate::constants::vk_auth::MODE_CALLS.to_string(),
            obfs_mode: crate::constants::tunnel::DEFAULT_OBFS_MODE.to_string(),
            turn_transport: crate::constants::tunnel::DEFAULT_TURN_TRANSPORT.to_string(),
            srtp_mode: crate::constants::srtp_mode::DEFAULT.to_string(),
            captcha_mode: "auto".to_string(),
            captcha_solve_method: "auto".to_string(),
            captcha_wbv_solve_method: "auto".to_string(),
            vk_hash_mode: crate::constants::vk_auto_hash::MODE_MANUAL.to_string(),
            vk_access_token_user_id: String::new(),
            is_whitelist: false,
            extra_workers: false,
            domain_routing_enabled: false,
            domain_is_whitelist: false,
            exclude_ru_zones_enabled: false,
            template_domains_enabled: false,
            direct_domains: String::new(),
            direct_domains_black: String::new(),
            direct_domains_white: String::new(),
            ip_routing_enabled: false,
            ip_is_whitelist: false,
            direct_ips_black: String::new(),
            direct_ips_white: String::new(),
            selected_fingerprint: crate::constants::tunnel::DEFAULT_FINGERPRINT.to_string(),
            active_client_ids: crate::constants::tunnel::DEFAULT_CLIENT_IDS.to_string(),
            vk_hash_check_results: "{}".to_string(),
            csqtt_link: String::new(),
            csqtt_link_mode: false,
            participant_show_logs: false,
            connection_generation: 0,
        }
    }
}

impl Profile {
    /// Returns the human-readable display name, defaulting to "Профиль {index}" when blank.
    pub fn display_name(&self, index: usize) -> String {
        let trimmed = self.profile_name.trim();
        if trimmed.is_empty() {
            format!("Профиль {}", index)
        } else {
            trimmed.to_string()
        }
    }
}

/// Global and multi-profile settings mirroring Android `SettingsStore`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// DataStore key: `active_profile`
    pub active_profile: i32,
    /// DataStore key: `show_system_apps`
    pub show_system_apps: bool,
    /// DataStore key: `logging_enabled`
    pub logging_enabled: bool,
    /// DataStore key: `info_actions_expanded`
    pub info_actions_expanded: bool,
    /// DataStore key: `info_project_expanded`
    pub info_project_expanded: bool,
    /// DataStore key: `auto_js_risk_acknowledged`
    pub auto_js_risk_acknowledged: bool,
    /// DataStore key: `tcp_transport_risk_acknowledged`
    pub tcp_transport_risk_acknowledged: bool,
    /// DataStore key: `battery_optimization_prompt_handled`
    pub battery_optimization_prompt_handled: bool,
    /// DataStore key: `auto_pause_on_wifi`
    pub auto_pause_on_wifi: bool,
    /// DataStore key: `theme_mode`
    pub theme_mode: String,
    /// DataStore key: `is_dynamic_color`
    pub is_dynamic_color: bool,
    /// DataStore key: `theme_palette`
    pub theme_palette: String,
    /// DataStore key: `update_last_check_at`
    pub update_last_check_at: i64,
    /// DataStore key: `update_latest_version`
    pub update_latest_version: String,
    /// DataStore key: `update_last_error`
    pub update_last_error: String,
    /// DataStore key: `update_check_interval_hours`
    pub update_check_interval_hours: i32,
    /// DataStore key: `update_postpone_until`
    pub update_postpone_until: i64,
    /// DataStore key: `update_postpone_version`
    pub update_postpone_version: String,
    /// DataStore key: `update_dialog_last_shown_version`
    pub update_dialog_last_shown_version: String,
    /// DataStore key: `update_dialog_last_shown_at`
    pub update_dialog_last_shown_at: i64,
    /// DataStore key: `update_dialog_last_action_version`
    pub update_dialog_last_action_version: String,
    /// DataStore key: `update_dialog_last_action`
    pub update_dialog_last_action: String,
    /// DataStore key: `update_dialog_last_action_at`
    pub update_dialog_last_action_at: i64,
    /// DataStore key: `client_id_check_results`
    pub client_id_check_results: String,
    /// DataStore key: `tunnel_last_diagnostic`
    pub tunnel_last_diagnostic: String,
    /// DataStore key: `tunnel_last_diagnostic_at`
    pub tunnel_last_diagnostic_at: i64,
    /// DataStore key: `tunnel_was_running`
    pub tunnel_was_running: bool,
    /// DataStore key: `split_tunnel_whitelist_migrated`
    pub split_tunnel_whitelist_migrated: bool,
    /// Profiles 0 through 3 (CsqttConstants.Profiles.MIN_INDEX..MAX_INDEX).
    pub profiles: Vec<Profile>,
}

fn default_profiles() -> Vec<Profile> {
    (crate::constants::profiles::MIN_INDEX..=crate::constants::profiles::MAX_INDEX)
        .map(|_| Profile::default())
        .collect()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_profile: crate::constants::profiles::MIN_INDEX,
            show_system_apps: false,
            logging_enabled: true,
            info_actions_expanded: false,
            info_project_expanded: false,
            auto_js_risk_acknowledged: false,
            tcp_transport_risk_acknowledged: false,
            battery_optimization_prompt_handled: false,
            auto_pause_on_wifi: false,
            theme_mode: crate::constants::theme::MODE_SYSTEM.to_string(),
            is_dynamic_color: false,
            theme_palette: crate::constants::theme::PALETTE_INDIGO.to_string(),
            update_last_check_at: 0,
            update_latest_version: String::new(),
            update_last_error: String::new(),
            update_check_interval_hours: 24,
            update_postpone_until: 0,
            update_postpone_version: String::new(),
            update_dialog_last_shown_version: String::new(),
            update_dialog_last_shown_at: 0,
            update_dialog_last_action_version: String::new(),
            update_dialog_last_action: String::new(),
            update_dialog_last_action_at: 0,
            client_id_check_results: "{}".to_string(),
            tunnel_last_diagnostic: String::new(),
            tunnel_last_diagnostic_at: 0,
            tunnel_was_running: false,
            split_tunnel_whitelist_migrated: false,
            profiles: default_profiles(),
        }
    }
}

impl Settings {
    /// Clamps active profile index to valid bounds [0, profiles.len() - 1].
    pub fn active_profile_index(&self) -> usize {
        let max_idx = self.profiles.len().saturating_sub(1);
        (self.active_profile.max(0) as usize).min(max_idx)
    }

    /// Returns a reference to the active profile.
    pub fn current_profile(&self) -> &Profile {
        &self.profiles[self.active_profile_index()]
    }

    /// Returns a mutable reference to the active profile.
    pub fn current_profile_mut(&mut self) -> &mut Profile {
        let idx = self.active_profile_index();
        &mut self.profiles[idx]
    }

    /// Sets the active profile index bounded to [MIN_INDEX, MAX_INDEX].
    pub fn set_active_profile(&mut self, profile: i32) {
        self.active_profile = normalize_active_profile(profile);
    }

    /// Updates profile name according to SettingsStore rules: blank or default string removes custom name.
    pub fn save_profile_name(&mut self, profile: usize, name: &str) {
        if profile < self.profiles.len() {
            self.profiles[profile].profile_name =
                normalize_profile_name(profile, name).unwrap_or_default();
        }
    }

    /// Toggles manual port configuration. Disabling resets ports to defaults.
    pub fn set_manual_ports_enabled(&mut self, enabled: bool) {
        let p = self.current_profile_mut();
        p.manual_ports_enabled = enabled;
        if !enabled {
            p.deploy_ssh_port = crate::constants::network::DEFAULT_SSH_PORT.to_string();
            p.server_peer_port = crate::constants::network::DEFAULT_SERVER_PEER_PORT;
            p.server_web_port = crate::constants::network::DEFAULT_SERVER_WEB_PORT;
        }
    }

    /// Updates server peer port if within valid range (1..=65535).
    pub fn set_server_peer_port(&mut self, port: i32) -> bool {
        if (1..=65535).contains(&port) {
            self.current_profile_mut().server_peer_port = port;
            true
        } else {
            false
        }
    }

    /// Normalizes and updates workers per hash.
    pub fn set_workers_per_hash(&mut self, count: i32) {
        self.current_profile_mut().workers_per_hash = normalize_workers_per_hash(count);
    }

    /// Updates VK authentication mode and recalculates hash mode.
    pub fn set_vk_auth_mode(&mut self, mode: &str, has_vk_token: bool) {
        let p = self.current_profile_mut();
        let normalized = normalize_vk_auth_mode(mode);
        let next_hash_mode = crate::vk_mode_policy::vk_hash_mode_for_auth_mode(
            normalized,
            Some(&p.vk_hash_mode),
            has_vk_token,
        );
        p.vk_auth_mode = normalized.to_string();
        p.vk_hash_mode = next_hash_mode.to_string();
    }

    /// Updates VK hash mode and harmonizes authentication mode.
    pub fn set_vk_hash_mode(&mut self, mode: &str, has_vk_token: bool) {
        let p = self.current_profile_mut();
        let next_auth_mode =
            crate::vk_mode_policy::vk_auth_mode_for_hash_mode(mode, Some(&p.vk_auth_mode));
        let selection = crate::vk_mode_policy::normalize_vk_mode_selection(
            Some(next_auth_mode),
            Some(mode),
            has_vk_token,
        );
        p.vk_auth_mode = selection.auth_mode;
        p.vk_hash_mode = selection.hash_mode;
    }

    /// Selects a TURN subnet and refreshes custom TURN IP addresses.
    pub fn set_selected_turn_subnet(&mut self, subnet_id: &str, ip_limit: Option<&str>) {
        let p = self.current_profile_mut();
        let limit = ip_limit.unwrap_or(&p.turn_subnet_ip_limit);
        let ips = normalize_turn_subnet_ips(subnet_id, limit);
        p.selected_turn_subnet = subnet_id.to_string();
        p.custom_turn_ips = ips;
        if let Some(l) = ip_limit {
            p.turn_subnet_ip_limit = l.to_string();
        }
    }

    /// Updates direct domains and synchronizes the direct black domain list.
    pub fn set_direct_domains(&mut self, domains: &str) {
        let p = self.current_profile_mut();
        p.direct_domains = domains.to_string();
        p.direct_domains_black = domains.to_string();
    }

    /// Updates direct black domains and keeps direct_domains synchronized.
    pub fn set_direct_domains_black(&mut self, domains: &str) {
        let p = self.current_profile_mut();
        p.direct_domains_black = domains.to_string();
        p.direct_domains = domains.to_string();
    }

    /// Records formatted diagnostic code and message.
    pub fn set_tunnel_diagnostic(&mut self, code: &str, message: &str, timestamp_ms: i64) {
        self.tunnel_last_diagnostic = normalize_tunnel_diagnostic(code, message);
        self.tunnel_last_diagnostic_at = timestamp_ms;
    }

    /// Clears last recorded tunnel diagnostic.
    pub fn clear_tunnel_diagnostic(&mut self) {
        self.tunnel_last_diagnostic.clear();
        self.tunnel_last_diagnostic_at = 0;
    }

    /// Marks a specific VK hash as invalid in validation results.
    pub fn invalidate_vk_hash(&mut self, hash: &str) {
        let p = self.current_profile_mut();
        p.vk_hash_check_results =
            invalidate_vk_hash_results(&p.vk_hash_check_results, hash);
    }

    /// Advances connection generation counter monotonically.
    pub fn record_connection_generation(&mut self, generation: i64) -> i64 {
        let p = self.current_profile_mut();
        let persisted = p.connection_generation.max(generation.max(0));
        p.connection_generation = persisted;
        persisted
    }

    /// Reserves a new generation ID taking into account current timestamp and proposed generation ID.
    pub fn reserve_connection_generation(&mut self, proposed: i64, now_seconds: i64) -> i64 {
        let p = self.current_profile_mut();
        let reserved = crate::tunnel_lifecycle_state::reserve_tunnel_generation_id(
            now_seconds.max(0),
            proposed.max(0),
            p.connection_generation,
        );
        p.connection_generation = reserved;
        reserved
    }

    /// Builds a snapshot of deploy settings for the active profile.
    pub fn deploy_settings_snapshot(
        &self,
        secrets: &dyn Fn(SecretKey) -> Option<String>,
    ) -> DeploySettingsSnapshot {
        let p = self.current_profile();
        DeploySettingsSnapshot {
            profile: self.active_profile,
            ip: p.deploy_ip.clone(),
            ssh_login: p.deploy_login.clone(),
            ssh_password: secrets(SecretKey::DeployPassword).unwrap_or_default(),
            web_login: p.deploy_web_login.clone(),
            web_password: secrets(SecretKey::DeployWebPassword).unwrap_or_default(),
            main_password: secrets(SecretKey::DeployMainPassword).unwrap_or_default(),
            ssh_port: if p.deploy_ssh_port.trim().is_empty() {
                crate::constants::network::DEFAULT_SSH_PORT.to_string()
            } else {
                p.deploy_ssh_port.clone()
            },
            manual_ports_enabled: p.manual_ports_enabled,
            server_peer_port: p.server_peer_port,
            server_web_port: p.server_web_port,
            ssh_keys_mode: p.ssh_keys_mode,
            docker_install: p.docker_install,
            secret_path_enabled: p.deploy_secret_path_enabled,
            secret_path_uuid: p.deploy_secret_path_uuid.clone(),
            ssh_private_key: secrets(SecretKey::SshPrivateKey).unwrap_or_default(),
            ssh_key_passphrase: secrets(SecretKey::SshKeyPassphrase).unwrap_or_default(),
            ssh_certificate: secrets(SecretKey::SshCertificate).unwrap_or_default(),
        }
    }
}

/// Snapshot of deploy configuration mirroring Kotlin DeploySettingsSnapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploySettingsSnapshot {
    pub profile: i32,
    pub ip: String,
    pub ssh_login: String,
    pub ssh_password: String,
    pub web_login: String,
    pub web_password: String,
    pub main_password: String,
    pub ssh_port: String,
    pub manual_ports_enabled: bool,
    pub server_peer_port: i32,
    pub server_web_port: i32,
    pub ssh_keys_mode: bool,
    pub docker_install: bool,
    pub secret_path_enabled: bool,
    pub secret_path_uuid: String,
    pub ssh_private_key: String,
    pub ssh_key_passphrase: String,
    pub ssh_certificate: String,
}

impl Default for DeploySettingsSnapshot {
    fn default() -> Self {
        Self {
            profile: -1,
            ip: String::new(),
            ssh_login: String::new(),
            ssh_password: String::new(),
            web_login: String::new(),
            web_password: String::new(),
            main_password: String::new(),
            ssh_port: crate::constants::network::DEFAULT_SSH_PORT.to_string(),
            manual_ports_enabled: false,
            server_peer_port: crate::constants::network::DEFAULT_SERVER_PEER_PORT,
            server_web_port: crate::constants::network::DEFAULT_SERVER_WEB_PORT,
            ssh_keys_mode: false,
            docker_install: false,
            secret_path_enabled: true,
            secret_path_uuid: String::new(),
            ssh_private_key: String::new(),
            ssh_key_passphrase: String::new(),
            ssh_certificate: String::new(),
        }
    }
}

impl DeploySettingsSnapshot {
    pub fn is_loaded(&self) -> bool {
        self.profile >= 0
    }
}

// -----------------------------------------------------------------------------
// Validation and Normalisation Helpers
// -----------------------------------------------------------------------------

/// Normalizes active profile index to [MIN_INDEX, MAX_INDEX].
pub fn normalize_active_profile(profile: i32) -> i32 {
    profile.clamp(
        crate::constants::profiles::MIN_INDEX,
        crate::constants::profiles::MAX_INDEX,
    )
}

/// Normalizes custom profile name. Returns None if blank or equal to default name.
pub fn normalize_profile_name(profile_index: usize, name: &str) -> Option<String> {
    let trimmed = name.trim();
    let default_name = format!("Профиль {}", profile_index);
    if trimmed.is_empty() || trimmed == default_name {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Normalizes worker count using tunnel policy and upper bound.
pub fn normalize_workers_per_hash(workers: i32) -> i32 {
    crate::worker_count_policy::WorkerCountPolicy::normalize_with_maximum(
        workers,
        crate::constants::tunnel::MAX_WORKERS,
    )
}

/// Normalizes VK authentication mode string.
pub fn normalize_vk_auth_mode(mode: &str) -> &'static str {
    match mode {
        crate::constants::vk_auth::MODE_CAPTCHA => crate::constants::vk_auth::MODE_CAPTCHA,
        crate::constants::vk_auth::MODE_AUTO_JS => crate::constants::vk_auth::MODE_AUTO_JS,
        _ => crate::constants::vk_auth::MODE_CALLS,
    }
}

/// Normalizes obfuscation mode string.
pub fn normalize_obfs_mode(mode: &str) -> &'static str {
    if mode == "unified" {
        "unified"
    } else {
        crate::constants::tunnel::DEFAULT_OBFS_MODE
    }
}

/// Normalizes TURN transport protocol.
pub fn normalize_turn_transport(transport: &str) -> &'static str {
    match transport {
        crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS => {
            crate::constants::tunnel::TURN_TRANSPORT_TCP_TLS
        }
        _ => crate::constants::tunnel::DEFAULT_TURN_TRANSPORT,
    }
}

/// Normalizes SRTP mode identifier.
pub fn normalize_srtp_mode(mode: &str) -> &'static str {
    match mode {
        crate::constants::srtp_mode::WRAP => crate::constants::srtp_mode::WRAP,
        crate::constants::srtp_mode::DTLS => crate::constants::srtp_mode::DTLS,
        _ => crate::constants::srtp_mode::DEFAULT,
    }
}

/// Normalizes diagnostic string into "code|message" format with length clamping.
pub fn normalize_tunnel_diagnostic(code: &str, message: &str) -> String {
    let norm_code = code.trim();
    let norm_code_clamped: String = norm_code.chars().take(80).collect();
    let collapsed_message = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let norm_msg_clamped: String = collapsed_message.chars().take(400).collect();
    format!("{}|{}", norm_code_clamped, norm_msg_clamped)
}

/// Computes comma-separated TURN IPs for a subnet with an optional count limit.
pub fn normalize_turn_subnet_ips(subnet_id: &str, limit_str: &str) -> String {
    if subnet_id.trim().is_empty() {
        return String::new();
    }
    let all_ips = crate::turn_subnet_registry::get_ips_for_subnet(Some(subnet_id));
    if all_ips.is_empty() {
        return String::new();
    }
    let limit_count = limit_str.parse::<usize>().ok();
    match limit_count {
        Some(limit) if limit > 0 => all_ips.into_iter().take(limit).collect::<Vec<_>>().join(","),
        _ => all_ips.join(","),
    }
}

/// Decodes results JSON, invalidates given hash, and re-encodes.
pub fn invalidate_vk_hash_results(current_results_json: &str, hash: &str) -> String {
    let normalized = hash.trim();
    if normalized.is_empty() {
        return current_results_json.to_string();
    }
    let mut results = crate::vk_hash_validator::VkHashValidationCodec::decode(current_results_json);
    results.insert(
        normalized.to_string(),
        crate::vk_hash_validator::VkHashValidationStatus::Invalid,
    );
    crate::vk_hash_validator::VkHashValidationCodec::encode(&results)
}

// -----------------------------------------------------------------------------
// Config Export & Import (Android JSON format)
// -----------------------------------------------------------------------------

/// Result of importing a configuration JSON string.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedConfig {
    pub settings: Settings,
    pub secrets: Vec<(SecretKey, String)>,
}

fn escape_json(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

/// Exports current settings and secrets into an Android byte-compatible JSON string.
///
/// Builds keys in the exact order emitted by Kotlin's `exportConfigJson` with compact formatting
/// (no extra spaces) matching `org.json` serialization without indentation.
pub fn export_config_json(
    settings: &Settings,
    secrets: &dyn Fn(SecretKey) -> Option<String>,
) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    export_config_json_selective(settings, secrets, true, true, true, true, now_ms)
}

/// Exports configuration JSON with explicit section selectors and timestamp.
pub fn export_config_json_selective(
    settings: &Settings,
    secrets: &dyn Fn(SecretKey) -> Option<String>,
    include_tunnel: bool,
    include_deploy: bool,
    include_exceptions: bool,
    include_settings: bool,
    exported_at_ms: i64,
) -> String {
    let profile = settings.current_profile();
    let mut sections = Vec::new();

    sections.push("\"version\":1".to_string());
    sections.push(format!("\"exported_at\":{}", exported_at_ms));

    if include_tunnel {
        let mut tunnel = Vec::new();
        tunnel.push(format!("\"peer\":{}", escape_json(&profile.peer)));
        tunnel.push(format!("\"listen_port\":{}", profile.listen_port));
        tunnel.push(format!(
            "\"manual_ports_enabled\":{}",
            profile.manual_ports_enabled
        ));
        tunnel.push(format!(
            "\"server_peer_port\":{}",
            profile.server_peer_port
        ));
        let conn_pw = secrets(SecretKey::ConnectionPassword).unwrap_or_default();
        tunnel.push(format!("\"connection_password\":{}", escape_json(&conn_pw)));
        tunnel.push(format!("\"obfs_mode\":{}", escape_json(&profile.obfs_mode)));
        tunnel.push(format!(
            "\"turn_transport\":{}",
            escape_json(&profile.turn_transport)
        ));
        tunnel.push(format!("\"srtp_mode\":{}", escape_json(&profile.srtp_mode)));
        tunnel.push(format!(
            "\"custom_turn_ips\":{}",
            escape_json(&profile.custom_turn_ips)
        ));
        tunnel.push(format!("\"sni\":{}", escape_json(&profile.sni)));
        tunnel.push(format!("\"no_dns\":{}", profile.no_dns));
        sections.push(format!("\"tunnel\":{{{}}}", tunnel.join(",")));
    }

    if include_deploy {
        let mut deploy = Vec::new();
        deploy.push(format!("\"deploy_ip\":{}", escape_json(&profile.deploy_ip)));
        deploy.push(format!(
            "\"deploy_login\":{}",
            escape_json(&profile.deploy_login)
        ));
        deploy.push(format!(
            "\"deploy_ssh_port\":{}",
            escape_json(&profile.deploy_ssh_port)
        ));
        deploy.push(format!(
            "\"server_web_port\":{}",
            profile.server_web_port
        ));
        deploy.push(format!("\"docker_install\":{}", profile.docker_install));
        deploy.push(format!("\"ssh_keys_mode\":{}", profile.ssh_keys_mode));
        deploy.push(format!("\"vk_hashes\":{}", escape_json(&profile.vk_hashes)));
        deploy.push(format!(
            "\"secondary_vk_hash\":{}",
            escape_json(&profile.secondary_vk_hash)
        ));
        deploy.push(format!(
            "\"vk_auth_mode\":{}",
            escape_json(&profile.vk_auth_mode)
        ));
        deploy.push(format!(
            "\"vk_hash_mode\":{}",
            escape_json(&profile.vk_hash_mode)
        ));
        let vk_token = secrets(SecretKey::VkAccessToken).unwrap_or_default();
        deploy.push(format!("\"vk_access_token\":{}", escape_json(&vk_token)));
        deploy.push(format!(
            "\"workers_per_hash\":{}",
            profile.workers_per_hash
        ));
        deploy.push(format!("\"extra_workers\":{}", profile.extra_workers));
        deploy.push(format!("\"csqtt_link\":{}", escape_json(&profile.csqtt_link)));
        deploy.push(format!(
            "\"csqtt_link_mode\":{}",
            profile.csqtt_link_mode
        ));
        deploy.push(format!(
            "\"secret_path_enabled\":{}",
            profile.deploy_secret_path_enabled
        ));
        deploy.push(format!(
            "\"secret_path_uuid\":{}",
            escape_json(&profile.deploy_secret_path_uuid)
        ));
        sections.push(format!("\"deploy\":{{{}}}", deploy.join(",")));
    }

    if include_exceptions {
        let mut exc = Vec::new();
        exc.push(format!(
            "\"excluded_apps\":{}",
            escape_json(&profile.excluded_apps)
        ));
        exc.push(format!(
            "\"whitelist_apps\":{}",
            escape_json(&profile.whitelist_apps)
        ));
        exc.push(format!("\"is_whitelist\":{}", profile.is_whitelist));
        exc.push(format!(
            "\"domain_routing_enabled\":{}",
            profile.domain_routing_enabled
        ));
        exc.push(format!(
            "\"domain_is_whitelist\":{}",
            profile.domain_is_whitelist
        ));
        exc.push(format!(
            "\"exclude_ru_zones_enabled\":{}",
            profile.exclude_ru_zones_enabled
        ));
        exc.push(format!(
            "\"template_domains_enabled\":{}",
            profile.template_domains_enabled
        ));
        let direct_black = if !profile.direct_domains_black.is_empty() {
            &profile.direct_domains_black
        } else {
            &profile.direct_domains
        };
        exc.push(format!(
            "\"direct_domains_black\":{}",
            escape_json(direct_black)
        ));
        exc.push(format!(
            "\"direct_domains_white\":{}",
            escape_json(&profile.direct_domains_white)
        ));
        exc.push(format!(
            "\"ip_routing_enabled\":{}",
            profile.ip_routing_enabled
        ));
        exc.push(format!("\"ip_is_whitelist\":{}", profile.ip_is_whitelist));
        exc.push(format!(
            "\"direct_ips_black\":{}",
            escape_json(&profile.direct_ips_black)
        ));
        exc.push(format!(
            "\"direct_ips_white\":{}",
            escape_json(&profile.direct_ips_white)
        ));
        sections.push(format!("\"exceptions\":{{{}}}", exc.join(",")));
    }

    if include_settings {
        let mut set = Vec::new();
        set.push(format!("\"theme_mode\":{}", escape_json(&settings.theme_mode)));
        set.push(format!(
            "\"is_dynamic_color\":{}",
            settings.is_dynamic_color
        ));
        set.push(format!(
            "\"theme_palette\":{}",
            escape_json(&settings.theme_palette)
        ));
        set.push(format!(
            "\"auto_pause_on_wifi\":{}",
            settings.auto_pause_on_wifi
        ));
        set.push(format!(
            "\"selected_fingerprint\":{}",
            escape_json(&profile.selected_fingerprint)
        ));
        set.push(format!(
            "\"selected_turn_subnet\":{}",
            escape_json(&profile.selected_turn_subnet)
        ));
        set.push(format!(
            "\"turn_subnet_ip_limit\":{}",
            escape_json(&profile.turn_subnet_ip_limit)
        ));
        set.push(format!(
            "\"logging_enabled\":{}",
            settings.logging_enabled
        ));
        set.push(format!("\"detailed_logs\":{}", profile.detailed_logs));
        set.push(format!(
            "\"participant_show_logs\":{}",
            profile.participant_show_logs
        ));
        sections.push(format!("\"settings\":{{{}}}", set.join(",")));
    }

    format!("{{{}}}", sections.join(","))
}

fn val_as_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn val_as_i32(v: &serde_json::Value) -> Option<i32> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().map(|x| x as i32),
        serde_json::Value::String(s) => s.parse::<i32>().ok(),
        _ => None,
    }
}

fn val_as_bool(v: &serde_json::Value) -> Option<bool> {
    match v {
        serde_json::Value::Bool(b) => Some(*b),
        serde_json::Value::String(s) => s.parse::<bool>().ok(),
        _ => None,
    }
}

/// Imports configuration JSON onto a copy of `base`, returning updated settings and extracted secrets.
pub fn import_config_json(json: &str, base: &Settings) -> Result<ImportedConfig, String> {
    import_config_json_selective(json, base, true, true, true, true)
}

/// Imports configuration JSON with selective section inclusion.
pub fn import_config_json_selective(
    json: &str,
    base: &Settings,
    include_tunnel: bool,
    include_deploy: bool,
    include_exceptions: bool,
    include_settings: bool,
) -> Result<ImportedConfig, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {e}"))?;
    let root = parsed
        .as_object()
        .ok_or_else(|| "JSON root must be an object".to_string())?;

    let mut settings = base.clone();
    let mut secrets = Vec::new();

    if include_tunnel {
        if let Some(tunnel) = root.get("tunnel").and_then(|v| v.as_object()) {
            let p = settings.current_profile_mut();
            if let Some(v) = tunnel.get("peer").and_then(val_as_string) {
                p.peer = v;
            }
            if let Some(v) = tunnel.get("listen_port").and_then(val_as_i32) {
                p.listen_port = v;
            }
            if let Some(v) = tunnel.get("manual_ports_enabled").and_then(val_as_bool) {
                p.manual_ports_enabled = v;
            }
            if let Some(v) = tunnel.get("server_peer_port").and_then(val_as_i32) {
                p.server_peer_port = v;
            }
            if let Some(v) = tunnel.get("connection_password").and_then(val_as_string) {
                secrets.push((SecretKey::ConnectionPassword, v));
            }
            if let Some(v) = tunnel.get("obfs_mode").and_then(val_as_string) {
                p.obfs_mode = v;
            }
            if let Some(v) = tunnel.get("turn_transport").and_then(val_as_string) {
                p.turn_transport = v;
            }
            if let Some(v) = tunnel.get("srtp_mode").and_then(val_as_string) {
                p.srtp_mode = v;
            }
            if let Some(v) = tunnel.get("custom_turn_ips").and_then(val_as_string) {
                p.custom_turn_ips = v;
            }
            if let Some(v) = tunnel.get("sni").and_then(val_as_string) {
                p.sni = v;
            }
            if let Some(v) = tunnel.get("no_dns").and_then(val_as_bool) {
                p.no_dns = v;
            }
        }
    }

    if include_deploy {
        if let Some(deploy) = root.get("deploy").and_then(|v| v.as_object()) {
            let p = settings.current_profile_mut();
            if let Some(v) = deploy.get("deploy_ip").and_then(val_as_string) {
                p.deploy_ip = v;
            }
            if let Some(v) = deploy.get("deploy_login").and_then(val_as_string) {
                p.deploy_login = v;
            }
            if let Some(v) = deploy.get("deploy_ssh_port").and_then(val_as_string) {
                p.deploy_ssh_port = v;
            }
            if let Some(v) = deploy.get("server_web_port").and_then(val_as_i32) {
                p.server_web_port = v;
            }
            if let Some(v) = deploy.get("docker_install").and_then(val_as_bool) {
                p.docker_install = v;
            }
            if let Some(v) = deploy.get("ssh_keys_mode").and_then(val_as_bool) {
                p.ssh_keys_mode = v;
            }
            if let Some(v) = deploy.get("vk_hashes").and_then(val_as_string) {
                p.vk_hashes = v;
            }
            if let Some(v) = deploy.get("secondary_vk_hash").and_then(val_as_string) {
                p.secondary_vk_hash = v;
            }
            if let Some(v) = deploy.get("vk_auth_mode").and_then(val_as_string) {
                p.vk_auth_mode = v;
            }
            if let Some(v) = deploy.get("vk_hash_mode").and_then(val_as_string) {
                p.vk_hash_mode = v;
            }
            if let Some(v) = deploy.get("vk_access_token").and_then(val_as_string) {
                secrets.push((SecretKey::VkAccessToken, v));
            }
            if let Some(v) = deploy.get("workers_per_hash").and_then(val_as_i32) {
                p.workers_per_hash = v;
            }
            if let Some(v) = deploy.get("extra_workers").and_then(val_as_bool) {
                p.extra_workers = v;
            }
            if let Some(v) = deploy.get("csqtt_link").and_then(val_as_string) {
                p.csqtt_link = v;
            }
            if let Some(v) = deploy.get("csqtt_link_mode").and_then(val_as_bool) {
                p.csqtt_link_mode = v;
            }
            if let Some(v) = deploy.get("secret_path_enabled").and_then(val_as_bool) {
                p.deploy_secret_path_enabled = v;
            }
            if let Some(v) = deploy.get("secret_path_uuid").and_then(val_as_string) {
                p.deploy_secret_path_uuid = v;
            }
        }
    }

    if include_exceptions {
        if let Some(exc) = root.get("exceptions").and_then(|v| v.as_object()) {
            let p = settings.current_profile_mut();
            if let Some(v) = exc.get("excluded_apps").and_then(val_as_string) {
                p.excluded_apps = v;
            }
            if let Some(v) = exc.get("whitelist_apps").and_then(val_as_string) {
                p.whitelist_apps = v;
            }
            if let Some(v) = exc.get("is_whitelist").and_then(val_as_bool) {
                p.is_whitelist = v;
            }
            if let Some(v) = exc.get("domain_routing_enabled").and_then(val_as_bool) {
                p.domain_routing_enabled = v;
            }
            if let Some(v) = exc.get("domain_is_whitelist").and_then(val_as_bool) {
                p.domain_is_whitelist = v;
            }
            if let Some(v) = exc.get("exclude_ru_zones_enabled").and_then(val_as_bool) {
                p.exclude_ru_zones_enabled = v;
            }
            if let Some(v) = exc.get("template_domains_enabled").and_then(val_as_bool) {
                p.template_domains_enabled = v;
            }
            if let Some(v) = exc.get("direct_domains_black").and_then(val_as_string) {
                p.direct_domains_black = v;
            }
            if let Some(v) = exc.get("direct_domains_white").and_then(val_as_string) {
                p.direct_domains_white = v;
            }
            if let Some(v) = exc.get("ip_routing_enabled").and_then(val_as_bool) {
                p.ip_routing_enabled = v;
            }
            if let Some(v) = exc.get("ip_is_whitelist").and_then(val_as_bool) {
                p.ip_is_whitelist = v;
            }
            if let Some(v) = exc.get("direct_ips_black").and_then(val_as_string) {
                p.direct_ips_black = v;
            }
            if let Some(v) = exc.get("direct_ips_white").and_then(val_as_string) {
                p.direct_ips_white = v;
            }
        }
    }

    if include_settings {
        if let Some(s) = root.get("settings").and_then(|v| v.as_object()) {
            if let Some(v) = s.get("theme_mode").and_then(val_as_string) {
                settings.theme_mode = v;
            }
            if let Some(v) = s.get("is_dynamic_color").and_then(val_as_bool) {
                settings.is_dynamic_color = v;
            }
            if let Some(v) = s.get("theme_palette").and_then(val_as_string) {
                settings.theme_palette = v;
            }
            if let Some(v) = s.get("auto_pause_on_wifi").and_then(val_as_bool) {
                settings.auto_pause_on_wifi = v;
            }
            if let Some(v) = s.get("logging_enabled").and_then(val_as_bool) {
                settings.logging_enabled = v;
            }
            let p = settings.current_profile_mut();
            if let Some(v) = s.get("selected_fingerprint").and_then(val_as_string) {
                p.selected_fingerprint = v;
            }
            if let Some(v) = s.get("selected_turn_subnet").and_then(val_as_string) {
                p.selected_turn_subnet = v;
            }
            if let Some(v) = s.get("turn_subnet_ip_limit").and_then(val_as_string) {
                p.turn_subnet_ip_limit = v;
            }
            if let Some(v) = s.get("detailed_logs").and_then(val_as_bool) {
                p.detailed_logs = v;
            }
            if let Some(v) = s.get("participant_show_logs").and_then(val_as_bool) {
                p.participant_show_logs = v;
            }
        }
    }

    Ok(ImportedConfig { settings, secrets })
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let s = Settings::default();
        assert_eq!(s.active_profile, 0);
        assert!(!s.show_system_apps);
        assert!(s.logging_enabled);
        assert!(!s.info_actions_expanded);
        assert!(!s.info_project_expanded);
        assert!(!s.auto_js_risk_acknowledged);
        assert!(!s.tcp_transport_risk_acknowledged);
        assert!(!s.battery_optimization_prompt_handled);
        assert!(!s.auto_pause_on_wifi);
        assert_eq!(s.theme_mode, "system");
        assert!(!s.is_dynamic_color);
        assert_eq!(s.theme_palette, "indigo");
        assert_eq!(s.update_check_interval_hours, 24);
        assert_eq!(s.client_id_check_results, "{}");
        assert_eq!(s.tunnel_last_diagnostic, "");
        assert!(!s.tunnel_was_running);
        assert_eq!(s.profiles.len(), 4);

        let p0 = s.current_profile();
        assert_eq!(p0.peer, "");
        assert_eq!(p0.workers_per_hash, 18);
        assert_eq!(p0.protocol, "udp");
        assert_eq!(p0.listen_port, 0);
        assert!(!p0.manual_ports_enabled);
        assert_eq!(p0.server_peer_port, 46000);
        assert_eq!(p0.server_web_port, 46002);
        assert_eq!(p0.vk_auth_mode, "vkcalls");
        assert_eq!(p0.obfs_mode, "unified");
        assert_eq!(p0.turn_transport, "udp");
        assert_eq!(p0.srtp_mode, "auto");
        assert_eq!(p0.captcha_mode, "auto");
        assert_eq!(p0.turn_subnet_ip_limit, "ALL");
        assert_eq!(p0.selected_fingerprint, "firefox");
        assert_eq!(p0.active_client_ids, "8202606,6287487");
        assert!(!p0.csqtt_link_mode);
        assert_eq!(p0.connection_generation, 0);
    }

    #[test]
    fn test_secret_key_storage_names() {
        assert_eq!(
            SecretKey::ConnectionPassword.storage_name(),
            "connection_password_encrypted"
        );
        assert_eq!(
            SecretKey::VkAccessToken.storage_name(),
            "vk_access_token_encrypted"
        );
        assert_eq!(
            SecretKey::DeployPassword.storage_name(),
            "deploy_password_encrypted"
        );
        assert_eq!(
            SecretKey::DeployMainPassword.storage_name(),
            "deploy_main_password_encrypted"
        );
        assert_eq!(
            SecretKey::DeployWebPassword.storage_name(),
            "deploy_web_password_encrypted"
        );
        assert_eq!(
            SecretKey::SshPrivateKey.storage_name(),
            "ssh_private_key_encrypted"
        );
        assert_eq!(
            SecretKey::SshKeyPassphrase.storage_name(),
            "ssh_key_passphrase_encrypted"
        );
        assert_eq!(
            SecretKey::SshCertificate.storage_name(),
            "ssh_certificate_encrypted"
        );
    }

    #[test]
    fn test_export_import_round_trip() {
        let mut base = Settings::default();
        {
            let p = base.current_profile_mut();
            p.peer = "example.com:443".to_string();
            p.listen_port = 1080;
            p.manual_ports_enabled = true;
            p.server_peer_port = 45000;
            p.obfs_mode = "unified".to_string();
            p.turn_transport = "tcp_tls".to_string();
            p.deploy_ip = "192.168.1.100".to_string();
            p.deploy_login = "admin".to_string();
            p.workers_per_hash = 20;
            p.excluded_apps = "app1,app2".to_string();
            p.direct_domains_black = "blocked.org".to_string();
        }
        base.theme_mode = "dark".to_string();

        let secrets_fn = |key: SecretKey| match key {
            SecretKey::ConnectionPassword => Some("my-secret-pass".to_string()),
            SecretKey::VkAccessToken => Some("vk-token-xyz".to_string()),
            _ => None,
        };

        let exported = export_config_json_selective(
            &base,
            &secrets_fn,
            true,
            true,
            true,
            true,
            1_700_000_000_000,
        );

        assert!(exported.starts_with("{\"version\":1,\"exported_at\":1700000000000,"));
        assert!(exported.contains("\"connection_password\":\"my-secret-pass\""));
        assert!(exported.contains("\"vk_access_token\":\"vk-token-xyz\""));

        let initial = Settings::default();
        let imported = import_config_json(&exported, &initial).expect("import succeeds");

        let imp_p = imported.settings.current_profile();
        assert_eq!(imp_p.peer, "example.com:443");
        assert_eq!(imp_p.listen_port, 1080);
        assert!(imp_p.manual_ports_enabled);
        assert_eq!(imp_p.server_peer_port, 45000);
        assert_eq!(imp_p.obfs_mode, "unified");
        assert_eq!(imp_p.turn_transport, "tcp_tls");
        assert_eq!(imp_p.deploy_ip, "192.168.1.100");
        assert_eq!(imp_p.deploy_login, "admin");
        assert_eq!(imp_p.workers_per_hash, 20);
        assert_eq!(imp_p.excluded_apps, "app1,app2");
        assert_eq!(imp_p.direct_domains_black, "blocked.org");
        assert_eq!(imported.settings.theme_mode, "dark");

        assert_eq!(
            imported.secrets,
            vec![
                (
                    SecretKey::ConnectionPassword,
                    "my-secret-pass".to_string()
                ),
                (SecretKey::VkAccessToken, "vk-token-xyz".to_string()),
            ]
        );
    }

    #[test]
    fn test_import_android_style_json_sample() {
        // Construct realistic sample formatted with standard 2-space Android org.json output
        let json_sample = r#"{
  "version": 1,
  "exported_at": 1710000000000,
  "tunnel": {
    "peer": "android.test.csqtt:8443",
    "listen_port": 12345,
    "manual_ports_enabled": true,
    "server_peer_port": 46500,
    "connection_password": "android_password_123",
    "obfs_mode": "unified",
    "turn_transport": "tcp_tls",
    "srtp_mode": "wrap-srtp",
    "custom_turn_ips": "1.2.3.4,5.6.7.8",
    "sni": "my-sni.com",
    "no_dns": true
  },
  "deploy": {
    "deploy_ip": "10.0.0.1",
    "deploy_login": "root",
    "deploy_ssh_port": "2222",
    "server_web_port": 46002,
    "docker_install": true,
    "ssh_keys_mode": false,
    "vk_hashes": "hash_a,hash_b",
    "secondary_vk_hash": "hash_sec",
    "vk_auth_mode": "vkcalls",
    "vk_hash_mode": "manual",
    "vk_access_token": "token_android_sample",
    "workers_per_hash": 30,
    "extra_workers": false,
    "csqtt_link": "csqtt://test",
    "csqtt_link_mode": true,
    "secret_path_enabled": true,
    "secret_path_uuid": "f47ac10b-58cc-4372-a567-0e02b2c3d479"
  },
  "exceptions": {
    "excluded_apps": "com.android.chrome",
    "whitelist_apps": "org.telegram.messenger",
    "is_whitelist": true,
    "domain_routing_enabled": true,
    "domain_is_whitelist": false,
    "exclude_ru_zones_enabled": true,
    "template_domains_enabled": true,
    "direct_domains_black": "domain1.com,domain2.com",
    "direct_domains_white": "white1.com",
    "ip_routing_enabled": true,
    "ip_is_whitelist": true,
    "direct_ips_black": "8.8.8.8",
    "direct_ips_white": "1.1.1.1"
  },
  "settings": {
    "theme_mode": "dark",
    "is_dynamic_color": true,
    "theme_palette": "forest",
    "auto_pause_on_wifi": true,
    "selected_fingerprint": "chrome",
    "selected_turn_subnet": "95_163_34_0_24",
    "turn_subnet_ip_limit": "10",
    "logging_enabled": false,
    "detailed_logs": true,
    "participant_show_logs": true
  }
}"#;

        let base = Settings::default();
        let result = import_config_json(json_sample, &base).expect("valid sample must parse");
        let p = result.settings.current_profile();

        assert_eq!(p.peer, "android.test.csqtt:8443");
        assert_eq!(p.listen_port, 12345);
        assert!(p.manual_ports_enabled);
        assert_eq!(p.server_peer_port, 46500);
        assert_eq!(p.obfs_mode, "unified");
        assert_eq!(p.turn_transport, "tcp_tls");
        assert_eq!(p.srtp_mode, "wrap-srtp");
        assert_eq!(p.custom_turn_ips, "1.2.3.4,5.6.7.8");
        assert_eq!(p.sni, "my-sni.com");
        assert!(p.no_dns);

        assert_eq!(p.deploy_ip, "10.0.0.1");
        assert_eq!(p.deploy_login, "root");
        assert_eq!(p.deploy_ssh_port, "2222");
        assert_eq!(p.server_web_port, 46002);
        assert!(p.docker_install);
        assert_eq!(p.vk_hashes, "hash_a,hash_b");
        assert_eq!(p.secondary_vk_hash, "hash_sec");
        assert_eq!(p.workers_per_hash, 30);
        assert!(p.csqtt_link_mode);
        assert_eq!(p.csqtt_link, "csqtt://test");
        assert!(p.deploy_secret_path_enabled);
        assert_eq!(
            p.deploy_secret_path_uuid,
            "f47ac10b-58cc-4372-a567-0e02b2c3d479"
        );

        assert_eq!(p.excluded_apps, "com.android.chrome");
        assert_eq!(p.whitelist_apps, "org.telegram.messenger");
        assert!(p.is_whitelist);
        assert!(p.domain_routing_enabled);
        assert!(!p.domain_is_whitelist);
        assert!(p.exclude_ru_zones_enabled);
        assert_eq!(p.direct_domains_black, "domain1.com,domain2.com");
        assert_eq!(p.direct_domains_white, "white1.com");

        assert_eq!(result.settings.theme_mode, "dark");
        assert!(result.settings.is_dynamic_color);
        assert_eq!(result.settings.theme_palette, "forest");
        assert!(result.settings.auto_pause_on_wifi);
        assert!(!result.settings.logging_enabled);

        assert_eq!(p.selected_fingerprint, "chrome");
        assert_eq!(p.selected_turn_subnet, "95_163_34_0_24");
        assert_eq!(p.turn_subnet_ip_limit, "10");
        assert!(p.detailed_logs);
        assert!(p.participant_show_logs);

        assert_eq!(
            result.secrets,
            vec![
                (
                    SecretKey::ConnectionPassword,
                    "android_password_123".to_string()
                ),
                (
                    SecretKey::VkAccessToken,
                    "token_android_sample".to_string()
                ),
            ]
        );
    }

    #[test]
    fn test_unknown_keys_ignored() {
        let json = r#"{
  "version": 1,
  "exported_at": 1000,
  "unknown_root_key": "some_value",
  "tunnel": {
    "peer": "vpn.org",
    "future_tunnel_flag": 999
  },
  "deploy": {
    "random_extra_prop": [1, 2, 3]
  }
}"#;
        let base = Settings::default();
        let res = import_config_json(json, &base);
        assert!(res.is_ok(), "unknown keys must not cause errors");
        let imported = res.unwrap();
        assert_eq!(imported.settings.current_profile().peer, "vpn.org");
    }

    #[test]
    fn test_invalid_json_returns_err() {
        let base = Settings::default();
        assert!(import_config_json("not valid json", &base).is_err());
        assert!(import_config_json("{ unquoted_broken ", &base).is_err());
        assert!(import_config_json("[1, 2, 3]", &base).is_err());
    }

    #[test]
    fn test_normalize_profile_name() {
        assert_eq!(normalize_profile_name(0, "  "), None);
        assert_eq!(normalize_profile_name(0, "Профиль 0"), None);
        assert_eq!(normalize_profile_name(1, "Профиль 1"), None);
        assert_eq!(
            normalize_profile_name(0, "Work"),
            Some("Work".to_string())
        );
        assert_eq!(
            normalize_profile_name(2, "  Custom Name  "),
            Some("Custom Name".to_string())
        );
    }

    #[test]
    fn test_normalize_workers_per_hash() {
        assert_eq!(normalize_workers_per_hash(0), 5);
        assert_eq!(normalize_workers_per_hash(4), 5);
        assert_eq!(normalize_workers_per_hash(5), 5);
        assert_eq!(normalize_workers_per_hash(14), 10);
        assert_eq!(normalize_workers_per_hash(18), 10);
        assert_eq!(normalize_workers_per_hash(20), 20);
        assert_eq!(normalize_workers_per_hash(999), 100);
    }

    #[test]
    fn test_normalize_active_profile() {
        assert_eq!(normalize_active_profile(-5), 0);
        assert_eq!(normalize_active_profile(0), 0);
        assert_eq!(normalize_active_profile(2), 2);
        assert_eq!(normalize_active_profile(3), 3);
        assert_eq!(normalize_active_profile(4), 3);
    }

    #[test]
    fn test_manual_ports_toggle_resets_defaults() {
        let mut s = Settings::default();
        s.set_manual_ports_enabled(true);
        s.current_profile_mut().server_peer_port = 55555;
        s.current_profile_mut().server_web_port = 55556;
        s.current_profile_mut().deploy_ssh_port = "2200".to_string();

        assert_eq!(s.current_profile().server_peer_port, 55555);

        s.set_manual_ports_enabled(false);
        assert_eq!(s.current_profile().server_peer_port, 46000);
        assert_eq!(s.current_profile().server_web_port, 46002);
        assert_eq!(s.current_profile().deploy_ssh_port, "22");
    }

    #[test]
    fn test_server_peer_port_validation() {
        let mut s = Settings::default();
        assert!(s.set_server_peer_port(8080));
        assert_eq!(s.current_profile().server_peer_port, 8080);
        assert!(!s.set_server_peer_port(0));
        assert_eq!(s.current_profile().server_peer_port, 8080);
        assert!(!s.set_server_peer_port(65536));
        assert_eq!(s.current_profile().server_peer_port, 8080);
        assert!(s.set_server_peer_port(65535));
        assert_eq!(s.current_profile().server_peer_port, 65535);
    }

    #[test]
    fn test_tunnel_diagnostic_normalization() {
        let norm = normalize_tunnel_diagnostic("ERR_TUNNEL", "multiple   spaces   and\nnewlines");
        assert_eq!(norm, "ERR_TUNNEL|multiple spaces and newlines");

        let long_code = "a".repeat(100);
        let long_msg = "b".repeat(500);
        let clamped = normalize_tunnel_diagnostic(&long_code, &long_msg);
        let parts: Vec<&str> = clamped.split('|').collect();
        assert_eq!(parts[0].len(), 80);
        assert_eq!(parts[1].len(), 400);
    }

    #[test]
    fn test_turn_subnet_ips_normalization() {
        let ips_all = normalize_turn_subnet_ips("185_180_200_0_22", "ALL");
        assert_eq!(ips_all, "185.180.202.14,185.180.202.15");

        let ips_limited = normalize_turn_subnet_ips("185_180_200_0_22", "1");
        assert_eq!(ips_limited, "185.180.202.14");

        let empty = normalize_turn_subnet_ips("", "ALL");
        assert_eq!(empty, "");
    }

    #[test]
    fn test_invalidate_vk_hash() {
        let initial = "{}";
        let updated = invalidate_vk_hash_results(initial, "0123456789abcdef");
        assert!(updated.contains("\"0123456789abcdef\":\"invalid\""));
    }

    #[test]
    fn test_deploy_settings_snapshot() {
        let mut s = Settings::default();
        s.current_profile_mut().deploy_ip = "10.0.0.5".to_string();
        let snapshot = s.deploy_settings_snapshot(&|k| match k {
            SecretKey::DeployPassword => Some("ssh-pass".to_string()),
            _ => None,
        });
        assert!(snapshot.is_loaded());
        assert_eq!(snapshot.ip, "10.0.0.5");
        assert_eq!(snapshot.ssh_password, "ssh-pass");
        assert_eq!(snapshot.ssh_port, "22");
    }
}
