# NBA graph readability completion

This follow-up completes the remaining readability and interaction work for [issue 22](https://github.com/nicolas-found42/7-degrees/issues/22). Force now spreads roster pockets with visible whitespace, and phone Concentric views retain round, separated bands and readable franchise headings. The complete evidenced graph still contains 5,106 players and 101,395 teammate edges. This change acquires no sources or snapshot data.

The [earlier report](../readability-scaling-2026-10-14/README.md) describes commit `55a2857`. The follow-up resolves its dense-view caveats and the requirements found by the subsequent specification audit.

## Readability and camera behavior

Force uses bounded logarithmic, proof-weighted attraction with teammate-count dissuasion and global Barnes–Hut repulsion. It starts from deterministic Era positions, traverses canonical edges and stops after 360 steps. The roster-clique fixture verifies separation; real graph captures show dispersed pockets and readable labels at inspection zoom.

A shared camera transform projects world geometry uniformly in CSS pixels. Phone rings remain circular, curve drawing and picking use the same CSS geometry, and rotating fingers at constant CSS separation preserves zoom. Concentric spreads tied proofs through the one-hop band and collision-places cached franchise headings.

Detailed labels fade across semantic zoom boundaries. Direct teammate labels retain weight priority even with an active chain. Important badges find available space around the HUD, have touch targets, and point back to displaced nodes. Background edges honor era and franchise dimming; only on-screen teammates contribute to franchise trunks. Canonical quadratic bends and adaptive spline picking preserve edge identity.

## Transition and drawer interactions

Press-time picking records the item in the displayed frame before settling the layout. Node, edge and empty taps leave final deterministic positions. Layout changes rebuild the spatial index for their first displayed frame; unrelated keys leave the animation running; ego fits and breadcrumb centering supersede pending camera motion. Minimap presses interpret the displayed projection and continue dragging without a jump.

A delayed chain result frames its nodes beside an already-open desktop evidence drawer and reserves the drawer area for badges. Opening or closing evidence by itself retains the camera.

## Verification

The full delivery profile passed ten checks: **139 Rust tests, 108 Python tests and 13 serialized Chromium/WASM journeys**. Formatting, host and WASM Clippy, WASM build, derived snapshot pins, immutable historical audit and frozen semantic audit passed. The recovery journey deliberately rejects two requests and verifies recovery; the remaining journeys assert no browser errors. See the [compact receipt](checks/review-summary.json), [manifest](checks/manifest.json) and [browser log](checks/browser-matrix.log).

The [source-byte certificate](source-bytes.json) relates the checked working tree to the implementation files. Existing user documentation edits were present during checks and excluded from the commit.

The existing browser journeys now explicitly verify Macro/Meso backbone rendering and Micro non-backbone edges, weighted direct-label order, repeat-load position signatures and labels, absence of idle physics, default A selection after B is set, desktop and phone Era/Force spline activation at bodies and near endpoints, displaced badge touch activation, and a six-hop chain with all eras dimmed and ego mode enabled. The interrupted-motion journey covers node and empty taps, breadcrumb centering, unrelated keys, ego fits, minimap press/drag and a late chain beside an open drawer.

## Requirement audit

The [acceptance matrix](acceptance.json) checks all 22 stories, 14 B criteria, 10 implementation seams and 35 inherited A criteria against source and execution evidence. Eighty items are covered; inherited A08 is intentionally superseded by issue 22's announced default focal selection. Standards and Spec reviews found no remaining implementation blocker after their corrections.

## Solver comparison

An isolated experiment evaluated `fa2` 0.3.0 with Barnes–Hut, single-threaded execution and 600 iterations, using Era-seeded positions and all graph edges. The second configuration additionally reduced attraction at hubs. Both retained an elongated central distribution in their trial plots. The extended solver produced a broader distribution without adding an application dependency. The [comparison program](solver-experiment/src/main.rs), [trial log](solver-evaluation.log) and [trial plots](solver-plots/) preserve the experiment. Native solve samples were 1,195 ms extended, 3,871 ms fa2 base and 4,292 ms fa2 with hub preprocessing; these are one-run observations on this host.

## Visual evidence

| View | Capture |
| --- | --- |
| Unfocused Force overview | [Full graph](screenshots/all-player-force-network.png) |
| Focused Force | [Overview](screenshots/maravich-force-overview.png), [labels](screenshots/maravich-force-labels.png) |
| Concentric | [Franchise sectors](screenshots/maravich-concentric-sectors.png), [phone bands](screenshots/maravich-phone-sectors.png) |
| Era lanes | [Overview](screenshots/maravich-era-overview.png), [labels](screenshots/maravich-era-labels.png) |
| Chain precedence | [Six-hop Macro ego view with every era dimmed](screenshots/six-hop-macro-ego-chain.png) |
| Drawer-aware fit | [Late chain beside open evidence](screenshots/chain-beside-open-drawer.png) |
| Mobile controls and evidence | [Toolbar](screenshots/fixture-mobile-toolbar.png), [drawer](screenshots/fixture-mobile-drawer.png) |

[Capture hashes](screenshots.json) and the [observation transcript](visual-observations.txt) identify the reviewed images. The final [Jev gate](jev-gate.json) accompanies the source and execution review.

## Jev and completion review

The gate required manual review: composite 0.637 and minimum safe-to-apply 0.27, with low confidence in broad per-file rubrics. Its compound chain/default claim was marked contradicted at confidence 0.09. Inspection of the actual assertions and passing log, followed by focused verification with the omitted default-selection assertions, resolved that discrepancy. Report verification supports execution at 0.98, solver facts at 0.80 and browser coverage at 0.89. The 81-item breakdown was independently counted from unique IDs. The [completion audit](completion-audit.json) records these resolutions; automatic acceptance is not claimed.
