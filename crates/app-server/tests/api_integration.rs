//! Fixture-backed API and UI integration tests.
//!
//! This is the approved API integration seam (spec §Testing Decisions): tests
//! feed the synthetic roster fixture through the same data-build and app
//! interface a real data import would use, and assert only externally
//! observable HTTP behavior — status codes, JSON bodies, and rendered UI
//! content. The UI itself is server-rendered Rust (spec Rust constraint: no
//! JavaScript UI/graph libraries), so its pages are tested through the same
//! in-process HTTP seam. The fixture is the spec's Concrete Example (Players
//! A–D, Teams Red/Blue) plus fixture-only players C2 and E covering the
//! mid-season-move and repeated-overlap rules.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jev_client::{JevAnswer, JevClient, JevConfig, JevOutcome, MemoryTransport};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Run one HTTP request against the app with the fixture graph preloaded.
async fn get(path: &str) -> (StatusCode, Option<Value>) {
    let app = app_server::app_with_fixture_data();
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request builds"),
        )
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

/// Run one HTTP request against the app, returning status and body text.
async fn get_html(path: &str) -> (StatusCode, String) {
    let app = app_server::app_with_fixture_data();
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("in-process request completes");
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .expect("header is text")
        .to_string();
    assert!(
        content_type.starts_with("text/html"),
        "{path} serves HTML, got {content_type}"
    );
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body reads")
        .to_bytes();
    (
        status,
        String::from_utf8(body.to_vec()).expect("body is UTF-8"),
    )
}

#[tokio::test]
async fn overlap_tenures_on_same_team_create_edge() {
    // A (Red day 1–5) and B (Red day 2–4) overlap on Red: direct teammates,
    // even though they share no asserted game appearance.
    let (status, body) = get("/api/connection?from=A&to=B").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["result"], "connected");
    assert_eq!(connection["degree"], 1);
    assert_eq!(connection["path"], json!(["A", "B"]));
    assert_eq!(connection["links"][0]["team"], "Red");
    assert_eq!(connection["links"][0]["overlap_days"], 2);
}

#[tokio::test]
async fn non_overlapping_same_franchise_tenures_create_no_edge() {
    // D joins Red only after A's Red tenure [1,5) ends, so the shared Red
    // franchise history alone must not link them.
    let (status, body) = get("/api/connection?from=A&to=D").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["result"], "disconnected");
    assert!(
        connection["path"].is_null(),
        "no path for a disconnected pair"
    );
    assert!(connection["degree"].is_null(), "no degree without a chain");
}

#[tokio::test]
async fn shortest_chain_reaches_second_degree_through_b() {
    let (status, body) = get("/api/connection?from=A&to=C").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["degree"], 2);
    assert_eq!(connection["path"], json!(["A", "B", "C"]));
    // Ordered players and links: n players in the chain => n-1 links, and the
    // link count equals the reported degree of separation.
    let links = connection["links"].as_array().expect("links");
    assert_eq!(links.len(), 2);
    assert_eq!(links[0]["from"], "A");
    assert_eq!(links[0]["to"], "B");
    assert_eq!(links[0]["team"], "Red");
    assert_eq!(links[0]["overlap_days"], 2);
    assert_eq!(links[1]["from"], "B", "links follow path order");
    assert_eq!(links[1]["to"], "C");
    assert_eq!(links[1]["team"], "Blue");
    assert_eq!(links[1]["overlap_days"], 1);
}

#[tokio::test]
async fn mid_season_move_links_only_overlapping_stints() {
    // B is on Red [2,4) and later on Blue [10,13); the Blue stint
    // links B to C2 (Blue [11,15), overlap 2 days) as DIRECT teammates...
    let (status, body) = get("/api/connection?from=B&to=C2").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(
        connection["degree"], 1,
        "B's Blue stint directly links B–C2"
    );
    assert_eq!(connection["links"][0]["team"], "Blue");
    assert_eq!(connection["links"][0]["overlap_days"], 2);

    // ...while B's Red stint must NOT link B to D (Red [20,25)): the stints do
    // not overlap, so same-franchise history stays unconnected.
    let (status, body) = get("/api/connection?from=B&to=D").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["result"], "disconnected");

    // A therefore reaches C2 only through B's overlapping Blue stint.
    let (status, body) = get("/api/connection?from=A&to=C2").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["degree"], 2);
    assert_eq!(connection["path"], json!(["A", "B", "C2"]));
}

