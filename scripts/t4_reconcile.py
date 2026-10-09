#!/usr/bin/env python3
"""T4 — reconstruct roster tenure as dated intervals and classify coverage (ticket #6).

Inputs (all read-only, see docs/data/source-manifest.md):
  S2  data/sumitrodatta/Player Season Info.csv + Team Abbrev.csv
      (player-season-team membership; BBR lineage)
  S1  data/nba.sqlite `game` (season windows: first/last game dates per team-season,
      Regular Season + Playoffs rows; read-only SQLite URI)
  T3  docs/reports/t3/franchise-crosswalk.csv + player-universe.csv (canonical
      franchise identities + the 5,105-player universe; committed artifacts)
      + docs/reports/t3/membership-mismatches.csv (S1-only membership rows S2 omits)
  BBR data/t4/transactions-parsed.pkl (cached transaction-page parse from
      scripts/t4_fetch_bbr.py; legs: player slug + depart/arrive abbr per date)

Outputs (retained under docs/reports/t4/, working state under data/t4/):
  docs/reports/t4/tenures.csv            one row per reconstructed tenure
  docs/reports/t4/unresolved-flags.csv   every flagged for-review tenure
  docs/reports/t4/coverage-counts.json   per-class/per-era counts + pair counts
  docs/reports/t4/examples.csv           examples per evidence class
  data/t4/.state.pkl                     report-writer state
  (report rendered by scripts/t4_report.py -> docs/reports/t4-tenure-coverage.md)

Evidence classes (spec Implementation Decisions, "Coverage and provenance";
GLOSSARY.md "roster tenure"):
  directly-evidenced  both bounds anchored by dated transaction rows
  cross-checked       transaction-anchored bound(s) + independent season-window
                      agreement (S1 stats.nba.com lineage vs BBR lineage)
  inferred            season-window bracket only (membership season + S1 games)
  unresolved          conflicts, fuzzy dates, same-day ordering ambiguity, or
                      no dated evidence — flagged, never invented

Interval conventions (see t4_core.py): half-open [start, end) day numbers; a
dated departure is the FIRST DAY OFF the roster; mid-season moves yield
separate intervals per stint; touching stints never overlap.

Run:  python3 scripts/t4_reconcile.py [data_dir]
"""

import csv
import json
import os
import pickle
import re
import sqlite3
import sys
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)

from t4_core import (  # noqa: E402
    CLASS_CROSS_CHECKED,
    CLASS_DIRECT,
    CLASS_INFERRED,
    CLASS_UNRESOLVED,
    TenureInterval,
    classify_tenure,
    date_of_day_number as day_iso,
    dated_leg_season,
    day_number,
)

S2_DIR_NAME = "sumitrodatta"
REPORT_DIR = os.path.join(REPO, "docs", "reports")
T3_DIR = os.path.join(REPORT_DIR, "t3")
T4_DIR = os.path.join(REPORT_DIR, "t4")

ERAS = [
    ("BAA 1946-49", lambda lg, s: lg == "BAA"),
    ("1950-66", lambda lg, s: lg == "NBA" and 1950 <= s <= 1966),
    ("1967-80", lambda lg, s: lg == "NBA" and 1967 <= s <= 1980),
    ("1981-99", lambda lg, s: lg == "NBA" and 1981 <= s <= 1999),
    ("2000-2025/26", lambda lg, s: lg == "NBA" and 2000 <= s <= 2026),
]


def era_label(lg, season):
    for name, pred in ERAS:
        if pred(lg, season):
            return name
    return None


def read_csv(path):
    with open(path, newline="", encoding="utf-8-sig") as f:
        return list(csv.DictReader(f))


def write_csv(path, header, rows):
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(header)
        w.writerows(rows)


# ------------------------------------------------------------- season logic --
def s1_season_id_to_s2_season(season_id):
    """S1 game.season_id -> S2 convention (season starting year + 1)."""
    return int(str(season_id)[1:5]) + 1


# --------------------------------------------------------- S1 season windows --
# S1 game-table abbreviation quirk T4 resolves locally (counted, not silent):
# S1 spells the San Antonio Spurs 'SAN' in seasons 1977-1996 (830 home + 833
# away rows, verified same team_name 'San Antonio Spurs') while the same table
# uses 'SAS' in seasons 1977-2026 elsewhere; the shared seed knows only 'SAS'.
# Same franchise, one spelling drift -> mapped unconditionally; every applied
# fixup is counted in the returned diagnostics.
S1_ABBR_FIXUP = {
    "SAN": "SAS",   # San Antonio Spurs (S1 spelling drift, seasons 1977-1996)
}


def franchise_era_index():
    """(lg, season) -> list of (canonical_id, era_tuple); same shape as T3's
    `franchise_era_index` (kept local so this module does not import the heavy
    t3_reconcile module for one helper)."""
    from t3_franchise_seed import FRANCHISES
    idx = defaultdict(list)
    for cid, eras in FRANCHISES.items():
        for lg, name, ab, ab_s1, s_start, s_end in eras:
            for ses in range(s_start, s_end + 1):
                idx[(lg, ses)].append((cid, (lg, name, ab, ab_s1, s_start, s_end)))
    return idx


