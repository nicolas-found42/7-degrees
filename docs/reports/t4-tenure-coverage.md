# T4 — Roster Tenure Reconstruction and Coverage Report

Ticket: #6 (*T4: Reconstruct roster tenures and report coverage*) · parent #1 ·
spec: `docs/specs/nba-teammate-degrees.md` · glossary: `GLOSSARY.md` ·
inputs: T3 artifacts (`docs/reports/t3/`), S1 `game` season windows,
cached Basketball-Reference transaction pages (this report §4).

Report generated 2026-10-09 08:09 UTC by `scripts/t4_reconcile.py` + `scripts/t4_report.py`
(fetch pass: `scripts/t4_fetch_bbr.py`; tests: `scripts/test_t4_core.py`,
`scripts/test_t4_reconcile.py`, `scripts/test_t4_fetch_bbr.py`). Artifacts retained under `docs/reports/t4/`.

> **No complete-coverage claim.** This report never claims complete exact-date
> coverage (spec: "Do not claim complete date-level coverage until the evidence
> supports it"). Tenures are dated intervals where evidence allows; every
> interval below is an *estimate between* evidenced bounds, and the evidence
> class states how strong the estimate is.

## Verdict at a glance

| question | result |
|---|---|
| membership rows reconstructed (universe players, NBA/BAA) | 28,836 rows (S2 28,826 + S1-only supplements 10) |
| tenure records (incl. multi-stint and unresolved membership rows) | 31,973 records from 28,836 membership rows; 29,099 have interval bounds, 2,874 have no constructible interval |
| season-window coverage on tenure records | 28,652 with a season window; 3,321 without one |
| directly evidenced tenures (both bounds from dated transactions) | 3,565 (11.2%) |
| cross-checked (transaction + independent season-window agreement) | 10,713 (33.5%) |
| inferred (season-window bracket only) | 12,385 (38.7%) |
| unresolved (flagged, not invented) | 5,310 (16.6%) |
| distinct player pairs with a positive tenure overlap (potential edges) | 137,759 (44,506 both-evidenced / 82,839 involves-inferred / 10,414 involves-unresolved) |
| 1946-1950 BAA source coverage gap | explicitly carried (§6; BAA-era evidence is structurally thinner) |

## 1. What a tenure is here

**Roster tenure** (GLOSSARY.md): a time-bounded period in which a player was on
an NBA/BAA team's roster, reconstructed from the best available dated source
records. In this artifact it is a **half-open dated interval** `[start, end)`
in integer day numbers — end exclusive, matching
`crates/graph-core/src/lib.rs` (`Tenure { start, end }`, "end is exclusive")
and the spec fixture (`[day 1, day 5)`). A dated departure is the FIRST DAY OFF
the roster; a one-day stint spans `[D, D+1)`. Mid-season moves yield separate
interval rows per stint. A teammate edge later requires a *positive overlap*
of two tenures on the same franchise — adjacency alone (`[a,b)` + `[b,c)`)
creates no edge day and no edge.

Membership baseline: T3's canonical player-season-team rows (S2 NBA/BAA rows
plus the 12 S1-only supplements from T3 §5), 5,105-player universe, canonical
franchise ids from `franchise-crosswalk.csv`.

## 2. How tenures are constructed

For each (universe player, canonical franchise, season) membership row:

1. **Season-window bracket** — first/last game dates for that team-season from
   the S1 `game` table (`Regular Season` + `Playoffs` team rows; read-only
   SQLite), converted to half-open day bounds. This brackets the roster tenure
   in time; season membership alone is NOT treated as proof of simultaneous
   tenure (spec: Temporal evidence). S1's `game` table ends with the 2022-23
   season — later seasons get their transaction-dated bounds but no independent
   window, hence a weaker class (§3 note).
