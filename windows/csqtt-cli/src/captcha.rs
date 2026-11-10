//! Captcha solving in WebView2 (port of CaptchaWebViewManager.kt, ManlCaptchaWebViewManager.kt,
//! ManlCaptchaActivity.kt and the method selection in TunnelManager.kt solveCaptcha*).

use std::time::Duration;

use crate::webview::{self, Action, Options, WebEvent, WebViewError};

/// Error token returned when a slider / kaleidoscope captcha is detected.
pub const ERROR_SLIDER_DETECTED: &str = "slider_detected";

/// Profile name for captcha cookie jar and cache isolation.
pub const CAPTCHA_PROFILE: &str = "captcha";

/// Viewport widths from CsqttConstants.Captcha.AUTO_VIEWPORT_WIDTHS.
pub const AUTO_VIEWPORT_WIDTHS: [i32; 7] = [356, 358, 360, 362, 364, 366, 368];

/// Viewport heights from CsqttConstants.Captcha.AUTO_VIEWPORT_HEIGHTS.
pub const AUTO_VIEWPORT_HEIGHTS: [i32; 7] = [376, 378, 380, 382, 384, 386, 388];

/// Timeout for automatic captcha solving (10 seconds).
pub const CAPTCHA_AUTO_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for manual captcha solving (60 seconds).
pub const CAPTCHA_MANUAL_TIMEOUT: Duration = Duration::from_secs(60);

const SCRIPT_ID_INIT: u32 = 1;
const SCRIPT_ID_FIND_LABEL: u32 = 2;
const SCRIPT_ID_CLICK: u32 = 3;
const SCRIPT_ID_DETECT_SLIDER: u32 = 4;
const SCRIPT_ID_HIDE_ELEMENTS: u32 = 5;

const CSQTT_MARKER_SLIDER_DETECTED: &str = "__CSQTT_SLIDER_DETECTED__";
const CSQTT_MARKER_CANCELLED: &str = "__CSQTT_CANCELLED__";
const CSQTT_MARKER_ERROR_PREFIX: &str = "__CSQTT_ERROR__";

/// Bridge shim ensuring window.CsqttCaptcha methods forward to window.chrome.webview.postMessage.
const BRIDGE_SHIM_JS: &str = r#"
(function() {
    if (!window.CsqttCaptcha) {
        window.CsqttCaptcha = {
            onSuccess: function(token) {
                if (window.chrome && window.chrome.webview) {
                    window.chrome.webview.postMessage(JSON.stringify({ method: 'onSuccess', token: token }));
                }
            },
            onSliderDetected: function(source) {
                if (window.chrome && window.chrome.webview) {
                    window.chrome.webview.postMessage(JSON.stringify({ method: 'onSliderDetected', source: source }));
                }
            },
            onError: function(err) {
                if (window.chrome && window.chrome.webview) {
                    window.chrome.webview.postMessage(JSON.stringify({ method: 'onError', error: typeof err === 'string' ? err : JSON.stringify(err) }));
                }
            },
            onCancelAndStop: function() {
                if (window.chrome && window.chrome.webview) {
                    window.chrome.webview.postMessage(JSON.stringify({ method: 'onCancelAndStop' }));
                }
            }
        };
    }
})();
"#;