def load_s1_season_windows(data_dir):
    """(canonical_franchise, s2_season) -> (window_start_day, window_end_day_exclusive).

    Per S1 game row (Regular Season + Playoffs only) take team first/last game
    dates; S1 team abbreviations resolve to canonical franchises through the
    (league, season) franchise era index over the shared T3 seed.
    Returns (windows, s1_abbr_diag, s1_excluded, n_team_seasons): windowless
    abbreviations are counted in the diagnostics, not silently dropped.
    """
    from t3_franchise_seed import S1_EXCLUDED_ABBREVIATIONS
    from t3_franchise_seed import FRANCHISES as _FR

    era_idx = franchise_era_index()

    def resolve_s1_abbr(season, abbr):
        cands = set()
        for lg in ("NBA", "BAA"):
            for cid, era in era_idx.get((lg, season), []):
                if era[3] == abbr:
                    cands.add(cid)
        return cands

    path = os.path.join(data_dir, "nba.sqlite")
    con = sqlite3.connect("file:%s?mode=ro" % path, uri=True)
    cur = con.cursor()
    per_ts = {}   # (s1_abbr, season) -> [min_day, max_day]
    excluded = Counter()
    diag_bad = []
    for abbr, ses, gd in cur.execute(
        "select team_abbreviation_home, substr(season_id,2,4)+1, game_date from game "
        "where season_type in ('Regular Season','Playoffs') "
        "union all "
        "select team_abbreviation_away, substr(season_id,2,4)+1, game_date from game "
        "where season_type in ('Regular Season','Playoffs')"
    ):
        abbr = abbr.strip()
        if abbr in S1_EXCLUDED_ABBREVIATIONS:
            excluded[abbr] += 1
            continue
        s2s = int(ses)
        # game_date like '1946-11-01 00:00:00'
        m = re.match(r"^(\d{4}-\d{2}-\d{2})", gd)
        if not m:
            diag_bad.append((abbr, ses, gd))
            continue
        day = day_number(m.group(1))
        lo_hi = per_ts.get((abbr, s2s))
        if lo_hi is None:
            per_ts[(abbr, s2s)] = [day, day]
        else:
            if day < lo_hi[0]:
                lo_hi[0] = day
            if day > lo_hi[1]:
                lo_hi[1] = day
    con.close()
    windows = {}
    s1_abbr_diag = Counter()
    if diag_bad:
        s1_abbr_diag[("<bad-date>", "", len(diag_bad))] = len(diag_bad)
    for (abbr, s2s), (lo, hi) in per_ts.items():
        fixed = S1_ABBR_FIXUP.get(abbr)
        if fixed:
            abbr = fixed
            s1_abbr_diag[("fixed:SAN->SAS", s2s, 1)] = 1  # counted, not silent
        cands = resolve_s1_abbr(s2s, abbr)
        if len(cands) != 1:
            s1_abbr_diag[(abbr, s2s, len(cands))] += 1
            continue
        cid = next(iter(cands))
        windows[(cid, s2s)] = (lo, hi + 1)   # last game day included; end exclusive
    return windows, s1_abbr_diag, excluded, len(per_ts)


# --------------------------------------------------- franchise resolution ----
def load_franchise_lookup():
    """(season, s2_abbr) -> canonical id, from the committed T3 crosswalk."""
    canon = {}
    ambiguous = Counter()
    for r in read_csv(os.path.join(T3_DIR, "franchise-crosswalk.csv")):
        key = (int(r["season"]), r["abbreviation"])
        cid = r["canonical_id"]
        if not cid or cid.startswith("AMBIGUOUS:") or cid == "UNRESOLVED":
            ambiguous[key] += 1
            continue
        if key in canon and canon[key] != cid:
            ambiguous[key] += 1
        canon[key] = cid
    return canon, ambiguous


def resolve_event_franchise(era_idx, league, season, abbr):
    """BBR page team abbr (data-attr from/to) -> canonical franchise id or None."""
    if not abbr:
        return None
    cands = set()
    for cid, era in era_idx.get((league, season), []):
        if era[2] == abbr or era[3] == abbr:
            cands.add(cid)
    if len(cands) == 1:
        return next(iter(cands))
    return None


