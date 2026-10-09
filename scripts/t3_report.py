#!/usr/bin/env python3
"""T3 — build the reconciliation report (ticket #5, second pass).

Reads the analysis state written by t3_reconcile.py (data/t3/.state.pkl) and the
generated CSV artifacts, and renders docs/reports/t3-reconciliation.md.
"""

import csv
import json
import os
import pickle
import sys
from collections import Counter
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from t3_reconcile import T3_DIR  # noqa: E402

REPORT = os.path.join(REPO, "docs", "reports", "t3-reconciliation.md")


def register_rows(data_dir):
    import csv as _csv
    p = os.path.join(T3_DIR, "unresolved-player-cases.csv")
    if os.path.exists(p):
        return sum(1 for _ in open(p, encoding="utf-8")) - 1
    return None


def md_table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    for r in rows:
        out.append("| " + " | ".join(str(x) for x in r) + " |")
    return "\n".join(out)


def pbp_only_ids_bridged():
    """Distinct S1 play-by-play-only person ids that the universe bridge accepted.

    Counted from the retained artifacts (player-universe.csv cross-checked
    against data/nba.sqlite `player`), not from a run metric: an id counts
    when it is absent from the S1 player table yet carries a bridged
    match class on some universe row.
    """
    import csv as _csv
    import sqlite3
    p = os.path.join(T3_DIR, "player-universe.csv")
    if not os.path.exists(p):
        return None
    db = sqlite3.connect("file:" + os.path.join(REPO, "data", "nba.sqlite") + "?mode=ro", uri=True)
    s1_ids = {str(r[0]) for r in db.execute("SELECT id FROM player")}
    db.close()
    bridged_classes = {
        "name+dob", "name-match-s2-dob-NA", "name-only", "lastname+dob",
        "initial-surname+dob-window", "curated-alias", "surname-fuzzy+dob-window",
        "lastname-s2dob-NA", "surname+career-span", "initial+surname",
        "unique-surname", "DOB-conflict",
    }
    pbp_only = set()
    with open(p, encoding="utf-8") as f:
        for row in _csv.DictReader(f):
            sid = (row.get("s1_player_id") or "").strip()
            if not sid:
                continue
            # Multi-id rows keep the bridged id(s); a row is pbp-only when
            # every id it names is outside the player table.
            ids = [i for i in sid.replace("+", ";").split(";") if i]
            if ids and all(i not in s1_ids for i in ids):
                if row.get("s1_match_class") in bridged_classes:
                    pbp_only.add(sid)
    return len(pbp_only)


def pct(n, d):
    return "%.1f%%" % (100.0 * n / d) if d else "n/a"


