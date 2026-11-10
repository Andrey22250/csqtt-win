//! Port of VkNetworkProbePolicy.kt.

/// Computes the retry delay in milliseconds based on consecutive probe failures.
pub fn vk_probe_retry_delay_ms(failures: i32) -> i64 {
    match failures.max(1) {
        1..=15 => 1_000,
        16 => 2_000,
        17 => 3_000,
        18 => 5_000,
        19 => 8_000,
        _ => 15_000,
    }
}

/// Checks whether an HTTP status code indicates that VK is reachable.
pub fn is_vk_probe_http_response(code: i32) -> bool {
    (100..=599).contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_is_bounded() {
        assert_eq!(1_000, vk_probe_retry_delay_ms(0));
        assert_eq!(1_000, vk_probe_retry_delay_ms(15));
        assert_eq!(2_000, vk_probe_retry_delay_ms(16));
        assert_eq!(15_000, vk_probe_retry_delay_ms(20));
    }

    #[test]
    fn any_http_response_means_vk_is_reachable() {
        assert!(is_vk_probe_http_response(100));
        assert!(is_vk_probe_http_response(200));
        assert!(is_vk_probe_http_response(403));
        assert!(is_vk_probe_http_response(599));
        assert!(!is_vk_probe_http_response(99));
        assert!(!is_vk_probe_http_response(600));
    }
}
