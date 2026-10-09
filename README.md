# 7 Degrees

**NBA degrees of separation** — every NBA/BAA player in history as a node; a direct link
between two players if they were ever teammates (same franchise, simultaneously overlapping
roster occupancy). Playing for the same franchise at different times does *not* create a link.

Degree of separation between two players = smallest number of teammate links needed to
connect them.

100% Rust. In-app Jev (TypeSafe System One) judgments run server-side; graph math is code.

## Running locally

The default app builds the real canonical NBA/BAA graph from committed T3 player identities
and T4 dated-tenure reports. No raw source artifact or database server is needed at runtime.

```sh
cargo run -p app-server
```

The server listens on <http://127.0.0.1:3000>. Set `NBA_PORT` for another localhost port,
or `NBA_REPORT_DIR` for another directory containing `t3/` and `t4/` report snapshots.

Only tenures with **both dated transaction boundaries** and no blocking unresolved flags
create edges. Season-only brackets and single-bound cross-checks stay visible as coverage
gaps. Degrees/statistics are exact within this evidenced graph; the snapshot does not
establish the full historical teammate graph. See [T5 graph report](docs/reports/t5-real-graph.md).

- `/` — player connect form, coverage warning, and the first 100 teammate edges
- `/chain?from=acyqu01&to=bogutan01` — real Quincy Acy → Andrew Bogut chain
- `/api/graph` — player and edge summary (`/api/fixture` remains an alias for compatibility)
- `/api/players` — canonical player identities, aliases, first/last season and franchise context
- `/api/edges`, `/api/edges/{player}` — edges with overlapping source records and dates
- `/api/connection?from=acyqu01&to=bogutan01` — exact shortest chain and coverage status
- `/api/paths?from=acyqu01&to=bogutan01&limit=100&offset=0` — equally short alternatives,
  deterministic ID order, exact decimal `total_exact` and resumable `next_cursor`;
  pass the returned cursor unchanged as `cursor=...` to fetch the next page.
  Numeric `offset`/`next_offset` remain compatible; maximum page size 500
- `/api/stats` — cached exact finite separation statistics over unordered pairs
- `/api/coverage`, `/api/coverage/{player}` — coverage counts and individual excluded records
- `/api/semantic-status` — whether semantic (Jev) features are available

Run the unchanged synthetic fixture explicitly for browser tests:

```sh
NBA_DATA_MODE=fixture cargo run -p app-server
```

All application code is Rust, including the server-rendered UI: the served pages are plain
HTML and contain no JavaScript.

## Jev (semantic) features and the safe fallback

Set `OPENROUTER_API_KEY` (or `TYPESAFE_API_KEY`) in the environment to enable the
server-side Jev client — TypeSafe judgments via OpenRouter (`https://openrouter.ai/api`,
model `typesafe/jev-1.13`), config verified against the live TypeSafe/OpenRouter docs; see
`docs/reports/t11-jev-client.md`. The key stays server-side only: it never appears in any
served page, response, or routine log.

With no key, or when the provider is unreachable (fail-soft: explicit timeout + one retry,
then `Unavailable`), every deterministic feature keeps working unchanged and the UI shows
`Semantic features (Jev): unavailable` on every page (`/api/semantic-status` is the
machine-readable form). Graph facts never depend on Jev.

## Tests

```sh
cargo test
```

The fixture-backed API integration tests drive the app in-process over HTTP, exercising the
spec's graph rules: tenure overlap, same-franchise non-overlap, mid-season moves,
repeated-overlap deduplication, minimal-degree chains, unknown-player handling, and the
separation statistics.

Status: the canonical report loader and real dated-evidence graph (T5) are implemented;
fixture API tests remain available unchanged. Server-side Jev client with safe fallback
(T11) landed: see the "Jev (semantic) features and the safe fallback" section and
`docs/reports/t11-jev-client.md`.