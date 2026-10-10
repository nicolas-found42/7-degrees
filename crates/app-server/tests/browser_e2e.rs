//! Whole user journeys at the approved actual-browser seam. No live provider calls.
#[path = "support/browser.rs"]
mod browser;
use browser::{Browser, Server};
use serde_json::json;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM E2E; build scripts/build-canvas.sh first"]
async fn home_lookup_and_connections_are_accessible_in_the_running_fixture_app() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    println!("Chromium {}", b.call(json!({"op":"launch"})));
    b.call(json!({"op":"goto","url":server.url}));
    assert!(b.text("main").contains("fixture demo"));
    b.call(json!({"op":"fill","selector":"#player-query","value":"Player A"}));
    b.click("Search players");
    assert!(b.text("main").contains("Player A"));
    assert!(b.text("main").contains("Green"));
    b.call(json!({"op":"click","role":"link","name":"Player A"}));
    assert_eq!(b.text("h1"), "Player A");
    b.call(json!({"op":"click","role":"link","name":"Start a connection with Player A"}));
    b.call(json!({"op":"select","selector":"#to","value":"B"}));
    b.click("Connect");
    assert_chain(&mut b, 1, &["Player A", "Player B"]);
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(json!({"op":"select","selector":"#from","value":"A"}));
    b.call(json!({"op":"select","selector":"#to","value":"C"}));
    b.click("Connect");
    assert_chain(&mut b, 2, &["Player A", "Player B", "Player C"]);
    b.call(json!({"op":"click","role":"link","name":"Explore selected chain in the graph"}));
    b.ready();
    let original_zoom = b.attr("data-zoom");
    let original_pan = b.attr("data-pan-x");
    b.call(json!({"op":"focus","selector":"#graph-canvas"}));
    b.call(json!({"op":"key","key":"ArrowRight"}));
    assert_ne!(b.attr("data-pan-x"), original_pan);
    b.click("Zoom in");
    assert_ne!(b.attr("data-zoom"), original_zoom);
    b.click("Refocus selected chain");
    assert_eq!(b.attr("data-zoom"), original_zoom);
    assert_eq!(b.attr("data-pan-x"), original_pan);
    assert_chain(&mut b, 2, &["Player A", "Player B", "Player C"]);
    b.call(json!({"op":"select","selector":"#graph-player","value":"C"}));
    b.click("Expand direct connections");
    b.ready();
    assert!(b.text("#graph-players").contains("Player C2"));
    assert!(!b.text("#graph-players").contains("Player E"));
    b.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    b.click("Expand nearby connections");
    b.ready();
    assert!(b.text("#graph-players").contains("Player E"));
    assert!(!b.text("#graph-players").contains("Player D"));
    b.call(json!({"op":"focus","selector":"#select-node-0"}));
    b.call(json!({"op":"key","key":"Enter"}));
    assert!(b.text("#graph-selection").contains("Player A"));
    assert_chain(&mut b, 2, &["Player A", "Player B", "Player C"]);
    b.click("Player A ↔ Player B");
    b.call(json!({"op":"ready","frame":"#edge-provenance-frame","selector":"h1"}));
    let fixture_panel =
        b.call(json!({"op":"text","frame":"#edge-provenance-frame","selector":"main"}));
    assert!(
        fixture_panel
            .as_str()
            .unwrap()
            .contains("Overlap: [day 2, day 4)")
    );
    assert!(
        fixture_panel
            .as_str()
            .unwrap()
            .contains("Synthetic fixture")
    );
    screenshot(
        &mut b,
        "fixture-evidence-panel.png",
        Some("#edge-provenance-panel"),
    );
    screenshot(&mut b, "fixture-graph.png", None);
    b.call(json!({"op":"goto","url":format!("{}/chain?from=A&to=D",server.url)}));
    assert!(
        b.text("main")
            .contains("Verified disconnected within this synthetic fixture")
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":"#selected-chain"})),
        0
    );
    b.call(json!({"op":"goto","url":format!("{}/chain?from=A&to=A",server.url)}));
    assert_chain(&mut b, 0, &["Player A"]);
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(json!({"op":"fill","selector":"#player-query","value":"intergalactic banana"}));
    b.click("Search players");
    assert!(b.text("main").contains("No matching players found"));
    b.call(json!({"op":"click","role":"link","name":"Network statistics"}));
    let stats = b.text("main");
    for phrase in [
        "Unreachable unordered pairs",
        "Self-pairs are excluded",
        "maximum finite shortest-path length",
        "diameter 0",
        "Histogram bins count reachable pairs",
    ] {
        assert!(stats.contains(phrase), "{phrase}");
    }
    assert_eq!(b.text("tbody"), "152332");
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

