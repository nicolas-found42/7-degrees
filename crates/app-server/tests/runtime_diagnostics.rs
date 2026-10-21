//! Runtime diagnostics: the surfaces that make a past answer diagnosable.
//!
//! An answer of this server is a function of the code that produced it, the
//! snapshot it loaded and the request it served. These tests drive the real
//! HTTP seam and assert that the server can still say which build answered,
//! which snapshot it answered from, what it served, and why a semantic
//! judgment did not produce an answer.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Run one request against one router and return its status and JSON body.
async fn call(app: &Router, request: Request<Body>) -> (StatusCode, Option<Value>) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("in-process request completes");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body reads")
        .to_bytes();
    let json = if body.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(&body).expect("body is JSON"))
    };
    (status, json)
}

/// Run one GET request and return its status and JSON body.
async fn get(app: &Router, path: &str) -> (StatusCode, Option<Value>) {
    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request builds");
    call(app, request).await
}

#[tokio::test]
async fn diagnostics_report_the_build_the_snapshot_and_the_served_requests() {
    let app = app_server::app_with_fixture_data();
    let (status, _) = get(&app, "/api/stats").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = get(&app, "/api/diagnostics").await;
    assert_eq!(status, StatusCode::OK);
    let body = body.expect("diagnostics body");
    assert!(
        body["build"]["commit"].is_string(),
        "the build names its commit: {body}"
    );
    assert!(
        body["build"]["dirty"].is_boolean(),
        "the build says whether its tree was dirty: {body}"
    );
    assert_eq!(body["build"]["mode"], "fixture");
    assert!(
        body["requests"]["total"].as_u64().expect("request total") >= 1,
        "the served requests are counted: {body}"
    );
    let recent = body["requests"]["recent"]
        .as_array()
        .expect("recent request records");
    assert!(
        recent.iter().any(|record| record["path"] == "/api/stats"),
        "the served request is on record: {recent:?}"
    );
    for record in recent {
        assert!(record["request_id"].is_string());
        assert!(record["method"].is_string());
        assert!(record["status"].as_u64().is_some());
        assert!(record["duration_ms"].as_f64().is_some());
        let path = record["path"].as_str().expect("record path");
        assert!(
            !path.contains('?'),
            "a record keeps the path, never the query string: {path}"
        );
    }
    assert!(
        body["snapshot"]["players"].as_u64().expect("player count") >= 1,
        "the snapshot names its size: {body}"
    );
    assert!(body["snapshot"]["edges"].as_u64().is_some());
    assert!(body["snapshot"]["sources"].is_array());
    assert!(body["snapshot"]["assets"].is_array());
}

#[tokio::test]
async fn a_judgment_is_recorded_with_the_cause_of_its_failure() {
    let app = app_server::app_with_fixture_data();
    let (status, _) = get(&app, "/api/query?q=who%20played%20with%20Magic%20Johnson").await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = get(&app, "/api/diagnostics").await;
    let body = body.expect("diagnostics body");
    assert_eq!(body["semantic"]["state"], "unconfigured");
    let events = body["semantic"]["events"]
        .as_array()
        .expect("semantic events");
    assert!(!events.is_empty(), "the judgment is on record: {body}");
    let event = events.last().expect("the newest judgment");
    assert_eq!(event["outcome"], "unavailable");
    assert_eq!(
        event["failure"], "unconfigured",
        "the failure is named, not just absent: {event}"
    );
    for forbidden in [
        "model",
        "provider",
        "request_id",
        "input_tokens",
        "output_tokens",
        "cost_usd",
    ] {
        assert!(
            event.get(forbidden).is_none(),
            "the browser-facing event carries no {forbidden}: {event}"
        );
    }
    let (_, status_body) = get(&app, "/api/semantic-status").await;
    let status_body = status_body.expect("status body");
    assert_eq!(status_body["reason"], "unconfigured");
    assert_eq!(
        status_body["last_failure"], "unconfigured",
        "the status surface names the cause too: {status_body}"
    );
}

#[tokio::test]
async fn a_client_failure_report_is_accepted_and_kept() {
    let app = app_server::app_with_fixture_data();
    let payload = json!({
        "kind": "fetch_failed",
        "message": "HTTP 500",
        "url": "/api/search?q=abba",
        "context": {"status": 500},
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/diagnostics/client")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("request builds");
    let (status, _) = call(&app, request).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (_, body) = get(&app, "/api/diagnostics").await;
    let body = body.expect("diagnostics body");
    let reports = body["client_reports"].as_array().expect("client reports");
    let report = reports.last().expect("the report is kept");
    assert_eq!(report["kind"], "fetch_failed");
    assert_eq!(report["message"], "HTTP 500");
    assert_eq!(
        report["url"], "/api/search",
        "the report keeps the path, never the query string: {report}"
    );
    assert_eq!(report["context"]["status"], 500);
}

#[tokio::test]
async fn the_diagnostics_body_carries_no_credential_material() {
    let app = app_server::app_with_fixture_data();
    let (status, _) = get(&app, "/api/query?q=who%20played%20with%20Magic%20Johnson").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = get(&app, "/api/diagnostics").await;
    assert_eq!(status, StatusCode::OK);
    let text = body.expect("diagnostics body").to_string();
    for secret in ["sk-", "api_key", "apiKey", "Authorization", "bearer"] {
        assert!(
            !text.contains(secret),
            "the diagnostics body carries no {secret}: {text}"
        );
    }
}
