# NBA Data Source Research — inventory of sources beyond the spec's two Kaggle candidates

*Researched 2026-10-09. Feeds the closed source-inventory work under #3 and the open issues #5
(T3 cross-validation) and #6 (T4 tenure reconstruction). Findings were checked against direct
fetches on 2026-10-09, local Kaggle CLI downloads, and the authoring-pass document.*

**Jev handling:** fetched source descriptions were screened before use (`jev_screen`: `pass`,
injection 0.06), candidate sources were role-classified in one batch (`jev_classify`, 5 of 12
`auto`), 12 factual claims were verified against fetched/local evidence (`jev_verify`: 11
`auto-verified`, 1 flagged `review`), and the spec's BBR coverage claim was reconciled against
this pass's fetches (`jev_compare`: `contradicts`, p = 1.0). See the Jev summary at the end.

## Headline findings

1. **The spec's Basketball-Reference coverage claim is wrong.** The spec records "transaction
   pages for 1951 onward and none for 1947–1950". Fresh fetches on 2026-10-09 returned HTTP 200
   for `/leagues/BAA_1947_transactions.html` (covering July 1, 1946 → June 30, 1947) and
   `/leagues/NBA_1950_transactions.html` (July 1, 1949 → June 30, 1950). `jev_compare` judged the
   spec's claim contradicted by this evidence with probability 1.0. The spec should be corrected;
   the "material coverage gap" for BAA tenure reconstruction is smaller than recorded.
2. **BAA-era tenure evidence is better supplied than assumed.** BBR's earliest transaction pages
   carry dated rows — trades, player-rights sales, waiver wire, and dispersal drafts after the
   1947 franchise foldings — with player and franchise slugs hyperlinked. Some rows carry fuzzy
   dates ("November ?, 1947", "February ?, 1947"), which is a bounded parsing problem, not a void.
3. **A third bulk Kaggle dataset exists** — `deocheng/nba-data-1946-2026` (snapshot 2026-08-18,
   92 tables / 5.9 GB) — shipping a pre-aggregated transaction table
   (`public.transactions.csv`, 28,667 dated typed rows, 1946-12-12 → 2026-06-24), a
   name→BRef-slug id bridge, and unified player names. That materially helps #5 and #6. Its
   license is not stated on the dataset page read in this pass — flagged for manual check before
   import (`jev_classify` put it in `manual_review` on exactly this basis, 0.43/0.36 split).
4. **Every bulk-download prerequisite is already in place locally.** The `kaggle` CLI works with
   `~/.kaggle/access_token`, and per-file downloads from large datasets work without pulling the
   full archive. The spec's "Kaggle access requires credentials the user is willing to provide"
   note should be updated: access exists on this machine today.
5. **Two genuine cross-check axes exist.** nbadb (stats.nba.com bulk API export) vs sumitrodatta
   (Basketball-Reference scraped season stats with BAA/ABA tagging) — different provenance
   pipelines for the same historical facts — plus BBR transaction rows vs Wikipedia's structured
   mirrors for dated events.

## Verified numbers by source

### 1. Kaggle wyattowalsh/basketball ("NBA Database") — primary bulk source

- Origin: stats.nba.com bulk extraction pipeline (GitHub `wyattowalsh/nbadb`, MIT; pipeline docs
  at nbadb.w4w.dev — DuckDB/SQLite/CSV/Parquet exports, `nbadb init` full build, daily updater).
- Kaggle listing: 730,695,607 bytes total, updated 2026-06-29. **The spec's unresolved
  "731 MB vs 4.66 GB" discrepancy is explained: 731 MB is the Kaggle dataset size; the
  uncompressed SQLite (`nba.sqlite`) alone is 2,349,588,480 bytes — CSV + SQLite + DuckDB
  artifacts together plausibly reach ~4.66 GB uncompressed. Closed.**
- Local files inventory (via `kaggle datasets files`): `csv/game.csv` 20.3 MB;
  `csv/play_by_play.csv` 2.26 GB; `csv/common_player_info.csv` 1.0 MB; `csv/player.csv` 170 KB;
  `csv/inactive_players.csv` 7.4 MB; `csv/game_summary.csv` 5.6 MB; plus the 4 team CSVs
  (`team`, `team_details`, `team_history`, `team_info_common`) — 16 CSVs in all, matching
  the pinned zip.
