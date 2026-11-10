//! Core command line, env and stdin commands (port of TunnelManager.kt).
use std::path::PathBuf;

/// Everything needed to start the core (mirrors TunnelParams in TunnelManager.kt).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CoreParams {
    /// VK access token for VK_JS_BOOTSTRAP (auto_js hash mode); not passed on the command line.
    pub vk_access_token: String,
    pub peer: String,
    pub vk_hashes: Vec<String>,
    pub workers_per_hash: u32,
    pub port: u32,
    pub connection_password: String,
    pub vk_auth_mode: String,
    pub captcha_mode: String,
    pub fingerprint: String,
    pub client_ids: String,
    pub obfs_mode: String,
    pub turn_transport: String,
    pub srtp_mode: String,
    pub generation_id: u64,
    pub session_salt: String,
    pub allow_hash_redistribution: bool,
    pub vk_hash_mode: String,
    pub custom_turn_ips: String,
    pub domain_routing_enabled: bool,
    pub domain_is_whitelist: bool,
    pub exclude_ru_zones: bool,
    pub template_domains: bool,
    pub template_domains_file: PathBuf,
    pub direct_domains_file: PathBuf,
    pub ip_routing_enabled: bool,
    pub ip_is_whitelist: bool,
    pub direct_ips: String,
    /// Windows only: executables whose traffic bypasses the tunnel (file names or full paths).
    pub excluded_apps: Vec<String>,
}