fn assert_chain(b: &mut Browser, degree: usize, names: &[&str]) {
    assert!(
        b.text("#selected-chain")
            .contains(&format!("Degree of separation: {degree}"))
    );
    assert!(
        b.text("#selected-chain")
            .contains(&format!("Teammate links in this chain: {degree}"))
    );
    let text = b.text("#selected-chain .path");
    assert_eq!(text, names.join(""));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#selected-chain .path li"})),
        names.len()
    );
    for layout in ["desktop", "mobile"] {
        assert_eq!(
            b.call(json!({"op":"count","selector":format!(".chain-map-{layout} .map-player")})),
            names.len()
        );
        assert_eq!(
            b.call(json!({"op":"count","selector":format!(".chain-map-{layout} .map-connection")})),
            degree
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium responsive player-chain visualizer"]
async fn maravich_mcgrady_visualizer_links_profiles_and_evidence_on_desktop_and_mobile() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    let url = format!("{}/chain?from=maravpe01&to=mcgratr01", server.url);
    b.call(json!({"op":"goto","url":url}));
    assert_chain(
        &mut b,
        3,
        &["Pete Maravich", "Larry Bird", "Dee Brown", "Tracy McGrady"],
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":".chain-map-desktop .map-player"})),
        4
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":".chain-map-desktop .map-connection"})),
        3
    );
    assert!(b.text(".chain-map-desktop").contains("≥ 35 shared games"));
    screenshot(
        &mut b,
        "maravich-mcgrady-visualizer.png",
        Some("#selected-chain"),
    );
    b.call(json!({"op":"focus","selector":".chain-map-desktop .map-player[data-player-id='birdla01']"}));
    b.call(json!({"op":"key","key":"Enter"}));
    assert_eq!(b.text("h1"), "Larry Bird");
    b.call(json!({"op":"goto","url":url}));
    b.call(json!({"op":"click","selector":".chain-map-desktop .map-connection[data-from='maravpe01'] .map-evidence"}));
    assert!(b.text("main").contains("Pete Maravich ↔ Larry Bird"));
    b.call(json!({"op":"viewport","width":390,"height":844}));
    b.call(json!({"op":"goto","url":url}));
    b.call(json!({"op":"ready","selector":".chain-map-mobile"}));
    let desktop = b.call(json!({"op":"box","selector":".chain-map-desktop"}));
    assert!(desktop.is_null());
    let mobile = b.call(json!({"op":"box","selector":".chain-map-mobile"}));
    assert!(mobile["width"].as_f64().unwrap() <= 390.0);
    screenshot(&mut b, "maravich-mcgrady-mobile.png", Some(".chain-map"));
    b.call(json!({"op":"click","selector":".chain-map-mobile .map-connection[data-from='brownde01'] .map-evidence"}));
    assert!(b.text("main").contains("Dee Brown ↔ Tracy McGrady"));
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium over every player and every teammate connection"]
async fn whole_network_renders_all_connections_and_keeps_them_when_searching_and_zooming() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    let start = std::time::Instant::now();
    b.call(json!({"op":"goto","url":format!("{}/graph?from=maravpe01&to=mcgratr01&all=true",server.url)}));
    b.ready();
    println!(
        "Whole network: 5106 players / 101395 connections ready in {} ms",
        start.elapsed().as_millis()
    );
    assert_eq!(b.attr("data-players"), "5106");
    assert_eq!(b.attr("data-relationships"), "101395");
    let counts = b.call(json!({"op":"payload-counts"}));
    assert_eq!(
        counts,
        json!({"players":5106,"relationships":101395,"full_network":true,"truncated":false,"path":["maravpe01","birdla01","brownde01","mcgratr01"],"highlighted":3})
    );
    let overview_zoom = b.attr("data-zoom");
    screenshot(&mut b, "all-player-network.png", Some("#graph-canvas"));
    b.call(json!({"op":"fill","selector":"#network-search","value":"Tracy McGrady"}));
    b.click("Find in network");
    assert!(b.text("#graph-selection").contains("Tracy McGrady"));
    assert_ne!(b.attr("data-zoom"), overview_zoom);
    assert!(b.text("#network-player-links").contains("Dee Brown"));
    assert_eq!(b.call(json!({"op":"payload-counts"})), counts);
    b.call(json!({"op":"click","selector":"#network-player-links button","role":"button","name":"Dee Brown — RAPTORS"}));
    assert!(b.text("#selected-edge").contains("Dee Brown"));
    b.call(json!({"op":"fill","selector":"#network-search","value":"Timmy Allen"}));
    b.click("Find in network");
    assert!(b.text("#graph-selection").contains("Timmy Allen"));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#open-selected-edge","name":"href"})),
        serde_json::Value::Null
    );
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#open-selected-edge","name":"hidden"})),
        json!("")
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":"#network-player-links li"})),
        0
    );
    b.click("Fit whole network");
    assert_eq!(b.attr("data-zoom"), overview_zoom);
    assert_eq!(b.call(json!({"op":"payload-counts"})), counts);
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}
fn screenshot(b: &mut Browser, name: &str, selector: Option<&str>) {
    let root = std::env::var("BROWSER_E2E_ARTIFACTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-e2e")
        });
    std::fs::create_dir_all(&root).unwrap();
    let mut q = json!({"op":"screenshot","path":root.join(name).to_string_lossy()});
    if let Some(selector) = selector {
        q["selector"] = json!(selector);
    }
    b.call(q);
}

