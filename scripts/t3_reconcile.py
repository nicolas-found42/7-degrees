#!/usr/bin/env python3
"""T3 — cross-validate S1/S2/S3 and fix the canonical player universe (ticket #5).

Reconciles, across the three pinned bulk sources:
  S1  Kaggle wyattowalsh/basketball v238 (nba.sqlite; stats.nba.com lineage)
  S2  Kaggle sumitrodatta/nba-aba-baa-stats v56 (Basketball-Reference lineage; CSVs)
  S3  Kaggle romainmorleghem/nba-players-info-and-headlinestats-up-to-2025 v1
      (NBA API commonplayerinfo snapshot; CSV)

Produces (all under docs/reports/t3/):
  - player-universe.csv          the canonical NBA/BAA player universe
  - franchise-crosswalk.csv      every S2 (season,lg,team,abbr) row -> canonical franchise
  - unresolved-player-cases.csv  every unresolved player-identity disagreement
  - membership-mismatches.csv    every player-season-team disagreement between S2 and S1
  - plus the retained report docs/reports/t3-reconciliation.md (written here)

Every reconciliation class is counted and listed; nothing is silently dropped.

Run:  python3 scripts/t3_reconcile.py [data_dir]
  data_dir defaults to the repo's `data/` directory, falling back to the
  sibling main checkout `../7-degrees/data` when the worktree has none.
  Read-only against data/: sqlite opened with mode=ro, CSVs read-only.
"""

import csv
import json
import os
import re
import sqlite3
import sys
import unicodedata
from collections import Counter, defaultdict
from datetime import date

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from t3_franchise_seed import (  # noqa: E402
    FRANCHISES,
    PLAYER_ALIASES_S1_TO_S2,
    S1_EXCLUDED_ABBREVIATIONS,
    S1_TEAM_NAME_FIXUPS,
    s1_season_id_to_season_year,
)

REPO = os.path.dirname(HERE)
S2_DIR_NAME = "sumitrodatta"
REPORT_DIR = os.path.join(REPO, "docs", "reports")
T3_DIR = os.path.join(REPORT_DIR, "t3")

# ---------------------------------------------------------------- utilities --
SUFFIX_TOKENS = {"jr", "sr", "ii", "iii", "iv", "v"}


def canon(name):
    s = unicodedata.normalize("NFKD", name or "")
    s = "".join(c for c in s if not unicodedata.combining(c))
    # characters with no NFKD decomposition
    for a, b in (("ı", "i"), ("ł", "l"), ("ø", "o"), ("đ", "d"), ("ð", "d"), ("ß", "ss")):
        s = s.replace(a, b)
    s = s.casefold().replace("’", "").replace("'", "").replace("-", "")
    s = re.sub(r"[^a-z0-9]+", " ", s)
    return " ".join(s.split())


def name_variants(name):
    """Exact canonical form + nickname-tolerant / suffix-stripped forms."""
    base = canon(name)
    if not base:
        return set()
    out = {base}
    raw_toks = base.split()
    toks = [t for t in raw_toks if t not in SUFFIX_TOKENS]
    # name ending in a generational suffix: add the stripped form
    # ("brian bowen ii" -> "brian bowen")
    if len(raw_toks) > len(toks) and len(toks) >= 2:
        out.add(" ".join(toks))
    if len(toks) >= 3:
        # drop single-letter middle initials: "george g gervin" -> "george gervin"
        toks2 = [t for t in toks if not (len(t) == 1 and t != toks[0] and t != toks[-1])]
        if len(toks2) >= 2:
            out.add(" ".join(toks2))
            out.add(toks2[0] + " " + toks2[-1])
    if len(toks) == 3:
        out.add(toks[0] + " " + toks[2])
    return {v for v in out if v}


def dob_ok(d):
    return bool(re.match(r"^\d{4}-\d{2}-\d{2}$", d or ""))


def dob_tuple(d):
    return (int(d[0:4]), int(d[5:7]), int(d[8:10])) if dob_ok(d) else None


def dob_corroborates(d1, d2):
    """Require a shared birth year or an exact month/day beyond a ±2-year window."""
    t1, t2 = dob_tuple(d1), dob_tuple(d2)
    return bool(t1 and t2 and (t1[0] == t2[0] or t1[1:] == t2[1:]))


def levenshtein1(a, b):
    """True when a and b differ by exactly one character edit (substitution)."""
    if a == b or abs(len(a) - len(b)) > 1:
        return False
    if len(a) == len(b):
        return sum(1 for x, y in zip(a, b) if x != y) == 1
    short, long = (a, b) if len(a) < len(b) else (b, a)
    i = 0
    while i < len(short) and short[i] == long[i]:
        i += 1
    return long[i + 1:] == short[i:]


def per_player_by_surname(surname, idx_name):
    ps = set()
    for f, pids in idx_name.items():
        if f.split()[-1] == surname:
            ps |= pids
    return ps


def s1_dob_norm(d):
    m = re.match(r"^(\d{4}-\d{2}-\d{2})", (d or "").replace("T", " "))
    return m.group(1) if m else None


def read_csv(path):
    with open(path, newline="", encoding="utf-8-sig") as f:
        return list(csv.DictReader(f))


def write_csv(path, header, rows):
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(header)
        w.writerows(rows)


def md_table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    for r in rows:
        out.append("| " + " | ".join(str(x) for x in r) + " |")
    return "\n".join(out)


def md_list(items):
    return "\n".join("- " + str(i) for i in items)


# ------------------------------------------------------------------- loading --
def load_s2(data_dir):
    s2dir = os.path.join(data_dir, S2_DIR_NAME)
    season = read_csv(os.path.join(s2dir, "Player Season Info.csv"))
    career = read_csv(os.path.join(s2dir, "Player Career Info.csv"))
    teams = read_csv(os.path.join(s2dir, "Team Abbrev.csv"))
    return season, career, teams


def load_s3(data_dir):
    return read_csv(os.path.join(data_dir, "romainmorleghem", "CommonPlayerInfo_ALL.csv"))


