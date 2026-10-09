use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;
async fn get(app: Router, uri: &str) -> (u16, String) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    (
        status,
        String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap(),
    )
}
#[tokio::test]
async fn opening_a_fixture_teammate_link_shows_both_stints_and_explicit_synthetic_overlap() {
    let app = app_server::app_with_fixture_data();
    let (status, body) = get(app.clone(), "/edge?from=A&to=B").await;
    assert_eq!(status, 200);
    for text in [
        "Canonical franchise: Red",
        "Synthetic fixture",
        "Overlap: [day 2, day 4)",
        "fixture:tenure:1",
        "fixture:tenure:3",
        "synthetic-fixture",
        "Player A",
        "Player B",
    ] {
        assert!(body.contains(text), "missing {text}");
    }
    assert!(
        !body.contains("1946-01-03"),
        "fixture days must not masquerade as historical dates"
    );
    let (_, chain) = get(app, "/chain?from=A&to=C&cursor=v1:00&selected=0&limit=1").await;
    assert!(chain.contains("href=\"/edge?from=A&amp;to=B\""));
    assert!(chain.contains("Degree of separation: <strong>2</strong>"));
}

#[tokio::test]
async fn real_acy_bogut_overlap_has_calendar_dates_both_source_records_anchors_and_versions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let app =
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap();
    let (status, body) = get(app.clone(), "/edge?from=acyqu01&to=bogutan01").await;
    assert_eq!(status, 200);
    for text in [
        "Canonical franchise: MAVERICKS",
        "Overlap: [2016-07-20, 2016-11-18)",
        "2016–17",
        "Quincy Acy",
        "Andrew Bogut",
        "directly-evidenced",
        "Arrival boundary: transaction anchored",
        "Departure boundary: transaction anchored",
        "S1 v238",
        "S2 v56",
        "Source/version manifest",
    ] {
        assert!(body.contains(text), "missing {text}");
    }
    assert!(!body.contains("Synthetic fixture"));
    assert!(
        body.matches("/sources/tenure?record=t4%2Ftenures.csv%3A")
            .count()
            >= 2
    );
    let (_, api) = get(app.clone(), "/api/edges/acyqu01").await;
    let api: serde_json::Value = serde_json::from_str(&api).unwrap();
    let edge = api["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["a"] == "acyqu01" && e["b"] == "bogutan01")
        .unwrap();
    for record in edge["evidence"][0]["overlaps"][0]["records"]
        .as_array()
        .unwrap()
    {
        let id = record["record"].as_str().unwrap();
        assert!(body.contains(id));
        let (_, record_body) = get(
            app.clone(),
            &format!(
                "/sources/tenure?record={}",
                id.replace('/', "%2F").replace(':', "%3A")
            ),
        )
        .await;
        assert!(record_body.contains(id));
        assert!(record_body.contains("MAVERICKS"));
        assert!(record_body.contains("transaction anchored"));
    }
    let (_, manifest) = get(app, "/sources/manifest").await;
    assert!(manifest.contains("Version: 238"));
    assert!(manifest.contains("Version: 56"));
}

fn incomplete() -> Router {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/provenance-fixture")
        .join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"),"bbr_player_id,display_name,first_season,last_season,aba_only\na,Alpha,2000,2000,N\nb,Beta,2000,2000,N\n").unwrap();
    std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored,unresolved_reasons\na,2000,NBA,RED,S2,directly-evidenced,1,5,1,1,\nb,2000,NBA,RED,S2,inferred,1,5,0,0,<img src=x onerror=alert(1)>\n").unwrap();
    app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap()
}
#[tokio::test]
async fn unadmitted_edge_explains_incomplete_evidence_and_exposes_the_excluded_source_record() {
    let app = incomplete();
    let (status, body) = get(app.clone(), "/edge?from=a&to=b").await;
    assert_eq!(status, 200);
    for text in [
        "Source evidence is incomplete",
        "Missing evidence does not prove a historical non-relationship",
        "Coverage gaps",
        "t4/tenures.csv:3",
        "inferred",
        "RED",
        "1999–00",
    ] {
        assert!(body.contains(text), "missing {text}");
    }
    assert!(!body.contains("<img"));
    assert!(body.contains("&lt;img"));
    let (status, record) = get(app, "/sources/tenure?record=t4%2Ftenures.csv%3A3").await;
    assert_eq!(status, 200);
    assert!(record.contains("Excluded from teammate evidence"));
    assert!(record.contains("inferred"));
}

#[tokio::test]
async fn canvas_has_an_accessible_selected_edge_panel_and_allowlisted_source_routes() {
    let app = app_server::app_with_jev_and_canvas_assets(
        app_server::JevHandle::unconfigured(),
        app_server::graph_view::CanvasAssets::Unavailable,
    );
    let (_, graph) = get(
        app.clone(),
        "/graph?from=A&to=C&cursor=v1:00&limit=1&selected=0",
    )
    .await;
    assert!(
        graph.contains("Canvas unavailable; use the accessible player and relationship lists.")
    );
    assert!(graph.contains("id=\"edge-provenance-frame\""));
    assert!(graph.contains("title=\"Selected teammate overlap evidence\""));
    assert!(graph.contains("id=\"open-selected-edge\""));
    assert!(graph.contains("href=\"/edge?from=A&amp;to=B\""));
    assert!(graph.contains("name=\"cursor\" value=\"v1:00\""));
    for uri in [
        "/sources/tenure?record=../../.env",
        "/sources/tenure?record=javascript:alert(1)",
        "/sources/tenure?record=t4%2Ftenures.csv%3A999999",
        "/edge?from=%3Cscript%3E&to=A",
    ] {
        assert_eq!(get(app.clone(), uri).await.0, 404, "{uri}");
    }
    assert_eq!(get(app.clone(), "/edge?from=A").await.0, 400);
    let (_, repeated) = get(app.clone(), "/edge?from=A&to=E").await;
    assert!(repeated.contains("Overlap: [day 6, day 8)"));
    assert!(repeated.contains("Overlap: [day 9, day 11)"));
    let (_, source) = get(app, "/sources/tenure?record=fixture%3Atenure%3A1").await;
    assert!(source.contains("Source record fixture:tenure:1"));
}
