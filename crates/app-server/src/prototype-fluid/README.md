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

Prototype code is isolated from the production Rust routes and build. The switcher is available only through this separate local runner. No winning design has been chosen or promoted. After critique, record the verdict on the implementation issue and preserve these artifacts on a throwaway branch before implementing the chosen direction.
