//! Host-side VK auto calls (port of VkAutoCallsManager.kt + the VkApi.kt calls it uses):
//! with a VK access token create N group calls, return their join hashes, end them on stop.

#![allow(dead_code)]

use std::time::Duration;

use csqtt_logic::constants::{tunnel, vk_auto_hash};
use csqtt_logic::vk_call_policies::{self, VkApiResult};
use csqtt_logic::worker_count_policy::WorkerCountPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartResult {
    /// Comma-separated hashes, as Kotlin StartResult.hashes.
    pub hashes: String,
    pub requested_calls: i32,
    pub created_calls: i32,
}

impl StartResult {
    pub fn needs_worker_redistribution(&self) -> bool {
        self.created_calls < self.requested_calls
    }
}

/// Represents an active call created during the current session.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveCall {
    call_id: String,
    hash: String,
}

/// Information returned by a successful calls.start request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedCall {
    pub call_id: String,
    pub hash: String,
    pub join_link: String,
}

/// Number of calls for a worker count (callCountForWorkers).
pub fn call_count_for_workers(workers: i32) -> i32 {
    for count in 1..=tunnel::MAX_VK_HASHES {
        if workers <= WorkerCountPolicy::maximum_for_hashes(count) {
            return count;
        }
    }
    tunnel::MAX_VK_HASHES
}

/// Request delay for a call count (requestDelayForCallCount).
pub fn request_delay_for_call_count(call_count: i32) -> i64 {
    if call_count <= 4 {
        vk_auto_hash::AUTO_CALL_SMALL_DELAY_MS
    } else {
        vk_auto_hash::AUTO_CALL_LARGE_DELAY_MS
    }
}

/// Extracts the call hash from a join link URL (substringAfterLast('/').trimEnd('/')).
pub fn extract_hash_from_join_link(join_link: &str) -> String {
    let trimmed = join_link.trim();
    let after_slash = match trimmed.rfind('/') {
        Some(idx) => &trimmed[idx + 1..],
        None => trimmed,
    };
    after_slash.trim_end_matches('/').to_string()
}

/// Parses the JSON response of VK API calls.start.
pub fn parse_start_call_response(json_str: &str) -> VkApiResult<StartedCall> {
    let json: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => return VkApiResult::Failed(format!("json parse error: {e}")),
    };

    if let Some(error_obj) = json.get("error").and_then(|e| e.as_object()) {
        let code = match error_obj.get("error_code") {
            Some(serde_json::Value::Number(n)) => n.as_i64().map(|v| v as i32).unwrap_or(1),
            Some(serde_json::Value::String(s)) => s.parse::<i32>().unwrap_or(1),
            _ => 1,
        };
        let message = error_obj
            .get("error_msg")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        return VkApiResult::ApiError { code, message };
    }

    let response = match json.get("response").and_then(|r| r.as_object()) {
        Some(r) => r,
        None => {
            return VkApiResult::Failed("пустой call_id/hash в ответе calls.start".to_string());
        }
    };

    let call_id = response
        .get("call_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let join_link = response
        .get("join_link")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let hash = if let Some(ok_link) = response.get("ok_join_link").and_then(|v| v.as_str()) {
        let trimmed = ok_link.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else {
            extract_hash_from_join_link(&join_link)
        }
    } else {
        extract_hash_from_join_link(&join_link)
    };

    if call_id.trim().is_empty() || hash.trim().is_empty() {
        return VkApiResult::Failed("пустой call_id/hash в ответе calls.start".to_string());
    }

    VkApiResult::Ok(StartedCall {
        call_id,
        hash,
        join_link,
    })
}

/// Parses the JSON response of VK API calls.forceFinish.
pub fn parse_force_finish_response(json_str: &str) -> VkApiResult<i32> {
    let json: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => return VkApiResult::Failed(format!("json parse error: {e}")),
    };

    if let Some(error_obj) = json.get("error").and_then(|e| e.as_object()) {
        let code = match error_obj.get("error_code") {
            Some(serde_json::Value::Number(n)) => n.as_i64().map(|v| v as i32).unwrap_or(1),
            Some(serde_json::Value::String(s)) => s.parse::<i32>().unwrap_or(1),
            _ => 1,
        };
        let message = error_obj
            .get("error_msg")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        return VkApiResult::ApiError { code, message };
    }

    let code = match json.get("response") {
        Some(serde_json::Value::Number(n)) => n.as_i64().map(|v| v as i32).unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.parse::<i32>().unwrap_or(0),
        _ => 0,
    };

    VkApiResult::Ok(code)
}

/// Configures a ureq Agent with 8s global timeout and without turning 4xx/5xx into Error::StatusCode.
fn vk_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(8_000)))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(config)
}

