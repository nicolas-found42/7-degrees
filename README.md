# 7 Degrees

Explore NBA/BAA teammates and their shortest chains. An undirected link requires evidence of simultaneous roster membership on the same canonical franchise. Playing for a franchise in different years, or sharing only an All-Star, national-team, summer-league or G League association, creates no link. Injured and inactive roster occupants remain eligible; shared game minutes are not required.

The current pinned build contains **5,106 evidenced player identities and 101,395 teammate edges**. Links are established by certified dated tenures, mathematically guaranteed shared game appearances, or identified participants of the same dated official team game. Appearance evidence proves the relationship while leaving full roster overlap dates and duration unknown; the UI and API preserve that distinction. **Historical roster coverage remains incomplete**, including 1946–1950 BAA: 41 players remain isolated in the available evidence. Missing edges may shorten historical chains or connect those players. See the [link-repair audit](docs/reports/link-repair/README.md), [tenure coverage report](docs/reports/t4-tenure-coverage.md), and [identity reconciliation](docs/reports/t3-reconciliation.md). Earlier final-verification reports describe the pre-repair snapshot.


## Start the complete app

From this checkout, with Rust/Cargo installed:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
./scripts/run-app.sh
```

The first two commands provision build tools if needed. The wrapper builds the Rust canvas as WASM, generates matching wasm-bindgen bindings, and starts the default `app-server` binary. Open [localhost:3000](http://127.0.0.1:3000). It imports the committed `docs/reports/t3/` and `t4/` snapshots, including appearance counts and dated game witnesses; no source download, credential, raw SQLite file or separately managed database service is needed to browse players and calculate paths.

```sh
NBA_PORT=3001 ./scripts/run-app.sh
NBA_REPORT_DIR=/path/to/reports ./scripts/run-app.sh
# Explicit synthetic fixture, useful for demonstrations/tests:
NBA_DATA_MODE=fixture ./scripts/run-app.sh
```

`NBA_REPORT_DIR` must contain compatible `t3/` and `t4/` reports. Custom snapshots may omit `t4/appearance-counts.csv` and `t4/game-witnesses.csv`; they then use only their certified dated tenures. WASM output defaults to ignored `target/canvas-web`; the wrapper honors `CARGO_TARGET_DIR`. Set `NBA_CANVAS_ASSET_DIR` to serve matching assets built elsewhere. `cargo run -p app-server` uses the same default report import; without built canvas assets its accessible paths, lists and expansion forms still work.

All application UI, interaction and graph logic is Rust. Axum renders accessible HTML and `canvas-view` compiles to WASM. Generated JavaScript only loads wasm-bindgen bindings. Browser test tooling uses a small Node/Playwright driver; no JavaScript graph/UI library implements the application.

## Explore

- Search at `/search?q=Quincy%20Acy`, open a profile at `/players/acyqu01`, or enter names from the home page. Results show stable IDs, era and franchise context. Same-name people such as the two Dee Browns remain separate choices.
- `/chain?from=maravpe01&to=abdulka01` returns a degree-two historical connection with several alternatives, including Gail Goodrich. Its evidence pages show the source counts and guaranteed minimum shared games without invented roster dates.
- `/chain?from=acyqu01&to=bogutan01` shows the real Quincy Acy → Andrew Bogut direct chain. `/edge?from=acyqu01&to=bogutan01` shows the Dallas evidence: 121 overlapping days `[2016-07-20, 2016-11-18)`, with each tenure record and source anchors.
- `/graph?from=acyqu01&to=bogutan01` focuses that chain. Select a player or link, pan/zoom, refocus, and expand direct or nearby teammates. `/graph?player=acyqu01&depth=2` explores a bounded neighborhood. Keyboard controls and player/path lists complement the canvas.
- When equal shortest paths exist, inspect alternatives and choose one. Pagination preserves the exact degree and selected path. A disconnected result distinguishes the present component facts from missing historical evidence.
- `/stats` reports the reachable unordered-pair histogram, components, finite diameter and unreachable pairs. Isolates are components; self-pairs and unreachable pairs do not enter the histogram.
- With semantic features available, `/resolve?q=Shaq` suggests a retrieved identity, and `/query?q=Connect%20Quincy%20Acy%20to%20Andrew%20Bogut` interprets a connection request. Profile, neighbors, comparison and unsupported requests have distinct visible outcomes. Uncertain mentions/arguments ask for clarification before a path.
- Natural-language team and numeric season-ending-year filters change the requested view, not the stored teammate graph. Connection filters constrain the eligible chain view. Detail links explicitly identify when they open the **unfiltered evidence graph**; removing the query filter restores that view.
- `/rank?from=...&to=...&interest=...&cursor=v1%3A0` can suggest an ordering within a page of equal-length shortest chains. Its scores are semantic suggestions; exact players, links and minimum degree remain code-computed.

`overlap_days` is a number for certified dated overlap and `null` when a relationship is proved without its full duration. Such links expose `minimum_shared_games`; detailed edge evidence supplies the appearance-count calculation or original dated game witness.

Open [`/graph?all=true`](http://127.0.0.1:3000/graph?all=true) for all 5,106 players and 101,395 evidenced teammate connections in one field. Search for a player, pan or zoom, and inspect their connection evidence. From a chain page, “See all players and connections in one field” retains the selected path as a blue highlight within the full graph. Fit-all, fit-chain and player-focus controls change the viewport without truncating the graph; isolated players remain present. Player positions follow debut season, and labels appear as you zoom. See the [research note](docs/reports/chain-visualizer-research.md) and [verification receipts](docs/reports/chain-visualizer/README.md).

Machine-readable routes include `/api/players`, `/api/search?q=...`, `/api/resolve?q=...`, `/api/query?q=...`, `/api/rank`, `/api/graph`, `/api/edges`, `/api/edges/{player}`, `/api/neighborhood?player=acyqu01&depth=1&limit=60`, `/api/connection?from=acyqu01&to=bogutan01`, `/api/coverage`, `/api/coverage/{player}`, `/api/stats`, and `/api/semantic-status`. `/api/fixture` is a compatibility alias for the current graph summary. `/api/graph`, `/api/fixture` and `/api/edges` return edge pages: `limit` (1–5000, default 1000) and `offset` select a window, and the response carries `total_edges` and `next_offset` (null on the last page). Neighborhood depth is 1 or 2, with at most 200 nodes and an explicit truncation warning.

`/api/paths?from=...&to=...&limit=100&offset=0` returns equal shortest alternatives in deterministic ID order, exact decimal `total_exact`, and resumable `next_cursor`. Pass that cursor unchanged as `cursor=...`; maximum page size is 500. Numeric offsets remain compatible.

## Semantic availability and measured limits

The server reads an existing `OPENROUTER_API_KEY` or `TYPESAFE_API_KEY` from its environment. When a key is already present in the launching shell, no additional setup is needed. The Rust client calls `https://openrouter.ai/api` with `typesafe/jev-1.13`; see the [verified client configuration](docs/reports/t11-jev-client.md). Credentials stay server-side. Browser canary checks and an explicit log-capture test cover the exercised routes and routine provider logs.

