#!/usr/bin/env python3
"""T4 — render the tenure coverage report (ticket #6, second pass).

Reads the analysis state written by t4_reconcile.py (data/t4/.state.pkl), the
fetch summary + request ledger (data/t4/), and renders
docs/reports/t4-tenure-coverage.md.
"""

import csv
import json
import os
import pickle
import sys
from collections import Counter, defaultdict
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)

T4_REPORT_DIR = os.path.join(REPO, "docs", "reports", "t4")
T4_DIR = T4_REPORT_DIR
REPORT = os.path.join(REPO, "docs", "reports", "t4-tenure-coverage.md")

CLASS_ORDER = ["directly-evidenced", "cross-checked", "inferred", "unresolved"]
ERA_ORDER = ["BAA 1946-49", "1950-66", "1967-80", "1981-99", "2000-2025/26"]


def md_table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    for r in rows:
        out.append("| " + " | ".join(str(x) for x in r) + " |")
    return "\n".join(out)


def pct(n, d):
    return "%.1f%%" % (100.0 * n / d) if d else "n/a"


def fmt(n):
    return "{:,}".format(n) if isinstance(n, int) else str(n)


def main(argv=None):
    if argv is None:
        argv = sys.argv
    data_dir = argv[1] if len(argv) > 1 else os.path.join(REPO, "data")
    if not os.path.isdir(data_dir):
        data_dir = os.path.normpath(os.path.join(REPO, "..", "7-degrees", "data"))
    st = pickle.load(open(os.path.join(data_dir, "t4", ".state.pkl"), "rb"))
    fetch_summary = json.load(open(os.path.join(data_dir, "t4", "fetch-summary.json")))
    coverage = json.load(open(os.path.join(T4_DIR, "coverage-counts.json")))
    R = coverage["R"]
    with open(os.path.join(T4_DIR, "tenures.csv"), newline="", encoding="utf-8") as f:
        tenures = list(csv.DictReader(f))
    ledger = []
    ledger_path = os.path.join(data_dir, "t4", "bbr-request-ledger.jsonl")
    if os.path.exists(ledger_path):
        with open(ledger_path, encoding="utf-8") as f:
            ledger = [json.loads(l) for l in f if l.strip()]
    http_rows = [r for r in ledger if r.get("action", "").startswith("http")]
    legacy_starts = []
    for row in http_rows:
        stamp = row.get("ts_utc", "")
        if "." not in stamp:
            try:
                legacy_starts.append(datetime.fromisoformat(stamp.replace("Z", "+00:00")))
            except ValueError:
                pass
    legacy_starts.sort()
    legacy_deltas = [(b - a).total_seconds()
                     for a, b in zip(legacy_starts, legacy_starts[1:])]
    legacy_min_delta = min(legacy_deltas) if legacy_deltas else None

    L = []
    A = L.append
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    era_counts = coverage["era_counts"]
    total = len(tenures)
    interval_count = sum(1 for t in tenures if t["start_day"] and t["end_day"])
    no_interval_count = total - interval_count
    by_class = Counter(t["evidence_class"] for t in tenures)
    pairs = coverage["pair_analysis"]
    leg_diag = st["leg_diag"]
    fetch_fail_pages = sorted(fetch_summary.get("failures", {}).keys())
    csv_era_counts = defaultdict(Counter)
    for row in tenures:
        if row["era"]:
            csv_era_counts[row["era"]][row["evidence_class"]] += 1
    class_counts_match = all(
        by_class.get(cls, 0) == R.get("tenures-" + cls, 0) for cls in CLASS_ORDER)
    if (total != R.get("tenures")
            or interval_count != R.get("intervals-present")
            or no_interval_count != R.get("intervals-absent")
            or {e: dict(c) for e, c in csv_era_counts.items()} != era_counts
            or not class_counts_match):
        raise ValueError("coverage-counts.json does not match final tenures.csv")

    A("# T4 — Roster Tenure Reconstruction and Coverage Report")
    A("")
    A("Ticket: #6 (*T4: Reconstruct roster tenures and report coverage*) · parent #1 ·")
    A("spec: `docs/specs/nba-teammate-degrees.md` · glossary: `GLOSSARY.md` ·")
    A("inputs: T3 artifacts (`docs/reports/t3/`), S1 `game` season windows,")
    A("cached Basketball-Reference transaction pages (this report §4).")
    A("")
    A("Report generated %s by `scripts/t4_reconcile.py` + `scripts/t4_report.py`" % now)
    A("(fetch pass: `scripts/t4_fetch_bbr.py`; tests: `scripts/test_t4_core.py`,")
    A("`scripts/test_t4_reconcile.py`, `scripts/test_t4_fetch_bbr.py`). Artifacts retained under `docs/reports/t4/`.")
    A("")
    A("> **No complete-coverage claim.** This report never claims complete exact-date")
    A("> coverage (spec: \"Do not claim complete date-level coverage until the evidence")
    A("> supports it\"). Tenures are dated intervals where evidence allows; every")
    A("> interval below is an *estimate between* evidenced bounds, and the evidence")
    A("> class states how strong the estimate is.")
    A("")

    A("## Verdict at a glance")
    A("")
    A(md_table(["question", "result"], [
        ["membership rows reconstructed (universe players, NBA/BAA)",
         "%s rows (S2 %s + S1-only supplements %s)"
         % (fmt(R["membership_rows"]), fmt(R.get("rows_S2", 0)),
            fmt(R.get("rows_S1_only_supplement", 0)))],
        ["tenure records (incl. multi-stint and unresolved membership rows)",
         "%s records from %s membership rows; %s have interval bounds, %s have no constructible interval"
         % (fmt(total), fmt(R["membership_rows"]), fmt(interval_count),
            fmt(no_interval_count))],
        ["season-window coverage on tenure records",
         "%s with a season window; %s without one"
         % (fmt(R.get("membership-window-present", 0)),
            fmt(R.get("membership-window-missing", 0)))],
        ["directly evidenced tenures (both bounds from dated transactions)",
         "%s (%s)" % (fmt(by_class["directly-evidenced"]), pct(by_class["directly-evidenced"], total))],
        ["cross-checked (transaction + independent season-window agreement)",
         "%s (%s)" % (fmt(by_class["cross-checked"]), pct(by_class["cross-checked"], total))],
        ["inferred (season-window bracket only)", "%s (%s)" % (fmt(by_class["inferred"]), pct(by_class["inferred"], total))],
        ["unresolved (flagged, not invented)", "%s (%s)" % (fmt(by_class["unresolved"]), pct(by_class["unresolved"], total))],
        ["distinct player pairs with a positive tenure overlap (potential edges)",
         "%s (%s both-evidenced / %s involves-inferred / %s involves-unresolved)"
         % (fmt(pairs["distinct_player_pairs_with_positive_overlap"]),
            fmt(pairs["pair_breakdown"].get("both-evidenced", 0)),
            fmt(pairs["pair_breakdown"].get("involves-inferred", 0)),
            fmt(pairs["pair_breakdown"].get("involves-unresolved", 0)))],
        ["1946-1950 BAA source coverage gap", "explicitly carried (§6; BAA-era evidence is structurally thinner)"],
    ]))
    A("")

    # ---------------------------------------------------------------- 1. terms
    A("## 1. What a tenure is here")
    A("")
    A("**Roster tenure** (GLOSSARY.md): a time-bounded period in which a player was on")
    A("an NBA/BAA team's roster, reconstructed from the best available dated source")
    A("records. In this artifact it is a **half-open dated interval** `[start, end)`")
    A("in integer day numbers — end exclusive, matching")
    A("`crates/graph-core/src/lib.rs` (`Tenure { start, end }`, \"end is exclusive\")")
    A("and the spec fixture (`[day 1, day 5)`). A dated departure is the FIRST DAY OFF")
    A("the roster; a one-day stint spans `[D, D+1)`. Mid-season moves yield separate")
    A("interval rows per stint. A teammate edge later requires a *positive overlap*")
    A("of two tenures on the same franchise — adjacency alone (`[a,b)` + `[b,c)`)")
    A("creates no edge day and no edge.")
    A("")
    A("Membership baseline: T3's canonical player-season-team rows (S2 NBA/BAA rows")
    with open(os.path.join(REPO, "docs", "reports", "t3", "player-universe.csv"),
              newline="", encoding="utf-8") as f:
        universe_count = sum(1 for _ in csv.DictReader(f))
    A("plus the %s S1-only supplements from T3 §5), %s-player universe, canonical"
      % (R["rows_S1_only_supplement"], fmt(universe_count)))
    A("franchise ids from `franchise-crosswalk.csv`.")
    A("")

    # ------------------------------------------------------- 2. construction
    A("## 2. How tenures are constructed")
    A("")
    A("For each (universe player, canonical franchise, season) membership row:")
    A("")
    A("1. **Season-window bracket** — first/last game dates for that team-season from")
    A("   the S1 `game` table (`Regular Season` + `Playoffs` team rows; read-only")
    A("   SQLite), converted to half-open day bounds. This brackets the roster tenure")
    A("   in time; season membership alone is NOT treated as proof of simultaneous")
    A("   tenure (spec: Temporal evidence). S1's `game` table ends with the 2022-23")
    A("   season — later seasons get their transaction-dated bounds but no independent")
    A("   window, hence a weaker class (§3 note).")
    A("2. **Dated transaction refinement** — BBR transaction legs (player slug,")
    A("   depart/arrive franchise, date) matching the player slug and the leg's")
    A("   date-derived season bucket. Every precise date is checked against its")
    A("   source page's parsed bounds; off-page but unambiguous dates are")
    A("   re-bucketed by date unless an S1 game window independently confirms an")
    A("   exceptional page-season extension (2019-20 bubble); missing or")
    A("   contradictory windows are flagged and excluded. Arrival events")
    A("   (sign/trade-in/claim/dispersal) anchor the start; departure events")
    A("   (trade-out/waive/release/sale) anchor the end. Fuzzy dates")
    A("   (\"February ?, 1947\") are NEVER resolved to a day — the rows are retained as")
    A("   `fuzzy_events` and only flag tenures for review.")
    A("   A precise offseason arrival carries forward only when adjacent team-season")
    A("   windows bracket the dated move and the same player/team has next-season")
    A("   membership; its transaction date remains the tenure start boundary.")
    atl = next((t for t in tenures if t["bbr_player_id"] == "abdursh01"
                and t["season"] == "2002" and t["canonical_franchise"] == "HAWKS"), None)
    if atl:
        A("   **Cross-season example:** Shareef Abdur-Rahim's NBA_2001 VAN→ATL move")
        A("   on 2001-06-27 anchors the 2002 ATL membership at 2001-06-27 (%s; %s)."
          % (atl["evidence_class"], atl["interval_iso"]))
    A("3. **Stint walking** — arrivals/departures in day order produce stints;")
    A("   conventions as in §1. Ordering anomalies and same-day conflicts are flagged.")
    A("4. **Classification** — `classify_tenure`: unresolved reasons dominate;")
    A("   both bounds transaction-anchored → directly-evidenced (the clean pair;")
    A("   season-window agreement is recorded alongside in")
    A("   `tenures.csv.season_window_iso` — a confirmed-both-sides pair, the")
    A("   strongest possible bounds, is still reported as directly-evidenced);")
    A("   one bound + agreement → cross-checked; otherwise inferred.")
    A("")

    # ------------------------------------------------------------- 3. counts
    A("## 3. Evidence classes — counts")
    A("")
    rows = []
    for cls in CLASS_ORDER:
        rows.append([cls, fmt(by_class[cls]), pct(by_class[cls], total),
                     _class_meaning(cls)])
    A(md_table(["evidence class", "tenures", "share", "meaning"], rows))
    A("")
    A("**Per era** (era boundaries per spec Source validation):")
    A("")
    era_rows = []
    for era in ERA_ORDER:
        c = era_counts.get(era, {})
        era_total = sum(c.values())
        era_rows.append([era, fmt(era_total)] + [fmt(c.get(k, 0)) for k in CLASS_ORDER])
    A(md_table(["era", "tenures"] + CLASS_ORDER, era_rows))
    A("")
    notes = []
    if era_counts.get("BAA 1946-49", {}).get("inferred", 0):
        notes.append("BAA-era tenures are mostly **inferred**: S1 `game` windows exist,")
        notes.append("but BBR 1946-50 transaction rows are sparse and partly fuzzy-dated;")
        notes.append("see the §6 gap callout.")
    modern = era_counts.get("2000-2025/26", {})
    if modern.get("directly-evidenced", 0) or modern.get("cross-checked", 0):
        notes.append("2000-2025/26 tenures carry transaction bounds but lack the")
        notes.append("independent S1 window past 2022-23 — transaction bounds with no")
        notes.append("independent window report as directly-evidenced (§2 step 4), the")
        notes.append("weaker of the two transaction classes.")
    for n in notes:
        A(n)
    A("")

    # ---------------------------------------------------------- 4. requests
    A("## 4. Supplemental web retrieval — BRef transaction pages (request accounting)")
    A("")
    A("`scripts/t4_fetch_bbr.py` fetched the bounded page set")
    A("`/leagues/{BAA|NBA}_{year}_transactions.html` (BAA 1947–1949, NBA 1950–2026 =")
    A("%d pages) **cache-first, with a code-enforced %.1f s minimum start spacing using a"
      % (fetch_summary["pages_targeted"], fetch_summary["throttle_seconds"]))
    A("monotonic clock, with one 45 s back-off + single retry on 429/503/Cloudflare-1015**.")
    A("The historical request ledger has whole-second timestamps; its observed minimum")
    if legacy_min_delta is not None:
        A("adjacent timestamp delta was %.1f s, not proof of a ≥ %.1f s minimum. New HTTP ledger rows"
          % (legacy_min_delta, fetch_summary["throttle_seconds"]))
    else:
        A("timestamp precision does not prove a ≥ %.1f s minimum. New HTTP ledger rows"
          % fetch_summary["throttle_seconds"])
    A("record subsecond UTC start times and measured monotonic elapsed spacing. Every")
    A("request and cache hit is retained in `docs/reports/t4/bbr-request-ledger.csv`.")
    A("Numbers below separate this pass from the ledger's lifetime record.")
    A("")
    statuses = Counter(r.get("status") for r in http_rows)
    A(md_table(["fetch metric", "value"], [
        ["pages targeted", fetch_summary["pages_targeted"]],
        ["**this pass: HTTP requests issued**", fetch_summary.get("http_fetched", 0)],
        ["**this pass: cache hits (no request)**", fetch_summary.get("cache_hits", 0)],
        ["…this pass bytes fetched", fmt(fetch_summary.get("bytes_fetched", 0))],
        ["request ledger lifetime: HTTP requests issued", len(http_rows)],
        ["historical whole-second timestamp precision / minimum adjacent delta",
         "whole-second; %s s (not proof of configured minimum)"
         % ("n/a" if legacy_min_delta is None else "%.1f" % legacy_min_delta)],
        ["…by status", ", ".join("%s: %s" % (k, v) for k, v in sorted(statuses.items(), key=lambda x: (x[0] is None, str(x[0]))))],
        ["request ledger lifetime: cache hits (no request issued)", sum(1 for r in ledger if r.get("action") == "cache-hit")],
        ["retry attempts", sum(1 for r in ledger if r.get("action") == "http-get-retry")],
        ["retries that then succeeded", fetch_summary.get("retries_that_then_succeeded", 0)],
        ["**unresolved fetches (pages)**", "%s%s" % (
            len(fetch_fail_pages),
            (" — `%s`" % ", ".join(fetch_fail_pages)) if fetch_fail_pages else "")],
    ]))
    A("")
    parsed_rows = fetch_summary.get("parsed_rows", 0)
    A(md_table(["parse metric", "value"], [
        ["transaction rows parsed from cache", fmt(parsed_rows)],
        ["rows with precise dates", fmt(parsed_rows - leg_diag.get("row-fuzzy-or-undated", 0))],
        ["rows fuzzy/undated (kept, flagged; never guessed)", fmt(leg_diag.get("row-fuzzy-or-undated", 0))],
        ["dated movement legs extracted", fmt(leg_diag.get("legs", 0))],
        ["legs usable as interval anchors (no blocking flags)", fmt(leg_diag.get("legs_usable", 0))],
        ["precise-dated legs re-bucketed from a different page season",
         fmt(leg_diag.get("leg-note:season-page-rebucketed", 0))],
        ["legs excluded: ambiguous or same-bucket date outside page window",
         fmt(leg_diag.get("leg-flag:page-window-ambiguous", 0)
             + leg_diag.get("leg-flag:date-outside-page-window", 0))],
        ["legs flagged unusable team/shape (row retained; overlaps with other flags)",
         fmt(leg_diag.get("leg-team-unresolved", 0)
             + leg_diag.get("leg-flag:no-team-anchor-on-leg", 0)
             + leg_diag.get("leg-flag:depart-equals-arrive", 0))],
    ]))
    A("")
    rebucketed = leg_diag.get("leg-note:season-page-rebucketed", 0)
    s1_confirmed = leg_diag.get("leg-note:season-s1-window-confirmed", 0)
    ambiguous_window = (leg_diag.get("leg-flag:page-window-ambiguous", 0)
                        + leg_diag.get("leg-flag:date-outside-page-window", 0))
    if rebucketed or s1_confirmed or ambiguous_window:
        A("**Season-bucket guard (BBR page-repetition leak):** BBR repeats major")
        A("trades on the NEXT season's page (the Perkins trade dated 2011-02-24")
        A("appears on both NBA_2011 and NBA_2012), and some rows sit outside")
        A("dates with valid page windows are re-bucketed to the season their own")
        A("date determines unless an S1 game window independently confirms an")
        A("exceptional page-season extension (the 2019-20 bubble); identical")
        A("movement days collapse to one anchor. If the page window is missing/")
        A("malformed or otherwise contradictory, the leg stays in the CSV, is")
        A("flagged, and excluded as ambiguous:")
        A("")
        A(md_table(["season/window validation", "legs"], [
            ["precise date re-bucketed to its own season (usable; duplicate anchors collapse)",
             fmt(rebucketed)],
            ["page season confirmed by independent S1 game window", fmt(s1_confirmed)],
            ["ambiguous or contradictory page window (excluded from anchor use)",
             fmt(ambiguous_window)],
        ]))
        A("")
    if fetch_fail_pages:
        A("**Unresolved fetches:** %s page(s) %s failed after the fetch retry."
          % (len(fetch_fail_pages), ", ".join("`%s`" % page for page in fetch_fail_pages)))
        A("These failures do not prove the underlying historical records are absent.")
        A("Conservatively, reconcile marks all tenures in the failed page's league-season")
        A("and the next membership season (offseason arrivals can cross-link from the")
        A("prior page) `unresolved`, adding `fetch-failure:<league>_<year>`; this is not")
        A("limited to tenures whose other evidence was already missing.")
    else:
        A("This pass had zero unresolved fetch failures. If a future fetch summary")
        A("contains a persistently failed page, reconcile marks every tenure in")
        A("that page's league-season and the next membership season unresolved with")
        A("`fetch-failure:<league>_<year>`; absence of a fetched transaction page is")
        A("never evidence that an underlying historical record did not exist.")
    A("")
    A("Re-run idempotency: with the cache present the pass issues **zero** HTTP")
    A("requests and just re-parses the cached pages.")
    A("")

    # --------------------------------------------------- 5. examples + flags
    A("## 5. Examples per evidence class")
    A("")
    ex_rows = list(csv.reader(open(os.path.join(T4_DIR, "examples.csv"), encoding="utf-8")))
    if ex_rows:
        A(md_table(ex_rows[0], ex_rows[1:80]))
    A("")
    flags = st["flags_rows"]
    A("### 5a. Flags for review (unresolved, never invented)")
    A("")
    A("`docs/reports/t4/unresolved-flags.csv` — all %s flagged rows:" % len(flags))
    A("")
    reason_counts = Counter()
    # count from the UNRESOLVED tenure rows' original reason LISTS: a reason's
    # own text can itself contain "; " inside a parenthetical, so splitting the
    # joined strings would fabricate tail-fragment keys
    for t in st["tenures"]:
        if t["evidence_class"] != "unresolved":
            continue
        for reason in t["reasons"]:
            reason_counts[reason.split("(")[0].strip()] += 1
    A(md_table(["flag reason", "rows"], sorted(reason_counts.items(), key=lambda kv: -kv[1])))
    A("")
    examples_flagged = [f for f in flags if any(
        "same-day" in r for r in f[7].split("; "))][:6]
    if examples_flagged:
        A("**Same-day / ordering-unresolved examples** (each retained verbatim for")
        A("review; none resolved by invention):")
        A("")
        A(md_table(["player", "season", "franchise", "reasons"],
                   [[f[1] or f[0], f[2], f[3], f[7][:120]] for f in examples_flagged]))
    else:
        A("No same-day/ordering-unresolved tenures occurred in this pass. The")
        A("handling path is proven by unit tests")
        A("(`scripts/test_t4_reconcile.py::TestWalkStints::test_same_day_arrival_and_departure_flagged`)")
        A("and remains active for future runs.")
    A("")

    # ----------------------------------------------------------- 6. the gap
    A("## 6. The 1946-1950 BAA source coverage gap (explicit callout)")
    A("")
    A("The pinned bulk sources carry **no transaction dates**, and per-player dated")
    A("evidence before 1996-97 is absent from S1 (`play_by_play` starts 1996-97; the")
    A("S1 `game` table provides only team-level season windows). For the **1946-1950 BAA")
    A("seasons the tenure evidence is therefore structurally thinner than for later")
    A("eras**: tenure records there rest on (a) S1 season windows and (b) the")
    A("sparse, partly fuzzy-dated BBR transaction rows for 1946-50. Per GLOSSARY.md,")
    A("a missing record is **not proof a tenure or edge did not exist**; per the spec,")
    A("uncovered intervals surface here instead of silently becoming graph facts.")
    A("Concretely:")
    A("")
    baa = era_counts.get("BAA 1946-49", {})
    baa_total = sum(baa.values())
    legs = st["legs"]
    legs_per_1950 = sum(1 for r in legs
                        if r["page"].startswith(("BAA_", "NBA_1950")))
    parsed_rows_1950 = st.get("parsed_rows_1950", {})
    rows_1950 = sum(v["total"] for v in parsed_rows_1950.values())
    fuzzy_1950_rows = sum(v["fuzzy_or_undated"]
                          for v in parsed_rows_1950.values())
    A("- %s BAA-era tenure records; %s inferred (season-window only),"
      % (fmt(baa_total), fmt(baa.get("inferred", 0))))
    A("  %s transaction-evidenced (%s directly-evidenced + %s cross-checked)"
      % (fmt(baa.get("directly-evidenced", 0) + baa.get("cross-checked", 0)),
         fmt(baa.get("directly-evidenced", 0)), fmt(baa.get("cross-checked", 0))))
    A("  — the dated transaction layer covers only a fraction of BAA movement;")
    A("- %s dated movement legs parsed from the 1946-50 pages (BAA_1947..BAA_1949 +"
      % fmt(legs_per_1950))
    A("  NBA_1950); %s of the pages' %s transaction rows there carry fuzzy/undated"
      % (fmt(fuzzy_1950_rows), fmt(rows_1950)))
    A("  (\"February ?, 1947\" is a real row shape);")
    A("- all 1946-1950 franchise identities still resolve canonically (T3 crosswalk),")
    A("  so the gap is in *dated movement evidence*, not franchise identity;")
    A("- downstream (T5+) BAA-era teammate edges are therefore weaker-evidenced on")
    A("  average and the app must surface that provenance difference (T10).")
    A("")

    # ------------------------------------------------- 7. downstream feeding
    A("## 7. What this feeds (T5 teammate edges)")
    A("")
    A(md_table(["item", "count"], [
        ["tenure intervals with constructed bounds",
         fmt(interval_count)],
        ["membership/franchise-season tenure records (incl. unresolved no-interval rows)",
         fmt(total)],
        ["distinct player pairs with positive tenure overlap (potential teammate edges, pre-dedup across teams)", fmt(pairs["distinct_player_pairs_with_positive_overlap"])],
        ["…both tenures transaction/season-agreement evidenced", fmt(pairs["pair_breakdown"].get("both-evidenced", 0))],
        ["…at least one inferred tenure", fmt(pairs["pair_breakdown"].get("involves-inferred", 0))],
        ["…at least one unresolved tenure (edge blocked until resolved)", fmt(pairs["pair_breakdown"].get("involves-unresolved", 0))],
        ["membership duplicates/summary rows excluded (2TM/3TM/TOT)", fmt(R.get("row-summary-excluded", 0))],
    ]))
    A("")
    A("Pair counts are *upper bounds on edges*: the graph build deduplicates")
    A("repeated overlaps to one edge per pair (spec AC15) and resolves franchise")
    A("continuity; unresolved tenures do not silently produce edges.")
    A("")

    # --------------------------------------------------------- 8. reproduce
    A("## 8. How to reproduce")
    A("")
    A("```bash")
    A("# from the repo root; requires the pinned data snapshot under data/ (docs/data/source-manifest.md)")
    A("# 1) one-time cached throttled fetch (~8 min; subsequent runs are cache-only, 0 requests)")
    A("python3 scripts/t4_fetch_bbr.py [data_dir]")
    A("# 2) reconstruction + classification")
    A("python3 scripts/t4_reconcile.py [data_dir]   # writes docs/reports/t4/*.csv + data/t4/.state.pkl")
    A("# 3) this report (accepts the same data_dir argument)")
    A("python3 scripts/t4_report.py [data_dir]")
    A("# unit tests (no data dependency, plain python3)")
    A("python3 -m unittest scripts.test_t4_core scripts.test_t4_reconcile scripts.test_t4_fetch_bbr")
    A("```")
    A("")
    A("Python 3.9 stdlib only (`sqlite3`, `csv`, `urllib`, `html.parser`, `pickle`).")
    A("\"data/\" sources are read-only (SQLite is opened in read-only mode);")
    A("cache pages and intermediate state are local under `data/cache/bbr/` and")
    A("`data/t4/`, excluded from git, and not included in the committed artifacts.")
    A("")

    # ------------------------------------------------- 9. open items
    A("## 9. Open items for review (not silently resolved)")
    A("")
    A("1. %s tenure rows remain unresolved with reasons retained in" % fmt(len(flags)))
    A("   `unresolved-flags.csv`; each flag names the exact ambiguity class.")
    A("2. Fuzzy/undated BBR rows (%s) are retained (parsed CSV `row_flags`, fuzzy_events)" % fmt(leg_diag.get("row-fuzzy-or-undated", 0)))
    A("   and never guessed to dates; a manual pass could resolve month-precision rows")
    A("   against Wikipedia season-transaction mirrors (research doc §5).")
    A("3. Seasons past 2022-23 lack the independent S1 season window (`game` table")
    A("   ends 2022-23 S1): transaction-evidenced bounds stay `directly-evidenced`")
    A("   without cross-check until a later S1 refresh.")
    if fetch_fail_pages:
        A("4. %s unresolved fetch(es) recorded in §4 — retry via cache-warming;"
          % len(fetch_fail_pages))
        A("   no absence claim is derived from them.")
    else:
        A("4. No unresolved fetches this pass. On a future persistent page failure,")
        A("   reconcile marks tenures in that page's league-season and the next")
        A("   membership season unresolved with `fetch-failure:<league>_<year>`;")
        A("   it is never proof that a historical record does not exist.")
    A("5. T3's 50 `S1-pbp-shows-other-teams-same-season` membership disagreements")
    A("   remain listed in `docs/reports/t3/membership-mismatches.csv`; T4 inherits")
    A("   the S2 row here (both rows appear when both sources carry them).")
    A("")

    with open(REPORT, "w", encoding="utf-8") as f:
        f.write("\n".join(L) + "\n")
    print("wrote", REPORT)


def _class_meaning(cls):
    return {
        "directly-evidenced": "both bounds anchored by dated transaction rows"
                              " (season-window agreement recorded alongside when present)",
        "cross-checked": "one transaction-anchored bound + independent season-window"
                         " agreement (two sources corroborate the bounds)",
        "inferred": "season-window bracket only (membership season + S1 games)",
        "unresolved": "flagged: conflicts / fuzzy / same-day ordering / ambiguous"
                      " source evidence — needs review",
    }[cls]


if __name__ == "__main__":
    main()