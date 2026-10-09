# T7 — Degree and shortest-chain view

Issue #9, parent #1; spec `docs/specs/nba-teammate-degrees.md`.

The Rust SSR `/chain` page now lists equally short alternatives and renders an
identifiable selected chain. Ordered player names, ordered teammate links,
minimum degree and actual link count remain available as accessible text.
Direct relationships say degree 1; the fixture A→B→C chain says degree 2;
a self-query says degree 0 and explicitly explains that it needs no links.

## Alternative inspection

`/chain?from=<canonical-id>&to=<canonical-id>` defaults to at most 20 alternatives.
The `limit` argument accepts 1–100 alternatives per page. Each selection link
retains both endpoints, the exact current `cursor`, the page limit and the
selected index on that page. The selected link has `aria-current`, a visible
highlight and a text label identifying the selected alternative.

“Next shortest alternatives” follows the graph's exact `next_cursor`, rather
than converting it to an integer. “First shortest alternatives” returns to the
first page. Every displayed candidate comes from the shortest-path DAG; longer
paths are excluded. No call to the unbounded `all_shortest_chains` enumerator
is made. The exact total is rendered as decimal text.

A real HTTP probe over the synthetic 130-node, 64-binary-layer graph checked
all 18,446,744,073,709,551,616 shortest chains remain pageable: the penultimate
rank's next link retained `v1:18446744073709551615`; following it rendered the
final all-b path with 65 links and degree 65. Selecting that final alternative
again preserved the same path. This is a bounded traversal check, not an
attempt to materialize or print all chains.

## Uncertainty and invalid arguments

Fixture A→D is explicitly “Verified disconnected within this synthetic fixture.”
A known pair without a chain in the imported report graph instead says “Missing
dated roster evidence may hide a historical connection.” It carries the coverage
warning and displays no fabricated degree or selected-chain payload.

A partial `/chain?from=<id>` request presents a choose-second-player form and
shows no chain; this supports profile/search navigation. Blank or unknown IDs,
invalid cursors, conflicting cursor/offset arguments, invalid page sizes,
out-of-range selections and exhausted page requests return explicit errors.
An exhausted page for a connected pair is not presented as historical
non-connectivity. Query text is escaped; invalid markup-like cursor text is
rejected.

## Selected-chain seam for later graph and provenance views

`app_server::chain_view` exports `ChainQuery`, `SelectedChain::from_chain`,
`render_selected_chain` and `page`. `SelectedChain` contains ordered canonical
player IDs and display names, ordered links, and the exact degree. Its facts
come from the deterministic graph result.

The rendered section `#selected-chain` provides `data-degree` and JSON
`data-path`. `template#selected-chain-payload` provides the escaped JSON payload
in `data-payload`; it is inert markup, not executable code. Read the DOM
attribute to obtain decoded JSON. The containing wrapper provides the exact
current `data-cursor` and selected index. No payload appears for errors or
no-path outcomes.

## Validation

Six new tests exercise the approved import/HTTP/SSR seam: equally short
selection, cursor navigation retaining selection, distinct no-path certainty,
direct/indirect/self link counts and payloads, invalid page/selection behavior,
and hostile query/cursor text. The five feature slices were observed red
before green; hostile-input assertions additionally verify the existing shared
escaping behavior. Existing fixture tests remain unchanged.

The localhost boundary probe retains its HTML and log outside the repository
under `/Users/Nicolas/Documents/github/7-degrees-review-notes/t7-chain-view/`.
Full workspace tests, formatter and clippy are checked again after merging the
latest integration tip. Browser interaction verification remains part of the
later complete-app browser acceptance run.