# ------------------------------------------------------------- membership ----
def load_membership(data_dir):
    """Membership rows (S2 base + S1-only supplements), canonical franchise ids.

    Returns (rows, diagnostics): rows are dicts with keys
      season (int, S2 convention), lg, bbr_player_id, display_name, franchise
      (canonical id), membership_source ('S2'|'S1-only-supplement'), era label.
    """
    canon_of_key, ambiguous = load_franchise_lookup()
    universe_rows = read_csv(os.path.join(T3_DIR, "player-universe.csv"))
    universe = {r["bbr_player_id"] for r in universe_rows}
    names = {r["bbr_player_id"]: r["display_name"] for r in universe_rows}
    rows = []
    diag = Counter()
    s2_season = read_csv(os.path.join(data_dir, S2_DIR_NAME, "Player Season Info.csv"))
    seen = set()
    for r in s2_season:
        lg = r["lg"]
        if lg not in ("NBA", "BAA"):
            diag["row-aba-excluded"] += 1
            continue
        team = r["team"]
        if re.fullmatch(r"\dTM", team or "") or (team or "") == "TOT":
            diag["row-summary-excluded"] += 1
            continue
        pid = r["player_id"]
        if pid not in universe:
            diag["row-player-outside-universe"] += 1
            continue
        season = int(r["season"])
        cid = canon_of_key.get((season, team))
        if cid is None:
            diag["row-franchise-unresolvable"] += 1
            diag["row-franchise-unresolvable-abbr:" + team] += 1
            continue
        key = (season, pid, cid)
        if key in seen:
            diag["row-duplicate"] += 1
            continue
        seen.add(key)
        rows.append({
            "season": season, "lg": lg, "bbr_player_id": pid,
            "display_name": names.get(pid, ""), "franchise": cid,
            "membership_source": "S2", "era": era_label(lg, season),
        })
    # S1-only supplements: S1 play-by-play evidences a (season, player, franchise)
    # membership S2 omits (T3 §5, the 12 S1-only rows). T4 must not rely on S2
    # completeness for late-season moves, so these rows enter reconstruction
    # explicitly, marked with their source.
    for r in read_csv(os.path.join(T3_DIR, "membership-mismatches.csv")):
        if r["class"] != "S1-only":
            continue
        season = int(r["season"])
        pid = r["bbr_player_id"]
        cid = r["canonical_franchise"]
        if pid not in universe:
            diag["supplement-player-outside-universe"] += 1
            continue
        if not cid or cid == "UNRESOLVED" or cid.startswith("AMBIGUOUS:"):
            diag["supplement-franchise-unresolvable"] += 1
            continue
        key = (season, pid, cid)
        if key in seen:
            diag["supplement-duplicate"] += 1
            continue
        seen.add(key)
        rows.append({
            "season": season, "lg": "NBA", "bbr_player_id": pid,
            "display_name": names.get(pid, pid), "franchise": cid,
            "membership_source": "S1-only-supplement",
            "era": era_label("NBA", season),
        })
    diag["membership_rows"] = len(rows)
    diag["rows_S2"] = sum(1 for r in rows if r["membership_source"] == "S2")
    diag["rows_S1_only_supplement"] = sum(
        1 for r in rows if r["membership_source"] == "S1-only-supplement")
    return rows, diag, ambiguous


# ------------------------------------------------------- transaction legs ----
def leg_season_bucket(date_iso, page_season, league, page_franchises=None,
                      windows=None):
    """Season bucket a precise-dated leg belongs to, checked against S1 games.

    Ordinarily the precise date determines the BBR/S2 league-year bucket (the
    label flips July 1). If the page label differs but the date falls inside
    the S1 game window for one of that row's page-season franchises, independent
    game evidence confirms the page season instead. This matters for exceptional
    schedule extensions (e.g. the 2019-20 NBA bubble through August 2020). The
    caller must still have parsed page bounds; absent bounds are ambiguous.
    """
    own_season = dated_leg_season(date_iso)
    if own_season != page_season and windows and page_franchises:
        day = day_number(date_iso)
        for fid in page_franchises:
            window = windows.get((fid, page_season))
            if window and window[0] <= day < window[1]:
                return page_season
    return own_season