def load_s1(data_dir):
    path = os.path.join(data_dir, "nba.sqlite")
    con = sqlite3.connect("file:%s?mode=ro" % path, uri=True)
    cur = con.cursor()
    players = {str(r[0]): r[1] for r in cur.execute("select id, full_name from player")}
    cpi = {}
    for r in cur.execute(
        "select person_id, display_first_last, display_last_comma_first, birthdate,"
        " from_year, to_year from common_player_info"
    ):
        cpi[str(r[0])] = {
            "name": r[1],
            "name_lcf": r[2],
            "dob": s1_dob_norm(r[3]),
            "from_year": r[4],
            "to_year": r[5],
        }
    team = [tuple(r) for r in cur.execute(
        "select id, full_name, abbreviation, nickname, city, state, year_founded from team order by id")]
    team_history = [tuple(r) for r in cur.execute(
        "select team_id, city, nickname, year_founded, year_active_till from team_history order by team_id, year_founded")]
    # game-table team usage: abbr -> names, seasons, season types (in-scope types only)
    gteams = defaultdict(lambda: {"names": set(), "seasons": set(), "types": Counter()})
    for ab, nm, ses, st in cur.execute(
        "select team_abbreviation_home, team_name_home, substr(season_id,2,4)+1, season_type from game "
        "union select team_abbreviation_away, team_name_away, substr(season_id,2,4)+1, season_type from game"
    ):
        g = gteams[ab]
        g["names"].add(nm)
        g["seasons"].add(int(ses))
        g["types"][st] += 1
    # per-season in-scope team sets and row counts (franchise evidence + gaps)
    season_counts = defaultdict(lambda: Counter())
    for ses, st, c in cur.execute(
        "select substr(season_id,2,4)+1, season_type, count(*) from game group by 1,2"
    ):
        season_counts[int(ses)][st] += c
    season_teams = defaultdict(set)
    for ab, ses in cur.execute(
        "select distinct team_abbreviation_home, substr(season_id,2,4)+1 from game "
        "where season_type in ('Regular Season','Playoffs') "
        "union select distinct team_abbreviation_away, substr(season_id,2,4)+1 from game "
        "where season_type in ('Regular Season','Playoffs')"
    ):
        season_teams[int(ses)].add(ab)
    return con, {"players": players, "cpi": cpi, "team": team, "team_history": team_history,
                 "gteams": gteams, "season_counts": season_counts, "season_teams": season_teams}


def load_s1_membership(con):
    """Distinct (season, player_id, team_abbr) from play_by_play on RS/PO games only."""
    cur = con.cursor()
    cur.execute(
        "create temp table m1 as "
        "select distinct substr(g.season_id,2,4)+1 as ses, s.player1_id as pid, "
        "s.player1_team_abbreviation as abbr "
        "from play_by_play s join game g on g.game_id = s.game_id "
        "where s.player1_team_abbreviation is not null and s.player1_id > 0 "
        "and g.season_type in ('Regular Season','Playoffs')"
    )
    rows = [(int(a), str(b), c) for a, b, c in cur.execute("select ses, pid, abbr from m1")]
    n_abbr_events = cur.execute(
        "select count(*) from play_by_play s join game g on g.game_id=s.game_id "
        "where g.season_type in ('Regular Season','Playoffs') and s.player1_id>0 "
        "and s.player1_team_abbreviation is not null"
    ).fetchone()[0]
    return rows, n_abbr_events


# ----------------------------------------------------------- franchise logic --
from t3_franchise_mapping import franchise_era_index, resolve_s1_abbr


def resolve_s2_row(era_idx, season, lg, team, abbr):
    cands = set()
    for cid, era in era_idx.get((lg, int(season)), []):
        if era[2] == team or era[1] == team or era[2] == abbr:
            cands.add(cid)
    return cands


