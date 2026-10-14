# Adversarial teardown — 2026-10-11

Status: investigation in progress. Local changes only; no publication, deployment, source refresh, or live semantic evaluation is authorized by this run.

Baseline: `a6382963ad83f0220f6d6cb36f49faa17c657bb3`, clean tracked and untracked worktree. Target: the documented Rust NBA/BAA teammate explorer. macOS arm64, Rust/Cargo 1.98.1, Python 3.14.7, wasm-bindgen 0.2.129; installed wasm32 target. Initial full checks use `target/verification/adversarial-baseline-20261011`.

Working budget: 60 minutes, approximately 20 investigation, 20 implementation, 20 final verification/reporting; expand only for consequential new evidence. Core promises take priority over speculative features.

Persona: a simulated skeptical basketball researcher seeking Quincy Acy–Andrew Bogut and its evidence, with no semantic key, keyboard interaction, and a phone viewport. No source knowledge before initial UI attempt. This establishes software behavior, not population preferences. API probes use a real local HTTP consumer.

Acceptance criteria fixed before optimization:

| ID | Requirement | Observable success | Coverage |
|---|---|---|---|
| R01 | Install/start and no-key fallback | Documented wrapper starts pinned reports; deterministic routes succeed; semantic unavailability visible | Pending |
| R02 | Identity retrieval | Known/typo/Unicode queries find the correct identity, ambiguous names remain separate, nonsense does not silently select | Pending |
| R03 | Exact paths and pagination | Direct degree 1, historical degree 2, pages preserve all alternatives without duplicates; malformed requests reject | Pending |
| R04 | Evidence fidelity | 121 dated days for Acy–Bogut; appearance proofs preserve unknown duration; gaps stay explicit | Pending |
| R05 | Usable graph and recovery | Keyboard/text routes, phone layout, full network retention, invalid input recovery | Pending |
| R06 | Reliability and operational burden | Bounded probes do not break browsing; repeated first statistics work is evaluated | Pending |
| R07 | Regression and dependency eligibility | Required full profile passes; new dependencies meet accountless, credentialless, charge-free rules | Pending |

Historical source acquisition, exhaustive NBA coverage, live paid semantic accuracy, screen-reader participant testing, and cross-platform runtime behavior are excluded from completion claims. Existing paid/credentialed dependencies are baseline constraints; no new project dependency is presumed eligible.

Evidence, findings, experiments, decisions, and dispositions will be linked here before delivery.