2. **Dated transaction refinement** — BBR transaction legs (player slug,
   depart/arrive franchise, date) matching the player slug and the leg's
   date-derived season bucket. Every precise date is checked against its
   source page's parsed bounds; off-page but unambiguous dates are
   re-bucketed by date unless an S1 game window independently confirms an
   exceptional page-season extension (2019-20 bubble); missing or
   contradictory windows are flagged and excluded. Arrival events
   (sign/trade-in/claim/dispersal) anchor the start; departure events
   (trade-out/waive/release/sale) anchor the end. Fuzzy dates
   ("February ?, 1947") are NEVER resolved to a day — the rows are retained as
   `fuzzy_events` and only flag tenures for review.
   A precise offseason arrival carries forward only when adjacent team-season
   windows bracket the dated move and the same player/team has next-season
   membership; its transaction date remains the tenure start boundary.
   **Cross-season example:** Shareef Abdur-Rahim's NBA_2001 VAN→ATL move
   on 2001-06-27 anchors the 2002 ATL membership at 2001-06-27 (cross-checked; [2001-06-27, 2002-04-18)).
3. **Stint walking** — arrivals/departures in day order produce stints;
   conventions as in §1. Ordering anomalies and same-day conflicts are flagged.
4. **Classification** — `classify_tenure`: unresolved reasons dominate;
   both bounds transaction-anchored → directly-evidenced (the clean pair;
   season-window agreement is recorded alongside in
   `tenures.csv.season_window_iso` — a confirmed-both-sides pair, the
   strongest possible bounds, is still reported as directly-evidenced);
   one bound + agreement → cross-checked; otherwise inferred.

## 3. Evidence classes — counts

| evidence class | tenures | share | meaning |
|---|---|---|---|
| directly-evidenced | 3,565 | 11.2% | both bounds anchored by dated transaction rows (season-window agreement recorded alongside when present) |
| cross-checked | 10,713 | 33.5% | one transaction-anchored bound + independent season-window agreement (two sources corroborate the bounds) |
| inferred | 12,385 | 38.7% | season-window bracket only (membership season + S1 games) |
| unresolved | 5,310 | 16.6% | flagged: conflicts / fuzzy / same-day ordering / ambiguous source evidence — needs review |

**Per era** (era boundaries per spec Source validation):

| era | tenures | directly-evidenced | cross-checked | inferred | unresolved |
|---|---|---|---|---|---|
| BAA 1946-49 | 512 | 4 | 106 | 359 | 43 |
| 1950-66 | 2,188 | 49 | 433 | 1,443 | 263 |
| 1967-80 | 3,431 | 147 | 824 | 1,785 | 675 |
| 1981-99 | 7,835 | 654 | 2,938 | 4,025 | 218 |
| 2000-2025/26 | 18,007 | 2,711 | 6,412 | 4,773 | 4,111 |

BAA-era tenures are mostly **inferred**: S1 `game` windows exist,
but BBR 1946-50 transaction rows are sparse and partly fuzzy-dated;
see the §6 gap callout.
2000-2025/26 tenures carry transaction bounds but lack the
independent S1 window past 2022-23 — transaction bounds with no
independent window report as directly-evidenced (§2 step 4), the
weaker of the two transaction classes.

## 4. Supplemental web retrieval — BRef transaction pages (request accounting)

`scripts/t4_fetch_bbr.py` fetched the bounded page set
`/leagues/{BAA|NBA}_{year}_transactions.html` (BAA 1947–1949, NBA 1950–2026 =
80 pages) **cache-first, with a code-enforced 5.2 s minimum start spacing using a
monotonic clock, with one 45 s back-off + single retry on 429/503/Cloudflare-1015**.
The historical request ledger has whole-second timestamps; its observed minimum
adjacent timestamp delta was 4.0 s, not proof of a ≥ 5.2 s minimum. New HTTP ledger rows
record subsecond UTC start times and measured monotonic elapsed spacing. Every
request and cache hit is retained in `docs/reports/t4/bbr-request-ledger.csv`.
Numbers below separate this pass from the ledger's lifetime record.