- Downloaded and locally verified in this pass:
  - `game.csv`: 65,698 games, 1946-11-01 → 2023-06-12; season_type {Regular Season 60,192,
    Playoffs 3,842, Pre Season 1,536, All-Star 128}; pre-1950-08 games carry defunct-era
    abbreviations (AND BAL BOM BOS CHS CLR DEF DN FTW HUS INO JET MNL NYK PHW PIT PRO ROC SHE
    SYR TCB WAS WAT).
  - `common_player_info.csv`: 4,171 players, `from_year` min 1946 (136 players debut 1946).
  - `player.csv`: 4,831 ids. `inactive_players.csv`: game-id-level player absences (no dates).
  - Artifact-form note (verified 2026-10-09): the CSV exports in `data/csv/` are not
    row-identical to the SQLite — `player.csv` 4,831 ids vs SQLite `player` 4,815
    (CSV-only 21, SQLite-only 5); `common_player_info.csv` 4,171 rows vs SQLite 3,632
    (overlap 3,120, CSV-only 1,051, SQLite-only 512). The source manifest's S1 row counts
    describe the SQLite, the designated primary artifact; T3's reconciliation must state
    which artifact each count comes from.
- Schema findings: **no dated roster-tenure or transaction table exists in nbadb.** `star-reference.json`
  fetch failed in this pass (raw.githubusercontent fetch returned None mapping), but the local CSV
  headers cover the expected surface; a full header sweep of all 16 CSVs is follow-up work.
- Per-table single-file downloads work (`kaggle datasets download REPO -f csv/game.csv --unzip`)
  — a full 731 MB pull is unnecessary for scoping.

### 2. Kaggle sumitrodatta/nba-aba-baa-stats ("NBA Stats 1947-present") — independent cross-check

- Origin: author-self-described scrape of Basketball-Reference (Lahman-db style); player ids are
  BRef player slugs (`abramjo01`-style). Updated 2026-04-13; 11.1 MB.
- Local verification: `Player Career Info.csv` — 5,416 distinct players, `from` 1947–2026, debut
  dates with timestamps (`1946-11-01T00:00:00Z`), HOF flag; `Player Season Info.csv` — 33,339
  player-season-team rows, league tags {BAA: 582, NBA: 31,119, ABA: 1,638}, seasons 1947–2026
  (ABA rows enable the out-of-scope ABA-only filter); `Team Abbrev.csv` — 1,818 season-team
  abbreviation rows (franchise-alias input).
- Role: cross-check of player universe and player-season-team membership against nbadb; also the
  best bulk source of league tagging for ABA exclusion.

### 3. Kaggle deocheng/nba-data-1946-2026 — transaction/identity accelerant (license unverified)

- Self-description: 92-table ~5.9 GB relational export, snapshot 2026-08-18, assembled from
  Basketball-Reference (primary, incl. transactions), NBA.com/Stats API, ESPN, dunksandthrees
  (EPM), Spotrac. All tables downloadable as CSV (manifest verified: 92 rows, 5.9 GB total).
- Local verification:
  - `public.transactions.csv`: 28,667 rows; `transaction_date` range 1946-12-12 → 2026-06-24;
    schema {id, transaction_date, team_abbr, transaction_type, description, source, created_at,
    crawl_date}; types {Signed 15,398, Waived 6,390, Traded 3,397, Other 2,627, Released 341,
    Contract Converted 174, Claimed 111, Coach Hired 68, Coach Fired 60…}; `source` =
    basketball-reference.com.
  - Date precision of those 28,667 rows: 2,040 day-precision (7.1%), 24,347 season-resolution
    (84.9%), 2,280 no date (8.0%). So the file is mostly season-resolution — a starting point
    for tenure intervals, not a replacement for BBR's dated transaction pages (day-precision).
  - `public.player_id_bridge.csv`: {nba_player_id, player_name, br_player_id, match_strategy}
    with strategy labels (e.g. `lastname_unique`) — id-join aid.
  - `public.team_roster.csv`: 1,858 rows spanning only seasons 2024–2026 — recent-season only,
    not a historical roster source.
  - Also present: `player_name_unified.csv` (553 KB), `dim_franchise.csv`, `dim_players.csv`,
    `good_name_map.csv`, ESPN box scores (302 MB), `player_gamelog.csv` (267 MB).
- License: **not stated in the About section read in this pass** — `jev_verify` returned the
  license claim with `action: review` (p supports 0.75 / says_nothing 0.22). Check the license
  before any import; single-author dataset.

### 4. Basketball-Reference league transaction pages — day-precision dated events

- Live and covering the BAA era: `BAA_1947` (1946-47) fetched HTTP 200 with header "Transactions
  listed are from July 1, 1946 to June 30, 1947"; `BAA_1948` covers July 1, 1947 → June 30, 1948;
  `NBA_1950` covers July 1, 1949 → June 30, 1950. URL pattern is `/leagues/{BAA|NBA}_{year}_transactions.html`.
