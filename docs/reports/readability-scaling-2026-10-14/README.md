# Graph readability scaling — issue #22

Implemented [issue #22](https://github.com/nicolas-found42/7-degrees/issues/22): admitted-proof weights and a deterministic visual backbone, three semantic zoom tiers, ranked padded labels, focused franchise splines with shared drawing/picking geometry, franchise-sector Concentric bands and weighted hub-dissuaded Force relaxation. The complete evidenced graph remains available to distances, ego views, counts and evidence. Historical coverage remains incomplete.

Weights map certified days and guaranteed shared games separately, with fixed saturation at 1,825 days and 246 games; game-witness-only evidence gets 0.15. Adding proofs cannot lower weight. Primary franchise uses strongest per-franchise weight and canonical-order ties. These are visual hints, not friendship scores or certificates of unknown roster duration. The maximum spanning forest plus each player's three heaviest edges is cached when the full graph loads; filters never rebuild it.

## Execution and preserved source bytes

The final `issue22-20261014-final` full profile passed all ten checks: **131 Rust tests, 108 Python tests and 13 serialized Chromium/WASM journeys**, zero failed checks. Formatting, host/WASM Clippy, WASM build, derived pins, immutable historical audit and frozen semantic audit passed. The recovery journey intentionally injects two request failures and verifies recovery; other journeys assert no browser errors. [Compact receipt](checks/review-summary.json), [manifest](checks/manifest.json), [browser log](checks/browser-matrix.log).

[Source-byte certificate](source-bytes.json) confirms the tracked diff and new code/test hashes still matched the run before staging. The receipt records starting commit `2b5379349c62703f86612f2a3d02c4dbd7c63e7b` plus the working tree. Existing user documentation edits were present during checks and excluded from this implementation commit. Sources and snapshot pins were unchanged.

The pinned overview draws **13,583 backbone edges from 101,395 loaded relationships**. The final log recorded a **7 ms Macro draw** and **3 ms Micro draw** with 14,446 local background edges. These are individual debug-app observations on this host, not portable latency guarantees. The payload contract independently compares every player/edge to the admitted graph and includes isolates.

## Acceptance evidence

| Criteria | Evidence |
| --- | --- |
| B01–B02 | Public payload and native proof/backbone tests: additive defaults, shortened-season denominator, max-over-proof mappings, stable canonical ties, top-three incident edges, dense-clique thinning, component connectivity and isolates. |
| B03–B07 | Serialized browser checks of Macro/Meso backbone bounds, Micro local rendering, mobile caps, filtered ego chain badges/edges; native priority/weight ordering, padded collision index and crowded cap-exempt badges. |
| B08–B09 | Native spline endpoints, grouping threshold and chain exclusion; an actual real-data spline hover/click checks its teammate tooltip, canonical evidence URL and drawer proof card. |
| B10–B12 | Native sector separation, unknown-last ordering, weight/radius order and deterministic weight-sensitive Force output; browser default-player announcement, ego restoration, layout controls and reduced-motion settling. |
| B13–B14 | Full existing browser matrix, counts/payload assertions, keyboard/touch/minimap/drawer history, mobile controls and error assertions. |

## Visual inspection

The Era overview no longer has an opaque gray edge mass. Label captures show fewer, spaced names with stronger direct-teammate styling. Concentric headers identify franchise sectors, while its default ego view removes the distant outer ring. The final phone selected-player badge avoids both the minimap toggle and camera controls. [Observation transcript](visual-observations.txt); [capture hashes](screenshots.json). Earlier [issue #21 captures](../explorer-upgrade-2026-10-14/screenshots/) provide the comparison baseline.

Force is more spread after weighted attraction and bounded grid repulsion, but the full NBA/BAA graph still forms a connected cross-era cloud. These captures do not establish inferred communities or a statistical clustering result. No solver dependency or continuous physics was added.

| View | Captures |
| --- | --- |
| Whole network | [Desktop](screenshots/all-player-network.png), [phone](screenshots/all-player-network-phone.png) |
| Pete Maravich — Era | [Overview](screenshots/maravich-era-overview.png), [labels](screenshots/maravich-era-labels.png) |
| Pete Maravich — Concentric | [Ego sectors](screenshots/maravich-concentric-sectors.png), [labels](screenshots/maravich-concentric-labels.png), [phone](screenshots/maravich-phone-sectors.png) |
| Pete Maravich — Force | [Overview](screenshots/maravich-force-overview.png), [labels](screenshots/maravich-force-labels.png) |
| Exact relationships | [Bundled click evidence](screenshots/maravich-bundled-evidence.png), [Macro filtered chain](screenshots/maravich-chain.png) |

## Standards

Separate Standards review found no documented-rule violations. Its optional findings were addressed: unused sector parameters were removed, and label priorities now use a named enum.

## Spec

Separate Spec review found three defects, all corrected: crowded important badges now try neighboring columns before overlapping; focused opacity stays in the retained 0.18–0.28 range; unknown franchise groups sort last. Regression tests failed before the collision/ordering fixes and passed afterward. The final manual completion audit independently validated every receipt log hash, the current tracked diff and all new code/test hashes, and inspected the actual browser assertions. No substantive source blockers remained. Force cluster separation remains a visual judgment rather than a community-detection claim; focused Force views and the narrow phone Concentric field can still be dense.

## Jev advisory results

The [final per-file gate](jev-reviews.json) did **not** grant automatic acceptance: composite 0.624, minimum safe-to-apply 0.25, with low confidence across review rubrics. It returned five verified/low-confidence claims, one unsupported combined browser claim, and no contradicted claims. The unsupported claim was then checked against the actual test assertions and passing browser log: focused verification supported it at confidence 0.92, and verified execution counts at 1.0. The performance-log claim remained verified but advisory (0.79 confidence); the raw log contains the exact samples. These model judgments do not replace the execution receipts or manual source/visual review.
