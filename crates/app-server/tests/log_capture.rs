//! Captured-log credential-isolation proof (runs in its own test process).
//!
#![cfg(test_capture)]
//!
//! This target installs no logger and never logs key material, so a real
//! logger observing `log` records from this process sees only records that
//! lack the canary — while traffic flows with a configured-but-failing Jev
//! client holding the canary as its key.
//!
//! Compiled only under `--cfg test_capture` — enable with
//! `RUSTFLAGS='--cfg test_capture' cargo test`; without that flag this target
//! compiles empty and the standard suite skips it.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jev_client::{JevClient, JevConfig, JevOutcome, MemoryTransport};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn the_credential_canary_never_appears_in_any_log_record() {
    let canary = "sk-canary-never-log-9012";
    let handle = app_server::JevHandle::from_client(JevClient::new(
        JevConfig::new(canary.to_string()),
        MemoryTransport::scripted(vec![]),
    ));

    // Drive the configured-but-failing provider and the served surfaces.
    let request = jev_client::JevRequest {
        state: json!({ "candidates": ["Player B"] }),
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
    // facade: no canary, no credential prefix.
    let captured = app_server::test_log_capture()
        .lock()
        .expect("capture buffer");
    assert!(
        !captured.contains(canary),
        "credential canary leaked into logs: {captured}"
    );
    assert!(
        !captured.to_ascii_lowercase().contains("sk-canary"),
        "credential prefix leaked into logs: {captured}"
    );
}