- Content: dated bullets — dispersal drafts after franchise foldings (e.g. July 9, 1947: Boston
  Celtics select John Janisch from the Detroit Falcons), trades (Dec 4, 1946: Cleveland Rebels
  trade Kleggie Hermsen to Toronto Huskies for George Nostrand), player-rights sales, waiver
  wire, signings, coach moves. Players and franchises are hyperlinked slugs
  (`players/h/hermskl01.html`, `teams/CHS/1948.html`) — resolvable to ids.
- Fuzzy dates appear within rows (`November ?, 1947`, `February ?, 1947`, `August ?, 1949`).
- Fetching notes: 200 via curl with browser UA in this pass (spec's 429 was transient); throttle
  and cache per the spec's standing rule regardless.

### 5. Wikipedia "List of {season} transactions" — structured cross-check

- e.g. "List of 1947–48 BAA season transactions" (89K chars): structured tables — Retirements,
  player-movement with `Player | New team | Former team` (including league-exit rows to NBL/ABL/
  PBLA with destination league named), Purchases with sold dates, navigation across seasons
  1947-48 → 2024-25. Cites BBR transaction pages (e.g. `brtransactions47`) — a structured
  restatement, useful as a second read of the same movement and for catching BBR omissions.

### 6. Basketball-Reference team-season roster pages — season-membership backbone

- `/teams/PHW/1947.html` fetched HTTP 200 with a Roster Table section and ~72 player links for a
  single franchise-season page. Provides per-season team membership (and season stats), but no
  join/leave dates on the roster table itself; combine with transaction pages for intervals.

### 7. NBA Hoops Online — supplemental, esp. defunct franchises

- `/teams/index.html` ("NBA Team Roots", HTTP 200): franchise season-by-season lineage with
  league tags incl. pre-BAA NBL/AAU and defunct BAA franchises (Chicago Stags 1946-49 BAA /
  1949-50 NBA; Toronto Huskies 1946-47; Washington Capitols 1946-51; Anderson Packers incl.
  NPBL 1950-51). Useful to complete franchise-alias crosswalk and league membership windows.
- `/History/Rosters/index.html` exists (HTTP 200); a guessed deep roster URL 404'd — inventory
  of actual roster pages is follow-up. No API; volunteer-maintained (treat as secondary).

### 8. RealGM NBA transactions — unconfirmed archive depth

- Season selector lists seasons from 1946-47 through 2026-27 (via reader-mode fetch), which
  claims deep historical archive. But direct curl is blocked (HTTP 403) and reader-mode fetches
  with season parameters (`?season=1946`, `/transactions/1946-1947`, `?season=1956`) all served
  the current-season page instead of the selected season, so the historical depth is **claimed
  by the selector but not confirmed by fetch**. `jev_classify`: `manual_review` 0.96. Reattempt
  with a real browser (chrome-devtools MCP) if needed as a supplement.

### 9. balldontlie NBA API — paid, no roster/transaction endpoints

- Docs: "data from 1946-current"; API key required (401 unauthenticated verified). Free tier:
  Teams/Players/Games only; game player stats, box scores, lineups behind paid tiers (ALL-STAR /
  GOAT; ALL-ACCESS $299.99/mo). No transaction or historical roster endpoints documented.
- Verdict: not useful for this build (games/players bulk coverage duplicates nbadb).

### 10. prosportstransactions.com — blocked aggregator

- Search UI over NBA trades/signings/fines; `rsforbes/pro_sports_transactions` (22★, MIT) wraps
  it but its README warns the site is behind a Cloudflare challenge requiring the nodriver
  browser handler or an Unflare sidecar; direct curl 403. NBA archive depth unverified here.
- Even if accessible, its data are BBR/press-derived; prefer BBR directly. Parked.

### 11. stats.nba.com / cdn.nba.com direct — unusable from this environment

- Probes: `commonallplayers` (all-time), `commonteamroster` (1946-47), `leaguegamelog`
  (1947-48) all read-timeout (25 s); `cdn.nba.com/.../allPlayers.json` → HTTP 403 with browser
  headers. Consistent with the spec's earlier 403 note. nbadb is the cache of this API; use it.

### 12. jaebradley/basketball_reference_web_scraper — crawl tooling option

- 560★ Python library scraping BRef incl. transactions; community-standard client. Still subject
  to BRef throttling. Candidate tool for the throttled transaction-page crawl (or a bespoke
  Rust crawler per the "Rust-only application code" constraint — data prep is not app code, so
  Python tooling is acceptable for the crawl itself; decide in T4).

## Role summary (jev_classify result, thresholds 0.80/0.40)

| Source | Role | Decision | Top prob / margin |
| --- | --- | --- | --- |
| wyattowalsh/basketball (nbadb) | **bulk_primary** | auto | 0.88 / 0.81 |
| sumitrodatta/nba-aba-baa-stats | **independent_crosscheck** | review (0.78, just under 0.80) | 0.78 / 0.67 |
| deocheng/nba-data-1946-2026 | **manual_review** (license unsettled; dated_event_source 0.36 close second) | review | 0.43 / 0.07 |
| BBR league transaction pages | **dated_event_source** | auto | 0.95 / 0.92 |
| Wikipedia transaction lists | dated_event_source | review | 0.77 / 0.69 |
| BBR team-season roster pages | independent_crosscheck | review | 0.60 / 0.37 |
| NBA Hoops Online | supplemental_context | review | 0.72 / 0.63 |
| RealGM transactions | manual_review (depth unconfirmed) | auto (for manual_review) | 0.96 / 0.94 |
| balldontlie NBA API | not_useful (for this build) | review | 0.67 / 0.51 |
| prosportstransactions | not_useful (blocked; redundant) | review | 0.53 / 0.30 |
| stats.nba.com direct | not_useful (403/timeouts) | auto | 0.99 / 0.98 |
| jaebradley scraper | dated_event_source (tooling) | auto | 0.83 / 0.75 |

Where the classifier and I diverge, the table above records both: `review` decisions are not
auto-accepted; the resolved role assignment is in the per-source sections.

## Recommended build shape (updates the spec's source strategy)

1. **Primary bulk:** nbadb CSVs (download per-table; `game.csv`, `player.csv`,
   `common_player_info.csv` first; `game_summary.csv` for season granularity). No SQLite
   download needed initially (2.35 GB uncompressed).
2. **Cross-check bulk:** sumitrodatta (player universe, player-season-team, BAA/ABA tags,
   team abbreviations per season).
3. **Dated events for tenure intervals:** BBR league transaction pages (day precision, from
   1946-47), cross-checked against Wikipedia mirrors; pre-aggregated bootstrap from
   deocheng's `transactions.csv` **after license clearance** (its rows are mostly
   season-resolution; BBR pages carry the day-precision layer).
