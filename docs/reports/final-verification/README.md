# Final application verification — T17 / issue #18

The complete Rust application was started through `./scripts/run-app.sh` against the full committed report snapshot, with release WASM built first, no database service and semantic credentials removed from the launching environment. Actual HTTP responses and browser journeys confirm the implemented behavior. This is local evidence; no remote CI result or complete historical graph is claimed.

[Ordered commands, results and hashes](commands-and-hashes.json) link the actual [raw check output](logs/). Machine-specific path prefixes were removed and redundant trailing empty lines normalized in published logs; hashes of the unchanged external originals are retained. Checks used integration source base `c63edd0437249c0b57b999e5ca9e02f11edb13c9`, with the T17 documentation/header correction. Final review base is origin/main `f8e743ca30939476433c680e7a29169d70c3fa41`. Generated audit utilities were also executed against retained artifacts after the application test pipeline. No raw source data, ignored cache or checkpoint was changed.

## Actual checks

| Check | Actual result / receipt |
|---|---|
| Release canvas build before workspace suite | Passed; [01](logs/01-wasm-build.log) |
| Rust workspace | **102 passed, 7 intentionally ignored**, zero failures; [02](logs/02-rust-workspace.log) |
| Source-preparation Python suite | **90 passed**; [03](logs/03-python-source.log) |
| Rust formatting | Passed; [04](logs/04-format.log) |
| Host and wasm32 Clippy, warnings denied | Passed; [05](logs/05-clippy-host.log), [06](logs/06-clippy-wasm.log) |
| Explicit browser E2E + canvas browser | **3 + 1 passed**, Chromium 153.0.8010.12; [07](logs/07-browser.log) |
| Explicit routine provider log capture | **1 passed** with `RUSTFLAGS='--cfg test_capture'`; [08](logs/08-credential-logs.log). The ordinary target contains zero tests. |
| Labeled semantic validation | **44 valid cases**, disjoint IDs/splits; [09](logs/09-semantic-labels.log) |
| Frozen runtime replay | **132 outcomes**, no provider calls; [10](logs/10-semantic-replay.log) |
| Semantic artifact consistency | 44 labels, 132 outcomes, 18 summaries, policy/order/source hash/usage/degree invariants audited; [11](logs/11-semantic-audit.log) |
| Current-vs-original audit join and audit integrity | 6,246 original cases, 1,548 groups, six 50-case strata, 21,984 source locators and 123,278 associations; [12](logs/12-corrected-audit.log), [13](logs/13-historical-audit.log) |
| Actual raw snapshot SHA verification | All three bulk archives and **80 cached pages**, zero downloads; [14](logs/14-source-snapshot.log) |
| Default full-app wrapper startup | Port 49332; [15](logs/15-app-startup.log) |
| Fresh full-snapshot APIs and WASM response | Counts, evidence, no-key fallback and asset magic checked; [16](logs/16-runtime-apis.log), [API receipts](runtime-apis.json) |

The seven workspace ignores are explicit browser/canvas/live-provider checks. Browser targets are separately exercised above; live-only checks are not rerun in T17. Recorded paid semantic experiments remain in [evaluation evidence](../../evaluation/README.md). The tested API/browser seams are the user-approved black-box seams, with source-preparation unit tests retained for deterministic parsing arithmetic.

To repeat the runtime receipt capture, start the wrapper with a fresh server before the first statistics request, then run:

```sh
env -u OPENROUTER_API_KEY -u TYPESAFE_API_KEY NBA_PORT=49332 ./scripts/run-app.sh
# In a second terminal:
python3 scripts/capture-runtime-evidence.py --url http://127.0.0.1:49332 --output target/runtime-apis.json
```

No key is required for these requests. `scripts/publish-final-evidence.py --raw-root <raw-log-root>` publishes the actual logs with path prefixes sanitized. It never fabricates test output. The command/result manifest explicitly separates startup from completed checks. Committed runtime graph metadata summarizes the full response count; the full raw graph response is retained outside the repo with its capture hash. The published Acy evidence response contains only the actual Acy/Bogut edge for compactness.

## Full evidenced graph

