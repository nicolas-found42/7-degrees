# Historical source coverage navigation

For changes to source acquisition, identity reconciliation, tenure certification or teammate evidence, start with these current artifacts:

- [Snapshot pins](../data/snapshot-pins.json): machine-readable archive versions and current derived-file hashes. [Source manifest](../data/source-manifest.md): actual schemas, licensing, cache layout and acquisition rules.
- [Latest relationship repair audit](../reports/link-repair/README.md) and its `audit.json`: accepted proof classes, current repair baseline and residual isolates. Regenerate a new coverage audit after changing the data; this report remains a dated receipt.
- [Tenure coverage](../reports/t4-tenure-coverage.md), `../reports/t4/unresolved-flags.csv` and `../reports/t4/coverage-counts.json`: unresolved boundaries and evidence classes. Relationship proof and full dated roster occupancy are distinct claims.
- [Historical audit](../reports/historical-audit/README.md) and `export-manifest.json`: immutable original reading cases, source locators and preservation hashes. Original audit IDs refer to the original snapshot; current delta summaries are separate joins.
- [Verification](verification.md): reproducible check profiles and publication of actual execution receipts.

Current source pins are authoritative for current file bytes. Older final-verification reports and original audit receipts describe their named snapshots. Preserve their original hashes and case IDs. Update current pins only after inspecting a reviewed data change, then run the committed-data consistency check; preserve raw source versions separately.

Use the source manifest's scripts and their current CLI arguments for reconstruction. A connected graph, zero isolates or zero missing edges relative to existing certificates does not establish complete historical relationships. Measure coverage against an explicit era/team/player or pair denominator and retain unresolved cases with source attempts.