/// Interceptor script for automatic captcha solving (verbatim from CaptchaWebViewManager.kt, adapted bridge call).
const AUTO_INTERCEPTOR_JS: &str = r#"
(function() {
    if (window.__csqtt_interceptor_installed) return;
    window.__csqtt_interceptor_installed = true;

    const origFetch = window.fetch;
    window.fetch = async function() {
        const args = arguments;
        const url = args[0] || '';
        if (typeof url === 'string' && url.includes('captchaNotRobot.check')) {
            const response = await origFetch.apply(this, args);
            const clone = response.clone();
            try {
                const data = await clone.json();
                if (data.response && data.response.success_token) {
                    if (window.chrome && window.chrome.webview) {
                        window.chrome.webview.postMessage(JSON.stringify({ method: 'onSuccess', token: data.response.success_token }));
                    } else if (window.CsqttCaptcha) {
                        window.CsqttCaptcha.onSuccess(data.response.success_token);
                    }
                } else if (
                    data.response &&
                    data.response.show_captcha_type === 'slider'
                ) {
                    if (window.chrome && window.chrome.webview) {
                        window.chrome.webview.postMessage(JSON.stringify({ method: 'onSliderDetected', source: 'check_response' }));
                    } else if (window.CsqttCaptcha) {
                        window.CsqttCaptcha.onSliderDetected('check_response');
                    }
                } else if (data.error) {
                    if (window.chrome && window.chrome.webview) {
                        window.chrome.webview.postMessage(JSON.stringify({ method: 'onError', error: JSON.stringify(data.error) }));
                    } else if (window.CsqttCaptcha) {
                        window.CsqttCaptcha.onError(JSON.stringify(data.error));
                    }
                }
            } catch(e) {}
            return response;
        }
        return origFetch.apply(this, args);
    };

    const origXHROpen = XMLHttpRequest.prototype.open;
    const origXHRSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function(method, url) {
        this._csqtt_url = url;
        return origXHROpen.apply(this, arguments);
    };
    XMLHttpRequest.prototype.send = function() {
        const xhr = this;
        if (xhr._csqtt_url && xhr._csqtt_url.includes('captchaNotRobot.check')) {
            xhr.addEventListener('load', function() {
                try {
                    const data = JSON.parse(xhr.responseText);
                    if (data.response && data.response.success_token) {
                        if (window.chrome && window.chrome.webview) {
                            window.chrome.webview.postMessage(JSON.stringify({ method: 'onSuccess', token: data.response.success_token }));
                        } else if (window.CsqttCaptcha) {
                            window.CsqttCaptcha.onSuccess(data.response.success_token);
                        }
                    } else if (
                        data.response &&
                        data.response.show_captcha_type === 'slider'
                    ) {
                        if (window.chrome && window.chrome.webview) {
                            window.chrome.webview.postMessage(JSON.stringify({ method: 'onSliderDetected', source: 'check_response' }));
                        } else if (window.CsqttCaptcha) {
                            window.CsqttCaptcha.onSliderDetected('check_response');
                        }
                    } else if (data.error) {
                        if (window.chrome && window.chrome.webview) {
                            window.chrome.webview.postMessage(JSON.stringify({ method: 'onError', error: JSON.stringify(data.error) }));
                        } else if (window.CsqttCaptcha) {
                            window.CsqttCaptcha.onError(JSON.stringify(data.error));
                        }
                    }
                } catch(e) {}
            });
        }
        return origXHRSend.apply(this, arguments);
    };
})();
"#;

/// Interceptor script for manual captcha solving (verbatim from ManlCaptchaActivity.kt, adapted bridge call).
const MANUAL_INTERCEPTOR_JS: &str = r#"
(function() {
    if (window.__csqtt_interceptor_installed) return;
    window.__csqtt_interceptor_installed = true;

    const origFetch = window.fetch;
    window.fetch = async function() {
        const args = arguments;
        const url = args[0] || '';
        if (typeof url === 'string' && url.includes('captchaNotRobot.check')) {
            const response = await origFetch.apply(this, args);
            const clone = response.clone();
            try {
                const data = await clone.json();
                if (data.response && data.response.success_token) {
                    if (window.chrome && window.chrome.webview) {
                        window.chrome.webview.postMessage(JSON.stringify({ method: 'onSuccess', token: data.response.success_token }));
                    } else if (window.CsqttCaptcha) {
                        window.CsqttCaptcha.onSuccess(data.response.success_token);
                    }
                } else if (data.error) {
                    if (window.chrome && window.chrome.webview) {
                        window.chrome.webview.postMessage(JSON.stringify({ method: 'onError', error: JSON.stringify(data.error) }));
                    } else if (window.CsqttCaptcha) {
                        window.CsqttCaptcha.onError(JSON.stringify(data.error));
                    }
                }
            } catch(e) {}
            return response;
        }
        return origFetch.apply(this, args);
    };

    const origXHROpen = XMLHttpRequest.prototype.open;
    const origXHRSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function(method, url) {
        this._csqtt_url = url;
        return origXHROpen.apply(this, arguments);
    };
    XMLHttpRequest.prototype.send = function() {
        const xhr = this;
        if (xhr._csqtt_url && xhr._csqtt_url.includes('captchaNotRobot.check')) {
            xhr.addEventListener('load', function() {
                try {
                    const data = JSON.parse(xhr.responseText);
                    if (data.response && data.response.success_token) {
                        if (window.chrome && window.chrome.webview) {
                            window.chrome.webview.postMessage(JSON.stringify({ method: 'onSuccess', token: data.response.success_token }));
                        } else if (window.CsqttCaptcha) {
                            window.CsqttCaptcha.onSuccess(data.response.success_token);
                        }
                    } else if (data.error) {
                        if (window.chrome && window.chrome.webview) {
                            window.chrome.webview.postMessage(JSON.stringify({ method: 'onError', error: JSON.stringify(data.error) }));
                        } else if (window.CsqttCaptcha) {
                            window.CsqttCaptcha.onError(JSON.stringify(data.error));
                        }
                    }
                } catch(e) {}
            });
        }
        return origXHRSend.apply(this, arguments);
    };
})();
"#;

