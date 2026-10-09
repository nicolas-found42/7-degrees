# T3 — Cross-Source Reconciliation Report (S1/S2/S3)

Ticket: #5 (*T3: Cross-validate sources and fix the player universe*) · parent #1 ·
spec: `docs/specs/nba-teammate-degrees.md` · manifest: `docs/data/source-manifest.md`.

Sources (pinned snapshot, see the manifest for versions/checksums):

- **S1** — Kaggle `wyattowalsh/basketball` v238, `data/nba.sqlite` + CSV exports
  (stats.nba.com lineage; NBA-API person ids).
- **S2** — Kaggle `sumitrodatta/nba-aba-baa-stats` v56, `data/sumitrodatta/*.csv`
  (independent Basketball-Reference scrape; BBR person slugs; BAA/ABA tagging).
- **S3** — Kaggle `romainmorleghem/nba-players-info-and-headlinestats-up-to-2025` v1,
  `data/romainmorleghem/CommonPlayerInfo_ALL.csv` (NBA API `commonplayerinfo` ids).

Report generated 2026-10-09 20:07 UTC by `scripts/t3_reconcile.py` + `scripts/t3_report.py`
(reproduce: `python3 scripts/t3_reconcile.py [data_dir] && python3 scripts/t3_report.py`).
All artifacts referenced below are retained under `docs/reports/t3/`.

