# Source Manifest

Pinned data snapshot for the NBA/BAA teammate graph. Acquired 2026-10-09 (UTC).
All raw artifacts live under `data/` (gitignored); this manifest is the committed record
of what was acquired, verified, and why.

## Sources

### S1 — Kaggle `wyattowalsh/basketball` ("NBA Database", nbadb)

- URL: https://www.kaggle.com/datasets/wyattowalsh/basketball
- Version: 238 (files dated 2026-06-29; lastUpdated 2026-06-29T23:54:58Z)
- License: CC BY-SA 4.0 (attribution + share-alike)
- Upstream: github.com/wyattowalsh/nbadb; underlying data from stats.nba.com
- Artifact: `data/basketball.zip` — 730,695,607 bytes — sha256 `8145cbb7586e6c0a5fbc9ad73b1ed7badb5004182ac66ac069a07dddc8490ada`
- Uncompressed: 20 files, 4,660,601,876 bytes — exactly matches the Kaggle
  dataset-view `totalBytes`. The dataset-list figure (730,695,607) is the compressed
  archive size. The earlier metadata discrepancy is resolved against the real artifact.
- Zip contents verified against the Kaggle file inventory: all 20 files present with
  matching sizes.
- `nba.duckdb` is a 12,288-byte stub — do not use. `nba.sqlite` is the populated file.

**Schema finding (verified against the artifact):** the published 261-table star schema
(`docs/lib/generated/star-reference.json` on the nbadb repo main branch, including
`bridge_player_team_season` and `fact_player_game_log`) is **not present in v238** —
the SQLite contains only the 16 legacy tables. Player-season-team membership and
per-game player appearance logs are therefore **not available from this artifact**.

Legacy tables inventoried (SQLite `nba.sqlite` row counts, verified against the artifact):

| Table | Rows | Notes |
|---|---|---|
| player | 4,815 | NBA API person ids, names, is_active |
| team | 30 | id, full_name, abbreviation, city, state, year_founded |
| team_history | 50 | franchise eras (team_id, city, nickname, years); eras end at 2019 — stale, do not rely on for aliases |
| team_details | 27 | current arena/owner/coach details |
| team_info_common | 0 | empty |
| common_player_info | 3,632 | biographical + from_year/to_year + last team |
| game | 65,698 | game-level rows with game_date, both teams' stats, season_type |
| game_info | 58,053 | attendance, game_time per game |
| game_summary | 58,110 | game metadata |
| inactive_players | 110,191 | DNP/inactive player per game (partial player-game signal) |
| line_score | 58,053 | per-game line scores |
| officials | 70,971 | officials per game |
| other_stats | 28,271 | team-game paint/fast-break stats (modern era) |
| play_by_play | ~2.26 GB | event-level play-by-play with player ids (1996-97 onward) |
| draft_history | 8,257 | person_id, season, pick, drafting team |
| draft_combine_stats | 1,633 | combine measurements |

### S2 — Kaggle `sumitrodatta/nba-aba-baa-stats`

- URL: https://www.kaggle.com/datasets/sumitrodatta/nba-aba-baa-stats
- Version: 56 (updated 2026-04-13T13:41:55Z)
- License: CC0: Public Domain
- Artifact: `data/nba-aba-baa-stats.zip` — 11,152,505 bytes — sha256 `5be35c2837020214a98148f42c21f90bfba0ea1c75d2b3ab8ff6b91baaa4917f`
- Contents: 22 CSVs, scraped from Basketball-Reference (independent lineage from S1).
- Key tables (verified):
  - `Player Season Info.csv` — 33,339 rows (NBA 31,119 / ABA 1,638 / BAA 582);
    seasons 1947→2026; columns `season, lg, player, player_id, age, team, pos, experience`.
    NBA/BAA rows: 31,701 player-season-team rows across **5,105 distinct players**;
    multi-team seasons carry a `2TM`/`3TM` summary row plus one row per team (no TOT rows);
    2,875 player-seasons with 2+ teams.
  - `Team Abbrev.csv` — 1,818 season-level team identities (`season, lg, team, abbreviation,
    playoffs`); 96 distinct team names, 104 abbreviations; includes all 16 BAA teams
    (BLB, BOS, CHS, CLR, DTF, FTW, INJ, MNL, NYK, PHW, PIT, PRO, ROC, STB, TRH, WSC) and ABA teams.
  - `Player Career Info.csv`, `Player Per Game.csv`, `Player Totals.csv`, `Per 36 Minutes.csv`,
    `Per 100 Poss.csv`, `Advanced.csv`, `Player Shooting.csv`, `Player Play By Play.csv`,
    `Team Summaries.csv`, `Team Totals.csv`, `Draft Pick History.csv`, `All-Star Selections.csv`,
    `End of Season Teams.csv`, `Player Award Shares.csv`, opponent stats files.
