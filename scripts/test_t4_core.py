#!/usr/bin/env python3
"""T4 unit tests — interval math + classification rules (ticket #6).

Inline fixture rows only (spec: Testing Decisions — small, hand-checked,
deterministic fixtures). No data/ dependency: runs with plain
`python3 -m unittest scripts.test_t4_core` from the repo root, or
`python3 scripts/test_t4_core.py`.
"""

import os
import sys
import unittest
from datetime import date

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from t4_core import (  # noqa: E402
    CLASS_CROSS_CHECKED,
    CLASS_DIRECT,
    CLASS_INFERRED,
    CLASS_UNRESOLVED,
    DatedBound,
    PRECISION_DAY,
    PRECISION_MONTH,
    PRECISION_SEASON_BRACKET,
    PRECISION_YEAR,
    TenureInterval,
    classify_tenure,
    date_of_day_number,
    day_number,
    days_in_month,
    intersect_windows,
    month_window,
    parse_iso_date,
    year_window,
)


def d(iso):
    return day_number(iso)


class TestDayMath(unittest.TestCase):
    def test_epoch_and_known_days(self):
        # 1946-11-01 is the first recorded league game (S1 game table min(game_date))
        self.assertEqual(day_number("1946-11-01"), 304)
        self.assertEqual(date_of_day_number(304), date(1946, 11, 1))
        # leap year: 1996-02-29 exists
        self.assertEqual(day_number("1996-02-29") - day_number("1996-02-28"), 1)
        # round trip
        self.assertEqual(date_of_day_number(day_number("2023-06-12")), date(2023, 6, 12))

    def test_parse_iso_rejects_malformed(self):
        for bad in ("19461101", "1946-13-01", "1946-02-30", "", "February ?, 1947", None):
            with self.assertRaises((ValueError, TypeError)):
                parse_iso_date(bad)

    def test_days_in_month(self):
        self.assertEqual(days_in_month(1996, 2), 29)
        self.assertEqual(days_in_month(1995, 2), 28)
        self.assertEqual(days_in_month(1947, 12), 31)
        self.assertEqual(days_in_month(1947, 4), 30)


