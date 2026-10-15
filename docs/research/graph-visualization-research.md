# Graph Visualization Research — making the 7-degrees canvas more readable, beautiful, and navigable

*Researched 2026-10-14. Feeds a follow-up implementation pass over the Rust/WASM canvas
renderer (`crates/canvas-view/src/browser.rs`) and its page chrome
(`crates/app-server/src/{graph_view.rs,ui.rs,style.css}`). Findings were read against the
system as it exists today: 5,106 nodes / 101,395 edges / 42 components / diameter 11 / 41
isolates (pinned audit, `docs/reports/link-repair/audit.json`), era-lane layout, a single
`CanvasRenderingContext2d`, fixed 1000×600 internal resolution, and the repo rule that
"All application UI, interaction and graph logic is Rust" (`README.md:29`).*

**Method and evidence standard.** Every load-bearing claim below is traced to the source
that owns it — an official specification (`WHATWG HTML`, `W3C WCAG 2.2`, `W3C WAI-ARIA 1.2`,
`MDN Web Docs`, `Khronos`), first-party library documentation used as reference material
(D3, vis-network), or a published paper. Papers cited by DOI were verified against the
Crossref API on 2026-10-14; spec text was read from the live documents on the same date.
Where a statement is my own design inference for this repo rather than a claim from a source
it is marked **[INFERENCE]**. Contrast ratios and layout arithmetic in this note were
computed locally from the sRGB relative-luminance formula specified in WCAG 2.2 and from the
current source constants.

## Headline findings

1. **Keep the era lanes, but stop relying on them alone.** Lanes are defensible: x encodes a
   real ordinal attribute (debut year) that force-directed layout would discard, and the
   canvas is the only place that attribute is shown. Force-directed placement has no notion
   of external attributes, is prone to local minima, and "results are poor for graphs with more
   than a few hundred vertices" without multilevel coarsening (Fruchterman & Reingold 1991;
   Kobourov 2012, §1). At 5,106 nodes a naive spring embedder costs Θ(|V|²) *per iteration*
   (Fruchterman & Reingold 1991), which is not a candidate for the single WASM thread; even the
   O(n log n) Barnes–Hut approximation (Barnes & Hut 1986; D3's `forceManyBody` documents the
   quadtree approximation and its `O(n log n)` cost) belongs in an offline, precomputed,
   deterministic pass, not in an interaction path. **Recommendation:** keep lanes, and spend
   the layout budget on *ordering within lanes* to reduce within-lane crossings, which is a
   1-D problem solvable with the layered-graph machinery of Sugiyama et al. 1981.
2. **Add a minimap (overview+detail).** The system under study is an 8,795 × 5,200-world-unit
   strip rendered at ~9.4 % scale to fit; the review of overview+detail, zooming, and
   focus+context finds overview+detail interfaces are consistently *preferred* and, for
   comprehension tasks, hard to beat (Cockburn, Karlson & Bederson 2009, §7.2, §8). The same
   review notes the classic cost — split attention — and the standard mitigation: let the
   overview be browsed without moving the detail view, and always reflect detail movement in
   the overview (ibid., §3.1, §8). **[INFERENCE]** For a single wide strip one overview
   rectangle is sufficient; Furnas 2006's warning that very large worlds need several layers
   of context does not bite here because the world is one-dimensional and bounded.
3. **Use the existing `distance` field for degree-of-interest focus.** `GraphNode.distance`
   already exists (`crates/canvas-view/src/lib.rs:10`) and `neighborhood()` already computes it
   by BFS (`crates/app-server/src/graph_view.rs:95-108`), but the full-network payload sets it
   to `None` for every node (`network()`, `graph_view.rs:195-207`). Furnas' generalized fisheye
   degree of interest — `DOI(x|y) = API(x) − D(x,y)`, with items below a threshold removed —
   is the canonical formalism for exactly this (Furnas 1986); van Ham & Perer apply degree-of-
   interest *diffusion* from focused nodes to large-graph exploration (van Ham & Perer 2009).
   **Recommendation:** compute hop distance to the selected player in the full-network payload
   and dim in three tiers. This is the highest-value change per line of code.
4. **The current focus styling does not meet the 3:1 non-text contrast threshold.** WCAG 2.2
   SC 1.4.11 requires graphical objects needed to understand content to have a contrast ratio
   of at least 3:1 against adjacent colours (W3C, *Understanding SC 1.4.11*). The selected
   player's incident edges are stroked at `rgba(176,79,0,0.45)` (`browser.rs:236`), which
   composites on white to `rgb(219,176,140)` — **1.98:1**. The base lattice at
   `rgba(66,109,122,0.075)` (`browser.rs:213`) composites to **1.11:1**. The selected node
   `#b04f00` (5.30:1) and path node `#176896` (6.07:1) do pass. **Recommendation:** strokes that
   carry meaning should be opaque or at least α ≈ 0.69 at the current hue.
5. **One of the nine decade colours fails the same threshold, and no legend explains any of
   them.** `#bc9a2c` (the 1980s bucket) is **2.69:1** on white; the other eight range 3.24–5.06:1.
   The page ships no legend for the decade palette (`graph_view.rs` `page()` renders
   `.network-legend` prose only, and `style.css` has no swatch rules). Two primary sources
   converge here: colour must not be the only visual means of conveying information (WCAG 2.2
   SC 1.4.1), and Okabe & Ito's Color Universal Design explicitly requires "redundant coding"
   and labels placed on the graphic rather than in a distant key. ColorBrewer's guidance is
   also directly applicable: qualitative schemes work best when hue varies and lightness /
   saturation are held nearly constant, and only ~7 classes stay differentiable for
   non-adjacent marks (Harrower & Brewer 2003).
6. **Labels should be culled in screen space, not gated by one global zoom number.** Today a
   label is drawn whenever `self.scale >= 0.7` (`browser.rs:314`). At the fit scale of ~0.094
   that threshold requires a 7.4× zoom-in, at which point only ~16 % of the strip's width is
   on screen. Optimal point-feature label placement is NP-hard in the general case, so the
   literature's practical answer is greedy local search with a small candidate set and an
   objective that penalises overlap, placement preference, and dropped labels (Christensen,
   Marks & Shieber 1995). D3's quadtree finds the nearest node within a radius in one call,
   and the repo already builds an equivalent 80 px spatial hash (`browser.rs`, `start()`).
   **Recommendation:** label only the selected/nearest node and its 1-hop neighbours, plus a
   density-limited set at high zoom.