> **Snapshot-integrity note.** The main checkout's `data/` directory disappeared from
> disk mid-run (cause outside this ticket's scope). The pinned artifacts were
> re-downloaded with the Kaggle CLI (one bulk request per dataset; request count
> unaffected in kind) and verified row-by-row against this manifest's pinned
> counts before reuse: S1 `player` 4,815 / `game` 65,698; S2 `Player Season Info`
> 33,339 rows (NBA 31,119 / ABA 1,638 / BAA 582) with 5,105 distinct NBA/BAA
> players; S2 `Team Abbrev.csv` 1,818 rows / 96 names / 104 abbreviations; S3 5,135 ids.

## Verdict at a glance

| question | result |
|---|---|
| canonical NBA/BAA player universe (S2 + official S1 appearances) | **5,106 players** (5,105 S2 NBA/BAA players + 1 named S1 official-game players absent from S2) |
| S1-evidenced person IDs bridged into the universe (player table + recovered PBP-only IDs) | 4,794 of 4,824 |
| Unresolved S1-only people (no canonical identity) | 30 (S1 pbp-only ids: 9; detailed in the register) |
| ABA-only classifications (excluded from the universe) | 311 S2 players are ABA-only; 0 S1 people match an ABA-only S2 player |
| S2↔S3 identity bridge | 5076 of 5106 universe players carry an S3 NBA-API id (99.4%) |
| franchise crosswalk | 1818 of 1818 S2 season-team rows resolve to one canonical franchise id; 0 ambiguous |
| player-season-team membership diff (both directions, seasons ≤2023 usable for S1) | S2-only: 12,734 (shape breakdown below) · S1-only: 11 |

## 1. Sources and headline counts

| source | artifact | players/people | notes |
|---|---|---|---|
| S1 | `nba.sqlite` `player` | 4,815 | NBA API person ids; + 9 play-by-play-only ids absent from the table (recovered from pbp names) |
| S1 | `common_player_info` | 3,632 | biographical subset with birthdates |
| S2 | `Player Season Info.csv` | 5,105 NBA/BAA + 311 ABA-only | 33,339 rows: NBA 31,119 / BAA 582 / ABA 1,638 |
| S2 | `Player Career Info.csv` | 5,416 | birthdates for the bridge; **no `lg` column** (the ticket's suggested ABA-classification source does not exist; `lg` comes from `Player Season Info.csv`) |
| S2 | `Team Abbrev.csv` | 96 names / 104 abbreviations | 1,818 season-team rows, 1947–2026 |
| S3 | `CommonPlayerInfo_ALL.csv` | 5,135 | NBA API `commonplayerinfo` snapshot (2025); 4,814 of S1 ids present |

- S2 dual-league players (NBA/BAA **and** ABA rows): **207** — these stay in scope (NBA/BAA appearance).
- S1↔S3 share the NBA-API id namespace (4,814 of 4,815 S1 ids appear in S3);
  S1↔S2 and S2↔S3 require an identity bridge (different namespaces).

## 2. The canonical player universe

**Definition (spec):** a person with at least one official NBA/BAA regular-season or
postseason game appearance. ABA-only players are excluded.

**Resolution.** S2 BBR person slugs establish the season-statistics baseline across
NBA/BAA/ABA eras. Named S1 players with positive gameplay events in official
regular-season or postseason games remain in scope even when S2 omits them.
Their canonical key is `nba:<person_id>`; it is not a fabricated BBR slug.
S1 and S3 identities bridge by name + birth date; see §3.

| item | count |
|---|---|
| universe size (S2 baseline plus official S1 appearances) | **5,106** |
| S1 people inside the universe (bridged) | 4,794 |
| S1 people outside it | 30 |
| S2 universe players with an S3 id | 5,076 |
| S2 universe players with no S1 row | 313 (of which 9 debut ≤2023) |
| S2 universe players with no S3 id | 30 |
| ABA-only players excluded (S2 classification) | 311 |

`docs/reports/t3/player-universe.csv` — one row per universe player:
`bbr_player_id, display_name, birth_date, leagues, first_season, last_season,
nba_baa_season_rows, aba_only, s1_player_id, s1_match_class, s1_display_name,
s3_person_id, s3_bridge_method, universe_source`.

## 3. Player-identity reconciliation (S1 ↔ S2 ↔ S3)

The three sources use two disjoint id namespaces (NBA-API numeric ids in S1/S3;
Basketball-Reference slugs in S2), so the bridge is **name + birth date**:

| S1 match class | count | rule / meaning |
|---|---|---|
| `name+dob` | 4445 | a (name-form, birth-date) pair uniquely identifies one S2 player — trusted |
| `lastname+dob` | 194 | unique (surname, birth-date) — trusted (handles "Steven Smith"/"Steve Smith"-style spellings) |
| `initial-surname+dob-window` | 8 | same first-initial + surname with birth dates within ±2 years and a shared birth year or exact month/day — unique candidate, accepted and flagged for review (handles "Norman Richardson"↔"Norm Richardson" year shifts and day/month swaps) |
| `surname-fuzzy+dob-window` | 2 | surname within one character edit + first-initial + DOB window, unique — accepted, flagged (handles Guðmundsson/Gudmundsson-style transliteration drift) |
| `unique-surname` | 0 | surname carried by exactly one S2 player, matched without a usable DOB — accepted, flagged |
| `lastname-s2dob-NA` | 0 | full-name match where S2's birth date is NA — accepted, flagged |
| `surname+career-span` | 2 | surname carried by several S2 players but exactly one S2 career span fits inside the S1 career span — accepted, flagged (pre-pbp nickname pairs like "Johnny Kerr"↔"Red Kerr") |
| `initial+surname` | 0 | one-letter first name + surname, unique — accepted, flagged |
| `DOB-conflict` | 142 | unique name but the sources disagree on the birth date — identity accepted, flagged for review |
| `ambiguous-name` | 0 | same display name (no DOB disambiguation) — **not** bridged; listed below |
| `no-match` → subclasses | 30 | no plausible S2 identity; subclassified into `S1-only-*` below |
| `matches-ABA-only-player` | 0 | S1 person matches an S2 ABA-only player (correctly outside the universe) |

### 3a. Every S1 ↔ S2 identity disagreement (register)

`docs/reports/t3/unresolved-player-cases.csv` lists **all 5493 rows** (source, class, both ids, both names). Summaries:

| DOB-conflict magnitude (S1 vs S2 birth dates) | count |
|---|---|
| same year, different day/month | 56 |
| 1 year apart | 27 |
| 2–5 years apart | 13 |
| >5 years apart | 13 |
| one or both birth years unavailable/invalid | 33 |
| total DOB-conflict rows | 142 |

Top-10 `DOB-conflict` examples (S1 id ↔ S2 id, both names):

| S1 name | S1 id | S2 id | S2 name | S1 DOB | S2 DOB |
|---|---|---|---|---|---|
| Aaron Swinson | 78286 | swinsaa01 | Aaron Swinson | 1970-12-21 | 1971-01-09 |
| Adrian Dantley | 76504 | dantlad01 | Adrian Dantley | 1955-02-26 | 1955-02-28 |
| Al Lujack | 77423 | lujacal01 | Al Lujack |  | 1920-10-05 |
| Alvin Heggs | 76987 | heggsal01 | Alvin Heggs | 1967-12-08 | 1967-12-12 |
| Andreas Glyniadakis | 2601 | glynian01 | Andreas Glyniadakis | 1981-08-26 | 1981-08-21 |
| Andy Johnson | 77131 | johnsan01 | Andy Johnson |  | 1932-11-08 |
| Andy Panko | 1950 | pankoan01 | Andy Panko | 1977-11-29 | 1977-11-27 |
| Arnie Johnson | 77132 | johnsar01 | Arnie Johnson |  | 1920-05-16 |
| Art Spector | 78226 | spectar01 | Art Spector | 1920-10-17 | 1918-10-17 |
| Artis Gilmore | 600014 | gilmoar01 | Artis Gilmore | 1948-09-21 | 1949-09-21 |

### 3b. S1-only people (no S2 identity) — subclassified

| S1-only subclass | count | meaning |
|---|---|---|
| `S1-only-has-play-by-play` | 5 | S1 pbp evidences the person; S2 lacks the player entirely |
| `S1-only-inactive-list-only` | 16 | present in S1 `inactive_players` (DNP roster slot) but never evidenced with stats by either source |
| `S1-only-debut-2024-plus` | 0 | S1-only person debuting season 2024+ — S2 v56 (2026-04-13) predates their debut |
| `S1-only-no-pbp-evidence` | 9 | S1 person with career years but no pbp-era evidence in either source (legacy-API ghost rows) |

Top-10 by subclass (name — S1 id):

- **S1-only-has-play-by-play** (5):  (471);  (775);  (1277);  (1787);  (2794)
- **S1-only-inactive-list-only** (16): Herbert Hill (201195); Robert Vaden (201987); Curtis Jerrells (201998); Diamon Simpson (202067); Tony Gaffney (202070); Brian Butch (202221); Kenny Hasbrouck (202238); Terrico White (202358); Da'Sean Butler (202364); Magnum Rolle (202375)
- **S1-only-debut-2024-plus** (0): 
- **S1-only-no-pbp-evidence** (9): Marcus Mann (986); Francis Crossin (76479); Wayne Englestad (76671); Gene Gillette (76813); Adolph Hoefer (77035); Herm Klotz (77284); Buckshot O'Brien (77743); Rabbit Walthour (78448); Marqus Blakely (202392)

### 3c. Official appearances absent from S2

| S1 id | canonical player key | name | official-game seasons |
|---|---|---|---|
| 1630492 | nba:1630492 | Luca Vildoza | 2022 |
These players are included; missing S2 rows remain explicit identity and membership disagreements.
Nameless S1 references remain in the review register rather than creating speculative duplicate people.

### 3d. Unresolved name ambiguities (if any)


### 3e. S1 matches to ABA-only players

Count: **0**. Examples: —

## 4. S2 universe vs S3

| S3 relationship | count | meaning |
|---|---|---|
| S3 people also in S1 | 4814 | S3 ⊆ S1 almost exactly |
| S3-only people (not in S1) | 321 | newer NBA-API ids than S1's v238 export |
| …of those, with S2 NBA/BAA rows | 287 | real NBA players missing from S1's player list (S1 gap) |
| …of those, ABA-only per S2 | 0 | ABA-only names in the NBA-API metadata |
| …of those, with no S2 row at all | 34 | no NBA/BAA/ABA stats row in S2 |
| S1 person absent from S3 | 1 | Makhtar N'Diaye (id 1626122; present in S1 `common_player_info`) |

S2→S3 bridge methods: {"via-S1-bridge": 4793, "name-+dob": 278, "name-DOB-conflict": 5}
All missing S3 identities and direct-name DOB conflicts are also listed in `unresolved-player-cases.csv`.

## 5. Player-season-team membership reconciliation

Compared sets (season-scoped, canonical franchise ids on both sides):

- **S2 side**: `Player Season Info.csv` NBA/BAA rows, excluding `2TM`/`3TM`/`4TM`/`5TM` summary rows — one row per (season, player, team).
- **S1 side**: distinct (season, person, team) from `play_by_play` on Regular Season + Playoffs games, seasons 1997–2023 (S1 has no per-player evidence before 1996-97; `game` ends 2022-23).

+ 9 S1 pbp references person ids that `player` omits (recovered from pbp names; 4 of those included in the universe).

| membership diff (S1-pbp era, seasons ≤2023) | rows | interpretation |
|---|---|---|
| S2-only (`S1-no-per-player-evidence-pre-1997`) | 12354 | structural: S1 has no per-player team evidence before 1996-97 — not a data disagreement |
| S2-only (`S1-pbp-no-event-for-player`) | 330 | S1 pbp has no event for that player in that season — candidates: brief stints, pbp source gaps, identity edge cases |
| S2-only (`S1-pbp-shows-other-teams-same-season`) | 50 | both sources have the (season, player) but the team sets disagree — real disagreements, individually reviewed |
| S1-only (S1 pbp row without an S2 row) | 11 | S1 official-game memberships absent from S2 — all 11 individually listed below |
| S2-only, season ≥2024 | 1972 | structural: S1 `game` ends 2022-23 — no S1 comparison possible |

**The 11 S1-only membership rows** (all verified against raw S2: the player's S2 rows)
skip that season or list only other teams):

| season | player (canonical name) | canonical franchise | S1 id/name |
|---|---|---|---|
| 2009 | Sam Cassell | CELTICS | 208 / Sam Cassell |
| 2013 | Tracy McGrady | SPURS | 1503 / Tracy McGrady |
| 2013 | Scott Machado | WARRIORS | 203159 / Scott Machado |
| 2016 | John Holland | CELTICS | 204066 / John Holland |
| 2016 | Dorell Wright | HEAT | 2748 / Dorell Wright |
| 2018 | Ty Lawson | WIZARDS | 201951 / Ty Lawson |
| 2020 | Jaylen Adams | TRAIL BLAZERS | 1629121 / Jaylen Adams |
| 2022 | Luca Vildoza | BUCKS | 1630492 / Luca Vildoza |
| 2023 | DaQuan Jeffries | KNICKS | 1629610 / DaQuan Jeffries |
| 2023 | Shaquille Harrison | LAKERS | 1627885 / Shaquille Harrison |
| 2023 | Tristan Thompson | LAKERS | 202684 / Tristan Thompson |

Reviewed examples (raw S2 rows checked): Steve Smith has HOU in 1996-97, not HAWKS;
McGrady's Spurs row is missing altogether from S2; Ty Lawson's 2017-18 is SAC-only in
S2 (his WSH 10-day contract is missing); Tristan Thompson's 2022-23 LAL stint (signed
Apr 2023) is missing from S2. These are genuine S2 (Basketball-Reference scrape)
omissions and T4 must treat S2 as *incomplete* for late-season moves.

`docs/reports/t3/membership-mismatches.csv` — all 14717 diff rows with the shape classification column (`diff_shape`).

## 6. Franchise identities and the alias crosswalk

**Method.** `scripts/t3_franchise_seed.py` holds a curated per-franchise lineage table
((league, season span) → name + abbreviation + S1 abbreviation) — 54 canonical franchise identities
ids covering every S2 era, each lineage assembled from historical franchise records
(relocations, renames, league transitions) and verified against the pinned sources
in the reconciliation run. Shared display names across genuinely distinct franchises
are split by season span:

| display name (S2) | franchises | season spans | resolution |
|---|---|---|---|
| "Denver Nuggets" | 2 | DNN 1950 (defunct) · ABA Rockets/Nuggets → NBA Nuggets (1976→) | `NUGGETS-DEFUNCT` vs `NUGGETS` |
| "Baltimore Bullets" | 2 | 1948–1955 (folded) · 1963–1973 (Chicago/Zephyrs line → Washington) | `BULLETS-DEFUNCT` vs `WIZARDS` |
| "Washington Capitols" | 2 | BAA/NBA 1946–1951 (folded) · ABA 1969-70 "Caps" (Oaks/Squires line) | `CAPITOLS` vs `ABA-OAKS-CAPS-SQUIRES` |
| "Charlotte Hornets" | 2 | 1988–2002 → New Orleans → Pelicans · Bobcats 2004 → renamed Hornets 2014 | `HORNETS-PELICANS` vs `BOBCATS-HORNETS2` |

Result: **1818 of 1818** `Team Abbrev.csv` rows resolve to exactly one canonical franchise id (0 ambiguous, 0 unresolved → all 0 New York Knicks rows until `NYK` was added to the seed; now 0). The 96 S2 team names and 104 abbreviations map onto 54 canonical franchises (45 NBA/BAA-scope + ABA-only lineages kept separate on purpose).

`docs/reports/t3/franchise-crosswalk.csv` — every S2 season-team row → canonical id.

**Spot checks demanded by the ticket** (each verified in the crosswalk):

| check | resolution |
|---|---|
| Seattle SuperSonics → OKC | one id `THUNDER`: SEA 1968–2008 + OKC 2009→ |
| Vancouver Grizzlies → Memphis | one id `GRIZZLIES`: VAN 1996–2001 + MEM 2002→ |
| Charlotte 1988/2002/2004/2013 history | CHH+NOH+NOK+NOP → `HORNETS-PELICANS`; CHA(2004)+CHO → `BOBCATS-HORNETS2` |
| Tri-Cities → Milwaukee → St. Louis → Atlanta | one id `HAWKS` (TRI/MLH/STL/ATL) |
| Defunct BAA/NBA franchises in S2's 96 names | AND/SHE/WAT/DNN 1950 one-season teams, CHS/STB/PRO/WSC/BLB/BAL, TRH/CLR/PIT/DTF/INJ — all present, each its own canonical id |
| S1 abbreviation collisions | `WAS` = Wizards *and* Capitols (1947–51); `CHA` = Bobcats *and* Hornets-2; `BLB`/`BAL` = the two Bullets franchises; `DEN`/`DN` = the two Nuggets — all resolved by (season, league) scoping |

**S2 franchise coverage notes** (checked against the raw tables):

| S2 `Team Abbrev.csv` / season fact | detail |
|---|---|
| BAA 1946-47 Baltimore coverage | No roster gap: season 1947 has no BLB player rows; Baltimore joined the BAA in 1947-48, where S2 has 18 BLB player rows, and 21 in 1948-49. `Team Abbrev.csv` includes both seasons. BLB-1947 in `Draft Pick History.csv` is a draft-team entry, not a roster-season row. |
| BAA/NBA boundary | S2 labels the 1949-50 Tri-Cities season (and all other 1949-50 teams) NBA; the BAA's last season was 1948-49 — verified and modeled |
| `CHO`/`CHA` for the renamed Hornets | S2 uses CHO for 2015+ while pre-2004 Charlotte used CHH and 2004-14 CHA — distinct franchises, not aliases of one line |

S1 game-table team-name variants (`Ft. Wayne Zollner Pistons`, `Sheboygan Redskins`,
`LA Clippers`) are normalized via `S1_TEAM_NAME_FIXUPS` in the seed module.

## 7. Era-stratified samples

Stratified samples across eras are listed for manual review; the 50-records-per-era
manual sweep is **T4/T9's** stratified-validation duty (see issue #6/#5 scope note) —
this report provides the sampling frames.

| sample class | era | n | examples |
|---|---|---|---|
| DOB-conflict | — | 35 | ('Chet Aubuchon', '76071', 'Chet Aubuchon'); ('Don Carlson', '76344', 'Don Carlson'); ('Joe Colone', '76422', 'Joe Colone') … |
| DOB-conflict | 1950-66 | 31 | ('Don Bielke', '76164', 'Don Bielke'); ('Bob Brannum', '76238', 'Bob Brannum'); ('Cal Christensen', '76394', 'Cal Christensen') … |
| DOB-conflict | 1967-80 | 15 | ('Ron Brewer', '76249', 'Ron Brewer'); ('George Carter', '76357', 'George Carter'); ('Ben Clyde', '76409', 'Ben Clyde') … |
| DOB-conflict | 1981-99 | 27 | ('Greg Minor', '65', 'Greg Minor'); ('Dwayne Morton', '132', 'Dwayne Morton'); ('Felton Spencer', '280', 'Felton Spencer') … |
| DOB-conflict | 2000-2025/26 | 34 | ('Maceo Baston', '1766', 'Maceo Baston'); ('Tim James', '1906', 'Tim James'); ('Andy Panko', '1950', 'Andy Panko') … |
| S1-player-no-S2-match | — | 9 | ('', '471'); ('', '775'); ('', '1277') … |
| S1-player-no-S2-match | 1950-66 | 2 | ("Buckshot O'Brien", '77743'); ('Rabbit Walthour', '78448') |
| S1-player-no-S2-match | 1981-99 | 2 | ('Marcus Mann', '986'); ('Wayne Englestad', '76671') |
| S1-player-no-S2-match | 2000-2025/26 | 17 | ('Herbert Hill', '201195'); ('Robert Vaden', '201987'); ('Curtis Jerrells', '201998') … |
| S2-player-no-S1-match-2024-plus | 2000-2025/26 | 304 | ('Trey Alexander', 'alexatr01'); ('Timmy Allen', 'allenti01'); ('Alex Antetokounmpo', 'antetal01') … |
| S2-player-no-S1-match-pre-2023 | — | 4 | ('Chink Crossin', 'crossch01'); ('Gene Gallette', 'gillege01'); ('Charlie Hoefer', 'hoefech01') … |
| S2-player-no-S1-match-pre-2023 | 1950-66 | 3 | ("Ralph O'Brien", 'obriera01'); ('Isaac Walthour', 'walthis01'); ('Bobby Watson', 'watsobo01') |
| S2-player-no-S1-match-pre-2023 | 1981-99 | 1 | ('Wayne Engelstad', 'englewa01') |
| S2-player-no-S1-match-pre-2023 | 2000-2025/26 | 1 | ('RaiQuan Gray', 'grayra01') |
| S2-universe-no-S3-id | — | 4 | ('Chink Crossin', 'crossch01'); ('Gene Gallette', 'gillege01'); ('Charlie Hoefer', 'hoefech01') … |
| S2-universe-no-S3-id | 1950-66 | 3 | ("Ralph O'Brien", 'obriera01'); ('Isaac Walthour', 'walthis01'); ('Bobby Watson', 'watsobo01') |
| S2-universe-no-S3-id | 1981-99 | 1 | ('Wayne Engelstad', 'englewa01') |
| S2-universe-no-S3-id | 2000-2025/26 | 22 | ('Alex Antetokounmpo', 'antetal01'); ('Adama-Alpha Bal', 'balad01'); ('Darius Brown II', 'brownda04') … |
| S3-only-but-NBA-per-S2 | — | 1 | ('Keshon Gilbert', '1642933') |
| S3-only-but-NBA-per-S2 | 2000-2025/26 | 286 | ('Sasha Vezenkov', '1628426'); ('Chance Comanche', '1628435'); ('Jack McVeigh', '1629098') … |
| S3-only-no-S2-row | — | 264 | ('CJ Miles', '101139'); ('Ike Austin', '1134'); ('RJ Hunter', '1626154') … |
| membership-S1-only-vs-S2 | 2000-2025/26 | 11 | (2009, 'cassesa01', 'CELTICS'); (2013, 'machasc01', 'WARRIORS'); (2013, 'mcgratr01', 'SPURS') … |
| membership-S2-only-vs-S1 | 1950-66 | 2187 | (1950, 'armstcu01', 'PISTONS'); (1950, 'barkecl01', 'OLYMPIANS'); (1950, 'barnhle01', 'STAGS') … |
| membership-S2-only-vs-S1 | 1967-80 | 3419 | (1967, 'abdulma01', 'LAKERS'); (1967, 'akinhe01', 'KNICKS'); (1967, 'attleal01', 'WARRIORS') … |
| membership-S2-only-vs-S1 | 1981-99 | 6244 | (1981, 'abdulka01', 'LAKERS'); (1981, 'abernto01', 'PACERS'); (1981, 'abernto01', 'WARRIORS') … |
| membership-S2-only-vs-S1 | 2000-2025/26 | 372 | (2000, 'jacksra02', 'MAVERICKS'); (2001, 'millean01', '76ERS'); (2001, 'pankoan01', 'HAWKS') … |
| membership-S2-only-vs-S1 | BAA 1946-49 | 512 | (1947, 'abramjo01', 'IRONMEN'); (1947, 'aubucch01', 'FALCONS'); (1947, 'bakerno01', 'STAGS') … |

## 8. ABA exclusion

- ABA-only players (S2 `lg` == `ABA` for every season row): **311** — all excluded from the universe.
- Dual-league players (ABA + NBA/BAA): **207** — included (official NBA/BAA appearance).
- S1 people matching ABA-only S2 players: **0** — correctly outside the universe; listed in the register.
- ABA teams stay in the crosswalk as separate canonical ids so a wrong NBA merge can never happen silently.

## 9. How to reproduce

```bash
# from the repo root; requires the pinned data snapshot under data/ (see docs/data/source-manifest.md)
python3 scripts/t3_reconcile.py [data_dir]   # writes docs/reports/t3/*.csv + data/t3/.state.pkl
python3 scripts/t3_report.py                 # renders docs/reports/t3-reconciliation.md
```

Runtime: ~2–4 minutes (the play-by-play scan dominates). Python 3.9 stdlib only
(`sqlite3`, `csv`); no pandas required. `data/` is gitignored and never modified
(SQLite opened read-only).

## 10. Open items for review (not silently resolved)

1. 142 `DOB-conflict` identities accepted-but-flagged (§3a) — T4/T13 should confirm
   the DOB discrepancies are source errors, not distinct people.
2. The 11 S1-only membership rows (§5) — S2 omissions for late-season moves;
   T4's transaction reconstruction must not rely on S2 completeness for those.
3. 50 `S1-pbp-shows-other-teams-same-season` rows and 330 `S1-pbp-no-event-for-player` rows — retained in `membership-mismatches.csv` for T4 review.
4. 9 S1 play-by-play people absent from `player` (5 unbridged) — S1's own export gap;
   listed in `unresolved-player-cases.csv`.
5. 9 universe players (debut ≤2023) with no S1 identity — S1's `player` export is incomplete for recent seasons; S3 covers 1 of those same players.
6. The ticket's suggested `lg`-column location ("Player Career Info.csv") does not
   exist; ABA classification uses `Player Season Info.csv` `lg`. Recorded per the
   "check exact column names first" instruction.