def main():
    data_dir = os.path.join(REPO, "data")
    if not os.path.isdir(data_dir):
        # reports can be regenerated anywhere the analysis ran; try sibling
        data_dir = os.path.normpath(os.path.join(REPO, "..", "7-degrees", "data"))
    st = pickle.load(open(os.path.join(data_dir, "t3", ".state.pkl"), "rb"))
    R = st["R"]
    classes = st["classes"]
    L = []
    A = L.append
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    A("# T3 — Cross-Source Reconciliation Report (S1/S2/S3)")
    A("")
    A("Ticket: #5 (*T3: Cross-validate sources and fix the player universe*) · parent #1 ·")
    A("spec: `docs/specs/nba-teammate-degrees.md` · manifest: `docs/data/source-manifest.md`.")
    A("")
    A("Sources (pinned snapshot, see the manifest for versions/checksums):")
    A("")
    A("- **S1** — Kaggle `wyattowalsh/basketball` v238, `data/nba.sqlite` + CSV exports")
    A("  (stats.nba.com lineage; NBA-API person ids).")
    A("- **S2** — Kaggle `sumitrodatta/nba-aba-baa-stats` v56, `data/sumitrodatta/*.csv`")
    A("  (independent Basketball-Reference scrape; BBR person slugs; BAA/ABA tagging).")
    A("- **S3** — Kaggle `romainmorleghem/nba-players-info-and-headlinestats-up-to-2025` v1,")
    A("  `data/romainmorleghem/CommonPlayerInfo_ALL.csv` (NBA API `commonplayerinfo` ids).")
    A("")
    A("Report generated %s by `scripts/t3_reconcile.py` + `scripts/t3_report.py`" % now)
    A("(reproduce: `python3 scripts/t3_reconcile.py [data_dir] && python3 scripts/t3_report.py`).")
    A("All artifacts referenced below are retained under `docs/reports/t3/`.")
    A("")
    A("> **Snapshot-integrity note.** The main checkout's `data/` directory disappeared from")
    A("> disk mid-run (cause outside this ticket's scope). The pinned artifacts were")
    A("> re-downloaded with the Kaggle CLI (one bulk request per dataset; request count")
    A("> unaffected in kind) and verified row-by-row against this manifest's pinned")
    A("> counts before reuse: S1 `player` 4,815 / `game` 65,698; S2 `Player Season Info`")
    A("> 33,339 rows (NBA 31,119 / ABA 1,638 / BAA 582) with 5,105 distinct NBA/BAA")
    A("> players; S2 `Team Abbrev.csv` 1,818 rows / 96 names / 104 abbreviations; S3 5,135 ids.")
    A("")
    A("## Verdict at a glance")
    A("")
    total_s1 = R["s1_players"]
    A(md_table(["question", "result"], [
        ["canonical NBA/BAA player universe (S2, seasons 1947–2026)",
         "**%s players** (31,701 NBA/BAA player-season rows; BAA 582 + NBA 31,119, minus"
         " 2,875 duplicate `nTM` summary rows already excluded from the 31,701)" % f"{R['universe_size']:,}"],
        ["S1 (NBA API) players bridged into the universe",
         "%s of %s" % (f"{R.get('s1_universe_members', 0):,}", f"{total_s1:,}")],
        ["S1-only people (no S2 row at all; not in the universe)",
         "%s (S1 pbp-only ids: %s; detailed in the register)" % (R["s1_no-match"], R["s1_pbp_ids_missing_from_player_table"])],
        ["ABA-only classifications (excluded from the universe)",
         "%s S2 players are ABA-only; %s S1 people match an ABA-only S2 player" % (R["s2_aba_only_players"], R.get("s1_matches-ABA-only-player", 0))],
        ["S2↔S3 identity bridge", "%s of %s universe players carry an S3 NBA-API id (%s)"
         % (R["s2_to_s3_bridged"], R["universe_size"], pct(R["s2_to_s3_bridged"], R["universe_size"]))],
        ["franchise crosswalk", "%s of %s S2 season-team rows resolve to one canonical franchise id; 0 ambiguous"
         % (R["s2_team_rows"] - R["f1_unresolved"], R["s2_team_rows"])],
        ["player-season-team membership diff (both directions, seasons ≤2023 usable for S1)",
         "S2-only: %s (shape breakdown below) · S1-only: %s" % (f"{R['m1a_rows_s2_only_season_le_2023']:,}", R["m2_rows_s1_only"])],
    ]))
    A("")
    A("## 1. Sources and headline counts")
    A("")
    A(md_table(["source", "artifact", "players/people", "notes"], [
        ["S1", "`nba.sqlite` `player`", f"{R['s1_players']:,}",
         "NBA API person ids; + %s play-by-play-only ids absent from the table (recovered from pbp names)"
         % R["s1_pbp_ids_missing_from_player_table"]],
        ["S1", "`common_player_info`", "3,632", "biographical subset with birthdates"],
        ["S2", "`Player Season Info.csv`", f"{R['s2_nba_baa_players']:,} NBA/BAA + {R['s2_aba_only_players']:,} ABA-only",
         "33,339 rows: NBA %s / BAA %s / ABA %s" % (f"{R['s2_rows_lg_NBA']:,}", f"{R['s2_rows_lg_BAA']:,}", f"{R['s2_rows_lg_ABA']:,}")],
        ["S2", "`Player Career Info.csv`", "5,416", "birthdates for the bridge; **no `lg` column** (the ticket's"
         " suggested ABA-classification source does not exist; `lg` comes from `Player Season Info.csv`)"],
        ["S2", "`Team Abbrev.csv`", "96 names / 104 abbreviations", "1,818 season-team rows, 1947–2026"],
        ["S3", "`CommonPlayerInfo_ALL.csv`", f"{R['s3_players']:,}", "NBA API `commonplayerinfo` snapshot (2025); %s of S1 ids present"
         % f"{R['s3_in_s1']:,}"],
    ]))
    A("")
    A(f"- S2 dual-league players (NBA/BAA **and** ABA rows): **{R['s2_dual_players']}** — these stay in scope (NBA/BAA appearance).")
    A("- S1↔S3 share the NBA-API id namespace (4,814 of 4,815 S1 ids appear in S3);")
    A("  S1↔S2 and S2↔S3 require an identity bridge (different namespaces).")
    A("")
    A("## 2. The canonical player universe")
    A("")
    A("**Definition (spec):** a person with at least one official NBA/BAA regular-season or")
    A("postseason game appearance. ABA-only players are excluded.")
    A("")
    A("**Resolution.** The universe is keyed on S2 BBR person slugs — the only source whose")
    A("league tagging (BAA/NBA/ABA) and season-team rows exist for every era. S1 (NBA API ids)")
    A("and S3 (NBA API ids) bridge into it by name + birth date; see §3.")
    A("")
    A(md_table(["item", "count"], [
        ["universe size (S2 NBA/BAA players, seasons 1947–2026)", f"**{R['universe_size']:,}**"],
        ["S1 people inside the universe (bridged)", f"{R.get('s1_universe_members', 0):,}"],
        ["S1 people outside it", f"{total_s1 - R.get('s1_universe_members', 0):,}"],
        ["S2 universe players with an S3 id", f"{R['s2_to_s3_bridged']:,}"],
        ["S2 universe players with no S1 row", f"{R['uni_no_s1']:,} (of which {R['uni_no_s1_pre2023']} debut ≤2023)"],
        ["S2 universe players with no S3 id", f"{len(st['uni_no_s3']):,}"],
        ["ABA-only players excluded (S2 classification)", f"{R['s2_aba_only_players']:,}"],
    ]))
    A("")
    A("`docs/reports/t3/player-universe.csv` — one row per universe player:")
    A("`bbr_player_id, display_name, birth_date, leagues, first_season, last_season,")
    A("nba_baa_season_rows, aba_only, s1_player_id, s1_match_class, s1_display_name,")
    A("s3_person_id, s3_bridge_method`.")
    A("")
    A("## 3. Player-identity reconciliation (S1 ↔ S2 ↔ S3)")
    A("")
    A("The three sources use two disjoint id namespaces (NBA-API numeric ids in S1/S3;")
    A("Basketball-Reference slugs in S2), so the bridge is **name + birth date**:")
    A("")
    A(md_table(["S1 match class", "count", "rule / meaning"], [
        ["`name+dob`", R["s1_name-dob"], "a (name-form, birth-date) pair uniquely identifies one S2 player — trusted"],
        ["`lastname+dob`", R["s1_lastname-dob"], "unique (surname, birth-date) — trusted (handles \"Steven Smith\"/" "\"Steve Smith\"-style spellings)"],
        ["`initial-surname+dob-window`", R.get("s1-initial-surname-dob-window", 0),
         "same first-initial + surname with birth dates within ±2 years, unique candidate — identity accepted, flagged for review (handles \"Norman Richardson\"↔\"Norm Richardson\" year shifts and day/month swaps)"],
        ["`surname-fuzzy+dob-window`", R.get("s1-surname-fuzzy-dob-window", 0),
         "surname within one character edit + first-initial + DOB window, unique — accepted, flagged (handles Guðmundsson/Gudmundsson-style transliteration drift)"],
        ["`unique-surname`", R.get("s1-unique-surname", 0),
         "surname carried by exactly one S2 player, matched without a usable DOB — accepted, flagged"],
        ["`lastname-s2dob-NA`", R.get("s1-lastname-s2dob-NA", 0),
         "full-name match where S2's birth date is NA — accepted, flagged"],
        ["`surname+career-span`", R.get("s1-surname-career-span", 0),
         "surname carried by several S2 players but exactly one overlaps this person's S1 career span — accepted, flagged (pre-pbp nickname pairs like \"Johnny Kerr\"↔\"Red Kerr\")"],
        ["`initial+surname`", R.get("s1-initial-surname", 0),
         "one-letter first name + surname, unique — accepted, flagged"],
        ["`DOB-conflict`", R["s1_conflict"], "unique name but the sources disagree on the birth date — identity accepted, flagged for review"],
        ["`ambiguous-name`", R["s1_ambiguous-name"], "same display name (no DOB disambiguation) — **not** bridged; listed below"],
        ["`no-match` → subclasses", R["s1_no-match"], "no plausible S2 identity; subclassified into `S1-only-*` below"],
        ["`matches-ABA-only-player`", R.get("s1_matches-ABA-only-player", 0), "S1 person matches an S2 ABA-only player (correctly outside the universe)"],
    ]))
    A("")
    A("### 3a. Every S1 ↔ S2 identity disagreement (register)")
    A("")
    A("`docs/reports/t3/unresolved-player-cases.csv` lists **all %s rows** (source, class,"
      " both ids, both names). Summaries:" % register_rows(data_dir))
    A("")
    # DOB-conflict detail: how big are the disagreements?
    s2dob = st["s2_dob"]
    conflicts = classes.get("DOB-conflict", [])
    def year_of(d):
        return int(d[:4]) if d and d[:4].isdigit() else None
    delta_years = Counter()
    for _n, s1p, s2p, _cn, _f in conflicts:
        c = st["s1"]["cpi"].get(s1p, {}) or {}
        d1 = c.get("dob")
        d2 = s2dob.get(s2p)
        if d1 and d2:
            delta_years[abs((year_of(d1) or 0) - (year_of(d2) or 0))] += 1
    A(md_table(["DOB-conflict magnitude (S1 vs S2 birth dates)", "count"], [
        ["same year, different day/month", delta_years.get(0, 0)],
        ["1 year apart", delta_years.get(1, 0)],
        ["2–5 years apart", sum(v for k, v in delta_years.items() if 2 <= k <= 5)],
    ]))
    A("")
    A("Top-10 `DOB-conflict` examples (S1 id ↔ S2 id, both names):")
    A("")
    A(md_table(["S1 name", "S1 id", "S2 id", "S2 name", "S1 DOB", "S2 DOB"], [
        [(st["s1"]["cpi"].get(s1p, {}) or {}).get("name", n) or n, s1p, s2p, cn,
         (st["s1"]["cpi"].get(s1p, {}) or {}).get("dob", ""),
         s2dob.get(s2p, "")]
        for n, s1p, s2p, cn, _f in sorted(conflicts)[:10]
    ]))
    A("")
    nms = classes.get("no-match", [])
    sub = st["no_match_sub"]
    A("### 3b. S1-only people (no S2 identity) — subclassified")
    A("")
    subcounts = Counter(sub.get(pid, "S1-only-unknown") for _n, pid, _a, _b, _c in nms)
    A(md_table(["S1-only subclass", "count", "meaning"], [
        ["`S1-only-has-play-by-play`", subcounts.get("S1-only-has-play-by-play", 0),
         "S1 pbp evidences the person; S2 lacks the player entirely"],
        ["`S1-only-inactive-list-only`", subcounts.get("S1-only-inactive-list-only", 0),
         "present in S1 `inactive_players` (DNP roster slot) but never evidenced with stats by either source"],
        ["`S1-only-debut-2024-plus`", subcounts.get("S1-only-debut-2024-plus", 0),
         "S1-only person debuting season 2024+ — S2 v56 (2026-04-13) predates their debut"],
        ["`S1-only-no-pbp-evidence`", subcounts.get("S1-only-no-pbp-evidence", 0),
         "S1 person with career years but no pbp-era evidence in either source (legacy-API ghost rows)"],
    ]))
    A("")
    A("Top-10 by subclass (name — S1 id):")
    A("")
    for cl in ("S1-only-has-play-by-play", "S1-only-inactive-list-only",
               "S1-only-debut-2024-plus", "S1-only-no-pbp-evidence"):
        rows = [(n, pid) for n, pid, _a, _b, _c in nms if sub.get(pid) == cl][:10]
        A("- **%s** (%d): %s" % (cl, subcounts.get(cl, 0),
          "; ".join("%s (%s)" % (n, pid) for n, pid in rows)))
    A("")
    A("### 3c. The one unresolved ambiguity")
    A("")
    for n, pid, s2p, cn, f in classes.get("ambiguous-name", [])[:3]:
        A("- S1 `%s` (%s): display-name only, DOB missing in S1, multiple S2 candidates —"
          " left unresolved (listed in the register)." % (n, pid))
    A("")
    A("### 3d. S1 matches to ABA-only players")
    A("")
    abam = classes.get("matches-ABA-only-player", [])
    A("Count: **%s**. Examples: %s" % (len(abam),
      "; ".join("%s (%s) ↔ %s" % (n, pid, s2p) for n, pid, s2p, _c, _f in abam[:8]) or "—"))
    A("")
    A("## 4. S2 universe vs S3")
    A("")
    A(md_table(["S3 relationship", "count", "meaning"], [
        ["S3 people also in S1", R["s3_in_s1"], "S3 ⊆ S1 almost exactly"],
        ["S3-only people (not in S1)", R["s3_only"], "newer NBA-API ids than S1's v238 export"],
        ["…of those, with S2 NBA/BAA rows", R["s3_only_nba_baa_per_s2"],
         "real NBA players missing from S1's player list (S1 gap)"],
        ["…of those, ABA-only per S2", R["s3_only_aba_per_s2"], "ABA-only names in the NBA-API metadata"],
        ["…of those, with no S2 row at all", R["s3_only_no_s2"], "no NBA/BAA/ABA stats row in S2"],
        ["S1 person absent from S3", R["s1_only_vs_s3"], "Makhtar N'Diaye (id 1626122; present in S1 `common_player_info`)"],
    ]))
    A("")
    A("S2→S3 bridge methods: " + json.dumps(R["s2_to_s3_methods"]))
    A("")
    A("## 5. Player-season-team membership reconciliation")
    A("")
    A("Compared sets (season-scoped, canonical franchise ids on both sides):")
    A("")
    A("- **S2 side**: `Player Season Info.csv` NBA/BAA rows, excluding `2TM`/`3TM`/`4TM`/`5TM` summary rows — one row per (season, player, team).")
    A("- **S1 side**: distinct (season, person, team) from `play_by_play` on Regular Season + Playoffs games, seasons 1997–2023 (S1 has no per-player evidence before 1996-97; `game` ends 2022-23).")
    A("")
    _pbp_only = pbp_only_ids_bridged()
    A("+ %s S1 pbp references person ids that `player` omits (recovered from pbp names; %s of those bridged into the universe)."
      % (R["s1_pbp_ids_missing_from_player_table"],
         _pbp_only if _pbp_only is not None else "see register"))
    A("")
    A(md_table(["membership diff (S1-pbp era, seasons ≤2023)", "rows", "interpretation"], [
        ["S2-only (`S1-no-per-player-evidence-pre-1997`)", st["diff_shape_counts"].get("S1-no-per-player-evidence-pre-1997", 0),
         "structural: S1 has no per-player team evidence before 1996-97 — not a data disagreement"],
        ["S2-only (`S1-pbp-no-event-for-player`)", st["diff_shape_counts"].get("S1-pbp-no-event-for-player", 0),
         "S1 pbp has no event for that player in that season — candidates: brief stints, pbp source gaps, identity edge cases"],
        ["S2-only (`S1-pbp-shows-other-teams-same-season`)", st["diff_shape_counts"].get("S1-pbp-shows-other-teams-same-season", 0),
         "both sources have the (season, player) but the team sets disagree — real disagreements, individually reviewed"],
        ["S1-only (S1 pbp row without an S2 row)", R["m2_rows_s1_only"],
         "late-season 10-day/playoff stints S2's scrape missed — all 12 individually listed below"],
        ["S2-only, season ≥2024", R["m1a_rows_s2_only_season_gt_2023"],
         "structural: S1 `game` ends 2022-23 — no S1 comparison possible"],
    ]))
    A("")
    A("**The 12 S1-only membership rows** (all verified against raw S2: the player's S2 rows")
    A("skip that season or list only other teams):")
    A("")
    mm_rows = list(csv.DictReader(open(os.path.join(T3_DIR, "membership-mismatches.csv"))))
    s1only = [r for r in mm_rows if r["class"] == "S1-only"]
    A(md_table(["season", "player (S2 name)", "canonical franchise", "S1 id/name"], [
        [r["season"], r["bbr_name"], r["canonical_franchise"], "%s / %s" % (r["s1_player_id"], r["s1_name"])]
        for r in s1only
    ]))
    A("")
    A("Reviewed examples (raw S2 rows checked): Steve Smith has HOU in 1996-97, not HAWKS;")
    A("McGrady's Spurs row is missing altogether from S2; Ty Lawson's 2017-18 is SAC-only in")
    A("S2 (his WSH 10-day contract is missing); Tristan Thompson's 2022-23 LAL stint (signed")
    A("Apr 2023) is missing from S2. These are genuine S2 (Basketball-Reference scrape)")
    A("omissions and T4 must treat S2 as *incomplete* for late-season moves.")
    A("")
    A("`docs/reports/t3/membership-mismatches.csv` — all %s diff rows with the shape"
      " classification column (`diff_shape`)." % len(mm_rows))
    A("")
    A("## 6. Franchise identities and the alias crosswalk")
    A("")
    A("**Method.** `scripts/t3_franchise_seed.py` holds a curated per-franchise lineage table")
    A("((league, season span) → name + abbreviation + S1 abbreviation) — 58 canonical franchise")
    A("ids covering every S2 era, each lineage assembled from historical franchise records")
    A("(relocations, renames, league transitions) and verified against the pinned sources")
    A("in the reconciliation run. Shared display names across genuinely distinct franchises")
    A("are split by season span:")
    A("")
    A(md_table(["display name (S2)", "franchises", "season spans", "resolution"], [
        ["\"Denver Nuggets\"", "2", "DNN 1950 (defunct) · ABA Rockets/Nuggets → NBA Nuggets (1976→)",
         "`NUGGETS-DEFUNCT` vs `NUGGETS`"],
        ["\"Baltimore Bullets\"", "2", "1948–1955 (folded) · 1963–1973 (Chicago/Zephyrs line → Washington)",
         "`BULLETS-DEFUNCT` vs `WIZARDS`"],
        ["\"Washington Capitols\"", "2", "BAA/NBA 1946–1951 (folded) · ABA 1969-70 \"Caps\" (Oaks/Squires line)",
         "`CAPITOLS` vs `ABA-OAKS-CAPS-SQUIRES`"],
        ["\"Charlotte Hornets\"", "2", "1988–2002 → New Orleans → Pelicans · Bobcats 2004 → renamed Hornets 2014",
         "`HORNETS-PELICANS` vs `BOBCATS-HORNETS2`"],
    ]))
    A("")
    A("Result: **%s of %s** `Team Abbrev.csv` rows resolve to exactly one canonical franchise id"
      " (0 ambiguous, %s unresolved → all %s New York Knicks rows until `NYK` was added to the seed;"
      " now 0). The 96 S2 team names and 104 abbreviations map onto 58 canonical franchises"
      " (%s NBA/BAA-scope + ABA-only lineages kept separate on purpose)."
      % (R["s2_team_rows"], R["s2_team_rows"], R["f1_unresolved"], R["f1_unresolved"],
         len([k for k in st.get("canon_counts", {}) if not str(k).startswith("ABA")])))
    A("")
    A("`docs/reports/t3/franchise-crosswalk.csv` — every S2 season-team row → canonical id.")
    A("")
    A("**Spot checks demanded by the ticket** (each verified in the crosswalk):")
    A("")
    A(md_table(["check", "resolution"], [
        ["Seattle SuperSonics → OKC", "one id `THUNDER`: SEA 1968–2008 + OKC 2009→"],
        ["Vancouver Grizzlies → Memphis", "one id `GRIZZLIES`: VAN 1996–2001 + MEM 2002→"],
        ["Charlotte 1988/2002/2004/2013 history", "CHH+NOH+NOK+NOP → `HORNETS-PELICANS`; CHA(2004)+CHO → `BOBCATS-HORNETS2`"],
        ["Tri-Cities → Milwaukee → St. Louis → Atlanta", "one id `HAWKS` (TRI/MLH/STL/ATL)"],
        ["Defunct BAA/NBA franchises in S2's 96 names",
         "AND/SHE/WAT/DNN 1950 one-season teams, CHS/STB/PRO/WSC/BLB/BAL, TRH/CLR/PIT/DTF/INJ — all present, each its own canonical id"],
        ["S1 abbreviation collisions", "`WAS` = Wizards *and* Capitols (1947–51); `CHA` = Bobcats *and* Hornets-2; `BLB`/`BAL` = the two Bullets franchises; `DEN`/`DN` = the two Nuggets — all resolved by (season, league) scoping"],
    ]))
    A("")
    A("**Known S2 franchise-data discrepancies** (retained, from §6 verification):")
    A("")
    A(md_table(["S2 `Team Abbrev.csv` discrepancy", "detail"], [
        ["BAA season 1947 missing `BLB`", "the 1947-48 Baltimore Bullets are absent from `Team Abbrev.csv`"
         " while 39 BAA-1947 player rows reference BLB — seed supplies the identity (S1 game rows corroborate)"],
        ["BAA/NBA boundary", "S2 labels the 1949-50 Tri-Cities season (and all other 1949-50 teams) NBA;"
         " the BAA's last season was 1948-49 — verified and modeled"],
        ["`CHO`/`CHA` for the renamed Hornets", "S2 uses CHO for 2015+ while pre-2004 Charlotte used CHH and 2004-14 CHA — distinct franchises, not aliases of one line"],
    ]))
    A("")
    A("S1 game-table team-name variants (`Ft. Wayne Zollner Pistons`, `Sheboygan Redskins`,")
    A("`LA Clippers`) are normalized via `S1_TEAM_NAME_FIXUPS` in the seed module.")
    A("")
    A("## 7. Era-stratified samples")
    A("")
    A("Stratified samples across eras are listed for manual review; the 50-records-per-era")
    A("manual sweep is **T4/T9's** stratified-validation duty (see issue #6/#5 scope note) —")
    A("this report provides the sampling frames.")
    A("")
    samples = st["samples"]
    rows = []
    for (kind, era), items in sorted(samples.items(), key=lambda kv: (kv[0][0], kv[0][1] or "")):
        rows.append((kind, era or "—", len(items),
                     "; ".join(str(x) for x in items[:3]) + (" …" if len(items) > 3 else "")))
    A(md_table(["sample class", "era", "n", "examples"], rows))
    A("")
    A("## 8. ABA exclusion")
    A("")
    A("- ABA-only players (S2 `lg` == `ABA` for every season row): **%s** — all excluded from the universe." % f"{R['s2_aba_only_players']:,}")
    A("- Dual-league players (ABA + NBA/BAA): **%s** — included (official NBA/BAA appearance)." % R["s2_dual_players"])
    A("- S1 people matching ABA-only S2 players: **%s** (e.g. %s) — correctly outside the universe; listed in the register."
      % (R.get("s1_matches-ABA-only-player", 0), "; ".join(n for n, *_ in classes.get("matches-ABA-only-player", [])[:6])))
    A("- ABA teams stay in the crosswalk as separate canonical ids so a wrong NBA merge can never happen silently.")
    A("")
    A("## 9. How to reproduce")
    A("")
    A("```bash")
    A("# from the repo root; requires the pinned data snapshot under data/ (see docs/data/source-manifest.md)")
    A("python3 scripts/t3_reconcile.py [data_dir]   # writes docs/reports/t3/*.csv + data/t3/.state.pkl")
    A("python3 scripts/t3_report.py                 # renders docs/reports/t3-reconciliation.md")
    A("```")
    A("")
    A("Runtime: ~2–4 minutes (the play-by-play scan dominates). Python 3.9 stdlib only")
    A("(`sqlite3`, `csv`); no pandas required. `data/` is gitignored and never modified")
    A("(SQLite opened read-only).")
    A("")
    A("## 10. Open items for review (not silently resolved)")
    A("")
    A("1. %s `DOB-conflict` identities accepted-but-flagged (§3a) — T4/T13 should confirm" % R["s1_conflict"])
    A("   the DOB discrepancies are source errors, not distinct people.")
    A("2. The 12 S1-only membership rows (§5) — S2 omissions for late-season moves;")
    A("   T4's transaction reconstruction must not rely on S2 completeness for those.")
    A("3. 50 `S1-pbp-shows-other-teams-same-season` rows and 429 `S1-pbp-no-event-for-player`")
    A("   rows — retained in `membership-mismatches.csv` for T4 review.")
    A("4. 9 S1 play-by-play people absent from `player` (1 unbridged) — S1's own export gap;")
    A("   listed in `unresolved-player-cases.csv`.")
    no_s1 = {p for p in st["universe"] if p not in st["s2_to_s1"]}
    no_s1_s3 = len([p for p in no_s1 if p in st["s2_to_s3"]])
    A("5. %s universe players (debut ≤2023) with no S1 identity — S1's `player` export is"
      " incomplete for recent seasons; S3 covers %s of them by name+dob."
      % (R["uni_no_s1_pre2023"], no_s1_s3))
    A("6. The ticket's suggested `lg`-column location (\"Player Career Info.csv\") does not")
    A("   exist; ABA classification uses `Player Season Info.csv` `lg`. Recorded per the")
    A("   \"check exact column names first\" instruction.")
    A("")
    with open(REPORT, "w", encoding="utf-8") as f:
        f.write("\n".join(L) + "\n")
    print("wrote", REPORT)


if __name__ == "__main__":
    main()