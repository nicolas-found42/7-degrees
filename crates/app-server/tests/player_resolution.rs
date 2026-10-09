//! Approved report-backed HTTP/HTML seam; provider outcomes are scripted.
use app_server::JevHandle;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jev_client::{JevClient, JevConfig, JevOutcome, MemoryTransport};
use serde_json::{Value, json};
use tower::ServiceExt;
fn app(outcome: JevOutcome) -> Router {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "seven-resolution-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"),"bbr_player_id,display_name,first_season,last_season,aba_only,s1_display_name\njordami01,Michael Jordan,1985,2003,N,\nonealsh01,Shaquille O'Neal,1993,2011,N,Shaquille O’Neal\nsmithjo01,Joe Smith,1996,2011,N,\nsmithjo02,Joe Smith,1947,1948,N,\n").unwrap();
    std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\njordami01,1985,NBA,BULLS,S2,inferred,1,5,0,0\nonealsh01,2000,NBA,LAKERS,S2,inferred,7,12,0,0\nsmithjo01,1996,NBA,WARRIORS,S2,inferred,7,12,0,0\nsmithjo02,1947,BAA,STAGS,S2,inferred,7,12,0,0\n").unwrap();
    let handle = JevHandle::from_client(JevClient::new(
        JevConfig::new("test-private-key".into()),
        MemoryTransport::scripted(vec![outcome]),
    ));
    app_server::app_with_report_data(root, handle).unwrap()
}
async fn request(app: Router, path: &str) -> (StatusCode, String) {
    let r = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    (
        r.status(),
        String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap(),
    )
}
async fn get(app: Router, path: &str) -> Value {
    let (status, body) = request(app, path).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).unwrap()
}
#[tokio::test]
async fn exact_identity_resolution_remains_available_without_semantics() {
    let result = get(
        app(JevOutcome::Unavailable),
        "/api/resolve?q=Michael%20Jordan",
    )
    .await;
    assert_eq!(result["status"], "resolved");
    assert_eq!(result["player"]["id"], "jordami01");
    assert_eq!(result["interpretation"], "exact_lookup");
    assert_eq!(result["player"]["teams"], json!(["BULLS"]));
}
fn judgment(choice: &str, p: f64, confidence: f64, exists: f64) -> JevOutcome {
    use jev_client::JevAnswer;
    JevOutcome::Answers(vec![
        (
            "choice".into(),
            JevAnswer::Choice(
                choice.into(),
                vec![("candidate_00".into(), p), ("none".into(), 1.0 - p)],
                confidence,
            ),
        ),
        ("exists".into(), JevAnswer::Noul(exists)),
    ])
}
#[tokio::test]
async fn semantic_nickname_alias_and_typo_copy_the_canonical_shortlisted_player() {
    for (q, id) in [
        ("Shaq", "onealsh01"),
        ("Air%20Jordan", "jordami01"),
        ("Micheal%20Jordan", "jordami01"),
    ] {
        let result = get(
            app(judgment("candidate_00", 0.98, 0.96, 0.99)),
            &format!("/api/resolve?q={q}"),
        )
        .await;
        assert_eq!(result["status"], "resolved", "{q}");
        assert_eq!(result["player"]["id"], id);
        assert_eq!(result["interpretation"], "semantic_interpretation");
        assert_eq!(result["semantic"]["existence_probability"], 0.99);
    }
}
#[tokio::test]
async fn ambiguity_low_confidence_no_match_and_unavailable_are_selectable_without_a_path() {
    let ambiguous = get(
        app(judgment("candidate_00", 0.99, 0.99, 0.99)),
        "/api/resolve?q=Joe%20Smith",
    )
    .await;
    assert_eq!(ambiguous["status"], "clarification");
    assert!(ambiguous["player"].is_null());
    assert_eq!(ambiguous["candidates"].as_array().unwrap().len(), 2);
    let low = get(
        app(judgment("candidate_00", 0.6, 0.4, 0.99)),
        "/api/resolve?q=Shaq",
    )
    .await;
    assert_eq!(low["status"], "clarification");
    assert_eq!(low["reason"], "low_confidence");
    let nonexistent = get(
        app(judgment("candidate_00", 0.99, 0.99, 0.01)),
        "/api/resolve?q=Shaq",
    )
    .await;
    assert_eq!(nonexistent["status"], "no_match");
    assert!(nonexistent["player"].is_null());
    assert_eq!(
        get(
            app(JevOutcome::Unavailable),
            "/api/resolve?q=intergalactic%20banana"
        )
        .await["status"],
        "no_match"
    );
    let unavailable = get(app(JevOutcome::Unavailable), "/api/resolve?q=Shaq").await;
    assert_eq!(unavailable["status"], "unavailable");
    assert_eq!(unavailable["candidates"][0]["player"]["id"], "onealsh01");
    assert!(unavailable.get("path").is_none());
    let (status, html) = request(
        app(judgment("candidate_00", 0.6, 0.4, 0.99)),
        "/resolve?q=Shaq",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Please clarify"));
    assert!(html.contains("/players/onealsh01"));
    assert!(html.contains("1993–2011"));
    assert!(!html.contains("Shortest chain"));
    assert!(!html.contains("test-private-key"));
}
#[tokio::test]
async fn malformed_answers_cannot_select_unlisted_ids_and_status_falls_back() {
    use jev_client::JevAnswer;
    let invalid = [
        judgment("invented_player", 0.98, 0.98, 0.99),
        judgment("candidate_00", 1.2, 0.98, 0.99),
        judgment("candidate_00", 0.98, f64::NAN, 0.99),
        JevOutcome::Answers(vec![(
            "choice".into(),
            JevAnswer::Choice(
                "candidate_00".into(),
                vec![("candidate_00".into(), 0.98), ("none".into(), 0.02)],
                0.98,
            ),
        )]),
        JevOutcome::Answers(vec![
            (
                "choice".into(),
                JevAnswer::Choice(
                    "candidate_00".into(),
                    vec![("candidate_00".into(), 0.9), ("none".into(), 0.9)],
                    0.98,
                ),
            ),
            ("exists".into(), JevAnswer::Noul(0.99)),
        ]),
    ];
    for bad in invalid {
        let router = app(bad);
        let result = get(router.clone(), "/api/resolve?q=Shaq").await;
        assert_eq!(result["status"], "unavailable");
        assert!(result["player"].is_null());
        assert!(result.get("measurement").is_none());
        assert!(result.get("metadata").is_none());
        assert_eq!(
            get(router.clone(), "/api/semantic-status").await["available"],
            false
        );
        assert_eq!(
            get(router, "/api/resolve?q=Michael%20Jordan").await["status"],
            "resolved"
        );
    }
}
#[tokio::test]
async fn stated_context_can_disambiguate_namesakes_but_uncertain_existence_cannot() {
    use jev_client::JevAnswer;
    let answer = JevOutcome::Answers(vec![
        (
            "choice".into(),
            JevAnswer::Choice(
                "candidate_01".into(),
                vec![
                    ("candidate_00".into(), 0.01),
                    ("candidate_01".into(), 0.98),
                    ("none".into(), 0.01),
                ],
                0.97,
            ),
        ),
        ("exists".into(), JevAnswer::Noul(0.99)),
    ]);
    let result = get(
        app(answer),
        "/api/resolve?q=Joe%20Smith&context=1947%20Stags",
    )
    .await;
    assert_eq!(result["status"], "resolved");
    assert_eq!(result["player"]["id"], "smithjo02");
    let result = get(
        app(judgment("candidate_00", 0.98, 0.98, 0.5)),
        "/api/resolve?q=Shaq",
    )
    .await;
    assert_eq!(result["status"], "clarification");
    assert!(result["player"].is_null());
    let too_long = format!("/api/resolve?q=Shaq&context={}", "a".repeat(161));
    assert_eq!(
        request(app(JevOutcome::Unavailable), &too_long).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, html) = request(
        app(JevOutcome::Unavailable),
        "/resolve?q=%3Cscript%3Eevil%3C%2Fscript%3E",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_service_uses_real_wire_choice_and_noul_order_variants_and_per_call_metrics() {
    use app_server::{
        resolution::{OptionOrder, PromptVariant, ResolutionConfig, ResolutionInput, resolve},
        search::{PlayerCatalog, SearchPlayer},
    };
    use axum::{Json, routing::post};
    use std::sync::{Arc, Mutex};
    let seen = Arc::new(Mutex::new(Vec::new()));
    let capture = seen.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let provider=Router::new().route("/v1/systemone",post(move|Json(payload):Json<Value>|{let capture=capture.clone();async move{
        capture.lock().unwrap().push(payload);
        Json(json!({"id":"resolution-call","model":"scripted-jev","provider":"scripted","usage":{"input_tokens":431,"output_tokens":45,"cost":0.00002},"answers":{"choice":{"type":"choice","choice":"candidate_00","probabilities":{"candidate_00":0.98,"candidate_01":0.01,"none":0.01},"confidence":0.97},"exists":{"type":"noul","noul":0.99}}}))
    }}));
    tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    });
    let mut provider_config = JevConfig::new("test-private-key".into());
    provider_config.base_url = format!("http://{addr}");
    let handle = JevHandle::from_client(JevClient::new(
        provider_config,
        jev_client::http_transport::HttpJevTransport::new(),
    ));
    let catalog = PlayerCatalog::new(vec![
        SearchPlayer {
            id: "smithjo01".into(),
            name: "Joe Smith".into(),
            aliases: vec![],
            first_season: Some(1996),
            last_season: Some(2011),
            teams: vec!["WARRIORS".into()],
        },
        SearchPlayer {
            id: "smithjo02".into(),
            name: "Joe Smith".into(),
            aliases: vec![],
            first_season: Some(1947),
            last_season: Some(1948),
            teams: vec!["STAGS".into()],
        },
    ]);
    let input = ResolutionInput {
        mention: "Joe Smith".into(),
        context: Some("1947 Stags".into()),
    };
    let config = ResolutionConfig {
        option_order: OptionOrder::Reverse,
        prompt_variant: PromptVariant::ExplicitIdentity,
        ..Default::default()
    };
    let call = resolve(&catalog, &handle, &input, &config).unwrap();
    assert_eq!(call.result.player.unwrap().id, "smithjo02");
    let metadata = call.measurement.unwrap().metadata.unwrap();
    assert_eq!(metadata.input_tokens, Some(431));
    assert_eq!(metadata.cost_usd, Some(0.00002));
    let payload = seen.lock().unwrap()[0].clone();
    assert_eq!(
        payload["state"]["candidates"][0]["player"]["id"],
        "smithjo02"
    );
    assert!(
        payload["questions"]["choice"]["criteria"]["candidate_00"]
            .as_str()
            .unwrap()
            .contains("STAGS")
    );
    assert!(
        payload["questions"]["choice"]["criteria"]["candidate_01"]
            .as_str()
            .unwrap()
            .contains("WARRIORS")
    );
    assert_eq!(payload["questions"]["exists"]["type"], "noul");
    assert_eq!(payload["questions"]["choice"]["type"], "choice");
    assert!(
        payload["questions"]["choice"]["criteria"]
            .get("none")
            .is_some()
    );
    let truncated = ResolutionConfig {
        candidate_limit: 1,
        ..Default::default()
    };
    let call = resolve(
        &catalog,
        &JevHandle::from_client(JevClient::new(
            JevConfig::new("fake".into()),
            MemoryTransport::scripted(vec![judgment("candidate_00", 0.99, 0.99, 0.99)]),
        )),
        &input,
        &truncated,
    )
    .unwrap();
    assert_eq!(call.result.status, "clarification");
    assert_eq!(call.result.reason, "shortlist_truncated");
    let absent = resolve(
        &catalog,
        &JevHandle::from_client(JevClient::new(
            JevConfig::new("fake".into()),
            MemoryTransport::scripted(vec![judgment("candidate_00", 0.99, 0.99, 0.01)]),
        )),
        &input,
        &truncated,
    )
    .unwrap();
    assert_eq!(
        absent.result.status, "clarification",
        "an omitted candidate cannot be ruled out"
    );
}
/// Bounded opt-in provider smoke; full accuracy/calibration belongs to #16.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an explicitly requested live OpenRouter call"]
async fn live_resolution_smoke_uses_the_runtime_service_and_pinned_catalog() {
    use app_server::{
        resolution::{ResolutionConfig, ResolutionInput, resolve},
        search::PlayerCatalog,
    };
    let report_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let (graph, reports) = app_server::report_data::load(&report_root).unwrap();
    let catalog = PlayerCatalog::from_graph(&graph, Some(&reports));
    let key = std::env::var("OPENROUTER_API_KEY")
        .expect("live smoke explicitly needs OpenRouter configuration");
    let handle = JevHandle::from_client(JevClient::new(
        JevConfig::new(key),
        jev_client::http_transport::HttpJevTransport::new(),
    ));
    let mut receipt = Vec::new();
    for (q, id) in [
        ("Shaq", "onealsh01"),
        ("Air Jordan", "jordami01"),
        ("Micheal Jordan", "jordami01"),
    ] {
        let call = resolve(
            &catalog,
            &handle,
            &ResolutionInput {
                mention: q.into(),
                context: None,
            },
            &ResolutionConfig::default(),
        )
        .unwrap();
        let measurement = call.measurement.as_ref().unwrap();
        receipt.push(json!({"mention":q,"expected_id":id,"result":call.result,"elapsed_ms":measurement.elapsed_ms,"metadata":measurement.metadata}));
        // Smoke checks boundary safety; acceptance rate is evaluated separately.
        if let Some(player) = call.result.player {
            assert_eq!(player.id, id);
        }
        assert!(matches!(
            call.result.status.as_str(),
            "resolved" | "clarification" | "unavailable" | "no_match"
        ));
    }
    if let Ok(path) = std::env::var("RESOLUTION_SMOKE_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&receipt).unwrap()).unwrap();
    }
}
