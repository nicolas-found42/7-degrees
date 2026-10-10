# T11 report: Server-side Jev client with safe fallback (issue #4)

Branch `feature/t11-jev-client` (3 commits on the integration tip 182442c).
All work is Rust; the UI remains plain server-rendered HTML with no JavaScript.

## 1. Live verification of the HTTP request configuration

Done BEFORE implementing, per the ticket. Sources (fetched live this session):

- TypeSafe API reference: <https://docs.typesafe.ai/api.md>
  - `POST /v1/systemone` with `Authorization: Bearer` and
    `Content-Type: application/json`
  - request body: `model`, `state` (string or structured value), `questions`
    (map of typed `noul`/`choice`/`score` questions with `instructions` and
    optional `criteria`)
  - response body: `model`, `answers` (one typed answer per question id),
    `usage{input_tokens,output_tokens}`; errors are standard HTTP statuses
    (401/422/429/529 documented, 429/529 explicitly retryable)
- OpenRouter TypeSafe compatibility guide:
  <https://openrouter.ai/docs/guides/community/typesafe-sdk>
  - base URL `https://openrouter.ai/api`; the System One path is
    `https://openrouter.ai/api/v1/systemone`
  - auth: the OpenRouter API key as `Authorization: Bearer <token>`
  - model ids: `typesafe/jev-1.13` is used as-is; bare `jev-1.13` is mapped
    onto the `typesafe/` namespace; responses are TypeSafe's envelope plus
    pass-through `id`, `provider`, `usage.cost`
- Live smoke call (curl, real key from `~/.zshenv`, model
  `typesafe/jev-1.13`, one noul question): HTTP 200 with
  `{"model":"typesafe/jev-1.13-20260917","answers":{"overlapping_teammates":{"type":"noul","noul":0.99}},"usage":{...,"cost":0.000013062},...}`
  — endpoint, header form, body fields, envelope, and model routing all
  confirmed against the configured provider before any client code was
  written.

The same configuration is proven in Rust: `crates/jev-client/tests/live_smoke.rs`
(drives the real HTTP transport end-to-end; `#[ignore]`d by default — run with
`cargo test -p jev-client --features http-transport --test live_smoke -- --ignored`).
Verified run this session: `test result: ok. 1 passed ... finished in 0.30s`.

## 2. Credential isolation (server-side only)

- The key enters the process only via `TYPESAFE_API_KEY` or the local
  `OPENROUTER_API_KEY` convention (read from the process env at startup;
  never read from any file, never committed — `data/` and `.env*` are
  gitignored independently).
- `JevConfig` holds the key; its `Debug` impl redacts it (`[redacted]`,
  unit-tested).
- Route handlers never touch the config: application code goes through
  `app_server::JevHandle::judge(&JevRequest) -> JevOutcome` and the status
  tuple `(available, reason)`. The status body carries only `available` +
  `reason` (test asserts no credential-shaped string appears).
- `JevTransport::evaluate(config, …)` receives `&JevConfig` but the transport
  uses only `bearer_auth(&config.api_key)` for the outbound header; it never
  logs the key or request bodies — provider warnings carry the error and
  status only.
- Test evidence (api_integration.rs):
  - `served_responses_and_assets_never_contain_credentials` serves 11
    surfaces (pages, style.css, every API route, 404s) with a canary-key
    client and asserts the canary appears in none of them.
  - `credentials_never_reach_routine_logs` runs a failing judgment plus
    traffic with the canary and asserts no leak via the served status.
  - `tests/log_capture.rs` (`RUSTFLAGS='--cfg test_capture' cargo test`,
    own test process, race-free) installs a capture logger and asserts every
    `log` record emitted during a full traffic pass is canary-free.

## 3. Safe fallback (unconfigured + unreachable)

- `JevHandle::from_env()` → `Unconfigured` with no env key;
  `judge()` returns `JevOutcome::Unavailable` deterministically.
- `HttpJevTransport` fail-soft paths: payload guardrails (state > 32 KiB /
  > 8 questions → Unavailable), transport errors (one explicit retry, then
  Unavailable), non-200s (429/529 retried once, then Unavailable), malformed
  envelopes (any parse problem → Unavailable). No unbounded hangs: every
  request carries `config.timeout_secs` (default 10s).
- Degradation latching: a configured provider that fails a call flips the
  status to `(false, "degraded")`; the graph never waits on Jev.
- Test evidence: `graph_and_search_features_work_with_jev_unconfigured`
  (connection/paths/stats/fixture all deterministic), `unreachable_provider_
  fails_soft_and_features_keep_working` (judgment → Unavailable; UI + graph
  responses exact), `jev_unavailable_outcome_is_deterministic_across_calls`,
  `jev_answers_surface_only_through_the_seam_not_the_graph` (graph facts are
  Jev-independent even when the provider answers).

## 4. UI unavailable-status surface

- New endpoint `/api/semantic-status` → `{"available":bool,"reason":
  "unconfigured"|"degraded"|"available"}`.
- Every page (home + all `/chain` variants) opens with a server-rendered
  line `Semantic features (Jev): available|unavailable` plus a
  machine-readable `data-semantic-status` attribute (class
  `semantic-status`, highlighted styling when unavailable).
- Test evidence: `ui_reports_semantic_features_unavailable_by_default`,
  `ui_reports_semantic_features_available_with_a_provider`,
  `semantic_status_reports_unavailable_without_configuration`,
  `semantic_status_reports_available_with_a_provider`.

## 5. Data minimization

- `JevRequest` = bounded `state` (serde_json value) + typed
  `JevQuestion`s — transports enforce `MAX_STATE_BYTES` (32 KiB) and
  `MAX_QUESTIONS` (8) and fail soft to `Unavailable` on violation; the
  whole player graph is never sendable through the seam.

## 6. Test matrix (observed outputs this session)

- `cargo test --workspace` (plain): 34 passed, 0 failed — 29 api_integration
  (19 T1 incl. the review fix + 10 new T11) + 1 graph-core unit + 1 fixture
  unit + 3 jev-client seam unit — live smoke correctly `1 ignored`.
- `RUSTFLAGS='--cfg test_capture' cargo test --workspace`: everything above
  plus the log-capture proof, all green.
- Offline transport fail-soft proofs (added post-review,
  `cargo test -p jev-client --features http-transport --test transport_fallback`):
  7 passed — unreachable provider, current-thread non-panic, 429→recovery,
  429-exhausted, 401-immediate, malformed envelope, provider timeout.
- Live: `cargo test -p jev-client --features http-transport --test live_smoke
  -- --ignored` → 1 passed (0.30s).
- Clippy over app-server + jev-client (all targets): clean; the only
  workspace warnings are T1's pre-existing graph-core ones (out of scope).

Count note: 19 = T1's api_integration tests (18 original + 1 added by the
T1 review fix); the same suite file now holds 29 including the 10 new T11
assertions; the offline transport proofs live in their own target and
complete the matrix above.