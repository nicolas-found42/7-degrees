# Retrospective environment improvements

Implemented 2026-10-10 after visualizer commit `7f51096`. Operational instructions live in [verification](../../agents/verification.md) and [coverage navigation](../../agents/source-coverage.md).

The verification runner now executes argument vectors and records real exit codes, times, source state, log hashes, test names and deterministic totals. Its compact summary keeps log references without duplicating full output. Publication validates all receipts before writing and refuses failed/incomplete/tampered input or an existing destination. Regression tests cover a failed final command, early termination, missing/changed logs, incomplete manifests, grouped test arithmetic, archive-free derived checks, and exact preservation of a generated PR appendix.

The reviewed tenure hash was synchronized in the current snapshot pins and source manifest. Historical receipts were preserved. Portable consistency checks now require no ignored raw archives; raw archive/cache verification remains separately available.

GitHub Actions runs portable and explicit browser jobs with failure-receipt uploads. Browser tooling is installed in runner temporary storage; wasm-bindgen CLI follows Cargo.lock. The existing graph interaction regressions are documented as a state matrix and included in the check profiles. Browser response-body capture is opt-in for the two credential-canary journeys; the full-network journey reads DOM/payload state without retaining response bodies.

The PR helper reads just the maintained prefix and composes a replacement while preserving the recognized Qodo appendix exactly. Unknown Qodo boundaries require inspection. Short conditional pointers in AGENTS.md lead source and delivery work to the appropriate reference docs.

## Observed verification

- [Full-profile receipts](checks/manifest.json): 112 Rust passed / 0 failed / 10 ignored; 102 Python passed; 7 explicit browser passed / 0 failed. Formatting, host/WASM Clippy, committed-derived pins, immutable historical audit, frozen semantic audit and WASM build all exited 0.
- [Compact review evidence](checks/review-summary.json): deterministic counts and log pointers, without repeated full logs.
- [Raw snapshot receipts](raw-checks/manifest.json): existing pinned archives/cache plus derived hashes verified with zero source requests.
- [Final utility receipts](utility-checks/manifest.json): all eight verification/PR-helper regressions passed after adding compact-summary publication. The earlier full-profile receipt predates that final utility-only change; its source state is retained accurately.

These are local execution receipts. Remote workflow outcomes should be read from the current GitHub Actions run, not inferred from these files. No historical coverage expansion or graph-data regeneration was performed for this maintenance work.

## Fresh-runner browser correction

The first GitHub browser job exposed resource 404 errors on every page in full Chromium. Local headless-shell runs had not requested the implicit favicon. The document shell now declares an inline empty icon, and the driver reports HTTP error URLs independently of optional response-body capture. The fixture journey passed locally with full Chromium after this correction. [Fresh full-profile receipts](browser-fix-checks/manifest.json) again record 112 Rust, 102 Python and seven explicit browser tests passing, plus all format, Clippy and data/audit checks. The original failed GitHub run remains visible in Actions; subsequent run results should be read there.