4. **Identity:** deocheng `player_id_bridge.csv` + `player_name_unified.csv` (license-gated);
   independent BRef-slug ↔ stats.nba.com id mapping to be built in T3 otherwise.
5. **Defunct/franchise lineage:** NBA Hoops Online "Team Roots" + `common_team_roster`-shaped
   checks across eras.

## Concrete next actions

1. Done 2026-10-09: the spec (docs/specs/nba-teammate-degrees.md, mirrored in issue #1) now
   records BBR transaction coverage from 1946-47, re-qualifies the BAA gap as a bounded parsing
   risk, and records Kaggle access as provisioned (`~/.kaggle/access_token`, mode 600).
2. **For #5 (T3):** run the two-bulk cross-check (nbadb vs sumitrodatta) — start with player
   universe diff and season-team diff for 1946-50 and 2010-26; add deocheng id bridge pending
   license check.
3. **For #6 (T4):** prototype tenure-interval reconstruction on BAA 1946-48 using BBR transaction
   pages + Wikipedia mirror + deocheng (license-gated) as bootstrap; include fuzzy-date handling
   ("November ?, 1947") and the "Transactions listed are from…" page header as the interval bbox.
4. **License sweep at import time:** pin nbadb/sumitrodatta licenses from their Kaggle pages and
   the nbadb GitHub MIT; resolve deocheng (contact author or skip). Retain manifest rows
   (source/version/date/license) per the spec's provenance requirement.

## Request accounting for this pass

13 per-file Kaggle CSV downloads (6 from S1's dataset, 3 from S2, 4 from S4's deocheng
dataset), Kaggle metadata/inventory API reads, ~12 direct HTTP probes (each candidate URL
fetched once), and web searches. No per-season or per-page crawl was started; the BBR
transaction crawl remains planned, cached and throttled.

## Jev summary

- `jev_screen` (fetched descriptions, `purpose` given): pass — injection 0.06, substance 0.94,
  relevance 0.90. No review/block content.
- `jev_classify` (12 sources → 7-role catalog): 5 auto, 7 review, 0 invalid. Full table above.
  3,427 in / 869 out tokens.
- `jev_verify` (12 claims vs 11 evidence items): 12 verified (11 auto, 1 review — the license
  claim, p supports 0.75, says_nothing 0.22 — correctly left unresolved pending the license
  sweep). 9,187 in / 2,864 out tokens.
- `jev_compare` (spec claim vs fresh fetches, aspects: page existence / dated rows / fuzzy
  dates): overall `contradicts` p=1.0 (auto); "dated rows" aspect review (0.71/0.27) — resolved
  by the row excerpts verbatim in this doc; "fuzzy dates" different_facts (spec silent).
  1,390 in / 177 out tokens.
- Unresolved (escalated deliberately): deocheng license; RealGM archive depth (needs a real
  browser attempt); a handful of classify `review` rows resolved by judgment as noted.