- Role: **primary source for player-season-team membership and the franchise-identity
  crosswalk** (season-level team aliases across eras) — S1 lacks both — and the
  independent cross-check for the player universe.

### S3 — Kaggle `romainmorleghem/nba-players-info-and-headlinestats-up-to-2025`

- URL: https://www.kaggle.com/datasets/romainmorleghem/nba-players-info-and-headlinestats-up-to-2025
- Version: 1 (updated 2025-10-17T09:27:34Z)
- License: MIT
- Artifact: `data/nba-players-info-and-headlinestats-up-to-2025.zip` — 242,325 bytes — sha256 `6289efa9198f46f2b15c9a8ea7cfbb477ddf376e645449df065afe484f781ae3`
- Contents: `CommonPlayerInfo_ALL.csv` (5,135 rows, 5,135 unique PERSON_ID — built from
  the NBA API `commonplayerinfo` endpoint), `PlayerHeadlineStats_ALL.csv`.
- Role: independent player-universe cross-check (third source).

## Acquisition record

- Requests: one bulk download per dataset (3 total), plus read-only metadata API calls
  (dataset view / file inventory). No per-season or per-page request storms; no
  rate-limit incidents.
- Follow-up research pass (2026-10-09): per-file Kaggle CSV downloads and one-shot HTTP
  probes recorded in `docs/research/nba-data-sources.md` (request accounting there); the
  `deocheng/nba-data-1946-2026` candidate is not pinned or imported (license not stated
  on its dataset page).
- Credentials: Kaggle API token at `~/.kaggle/access_token` (mode 600). Not committed.
  `data/` archives, HTML cache, SQLite and pipeline checkpoints are gitignored; credentials are not committed. Derived identities, tenure CSVs, parsed transaction records, source-page hashes, request ledgers and audit metadata are committed under `docs/reports/`. These derived records include source factual text; they are not the raw ignored artifacts.

## Pinned verification and local rebuild

The [machine-readable pins](snapshot-pins.json) record the three exact archives and the final identity/tenure hashes. Verify an existing local snapshot without downloading:

```sh
python3 scripts/verify-source-snapshot.py --data-dir data
```

The verifier streams archive hashes, checks size/file count, and verifies all 80 cached transaction pages against `docs/reports/t4/transaction-source-pages.csv`. It also checks committed T3/T4 hashes. The final verification used the existing raw snapshot and made zero network requests. A mismatch is a changed snapshot, not permission to silently relabel it.

S1 extracts its populated `nba.sqlite` directly under `data/`; S2 CSVs live under `data/sumitrodatta/` and S3 under `data/romainmorleghem/`. Rebuild with the scripts' actual supported arguments:

```sh
python3 scripts/t3_reconcile.py data
python3 scripts/t3_report.py
python3 scripts/t4_fetch_bbr.py data --no-fetch
python3 scripts/t4_reconcile.py data
python3 scripts/t4_report.py data
```

`--no-fetch` parses the existing cache without HTTP. T3 report takes **no data-directory CLI argument**; it reads this checkout's `data/t3/.state.pkl`, falling back to sibling `../7-degrees/data` when this checkout has no data directory. The other commands take the optional positional data directory. All scripts write reports relative to their own checkout; isolated worktrees can point preparation at an existing snapshot while preserving the raw archives/cache. Reconciliation checkpoints remain ignored. The current corrected tenure hash is `bfe9f5f2b60615af1160a157f7c482ee7ea3b9c86390e28f87a46c863ae39c04`. Hash/diff the derived outputs after a rebuild and inspect disagreements before updating pins.

## Deliberate future refresh

`kaggle datasets download -d <slug> -p <new-snapshot-directory>` obtains the provider's current release, **not a guaranteed version 238/56/1**. Preserve the pinned archives and extracted data. Stage each of the three named slugs above in a new dated directory, record its actual version/update metadata, archive SHA-256, bytes, file inventory and request count, and extract into the expected source subdirectories. Existing machine credentials can authenticate Kaggle; do not copy them into artifacts. Compare schemas before rerunning T3/T4, retain identity/membership/date disagreements, and update this manifest and snapshot pins only after review.

