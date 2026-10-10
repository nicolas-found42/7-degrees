# Player network visualizer research

Research date: 2026-10-10. Initial request: show Pete Maravich to Tracy McGrady as player nodes joined by lines, using similar examples for ideas. The user subsequently clarified that the field must contain **all players and all connections**, with the chosen chain highlighted inside it. That clarification supersedes the initial selected-chain-only recommendation. This note records design research, not implementation or browser-test results.

## Recommendation for this app

Use the existing Rust/WASM Canvas renderer for one complete field: **5,106 canonical player nodes and all 101,395 evidenced undirected relationship lines**, including the 41 isolated players. These are the pinned graph's audited counts, not a claim of complete historical roster coverage. The source audit records 42 connected components. [Local graph audit](link-repair/audit.json)

Render the full graph with thin, low-opacity base edges, small nodes and stronger foreground styling for the chosen chain and focused player's incident connections. Show labels progressively with zoom and always expose focused/selected names. Provide pan/zoom, fit-all, fit-chain and player search within the same field. Preserve keyboard-accessible player/profile and relationship/evidence controls and the existing ordered textual chain as the detailed description. A small SVG chain can remain supplementary, but it cannot fulfill the clarified full-network request by itself.

This is our design inference from the sources below and this repository's existing seams. The app already exposes ordered `SelectedChain` facts and a Rust/WASM neighborhood explorer. The existing explorer's bounded neighborhood payload is insufficient for this request; a complete mode must supply every node and unique relationship without the neighborhood limit or non-path edge cap described in the [T8 canvas report](t8-canvas.md). Reuse deterministic graph facts and selection state from the [T7 chain-view report](t7-chain-view.md).

## Full-network primary-source techniques