use jev_client::{
    JevAnswer, JevClient, JevConfig, JevOutcome, JevQuestion, JevRequest, JevTransport,
};
const CANARY: &str = "browser-server-only-canary-17";
struct Scripted;
impl JevTransport for Scripted {
    fn evaluate(&self, _: &JevConfig, r: &JevRequest) -> JevOutcome {
        if let Some(mention) = r.state["mention"].as_str() {
            let intended = match mention {
                "Shaq" => "onealsh01",
                "Micheal Jordan" | "Air Jordan" => "jordami01",
                "Quincy Aci" => "acyqu01",
                "Dee Brown" => "brownde03",
                _ => panic!("unscripted mention {mention}"),
            };
            let selected = r.state["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["player"]["id"] == intended)
                .unwrap()["option"]
                .as_str()
                .unwrap();
            let JevQuestion::Choice { criteria, .. } = &r.questions[0].1 else {
                panic!("Choice")
            };
            let confidence = if r.state["context"] == "uncertain" {
                0.2
            } else {
                1.0
            };
            return JevOutcome::Answers(vec![
                (
                    "choice".into(),
                    JevAnswer::Choice(
                        selected.into(),
                        criteria
                            .iter()
                            .map(|(id, _)| (id.clone(), f64::from(id == selected)))
                            .collect(),
                        confidence,
                    ),
                ),
                ("exists".into(), JevAnswer::Noul(1.0)),
            ]);
        }
        let q = r.state["query"].as_str().expect("scripted query");
        if q == "provider down" {
            return JevOutcome::Unavailable;
        }
        let operation = if q == "forecast weather" {
            "Unsupported, off-topic, adversarial or unrequested operation"
        } else if q.starts_with("connect") {
            "Connect two players through their shortest teammate chain"
        } else if q.starts_with("profile") {
            "Show a player's sourced identity, teams and era"
        } else {
            "Show a player's evidenced direct teammates"
        };
        let first = if q.contains("Dee Brown") {
            "Player mention: Dee Brown"
        } else if q.contains("Player Alpha") {
            "Player mention: Player Alpha"
        } else if q.contains("Player A") {
            "Player mention: Player A"
        } else if q.contains("Shaq") {
            "Player mention: Shaq"
        } else {
            "No player mention stated"
        };
        let second = if q.contains("Quincy Acy") {
            "Player mention: Quincy Acy"
        } else if q.contains("Player C") {
            "Player mention: Player C"
        } else {
            "No player mention stated"
        };
        let team = if q.contains("MAVERICKS") {
            "Team filter: MAVERICKS"
        } else {
            "No stated team filter"
        };
        let era = if q.contains("2000") {
            "Era filter: 2000 (season-ending years 2000–2000)"
        } else if q.contains("2010") {
            "Era filter: 2010 (season-ending years 2010–2010)"
        } else {
            "No stated era filter"
        };
        let confidence = if q.starts_with("uncertain") { 0.2 } else { 1.0 };
        JevOutcome::Answers(
            r.questions
                .iter()
                .map(|(id, question)| {
                    let wanted = match id.as_str() {
                        "operation" => operation,
                        "first_mention" => first,
                        "second_mention" => second,
                        "team" => team,
                        "era" => era,
                        _ => panic!("unscripted field {id}"),
                    };
                    let JevQuestion::Choice { criteria, .. } = question else {
                        panic!("Choice")
                    };
                    let selected = &criteria
                        .iter()
                        .find(|(_, meaning)| meaning == wanted)
                        .unwrap_or_else(|| panic!("missing {wanted}"))
                        .0;
                    (
                        id.clone(),
                        JevAnswer::Choice(
                            selected.clone(),
                            criteria
                                .iter()
                                .map(|(id, _)| (id.clone(), f64::from(id == selected)))
                                .collect(),
                            confidence,
                        ),
                    )
                })
                .collect(),
        )
    }
}
fn semantic() -> app_server::JevHandle {
    app_server::JevHandle::from_client(JevClient::new(JevConfig::new(CANARY.into()), Scripted))
}
fn synthetic_diamond() -> axum::Router {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/browser-e2e-inputs/synthetic-diamond");
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"),"bbr_player_id,display_name,first_season,last_season,aba_only\na,Player Alpha,2000,2010,N\nb,Player Beta,2000,2010,N\nc,Player Gamma,2010,2010,N\ng,Player Goal,2010,2010,N\n").unwrap();
    // A-B-G and A-C-G are the only equal shortest paths. Distinct teams prevent cross links.
    std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\na,2000,NBA,MAVERICKS,synthetic-fixture,directly-evidenced,1,5,1,1\nb,2000,NBA,MAVERICKS,synthetic-fixture,directly-evidenced,2,4,1,1\na,2010,NBA,LAKERS,synthetic-fixture,directly-evidenced,10,15,1,1\nc,2010,NBA,LAKERS,synthetic-fixture,directly-evidenced,11,14,1,1\nb,2010,NBA,BUCKS,synthetic-fixture,directly-evidenced,20,25,1,1\ng,2010,NBA,BUCKS,synthetic-fixture,directly-evidenced,21,24,1,1\nc,2010,NBA,BULLS,synthetic-fixture,directly-evidenced,30,35,1,1\ng,2010,NBA,BULLS,synthetic-fixture,directly-evidenced,31,34,1,1\n").unwrap();
    app_server::app_with_report_data(root, semantic()).unwrap()
}
fn ask(b: &mut Browser, server: &Server, q: &str) {
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(json!({"op":"fill","selector":"#network-query","value":q}));
    b.click("Ask");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM E2E; explicitly synthetic report-backed diamond"]
async fn alternatives_filters_uncertainty_and_provider_failure_are_visible_user_outcomes() {
    let server = Server::start(synthetic_diamond()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch","captureResponses":true}));
    b.call(json!({"op":"goto","url":format!("{}/chain?from=a&to=g&limit=1",server.url)}));
    assert_chain(&mut b, 2, &["Player Alpha", "Player Beta", "Player Goal"]);
    assert!(b.text(".alternatives").contains("2 shortest chain(s)"));
    b.call(json!({"op":"click","role":"link","name":"Next shortest alternatives"}));
    assert_chain(&mut b, 2, &["Player Alpha", "Player Gamma", "Player Goal"]);
    let cursor =
        b.call(json!({"op":"attribute","selector":"[data-selected-index]","name":"data-cursor"}));
    assert_eq!(cursor, "v1:1");
    b.call(json!({"op":"click","role":"link","name":"Select alternative 1"}));
    assert_chain(&mut b, 2, &["Player Alpha", "Player Gamma", "Player Goal"]);
    b.call(json!({"op":"click","role":"link","name":"Explore selected chain in the graph"}));
    b.ready();
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"input[name=cursor]","name":"value"})),
        cursor
    );
    assert_chain(&mut b, 2, &["Player Alpha", "Player Gamma", "Player Goal"]);
    b.call(json!({"op":"goto","url":format!("{}/chain?from=a&to=g&selected=1",server.url)}));
    assert_chain(&mut b, 2, &["Player Alpha", "Player Gamma", "Player Goal"]);
    assert_eq!(b.call(json!({"op":"attribute","selector":"[data-selected-index]","name":"data-selected-index"})),"1");
    ask(&mut b, &server, "teammates Player Alpha");
    assert!(b.text("main").contains("2 evidenced direct teammate(s)"));
    ask(
        &mut b,
        &server,
        "teammates Player Alpha on MAVERICKS in 2000",
    );
    let filtered = b.text("main");
    assert!(filtered.contains("Team: MAVERICKS"));
    assert!(filtered.contains("Season-ending years: 2000–2000"));
    assert!(filtered.contains("1 evidenced direct teammate(s)"));
    assert_eq!(b.text("main > ul"), "Player Beta");
    ask(&mut b, &server, "teammates Player Alpha in 2010");
    assert_eq!(b.text("main > ul"), "Player Gamma");
    ask(&mut b, &server, "teammates Player Alpha");
    assert!(b.text("main").contains("2 evidenced direct teammate(s)"));
    ask(&mut b, &server, "uncertain Player Alpha");
    assert_eq!(
        b.call(
            json!({"op":"attribute","selector":"[data-query-status]","name":"data-query-status"})
        ),
        "clarification"
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":"#selected-chain"})),
        0
    );
    ask(&mut b, &server, "forecast weather");
    assert_eq!(
        b.call(
            json!({"op":"attribute","selector":"[data-query-status]","name":"data-query-status"})
        ),
        "unsupported"
    );
    ask(&mut b, &server, "provider down");
    assert!(
        b.text("main")
            .contains("Semantic features (Jev): unavailable")
    );
    b.call(json!({"op":"click","role":"link","name":"Connect known players"}));
    b.call(json!({"op":"select","selector":"#from","value":"a"}));
    b.call(json!({"op":"select","selector":"#to","value":"g"}));
    b.click("Connect");
    assert_chain(&mut b, 2, &["Player Alpha", "Player Beta", "Player Goal"]);
    assert_eq!(
        b.call(json!({"op":"response-contains","text":CANARY})),
        false
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
    let interpreted = Server::start(app_server::app_with_jev(semantic())).await;
    ask(&mut b, &interpreted, "connect Player A to Player C");
    assert!(b.text("main").contains("Degree of separation: 2"));
    assert_eq!(b.text("main > ol"), "Player APlayer BPlayer C");
    assert!(
        b.text("main")
            .contains("Semantic interpretation; graph results are deterministic")
    );
    assert_eq!(
        b.call(json!({"op":"response-contains","text":CANARY})),
        false
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
    println!(
        "Synthetic diamond: equal alternatives/cursor/selection, team+era filters/removal, uncertainty/unsupported/provider-down fallback passed."
    );
}

fn artifacts() -> std::path::PathBuf {
    std::env::var("BROWSER_E2E_ARTIFACTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-e2e")
        })
}
fn real_slice() -> axum::Router {
    use sha2::{Digest, Sha256};
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/browser-e2e-inputs/real-slice");
    let selected = [
        "acyqu01",
        "bogutan01",
        "brownde01",
        "brownde03",
        "jordami01",
        "onealsh01",
        "nba:1630492",
    ];
    let mut mapping = Vec::new();
    for (file, pinned) in [
        (
            "t3/player-universe.csv",
            "81031c7e3cf95845060119a3347d3cf37ec60c6a24b0ced9978fb7df2e7e651d",
        ),
        (
            "t4/tenures.csv",
            "2bd2106ba3c0819b1a065246950f527a4267feb5f8a2054a11318205c04a1ab1",
        ),
    ] {
        let bytes = std::fs::read(source.join(file)).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            hash, pinned,
            "Revalidate the pinned real browser slice after source changes"
        );
        let mut reader = csv::Reader::from_reader(bytes.as_slice());
        let header = reader.headers().unwrap().clone();
        let id = header.iter().position(|c| c == "bbr_player_id").unwrap();
        std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
        let mut writer = csv::Writer::from_path(root.join(file)).unwrap();
        writer.write_record(&header).unwrap();
        let mut slice_line = 2;
        for (index, row) in reader.records().enumerate() {
            let row = row.unwrap();
            if !selected.contains(&&row[id]) {
                continue;
            }
            if file.starts_with("t4") {
                let field = |name: &str| {
                    row.get(header.iter().position(|c| c == name).unwrap())
                        .unwrap()
                };
                mapping.push(json!({"slice_record":format!("{file}:{slice_line}"),"original_record":format!("{file}:{}",index+2),"snapshot_sha256":hash,"player":field("bbr_player_id"),"team":field("canonical_franchise"),"season":field("season"),"evidence_class":field("evidence_class")}));
                if index + 2 == 123 {
                    assert_eq!(
                        (
                            &row[id],
                            field("canonical_franchise"),
                            field("start_day"),
                            field("end_day"),
                            field("start_anchored"),
                            field("end_anchored")
                        ),
                        ("acyqu01", "MAVERICKS", "25768", "25889", "1", "1")
                    );
                }
                if index + 2 == 2602 {
                    assert_eq!(
                        (
                            &row[id],
                            field("canonical_franchise"),
                            field("start_day"),
                            field("end_day"),
                            field("start_anchored"),
                            field("end_anchored")
                        ),
                        ("bogutan01", "MAVERICKS", "25755", "25986", "1", "1")
                    );
                }
            }
            writer.write_record(&row).unwrap();
            slice_line += 1;
        }
        writer.flush().unwrap();
        if file.starts_with("t3") {
            assert_eq!(slice_line - 2, 7);
        }
    }
    for original in [
        "t4/tenures.csv:123",
        "t4/tenures.csv:2602",
        "t4/tenures.csv:19618",
    ] {
        assert!(mapping.iter().any(|r| r["original_record"] == original));
    }
    let out = artifacts();
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("real-slice-map.json"),serde_json::to_string_pretty(&json!({"scope":"Seven selected canonical player nodes; not the full universe","coverage_complete":false,"records":mapping})).unwrap()).unwrap();
    println!(
        "Pinned real slice: 7 players, {} exact committed tenure rows; mapping retains original record references and snapshot SHA-256.",
        mapping.len()
    );
    app_server::app_with_report_data(root, semantic()).unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM E2E over a seven-player pinned real-data slice"]