7. **The canvas is under-sampled on every Retina display, and that is a cheap fix.** The
   element is fixed at `width="1000" height="600"` (`graph_view.rs`, `page()`), while CSS
   stretches it to the ~1,054 px inner column of the 70 rem page (1,120 − 2 rem page padding
   − 2 rem panel padding − 2 px border) — about **2.11 device pixels per canvas pixel at
   DPR 2**, and ~1.05 at DPR 1. MDN documents both the symptom ("A `<canvas>` can appear too
   blurry on retina screens") and the remedy: size the bitmap by `window.devicePixelRatio`
   and `ctx.scale(dpr, dpr)`. `web-sys` already exposes `Window::device_pixel_ratio`,
   `HtmlCanvasElement::set_width/set_height`, and `CanvasRenderingContext2d::set_transform`.
8. **The expensive frame work is already isolated; cache it rather than repeat it.** The base
   lattice is the only 100k-segment draw and it is already batched into one path and culled
   off-canvas (`browser.rs:215-233`). Two further cheap wins are documented by MDN's
   *Optimizing canvas*: pre-render repeating/static drawing to an offscreen canvas and
   `drawImage` it, and batch calls instead of changing state per primitive. The current node
   loop calls `begin_path`, `arc`, `set_fill_style_str` and `fill` once per node and re-sets
   `set_font`/`set_text_align` every node (`browser.rs:281-312`) — five thousand redundant
   state changes per frame.
9. **A stroked path is unioned, not accumulated — which changes what the faint lattice
   means.** The HTML Standard's stroke steps fill the traced path, and "overlapping parts of
   the paths in one stroke operation are treated as if their union was what was painted"
   (WHATWG HTML, §4.12.5.1.10). Consequently the 101,395-edge lattice renders at a *uniform*
   0.075 alpha regardless of how many edges cross a pixel: it is a flat 1.11:1 wash, and its
   cost is rasterisation of ~100k segments, not alpha accumulation. Any "reduce the alpha"
   intuition is therefore wrong; the lever is coverage and density, not opacity.
10. **Prefer no new dependencies.** `web-sys 0.3.106` already exposes everything the cheap
    recommendations need — `Path2d`, `OffscreenCanvas`, `OffscreenCanvasRenderingContext2d`,
    `ImageBitmapRenderingContext`, `CanvasRenderingContext2dSettings`-shaped context options
    (`get_context_with_context_options` takes a `JsValue`), and `HtmlCanvasElement::transfer_control_to_offscreen`.
    A WebGL renderer is a different matter: it is the documented escape hatch when 2D fill
    cost dominates (Khronos WebGL 1.0 specification; `ANGLE_instanced_arrays` for instanced
    edge drawing), but it is a large, dependency-adding rewrite of a renderer that must stay
    in Rust (`README.md:29`). Decide it from measurement, not aesthetics.

## 1. Layout: are era lanes defensible, and what do force-directed layouts buy?

**What the code does.** In `start()` (`browser.rs:355`) every full-network node is placed at
`x = (debut_year − 1947) * 110 + jitter * 105`, `y = (lane − 0.5) * 5200`, where both `jitter`
and `lane` come from an FNV-1a hash of the node id. There is no relaxation step and no edge
is consulted for placement. The result is an 8,795 × 5,200 strip. Because the y coordinate is a
hash, the vertical order carries no information at all — it exists only to spread nodes out.
**[INFERENCE]** This is the single largest layout inefficiency: within a lane, edges between
same-era players are drawn as arbitrary long diagonals, and the era bands (a real signal) are
not visually separated from the hash scatter.

**What force-directed buys.** The spring-embedder tradition is precisely characterised by its
sources. Eades' 1984 spring embedder replaces vertices with steel rings and edges with
logarithmic-strength springs and "make[s] nonadjacent vertices repel each other" (Eades 1984,
quoted in Kobourov 2012, §2). Fruchterman & Reingold 1991 modify those forces to
`f_a(d) = d²/k`, `f_r(d) = −k²/d` with `k = C·√(area/|V|)` and add a cooling "temperature" that
caps displacement, aiming at even distribution, uniform edge lengths, and symmetry. What this
buys is *structural* proximity: Noack showed that force-directed layouts optimise a measure
that corresponds to Newman's modularity, so "communities appear as groups of nodes"
(Jacomy et al. 2014). What it costs is stated just as clearly: the classical algorithms target
30–40 vertex graphs (Eades used `M = 100` iterations; Fruchterman & Reingold likewise), a
random initial layout means "the result varies depending on the initial state" and "the
process can get stuck in a local minimum" (Jacomy et al. 2014), and vertex position "cannot be
read as a Cartesian projection" — "it has to be compared to the others."

**Cost in a single WASM thread.** Per Fruchterman & Reingold's own complexity analysis each
iteration is Θ(|V|² + |E|) for the basic algorithm and Θ(|V| + |E|) for their grid variant
"assuming that vertices are uniformly distributed over the grid boxes"; their grid variant
restricts repulsion to a radius of `2k` and distorts results when vertices are not uniform.
Barnes & Hut's tree code computes forces in O(N log N) (Barnes & Hut 1986), which is what D3's
`forceManyBody` implements ("a quadtree and the Barnes–Hut approximation … `O(n log n)` per
application"). **[INFERENCE]** With 5,106 nodes and 101,395 edges, one Barnes–Hut tick is cheap
in absolute terms, but the number of ticks is chosen by guesswork — Fruchterman & Reingold are
explicit that "our algorithm's termination is guesswork, as was Eades'" — and D3's default
alpha decay corresponds to 300 iterations. Running that interactively inside a WASM module on
the main thread would freeze the page. If this layout is ever adopted it must be precomputed
and shipped as coordinates.

**Hybrid options.** Two are directly supported by the literature:

- **Lane-constrained relaxation.** Fruchterman & Reingold describe constraining vertices
  against a frame ("we chose to have a wall stop that component of displacement *normal* to
  it"), and D3 documents fixing a node's position with `node.fx` / `node.fy`, which is exactly a
  constrained coordinate. Holding x fixed per lane and relaxing only y is one bounded pass.
- **Keep lanes, order within lanes.** Sugiyama, Tagawa & Toda's framework determines "the order
  of the vertices in each level … to reduce the number of crossings of edges", then fixes
  horizontal positions subject to that order; the ordering heuristic works on *barycenters*,
  which for a vertex in a level are averages over the horizontal positions of its adjacent
  vertices in the neighbouring level (Sugiyama, Tagawa & Toda 1981). **[INFERENCE]** Ordering
  each era band's nodes by a single barycenter sweep over the previous band's order would
  convert many arbitrary diagonals into short near-vertical edges, at negligible cost, and
  without touching the semantics of x.

**Stability when adding focus.** The mental-map literature is unambiguous that a drawing that
rearranges itself on interaction is harmful: layout adjustment methods are judged by
"preservation of the 'mental map' of the diagram" (Misue, Eades, Lai & Sugiyama 1995), and
Holten's edge-bundling paper lists "computational complexity and layout stability" as the
known issues of force-directed methods for this class of data (Holten 2006, §2.2). **[INFERENCE]**
Selecting a player must therefore change *emphasis and viewport*, never node coordinates. The
current `select()` → `focus_player()` path already obeys this; any hybrid layout must too.

## 2. Edge rendering: 101k strokes at α 0.075

**What the code does.** In `draw()` the base lattice is one `begin_path()` / `move_to` /
`line_to` loop / one `stroke()` at `rgba(66,109,122,0.075)`, width 0.6, skipped entirely while a
drag is in progress, with a per-segment rejection test against the four canvas edges
(`browser.rs:211-235`). The selected player's incident edges are a second single path at
`rgba(176,79,0,0.45)`, width 1.3 (`browser.rs:236-252`).

**The perception result of the current choice.** Because one stroke operation unions overlaps
(WHATWG HTML §4.12.5.1.10), the lattice is a *uniform* 1.11:1 wash on white; every pixel that
any edge touches gets exactly one 0.075 composite. Alpha cannot be tuned to make a single edge
legible without raising the whole wash, and no amount of overdraw darkens it. Two consequences:
the lattice is invisible as individual edges at any zoom (they never get darker), and the only
thing that makes structure visible is *density*. Ware, Purchase, Colpoys & McGill measured the
cognitive cost of exactly this kind of reading: in a controlled shortest-path experiment,
response time regressed on path length, path continuity ("bendiness"), number of crossings on
the path, and the number of branches at intermediate nodes; **each crossing on the path cost
about 0.65 s and 100° of bendiness about 1.7 s**, i.e. "the cognitive cost of a single crossing
is approximately the same as 38 degrees of continuity" (Ware et al. 2002). They also found that
Purchase's headline metric — *total* crossings in the drawing — was **not** a significant
predictor for path-tracing; what matters is crossings on the path of interest. Purchase's own
earlier study is the one that established edge crossings as "by far the most important
aesthetic" among bends, crossings, minimum angle, orthogonality and symmetry, over five
hand-drawn graph pairs (Purchase 1997). Huang & Hong extend this to the *angle* of crossings:
in their path-tracing study "task response time decreased as the crossing angle increased",
levelling off near 90°, and they conclude that "just minimizing the crossing number is not
sufficient … The angles of remaining crossings should be maximized as well"
(Huang & Hong 2008, abstract).
**[INFERENCE]** For this graph the actionable reading is: do not try to make 101k straight
edges individually legible; make the *path of interest* short, straight and crossing-free by
drawing it last and strongly, and let the rest be texture.

**Alpha / width / colour guidance.** Tufte's data-ink ratio — "the proportion of a graphic's
ink devoted to the non-redundant display of data-information" — is the standing argument for
erasing non-data ink (Tufte 1983, ch. 4). The WCAG numbers give hard floors for anything that
carries meaning: 3:1 for graphical objects required for understanding (W3C, SC 1.4.11) and, for
text, 4.5:1 normal / 3:1 large (W3C, SC 1.4.3). Measured against the current source, the
selected-edge stroke (1.98:1) and each of the three meaningful hues needs α ≈ 0.66–0.69 to
reach 3:1 at its present hue. Android's own rendering guidance states the mechanism behind the
cost: "Rendering transparent pixels on screen, known as *alpha rendering*, is a key contributor
to overdraw. Unlike standard overdraw … transparent objects require existing pixels to be
drawn first, so that the right blending equation can occur" (Android Developers, *Reduce
overdraw*).

**Alternatives, in increasing order of cost.**

- **Bundling.** Holten's hierarchical edge bundles bend each adjacency edge along the polyline
  through the hierarchy's least common ancestor, modelled as a piecewise cubic B-spline, with
  bundling strength β ∈ [0,1] trading low-level connectivity against high-level structure;
  it "reduces visual clutter when dealing with large numbers of adjacency edges" (Holten 2006).
  **[INFERENCE]** This graph *has* a natural hierarchy — era → decade — so hierarchical bundling
  is the one bundling variant that needs no new structure. Two cautions: Holten's own informal
  evaluation found the technique least pleasing on layouts with "a large number of collinear
  nodes … which causes bundles to overlap along the collinearity axes" (Holten 2006, §4.1),
  which is a precise description of era lanes; and an empirical study found bundling "negatively
  impacts user performance at tracing paths between nodes, both in terms of accuracy and time"
  (McGee & Dingliana 2012). Holten & van Wijk's force-directed bundling removes the hierarchy
  requirement but is an iterative self-organising process (Holten & van Wijk 2009);
  Wallinger et al.'s Edge-Path bundling clusters each edge along a weighted shortest path to
  "greatly reduc[e] ambiguities compared to previous bundling techniques" (Wallinger et al. 2021).
- **Hairball mitigation by subtraction rather than addition.** Hu & Shi's review of large-graph
  visualisation groups the view-transformation family — "edge filtering and bundling" — and
  reports that filtering edges by a centrality measure "significantly reduc[es] visual
  complexity" while keeping most topology, and that a minimum-spanning-tree backbone reveals
  structure "though many topology information are lost after the transformation"
  (Hu & Shi 2012, §4.4, citing Jia et al. 2008 and van Ham & Wattenberg 2008). Dwyer et al.'s
  edge compression attacks the same problem by aggregating bundles of parallel adjacency into
  single visual elements (Dwyer et al. 2013). **[INFERENCE]** Both are lossy in ways this repo
  must not be — the brief is explicit that every evidenced relationship stays in the field — so
  the applicable lesson is the *ordering* one: reduce the cost of the background by making it
  uniform texture, and spend the ink freed by that on the path.
- **Pre-rendered static layer.** MDN: "If you find yourself repeating some of the same drawing
  operations on each animation frame, consider offloading them to an offscreen canvas. You can
  then render the offscreen image to your primary canvas as often as needed" — with the
  documented caveat that the best case is "not [to] scale the canvas". A world-space lattice
  bitmap re-blitted with the current transform is the direct application.
- **WebGL instancing.** The WebGL 1.0 specification defines the canvas's `webgl` context
  (Khronos, *WebGL Specification* 1.0), and `ANGLE_instanced_arrays` adds an array divisor so
  "the same object, or groups of similar objects [can be drawn] multiple times, [if] they share
  the same vertex data" (Khronos, *ANGLE_instanced_arrays*). That is the shape of a 100k-edge
  renderer. **[INFERENCE]** It is also a full renderer rewrite in a repo whose rule is that
  graph logic is Rust, and it should be justified by a measurement of the current frame cost
  before it is considered.

## 3. Node rendering and colour

**What the code does.** Full-network nodes are filled circles; the radius is
`(5.0 * scale).clamp(1.3, 8.0)` for ordinary nodes and a fixed 5.0 px for selected/path nodes
(`browser.rs:281-296`). Colour is one of nine hardcoded decade buckets in
`["#64748b", "#8b5cf6", "#c05b9c", "#ca734b", "#bc9a2c", "#679a50", "#2d9f91", "#3880b6",
"#5a68bd"]`, indexed by `(year − 1940) / 10`, plus `#b04f00` for selected and `#176896` for path
nodes (`browser.rs:297-311`).

**Node-link vs alternatives.** Ghoniem, Fekete & Castagliola compared node-link diagrams with
adjacency matrices over seven tasks and three sizes, and found that "when graphs are bigger
than twenty vertices, the matrix-based visualization outperforms node-link diagrams on most
tasks. Only path finding is consistently in favor of node-link diagrams throughout the
evaluation" (Ghoniem et al. 2005). Their node-link diagrams "suffer from link overlapping —
interfering with neighborhood finding and link counting", and node finding by name degrades
badly with size because "nodes are not laid out in a predictive order." **[INFERENCE]** Two
readings for this repo: (a) the decision to stay node-link is right *because* the primary task
here is following a chain between two players, which is the one task node-link wins; (b) the
node-link weakness the study names — unpredictable position — is exactly what era lanes fix,
since debut year *is* a predictive order. A matrix view is therefore not worth adding.

**Categorical palettes.** ColorBrewer's design rules are the clearest statement of what the
current palette gets wrong: qualitative schemes "rely primarily on differences in hue … and
saturation and lightness are kept or nearly constant", and the authors explicitly warn that
mixing high-saturation and pastel colours "might imply order". They also note that
differentiating colours "is a harder perceptual task than seeing differences in simple and
logically ordered data legends" once linework and labels are overlaid, which is the case here,
and that as a rule of thumb cartographers seldom use more than ~7 classes (Harrower & Brewer
2003). The current nine colours vary in lightness without carrying order — the 1980s yellow
`#bc9a2c` is by far the lightest (relative luminance 0.340 vs 0.157–0.274 for the rest) — so it
reads as *more important* while also being the least legible on white (2.69:1). Okabe & Ito's
Color Universal Design answers the accessibility side with an eight-colour palette chosen so
that it is "unambiguous both to colorblinds and non-colorblinds", plus four rules: avoid the
red-on-dark pairing, do not convey information by colour alone, make marks thick or big, and
use vermilion/light red rather than pure red (Okabe & Ito 2008). WCAG 2.2 SC 1.4.1 requires the
same: "Color is not used as the only visual means of conveying information" (W3C, SC 1.4.1).
The perceptual literature behind both prescriptions is surveyed by Healey & Enns, who "discuss
theories of low-level visual perception, then show how these findings form a foundation for
more recent work on visual memory and visual attention", with the explicit aim of identifying
"results that have direct relevance to visualization and visual analytics" — the reason a
legend that requires comparing two distant colour patches, or a scheme that encodes category
by hue alone, is a weak instrument (Healey & Enns 2012, abstract).

**Node size by degree.** **[INFERENCE]** Sizing ordinary nodes by degree is *not* recommended
here: with mean degree 39.7 (2·101,395 / 5,106) the degree distribution is dominated by
tenure length, and radius is the weakest accurate channel — the repo already encodes more
useful information in position. Where degree *should* enter is in ForceAtlas2-style repulsion
("repulsion by degree" brings poorly connected nodes closer to hubs and reduces the visual
clutter of leaf forests; Jacomy et al. 2014), which is relevant only if a force layout is ever
adopted.

**Halo for the selected node.** **[INFERENCE]** A halo is a *redundant code* in Okabe & Ito's
sense and is cheaper than colour alone; MDN documents `shadowColor`/`shadowBlur` for it but
also warns to "Avoid the `shadowBlur` property whenever possible", so a two-pass approach —
draw a larger disc of the halo colour, then the node — is preferable to a blurred shadow.

## 4. Labels

**What the code does.** Every node re-sets `set_font("15px sans-serif")` and
`set_text_align("center")` and then draws its name with `fill_text` when
`!full_network || important || self.scale >= 0.7` (`browser.rs:300-321`). Nothing measures text
width and nothing checks for overlap.

**What the literature says about placement.** Christensen, Marks & Shieber established the
canonical model: a discrete set of candidate positions per point feature ranked by preference,
and an objective combining label–feature overlap, label–label overlap, placement preference,
and the number of unlabelled points; they prove the basic problem NP-hard and evaluate greedy
gradient descent against simulated annealing, recommending approaches "as a function of
available computation time" (Christensen, Marks & Shieber 1995). **[INFERENCE]** For a
real-time canvas the relevant conclusion is that only a greedy, bounded, per-frame heuristic is
affordable — which is what screen-space culling plus a small priority list is.

**Density and de-cluttering.** Perlin & Fox's Pad introduced semantic zooming: "when a text
document is small on the screen the user may only want to see its title. As the object is
magnified, this may be augmented by a short summary or outline" (Perlin & Fox 1993, §1.3),
implemented through per-object visibility ranges and transparency ranges ("each graphic can
have a range of magnification outside of which it is invisible … or transparent", ibid. §3.2).
Heer & Shneiderman describe the same effect in modern terms and note that semantic zooming
"can modify both the amount of information shown and how it is displayed" (Heer & Shneiderman
2012, "NAVIGATE"). **[INFERENCE]** The concrete scheme for this repo is a three-step ladder
driven by scale *and* priority: at all scales show selected/path/nearest-hovered names; above a
screen-space density threshold add names of nodes whose on-screen separation exceeds the
label's measured width; above a higher threshold show all.

**Font rendering cost and techniques.** MDN's optimization list includes "Avoid text rendering
whenever possible" and enumerates the levers that matter here: batch calls, "avoid unnecessary
canvas state changes", and "render screen differences only, not the whole new state". MDN
documents `measureText()` returning a `TextMetrics` object with the text width in the current
text style, and `strokeText()` as the stroking counterpart of `fillText()` on the same
baseline — which is the supported way to get an outline/halo (draw `strokeText` with a
wide light stroke, then `fillText`). `roundRect()` is available on both
`CanvasRenderingContext2D` and `Path2D` (WHATWG HTML §4.12.5.1.7; MDN *Path2D*), so rounded label
boxes need no path bookkeeping.

## 5. Interaction

**What the code does.** Pointer drag with pointer capture and a 5 px movement threshold; wheel
zoom anchored at the cursor (factor 1.15 per notch, `zoom()` at `browser.rs:52`); arrow keys pan
60 px, `+`/`-` zoom, `f` fits; buttons pan ×60 px / zoom ×1.25 / fit / focus; `<select>` picker;
click hit-test via the 80 px spatial hash. For context, Herman, Melançon & Marshall's survey is
the standard inventory of navigation techniques for graph visualisation — it covers the
zoom/pan/focus+context space this section draws from (Herman et al. 2000).

**Zoom-to-cursor is right and standard.** **[INFERENCE]** Keep it. The literature's live
questions here are step size, animation and control coupling:

- **Animation.** Cockburn et al. report that "animation is frequently used to help users
  assimilate the relationship between pre- and post-zoom states", that "research suggests values
  between 0.3 and 1.0 second are appropriate", and that animated zoom "helped users form a
  spatial model of their information space" (Bederson & Boltman 1999, cited ibid. §4.3, §7.2.3).
  Van Wijk & Nuij derive closed-form optimal pan-zoom trajectories from a metric on perceived
  velocity, `ds² = (1/w²)du² + (1/ρ²)dw²`, with an experimentally determined speed `V ≈ 0.90`
  (SD 0.43) and zoom/pan trade-off `ρ ≈ 1.42` (SD 0.47) over 26 participants, and note their
  formulation is stateless and can recompute the path every frame
  (van Wijk & Nuij 2004, §6, and the implementation note in §5). Both papers are directly
  applicable to the "fit-network" and "focus-player" buttons, which today jump instantly.
- **Double-click to focus / zoom.** Cockburn et al. document double-click-to-zoom as a
  discoverability problem: in an informal study of six experienced Word users, "none of them
  realized that double clicking with the left button is a zoom-in shortcut in Google Maps", and
  the reverse action was hard to find (ibid. §4.1). **[INFERENCE]** If double-click-to-focus is
  added it must be paired with a visible control and a keyboard equivalent.
- **Selection forms.** Heer & Shneiderman enumerate the recognised set: "mouse hover, mouse
  click, region selections (e.g., rectangular and elliptical regions, or free-form 'lassos'),
  and area cursors (e.g., 'brushes' … such as the bubble cursor, which selects the item
  currently closest to the mouse pointer)" (Heer & Shneiderman 2012, "SELECT"). The repo's
  click-nearest-node test is the simplest member of that family; a box/lasso selection over a
  1-D strip is a natural extension because the strip is only ~5,200 units tall — a rectangle is
  almost a 1-D interval query.
- **Hover tooltips.** Heer & Shneiderman warn that hover-driven details-on-demand "is
  inappropriate when using touch-based input on a tablet or mobile phone" — relevant because
  the canvas already sets `touch-action: none` (`style.css:94`).
- **Overview + detail.** See finding 2; the review's recommended coupling is one-way in the
  standard direction (browsing the overview must not move the detail; detail movement must be
  reflected in the overview) (Cockburn et al. 2009, §3.1).
- **Fisheye lenses.** Furnas' generalized fisheye replaces position/scale distortion with
  selection: "while an optical Fisheye Lens is all about distortion, the Furnas version was
  about selection — what is to be included (the more interesting stuff) in the view and what is
  to be left out" (Furnas 2006, "What vs. How"). The review is blunt about geometric fisheye
  distortion: it "is likely to damage the user's ability to correctly assess spatial
  properties such as directions, distances, and scales", and several studies show it harms
  target acquisition (Cockburn et al. 2009, §5.1 and §8). **[INFERENCE]** Use the DOI *selection*
  form, not a lens.
- **Follow-the-link navigation.** **Link Sliding** "allows users to slide along a link to its
  destination", and **Bring & Go** "brings all possible destinations within the user's view, and
  automatically transports" the user — both exploit the network's own edges as navigation
  affordances rather than treating panning as the only motion primitive (Moscovich et al. 2009).
  **[INFERENCE]** This is the most directly applicable graph-navigation idea for this repo: with
  a selected player, sliding along one of their edges to the far endpoint is a single gesture
  that keeps the mental map intact, and it is implementable on top of the existing
  `focus_player()` viewport math.
- **Keyboard navigation of the picker.** A native `<select>` already supports type-ahead and
  arrow keys; WCAG 2.2 SC 2.1.1 requires that "All functionality of the content is operable
  through a keyboard interface", and the repo's per-button listeners already discharge this for
  the view controls (W3C, SC 2.1.1; `graph_view.rs` `page()` builds the button row).

## 6. Focus + context while keeping whole-graph navigation

**Theory.** Furnas' degree of interest is the anchor: `DOI(x|y) = API(x) − D(x,y)`, where
`API(x)` is a priori interest and `D(x,y)` is spatial or semantic distance from the focus;
an interface "would present all points, x, that at least meet some minimal criterion c of
interestingness" (Furnas 1986, as restated by Furnas 2006 and by Cockburn et al. 2009, §5.1).
Furnas' 2006 follow-up argues the interesting part is *what* is shown rather than *how*: he
shows that concentric zooming yields "exactly a FE-DOI family of functions", so zoom and
fisheye are two presentations of the same subset. Heer & Shneiderman give the implementable
form: "The goal is to identify which data items are currently of high interest (focus), which
are of high importance regardless of the current focus (context), and which can be safely
removed from view" and describe exactly the technique this repo needs — "the distribution of
DOI scores can [be used] to selectively control the visibility of information", with
"low-interest items … elided but still accessible through aggregate representations"
(Heer & Shneiderman 2012, "NAVIGATE"). Van Ham & Perer's degree-of-interest *diffusion* from
focused nodes is the graph-specific instance (van Ham & Perer 2009); Gansner, Koren & North's
topological fisheye does the same with a precomputed multilevel coarsening tree
(Gansner et al. 2005). Shneiderman's widely cited visual-information-seeking mantra —
"overview first, zoom and filter, then details-on-demand" — is the task-level statement of the
same ordering, and is the reason a minimap plus tiered dimming is the right shape of change
here (Shneiderman 1996).

**Dimming vs hiding.** The empirical record for *hiding* or distorting non-neighbourhood items
is mixed and task-dependent. The review's own conclusion is that "distortion-oriented displays
are likely to impair the user's ability to make relative spatial judgments, and they can cause
target acquisition problems" (Cockburn et al. 2009, §8); §5.1 names those two problems as
"misinterpretation of the underlying data" and "challenges in target acquisition, both of which
are caused by distortion of the information space". Against that, in Pietriga et al.'s
multiscale-search comparison the overview+detail-with-pan-and-zoom condition was fastest, the
DragMag overview+detail lens next, the fisheye lens third, and plain pan-and-zoom last, while
Schaffer et al.'s graph-editing study found a continuous fisheye faster than full zooming
(ibid., §7.1.3, Table II). Kosara, Miksch & Hauser's semantic depth of field takes the opposite
tack to hiding — items failing the query are blurred, i.e. have their contrast reduced, and
remain present (Kosara et al. 2001). **[INFERENCE]** Blur is expensive on 2-D canvas; contrast
reduction is not, and it is what the repo's "draw everything faintly, then highlight" already
approximates. The measured problem is only that "faintly" is 1.11:1 and "highlight" is 1.98:1
(see findings 4–5).

**Concrete, implementable scheme.** **[INFERENCE]** Three tiers, driven by hop distance to the
selected player, matching the `distance` field's semantics (`Some(0)` = the player,
`Some(1)` = teammate, `Some(2)` = teammate-of-teammate):

| Tier | Membership | Node | Edges touching it | Label |
| --- | --- | --- | --- | --- |
| Focus | `id == selected` | `#b04f00`, r 6 px, 1.5 px light halo | — | always |
| 1-hop | `distance == 1` | decade colour, r 4 px | width 1.5, α ≥ 0.7 in `#b04f00` | on hover / when not overlapping |
| 2-hop | `distance == 2` | decade colour, α 0.55, r 3 px | not drawn (or α 0.12) | suppressed |
| Context | everything else | decade colour, α 0.25, r 1.3 px | current uniform wash | suppressed |

This satisfies the DOI rule (interest decreases with graph distance, not screen distance),
keeps the whole graph on screen (so the mental map of Misue et al. 1995 is preserved — no node
moves), and gives the reader an accurate count of what is being de-emphasised. The cost is one
server-side BFS, which `neighborhood()` already implements.

**Semantic zoom.** The existing `scale >= 0.7` label gate is already a two-level semantic
zoom; Perlin & Fox's mechanism is the generalisation to more levels via per-object visibility
and transparency ranges (Perlin & Fox 1993, §3.2), and Furnas & Bederson's space-scale diagrams
are the design notation for reasoning about the resulting scale/space trajectory
(Furnas & Bederson 1995). **[INFERENCE]** No new theory is needed — only more than one rung.

## 7. Performance in WASM + 2D canvas

**Free already.** Off-canvas segment rejection and drag-time lattice suppression
(`browser.rs:215-233`) are both implemented. The 80 px spatial hash for hit-testing
(`browser.rs` `start()`, `hit()` at `browser.rs:136`) avoids O(5,106) scans when zoomed in and
falls back to a full scan when the cell count exceeds the node count.

**Cheap wins, each with its owning source.**

- **DPR-aware sizing.** MDN documents the fix verbatim (see finding 7); the required APIs are
  `Window::device_pixel_ratio`, `HtmlCanvasElement::set_width` / `set_height`,
  `HtmlCanvasElement::get_bounding_client_rect`, and `CanvasRenderingContext2d`'s transform.
  The hit-testing transform in `pointer()` hardcodes `1000.0` and `600.0` (`browser.rs:101-106`),
  so this change must also introduce a single `logical_width`/`logical_height` pair used by
  `pointer()`, `draw()`, `fit()`, `fit_network()`, `focus_player()` and the wheel handler.
- **Offscreen pre-render of the static lattice.** MDN, "Pre-render similar primitives or
  repeating objects on an offscreen canvas". The W3C spec makes `Path2D` retainable and
  replayable ("gives you the convenience of being able to retain and replay your path whenever
  desired", MDN *Path2D*), and `OffscreenCanvas` is available both to the main thread and to
  workers (MDN *OffscreenCanvas*). `web-sys 0.3.106` exposes `Path2d`,
  `OffscreenCanvas`, and `OffscreenCanvasRenderingContext2d`.
- **Batch by colour and stop per-primitive state churn.** MDN: "Batch canvas calls together",
  "Avoid unnecessary canvas state changes", "Render screen differences only". Concretely the
  node loop currently re-issues `set_fill_style_str`, `set_font` and `set_text_align` per node
  (`browser.rs:281-312`); grouping nodes by colour and hoisting the constant text state out of
  the loop removes ~15k redundant calls per frame. Path grouping also matters for correctness
  of appearance: because one stroke unions overlaps, grouping edges of *different* colours into
  a single path would change what is painted, so batches must be per colour. The spec also
  documents the `alpha:false` context option ("When … alpha is false, then its alpha component
  must be fixed to 1.0 … the bitmap of such a context starts off as opaque black") and the
  `desynchronized` option, which "may optimize the rendering of the canvas to reduce the
  latency, as measured from input events to rasterization" (WHATWG HTML §4.12.5.1.2) — the
  latter is reachable through `HtmlCanvasElement::get_context_with_context_options`, whose
  options parameter is a plain `JsValue`.
- **requestAnimationFrame coalescing.** The current handlers call `draw()` synchronously inside
  every pointermove, wheel and key event. MDN documents `requestAnimationFrame` as the
  substitute for timer-driven painting, one-shot, paused in background tabs, and called "before
  the next repaint". **[INFERENCE]** Coalescing redraws into one rAF callback per frame both
  bounds work at the display refresh rate and lets the drag path skip the lattice more
  aggressively, since intermediate frames are never presented.
- **Latency budget.** Liu & Heer's controlled study is the sizing argument: "an additional
  delay of 500 ms incurs significant costs, decreasing user activity and data set coverage",
  and increased latency "reduces the rate at which users make observations, draw
  generalizations and generate hypotheses"; the effects on pan and zoom rates specifically were
  not significant, whereas brushing (a "perceptual fusion" act at the 100 ms band) was
  (Liu & Heer 2014, §4.2–4.3, §6.1). ForeCache's operational target for exploratory browsing is
  "extremely fast (*i.e.,* 500 ms or less)" (Battle, Chang & Stonebraker 2016). **[INFERENCE]**
  The practical budget for this repo: keep any single pan/zoom/select redraw under ~100 ms and
  never let a focus change require a server round-trip before the visual response.
- **Avoid per-frame text measurement storms.** If label overlap must be resolved, measure once
  per label per (font, text) pair and cache; MDN notes `measureText` is the only way to get the
  advance width in the current text style.

## 8. Accessibility and page chrome

**What exists.** `<canvas id="graph-canvas" tabindex="0" aria-label="Interactive teammate
graph" aria-describedby="graph-help graph-selection">` with fallback content, a
`<p id="graph-help">` describing drag/scroll/keys, `<p id="canvas-status" role="status">` that
`draw()` updates with the player/relationship counts and zoom percentage plus machine-readable
`data-*` attributes, and `<p id="graph-selection" role="status">` for the selected player
(`graph_view.rs` `page()`; `browser.rs:323-341`). `style.css` already defines
`:focus-visible` outlines of 3 px `#b04f00` for buttons, the canvas and the picker
(`style.css:97`).

**What is missing, with the owning requirement.**

- **A legend.** The nine decade colours are documented nowhere except the prose
  "colors mark eras". WCAG 2.2 SC 1.4.1 requires a non-colour means of conveying the
  information; Okabe & Ito's rule is stronger — "label elements of the graph on the graph
  itself rather than making a separate color-coded key, since matching same colors in distant
  places is extremely difficult" (Okabe & Ito 2008, "Most important practical points"). A legend
  is the minimum; per-band labels on the lane axis are better.
- **Status announcements are already correct and should not be regressed.** `role="status"`
  "is a form of live region" with "an implicit `aria-live` value of `polite` and an implicit
  `aria-atomic` value of `true`", and authors "SHOULD ensure an element with role status does
  not receive focus as a result of change in status" (WAI-ARIA 1.2, `status` role). **[INFERENCE]**
  The status string is rewritten on every frame today, including every pointermove; because
  `aria-atomic` is true the whole string is re-announced, so during a drag this is a
  screen-reader flood. Only update the live region when the *content* changes, and keep the
  `data-*` attributes updating freely.
- **Hit targets.** WCAG 2.2 SC 2.5.8 requires targets of at least 24×24 CSS px, with an
  exception when "the presentation of the target is essential", and names "an interactive data
  visualization where targets are necessarily dense" as an example of that exception (W3C,
  SC 2.5.8). The canvas therefore does not fail for small nodes, but the *page controls* must
  meet 24×24 — the current `.controls button` padding of `0.4rem 1rem` on a 1 rem base clears it
  comfortably (`style.css:44-51`). The canvas's own 22 px pick radius in non-full-network mode
  (`browser.rs:137`) is the right order of magnitude; in full-network mode 7 px is below the
  guideline and, under the essential exception, should be documented as a deliberate trade-off.
- **High-contrast / reduced-motion.** `prefers-contrast: more` is the standard hook for raising
  lattice and focus contrast (MDN, *prefers-contrast*), and `prefers-reduced-motion: reduce`
  should disable any animated pan/zoom transition added under §5 (MDN,
  *prefers-reduced-motion*).
- **A legend entry for state, not just era.** The selection colours `#b04f00` (selected) and
  `#176896` (path) carry meaning and pass contrast; they should appear in the same legend as
  the decades, labelled.
- **Zoom readout.** Already present in the status text ("Zoom X%") and in `data-zoom`.

## Recommended implementation order for this repo

Ranked so that each step is independently shippable and each is verifiable in the existing
browser smoke. "No new dependency" means usable with the pinned `web-sys 0.3.106` feature set,
extended at most by adding a feature name to `crates/canvas-view/Cargo.toml`.

### Tier 1 — no new dependencies, high value per line

1. **Degrees-of-separation dimming.** *Medium.* Populate `distance` for the full-network
   payload: `network()` in `crates/app-server/src/graph_view.rs:195` currently passes `None`
   for every node — reuse the BFS in `neighborhood()` (`graph_view.rs:95-108`) seeded at the
   selected player, capped at depth 2. Then read it in `CanvasView::draw()`
   (`browser.rs:209`) to pick per-tier node alpha/radius and per-tier edge styling; add a
   `selected_distance: Option<usize>` alongside `selected` to avoid per-node map lookups.
   Source: Furnas 1986; van Ham & Perer 2009; Heer & Shneiderman 2012.
2. **Raise the meaningful strokes to 3:1.** *Small.* In `draw()`, make the selected player's
   incident edges opaque `#b04f00` (or α ≥ 0.70) and widen the base lattice's distinction from
   the focus by *coverage* rather than alpha; keep the one-path union semantics intact
   (`browser.rs:236-252`). Source: WCAG 2.2 SC 1.4.11; computed 1.98:1 and 1.11:1 composites.
3. **A decade legend with swatches and state colours.** *Small.* Add the swatch markup in
   `page()` (`graph_view.rs:256`) and the rules to `crates/app-server/src/style.css`; the
   existing `.network-legend` paragraph is the natural anchor. Source: WCAG 2.2 SC 1.4.1;
   Okabe & Ito 2008.
4. **Replace the nine-colour palette with an ordered, colourblind-safe one and fix the
   `#bc9a2c` failure.** *Small.* Single array in `draw()` (`browser.rs:297-311`) plus the legend
   from step 3. Source: Okabe & Ito 2008; Harrower & Brewer 2003; WCAG 2.2 SC 1.4.11.
5. **DPR-aware canvas sizing.** *Small–medium.* In `start()` (`browser.rs:355`) size the bitmap
   from `get_bounding_client_rect()` and `device_pixel_ratio`, scale the context, and replace
   the hardcoded `1000.0`/`600.0` in `pointer()` (`browser.rs:101`), `draw()`'s clear rect
   (`browser.rs:211`), the cull bounds, `fit()`, `fit_network()` and `focus_player()` with one
   stored logical-size pair. Source: MDN *devicePixelRatio*; MDN *Optimizing canvas*.
6. **rAF-coalesced redraws.** *Small.* Replace the synchronous `v.draw()` calls in the pointer,
   wheel and key handlers (`browser.rs` `start()`) with a dirty flag plus a
   `requestAnimationFrame` callback. Source: MDN *requestAnimationFrame*; MDN *Optimizing
   canvas*; Liu & Heer 2014.
7. **Label policy: nearest/selected only, measured once.** *Medium.* In the label branch
   (`browser.rs:314`) replace `self.scale >= 0.7` with (a) always label selected/path, (b) label
   the hovered node, (c) label others only when their screen separation exceeds a cached
   `measureText` width at the current scale. Source: Christensen et al. 1995; Perlin & Fox 1993;
   MDN *Drawing text*.
8. **Stop rewriting the live region every frame.** *Small.* In `draw()` (`browser.rs:323`)
   only call `set_text_content` when the formatted string differs from the last one; keep the
   `data-*` attributes unconditional. Source: WAI-ARIA 1.2 `status` role.

### Tier 2 — no new dependencies, medium effort

9. **Minimap (overview + detail).** *Medium.* A second, small canvas element rendered in
   `page()` (`graph_view.rs:256`) and drawn from the same `CanvasView`; one-way coupling —
   detail movement redraws the overview viewport rectangle, dragging the rectangle pans the
   detail. Source: Cockburn et al. 2009, §3.1, §8; MDN *Optimizing canvas* (layered canvases).
10. **Animated fit and focus transitions.** *Medium.* Replace the instant jumps in
    `fit_network()` / `focus_player()` (`browser.rs:61,92`) with the closed-form trajectory of
    van Wijk & Nuij (ρ ≈ 1.4, V ≈ 0.9), driven by the rAF loop from step 6, gated on
    `prefers-reduced-motion`. Source: van Wijk & Nuij 2004; Cockburn et al. 2009 §4.3 (0.3–1.0 s);
    MDN *prefers-reduced-motion*.
11. **Offscreen static lattice layer.** *Medium.* Cache the world-space base lattice into a
    `Path2d` (or an `OffscreenCanvas` blitted with `drawImage`) invalidated only on view change;
    `Path2d` and `OffscreenCanvas` are already in the pinned `web-sys`. Source: WHATWG HTML
    §4.12.5.1.10 (stroke union — the layer is safe to cache because it is view-independent);
    MDN *OffscreenCanvas*; MDN *Path2D*.
12. **Batch the node pass by colour, hoist text state.** *Small.* `draw()` (`browser.rs:281`)
    currently issues ~5 state changes per node. Source: MDN *Optimizing canvas*.
13. **Within-lane ordering to shorten edges.** *Medium.* In `start()` (`browser.rs:355`),
    replace the hash-derived lane order with a barycentre pass over the previous era band.
    Source: Sugiyama et al. 1981; Ware et al. 2002 (crossings and continuity dominate).
14. **Hover tooltip and box selection.** *Medium.* Extend `hit()` (`browser.rs:136`) with a
    nearest-node-on-move query and label it; add a drag-with-modifier rectangle query over the
    1-D strip. Source: Heer & Shneiderman 2012 ("SELECT"); WCAG 2.2 SC 2.1.1 for the keyboard
    equivalent.
15. **Zoom-step refinement and inertia.** *Small.* `zoom()` (`browser.rs:52`) currently applies
    a fixed 1.15 factor per wheel notch and clamps to 4.0. Source: van Wijk & Nuij 2004 for the
    trajectory; Liu & Heer 2014 for why pan/zoom latency is the *least* sensitive operation.

### Tier 3 — would add a dependency; justify by measurement first

16. **WebGL edge renderer.** *Large, new dependency.* Instanced line drawing via
    `ANGLE_instanced_arrays` (Khronos) or WebGL 2 instancing; `CanvasRenderingContext2d`'s
    `webgl` context is defined by the Khronos WebGL 1.0 specification. This is a renderer
    rewrite in Rust through a new binding layer, not a JavaScript graph library, so it does not
    violate `README.md:29` — but it is the only recommendation here that changes the rendering
    architecture. Adopt only if measurements after Tier 1–2 show the 101k-segment stroke
    dominating the frame. Source: Khronos, *WebGL Specification*; Khronos,
    *ANGLE_instanced_arrays*; Holten & van Wijk 2009 for the alternative (bundling) that stays
    in 2D.
17. **Edge bundling.** *Large, no new dependency but a new algorithm.* Hierarchical bundling
    over the era→decade hierarchy (Holten 2006) is the variant that needs no new structure;
    note Holten's own finding that collinear layouts make bundles overlap along the
    collinearity axes — the exact geometry of era lanes — and McGee & Dingliana's empirical
    result that bundling harms path tracing. Source: Holten 2006; McGee & Dingliana 2012;
    Wallinger et al. 2021.
18. **A precomputed force-directed or multilevel layout.** *Large, offline only.* If attempted,
    it must be computed server-side with a fixed seed (D3 documents `simulation.randomSource()`
    defaulting to a fixed-seed LCG, and `simulation.tick()` for manual stepping) and shipped as
    coordinates, because the classical algorithms are quadratic per iteration and terminate by
    guesswork. Source: Fruchterman & Reingold 1991; Barnes & Hut 1986; Kobourov 2012;
    Jacomy et al. 2014; D3 *Force simulations*; D3 *Many-body force*.

### Things deliberately *not* recommended

- **A matrix view.** It wins most tasks above ~20 vertices, but the one task it loses —
  path finding — is this application's core task (Ghoniem et al. 2005).
- **A geometric fisheye lens.** It damages relative spatial judgements and target acquisition;
  the same DOI subset is achievable by selection (Furnas 2006; Cockburn et al. 2009, §8).
- **Node size by degree.** With mean degree ~40 it mostly re-encodes tenure length and spends
  the weakest channel on redundant information (see §3).

## References

Citations are given with the identifier the source owns; DOIs were verified via Crossref on
2026-10-14.

**Papers**

- Barnes, J. & Hut, P. (1986). A hierarchical O(N log N) force-calculation algorithm.
  *Nature* 324, 446–449. doi:10.1038/324446a0
- Battle, L., Chang, R. & Stonebraker, M. (2016). Dynamic Prefetching of Data Tiles for
  Interactive Visualization. *SIGMOD '16*. doi:10.1145/2882903.2882919
- Christensen, J., Marks, J. & Shieber, S. (1995). An empirical study of algorithms for
  point-feature label placement. *ACM Transactions on Graphics* 14(3), 203–232.
  doi:10.1145/212332.212334
- Cockburn, A., Karlson, A. & Bederson, B. B. (2009). A review of overview+detail, zooming, and
  focus+context interfaces. *ACM Computing Surveys* 41(1), Article 2. doi:10.1145/1456650.1456652
- Dwyer, T., Riche, N. H., Marriott, K. & Mears, C. (2013). Edge Compression Techniques for
  Visualization of Dense Directed Graphs. *IEEE TVCG* 19(12), 2596–2605.
  doi:10.1109/tvcg.2013.151 (cited as the modern alternative to bundling for dense graphs)
- Eades, P. (1984). A heuristic for graph drawing. *Congressus Numerantium* 42, 149–160.
  (No DOI; quoted through Kobourov 2012 and Fruchterman & Reingold 1991.)
- Fruchterman, T. M. J. & Reingold, E. M. (1991). Graph drawing by force-directed placement.
  *Software—Practice and Experience* 21(11), 1129–1164. doi:10.1002/spe.4380211102
- Furnas, G. W. (1986). Generalized fisheye views. *CHI '86*, 16–23. doi:10.1145/22339.22342
- Furnas, G. W. (2006). A fisheye follow-up: further reflections on focus + context.
  *CHI '06*, 999–1008. doi:10.1145/1124772.1124921
- Furnas, G. W. & Bederson, B. B. (1995). Space-scale diagrams: understanding multiscale
  interfaces. *CHI '95*, 234–241. doi:10.1145/223904.223934
- Gansner, E. R., Koren, Y. & North, S. (2005). Topological Fisheye Views for Visualizing Large
  Graphs. *IEEE TVCG* 11(4), 457–468. doi:10.1109/tvcg.2005.66
- Ghoniem, M., Fekete, J.-D. & Castagliola, P. (2005). On the readability of graphs using
  node-link and matrix-based representations: a controlled experiment and statistical analysis.
  *Information Visualization* 4(2), 114–135. doi:10.1057/palgrave.ivs.9500092
  (conference version doi:10.1109/infvis.2004.1)
- Harrower, M. & Brewer, C. A. (2003). ColorBrewer.org: An Online Tool for Selecting Colour
  Schemes for Maps. *The Cartographic Journal* 40(1), 27–37. doi:10.1179/000870403235002042
- Healey, C. G. & Enns, J. T. (2012). Attention and Visual Memory in Visualization and
  Computer Graphics. *IEEE TVCG* 18(7), 1170–1188. doi:10.1109/tvcg.2011.127
- Heer, J. & Shneiderman, B. (2012). Interactive Dynamics for Visual Analysis.
  *Communications of the ACM* 55(4). doi:10.1145/2133806.2133821
- Herman, I., Melançon, G. & Marshall, M. S. (2000). Graph visualization and navigation in
  information visualization: A survey. *IEEE TVCG* 6(1), 24–43. doi:10.1109/2945.841119
- Holten, D. (2006). Hierarchical Edge Bundles: Visualization of Adjacency Relations in
  Hierarchical Data. *IEEE TVCG* 12(5), 741–748. doi:10.1109/TVCG.2006.147
- Holten, D. & van Wijk, J. J. (2009). Force-Directed Edge Bundling for Graph Visualization.
  *Computer Graphics Forum* 28(3), 983–990. doi:10.1111/j.1467-8659.2009.01450.x
- Holten, D. & van Wijk, J. J. (2009). A user study on visualizing directed edges in graphs.
  *CHI '09*. doi:10.1145/1518701.1519054
- Huang, W. & Hong, S.-H. (2008). Effects of Crossing Angles. *IEEE PacificVis 2008*.
  doi:10.1109/pacificvis.2008.4475457
- Jacomy, M., Venturini, T., Heymann, S. & Bastian, M. (2014). ForceAtlas2, a Continuous Graph
  Layout Algorithm for Handy Network Visualization Designed for the Gephi Software.
  *PLoS ONE* 9(6): e98679. doi:10.1371/journal.pone.0098679
- Kobourov, S. G. (2012). Spring Embedders and Force Directed Graph Drawing Algorithms.
  arXiv:1201.3011
- Kosara, R., Miksch, S. & Hauser, H. (2001). Semantic Depth of Field. *IEEE InfoVis 2001*,
  97–104. doi:10.1109/infvis.2001.963286
- Liu, Z. & Heer, J. (2014). The Effects of Interactive Latency on Exploratory Visual Analysis.
  *IEEE TVCG* 20(12) (Proc. InfoVis 2014). doi:10.1109/tvcg.2014.2346452
- McGee, F. & Dingliana, J. (2012). An empirical study on the impact of edge bundling on user
  comprehension of graphs. *AVI '12*. doi:10.1145/2254556.2254670
- Misue, K., Eades, P., Lai, W. & Sugiyama, K. (1995). Layout Adjustment and the Mental Map.
  *Journal of Visual Languages and Computing* 6(2), 183–210. doi:10.1006/jvlc.1995.1010
- Moscovich, T., Chevalier, F., Henry, N., Pietriga, E. & Fekete, J.-D. (2009). Topology-aware
  navigation in large networks. *CHI '09*. doi:10.1145/1518701.1519056
- Noack, A. (2007). Energy Models for Graph Clustering. *Journal of Graph Algorithms and
  Applications* 11(2), 453–480. doi:10.7155/jgaa.00154
- Okabe, M. & Ito, K. (2008). *Color Universal Design (CUD): How to make figures and
  presentations that are friendly to colorblind people.* https://jfly.uni-koeln.de/color/
  (also published as: Color universal design: the selection of four easily distinguishable
  colors for all color vision types, doi:10.1117/12.765420)
- Perlin, K. & Fox, D. (1993). Pad: An Alternative Approach to the Computer Interface.
  *SIGGRAPH '93*, 57–64. doi:10.1145/166117.166125
- Purchase, H. C. (1997). Which aesthetic has the greatest effect on human understanding?
  *Graph Drawing (GD '97)*, LNCS 1353, 248–261. doi:10.1007/3-540-63938-1_67
- Shneiderman, B. (1996). The Eyes Have It: A Task by Data Type Taxonomy for Information
  Visualizations. *IEEE Symposium on Visual Languages*, 336–343. doi:10.1109/vl.1996.545307
- Sugiyama, K., Tagawa, S. & Toda, M. (1981). Methods for Visual Understanding of Hierarchical
  System Structures. *IEEE Trans. Systems, Man, and Cybernetics* SMC-11(2), 109–125.
  doi:10.1109/tsmc.1981.4308636
- Tufte, E. R. (1983). *The Visual Display of Quantitative Information.* Graphics Press.
  (Data-ink ratio, ch. 4; no DOI.)
- van Ham, F. & Perer, A. (2009). "Search, Show Context, Expand on Demand": Supporting Large
  Graph Exploration with Degree-of-Interest. *IEEE TVCG* 15(6), 953–960.
  doi:10.1109/tvcg.2009.108
- van Wijk, J. J. & Nuij, W. A. A. (2004). Smooth and Efficient Zooming and Panning.
  *IEEE InfoVis 2003*, 15–22. doi:10.1109/infvis.2003.1249004
- Wallinger, M., Archambault, D., Auber, D., Nöllenburg, M. & Peltonen, J. (2021).
  Edge-Path Bundling: A Less Ambiguous Edge Bundling Approach. arXiv:2108.05467
- Ware, C., Purchase, H., Colpoys, L. & McGill, M. (2002). Cognitive Measurements of Graph
  Aesthetics. *Information Visualization* 1(2), 103–110. doi:10.1057/palgrave.ivs.9500013
- Ware, C. (2012). *Information Visualization: Perception for Design* (3rd ed.). Morgan
  Kaufmann. (Preattentive processing, ch. 5; no DOI.)

**Specifications and official documentation**

- WHATWG. *HTML Living Standard*, §4.12.5 The `canvas` element — including the stroke steps
  ("overlapping parts of the paths in one stroke operation are treated as if their union was
  what was painted"), the canvas settings (`alpha`, `desynchronized`), `Path2D`, `roundRect`,
  and text styles. https://html.spec.whatwg.org/multipage/canvas.html (read 2026-10-14)
- W3C. *Understanding Success Criterion 1.4.3: Contrast (Minimum)* (WCAG 2.2).
  https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
- W3C. *Understanding Success Criterion 1.4.11: Non-text Contrast* (WCAG 2.2).
  https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
- W3C. *Understanding Success Criterion 1.4.1: Use of Color* (WCAG 2.2).
  https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html
- W3C. *Understanding Success Criterion 2.1.1: Keyboard* (WCAG 2.2).
  https://www.w3.org/WAI/WCAG22/Understanding/keyboard.html
- W3C. *Understanding Success Criterion 2.5.8: Target Size (Minimum)* (WCAG 2.2).
  https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html
- W3C. *Accessible Rich Internet Applications (WAI-ARIA) 1.2*, `status` role.
  https://www.w3.org/TR/wai-aria-1.2/#status
- MDN Web Docs. *Optimizing canvas.*
  https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Optimizing_canvas
- MDN Web Docs. *OffscreenCanvas.*
  https://developer.mozilla.org/en-US/docs/Web/API/OffscreenCanvas
- MDN Web Docs. *Path2D.* https://developer.mozilla.org/en-US/docs/Web/API/Path2D
- MDN Web Docs. *Window: devicePixelRatio property.*
  https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio
- MDN Web Docs. *Window: requestAnimationFrame() method.*
  https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame
- MDN Web Docs. *Drawing text / measureText / strokeText* (Canvas tutorial).
  https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Drawing_text
- MDN Web Docs. *CanvasRenderingContext2D: drawImage()* and *shadowBlur.*
  https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage
- MDN Web Docs. *prefers-contrast* and *prefers-reduced-motion* CSS media features.
- MDN Web Docs. *ARIA: aria-live attribute.*
  https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Reference/Attributes/aria-live
- Khronos Group. *WebGL Specification*, Version 1.0.3.
  https://registry.khronos.org/webgl/specs/1.0
- Khronos Group. *ANGLE_instanced_arrays*, WebGL ratified extension.
  https://registry.khronos.org/webgl/extensions/ANGLE_instanced_arrays
- Android Developers. *Reduce overdraw.*
  https://developer.android.com/topic/performance/rendering/overdraw

**First-party library documentation (reference for techniques, not proposed dependencies)**

- D3. *Force simulations* (velocity Verlet integrator, `alpha`/`alphaDecay`, `tick()`,
  `randomSource()` fixed-seed LCG, `fx`/`fy` fixed positions).
  https://d3js.org/d3-force/simulation
- D3. *Many-body force* (quadtree + Barnes–Hut, `O(n log n)` per application, `theta`,
  `distanceMin`, `distanceMax`). https://d3js.org/d3-force/many-body
- vis-network. *Physics* (barnesHut / forceAtlas2Based solvers, stabilisation parameters).
  https://visjs.github.io/vis-network/docs/network/physics.html
- Cytoscape.js. *Performance* and *Styles / Labels* sections. https://js.cytoscape.org/
- web-sys 0.3.106 API surface verified on docs.rs and in the wasm-bindgen source tree:
  `Path2d`, `OffscreenCanvas`, `OffscreenCanvasRenderingContext2d`,
  `ImageBitmapRenderingContext`, `HtmlCanvasElement::get_context_with_context_options`
  (options as `JsValue`), `CanvasRenderingContext2d::{set_global_alpha, set_line_join,
  set_text_baseline, fill_text, stroke_text, measure_text, begin_path, fill, stroke}`.
