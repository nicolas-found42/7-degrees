# T5 — Real canonical teammate graph

Issue #7; parent #1; spec `docs/specs/nba-teammate-degrees.md`.

The local Rust application now imports the canonical T3 player universe and T4
roster-tenure reports, replacing the fixture at runtime. The fixture builder
and all existing fixture API tests remain available unchanged.

## Evidence policy

A player becomes a node by canonical source identity (`bbr_player_id`, including
NBA source keys for NBA-only appearances), never by display name. Alternate S1
names become alias context; two people with the same name retain separate IDs.
Every NBA/BAA canonical universe row becomes a node, including players with no
certified roster interval. ABA-only rows and ABA tenures stay outside the graph.

The T4 audit's inferred season brackets and single-bound cross-checks are not
proof of simultaneous roster tenure. An edge requires positive overlap of two
**directly-evidenced** tenures with both dated boundaries, no blocking flags,
and the same canonical franchise. `repeat-signing-continues-open-stint` is a
nonblocking T4 note and does not erase otherwise evidenced tenure. Touching
intervals and self-pairs create no edge. Exact duplicate source intervals count
once; repeated overlap evidence collapses to one undirected player-pair edge.

`/api/edges` and `/api/edges/{player}` retain every overlapping record pair,
its half-open day bounds, T4 CSV line references, membership source, evidence
class, dated arrival/departure anchors and manifest pointer. Day numbers follow
T4's 1946-01-01 epoch (day 0). The source/version and licensing record remains
`docs/data/source-manifest.md`; T4 transaction rows and the request ledger are
retained under `docs/reports/t4/`.

`/api/coverage` reports admitted/excluded counts and missing player coverage.
`/api/coverage/{player}` retains excluded records and their reasons. Runtime
HTML and connection/path responses carry historical uncertainty: a no-path
result affected by missing coverage says `unresolved_coverage`. Fixture
no-path results say `verified_disconnected`. No complete history claim follows
from a successful chain; its degree is exact within the evidenced snapshot.

## Real run

Actual localhost API and HTML checks, 2026-10-09. Machine-readable output,
input hashes and full histogram: `t5/graph-statistics.json`.

| measure | result |
|---|---:|
| canonical nodes | 5,106 |
| admitted two-boundary tenure rows | 3,565 |
| excluded cross-checked rows | 10,713 |
| excluded inferred rows | 12,386 |
| excluded unresolved rows | 5,310 |
| nodes without admitted tenure | 3,235 |
| undirected certified teammate edges | 2,512 |
| connected components (including isolates) | 3,813 |
| maximum finite degree in this evidenced graph | 27 |
| unreachable unordered pairs | 12,732,753 |
| cold exact statistics HTTP call (debug build) | 0.655 s |
| cached statistics HTTP call | 0.000739 s |

Luca Vildoza (`nba:1630492`) remains a node based on his official postseason appearance. His T4 inferred tenure is visible as a coverage gap and creates no edge.

The many isolates and long finite diameter measure the deliberately incomplete
**dated-evidence graph**, not the complete historical NBA teammate network.
The 1946–1950 BAA evidence gap and all weaker classes remain explicit.

Real connection query:

```text
GET /api/connection?from=acyqu01&to=bogutan01
Quincy Acy → Andrew Bogut
Degree: 1
Team: MAVERICKS
Overlap: [25768, 25889), 121 days
Source records: t4/tenures.csv:121 and t4/tenures.csv:2771
```

The same query through `/chain` rendered both player names, degree 1 and the
incomplete-coverage warning. This is an HTTP/rendered-HTML check; final browser
interaction verification belongs to the later UI tickets.

## Traversal and runtime costs

Shortest degree is exact BFS. Neighbor order is stable by canonical ID.
`/api/paths` traverses a shortest-path DAG, counts alternatives without
materializing them, and returns bounded deterministic pages (default 100,
maximum 500). `offset` skips counted DAG subtrees; `next_offset` allows every
alternative to remain inspectable. Extremely large counts saturate at `u64::MAX`
and report `total_saturated`; no degree or edge is changed by pagination.

Statistics use dense-index BFS over each node, count each unordered pair once,
and retain one distance row at a time. The immutable graph caches its result.
Isolates count as components; diameter is the maximum finite shortest length;
unreachable pairs are reported separately. Tenure construction compares sorted
intervals only while overlap remains possible, rather than comparing every
historical stint on a franchise with every other stint.

## Reproduction and tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
NBA_PORT=3017 cargo run -p app-server
curl 'http://127.0.0.1:3017/api/coverage'
curl 'http://127.0.0.1:3017/api/connection?from=acyqu01&to=bogutan01'
curl 'http://127.0.0.1:3017/api/stats'
NBA_DATA_MODE=fixture cargo run -p app-server
```

Both Rust checks passed. The 29 existing API tests passed unchanged. Six new
approved API-seam tests passed, each implemented with a red/green cycle:
canonical import/provenance, excluded season-only uncertainty/context, duplicate
source interval deduplication, stable shortest-path pagination, legitimate
continuing-signing evidence, and per-player coverage-record inspection. The
fixture and provider-fallback suites also passed; the live provider smoke test
remains intentionally ignored by the normal offline suite.