def load_transaction_legs(data_dir, windows=None):
    """Dated movement legs from the cached BBR parse (precise dates only).

    Returns (legs, fuzzy_events, diag, parsed_rows_1950): legs are dicts
      {page, season, league, li_index, date_text, date_iso, event_class, slug,
       player_name, depart_abbr, arrive_abbr, depart, arrive, leg_flags}
    where depart/arrive are canonical franchise ids resolved with the leg
    bucket's (league, season); '' marks no side; a team that does not resolve
    marks the leg with `team-unresolved` (row retained, not usable as an
    anchor).

    SEASON-BUCKET GUARD (the documented BBR page-repetition leak): a leg is
    bucketed to the season its own precise date determines, NOT the page it
    was parsed from. If that differs from the page season (`season` is the
    date-derived bucket; `page_season` retains the source page label), the leg
    is re-bucketed — for the Perkins trade dated 2011-02-24 the NBA_2012
    page's copy lands in the 2011 bucket alongside the NBA_2011 page's copy,
    where extract_anchors' set semantics collapse the identical date into one
    anchor instead of fabricating overlapping THUNDER stints across 2011/2012.
    Franchise resolution uses the BUCKET season ((league, bucket_season) era
    index), so a first-page-of-July leg resolves against the right league era.
    The page's parsed bounds are required: a leg that does not fit its page
    window is normally re-bucketed to its own precise date season and carries
    a non-blocking `season-page-rebucketed` note. If the S1 game window for a
    page-season franchise contains the date, that independent game evidence
    confirms the page season (e.g. the 2019-20 bubble); this is recorded as a
    non-blocking `season-s1-window-confirmed` note. If the window is missing or
    malformed, or the date is outside the page window and neither date-season
    nor S1-window attribution is usable, the leg is flagged/excluded as
    ambiguous. Fuzzy/undated rows NEVER produce legs (never guessed to a date); they are retained as fuzzy_events and only
    flag the tenures they bear on.
    """
    src_pkl = os.path.join(data_dir, "t4", "transactions-parsed.pkl")
    with open(src_pkl, "rb") as f:
        blob = pickle.load(f)
    parse_rows = blob["rows"]
    era_idx = franchise_era_index()
    legs, fuzzy_events = [], []
    diag = Counter()
    parsed_rows_1950 = {}   # per 1946-50 page: total/fuzzy row counts
    for r in parse_rows:
        if r["page"].startswith(("BAA_", "NBA_1950")):
            counts = parsed_rows_1950.setdefault(
                r["page"], {"total": 0, "fuzzy_or_undated": 0})
            counts["total"] += 1
            if r["date_precision"] != "precise":
                counts["fuzzy_or_undated"] += 1
        if r["date_precision"] != "precise":
            if r["event_class"] not in ("coach-hire", "coach-fire", "coach-other",
                                        "other", "no-segment"):
                fuzzy_events.append({
                    "page": r["page"], "season": r["season"],
                    "date_text": r["date_text"], "li_index": r["li_index"],
                    "event_class": r["event_class"],
                    "player_slugs": r["player_slugs"], "team_abbrs": r["team_abbrs"],
                    "text": r["text"],
                })
            diag["row-fuzzy-or-undated"] += 1
            continue
        legs_from_row = json.loads(r["legs_json"]) if r.get("legs_json") else []
        if not legs_from_row:
            diag["row-no-legs"] += 1
            continue
        diag["row-with-legs"] += 1
        wf, wt = r["page_window_from"], r["page_window_to"]
        window_valid = (wf and wt and len(r["date_iso"]) == 10
                        and len(wf) == 10 and len(wt) == 10)
        in_page_window = bool(window_valid and wf <= r["date_iso"] <= wt)
        for l in legs_from_row:
            # First resolve page-season teams so an exceptional S1 window
            # extension can disambiguate a date that falls beyond June 30.
            page_franchises = set()
            if l["depart"]:
                c = resolve_event_franchise(era_idx, r["league"], r["season"],
                                            l["depart"])
                if c:
                    page_franchises.add(c)
            if l["arrive"]:
                c = resolve_event_franchise(era_idx, r["league"], r["season"],
                                            l["arrive"])
                if c:
                    page_franchises.add(c)
            bucket = leg_season_bucket(r["date_iso"], r["season"], r["league"],
                                      page_franchises, windows)
            page_bucket_flip = bucket != r["season"]
            s1_confirms_page_season = (
                dated_leg_season(r["date_iso"]) != r["season"]
                and bucket == r["season"])
            # franchise resolution against the selected leg season bucket
            depart_c = resolve_event_franchise(era_idx, r["league"], bucket,
                                               l["depart"]) if l["depart"] else ""
            arrive_c = resolve_event_franchise(era_idx, r["league"], bucket,
                                               l["arrive"]) if l["arrive"] else ""
            bad = (l["depart"] and depart_c is None) or \
                  (l["arrive"] and arrive_c is None)
            flags = []
            # conservative review flags — they mark legs needing eyes, never
            # guessed: no team anchor on a movement leg, or a leg whose depart
            # equals arrive (degenerate), or an unresolvable team abbr
            if not l["depart"] and not l["arrive"]:
                flags.append("no-team-anchor-on-leg")
            elif l["depart"] and l["depart"] == l["arrive"]:
                flags.append("depart-equals-arrive")
            if bad:
                flags.append("team-unresolved")
            if not window_valid:
                flags.append("page-window-ambiguous")
            elif page_bucket_flip:
                # page bounds parsed; precise date identifies a different
                # season, and no S1 game window confirms the page season.
                diag["leg-note:season-page-rebucketed"] += 1
            elif s1_confirms_page_season and not in_page_window:
                diag["leg-note:season-s1-window-confirmed"] += 1
            elif not in_page_window:
                # neither page bounds nor the S1 window resolves the apparent
                # same-bucket discrepancy: retain the row, exclude the anchor.
                flags.append("date-outside-page-window")
            legs.append({
                "page": r["page"], "season": bucket, "page_season": r["season"],
                "league": r["league"],
                "bucket_season": bucket,
                "li_index": r["li_index"], "date_text": r["date_text"],
                "date_iso": r["date_iso"], "event_class": r["event_class"],
                "slug": l["slug"], "player_name": l.get("name", ""),
                "depart_abbr": l["depart"], "arrive_abbr": l["arrive"],
                "depart": depart_c or "", "arrive": arrive_c or "",
                "leg_flags": "|".join(flags),
            })
            if bad:
                diag["leg-team-unresolved"] += 1
            for f in flags:
                if f != "team-unresolved":
                    diag["leg-flag:" + f] += 1
    diag["legs"] = len(legs)
    diag["legs_usable"] = len([l for l in legs if not l["leg_flags"]])
    return legs, fuzzy_events, diag, parsed_rows_1950


