# Make the NBA/BAA graph readable at every scale: edge backbone, semantic zoom, ranked labels, bundled focus edges, and weighted layouts

## Problem Statement

The explorer loads the whole evidenced network (the pinned snapshot has 5,106 player nodes and 101,395 teammate edges; these are baseline figures, not constants for application logic). The issue #21 upgrade made focused views legible, but the canvas is still hard to read at three scales:

1. **Overview overdraw.** At whole-network zoom (about 9%), drawing every teammate edge, even at low alpha, produces a dense gray-brown mass in Era lanes and Force-directed layouts. Structural clusters and bridge players are not visible.
2. **Focus fan-out.** Selecting a well-connected player (for example, Pete Maravich: roughly 77 evidenced teammates across the `CELTICS`, `HAWKS`, and `JAZZ` franchises) draws dozens of independent curves from one node, although the relationships come from only a few franchises.
3. **Unranked mid-zoom labels.** At mid zoom the density-limited declutterer admits labels in iteration order. Peripheral two-hop players compete with core direct teammates, producing a flat wall of text.
4. **Unstructured Concentric rings.** One-hop and two-hop players form an undifferentiated cloud with no grouping by shared franchise and no radial meaning, and the remaining ~4,000 players form a heavy outer ring.
5. **Uniform Force-directed compression.** The spring relaxation treats a one-game overlap and a multi-season overlap identically and has no hub dissuasion, so the graph collapses into a dense rectangle instead of franchise-era clusters.

## Solution

Extend the Rust layout and canvas pipeline so the same complete graph reads clearly at every zoom, without changing what is admitted as evidence:

