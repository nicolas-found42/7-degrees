# T12: Natural-language query routing (#13)

## Exact acceptance criteria and observable behavior

> A natural-language connection request routes to the graph connection operation.

`GET /api/query?q=...` and `/query?q=...` invoke the same public `query::execute` service. A server-side batch of five Jev Choices selects the operation, first/second verbatim player-mention spans, and stated team/era filters. Required mentions go through the shared `resolution::resolve` service. Only canonical catalog identities reach deterministic Rust graph traversal. A connection displays its minimum degree and ordered players, with an explicitly labeled link to full-graph shortest alternatives/evidence. Fixture HTTP tests independently expect A–B–C, degree 2.

> Player profile, neighbor, and comparison requests reach their own visible outcomes.

Profile output contains sourced identity, teams and season context. Neighbors are endpoints of evidenced direct teammate edges, with full-graph evidence links. Comparison displays sourced player context, shared team affiliations (explicitly not proof of overlapping tenure), and a deterministic direct-teammate result. Each has a distinct SSR heading and typed operation in JSON. The fixture tests separately exercise all three.

> Unsupported or off-topic input gets a visible unsupported outcome rather than a guessed action.

Unsupported is an explicit operation Choice. Empty/missing arguments and valid low-confidence decisions produce clarification before graph execution; unresolved/no-match player mentions retain the resolver's distinct outcome and candidates. Malformed answer packets or provider failures produce unavailable status and deterministic search/connection links. Unconfigured or degraded Jev is visibly reported. The scripted HTTP tests cover off-topic/hostile input, missing second player, unknown operation keys, invalid probabilities, duplicate answers, valid uncertain answers, and credential non-appearance.

> A stated era/team filter changes the returned view without changing the underlying teammate-edge definition.

Team values are bounded to code-retrieved canonical graph franchises, with exact alias matching against the pinned T3 franchise crosswalk. Supported era syntax is a season-ending year, compact year range (`1990-1999`, including Unicode dash), or decade (`1990s`). Unsupported/ambiguous filter syntax asks for clarification. The app does not infer a calendar era from fixture day numbers.

A filtered view retains only original certified roster tenures on the selected franchise and in the selected season-ending-year range. It builds a separate graph using the existing positive-overlap rules; the original graph and API remain unchanged. Player identity remains fixed, even when no tenure matches. Profile/comparison context and teammate lists are scoped to retained records; connection degree belongs to the filtered view. A filtered connection also reports the unfiltered degree. Links to `/chain` and full-graph evidence explicitly say they open the unfiltered network. Coverage warnings remain visible, and filtered no-path is not proof of historical disconnection.

The synthetic report-import test limits Alpha's RED teammates to Beta in the 2000s, while Delta remains in the unchanged full graph. A separate filtered connection is unreachable in RED while its unfiltered degree remains 2. A profile filter reduces Beta's team/season context without changing Beta's identity or the unfiltered catalog.

## Public runtime and evaluation hooks

- `query::execute(graph, catalog, reports, jev, text, config)` is shared by SSR, JSON and later labeled evaluation; no parallel prompt recreation is needed.
- `QueryConfig` exposes provisional probability/confidence/margin thresholds, two operation instruction/criteria variants, actual opaque-option-key reassignment in reversed order, and the shared `ResolutionConfig`.
- `QueryResult` contains operation, filter scope, copied catalog players, deterministic data, normalized semantic distributions and typed resolution evidence. Its interpretation label separates model interpretation from graph facts.
- `QueryCall.measurements` retains each `JevEvaluation` separately, including optional actual provider/model/token/cost metadata and elapsed time. HTTP/browser DTOs omit these server-only receipts. Missing metadata remains unknown.
- A request is limited to 320 characters/32 words. Mention options are contiguous source spans of at most five words/160 characters, with an explicit missing-mention choice. No model-generated name, ID, edge, biography or free-form explanation is accepted.
- A view filter is distinct from an identity clue: unique exact canonical endpoint names resolve without requiring membership in the selected view; filters may supply context for nonexact/ambiguous mentions.

Thresholds default to 0.9 top probability, 0.9 confidence and 0.25 margin. They are provisional, not an accuracy guarantee; #16 must calibrate and evaluate them. The resolver separately applies its existence policy.

## Executed evidence

Nine tests in `crates/app-server/tests/query_routing.rs` cover the eight HTTP behavior slices plus the public evaluation seam's actual criteria/order perturbation and per-call receipt retention. Each slice recorded a failing result before its implementation; red/green logs are retained under `/Users/Nicolas/Documents/7-degrees-review-notes/t12/`. An initial fake-provider script incorrectly assumed a misspelling returned only one lexical candidate; that failure was retained and the script corrected to choose Player A from the actual bounded shortlist.

Against integration `472102f40fe966376df265063d5a13c6598891a8`:

```sh
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Workspace: **88 passed, 0 failed, 3 ignored** (two opt-in live provider tests and the explicit browser test). Formatting, Clippy and whitespace checks passed. Full outputs: `workspace-tests.log`, `fmt.log`, `clippy.log`, `whitespace.log` in the retained T12 log directory. Browser E2E for natural-language queries is assigned to the later shared browser-test work; it is not claimed here.

A single bounded real-provider smoke check used the same public service and the synthetic roster fixture: `connect Player A to Player C` executed with degree 2. [The retained receipt](t12-live-smoke.json) records model `typesafe/jev-1.13-20260917`, provider TypeSafe, 1,686 input/584 output tokens, actual cost USD **0.000070812**, measured inference **459.800292 ms** and end-to-end **463.910458 ms**. This verifies this runtime path, not general routing accuracy or real historical connectivity.

The implementation followed current [Choice documentation](https://docs.typesafe.ai/primitives/choice), [function calling cookbook](https://docs.typesafe.ai/cookbooks/function_calling) and [API contract](https://docs.typesafe.ai/api), screened before use. It reuses the previously verified Rust OpenRouter transport; credentials remain server-side.

## Completion gate

The first oversized gate request returned `max_tokens_exceeded` without a judgment. The input was repaired to include the four raw runtime/test diffs, full actual workspace/Clippy output and a bounded live receipt. The single valid judgment returned **escalate**, not auto: safe-to-apply 0.38, composite 0.737125, zero contradicted claims, two verified and two unsupported claims, with low rubric/claim confidence. Independent acceptance review must adjudicate the patch against the retained source and logs. No review verdict was retried to seek green. Both the operational failure and valid result are retained in the T12 log directory.
