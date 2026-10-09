//! Captured-log credential-isolation proof (runs in its own test process).
//!
#![cfg(test_capture)]
//!
//! The test itself installs the capture logger (nothing else does — the
//! harness installs no logger), then drives traffic with a REAL HTTP
//! transport whose API key is the canary and whose provider endpoint is
//! unreachable, so the fail-soft `warn!` paths actually fire. The capture
//! buffer must therefore be non-empty (proof the logger observed real
//! records) and must contain no trace of the canary key.
//!
//! Compiled only under `--cfg test_capture` — enable with
//! `RUSTFLAGS='--cfg test_capture' cargo test`; without that flag this target
//! compiles empty and the standard suite skips it.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jev_client::{JevClient, JevConfig, JevOutcome};
use tower::ServiceExt;

#[tokio::test]
async fn the_credential_canary_never_appears_in_any_log_record() {
    let canary = "sk-canary-never-log-9012";

    // A real HTTP transport with the canary as its key, pointed at a
    // refused port: every documented failure path runs (connect error,
    // one retry), and the provider-unreachable warnings fire — the exact
    // records a leak would ride on.
    let mut config = JevConfig::new(canary.to_string());
    config.base_url = "http://127.0.0.1:1".to_string();
    config.timeout_secs = 2;
    let client = JevClient::new(config, jev_client::http_transport::HttpJevTransport::new());
    let handle = app_server::JevHandle::from_client(client);

    // Install the capture logger BEFORE any app traffic (the review found
    // the previous version never installed it, so its buffer was empty and
    // every assertion was vacuous).
    app_server::install_capture_logger();

    let request = jev_client::JevRequest {
        state: serde_json::json!({ "candidates": ["Player B"] }),
        questions: vec![(
            "pick".to_string(),
            jev_client::JevQuestion::Choice {
                instructions: "Which candidate is intended?".to_string(),
                criteria: vec![("b".to_string(), "Player B".to_string())],
            },
        )],
    };
    assert_eq!(handle.judge(&request), JevOutcome::Unavailable);

    let app = app_server::app_with_jev(handle);
    for path in [
        "/api/connection?from=A&to=B",
        "/api/semantic-status",
        "/",
        "/chain?from=A&to=C",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("in-process request completes");
        assert_eq!(response.status(), StatusCode::OK, "{path} serves");
        let _ = response.into_body().collect().await.expect("body reads");
    }

    // Everything the app logged during the run, captured through the log
    // facade. The buffer MUST be non-empty: the unreachable-provider path
    // warns twice, so an empty buffer here would mean the logger observed
    // nothing and this proof was vacuous.
    let captured = app_server::test_log_capture()
        .lock()
        .expect("capture buffer");
    assert!(
        captured.contains("WARN"),
        "the logger observed real records (non-empty capture): {captured:?}"
    );
    assert!(
        !captured.contains(canary),
        "credential canary leaked into logs: {captured}"
    );
    assert!(
        !captured.to_ascii_lowercase().contains("sk-canary"),
        "credential prefix leaked into logs: {captured}"
    );
}