/// Hide elements and style injection script (verbatim from ManlCaptchaActivity.kt, adapted bridge call).
const HIDE_ELEMENTS_JS: &str = r#"
(function() {
    document.addEventListener('click', function(e) {
        if (e.target.closest('.vkc__ModalCardBase-module__dismiss')) {
            if (window.chrome && window.chrome.webview) {
                window.chrome.webview.postMessage(JSON.stringify({ method: 'onCancelAndStop' }));
            } else if (window.CsqttCaptcha) {
                window.CsqttCaptcha.onCancelAndStop();
            }
        }
    });

    const style = document.createElement('style');
    style.innerHTML = `
        .vkc__VisuallyHiddenModalOverlay-module__host,
        .vkc__ModalOverlay-module__host,
        .vkc__KaleidoscopeScreen-module__logoBlock,
        .vkc__KaleidoscopeScreen-module__captchaId,
        .vkc__SliderCaptcha-module__descriptionLink,
        .vkc__SliderCaptcha-module__changeTypeButton {
            display: none !important;
        }

        body, html, .vkc__ModalCard-module__host, .vkc__AppRoot-module__host, .vkui__root {
            background: transparent !important;
            box-shadow: none !important;
        }

        .vkc__ModalCardBase-module__container {
            background: #000000 !important;
            box-shadow: none !important;
        }

        .vkc__ModalCardBase-module__dismiss {
            color: #ef4444 !important;
            transform: scale(0.8) translateX(-12px) !important;
        }
        .vkc__ModalCardBase-module__dismiss svg {
            fill: #ef4444 !important;
        }

        .vkc__RefreshButton-module__text,
        .vkc__SliderCaptcha-module__description {
            color: #ffffff !important;
        }

        .vkc__SwipeButton-module__track {
            background-color: #ffffff !important;
        }

        .vkc__SwipeButton-module__track span {
            color: #0000FF !important;
        }
    `;
    document.head.appendChild(style);
})();
"#;

/// Checkbox label search script (verbatim from CaptchaWebViewManager.kt).
const FIND_LABEL_JS: &str = r#"
(function() {
    var slider = document.querySelector(
        '[class*="SliderCaptcha"], [class*="Kaleidoscope"], ' +
        '.vkc__SliderCaptcha-module__description, ' +
        '.vkc__KaleidoscopeScreen-module__captchaId'
    );
    if (slider) return 'slider_detected';

    var el = document.querySelector('label.vkc__Checkbox-module__Checkbox');
    if (!el) el = document.querySelector('label[for="not-robot-captcha-checkbox"]');
    if (!el) el = document.getElementById('not-robot-captcha-checkbox');
    if (!el) return 'not_found';

    var rect = el.getBoundingClientRect();
    var style = window.getComputedStyle(el);
    if (rect.width < 5 || rect.height < 5 ||
        style.display === 'none' || style.visibility === 'hidden') {
        return 'not_found';
    }
    return rect.left + ',' + rect.top + ',' + rect.width + ',' + rect.height;
})();
"#;

/// Fallback JS click script (verbatim from CaptchaWebViewManager.kt).
const JS_CLICK: &str = r#"
(function() {
    var el = document.querySelector('label.vkc__Checkbox-module__Checkbox');
    if (!el) el = document.getElementById('not-robot-captcha-checkbox');
    if (el) { el.click(); return 'clicked'; }
    return 'nothing';
})();
"#;

/// Slider detection script after click (verbatim from CaptchaWebViewManager.kt).
const DETECT_SLIDER_JS: &str = r#"
(function() {
    var slider = document.querySelector(
        '[class*="SliderCaptcha"], [class*="Kaleidoscope"], ' +
        '.vkc__SliderCaptcha-module__description, ' +
        '.vkc__KaleidoscopeScreen-module__captchaId, ' +
        '.vkc__SwipeButton-module__track'
    );
    if (slider) return 'slider';

    var success = document.querySelector(
        '[class*="success"], [class*="Success"], [class*="passed"], [class*="Passed"]'
    );
    if (success) return 'success_ui';

    return 'none';
})();
"#;

/// Structured message received from the injected captcha JS bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptchaBridgeMessage {
    Success { token: String },
    SliderDetected { source: String },
    Error { error: String },
    CancelAndStop,
}

