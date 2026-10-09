//! Approved import + HTTP/SSR seam: chains are worked examples, not graph internals.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn html(app: Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}
fn alternatives() -> Router {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "seven-degrees-chain-view-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"),"bbr_player_id,display_name,first_season,last_season,aba_only,s1_display_name\ns,Start,2000,2000,N,\nb,Beta,2000,2000,N,\nc,Gamma,2000,2000,N,\ng,Goal,2000,2000,N,\nd,Disconnected,2000,2000,N,\ne,Longer One,2000,2000,N,\nf,Longer Two,2000,2000,N,\n").unwrap();
    let mut rows = String::from(
        "bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored,unresolved_reasons,arrival_days,departure_days\n",
    );
    for (i, (a, b)) in [
        ("s", "b"),
        ("b", "g"),
        ("s", "c"),
        ("c", "g"),
        ("s", "e"),
        ("e", "f"),
        ("f", "g"),
    ]
    .iter()
    .enumerate()
    {
        let start = i * 10 + 1;
        let end = start + 1;
        for p in [a, b] {
            rows.push_str(&format!(
                "{p},2000,NBA,T{i},S2,directly-evidenced,{start},{end},1,1,,{start},{end}\n"
            ));
        }
    }
    std::fs::write(root.join("t4/tenures.csv"), rows).unwrap();
    app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap()
}
#[tokio::test]
async fn equally_short_chains_have_visible_selection_and_keep_the_minimum_degree() {
    let (status, body) = html(alternatives(), "/chain?from=s&to=g").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Degree of separation: <strong>2</strong>"));
    assert!(body.contains("data-total-shortest=\"2\""));
    assert!(body.contains("Select alternative 2"));
    assert!(body.contains("Start → Gamma → Goal"));
    assert!(body.contains("data-selected=\"true\""));
    assert!(!body.contains("Degree of separation: <strong>3</strong>"));
    let (status, second) = html(alternatives(), "/chain?from=s&to=g&selected=1").await;
    assert_eq!(status, StatusCode::OK);
    assert!(second.contains("data-selected-index=\"1\""));
    assert!(second.contains("data-path=\"[&quot;s&quot;,&quot;c&quot;,&quot;g&quot;]\""));
}

fn href_for(body: &str, marker: &str) -> String {
    let tail = body.split_once(marker).expect("navigation marker").1;
    tail.split_once("href=\"")
        .expect("navigation href")
        .1
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&")
}
#[tokio::test]
async fn browsing_pages_keeps_the_cursor_and_selected_shortest_chain() {
    let app = alternatives();
    let (status, first) = html(app.clone(), "/chain?from=s&to=g&limit=1").await;
    assert_eq!(status, StatusCode::OK);
    assert!(first.contains("data-path=\"[&quot;s&quot;,&quot;b&quot;,&quot;g&quot;]\""));
    let next = href_for(&first, "id=\"next-shortest-page\"");
    assert!(next.contains("cursor=v1%3A1"));
    assert!(next.contains("limit=1"));
    let (status, second) = html(app.clone(), &next).await;
    assert_eq!(status, StatusCode::OK);
    assert!(second.contains("data-path=\"[&quot;s&quot;,&quot;c&quot;,&quot;g&quot;]\""));
    assert!(!second.contains("id=\"next-shortest-page\""));
    let selected = href_for(&second, "data-selected=\"true\"");
    assert!(selected.contains("cursor=v1%3A1"));
    let (_, selected_again) = html(app, &selected).await;
    assert!(selected_again.contains("data-path=\"[&quot;s&quot;,&quot;c&quot;,&quot;g&quot;]\""));
}

#[tokio::test]
async fn disconnected_fixture_and_incomplete_history_have_distinct_visible_outcomes() {
    let (status, known) = html(app_server::app_with_fixture_data(), "/chain?from=A&to=D").await;
    assert_eq!(status, StatusCode::OK);
    assert!(known.contains("Verified disconnected within this synthetic fixture."));
    assert!(known.contains("data-result=\"disconnected\""));
    assert!(!known.contains("id=\"selected-chain-payload\""));
    let (status, unknown) = html(alternatives(), "/chain?from=s&to=d").await;
    assert_eq!(status, StatusCode::OK);
    assert!(unknown.contains("Missing dated roster evidence may hide a historical connection."));
    assert!(unknown.contains("data-connection-certainty=\"unresolved_coverage\""));
    assert!(unknown.contains("data-result=\"disconnected\""));
    assert!(!unknown.contains("Verified disconnected"));
    assert!(!unknown.contains("Degree of separation:"));
}

fn selected_payload(body: &str) -> serde_json::Value {
    let text = body
        .split_once("id=\"selected-chain-payload\" data-payload=\"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    let json = text
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    serde_json::from_str(&json).unwrap()
}
#[tokio::test]
async fn direct_indirect_and_self_chains_explain_and_export_the_exact_link_count() {
    let app = app_server::app_with_fixture_data();
    for (to, degree) in [("B", 1), ("C", 2), ("A", 0)] {
        let (status, body) = html(app.clone(), &format!("/chain?from=A&to={to}")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(&format!("Degree of separation: <strong>{degree}</strong>")));
        assert!(body.contains(&format!(
            "Teammate links in this chain: <strong>{degree}</strong>"
        )));
        let payload = selected_payload(&body);
        assert_eq!(payload["degree"], degree);
        assert_eq!(payload["links"].as_array().unwrap().len(), degree);
        assert_eq!(payload["path"].as_array().unwrap().len(), degree + 1);
        if degree == 0 {
            assert!(body.contains("Same player; no teammate links are needed."));
        }
    }
}

#[tokio::test]
async fn incomplete_selection_and_invalid_pages_never_substitute_a_chain() {
    let app = app_server::app_with_fixture_data();
    for uri in [
        "/chain?selected=1",
        "/chain?from=A&limit=0",
        "/chain?from=A&to=",
        "/chain?from=A&to=C&selected=99",
        "/chain?from=A&to=C&limit=101",
        "/chain?from=A&to=C&cursor=v1:0&offset=0",
        "/chain?from=A&to=C&cursor=v1:1",
    ] {
        let (status, body) = html(app.clone(), uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert!(!body.contains("id=\"selected-chain\""), "{uri}");
        assert!(
            !body.contains("data-connection-certainty=\"verified_disconnected\""),
            "{uri}"
        );
    }
    let (status, partial) = html(app, "/chain?from=A").await;
    assert_eq!(status, StatusCode::OK);
    assert!(partial.contains("Pick two players and connect them."));
    assert!(!partial.contains("Degree of separation:"));
}

#[tokio::test]
async fn hostile_query_text_is_escaped_and_cannot_become_a_selected_chain() {
    let (status, body) = html(
        app_server::app_with_fixture_data(),
        "/chain?from=%3Cscript%3Ealert%281%29%3C%2Fscript%3E&to=A",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!body.contains("<script>"));
    assert!(!body.contains("id=\"selected-chain\""));
    let (status, body) = html(
        app_server::app_with_fixture_data(),
        "/chain?from=A&to=C&cursor=%3Cimg%20src%3Dx%20onerror%3Dalert%281%29%3E",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!body.contains("<img"));
    assert!(!body.contains("id=\"selected-chain\""));
}
