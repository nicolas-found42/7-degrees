#!/usr/bin/env python3
"""T4 — tenure-interval core: calendar arithmetic, half-open intervals,
classification rules (ticket #6).

Everything here is deterministic and data-free: it runs on inline fixture rows so
`scripts/test_t4_core.py` and `scripts/test_t4_reconcile.py` exercise it with
plain python3, no data dependency.

Conventions
-----------
* Dates are calendar dates (`datetime.date`); interval arithmetic uses an
  integer day count (`day_number`, epoch 1946-01-01) so interval lengths and
  overlaps are exact and deterministic.
* Roster-tenure intervals are HALF-OPEN [start, end) — end is exclusive,
  matching the Rust graph-core convention
  (`crates/graph-core/src/lib.rs`: `Tenure { start, end }`, "end is exclusive";
  the spec fixture writes "Player A is on Team Red during [day 1, day 5)").
  A dated departure is the FIRST DAY OFF the roster; a one-day stint spans
  [D, D+1) — length 1, never 0.
* A tenure is one (player, franchise) stint. Its boundaries come from evidence
  of varying precision; `classify_tenure` combines boundary evidence into the
  four coverage classes (spec: directly evidenced / cross-checked / inferred /
  unresolved).
"""

import re
from datetime import date, timedelta

# ------------------------------------------------------------------ day math --
_DAY_ONE = date(1946, 1, 1)   # epoch for stable integer day numbers


def day_number(d):
    """Calendar date or ISO string -> integer day number (1946-01-01 = day 0).

    The zero point is arbitrary; determinism is what matters. League play starts
    1946-11-01 = day 304.
    """
    if isinstance(d, str):
        d = parse_iso_date(d)
    return (d - _DAY_ONE).days


def date_of_day_number(n):
    return _DAY_ONE + timedelta(days=n)


def parse_iso_date(s):
    """'YYYY-MM-DD' -> date; raises ValueError on malformed input."""
    m = re.match(r"^(\d{4})-(\d{2})-(\d{2})$", s or "")
    if not m:
        raise ValueError("not an ISO calendar date: %r" % (s,))
    return date(int(m.group(1)), int(m.group(2)), int(m.group(3)))


def dated_leg_season(date_iso):
    """S2 season bucket ('season starting year' + 1, S2 convention) a precise
    date belongs to.

    BBR league-year convention: the season label flips on July 1 — the
    transaction-page windows themselves run July 1..June 30 — and S2 seasons
    are the starting year + 1 (same convention as `s1_season_id_to_s2_season`
    in t4_reconcile: S1 season_id '22010' -> S2 season 2011).
    '2011-02-24' -> 2011 (the 2010-11 season); '2011-10-31' -> 2012;
    '2020-07-01' -> 2021; '1947-07-01' -> 1948.
    """
    y, m = int(date_iso[:4]), int(date_iso[5:7])
    return y + 1 if m >= 7 else y


def days_in_month(year, month):
    if month == 12:
        return 31
    return (date(year, month + 1, 1) - timedelta(days=1)).day


# ----------------------------------------------------------------- interval --
class TenureInterval(object):
    """Half-open day interval [start, end); end exclusive (graph-core convention).

    Invariant: start < end — an empty interval can never be constructed, so a
    positive overlap between evidenced tenures is the only way a teammate edge
    can arise later (spec: "evidenced, positive time overlap").
    """

    __slots__ = ("start", "end")

    def __init__(self, start, end):
        s, e = int(start), int(end)
        if e <= s:
            raise ValueError(
                "empty or inverted half-open interval [%s, %s); end must be > start"
                % (s, e))
        self.start = s
        self.end = e

    def __eq__(self, other):
        return (isinstance(other, TenureInterval)
                and self.start == other.start and self.end == other.end)

    def __hash__(self):
        return hash((self.start, self.end))

    def __repr__(self):
        return "TenureInterval(%d, %d)" % (self.start, self.end)

    def iso(self):
        return "[%s, %s)" % (date_of_day_number(self.start).isoformat(),
                             date_of_day_number(self.end).isoformat())

    def length_days(self):
        return self.end - self.start

    def overlaps(self, other):
        """Positive-overlap test for two half-open intervals."""
        return self.start < other.end and other.start < self.end

    def overlap_days(self, other):
        lo = max(self.start, other.start)
        hi = min(self.end, other.end)
        return max(0, hi - lo)