# ------------------------------------------------------------- stint walk ----
def extract_anchors(legs_for_player_season):
    """(arrival_days_by_franchise, departure_days_by_franchise).

    legs_for_player_season: usable (unflagged) leg rows for one (slug, season).
    A trade/sale leg with depart+arrive opens membership for `arrive` and closes
    it for `depart` on the same date; a degenerate leg (same franchise both
    sides) is ignored. Precise dates only (fuzzy rows are excluded upstream).
    Days are DE-DUPLICATED per (franchise, direction): a repeated identical row
    (source repetition) must not double an anchor — a sign dated twice must not
    make day X both open and close a stint.
    """
    arrs, deps = defaultdict(list), defaultdict(list)
    for l in legs_for_player_season:
        day = day_number(l["date_iso"])
        dep, arr = l["depart"], l["arrive"]
        if dep and arr:
            if dep != arr:
                deps[dep].append(day)
                arrs[arr].append(day)
        elif arr:
            arrs[arr].append(day)
        elif dep:
            deps[dep].append(day)
    return ({f: sorted(set(days)) for f, days in arrs.items()},
            {f: sorted(set(days)) for f, days in deps.items()})


def walk_stints(arr_days, dep_days, window):
    """Turn sorted arrival/departure day lists + season window into stints.

    window: (ws, we) half-open season-window days, or None when unavailable.
    Returns (stints, flags, anomalies, notes) with stints as
    (start_day, end_day_exclusive, start_anchored, end_anchored).

    Conventions (documented in the coverage report):
      arrival day x   -> membership STARTS on x (first day on the roster)
      departure day x -> membership ENDS at x, exclusive: the dated departure is
        the FIRST DAY OFF the roster, so the stint is [.., x). A trade leg
        (depart+arrive the same date) therefore yields two touching,
        NON-overlapping stints — no dual-membership day is ever invented.
      a departure dated BEFORE the season-window start (or before every arrival,
        with no window) is a PRIOR-spell exit (off-season cut/re-sign cycles);
        it does not bracket this season's stint and is recorded as a note.
      a second arrival while a stint is open: the roster spell CONTINUES (BBR
        has no intervening departure row; repeated signings/10-day cycles);
        recorded as a note, not a fork — intervals are never invented.
      same-day arrival+departure for one franchise -> ordering-unresolved flag;
          the open stint closes at x and nothing is opened after it.
      an open stint extends to the window end when a window exists; with no
        window the open stint is not constructible (anomaly), but a complete
        arrival+departure PAIR never needs the window.
    """
    stints = []
    flags = []
    anomalies = []
    notes = []
    open_at = None
    arr_set = set(arr_days)
    dep_set = set(dep_days)
    win_start = window[0] if window else None

    def close(end_day, end_anchored):
        stints.append((open_at, end_day, True, end_anchored))

    for day in sorted(set(arr_days) | set(dep_days)):
        if day in arr_set and day in dep_set:
            flags.append(("same-day-arrival-and-departure", day))
            # ordering unresolved: close any open stint at this day; invent none
            if open_at is not None:
                close(day, True)
                open_at = None
            elif window:
                stints.append((window[0], day, False, True))
            continue
        if day in arr_set:
            if open_at is not None:
                # repeated signing with no intervening departure row: the spell
                # continues under the same stint (no interval invention)
                notes.append(("repeat-signing-continues-open-stint", day))
            else:
                open_at = day
        if day in dep_set:
            if open_at is not None:
                close(day, True)
                open_at = None
            elif day not in arr_set:
                if win_start is not None and day > win_start and not stints:
                    # pre-season membership closed by a dated in-window departure
                    stints.append((win_start, day, False, True))
                else:
                    notes.append(("departure-after-closed-stint(prior-spell-exit,"
                                  "or-source-noise)", day))
    if open_at is not None:
        if window:
            if open_at < window[1]:
                stints.append((open_at, window[1], True, False))
            else:
                # arrival dated after the team's last game of THIS season window:
                # it belongs to the NEXT season (off-season move) and cannot
                # open a stint here — flagged as an anomaly, never stretched
                # over the gap into a fabricated interval.
                anomalies.append(("arrival-after-window-end(next-season-move)", open_at))
        else:
            anomalies.append(("open-stint-without-window", open_at))
    return stints, flags, anomalies, notes


