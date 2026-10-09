# T16 — Actual browser end-to-end journeys

Issue #17, parent #1; `docs/specs/nba-teammate-degrees.md`.

The suite runs Chromium against Rust-owned Axum servers bound to ephemeral
localhost ports. Assertions and case data live in Rust. The existing Node driver
provides only browser RPC plumbing. Every semantic outcome is produced by a
scripted Rust `JevTransport`; these tests make no live provider calls and measure
application behavior, not model accuracy.

## Reproduce

From the repository root, with the installed Node/Playwright/Chromium available:

```sh
./scripts/build-canvas.sh
cargo test --workspace
cargo test -p app-server --test browser_e2e --test canvas_browser -- --ignored --nocapture
```

`BROWSER_NODE`, `PLAYWRIGHT_PACKAGE_PATH`, and `BROWSER_EXECUTABLE` override the
existing runtime discovery. `BROWSER_E2E_ARTIFACTS` overrides the new suite's
artifact directory (default `target/browser-e2e`); `CANVAS_SCREENSHOT` controls
the retained canvas smoke's capture. No environment variables are mutated by
tests. Servers/browser processes are guarded and stopped when each case ends.

## Case coverage

| Journey | Observable checks |
| --- | --- |
| Synthetic complete fixture | Home search, profile and connection form; direct degree 1, indirect degree 2 with ordered A/B/C, self degree 0; complete-fixture disconnected result; no-match; accessible ordered path and player lists; keyboard selection/pan, zoom/refocus, direct and nearby expansion; fixture provenance; statistics histogram and finite-diameter/unreachable/self-pair definitions. |
| Explicitly synthetic report-backed diamond | Exactly two degree-2 paths A/B/G and A/C/G; actual next-page and alternative selection links; exact cursor retained into canvas; team and season-ending-year natural-language filters change visible teammates and removing filters restores them; low confidence clarifies before a path; unsupported request stays unsupported; provider-down status remains visible while deterministic connection controls work; natural-language connection reaches A/B/C. |
| Seven-player pinned real slice | Home typo search for Quincy Acy, canonical era/team context; Acy/Bogut direct path; native evidence page and selected-edge iframe read as real frame documents; canonical MAVERICKS, 121 days `[2016-07-20, 2016-11-18)`, season 2016–17, both tenure refs/anchors/dates/versions; Dee Brown same-name choices and era/team clarification; nickname/typo semantic suggestions visibly distinguished from graph facts; postseason-only Luca Vildoza remains selectable while inferred tenure is excluded, and no-path/provenance keep coverage unresolved. |
| Retained canvas smoke | Actual pointer node/edge selection, drag and wheel; keyboard/button transforms; direct/nearby expansion; exact cursor/degree retention; fixture and full committed-report Acy/Bogut evidence frames. |

The configured browser journeys assert that their server-only credential canary
is absent from captured browser response bytes, including HTML, assets, WASM,
JSON and iframe responses. Console/page errors are asserted empty. This checks
credential nonappearance for the exercised responses, not every possible route.

## Real slice identity and traceability

The slice contains only `acyqu01`, `bogutan01`, `brownde01`, `brownde03`,
`jordami01`, `onealsh01`, and `nba:1630492`: seven selected player nodes and all
76 corresponding committed T4 rows. It is not the full player universe or full
historical graph. No synthetic roster rows are mixed into the real slice.
Coverage remains incomplete.

Before extraction, the test checks SHA-256 of the committed T3 universe
`81031c7e3cf95845060119a3347d3cf37ec60c6a24b0ced9978fb7df2e7e651d` and T4 tenures
`5dbff2a790e87941b6bb9cd9c05c6f91414ae0f47ae1f2992d8d3c230a4fc24e`. Source
changes require explicit revalidation of these pins. Exact CSV fields are
copied through the runtime import seam. Independent literal checks retain Acy
and Bogut's canonical team, source dates and anchor flags; both admitted source
links displayed in the browser must trace to the original records.

**References in slice screenshots are slice-local.** In particular,
`t4/tenures.csv:8` maps to full snapshot record `t4/tenures.csv:120` (Acy), and
`:23` maps to full record `:2563` (Bogut). The portable
[real-slice map](t16/real-slice-map.json) maps every slice-local tenure reference
to its original full-snapshot record and SHA-256, including Vildoza's inferred
record `:19110`. It does not assert global line equivalence for the slice.

## Evidence

The final run builds release WASM before workspace tests and explicitly runs
the otherwise ignored browser targets. Chromium 153.0.8010.12 passed all three
new journeys and the retained canvas smoke. Formatting, host Clippy and wasm32
Clippy use warnings denied. Raw output and the completion gate are retained
outside the repository in `7-degrees-review-notes/t16-browser-e2e`.

The first harness slices failed on missing fill and browser-response readback
operations, and the native-page slice failed until screenshot capture existed;
each passed after adding the corresponding thin browser operation. No Rust
application or WASM behavior was changed to satisfy these cases.

Selected actual screenshots were inspected and committed for reviewers. Panel
locator capture scrolls the iframe into view before full-page capture; the
standalone page includes both records and coverage warnings:

![Synthetic fixture indirect path and neighborhood](t16/fixture-graph.png)

![Real Acy/Bogut evidence with both tenure records](t16/real-evidence-page.png)

![Focused selected-edge evidence panel](t16/real-evidence-panel.png)