1. **Edge weight and backbone.** Derive a deterministic weight for every teammate edge from its admitted proof. Mark a sparse backbone (maximum spanning forest plus each player's top-k heaviest edges) used only to thin the unfocused background drawing.
2. **Three-tier semantic zoom.** Macro, Meso, and Micro tiers control node size, background edge visibility, and label budget.
3. **Ranked label priority queue.** Labels are placed in deterministic priority order into a padded screen-space index, so important labels win over ordinary ones.
4. **Franchise-bundled one-hop edges.** A focused player's direct teammate edges share a trunk per canonical franchise and draw as splines, with stroke and alpha scaled by weight.
5. **Sector-partitioned Concentric rings and weighted Force-directed layout.** Franchise angular sectors and weight-ordered radii in Concentric; weighted, hub-dissuaded attraction in Force-directed.

**Invariants (must hold throughout).** All teammate edges in the snapshot (101,395 in the pinned baseline) remain in the graph payload and available for shortest-chain search, filtering, ego inspection, evidence lookup, and teammate counts. The backbone is a rendering hint only. The active shortest teammate chain bypasses every cull.

## Relationship to existing specs

- **Builds on [nba-graph-explorer-upgrade.md](nba-graph-explorer-upgrade.md) (issue #21).** Reuses its DOM contracts, state ownership, edge hierarchy, label pills, degree-scaled nodes, Concentric bands, ego mode, filters, chain lifecycle, evidence drawer, reduced-motion rules, and acceptance seams. Where this spec differs it says so; otherwise #21 stands.
- **Builds on [nba-teammate-degrees.md](nba-teammate-degrees.md).** No change to player identity, edge semantics, shortest-chain definition, or deterministic-versus-semantic split.
- **Supersedes #21 only in these places:**

| Area | Issue #21 | This spec |
| --- | --- | --- |
| Overview background edges | Every edge at alpha 0.03, width 0.5 | Macro/Meso unfocused background draws backbone edges only; Micro restores non-backbone edges in the viewport |
| Focused one-hop edges | Quadratic Bézier, alpha 0.23, width 0.85 | Franchise-bundled spline with weight-scaled stroke/alpha when a franchise group is large enough (see Implementation Decisions); quadratic curve remains for ungrouped and two-hop edges |
| Label placement | Reuse existing density-aware decluttering | Deterministic priority queue with padded occupancy index; tier-based caps |
| Node radius | `baseRadius + log2(1 + degree) * scale` | Same formula family, with per-tier constants (the draft source used `ln`; the base is a constant choice, not a semantic one) |
| Concentric with no Player A / ego default | Prompt, no auto-select; ego opt-in | Auto-select a default focal player; ego mode on by default |
| Concentric angular order | Shared franchise, then debut season, then ID | Same keys, now rendered as labeled sectors with a gap; radius within the one-hop band ordered by weight |
| Force layout | Retain existing spring relaxation | Weighted, hub-dissuaded forces; still deterministic and seeded from Era lanes |

- **Deliberate reversals of #21 (decided by the maintainer):**
  - Choosing Concentric rings with no Player A **auto-selects a default focal player** (#21: focus the picker, never auto-select; supersedes #21's Concentric-without-A rule, its "do not silently choose a player" rule, and acceptance criterion A08).
  - Entering Concentric rings with a player selected **auto-enables ego mode** (#21: ego mode is opt-in, distant players drawn at alpha 0.05). The #21 ego toggle, reset, and chain-override rules are otherwise unchanged. #21 acceptance criteria A08 and the Concentric parts of A09/A10 must be updated when this spec is implemented.

## Terminology

This spec uses [GLOSSARY.md](../../GLOSSARY.md): **player node**, **teammate edge**, **roster tenure**, **degree of separation**, **shortest teammate chain**, **franchise identity**, **source coverage gap**. It also uses the established UI/spec terms **Era lanes**, **Concentric rings**, **Force-directed**, **ego network**, **evidence/proof**, and **debut era**.

Terms in the draft source (`spec.md`) that conflict with or are missing from the glossary, flagged rather than silently renamed:

| Draft term | Issue | Resolution in this spec |
| --- | --- | --- |
| "Visual Backbone" | Not in the glossary; "backbone" could imply the graph's structure rather than a drawing subset | Kept as **visual backbone**, defined strictly as a rendering subset of teammate edges. Proposed glossary entry if accepted (see Open Questions) |
| "Stint Trunks" | "Stint" is not a glossary term; glossary says **roster tenure**. A trunk is per franchise, not per tenure | Renamed **franchise trunk**. Bundling is by canonical franchise identity, not by tenure |
| "teammate relationship(s)" | Glossary term is **teammate edge**; "relationship" appears in UI text and API names | **Teammate edge** in domain statements; "relationships" kept only where it quotes existing UI/status text |
| "degree" / "total degree" | Glossary reserves **degree of separation** for chain length; node degree is a different concept (#21 also says "degree") | **Teammate count** for the number of distinct evidenced teammate edges at a node; **degree of separation** only for chains |
| "unreachable/3+ hop nodes" | Fine as a description, but unreachable does not mean historically unconnected (**source coverage gap**) | Phrased as "three-plus-hop and unreachable in the evidenced graph" |
| "Witnessed game" | Used in #21 as a proof class | Kept as **game-witness proof**, matching #21 |
| "weight" | New concept, not in the glossary | Defined below; proposed glossary entry (Open Questions) |
| "primary shared franchise" | A teammate edge can be admitted under several franchises | Defined deterministically below |

## User Stories

1. As an explorer viewing the whole network at about 9% zoom, I want background edges sparsified to the visual backbone, so that I can see era lanes and franchise clusters instead of a solid gray smudge.
2. As an explorer at Macro zoom, I want only the highest-teammate-count players in each debut decade to show landmark badges, so that I get historical reference points without text collisions.
3. As an explorer zooming into the Meso tier, I want player node radii to scale logarithmically with teammate count, so that multi-team journeymen and era bridges stand out from single-season players.
4. As an explorer past the Micro threshold, I want all local teammate edges and detailed labels to fade in smoothly, so that I can inspect every connection in a neighborhood.
5. As a user who selects a player in Era lanes or Force-directed layout, I want their direct teammate edges bundled into one trunk per canonical franchise, so that roughly 77 connections read as a handful of team groups rather than 77 crisscrossing lines.
6. As a user inspecting a focused player's direct teammates, I want edge thickness and opacity to reflect the strength of the proof (multi-season overlap versus a single witnessed game), so that I can spot long-term teammates.
7. As a user viewing a focused player at mid zoom, I want labels for direct teammates with the strongest proof to be prioritized over two-hop players, so that key teammates such as Gail Goodrich are labeled before distant acquaintances.
8. As a user reading labels at any zoom, I want enforced horizontal and vertical padding around every placed label, so that adjacent names never touch.
9. As a user viewing a focused player, I want direct-teammate labels styled with more weight and contrast than two-hop context labels, so that I can tell them apart at a glance.
10. As a user switching to Concentric rings, I want the ego network (one-hop and two-hop players) shown by default with three-plus-hop and unreachable players hidden, so that the viewport frames the rings cleanly without an outer gray smoke ring.
11. As a user in Concentric rings who wants league context, I want to restore the distant outer ring on demand using `Show ego network only (1–2 hops)`, so that I never lose whole-graph context.
12. As a user exploring a player in Concentric rings, I want the one-hop ring partitioned into labeled angular sectors by shared canonical franchise, so that I can see which teammates played together on the `HAWKS`, `JAZZ`, or `CELTICS`.
13. As a user exploring a player in Concentric rings, I want teammates with the strongest proof placed nearer the inner edge of the one-hop band and brief overlaps nearer its outer edge, so that radial distance encodes proof strength.
14. As a user choosing Concentric rings with no player selected, I want the app to select a default high-teammate-count focal player and tell me which one it chose, so that I never get all players collapsed onto one ring and can change the player immediately.
15. As a user switching to Force-directed layout, I want attraction weighted by proof strength with hub dissuasion, so that tight franchise-era rosters form readable islands separated by whitespace.
16. As a user selecting Player A and Player B, I want every node and edge on the active shortest teammate chain, plus its badges, to bypass semantic-zoom culling, backbone filtering, and label caps, so that the full blue chain stays visible at any zoom or filter state.
17. As a user hovering or clicking a bundled edge, I want hit testing to resolve the exact curve under my pointer, so that `/edge?from=X&to=Y` evidence opens reliably.
18. As a user toggling debut-era filters or franchise chips, I want label priority, semantic zoom, and the minimap to reflect the filtered emphasis immediately, while the backbone itself stays fixed.
19. As a user with `prefers-reduced-motion`, I want tier cross-fades, layout transitions, and camera fits to settle immediately into final deterministic positions.
20. As a keyboard-only or screen-reader user, I want the live status text and selection summary to report true graph totals regardless of semantic zoom or backbone thinning, so that visual decluttering never alters what the graph says.
21. As a mobile user at 390 px width, I want tighter label caps and adequate touch targets on bundled curves and badges, so that the small canvas stays legible and interactive.
22. As a maintainer, I want weights, the backbone flag, label order, and layout positions to be identical across reloads for the same snapshot and state, so that changes are reviewable and tests are stable.

## Implementation Decisions

### Where the work lives

- **Server-side, from full evidence:** weight derivation needs the admitted proofs, which live with the graph and provenance, so weight and the backbone flag are computed once where the complete edge evidence is available and carried in the graph payload as additive fields with defaults (compatible with fixture and custom payloads). The browser never recomputes weight from partial data. Exact module placement (graph core, graph-view composition, shared API types) is left to implementation; follow the seams #21 already uses.
- **Rust/WASM canvas crate:** semantic-zoom tier selection, label priority ordering, the screen-space occupancy index, franchise-trunk geometry, curve hit testing, Concentric sectoring, and the weighted force relaxation are pure Rust logic, so they can be unit-tested natively. Only the Canvas 2D drawing calls stay in the browser-facing module.
- **No new JavaScript graph/UI library.** Stack unchanged.
- **Spatial index.** Start with a screen-space occupancy grid (no dependency) for label collision. Adopt an `rstar` R*-tree only if measurement on the pinned graph shows the grid is insufficient. Rationale: the research ranks new dependencies as justify-by-measurement (see Sources). Hit testing against bundled curves may use the same index for candidate narrowing.

### Edge weight derivation

- Each teammate edge gets `weight` in `[0.1, 1.0]`, deterministic and monotonic in proof strength, computed from all of the edge's admitted proofs. The three proof classes remain exactly as #21 defines them: certified positive dated roster overlap, shared official dated team game (game-witness), and appearance-count arithmetic guaranteeing shared team games.
  - **Dated overlap:** logarithmic in total overlap days (half-open `[start, end)` intervals, summed across shared roster tenures).
  - **Appearance proof:** logarithmic in the sum over seasons of the guaranteed minimum shared games `A + B − N`, where `N` is that season's certified team-game total (not a universal 82).
  - **Game-witness only:** fixed baseline `0.15`.
- An edge with several proofs takes the maximum of its per-proof weights, so adding evidence can never lower weight. Ties between edges are broken by canonical player-ID pair.
- Weight is a **visual and ranking hint**. It must not be presented as a statistical measure of friendship or as proof of complete roster duration. An appearance proof still never implies dates or tenure (#21).
- Saturation constants (the day count and game count that map to weight `1.0`) are fixed constants, not derived from the current snapshot, so a data refresh does not silently reshuffle weights. Their values are tuned against the pinned snapshot during implementation; tests assert ordering, not numeric values.
- Days and games are different units. Edges proved by dated overlap and edges proved by appearance counts are compared only through these per-class mappings (see Open Question 4).

### Visual backbone

- A per-edge `in_visual_backbone` flag, computed deterministically on the **whole graph** (not on any filtered view):
  1. Maximum spanning forest over all player nodes, ordering by descending weight with ties broken by canonical player-ID pair, so every connected component stays connected (isolates have no edges and are unaffected).
  2. Union with each player's top-k heaviest incident edges (k = 3).
- The backbone is stable for a given snapshot. Era and franchise filters dim nodes; they do not change which edges are in the backbone.
- Only the **unfocused background** pass consults the flag. The following always render regardless of the flag: focus edges (direct and two-hop), hovered or inspected edges, active-chain edges, and, at the Micro tier, local non-backbone edges inside the viewport.
- The research notes that backbone filtering is lossy and that every evidenced relationship must remain reachable. This is satisfied because the flag affects drawing only. Pathfinding, ego mode, teammate counts, filters, and evidence always use all edges.

### Semantic zoom

- Three tiers from camera scale, with opacity cross-fades over a transition band at each boundary. Tier boundaries and alphas are tunable constants; starting values `<0.25`, `0.25–<0.75`, `≥0.75`.
- **Macro:** unfocused background draws backbone edges only, at faint alpha; ordinary nodes small and translucent; the top teammate-count players per debut decade get larger nodes and landmark badges under a small total cap.
- **Meso:** nodes sized by teammate count (`baseRadius + log2(1 + teammate count) * scale`, clamped to readable bounds, positive for isolates, with a larger hit region than the visual dot per #21); backbone background edges; ranked labels under a mid cap (lower at ≤640 px).
- **Micro:** non-backbone local edges fade in within the viewport; the highest label cap.
- Label caps (draft values: 10 / 24 / 50 desktop, 12 Meso mobile) are budgets, not guarantees; Tier 0 labels are exempt.
- Semantic zoom changes visuals only. Status text, tooltips, selection summary, pickers, and keyboard shortcuts always reflect the complete graph and distinguish loaded totals from emphasized totals as #21 requires.

### Label priority queue

- Deterministic order for nodes inside the viewport:
  1. **Tier 0 (always drawn, exempt from caps and culling):** Player A, Player B, every node on the active shortest teammate chain, the hovered node.
  2. **Tier 1:** direct teammates of the focused player, ordered by edge weight (descending), then teammate count (descending), then canonical player ID.
  3. **Tier 2:** two-hop and unfocused nodes, ordered by filter-match state, teammate count, then canonical player ID.
- Before accepting a Tier 1 or Tier 2 label, pad its measured screen-space box (draft: 14 px horizontal, 8 px vertical; constants tunable) and test it against the occupancy index already holding Tier 0 badges and all previously accepted labels. Greedy and bounded per frame, as the label-placement research recommends.
- Styling: Tier 1 uses heavier weight and full alpha; Tier 2 uses regular weight and reduced alpha. Both keep the dark-slate text and background halo from #21 (a different Tier 2 fill color from the draft is a visual constant, not a contract). Color is never the only cue.
- Label work stays bounded: cached text measurements, no per-frame allocation proportional to all 5,106 nodes outside the viewport.

### Franchise trunks (bundled one-hop edges)

- Applies when a focal player is selected in Era lanes or Force-directed layout. Concentric rings keeps radial geometry and does not bundle.
- **Primary shared franchise** of an edge: among the canonical franchises under which the edge is admitted, the one with the strongest per-franchise weight, ties broken by stable canonical franchise order. Each edge belongs to exactly one trunk group. Evidence shown in the drawer still lists every admitted franchise.
- A franchise group needs a minimum number of visible teammates to bundle (draft: 3). Smaller groups, two-hop edges, and edges with no franchise evidence keep #21's quadratic curves.
- Each bundled edge is a deterministic cubic spline from the focal player through a shared trunk control point toward the teammate. The trunk point lies on the line to the weight-averaged centroid of that franchise group's teammates, at a fixed fraction of the distance (draft: 0.35). Stroke width and alpha scale with weight within the focus ranges. The geometry used for drawing is the geometry used for hit testing (consistent with #21's curve rule), including at the curve's end near the teammate node.
- Bundling must not obscure endpoint nodes, handle zero-length or self edges, or hide individual edges: hovering a bundled edge highlights it in full and shows the teammate in the tooltip. The research warns that bundling can hurt path tracing and works poorly with collinear nodes (the Era lanes case), so bundling applies only to one focal player's direct edges and never to active-chain edges, which stay unbundled and blue.

### Concentric rings

- Keep #21's center, bands, and ordering keys. Add:
  - **Franchise sectors** across the one-hop band with a fixed angular gap, with sector header labels for each canonical franchise group, unmatched or unknown groups last. Sector size is proportional to group size, with a minimum angular width so tiny groups remain legible.
  - **Weight-ordered radius** within the one-hop band: strongest proof nearest the inner radius.
  - Two-hop placement follows #21 (grouped by evidenced predecessor's franchise, stable tie-break).
- **Ego by default.** Switching to Concentric rings with Player A set turns on the existing `Show ego network only (1–2 hops)` mode (#21 semantics). Turning it off restores three-plus-hop and unreachable context (drawn at alpha 0.05 per #21). The active chain stays included beyond two hops either way.
- **Default focal player.** Choosing Concentric rings with no Player A selects the player with the highest teammate count in the loaded graph (ties by canonical player ID), sets it as Player A exactly as a canvas or picker selection would (so a set Player B recomputes the chain), and announces the choice in the status region. The picker remains editable. An empty snapshot still disables the request with an explanatory state (#21). This is a deterministic choice, not a semantic or random one.

### Force-directed layout

- Keep the deterministic spring relaxation seeded from Era lanes. Weight attraction by edge weight and divide by a hub term (teammate count + 1) in the ForceAtlas2 style. Same snapshot, mode, and selection yield identical final positions. Filters do not rerun the layout (as in #21).
- Evaluate adopting a published solver crate against extending the existing relaxer only if the extended relaxer fails the cluster-separation visual check. Preference is to extend the existing code (no new dependency); the research on `fa2` and Barnes–Hut crates is the fallback evidence.

### Accessibility, motion, and performance

- All #21 accessibility contracts are unchanged: DOM IDs, combobox pickers, `data-layout`, announcements only on meaningful change, no color-only meaning.
- Tier cross-fades and bundling transitions respect `prefers-reduced-motion` exactly as #21's layout transitions do.
- No continuous background physics; relaxation terminates and animation work stops when settled.
- Per-frame work budget: backbone filtering must reduce, not add to, edge passes at overview. Measure against the pinned snapshot at Macro and Micro.

## Testing Decisions

- **What makes a good test (carried over from #21):** assert observable behavior, deterministic output invariants, and accessible state. Do not assert private data layouts, spring-iteration counts, exact coordinates, canvas-call order, or random samples. Independently check minimality and connectivity from known fixtures rather than mirroring the implementation.
- **Seam 1: deterministic pure-logic tests (native Rust, synthetic fixtures)** using the existing fixture crate (the dated Player A–B–C fixture, the Alpha–Beta shortened-season fixture, the diamond and duplicate-name fixtures). Add only small missing cases: an edge with several admitted franchises, a dated game-witness-only edge, and a dense clique.
  - Weight: monotonic ordering across dated overlap, appearance proof, and game-witness-only; max-over-proofs; ties break by canonical pair; appearance weight uses the actual team-game total `N`.
  - Backbone: every connected component stays connected; edge count falls on a dense clique; top-k incident edges present; result identical across runs and independent of any filter; isolates unaffected.
  - Label queue: Tier 0 never culled at any tier or cap; a higher-weight teammate outranks a lower-weight one; padded boxes never overlap; order is deterministic with ties by canonical ID.
  - Trunk grouping: each edge in exactly one group; groups below the minimum stay unbundled; chain edges never bundled; curve hit test resolves a point on a bundled curve and rejects a point on the old straight chord.
  - Concentric: higher-weight teammate gets a smaller radius within the band; franchise groups occupy separate sectors; an isolated focus still frames.
  - Force: identical positions across repeated runs; heavy intra-franchise edges end up shorter than light ones on a fixture.
- **Seam 2: HTTP/payload contract (existing integration seam).** The graph payload still contains every node and edge; new fields are additive with defaults; `weight` and `in_visual_backbone` present and consistent with Seam 1; shortest-chain, search, evidence, and coverage responses unchanged.
- **Seam 3: real-browser journeys (primary acceptance, existing serial Chromium harness).** Extend the current 25-capture suite rather than adding a harness: zero console/page errors; status text still reports exact loaded totals at every tier; a bundled edge click opens the drawer and updates the canonical URL; active chain fully visible at Macro with era filters and ego mode on; reduced-motion settles immediately; 390×844 mobile caps and touch targets; Pete Maravich in all three layouts at overview and label zoom.
- **Determinism and accessibility contracts preserved:** a repeat-load test comparing layout-derived observable output; the #21 accessibility assertions (roles, names, announcements, no frame-by-frame status churn) re-run unchanged.
- **Visual verification:** review actual screenshots for the smudge, fan-out, label wall, and ring blob before and after; screenshots supplement but never replace behavioral assertions. Follow [verification.md](../agents/verification.md) for check profiles, receipts, and evidence publication; do not call unrun checks passing. Per [source-coverage.md](../agents/source-coverage.md), this work changes no sources, so no coverage audit is required, but the spec must not be read as certifying completeness.
- **Fixture caveat:** specific figures quoted in the draft (Maravich 77 teammates and 18 + 49 + 41 = 108 guaranteed shared games with Goodrich; 121 days for Acy and Bogut; game `0022100058`) come from the draft's screenshots. Re-derive them from the pinned snapshot before using them as test oracles.

### Acceptance criteria

| ID | Observable requirement | Evidence/seam |
| --- | --- | --- |
| B01 | Every edge has a deterministic weight in `[0.1, 1.0]` ordered dated/appearance/game-witness by proof strength; game-witness-only edges use the baseline. | Fixture unit + payload contract |
| B02 | The backbone preserves every connected component and is stable across runs and filters; the full edge set remains in the payload. | Fixture unit + payload contract |
| B03 | At Macro and Meso with nothing focused, only backbone edges are drawn in the unfocused pass; at Micro non-backbone local edges appear in the viewport. | Browser captures |
| B04 | Pathfinding, ego mode, filters, teammate counts, and evidence use all edges: results match the pre-spec behavior. | Existing contract and browser tests unchanged |
| B05 | Active chain nodes, edges, and badges are never culled by tier, backbone, caps, filters, or ego mode. | Fixture unit + browser |
| B06 | Label order is deterministic, Tier 0 exempt, padded boxes non-overlapping, caps scale by tier and mobile width. | Unit + browser captures |
| B07 | Stronger-proof direct teammates are labeled before weaker ones and before two-hop players at Meso. | Unit + browser |
| B08 | A focused player's direct edges bundle by primary franchise (≥ minimum group size) in Era lanes and Force-directed; ungrouped edges keep quadratic curves; chain edges unbundled. | Unit + capture |
| B09 | Clicking or hovering any bundled edge resolves the correct edge and opens `/edge?from=X&to=Y` in the drawer. | Browser |
| B10 | Concentric shows labeled franchise sectors; stronger proof sits nearer the inner band edge; repeated selection yields identical layout. | Unit + capture |
| B11 | Concentric with no Player A selects the highest-teammate-count player (ties by ID), announces it, and recomputes any set chain; Concentric with Player A enables ego mode by default and the toggle restores distant context. | Browser |
| B12 | Force-directed output is reproducible and weight-sensitive; no background physics runs when settled. | Unit + browser |
| B13 | Status text, tooltip, selection summary, pickers, and shortcuts report full-graph truth at every tier; reduced motion settles immediately. | Browser |
| B14 | Pinned full graph survives repeated tier, layout, filter, chain, and drawer interaction with no console/page errors and acceptable frame cost at overview. | Full serialized browser profile with receipts |

## Out of Scope

- Any change to what is admitted as evidence, teammate-edge semantics, player universe, source acquisition, tenure reconstruction, or snapshot refresh. Season-only memberships and unresolved transaction brackets never create edges.
- Removing or hiding edges from the payload, pathfinding, ego mode, search, or evidence; the backbone is a drawing hint only.
- Filter-constrained pathfinding or alternative-chain UI.
- Replacing the single-page architecture, adding player profile or search-result pages, new database, or new JavaScript graph/UI frameworks.
- Continuous or non-terminating physics; any layout that differs across reloads for the same snapshot and state.
- WebGL/Canvas-library migration, pre-rendered static layers, or a pluggable layout-engine refactor (the research proposes these; they are separate decisions).
- Community-detection layouts (Leiden/Louvain) and hierarchical or force-directed edge bundling beyond the franchise trunks specified here.
- Saved view state, sharing of camera/zoom state, analytics, or exports.
- Reproducing the draft's awesome-list, product, and crate link inventory; the research documents are the reference for those.

## Open Questions

Resolved with the maintainer: **top-k = 3**; **tier boundaries 0.25 / 0.75** (starting values, retunable against captures); **default focal player = auto-select highest teammate count** (reverses #21); **Concentric ego mode on by default** (reverses #21). Still open:

1. **Tier alphas and label caps.** Starting values (backbone alpha 0.02 Macro / 0.04 Meso; caps 10 / 24 / 50, 12 on mobile Meso) are tunable; confirm after the first captures.
2. **Definition of "hub" for the default focal player.** Highest teammate count, ties by canonical ID, is assumed; confirm that is the intended definition.
3. **Glossary additions.** Add **visual backbone**, **teammate count**, and **edge weight** (and perhaps **franchise trunk**) to GLOSSARY.md, or keep them spec-local?
4. **Cross-unit calibration.** Days of dated overlap and guaranteed shared games are different units mapped onto one weight scale. Is a per-class mapping with fixed saturation constants acceptable, or must dated and appearance weights be tuned to be comparable?
5. **Macro landmark rule.** "Top 3% by teammate count per debut decade" (about 150 players) under a small viewport cap: confirm the percentage and cap, and what the per-decade rule does for unknown-debut nodes.

## Sources

Conclusions below are supported by the repository's research documents; items tagged inference in those documents remain inferences.

- [docs/research/graph-visualization-research.md](../research/graph-visualization-research.md)
  - §2 Edge rendering: hairball mitigation by subtraction. Edge filtering and minimum-spanning-tree backbones reduce clutter but lose topology (Hu & Shi 2012), which is why the backbone here is a drawing hint with all edges retained. Bundling benefits and cautions (Holten 2006; McGee & Dingliana 2012; collinear layouts such as Era lanes).
  - §4 Labels: greedy, bounded, priority-based placement (Christensen, Marks & Shieber 1995); semantic zooming (Perlin & Fox 1993; Heer & Shneiderman 2012); `measureText`/halo/`roundRect` canvas facilities.
  - §1 Layout and §7 Performance: Barnes–Hut cost; Canvas 2D and WASM performance guidance.
- [docs/research/rust-graph-visualization-and-dx.md](../research/rust-graph-visualization-and-dx.md)
  - §2 Rust ecosystem evaluation and §3.1 layouts (ForceAtlas2, Barnes–Hut, Concentric/radial layouts); §4.4 three-tier semantic zoom table (research thresholds 0.2 / 0.8, versus the 0.25 / 0.75 chosen here); §5.3 flakiness mitigation in browser tests.
- [docs/research/nba-data-sources.md](../research/nba-data-sources.md): the evidence classes and coverage limits that weight derivation must respect.
- Not independently verified: the draft's quoted proof figures (see Fixture caveat) and its external link inventory.
