//! User-visible navigation contract for the desktop explorer.
#[path = "support/browser.rs"]
mod browser;
use browser::{Browser, Server};
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM; build canvas first"]
async fn search_scope_and_selection_preserve_the_map() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"motion","value":"reduce"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    assert_eq!(b.attr("data-scope"), "full");
    b.call(json!({"op":"select","selector":"#graph-player","value":"B"}));
    assert!(b.text("#player-summary").contains("Player B"));
    assert_eq!(b.attr("data-scope"), "full");
    let overview = (
        b.attr("data-pan-x"),
        b.attr("data-pan-y"),
        b.attr("data-zoom"),
    );
    let layout = b.attr("data-layout-signature");
    b.click("Selected");
    assert_eq!(b.attr("data-scope"), "selected");
    b.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    assert_eq!(b.attr("data-scope"), "selected");
    assert_eq!(b.attr("data-layout-signature"), layout);
    b.click("Full network");
    assert_eq!(
        (
            b.attr("data-pan-x"),
            b.attr("data-pan-y"),
            b.attr("data-zoom")
        ),
        overview
    );
    // Picking a displayed player selects without a camera jump or layout change.
    let click = b.call(json!({"op":"pointer-at-label","player":"*"}));
    assert_eq!(b.attr("data-pan-x"), click["camera"][0]);
    assert_eq!(b.attr("data-pan-y"), click["camera"][1]);
    assert_eq!(b.attr("data-layout-signature"), layout);
    b.click("Find connection");
    b.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    b.call(json!({"op":"select","selector":"#graph-target","value":"C"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    b.call(json!({"op":"click","selector":"#graph-chain a[data-evidence] >> nth=0"}));
    b.call(json!({"op":"ready","selector":"#edge-evidence-drawer[open] .proof-card"}));
    b.click("Back to player");
    assert!(b.text("#player-summary").contains("Player A"));
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM over committed production data"]
async fn production_drag_moves_player_without_panning_and_settles() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let server = Server::start(
        app_server::app_with_report_data(root, app_server::JevHandle::unconfigured()).unwrap(),
    )
    .await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    b.click("Dismiss map guidance");
    b.call(json!({"op":"select","selector":"#graph-player","value":"jamesle01"}));
    b.call(json!({"op":"settled"}));
    let before: Vec<f64> = serde_json::from_str(&b.attr("data-focus-point")).unwrap();
    let camera = (b.attr("data-pan-x"), b.attr("data-pan-y"));
    let bounds = b.call(json!({"op":"box","selector":"#graph-canvas"}));
    let x = bounds["x"].as_f64().unwrap() + before[0];
    let y = bounds["y"].as_f64().unwrap() + before[1];
    b.call(json!({"op":"mouse","action":"move","x":x,"y":y}));
    b.call(json!({"op":"mouse","action":"down"}));
    b.call(json!({"op":"mouse","action":"move","x":x+70.0,"y":y+35.0}));
    assert_eq!(b.attr("data-physics"), "active");
    assert_eq!((b.attr("data-pan-x"), b.attr("data-pan-y")), camera);
    let moved: Vec<f64> = serde_json::from_str(&b.attr("data-focus-point")).unwrap();
    assert!(moved[0] > before[0] + 40.0);
    println!(
        "Production graph drag draw: {} ms; {} players; {} relationships",
        b.attr("data-draw-ms"),
        b.attr("data-players"),
        b.attr("data-relationships")
    );
    b.call(json!({"op":"mouse","action":"up"}));
    b.call(json!({"op":"settled"}));
    assert_eq!(b.attr("data-physics"), "settled");
    let after: Vec<f64> = serde_json::from_str(&b.attr("data-focus-point")).unwrap();
    assert!((after[0] - before[0]).abs() < 0.1 && (after[1] - before[1]).abs() < 0.1);
    b.call(json!({"op":"screenshot","path":concat!(env!("CARGO_MANIFEST_DIR"),"/../../target/browser-e2e/redesigned-player.png")}));
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM inspector selection and history"]
async fn relationship_scope_and_player_inspector_follow_selection_and_history() {
    let server = Server::start(app_server::app_with_fixture_data()).await;
    let mut b = Browser::start();
    b.call(json!({"op":"launch"}));
    b.call(json!({"op":"motion","value":"reduce"}));
    b.call(json!({"op":"goto","url":server.url}));
    b.ready();
    // The same public evidence event emitted by pointer hits, before selecting a player.
    b.call(json!({"op":"custom-event","name":"explorer-evidence","detail":"/edge?from=A&to=B"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    b.click("Selected");
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "2"
    );
    b.call(json!({"op":"history","direction":"back"}));
    assert_eq!(b.attr("data-scope"), "full");
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "6"
    );
    b.call(json!({"op":"history","direction":"forward"}));
    b.call(json!({"op":"ready","selector":"#drawer-content .proof-card >> nth=0"}));
    b.click("Selected");
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#filter-status","name":"data-visible"})),
        "2"
    );
    b.call(json!({"op":"select","selector":"#graph-player","value":"C"}));
    assert_eq!(
        b.call(json!({"op":"count","selector":"#edge-evidence-drawer[open]"})),
        0
    );
    assert!(b.text("#player-summary").contains("Player C"));
    b.click("Find connection");
    b.call(json!({"op":"select","selector":"#graph-target","value":"A"}));
    b.call(json!({"op":"ready","selector":"#graph-chain[data-state=connected]"}));
    let clicked = b.call(json!({"op":"pointer-at-label","player":"B"}));
    b.call(json!({"op":"network-idle"}));
    assert_eq!(b.attr("data-pan-x"), clicked["camera"][0]);
    assert_eq!(b.attr("data-pan-y"), clicked["camera"][1]);
    assert_eq!(
        b.call(json!({"op":"attribute","selector":"#graph-target","name":"data-selected-id"})),
        ""
    );
    assert_eq!(b.call(json!({"op":"errors"})), json!([]));
}