class TestTenureIntervalHalfOpen(unittest.TestCase):
    def test_length_counts_the_start_not_the_end(self):
        # A on Red [day 1, day 5) = 4 days (fixture convention, spec Concrete Example)
        t = TenureInterval(1, 5)
        self.assertEqual(t.length_days(), 4)

    def test_one_game_day_is_length_one_not_zero(self):
        # a one-day stint [D, D+1) is representable, never collapses to 0
        t = TenureInterval(100, 101)
        self.assertEqual(t.length_days(), 1)
        self.assertEqual(t.overlap_days(TenureInterval(101, 102)), 0)

    def test_empty_and_inverted_rejected(self):
        with self.assertRaises(ValueError):
            TenureInterval(5, 5)     # empty — half-open [x, x) holds no days
        with self.assertRaises(ValueError):
            TenureInterval(7, 3)     # inverted

    def test_touching_intervals_do_not_overlap(self):
        # [1,5) and [5,9) do NOT overlap: day 5 belongs to the second interval
        a, b = TenureInterval(1, 5), TenureInterval(5, 9)
        self.assertFalse(a.overlaps(b))
        self.assertFalse(b.overlaps(a))
        self.assertEqual(a.overlap_days(b), 0)

    def test_positive_overlap_cases(self):
        cases = [
            ((1, 5), (2, 4), 2),    # containment
            ((1, 5), (4, 9), 1),    # partial
            ((1, 5), (5, 9), 0),    # adjacency (no positive overlap)
            ((1, 5), (0, 2), 1),
            ((1, 5), (6, 9), 0),    # disjoint
        ]
        for (s1, e1), (s2, e2), want in cases:
            a, b = TenureInterval(s1, e1), TenureInterval(s2, e2)
            self.assertEqual(a.overlap_days(b), want, (a, b))
            self.assertEqual(b.overlap_days(a), want)
            self.assertEqual(a.overlaps(b), want > 0)
            self.assertEqual(b.overlaps(a), want > 0)

    def test_overlap_matches_graph_core_semantics(self):
        # graph-core Tenure::overlap_days: max(starts) -> min(ends), saturating sub
        a = TenureInterval(d("1996-11-01"), d("1996-11-05"))
        b = TenureInterval(d("1996-11-04"), d("1996-11-08"))
        self.assertEqual(a.overlap_days(b), 1)
        self.assertEqual(b.overlap_days(a), 1)

    def test_iso_round_trip_and_equality(self):
        t = TenureInterval(d("1946-11-01"), d("1947-04-30"))
        s, e = t.iso().strip("[])").split(", ")
        self.assertEqual(TenureInterval(d(s), d(e)), t)
        self.assertEqual(hash(t), hash(TenureInterval(d(s), d(e))))

    def test_mid_season_moves_keep_separate_intervals(self):
        # traded Dec 4: stint1 [season_start, trade_day+1), stint2 [trade_day+1, season_end+1)
        season_start = d("2023-10-25")
        season_end_plus = d("2024-04-15")
        trade_out_day = d("2023-12-04")
        trade_in_day = d("2023-12-05")
        stint_a = TenureInterval(season_start, trade_out_day + 1)  # leaves after Dec 4
        stint_b = TenureInterval(trade_in_day, season_end_plus)    # joins Dec 5
        # adjacent, non-overlapping with itself; no invention
        self.assertEqual(stint_a.overlap_days(stint_b), 0)
        self.assertEqual(stint_a.end, d("2023-12-05"))
        self.assertEqual(stint_b.start, d("2023-12-05"))


class TestWindows(unittest.TestCase):
    def test_month_window_half_open(self):
        lo, hi = month_window(1947, 2)
        self.assertEqual(date_of_day_number(lo), date(1947, 2, 1))
        self.assertEqual(date_of_day_number(hi), date(1947, 3, 1))   # exclusive end
        self.assertEqual(hi - lo, 28)

    def test_year_window_covering(self):
        lo, hi = year_window(1950)
        self.assertEqual(date_of_day_number(lo), date(1950, 1, 1))
        self.assertEqual(hi - lo, 365)

    def test_intersect(self):
        w1 = month_window(1947, 2)
        w2 = (d("1947-02-10"), d("1947-03-15"))
        self.assertEqual(intersect_windows([w1, w2]), (d("1947-02-10"), d("1947-03-01")))
        self.assertIsNone(intersect_windows([month_window(1947, 2), (d("1947-03-01"), d("1947-03-31"))]))
        self.assertIsNone(intersect_windows([w1, (d("1947-03-01"), d("1947-03-31"))]))

    def test_intersect_empty_window(self):
        self.assertIsNone(intersect_windows([(5, 5)]))
        self.assertIsNone(intersect_windows([(7, 3)]))


class TestDatedBound(unittest.TestCase):
    def test_day_bound_exact(self):
        b = DatedBound(PRECISION_DAY, value=d("1947-01-16"))
        self.assertEqual((b.lo, b.hi), (d("1947-01-16"), d("1947-01-16") + 1))

    def test_month_bound_covers_the_month(self):
        b = DatedBound(PRECISION_MONTH, value=(1947, 2))
        self.assertEqual((b.lo, b.hi), month_window(1947, 2))

    def test_year_bound_covers_the_year(self):
        b = DatedBound(PRECISION_YEAR, value=1949)
        self.assertEqual((b.lo, b.hi), year_window(1949))

    def test_season_bracket_bound_passes_window_through(self):
        w = (d("1946-11-01"), d("1947-04-23"))
        b = DatedBound(PRECISION_SEASON_BRACKET, value=w)
        self.assertEqual((b.lo, b.hi), w)

    def test_day_bound_requires_a_day(self):
        with self.assertRaises(ValueError):
            DatedBound(PRECISION_DAY)
        with self.assertRaises(ValueError):
            DatedBound("nonsense", value=3)