| fetch metric | value |
|---|---|
| pages targeted | 80 |
| **this pass: HTTP requests issued** | 0 |
| **this pass: cache hits (no request)** | 80 |
| …this pass bytes fetched | 0 |
| request ledger lifetime: HTTP requests issued | 79 |
| historical whole-second timestamp precision / minimum adjacent delta | whole-second; 4.0 s (not proof of configured minimum) |
| …by status | 200: 79 |
| request ledger lifetime: cache hits (no request issued) | 641 |
| retry attempts | 0 |
| retries that then succeeded | 0 |
| **unresolved fetches (pages)** | 0 |

| parse metric | value |
|---|---|
| transaction rows parsed from cache | 30,327 |
| rows with precise dates | 30,250 |
| rows fuzzy/undated (kept, flagged; never guessed) | 77 |
| dated movement legs extracted | 34,770 |
| legs usable as interval anchors (no blocking flags) | 34,165 |
| precise-dated legs re-bucketed from a different page season | 71 |
| legs excluded: ambiguous or same-bucket date outside page window | 0 |
| legs flagged unusable team/shape (row retained; overlaps with other flags) | 605 |

**Season-bucket guard (BBR page-repetition leak):** BBR repeats major
trades on the NEXT season's page (the Perkins trade dated 2011-02-24
appears on both NBA_2011 and NBA_2012), and some rows sit outside
dates with valid page windows are re-bucketed to the season their own
date determines unless an S1 game window independently confirms an
exceptional page-season extension (the 2019-20 bubble); identical
movement days collapse to one anchor. If the page window is missing/
malformed or otherwise contradictory, the leg stays in the CSV, is
flagged, and excluded as ambiguous:

| season/window validation | legs |
|---|---|
| precise date re-bucketed to its own season (usable; duplicate anchors collapse) | 71 |
| page season confirmed by independent S1 game window | 12 |
| ambiguous or contradictory page window (excluded from anchor use) | 0 |

This pass had zero unresolved fetch failures. If a future fetch summary
contains a persistently failed page, reconcile marks every tenure in
that page's league-season and the next membership season unresolved with
`fetch-failure:<league>_<year>`; absence of a fetched transaction page is
never evidence that an underlying historical record did not exist.

Re-run idempotency: with the cache present the pass issues **zero** HTTP
requests and just re-parses the cached pages.

## 5. Examples per evidence class

| evidence_class | bbr_player_id | display_name | season | era | canonical_franchise | interval_iso | reasons_or_note |
|---|---|---|---|---|---|---|---|
| directly-evidenced | beendha01 | Hank Beenders | 1948 | BAA 1946-49 | WARRIORS | [1948-01-15, 1948-05-01) | (clean) |
| directly-evidenced | halbech01 | Chick Halbert | 1949 | BAA 1946-49 | CELTICS | [1948-05-01, 1949-01-16) | (clean) |
| directly-evidenced | youngch01 | Chris Youngblood | 2026 | 2000-2025/26 | THUNDER | [2025-09-26, 2026-02-06) | repeat-signing-continues-open-stint |
| directly-evidenced | youngja05 | Jahmir Young | 2026 | 2000-2025/26 | HEAT | [2025-09-11, 2026-04-11) | repeat-signing-continues-open-stint |
| cross-checked | beckemo01 | Moe Becker | 1947 | BAA 1946-49 | CELTICS | [1946-12-12, 1947-03-31) | (clean) |
| cross-checked | beckemo01 | Moe Becker | 1947 | BAA 1946-49 | IRONMEN | [1946-11-02, 1946-12-12) | (clean) |
| cross-checked | zelleco01 | Cody Zeller | 2023 | 2000-2025/26 | HEAT | [2023-02-20, 2023-06-13) | (clean) |
| cross-checked | zubaciv01 | Ivica Zubac | 2023 | 2000-2025/26 | CLIPPERS | [2022-06-28, 2023-04-26) | (clean) |
| inferred | abramjo01 | John Abramovic | 1947 | BAA 1946-49 | IRONMEN | [1946-11-02, 1947-03-27) | (clean) |
| inferred | aubucch01 | Chet Aubuchon | 1947 | BAA 1946-49 | FALCONS | [1946-11-02, 1947-03-30) | (clean) |
| inferred | youngtr01 | Trae Young | 2023 | 2000-2025/26 | HAWKS | [2022-10-19, 2023-04-28) | (clean) |
| inferred | yurtsom01 | Omer Yurtseven | 2023 | 2000-2025/26 | HEAT | [2022-10-19, 2023-06-13) | (clean) |
| unresolved | beckemo01 | Moe Becker | 1947 | BAA 1946-49 | FALCONS | [1946-11-02, 1947-03-30) | multi-team-season(ordering-unresolved): player has dated moves to other franchises this season; bracket-only stint cannot be ordered against them |
| unresolved | militna01 | Nat Militzok | 1947 | BAA 1946-49 | KNICKS |  | fuzzy-dated-rows-bear-on-this-tenure |
| unresolved | zubaciv01 | Ivica Zubac | 2026 | 2000-2025/26 | PACERS |  | no-season-window(S1-game-table-ends-2022-23-or-season-absent-from-S1); open-stint-without-window |
| unresolved | zubaciv01 | Ivica Zubac | 2026 | 2000-2025/26 | CLIPPERS |  | no-season-window(S1-game-table-ends-2022-23-or-season-absent-from-S1); departure-after-closed-stint(prior-spell-exit,or-source-noise) |