/// Sends a POST request to VK API method with form-encoded parameters.
fn vk_request(
    agent: &ureq::Agent,
    method: &str,
    token: &str,
    params: &[(&str, &str)],
) -> Result<String, String> {
    let url = format!("{}{}", vk_auto_hash::API_METHOD_BASE_URL, method);
    let mut form_params: Vec<(&str, &str)> = Vec::with_capacity(params.len() + 1);
    form_params.extend_from_slice(params);
    form_params.push(("v", vk_auto_hash::API_VERSION));

    let mut response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .send_form(form_params)
        .map_err(|e| e.to_string())?;

    response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Blocking execution of calls.start.
fn vk_start_call(agent: &ureq::Agent, token: &str) -> VkApiResult<StartedCall> {
    match vk_request(agent, "calls.start", token, &[]) {
        Ok(body) => parse_start_call_response(&body),
        Err(err) => VkApiResult::Failed(err),
    }
}

/// Blocking execution of calls.forceFinish.
fn vk_force_finish_call(agent: &ureq::Agent, token: &str, call_id: &str) -> VkApiResult<i32> {
    match vk_request(agent, "calls.forceFinish", token, &[("call_id", call_id)]) {
        Ok(body) => parse_force_finish_response(&body),
        Err(err) => VkApiResult::Failed(err),
    }
}

/// Closes the given active calls in parallel and optionally logs the outcomes.
async fn finish_calls_internal(
    agent: &ureq::Agent,
    calls: Vec<ActiveCall>,
    token: &str,
    log_results: bool,
    log: &(dyn Fn(&str) + Send + Sync),
) {
    if calls.is_empty() {
        return;
    }

    let mut handles = Vec::with_capacity(calls.len());
    for call in calls {
        let agent_clone = agent.clone();
        let token_string = token.to_string();
        let call_id = call.call_id;
        handles.push(tokio::task::spawn_blocking(move || {
            vk_force_finish_call(&agent_clone, &token_string, &call_id)
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(res) => results.push(res),
            Err(e) => results.push(VkApiResult::Failed(format!("task join error: {e}"))),
        }
    }

    if log_results {
        for result in results {
            match result {
                VkApiResult::Ok(_) => {
                    log("[OK] Звонок VK завершён");
                }
                VkApiResult::ApiError { code, .. } => {
                    log(&format!("[ERR] Звонок VK не завершён · код={code}"));
                }
                VkApiResult::Failed(reason) => {
                    log(&format!("[ERR] Звонок VK не завершён · {reason}"));
                }
            }
        }
    }
}

/// Keeps the active calls of the current session (Kotlin object state).
#[derive(Default, Debug)]
pub struct VkAutoCalls {
    active_calls: Vec<ActiveCall>,
    active_token: Option<String>,
}

impl VkAutoCalls {
    pub fn new() -> Self {
        Self::default()
    }

    /// startAutoCalls. `log` receives the same Russian log lines as Kotlin updateLog.
    /// Ok(None) where Kotlin returns null; `invalid_token` is set to true when Kotlin would
    /// clear the stored token (clearStoredTokenOnInvalid) so the caller can delete it.
    pub async fn start(
        &mut self,
        token: &str,
        workers: i32,
        log: &(dyn Fn(&str) + Send + Sync),
        invalid_token: &mut bool,
    ) -> anyhow::Result<Option<StartResult>> {
        *invalid_token = false;
        if token.trim().is_empty() {
            return Ok(None);
        }

        self.active_calls.clear();
        self.active_token = None;

        let count = call_count_for_workers(workers);
        let mut hashes = Vec::with_capacity(count as usize);

        let request_delay = Duration::from_millis(request_delay_for_call_count(count) as u64);
        let mut next_request_at = tokio::time::Instant::now();

        let agent = vk_agent();
        let mut completed_slots = 0;

        while completed_slots < count {
            let mut result = VkApiResult::Failed("not attempted".to_string());
            let mut attempts = 0;

            while attempts < vk_auto_hash::AUTO_CALL_MAX_ATTEMPTS {
                attempts += 1;

                let now = tokio::time::Instant::now();
                if next_request_at > now {
                    tokio::time::sleep(next_request_at - now).await;
                }
                next_request_at = tokio::time::Instant::now() + request_delay;

                let agent_clone = agent.clone();
                let token_string = token.to_string();
                let api_res = tokio::task::spawn_blocking(move || {
                    vk_start_call(&agent_clone, &token_string)
                })
                .await;

                result = match api_res {
                    Ok(res) => res,
                    Err(e) => VkApiResult::Failed(format!("task join error: {e}")),
                };

                match &result {
                    VkApiResult::Ok(_) => break,
                    VkApiResult::ApiError { code, .. } => {
                        if vk_call_policies::is_token_invalid(*code) {
                            break;
                        }
                        if *code == 6 {
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }
                    VkApiResult::Failed(_) => {}
                }
            }

            completed_slots += 1;

            match result {
                VkApiResult::Ok(call) => {
                    self.active_token = Some(token.to_string());
                    self.active_calls.push(ActiveCall {
                        call_id: call.call_id.clone(),
                        hash: call.hash.clone(),
                    });
                    hashes.push(call.hash);
                    log("[OK] Звонок VK создан ✓");
                }
                VkApiResult::ApiError { code, message } => {
                    log(&format!("[ERR] Звонок VK не создан · код={code} {message}"));
                    if vk_call_policies::is_token_invalid(code) {
                        *invalid_token = true;
                        log("[ERR] Токен VK недействителен · войдите снова");
                        let calls_to_finish = std::mem::take(&mut self.active_calls);
                        self.active_token = None;
                        let timeout = Duration::from_millis(vk_auto_hash::FINISH_CALLS_TIMEOUT_MS as u64);
                        let _ = tokio::time::timeout(
                            timeout,
                            finish_calls_internal(&agent, calls_to_finish, token, false, log),
                        )
                        .await;
                        return Ok(None);
                    }
                }
                VkApiResult::Failed(reason) => {
                    log(&format!("[ERR] Звонок VK не создан · {reason}"));
                }
            }
        }

        if !hashes.is_empty() && (hashes.len() as i32) < count {
            log(&format!(
                "[OK] Звонки VK {}/{} · потоки распределены",
                hashes.len(),
                count
            ));
        }

        if hashes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(StartResult {
                hashes: hashes.join(","),
                requested_calls: count,
                created_calls: hashes.len() as i32,
            }))
        }
    }

    pub fn has_active_calls(&self) -> bool {
        !self.active_calls.is_empty()
    }

    /// End all active calls (Kotlin finish/stop path).
    pub async fn finish_all(&mut self, log: &(dyn Fn(&str) + Send + Sync)) {
        if self.active_calls.is_empty() {
            return;
        }

        let calls = std::mem::take(&mut self.active_calls);
        let token = self.active_token.take();
        let Some(token) = token.filter(|t| !t.trim().is_empty()) else {
            return;
        };

        let agent = vk_agent();
        let timeout = Duration::from_millis(vk_auto_hash::FINISH_CALLS_TIMEOUT_MS as u64);
        let _ = tokio::time::timeout(
            timeout,
            finish_calls_internal(&agent, calls, &token, true, log),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automatic_call_count_scales_to_seven_hashes() {
        assert_eq!(1, call_count_for_workers(10));
        assert_eq!(2, call_count_for_workers(20));
        assert_eq!(3, call_count_for_workers(30));
        assert_eq!(5, call_count_for_workers(45));
        assert_eq!(6, call_count_for_workers(60));
        assert_eq!(7, call_count_for_workers(100));
        assert_eq!(
            vk_auto_hash::AUTO_CALL_SMALL_DELAY_MS,
            request_delay_for_call_count(1)
        );
        assert_eq!(
            vk_auto_hash::AUTO_CALL_SMALL_DELAY_MS,
            request_delay_for_call_count(4)
        );
        assert_eq!(
            vk_auto_hash::AUTO_CALL_LARGE_DELAY_MS,
            request_delay_for_call_count(5)
        );
        assert_eq!(
            vk_auto_hash::AUTO_CALL_LARGE_DELAY_MS,
            request_delay_for_call_count(6)
        );
    }

    #[test]
    fn test_call_count_bounds() {
        assert_eq!(1, call_count_for_workers(i32::MIN));
        assert_eq!(1, call_count_for_workers(0));
        assert_eq!(1, call_count_for_workers(5));
        assert_eq!(1, call_count_for_workers(10));
        assert_eq!(2, call_count_for_workers(11));
        assert_eq!(2, call_count_for_workers(20));
        assert_eq!(3, call_count_for_workers(21));
        assert_eq!(3, call_count_for_workers(30));
        assert_eq!(4, call_count_for_workers(40));
        assert_eq!(5, call_count_for_workers(50));
        assert_eq!(6, call_count_for_workers(60));
        assert_eq!(7, call_count_for_workers(70));
        assert_eq!(7, call_count_for_workers(100));
        assert_eq!(7, call_count_for_workers(i32::MAX));
    }

    #[test]
    fn test_start_result_redistribution() {
        let res_needed = StartResult {
            hashes: "hash1".into(),
            requested_calls: 2,
            created_calls: 1,
        };
        assert!(res_needed.needs_worker_redistribution());

        let res_exact = StartResult {
            hashes: "hash1,hash2".into(),
            requested_calls: 2,
            created_calls: 2,
        };
        assert!(!res_exact.needs_worker_redistribution());
    }

    #[test]
    fn test_extract_hash_from_join_link() {
        assert_eq!(
            "abcdef1234567890",
            extract_hash_from_join_link("https://vk.com/call/join/abcdef1234567890")
        );
        assert_eq!(
            "",
            extract_hash_from_join_link("https://vk.com/call/join/abcdef1234567890/")
        );
        assert_eq!("raw_hash", extract_hash_from_join_link("raw_hash"));
        assert_eq!(
            "last_segment",
            extract_hash_from_join_link("https://domain.tld/path/to/last_segment")
        );
    }

    #[test]
    fn test_parse_start_call_success_with_join_link() {
        let json = r#"{
            "response": {
                "call_id": "7_12345_67890",
                "join_link": "https://vk.com/call/join/7X9aBcDeFgHiJkLmNoPqRsTuVwXyZ"
            }
        }"#;
        let res = parse_start_call_response(json);
        match res {
            VkApiResult::Ok(call) => {
                assert_eq!("7_12345_67890", call.call_id);
                assert_eq!("7X9aBcDeFgHiJkLmNoPqRsTuVwXyZ", call.hash);
                assert_eq!(
                    "https://vk.com/call/join/7X9aBcDeFgHiJkLmNoPqRsTuVwXyZ",
                    call.join_link
                );
            }
            _ => panic!("expected Ok, got {res:?}"),
        }
    }

    #[test]
    fn test_parse_start_call_success_with_ok_join_link() {
        let json = r#"{
            "response": {
                "call_id": "call_42",
                "join_link": "https://vk.com/call/join/ignored_hash",
                "ok_join_link": "preferred_hash"
            }
        }"#;
        let res = parse_start_call_response(json);
        match res {
            VkApiResult::Ok(call) => {
                assert_eq!("call_42", call.call_id);
                assert_eq!("preferred_hash", call.hash);
                assert_eq!("https://vk.com/call/join/ignored_hash", call.join_link);
            }
            _ => panic!("expected Ok, got {res:?}"),
        }
    }

    #[test]
    fn test_parse_start_call_empty_id_or_hash() {
        let json_empty_id = r#"{
            "response": {
                "call_id": "",
                "join_link": "https://vk.com/call/join/valid_hash"
            }
        }"#;
        let res1 = parse_start_call_response(json_empty_id);
        assert_eq!(
            VkApiResult::Failed("пустой call_id/hash в ответе calls.start".into()),
            res1
        );

        let json_empty_link = r#"{
            "response": {
                "call_id": "call_1",
                "join_link": ""
            }
        }"#;
        let res2 = parse_start_call_response(json_empty_link);
        assert_eq!(
            VkApiResult::Failed("пустой call_id/hash в ответе calls.start".into()),
            res2
        );
    }

    #[test]
    fn test_parse_start_call_api_error() {
        let json = r#"{
            "error": {
                "error_code": 5,
                "error_msg": "User authorization failed: invalid access_token."
            }
        }"#;
        let res = parse_start_call_response(json);
        match res {
            VkApiResult::ApiError { code, message } => {
                assert_eq!(5, code);
                assert_eq!("User authorization failed: invalid access_token.", message);
            }
            _ => panic!("expected ApiError, got {res:?}"),
        }
    }

    #[test]
    fn test_parse_start_call_malformed_json() {
        let res = parse_start_call_response("not a valid json");
        assert!(matches!(res, VkApiResult::Failed(_)));
    }

    #[test]
    fn test_parse_force_finish_success() {
        let json = r#"{ "response": 1 }"#;
        assert_eq!(VkApiResult::Ok(1), parse_force_finish_response(json));

        let json_zero = r#"{ "response": 0 }"#;
        assert_eq!(VkApiResult::Ok(0), parse_force_finish_response(json_zero));
    }

    #[test]
    fn test_parse_force_finish_api_error() {
        let json = r#"{
            "error": {
                "error_code": 100,
                "error_msg": "One of the parameters specified was missing or invalid: call_id is incorrect"
            }
        }"#;
        let res = parse_force_finish_response(json);
        match res {
            VkApiResult::ApiError { code, message } => {
                assert_eq!(100, code);
                assert_eq!(
                    "One of the parameters specified was missing or invalid: call_id is incorrect",
                    message
                );
            }
            _ => panic!("expected ApiError, got {res:?}"),
        }
    }

    #[test]
    fn test_parse_force_finish_malformed_json() {
        let res = parse_force_finish_response("{ broken json");
        assert!(matches!(res, VkApiResult::Failed(_)));
    }

    #[test]
    fn test_vk_auto_calls_default_has_no_active_calls() {
        let calls = VkAutoCalls::new();
        assert!(!calls.has_active_calls());
    }
}