With no key or a provider failure, pages report semantic features unavailable and deterministic browsing, distances, paths, evidence and statistics remain usable. The client has one total ten-second deadline and at most two transport attempts. Semantic output cannot create players/edges or override graph calculations.

The [44-case labeled experiment](docs/evaluation/README.md) separates calibration from held-out cases and measures actual wording/order variants. Held-out baseline identity resolution matched **11/11** labels; routing matched **4/8**, and ranking **0/4**. Ranking abstained three times and gave one wrong preference for a hostile request; its degree-two graph facts remained unchanged. These small stratified sets do not establish broad accuracy. Frozen thresholds come from calibration, and future tuning needs a new untouched holdout. The 132 final outcomes used 102 provider calls, 129,531 input/29,352 output tokens and $0.005440302 reported cost. Initial aborted-run spend is unknown and excluded; retained pilot/supplement chronology and failures are documented.

## Verify and rebuild data

The reproducible check runner is `python3 scripts/check.py --profile full`. GitHub Actions runs the portable and browser profiles; raw-source verification is available separately. See [verification and receipts](docs/agents/verification.md) and [current source coverage navigation](docs/agents/source-coverage.md). The commands below remain useful for individual pipeline operations.

```sh
./scripts/build-canvas.sh
cargo test --workspace
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p canvas-view --target wasm32-unknown-unknown -- -D warnings
cargo test -p app-server --test browser_e2e --test canvas_browser -- --ignored --nocapture
RUSTFLAGS='--cfg test_capture' cargo test -p app-server --test log_capture -- --nocapture
cargo run -p app-server --bin semantic-eval -- --validate
cargo run -p app-server --bin semantic-eval -- --replay
python3 scripts/check-semantic-evaluation.py
python3 scripts/check-historical-audit.py
python3 scripts/check-game-witnesses.py
cargo run --release -p app-server --bin link-audit
```

The recorded run passed 104 ordinary Rust tests (7 intentionally ignored), 92 Python tests, three explicit browser journeys, one canvas browser smoke and one log-capture test. The ordinary `log_capture` target has zero tests unless `test_capture` is enabled. The browser suite discovers installed Node/Playwright/Chromium; `BROWSER_NODE`, `PLAYWRIGHT_PACKAGE_PATH` and `BROWSER_EXECUTABLE` override their locations. See [browser evidence and screenshots](docs/reports/t16-browser-e2e.md) and the [ordered final logs](docs/reports/final-verification/README.md). Semantic validation/replay uses retained results without network calls; paid `--live` runs automatically archive artifacts before overwriting them.

Pinned archives, licensing, local cached rebuild commands and deliberate future-version refresh are documented in the [source manifest](docs/data/source-manifest.md). The [compact historical audit](docs/reports/historical-audit/README.md) retains 6,246 immutable original cases, six 50-record era strata, all 798 BAA/defunct cases and all 1,548 complex groups. It records actual source-reading outcomes separately from model signals and deterministic corrected-output comparisons.
