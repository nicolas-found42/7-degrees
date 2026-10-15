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
        assert!(b.text("#graph-payload").contains(name), "missing {name}");
    }
    // The player list and in-page evidence panel are gone; evidence is standalone.
    for selector in [
        "#graph-players",
        "#network-player-links",
        "#edge-provenance-panel",
    ] {
        assert_eq!(
            b.call(json!({"op":"count","selector":selector})),
            0,
            "{selector} is removed"
        );
    }
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
    // Selecting a player in the picker centers them and reports the selection.
    b.call(json!({"op":"select","selector":"#graph-player","value":"B"}));
    assert!(b.text("#graph-selection").contains("Player B"));
    // Relationship evidence is standalone; the A/B URL is deterministic, so
    // assert it directly. Picker selection proves accessible reporting.
    b.call(json!({"op":"goto","url":format!("{}/edge?from=A&to=B",server.url)}));
    let panel = b.text("main");
    assert!(panel.contains("Overlap: [day 2, day 4)"));
    assert!(panel.contains("Synthetic fixture"));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    screenshot(&mut b, "fixture-graph.png", None);
    // Player-specific query parameters are inert: the whole graph is always shown.
    b.call(json!({"op":"goto","url":format!("{}/?from=A&to=D&player=B&depth=2",server.url)}));
    b.ready();
    assert_eq!(b.attr("data-players"), "6");
    assert_eq!(b.attr("data-relationships"), "5");
    assert_eq!(b.call(json!({"op":"payload-counts"}))["full_network"], true);
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

// Actual pointer dispatch verifies that the renderer and hit-test share the spline.
fn click_visible_bundle(b: &mut Browser, payload: &canvas_view::GraphPayload, focus: &str) {
    let curves: Vec<(String, String, canvas_view::visual::Curve)> =
        serde_json::from_str(&b.attr("data-bundles")).unwrap();
    assert!(!curves.is_empty(), "visible franchise group must bundle");
    b.call(json!({"op":"focus","selector":"#graph-canvas"}));
    let bounds = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    let (w, h) = (
        bounds["width"].as_f64().unwrap(),
        bounds["height"].as_f64().unwrap(),
    );
    for samples in [[0.45, 0.55, 0.7], [0.85, 0.93, 0.97]] {
        let mut chosen = None;
        'pick: for (from, to, curve) in &curves {
            for t in samples {
                let p = curve.point(t);
                if p.0 < 20.0 || p.0 > w - 20.0 || p.1 < 40.0 || p.1 > h - 110.0 {
                    continue;
                }
                let (px, py) = (
                    bounds["x"].as_f64().unwrap() + p.0,
                    bounds["y"].as_f64().unwrap() + p.1,
                );
                b.call(json!({"op":"mouse","action":"move","x":px,"y":py}));
                if b.attr("data-hovered-edge") == format!("{from}|{to}") {
                    chosen = Some((from, to, px, py));
                    break 'pick;
                }
            }
        }
        let (from, to, px, py) =
            chosen.expect("actual bundled curve has a distinct pointer target");
        let other = if from == focus { to } else { from };
        assert_eq!(
            b.call(json!({"op":"attribute","selector":"#graph-tooltip","name":"data-player"})),
            *other
        );
        assert!(
            b.text("#graph-tooltip")
                .contains(&payload.nodes.iter().find(|n| n.id == *other).unwrap().name)
        );
        if w <= 640.0 {
            b.call(json!({"op":"touch","type":"touchStart","points":[{"x":px,"y":py}]}));
            b.call(json!({"op":"touch","type":"touchEnd"}));
        } else {
            b.call(json!({"op":"mouse","action":"click","x":px,"y":py}));
        }
        b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
        assert!(
            b.call(json!({"op":"url"}))
                .as_str()
                .unwrap()
                .ends_with(&format!("/edge?from={from}&to={to}"))
        );
        screenshot(b, "maravich-bundled-evidence.png", None);
        b.click("Close evidence");
    }
}