#[tokio::test]
async fn repeated_overlaps_collapse_to_one_edge() {
    // A and E overlap twice on Team Green ([6,8) and [9,11)); the pair must
    // still have exactly one teammate edge whose overlap days sum to 4.
    let (status, body) = get("/api/edges/E").await;
    assert_eq!(status, StatusCode::OK);
    let edges = body.expect("JSON body present");
    let e_pairs: Vec<&Value> = edges["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .filter(|edge| {
            let a = edge["a"].as_str().unwrap();
            let b = edge["b"].as_str().unwrap();
            let (x, y) = ("A", "E");
            (a == x && b == y) || (a == y && b == x)
        })
        .collect();
    assert_eq!(
        e_pairs.len(),
        1,
        "exactly one A–E edge despite two overlaps"
    );
    assert_eq!(
        e_pairs[0]["overlap_days"], 4,
        "overlap days sum across stints"
    );

    // The deduplication also holds in the graph-wide edge list.
    let (status, body) = get("/api/edges").await;
    assert_eq!(status, StatusCode::OK);
    let edges = body.expect("JSON body present");
    let pairs: Vec<(String, String)> = edges["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .map(|edge| {
            (
                edge["a"].as_str().unwrap().to_string(),
                edge["b"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(pairs.len(), 5, "no duplicate (or unexpected) edges");
    let mut sorted = pairs.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            ("A".to_string(), "B".to_string()),
            ("A".to_string(), "E".to_string()),
            ("B".to_string(), "C".to_string()),
            ("B".to_string(), "C2".to_string()),
            ("C".to_string(), "C2".to_string()),
        ]
    );
}

#[tokio::test]
async fn direct_teammates_report_degree_one() {
    let (status, body) = get("/api/connection?from=B&to=C2").await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["degree"], 1);
    assert_eq!(connection["path"], json!(["B", "C2"]));
}

#[tokio::test]
async fn unknown_player_is_a_not_found_result() {
    let (status, body) = get("/api/connection?from=A&to=Zed").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["error"], "player-not-found");
}

#[tokio::test]
async fn fixture_summary_lists_all_players_and_edges() {
    let (status, body) = get("/api/fixture").await;
    assert_eq!(status, StatusCode::OK);
    let summary = body.expect("JSON body present");
    let players: Vec<&str> = summary["players"]
        .as_array()
        .expect("players array")
        .iter()
        .map(|p| p["id"].as_str().expect("player id string"))
        .collect();
    assert_eq!(
        players,
        ["A", "B", "C", "C2", "D", "E"],
        "every fixture player appears in the graph"
    );
    assert_eq!(summary["edges"].as_array().map(Vec::len), Some(5));
}

#[tokio::test]
async fn statistics_report_components_diameter_and_unreachable_pairs() {
    // Component {A,B,C,C2,E}; D is an isolate. Reachable unordered pairs:
    // distance 1 = the 5 edges, distance 2 = A–C, A–C2, B–E, distance 3 =
    // C–E and C2–E (via A and B). Every pair involving D is unreachable
    // (5 pairs).
    let (status, body) = get("/api/stats").await;
    assert_eq!(status, StatusCode::OK);
    let stats = body.expect("JSON body present");
    assert_eq!(stats["players"], 6);
    assert_eq!(stats["components"], 2, "{{A,B,C,C2,E}} plus isolated D");
    assert_eq!(stats["diameter"], 3, "max finite distance is C–E at 3");
    assert_eq!(stats["unreachable_pairs"], 5, "all pairs involving D");
    assert_eq!(stats["histogram"]["1"], 5);
    assert_eq!(stats["histogram"]["2"], 3);
    assert_eq!(stats["histogram"]["3"], 2, "C–E and C2–E both at 3");
}

