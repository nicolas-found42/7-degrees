//! Statistics UI through the approved fixture-backed HTTP seam.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn get(app: Router, path: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn statistics_page_displays_hand_checked_fixture_counts_and_histogram() {
    // Component A-B-C/C2 with E attached to A, plus isolated D: 10 reachable
    // unordered pairs split 5/3/2 across degrees 1/2/3; five unreachable.
    let (status, html) = get(app_server::app_with_fixture_data(), "/stats").await;
    assert_eq!(status, StatusCode::OK);
    for text in [
        "<h1>Network statistics</h1>",
        "<dt>Player nodes</dt><dd>6</dd>",
        "<dt>Connected components</dt><dd>2</dd>",
        "<dt>Maximum finite diameter</dt><dd>3</dd>",
        "<dt>Reachable unordered pairs</dt><dd>10</dd>",
        "<dt>Unreachable unordered pairs</dt><dd>5</dd>",
        "<caption>Reachable unordered player pairs by degree of separation</caption>",
        "<th scope=\"row\">1</th><td>5</td>",
        "<th scope=\"row\">2</th><td>3</td>",
        "<th scope=\"row\">3</th><td>2</td>",
    ] {
        assert!(html.contains(text), "missing {text} in {html}");
    }
}

#[tokio::test]
async fn statistics_page_explains_pair_scope_and_is_reachable_without_semantic_service() {
    let app = app_server::app_with_fixture_data();
    let (_, home) = get(app.clone(), "/").await;
    assert!(
        home.contains("href=\"/stats\""),
        "home navigation links to statistics"
    );
    let (_, html) = get(app.clone(), "/stats").await;
    for text in [
        "Synthetic fixture",
        "Semantic features (Jev): unavailable",
        "Self-pairs are excluded",
        "Unordered pairs are counted once",
        "Every isolated player is a component",
        "Unreachable pairs are excluded from the histogram",
        "maximum finite shortest-path length across all components",
        "If there are no reachable distinct pairs, the diameter is 0",
        "ordinary deterministic Rust",
        "href=\"/api/stats\"",
    ] {
        assert!(html.contains(text), "missing interpretation: {text}");
    }
    let (_, again) = get(app.clone(), "/stats").await;
    assert_eq!(html, again, "immutable graph renders deterministically");
    let (_, api) = get(app, "/api/stats").await;
    let api: serde_json::Value = serde_json::from_str(&api).unwrap();
    assert_eq!(
        api,
        serde_json::json!({
            "players":6,"components":2,"diameter":3,"unreachable_pairs":5,
            "histogram":{"1":5,"2":3,"3":2}
        })
    );
}

fn report_fixture(player_rows: &str, tenure_rows: &str) -> Router {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "seven-degrees-stats-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"), format!("bbr_player_id,display_name,first_season,last_season,aba_only,s1_display_name\n{player_rows}")).unwrap();
    std::fs::write(root.join("t4/tenures.csv"), format!("bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored,unresolved_reasons,arrival_days,departure_days\n{tenure_rows}")).unwrap();
    app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap()
}

#[tokio::test]
async fn multiple_nontrivial_components_and_isolates_keep_coverage_warning_visible() {
    // Two disjoint teammate pairs and one isolate: 2 reachable out of 10
    // unordered pairs. Missing roster evidence must qualify these counts.
    let app = report_fixture(
        "a,Alpha,2000,2000,N,\nb,Beta,2000,2000,N,\nc,Gamma,2000,2000,N,\nd,Delta,2000,2000,N,\ne,Epsilon,2000,2000,N,\n",
        "a,2000,NBA,RED,S2,directly-evidenced,1,4,1,1,,1,4\nb,2000,NBA,RED,S2,directly-evidenced,2,3,1,1,,2,3\nc,2000,NBA,BLUE,S2,directly-evidenced,1,4,1,1,,1,4\nd,2000,NBA,BLUE,S2,directly-evidenced,2,3,1,1,,2,3\ne,2000,NBA,RED,S2,inferred,1,4,0,0,,,\n",
    );
    let (_, api) = get(app.clone(), "/api/stats").await;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&api).unwrap(),
        serde_json::json!({
            "players":5,"components":3,"diameter":1,"unreachable_pairs":8,"histogram":{"1":2}
        })
    );
    let (status, html) = get(app, "/stats").await;
    assert_eq!(status, StatusCode::OK);
    for text in [
        "<dt>Connected components</dt><dd>3</dd>",
        "<dt>Maximum finite diameter</dt><dd>1</dd>",
        "<dt>Unreachable unordered pairs</dt><dd>8</dd>",
        "Incomplete dated roster coverage",
        "1946–1950 BAA",
        "missing edges may shorten a historical chain",
        "These statistics describe the evidenced graph, not complete historical connectivity",
    ] {
        assert!(html.contains(text), "missing {text}");
    }
}

#[tokio::test]
async fn isolate_only_and_empty_snapshots_show_zero_diameter_and_an_empty_histogram_message() {
    for (players, expected) in [
        (
            "a,Alpha,2000,2000,N,\nb,Beta,2000,2000,N,\n",
            serde_json::json!({"players":2,"components":2,"diameter":0,"unreachable_pairs":1,"histogram":{}}),
        ),
        (
            "",
            serde_json::json!({"players":0,"components":0,"diameter":0,"unreachable_pairs":0,"histogram":{}}),
        ),
    ] {
        let app = report_fixture(players, "");
        let (_, api) = get(app.clone(), "/api/stats").await;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&api).unwrap(),
            expected
        );
        let (status, html) = get(app.clone(), "/stats").await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains("No reachable distinct player pairs in this snapshot."));
        assert!(html.contains("<dt>Maximum finite diameter</dt><dd>0</dd>"));
        assert!(html.contains("<dt>Reachable unordered pairs</dt><dd>0</dd>"));
        if players.is_empty() {
            let (status, api) = get(app.clone(), "/api/network").await;
            assert_eq!(status, StatusCode::OK);
            let payload: serde_json::Value = serde_json::from_str(&api).unwrap();
            assert_eq!(payload["nodes"], serde_json::json!([]));
            assert_eq!(payload["links"], serde_json::json!([]));
            assert_eq!(payload["full_network"], true);
            let (status, html) = get(app, "/graph?all=true").await;
            assert_eq!(status, StatusCode::OK);
            assert!(html.contains("No players are present in this snapshot."));
        }
    }
}