# ---------------------------------------------------------------- pipeline ---
def reconstruct(data_dir):
    print("loading S1 season windows ...", flush=True)
    windows, s1_abbr_diag, s1_excluded, s1_window_team_seasons = \
        load_s1_season_windows(data_dir)
    print("loading membership ...", flush=True)
    membership, diag, amb = load_membership(data_dir)
    print("loading transaction legs ...", flush=True)
    legs, fuzzy_events, leg_diag, parsed_rows_1950 = load_transaction_legs(data_dir, windows)

    # index legs by (slug, season)
    legs_by_player_season = defaultdict(list)
    for l in legs:
        legs_by_player_season[(l["slug"], l["season"])].append(l)
    # fuzzy event rows per (slug, season) for flagging
    fuzzy_by_player_season = defaultdict(list)
    for f in fuzzy_events:
        for slug in (f["player_slugs"] or "").split(","):
            slug = slug.strip()
            if slug:
                fuzzy_by_player_season[(slug, f["season"])].append(f)

    tenures = []
    flags_rows = []
    R = Counter()
    window_hits = Counter()

    by_player_season = defaultdict(list)
    for m in membership:
        by_player_season[(m["bbr_player_id"], m["season"])].append(m)

    for (slug, season), mems in sorted(by_player_season.items()):
        plist = legs_by_player_season.get((slug, season), [])
        usable = [l for l in plist if not l["leg_flags"]]
        flagged = [l for l in plist if l["leg_flags"]]
        arrs, deps = extract_anchors(usable)
        fz = fuzzy_by_player_season.get((slug, season), [])
        fuzzy_touch = bool(fz)

        # multi-franchise same-season ordering: same-day arrivals to two
        # different franchises (while both rows claim the season) cannot be
        # ordered -> flag, do not guess.
        cross_ambiguous = False
        arr_by_day = defaultdict(set)
        for f2, days in arrs.items():
            for dnum in days:
                arr_by_day[dnum].add(f2)
        for dnum, franchises in arr_by_day.items():
            if len(franchises) > 1:
                cross_ambiguous = True

        for m in mems:
            fid = m["franchise"]
            ws_we = windows.get((fid, season))
            window_hits["window-present" if ws_we else "window-missing"] += 1
            arr_days = sorted(set(arrs.get(fid, [])))
            dep_days = sorted(set(deps.get(fid, [])))

            # BRACKET-ONLY BASE STINT (membership row + season window, no dated
            # movements): the player was with (player, franchise, season) per the
            # membership source; the S1 game window dates the season's extent.
            # This is a SEASON-WINDOW BRACKET, reported as inferred — never as
            # proof of simultaneous tenure before/after the window edges.
            if not arr_days and not dep_days and not flagged and not fuzzy_touch:
                if ws_we is not None:
                    lo, hi = ws_we
                    interval = TenureInterval(lo, hi)
                    window_iso = "[%s, %s)" % (day_iso(lo).isoformat(),
                                               day_iso(hi).isoformat())
                    # sibling legs for OTHER franchises this same season prove a
                    # mid-season move; a bracket-only row beside them cannot know
                    # where this stint ends -> order unresolved
                    sibling = bool(arrs or deps)   # other-franchise anchors exist
                    if sibling:
                        reasons = ["multi-team-season(ordering-unresolved):"
                                   " player has dated moves to other franchises"
                                   " this season; bracket-only stint cannot be"
                                   " ordered against them"]
                        tenures.append(_tenure_row(m, slug, season, interval,
                                                   CLASS_UNRESOLVED, reasons,
                                                   arr_days, dep_days, window_iso))
                        flags_rows.append([slug, m["display_name"], season, fid,
                                           m["membership_source"], CLASS_UNRESOLVED,
                                           interval.iso(), reasons[0]])
                    else:
                        tenures.append(_tenure_row(m, slug, season, interval,
                                                   CLASS_INFERRED, [],
                                                   arr_days, dep_days, window_iso))
                    R["tenures"] += 1
                    R["tenures-" + ("unresolved" if sibling else "inferred")] += 1
                    continue
                # no window either: nothing dated at all
                reasons = ["no-dated-evidence(membership-season-only;"
                           " S1-window-missing-or-season-past-2022-23)"]
                tenures.append(_tenure_row(m, slug, season, None, CLASS_UNRESOLVED,
                                           reasons, arr_days, dep_days))
                flags_rows.append([slug, m["display_name"], season, fid,
                                   m["membership_source"], CLASS_UNRESOLVED, "",
                                   reasons[0]])
                R["membership_rows_no_stint"] += 1
                continue

            if cross_ambiguous:
                reasons = ["same-day-arrival-to-multiple-franchises(ordering-unresolved)"]
                if flagged:
                    reasons.append("flagged-transaction-legs(%s)"
                                   % ";".join(sorted({l["leg_flags"] for l in flagged})))
                if fuzzy_touch:
                    reasons.append("fuzzy-dated-rows-bear-on-this-tenure")
                tenures.append(_tenure_row(m, slug, season, None, CLASS_UNRESOLVED,
                                           reasons, arr_days, dep_days))
                flags_rows.append([slug, m["display_name"], season, fid,
                                   m["membership_source"], CLASS_UNRESOLVED, "",
                                   "; ".join(reasons)])
                R["membership_rows_no_stint"] += 1
                continue

            stints, same_day_flags, anomalies, walk_notes = walk_stints(arr_days, dep_days, ws_we)

            if not stints:
                reasons = []
                if not ws_we:
                    reasons.append("no-season-window(S1-game-table-ends-2022-23-"
                                   "or-season-absent-from-S1)")
                for a in anomalies:
                    reasons.append(a[0])
                for n_note in walk_notes:
                    reasons.append(n_note[0])
                if flagged:
                    reasons.append("flagged-transaction-legs(%s)"
                                   % ";".join(sorted({l["leg_flags"] for l in flagged})))
                if fuzzy_touch:
                    reasons.append("fuzzy-dated-rows-bear-on-this-tenure")
                if not arr_days and not dep_days and not flagged and not fuzzy_touch:
                    reasons.append("no-dated-evidence(membership-season-only)")
                tenures.append(_tenure_row(m, slug, season, None, CLASS_UNRESOLVED,
                                           reasons, arr_days, dep_days))
                flags_rows.append([slug, m["display_name"], season, fid,
                                   m["membership_source"], CLASS_UNRESOLVED, "",
                                   "; ".join(r for r in reasons if r)])
                R["membership_rows_no_stint"] += 1
                continue

            # one tenure row per stint (mid-season moves yield separate intervals)
            for (s0, e0, s_anchor, e_anchor) in stints:
                reasons = []
                if same_day_flags:
                    reasons.append("same-day-arrival-and-departure(ordering-flagged)")
                for a in anomalies:
                    if not (a[0] == "open-stint-without-window" and ws_we is not None):
                        reasons.append(a[0])
                for n_note in walk_notes:
                    reasons.append(n_note[0])
                if fuzzy_touch:
                    reasons.append("fuzzy-dated-rows-bear-on-this-tenure(fuzzy-not-guessed)")
                if flagged:
                    reasons.append("flagged-transaction-legs(%s)"
                                   % ";".join(sorted({l["leg_flags"] for l in flagged})))
                interval = None
                try:
                    interval = TenureInterval(s0, e0)
                except ValueError:
                    # a dated bound falls outside the season context the row's
                    # other evidence covers (off-season arrival/departure paired
                    # with a windowless bound) — flagged for review, never guessed
                    reasons.append(
                        "interval-construction-failed(bound-outside-season-context,"
                        "start=%d,end=%d)" % (s0, e0))
                    interval = None
                # repeat-signing is a review NOTE, not an ordering ambiguity:
                # the bounds are dated and ordered (spell continues), so it is
                # recorded in reasons but does not force unresolved
                blocking = [r for r in reasons
                            if not r.startswith("repeat-signing-continues-open-stint")]
                window_iso = ""
                agreement = False
                if ws_we is not None and interval is not None:
                    lo, hi = ws_we
                    window_iso = "[%s, %s)" % (day_iso(lo).isoformat(),
                                               day_iso(hi).isoformat())
                    # The season window brackets in-season play; BBR pages span the
                    # whole league year, so dated bounds a few days outside the
                    # window are normal (off-season cuts, pre-season signings,
                    # Finals). Agreement = a POSITIVE overlap between the stint's
                    # dated bounds and the team's window — the two independent
                    # sources (S1 games, BBR transactions) corroborate the same
                    # membership; stints entirely before/after the window do not
                    # get it (they stand on transactions alone).
                    agreement = max(s0, lo) < min(e0, hi)
                    if not agreement:
                        reasons.append("stint-entirely-outside-team-season-window"
                                       "(stands-on-transactions-alone)")
                        blocking.append(reasons[-1])
                cls = classify_tenure(bool(s_anchor), bool(e_anchor), agreement,
                                      blocking)
                tenures.append(_tenure_row(m, slug, season, interval, cls, reasons,
                                           arr_days, dep_days, window_iso))
                if cls == CLASS_UNRESOLVED:
                    flags_rows.append([slug, m["display_name"], season, fid,
                                       m["membership_source"], cls,
                                       interval.iso() if interval else "",
                                       "; ".join(reasons)])
                R["tenures"] += 1
                R["tenures-" + cls] += 1

    R["membership_rows"] = diag["membership_rows"]
    R["rows_S2"] = diag["rows_S2"]
    R["rows_S1_only_supplement"] = diag["rows_S1_only_supplement"]
    R.update({k: v for k, v in diag.items() if k.startswith("row-") or
              k.startswith("supplement-")})
    R["s1_window_team_seasons"] = s1_window_team_seasons
    R["s1_window_unresolved_abbr_keys"] = len(s1_abbr_diag)
    R["s1_window_san_fixed_rows"] = sum(v for k, v in s1_abbr_diag.items()
                                        if str(k[0]).startswith("fixed:"))
    R["membership_window_missing"] = window_hits["window-missing"]
    R["membership_window_present"] = window_hits["window-present"]

    # era breakdown
    era_counts = defaultdict(Counter)
    for t in tenures:
        era_counts[t["era"]][t["evidence_class"]] += 1
    R["era_classes"] = {e: dict(c) for e, c in era_counts.items()}

    # `parsed_rows_1950` is returned by load_transaction_legs from the same
    # parsed blob; preserve those per-page total/fuzzy row counts for reporting.

    # ---- retained CSVs -------------------------------------------------------
    os.makedirs(T4_DIR, exist_ok=True)
    write_csv(
        os.path.join(T4_DIR, "tenures.csv"),
        ["bbr_player_id", "display_name", "season", "lg", "era", "canonical_franchise",
         "membership_source", "evidence_class", "start_day", "end_day", "interval_iso",
         "start_anchored", "end_anchored", "season_window_iso", "unresolved_reasons",
         "arrival_days", "departure_days"],
        [[t["bbr_player_id"], t["display_name"], t["season"], t["lg"], t["era"] or "",
          t["franchise"], t["membership_source"], t["evidence_class"],
          t["start"] if t["start"] is not None else "",
          t["end"] if t["end"] is not None else "",
          t["interval_iso"],
          1 if t["start_anchored"] else 0, 1 if t["end_anchored"] else 0,
          t["season_window_iso"],
          "; ".join(t["reasons"]),
          ",".join(str(x) for x in t["arrival_days"]),
          ",".join(str(x) for x in t["departure_days"])]
         for t in tenures])
    write_csv(
        os.path.join(T4_DIR, "unresolved-flags.csv"),
        ["bbr_player_id", "display_name", "season", "canonical_franchise",
         "membership_source", "evidence_class", "interval_iso", "reasons"],
        flags_rows)
    write_csv(
        os.path.join(T4_DIR, "examples.csv"),
        ["evidence_class", "bbr_player_id", "display_name", "season", "era",
         "canonical_franchise", "interval_iso", "reasons_or_note"],
        _examples(tenures))
    pairs = _pair_analysis(tenures)
    coverage = {
        "R": dict(R), "era_counts": {e: dict(c) for e, c in era_counts.items()},
        "pair_analysis": pairs,
    }
    with open(os.path.join(T4_DIR, "coverage-counts.json"), "w", encoding="utf-8") as f:
        json.dump(coverage, f, indent=2, sort_keys=True)
    state = {
        "tenures": tenures,
        "flags_rows": flags_rows,
        "R": dict(R),
        "legs": legs,
        "fuzzy_events": fuzzy_events,
        "leg_diag": dict(leg_diag),
        "parsed_rows_1950": parsed_rows_1950,
        "windows": {("%s|%s" % k): v for k, v in windows.items()},
        "membership": membership,
        "era_counts": {e: dict(c) for e, c in era_counts.items()},
        "pair_analysis": pairs,
    }
    with open(os.path.join(data_dir, "t4", ".state.pkl"), "wb") as f:
        pickle.dump(state, f)
    print(json.dumps({k: v for k, v in sorted(R.items()) if not k.startswith("row-")},
                     indent=2, sort_keys=True), flush=True)
    return state, R


