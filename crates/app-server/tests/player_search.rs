//! Approved HTTP seam: hand-checked canonical identities and historical context.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "seven-search-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"), "bbr_player_id,display_name,first_season,last_season,aba_only,s1_display_name\njordami01,Michael Jordan,1985,2003,N,\nonealsh01,Shaquille O'Neal,1993,2011,N,Shaquille O’Neal\nervinju01,Julius Erving,1977,1987,N,\niversal01,Allen Iverson,1997,2010,N,\nsmithjo01,Joe Smith,1996,2011,N,\nsmithjo02,Joe Smith,1947,1948,N,\nalpha,José Calderón,2006,2019,N,Jose Calderon\n").unwrap();
    std::fs::write(root.join("t4/tenures.csv"), "bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\njordami01,1985,NBA,BULLS,S2,inferred,1,5,0,0\njordami01,2003,NBA,WIZARDS,S2,inferred,10,13,0,0\nonealsh01,1993,NBA,MAGIC,S2,inferred,2,5,0,0\nonealsh01,2000,NBA,LAKERS,S2,inferred,7,12,0,0\nervinju01,1983,NBA,SIXERS,S2,inferred,1,5,0,0\niversal01,2001,NBA,SIXERS,S2,inferred,7,12,0,0\nsmithjo01,1996,NBA,WARRIORS,S2,inferred,7,12,0,0\nsmithjo02,1947,BAA,STAGS,S2,inferred,7,12,0,0\nalpha,2006,NBA,RAPTORS,S2,inferred,7,12,0,0\n").unwrap();
    app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap()
}
async fn request(app: Router, path: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    (
        response.status(),
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
async fn json_get(path: &str) -> Value {
    let (status, body) = request(app(), path).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).unwrap()
}
#[tokio::test]
async fn exact_name_returns_canonical_player_with_era_and_teams() {
    let result = json_get("/api/search?q=Michael%20Jordan").await;
    assert_eq!(result["status"], "exact_match");
    assert_eq!(result["candidates"].as_array().unwrap().len(), 1);
    let player = &result["candidates"][0]["player"];
    assert_eq!(player["id"], "jordami01");
    assert_eq!(player["first_season"], 1985);
    assert_eq!(player["last_season"], 2003);
    assert_eq!(player["teams"], json!(["BULLS", "WIZARDS"]));
}
#[tokio::test]
async fn source_aliases_and_curated_nicknames_return_ranked_canonical_candidates() {
    for (q, id) in [
        ("Shaq", "onealsh01"),
        ("The%20Answer", "iversal01"),
        ("Dr.%20J", "ervinju01"),
        ("Jose%20Calderon", "alpha"),
    ] {
        let result = json_get(&format!("/api/search?q={q}")).await;
        assert_eq!(result["candidates"][0]["player"]["id"], id, "query {q}");
        assert_eq!(result["matches_total"], 1);
    }
}
#[tokio::test]
async fn misspelling_uses_code_ranked_shortlist_and_unrelated_text_is_no_match() {
    let result = json_get("/api/search?q=Micheal%20Jordan").await;
    assert_eq!(result["status"], "candidates");
    assert_eq!(result["candidates"][0]["player"]["id"], "jordami01");
    assert_eq!(result["candidates"][0]["match_kind"], "typo");
    assert_eq!(
        json_get("/api/search?q=intergalactic%20banana").await["status"],
        "no_match"
    );
}
#[tokio::test]
async fn surname_and_partial_name_rank_candidates_without_collapsing_same_name_people() {
    let result = json_get("/api/search?q=Smith").await;
    assert_eq!(result["status"], "ambiguous");
    assert_eq!(result["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(result["candidates"][0]["player"]["id"], "smithjo01");
    assert_eq!(result["candidates"][1]["player"]["id"], "smithjo02");
    assert_ne!(
        result["candidates"][0]["player"]["first_season"],
        result["candidates"][1]["player"]["first_season"]
    );
    assert_eq!(
        json_get("/api/search?q=Michael%20J").await["candidates"][0]["player"]["id"],
        "jordami01"
    );
    assert_eq!(
        json_get("/api/search?q=Joe%20Smith").await["status"],
        "ambiguous"
    );
}
#[tokio::test]
async fn rendered_candidates_offer_identity_selection_with_era_team_and_coverage_context() {
    let (status, html) = request(app(), "/search?q=Joe%20Smith").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Choose the intended player"));
    assert!(html.contains("1996–2011") && html.contains("1947–1948"));
    assert!(html.contains("WARRIORS") && html.contains("STAGS"));
    assert!(
        html.contains("href=\"/players/smithjo01\"")
            && html.contains("href=\"/players/smithjo02\"")
    );
    assert!(html.contains("Incomplete dated roster coverage"));
    let (status, profile) = request(app(), "/players/jordami01").await;
    assert_eq!(status, StatusCode::OK);
    assert!(profile.contains("Michael Jordan") && profile.contains("1985–2003"));
    assert!(profile.contains("/chain?from=jordami01"));
}
#[tokio::test]
async fn bounded_queries_do_not_hide_ambiguity_or_expand_an_empty_query() {
    for uri in ["/api/search?limit=0", "/api/search?limit=51"] {
        assert_eq!(request(app(), uri).await.0, StatusCode::BAD_REQUEST);
    }
    let too_long = format!("/api/search?q={}", "x".repeat(161));
    assert_eq!(request(app(), &too_long).await.0, StatusCode::BAD_REQUEST);
    let result = json_get("/api/search?q=Joe%20Smith&limit=1").await;
    assert_eq!(result["status"], "ambiguous");
    assert_eq!(result["matches_total"], 2);
    assert_eq!(result["has_more"], true);
    assert_eq!(result["candidates"].as_array().unwrap().len(), 1);
    let empty = json_get("/api/search?q=...").await;
    assert_eq!(empty["status"], "empty_query");
    assert_eq!(empty["matches_total"], 0);
}
#[tokio::test]
async fn empty_no_match_and_invalid_profile_are_visible_and_query_text_is_escaped() {
    assert!(
        request(app(), "/search")
            .await
            .1
            .contains("Enter a player name")
    );
    let (_, html) = request(app(), "/search?q=%3Cscript%3Ealert%281%29%3C%2Fscript%3E").await;
    assert!(html.contains("No matching players found"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
    let (status, html) = request(app(), "/players/unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(html.contains("Player not found"));
    assert!(request(app(), "/").await.1.contains("action=\"/search\""));
}
