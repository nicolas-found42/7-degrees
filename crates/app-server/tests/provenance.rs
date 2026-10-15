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
    // The graph page ships the whole network for canvas selection plus the
    // picker; standalone evidence and the removed sections stay as asserted.
    let (_, graph) = get(app, "/").await;
    assert!(graph.contains("id=\"graph-payload\""));
    assert!(graph.contains("id=\"graph-player\""));
    assert!(!graph.contains("data-select-edge-from="));
    for removed in [
        "id=\"graph-players\"",
        "data-select-player=",
        "id=\"network-player-links\"",
        "Connections of selected player",
        "id=\"edge-provenance-panel\"",
        "id=\"edge-provenance-frame\"",
        "Selected relationship evidence",
    ] {
        assert!(
            !graph.contains(removed),
            "removed section still present: {removed}"
        );
    }
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
async fn canvas_page_has_no_in_page_evidence_panel_and_allowlisted_source_routes() {
    let app = app_server::app_with_jev_and_canvas_assets(
        app_server::JevHandle::unconfigured(),
        app_server::graph_view::CanvasAssets::Unavailable,
    );
    let (_, graph) = get(app.clone(), "/graph").await;
    assert!(graph.contains("Canvas unavailable; use the player picker below."));
    for removed in [
        "id=\"edge-provenance-frame\"",
        "Selected teammate overlap evidence",
        "id=\"open-selected-edge\"",
        "Selected relationship evidence",
        "id=\"graph-players\"",
        "id=\"network-player-links\"",
    ] {
        assert!(
            !graph.contains(removed),
            "removed section still present: {removed}"
        );
    }
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

#[tokio::test]
async fn drawer_representation_is_a_fragment_for_success_and_error_states() {
    let app = app_server::app_with_fixture_data();
    for (pair, expected, status) in [
        ("A&to=B", "Overlap: [day 2, day 4)", 200),
        ("A&to=D", "No admitted direct teammate edge", 200),
        ("missing&to=B", "Unknown player endpoint", 404),
    ] {
        let (actual, body) = get(app.clone(), &format!("/edge?from={pair}&view=drawer")).await;
        assert_eq!(actual, status);
        assert!(body.contains(expected));
        assert!(!body.contains("<!doctype"));
        assert!(!body.contains("<html"));
    }
}

#[tokio::test]
async fn shortened_season_and_game_witness_cards_preserve_proof_limits() {
    let root = std::env::temp_dir().join(format!("seven-proof-upgrade-{}", std::process::id()));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"),"bbr_player_id,display_name,first_season,last_season,aba_only\na,Player Alpha,1999,1999,N\nb,Player Beta,1999,1999,N\n").unwrap();
    std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\na,1999,NBA,RED,S2,inferred,,,0,0\nb,1999,NBA,RED,S2,inferred,,,0,0\n").unwrap();
    std::fs::write(root.join("t4/appearance-counts.csv"),"player,team,season,lg,games,team_games,player_record,team_record\na,RED,1999,NBA,60,66,S2:player:1,S2:team:1\nb,RED,1999,NBA,50,66,S2:player:2,S2:team:1\n").unwrap();
    let app =
        app_server::app_with_report_data(&root, app_server::JevHandle::unconfigured()).unwrap();
    let (_, html) = get(app, "/edge?from=b&to=a&view=drawer").await;
    for text in [
        "60 + 50 − 66 = at least 44",
        "Player Alpha: 60/66",
        "Player Beta: 50/66",
        "actual season total 66",
        "max=\"66\"",
        "which specific games",
        "2 excluded record(s)",
        "dated interval unavailable",
        "S2:player:1",
    ] {
        assert!(html.contains(text), "missing {text}: {html}");
    }
    assert!(!html.contains("82-game season"));
    std::fs::remove_file(root.join("t4/appearance-counts.csv")).unwrap();
    std::fs::write(root.join("t4/game-witnesses.csv"),"a,b,team,season,game_id,date,a_source,b_source\na,b,RED,1999,0029800001,1998-11-01,S1:game:1:event:10:player1,S1:game:1:event:11:player1\n").unwrap();
    let app =
        app_server::app_with_report_data(&root, app_server::JevHandle::unconfigured()).unwrap();
    let (_, html) = get(app, "/edge?from=a&to=b").await;
    for text in [
        "1998-11-01",
        "0029800001",
        "S1:game:1:event:10:player1",
        "duration remain unknown",
        "S1 v238",
    ] {
        assert!(html.contains(text), "missing {text}");
    }
    assert!(!html.contains("Overlap: ["));
}
