//! Offline fail-soft proofs for the real HTTP transport.
//!
//! No provider is contacted: a local stub server stands in for the TypeSafe
//! System One endpoint, and an unroutable port stands in for a down one. The
//! tests pin the documented fail-soft contract — timeouts, one retry on
//! transport errors and retryable statuses (429/529), immediate failure on
//! other statuses, soft failure on malformed envelopes — and that `evaluate`
//! never panics even when called from a current-thread tokio runtime.

use axum::{Json, Router, routing::post};
use jev_client::http_transport::HttpJevTransport;
use jev_client::{JevClient, JevConfig, JevOutcome, JevQuestion, JevRequest};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::sleep;

fn request() -> JevRequest {
    JevRequest {
        state: json!({ "candidates": ["Player B"] }),
        questions: vec![(
            "pick".to_string(),
            JevQuestion::Noul {
                instructions: "Is there any candidate?".to_string(),
                criteria: None,
            },
        )],
    }
}

/// The blocking-side seam, driven exactly as application code drives it.
fn judge(config: JevConfig) -> JevOutcome {
    JevClient::new(config, HttpJevTransport::new()).evaluate(&request())
}

fn config_at(port: u16) -> JevConfig {
    let mut config = JevConfig::new("test-key-not-a-real-credential".to_string());
    config.base_url = format!("http://127.0.0.1:{port}");
    // Keep the suite fast: the retry sleeps briefly on failure.
    config.timeout_secs = 2;
    config
}

/// Bind a stub provider on an ephemeral port and serve `routes`; returns the
/// port. `spawn_blocking` gives each judge call a real blocking thread, the
/// way the server runs it — so current-thread-runtime behavior is not tested
/// by accident.
async fn stub(routes: Router) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("stub binds");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        axum::serve(listener, routes).await.expect("stub serves");
    });
    // Give the accept loop a beat to start.
    sleep(Duration::from_millis(50)).await;
    port
}

#[tokio::test]
async fn an_unreachable_provider_fails_soft() {
    // Port 1 on loopback is refused connection in practice.
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(1)))
        .await
        .expect("join");
    assert_eq!(outcome, JevOutcome::Unavailable);
}

#[tokio::test]
async fn evaluate_panics_never_escape_a_current_thread_runtime() {
    // The default #[tokio::test] flavor is current-thread; the blocking-side
    // transport must fail soft here, not panic ("Cannot start a runtime from
    // within a runtime") — the failure mode probed by the T11 spec review.
    let outcome = judge(config_at(1));
    assert_eq!(outcome, JevOutcome::Unavailable);
}

#[tokio::test]
async fn a_retryable_status_retries_once_and_recovers() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = hits.clone();
    let routes = Router::new().route(
        "/v1/systemone",
        post(move || async move {
            let n = hits_for_route.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                (
                    axum::http::StatusCode::from_u16(529).expect("529 builds"),
                    Json(json!(null)),
                )
            } else {
                (
                    axum::http::StatusCode::OK,
                    Json(json!({
                        "answers": { "pick": { "type": "noul", "noul": 0.9 } }
                    })),
                )
            }
        }),
    );
    let port = stub(routes).await;
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    let jev_client::JevOutcome::Answers(answers) = outcome else {
        panic!("a 529-then-200 pair recovers on the one retry: got {outcome:?}");
    };
    assert_eq!(answers.len(), 1);
    assert_eq!(hits.load(Ordering::SeqCst), 2, "exactly one retry fired");
}

#[tokio::test]
async fn a_retryable_status_exhausting_its_retry_fails_soft() {
    let routes = Router::new().route(
        "/v1/systemone",
        post(|| async { (axum::http::StatusCode::TOO_MANY_REQUESTS, Json(json!(null))) }),
    );
    let port = stub(routes).await;
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    assert_eq!(outcome, JevOutcome::Unavailable);
}

#[tokio::test]
async fn non_retryable_statuses_fail_immediately() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = hits.clone();
    let routes = Router::new().route(
        "/v1/systemone",
        post(move || async move {
            hits_for_route.fetch_add(1, Ordering::SeqCst);
            (axum::http::StatusCode::UNAUTHORIZED, Json(json!(null)))
        }),
    );
    let port = stub(routes).await;
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    assert_eq!(outcome, JevOutcome::Unavailable, "401 fails immediately");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "no retry after a non-retryable status"
    );
}

#[tokio::test]
async fn a_malformed_envelope_fails_soft() {
    let routes = Router::new().route(
        "/v1/systemone",
        post(|| async { Json(json!({ "answers": "not-an-object" })) }),
    );
    let port = stub(routes).await;
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    assert_eq!(outcome, JevOutcome::Unavailable);
}

