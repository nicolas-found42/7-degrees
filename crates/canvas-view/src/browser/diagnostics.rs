//! Client-side failure reports, relayed to the server's diagnostics surface.
//!
//! The canvas runs in the browser, so a failure it sees is invisible to the
//! server unless it is reported. Every failed fetch, every response it could
//! not read and every uncaught error is posted to the server's client
//! channel, where the diagnostics surface keeps it next to the requests and
//! the semantic events of the same run.
//!
//! A report is best-effort: it never changes what the page does, and a
//! failing report is dropped silently.

use serde_json::{Value, json};
use web_sys::{Headers, Request, RequestInit};

/// The endpoint that receives client-side failure reports.
const ENDPOINT: &str = "/api/diagnostics/client";

/// Reports one client-side failure to the server.
pub(super) fn report(kind: &str, message: &str, url: &str, context: Value) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let body = json!({
        "kind": kind,
        "message": message,
        "url": url,
        "context": context,
    });
    let Ok(body) = serde_json::to_string(&body) else {
        return;
    };
    let Ok(headers) = Headers::new() else {
        return;
    };
    if headers.set("Content-Type", "application/json").is_err() {
        return;
    }
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&wasm_bindgen::JsValue::from_str(&body));
    let Ok(request) = Request::new_with_str_and_init(ENDPOINT, &init) else {
        return;
    };
    // Fire and forget: the report must not gate the interaction that
    // produced it, and its own failure is not worth a second report.
    let _ = window.fetch_with_request(&request);
}

/// The page URL without its query string: a report never carries the free
/// text a query string holds.
pub(super) fn page_path() -> String {
    web_sys::window()
        .and_then(|window| window.location().pathname().ok())
        .unwrap_or_default()
}

/// Describes a JavaScript error value as text.
pub(super) fn describe(error: &wasm_bindgen::JsValue) -> String {
    if let Some(text) = error.as_string() {
        return text;
    }
    js_sys::Reflect::get(error, &wasm_bindgen::JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .unwrap_or_else(|| "transport failure".to_string())
}

/// Describes the reason of a rejected promise as text.
pub(super) fn describe_reason(reason: &wasm_bindgen::JsValue) -> String {
    if let Some(text) = reason.as_string() {
        return text;
    }
    if let Ok(message) = js_sys::Reflect::get(reason, &wasm_bindgen::JsValue::from_str("message"))
        && let Some(text) = message.as_string()
    {
        return text;
    }
    js_sys::JSON::stringify(reason)
        .ok()
        .and_then(|text| text.as_string())
        .unwrap_or_else(|| "rejected promise".to_string())
}