class TestClassification(unittest.TestCase):
    """The four evidence classes (issue #6 AC2; spec Implementation Decisions)."""

    def test_unresolved_wins_over_everything(self):
        for reason in (
            ["same-day ambiguity: trade and sign rows share one date"],
            ["fuzzy date only (no resolvable day/month)"],
            ["ordering-unresolved: two dated events, direction ambiguous"],
            ["conflicting team assignment across sources"],
            ["transaction rows contradict the season-window team membership"],
        ):
            self.assertEqual(
                classify_tenure(True, True, True, reason), CLASS_UNRESOLVED, reason)
            self.assertEqual(
                classify_tenure(False, False, False, reason), CLASS_UNRESOLVED)

    def test_directly_evidenced_both_bounds_from_transactions(self):
        # both bounds transaction-anchored -> directly-evidenced; season-window
        # agreement is recorded alongside but does not change the class from
        # the clean pair (t4_reconcile records agreement in season_window_iso)
        self.assertEqual(classify_tenure(True, True, False, []), CLASS_DIRECT)
        self.assertEqual(classify_tenure(True, True, True, []), CLASS_DIRECT)

    def test_cross_checked_transaction_plus_season_agreement(self):
        # ONE transaction-anchored boundary + the independent season window agrees
        self.assertEqual(classify_tenure(True, False, True, []), CLASS_CROSS_CHECKED)
        self.assertEqual(classify_tenure(False, True, True, []), CLASS_CROSS_CHECKED)

    def test_inferred_season_bracket_only(self):
        self.assertEqual(classify_tenure(False, False, False, []), CLASS_INFERRED)
        # one-sided transaction evidence without season-window agreement stays inferred
        self.assertEqual(classify_tenure(True, False, False, []), CLASS_INFERRED)
        self.assertEqual(classify_tenure(False, True, False, []), CLASS_INFERRED)


class TestFixtureScenarioSemantics(unittest.TestCase):
    """Spec Concrete Example mapped onto T4 evidence classes + interval math."""

    def test_spec_fixture_overlaps(self):
        # A Red [1,5), B Red [2,4), B Blue [10,13), C Blue [11,12), D Red [20,25)
        a_red = TenureInterval(1, 5)
        b_red = TenureInterval(2, 4)
        b_blue = TenureInterval(10, 13)
        c_blue = TenureInterval(11, 12)
        d_red = TenureInterval(20, 25)
        self.assertTrue(a_red.overlaps(b_red))          # A–B edge
        self.assertTrue(b_blue.overlaps(c_blue))        # B–C edge
        self.assertFalse(a_red.overlaps(d_red))         # A–D no edge
        self.assertFalse(a_red.overlaps(b_blue))        # same player, non-overlap
        # A to C degree 2 via B: no direct A–C overlap
        self.assertFalse(a_red.overlaps(c_blue))

    def test_repeated_overlaps_collapse_to_one_edge(self):
        # A Green [6,11); E Green [6,8) and [9,11) — 4 overlap days, ONE edge
        a = TenureInterval(6, 11)
        e1 = TenureInterval(6, 8)
        e2 = TenureInterval(9, 11)
        self.assertEqual(a.overlap_days(e1) + a.overlap_days(e2), 4)

    def test_unresolved_never_becomes_an_interval_fact(self):
        # a fuzzy-only boundary (March ?, 1949) cannot anchor an exact start;
        # the tenure stays unresolved with the reason retained.
        reasons = ["fuzzy start: 'March ?, 1949' (month-precision)"]
        self.assertEqual(classify_tenure(False, False, False, reasons), CLASS_UNRESOLVED)


if __name__ == "__main__":
    unittest.main()