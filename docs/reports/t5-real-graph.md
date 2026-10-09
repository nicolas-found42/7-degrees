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

Actual localhost API and HTML checks, 2026-10-09, refreshed after issue #6 roster-scope correction. G-League movement, draft rights/pick references and contract conversions supply no active-roster boundaries. The T4 appearance review retains source count conflicts and date-only game ordering as unresolved. A pinned draft-year contract guard also keeps new entrants’ pre-contract transfers unresolved; actual later signings establish arrival. Missing a cached signing is uncertainty, not proof that a contract did not exist. Machine-readable output,
input hashes and full histogram: `t5/graph-statistics.json`.

| measure | result |
|---|---:|
| canonical nodes | 5,106 |
| admitted two-boundary tenure rows | 2,227 |
| excluded cross-checked rows | 10,176 |
| excluded inferred rows | 13,445 |
| excluded unresolved rows | 3,446 |
| nodes without admitted tenure | 3,537 |
| undirected certified teammate edges | 1,517 |
| connected components (including isolates) | 4,168 |
| maximum finite degree in this evidenced graph | 20 |
| unreachable unordered pairs | 12,975,882 |
| cold exact statistics HTTP call (debug build) | 0.284 s |
| cached statistics HTTP call | 0.000418 s |

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
Source records: t4/tenures.csv:120 and t4/tenures.csv:2563
```

The same query through `/chain` rendered both player names, degree 1 and the
incomplete-coverage warning. This is an HTTP/rendered-HTML check; final browser
interaction verification belongs to the later UI tickets.

## Traversal and runtime costs

Shortest degree is exact BFS. Neighbor order is stable by canonical ID.
`/api/paths` traverses a shortest-path DAG, counts alternatives without
materializing them, and returns bounded deterministic pages (default 100,
maximum 500). Ranks and DAG counts use arbitrary-precision integers. The exact
count is returned as decimal text `total_exact`; `next_cursor` resumes every
alternative without a fixed-width integer boundary. Pass that returned string
unchanged as the next request's `cursor` argument. Numeric `offset` and
`next_offset` remain supported where ranks fit `u64`. Only the legacy numeric
`total` field saturates at `u64::MAX`, with `total_saturated` marking that case;
the exact count and cursor remain available. No degree or edge changes through
pagination.

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

Both Rust checks passed. The 29 existing API tests passed unchanged. Eight new
approved API-seam tests passed, each implemented with a red/green cycle:
canonical import/provenance, excluded season-only uncertainty/context, duplicate
source interval deduplication, stable shortest-path pagination, legitimate
continuing-signing evidence, per-player coverage-record inspection, the final
chain at the `u64` boundary, and arbitrary-precision cursor continuation over a
fixture with 2^129 shortest alternatives. The
fixture and provider-fallback suites also passed; the live provider smoke test
remains intentionally ignored by the normal offline suite.