/// Parses a message from `window.chrome.webview.postMessage(x)`.
pub fn parse_bridge_message(raw: &str) -> Option<CaptchaBridgeMessage> {
    let mut val: serde_json::Value = serde_json::from_str(raw).ok()?;
    if let serde_json::Value::String(s) = &val {
        if let Ok(nested) = serde_json::from_str::<serde_json::Value>(s) {
            val = nested;
        }
    }

    if let Some(obj) = val.as_object() {
        let method = obj
            .get("method")
            .or_else(|| obj.get("type"))
            .or_else(|| obj.get("action"))
            .and_then(|v| v.as_str());

        match method {
            Some("onSuccess") | Some("success") => {
                let token = obj
                    .get("token")
                    .or_else(|| obj.get("success_token"))
                    .and_then(|v| v.as_str())?;
                return Some(CaptchaBridgeMessage::Success {
                    token: token.to_string(),
                });
            }
            Some("onSliderDetected") | Some("slider_detected") | Some("slider") => {
                let source = obj
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("check_response");
                return Some(CaptchaBridgeMessage::SliderDetected {
                    source: source.to_string(),
                });
            }
            Some("onError") | Some("error") => {
                let error = obj
                    .get("error")
                    .and_then(|v| {
                        if v.is_string() {
                            v.as_str().map(|s| s.to_string())
                        } else {
                            Some(v.to_string())
                        }
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                return Some(CaptchaBridgeMessage::Error { error });
            }
            Some("onCancelAndStop") | Some("cancel_and_stop") | Some("cancel") => {
                return Some(CaptchaBridgeMessage::CancelAndStop);
            }
            _ => {
                if let Some(token) = obj.get("success_token").and_then(|v| v.as_str()) {
                    return Some(CaptchaBridgeMessage::Success {
                        token: token.to_string(),
                    });
                }
            }
        }
    }

    None
}

/// Strips surrounding JSON quotes or parses a string return value from `ExecuteScript`.
pub fn parse_script_result(raw: &str) -> String {
    if let Ok(serde_json::Value::String(s)) = serde_json::from_str::<serde_json::Value>(raw) {
        s
    } else {
        let trimmed = raw.trim();
        trimmed.trim_matches('"').to_string()
    }
}

/// Checks whether the given URL is a known VK captcha challenge page.
pub fn is_captcha_page(url: &str) -> bool {
    url.contains("not_robot_captcha")
        || url.contains("id.vk.ru/captcha")
        || url.contains("not_robot")
}

/// Resolved captcha solving strategy based on request mode and client settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptchaMode {
    Auto,
    Manual,
    AutoFallback,
}

/// Selects the solving mode based on request mode and default solve method setting.
pub fn select_mode(mode: &str, solve_method: &str) -> CaptchaMode {
    match mode.to_ascii_lowercase().as_str() {
        "auto" => CaptchaMode::Auto,
        "manual" => CaptchaMode::Manual,
        _ => {
            if solve_method.eq_ignore_ascii_case("auto") {
                CaptchaMode::AutoFallback
            } else {
                CaptchaMode::Manual
            }
        }
    }
}

/// Selects pseudorandom viewport dimensions matching Android CaptchaWebViewManager.
pub fn choose_viewport() -> (i32, i32) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as usize;
    let w = AUTO_VIEWPORT_WIDTHS[nanos % AUTO_VIEWPORT_WIDTHS.len()];
    let h = AUTO_VIEWPORT_HEIGHTS[(nanos / 7) % AUTO_VIEWPORT_HEIGHTS.len()];
    (w, h)
}

/// Internal outcome of a single WebView2 run.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SolveOutcome {
    Success(String),
    SliderDetected,
    Timeout,
    Cancelled,
    Error(String),
}

