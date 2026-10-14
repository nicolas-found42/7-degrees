# Explorer upgrade — issue #21

Implemented [Upgrade NBA/BAA graph explorer: clarity, shortest teammate chains, filters, and evidence drawer](https://github.com/nicolas-found42/7-degrees/issues/21). The default still loads every admitted player and edge, including isolates. Pair selection, era/franchise emphasis, ego views and drawer navigation operate on that network. Distances are exact within the evidenced graph; historical coverage remains incomplete.

## Execution

The final delivery run (`issue21-20261014-accessibility-final`) passed all ten checks: **121 Rust tests, 108 Python tests, and 13 serialized Chromium/WASM journeys**, with zero failures. Formatting, host/WASM Clippy, WASM build, derived pins, immutable historical audit and frozen semantic replay also passed. The browser matrix intentionally injects two rejected requests in its recovery case; their expected `net::ERR_FAILED` entries are asserted and logged, and retry succeeds. Other journeys assert no browser errors. Application semantic providers are unconfigured or scripted.

- [Final compact receipt](checks-delivery/review-summary.json), [manifest](checks-delivery/manifest.json), [browser log](checks-delivery/browser-matrix.log).
- [Source-byte certificate](source-bytes.json): the tracked diff and new Rust/config file hashes were checked against the successful run before staging. The receipt records starting HEAD `dd4c8d35f988a41843eed95cd166164e76dc8039` plus uncommitted changes. The implementation commit adds these identical application/test bytes and report artifacts.
- [Initial successful run](checks/review-summary.json) and [pre-accessibility successful run](checks-final/review-summary.json) remain historical receipts for their recorded source states. A separate successful intermediate overlay run remains in ignored `target/verification/issue21-20261014-overlay-final`.
- [Screenshot hashes](screenshots.json) and [four supplemental capture commands/results](supplemental-capture.json). These supplemental runs use the same built Rust/WASM app and existing Playwright transport, and assert no browser errors.

## Acceptance evidence navigation

| Criteria | Evidence |
| --- | --- |
| A01–A02 | Complete fixture/full-report payload contracts, 5,106/101,395 baseline, empty and single-isolate browser fixtures. |
| A03–A11 | Layout controls/pending selection, annuli, isolated focus, complete ego counts, interrupted transitions, reduced motion; shared layout source and screenshots. |
| A12–A13 | Alias/duplicate identity HTTP tests, context filters before limits, keyboard combobox journey and delayed search boundary. |
| A14–A19 | Known direct/indirect/self/disconnected and diamond paths, stale/failing chain requests, breadcrumb camera-only actions, distinct random IDs, Clear chain, shared-link initial framing. |
| A20–A21 | Fixture Red filter excludes non-overlapping Player D, era/franchise composition, reset and ego/chain visibility while loaded totals and distances remain intact. |
| A22–A25 | Camera pointer/keyboard journey, actual CDP touch pan/pinch/cancel/rebase, DPR 2 and minimap dragging, 390×844 overflow check. |
| A26–A29 | Native modal focus restoration, Escape/Back/Forward, canonical URL, modified-click new tab, fragment/standalone contracts, late-close cancellation, retry. |
| A30–A33 | Reversed endpoint meters, actual shortened 66-game denominator, original witnessed date/records, synthetic fixture days, dated Acy/Bogut interval, bounded excluded-record table and persistent caveat. |
| A34–A35 | Native GET pair/chain links, unconfigured application provider, repeated full-network interactions, final receipts and inspected visual captures. |

Tests use the existing HTTP and real-browser seams. Screenshots supplement behavioral assertions and source inspection; they do not establish exact shortest paths or complete historical coverage by themselves.

## Standards

No definite documented-standard violations remain. The review found duplicate spatial-index rebuilding and an inaccurate Sugiyama-style comment; both were corrected. A separate stale-endpoint risk was corrected: typing into a picker now invalidates its canonical endpoint and old highlight. Camera-fitting arithmetic repetition remains an optional maintainability finding; it is not a blocking behavior issue.

## Spec

Both actionable review findings were corrected: a pending layout animation can no longer overwrite successful chain framing, and zoom announcements wait until animation/gestures settle. Follow-up source review found no concrete regressions. Final visual/behavior inspection also corrected phone badge/HUD overlap, shared-pair initial framing, and full proof tables being an atomic live region. The drawer now announces a concise loading/loaded/error status separately from proof content.

Review summary: Standards — zero hard violations, one optional duplication finding; Spec — zero unresolved actionable findings. Independent reviewers inspected source; the execution receipts provide test evidence.

## Jev advisory judgments

[Recorded Jev gates](jev-reviews.json) verified execution claims, but **did not automatically accept the patch**. Broad partition safe-to-apply scores were 0.39–0.53; the last incremental gate scored 0.47 (composite 0.721), with low confidence and a test-gap limitation. All returned `escalate`; these are disclosed for manual inspection. They are advisory judgments, not proof of defects or automatic approval. Concrete source-review findings were resolved and real checks passed. Separate count verification supported 121 Rust / 108 Python / 13 browser results. Visual transcript screening/auditing passed; that audit checks text observations, not the underlying pixels.

## Visual inspection

Actual images were inspected for opaque orange bands, important label overlap, overlay obstruction, proof readability and phone overflow. Pete Maravich's badge remains readable in the three layout label views; focus lines stay thin. Phone Player A/B badges now avoid the docked controls. Ordinary labels are culled around reserved badges and overlays. Proof cards retain arithmetic, interval boundaries, source references and unknown-duration wording.

| Capture | Links |
| --- | --- |
| Whole network | [Desktop](screenshots/all-player-network.png), [phone](screenshots/all-player-network-phone.png) |
| Pete Maravich — Era | [Overview](screenshots/maravich-era-overview.png), [labels](screenshots/maravich-era-labels.png) |
| Pete Maravich — Concentric | [Overview](screenshots/maravich-concentric-overview.png), [labels](screenshots/maravich-concentric-labels.png) |
| Pete Maravich — Force | [Overview](screenshots/maravich-force-overview.png), [labels](screenshots/maravich-force-labels.png) |
| Chains and focus | [Real chain](screenshots/maravich-chain.png), [ego](screenshots/maravich-ego.png), [real era/franchise filter](screenshots/maravich-franchise-era-filter.png), [synthetic filtered chain](screenshots/fixture-chain-filtered.png) |
| Proofs | [Real dated drawer](screenshots/acy-bogut-dated-drawer.png), [real count drawer](screenshots/maravich-count-drawer.png), [synthetic dated drawer](screenshots/fixture-drawer.png), [synthetic shortened season/game witness](screenshots/shortened-season-and-game-witness.png) |
| Phone and motion | [Phone toolbar](screenshots/fixture-mobile-toolbar.png), [phone synthetic drawer](screenshots/fixture-mobile-drawer.png), [phone real count drawer](screenshots/mobile-count-drawer.png), [reduced-motion settled view](screenshots/reduced-motion-final.png) |