/// CLI args exactly as TunnelManager.kt builds them (same order and flags), but `-tun-uds <tun_pipe>`
/// carries the Windows named pipe name and paths are Windows paths.
pub fn build_args(params: &CoreParams, tun_pipe: &str) -> Vec<String> {
    const MAX_VK_HASHES: usize = 7;
    let hash_list: Vec<String> = params
        .vk_hashes
        .iter()
        .flat_map(|s| s.split(|c: char| c == ',' || c.is_whitespace()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(MAX_VK_HASHES)
        .collect();

    let js_hash_mode = params.vk_hash_mode == "auto_js";
    let js_auth_mode = params.vk_auth_mode == "auto_js";
    let effective_auth_mode = if js_auth_mode && !js_hash_mode {
        "vkcalls"
    } else {
        params.vk_auth_mode.as_str()
    };

    // WorkerCountPolicy.normalizeForHashValues, as in TunnelManager.kt.
    let total_workers = csqtt_logic::worker_count_policy::WorkerCountPolicy::normalize_for_hash_values(
        i32::try_from(params.workers_per_hash).unwrap_or(i32::MAX),
        &hash_list,
        params.allow_hash_redistribution || js_hash_mode,
    );

    let mut cmd = vec![
        "-peer".to_string(),
        params.peer.clone(),
        "-n".to_string(),
        total_workers.to_string(),
        "-listen".to_string(),
        format!("127.0.0.1:{}", params.port),
        "-tun-uds".to_string(),
        tun_pipe.to_string(),
    ];

    if !hash_list.is_empty() {
        cmd.push("-vk".to_string());
        cmd.push(hash_list.join(","));
    }

    cmd.push("-vk-hash-mode".to_string());
    cmd.push(params.vk_hash_mode.clone());

    if !params.fingerprint.is_empty() {
        cmd.push("-fingerprint".to_string());
        cmd.push(params.fingerprint.clone());
    }

    if !params.client_ids.is_empty() {
        cmd.push("-client-ids".to_string());
        cmd.push(params.client_ids.clone());
    }

    cmd.push("-obfs".to_string());
    cmd.push(params.obfs_mode.clone());

    cmd.push("-turn-transport".to_string());
    cmd.push(params.turn_transport.clone());

    cmd.push("-srtp-mode".to_string());
    cmd.push(params.srtp_mode.clone());

    if !params.custom_turn_ips.trim().is_empty() {
        cmd.push("-custom-turn-ips".to_string());
        cmd.push(params.custom_turn_ips.clone());
    }

    cmd.push("-vk-auth-mode".to_string());
    cmd.push(effective_auth_mode.to_string());

    if params.generation_id > 0 {
        cmd.push("-gen".to_string());
        cmd.push(params.generation_id.to_string());
    }

    if !params.session_salt.is_empty() {
        cmd.push("-salt".to_string());
        cmd.push(params.session_salt.clone());
    }

    cmd.push("-captcha-mode".to_string());
    cmd.push(params.captcha_mode.clone());

    let has_direct_domains = !params.direct_domains_file.as_os_str().is_empty();
    let is_domain_routing_active = params.domain_routing_enabled
        || params.exclude_ru_zones
        || params.template_domains
        || has_direct_domains;

    if is_domain_routing_active {
        cmd.push("--domain-routing".to_string());
        if params.domain_is_whitelist {
            cmd.push("--domain-whitelist".to_string());
        }
        if params.exclude_ru_zones && !params.domain_is_whitelist {
            cmd.push("--exclude-ru-zones".to_string());
        }
        if params.template_domains && !params.domain_is_whitelist {
            cmd.push("--template-domains".to_string());
            if !params.template_domains_file.as_os_str().is_empty() {
                cmd.push("--domains-file".to_string());
                cmd.push(params.template_domains_file.to_string_lossy().to_string());
            }
        }
        if has_direct_domains {
            cmd.push("--domains-file".to_string());
            cmd.push(params.direct_domains_file.to_string_lossy().to_string());
        }
    }

    let has_direct_ips = !params.direct_ips.trim().is_empty();
    if params.ip_routing_enabled || has_direct_ips {
        cmd.push("--ip-routing".to_string());
        if params.ip_is_whitelist {
            cmd.push("--ip-whitelist".to_string());
        }
        if has_direct_ips {
            cmd.push("--direct-ips".to_string());
            cmd.push(params.direct_ips.clone());
        }
    }

    cmd
}

/// Env vars: CSQTT_EVENTS=1, RAYON_NUM_THREADS=2, CSQTT_PASSWORD, CSQTT_BYPASS_IFINDEX when Some,
/// CSQTT_EXCLUDED_APPS ('|'-separated) when excluded_apps is not empty.
pub fn build_env(params: &CoreParams, bypass_ifindex: Option<u32>) -> Vec<(String, String)> {
    let mut env = vec![
        ("CSQTT_EVENTS".to_string(), "1".to_string()),
        ("RAYON_NUM_THREADS".to_string(), "2".to_string()),
    ];
    if !params.connection_password.is_empty() {
        env.push(("CSQTT_PASSWORD".to_string(), params.connection_password.clone()));
    }
    if let Some(ifindex) = bypass_ifindex {
        env.push(("CSQTT_BYPASS_IFINDEX".to_string(), ifindex.to_string()));
    }
    if !params.excluded_apps.is_empty() {
        env.push(("CSQTT_EXCLUDED_APPS".to_string(), params.excluded_apps.join("|")));
    }
    env
}

/// Host -> core stdin commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreCommand {
    Stop,
    FinishVkCalls,
    Pause,
    Resume,
    CaptchaResult(String),
    VkJsBootstrap(String),
    UpdateDomains(String),
}

impl CoreCommand {
    /// The exact stdin line including the trailing '\n'.
    pub fn line(&self) -> String {
        match self {
            CoreCommand::Stop => "STOP\n".to_string(),
            CoreCommand::FinishVkCalls => "FINISH_VK_CALLS\n".to_string(),
            CoreCommand::Pause => "PAUSE\n".to_string(),
            CoreCommand::Resume => "RESUME\n".to_string(),
            CoreCommand::CaptchaResult(res) => format!("CAPTCHA_RESULT|{res}\n"),
            CoreCommand::VkJsBootstrap(encoded) => format!("VK_JS_BOOTSTRAP:{encoded}\n"),
            CoreCommand::UpdateDomains(spec) => format!("UPDATE_DOMAINS|{spec}\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_args_default() {
        let params = CoreParams::default();
        let args = build_args(&params, "csqtt_tun_pipe");
        let expected = vec![
            "-peer".to_string(),
            "".to_string(),
            "-n".to_string(),
            "5".to_string(),
            "-listen".to_string(),
            "127.0.0.1:0".to_string(),
            "-tun-uds".to_string(),
            "csqtt_tun_pipe".to_string(),
            "-vk-hash-mode".to_string(),
            "".to_string(),
            "-obfs".to_string(),
            "".to_string(),
            "-turn-transport".to_string(),
            "".to_string(),
            "-srtp-mode".to_string(),
            "".to_string(),
            "-vk-auth-mode".to_string(),
            "".to_string(),
            "-captcha-mode".to_string(),
            "".to_string(),
        ];
        assert_eq!(args, expected);
    }

    #[test]
    fn test_build_args_all_options() {
        let params = CoreParams {
            vk_access_token: String::new(),
            peer: "198.51.100.1:46000".to_string(),
            vk_hashes: vec!["hash1".to_string(), "hash2".to_string()],
            workers_per_hash: 20,
            port: 1080,
            connection_password: "secret_password".to_string(),
            vk_auth_mode: "vkcalls".to_string(),
            captcha_mode: "auto".to_string(),
            fingerprint: "firefox".to_string(),
            client_ids: "8202606,6287487".to_string(),
            obfs_mode: "unified".to_string(),
            turn_transport: "udp".to_string(),
            srtp_mode: "auto".to_string(),
            generation_id: 10,
            session_salt: "salt_abc".to_string(),
            allow_hash_redistribution: true,
            vk_hash_mode: "manual".to_string(),
            custom_turn_ips: "192.0.2.1,192.0.2.2".to_string(),
            domain_routing_enabled: true,
            domain_is_whitelist: false,
            exclude_ru_zones: true,
            template_domains: true,
            template_domains_file: PathBuf::from(r"C:\vpn\domains.txt"),
            direct_domains_file: PathBuf::from(r"C:\vpn\user.txt"),
            ip_routing_enabled: true,
            ip_is_whitelist: false,
            direct_ips: "198.51.100.0/24".to_string(),
            excluded_apps: Vec::new(),
        };

        let args = build_args(&params, "csqtt_tun_pipe");
        let expected = vec![
            "-peer".to_string(),
            "198.51.100.1:46000".to_string(),
            "-n".to_string(),
            "20".to_string(),
            "-listen".to_string(),
            "127.0.0.1:1080".to_string(),
            "-tun-uds".to_string(),
            "csqtt_tun_pipe".to_string(),
            "-vk".to_string(),
            "hash1,hash2".to_string(),
            "-vk-hash-mode".to_string(),
            "manual".to_string(),
            "-fingerprint".to_string(),
            "firefox".to_string(),
            "-client-ids".to_string(),
            "8202606,6287487".to_string(),
            "-obfs".to_string(),
            "unified".to_string(),
            "-turn-transport".to_string(),
            "udp".to_string(),
            "-srtp-mode".to_string(),
            "auto".to_string(),
            "-custom-turn-ips".to_string(),
            "192.0.2.1,192.0.2.2".to_string(),
            "-vk-auth-mode".to_string(),
            "vkcalls".to_string(),
            "-gen".to_string(),
            "10".to_string(),
            "-salt".to_string(),
            "salt_abc".to_string(),
            "-captcha-mode".to_string(),
            "auto".to_string(),
            "--domain-routing".to_string(),
            "--exclude-ru-zones".to_string(),
            "--template-domains".to_string(),
            "--domains-file".to_string(),
            r"C:\vpn\domains.txt".to_string(),
            "--domains-file".to_string(),
            r"C:\vpn\user.txt".to_string(),
            "--ip-routing".to_string(),
            "--direct-ips".to_string(),
            "198.51.100.0/24".to_string(),
        ];
        assert_eq!(args, expected);
    }

    #[test]
    fn test_build_args_whitelist() {
        let params = CoreParams {
            peer: "198.51.100.1:46000".to_string(),
            port: 1080,
            domain_routing_enabled: true,
            domain_is_whitelist: true,
            exclude_ru_zones: true,   // Whitelist ignores exclude_ru_zones
            template_domains: true,   // Whitelist ignores template_domains
            template_domains_file: PathBuf::from(r"C:\vpn\domains.txt"),
            direct_domains_file: PathBuf::from(r"C:\vpn\user.txt"),
            ip_routing_enabled: true,
            ip_is_whitelist: true,
            direct_ips: "10.0.0.1".to_string(),
            excluded_apps: Vec::new(),
            ..Default::default()
        };

        let args = build_args(&params, "csqtt_tun_pipe");
        assert!(args.contains(&"--domain-routing".to_string()));
        assert!(args.contains(&"--domain-whitelist".to_string()));
        assert!(!args.contains(&"--exclude-ru-zones".to_string()));
        assert!(!args.contains(&"--template-domains".to_string()));
        assert!(args.contains(&"--ip-routing".to_string()));
        assert!(args.contains(&"--ip-whitelist".to_string()));
        assert!(args.contains(&"--direct-ips".to_string()));
    }

    #[test]
    fn test_effective_auth_mode_fallback() {
        let params = CoreParams {
            vk_auth_mode: "auto_js".to_string(),
            vk_hash_mode: "manual".to_string(),
            ..Default::default()
        };
        let args = build_args(&params, "pipe");
        let auth_idx = args.iter().position(|a| a == "-vk-auth-mode").unwrap();
        assert_eq!(args[auth_idx + 1], "vkcalls");

        let params_both_js = CoreParams {
            vk_auth_mode: "auto_js".to_string(),
            vk_hash_mode: "auto_js".to_string(),
            ..Default::default()
        };
        let args_both = build_args(&params_both_js, "pipe");
        let auth_idx2 = args_both.iter().position(|a| a == "-vk-auth-mode").unwrap();
        assert_eq!(args_both[auth_idx2 + 1], "auto_js");
    }

    #[test]
    fn test_build_env() {
        let params = CoreParams {
            connection_password: "my_secret_pass".to_string(),
            ..Default::default()
        };
        let env_with_bypass = build_env(&params, Some(42));
        assert_eq!(
            env_with_bypass,
            vec![
                ("CSQTT_EVENTS".to_string(), "1".to_string()),
                ("RAYON_NUM_THREADS".to_string(), "2".to_string()),
                ("CSQTT_PASSWORD".to_string(), "my_secret_pass".to_string()),
                ("CSQTT_BYPASS_IFINDEX".to_string(), "42".to_string()),
            ]
        );

        let env_no_bypass = build_env(&CoreParams::default(), None);
        assert_eq!(
            env_no_bypass,
            vec![
                ("CSQTT_EVENTS".to_string(), "1".to_string()),
                ("RAYON_NUM_THREADS".to_string(), "2".to_string()),
            ]
        );
    }

    #[test]
    fn test_core_commands() {
        assert_eq!(CoreCommand::Stop.line(), "STOP\n");
        assert_eq!(CoreCommand::FinishVkCalls.line(), "FINISH_VK_CALLS\n");
        assert_eq!(CoreCommand::Pause.line(), "PAUSE\n");
        assert_eq!(CoreCommand::Resume.line(), "RESUME\n");
        assert_eq!(
            CoreCommand::CaptchaResult("result_ok_123".to_string()).line(),
            "CAPTCHA_RESULT|result_ok_123\n"
        );
        assert_eq!(
            CoreCommand::VkJsBootstrap("base64token".to_string()).line(),
            "VK_JS_BOOTSTRAP:base64token\n"
        );
        assert_eq!(
            CoreCommand::UpdateDomains("enabled=1;ru=1".to_string()).line(),
            "UPDATE_DOMAINS|enabled=1;ru=1\n"
        );
    }
}
