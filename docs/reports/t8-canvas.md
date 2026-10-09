# T8 — Rust canvas neighborhood exploration

Issue #11, parent #1; spec `docs/specs/nba-teammate-degrees.md`.

## Behavior

The selected-chain page links to `/graph` retaining canonical endpoints, the
exact cursor string, chain-page limit and selected alternative index. The
canvas page uses the same `chain_view::SelectedChain` and accessible rendering
as T7. Ordered player names, teammate links and the minimum degree remain
present during exploration. A self-chain remains degree zero; disconnected
pairs retain T7's fixture-versus-incomplete-history distinction.

Rust/WASM draws the selected chain in blue and highlights selected players and
relationships. Pointer clicks select a node or edge; dragging pans; wheel zoom
anchors to the cursor. Native buttons pan, zoom and refocus the selected chain;
a focused canvas also accepts arrow keys, +/− and f. Every transform redraws
without changing graph facts. Visible status exposes zoom and viewport state.

Player selection announces name, source-backed season range and canonical
franchise context. Fixture context is explicitly synthetic day ranges. Direct
expansion loads one graph hop; nearby expansion loads two. The server preserves
the selected chain while rebuilding that selected player's neighborhood. The
accessible dropdown and expansion forms also work without WASM. Visible player
and relationship lists mirror the canvas facts.

`/api/neighborhood?player=<canonical-id>&depth=1|2&limit=60` returns deterministic
node/context and relationship facts. Its node limit accepts 1–200, including
the focus player. `/graph` uses `neighbor_limit` for that bound and accepts
`depth=0` for the initial chain. A graph page always retains selected-chain
nodes and links in addition to its neighborhood bound. Non-path links are
bounded to 600; omitted nodes/relationships set an explicit truncation warning.
Missing evidence remains a coverage warning, never an assertion of historical
absence. Exact graph facts are independent of Jev.

## Public hooks and build

The new `canvas-view` crate shares serializable `GraphPayload`, `GraphNode` and
`GraphLink` structs with the server. Its WASM module owns layout, drawing,
selection, viewport state and input listeners. No JavaScript graph or UI
library is used. The only application JavaScript is a two-statement binding
bootstrap importing and initializing generated wasm-bindgen glue.

`template#graph-payload[data-payload]` contains inert, attribute-escaped JSON.
The selected path payload remains available through T7's
`template#selected-chain-payload`. The `#selected-edge` element exposes canonical
`data-from`/`data-to` and accessible team/overlap facts. Selecting an edge emits a
`teammate-edge-selected` document CustomEvent whose detail is JSON text for the
server-provided `GraphLink`. This is the seam for the later provenance panel;
this ticket does not claim to render its full dated-source panel.

`scripts/build-canvas.sh` compiles the pinned wasm-bindgen 0.2.129 / web-sys
0.3.106 crate and generates binding assets under ignored `target/canvas-web`.
`scripts/run-app.sh` builds and serves matching assets, honoring custom
`CARGO_TARGET_DIR` / `NBA_CANVAS_ASSET_DIR`. Only the two named generated JS/WASM
assets are served; arbitrary paths are rejected. Missing assets produce a
visible accessible fallback. No generated assets or source artifacts are
committed.

## Validation

Five import/HTTP tests cover worked fixture one-hop/two-hop facts and contexts,
selected path/cursor retention, bounded truncation and invalid arguments,
missing-asset fallback, and real report player context with coverage. The API,
SSR, loader and fallback slices were observed failing before their fixes.
Original fixture tests remain unchanged.

The Rust-authored ignored `canvas_browser` smoke starts a fresh fixture server
with Jev unconfigured and drives the actual installed Chromium through a thin
Node/Playwright RPC transport. Chromium 153.0.8010.12 initialized WASM without
page/console errors. Assertions checked changed canvas pixels after zoom,
button transforms, keyboard pan, refocus, pointer node and edge selection,
pointer drag and wheel zoom, direct/nearby expansion, continued path degree,
exact cursor retention, accessible era/team context and canonical edge event.
The screenshot was inspected. Actual logs and screenshots are outside the
repository at `7-degrees-review-notes/t8-canvas/`.

Host workspace tests, host and wasm32 Clippy with warnings denied, formatter,
release WASM build and the actual browser smoke are run before reporting.
This is a focused canvas smoke, not completion of the later whole-app browser
acceptance suite or a claim of full historical source coverage.

## Independent review correction — asset response testing

A post-build review reproduced an invalid UTF-8 panic in the graph-view test
helper: it decoded the generated WASM response as text. The earlier retained
workspace run preceded the WASM build and did not expose that failure.

The corrected HTTP helper retains response bytes and headers for binary routes.
The asset test uses two independently configured routers concurrently: a private
asset fixture directory containing a valid WASM binary with non-UTF-8 custom
section data, and an explicitly unavailable asset configuration. It asserts exact
binary bytes, WASM/JavaScript MIME types, missing-asset 503 responses, visible SSR
fallback, retained path names/degree, and rejection of a non-allowlisted file
even when that file exists. Neither shared assets nor process-global environment
variables are changed by the tests.

`graph_view::CanvasAssets` now belongs to `AppState`; both SSR readiness and asset
serving use that same router's configuration. Public builders
`app_with_jev_and_canvas_assets` and `app_with_report_data_and_canvas_assets`
accept explicit `Directory` or `Unavailable` configuration. Existing builders
use the default generated-asset directory. Runtime startup resolves
`NBA_CANVAS_ASSET_DIR` once; the browser smoke passes its directory explicitly.

Correction evidence is saved as `t8-canvas/fix-*.log`: a release WASM build runs
**before** the full workspace suite, followed by formatter, host/wasm32 Clippy
and the actual browser smoke. The prior Jev escalation is retained; no gate
retry was made to seek automatic acceptance.