Fresh `/api/graph`, `/api/stats` and `/api/coverage` agreed: **5,106 nodes, 2,227 certified tenures, 1,517 undirected edges, 4,168 connected components, finite diameter 20, 57,183 reachable unordered pairs and 12,975,882 unreachable unordered pairs**. These sum to `5106 × 5105 / 2 = 13,033,065` distinct unordered pairs. Self-pairs are excluded; isolates each count as a component. Unreachable pairs are omitted from the histogram and finite diameter, not assigned a finite distance. Computation is deterministic BFS in Rust.

| Degree | Reachable pairs | Degree | Reachable pairs |
|---:|---:|---:|---:|
| 1 | 1,517 | 11 | 3,891 |
| 2 | 1,989 | 12 | 3,002 |
| 3 | 2,752 | 13 | 2,368 |
| 4 | 3,916 | 14 | 1,481 |
| 5 | 5,028 | 15 | 941 |
| 6 | 6,175 | 16 | 500 |
| 7 | 6,547 | 17 | 343 |
| 8 | 6,240 | 18 | 195 |
| 9 | 5,535 | 19 | 88 |
| 10 | 4,635 | 20 | 40 |

Measured cold statistics HTTP wall time was **0.279255 s**, followed by **0.000479 s** cached. These single samples came from a localhost debug server on Darwin/arm64, not a general performance guarantee. OS/Python versions and exact timings are in the API receipts.

Only positive intervals with both dated transaction anchors, stable identity and no blocking unresolved flags enter the graph. Excluded tenure counts are 10,176 cross-checked, 13,445 inferred and 3,446 unresolved. **3,537 players have no certified tenure**, coverage is `complete: false`, and unresolved 1946–1950 BAA evidence remains explicit. The real Acy/Bogut edge is Dallas/MAVERICKS for 121 days; current full snapshot source references are `t4/tenures.csv:120` and `:2563`. Postseason-only Luca Vildoza (`nba:1630492`) is included as an evidenced person; his inferred 2022 stint (`:19110`) does not create edges. Five nameless S1 play-by-play references (471, 775, 1277, 1787, 2794) remain unresolved identity candidates. Other source disagreements are retained in the T3 register; the build does not claim every historical identity is fully reconciled.

The [historical audit](../historical-audit/README.md) preserves actual reading outcomes and fallible model signals separately. Current membership-key joins do not count as new source reading. Its original selected rows are Git-recoverable at commit `474481bde27ac33887ae9f285e351735f0905571`; current source hash is `5dbff2a790e87941b6bb9cd9c05c6f91414ae0f47ae1f2992d8d3c230a4fc24e`. Pre-correction discrepancies are not represented as current graph defects.

## Semantic evidence and limits

No live provider experiment was repeated in T17. The retained evaluation has 44 hand-labeled cases (21 calibration, 23 held-out), 132 frozen variants and 102 final calls. Held-out baseline labels matched **11/11 resolution, 4/8 routing and 0/4 ranking**; ranking abstained three times and gave one incorrect hostile preference ordering. This is a semantic error, not a graph arithmetic error. Wording/order sensitivity, each failed/uncertain outcome, frozen calibration objective/grid/tie-break and all per-call metadata remain published. Costs/tokens are reported provider receipts; initial aborted-run spend is unknown. Original 42 labels and two later pre-inference supplements, pilot captures and outcome-inspection chronology are explicitly retained. No broad accuracy claim follows from this small set.

## Actual browser artifacts

These screenshots are the committed inspected T16 captures, not new T17 screenshots. The real journey uses seven selected canonical players and all 76 corresponding committed tenure rows. It is a **real slice**, not the full snapshot or a mixture with synthetic rows. Slice-local Acy record `:8` maps to full record `:120`, and Bogut `:23` maps to `:2563`; every mapping and hash is retained in the [real-slice map](../t16/real-slice-map.json). The separate synthetic fixture and diamond test alternatives/filters deterministically.

![Synthetic fixture indirect path and focused neighborhood](../t16/fixture-graph.png)

![Real slice Acy/Bogut native evidence page; displayed line references are slice-local](../t16/real-evidence-page.png)

![Real slice selected-edge panel; full-snapshot mapping retained separately](../t16/real-evidence-panel.png)

## Acceptance evidence

[The 38-criterion matrix](acceptance-matrix.md) links each requirement to observable evidence and states its limitations. It does not turn unresolved source coverage or low ranking accuracy into a success claim. Final Jev gate scope/result and stronger independent review are separate review artifacts; this report alone does not assert PR readiness, remote green checks, integration merge or issue closure.
