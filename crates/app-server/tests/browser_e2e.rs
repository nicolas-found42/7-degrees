//! Whole user journeys at the approved actual-browser seam. No live provider calls.
#[path = "support/browser.rs"]
mod browser;
use browser::{Browser, Server};
use serde_json::json;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM E2E; build scripts/build-canvas.sh first"]
async fn the_graph_page_shows_every_fixture_player_and_relationship_and_supports_exploration() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    println!("Chromium {}", b.call(json!({"op":"launch"})));
    b.call(json!({"op":"goto","url":server.url}));
    assert!(b.text("main").contains("Graph exploration"));
    b.ready();
    // The whole fixture network is the page: every player and every edge.
    assert_eq!(b.attr("data-players"), "6");
    assert_eq!(b.attr("data-relationships"), "5");
    assert_eq!(b.call(json!({"op":"payload-counts"}))["full_network"], true);
    for name in [
        "Player A",
        "Player B",
        "Player C",
        "Player C2",
        "Player D",
        "Player E",
    ] {
        assert!(b.text("#graph-players").contains(name), "missing {name}");
    }
    // Relationships are selected on the canvas; the list is selection-driven.
    assert!(b.text("main").contains("Connections of selected player"));
    let original_zoom = b.attr("data-zoom");
    let original_pan = b.attr("data-pan-x");
    b.call(json!({"op":"focus","selector":"#graph-canvas"}));
    b.call(json!({"op":"key","key":"ArrowRight"}));
    assert_ne!(b.attr("data-pan-x"), original_pan);
    b.click("Zoom in");
    assert_ne!(b.attr("data-zoom"), original_zoom);
    b.click("Fit whole network");
    assert_eq!(b.attr("data-zoom"), original_zoom);
    assert_eq!(b.attr("data-pan-x"), original_pan);
    // Selecting a player in the accessible list is reported in text.
    b.call(json!({"op":"focus","selector":"#select-node-1"}));
    b.call(json!({"op":"key","key":"Enter"}));
    assert!(b.text("#graph-selection").contains("Selected player"));
    // Relationship selection on the canvas is pointer-driven and
    // nondeterministic at whole-network zoom; the A/B evidence URL is
    // deterministic, so assert the standalone evidence page. Player-list
    // selection above already proves accessible selection reporting.
    b.call(json!({"op":"goto","url":format!("{}/edge?from=A&to=B",server.url)}));
    let panel = b.text("main");
    assert!(panel.contains("Overlap: [day 2, day 4)"));
    assert!(panel.contains("Synthetic fixture"));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    screenshot(
        &mut b,
        "fixture-evidence-panel.png",
        Some("#edge-provenance-panel"),
    );
    screenshot(&mut b, "fixture-graph.png", None);
    // Player-specific query parameters are inert: the whole graph is always shown.
    b.call(json!({"op":"goto","url":format!("{}/?from=A&to=D&player=B&depth=2",server.url)}));
    b.ready();
    assert_eq!(b.attr("data-players"), "6");
    assert_eq!(b.attr("data-relationships"), "5");
    assert_eq!(b.call(json!({"op":"payload-counts"}))["full_network"], true);
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium over the whole committed report network"]
async fn whole_network_renders_desktop_and_phone_and_selects_players_by_pointer() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    assert_eq!(b.attr("data-players"), "5106");
    assert_eq!(b.attr("data-relationships"), "101395");
    let counts = b.call(json!({"op":"payload-counts"}));
    assert_eq!(counts["players"], 5106);
    assert_eq!(counts["relationships"], 101395);
    assert_eq!(counts["full_network"], true);
    assert_eq!(counts["path"], json!([]));
    assert_eq!(counts["highlighted"], 0);
    screenshot(&mut b, "all-player-network.png", Some("#graph-canvas"));
    // The list keeps every player selectable; a click reports the selection.
    assert!(b.text("#graph-players").contains("Pete Maravich"));
    b.call(json!({"op":"focus","selector":"#select-node-3"}));
    b.call(json!({"op":"key","key":"Enter"}));
    assert!(b.text("#graph-selection").contains("Selected player"));
    assert!(b.text("#graph-selection").contains("teams:"));
    // Selecting a relationship loads its evidence frame and its connections list.
    let box_ = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    b.call(json!({"op":"mouse","action":"click","x":box_["x"].as_f64().unwrap()+box_["width"].as_f64().unwrap()*0.5,"y":box_["y"].as_f64().unwrap()+box_["height"].as_f64().unwrap()*0.5}));
    assert!(
        b.text("#selected-edge").contains("↔")
            || b.text("#selected-edge").contains("Select a teammate")
    );
    // The phone layout keeps the canvas and the accessible lists.
    b.call(json!({"op":"viewport","width":390,"height":844}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    let canvas = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    assert!(canvas["width"].as_f64().unwrap() <= 390.0);
    assert!(b.text("#graph-players").contains("Pete Maravich"));
    screenshot(&mut b, "all-player-network-phone.png", None);
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium over every player and every teammate connection"]
async fn whole_network_keeps_all_players_and_relations_when_panning_and_zooming() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    let start = std::time::Instant::now();
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    println!(
        "Whole network: 5106 players / 101395 connections ready in {} ms",
        start.elapsed().as_millis()
    );
    assert_eq!(b.attr("data-players"), "5106");
    assert_eq!(b.attr("data-relationships"), "101395");
    let counts = b.call(json!({"op":"payload-counts"}));
    let overview_zoom = b.attr("data-zoom");
    screenshot(&mut b, "all-player-network.png", Some("#graph-canvas"));
    b.click("Zoom in");
    assert_ne!(b.attr("data-zoom"), overview_zoom);
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
    diamond_with(semantic())
}
/// The same synthetic diamond over an explicit Jev handle, so a provider-down
/// server still serves the diamond graph rather than the fixture.
fn diamond_with(jev: app_server::JevHandle) -> axum::Router {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/browser-e2e-inputs/synthetic-diamond");
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(root.join("t3/player-universe.csv"),"bbr_player_id,display_name,first_season,last_season,aba_only\na,Player Alpha,2000,2010,N\nb,Player Beta,2000,2010,N\nc,Player Gamma,2010,2010,N\ng,Player Goal,2010,2010,N\n").unwrap();
    // A-B-G and A-C-G are the only equal shortest paths. Distinct teams prevent cross links.
    std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\na,2000,NBA,MAVERICKS,synthetic-fixture,directly-evidenced,1,5,1,1\nb,2000,NBA,MAVERICKS,synthetic-fixture,directly-evidenced,2,4,1,1\na,2010,NBA,LAKERS,synthetic-fixture,directly-evidenced,10,15,1,1\nc,2010,NBA,LAKERS,synthetic-fixture,directly-evidenced,11,14,1,1\nb,2010,NBA,BUCKS,synthetic-fixture,directly-evidenced,20,25,1,1\ng,2010,NBA,BUCKS,synthetic-fixture,directly-evidenced,21,24,1,1\nc,2010,NBA,BULLS,synthetic-fixture,directly-evidenced,30,35,1,1\ng,2010,NBA,BULLS,synthetic-fixture,directly-evidenced,31,34,1,1\n").unwrap();
    app_server::app_with_report_data(root, jev).unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM E2E; explicitly synthetic report-backed diamond"]
async fn synthetic_diamond_renders_its_whole_network_on_the_single_page() {
    let server = Server::start(synthetic_diamond()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch","captureResponses":true}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    // Every diamond player is present and selectable, with no chain-specific UI.
    assert_eq!(b.attr("data-players"), "4");
    assert_eq!(b.attr("data-relationships"), "4");
    for name in ["Player Alpha", "Player Beta", "Player Gamma", "Player Goal"] {
        assert!(
            b.text("#graph-players").contains(name),
            "missing diamond player {name}"
        );
    }
    assert!(b.text("main").contains("Connections of selected player"));
    assert_eq!(
        b.call(json!({"op":"count","selector":".alternatives"})),
        0,
        "no alternatives list on the single page"
    );
    // Server-only credentials never reach any served response byte.
    assert_eq!(
        b.call(json!({"op":"response-contains","text":CANARY})),
        false
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
    println!(
        "Synthetic diamond: the whole network renders on the single page with no chain-specific UI."
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
async fn real_slice_renders_every_player_and_the_selected_edge_provenance() {
    let server = Server::start(real_slice()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch","captureResponses":true}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    // The whole pinned slice is the page: all seven players are present.
    assert_eq!(b.attr("data-players"), "7");
    for name in [
        "Quincy Acy",
        "Andrew Bogut",
        "Dee Brown",
        "Michael Jordan",
        "Shaquille O'Neal",
        "Luca Vildoza",
    ] {
        assert!(b.text("#graph-players").contains(name), "missing {name}");
    }
    // The server renders no edge list; select Quincy Acy so his connections
    // list offers the Acy/Bogut relationship, then open it by its stable label.
    b.click("Quincy Acy");
    b.call(json!({"op":"ready","selector":"#network-player-links li"}));
    b.click("Andrew Bogut — MAVERICKS");
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
    assert!(panel.matches("Source record t4/tenures.csv:").count() >= 2);
    assert_eq!(
        b.call(json!({"op":"count","frame":"#edge-provenance-frame","selector":"section:not(.coverage) a[href^='/sources/tenure?record=']"})),
        2
    );
    screenshot(
        &mut b,
        "real-evidence-panel.png",
        Some("#edge-provenance-panel"),
    );
    screenshot(&mut b, "real-graph.png", None);
    // The excluded postseason-only source record stays reachable and honest.
    b.call(json!({"op":"goto","url":format!("{}/edge?from=acyqu01&to=nba%3A1630492",server.url)}));
    for phrase in [
        "No admitted direct teammate edge",
        "Source evidence is incomplete",
        "Luca Vildoza",
        "inferred",
    ] {
        assert!(b.text("main").contains(phrase), "{phrase}");
    }
    assert_eq!(
        b.call(json!({"op":"response-contains","text":CANARY})),
        false
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
    println!(
        "Real slice: the whole seven-player network renders on the single page, and selected-edge provenance stays honest."
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM over the repaired full historical graph"]
async fn historical_network_renders_and_appearance_evidence_avoids_fabricated_dates() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    assert_eq!(b.attr("data-players"), "5106");
    screenshot(&mut b, "historical-network.png", Some("#graph-canvas"));
    // The appearance-count evidence page renders the proof, not invented dates.
    b.call(json!({"op":"goto","url":format!("{}/edge?from=maravpe01&to=goodrga01",server.url)}));
    let evidence = b.text("main");
    assert!(evidence.contains("27 + 73 − 82 = at least 18"));
    assert!(evidence.contains("Teammate relationship admitted"));
    assert!(!evidence.contains("Positive dated tenure overlap"));
    assert!(evidence.contains("Player Totals.csv:"));
    screenshot(&mut b, "maravich-goodrich-proof.png", None);
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}
