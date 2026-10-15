# 7-Degrees: visual and functional context

This guide is for readers and agents using the screenshots without access to the application source. It describes the explorer implemented for [issue #21](https://github.com/nicolas-found42/7-degrees/issues/21), captured on October 14, 2026 against application commit `118a446`.

## What the app shows

The explorer loads the complete **evidenced NBA/BAA teammate network**: the pinned snapshot has **5,106 player nodes and 101,395 admitted teammate edges**, including isolated players. An edge is supported by dated roster overlap, a shared official dated team game, or appearance-count arithmetic guaranteeing shared team games. Franchise continuity or season co-membership alone does not establish an edge.

Degrees are exact within this evidenced graph. Historical coverage is incomplete: missing evidence may shorten a historical chain or connect currently unreachable players. Filtering and ego views change presentation, not the graph used for shortest paths.

The explorer is at `/`, with `/graph` as an alias. Enhanced evidence links open a drawer in the same explorer. Direct or refreshed `/edge?from=<id>&to=<id>` URLs remain standalone evidence pages.

## Page anatomy

Above the canvas are **Player A and Player B** search controls, Connect players, Random pair, Clear chain, three layout buttons, the selection summary and an ordered chain breadcrumb. Pickers support names/known aliases, explicit debut-decade and canonical-franchise filters, contextual results, arrow-key navigation, Enter selection and Escape dismissal. Typing clears that endpoint's previous canonical selection; choosing a result is explicit. Older search and chain responses cannot replace newer state.

Nearby, eight labeled era toggles expose pressed states. Selected-player franchise chips emphasize that player's admitted teammates on that franchise, not everyone who ever played there. Era and franchise emphasis combine; disabled eras dim ordinary context. Text states matching and visible counts separately from loaded totals. Selected players, hovered players and active chain nodes retain priority. Reset filters restores all eras, clears franchise emphasis and exits ego view.

The canvas has a docked **minimap at bottom left** and labeled **camera controls at bottom right**. Its orange viewport frame follows the main camera. Click the overview to navigate or drag its frame. Hover text and the selection summary provide player name, seasons/franchises and complete-graph teammate count.

The canvas uses a 1000×600 logical coordinate plane with a device-pixel-ratio-scaled bitmap; the minimap uses 200×120 logical units. CSS display dimensions vary, especially on phones.

## Layouts and rendering

- **Era lanes:** horizontal placement uses explicit debut-season metadata, with deterministic vertical spread. Colored decade bands and top-of-canvas headers provide temporal context. Unknown debut is explicitly unclassified.
- **Concentric rings:** Player A is the center. Direct teammates and two-hop players occupy separate radial bands with width, rather than thin rings. Distant/unreachable context is faint. Requesting this layout without A prompts for selection and leaves the previous layout active until a player is chosen.
- **Force-directed:** a deterministic relaxation seeded from Era lanes. Identical graph/state yields repeatable settled placement.

Layout transitions are short; reduced motion applies the final state immediately. Orange indicates ordinary player focus, and blue marks active chain nodes/edges. Background edges are faint; focused curves have stronger hierarchy. Nodes reflect evidenced teammate degree, and isolates remain visible. Selected, hovered and chain labels use prominent badges; ordinary dark labels have halos and are culled around important labels and docked overlays. Neighbors are not guaranteed a label at every zoom.

**Show ego network only (1-2 hops)** includes the full two-hop neighborhood, without the bounded API's cap. Active chains remain included beyond two hops. Fit whole network exits ego hiding and retains current emphasis filters.

## Pair connections

Both canonical endpoints request the server's exact shortest teammate chain. The breadcrumb states degree, ordered names, franchises and proof summaries. Breadcrumb player buttons center the camera without changing the pair; evidence links inspect the actual hop. A self-pair has degree zero; a direct teammate pair has degree one. A disconnected historical pair is described as unresolved evidence, not proof of a historical non-relationship.

Pair changes immediately clear old highlights. Loading, request failure and retry are distinct states. Random pair chooses two distinct existing IDs when possible and may legitimately find no chain. Clear chain removes B and the result while retaining A, layout and view filters. Shared `from`/`to` URLs initialize and frame a chain; `from` takes precedence over `player`. Native GET selectors/forms provide usable chain and evidence text without WASM.

## Evidence and source limits

The modal drawer retains the explorer instance, pair, camera, layout and filters. Close, Escape or browser Back returns to the graph; Forward reopens evidence. Modified-click follows the real anchor's normal new-tab behavior. Focus stays in the open dialog and returns to the trigger on close. Loading and failure have concise announcements, retry and a standalone recovery link; late responses cannot reopen a closed drawer.

- **Appearance-count cards:** show A/N and B/N plus the guaranteed minimum A+B-N, using actual recorded team-game totals. Maravich/Goodrich includes `27 + 73 - 82 = at least 18`; the synthetic shortened-season example uses `60 + 50 - 66 = at least 44`. Counts do not identify exact shared games or roster duration.
- **Dated cards:** retain half-open intervals and end-exclusive wording. Acy/Bogut's admitted MAVERICKS overlap is `[2016-07-20, 2016-11-18)`, 121 days, with underlying tenure records and source anchors.
- **Game-witness cards:** retain original game date and record references without inventing full tenure dates.
- **Coverage:** an overall caveat stays visible. Excluded records appear in collapsed details with a traceable table of player, franchise, season, interval, evidence class and reason; unknowns and truncation remain explicit. These rows are not admitted proofs.

Shots 18, 21, 22 and 23 are explicitly **synthetic test fixtures**, not claims about historical NBA players. Shot 23 demonstrates shortened-season arithmetic. Shot 25 is a real pinned official game witness, not a synthetic date.

## Keyboard, phone and motion

Canvas focus supports arrow-key pan, +/− zoom and `f` fit; picker and dialog keyboard input is handled separately. At 390×844, pickers stack above the canvas, navigation has compact touch targets, and the minimap starts collapsed behind its accessible toggle. Single-finger pan and two-finger pinch suppress accidental node/edge activation after a gesture; cancellation and pinch-to-pan recovery are supported. Important badges avoid the docked controls. The rest of the page remains scrollable.

## Screenshot index

The original 01-12 filenames are retained with refreshed captures. New files 13-25 document the upgraded interactions and proof states. Canvas-only captures emphasize rendering; full-page captures show control placement. Drawer captures show the visible dialog viewport, whose content scrolls internally.

| # | File | What it shows |
| --- | --- | --- |
| 01 | [01-full-page-era-lanes.png](01-full-page-era-lanes.png) | Default explorer with both pickers, layout controls, era toggles and the complete network. |
| 02 | [02-canvas-whole-network.png](02-canvas-whole-network.png) | All 5,106 player nodes and 101,395 admitted teammate edges; no active chain. |
| 03 | [03-layout-concentric.png](03-layout-concentric.png) | Pete Maravich at the center; direct and two-hop teammates spread across radial bands. |
| 04 | [04-layout-force.png](04-layout-force.png) | Deterministic force layout seeded from Era lanes, with Pete Maravich selected. |
| 05 | [05-zoomed-labels.png](05-zoomed-labels.png) | Readable selected-player badge and ordinary labels with halos and decluttering. |
| 06 | [06-player-selected-focus.png](06-player-selected-focus.png) | Pete Maravich selection summary, franchises, filters and focused era canvas. |
| 07 | [07-focus-highlight-canvas.png](07-focus-highlight-canvas.png) | Thin focused curves, selected badge and readable direct teammate context. |
| 08 | [08-minimap-and-controls.png](08-minimap-and-controls.png) | Minimap and its viewport frame at bottom left; camera controls at bottom right. |
| 09 | [09-labels-while-zoomed.png](09-labels-while-zoomed.png) | Pete Maravich selected in Force layout at useful label zoom. |
| 10 | [10-mobile-view.png](10-mobile-view.png) | 390-pixel layout with stacked pickers and the minimap collapsed initially. |
| 11 | [11-edge-evidence.png](11-edge-evidence.png) | Direct /edge page for Pete Maravich and Gail Goodrich; count proofs and coverage caveats. |
| 12 | [12-concentric-full-page.png](12-concentric-full-page.png) | Toolbar, selected-player context and centered concentric canvas. |
| 13 | [13-shortest-chain.png](13-shortest-chain.png) | Pete Maravich to Gail Goodrich, degree one; blue chain and ordered evidence breadcrumb. |
| 14 | [14-era-franchise-filter.png](14-era-franchise-filter.png) | 1970s debut emphasis combined with Maravich's admitted JAZZ teammate relationships. |
| 15 | [15-ego-network.png](15-ego-network.png) | Complete one- and two-hop neighborhood; distant context hidden without unloading graph data. |
| 16 | [16-count-proof-drawer.png](16-count-proof-drawer.png) | Actual appearance denominators and arithmetic; exact overlap dates remain unknown. |
| 17 | [17-dated-proof-drawer.png](17-dated-proof-drawer.png) | Quincy Acy and Andrew Bogut: MAVERICKS overlap [2016-07-20, 2016-11-18), 121 days. |
| 18 | [18-mobile-toolbar.png](18-mobile-toolbar.png) | Synthetic A/B chain; stacked controls, readable badges and compact docked navigation. |
| 19 | [19-mobile-evidence-drawer.png](19-mobile-evidence-drawer.png) | Real Acy/Bogut dated proof in the viewport-width modal drawer. |
| 20 | [20-reduced-motion.png](20-reduced-motion.png) | Settled concentric layout with Pete Maravich selected and reduced motion enabled. |
| 21 | [21-filtered-chain-fixture.png](21-filtered-chain-fixture.png) | Known A-B-C chain remains visible while eras are disabled and franchise/ego filters apply. |
| 22 | [22-synthetic-dated-proof.png](22-synthetic-dated-proof.png) | A/B worked-example overlap [day 2, day 4); fixture days are not historical dates. |
| 23 | [23-shortened-season-proof.png](23-shortened-season-proof.png) | A 66-game team total guarantees at least 44 shared games from 60 and 50 appearances. |
| 24 | [24-mobile-count-proof.png](24-mobile-count-proof.png) | Maravich/Goodrich proof cards with appearance meters, actual 82-game totals and source records. |
| 25 | [25-game-witness-proof.png](25-game-witness-proof.png) | Precious Achiuwa and David Johnson: RAPTORS game 0022100058 on October 27, 2021; original event records. |

## Files and verification

- [PDF screenshot collection](7-degrees-screenshots.pdf): one labeled capture per page, with page heights adapted to full-page images so text is not shrunk into a thumbnail.
- [Manifest](manifest.json): captions, origins, source paths and SHA-256 hashes for all 25 PNGs.
- [Fresh capture receipt](capture-receipt.json): actual browser commands and results for the nine refreshed views; zero browser errors.
- [Implementation verification and review](../reports/explorer-upgrade-2026-10-14/README.md): validated portable/browser receipts, 121 Rust / 108 Python / 13 browser tests, source-byte certificate and advisory Jev review results.

The other 16 images are byte-identical copies from the verified implementation report; their source hashes were checked when copying. Screenshots illustrate sampled states and do not independently prove minimum graph distance, complete historical coverage, or behavior across every possible interaction.
