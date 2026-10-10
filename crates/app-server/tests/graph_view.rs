use app_server::app_with_fixture_data;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn response(app: axum::Router, uri: &str) -> (u16, axum::http::HeaderMap, Vec<u8>) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}
async fn get(uri: &str) -> (u16, String) {
    let (status, _, bytes) = response(app_with_fixture_data(), uri).await;
    (
        status,
        String::from_utf8(bytes).expect("text endpoint response"),
    )
}

#[tokio::test]
async fn whole_network_has_every_player_and_edge_including_isolates_without_caps() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let app =
        app_server::app_with_report_data(&root, app_server::JevHandle::unconfigured()).unwrap();
    let (status, _, bytes) = response(app, "/api/network").await;
    assert_eq!(status, 200);
    let payload: canvas_view::GraphPayload = serde_json::from_slice(&bytes).unwrap();
    let (graph, _) = app_server::report_data::load(&root).unwrap();
    let expected: std::collections::BTreeSet<_> =
        graph.edges().into_iter().map(|e| (e.a, e.b)).collect();
    let actual: std::collections::BTreeSet<_> = payload
        .links
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(payload.nodes.len(), graph.roster.players.len());
    assert_eq!(payload.links.len(), expected.len());
    assert_eq!(payload.nodes.len(), 5106);
    assert_eq!(payload.links.len(), 101395);
    assert!(payload.nodes.iter().any(|n| n.id == "allenti01"));
    assert!(payload.full_network && !payload.truncated);
}
#[tokio::test]
async fn direct_and_nearby_neighborhoods_expose_only_evidenced_fixture_connections() {
    let (status, body) = get("/api/neighborhood?player=A&depth=1").await;
    assert_eq!(status, 200);
    let direct: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        direct["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["A", "B", "E"]
    );
    assert_eq!(direct["links"].as_array().unwrap().len(), 2);
    assert_eq!(
        direct["nodes"][0]["teams"],
        serde_json::json!(["Green", "Red"])
    );
    assert_eq!(direct["nodes"][0]["era"], "Synthetic fixture days 1–11");
    let (_, body) = get("/api/neighborhood?player=A&depth=2").await;
    let nearby: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        nearby["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["A", "B", "C", "C2", "E"]
    );
    assert_eq!(nearby["links"].as_array().unwrap().len(), 5);
    assert_eq!(nearby["nodes"][2]["distance"], 2);
    assert_eq!(
        nearby["coverage"],
        "Synthetic fixture; complete within this example."
    );
}

#[tokio::test]
async fn graph_exploration_retains_the_selected_chain_and_exact_alternative_cursor() {
    let (status, body) =
        get("/graph?from=A&to=C&cursor=v1:00&limit=1&selected=0&player=B&depth=2").await;
    assert_eq!(status, 200);
    assert!(body.contains("Degree of separation: <strong>2</strong>"));
    assert!(body.contains("Teammate links in this chain: <strong>2</strong>"));
    assert!(body.contains("id=\"graph-canvas\""));
    assert!(body.contains("name=\"cursor\" value=\"v1:00\""));
    assert!(body.contains("name=\"selected\" value=\"0\""));
    assert!(body.contains("Expand direct connections"));
    assert!(body.contains("Expand nearby connections"));
    assert!(body.contains("Synthetic fixture days 2–13"));
    assert!(body.contains("Player B"));
    assert!(body.contains("id=\"graph-payload\" data-payload=\""));
    assert!(body.contains("Canvas loading") || body.contains("Canvas unavailable"));
    assert!(!body.contains("<script type=\"application/json\""));
    let (_, chain) = get("/chain?from=A&to=C&cursor=v1:00&limit=1&selected=0").await;
    assert!(chain.contains("/graph?from=A&amp;to=C&amp;cursor=v1%3A00&amp;limit=1&amp;selected=0"));
}

#[tokio::test]
async fn bounded_exploration_reports_truncation_and_rejects_invalid_graph_arguments() {
    let (status, body) = get("/api/neighborhood?player=A&depth=2&limit=1").await;
    assert_eq!(status, 200);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["truncated"], true);
    assert_eq!(v["nodes"].as_array().unwrap().len(), 1);
    for uri in [
        "/api/neighborhood?player=A&depth=3",
        "/api/neighborhood?player=A&limit=0",
        "/graph?from=A&to=C&selected=99",
        "/graph?from=A&to=C&depth=3",
        "/graph?from=A&to=C&cursor=v1:1",
        "/graph?from=A",
        "/graph?player=A&cursor=v1:0",
    ] {
        assert_eq!(get(uri).await.0, 400, "{uri}");
    }
    let (status, body) = get("/graph?from=A&to=D").await;
    assert_eq!(status, 200);
    assert!(body.contains("verified_disconnected"));
    assert!(!body.contains("graph-canvas"));
    assert_eq!(get("/graph?player=A&depth=1").await.0, 200);
    let (status, body) = get("/assets/canvas-loader.js").await;
    assert_eq!(status, 200);
    assert!(body.contains("canvas_view.js"));
}

