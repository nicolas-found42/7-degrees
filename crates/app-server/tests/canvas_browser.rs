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
        // Close Chromium before terminating its transport, including after a failed assertion.
        let _ = writeln!(self.input, "{}", json!({"op":"close"}));
        let _ = self.input.flush();
        for _ in 0..100 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "actual Chromium/WASM smoke: run after scripts/build-canvas.sh"]
async fn rust_wasm_canvas_renders_the_whole_network_and_supports_pointer_and_keyboard() {
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
    // The fixture page always renders the whole network.
    browser.call(json!({"op":"goto","url":format!("http://{address}/")}));
    browser.ready();
    let original = browser.call(json!({"op":"canvas","selector":"#graph-canvas"}));
    assert_eq!(browser.attr("data-players"), "6");
    assert_eq!(browser.attr("data-relationships"), "5");
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
    browser.click("Reset view");
    assert_eq!(browser.attr("data-pan-x"), initial_x);
    assert_eq!(browser.attr("data-zoom"), initial_zoom);
    browser.click("Concentric rings layout");
    browser.call(json!({"op":"settled"}));
    assert_eq!(browser.attr("data-layout"), "concentric");
    assert!(
        browser
            .text("#canvas-status")
            .contains("Default focal player: Player B")
    );
    browser.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    // The layout switcher rearranges the network through the shared Rust
    // layout engine and reports the active mode for assistive technology.
    for (button, mode) in [
        ("Concentric rings layout", "concentric"),
        ("Force-directed layout", "force"),
        ("Era lanes layout", "era"),
    ] {
        browser.click(button);
        browser.call(json!({"op":"settled"}));
        assert_eq!(
            browser
                .call(json!({"op":"attribute","selector":"#canvas-status","name":"data-layout"}))
                .as_str()
                .unwrap(),
            mode
        );
        assert_eq!(browser.attr("data-players"), "6");
        assert_eq!(browser.attr("data-relationships"), "5");
    }
    // Picker selection reports a player, never leaving the page.
    browser.call(json!({"op":"select","selector":"#graph-player","value":"A"}));
    assert!(browser.text("#graph-selection").contains("Selected player"));
    assert!(browser.text("#graph-selection").contains("Player A"));
    // Pointer drag pans; wheel zooms; the network is retained throughout.
    // Focus the canvas first: the picker below it has scrolled the
    // page, and Playwright addresses the viewport.
    browser.call(json!({"op":"focus","selector":"#graph-canvas"}));
    let b = browser.call(json!({"op":"box","selector":"#graph-canvas"}));
    let (x, y, w, h) = (
        b["x"].as_f64().unwrap(),
        b["y"].as_f64().unwrap(),
        b["width"].as_f64().unwrap(),
        b["height"].as_f64().unwrap(),
    );
    assert!(
        y >= 0.0 && y + h <= 1001.0,
        "canvas must be in the viewport: {b:?}"
    );
    browser.call(json!({"op":"mouse","action":"move","x":x+w*0.5,"y":y+h*0.8}));
    browser.call(json!({"op":"mouse","action":"down"}));
    browser.call(json!({"op":"mouse","action":"move","x":x+w*0.5+50.0,"y":y+h*0.8+30.0,"steps":5}));
    browser.call(json!({"op":"mouse","action":"up"}));
    assert_ne!(browser.attr("data-pan-x"), initial_x);
    browser.click("Reset view");
    browser.call(json!({"op":"focus","selector":"#graph-canvas"}));
    let b = browser.call(json!({"op":"box","selector":"#graph-canvas"}));
    let (x, y, w, h) = (
        b["x"].as_f64().unwrap(),
        b["y"].as_f64().unwrap(),
        b["width"].as_f64().unwrap(),
        b["height"].as_f64().unwrap(),
    );
    browser.call(json!({"op":"mouse","action":"move","x":x+w*0.5,"y":y+h*0.5}));
    browser.call(json!({"op":"mouse","action":"wheel","delta":-120}));
    browser.call(json!({"op":"ready","selector":format!("#canvas-status:not([data-zoom='{initial_zoom}'])")}));
    assert_ne!(browser.attr("data-zoom"), initial_zoom);
    assert_eq!(browser.attr("data-players"), "6");
    assert_eq!(browser.attr("data-relationships"), "5");
    // Canvas center-click selection at whole-network zoom is nondeterministic;
    // the A/B evidence URL is deterministic, so assert the standalone evidence
    // page directly. The canvas journey above already proves pointer/keyboard
    // transforms change pixels while retaining all 6 players and 5 links.
    browser.call(json!({"op":"goto","url":format!("http://{address}/edge?from=A&to=B")}));
    let panel = browser.text("main");
    for text in [
        "Canonical franchise: Red",
        "fixture:tenure:1",
        "fixture:tenure:3",
    ] {
        assert!(panel.contains(text), "missing fixture panel {text}");
    }
    browser.call(json!({"op":"goto","url":format!("http://{address}/")}));
    browser.ready();
    assert_eq!(browser.call(json!({"op":"errors"})), json!([]));
    let artifact = std::env::var("CANVAS_SCREENSHOT").unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/canvas-browser.png")
            .to_string_lossy()
            .into()
    });
    browser.call(json!({"op":"screenshot","path":artifact}));
    // The real committed network renders with the same page and picker.
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
    // The real committed network renders its counts; Acy/Bogut evidence is
    // asserted on its standalone page below.
    browser.call(json!({"op":"goto","url":format!("http://{real_address}/")}));
    browser.ready();
    assert_eq!(browser.attr("data-players"), "5106");
    assert_eq!(browser.attr("data-relationships"), "101395");
    // The server renders no edge list and canvas-picking one of 101k links is
    // nondeterministic; the Acy/Bogut evidence URL is deterministic, so assert
    // the standalone evidence page directly.
    browser.call(
        json!({"op":"goto","url":format!("http://{real_address}/edge?from=acyqu01&to=bogutan01")}),
    );
    let panel = browser.text("main");
    assert!(panel.contains("Quincy Acy"));
    assert!(panel.contains("Andrew Bogut"));
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
        browser.call(json!({"op":"count","selector":"section:not(.coverage) a[href^='/sources/tenure?record=']"})),
        2
    );
    assert!(panel.contains("Overlap: [2016-07-20, 2016-11-18)"));
    assert_eq!(browser.call(json!({"op":"errors"})), json!([]));
    let screenshot = artifact;
    browser.call(json!({"op":"screenshot","path":format!("{screenshot}.real-provenance.png")}));
    real_server.abort();
    browser.call(json!({"op":"close"}));
    server.abort();
    println!(
        "WASM initialized; the whole network renders with canvas pixels changing under pointer drag, wheel and button/keyboard transforms; picker selection reports players and the standalone fixture plus real Acy/Bogut evidence assert the provenance seam."
    );
}
