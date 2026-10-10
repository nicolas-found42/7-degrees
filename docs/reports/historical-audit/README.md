# Historical source-reading audit

This publication retains the original historical validation sample and the outcomes of completed original-source reading. It does **not** certify complete historical roster occupancy. The 6,246 selected rows come from the immutable original T4 snapshot, SHA-256 `f4fe081d35518782969f502c68c7ba622b987a4d7bf042a729bf2333ff18a030`, available in Git commit `474481bde27ac33887ae9f285e351735f0905571` at `docs/reports/t4/tenures.csv`:

```sh
git show 474481bde27ac33887ae9f285e351735f0905571:docs/reports/t4/tenures.csv > target/original-audited-tenures.csv
shasum -a 256 target/original-audited-tenures.csv
python3 scripts/check-historical-audit.py
python3 scripts/compare-audit-snapshot.py
```

`cases.csv`'s `csv_line` refers to that original file, including its header. `tenure-00082` means original CSV line 82. These IDs and lines are never retargeted to current data. The [export manifest](export-manifest.json) pins retained inputs, original source versions/hashes, method, seed, sample frames and quotas. [Artifact hashes](artifact-hashes.json) cover all metadata tables. The integrity checker recovers the Git snapshot, compares original keys/classes/intervals, verifies unique IDs, completed outcomes, groups, strata, source hashes, associations and current delta totals. It verifies receipt consistency rather than reperforming source reading or proving the paragraphs true.

## Selection and completed reading

The deterministic selection seed is `7-degrees-source-audit-2026-10-09-v1`; cases are ranked by SHA-256(seed + `|` + audit ID), with 12 per evidence class then remaining ranked rows to reach 50 where available. [Coverage](coverage.json) lists exact selected IDs for six 50-record strata: BAA 1946–49, early NBA 1950–66, expansion 1967–80, merger/transition 1976–78, modern 1981–99, and 2000–2025/26. Overlapping strata are not additive independent samples.

The union includes all **798 BAA/defunct cases** in the pinned selection frame and all rows belonging to **1,548 unusually complex player-season groups**. Selection reasons and group membership are retained. Partitions cover 3,141 main-host cases, 798 BAA/defunct cases and 2,307 modern cases: all 6,246 have completed original-source reading outcomes, with no pending selected cases/groups. This describes the audited sample/frame, not all historical NBA identities or all roster intervals.

| Pre-correction reading outcome | Count | Meaning |
|---|---:|---|
| Supported | 19 | Original bounded source check supported the selected record. |
| Supported transaction-anchor pair | 696 | Both selected movement anchors corroborated; does not prove uninterrupted service between them. |
| Insufficient dates | 3,567 | Original context could not establish the required exact dates. |
| Scope/date discrepancy | 1,964 | Original source scope/date problem retained for correction/review. |

These **pre-correction** outcomes are not counts of defects in the final graph. For example, semantic source-scope checks found G League movement that must not imply NBA roster tenure even when a model said “verified.” The host reading outcome preserves that distinction. Known mid-season moves and same-day/ambiguous movement are also retained through T4 [examples](../t4/examples.csv), [excluded events](../t4/excluded-non-roster-events.csv), and [review flags](../t4/unresolved-flags.csv).

## Portable receipts and boundaries of the evidence

- [Cases](cases.csv) preserve immutable keys, original class/interval, reasons, completed reading status/outcome and relative receipt pointers into retained partition files.
- [Complex groups](complex-groups.csv) preserve all 1,548 group IDs, reasons, member case IDs and completion status.
- [Sources](sources.csv) provide 21,984 distinct original CSV/HTML/SQL witness locators and SHA-256 where available; unknown hashes are blank. S1 SQL witnesses are tied to v238, game IDs, row IDs and player slots. Seven S1-only supplement cases retain actual appearance witnesses; appearance is not a contract boundary.
- [Case-source associations](case-sources.csv) contain 123,278 deterministic links to retained dossier context, boundary/date roles, team mapping and official appearance witnesses. Associations identify the source context available/read; their count is not a count of independent readings or newly certified facts.
- [Model signals](model-signals.csv) preserve Jev verdicts, distributions, confidence where present, original signals and operational repair pointers separately from source-reading outcomes. Missing fields are unknown. Initial oversized or invalid model judgments were repaired in the retained records; model output alone never substitutes for reading or deterministic graph validation.

Raw source paragraphs, full 26 MB case JSONL and detailed dossiers remain in the separately retained historical-audit directory; they are not copied into this publication. Relative receipt pointers and input hashes allow an owner of that evidence to inspect the original records. Cache HTML/SQLite/archives remain ignored, unchanged and retained. The metadata export can be reproduced from the retained root with `python3 scripts/export-historical-audit.py --audit-root <retained-audit-root>`. This command performs a deterministic export, not new source inspection or new model inference. The current committed compact files can be checked without that external root.

## Comparison with the final corrected snapshot

[Corrected delta](corrected-delta.csv) rejoins each original case to current T4 by `(bbr_player_id, league, season, canonical franchise)` and compares exact dates. Current SHA-256 is `bfe9f5f2b60615af1160a157f7c482ee7ea3b9c86390e28f87a46c863ae39c04`; its current CSV lines are explicitly separate from the original lines. [Summary](corrected-delta-summary.json):

| Deterministic comparison | Cases |
|---|---:|
| Original selected cases / current membership groups found | 6,246 / 6,246 |
| At least one unchanged exact interval | 2,364 |
| Originally directly evidenced | 1,812 |
| Original direct interval still direct at same dates | 503 |
| Group has any corrected positive directly evidenced interval | 1,053 |

This comparison is not a new source reading, uninterrupted-service certification, or proof of an edge. The Rust loader additionally checks identity, anchor flags, blocking reasons and pairwise overlap. Its actual final graph admits 2,227 certified tenures and 1,501 edges; 3,537 players have no certified tenure. The [runtime report](../final-verification/README.md) and coverage warnings retain this limitation.