### 5a. Flags for review (unresolved, never invented)

`docs/reports/t4/unresolved-flags.csv` — all 5310 flagged rows:

| flag reason | rows |
|---|---|
| departure-after-closed-stint | 2748 |
| no-season-window | 1349 |
| same-day-arrival-and-departure | 1182 |
| open-stint-without-window | 1151 |
| no-dated-evidence | 1009 |
| repeat-signing-continues-open-stint | 665 |
| flagged-transaction-legs | 527 |
| stint-entirely-outside-team-season-window | 422 |
| same-day-arrival-to-multiple-franchises | 113 |
| fuzzy-dated-rows-bear-on-this-tenure | 71 |
| multi-team-season | 54 |
| arrival-after-window-end | 25 |
| interval-construction-failed | 8 |

**Same-day / ordering-unresolved examples** (each retained verbatim for
review; none resolved by invention):

| player | season | franchise | reasons |
|---|---|---|---|
| Maurice Ager | 2008 | MAVERICKS | same-day-arrival-to-multiple-franchises(ordering-unresolved) |
| Maurice Ager | 2008 | NETS | same-day-arrival-to-multiple-franchises(ordering-unresolved) |
| Santi Aldama | 2022 | GRIZZLIES | same-day-arrival-and-departure(ordering-flagged); repeat-signing-continues-open-stint; departure-after-closed-stint(prio |
| Santi Aldama | 2022 | GRIZZLIES | same-day-arrival-and-departure(ordering-flagged); repeat-signing-continues-open-stint; departure-after-closed-stint(prio |
| Santi Aldama | 2022 | GRIZZLIES | same-day-arrival-and-departure(ordering-flagged); repeat-signing-continues-open-stint; departure-after-closed-stint(prio |
| Santi Aldama | 2022 | GRIZZLIES | same-day-arrival-and-departure(ordering-flagged); repeat-signing-continues-open-stint; departure-after-closed-stint(prio |

## 6. The 1946-1950 BAA source coverage gap (explicit callout)

The pinned bulk sources carry **no transaction dates**, and per-player dated
evidence before 1996-97 is absent from S1 (`play_by_play` starts 1996-97; the
S1 `game` table provides only team-level season windows). For the **1946-1950 BAA
seasons the tenure evidence is therefore structurally thinner than for later
eras**: tenure records there rest on (a) S1 season windows and (b) the
sparse, partly fuzzy-dated BBR transaction rows for 1946-50. Per GLOSSARY.md,
a missing record is **not proof a tenure or edge did not exist**; per the spec,
uncovered intervals surface here instead of silently becoming graph facts.
Concretely:

- 512 BAA-era tenure records; 359 inferred (season-window only),
  110 transaction-evidenced (4 directly-evidenced + 106 cross-checked)
  — the dated transaction layer covers only a fraction of BAA movement;
- 198 dated movement legs parsed from the 1946-50 pages (BAA_1947..BAA_1949 +
  NBA_1950); 17 of the pages' 199 transaction rows there carry fuzzy/undated
  ("February ?, 1947" is a real row shape);
- all 1946-1950 franchise identities still resolve canonically (T3 crosswalk),
  so the gap is in *dated movement evidence*, not franchise identity;
- downstream (T5+) BAA-era teammate edges are therefore weaker-evidenced on
  average and the app must surface that provenance difference (T10).

## 7. What this feeds (T5 teammate edges)

| item | count |
|---|---|
| tenure intervals with constructed bounds | 29,099 |
| membership/franchise-season tenure records (incl. unresolved no-interval rows) | 31,973 |
| distinct player pairs with positive tenure overlap (potential teammate edges, pre-dedup across teams) | 137,759 |
| …both tenures transaction/season-agreement evidenced | 44,506 |
| …at least one inferred tenure | 82,839 |
| …at least one unresolved tenure (edge blocked until resolved) | 10,414 |
| membership duplicates/summary rows excluded (2TM/3TM/TOT) | 2,875 |

Pair counts are *upper bounds on edges*: the graph build deduplicates
repeated overlaps to one edge per pair (spec AC15) and resolves franchise
continuity; unresolved tenures do not silently produce edges.

## 8. How to reproduce

```bash
# from the repo root; requires the pinned data snapshot under data/ (docs/data/source-manifest.md)
# 1) one-time cached throttled fetch (~8 min; subsequent runs are cache-only, 0 requests)
python3 scripts/t4_fetch_bbr.py [data_dir]
# 2) reconstruction + classification
python3 scripts/t4_reconcile.py [data_dir]   # writes docs/reports/t4/*.csv + data/t4/.state.pkl
# 3) this report (accepts the same data_dir argument)
python3 scripts/t4_report.py [data_dir]
# unit tests (no data dependency, plain python3)
python3 -m unittest scripts.test_t4_core scripts.test_t4_reconcile scripts.test_t4_fetch_bbr
```

Python 3.9 stdlib only (`sqlite3`, `csv`, `urllib`, `html.parser`, `pickle`).
"data/" sources are read-only (SQLite is opened in read-only mode);
cache pages and intermediate state are local under `data/cache/bbr/` and
`data/t4/`, excluded from git, and not included in the committed artifacts.

## 9. Open items for review (not silently resolved)

1. 5,310 tenure rows remain unresolved with reasons retained in
   `unresolved-flags.csv`; each flag names the exact ambiguity class.
2. Fuzzy/undated BBR rows (77) are retained (parsed CSV `row_flags`, fuzzy_events)
   and never guessed to dates; a manual pass could resolve month-precision rows
   against Wikipedia season-transaction mirrors (research doc §5).
3. Seasons past 2022-23 lack the independent S1 season window (`game` table
   ends 2022-23 S1): transaction-evidenced bounds stay `directly-evidenced`
   without cross-check until a later S1 refresh.
4. No unresolved fetches this pass. On a future persistent page failure,
   reconcile marks tenures in that page's league-season and the next
   membership season unresolved with `fetch-failure:<league>_<year>`;
   it is never proof that a historical record does not exist.
5. T3's 50 `S1-pbp-shows-other-teams-same-season` membership disagreements
   remain listed in `docs/reports/t3/membership-mismatches.csv`; T4 inherits
   the S2 row here (both rows appear when both sources carry them).

