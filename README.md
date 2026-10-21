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

- `/` opens the whole NBA/BAA teammate network in a desktop-first workspace (`/graph` remains an alias). A stable relationship layout, progressive labels and a faint visual backbone keep the overview readable. Every evidenced player and relationship remains loaded for exact searches and paths.
- **Find a player** searches names and aliases. Choosing a result brings that player into view; clicking a player on the map highlights their direct teammates and opens their career/team details without moving the camera. **Focus on player** is an explicit camera action.
- **Full network / Selected** switches between the overview and a selected player's direct teammates, a selected relationship's endpoints, or an active connection chain. Search preserves this mode. Returning to Full network restores the earlier overview position and zoom while retaining selection.
- **Find connection** opens the second-player search. Results highlight an exact shortest chain and provide a readable list with team and available season/game context. Each link opens admitted relationship evidence in the same inspector; Back to player, Escape and browser history return to browsing. Direct `/edge` links still serve standalone evidence.
- Drag the background to pan, scroll or pinch to zoom, or use the keyboard/camera buttons. Dragging a player gently moves their immediate neighbors; damped springs return them toward the layout after release. Nodes are never permanently pinned. **Reset view** restores layout positions and the overview. Intentional camera transitions respect the operating system's motion preference; there is no separate app motion setting.
- **Filters & layout** contains era/franchise emphasis and alternative Era lanes, Concentric rings, Force-directed and Orbit layouts. These controls never change graph distances or source facts. The optional overview map, collapsed search refinements and dismissible guidance keep the primary workspace compact.
- **Orbit** layout plus **Explore from start player** build a chain by hand. Every offered teammate keeps the chain on a shortest route to the chosen destination, earlier players and their teammates are withdrawn, and the route can be rewound from the chain list. **Discover** ranks the loaded snapshot: the ten pairs with the greatest finite shortest distance and the ten players with the most unique direct teammates. These rankings describe this snapshot, not complete historical coverage.
- `from` selects the starting player (taking precedence over `player`), `to` selects the destination, and a complete pair initializes a chain. Native GET controls and standalone evidence work without WASM. `depth` and `all` remain inert compatibility parameters. Historical coverage gaps remain visible under **About this network** and in evidence.
- `/edge?from=acyqu01&to=bogutan01` opens dated Dallas overlap evidence; `/edge?from=maravpe01&to=goodrga01` shows appearance-count proof without invented roster dates.


`overlap_days` is a number for certified dated overlap and `null` when a relationship is proved without its full duration. Such links expose `minimum_shared_games`; detailed edge evidence supplies the appearance-count calculation or original dated game witness.

Open [`/`](http://127.0.0.1:3000/) to explore the full pinned graph, including isolated players. At distant zoom levels, a deterministic visual backbone simplifies background edges; closer views reveal local edges. Proof-derived weights influence edge rendering without claiming friendship or complete roster duration. See the [UI interaction research](docs/research/ui-interaction-comparables-2026-10-15.md) and [earlier explorer verification](docs/reports/explorer-upgrade-2026-10-14/README.md).

Machine-readable routes include `/api/players`, `/api/search?q=...`, `/api/resolve?q=...`, `/api/query?q=...`, `/api/rank`, `/api/graph`, `/api/edges`, `/api/edges/{player}`, `/api/neighborhood?player=acyqu01&depth=1&limit=60`, `/api/connection?from=acyqu01&to=bogutan01`, `/api/discovery`, `/api/coverage`, `/api/coverage/{player}`, `/api/stats`, `/api/semantic-status`, `/api/diagnostics`, and `/api/diagnostics/client`. `/api/discovery` returns the ten longest finite shortest pairs and the ten most connected players of the loaded snapshot; the result is computed once and cached. `/api/fixture` is a compatibility alias for the current graph summary. `/api/graph`, `/api/fixture` and `/api/edges` return edge pages: `limit` (1–5000, default 1000) and `offset` select a window, and the response carries `total_edges` and `next_offset` (null on the last page). Neighborhood depth is 1 or 2, with at most 200 nodes and an explicit truncation warning.

`/api/paths?from=...&to=...&limit=100&offset=0` returns equal shortest alternatives in deterministic ID order, exact decimal `total_exact`, and resumable `next_cursor`. Pass that cursor unchanged as `cursor=...`; maximum page size is 500. Numeric offsets remain compatible.

## Semantic availability and measured limits

The server reads an existing `OPENROUTER_API_KEY` or `TYPESAFE_API_KEY` from its environment. When a key is already present in the launching shell, no additional setup is needed. The Rust client calls `https://openrouter.ai/api` with `typesafe/jev-1.14`; see the [earlier 1.13 client verification](docs/reports/t11-jev-client.md) for the transport smoke test. Credentials stay server-side. Browser canary checks and an explicit log-capture test cover the exercised routes and routine provider logs.

With no key or a provider failure, pages report semantic features unavailable and deterministic browsing, distances, paths, evidence and statistics remain usable. The client has one total ten-second deadline and at most two transport attempts. Semantic output cannot create players/edges or override graph calculations.

The [44-case labeled experiment](docs/evaluation/README.md) separates calibration from held-out cases and measures actual wording/order variants. Held-out baseline identity resolution matched **11/11** labels; routing matched **4/8**, and ranking **0/4**. Ranking abstained three times and gave one wrong preference for a hostile request; its degree-two graph facts remained unchanged. These small stratified sets do not establish broad accuracy. Frozen thresholds come from calibration, and future tuning needs a new untouched holdout. The 132 final outcomes used 102 provider calls, 129,531 input/29,352 output tokens and $0.005440302 reported cost. Initial aborted-run spend is unknown and excluded; retained pilot/supplement chronology and failures are documented.

## Runtime diagnostics

Every answer of this server is a function of the code that produced it, the snapshot it loaded and the request it served. The server can still say which of those three produced a given answer:

- `/api/diagnostics` reports the build that answers (commit, dirty flag, data mode), the snapshot it answers from (player and edge counts plus the SHA-256 digest of every source file and served canvas asset), the requests it served (request id, method, path without its query string, status, duration), the semantic judgments it made (outcome, the named cause of every failure, latency) and the failure reports the canvas relayed. The body carries no credential material, no provider receipt, no headers and no query strings.
- `/api/diagnostics/client` receives one client-side failure report (`kind`, `message`, `url`, `context`) and answers `202`. The canvas posts failed fetches, unreadable responses and uncaught page errors there, so a browser failure is visible next to the server's own records.
- `NBA_RUNTIME_LOG_PATH` names a JSONL file the server appends every runtime record to (one JSON object per line, tagged with `kind`). The file outlives the process and is the artifact that answers "what actually happened" for a run whose output is no longer on screen.
- `RUST_LOG` sets the log level; the default keeps records at `warn` or rarer. The provider client logs only status summaries, never credentials.

```sh
NBA_RUNTIME_LOG_PATH=/tmp/7-degrees.jsonl ./scripts/run-app.sh
```

## Verify and rebuild data

The reproducible check runner is `python3 scripts/check.py --profile full`. GitHub Actions runs the portable and browser profiles; raw-source verification is available separately. See [verification and receipts](docs/agents/verification.md) and [current source coverage navigation](docs/agents/source-coverage.md). The commands below remain useful for individual pipeline operations.

```sh
./scripts/build-canvas.sh
cargo test --workspace
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p canvas-view --target wasm32-unknown-unknown -- -D warnings
cargo test -p app-server --test browser_e2e --test canvas_browser -- --ignored --nocapture --test-threads=1
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
