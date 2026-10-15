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

Prototype code is isolated from the production Rust routes and build. The switcher is available only through this separate local runner. The user preferred B / Orbit in round one and F / Team Compass in round two. D–F explore shortest-chain navigation based on F; G–I restore destination queries and add feature experiments. After those features were rejected, J–L explore different visual structures with only the core controls. Nothing has been promoted to production. After critique, record the verdict on the implementation issue and preserve these artifacts on a throwaway branch before implementing the chosen direction.


## D–F: shortest-chain Team Compass prototypes

Open the same runner at `/graph?variant=F` (or D / E). Original A / B / C remain available. The prior induced-path versions of D–F are preserved in commit `8ffb891` on the prototype branch; current D–F replace that interaction.

- **D / Open Compass:** a wide franchise-grouped circle, floating directory and bottom chain trail.
- **E / Route Ledger:** a persistent chain history with bounce counts, compact franchise-grouped rings and a separate directory.
- **F / Team Compass:** the preferred layout, with the chain above a franchise-grouped circle and a horizontal teammate directory below.

All start with Michael Jordan. Click Scottie Pippen, then Rasheed Wallace, either on the graph or in the searchable directory. Every visible player node can be dragged, including the current player and earlier chain steps. Dragging updates incident edge geometry and never extends the chain; the next selection recenters the new current player. Reset positions restores the layout. Pointer and touch dragging are supported; the directory provides ordinary keyboard buttons for choosing players. Reduced-motion preferences turn interpolation off by default. The bottom switcher preserves the in-memory chain across D / E / F; reloading starts a new Jordan chain.

### Fewest bounces from the starting player

Earlier-player and teammate exclusions are retained. Those exclusions alone only prevent shortcuts to vertices already in the chain; they can still produce a route with a shorter alternative elsewhere in the graph.

On every new start, breadth-first search computes the minimum unweighted distance from that player to every player in the full loaded app graph, including hidden players and edges. A next choice must be a direct teammate of the current player, must pass the earlier-player exclusions, and must have a global distance from the start equal to the number of bounces the proposed extended chain would contain. This makes every accepted prefix a shortest chain to its endpoint. Multiple equally short routes are allowed. New starting players recompute all distances; Undo, rewind and variant switching keep distances from the same starting player. Unreachable players never become candidates.

A player with distance 2 from Michael Jordan cannot be selected as bounce 3, even if they have never been a teammate of Jordan or Pippen. The unavailable explanation lists that player's shorter distance and the distance this move would take. Unavailable names are plain text in that explanation; their nodes and non-chain edges remain hidden, and they cannot be selected to extend. Previous chain nodes retain only their consecutive chain edges.

The bounce badge, current-node label, history and next-player directory use the same distances. If no next-layer teammate exists, the directory explains that no shortest extension is available and offers rewind through the chain. Filtering locates players without removing other eligible nodes from the circle. Each player with multiple shared franchises is grouped under the first sorted shared franchise. Dense and narrow views use concentric rings. Append `&preview=phone` for a 390px embedded preview.

Browser checks covered Jordan → Pippen → Wallace → Rondo, shortest-distance exclusions and their explanation, stopping when a shortest chain cannot extend, changing the starting player, rewinding and preserving state across variants. The state bar exposes chain names, actual/shortest bounce counts, next distance, available/unavailable counts, drag count and motion state. These are throwaway design studies; no production regression suite was run or test files added.


## G–I: destinations and a fluid trail

Use the same local runner. Open `/graph?variant=G`, H or I. A–F remain unchanged. All three new prototypes retain the Team Compass visual language, editable **Start** and **End** player pickers, swap, shortest routes, drag placement, rewind and reduced motion.

- **G / Route Weave:** a wide graph with a comparison drawer. Its new feature overlays up to five representative equally short routes. Cards choose an entire alternative; they are examples, not an exhaustive list or a ranking. The total shortest-route count is calculated independently. Alternative overlays appear in the completed route overview; during manual exploration, earlier players' non-chain relationships stay hidden.
- **H / Destination Radar:** the preferred F-style graph above a horizontal next-teammate directory. Its new feature previews what opens beyond a next teammate without committing a move. Hover or focus a teammate, or use its **Peek** button. A faint fan shows up to eight next nodes, and the instrument lists remaining bounces and the count of shortest continuations. On phones the instrument moves into the directory so it does not cover the graph.
- **I / Bridge Lens:** a bridge-selection rail beside the graph. Its new feature identifies connectors at the middle distance layer of shortest routes (the earlier middle layer when the bounce count is odd). Cards show how many shortest routes pass through a connector and reveal a shortest route through that player. An intermediate is called unavoidable only when its route count equals the total. Direct teammates and identical endpoints have no intermediate bridge.