def _tenure_row(m, slug, season, interval, cls, reasons, arr_days, dep_days,
                window_iso=""):
    return {
        "bbr_player_id": slug,
        "display_name": m["display_name"] or slug,
        "season": season,
        "lg": m["lg"],
        "era": m["era"],
        "franchise": m["franchise"],
        "membership_source": m["membership_source"],
        "evidence_class": cls,
        "start": interval.start if interval else None,
        "end": interval.end if interval else None,
        "interval_iso": interval.iso() if interval else "",
        "start_anchored": bool(arr_days) and cls in (CLASS_DIRECT, CLASS_CROSS_CHECKED),
        "end_anchored": bool(dep_days) and cls in (CLASS_DIRECT, CLASS_CROSS_CHECKED),
        "season_window_iso": window_iso,
        "reasons": list(reasons),
        "arrival_days": list(arr_days),
        "departure_days": list(dep_days),
    }


def _examples(tenures):
    by_class = defaultdict(list)
    for t in tenures:
        by_class[t["evidence_class"]].append(t)
    out = []
    for cls in (CLASS_DIRECT, CLASS_CROSS_CHECKED, CLASS_INFERRED, CLASS_UNRESOLVED):
        rows = by_class.get(cls, [])
        rows = sorted(rows, key=lambda t: (t["season"], t["bbr_player_id"]))
        pick = rows[:2] + rows[-2:] if len(rows) > 4 else rows
        for t in pick:
            out.append([cls, t["bbr_player_id"], t["display_name"], t["season"],
                        t["era"] or "", t["franchise"], t["interval_iso"],
                        "; ".join(t["reasons"]) or "(clean)"])
    return out