#[tokio::test]
async fn all_paths_returns_alternative_shortest_chains() {
    // A→C has one shortest chain (degree 2); /api/paths returns it and any
    // equally short alternatives with matching link evidence.
    let (status, body) = get("/api/paths?from=A&to=C").await;
    assert_eq!(status, StatusCode::OK);
    let paths = body.expect("JSON body present");
    let paths = paths["paths"].as_array().expect("paths array");
    assert_eq!(paths.len(), 1, "A→C has exactly one minimal chain");
    assert_eq!(paths[0]["path"], json!(["A", "B", "C"]));
    assert_eq!(paths[0]["degree"], 2);
    assert_eq!(paths[0]["links"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn missing_query_parameters_are_a_bad_request() {
    let (status, body) = get("/api/connection").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = body.expect("JSON body present");
    assert_eq!(error["error"], "missing-parameter");
    let (status, _) = get("/api/paths?from=A").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// --- The minimal UI: server-rendered HTML via the same in-process seam. ---

#[tokio::test]
async fn home_page_shows_connect_form_and_fixture_edges() {
    let (status, html) = get_html("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("7 Degrees"), "app title renders");
    assert!(
        html.contains(r#"<form method="get" action="/chain""#),
        "connect form targets the chain page"
    );
    assert!(html.contains("Player A"), "player display names render");
    for pair in [
        "Player A — Player B",
        "Player B — Player C",
        "Player B — Player C2",
        "Player C — Player C2",
    ] {
        assert!(html.contains(pair), "edge list shows {pair}: {html}");
    }
    // A–E: two overlapping stints, exactly one deduplicated edge.
    let a_e_rows: Vec<&str> = html
        .lines()
        .flat_map(|line| line.split("<li>"))
        .filter(|part| part.contains("Player A — Player E"))
        .collect();
    assert_eq!(a_e_rows.len(), 1, "exactly one A–E edge row: {html}");
    assert!(
        html.contains("teammate edge on Green (4 day(s))"),
        "A–E overlap days sum across stints: {html}"
    );
    assert!(html.contains("Red (2 day(s))"), "edge provenance renders");
    assert!(!html.contains("<script"), "no JavaScript ships in the UI");
}

#[tokio::test]
async fn chain_page_orders_players_and_links_with_degree() {
    // The fixture's headline question: A connects to C through B at degree 2.
    let (status, html) = get_html("/chain?from=A&to=C").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Shortest teammate chain"));
    assert!(
        html.contains("Degree of separation: <strong>2</strong>"),
        "{html}"
    );
    assert!(html.contains("Player B"), "midlink player renders by name");
    // Ordered players: A before B before C in the rendered path list.
    let a_pos = html.find("Player A").expect("A in path");
    let b_pos = html.find("Player B").expect("B in path");
    let c_pos = html.find("Player C").expect("C in path");
    assert!(
        a_pos < b_pos && b_pos < c_pos,
        "path players render in order"
    );
    // Ordered links with evidence: A–B on Red before B–C on Blue.
    assert!(
        html.find("Player A → Player B")
            .expect("A–B link is present")
            < html
                .find("Player B → Player C")
                .expect("B–C link is present"),
        "links render in chain order"
    );
    assert!(html.contains("teammates on Red, overlapping roster tenure: 2 day(s)"));
    assert!(html.contains("teammates on Blue, overlapping roster tenure: 1 day(s)"));
}

#[tokio::test]
async fn chain_page_shows_direct_and_disconnected_results() {
    // Direct teammates report degree 1.
    let (status, html) = get_html("/chain?from=A&to=B").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Degree of separation: <strong>1</strong>"));

    // Same-franchise non-overlap stays unconnected in the UI, too.
    let (status, html) = get_html("/chain?from=A&to=D").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("No teammate chain connects Player A and Player D."),
        "disconnected pair explains itself: {html}"
    );
}

#[tokio::test]
async fn chain_page_reports_unknown_players() {
    let (status, html) = get_html("/chain?from=A&to=Zed").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("No player node for id \"Zed\""), "{html}");
}

#[tokio::test]
async fn chain_page_defaults_to_the_fixture_demo_pair() {
    let (status, html) = get_html("/chain").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(r#"value="A" selected"#),
        "from defaults to A: {html}"
    );
    assert!(html.contains(r#"value="C" selected"#), "to defaults to C");
}

#[tokio::test]
async fn stylesheet_is_served() {
    let app = app_server::app_with_fixture_data();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/style.css")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("in-process request completes");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("content-type header")
        .to_str()
        .expect("header is text")
        .to_string();
    assert!(
        content_type.starts_with("text/css"),
        "stylesheet content type"
    );
}

#[tokio::test]
async fn unknown_ui_route_is_a_html_not_found_page() {
    let (status, html) = get_html("/somewhere/else").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        html.contains("This page does not exist"),
        "HTML 404 page renders"
    );
    // API routes keep their JSON 404.
    let (status, body) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.expect("JSON body")["error"], "not-found");
}

// --- Jev availability: status surface, safe fallback, credential isolation.
//
// Spec §"Jev at runtime" + acceptance criteria: with Jev unconfigured or
// unreachable, deterministic search/graph features keep working unchanged and
// the UI reports semantic features as unavailable; credentials never reach
// the browser, responses, or routine logs.

use app_server::JevHandle;

/// A Jev handle from a scripted in-memory transport (no network, no key).
fn scripted_handle(outcomes: Vec<JevOutcome>) -> JevHandle {
    let client = JevClient::new(
        JevConfig::new("test-only-key-not-real".to_string()),
        MemoryTransport::scripted(outcomes),
    );
    JevHandle::from_client(client)
}

/// Run one request against an app with an explicit Jev handle.
async fn get_with_jev(path: &str, handle: JevHandle) -> (StatusCode, Option<Value>) {
    let app = app_server::app_with_jev(handle);
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request builds"),
        )
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

/// Run one request against an app with an explicit Jev handle, as HTML.
async fn get_html_with_jev(path: &str, handle: JevHandle) -> (StatusCode, String) {
    let app = app_server::app_with_jev(handle);
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("in-process request completes");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body reads")
        .to_bytes();
    (
        status,
        String::from_utf8(body.to_vec()).expect("body is UTF-8"),
    )
}

#[tokio::test]
async fn semantic_status_reports_unavailable_without_configuration() {
    // The default fixture app is built with no env key inside tests, so the
    // dedicated status endpoint reports semantic features unavailable.
    let (status, body) = get("/api/semantic-status").await;
    assert_eq!(status, StatusCode::OK);
    let status_body = body.expect("JSON body present");
    assert_eq!(status_body["available"], false);
    assert_eq!(status_body["reason"], "unconfigured");
    assert!(
        !status_body
            .as_object()
            .expect("object")
            .iter()
            .any(|(_key, value)| {
                value
                    .as_str()
                    .map(|s| s.to_ascii_lowercase())
                    .map(|s| s.contains("api_key") || s.contains("key=") || s.contains("bearer"))
                    .unwrap_or(false)
            }),
        "status body must not hint at any credential material: {status_body}"
    );
}

#[tokio::test]
async fn semantic_status_reports_available_with_a_provider() {
    let handle = scripted_handle(vec![]);
    let (status, body) = get_with_jev("/api/semantic-status", handle).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.expect("JSON body")["available"], true);
}

#[tokio::test]
async fn graph_and_search_features_work_with_jev_unconfigured() {
    // All deterministic features through the default (unconfigured) app.
    let (status, body) = get("/api/connection?from=A&to=C").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.expect("connected")["degree"], 2, "graph degrees work");

    let (status, body) = get("/api/paths?from=A&to=C").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.expect("paths")["paths"].as_array().map(Vec::len),
        Some(1),
        "path search works"
    );

    let (status, body) = get("/api/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.expect("stats")["players"], 6, "statistics work");

    let (status, body) = get("/api/fixture").await;
    assert_eq!(status, StatusCode::OK);
    let fixture = body.expect("fixture");
    let players = fixture["players"].as_array().expect("players");
    assert_eq!(players.len(), 6, "browsing works");
}

#[tokio::test]
async fn ui_reports_semantic_features_unavailable_by_default() {
    // Every page carries the availability line; the default seam is
    // deterministically unconfigured.
    let (status, html) = get_html("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("Semantic features (Jev): unavailable"),
        "home page surfaces the unavailable status: {html}"
    );
    assert!(
        html.contains(r#"data-semantic-status="unconfigured""#),
        "status is machine-readable in the HTML: {html}"
    );

    let (status, html) = get_html("/chain?from=A&to=C").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("Semantic features (Jev): unavailable"),
        "chain page surfaces the unavailable status: {html}"
    );
}

#[tokio::test]
async fn ui_reports_semantic_features_available_with_a_provider() {
    let handle = scripted_handle(vec![]);
    let (status, html) = get_html_with_jev("/", handle).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(r#"data-semantic-status="available""#),
        "home page surfaces the available status: {html}"
    );
    assert!(html.contains("Semantic features (Jev): available"));
}

#[tokio::test]
async fn unreachable_provider_fails_soft_and_features_keep_working() {
    // A configured client whose provider always fails transport-wise: after
    // one judgment attempt (the recording point for semantic features), the
    // UI must report semantic features unavailable while every deterministic
    // feature keeps its exact behavior.
    let handle = scripted_handle(vec![]);
    assert!(handle.configured(), "the provider is configured");
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
    let outcome = handle.judge(&request);
    assert_eq!(outcome, JevOutcome::Unavailable, "call fails soft");

    // The chain UI keeps working and reports unavailable semantics.
    let (status, html) = get_html_with_jev("/chain?from=A&to=C", handle.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("Semantic features (Jev): unavailable"),
        "degraded state is surfaced: {html}"
    );
    assert!(
        html.contains("Degree of separation: <strong>2</strong>"),
        "deterministic chain result unchanged: {html}"
    );

    // The graph API keeps answering with identical deterministic bodies.
    let (status, body) = get_with_jev("/api/connection?from=A&to=B", handle).await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("JSON body present");
    assert_eq!(connection["result"], "connected");
    assert_eq!(connection["degree"], 1);
}

#[tokio::test]
async fn served_responses_and_assets_never_contain_credentials() {
    // Serve every browser-reachable surface with a configured (fake-key) Jev
    // client that has already attempted (and failed) a judgment, and assert
    // the key material appears nowhere. The key is a unique canary string; it
    // must not leak into any response or asset.
    let canary = "sk-canary-never-serve-4451";
    let handle = JevHandle::from_client(JevClient::new(
        JevConfig::new(canary.to_string()),
        MemoryTransport::scripted(vec![]),
    ));
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
    assert_eq!(
        handle.judge(&request),
        JevOutcome::Unavailable,
        "the scripted provider fails soft"
    );

    for path in [
        "/",
        "/chain?from=A&to=C",
        "/style.css",
        "/api/fixture",
        "/api/edges",
        "/api/connection?from=A&to=C",
        "/api/paths?from=A&to=C",
        "/api/stats",
        "/api/semantic-status",
        "/does-not-exist",
        "/api/does-not-exist",
    ] {
        let app = app_server::app_with_jev(handle.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("in-process request completes");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body reads")
            .to_bytes();
        let text = String::from_utf8_lossy(&body);
        assert!(
            !text.contains(canary),
            "credential canary leaked into response for {path}: {text}"
        );
        assert!(
            !text.to_ascii_lowercase().contains("sk-canary"),
            "credential prefix leaked into response for {path}"
        );
    }
}

#[tokio::test]
async fn jev_unavailable_outcome_is_deterministic_across_calls() {
    // The seam itself: repeated calls amid provider failure produce the same
    // deterministic Unavailable outcome every time (spec: fail-soft, no
    // nondeterminism introduced into graph behavior).
    let handle = scripted_handle(vec![]);
    let outcomes: Vec<JevOutcome> = (0..5)
        .map(|_| {
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
            handle.judge(&request)
        })
        .collect();
    assert!(
        outcomes
            .iter()
            .all(|outcome| *outcome == JevOutcome::Unavailable),
        "every call is Unavailable: {outcomes:?}"
    );
    assert!(
        outcomes.iter().all(|outcome| *outcome == outcomes[0]),
        "outcomes are deterministic: {outcomes:?}"
    );
}

#[tokio::test]
async fn jev_answers_surface_only_through_the_seam_not_the_graph() {
    // Even when the provider answers, graph responses must not change: Jev
    // outcomes affect semantic features only, never graph facts.
    let handle = scripted_handle(vec![JevOutcome::Answers(vec![(
        "pick".to_string(),
        JevAnswer::Choice("b".to_string(), vec![("b".to_string(), 1.0)], 1.0),
    )])]);
    let (status, body) = get_with_jev("/api/connection?from=A&to=C", handle).await;
    assert_eq!(status, StatusCode::OK);
    let connection = body.expect("connected");
    assert_eq!(connection["degree"], 2, "graph facts are Jev-independent");
    assert_eq!(connection["path"], json!(["A", "B", "C"]));
}

#[tokio::test]
async fn credentials_never_reach_routine_logs() {
    // The credential canary must never be emitted through `log` while the
    // app serves traffic and runs a failing judgment. app-server installs no
    // logger itself and never logs key material (see
    // crates/app-server/tests/log_capture.rs for the captured-output proof
    // under the same in-process seam).
    let canary = "sk-canary-never-log-9012";
    let handle = JevHandle::from_client(JevClient::new(
        JevConfig::new(canary.to_string()),
        MemoryTransport::scripted(vec![]),
    ));
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
    let (status, _) = get_with_jev("/api/connection?from=A&to=B", handle.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = get_with_jev("/api/semantic-status", handle).await;
    assert_eq!(status, StatusCode::OK);
    // The served status also carries no trace of the canary.
    assert!(!body.expect("JSON body").to_string().contains("sk-canary"));
}