# --------------------------------------------------------------- date bounds --
def month_window(year, month):
    """Half-open [first day, last day + 1) day bounds of a calendar month."""
    first = date(year, month, 1)
    if month == 12:
        last = date(year, 12, 31)
    else:
        last = date(year, month + 1, 1) - timedelta(days=1)
    return day_number(first), day_number(last) + 1


def year_window(year):
    return day_number(date(year, 1, 1)), day_number(date(year, 12, 31)) + 1


# -------------------------------------------------------- evidence precision --
PRECISION_DAY = "day"        # exact calendar date evidenced
PRECISION_MONTH = "month"    # fuzzy day within an evidenced month ("February ?, 1947")
PRECISION_YEAR = "year"      # fuzzy month+day within an evidenced year ("1949")
PRECISION_SEASON_BRACKET = "season-bracket"  # bracketed by the season window only


class DatedBound(object):
    """One evidence-backed boundary of a tenure, as a half-open day window.

    `day`     — precise: the window covers the event day itself
    `month`   — the event happened sometime within the month
    `year`    — likewise for a calendar year
    `season-bracket` — a precomputed (lo, hi) window is passed through
    A bound is DIRECTIONAL by role: arrival events (sign/trade-in/claim) anchor
    a tenure START; departure events (trade-out/waive/release/sale) anchor an
    END. Fuzzy rows are parsed as month/year windows, never guessed to a day.
    """

    __slots__ = ("kind", "value", "lo", "hi", "note")

    def __init__(self, kind, value=None, note=""):
        self.kind = kind
        self.value = value
        self.note = note
        if kind == PRECISION_DAY:
            if value is None:
                raise ValueError("day bound requires a day")
            v: int = int(value)
            self.lo, self.hi = v, v + 1                     # covers the event day
        elif kind == PRECISION_MONTH:
            assert value is not None
            y, m = value                                    # (year, month) tuple
            self.lo, self.hi = month_window(y, m)
        elif kind == PRECISION_YEAR:
            assert value is not None
            self.lo, self.hi = year_window(int(value))
        elif kind == PRECISION_SEASON_BRACKET:
            assert value is not None
            self.lo, self.hi = value                        # already a (lo, hi) window
        else:
            raise ValueError("unknown bound precision %r" % (kind,))

    def __repr__(self):
        return "DatedBound(%s, %s..%s, %s)" % (self.kind, self.lo, self.hi, self.note)


def intersect_windows(windows):
    """Intersect half-open day windows -> (lo, hi) or None when empty."""
    lo = max(w[0] for w in windows)
    hi = min(w[1] for w in windows)
    if hi <= lo:
        return None
    return lo, hi


# ------------------------------------------------------------ classification --
# Evidence classes for one (player, franchise, season) tenure (GLOSSARY.md
# "roster tenure"; spec Implementation Decisions "Coverage and provenance").
CLASS_DIRECT = "directly-evidenced"
CLASS_CROSS_CHECKED = "cross-checked"
CLASS_INFERRED = "inferred"
CLASS_UNRESOLVED = "unresolved"


def classify_tenure(start_evidence, end_evidence, cross_agreement,
                    unresolved_reasons):
    """Evidence-class one tenure.

    Parameters
    ----------
    start_evidence, end_evidence : bool
        Whether each boundary is anchored by a dated transaction row resolving
        to a usable bound.
    cross_agreement : bool
        True when the tenure's bounds agree with the independent season-window
        source (containment in the S1 game window of the same franchise-season).
        Recorded as its own column; only upgrades a ONE-sided transaction bound.
    unresolved_reasons : list[str]
        non-empty -> tenure is unresolved (conflicts, same-day ambiguity,
        ordering-unresolved, fuzzy-only evidence) — flagged, never invented.

    Rules (ticket: "a clean pair = directly evidenced; transaction +
    season-window agreement = cross-checked; season-window bracket only =
    inferred; conflict/fuzzy/same-day ambiguity = unresolved with reasons"):
    - any unresolved reason -> **unresolved**
    - BOTH boundaries transaction-anchored -> **directly-evidenced** (the clean
      pair; season-window agreement is recorded alongside, it does not change
      the class)
    - ONE boundary transaction-anchored + season-window agreement for the other
      side -> **cross-checked** (two independent sources on the boundaries)
    - otherwise -> **inferred** (season-window bracket only, or a one-sided
      transaction bound with no independent window to confirm the other side)
    """
    if unresolved_reasons:
        return CLASS_UNRESOLVED
    if start_evidence and end_evidence:
        return CLASS_DIRECT
    if (start_evidence or end_evidence) and cross_agreement:
        return CLASS_CROSS_CHECKED
    return CLASS_INFERRED