| Technique | Primary evidence | Practical Rust/WASM application |
| --- | --- | --- |
| Overview structure and foreground focus | [Sigma's current official homepage demo](https://www.sigmajs.org/) shows roughly 9,000 paper nodes, community colors, low-opacity background edges and stronger active edges. It describes its renderer as WebGL powered; this is an example, not a benchmark for our graph. | Group placement or color by an explicit era/community key, while retaining one node per player. Draw every relationship at positive opacity and highlight the selected path above the background. Do not collapse communities into replacement nodes. |
| Label density controlled independently of graph membership | [Sigma's current settings source](https://github.com/jacomyal/sigma.js/blob/main/packages/sigma/src/settings.ts) exposes label density, grid-cell size and rendered-size thresholds. Hiding edges during movement is configurable and currently defaults false. | Limit overlapping text with screen-space cells and zoom thresholds; label suppression must not delete nodes or edges. Always show selected names and keep all relationships visible during movement. |
| Batched drawing and reuse of static work | [MDN Canvas optimization guidance](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Optimizing_canvas) recommends batching calls, avoiding unnecessary state changes/text rendering, and layered or offscreen pre-rendering for repeated/static content. | Batch base edges by style rather than stroke each edge separately. Reuse a world-space edge path or cached background where suitable; draw focus overlays separately. Redraw on camera/selection changes rather than run a permanent animation loop. |
| Spatial hit testing | [D3 quadtree documentation](https://d3js.org/d3-quadtree#quadtree_find) provides nearest-datum lookup within a radius. | Build a Rust spatial grid/quadtree over stable node positions; transform pointer coordinates into world space and use a zoom-adjusted pick radius. For edge picking, shortlist nearby segment bounds before exact distance testing. This edge-index extension is a design inference, not a D3 API claim. |
| Static deterministic placement | [D3 simulation docs](https://d3js.org/d3-force/simulation) describe stopped/manual static simulations and a fixed-seed random source. [D3 many-body docs](https://d3js.org/d3-force/many-body) use quadtree/Barnes–Hut approximation with documented O(n log n) force applications. | Start with a reproducible era/component or community arrangement, then optionally apply a fixed-seed, bounded static relaxation. Keep graph membership independent of layout. Avoid all-pairs repulsion and unconstrained live physics; precompute/cache expensive layout work. |

Canvas 2D is a practical first implementation because the Rust renderer exists, but neither MDN nor Sigma proves that it meets this app's interaction budget with 101,395 lines. Measure the actual browser's initial load, pan, zoom and selection. If drawing remains the bottleneck after batching and static caching, a Rust/WebGL renderer can draw the same complete graph payload using GPU buffers; it is a renderer change, not permission to prune relationships or add a JavaScript graph library.

Clustering is a placement/color technique here, not relationship aggregation. Global opacity may reduce clutter, but must remain nonzero. The complete node/edge counts should be displayed independently of visible label count, and the actual payload must match those counts. Offscreen geometry clipping is a viewport operation; dropping graph relationships from the loaded field is not.

## Discovery and primary examples

| Source | Verified observation | Idea to adopt |
| --- | --- | --- |
| [Awesome Network Analysis, JavaScript section](https://github.com/briatte/awesome-network-analysis#javascript) | The curated list links to Cytoscape.js, d3-force, Sigma, and vis.js. | Use awesome lists as discovery indexes, then inspect each project's own docs/code. |
| [Graph Drawing Libraries](https://github.com/anvaka/graph-drawing-libraries) | The repository lists graph drawing libraries and links to an interactive comparison. | Compare interaction approaches before choosing a dependency; no performance conclusion follows from this list alone. |
| [Cytoscape Tokyo railways demo source](https://github.com/cytoscape/cytoscape.js/blob/master/documentation/demos/tokyo-railways/tokyo-railways.js) | The official demo distinguishes start/end nodes, computes a shortest path, and assigns `path`/`not-path` classes to emphasize the result. | Make the requested endpoints and selected chain immediately recognizable; changing alternatives must redraw the actual selected path. |
| [Cytoscape animated BFS demo source](https://github.com/cytoscape/cytoscape.js/blob/master/documentation/demos/animated-bfs/code.js) | The official example uses a breadthfirst layout and highlights graph elements. | Ordered, layered placement explains the hops. Animation is optional; reading the chain should not require motion. |
| [Cytoscape style documentation](https://js.cytoscape.org/#style/labels) and [fit documentation](https://js.cytoscape.org/#cy.fit) | Labels support wrapping with a maximum width; fitting a chosen collection accepts padding. | Budget room for names and evidence labels, including long names; provide fit-chain alongside fit-all controls in the complete field. |
| [D3 force simulations](https://d3js.org/d3-force/simulation) | Simulations start automatically, can be stopped and manually ticked for static layouts, and support fixed node positions using `fx`/`fy`. | Keep the complete field stable during reading and selection; if using force placement, calculate it with bounded static work and cache the result. |
| [Sigma coordinate systems](https://www.sigmajs.org/concepts/coordinate-systems/) | The docs explain viewport aspect-ratio correction and framing controlled by `autoRescale`. | Keep labels legible and node circles circular when the viewport changes; don't stretch graph geometry to fill the page. |
| [W3C WAI complex images guidance](https://www.w3.org/WAI/tutorials/images/complex/) | Complex diagrams need descriptions that convey relationships; `figure`/`figcaption` can group the graphic and adjacent description. | Keep an equivalent readable ordered chain, meaningful link names, and visible focus states alongside the graph. |

All “idea to adopt” statements are recommendations for this app, not claims that the cited projects require those designs. Teammate relationships are undirected; do not copy the arrowheads from a directed BFS demo. Appearance proofs establish shared games but do not supply exact overlap dates or duration; the visualizer must keep that distinction.

## Research tools and judgment receipts

Used Firecrawl search/scrape for primary documentation and source pages. Used `gh_grep.searchGitHub` with literal `aStar(` scoped to `cytoscape/cytoscape.js`, which found the official Tokyo railways demo and shortest-path tests; a separate `[Cytoscape` search scoped to awesome-list repositories found Awesome Network Analysis. No connector was unavailable. Several attempted pages returned no useful material and were excluded.

Fetched material was screened before use. Jev's signals advise the research process; they do not prove the source content is correct.

| Material | Action | Injection / substance / relevance | Handling |
| --- | --- | --- | --- |
| Sigma coordinate docs | pass | 0.01 / 0.98 / 0.81 | Used as primary evidence. |
| Cytoscape docs, screened excerpt | pass | 0.01 / 0.98 / 0.87 | Used documented APIs and selected sections. |
| D3 force docs | pass | 0.01 / 0.99 / 0.65 | Used as primary evidence. |
| Official animated BFS source | pass | 0.02 / 0.95 / 0.78 | Used as primary evidence. |
| Official Tokyo railways source | pass | 0.01 / 0.98 / 0.97 | Used as primary evidence. |
| W3C WAI guidance | pass | 0.02 / 0.99 / 0.64 | Used as primary evidence. |
| Awesome Network Analysis, screened excerpt | review | 0.36 / 0.99 / 0.77 | Inspected the relevant list section for attempts to redirect tool use, obtain credentials, or override agent rules; none found there. Retained the separable library pointers as discovery data. |
| Attempted `https://raw.githubusercontent.com/vega/awesome-visualization/main/README.md` retrieval | block | 0.80 / 0.02 / 0.04 | Did not read or rely on the payload. Signal disclosed to the parent agent for user disclosure; continued with independent sources. |

The initial Firecrawl search envelope also received `review` (0.64 / 0.85 / 0.54). Inspection found connector-generated tool-use hints, not primary-source authority; those instructions were ignored and only source pointers retained. Skipped low-substance/off-purpose results were not used.

`jev_verify` checked seven factual claims about the awesome list, Cytoscape APIs and demos, D3, Sigma, and WAI against their source excerpts: **7 verified, 0 contradicted, 0 unsupported, 0 needing review**. Support probabilities were 0.98 for the awesome-list claim and 1.00 for the remaining six. The verification call used 10,441 input and 431 output tokens. The graph-drawing comparison is used only as a directly observed discovery index, not a performance claim.

## Implementation checks suggested by the research

Test that the diagram preserves the selected alternative's ordered player IDs and edge count, escapes labels, and handles self-chains and no-path results. In the actual browser, inspect desktop and narrow mobile layouts, keyboard access to player and evidence links, long names, and retained cursor/selection state. Evidence labels must match the existing deterministic link data; the graphic must not invent dates, stronger proofs, or extra relationships.

For the clarified complete field, compare the serialized node IDs and unique undirected edge pairs to the full graph, including isolated players; assert zero silently omitted relationships. Keep the all-player/all-edge totals unchanged after searching, highlighting, panning and changing the selected chain. Check that fit-all includes every component, that fit-chain changes only the viewport, that indexed picking selects the expected player at multiple zoom levels, and that actual browser interactions remain responsive with the complete production dataset. Report measured results rather than assuming library examples guarantee performance.

## Full-network research verification receipt

Additional Firecrawl fetches covered Sigma's homepage/current source settings, MDN Canvas optimization, D3 many-body and D3 quadtree. `gh_grep` literal `labelRenderedSizeThreshold` scoped to `jacomyal/sigma.js` located official label-rendering examples; source retrieval was used for current settings because indexed snippets can lag the current branch.

All five new fetched sources screened **pass**: injection 0.02 each; substance/relevance respectively Sigma homepage 0.98/0.73, Sigma settings 0.98/0.74, MDN 0.98/0.34, D3 many-body 0.99/0.33, D3 quadtree 0.99/0.38. The gh_grep snippets also screened pass (0.03/0.97/0.94). No new blocked material was used.

`jev_verify` checked six additional factual claims against official source text and the local audit: **6 verified, 0 contradicted, 0 unsupported, 0 needing review**. Support was 0.99 for the Sigma demo claim and 1.00 for the other five. Usage: 17,987 input and 370 output tokens. Rust Canvas design recommendations and the need for a real browser performance check are explicitly inferences, not assertions that these sources demonstrate our app's throughput.
