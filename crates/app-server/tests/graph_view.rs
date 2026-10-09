use app_server::app_with_fixture_data;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn get(uri: &str) -> (u16, String) {
    let response = app_with_fixture_data()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
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
async fn missing_canvas_assets_leave_a_visible_accessible_fallback() {
    let (asset_status, _) = get("/assets/canvas_view_bg.wasm").await;
    let (status, body) = get("/graph?from=A&to=C").await;
    assert_eq!(status, 200);
    assert!(body.contains("Player A"));
    assert!(body.contains("Player B"));
    assert!(body.contains("Player C"));
    assert!(body.contains("Degree of separation: <strong>2</strong>"));
    if asset_status == 503 {
        assert!(
            body.contains("Canvas unavailable; use the accessible player and relationship lists.")
        );
    } else {
        assert_eq!(asset_status, 200);
    }
    assert_eq!(get("/assets/not-an-asset").await.0, 404);
}

#[tokio::test]
async fn real_report_neighborhood_retains_source_context_and_coverage_without_expanding_the_full_graph()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let app =
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap();
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