The transaction fetcher's page list is explicitly BAA 1947–49 and NBA 1950–2026. `python3 scripts/t4_fetch_bbr.py <data_dir>` fetches **missing** cache pages only. Existing cached pages are never refreshed by that command; there is no `--force`/`--refresh` option. A new-season update requires reviewing the bounded year list, preserving old HTML/ledgers in the old snapshot, and using a new snapshot cache for changed pages. Request starts are spaced at least 5.2 seconds; transient 429/503/Cloudflare 1015/network failures get at most one retry after 45 seconds. Fetch failures remain gaps. See the [fetch/request accounting](../reports/t4-tenure-coverage.md#4-supplemental-web-retrieval--bref-transaction-pages-request-accounting) and committed `t4/bbr-request-ledger.csv`.

## Supplemental sources and attribution

Basketball-Reference league transaction pages are acquired, not merely planned: **80 cached pages**, 30,327 parsed paragraphs/records, **79 scripted HTTP requests plus one prior manual probe**. Later parsing/correction passes use those same bytes with zero additional source requests. Page URL/SHA-256/raw list-item and paragraph locators connect the derived records to the retained cache. Missing/fuzzy dates and non-NBA associations remain explicitly classified; the cache's availability does not establish complete roster tenure coverage.

Attribute S1 to [Wyatt Walsh/nbadb](https://github.com/wyattowalsh/nbadb) and its NBA statistics lineage, S2 to [Sumitro Datta](https://www.kaggle.com/datasets/sumitrodatta/nba-aba-baa-stats) and Basketball-Reference, and S3 to [Romain Morleghem](https://www.kaggle.com/datasets/romainmorleghem/nba-players-info-and-headlinestats-up-to-2025) and NBA API common-player information. S1's stated [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) requires attribution, notice of modifications and share-alike for adapted material. Derived reconciliation changes and exclusions are documented in T3/T4 reports; do not remove the source notices when distributing covered adaptations. S2's published CC0 and S3's published MIT labels are recorded source declarations; upstream content can retain separate rights.

Basketball-Reference is published by [Sports Reference](https://www.sports-reference.com/) and its [terms](https://www.sports-reference.com/termsofuse.html) govern use of that source. No unrestricted redistribution right for its cached HTML or source paragraphs is asserted here. Raw copyrighted pages/full audit paragraphs remain outside the repository; the portable audit publishes locators, hashes, outcomes and model signals. Review applicable upstream rights before a new public data distribution; this manifest does not relicense source material. Candidate `deocheng/nba-data-1946-2026` remains unlicensed/unimported.

## Source coverage gaps (carried forward to T3/T4; GLOSSARY.md "Source coverage gap")

- No transaction dates in the pinned bulk sources. Acquired supplemental dated source:
  Basketball-Reference league transaction pages — verified live 2026-10-09 from the first
  BAA season (`BAA_1947_transactions.html` HTTP 200, "Transactions listed are from
  July 1, 1946 to June 30, 1947"; `BAA_1948` and `NBA_1950` verified likewise — an earlier
  "1951 onward / HTTP 429" report was wrong, the 429 was transient) — acquired cached
  and throttled as recorded above. S4 candidate `deocheng/nba-data-1946-2026` `public.transactions.csv`
  (28,667 rows, 1946-12-12 → 2026-06-24, mostly season-resolution) per the 2026-10-09
  research pass — license not stated on the dataset page, not pinned, do not import before
  the license is cleared.
- No per-game appearance log for active players in S1 (`inactive_players` covers DNPs
  only; `play_by_play` covers 1996-97 onward events only).
- 1946-1950 BAA tenure evidence: dated transaction pages are reachable for every season
  from 1946-47 (verified above, with player and franchise slugs hyperlinked), so the
  remaining gap includes incomplete movement coverage and bounded day-precision parsing (fuzzy-dated rows such as
  "November ?, 1947"). Reachable pages alone do not certify uninterrupted service or both roster boundaries. Wikipedia season-transaction lists mirror
  the same movement as a structured cross-check.
- S1 `team_history` franchise eras end at 2019 (stale); the franchise-identity crosswalk
  comes from S2 `Team Abbrev.csv` (aliases across eras), with NBA Hoops Online "Team
  Roots" lineage (reached HTTP 200 on 2026-10-09) as a defunct-franchise secondary
  cross-check.
- S1's star-schema tables (bridge/fact/dim) are absent from v238; if a future nbadb
  version ships them, re-evaluate S2's primary role for membership data.