/// Runs a single hidden WebView2 instance executing automated JS click and slider observation.
fn run_auto_webview(redirect_uri: &str) -> SolveOutcome {
    let (width, height) = choose_viewport();
    let options = Options {
        title: "CSQTT Captcha".to_string(),
        url: redirect_uri.to_string(),
        width,
        height,
        visible: false,
        user_agent: None,
        init_scripts: vec![
            BRIDGE_SHIM_JS.to_string(),
            AUTO_INTERCEPTOR_JS.to_string(),
        ],
        intercept_filters: vec![],
        profile: CAPTCHA_PROFILE.to_string(),
        timeout: CAPTCHA_AUTO_TIMEOUT,
        tick: Some(Duration::from_millis(50)),
    };

    #[derive(Debug, PartialEq, Eq)]
    enum AutoState {
        WaitingNavigationCompleted,
        WaitingPageDelay { deadline: std::time::Instant },
        WaitingFindLabelResult,
        WaitingClickDelay { deadline: std::time::Instant },
        WaitingClickResult,
        WaitingPostClickInitialDelay { deadline: std::time::Instant },
        WatchingSlider { attempts_left: usize, next_check: std::time::Instant },
        WaitingDetectSliderResult { attempts_left: usize },
        WaitingOutcome,
    }

    let mut state = AutoState::WaitingNavigationCompleted;

    let res = webview::run(options, &mut |event, ctl| -> Action {
        match event {
            WebEvent::NavigationStarting { ref url } => {
                if !csqtt_logic::captcha_uri_policy::is_allowed(url) {
                    return Action::Block;
                }
                Action::Continue
            }
            WebEvent::ResourceRequested { ref url, .. } => {
                if !csqtt_logic::captcha_uri_policy::is_allowed(url) {
                    return Action::Block;
                }
                Action::Continue
            }
            WebEvent::NavigationCompleted { ref url, success, .. } => {
                ctl.execute_script(SCRIPT_ID_INIT, AUTO_INTERCEPTOR_JS);
                if is_captcha_page(url) || (success && is_captcha_page(redirect_uri)) {
                    state = AutoState::WaitingPageDelay {
                        deadline: std::time::Instant::now() + Duration::from_millis(700),
                    };
                }
                Action::Continue
            }
            WebEvent::WebMessage { ref json } => {
                if let Some(msg) = parse_bridge_message(json) {
                    match msg {
                        CaptchaBridgeMessage::Success { token } => {
                            return Action::Finish(token);
                        }
                        CaptchaBridgeMessage::SliderDetected { .. } => {
                            return Action::Finish(CSQTT_MARKER_SLIDER_DETECTED.to_string());
                        }
                        CaptchaBridgeMessage::Error { error } => {
                            return Action::Finish(format!("{CSQTT_MARKER_ERROR_PREFIX}{error}"));
                        }
                        CaptchaBridgeMessage::CancelAndStop => {
                            return Action::Finish(CSQTT_MARKER_CANCELLED.to_string());
                        }
                    }
                }
                Action::Continue
            }
            WebEvent::ScriptResult { id, ref json } => {
                match id {
                    SCRIPT_ID_FIND_LABEL => {
                        let res_str = parse_script_result(json);
                        if res_str == ERROR_SLIDER_DETECTED {
                            return Action::Finish(CSQTT_MARKER_SLIDER_DETECTED.to_string());
                        }
                        if res_str == "not_found" || res_str.split(',').count() < 4 {
                            ctl.execute_script(SCRIPT_ID_CLICK, JS_CLICK);
                            state = AutoState::WaitingClickResult;
                        } else {
                            state = AutoState::WaitingClickDelay {
                                deadline: std::time::Instant::now() + Duration::from_millis(500),
                            };
                        }
                    }
                    SCRIPT_ID_CLICK => {
                        state = AutoState::WaitingPostClickInitialDelay {
                            deadline: std::time::Instant::now() + Duration::from_millis(450),
                        };
                    }
                    SCRIPT_ID_DETECT_SLIDER => {
                        let slider_res = parse_script_result(json);
                        if slider_res == "slider" {
                            return Action::Finish(CSQTT_MARKER_SLIDER_DETECTED.to_string());
                        } else if slider_res == "success_ui" {
                            state = AutoState::WaitingOutcome;
                        } else if let AutoState::WaitingDetectSliderResult { attempts_left } = state {
                            if attempts_left > 1 {
                                state = AutoState::WatchingSlider {
                                    attempts_left: attempts_left - 1,
                                    next_check: std::time::Instant::now() + Duration::from_millis(350),
                                };
                            } else {
                                state = AutoState::WaitingOutcome;
                            }
                        }
                    }
                    _ => {}
                }
                Action::Continue
            }
            WebEvent::Tick => {
                let now = std::time::Instant::now();
                match state {
                    AutoState::WaitingPageDelay { deadline } => {
                        if now >= deadline {
                            ctl.execute_script(SCRIPT_ID_FIND_LABEL, FIND_LABEL_JS);
                            state = AutoState::WaitingFindLabelResult;
                        }
                    }
                    AutoState::WaitingClickDelay { deadline } => {
                        if now >= deadline {
                            ctl.execute_script(SCRIPT_ID_CLICK, JS_CLICK);
                            state = AutoState::WaitingClickResult;
                        }
                    }
                    AutoState::WaitingPostClickInitialDelay { deadline } => {
                        if now >= deadline {
                            ctl.execute_script(SCRIPT_ID_DETECT_SLIDER, DETECT_SLIDER_JS);
                            state = AutoState::WaitingDetectSliderResult { attempts_left: 14 };
                        }
                    }
                    AutoState::WatchingSlider { attempts_left, next_check } => {
                        if now >= next_check {
                            ctl.execute_script(SCRIPT_ID_DETECT_SLIDER, DETECT_SLIDER_JS);
                            state = AutoState::WaitingDetectSliderResult { attempts_left };
                        }
                    }
                    _ => {}
                }
                Action::Continue
            }
            WebEvent::Ready | WebEvent::Closed => Action::Continue,
        }
    });

    match res {
        Ok(val) => {
            if val == CSQTT_MARKER_SLIDER_DETECTED {
                SolveOutcome::SliderDetected
            } else if val == CSQTT_MARKER_CANCELLED {
                SolveOutcome::Cancelled
            } else if let Some(err) = val.strip_prefix(CSQTT_MARKER_ERROR_PREFIX) {
                SolveOutcome::Error(err.to_string())
            } else {
                SolveOutcome::Success(val)
            }
        }
        Err(WebViewError::Timeout) => SolveOutcome::Timeout,
        Err(WebViewError::Closed) => SolveOutcome::Cancelled,
        Err(WebViewError::Runtime(m)) => SolveOutcome::Error(format!("WebView2 Runtime: {m}")),
        Err(WebViewError::Other(m)) => SolveOutcome::Error(m),
    }
}

