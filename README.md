# 7 Degrees

**NBA degrees of separation** — every NBA/BAA player in history as a node; a direct link
between two players if they were ever teammates (same franchise, simultaneously overlapping
roster occupancy). Playing for the same franchise at different times does *not* create a link.

Degree of separation between two players = smallest number of teammate links needed to
connect them.

100% Rust. In-app Jev (TypeSafe System One) judgments run server-side; graph math is code.

## Running locally

The app currently serves the spec's synthetic fixture (Players A–D and Teams Red/Blue, plus
fixture-only players C2 and E); the real data import lands with the graph ticket (T5).

```sh
cargo run -p app-server
```

No separately managed database service is needed: the fixture-backed teammate graph is built
in memory at startup. The server listens on <http://127.0.0.1:3000>.

- `/` — connect form plus the fixture's teammate edge list
- `/chain?from=A&to=C` — the shortest teammate chain as ordered players and links with its
  degree of separation (Player A → Player B → Player C, degree 2)
- `/api/fixture`, `/api/edges`, `/api/edges/{player}`, `/api/connection?from=A&to=C`,
  `/api/paths?from=A&to=C`, `/api/stats` — the JSON API
- `/api/semantic-status` — whether semantic (Jev) features are available

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

Status: T1 scaffold complete (fixture graph core, Axum API, minimal UI); data acquired
(T2); the real NBA/BAA graph lands with T4/T5. Server-side Jev client with safe fallback
(T11) landed: see the "Jev (semantic) features and the safe fallback" section and
`docs/reports/t11-jev-client.md`.