#[tokio::test]
async fn present_and_missing_canvas_assets_are_isolated_and_preserve_binary_bytes_and_accessible_fallback()
 {
    use app_server::{JevHandle, app_with_jev_and_canvas_assets, graph_view::CanvasAssets};
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/asset-test-fixtures")
        .join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
    std::fs::create_dir_all(&root).unwrap();
    // Valid WASM header plus a custom section containing non-UTF-8 data.
    let wasm = b"\0asm\x01\0\0\0\0\x03\x01x\xff";
    let js = b"export default async function init() {}";
    std::fs::write(root.join("canvas_view_bg.wasm"), wasm).unwrap();
    std::fs::write(root.join("canvas_view.js"), js).unwrap();
    std::fs::write(root.join("not-an-asset"), b"not allowlisted").unwrap();
    let present =
        app_with_jev_and_canvas_assets(JevHandle::unconfigured(), CanvasAssets::Directory(root));
    let missing =
        app_with_jev_and_canvas_assets(JevHandle::unconfigured(), CanvasAssets::Unavailable);
    let ((status, headers, bytes), (missing_status, _, missing_bytes)) = tokio::join!(
        response(present.clone(), "/assets/canvas_view_bg.wasm"),
        response(missing.clone(), "/assets/canvas_view_bg.wasm")
    );
    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "application/wasm");
    assert_eq!(bytes, wasm);
    assert!(
        String::from_utf8(bytes).is_err(),
        "test must exercise an actual binary response"
    );
    assert_eq!(missing_status, 503);
    assert!(
        String::from_utf8(missing_bytes)
            .unwrap()
            .contains("Canvas assets unavailable")
    );
    let (status, headers, bytes) = response(present.clone(), "/assets/canvas_view.js").await;
    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "text/javascript; charset=utf-8");
    assert_eq!(bytes, js);
    for (app, expected) in [
        (
            present.clone(),
            "Canvas loading; the text lists remain available.",
        ),
        (
            missing.clone(),
            "Canvas unavailable; use the accessible player and relationship lists.",
        ),
    ] {
        let (status, _, bytes) = response(app.clone(), "/graph?from=A&to=C").await;
        assert_eq!(status, 200);
        let body = String::from_utf8(bytes).unwrap();
        for name in ["Player A", "Player B", "Player C"] {
            assert!(body.contains(name));
        }
        assert!(body.contains("Degree of separation: <strong>2</strong>"));
        assert!(body.contains(expected));
        assert_eq!(response(app, "/assets/not-an-asset").await.0, 404);
    }
    assert_eq!(
        response(missing.clone(), "/assets/canvas_view.js").await.0,
        503
    );
    // Both routers retain their own states after interleaved requests.
    assert_eq!(
        response(present, "/assets/canvas_view_bg.wasm").await.0,
        200
    );
    assert_eq!(
        response(missing, "/assets/canvas_view_bg.wasm").await.0,
        503
    );
}

#[tokio::test]
async fn real_report_neighborhood_retains_source_context_and_coverage_without_expanding_the_full_graph()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let app =
        app_server::app_with_report_data(&root, app_server::JevHandle::unconfigured()).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/neighborhood?player=acyqu01&depth=2&limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(v["nodes"][0]["name"], "Quincy Acy");
    assert_eq!(v["nodes"][0]["era"], "2013–2019 seasons");
    assert!(
        v["nodes"][0]["teams"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("KINGS"))
    );
    assert!(v["coverage"].as_str().unwrap().contains("evidence"));
    assert_eq!(v["truncated"], true);
}

#[tokio::test]
async fn full_network_page_keeps_a_text_fallback_without_a_selected_chain() {
    let (status, html) = get("/graph?all=true").await;
    assert_eq!(status, 200);
    assert!(html.contains("id=\"network-text-fallback\""), "{html}");
    assert!(html.contains("id=\"list-connections\""), "{html}");
    let players = html
        .split("<ul id=\"graph-players\">")
        .nth(1)
        .and_then(|rest| rest.split("</ul>").next())
        .unwrap();
    assert!(players.contains("data-select-player"), "{players}");
    let links = html
        .split("<ul id=\"graph-links\">")
        .nth(1)
        .and_then(|rest| rest.split("</ul>").next())
        .unwrap();
    assert!(links.contains("data-select-edge-from"), "{links}");
}

#[tokio::test]
async fn canvas_loader_is_only_referenced_when_assets_exist() {
    use app_server::{JevHandle, app_with_jev_and_canvas_assets, graph_view::CanvasAssets};
    let missing =
        app_with_jev_and_canvas_assets(JevHandle::unconfigured(), CanvasAssets::Unavailable);
    let (status, _, bytes) = response(missing, "/graph?all=true").await;
    assert_eq!(status, 200);
    let html = String::from_utf8(bytes).unwrap();
    assert!(
        !html.contains("canvas-loader.js"),
        "a page without assets must not import a module that will 503"
    );
    assert!(html.contains("Canvas unavailable"), "{html}");
}

#[tokio::test]
async fn neighborhood_and_chain_links_name_the_same_team_for_a_pair() {
    let (_, body) = get("/api/neighborhood?player=A&depth=1&limit=50").await;
    let payload: Value = serde_json::from_str(&body).unwrap();
    for link in payload["links"].as_array().unwrap() {
        let (from, to) = (link["from"].as_str().unwrap(), link["to"].as_str().unwrap());
        let (_, chain) = get(&format!("/api/connection?from={from}&to={to}")).await;
        let chain: Value = serde_json::from_str(&chain).unwrap();
        if chain["links"].as_array().map(Vec::len) == Some(1) {
            assert_eq!(link["team"], chain["links"][0]["team"], "{from}-{to}");
        }
    }
}
