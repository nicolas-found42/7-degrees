//! Actual browser seam: Node is only a Playwright transport; assertions are Rust.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};
struct Browser {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Browser {
    fn start() -> Self {
        let home = std::env::var("HOME").unwrap();
        let node = std::env::var("BROWSER_NODE").unwrap_or_else(|_| {
            format!("{home}/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node")
        });
        let driver =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/browser_driver.cjs");
        let mut child = Command::new(node)
            .arg(driver)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("launch installed Node/Playwright");
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }
    fn call(&mut self, command: Value) -> Value {
        writeln!(self.input, "{command}").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).expect("browser RPC response");
        assert_eq!(result["ok"], true, "{command}: {result}");
        result["result"].clone()
    }
    fn attr(&mut self, name: &str) -> String {
        self.call(json!({"op":"attribute","selector":"#canvas-status","name":name}))
            .as_str()
            .unwrap()
            .into()
    }
    fn click(&mut self, name: &str) {
        self.call(json!({"op":"click","role":"button","name":name}));
    }
    fn text(&mut self, selector: &str) -> String {
        self.call(json!({"op":"text","selector":selector}))
            .as_str()
            .unwrap()
            .into()
    }
    fn ready(&mut self) {
        self.call(json!({"op":"ready","selector":"#canvas-status[data-ready=true]"}));
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM smoke: run after scripts/build-canvas.sh"]
async fn rust_wasm_canvas_supports_pointer_keyboard_expansion_and_retains_chain_facts() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let assets = std::env::var_os("NBA_CANVAS_ASSET_DIR")
        .map(std::path::PathBuf::from)
        .map(app_server::graph_view::CanvasAssets::Directory)
        .unwrap_or_default();
    let app = app_server::app_with_jev_and_canvas_assets(
        app_server::JevHandle::unconfigured(),
        assets.clone(),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut browser = Browser::start();
    println!("Chromium {}", browser.call(json!({"op":"launch"})));
    browser.call(json!({"op":"goto","url":format!("http://{address}/graph?from=A&to=B&cursor=v1:00&limit=1&selected=0")}));
    browser.ready();
    let original = browser.call(json!({"op":"canvas","selector":"#graph-canvas"}));
    assert!(
        browser
            .text("#selected-chain")
            .contains("Degree of separation: 1")
    );
    assert!(
        browser
            .text("#selected-chain")
            .contains("Teammate links in this chain: 1")
    );
    let initial_zoom = browser.attr("data-zoom");
    let initial_x = browser.attr("data-pan-x");
    let initial_y = browser.attr("data-pan-y");
    browser.click("Zoom in");
    assert_ne!(browser.attr("data-zoom"), initial_zoom);
    assert_ne!(
        browser.call(json!({"op":"canvas","selector":"#graph-canvas"})),
        original
    );
    browser.click("Zoom out");
    assert_eq!(browser.attr("data-zoom"), initial_zoom);
    browser.click("Pan left");
    assert_ne!(browser.attr("data-pan-x"), initial_x);
    browser.click("Pan right");
    assert_eq!(browser.attr("data-pan-x"), initial_x);
    browser.click("Pan up");
    assert_ne!(browser.attr("data-pan-y"), initial_y);
    browser.click("Pan down");
    assert_eq!(browser.attr("data-pan-y"), initial_y);
    browser.call(json!({"op":"focus","selector":"#graph-canvas"}));
    browser.call(json!({"op":"key","key":"ArrowRight"}));
    assert_ne!(browser.attr("data-pan-x"), initial_x);
    browser.click("Refocus selected chain");
    assert_eq!(browser.attr("data-pan-x"), initial_x);
    assert_eq!(browser.attr("data-zoom"), initial_zoom);
    let b = browser.call(json!({"op":"box","selector":"#graph-canvas"}));
    let (x, y, w, h) = (
        b["x"].as_f64().unwrap(),
        b["y"].as_f64().unwrap(),
        b["width"].as_f64().unwrap(),
        b["height"].as_f64().unwrap(),
    );
    // The two-node chain is centered; select its right-hand endpoint by actual pointer.
    browser.call(json!({"op":"mouse","action":"click","x":x+w*0.636,"y":y+h*0.5}));
    assert!(browser.text("#graph-selection").contains("Player B"));
    assert!(
        browser
            .text("#graph-selection")
            .contains("Synthetic fixture days 2–13")
    );
    browser.call(json!({"op":"mouse","action":"move","x":x+w*0.5,"y":y+h*0.8}));
    browser.call(json!({"op":"mouse","action":"down"}));
    browser.call(json!({"op":"mouse","action":"move","x":x+w*0.5+50.0,"y":y+h*0.8+30.0,"steps":5}));
    browser.call(json!({"op":"mouse","action":"up"}));
    assert_ne!(browser.attr("data-pan-x"), initial_x);
    browser.click("Refocus selected chain");
    browser.call(json!({"op":"mouse","action":"move","x":x+w*0.5,"y":y+h*0.5}));
    browser.call(json!({"op":"mouse","action":"wheel","delta":-120}));
    // A following RPC allows Chromium to dispatch the wheel before readback.
    browser.call(json!({"op":"ready","selector":format!("#canvas-status:not([data-zoom='{initial_zoom}'])")}));
    assert_ne!(browser.attr("data-zoom"), initial_zoom);
    browser.click("Refocus selected chain");
    browser.call(json!({"op":"event-start","name":"teammate-edge-selected"}));
    browser.call(json!({"op":"mouse","action":"click","x":x+w*0.5,"y":y+h*0.5}));
    assert!(
        browser
            .text("#selected-edge")
            .contains("Player A ↔ Player B")
    );
    let events = browser.call(json!({"op":"events"}));
    let edge: Value = serde_json::from_str(events[0].as_str().unwrap()).unwrap();
    assert_eq!(edge["from"], "A");
    assert_eq!(edge["to"], "B");
    assert_eq!(edge["team"], "Red");
    browser.call(json!({"op":"ready","frame":"#edge-provenance-frame","selector":"h1"}));
    let pointer_panel =
        browser.call(json!({"op":"text","frame":"#edge-provenance-frame","selector":"main"}));
    assert!(
        pointer_panel
            .as_str()
            .unwrap()
            .contains("Overlap: [day 2, day 4)")
    );
    browser.click("Expand direct connections");
    browser.ready();
    assert!(browser.text("#graph-players").contains("Player C2"));
    assert!(!browser.text("#graph-players").contains("Player E"));
    assert!(
        browser
            .text("#selected-chain")
            .contains("Degree of separation: 1")
    );
    assert_eq!(
        browser.call(json!({"op":"attribute","selector":"input[name=cursor]","name":"value"})),
        "v1:00"
    );
    browser.click("Expand nearby connections");
    browser.ready();
    assert!(browser.text("#graph-players").contains("Player E"));
    assert!(!browser.text("#graph-players").contains("Player D"));
    browser.click("Player E");
    assert!(browser.text("#graph-selection").contains("Player E"));
    browser.click("Player A ↔ Player B");
    assert_eq!(
        browser.call(json!({"op":"attribute","selector":"#selected-edge","name":"data-from"})),
        "A"
    );
    assert!(
        browser
            .text("#selected-edge")
            .contains("Red, 2 overlap day(s)")
    );
    assert!(
        browser
            .text("#selected-chain")
            .contains("Degree of separation: 1")
    );
    browser.call(json!({"op":"ready","frame":"#edge-provenance-frame","selector":"h1"}));
    let panel = browser
        .call(json!({"op":"text","frame":"#edge-provenance-frame","selector":"main"}))
        .as_str()
        .unwrap()
        .to_string();
    for text in [
        "Canonical franchise: Red",
        "Overlap: [day 2, day 4)",
        "fixture:tenure:1",
        "fixture:tenure:3",
    ] {
        assert!(panel.contains(text), "missing fixture panel {text}");
    }
    assert!(
        browser
            .text("#selected-chain")
            .contains("Degree of separation: 1")
    );
    assert_eq!(
        browser.call(json!({"op":"attribute","selector":"input[name=cursor]","name":"value"})),
        "v1:00"
    );
    assert_eq!(browser.call(json!({"op":"errors"})), json!([]));
    let artifact = std::env::var("CANVAS_SCREENSHOT").unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/canvas-browser.png")
            .to_string_lossy()
            .into()
    });
    browser.click("Refocus selected chain");
    assert_eq!(browser.attr("data-pan-x"), initial_x);
    assert_eq!(browser.attr("data-pan-y"), initial_y);
    browser.call(json!({"op":"screenshot","path":artifact}));
    let real_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let real_address = real_listener.local_addr().unwrap();
    let reports = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reports");
    let real_app = app_server::app_with_report_data_and_canvas_assets(
        reports,
        app_server::JevHandle::unconfigured(),
        assets,
    )
    .unwrap();
    let real_server =
        tokio::spawn(async move { axum::serve(real_listener, real_app).await.unwrap() });
    browser.call(json!({"op":"goto","url":format!("http://{real_address}/graph?from=acyqu01&to=bogutan01&cursor=v1:00&limit=1&selected=0")}));
    browser.ready();
    browser.click("Quincy Acy ↔ Andrew Bogut");
    browser.call(json!({"op":"ready","frame":"#edge-provenance-frame","selector":"h1"}));
    let panel = browser
        .call(json!({"op":"text","frame":"#edge-provenance-frame","selector":"main"}))
        .as_str()
        .unwrap()
        .to_string();
    for text in [
        "Canonical franchise: MAVERICKS",
        "2016-07-20",
        "2016-11-18",
        "directly-evidenced",
        "transaction anchored",
        "S1 v238",
        "S2 v56",
        "Source evidence is incomplete",
    ] {
        assert!(panel.contains(text), "missing real panel {text}");
    }
    assert!(panel.matches("Source record t4/tenures.csv:").count() >= 2);
    assert_eq!(
        browser.call(json!({"op":"count","frame":"#edge-provenance-frame","selector":"section:not(.coverage) a[href^='/sources/tenure?record=']"})),
        2
    );
    let standalone = browser.call(json!({"op":"open-link-text","selector":"#selected-chain a"}));
    assert!(
        standalone
            .as_str()
            .unwrap()
            .contains("Canonical franchise: MAVERICKS")
    );
    assert!(
        standalone
            .as_str()
            .unwrap()
            .contains("Overlap: [2016-07-20, 2016-11-18)")
    );

    assert!(
        browser
            .text("#selected-chain")
            .contains("Degree of separation: 1")
    );
    assert_eq!(
        browser.call(json!({"op":"attribute","selector":"input[name=cursor]","name":"value"})),
        "v1:00"
    );
    assert_eq!(browser.call(json!({"op":"errors"})), json!([]));
    let screenshot =
        std::env::var("CANVAS_SCREENSHOT").unwrap_or_else(|_| "target/canvas-browser.png".into());
    // A locator screenshot scrolls this panel into view and waits for stable paint.
    // Full-page capture alone can leave an offscreen cross-origin frame unpainted.
    browser.call(json!({"op":"screenshot","selector":"#edge-provenance-panel","path":format!("{screenshot}.real-panel.png")}));
    browser.call(json!({"op":"screenshot","path":format!("{screenshot}.real-provenance.png")}));
    real_server.abort();
    browser.call(json!({"op":"close"}));
    server.abort();
    println!(
        "WASM initialized; canvas pixels changed; pointer selection/drag/wheel, button and keyboard transforms, direct/nearby expansion, preserved cursor/degree and edge hook passed. Fixture and real Acy/Bogut selected-edge panels loaded dates, both record references, source anchors and coverage."
    );
}
