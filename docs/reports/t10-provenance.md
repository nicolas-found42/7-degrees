# T10 — Teammate edge provenance panel

Issue #12, parent #1; spec `docs/specs/nba-teammate-degrees.md`.

## Opening evidence

Every selected-chain link and visible canvas relationship offers an accessible
“Open overlap evidence” link to `/edge?from=<canonical-id>&to=<canonical-id>`.
These links open a separate page, retaining the selected path, degree and exact
cursor in the original view. Selecting a canvas relationship by pointer or
native button additionally loads that same SSR page into the titled,
sandboxed `#edge-provenance-frame`. The Rust WASM selection hook updates the
frame and `#open-selected-edge` link; no authored JavaScript UI is added.
The existing canonical selected-edge data attributes/event remain available.
Native source links work when WASM assets are unavailable.

The page uses the admitted graph edge and the existing `edge_json` overlap
pairs. It shows canonical franchise identity, each exact positive half-open
interval, both player tenure records, end-of-season labels, evidence class,
membership source, arrival/departure transaction anchors and dated events,
source notes, and the source/version manifest. T4 integer days convert from
the documented 1946-01-01 epoch through checked Gregorian calendar conversion.
Fixture intervals remain explicitly synthetic day numbers; they never become
claims about 1946 historical dates. Repeated fixture overlaps remain separate
intervals with their corresponding source record pairs.

The real Quincy Acy / Andrew Bogut edge is shown on canonical MAVERICKS with
121 days of overlap `[2016-07-20, 2016-11-18)`, season 2016–17. Both dated tenure
records and their transaction-anchored arrival/departure dates are visible.
The panel also retains the snapshot's incomplete historical coverage warning.

## Coverage and source safety

A known pair without an admitted direct edge is not described as a proven
historical non-relationship. Imported reports explicitly say “Source evidence
is incomplete” and “Missing evidence does not prove a historical
non-relationship,” followed by excluded tenure records, evidence classes,
seasons, available intervals and reasons. Positive admitted edges also retain
endpoint coverage gaps; the panel does not claim complete historical evidence.
Coverage lists are bounded to 100 rows per endpoint with an explicit notice.

`/sources/tenure?record=<record-id>` resolves only a loaded certified tenure,
loaded excluded coverage row, or the explicit synthetic fixture record.
Unknown/path-like record IDs return 404. No path supplied by a request is read
from disk. `/sources/manifest` and `/sources/transactions` render only
compile-time committed source/version and transaction inventory documents.
Record links are local fixed routes, names and notes are escaped, and IDs are
URL encoded; source text is never used as an executable URL or HTML fragment.

## Validation and public seams

Four new tests exercise the approved HTTP/import seam: fixture record pairs and
chain evidence links; real Acy/Bogut dates, both source refs and versions;
unadmitted inferred evidence with escaped hostile reason text; and the
accessible missing-WASM panel/source-route allowlist. All four feature slices
were observed red before green. Existing fixture tests remain unchanged.

The actual ignored `canvas_browser` smoke now also checks the selected-edge
iframe. Chromium 153.0.8010.12 initialized the Rust WASM, clicked fixture and
real Acy/Bogut relationship controls, and read the evidence panel. It asserted
fixture intervals/ref pairs, real calendar dates/class/anchors/versions and
coverage, both real source refs, and the original path degree and exact cursor.
The fixture pointer click also exercises the same selection hook. A native
chain evidence link opened the standalone SSR page in a new tab. Both loaded
real source links were counted in the frame document. Panel locator capture
scrolls the iframe into view before full-page capture to avoid Chromium's
offscreen iframe painting race. Browser console/page errors were empty.
Screenshots were inspected.

Public server module: `app_server::provenance`; `/edge`, `/sources/tenure`,
`/sources/manifest`, `/sources/transactions`. Canvas hooks:
`#edge-provenance-frame`, `#open-selected-edge`, and the existing
`teammate-edge-selected` event. T4 `TenureRecord` now retains both original
anchor flags alongside the already retained source event days.

Release WASM is built before full workspace tests; formatter, host and wasm32
Clippy with warnings denied, and the actual browser smoke are checked after
merging the latest integration. Raw logs/screenshots and the single completion
gate are retained outside the repository under
`7-degrees-review-notes/t10-provenance/`. This does not claim completion of the
later full-app browser suite or full historical source coverage.