/// Runs a visible manual WebView2 window for the user to solve the captcha.
fn run_manual_webview(redirect_uri: &str) -> SolveOutcome {
    let options = Options {
        title: "Подтверждение капчи".to_string(),
        url: redirect_uri.to_string(),
        width: 450,
        height: 600,
        visible: true,
        user_agent: None,
        init_scripts: vec![
            BRIDGE_SHIM_JS.to_string(),
            MANUAL_INTERCEPTOR_JS.to_string(),
        ],
        intercept_filters: vec![],
        profile: CAPTCHA_PROFILE.to_string(),
        timeout: CAPTCHA_MANUAL_TIMEOUT,
        tick: None,
    };

    let res = webview::run(options, &mut |event, ctl| -> Action {
        match event {
            WebEvent::NavigationStarting { ref url } => {
                if !csqtt_logic::captcha_uri_policy::is_allowed(url) {
                    return Action::Block;
                }
                Action::Continue
            }
            WebEvent::ResourceRequested { ref url, .. } => {
                if !csqtt_logic::captcha_uri_policy::is_allowed(url) {
                    return Action::Block;
                }
                Action::Continue
            }
            WebEvent::NavigationCompleted { .. } => {
                ctl.execute_script(SCRIPT_ID_INIT, MANUAL_INTERCEPTOR_JS);
                ctl.execute_script(SCRIPT_ID_HIDE_ELEMENTS, HIDE_ELEMENTS_JS);
                Action::Continue
            }
            WebEvent::WebMessage { ref json } => {
                if let Some(msg) = parse_bridge_message(json) {
                    match msg {
                        CaptchaBridgeMessage::Success { token } => {
                            return Action::Finish(token);
                        }
                        CaptchaBridgeMessage::Error { error } => {
                            return Action::Finish(format!("{CSQTT_MARKER_ERROR_PREFIX}{error}"));
                        }
                        CaptchaBridgeMessage::CancelAndStop => {
                            return Action::Finish(CSQTT_MARKER_CANCELLED.to_string());
                        }
                        CaptchaBridgeMessage::SliderDetected { .. } => return Action::Continue,
                    }
                }
                Action::Continue
            }
            _ => Action::Continue,
        }
    });

    match res {
        Ok(val) => {
            if val == CSQTT_MARKER_CANCELLED {
                SolveOutcome::Cancelled
            } else if let Some(err) = val.strip_prefix(CSQTT_MARKER_ERROR_PREFIX) {
                SolveOutcome::Error(err.to_string())
            } else {
                SolveOutcome::Success(val)
            }
        }
        Err(WebViewError::Timeout) => SolveOutcome::Timeout,
        Err(WebViewError::Closed) => SolveOutcome::Cancelled,
        Err(WebViewError::Runtime(m)) => SolveOutcome::Error(format!("WebView2 Runtime: {m}")),
        Err(WebViewError::Other(m)) => SolveOutcome::Error(m),
    }
}