The default pair is Michael Jordan → Stephen Curry. **Find chain** immediately reveals a complete shortest route. **Explore from start** lets you build a shortest route step by step; **Reveal full chain** preserves your committed prefix and fills the remaining suffix. Clicking an earlier chain player rewinds. Same-player queries produce zero bounces; disconnected queries show an empty result without fabricating a route. Switching G / H / I preserves the chosen endpoints and chain.

Two full-graph breadth-first traversals compute distances from the start and end. A selectable next teammate must be exactly one bounce farther from the start and one bounce closer to the destination, with the two distances summing to the pair's minimum. Earlier-player and teammate exclusions remain. This preserves shortest prefixes and avoids dead ends on the way to a reachable selected destination. Shortest-route counts use exact BigInt dynamic programming on these distance layers. Route comparisons and bridge selections construct real paths through those layers.

Earlier chain players have curved spiral targets around the current player, rather than a bottom row. A damped spring animation with gentle separation settles after about 1.6 seconds; edges bend with the nodes. Dragging pins a player's position across subsequent steps and rewinds. Selecting that player as the new current player releases its pin so it can center. **Release positions** clears manual placements; new endpoint queries and variant switches reset placements. These are geometry experiments, not a production physics engine.

Browser checks covered alternative route selection, desktop preview without chain mutation, bridge selection, changing and swapping endpoints, actual pointer dragging with a pin retained through rewind and reveal, disconnected and same-player results, and phone layouts. The phone Peek interaction was visually inspected. Syntax checks pass. No test files or production regression suite were added for this throwaway prototype round.


## Newest J–L: three visual directions, core interactions only

Open `/graph?variant=J`, K or L using the same runner. All previous variants remain available. This round removes the route-comparison, next-step-preview and bridge-lens experiments from the new screens; the focus is visual structure rather than new features.

- **J / Ribbon:** cream paper, large serif typography and a left query rail. Named player capsules follow a broad flowing river, with horizontal Bézier connections. Next teammates form scattered tributaries ahead of the active player.
- **K / Afterglow:** a dark immersive field, luminous nodes with depth variation, a floating command deck below the graph and a compact teammate panel. A curved diagonal connection sits in a loose cloud rather than on a radial ring. Stars and tilted background sweeps are decorative.
- **L / Archipelago:** a landscape palette, compact top search, rectangular player markers and a map inset for next teammates. Soft islands group visible relationship franchises, with contour lines and dashed crossing routes. These are relationship-based spatial shapes, not geographic locations.

Start/end selection, finding a complete shortest chain, Explore, Full chain, Undo, teammate filtering, dragging and reduced-motion preferences remain. Two full-graph BFS traversals constrain every next teammate to be one bounce farther from the start and one closer to the destination, with earlier-player teammate exclusions retained. Full chain preserves the selected prefix. The default example uses Jordan → Pippen → Jermaine O'Neal → Curry; Pippen is preferred as the example's first move only when it is on an equally short valid route.

Each direction has a separate layout function as well as a different page structure, typography and node treatment. Damped motion settles after roughly 1.5 seconds. Dragged positions stay across exploration steps until that player becomes current, the user releases placements, changes endpoints, or switches directions. Node coordinates are reinitialized even when DOM elements are reused after an endpoint change. The background textures do not imply extra players or edges; foreground nodes are the actual chain, destination and eligible shortest-route continuations.

Desktop and 390px phone layouts were visually inspected for all three directions. Browser checks covered switching K/L while preserving the chain, choosing Pippen in the map, actual pointer dragging, retained placement through rewind and reveal, and an endpoint change from Curry to LeBron with the graph visibly redrawn. Python/JS syntax and whitespace checks were run. No production regression suite or test files were added for these throwaway visual studies.