async fn real_lookup_disambiguation_and_selected_edge_provenance_remain_honest() {
    let server = Server::start(real_slice()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch","captureResponses":true}));
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(
        json!({"op":"fill","selector":"#network-query","value":"connect Dee Brown to Quincy Acy"}),
    );
    b.click("Ask");
    let candidates = b.call(json!({"op":"text","selector":"[data-query-candidates]"}));
    let candidates = candidates.as_str().unwrap();
    assert!(candidates.contains("1991") && candidates.contains("2009"));
    assert!(candidates.contains("CELTICS") && candidates.contains("WIZARDS"));
    assert_eq!(
        b.call(json!({"op":"count","selector":"[data-query-candidates] button"})),
        2
    );
    assert_eq!(b.call(json!({"op":"count","selector":"main ol"})), 0);
    screenshot(&mut b, "query-clarification.png", Some("main"));
    b.click("Choose Dee Brown (brownde01)");
    assert_eq!(
        b.call(
            json!({"op":"attribute","selector":"[data-query-status]","name":"data-query-status"})
        ),
        "executed"
    );
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#network-query","name":"value"})),
        "connect Dee Brown to Quincy Acy"
    );
    let continued = b.call(json!({"op":"text","selector":"main"}));
    assert!(
        continued
            .as_str()
            .unwrap()
            .contains("No teammate chain is established")
    );
    let link = b.call(
        json!({"op":"attribute","selector":"a[href^='/chain?from=brownde01']","name":"href"}),
    );
    assert_eq!(link, "/chain?from=brownde01&to=acyqu01");
    screenshot(&mut b, "query-chosen.png", Some("main"));
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(json!({"op":"fill","selector":"#player-query","value":"Quincy Aci"}));
    b.click("Search players");
    assert!(b.text("main").contains("Quincy Acy"));
    assert!(b.text("main").contains("2013–2019"));
    assert!(b.text("main").contains("MAVERICKS"));
    b.call(json!({"op":"click","role":"link","name":"Quincy Acy"}));
    assert_eq!(b.text("h1"), "Quincy Acy");
    b.call(json!({"op":"click","role":"link","name":"Start a connection with Quincy Acy"}));
    b.call(json!({"op":"select","selector":"#to","value":"bogutan01"}));
    b.click("Connect");
    assert_chain(&mut b, 1, &["Quincy Acy", "Andrew Bogut"]);
    let standalone=b.call(json!({"op":"open-link-text","selector":"#selected-chain .links a","path":artifacts().join("real-evidence-page.png").to_string_lossy()}));
    assert!(
        artifacts().join("real-evidence-page.png").is_file(),
        "native evidence page screenshot must exist"
    );
    let standalone = standalone.as_str().unwrap();
    for phrase in [
        "MAVERICKS",
        "2016–17",
        "2016-07-20",
        "2016-11-18",
        "Andrew Bogut",
        "2016-07-07",
        "2017-02-23",
        "Source evidence is incomplete",
        "Missing evidence does not prove a historical non-relationship",
    ] {
        assert!(standalone.contains(phrase), "{phrase}");
    }
    assert!(standalone.matches("Source record t4/tenures.csv:").count() >= 2);
    let mapping: serde_json::Value =
        serde_json::from_slice(&std::fs::read(artifacts().join("real-slice-map.json")).unwrap())
            .unwrap();
    for original in ["t4/tenures.csv:123", "t4/tenures.csv:2602"] {
        let reference = mapping["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["original_record"] == original)
            .unwrap()["slice_record"]
            .as_str()
            .unwrap();
        assert!(
            standalone.contains(reference),
            "Both admitted references must trace through the pinned slice map: {original}"
        );
    }
    b.call(json!({"op":"click","role":"link","name":"Explore selected chain in the graph"}));
    b.ready();
    b.click("Quincy Acy ↔ Andrew Bogut");
    b.call(json!({"op":"ready","frame":"#edge-provenance-frame","selector":"h1"}));
    let panel = b.call(json!({"op":"text","frame":"#edge-provenance-frame","selector":"main"}));
    let panel = panel.as_str().unwrap();
    for phrase in [
        "MAVERICKS",
        "2016-07-20",
        "2016-11-18",
        "directly-evidenced",
        "transaction anchored",
        "S1 v238",
        "S2 v56",
        "Source evidence is incomplete",
    ] {
        assert!(panel.contains(phrase), "{phrase}");
    }
    assert_eq!(b.call(json!({"op":"count","frame":"#edge-provenance-frame","selector":"section:not(.coverage) a[href^='/sources/tenure?record=']"})),2);
    assert_chain(&mut b, 1, &["Quincy Acy", "Andrew Bogut"]);
    screenshot(
        &mut b,
        "real-evidence-panel.png",
        Some("#edge-provenance-panel"),
    );
    screenshot(&mut b, "real-graph.png", None);
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(json!({"op":"fill","selector":"#player-query","value":"Dee Brown"}));
    b.click("Search players");
    assert_eq!(
        b.call(json!({"op":"count","selector":".search-candidates > li"})),
        2
    );
    let choices = b.text(".search-candidates");
    for context in ["1991–2002", "2007–2009", "CELTICS", "JAZZ"] {
        assert!(choices.contains(context));
    }
    b.call(json!({"op":"click","selector":"a[href='/players/brownde03']"}));
    assert!(b.text("main").contains("2007–2009"));
    assert!(!b.text("main").contains("1991–2002"));
    b.call(json!({"op":"goto","url":format!("{}/resolve?q=Dee%20Brown",server.url)}));
    assert!(b.text("h1").contains("Please clarify"));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#selected-chain"})),
        0
    );
    b.call(json!({"op":"fill","selector":"#context","value":"2007 JAZZ"}));
    b.click("Find intended player");
    assert_eq!(b.text("h1"), "Player found");
    b.call(json!({"op":"click","role":"link","name":"View Dee Brown"}));
    assert!(b.text("main").contains("2007–2009"));
    for mention in ["Shaq", "Micheal Jordan"] {
        b.call(json!({"op":"goto","url":format!("{}/resolve",server.url)}));
        b.call(json!({"op":"fill","selector":"#mention","value":mention}));
        b.click("Find intended player");
        assert_eq!(b.text("h1"), "Player found");
        assert!(b.text("main").contains("semantic interpretation"));
    }
    ask(&mut b, &server, "profile Shaq");
    assert!(b.text("main").contains("Shaquille O'Neal"));
    assert!(
        b.text("main")
            .contains("Semantic interpretation; graph results are deterministic")
    );
    b.call(json!({"op":"goto","url":format!("{}/resolve?q=Air%20Jordan&context=uncertain",server.url)}));
    assert!(b.text("h1").contains("Please clarify"));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#selected-chain"})),
        0
    );
    b.call(json!({"op":"goto","url":format!("{}/",server.url)}));
    b.call(json!({"op":"fill","selector":"#player-query","value":"Luca Vildoza"}));
    b.click("Search players");
    assert!(b.text("main").contains("2022–2022"));
    b.call(json!({"op":"click","role":"link","name":"Luca Vildoza"}));
    assert!(b.text("main").contains("BUCKS"));
    b.call(json!({"op":"goto","url":format!("{}/chain?from=acyqu01&to=nba%3A1630492",server.url)}));
    assert!(
        b.text("main")
            .contains("Missing dated roster evidence may hide a historical connection")
    );
    assert!(!b.text("main").contains("Verified disconnected"));
    b.call(json!({"op":"goto","url":format!("{}/edge?from=acyqu01&to=nba%3A1630492",server.url)}));
    for phrase in [
        "No admitted direct teammate edge",
        "Source evidence is incomplete",
        "Luca Vildoza",
        "inferred",
    ] {
        assert!(b.text("main").contains(phrase));
    }
    assert_eq!(
        b.call(json!({"op":"response-contains","text":CANARY})),
        false
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
    println!(
        "Real slice: typo/alias, same-name context and clarification/choice, sourced Acy/Bogut chain/panel/native page, excluded postseason-only Vildoza and unresolved coverage passed."
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM over the repaired full historical graph"]
async fn historical_chain_and_appearance_evidence_render_without_fabricated_dates() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let (graph, _) = app_server::report_data::load(&root).unwrap();
    let chains = graph.all_shortest_chains("maravpe01", "abdulka01").unwrap();
    let selected = chains
        .iter()
        .position(|c| c.path == ["maravpe01", "goodrga01", "abdulka01"])
        .unwrap();
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":format!("{}/chain?from=maravpe01&to=abdulka01&selected={selected}",server.url)}));
    assert_chain(
        &mut b,
        2,
        &["Pete Maravich", "Gail Goodrich", "Kareem Abdul-Jabbar"],
    );
    assert!(b.text("#selected-chain").contains("shared team game(s)"));
    assert!(
        b.text("#selected-chain")
            .contains("roster overlap dates unknown")
    );
    screenshot(
        &mut b,
        "maravich-kareem-repaired.png",
        Some("#selected-chain"),
    );
    b.call(json!({"op":"goto","url":format!("{}/edge?from=maravpe01&to=goodrga01",server.url)}));
    let evidence = b.text("main");
    assert!(evidence.contains("27 + 73 − 82 = at least 18"));
    assert!(evidence.contains("Teammate relationship admitted"));
    assert!(!evidence.contains("Positive dated tenure overlap"));
    assert!(evidence.contains("Player Totals.csv:"));
    screenshot(&mut b, "maravich-goodrich-proof.png", None);
    b.call(json!({"op":"goto","url":format!("{}/graph?from=maravpe01&to=abdulka01&selected={selected}",server.url)}));
    b.ready();
    b.click("Gail Goodrich ↔ Pete Maravich");
    assert!(b.text("#selected-edge").contains("shared team game(s)"));
    b.call(json!({"op":"ready","frame":"#edge-provenance-frame","selector":"h1"}));
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}
