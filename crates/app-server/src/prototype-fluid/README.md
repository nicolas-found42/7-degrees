# Throwaway fluid graph prototypes

Question: which spatial treatment makes an active shortest teammate chain easy to follow at full network density?

Run from the repository root:

```sh
python3 scripts/prototype-fluid.py
```

Open http://127.0.0.1:4317/graph?variant=A. Requires the app's normal Rust toolchain and committed report snapshot. The runner builds and starts the real read-only app backend on 4318 and serves only the prototype on 4317. Ctrl-C stops both. Set PROTOTYPE_PORT to change the port pair.

- A / Spotlight: left discovery rail; a horizontal chain corridor opens within the graph.
- B / Orbit: graph-first radial focus; context moves into a halo; floating discovery and search controls.
- C / Storyline: a light split view with a graph above and a persistent numbered sequence below.

Use the bottom arrows or left/right keys to switch. The URL preserves the variant. Try **Replay motion**, **Clear**, player search, both discovery tabs and selecting ranking rows. The most-connected tab highlights all ten hubs; selecting a hub reveals its incident edges. The longest tab selects one ranked chain at a time. Append `&preview=phone` for a 390px embedded preview.

Data is fetched from the real app's `/api/network`. All player nodes and all relationship edges are retained. An in-memory Web Worker computes exact BFS distances for every source; unreachable pairs are excluded. The ten pairs with greatest finite shortest distances are shown, with ties resolved by player ID order. Direct teammate counts use adjacency cardinality. These are rankings of the loaded app snapshot, not claims of complete historical coverage. Geometry is a prototype spatial arrangement, not a production force simulation. Background edges can be toggled; they remain on by default. Motion settles after each interaction and honors reduced-motion preferences. No persistence, backend mutation, telemetry or deployment.

Prototype code is isolated from the production Rust routes and build. The switcher is available only through this separate local runner. The user preferred B / Orbit as the direction for a second prototype round; nothing has been promoted to production. After critique, record the verdict on the implementation issue and preserve these artifacts on a throwaway branch before implementing the chosen direction.


## Round two: build a chain by exploring teammates

Open the same runner at `/graph?variant=D` (or E / F). Original A / B / C remain available.

- **D / Free Orbit:** a wide circular graph with a floating teammate drawer and a bottom chain trail.
- **E / Chain Rail:** a persistent history column, compact concentric teammate rings, and a separate directory.
- **F / Team Compass:** a franchise-grouped circle with the chain above and a horizontal teammate directory below. A player with multiple shared franchises is grouped under the first sorted shared franchise.

All start with Michael Jordan. Click Scottie Pippen, then Rasheed Wallace, either on the graph or in the searchable directory. Every visible player node can be dragged, including the current player and earlier chain steps. Dragging updates incident edge geometry and never extends the chain; the next selection recenters the new current player. Reset positions restores the layout. Pointer and touch dragging are supported; the directory provides ordinary keyboard buttons for choosing players. Reduced-motion preferences turn interpolation off by default. The bottom switcher preserves the in-memory chain across D / E / F; reloading starts a new Jordan chain.

The exclusion rule is an **induced path**: candidate vertices must be direct teammates of the current endpoint, must not already be in the chain, and must not neighbor any earlier chain vertex. At `[Jordan, Pippen]`, exclude Jordan and every Jordan teammate from Pippen's next choices. At `[Jordan, Pippen, Wallace]`, also exclude every Pippen teammate from Wallace's next choices. Previous chain nodes and their consecutive chain edges remain visible; their other relationships and excluded candidate nodes are removed entirely. This is stricter than merely preventing repeated players. It is a manually explored chain, not a shortest-path claim.

Undo or selecting an earlier chain step truncates the chain and recomputes eligibility. If no candidates remain, the directory explains that you should undo or rewind. Search within the directory highlights matching nodes without removing other eligible nodes from the circle. A new starting player starts a fresh chain. Drag positions live only for the current step, and reset when changing steps or variants. Dense and mobile views use concentric rings; full candidate names remain accessible through hover and the directory. Append `&preview=phone` for the narrow preview.

The app snapshot supports the requested Jordan → Pippen → Wallace sequence. Browser checks covered selecting these graph nodes, dragging the current node without extending the chain, Undo, rewinding, and switching variants. The state bar exposes chain names, eligible/excluded counts, drag count and motion state. These remain throwaway design studies; no production regression suite was run or tests added.