def _pair_analysis(tenures):
    """Potential teammate overlaps: distinct player pairs with positive tenure
    overlap (same franchise-season), bucketed by evidence-class mixture."""
    by_fs = defaultdict(list)
    for t in tenures:
        if t["start"] is None:
            continue
        by_fs[(t["franchise"], t["season"])].append(t)
    pair_evidence = Counter()
    n_pairs = 0
    seen_pairs = set()
    for (fid, season), ten in sorted(by_fs.items()):
        for i in range(len(ten)):
            for j in range(i + 1, len(ten)):
                a, b = ten[i], ten[j]
                if a["bbr_player_id"] == b["bbr_player_id"]:
                    continue
                if a["start"] >= b["end"] or b["start"] >= a["end"]:
                    continue
                key = frozenset((a["bbr_player_id"], b["bbr_player_id"]))
                if key in seen_pairs:
                    continue
                seen_pairs.add(key)
                n_pairs += 1
                classes = {a["evidence_class"], b["evidence_class"]}
                if classes <= {CLASS_DIRECT, CLASS_CROSS_CHECKED}:
                    pair_evidence["both-evidenced"] += 1
                elif CLASS_UNRESOLVED in classes:
                    pair_evidence["involves-unresolved"] += 1
                else:
                    pair_evidence["involves-inferred"] += 1
    return {"distinct_player_pairs_with_positive_overlap": n_pairs,
            "pair_breakdown": dict(pair_evidence)}


def main(argv):
    data_dir = argv[1] if len(argv) > 1 else os.path.join(REPO, "data")
    if not os.path.isdir(data_dir):
        alt = os.path.normpath(os.path.join(REPO, "..", "7-degrees", "data"))
        if os.path.isdir(alt):
            data_dir = alt
    state, R = reconstruct(data_dir)
    print("wrote docs/reports/t4/*.csv + data/t4/.state.pkl", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))