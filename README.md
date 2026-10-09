# 7 Degrees

**NBA degrees of separation** — every NBA/BAA player in history as a node; a direct link
between two players if they were ever teammates (same franchise, simultaneously overlapping
roster occupancy). Playing for the same franchise at different times does *not* create a link.

Degree of separation between two players = smallest number of teammate links needed to
connect them.

100% Rust. In-app Jev (TypeSafe System One) judgments run server-side; graph math is code.

## Running locally

The default app builds the real canonical NBA/BAA graph from committed T3 player identities
and T4 dated-tenure reports. No raw source artifact or database server is needed at runtime.

```sh
./scripts/run-app.sh
```

The server listens on <http://127.0.0.1:3000>. Set `NBA_PORT` for another localhost port,
or `NBA_REPORT_DIR` for another directory containing `t3/` and `t4/` report snapshots.

Only tenures with **both dated transaction boundaries** and no blocking unresolved flags
create edges. Season-only brackets and single-bound cross-checks stay visible as coverage
gaps. Degrees/statistics are exact within this evidenced graph; the snapshot does not
establish the full historical teammate graph. See [T5 graph report](docs/reports/t5-real-graph.md).

- `/` — player connect form, coverage warning, and the first 100 teammate edges
- `/chain?from=acyqu01&to=bogutan01` — real Quincy Acy → Andrew Bogut chain
- `/api/graph` — player and edge summary (`/api/fixture` remains an alias for compatibility)
- `/api/players` — canonical player identities, aliases, first/last season and franchise context
- `/api/edges`, `/api/edges/{player}` — edges with overlapping source records and dates
- `/api/connection?from=acyqu01&to=bogutan01` — exact shortest chain and coverage status
- `/api/paths?from=acyqu01&to=bogutan01&limit=100&offset=0` — equally short alternatives,
  deterministic ID order, exact decimal `total_exact` and resumable `next_cursor`;
  pass the returned cursor unchanged as `cursor=...` to fetch the next page.
  Numeric `offset`/`next_offset` remain compatible; maximum page size 500
- `/api/stats` — cached exact finite separation statistics over unordered pairs
- `/api/coverage`, `/api/coverage/{player}` — coverage counts and individual excluded records
- `/api/semantic-status` — whether semantic (Jev) features are available

Run the unchanged synthetic fixture explicitly for browser tests:

```sh
NBA_DATA_MODE=fixture ./scripts/run-app.sh
```

All application UI and graph interaction code is Rust. The server renders accessible HTML;
the canvas compiles to WASM. `scripts/run-app.sh` builds the matching WASM and generated
wasm-bindgen loader before starting the server. It requires the `wasm32-unknown-unknown`
Rust target and wasm-bindgen CLI **0.2.129**. Generated JavaScript is only binding/bootstrap
glue; no JavaScript graph or UI library is used. `cargo run -p app-server` remains useful
without canvas assets: accessible paths, lists and expansion forms stay available.

- `/graph?from=acyqu01&to=bogutan01` — selected chain with Rust canvas exploration
- `/graph?player=acyqu01&depth=2` — bounded nearby neighborhood without a second player
- `/api/neighborhood?player=acyqu01&depth=1&limit=60` — deterministic node/context and
  relationship facts; depth 1 or 2, maximum 200 nodes, explicit truncation warning

WASM assets are generated under ignored `target/canvas-web`. Set `NBA_CANVAS_ASSET_DIR`
when serving artifacts built elsewhere; the startup wrapper honors `CARGO_TARGET_DIR`.

## Jev (semantic) features and the safe fallback

Set `OPENROUTER_API_KEY` (or `TYPESAFE_API_KEY`) in the environment to enable the
server-side Jev client — TypeSafe judgments via OpenRouter (`https://openrouter.ai/api`,
model `typesafe/jev-1.13`), config verified against the live TypeSafe/OpenRouter docs; see
`docs/reports/t11-jev-client.md`. The key stays server-side only: it never appears in any
served page, response, or routine log.

With no key, or when the provider is unreachable (fail-soft: explicit timeout + one retry,
then `Unavailable`), every deterministic feature keeps working unchanged and the UI shows
`Semantic features (Jev): unavailable` on every page (`/api/semantic-status` is the
machine-readable form). Graph facts never depend on Jev.

## Tests

```sh
cargo test
```

The fixture-backed API integration tests drive the app in-process over HTTP, exercising the
spec's graph rules: tenure overlap, same-franchise non-overlap, mid-season moves,
repeated-overlap deduplication, minimal-degree chains, unknown-player handling, and the
separation statistics.

Status: the canonical report loader and real dated-evidence graph (T5) are implemented;
fixture API tests remain available unchanged. Server-side Jev client with safe fallback
(T11) landed: see the "Jev (semantic) features and the safe fallback" section and
`docs/reports/t11-jev-client.md`.
Canvas browser smoke (after the WASM build):

```sh
./scripts/build-canvas.sh
cargo test -p app-server --test canvas_browser -- --ignored --nocapture
```

The test starts its own fixture server with Jev unconfigured. The installed Node,
Playwright and Chromium paths can be overridden with `BROWSER_NODE`,
`PLAYWRIGHT_PACKAGE_PATH` and `BROWSER_EXECUTABLE`. Its screenshot defaults to
ignored `target/canvas-browser.png` (`CANVAS_SCREENSHOT` overrides it).