#[tokio::test]
async fn an_oversized_reply_fails_soft_without_being_parsed() {
    let routes = Router::new().route(
        "/v1/systemone",
        post(|| async {
            let padding = "x".repeat(2 * 1024 * 1024);
            Json(json!({
                "answers": { "pick": { "type": "noul", "noul": 0.5 } },
                "padding": padding,
            }))
        }),
    );
    let port = stub(routes).await;
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    assert_eq!(outcome, JevOutcome::Unavailable);
}

#[tokio::test]
async fn a_reply_cut_off_before_its_declared_length_is_not_trusted() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut scratch = [0u8; 4096];
            let _ = socket.read(&mut scratch).await;
            // A complete, valid envelope that is shorter than the declared length.
            let body = r#"{"answers":{"pick":{"type":"noul","noul":0.5}}}"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                body.len() + 500
            );
            let _ = socket.write_all(head.as_bytes()).await;
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    assert_eq!(outcome, JevOutcome::Unavailable);
}

#[tokio::test]
async fn a_provider_timeout_fails_soft() {
    let routes = Router::new().route(
        "/v1/systemone",
        post(|| async {
            sleep(Duration::from_secs(5)).await;
            Json(json!({ "answers": {} }))
        }),
    );
    let port = stub(routes).await;
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .expect("join");
    assert_eq!(
        outcome,
        JevOutcome::Unavailable,
        "a request exceeding the 2s config timeout fails soft"
    );
}
#[tokio::test]
async fn measured_judgments_keep_metadata_local_to_each_call_and_missing_values_unknown() {
    let routes=Router::new().route("/v1/systemone",post(|Json(payload):Json<serde_json::Value>|async move {
        let label=payload["state"]["label"].as_str().unwrap_or("unknown");
        Json(json!({"id":label,"model":"stub-jev","provider":"stub","usage":{"input_tokens":123,"output_tokens":17,"cost":0.000021},"answers":{"pick":{"type":"noul","noul":0.9}}}))
    }));
    let port = stub(routes).await;
    let (a, b) = tokio::join!(
        tokio::task::spawn_blocking(move || {
            let mut r = request();
            r.state = json!({"label":"a"});
            JevClient::new(config_at(port), HttpJevTransport::new()).evaluate_measured(&r)
        }),
        tokio::task::spawn_blocking(move || {
            let mut r = request();
            r.state = json!({"label":"b"});
            JevClient::new(config_at(port), HttpJevTransport::new()).evaluate_measured(&r)
        })
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(a.outcome.is_available());
    assert!(a.elapsed_ms >= 0.0);
    let metadata = a.metadata.unwrap();
    assert_eq!(metadata.request_id.as_deref(), Some("a"));
    assert_eq!(b.metadata.unwrap().request_id.as_deref(), Some("b"));
    assert_eq!(metadata.input_tokens, Some(123));
    assert_eq!(metadata.output_tokens, Some(17));
    assert_eq!(metadata.cost_usd, Some(0.000021));
    let client = JevClient::new(config_at(1), jev_client::MemoryTransport::default());
    assert!(client.evaluate_measured(&request()).metadata.is_none());
}
#[tokio::test]
async fn transport_error_then_retryable_status_does_not_create_a_third_attempt() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let capture = hits.clone();
    let server = tokio::spawn(async move {
        for number in 0..3 {
            let accepted =
                tokio::time::timeout(Duration::from_millis(500), listener.accept()).await;
            let Ok(Ok((mut socket, _))) = accepted else {
                break;
            };
            let mut buf = [0; 8192];
            let _ = socket.read(&mut buf).await;
            capture.fetch_add(1, Ordering::SeqCst);
            if number == 0 {
                continue;
            } // EOF before response causes a transport error.
            let response = if number == 1 {
                "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 51\r\nConnection: close\r\n\r\n{\"answers\":{\"pick\":{\"type\":\"noul\",\"noul\":0.9}}}"
            };
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let outcome = tokio::task::spawn_blocking(move || judge(config_at(port)))
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "one retry total across transport/status failures"
    );
    assert_eq!(outcome, JevOutcome::Unavailable);
}
#[tokio::test]
async fn all_attempts_and_response_reads_share_one_call_deadline() {
    let routes = Router::new().route(
        "/v1/systemone",
        post(|| async {
            sleep(Duration::from_millis(700)).await;
            (axum::http::StatusCode::TOO_MANY_REQUESTS, Json(json!(null)))
        }),
    );
    let port = stub(routes).await;
    let measured = tokio::task::spawn_blocking(move || {
        let mut c = config_at(port);
        c.timeout_secs = 1;
        JevClient::new(c, HttpJevTransport::new()).evaluate_measured(&request())
    })
    .await
    .unwrap();
    assert_eq!(measured.outcome, JevOutcome::Unavailable);
    assert!(
        measured.elapsed_ms < 1250.0,
        "one-second total deadline, observed {}ms",
        measured.elapsed_ms
    );
}