fn assert_ranked_direct_labels(b: &mut Browser) {
    let labels: Vec<serde_json::Value> = serde_json::from_str(&b.attr("data-label-items")).unwrap();
    let mut weights = Vec::new();
    let mut saw_context = false;
    for label in labels {
        match label["priority"].as_str().unwrap() {
            "DirectTeammate" => {
                assert!(!saw_context, "direct labels must precede context");
                weights.push(label["weight"].as_f64().unwrap());
            }
            "Context" => saw_context = true,
            _ => {}
        }
    }
    assert!(
        !weights.is_empty(),
        "focused Meso must retain direct-teammate labels"
    );
    assert!(weights.windows(2).all(|p| p[0] >= p[1]));
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
    b.call(json!({"op":"launch","touch":true}));
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
    let payload: canvas_view::GraphPayload =
        serde_json::from_str(&b.text("#graph-payload")).unwrap();
    let backbone = payload
        .links
        .iter()
        .filter(|e| e.in_visual_backbone)
        .count();
    assert!(backbone < payload.links.len() / 4);
    assert_eq!(b.attr("data-tier"), "macro");
    assert_eq!(b.attr("data-non-backbone-edges"), "0");
    assert!(b.attr("data-background-edges").parse::<usize>().unwrap() <= backbone);
    let overview_ms = b.attr("data-draw-ms");
    println!(
        "Macro: {} drawn background edges / {backbone} backbone / {} loaded; {overview_ms} ms",
        b.attr("data-background-edges"),
        payload.links.len()
    );
    b.click("Zoom in");
    while b.attr("data-tier") == "macro" {
        b.click("Zoom in");
    }
    assert_eq!(b.attr("data-tier"), "meso");
    assert_eq!(b.attr("data-non-backbone-edges"), "0");
    assert!(b.attr("data-background-edges").parse::<usize>().unwrap() <= backbone);
    while b.attr("data-tier") != "micro" {
        b.click("Zoom in");
    }
    assert!(b.attr("data-non-backbone-edges").parse::<usize>().unwrap() > 0);
    println!(
        "Micro: {} local drawn background edges; {} ms",
        b.attr("data-background-edges"),
        b.attr("data-draw-ms")
    );
    b.click("Fit whole network");
    b.click("Force-directed layout");
    b.call(json!({"op":"settled"}));
    screenshot(
        &mut b,
        "all-player-force-network.png",
        Some("#graph-canvas"),
    );
    b.click("Era lanes layout");
    b.call(json!({"op":"settled"}));
    // The picker keeps every player selectable; a choice reports the selection.
    assert!(b.text("#graph-payload").contains("Pete Maravich"));
    b.call(json!({"op":"select","selector":"#graph-player","value":"maravpe01"}));
    assert!(b.text("#graph-selection").contains("Pete Maravich"));
    assert!(b.text("#graph-selection").contains("teams:"));
    click_visible_bundle(&mut b, &payload, "maravpe01");
    for (button, mode) in [
        ("Era lanes layout", "era"),
        ("Concentric rings layout", "concentric"),
        ("Force-directed layout", "force"),
    ] {
        if mode == "concentric" {
            let tap = b
                .call(json!({"op":"pointer-at-label","player":"*","control":"#layout-concentric"}));
            assert_eq!(
                tap["selected"], tap["id"],
                "first-frame spatial picking must follow displayed points"
            );
            b.call(json!({"op":"settled"}));
            b.call(json!({"op":"select","selector":"#graph-player","value":"maravpe01"}));
        } else {
            b.click(button);
        }
        b.call(json!({"op":"settled"}));
        if mode == "concentric" {
            assert_eq!(
                b.call(json!({"op":"attribute","selector":"#ego-toggle","name":"aria-pressed"})),
                "true"
            );
            screenshot(
                &mut b,
                "maravich-concentric-sectors.png",
                Some("#graph-canvas"),
            );
        }
        b.click("Fit whole network");
        assert_eq!(b.attr("data-tier"), "macro");
        assert!(b.attr("data-ordinary-labels").parse::<usize>().unwrap() <= 10);
        screenshot(
            &mut b,
            &format!("maravich-{mode}-overview.png"),
            Some("#graph-canvas"),
        );
        b.click("Zoom to selected player");
        if mode == "force" {
            click_visible_bundle(&mut b, &payload, "maravpe01");
        }
        b.click("Zoom out");
        b.click("Zoom out");
        assert_eq!(b.attr("data-tier"), "meso");
        assert_ranked_direct_labels(&mut b);
        b.click("Zoom to selected player");
        let signature = b.attr("data-layout-signature");
        assert_eq!(signature.len(), 16);
        let labels = b.attr("data-labels");
        b.call(json!({"op":"goto","url":format!("{}/?from=maravpe01",server.url)}));
        b.ready();
        b.click(button);
        b.call(json!({"op":"settled"}));
        b.click("Fit whole network");
        b.click("Zoom to selected player");
        assert_eq!(signature, b.attr("data-layout-signature"));
        assert_eq!(labels, b.attr("data-labels"));
        let frame = b.attr("data-frame");
        b.call(json!({"op":"network-idle"}));
        assert_eq!(frame, b.attr("data-frame"), "no idle physics frames");
        screenshot(
            &mut b,
            &format!("maravich-{mode}-labels.png"),
            Some("#graph-canvas"),
        );
    }
    b.call(json!({"op":"select","selector":"#graph-target","value":"goodrga01"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 1"));
    b.click("Zoom to selected player");
    b.click("Zoom out");
    b.click("Zoom out");
    assert_ranked_direct_labels(&mut b);
    b.click("Fit whole network");
    b.click("Show ego network only (1–2 hops)");
    while b.attr("data-tier") != "macro" {
        b.click("Zoom out");
    }
    for i in 0..8 {
        b.call(json!({"op":"click","selector":format!("#era-{i}")}));
    }
    assert_eq!(b.attr("data-chain-edges"), "1");
    let labels = b.attr("data-labels");
    assert!(labels.split(',').any(|id| id == "maravpe01"));
    assert!(labels.split(',').any(|id| id == "goodrga01"));
    assert_eq!(b.attr("data-players"), "5106");
    assert_eq!(b.attr("data-relationships"), "101395");
    screenshot(&mut b, "maravich-chain.png", None);
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence]"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    assert!(
        b.text("#drawer-content")
            .contains("27 + 73 − 82 = at least 18")
    );
    screenshot(&mut b, "maravich-count-drawer.png", None);
    b.click("Close evidence");
    b.click("Clear chain");
    b.click("Show ego network only (1–2 hops)");
    screenshot(&mut b, "maravich-ego.png", Some("#graph-canvas"));
    b.click("Reset filters");
    // The phone layout keeps the canvas and the picker.
    b.call(json!({"op":"viewport","width":390,"height":844}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    let canvas = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    assert!(canvas["width"].as_f64().unwrap() <= 390.0);
    assert!(b.text("#graph-payload").contains("Pete Maravich"));
    screenshot(&mut b, "all-player-network-phone.png", None);
    b.call(json!({"op":"motion","value":"reduce"}));
    b.call(json!({"op":"select","selector":"#graph-player","value":"maravpe01"}));
    b.click("Zoom out");
    b.click("Zoom out");
    assert_eq!(b.attr("data-tier"), "meso");
    assert_eq!(b.attr("data-label-cap"), "12");
    assert!(b.attr("data-ordinary-labels").parse::<usize>().unwrap() <= 12);
    for button in ["Era lanes layout", "Force-directed layout"] {
        b.click(button);
        b.call(json!({"op":"settled"}));
        b.click("Fit whole network");
        b.click("Zoom to selected player");
        click_visible_bundle(&mut b, &payload, "maravpe01");
    }
    b.click("Concentric rings layout");
    assert_eq!(b.attr("data-transition"), "settled");
    screenshot(&mut b, "maravich-phone-sectors.png", Some("#graph-canvas"));
    let center: Vec<f64> = serde_json::from_str(&b.attr("data-focus-point")).unwrap();
    let sectors: Vec<(f64, f64)> = serde_json::from_str(&b.attr("data-sector-geometry")).unwrap();
    assert_eq!(sectors.len(), 3);
    let radii: Vec<_> = sectors
        .iter()
        .map(|p| (p.0 - center[0]).hypot(p.1 - center[1]))
        .collect();
    assert!(
        radii.iter().copied().fold(0.0, f64::max)
            - radii.iter().copied().fold(f64::INFINITY, f64::min)
            < 0.1,
        "phone rings must project uniformly"
    );
    b.call(json!({"op":"select","selector":"#graph-target","value":"goodrga01"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    let labels: Vec<serde_json::Value> = serde_json::from_str(&b.attr("data-label-items")).unwrap();
    let badge = labels.iter().find(|l| l["id"] == "goodrga01").unwrap();
    let rect = &badge["bounds"];
    b.call(json!({"op":"focus","selector":"#graph-canvas"}));
    let canvas = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    let x = canvas["x"].as_f64().unwrap() + rect["left"].as_f64().unwrap() + 8.0;
    let y = canvas["y"].as_f64().unwrap() + rect["bottom"].as_f64().unwrap() + 8.0;
    b.call(json!({"op":"touch","type":"touchStart","points":[{"x":x,"y":y}]}));
    b.call(json!({"op":"touch","type":"touchEnd"}));
    b.call(json!({"op":"ready","selector":"#graph-selection[data-player=goodrga01]"}));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-player","name":"data-selected-id"})),
        "goodrga01"
    );
    // A six-hop real chain includes four nodes beyond the ordinary ego cutoff.
    b.call(json!({"op":"viewport","width":1200,"height":1000}));
    b.call(json!({"op":"goto","url":format!("{}/?from=russebi01&to=edwaran01",server.url)}));
    b.ready();
    let chain: canvas_view::GraphPayload = serde_json::from_str(&b.text("#graph-payload")).unwrap();
    assert_eq!(chain.path.len(), 7);
    b.click("Concentric rings layout");
    b.call(json!({"op":"settled"}));
    while b.attr("data-tier") != "macro" {
        b.click("Zoom out");
    }
    for i in 0..8 {
        b.call(json!({"op":"click","selector":format!("#era-{i}")}));
    }
    assert_eq!(b.attr("data-chain-edges"), "6");
    let labels = b.attr("data-labels");
    assert!(
        chain
            .path
            .iter()
            .all(|id| labels.split(',').any(|label| label == id))
    );
    screenshot(&mut b, "six-hop-macro-ego-chain.png", Some("#graph-canvas"));
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
    // Every diamond player is present in the picker, with no chain-specific UI.
    assert_eq!(b.attr("data-players"), "4");
    assert_eq!(b.attr("data-relationships"), "4");
    for name in ["Player Alpha", "Player Beta", "Player Gamma", "Player Goal"] {
        assert!(
            b.text("#graph-payload").contains(name),
            "missing diamond player {name}"
        );
    }
    assert_eq!(
        b.call(json!({"op":"count","selector":"#graph-players"})),
        0,
        "player list is removed"
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":".alternatives"})),
        0,
        "no alternatives list on the single page"
    );
    b.call(json!({"op":"select","selector":"#graph-player","value":"a"}));
    b.call(json!({"op":"select","selector":"#graph-target","value":"g"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 2"));
    let first = b.text("#graph-chain");
    b.click("Connect players");
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert_eq!(b.text("#graph-chain"), first);
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
async fn real_slice_renders_every_player_and_standalone_edge_provenance() {
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
        assert!(b.text("#graph-payload").contains(name), "missing {name}");
    }
    // Picker selection reports the player; evidence is a standalone page.
    b.call(json!({"op":"select","selector":"#graph-player","value":"acyqu01"}));
    assert!(b.text("#graph-selection").contains("Quincy Acy"));
    b.call(json!({"op":"goto","url":format!("{}/edge?from=acyqu01&to=bogutan01",server.url)}));
    let panel = b.text("main");
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
        "Real slice: the whole seven-player network renders on the single page, and standalone edge provenance stays honest."
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM explorer upgrade acceptance"]
async fn explorer_pair_filters_drawer_history_and_mobile_are_reversible() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    b.call(json!({"op":"select","selector":"#graph-target","value":"C"}));
    b.click("Concentric rings layout");
    b.call(json!({"op":"settled"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 1"));
    assert_eq!(b.attr("data-layout"), "concentric");
    assert!(
        b.text("#canvas-status")
            .contains("Default focal player: Player B")
    );
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#ego-toggle","name":"aria-pressed"})),
        "true"
    );
    b.click("Show ego network only (1–2 hops)");
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "6"
    );
    b.click("Show ego network only (1–2 hops)");
    b.call(json!({"op":"fill","selector":"#graph-player","value":"Player A"}));
    b.call(json!({"op":"ready","selector":"#graph-player-results [role=option]"}));
    let pan = b.attr("data-pan-x");
    b.call(json!({"op":"key","key":"ArrowDown"}));
    assert_eq!(b.attr("data-pan-x"), pan);
    b.call(json!({"op":"key","key":"Enter"}));
    assert_eq!(b.attr("data-layout"), "concentric");
    assert!(
        b.text("#graph-selection")
            .contains("2 teammate connections")
    );
    b.call(json!({"op":"select","selector":"#graph-target","value":"C"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 2"));
    assert!(b.text("#graph-chain").contains("Red"));
    assert!(b.text("#graph-chain").contains("Blue"));
    let selected =
        b.call(json!({"op":"attribute","selector":"#graph-player","name":"data-selected-id"}));
    b.call(json!({"op":"click","selector":"[data-center-player='B']"}));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-player","name":"data-selected-id"})),
        selected
    );
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "5"
    );
    b.call(json!({"op":"click","selector":"[data-franchise='Red']"}));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-matching"})),
        "2"
    );
    for i in 0..8 {
        b.call(json!({"op":"click","selector":format!("#era-{i}")}));
    }
    assert!(b.text("#filter-status").contains("No eras emphasized"));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "5"
    );
    assert!(b.text("#graph-chain").contains("degree 2"));
    screenshot(&mut b, "fixture-chain-filtered.png", None);
    let camera = (
        b.attr("data-pan-x"),
        b.attr("data-pan-y"),
        b.attr("data-zoom"),
    );
    let popup = b.call(
        json!({"op":"open-link-modified","selector":"#graph-chain a[data-evidence] >> nth=0"}),
    );
    assert!(popup.as_str().unwrap().ends_with("/edge?from=A&to=B"));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#edge-evidence-drawer[open]"})),
        0
    );
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence] >> nth=0"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    assert!(
        b.text("#drawer-content")
            .contains("Overlap: [day 2, day 4)")
    );
    assert!(
        b.call(json!({"op":"url"}))
            .as_str()
            .unwrap()
            .ends_with("/edge?from=A&to=B")
    );
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#drawer-content","name":"role"})),
        serde_json::Value::Null
    );
    assert_eq!(b.text("#drawer-status"), "Relationship evidence loaded.");
    screenshot(&mut b, "fixture-drawer.png", None);
    b.call(json!({"op":"key","key":"Escape"}));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#edge-evidence-drawer[open]"})),
        0
    );
    assert_eq!(b.call(json!({"op":"active-id"})), "A");
    assert_eq!(
        (
            b.attr("data-pan-x"),
            b.attr("data-pan-y"),
            b.attr("data-zoom")
        ),
        camera
    );
    b.call(json!({"op":"history","direction":"forward"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    b.click("Close evidence");
    b.click("Clear chain");
    assert_eq!(b.text("#graph-chain"), "");
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-target","name":"data-selected-id"})),
        ""
    );
    b.click("Reset filters");
    assert!(b.text("#filter-status").contains("6 matching"));
    b.call(json!({"op":"select","selector":"#graph-target","value":"A"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 0"));
    b.call(json!({"op":"select","selector":"#graph-target","value":"B"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 1"));
    b.call(json!({"op":"select","selector":"#graph-target","value":"D"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=disconnected]"}));
    assert!(
        b.text("#graph-chain")
            .contains("complete synthetic fixture")
    );
    b.click("Random pair");
    let a = b.call(json!({"op":"attribute","selector":"#graph-player","name":"data-selected-id"}));
    let target =
        b.call(json!({"op":"attribute","selector":"#graph-target","name":"data-selected-id"}));
    assert_ne!(a, target);
    b.call(json!({"op":"viewport","width":390,"height":844}));
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A&to=B",server.url)}));
    b.ready();
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#minimap-toggle","name":"aria-expanded"})),
        "false"
    );
    let size = b.call(json!({"op":"page-size"}));
    assert_eq!(size["width"], size["scroll"]);
    screenshot(&mut b, "fixture-mobile-toolbar.png", None);
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence]"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    let drawer = b.call(json!({"op":"box","selector":"#edge-evidence-drawer"}));
    assert!(drawer["width"].as_f64().unwrap() <= 390.0);
    screenshot(&mut b, "fixture-mobile-drawer.png", None);
    b.click("Close evidence");
    b.call(json!({"op":"motion","value":"reduce"}));
    b.click("Force-directed layout");
    assert_ne!(
        b.call(json!({"op":"attribute","selector":"#canvas-status","name":"data-transition"})),
        "running"
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM delayed and rejected HTTP boundaries"]
async fn explorer_ignores_old_search_chain_and_closed_drawer_responses_and_recovers() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A",server.url)}));
    b.ready();
    b.call(json!({"op":"intercept","pattern":"**/api/search?q=Player%20A*","delay":900}));
    b.call(json!({"op":"fill","selector":"#graph-player","value":"Player A"}));
    b.call(json!({"op":"fill","selector":"#graph-player","value":"Player B"}));
    b.call(json!({"op":"ready","selector":"#graph-player-results [data-player-id='B']"}));
    b.call(json!({"op":"network-idle"}));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#graph-player-results [data-player-id='A']"})),
        0
    );
    b.call(json!({"op":"key","key":"Escape"}));
    b.call(json!({"op":"unroute","pattern":"**/api/search?q=Player%20A*"}));
    b.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    b.call(json!({"op":"intercept","pattern":"**/api/connection?from=A&to=C","delay":900}));
    b.call(json!({"op":"select","selector":"#graph-target","value":"C"}));
    assert!(!b.text("#graph-chain").contains("degree"));
    b.call(json!({"op":"select","selector":"#graph-target","value":"B"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 1"));
    b.call(json!({"op":"network-idle"}));
    assert!(b.text("#graph-chain").contains("degree 1"));
    b.call(json!({"op":"intercept","pattern":"**/edge?from=A&to=B&view=drawer","delay":900}));
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence]"}));
    assert!(b.text("#drawer-content").contains("Loading"));
    b.click("Close evidence");
    b.call(json!({"op":"network-idle"}));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#edge-evidence-drawer[open]"})),
        0
    );
    b.call(json!({"op":"unroute","pattern":"**/edge?from=A&to=B&view=drawer"}));
    b.call(json!({"op":"intercept","pattern":"**/edge?from=A&to=B&view=drawer","fail":true}));
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence]"}));
    b.call(json!({"op":"ready","selector":"#retry-evidence"}));
    assert!(
        b.text("#drawer-content")
            .contains("Open standalone evidence")
    );
    b.call(json!({"op":"unroute","pattern":"**/edge?from=A&to=B&view=drawer"}));
    b.click("Retry evidence");
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    assert!(b.text("#drawer-content").contains("fixture:tenure:1"));
    b.click("Close evidence");
    b.call(json!({"op":"intercept","pattern":"**/api/connection*","fail":true}));
    b.click("Connect players");
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=error]"}));
    assert!(!b.text("#graph-chain").contains("degree"));
    b.call(json!({"op":"unroute","pattern":"**/api/connection*"}));
    b.click("Retry chain");
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert!(b.text("#graph-chain").contains("degree 1"));
    b.call(json!({"op":"fill","selector":"#graph-target","value":"unrelated banana"}));
    b.click("Connect players");
    assert!(!b.text("#graph-chain").contains("degree"));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-target","name":"data-selected-id"})),
        ""
    );
    let errors = b.call(json!({"op":"errors"}));
    for error in errors.as_array().unwrap() {
        assert!(
            error.as_str().unwrap().contains("net::ERR_FAILED"),
            "unexpected browser error: {error}"
        );
    }
    println!("Injected boundary failures: {errors}; retry recovered without runtime errors.");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual touch-capable Chromium/WASM pointer gestures"]
async fn touch_pan_pinch_cancel_and_minimap_drag_keep_selection_stable() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch","touch":true,"dpr":2}));
    b.call(json!({"op":"viewport","width":390,"height":844}));
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A",server.url)}));
    b.ready();
    b.call(json!({"op":"focus","selector":"#graph-canvas"}));
    let box_ = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    let x = box_["x"].as_f64().unwrap() + box_["width"].as_f64().unwrap() * 0.45;
    let y = box_["y"].as_f64().unwrap() + box_["height"].as_f64().unwrap() * 0.4;
    let pan = b.attr("data-pan-x");
    let zoom = b.attr("data-zoom");
    b.call(json!({"op":"touch","type":"touchStart","points":[{"id":1,"x":x-30.0,"y":y}]}));
    b.call(json!({"op":"touch","type":"touchMove","points":[{"id":1,"x":x,"y":y+20.0}]}));
    assert_ne!(b.attr("data-pan-x"), pan);
    b.call(json!({"op":"touch","type":"touchStart","points":[{"id":1,"x":x,"y":y+20.0},{"id":2,"x":x+40.0,"y":y+20.0}]}));
    let before_rotation = b.attr("data-zoom").parse::<f64>().unwrap();
    b.call(json!({"op":"touch","type":"touchMove","points":[{"id":1,"x":x+20.0,"y":y},{"id":2,"x":x+20.0,"y":y+40.0}]}));
    assert!(
        (b.attr("data-zoom").parse::<f64>().unwrap() - before_rotation).abs() < 0.001,
        "rotating a fixed CSS separation must not zoom"
    );
    b.call(json!({"op":"touch","type":"touchMove","points":[{"id":1,"x":x,"y":y+20.0},{"id":2,"x":x+40.0,"y":y+20.0}]}));
    b.call(json!({"op":"touch","type":"touchMove","points":[{"id":1,"x":x-30.0,"y":y+20.0},{"id":2,"x":x+70.0,"y":y+20.0}]}));
    b.call(json!({"op":"ready","selector":format!("#canvas-status:not([data-zoom=\"{zoom}\"])")}));
    assert_ne!(b.attr("data-zoom"), zoom);
    b.call(json!({"op":"touch","type":"touchEnd","points":[{"id":1,"x":x-30.0,"y":y+20.0}]}));
    b.call(json!({"op":"touch","type":"touchMove","points":[{"id":1,"x":x-15.0,"y":y+20.0}]}));
    b.call(json!({"op":"touch","type":"touchCancel"}));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-player","name":"data-selected-id"})),
        "A"
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":"#edge-evidence-drawer[open]"})),
        0
    );
    b.click("Network overview");
    let map = b.call(json!({"op":"box","selector":"#graph-minimap"}));
    let mx = map["x"].as_f64().unwrap() + map["width"].as_f64().unwrap() / 2.0;
    let my = map["y"].as_f64().unwrap() + map["height"].as_f64().unwrap() / 2.0;
    let before = b.attr("data-pan-x");
    b.call(json!({"op":"mouse","action":"move","x":mx,"y":my}));
    b.call(json!({"op":"mouse","action":"down"}));
    b.call(json!({"op":"mouse","action":"move","x":mx+20.0,"y":my+10.0,"steps":5}));
    b.call(json!({"op":"mouse","action":"up"}));
    assert_ne!(b.attr("data-pan-x"), before);
    assert_eq!(b.attr("data-players"), "6");
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

fn tiny_report(players: &str, counts: Option<&str>, witness: Option<&str>) -> axum::Router {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "seven-tiny-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("t3")).unwrap();
    std::fs::create_dir_all(root.join("t4")).unwrap();
    std::fs::write(
        root.join("t3/player-universe.csv"),
        format!("bbr_player_id,display_name,first_season,last_season,aba_only\n{players}"),
    )
    .unwrap();
    std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\n").unwrap();
    if counts.is_some() || witness.is_some() {
        std::fs::write(root.join("t4/tenures.csv"),"bbr_player_id,season,lg,canonical_franchise,membership_source,evidence_class,start_day,end_day,start_anchored,end_anchored\na,1999,NBA,RED,S2,inferred,,,0,0\nb,1999,NBA,RED,S2,inferred,,,0,0\na,1999,NBA,BLUE,S1,inferred,,,0,0\nb,1999,NBA,BLUE,S1,inferred,,,0,0\n").unwrap();
    }
    if let Some(rows) = counts {
        std::fs::write(
            root.join("t4/appearance-counts.csv"),
            format!("player,team,season,lg,games,team_games,player_record,team_record\n{rows}"),
        )
        .unwrap();
    }
    if let Some(rows) = witness {
        std::fs::write(
            root.join("t4/game-witnesses.csv"),
            format!("a,b,team,season,game_id,date,a_source,b_source\n{rows}"),
        )
        .unwrap();
    }
    app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM empty, isolate and proof-card fixtures"]
async fn small_snapshots_keep_empty_isolated_and_game_proof_states_usable() {
    let empty = Server::start(tiny_report("", None, None)).await;
    let single = Server::start(tiny_report("a,Player Alpha,1999,1999,N\n", None, None)).await;
    let proof=Server::start(tiny_report("a,Player Alpha,1999,1999,N\nb,Player Beta,1999,1999,N\n",Some("a,RED,1999,NBA,60,66,S2:player:1,S2:team:1\nb,RED,1999,NBA,50,66,S2:player:2,S2:team:1\n"),Some("a,b,BLUE,1999,0029800001,1998-11-01,S1:game:1:event:10:player1,S1:game:1:event:11:player1\n"))).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":empty.url}));
    b.ready();
    assert_eq!(b.attr("data-players"), "0");
    assert!(b.text("main").contains("No players are present"));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#random-pair:disabled"})),
        1
    );
    assert_eq!(
        b.call(json!({"op":"count","selector":"#layout-concentric:disabled"})),
        1
    );
    b.call(json!({"op":"goto","url":single.url}));
    b.ready();
    b.call(json!({"op":"select","selector":"#graph-player","value":"a"}));
    b.click("Concentric rings layout");
    b.call(json!({"op":"settled"}));
    b.click("Show ego network only (1–2 hops)");
    assert!(
        b.text("#graph-selection")
            .contains("0 teammate connections")
    );
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "1"
    );
    b.call(json!({"op":"goto","url":format!("{}/graph?from=a&to=b",proof.url)}));
    b.ready();
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence]"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    let text = b.text("#drawer-content");
    assert!(text.contains("60 + 50 − 66 = at least 44"));
    assert!(text.contains("1998-11-01"));
    assert!(!text.contains("Overlap: ["));
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#drawer-content meter >> nth=0","name":"max"})),
        "66"
    );
    screenshot(&mut b, "shortened-season-and-game-witness.png", None);
    b.click("Close evidence");
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM interrupted layout and chain-framing regression"]
async fn chain_framing_supersedes_animation_without_live_region_churn() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A&to=C",server.url)}));
    b.ready();
    b.call(json!({"op":"observe-status"}));
    b.click("Force-directed layout");
    b.call(json!({"op":"settled"}));
    assert!(
        b.call(json!({"op":"status-announcements"}))
            .as_array()
            .unwrap()
            .len()
            <= 1
    );
    b.call(json!({"op":"sequence","selectors":["#layout-era","#layout-concentric","#pair-form button[type=submit]"]}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    let camera = (
        b.attr("data-pan-x"),
        b.attr("data-pan-y"),
        b.attr("data-zoom"),
    );
    b.call(json!({"op":"network-idle"}));
    assert_eq!(
        (
            b.attr("data-pan-x"),
            b.attr("data-pan-y"),
            b.attr("data-zoom")
        ),
        camera
    );
    assert_eq!(b.attr("data-layout"), "concentric");
    assert_eq!(b.attr("data-transition"), "settled");
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A",server.url)}));
    b.ready();
    b.click("Force-directed layout");
    b.call(json!({"op":"settled"}));
    b.click("Concentric rings layout");
    let tap = b.call(json!({"op":"pointer-at-label","player":"B"}));
    assert_eq!(tap["transition"], "running");
    assert_eq!(
        tap["selected"], "B",
        "tap must use the displayed node position"
    );
    b.call(json!({"op":"settled"}));
    b.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    assert_eq!(
        b.attr("data-transition"),
        "running",
        "focal changes animate"
    );
    b.call(json!({"op":"settled"}));
    let final_labels = b.attr("data-label-items");
    b.click("Force-directed layout");
    b.call(json!({"op":"settled"}));
    b.click("Concentric rings layout");
    let canvas = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    let x = canvas["x"].as_f64().unwrap() + canvas["width"].as_f64().unwrap() - 5.0;
    let y = canvas["y"].as_f64().unwrap() + 5.0;
    for event in ["pointerdown", "pointerup"] {
        b.call(
            json!({"op":"pointer","event":event,"selector":"#graph-canvas","id":20,"x":x,"y":y}),
        );
    }
    assert_eq!(
        b.attr("data-label-items"),
        final_labels,
        "empty taps finish deterministic geometry"
    );
    b.call(json!({"op":"intercept","pattern":"**/api/connection?*","delay":1500}));
    b.call(json!({"op":"select","selector":"#graph-target","value":"C"}));
    b.call(json!({"op":"custom-event","name":"explorer-evidence","detail":"/edge?from=A&to=B"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    let canvas = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    let drawer = b.call(json!({"op":"box","selector":"#edge-evidence-drawer"}));
    let right = drawer["x"].as_f64().unwrap() - canvas["x"].as_f64().unwrap();
    let labels: Vec<serde_json::Value> = serde_json::from_str(&b.attr("data-label-items")).unwrap();
    for id in ["A", "B", "C"] {
        let label = labels.iter().find(|l| l["id"] == id).unwrap();
        assert!(
            label["anchor"][0].as_f64().unwrap() < right - 24.0,
            "chain node {id} must frame beside the open drawer"
        );
        assert!(
            label["bounds"]["right"].as_f64().unwrap() < right,
            "chain badge {id} must avoid the drawer"
        );
    }
    screenshot(&mut b, "chain-beside-open-drawer.png", None);
    b.click("Close evidence");
    b.call(json!({"op":"sequence","selectors":["#layout-force","[data-center-player='B']"]}));
    let labels: Vec<serde_json::Value> = serde_json::from_str(&b.attr("data-label-items")).unwrap();
    let b_label = labels.iter().find(|l| l["id"] == "B").unwrap();
    assert!(
        (b_label["anchor"][0].as_f64().unwrap() - canvas["width"].as_f64().unwrap() * 0.5).abs()
            < 0.1
    );
    assert!(
        (b_label["anchor"][1].as_f64().unwrap()
            - canvas["height"].as_f64().unwrap() * 265.0 / 600.0)
            .abs()
            < 0.1,
        "breadcrumb centers the settled player during interrupted motion"
    );
    b.click("Era lanes layout");
    b.call(json!({"op":"focus","selector":"#graph-canvas"}));
    b.call(json!({"op":"key","key":"Shift"}));
    b.call(json!({"op":"settled"}));
    assert_eq!(
        b.attr("data-transition"),
        "settled",
        "unrelated keys keep animation alive"
    );
    b.call(json!({"op":"sequence","selectors":["#layout-concentric","#ego-toggle"]}));
    let toggled_camera = (
        b.attr("data-pan-x"),
        b.attr("data-pan-y"),
        b.attr("data-zoom"),
    );
    b.call(json!({"op":"settled"}));
    assert_eq!(
        (
            b.attr("data-pan-x"),
            b.attr("data-pan-y"),
            b.attr("data-zoom")
        ),
        toggled_camera,
        "ego fit supersedes the pending camera transition"
    );
    b.click("Era lanes layout");
    b.call(json!({"op":"settled"}));
    for _ in 0..15 {
        b.click("Zoom in");
    }
    let navigation =
        b.call(json!({"op":"minimap-press","control":"#layout-force","fx":0.95,"fy":0.95}));
    assert_eq!(
        navigation["pressed"], navigation["stationary"],
        "stationary minimap drag must not jump after settlement"
    );
    assert_ne!(navigation["stationary"], navigation["moved"]);
    let world = &navigation["world"];
    let scale = navigation["pressed"][2]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    let pan_x = navigation["pressed"][0]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    let pan_y = navigation["pressed"][1]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    let aspect = 0.6 * canvas["width"].as_f64().unwrap() / canvas["height"].as_f64().unwrap();
    assert!(
        (pan_x - (500.0 - world[0].as_f64().unwrap() * scale)).abs() < 0.02,
        "minimap navigation interprets the displayed projection"
    );
    assert!((pan_y - (300.0 - world[1].as_f64().unwrap() * scale * aspect)).abs() < 0.02);
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM shared pair initial framing"]
async fn shared_pair_link_frames_the_same_chain_as_an_explicit_request() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A&to=C",server.url)}));
    b.ready();
    assert!(b.text("#graph-chain").contains("degree 2"));
    let initial = (
        b.attr("data-pan-x"),
        b.attr("data-pan-y"),
        b.attr("data-zoom"),
    );
    b.click("Connect players");
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    assert_eq!(
        (
            b.attr("data-pan-x"),
            b.attr("data-pan-y"),
            b.attr("data-zoom")
        ),
        initial
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM dismissed combobox keyboard regression"]
async fn dismissed_player_results_reopen_before_keyboard_selection() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    for id in ["graph-player", "graph-target"] {
        let selector = format!("#{id}");
        b.call(json!({"op":"fill","selector":selector,"value":"Player"}));
        b.call(json!({"op":"ready","selector":format!("#{id}-results [role=option] >> nth=0")}));
        b.call(json!({"op":"key","key":"ArrowDown"}));
        b.call(json!({"op":"key","key":"Escape"}));
        b.call(json!({"op":"key","key":"Enter"}));
        assert_eq!(
            b.call(json!({"op":"attribute","selector":selector,"name":"data-selected-id"})),
            ""
        );
        b.call(json!({"op":"key","key":"ArrowDown"}));
        assert_eq!(
            b.call(json!({"op":"attribute","selector":selector,"name":"aria-expanded"})),
            "true"
        );
        assert_eq!(
            b.call(json!({"op":"attribute","selector":format!("#{id}-results"),"name":"hidden"})),
            serde_json::Value::Null
        );
        b.call(json!({"op":"key","key":"Enter"}));
        assert_eq!(
            b.call(json!({"op":"input-value","selector":selector})),
            "Player A"
        );
        assert_eq!(
            b.text(&format!("#{id}-search-status")),
            "Selected: Player A."
        );
    }
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM shareable pair URL regression"]
async fn pair_url_tracks_selection_clear_and_evidence_history() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":format!("{}/graph?from=A&to=C",server.url)}));
    b.ready();
    b.click("Clear chain");
    assert_eq!(
        b.call(json!({"op":"url"})),
        format!("{}/graph?from=A", server.url)
    );
    b.call(json!({"op":"select","selector":"#graph-target","value":"B"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    let pair = format!("{}/graph?from=A&to=B", server.url);
    assert_eq!(b.call(json!({"op":"url"})), pair);
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence]"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    assert!(
        b.call(json!({"op":"url"}))
            .as_str()
            .unwrap()
            .ends_with("/edge?from=A&to=B")
    );
    b.call(json!({"op":"history","direction":"back"}));
    assert_eq!(b.call(json!({"op":"url"})), pair);
    b.call(json!({"op":"history","direction":"forward"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    b.click("Close evidence");
    b.call(json!({"op":"goto","url":pair}));
    b.ready();
    assert!(b.text("#graph-chain").contains("degree 1"));
    b.call(json!({"op":"fill","selector":"#graph-player","value":"not a player"}));
    assert_eq!(
        b.call(json!({"op":"url"})),
        format!("{}/graph?to=B", server.url)
    );
    b.call(json!({"op":"select","selector":"#graph-player","value":"C"}));
    assert_eq!(
        b.call(json!({"op":"url"})),
        format!("{}/graph?from=C&to=B", server.url)
    );
    b.click("Random pair");
    let a = b.call(json!({"op":"attribute","selector":"#graph-player","name":"data-selected-id"}));
    let target =
        b.call(json!({"op":"attribute","selector":"#graph-target","name":"data-selected-id"}));
    assert_eq!(
        b.call(json!({"op":"url"})),
        format!(
            "{}/graph?from={}&to={}",
            server.url,
            a.as_str().unwrap(),
            target.as_str().unwrap()
        )
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM programmatic picker reset regression"]
async fn programmatic_pair_changes_clear_stale_feedback_and_pending_search() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    b.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    b.call(json!({"op":"select","selector":"#graph-target","value":"B"}));
    b.click("Clear chain");
    assert_eq!(b.text("#graph-target-search-status"), "");
    b.call(json!({"op":"intercept","pattern":"**/api/search*","delay":900}));
    b.call(json!({"op":"fill","selector":"#graph-target","value":"Player C"}));
    b.click("Clear chain");
    b.call(json!({"op":"network-idle"}));
    assert_eq!(b.text("#graph-target-search-status"), "");
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-target","name":"aria-expanded"})),
        "false"
    );
    b.call(json!({"op":"fill","selector":"#graph-player","value":"Player C"}));
    b.click("Random pair");
    b.call(json!({"op":"network-idle"}));
    for id in ["graph-player", "graph-target"] {
        let name = b.call(json!({"op":"input-value","selector":format!("#{id}")}));
        assert_eq!(
            b.text(&format!("#{id}-search-status")),
            format!("Selected: {}.", name.as_str().unwrap())
        );
        assert_eq!(
            b.call(json!({"op":"attribute","selector":format!("#{id}"),"name":"aria-expanded"})),
            "false"
        );
        assert_eq!(
            b.call(json!({"op":"count","selector":format!("#{id}-results [role=option]")})),
            0
        );
    }
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM long combobox keyboard visibility"]
async fn picker_keyboard_navigation_keeps_active_result_inside_the_list() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"viewport","width":390,"height":844}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    for id in ["graph-player", "graph-target"] {
        b.call(json!({"op":"fill","selector":format!("#{id}"),"value":"Smith"}));
        b.call(json!({"op":"ready","selector":format!("#{id}-option-19")}));
        let input = b.call(json!({"op":"box","selector":format!("#{id}")}));
        for key in ["ArrowUp", "ArrowDown", "ArrowUp"] {
            b.call(json!({"op":"key","key":key}));
            let active = b.call(json!({"op":"attribute","selector":format!("#{id}"),"name":"aria-activedescendant"}));
            let option =
                b.call(json!({"op":"box","selector":format!("#{}", active.as_str().unwrap())}));
            let list = b.call(json!({"op":"box","selector":format!("#{id}-results")}));
            let top = option["y"].as_f64().unwrap();
            let bottom = top + option["height"].as_f64().unwrap();
            assert!(
                top >= 0.0 && bottom <= 844.0,
                "active option is outside phone viewport: {option}"
            );
            assert!(
                top >= list["y"].as_f64().unwrap(),
                "active option is above list: {option} / {list}"
            );
            assert!(
                bottom <= list["y"].as_f64().unwrap() + list["height"].as_f64().unwrap(),
                "active option is below list: {option} / {list}"
            );
        }
        assert_eq!(
            b.call(json!({"op":"box","selector":format!("#{id}")}))["y"],
            input["y"]
        );
        b.call(json!({"op":"key","key":"Enter"}));
        assert_eq!(
            b.call(json!({"op":"attribute","selector":format!("#{id}"),"name":"data-selected-id"})),
            "smithdr01"
        );
    }
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM combobox focus and delayed dismissal"]
async fn picker_focus_departure_dismisses_results_without_losing_pointer_selection() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    for id in ["graph-player", "graph-target"] {
        b.call(json!({"op":"fill","selector":format!("#{id}"),"value":"Player"}));
        b.call(json!({"op":"ready","selector":format!("#{id}-option-0")}));
        b.call(json!({"op":"key","key":"ArrowDown"}));
        b.call(json!({"op":"key","key":"Tab"}));
        assert_eq!(b.call(json!({"op":"active-id"})), format!("{id}-era"));
        assert_eq!(
            b.call(json!({"op":"attribute","selector":format!("#{id}"),"name":"aria-expanded"})),
            "false"
        );
        assert_eq!(
            b.call(
                json!({"op":"attribute","selector":format!("#{id}"),"name":"aria-activedescendant"})
            ),
            serde_json::Value::Null
        );
        b.call(json!({"op":"key","key":"Shift+Tab"}));
        b.call(json!({"op":"key","key":"ArrowDown"}));
        b.call(json!({"op":"click","selector":format!("#{id}-results [data-player-id='B']")}));
        assert_eq!(
            b.call(json!({"op":"attribute","selector":format!("#{id}"),"name":"data-selected-id"})),
            "B"
        );
        assert_eq!(b.call(json!({"op":"active-id"})), id);
        b.call(json!({"op":"intercept","pattern":"**/api/search*","delay":900}));
        b.call(json!({"op":"fill","selector":format!("#{id}"),"value":"Player A"}));
        b.call(json!({"op":"key","key":"Tab"}));
        b.call(json!({"op":"network-idle"}));
        assert_eq!(
            b.call(json!({"op":"attribute","selector":format!("#{id}"),"name":"aria-expanded"})),
            "false"
        );
        b.call(json!({"op":"unroute","pattern":"**/api/search*"}));
    }
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}
