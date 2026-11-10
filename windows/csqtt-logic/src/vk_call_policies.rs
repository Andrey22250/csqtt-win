//! Port of VkCallPolicies.kt.

/// Result of a VK API call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VkApiResult<T> {
    Ok(T),
    ApiError { code: i32, message: String },
    Failed(String),
}

impl<T> VkApiResult<T> {
    pub fn ok(value: T) -> Self {
        VkApiResult::Ok(value)
    }

    pub fn api_error(code: i32, message: impl Into<String>) -> Self {
        VkApiResult::ApiError {
            code,
            message: message.into(),
        }
    }

    pub fn failed(reason: impl Into<String>) -> Self {
        VkApiResult::Failed(reason.into())
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, VkApiResult::Ok(_))
    }

    pub fn is_api_error(&self) -> bool {
        matches!(self, VkApiResult::ApiError { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, VkApiResult::Failed(_))
    }

    pub fn is_token_invalid(&self) -> bool {
        match self {
            VkApiResult::ApiError { code, .. } => is_token_invalid(*code),
            _ => false,
        }
    }
}

/// Error codes that indicate an invalid/expired token.
pub const TOKEN_INVALID_CODES: [i32; 4] = [4, 5, 27, 28];

/// Returns true if the given VK API error code indicates an invalid token.
pub fn is_token_invalid(code: i32) -> bool {
    TOKEN_INVALID_CODES.contains(&code)
}

/// Pacer for VK API requests enforcing a minimum interval between requests.
pub struct VkRequestPacer<N, S> {
    interval_ms: i64,
    now_ms: N,
    sleep_ms: S,
    next_request_at_ms: i64,
}

impl<N, S> std::fmt::Debug for VkRequestPacer<N, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VkRequestPacer")
            .field("interval_ms", &self.interval_ms)
            .field("next_request_at_ms", &self.next_request_at_ms)
            .finish()
    }
}

impl<N, S> VkRequestPacer<N, S>
where
    N: FnMut() -> i64,
    S: FnMut(i64),
{
    pub fn new(interval_ms: i64, now_ms: N, sleep_ms: S) -> Self {
        Self {
            interval_ms,
            now_ms,
            sleep_ms,
            next_request_at_ms: 0,
        }
    }

    pub fn await_request_slot(&mut self) {
        let wait_ms = self.next_request_at_ms - (self.now_ms)();
        if wait_ms > 0 {
            (self.sleep_ms)(wait_ms);
        }
        self.next_request_at_ms = (self.now_ms)() + self.interval_ms;
    }
}

/// Executes a VK call with retry logic up to AUTO_CALL_MAX_ATTEMPTS.
pub fn run_vk_call_attempts<T>(
    request: impl FnMut() -> VkApiResult<T>,
) -> VkApiResult<T> {
    #[cfg(not(test))]
    let sleeper = |ms: i64| std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    #[cfg(test)]
    let sleeper = |_ms: i64| {};

    run_vk_call_attempts_with_sleeper(request, sleeper)
}

/// Executes a VK call with retry logic and custom sleep callback.
pub fn run_vk_call_attempts_with_sleeper<T>(
    mut request: impl FnMut() -> VkApiResult<T>,
    mut sleep_ms: impl FnMut(i64),
) -> VkApiResult<T> {
    let mut result = VkApiResult::Failed("not attempted".to_string());
    let mut attempts = 0;
    while attempts < crate::constants::vk_auto_hash::AUTO_CALL_MAX_ATTEMPTS {
        attempts += 1;
        result = request();
        match &result {
            VkApiResult::Ok(_) => break,
            VkApiResult::ApiError { code, .. } => {
                if is_token_invalid(*code) {
                    break;
                }
                if *code == 6 {
                    sleep_ms(1000);
                }
            }
            VkApiResult::Failed(_) => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_failures_succeed_on_the_third_attempt() {
        let mut attempts = 0;
        let result = run_vk_call_attempts(|| {
            attempts += 1;
            if attempts == 3 {
                VkApiResult::Ok("hash")
            } else {
                VkApiResult::Failed("temporary".to_string())
            }
        });
        assert_eq!(3, attempts);
        match result {
            VkApiResult::Ok(value) => assert_eq!("hash", value),
            _ => panic!("expected VkApiResult::Ok"),
        }
    }

    #[test]
    fn permanent_failure_stops_after_three_attempts() {
        let mut attempts = 0;
        let result = run_vk_call_attempts::<String>(|| {
            attempts += 1;
            VkApiResult::ApiError {
                code: 6,
                message: "too many requests".to_string(),
            }
        });
        assert_eq!(3, attempts);
        assert!(matches!(result, VkApiResult::ApiError { .. }));
    }

    #[test]
    fn invalid_token_is_never_retried() {
        let mut attempts = 0;
        let result = run_vk_call_attempts::<String>(|| {
            attempts += 1;
            VkApiResult::ApiError {
                code: 5,
                message: "authorization failed".to_string(),
            }
        });
        assert_eq!(1, attempts);
        assert!(matches!(result, VkApiResult::ApiError { .. }));
    }

    #[test]
    fn request_starts_respect_the_configured_interval() {
        let now = std::cell::Cell::new(0i64);
        let mut starts = Vec::new();
        let mut pacer = VkRequestPacer::new(
            202,
            || now.get(),
            |wait| now.set(now.get() + wait),
        );
        for i in 0..18 {
            pacer.await_request_slot();
            starts.push(now.get());
            let current = now.get();
            now.set(current + if i % 2 == 0 { 20 } else { 350 });
        }
        assert!(starts.windows(2).all(|w| w[1] - w[0] >= 202));
    }
}
