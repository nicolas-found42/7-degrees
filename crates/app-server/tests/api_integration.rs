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
async fn all_paths_for_disconnected_pair_returns_no_chains() {
    // A→D is the spec's same-franchise, non-overlapping case: no edge and no
    // chain, so /api/paths must answer a defined empty result — not a panic.
    let (status, body) = get("/api/paths?from=A&to=D").await;
    assert_eq!(status, StatusCode::OK);
    let paths = body.expect("JSON body present");
    let paths = paths["paths"].as_array().expect("paths array");
    assert!(
        paths.is_empty(),
        "disconnected pairs have no chain to list: {paths:?}"
    );
    // Reachability is undirected: the reversed query behaves identically.
    let (status, body) = get("/api/paths?from=D&to=A").await;
    assert_eq!(status, StatusCode::OK);
    let body = body.expect("JSON body present");
    assert_eq!(body["paths"].as_array().map(Vec::len), Some(0));
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
