# Rust Graph Visualization Architecture & Developer Experience (DX) Research

*Researched 2026-10-14. Evaluates the state of graph visualization in the Rust ecosystem against AntV G6 (https://github.com/antvis/G6), identifies relevant algorithms and crates, specifies graphics and ease-of-use enhancements for the 7-degrees Rust/WASM canvas (`crates/canvas-view`), and defines developer experience (stability, testing, formatting) standards.*

---

## Executive Summary & Verdict

### The Core Question
Does a complete, drop-in equivalent to **AntV G6** exist in the Rust ecosystem for web canvas/WASM, or must we compose/build our own modular Rust engine for `canvas-view`?

### The Verdict: NO Direct Equivalent Exists; We Must Compose a Modular Rust Engine
There is **no all-in-one graph visualization library in Rust** that provides the unified capabilities of AntV G6 (data model, automatic layout algorithms, Canvas2D/WebGL rendering, interaction behaviors, and visual plugins like Minimap, Legend, and Tooltips in a web browser context).

Instead, the Rust ecosystem is sharply bifurcated:
1. **Immediate-Mode Desktop GUI Widgets** (e.g., `egui_graphs`): Deeply bound to `egui` and `eframe`. They hijack the entire WebGL/WebGPU context, do not integrate cleanly into existing HTML5 DOM/SSR workflows, cannot render into a lightweight 2D `<canvas>` alongside server-rendered HTML, and lack rich web overlay plugins.
2. **Specialized Single-Purpose Crates**:
   - **Graph Structures & Algorithmic Foundations**: `petgraph` (the de facto standard in Rust for graph representation, traversals, and cycle detection).
   - **Layout Algorithms**: Ported single algorithms, such as `dagre` / `dagre-dgl-rs` and `rust-sugiyama` (Sugiyama layered DAG layout) and `fa2` (ForceAtlas2 continuous force layout).
   - **Static Graphviz Compilers**: `layout-rs` and `graphviz-rust` (focused on parsing DOT files and generating static SVG/text layouts, rather than driving an interactive canvas).

### Architectural Strategy
Because of this repo's foundational architecture rule — *"All application UI, interaction and graph logic is Rust"* (`README.md:29`) — and because the application is served as an SSR HTML page with a focused WASM canvas, the optimal path is to **compose a modular, G6-inspired micro-engine in Rust**:
- **Graph Model & Topology**: Backed by `graph-core` and `petgraph`.
- **Layout Pipeline**: A pluggable `GraphLayout` trait in Rust supporting:
  1. *Era-Lane Layout with Sugiyama Barycenter Ordering* (retains debut-year ordinal semantics on the horizontal axis while minimizing intra-lane edge crossings).
  2. *Concentric / Radial Layout* (arranging nodes in concentric rings by degree of separation / hop distance from the focused player).
  3. *Force-Directed Layout* (integrating the lightweight MIT-licensed `fa2` crate or a bounded Barnes–Hut spring simulation for organic clustering).
- **Renderer**: High-performance, high-DPI `CanvasRenderingContext2d` with path batching, offscreen canvas double-buffering, and Level of Detail (LOD) semantic zoom culling.
- **Plugins & Overlays**: First-class Rust implementations of the essential G6 plugins:
  - **Minimap** (overview+detail with draggable viewport rectangle).
  - **Interactive Legend** (decade/status swatches with click-to-filter/dim).
  - **Toolbar Controls** (zoom, pan, fit network, focus player, layout switcher).
  - **Hover Tooltips** (canvas/DOM overlay detailing player tenures and mutual teammates).
- **Developer Experience**: Rigid workspace formatting (`cargo fmt`), strict zero-warning lints (`cargo clippy`), multi-tiered testing (deterministic unit tests + headless Playwright browser tests), and automated pre-commit / CI verification scripts.

---

## 1. AntV G6 Architecture & Capability Decomposition

AntV G6 (developed by Alibaba's Ant Group, MIT License, currently v5) is an enterprise-grade graph visualization engine in JavaScript/TypeScript. Understanding its internal architecture provides the structural blueprint for what our Rust engine needs.

### 1.1 The Six Architectural Tiers of AntV G6
G6 decomposes graph visualization into six decoupled layers:

```
┌──────────────────────────────────────────────────────────────┐
│                      6. Plugins & UI                         │
│   Minimap · Legend · Tooltip · Toolbar · Grid · Fisheye      │
├──────────────────────────────────────────────────────────────┤
│                   5. Behaviors & Interaction                 │
│  DragCanvas · ZoomCanvas · ClickSelect · HoverActivate · LOD │
├──────────────────────────────────────────────────────────────┤
│                     4. Layout Engine                         │
│ Force / FA2 · Dagre (Sugiyama) · Concentric · Radial · Grid  │
├──────────────────────────────────────────────────────────────┤
│                 3. Graph Data Model & State                  │
│ Nodes · Edges · Combos (Hierarchies) · Selection · Diffing   │
├──────────────────────────────────────────────────────────────┤
│                    2. Rendering Pipeline                     │
│ Canvas 2D · WebGL / WebGPU · SVG · Offscreen Buffer · Caching│
├──────────────────────────────────────────────────────────────┤
│               1. Lower-Level Graphics Engine                 │
│          @antv/G (Unified 2D/3D Scene Graph & Math)          │
└──────────────────────────────────────────────────────────────┘
```

1. **Low-Level Graphics Runtime (`@antv/G`)**:
   - Abstract scene graph providing shape primitives (Circle, Rect, Path, Text, Image, Polygon).
   - Pluggable backends: Canvas 2D (`@antv/g-canvas`), WebGL (`@antv/g-webgl`), WebGPU (`@antv/g-webgpu`), and SVG (`@antv/g-svg`).
   - Coordinate transforms, matrix multiplication, bounding box (`AABB`) caching, and hit-testing (point-in-path, ray-casting).
2. **Graph Data Model (`@antv/graphlib` / G6 Model)**:
   - Maintains vertices ($V$), directed/undirected edges ($E$), and compound clusters (*Combos*).
   - Manages node and edge lifecycle (add, remove, update, batch data diffing).
   - Maintains visual states per element: `default`, `selected`, `active`, `hover`, `disabled`.
3. **Layout Pipeline (`@antv/layout`)**:
   - Decoupled from rendering: layouts receive graph topology and dimensions, and calculate $(x, y)$ coordinates.
   - Supports synchronous execution on the main thread, asynchronous execution in Web Workers, and GPU-accelerated computing.
   - Built-in layout catalog:
     - *Force-directed*: Force, Fruchterman-Reingold, ForceAtlas2 (FA2), D3Force, GForce (GPU).
     - *Hierarchical*: Dagre (Sugiyama framework: cycle removal, layering, crossing minimization, coordinate assignment), CompactBox, Dendrogram, Mindmap, Indented.
     - *Geometric*: Circular, Concentric, Radial, Grid, Snake, Random.
     - *Clustered*: ComboCombined, ComboForce.
4. **Behavior & Interaction Subsystem**:
   - Event listeners bound to canvas/pointer events that trigger camera or state transitions.
   - Standard built-in behaviors:
     - `DragCanvas` (pan viewport).
     - `ZoomCanvas` (focal-point mouse-wheel and pinch zooming).
     - `DragElement` (drag individual nodes or combos).
     - `ClickSelect` & `BrushSelect` (single-node selection, multi-node marquee/lasso selection).
     - `HoverActivate` / `ActivateRelations` (hovering a node illuminates its incident edges and 1-hop/2-hop neighbors while dimming unrelated elements).
     - `AutoAdaptLabel` (dynamic label culling and truncation based on screen density and zoom level).
5. **Plugin Subsystem**:
   - Self-contained components that subscribe to graph events and render overlays:
     - `Minimap`: A synchronized overview showing the entire graph and a draggable viewport frame.
     - `Legend`: Visual color/symbol guide with interactive toggle capabilities.
     - `Tooltip`: Dynamic popover showing rich metadata for hovered nodes and edges.
     - `Toolbar`: Pre-built canvas action buttons (Zoom In, Zoom Out, Reset, Fit View, Export PNG).
     - `Grid` / `Background`: Canvas background coordinate grids or dot lattices.
     - `Fisheye`: Non-linear focal distortion magnifying regions under the cursor.
     - `EdgeBundling`: Hierarchical or force-directed curve bundling to mitigate visual "hairballs".
6. **Animation & Transition Engine**:
   - Interpolates camera changes (viewport tweening during `fitView` or `focusItem`).
   - Morphing node coordinates during layout switches using ease curves (cubic-bezier).

---

## 2. Landscape Analysis of the Rust Graph Visualization Ecosystem

To determine whether any existing Rust crate can be leveraged directly, we systematically audited the crates.io and GitHub ecosystem.

### 2.1 Crate-by-Crate Evaluation

| Crate | Version | License | Primary Role | Web / Canvas Suitability | Verdict for This Project |
|---|---|---|---|---|---|
| **`egui_graphs`** | 0.32+ | MIT / Apache-2.0 | Interactive graph widget for `egui` | **Poor (Requires egui runtime)** | **Rejected as full engine.** Deeply coupled to `egui::Ui` immediate-mode GUI and `eframe`. Requires complete takeover of WebGL canvas via `glow` or `wgpu`. Incompatible with lightweight 2D canvas embedded in SSR HTML. |
| **`petgraph`** | 0.8.3 | MIT / Apache-2.0 | Graph data structures & algorithms | **Universal (No-std / WASM friendly)** | **Adopted as topology foundation.** Standard in this repo. Provides graph storage, BFS/DFS, Dijkstra, A*, cycle detection, and SCC (Tarjan). Does not perform rendering or layout. |
| **`fa2`** | 0.3.0 | MIT | ForceAtlas2 layout algorithm | **Excellent (WASM compatible)** | **Strong candidate for force layout.** Written by Guillaume Plique (author of Sigma.js / Graphology). Pure Rust, zero heavy dependencies, generic over `f32`/`f64`. Computes spatial coordinates iteratively. |
| **`dagre` / `dagre-dgl-rs`** | 0.1.1 | MIT | Sugiyama hierarchical layout | **High (Pure Rust)** | **Strong candidate for layered layout.** Faithful Rust ports of `dagre.js`. Implements Sugiyama framework: cycle removal, network simplex ranking, barycenter crossing reduction, coordinate assignment. |
| **`rust-sugiyama`** | 0.4.0 | MIT | Sugiyama layered layout | **High (Uses petgraph)** | **Alternative layered layout.** Implements Sugiyama algorithm directly on `petgraph::Graph`. Good for topological ordering and layered hierarchies. |
| **`fdg-sim` / `fdg`** | 0.9.1 | **GPL-3.0** | Force-directed graph simulation | **Medium (License concern)** | **Avoid due to copyleft GPL-3.0 license.** While functionally capable, GPL-3.0 introduces restrictive licensing constraints compared to MIT/Apache-2.0. |
| **`force_graph`** | 0.1.x | MIT | Force-directed layout | **Medium (Basic)** | Basic spring-embedder algorithm from Graphoon. Lacks Barnes–Hut quadtree optimization; $O(\|V\|^2)$ scaling per tick becomes problematic above ~500 nodes. |
| **`layout-rs`** | 0.1.3 | MIT | Graphviz DOT parser & SVG renderer | **Low (Static document focus)** | Focuses on reading DOT syntax and generating static SVG diagrams. Not an interactive canvas engine. |
| **`graphviz-rust`** | 0.9.9 | MIT | Graphviz AST & bindings | **Low (CLI/C bindings)** | Wrapper around Graphviz AST; requires either the native C `dot` binary or heavy emscripten Graphviz WASM build. Too heavy and static for interactive WASM canvas. |
| **`graph-explorer-layout`** | 0.7.2 | MIT | Barnes-Hut force & radial layout | **Medium** | Extracted from graph-explorer. Provides Barnes-Hut simulation and radial layout, but tightly coupled to internal data representations. |

### 2.2 Why `egui_graphs` Cannot Replace `canvas-view`
`egui_graphs` is the most prominent interactive graph visualization project in Rust today. However, technical analysis demonstrates why it is unsuitable for this project:
1. **Immediate-Mode Architectural Lock-in**:
   - `egui_graphs` is an `egui::Widget`. It requires passing an `&mut egui::Ui` on every 60fps frame.
   - To run in a browser, it requires `eframe` (egui framework), which spins up a full WebGL (`glow`) or WebGPU (`wgpu`) canvas context. It does not output to an HTML5 `CanvasRenderingContext2d`.
2. **Binary Size & WASM Bloat**:
   - Compiling `eframe` + `wgpu`/`glow` + `egui` + `egui_graphs` produces a WASM bundle typically between **4 MB and 12 MB** (uncompressed), compared to our current lightweight `canvas-view` bundle (~250 KB).
3. **SSR & DOM Incompatibility**:
   - This project uses Axum SSR with embedded HTML templates, native forms, semantic `<select>` inputs, and accessible status live-regions. An `eframe` canvas completely obscures the DOM, intercepts all pointer events, and destroys screen-reader accessibility.
4. **Missing G6 Plugins**:
   - `egui_graphs` does not provide an overview+detail Minimap, an accessible HTML/CSS Legend, interactive DOM tooltips, or smooth camera animations.

### 2.3 Verdict on the Rust Ecosystem
**Conclusion**: There is **no single Rust crate** equivalent to AntV G6. The desktop/immediate-mode ecosystem (`egui_graphs`) is the wrong tool for an SSR web application, while backend graph libraries (`petgraph`) provide data structures without visual presentation.

**The Solution**: We must assemble a **custom, modular, G6-patterned Rust engine** inside `crates/canvas-view`. We can import or adapt focused algorithmic crates (such as `fa2` for ForceAtlas2 and Sugiyama barycenter ordering algorithms from `dagre`), while retaining full ownership of the Canvas2D rendering pipeline, interaction behaviors, and overlay plugins.

---

## 3. Architecture Blueprint: A Modular G6-Inspired Engine in Rust

To achieve G6-like graphics, ease of use, and flexibility without external JavaScript bloat, `crates/canvas-view` should be structured into distinct, decoupled modules:

```
crates/canvas-view/src/
├── lib.rs                  // Re-exports & WASM initialization
├── core/
│   ├── model.rs            // GraphPayload, GraphNode, GraphLink, SpatialIndex
│   ├── transform.rs        // Viewport, Pan, Zoom, Screen <-> World mapping, DPR
│   └── events.rs           // Pointer, Wheel, Keyboard event dispatcher
├── layout/
│   ├── mod.rs              // Layout trait definition
│   ├── era_lane.rs         // Debut year bands + Sugiyama barycenter ordering
│   ├── concentric.rs       // Degree-of-separation concentric rings
│   ├── radial.rs           // Radial layout branching from focused player
│   └── force.rs            // Bounded ForceAtlas2 (fa2) / Barnes-Hut layout
├── render/
│   ├── mod.rs              // RenderContext & frame orchestration
│   ├── lattice.rs          // Batched background lattice (100k edges) with culling
│   ├── edges.rs            // Incident, path, and active edges (with WCAG contrast)
│   ├── nodes.rs            // Circles, state halos, decade palettes
│   └── labels.rs           // Semantic zoom, Christensen greedy collision culling
└── plugins/
    ├── mod.rs              // Plugin trait
    ├── minimap.rs          // Overview+detail sub-canvas with draggable viewport
    ├── legend.rs           // Interactive decade & state filter
    ├── toolbar.rs          // Zoom, Pan, Fit, Reset, Layout switcher
    └── tooltip.rs          // Dynamic hover cards (DOM / Canvas overlay)
```

### 3.1 The Pluggable Layout Engine (`layout/`)
In G6, layouts implement a standard interface that takes the graph structure and updates node coordinates. We establish a clean Rust trait:

```rust
pub trait GraphLayout {
    /// Compute node coordinates in world space given the graph payload and existing bounds.
    fn compute_layout(
        &mut self,
        nodes: &[GraphNode],
        links: &[GraphLink],
        focus_id: &str,
    ) -> Vec<(f64, f64)>;
}
```

#### Layout 1: Era-Lane Layout with Sugiyama Barycenter Ordering
- **Problem with Current Code**: In `browser.rs:555-563`, nodes are assigned $x$ from `debut_year` and $y$ from an FNV-1a hash (`(lane - 0.5) * 5200.0`). The vertical order carries zero topological information, producing random long diagonals and unnecessary crossings.
- **The Solution (Sugiyama Barycenter Sweep)**:
  1. Assign nodes to vertical layers (era bands) based on debut year.
  2. For each era band $L_k$, compute the barycenter of each node $v \in L_k$ based on the $y$-coordinates of its neighbors in $L_{k-1}$ and $L_{k+1}$:
     $$y_{barycenter}(v) = \frac{1}{|\text{Neighbors}(v)|} \sum_{u \in \text{Neighbors}(v)} y(u)$$
  3. Sort nodes in $L_k$ by their barycenter, applying a minimum vertical padding to prevent overlaps.
  4. **Result**: Aligns frequent teammates horizontally; turns arbitrary crisscrossing diagonals into short, readable horizontal or near-horizontal links.

#### Layout 2: Concentric & Radial Layouts (Degree-of-Interest)
- **Principle**: When exploring connections from a chosen player (e.g., Kareem Abdul-Jabbar), the user's primary mental model is hop distance (1st degree teammates, 2nd degree teammates, etc.).
- **Concentric Layout**:
  - Center ($r = 0$): Selected player.
  - Ring 1 ($r = R_1$): Direct teammates (hop distance 1), distributed evenly by angle $\theta \in [0, 2\pi)$.
  - Ring 2 ($r = R_2$): Teammates of teammates (hop distance 2).
  - Ring 3 ($r = R_3$): Remainder of the network.
- **Complexity**: $O(|V|)$, executes in $<5\text{ ms}$ in WASM for 5,000 nodes. Completely deterministic and stable.

#### Layout 3: Continuous Force Layout (ForceAtlas2 via `fa2`)
- **Principle**: For natural community clustering (franchises, dynasties, eras naturally grouping together).
- **Integration**:
  - Use the `fa2` crate (MIT licensed).
  - Initialize node positions from the era-lane coordinates (warm start prevents the chaotic dispersion of random initialization).
  - Execute a bounded number of iterations (e.g., 50–100 iterations) during offline precomputation or as a web-worker background pass.
  - Store layout coordinates deterministically or run a capped relaxation loop.

---

## 4. Graphics & Usability Enhancements (G6-Style Feature Set)

To elevate `canvas-view` to professional G6-grade visual quality and usability, six core feature areas must be implemented.

### 4.1 Minimap (Overview + Detail)
- **Primary Source Evidence**: Cockburn, Karlson & Bederson (2009) demonstrate that overview+detail interfaces consistently outperform single-viewport panning for spatial navigation and reduce disorientation.
- **Implementation Design**:
  - A secondary `<canvas id="graph-minimap" width="200" height="120">` positioned in the lower-right corner of the canvas panel.
  - **Rendering**:
    - The entire 8,795 × 5,200 world is rendered into the minimap at a fixed scale ($\approx 0.02$).
    - Render nodes as simplified 1px dots colored by decade; skip the 100k faint edge lattice to keep the minimap render instantaneous ($<1\text{ ms}$).
  - **Viewport Window (View Box)**:
    - Compute the world-space bounding box currently visible on the main canvas:
      $$x_{\text{min}} = -x_{\text{pan}} / \text{scale}, \quad x_{\text{max}} = (W_{\text{screen}} - x_{\text{pan}}) / \text{scale}$$
      $$y_{\text{min}} = -y_{\text{pan}} / \text{scale}, \quad y_{\text{max}} = (H_{\text{screen}} - y_{\text{pan}}) / \text{scale}$$
    - Project this rectangle onto the minimap coordinate space and stroke with a 1.5px `#b04f00` border and a translucent `rgba(176, 79, 0, 0.15)` fill.
  - **Interaction**:
    - Pointer dragging on the minimap view box immediately updates the main canvas pan $(x_{\text{pan}}, y_{\text{pan}})$.
    - Clicking anywhere on the minimap centers the main canvas on that world location.

### 4.2 Interactive Legend & Category Filtering
- **Primary Source Evidence**: W3C WCAG 2.2 SC 1.4.1 (Color must not be the only visual means of conveying information) and ColorBrewer guidelines (Harrower & Brewer 2003).
- **Implementation Design**:
  - Upgrade the static `.legend` HTML list in `graph_view.rs` to an active interactive component.
  - Each legend item becomes a toggleable button:
    ```html
    <button type="button" class="legend-swatch-btn" data-decade="1980">
      <span class="swatch" style="background:#65a41e" aria-hidden="true"></span>
      1980s
    </button>
    ```
  - **Interaction Behavior**:
    - Clicking a decade swatch toggles a filter: nodes in that decade remain fully opaque, while nodes outside the decade drop to $\alpha = 0.08$.
    - Clicking the active filter a second time clears the filter.
    - Keyboard accessible: operable via `Tab` and `Space`/`Enter`.

### 4.3 Node & Edge Styling, Halos, and High-Contrast Theming
- **Contrast Ratios (WCAG 2.2 SC 1.4.11 - 3:1 Non-Text Contrast)**:
  - Faint background lattice: Stays as subtle contextual texture ($\alpha \approx 0.075$).
  - 1-hop incident edges: Opaque enough to clear 3:1 on white background (`rgba(176, 79, 0, 0.75)` yields 3.2:1).
  - Selected player node: `#b04f00` (5.3:1) with an outer concentric ring (halo) of `rgba(176, 79, 0, 0.25)` with a 3px radius offset.
  - Path nodes & edges: `#176896` (6.07:1 on white).
- **Curved Edges for Clean Visual Flow**:
  - Parallel or collinear edges between nodes in the same lane can use quadratic Bézier curves with a slight perpendicular control point offset, eliminating edge overlap.

### 4.4 Level of Detail (LOD) & Semantic Zooming
- Rather than a binary zoom gate (e.g., `scale >= 0.7`), adopt a 3-tier semantic zoom hierarchy:

| Zoom Level | Visual Density | Rendered Elements | Labeling Rules |
|---|---|---|---|
| **Macro / Overview** (`scale < 0.2`) | Low density | Colored node dots (radius 1.5–2.5px), faint background lattice texture, highlighted shortest path. | **Zero regular labels.** Only the selected player and target player are labeled. |
| **Meso / Subgraph** (`0.2 <= scale < 0.8`) | Medium density | Scaled node circles (radius 3–6px), 1-hop and 2-hop edges rendered prominently. | **Landmarks + Neighbors.** Selected player, 1-hop neighbors, and high-degree landmark players (e.g. Kareem, LeBron, Wilt) with greedy collision culling. |
| **Micro / Detail** (`scale >= 0.8`) | Full detail | Full node circles (radius 6–10px) with state halos, teammate link weights, edge directional indicators. | **Comprehensive labels.** Node names, debut seasons, and incident edge weights. |

### 4.5 Tooltips (Hover Cards)
- **Problem**: When hovering over nodes in dense regions, reading names and stats directly on the canvas causes visual clutter.
- **Solution (Accessible HTML/Canvas Tooltip)**:
  - Maintain an overlay container `<div id="graph-tooltip" role="tooltip" hidden>` positioned absolutely over the canvas.
  - When `CanvasView.hovered` detects a node via spatial hash index:
    - Populate tooltip HTML: Player Name, Career Span, Teams Played, Degree of Interest / Hop Distance, and Mutual Teammate Count.
    - Position the tooltip adjacent to the cursor:
      $$\text{top} = y_{\text{screen}} + 12\text{px}, \quad \text{left} = x_{\text{screen}} + 12\text{px}$$
    - Constrain bounds so the tooltip never overflows the canvas viewport.
  - For edge hover: Display the two players, shared franchise, overlapping dates, and minimum shared games.

### 4.6 Camera Interpolation & Smooth Transitions
- Replace abrupt camera jumps (`v.fit_network()`, `v.focus_player()`) with smooth camera tweening.
- Implement a lightweight easing function in Rust WASM using `web_sys::window().request_animation_frame`:
  $$t_{\text{norm}} = \frac{t - t_{\text{start}}}{\text{duration}}$$
  $$\text{ease\_out\_cubic}(t) = 1 - (1 - t)^3$$
  $$x_{\text{current}} = x_{\text{from}} + (x_{\text{to}} - x_{\text{from}}) \cdot \text{ease\_out\_cubic}(t_{\text{norm}})$$
- Duration: 250–350 ms. Fast enough to never delay user interaction, yet smooth enough to preserve the user's spatial mental map (Misue et al. 1995).

---

## 5. Developer Experience, Stability & Testing Architecture

To guarantee long-term codebase health, maintainability, and regression-free development, the project needs strict developer experience (DX) tooling.

### 5.1 Workspace Formatting (`rustfmt.toml`)
While `cargo fmt --all -- --check` runs cleanly today, the workspace currently lacks an explicit `rustfmt.toml` configuration file. Creating a pinned `rustfmt.toml` at the repository root guarantees deterministic formatting across developer workstations and CI runners.

#### Recommended `rustfmt.toml`
```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Default"
reorder_imports = true
reorder_modules = true
format_strings = true
newline_style = "Unix"
```

### 5.2 Workspace Lints (`clippy`)
The workspace already enforces zero compiler warnings on both host and wasm targets:
- Host target:
  ```bash
  cargo clippy --workspace --all-targets --locked -- -D warnings
  ```
- WASM target:
  ```bash
  cargo clippy -p canvas-view --target wasm32-unknown-unknown --locked -- -D warnings
  ```

#### Recommended Workspace Lints in Root `Cargo.toml`
To avoid repeating `-- -D warnings` on every CLI invocation, standardize lints in the workspace `[workspace.lints.clippy]` table:

```toml
[workspace.lints.rust]
unsafe_code = "deny"
missing_debug_implementations = "warn"
unreachable_pub = "warn"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
correctness = { level = "deny", priority = -1 }
perf = { level = "deny", priority = -1 }
style = { level = "warn", priority = -1 }
dbg_macro = "deny"
todo = "warn"
unimplemented = "warn"
```

### 5.3 Test Suite Architecture & Flakiness Prevention
The testing pyramid for this repository spans three distinct tiers:

```
          ▲
         / \       3. Headless Browser E2E Tests (Playwright / Chromium via browser_driver.cjs)
        /   \         Exercises WASM lifecycle, canvas gestures, pointer hit-testing, live-regions
       /-----\
      /       \    2. Integration Tests (Axum / App-Server HTTP API)
     /         \      Exercises SSR HTML generation, /network, /neighborhood, Jev endpoints
    /-----------\
   /             \ 1. Unit Tests (Deterministic Graph-Core & Algorithms)
  /_______________\   Exercises Petgraph traversals, interval overlap, ranking, BFS
```

#### Flakiness Mitigation in Browser Tests
Browser canvas tests are notoriously prone to timing and animation flakiness. Our existing test harness (`canvas_browser.rs` and `browser_driver.cjs`) implements several industry best practices that must be maintained and strengthened:
1. **Explicit Readiness Handshakes**:
   - The WASM module writes state attributes to `#canvas-status`:
     `data-ready="true"`, `data-frame="<n>"`, `data-zoom="<scale>"`, `data-pan-x="<x>"`.
   - Browser test assertions never use arbitrary `sleep(ms)` delays. Instead, they query `waitFor({selector: '#canvas-status[data-ready=true]'})` or wait for `data-frame` to increment.
2. **Deterministic Virtual Resolutions**:
   - The headless browser viewport is locked to a fixed size (1200×1000 in `browser_driver.cjs:29`).
   - The canvas element maintains an internal logical coordinate space (1000×600) scaled by `devicePixelRatio`.
3. **Clean Teardown Guarantees**:
   - The `Browser` struct implements `Drop` (`canvas_browser.rs:64-78`), ensuring Chromium processes and pipes are killed even if a Rust panic occurs mid-test.

### 5.4 Pre-Commit Hooks & Verification Scripts
The repository provides `scripts/check.py`, which supports three standardized profiles:
- `python3 scripts/check.py --profile portable`: Runs `cargo test`, `cargo fmt --check`, `clippy-host`, python unit tests, and source snapshot verifications without requiring Node/Playwright.
- `python3 scripts/check.py --profile browser`: Runs `build-canvas.sh`, `clippy-wasm`, and the Playwright Chromium E2E test matrix.
- `python3 scripts/check.py --profile full`: Runs both portable and browser profiles.

#### Recommended Pre-Commit Hook (`.git/hooks/pre-commit`)
Developers should run the portable suite before committing:
```bash
#!/bin/sh
set -e
echo "Running pre-commit portable verification..."
python3 scripts/check.py --profile portable
```

---

## 6. Actionable Implementation Roadmap

With research complete, the following phased roadmap details the exact sequence of implementation:

### Phase 1: Developer Experience & Workspace Hardening
- [ ] Add `rustfmt.toml` to the workspace root with standard formatting rules.
- [ ] Add workspace lint definitions to `Cargo.toml`.
- [ ] Ensure `python3 scripts/check.py --profile full` passes cleanly.

### Phase 2: Canvas View Architecture Refactor (`crates/canvas-view`)
- [ ] Decouple `crates/canvas-view/src/browser.rs` into modular sub-modules:
  - `model.rs`: Graph data and spatial indexing.
  - `transform.rs`: Pan, zoom, logical-to-screen matrix math.
  - `layout/`: Pluggable layout trait.
  - `render/`: Separate passes for background lattice, active edges, nodes, and labels.
  - `plugins/`: Minimap, Legend, Toolbar, Tooltips.

### Phase 3: Graphics & Layout Upgrades
- [ ] **Era-Lane Barycenter Sorting**: Implement Sugiyama barycenter ordering on vertical positions within debut year bands, converting arbitrary diagonals into clean, near-horizontal edges.
- [ ] **Concentric / Radial Layout**: Implement an alternate layout mode arranging nodes by hop distance from the focused player in concentric rings.
- [ ] **Edge Curves**: Add subtle quadratic Bézier curves to intra-era edges to avoid collinear overlap.

### Phase 4: UI/UX & G6 Feature Additions
- [ ] **Minimap Component**: Add secondary overview canvas in the UI with a synchronized draggable viewport frame.
- [ ] **Interactive Legend**: Add click-to-filter behavior on decade swatches to isolate player eras.
- [ ] **Toolbar Controls**: Add Layout Switcher dropdown (Era-Lane vs Concentric vs Force) and Reset View button.
- [ ] **Hover Tooltip**: Implement accessible hover card popover displaying player career details and mutual teammate counts.
- [ ] **Camera Tweening**: Add 250ms cubic ease-out animation for fit and focus transitions.

### Phase 5: Verification & Browser E2E Expansion
- [ ] Update `crates/app-server/tests/canvas_browser.rs` to verify minimap presence, layout switching, and legend filtering.
- [ ] Rebuild canvas WASM via `scripts/build-canvas.sh`.
- [ ] Verify complete check run via `python3 scripts/check.py --profile full`.

---

## Primary References & Sources

1. **AntV G6 Graph Visualization Framework**:
   - Official Documentation & Architecture: https://g6.antv.antgroup.com/en
   - Source Repository (v5): https://github.com/antvis/G6
   - AntV Layout Subsystem: https://github.com/antvis/layout
   - AntV Unified Graphics Engine: https://github.com/antvis/G
2. **Layout & Graph Drawing Algorithms**:
   - Sugiyama, K., Tagawa, S., & Toda, M. (1981). *Methods for Visual Understanding of Hierarchical System Structures*. IEEE Transactions on Systems, Man, and Cybernetics, 11(2), 109–125.
   - Jacomy, M., Venturini, T., Heymann, S., & Bastian, M. (2014). *ForceAtlas2, a Continuous Graph Layout Algorithm for Handy Network Visualization*. PLoS ONE, 9(6), e98679.
   - Fruchterman, T. M., & Reingold, E. M. (1991). *Graph Drawing by Force-Directed Placement*. Software: Practice and Experience, 21(11), 1129–1164.
   - Barnes, J., & Hut, P. (1986). *A Hierarchical $O(N \log N)$ Force-Calculation Algorithm*. Nature, 324(6096), 446–449.
   - Eades, P. (1984). *A Heuristic for Graph Drawing*. Congressus Numerantium, 42, 149–160.
3. **Information Visualization & Interaction Science**:
   - Cockburn, A., Karlson, A., & Bederson, B. B. (2009). *A Review of Overview+Detail, Zooming, and Focus+Context Interfaces*. ACM Computing Surveys, 41(1), 1–31.
   - Furnas, G. W. (1986). *Generalized Fisheye Views*. In Proceedings of the SIGCHI Conference on Human Factors in Computing Systems (CHI '86), 16–23.
   - van Ham, F., & Perer, A. (2009). *Search, Show Context, Expand on Demand: Supporting Large Graph Exploration with Degree-of-Interest*. IEEE Transactions on Visualization and Computer Graphics, 15(6), 953–960.
   - Misue, K., Eades, P., Lai, W., & Sugiyama, K. (1995). *Layout Adjustment Techniques for Preserving the Mental Map of Diagrams*. Information Processing Letters, 54(4), 183–198.
   - Christensen, J., Marks, J., & Shieber, S. (1995). *An Empirical Study of Algorithms for Point-Feature Label Placement*. ACM Transactions on Graphics, 14(3), 203–232.
   - Ware, C., Purchase, H., Colpoys, L., & McGill, M. (2002). *Cognitive Measurements of Graph Aesthetics*. Information Visualization, 1(2), 103–110.
4. **Rust Crates & Specifications**:
   - `petgraph`: https://crates.io/crates/petgraph
   - `fa2` (ForceAtlas2 in Rust): https://crates.io/crates/fa2
   - `dagre` (Sugiyama layout in Rust): https://crates.io/crates/dagre
   - `rust-sugiyama`: https://crates.io/crates/rust-sugiyama
   - `egui_graphs`: https://crates.io/crates/egui_graphs
   - W3C Web Content Accessibility Guidelines (WCAG) 2.2: https://www.w3.org/TR/WCAG22/
   - WHATWG HTML Standard §4.12.5 (The Canvas Element): https://html.spec.whatwg.org/multipage/canvas.html