# ------------------------------------------------------------------ matching --
class Bridge:
    """S1 (NBA-API ids) <-> S2 (BRef slugs) player identity bridge on name+DOB."""

    def __init__(self, s2_career, s2_season, aba_only, s2_spans=None):
        self.s2_spans = s2_spans or {}  # pid -> (first_season, last_season)
        self.s2_names = defaultdict(set)      # pid -> display names seen
        for r in s2_season:
            self.s2_names[r["player_id"]].add(r["player"])
        for r in s2_career:
            self.s2_names[r["player_id"]].add(r["player"])
        self.s2_dob = {r["player_id"]: s1_dob_norm(r["birth_date"]) for r in s2_career}
        self.aba_only = set(aba_only)
        self.idx_name = defaultdict(set)
        self.idx_name_dob = defaultdict(set)
        self.idx_last_dob = defaultdict(set)
        self.idx_last_dates = defaultdict(list)   # surname -> [(pid, (y,m,d))]
        self.idx_initlast = defaultdict(list)     # (first-initial, surname) -> [(pid, (y,m,d))]
        self.unique_lastnames = set()             # surnames carried by exactly one S2 player
        self.idx_aba_name = defaultdict(set)
        self.aba_dob = {}
        # one-letter surname-name forms ("N Vlcek", "J Dowtin Jr."); index by
        # (initial, surname) so an S1 "N N Vlcek"-style row can still resolve
        self.idx_initial_surname = defaultdict(set)
        for pid, names in self.s2_names.items():
            dob = self.s2_dob.get(pid)
            forms = set()
            for n in names:
                forms |= name_variants(n)
            if pid in aba_only:
                for f in forms:
                    self.idx_aba_name[f].add(pid)
                self.aba_dob[pid] = dob
                continue
            for f in forms:
                self.idx_name[f].add(pid)
                if dob:
                    self.idx_name_dob[(f, dob)].add(pid)
                    self.idx_last_dob[(f.split()[-1], dob)].add(pid)
                    dd = dob_tuple(dob)
                    if dd:
                        self.idx_last_dates[f.split()[-1]].append((pid, dd))
                        self.idx_initlast[(f.split()[0][0], f.split()[-1])].append((pid, dd))
            for n in names:
                ct = canon(n).split()
                ct = [t for t in ct if t not in SUFFIX_TOKENS]
                if len(ct) == 2 and len(ct[0]) == 1:
                    self.idx_initial_surname[(ct[0], ct[1])].add(pid)
        # surnames carried by exactly one player (used by the s2dob-NA rule)
        per_last = defaultdict(set)
        for f, pids in self.idx_name.items():
            for p in pids:
                per_last[f.split()[-1]].add(p)
        self.unique_lastnames = {ln for ln, ps in per_last.items() if len(ps) == 1}

    def match(self, name_forms, dob, s1_span=None):
        """Return (class, s2_pid|None, detail). dob is the S1-side birthdate;
        s1_span is the S1 person's (from+1, to+1) season span when known."""
        # 1) exact (name-form, dob)
        for f in sorted(name_forms):
            u = self.idx_name_dob.get((f, dob))
            if u and len(u) == 1:
                return ("name+dob", next(iter(u)), f)
        # 2) same-name different-DOB pairs exist in both sources ("Steven Smith"
        #    b.1969 vs b.1983; "Glen Rice" senior vs junior), so (lastname, dob)
        #    uniqueness is checked before any conflict-prone full-name fallback;
        #    S2 may also spell the same person differently ("Steve Smith").
        for f in sorted(name_forms):
            u = self.idx_last_dob.get((f.split()[-1], dob))
            if u and len(u) == 1:
                return ("lastname+dob", next(iter(u)), f)
        # 3) unique full-name match with S2 DOB missing
        for f in sorted(name_forms):
            u = self.idx_name.get(f)
            if u and len(u) == 1:
                pid = next(iter(u))
                d2 = self.s2_dob.get(pid)
                if not d2:
                    return ("name-match-s2-dob-NA", pid, f)
                if d2 != dob:
                    return ("DOB-conflict", pid, f)
                return ("name-only", pid, f)
        # 4) bounded fuzzy window: same first-initial + surname, DOB within ±2
        #    years; require corroboration by birth year or an exact month/day.
        dob_window_unconfirmed = False
        if dob_ok(dob):
            dt = dob_tuple(dob) or (0, 0, 0)
            for f in sorted(name_forms):
                toks = f.split()
                if len(toks) >= 2:
                    nearby = {
                        (p, dd) for p, dd in self.idx_initlast.get((toks[0][0], toks[-1]), [])
                        if abs(dd[0] - dt[0]) <= 2
                    }
                    corroborated = {
                        p for p, dd in nearby
                        if dob_corroborates(dob, "%04d-%02d-%02d" % dd)
                    }
                    if nearby and not corroborated:
                        dob_window_unconfirmed = True
                    if len(corroborated) == 1:
                        return ("initial-surname+dob-window", next(iter(corroborated)), f)
            # 4a) near-surname (one-character difference, surname ≥ 6 chars) with the
            #     same first-initial and a matching/near DOB — flagged, not trusted
            for f in sorted(name_forms):
                toks = f.split()
                if len(toks) >= 2 and len(toks[-1]) >= 6:
                    cands = set()
                    for (ini, ln), lst in self.idx_initlast.items():
                        if ini == toks[0][0] and levenshtein1(ln, toks[-1]):
                            for p, dd in lst:
                                if abs(dd[0] - dt[0]) <= 2:
                                    cands.add(p)
                    if len(cands) == 1:
                        return ("surname-fuzzy+dob-window", next(iter(cands)), f)
        # 4b) exact/near surname unique in S2 with S2 birth date missing — flagged,
        #     not trusted (S2 'Player Career Info.csv' has NA birth dates)
        for f in sorted(name_forms):
            toks = f.split()
            if len(toks) >= 2 and toks[-1] in self.unique_lastnames:
                na = [p for p in self.idx_name.get(f, set())
                      if not self.s2_dob.get(p)]
                if len(na) == 1:
                    return ("lastname-s2dob-NA", na[0], f)
        # 4c) one-letter-first-name forms: S2 "F Crossin"-style rows match
        #     (initial, surname) via idx_initial_surname
        for f in sorted(name_forms):
            toks = f.split()
            if len(toks) >= 2 and len(toks[0]) >= 1 and len(toks[0]) <= 2 and len(toks[-1]) >= 5:
                ini = toks[0][0]
                ps = set(self.idx_initial_surname.get((ini, toks[-1]), set()))
                if len(ps) == 1:
                    return ("initial+surname", next(iter(ps)), f)
        # 4d) career-era rule: no unique surname match — if this S1 person is the
        #     ONLY surname-holder whose career span fits inside S1's from/to years,
        #     bridge but flag (covers "Johnny Kerr"↔"Red Kerr", "Peter"↔"Press
        #     Maravich" nickname/display variants in pre-pbp eras). A nearby same-initial
        #     DOB candidate rejected for contradictory DOB evidence vetoes this weaker
        #     fallback rather than being replaced by a different surname-holder.
        #     Uses the supplied S2 spans and (initial, surname) S1 keys.
        span_hits = [] if dob_window_unconfirmed else self.span_surname_match(name_forms, s1_span)
        if span_hits:
            return ("surname+career-span", span_hits[0], "")
        # 5) explicit ambiguity listings
        for f in sorted(name_forms):
            u = self.idx_name_dob.get((f, dob))
            if u and len(u) > 1:
                return ("ambiguous-name+dob", None, f)
        for f in sorted(name_forms):
            u = self.idx_name.get(f)
            if u and len(u) > 1:
                return ("ambiguous-name", None, f)
        return ("no-match", None, None)

    def span_surname_match(self, name_forms, s1_span):
        """Era-disambiguated surname rule (4d).

        For this S1 person (surname S; career span F..T in the S2 season
        convention), find S2 surname-holders whose entire own span fits inside
        [F, T]. Bridge only when exactly ONE S2 player with that surname fits —
        any name/dob signal would already have matched above.
        """
        if not s1_span:
            return []
        F, Tt = s1_span
        lastnames = {f.split()[-1] for f in name_forms if len(f.split()) >= 2}
        for ln in sorted(lastnames):
            ps = per_player_by_surname(ln, self.idx_name)
            if len(ps) < 2:
                continue  # covered by the unique-surname rule / pure ambiguity
            overlaps = set()
            for p in ps:
                sp = self.s2_spans.get(p)
                if sp and F <= sp[0] and sp[1] <= Tt:
                    overlaps.add(p)
            if len(overlaps) == 1:
                return [next(iter(overlaps))]
        return []

    def match_aba(self, name_forms, dob):
        for f in sorted(name_forms):
            u = self.idx_aba_name.get(f)
            if u and len(u) == 1:
                pid = next(iter(u))
                d2 = self.aba_dob.get(pid)
                if d2 in (None, dob):
                    return pid, f
        return None, None


def s1_official_supplements(identities, appearance_rows):
    """Preserve named official-game players absent from S2 under an NBA id key.

    Callers supply RS/PO appearance evidence, never an inactive roster entry.
    Nameless ids remain unresolved because inventing a person would duplicate
    already-known players. The reserved namespace never pretends to be a BBR slug.
    """
    seasons = defaultdict(set)
    for season, pid, _team in appearance_rows:
        seasons[pid].add(season)
    return {
        pid: ("nba:" + pid, name, tuple(sorted(seasons[pid])))
        for pid, (name, cls, target) in sorted(identities.items())
        if target is None and name and cls == "S1-only-has-play-by-play" and pid in seasons
    }


