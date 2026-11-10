//! Pure-logic port of the csqtt Android client (no Android/Windows APIs).
//! Each module mirrors one Kotlin file in app/src/main/java/com/csqtt/client and ports its unit tests.

// p1
pub mod constants;
pub mod turn_subnet_registry;
pub mod tunnel_session_id;
pub mod connection_source;
// p2
pub mod tunnel_event_parser;
pub mod tunnel_log_policy;
pub mod tunnel_startup_progress;
// p3
pub mod tunnel_lifecycle_state;
pub mod log_book;
pub mod worker_count_policy;
pub mod worker_recovery_policy;
pub mod connection_start_policy;
// p4
pub mod tun_recovery_policy;
pub mod vpn_recovery_policy;
pub mod physical_network_tracker;
pub mod network_pause_gate;
pub mod wifi_auto_pause_policy;
pub mod tunnel_service_policy;
// p5
pub mod vk_mode_policy;
pub mod vk_call_policies;
pub mod vk_network_probe_policy;
pub mod captcha_uri_policy;
pub mod turn_transport_policy;
// p6
pub mod stored_secret_state;
pub mod tunnel_password_storage;
pub mod tunnel_auth_snapshot;
pub mod dns_profile;
pub mod vpn_configuration;
pub mod update_launch_guard;
pub mod app_update;
pub mod vk_hash_validator;
// stage 4
pub mod csqtt_link;
pub mod settings;
