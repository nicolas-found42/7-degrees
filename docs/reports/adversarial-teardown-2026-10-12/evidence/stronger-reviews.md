# Independent stronger reasoning reviews

Two read-only reviews were run under the installed `code-review` skill's explicit parallel Standards/Spec workflow, after Jev escalated the final patch/gate. Fixed point: `a6382963ad83f0220f6d6cb36f49faa17c657bb3`; no commits since it, review includes the actual working-tree diff and new report/probe. Reviewers inspected source, test diff, manifest and logs; they did not rerun unchanged Jev decisions. These are reasoning reviews of source/evidence, not independent historical-source corroboration.

## Standards

No hard violations of the documented repository standards, actionable baseline smells, or essential production-code concerns found in the reviewed diff.

The implementation respects the glossary’s separate player identities. Duplicate labels gain escaped era/ID context while their canonical option values and preselection remain unchanged. `name_counts` is populated from the same player slice it indexes, so the new lookup has a valid invariant. Missing metadata falls back to IDs. Search/profile IDs pass through the existing HTML escaping function. The public `chain_view::page` signature is preserved; the modified `ui::home` function belongs to a private module. No new dependency, credential requirement, source-data change, or graph algorithm change appears.

The small wrapper around `page_with_reports` serves compatibility and is therefore not an actionable Middle Man heuristic. The synthetic fixture construction follows the existing integration-test pattern; the new regression covers the independent same-name/same-era case rather than duplicating a production assumption.

Following `docs/agents/verification.md`’s receipt rules, I checked the actual manifest and published logs. The recorded tracked-diff SHA256 exactly matches the current diff (`129a0d60291959fd9a9b99e37926f144b136794fb43309a34297ef18307c159b`). All ten published log hashes match their manifests. Actual result lines substantiate **129 Rust tests, 108 Python tests, and seven browser/canvas journeys passing**, with every check exiting zero. Browser serialization changes execution order without removing tests or assertions. Its documented causal limitation is appropriate.

The replayable probe uses the standard library and a local server. Its receipts support 39 duplicate-label groups before and zero afterward, unchanged graph/proof/page results, and the stated narrow precision/completeness results. These do not establish independent historical truth or broad retrieval performance.

One deliverable reconciliation remains: `docs/reports/adversarial-teardown-2026-10-12/README.md` still contains pending-final-check language in its status, R07, findings, and closing paragraph. Update those statements to the actual default receipts and final judgment disposition before presenting the report as finished.

The patch and check claims can stand under stronger source review. No essential code concern remains from this axis; final report/judgment reconciliation is still required. **Standards: zero hard violations, zero actionable smells, one unfinished report-state update.**

## Spec

One delivery gap remains in the report as inspected; no incorrect implementation or scope creep was found in the accepted changes.

- **Finalize completion evidence and status.** The specification requires, “Record the revision or artifact hash, initial worktree state, environment, commands, outputs,” and “State plainly whether the work is complete or partial.” The report still says final checks/gate are pending, F03 awaits default verification, and actual end/elapsed time will be appended. Those placeholders are now stale: `final-checks/review-summary.json` and the actual logs establish all ten default checks passed, with **129 ordinary Rust tests, 108 Python tests, and seven explicitly executed browser/canvas tests**. Update the coverage/dispositions, link these receipts, record elapsed time and the expanded budget, and document this stronger review’s resolution of the preserved Jev escalations before claiming completion.

The implementation satisfies the relevant README commitment: “Results show stable IDs, era and franchise context. Same-name people such as the two Dee Browns remain separate choices.” Duplicate-only option labels use report eras and canonical IDs, preserve option values and selection, and provide an escaped ID fallback without metadata. Search/profile context exposes escaped IDs. The public `chain_view::page` interface is preserved; `ui` is private. Graph/data/source/manifests are untouched.

The original F01/F02 regression logs fail at the intended missing-label/missing-ID assertions. Final checks exercise their passing counterparts plus identical-era and metadata-free escaping cases. The browser diff retains previous assertions and adds visible-label selection, preselection, narrow-viewport bounds, and keyboard submission. The unsupported popup-arrow assumption was a newly introduced probe; its replacement does not weaken an original product assertion. Popup-arrow accessibility remains unverified.

F03 changes only browser-test concurrency, with explicit loss of parallel speed. Passing serial tests support this operational mitigation, not a proven timeout cause or general reliability guarantee.

The HTTP consumer checks distinct graph boundaries: all edge pages, proof structure/arithmetic, and independent BFS agreement on the served graph. Its report correctly separates those from historical-source completeness. Six labeled positive queries support only the stated 7/7 precision and completeness sample. No new execution dependencies were added; isolated startup supports credential-free operation of the existing built app.

**The implementation and bounded factual claims can stand after stronger reasoning review. The remaining essential gap is final report/judgment resolution and completion bookkeeping, not a missing product fix or check.**

## Host reconciliation

The cited reporting gap is resolved in the final report: default receipts linked, F01–F03 dispositions and R07 updated, actual elapsed time and budget extension recorded, and the preserved Jev escalation explicitly distinguished from automatic approval. [Deterministic claim audit](deterministic-claim-audit.json) independently parses the actual log result lines and check exits: 129 Rust, 108 Python, seven browser/canvas and ten zero exits. No report-state update changes the checked tracked source diff. No essential implementation/check concern remains from either axis; coverage exclusions remain explicit.