/// Solve one captcha request. `mode` = mode from CAPTCHA_SOLVE ("auto" | "manual" | other),
/// `solve_method` = settings `captcha_solve_method` ("auto" | "manual").
/// Returns the token to send back, or "error:<reason>" exactly like TunnelManager.kt
/// (error:timeout, error:cancelled, error:<message>). Blocking; runs WebView2 windows on the
/// calling thread. `on_step` gets the "[КАПЧА WBV] ..." progress texts.
pub fn solve(
    mode: &str,
    redirect_uri: &str,
    session_token: &str,
    solve_method: &str,
    on_step: &mut dyn FnMut(&str),
) -> String {
    if !webview::runtime_available() {
        return "error:WebView2 Runtime не установлен".to_string();
    }

    if !csqtt_logic::captcha_uri_policy::is_allowed(redirect_uri) {
        let err = "Недопустимый адрес captcha";
        on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
        on_step("[КАПЧА WBV] WebView уничтожен");
        return format!("error:{err}");
    }

    let captcha_mode = select_mode(mode, solve_method);

    let res = match captcha_mode {
        CaptchaMode::Auto => {
            on_step("[КАПЧА WBV] Авто WebView попытка 10с...");
            let outcome = run_auto_webview(redirect_uri);
            match outcome {
                SolveOutcome::Success(token) => {
                    on_step("[КАПЧА WBV] Капча решена ✓");
                    token
                }
                SolveOutcome::Timeout => {
                    on_step("[КАПЧА WBV] Таймаут WebView");
                    "error:timeout".to_string()
                }
                SolveOutcome::Cancelled => {
                    on_step("[КАПЧА WBV] Отменено");
                    "error:cancelled".to_string()
                }
                SolveOutcome::SliderDetected => {
                    on_step(&format!("[КАПЧА WBV] {ERROR_SLIDER_DETECTED}"));
                    format!("error:{ERROR_SLIDER_DETECTED}")
                }
                SolveOutcome::Error(err) => {
                    on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
                    format!("error:{err}")
                }
            }
        }
        CaptchaMode::Manual => {
            if session_token.trim().is_empty() {
                let err = "Пустой session token captcha";
                on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
                on_step("[КАПЧА WBV] WebView уничтожен");
                return format!("error:{err}");
            }
            on_step("[КАПЧА WBV] Создание ручного WebView...");
            let outcome = run_manual_webview(redirect_uri);
            match outcome {
                SolveOutcome::Success(token) => {
                    on_step("[КАПЧА WBV] Капча решена ✓");
                    token
                }
                SolveOutcome::Timeout => {
                    on_step("[КАПЧА WBV] Таймаут WebView");
                    "error:timeout".to_string()
                }
                SolveOutcome::Cancelled => {
                    on_step("[КАПЧА WBV] Отменено");
                    "error:cancelled".to_string()
                }
                SolveOutcome::SliderDetected => {
                    on_step(&format!("[КАПЧА WBV] {ERROR_SLIDER_DETECTED}"));
                    format!("error:{ERROR_SLIDER_DETECTED}")
                }
                SolveOutcome::Error(err) => {
                    on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
                    format!("error:{err}")
                }
            }
        }
        CaptchaMode::AutoFallback => {
            let mut fallback_to_manual = false;
            let mut final_result: Option<String> = None;

            for attempt in 1..=2 {
                on_step(&format!("[КАПЧА WBV] Авто WebView попытка {attempt}/2..."));
                let outcome = run_auto_webview(redirect_uri);
                match outcome {
                    SolveOutcome::Success(token) => {
                        on_step("[КАПЧА WBV] Капча решена ✓");
                        final_result = Some(token);
                        break;
                    }
                    SolveOutcome::Cancelled => {
                        on_step("[КАПЧА WBV] Отменено");
                        final_result = Some("error:cancelled".to_string());
                        break;
                    }
                    SolveOutcome::Timeout => {
                        on_step(&format!("[КАПЧА WBV] Авто таймаут 10с ({attempt}/2)"));
                        if attempt == 2 {
                            on_step("[КАПЧА WBV] 2 таймаута авто, открыт ручной WebView");
                            fallback_to_manual = true;
                            break;
                        }
                    }
                    SolveOutcome::SliderDetected => {
                        on_step("[КАПЧА WBV] Обнаружен слайдер, открыт ручной WebView");
                        fallback_to_manual = true;
                        break;
                    }
                    SolveOutcome::Error(err) => {
                        on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
                        final_result = Some(format!("error:{err}"));
                        break;
                    }
                }
            }

            if fallback_to_manual {
                if session_token.trim().is_empty() {
                    let err = "Пустой session token captcha";
                    on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
                    on_step("[КАПЧА WBV] WebView уничтожен");
                    return format!("error:{err}");
                }
                let outcome = run_manual_webview(redirect_uri);
                match outcome {
                    SolveOutcome::Success(token) => {
                        on_step("[КАПЧА WBV] Капча решена ✓");
                        token
                    }
                    SolveOutcome::Timeout => {
                        on_step("[КАПЧА WBV] Таймаут WebView");
                        "error:timeout".to_string()
                    }
                    SolveOutcome::Cancelled => {
                        on_step("[КАПЧА WBV] Отменено");
                        "error:cancelled".to_string()
                    }
                    SolveOutcome::SliderDetected => {
                        on_step(&format!("[КАПЧА WBV] {ERROR_SLIDER_DETECTED}"));
                        format!("error:{ERROR_SLIDER_DETECTED}")
                    }
                    SolveOutcome::Error(err) => {
                        on_step(&format!("[КАПЧА WBV] Ошибка — {err}"));
                        format!("error:{err}")
                    }
                }
            } else {
                final_result.unwrap_or_else(|| "error:cancelled".to_string())
            }
        }
    };

    on_step("[КАПЧА WBV] WebView уничтожен");
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_selection() {
        assert_eq!(select_mode("auto", "auto"), CaptchaMode::Auto);
        assert_eq!(select_mode("AUTO", "manual"), CaptchaMode::Auto);
        assert_eq!(select_mode("manual", "auto"), CaptchaMode::Manual);
        assert_eq!(select_mode("MANUAL", "manual"), CaptchaMode::Manual);
        assert_eq!(select_mode("other", "auto"), CaptchaMode::AutoFallback);
        assert_eq!(select_mode("", "auto"), CaptchaMode::AutoFallback);
        assert_eq!(select_mode("other", "manual"), CaptchaMode::Manual);
        assert_eq!(select_mode("", "other"), CaptchaMode::Manual);
    }

    #[test]
    fn test_parse_bridge_message_success() {
        let msg = parse_bridge_message(r#"{"method":"onSuccess","token":"vk_token_abc"}"#);
        assert_eq!(
            msg,
            Some(CaptchaBridgeMessage::Success {
                token: "vk_token_abc".to_string()
            })
        );

        let msg2 = parse_bridge_message(r#"{"type":"success","token":"vk_token_123"}"#);
        assert_eq!(
            msg2,
            Some(CaptchaBridgeMessage::Success {
                token: "vk_token_123".to_string()
            })
        );

        let nested = parse_bridge_message(r#""{\"method\":\"onSuccess\",\"token\":\"nested_token\"}""#);
        assert_eq!(
            nested,
            Some(CaptchaBridgeMessage::Success {
                token: "nested_token".to_string()
            })
        );

        let direct = parse_bridge_message(r#"{"success_token":"direct_token"}"#);
        assert_eq!(
            direct,
            Some(CaptchaBridgeMessage::Success {
                token: "direct_token".to_string()
            })
        );
    }

    #[test]
    fn test_parse_bridge_message_slider() {
        let msg = parse_bridge_message(r#"{"method":"onSliderDetected","source":"check_response"}"#);
        assert_eq!(
            msg,
            Some(CaptchaBridgeMessage::SliderDetected {
                source: "check_response".to_string()
            })
        );

        let msg2 = parse_bridge_message(r#"{"type":"slider_detected","source":"watcher"}"#);
        assert_eq!(
            msg2,
            Some(CaptchaBridgeMessage::SliderDetected {
                source: "watcher".to_string()
            })
        );
    }

    #[test]
    fn test_parse_bridge_message_error() {
        let msg = parse_bridge_message(r#"{"method":"onError","error":"rate limit"}"#);
        assert_eq!(
            msg,
            Some(CaptchaBridgeMessage::Error {
                error: "rate limit".to_string()
            })
        );

        let msg2 = parse_bridge_message(r#"{"type":"error","error":{"code":100}}"#);
        assert_eq!(
            msg2,
            Some(CaptchaBridgeMessage::Error {
                error: r#"{"code":100}"#.to_string()
            })
        );
    }

    #[test]
    fn test_parse_bridge_message_cancel() {
        let msg = parse_bridge_message(r#"{"method":"onCancelAndStop"}"#);
        assert_eq!(msg, Some(CaptchaBridgeMessage::CancelAndStop));

        let msg2 = parse_bridge_message(r#"{"type":"cancel_and_stop"}"#);
        assert_eq!(msg2, Some(CaptchaBridgeMessage::CancelAndStop));
    }

    #[test]
    fn test_parse_bridge_message_invalid() {
        assert_eq!(parse_bridge_message("not json"), None);
        assert_eq!(parse_bridge_message(r#"{"unknown":123}"#), None);
    }

    #[test]
    fn test_parse_script_result() {
        assert_eq!(parse_script_result(r#""slider_detected""#), "slider_detected");
        assert_eq!(parse_script_result(r#""not_found""#), "not_found");
        assert_eq!(
            parse_script_result(r#""12.5,34.0,200.0,50.0""#),
            "12.5,34.0,200.0,50.0"
        );
        assert_eq!(parse_script_result("slider_detected"), "slider_detected");
    }

    #[test]
    fn test_is_captcha_page() {
        assert!(is_captcha_page("https://vk.com/not_robot_captcha?hash=123"));
        assert!(is_captcha_page("https://id.vk.ru/captcha?sid=456"));
        assert!(is_captcha_page("https://vk.com/some_not_robot_path"));
        assert!(!is_captcha_page("https://vk.com/feed"));
        assert!(!is_captcha_page("https://example.com"));
    }

    #[test]
    fn test_viewport_selection() {
        let (w, h) = choose_viewport();
        assert!(AUTO_VIEWPORT_WIDTHS.contains(&w));
        assert!(AUTO_VIEWPORT_HEIGHTS.contains(&h));
    }
}