# ---------------------------------------------------------------------- main --
def main():
    data_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "data")
    if not os.path.isdir(data_dir):
        alt = os.path.normpath(os.path.join(REPO, "..", "7-degrees", "data"))
        if os.path.isdir(alt):
            data_dir = alt
    os.makedirs(T3_DIR, exist_ok=True)
    R = Counter()                 # headline counters
    classes = defaultdict(list)   # class key -> detail rows for the register
    R_lines = []                  # report body lines

    print("loading S2 ...", flush=True)
    s2_season, s2_career, s2_teams = load_s2(data_dir)
    print("loading S3 ...", flush=True)
    s3 = load_s3(data_dir)
    print("loading S1 ...", flush=True)
    con, s1 = load_s1(data_dir)
    print("building S1 play-by-play membership (RS+PO)...", flush=True)
    m1_rows, m1_events = load_s1_membership(con)
    # play_by_play references some person ids that are absent from the `player`
    # table (the API export dropped them). They are still S1-evidenced players;
    # recover their display names from the pbp rows for bridging.
    cur = con.cursor()
    pbp_only_names = {}
    m1_pids = {p for _, p, _ in m1_rows}
    missing_ids = sorted(m1_pids - set(s1["players"]), key=int)
    for pid in missing_ids:
        names = {r[0] for r in cur.execute(
            "select distinct player1_name from play_by_play where player1_id=?", (int(pid),))}
        pbp_only_names[pid] = names.pop() if len(names) == 1 else ("|".join(sorted(names)) if names else "")
    R["s1_pbp_ids_missing_from_player_table"] = len(missing_ids)

    s2_lg = defaultdict(set)
    s2_seasons = defaultdict(list)
    for r in s2_season:
        pid = r["player_id"]
        s2_lg[pid].add(r["lg"])
        s2_seasons[pid].append(int(r["season"]))
    nba_baa = {p for p, l in s2_lg.items() if l & {"NBA", "BAA"}}
    aba_only = {p for p, l in s2_lg.items() if l == {"ABA"}}
    dual = nba_baa & (set(s2_lg) - nba_baa - aba_only)
    dual = {p for p in nba_baa if s2_lg[p] - {"NBA", "BAA"}}
    s2_dob = {r["player_id"]: s1_dob_norm(r["birth_date"]) for r in s2_career}
    s2_cname = {r["player_id"]: r["player"] for r in s2_career}
    s2_rowcount = Counter(r["player_id"] for r in s2_season if r["lg"] in ("NBA", "BAA"))
    s2_mset = set()
    s2_membership_lg = {}
    for r in s2_season:
        if r["lg"] not in ("NBA", "BAA"):
            continue
        if re.fullmatch(r"\dTM", r["team"]) or r["team"] == "TOT":
            continue
        membership = (int(r["season"]), r["player_id"], r["team"])
        s2_mset.add(membership)
        s2_membership_lg[membership] = r["lg"]
    R["s2_season_rows"] = len(s2_season)
    R["s2_nba_baa_rows"] = sum(1 for r in s2_season if r["lg"] in ("NBA", "BAA"))
    R["s2_aba_rows"] = sum(1 for r in s2_season if r["lg"] == "ABA")
    R["s2_nba_baa_players"] = len(nba_baa)
    R["s2_aba_only_players"] = len(aba_only)
    R["s2_dual_players"] = len(dual)
    for lg in ("BAA", "NBA", "ABA"):
        R["s2_rows_lg_" + lg] = sum(1 for r in s2_season if r["lg"] == lg)

    # ---- S3 --------------------------------------------------------------
    s3_by_id = {r["PERSON_ID"]: r for r in s3}
    s3_ids = set(s3_by_id)
    s3_dob = {p: (r["BIRTHDATE"] or "")[:10] for p, r in s3_by_id.items()}
    s3_from = {p: r["FROM_YEAR"] for p, r in s3_by_id.items()}

    # ---- franchise machinery ----------------------------------------------
    era_idx = franchise_era_index()
    franchise_leagues_by_season = defaultdict(set)
    for (lg, season), entries in era_idx.items():
        for cid, _era in entries:
            franchise_leagues_by_season[(season, cid)].add(lg)
    canon_of_s2row = {}
    unresolved_f1, ambiguous_f1 = [], []
    for r in s2_teams:
        key = (int(r["season"]), r["lg"], r["team"], r["abbreviation"])
        cands = resolve_s2_row(era_idx, *key)
        if len(cands) == 1:
            canon_of_s2row[key] = next(iter(cands))
        elif len(cands) > 1:
            ambiguous_f1.append((key, sorted(cands)))
        else:
            unresolved_f1.append(key)
    R["s2_team_rows"], R["s2_team_names"], R["s2_team_abbrs"] = (
        len(s2_teams), len({r["team"] for r in s2_teams}), len({r["abbreviation"] for r in s2_teams}))
    R["f1_unresolved"], R["f1_ambiguous"] = len(unresolved_f1), len(ambiguous_f1)
    # S2 membership canonicalization for cross-source membership diff
    canon_s2_cache = {}
    for (season, abbr) in {(s, t) for s, p, t in s2_mset}:
        for r in s2_teams:
            if int(r["season"]) == season and r["abbreviation"] == abbr:
                canon_s2_cache[(season, abbr)] = canon_of_s2row.get(
                    (season, r["lg"], r["team"], abbr))

    # ---- player bridge S1 -> S2 --------------------------------------------
    print("bridging S1 <-> S2 ...", flush=True)
    s2_spans = {p: (min(sea), max(sea)) for p, sea in s2_seasons.items()}
    bridge = Bridge(s2_career, s2_season, aba_only, s2_spans=s2_spans)
    s1_name_forms = {}
    for pid, name in s1["players"].items():
        forms = set(name_variants(name))
        c = s1["cpi"].get(pid)
        if c:
            forms |= name_variants(c["name"]) | name_variants(c["name_lcf"])
        s3r = s3_by_id.get(pid)
        if s3r:
            forms |= name_variants(s3r["DISPLAY_FIRST_LAST"])
        s1_name_forms[pid] = forms
    for pid, nm in pbp_only_names.items():
        forms = set(name_variants(nm)) if nm else set()
        s3r = s3_by_id.get(pid)
        if s3r:
            forms |= name_variants(s3r["DISPLAY_FIRST_LAST"])
        s1_name_forms[pid] = forms

    s1_dob = {}
    s1_span = {}
    for pid in s1["players"]:
        c = s1["cpi"].get(pid)
        if c and c["dob"]:
            s1_dob[pid] = c["dob"]
        else:
            s1_dob[pid] = s3_dob.get(pid) or None
        fy = c.get("from_year") if c else None
        ty = c.get("to_year") if c else None
        if s3_by_id.get(pid, {}).get("FROM_YEAR") not in (None, ""):
            fy = fy or s3_by_id[pid].get("FROM_YEAR")
        if s3_by_id.get(pid, {}).get("TO_YEAR") not in (None, ""):
            ty = ty or s3_by_id[pid].get("TO_YEAR")
        try:
            s1_span[pid] = (int(float(fy)) + 1, int(float(ty)) + 1)
        except (TypeError, ValueError):
            pass
    for pid in pbp_only_names:
        s1_dob.setdefault(pid, s3_dob.get(pid) or None)

    s1_to_s2 = {}
    for pid in sorted(set(s1["players"]) | set(pbp_only_names), key=int):
        forms = set(s1_name_forms[pid])
        cls, s2pid, detail = bridge.match(forms, s1_dob.get(pid), s1_span=s1_span.get(pid))
        if cls == "no-match":
            apid, aform = bridge.match_aba(forms, s1_dob.get(pid))
            cls_aba = bool(apid) and apid or None
        else:
            cls_aba = None
        display = s1["players"].get(pid) or pbp_only_names.get(pid) or ""
        if cls != "no-match":
            s1_to_s2[pid] = (display, cls, s2pid)
            classes[cls].append((display, pid, s2pid, s2_cname.get(s2pid, ""), detail))
        elif cls_aba:
            s1_to_s2[pid] = (display, "matches-ABA-only-player", cls_aba)
            classes["matches-ABA-only-player"].append(
                (display, pid, cls_aba, s2_cname.get(cls_aba, ""), ""))
        else:
            classes["no-match"].append((display, pid, "", "", ""))
    # curated aliases
    alias_by_name = {canon(k): v for k, v in PLAYER_ALIASES_S1_TO_S2.items()}
    for pid, name in pbp_only_names.items():
        if pid in s1_to_s2 or not name:
            continue
        tgt = alias_by_name.get(canon(name))
        if tgt and tgt in nba_baa:
            s1_to_s2[pid] = (name, "curated-alias-pbp-only", tgt)
            classes["curated-alias"].append((name, pid, tgt, s2_cname.get(tgt, ""), ""))

    R["s1_players"] = len(s1["players"])
    for k, v in classes.items():
        R["s1_" + re.sub(r"[^a-z0-9]+", "-", k).strip("-")] = len(v)
    s1_matched = {p for p, (_, cls, _) in s1_to_s2.items() if cls in ("name+dob", "name-match-s2-dob-NA", "name-only", "lastname+dob", "initial-surname+dob-window", "curated-alias", "surname-fuzzy+dob-window", "lastname-s2dob-NA", "surname+career-span", "initial+surname", "unique-surname", "DOB-conflict")}

    # ---- S1-only subclassification ------------------------------------------
    no_match_sub = {}   # s1 pid -> subclass of the no-match class
    pbp_pids = {str(p) for _, p, _ in m1_rows}
    inact_pids = {str(r[0]) for r in con.execute("select distinct player_id from inactive_players")}
    for row in list(classes["no-match"]):
        name, pid, _, _, _ = row
        c = s1["cpi"].get(pid, {})
        fy = c.get("from_year") or s3_by_id.get(pid, {}).get("FROM_YEAR")
        if pid in pbp_pids:
            sub = "S1-only-has-play-by-play"
        elif pid in inact_pids:
            sub = "S1-only-inactive-list-only"
        elif fy and int(float(fy)) >= 2024:
            sub = "S1-only-debut-2024-plus"
        else:
            sub = "S1-only-no-pbp-evidence"
        s1_to_s2[pid] = (name, sub, None)
        no_match_sub[pid] = sub

    # S2 season metadata misses postseason-only players (e.g. Luca Vildoza).
    # Require an actual scoring/shot/rebound/turnover event in an official game;
    # a bench technical or roster/substitution reference alone is insufficient.
    qualifying = []
    for name, pid, _, _, _ in classes["no-match"]:
        if not name or no_match_sub[pid] != "S1-only-has-play-by-play":
            continue
        for season, abbr in con.execute(
            "select distinct substr(g.season_id,2,4)+1, p.player1_team_abbreviation "
            "from play_by_play p join game g using(game_id) where p.player1_id=? "
            "and p.eventmsgtype in (1,2,3,4,5) "
            "and g.season_type in ('Regular Season','Playoffs')", (int(pid),)):
            if resolve_s1_abbr(era_idx, int(season), abbr):
                qualifying.append((int(season), pid, abbr))
    supplements = s1_official_supplements(s1_to_s2, qualifying)
    for pid, (key, name, seasons) in supplements.items():
        nba_baa.add(key)
        s2_cname[key] = name
        s2_dob[key] = s1_dob.get(pid)
        s2_lg[key] = {"NBA"}
        s2_seasons[key] = list(seasons)
        s1_to_s2[pid] = (name, "S1-official-appearance", key)
        classes["S1-official-appearance"].append(
            (name, pid, key, name, "Official RS/PO appearance; no S2 identity or season row"))
    classes["no-match"] = [r for r in classes["no-match"] if r[1] not in supplements]
    R["s1_no-match"] = len(classes["no-match"])
    R["s1_official_supplements"] = len(supplements)

    # ---- universe composition ----------------------------------------------
    universe = sorted(nba_baa)
    R["universe_size"] = len(universe)
    # S2-side: which universe players bridge to S1?
    s2_to_s1 = {}
    for s1p, (name, cls, s2p) in s1_to_s2.items():
        if s2p:
            s2_to_s1.setdefault(s2p, []).append((s1p, cls))
    # S1 people who are members of the canonical universe (bridged to an NBA/BAA
    # S2 player): counts by outcome for the report
    universe_membership = Counter()
    for s1p, (name, cls, s2p) in s1_to_s2.items():
        if s2p and s2p in nba_baa:
            universe_membership["in-universe"] += 1
        elif s2p:  # ABA-only target
            universe_membership["aba-only-target"] += 1
        else:
            universe_membership["no-s2-identity"] += 1
    R["s1_universe_members"] = universe_membership["in-universe"]
    R["s1_aba_only_targets"] = universe_membership["aba-only-target"]
    R["s1_no_s2_identity"] = universe_membership["no-s2-identity"]
    uni_no_s1 = [p for p in universe if p not in s2_to_s1]
    uni_no_s1_post2023 = [p for p in uni_no_s1 if min(s2_seasons[p]) >= 2024]
    uni_no_s1_pre2023 = [p for p in uni_no_s1 if min(s2_seasons[p]) < 2024]
    R["uni_no_s1"] = len(uni_no_s1)
    R["uni_no_s1_post2023"] = len(uni_no_s1_post2023)
    R["uni_no_s1_pre2023"] = len(uni_no_s1_pre2023)
    # S3-side
    s3_nba_baa = set()
    s3_aba = set()
    s3_nos2 = set()
    for p in s3_ids:
        cands = bridge.idx_name.get((canon(s3_by_id[p]["DISPLAY_FIRST_LAST"])), set())
        # name-only resolution against S2 names
        forms = name_variants(s3_by_id[p]["DISPLAY_FIRST_LAST"])
        cands = set()
        for f in forms:
            cands |= bridge.idx_name.get(f, set())
        if not cands:
            s3_nos2.add(p)
            apid, _ = bridge.match_aba(forms, s3_dob.get(p))
            if apid:
                s3_aba.add(p)
            continue
        if cands & nba_baa:
            s3_nba_baa.add(p)
    R["s3_players"] = len(s3_ids)
    R["s3_in_s1"] = len(s3_ids & set(s1["players"]))
    R["s3_only"] = len(s3_ids - set(s1["players"]))
    R["s3_only_nba_baa_per_s2"] = len((s3_ids - set(s1["players"])) & s3_nba_baa)
    R["s3_only_aba_per_s2"] = len(s3_aba & (s3_ids - set(s1["players"])))
    R["s3_only_no_s2"] = len(s3_nos2 & (s3_ids - set(s1["players"])))
    R["s3_no_s2_at_all"] = len(s3_nos2)
    # S1-only per S3
    R["s1_only_vs_s3"] = len(set(s1["players"]) - s3_ids)

    # ---- membership reconciliation S1 pbp vs S2 ----------------------------
    print("reconciling membership ...", flush=True)
    canon_of_s1_row = {}
    unres_s1_abbr = Counter()
    for ses, pid, abbr in m1_rows:
        if abbr in S1_EXCLUDED_ABBREVIATIONS:
            unres_s1_abbr["excluded-" + abbr] += 1
            continue
        cands = resolve_s1_abbr(era_idx, ses, abbr)
        if len(cands) == 1:
            canon_of_s1_row[(ses, pid, abbr)] = next(iter(cands))
        else:
            unres_s1_abbr[abbr] += 1
    R["m1_rows_total"] = len(m1_rows)
    R["m1_rows_excluded_nonfranchise"] = sum(v for k, v in unres_s1_abbr.items() if k.startswith("excluded-"))
    R["m1_rows_unresolved_abbr"] = sum(v for k, v in unres_s1_abbr.items() if not k.startswith("excluded-"))
    m1_canon = set()
    # Identity classes whose bridge is accepted for membership canonicalization
    # (DOB-conflict identities are accepted-but-flagged; matches-ABA-only-player
    # are excluded because their S2 ids live outside the NBA/BAA universe).
    bridged_classes = {"name+dob", "lastname+dob", "name-match-s2-dob-NA", "name-only",
                       "curated-alias", "DOB-conflict", "initial-surname+dob-window",
                       "surname-fuzzy+dob-window", "lastname-s2dob-NA", "surname+career-span",
                       "initial+surname", "unique-surname", "S1-official-appearance"}
    bridged_for_membership = {p for p, (_n, cls, _s) in s1_to_s2.items()
                              if cls in bridged_classes and _s is not None}
    s1_side = defaultdict(set)   # (season, s2pid) -> canon franchise ids seen in S1 pbp
    for (ses, pid, abbr), cid in canon_of_s1_row.items():
        s1_side[(ses, s1_to_s2[pid][2])].add(cid)
        if pid in bridged_for_membership:
            m1_canon.add((ses, s1_to_s2[pid][2], cid))
    untrans = [(s, p, a) for (s, p, a) in canon_of_s1_row if p not in bridged_for_membership]
    R["m1_rows_player_unbridged"] = len({p for _, p, _ in untrans})
    R["m1_rows_player_unbridged_named"] = len(untrans)
    # S2 membership rows canonicalized to franchise ids
    s2_mset_canon = set()
    s2_mset_canon_lg = {}
    unresolved_s2_team_keys = Counter()
    for s, p, t in s2_mset:
        cid = canon_s2_cache.get((s, t))
        if cid:
            key = (s, p, cid)
            s2_mset_canon.add(key)
            s2_mset_canon_lg[key] = s2_membership_lg[(s, p, t)]
        else:
            unresolved_s2_team_keys[t] += 1
    R["m2_unresolved_s2_team_rows"] = sum(unresolved_s2_team_keys.values())
    R["m2_unresolved_s2_team_keys"] = len(unresolved_s2_team_keys)
    m1a = s2_mset_canon - m1_canon
    m1a_lte2023 = {(s, p, c) for (s, p, c) in m1a if s <= 2023}
    m1a_gt2023 = m1a - m1a_lte2023
    R["m1a_rows_s2_only"] = len(m1a)
    R["m1a_rows_s2_only_season_le_2023"] = len(m1a_lte2023)
    R["m1a_rows_s2_only_season_gt_2023"] = len(m1a_gt2023)
    m2 = m1_canon - s2_mset_canon
    R["m2_rows_s1_only"] = len(m2)
    diff_shape = {}  # (season, bbr_pid, canon_id) -> reason S1 lacks the S2 row

    # ---- stratified samples --------------------------------------------------
    samples = defaultdict(list)
    # shape classification for every S2-only membership row (used in the CSV and
    # the report): why S1 lacks the matching row
    for (season, pid, team) in sorted(m1a_lte2023):
        s1t = s1_side.get((season, pid), set())
        if season < 1997:
            shape = "S1-no-per-player-evidence-pre-1997"
        elif not s1t:
            shape = "S1-pbp-no-event-for-player"
        elif team in s1t:
            shape = "S1-pbp-shows-team-rowset-artifact"
        else:
            shape = "S1-pbp-shows-other-teams-same-season"
        diff_shape[(season, pid, team)] = shape
    for (season, pid, team) in sorted(m1a_gt2023):
        diff_shape[(season, pid, team)] = "S1-ends-2022-23"
    ERAS = [("BAA 1946-49", lambda lg, s: lg == "BAA"),
            ("1950-66", lambda lg, s: lg == "NBA" and 1950 <= s <= 1966),
            ("1967-80", lambda lg, s: lg == "NBA" and 1967 <= s <= 1980),
            ("1981-99", lambda lg, s: lg == "NBA" and 1981 <= s <= 1999),
            ("2000-2025/26", lambda lg, s: lg == "NBA" and 2000 <= s <= 2026)]
    def era_label(lg, season):
        for name, pred in ERAS:
            if pred(lg, season):
                return name
        return None
    samples = defaultdict(list)
    for (season, pid, team) in sorted(m1a_lte2023):
        lg = s2_mset_canon_lg[(season, pid, team)]
        samples[("membership-S2-only-vs-S1", era_label(lg, season))].append((season, pid, team))
    for (season, pid, team) in sorted(m2):
        leagues = franchise_leagues_by_season.get((season, team), set())
        lg = next(iter(leagues)) if len(leagues) == 1 else None
        samples[("membership-S1-only-vs-S2", era_label(lg, season))].append((season, pid, team))
    for row in sorted(classes["no-match"], key=lambda r: r[1]):
        pass
    # era for player classes via S2/cpi years
    def era_for_s2pid(pid):
        ss = s2_seasons.get(pid)
        if ss:
            return era_label("NBA", min(ss))
        return None
    def era_for_s1pid(pid):
        c = s1["cpi"].get(pid, {})
        try:
            fy = int(float(c.get("from_year") or s3_by_id.get(pid, {}).get("FROM_YEAR") or 0))
        except (TypeError, ValueError):
            return None
        return era_label("NBA", fy + 1)
    for name, pid, s2p, cnm, det in classes["no-match"]:
        samples[("S1-player-no-S2-match", era_for_s1pid(pid))].append((name, pid))
    for pid in uni_no_s1_pre2023:
        samples[("S2-player-no-S1-match-pre-2023", era_for_s2pid(pid))].append((s2_cname[pid], pid))
    for pid in uni_no_s1_post2023:
        samples[("S2-player-no-S1-match-2024-plus", era_for_s2pid(pid))].append((s2_cname[pid], pid))
    for name, pid, s2p, cnm, det in classes["DOB-conflict"]:
        samples[("DOB-conflict", era_for_s1pid(pid))].append((name, pid, cnm))
    for p in sorted((s3_ids - set(s1["players"])) & s3_nba_baa):
        samples[("S3-only-but-NBA-per-S2", era_label("NBA", int(float(s3_from[p])) + 1 if s3_from[p] not in (None, "") else 0))].append((s3_by_id[p]["DISPLAY_FIRST_LAST"], p))
    for p in sorted(s3_aba & (s3_ids - set(s1["players"]))):
        samples[("S3-only-ABA-per-S2", None)].append((s3_by_id[p]["DISPLAY_FIRST_LAST"], p))
    for p in sorted(s3_nos2):
        samples[("S3-only-no-S2-row", None)].append((s3_by_id[p]["DISPLAY_FIRST_LAST"], p))

    # ---- S2 <-> S3 person-id bridge (slug -> NBA API id) -----------------------
    # S3 is NBA-API names+ids (like S1). S2 slugs bridge to S3 ids through the
    # S1 bridge (S1/S3 share the NBA-API namespace) or by name(+dob) matching.
    s2_names_formcache = {p: sorted(bridge.s2_names[p]) for p in universe}
    s3_names_byform = defaultdict(set)
    for s3p, r in s3_by_id.items():
        for f in name_variants(r["DISPLAY_FIRST_LAST"]):
            s3_names_byform[f].add(s3p)
    s2_to_s3 = {}
    s3_ambiguous = []
    for p in universe:
        forms = set(name_variants(s2_cname[p]))
        for f in s2_names_formcache[p]:
            forms |= name_variants(f)
        dob = s2_dob.get(p)
        hit_s1 = s2_to_s1.get(p)
        cand_s1p = sorted([s1p for s1p, _ in (hit_s1 or []) if s1p in s3_by_id], key=int)
        # direct (S2 name form) -> S3 id, with DOB checks
        cands = set()
        for f in forms:
            cands |= s3_names_byform.get(f, set())
        # S1/S3 share the NBA-API namespace: accept the S1 id directly when bridged
        if cand_s1p and cand_s1p[0] in s3_by_id:
            s2_to_s3[p] = (cand_s1p[0], "via-S1-bridge")
            continue
        if len(cands) == 1:
            s3p = next(iter(cands))
            d3 = s3_dob.get(s3p)
            if not d3 or not dob or d3 == dob:
                s2_to_s3[p] = (s3p, "name" + ("-+dob" if d3 and dob else "-only"))
            else:
                s2_to_s3[p] = (s3p, "name-DOB-conflict")
        elif len(cands) > 1:
            if dob:
                dc = {s3p for s3p in cands if s3_dob.get(s3p) == dob}
                if len(dc) == 1:
                    s2_to_s3[p] = (next(iter(dc)), "name+dob")
                    continue
            s3_ambiguous.append((p, sorted(cands)))
    R["s2_to_s3_bridged"] = len(s2_to_s3)
    R["s2_to_s3_ambiguous"] = len(s3_ambiguous)
    R["s2_to_s3_methods"] = dict(Counter(v[1] for v in s2_to_s3.values()))

    s2_to_s3_method = dict(Counter(v[1] for v in s2_to_s3.values()))
    for p, s3p in sorted(((p, v[0]) for p, v in s2_to_s3.items()), key=lambda x: x[0]):
        if p in s3_by_id and s3p != p:
            samples[("S2-slug-vs-S3-id-mismatch", None)].append((s2_cname[p], p, s3p))
    for p, cands in s3_ambiguous[:50]:
        samples[("S2-to-S3-ambiguous", None)].append((s2_cname[p], p, ",".join(cands)))
    uni_no_s3 = {p for p in universe if p not in s2_to_s3}
    for p in sorted(uni_no_s3):
        samples[("S2-universe-no-S3-id", era_for_s2pid(p))].append((s2_cname[p], p))

    # ---- CSV outputs ---------------------------------------------------------
    def s3_id_of(p):
        h = s2_to_s3.get(p)
        return h[0] if h else ""
    def s3_method_of(p):
        h = s2_to_s3.get(p)
        return h[1] if h else ""
    def s1_ids_of(p):
        return ";".join(sorted((s1p for s1p, _c in s2_to_s1.get(p, [])), key=lambda x: int(x)))
    def s1_cls_of(p):
        return ";".join(sorted({c for _, c in s2_to_s1.get(p, [])})) or "no-S1-match"
    def s1_names_of(p):
        return ";".join(sorted((s1["players"].get(s1p) or pbp_only_names.get(s1p, "")) for s1p, _c in s2_to_s1.get(p, [])))
    write_csv(
        os.path.join(T3_DIR, "player-universe.csv"),
        ["bbr_player_id", "display_name", "birth_date", "leagues", "first_season", "last_season",
         "nba_baa_season_rows", "aba_only", "s1_player_id", "s1_match_class", "s1_display_name",
         "s3_person_id", "s3_bridge_method", "universe_source"],
        [[p, s2_cname[p], s2_dob.get(p) or "", "|".join(sorted(s2_lg[p])), min(s2_seasons[p]),
          max(s2_seasons[p]), s2_rowcount[p], "N",
          s1_ids_of(p), s1_cls_of(p), s1_names_of(p), s3_id_of(p), s3_method_of(p),
          "S1-official-appearance" if p.startswith("nba:") else "S2-season-statistics"]
         for p in universe])
    write_csv(
        os.path.join(T3_DIR, "unresolved-player-cases.csv"),
        ["source", "class", "display_name", "source_id", "other_id", "other_name", "detail"],
        [[src, no_match_sub.get(row[1], cls) if cls == "no-match" else cls, *row]
         for src, cc in (("S1", classes),) for cls, rows in sorted(cc.items()) for row in sorted(rows, key=lambda r: r[1])]
        + [["S2", "no-S1-match-pre-2023", s2_cname[p], p, "", "", ""] for p in sorted(uni_no_s1_pre2023)]
        + [["S2", "no-S1-match-2024-plus", s2_cname[p], p, "", "", ""] for p in sorted(uni_no_s1_post2023)]
        + [["S3", "S3-only-not-in-S1", s3_by_id[p]["DISPLAY_FIRST_LAST"], p, "", "", ""]
           for p in sorted(s3_ids - set(s1["players"]))]
        + [["S2", "no-S3-id", s2_cname[p], p, "", "", ""] for p in sorted(uni_no_s3)]
        + [["S3", "name-DOB-conflict", s3_by_id[sp]["DISPLAY_FIRST_LAST"], sp, p,
            s2_cname[p], "S3 DOB=" + str(s3_dob.get(sp)) + "; S2 DOB=" + str(s2_dob.get(p))]
           for p, (sp, method) in sorted(s2_to_s3.items()) if method == "name-DOB-conflict"],
    )
    write_csv(
        os.path.join(T3_DIR, "membership-mismatches.csv"),
        ["class", "season", "canonical_franchise", "bbr_player_id", "bbr_name", "s1_player_id", "s1_name", "diff_shape"],
        sorted([["S2-only", s, c, p, s2_cname.get(p, ""), "", "", diff_shape.get((s, p, c), "")]
                for s, p, c in sorted(m1a_lte2023)]
               + [["S2-only", s, c, p, s2_cname.get(p, ""), "", "", "S1-ends-2022-23"]
                  for s, p, c in sorted(m1a_gt2023)]
               + [["S1-only", s, c, p, s2_cname.get(p, ""), sp, (s1["players"].get(sp) or pbp_only_names.get(sp, "")), ""]
                  for s, p, c in sorted(m2)
                  for sp in ([s1p for s1p, (_n, _c, s2p2) in s1_to_s2.items() if s2p2 == p][:1] or [""])]),
    )
    write_csv(
        os.path.join(T3_DIR, "franchise-crosswalk.csv"),
        ["season", "lg", "team_name", "abbreviation", "canonical_id"],
        [[s, lg, t, a, canon_of_s2row.get((s, lg, t, a), "")]
         for (s, lg, t, a) in sorted(canon_of_s2row)]
        + [[k[0], k[1], k[2], k[3], "UNRESOLVED"]
           for k in sorted(unresolved_f1)]
        + [[k[0], k[1], k[2], k[3], "AMBIGUOUS:" + "|".join(sorted(v))]
           for k, v in sorted(ambiguous_f1)],
    )

    # ---- report ---------------------------------------------------------------
    # (assembled below and written to docs/reports/t3-reconciliation.md)
    print("counts:", json.dumps({k: v for k, v in sorted(R.items())}, indent=2), flush=True)
    # stash for the report writer (data/ is gitignored)
    import pickle
    os.makedirs(os.path.join(data_dir, "t3"), exist_ok=True)
    with open(os.path.join(data_dir, "t3", ".state.pkl"), "wb") as f:
        pickle.dump(
            {"R": dict(R), "classes": {k: v for k, v in classes.items()},
             "s1": {k: v for k, v in s1.items() if k in ("players", "cpi", "team", "team_history")},
             "s2_teams": s2_teams, "canon_of_s2row": canon_of_s2row,
             "s2_cname": s2_cname, "s2_dob": s2_dob, "s2_seasons": {p: (min(v), max(v)) for p, v in s2_seasons.items()},
             "universe": universe, "s2_to_s1": s2_to_s1, "s1_to_s2_subset": {p: v for p, v in list(s1_to_s2.items())},
             "uni_no_s1": uni_no_s1, "uni_no_s1_pre2023": uni_no_s1_pre2023,
             "uni_no_s1_post2023": uni_no_s1_post2023,
             "s3_by_id": {p: {k: v for k, v in r.items()} for p, r in s3_by_id.items()},
             "s3_nba_baa": s3_nba_baa, "s3_aba": s3_aba, "s3_nos2": s3_nos2,
             "m1a_lte2023": sorted(m1a_lte2023), "m2": sorted(m2), "m1_events": m1_events,
             "m1_total": len(m1_rows), "unres_s1_abbr": dict(unres_s1_abbr),
             "unresolved_f1": unresolved_f1, "ambiguous_f1": ambiguous_f1,
             "s2_to_s3": s2_to_s3, "s3_ambiguous": s3_ambiguous,
             "diff_shape_counts": dict(Counter(diff_shape.values())),
             "no_match_sub": no_match_sub, "pbp_only_names": pbp_only_names,
             "uni_no_s3": sorted(uni_no_s3), "s1_official_supplements": supplements,
             "canon_counts": dict(Counter(canon_of_s2row.values())),
             "samples": dict(samples), "pbp_pids_size": len(pbp_pids), "inact_pids_size": len(inact_pids),
             "s1_dob_missing": sum(1 for v in s1_dob.values() if not v),
             "era_of_season": {},
             }, f)
    con.close()
    print("phase-1 complete; artifacts under", T3_DIR, flush=True)


if __name__ == "__main__":
    main()