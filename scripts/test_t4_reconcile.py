#!/usr/bin/env python3
"""T4 unit tests — reconcile logic: stint walking, anchors, tenure rows (ticket #6).

Inline fixture rows only (spec: Testing Decisions); NO data/ dependency and NO
network. Runs with plain `python3 -m unittest scripts.test_t4_reconcile` from the
repo root, or `python3 scripts/test_t4_reconcile.py`.

Covers the four acceptance-critical behaviors:
  * interval construction from arrival/departure + season window (half-open,
    mid-season moves yield separate intervals per stint)
  * same-day arrival+departure -> flagged, never invented
  * transaction + season-window agreement -> cross-checked classification
  * fuzzy/flagged transaction legs never anchor a boundary
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
    TenureInterval,
    classify_tenure,
    dated_leg_season,
    date_of_day_number,
    day_number,
)
from t4_reconcile import (  # noqa: E402
    apply_fetch_failures,
    derive_coverage_counts,
    extract_anchors,
    leg_season_bucket,
    load_transaction_legs,
    offseason_arrivals_by_membership,
    walk_stints,
)


def win(start_iso, end_iso):
    """Season window as half-open days [start, end)."""
    return (day_number(start_iso), day_number(end_iso))


SEASON_1947 = win("1946-11-01", "1947-04-23")     # BAA 1946-47 window (real shape)


class TestExtractAnchors(unittest.TestCase):
    def test_trade_leg_opens_and_closes(self):
        # a trade leg with depart+arrive splits one player into two franchise stints
        legs = [
            {"date_iso": "1946-12-04", "depart": "PIT", "arrive": "BOS"},
        ]
        arrs, deps = extract_anchors(legs)
        self.assertEqual(arrs, {"BOS": [day_number("1946-12-04")]})
        self.assertEqual(deps, {"PIT": [day_number("1946-12-04")]})

    def test_sign_leg_only_opens(self):
        legs = [{"date_iso": "1946-09-14", "depart": "", "arrive": "STB"}]
        arrs, deps = extract_anchors(legs)
        self.assertEqual(arrs, {"STB": [day_number("1946-09-14")]})
        self.assertEqual(deps, {})

    def test_waive_leg_only_closes(self):
        legs = [{"date_iso": "1946-12-20", "depart": "NYK", "arrive": ""}]
        arrs, deps = extract_anchors(legs)
        self.assertEqual(arrs, {})
        self.assertEqual(deps, {"NYK": [day_number("1946-12-20")]})

    def test_degenerate_self_leg_ignored(self):
        # a leg naming the same franchise for depart+arrive is degenerate; ignored
        legs = [{"date_iso": "1946-12-04", "depart": "BOS", "arrive": "BOS"}]
        arrs, deps = extract_anchors(legs)
        self.assertEqual((arrs, deps), ({}, {}))

    def test_repeated_identical_anchor_deduplicated(self):
        # source repetition: the same dated sign row twice must not make the day
        # both an open and a close anchor for the same franchise
        legs = [
            {"date_iso": "1946-09-14", "depart": "", "arrive": "BOS"},
            {"date_iso": "1946-09-14", "depart": "", "arrive": "BOS"},
        ]
        arrs, deps = extract_anchors(legs)
        self.assertEqual(arrs, {"BOS": [day_number("1946-09-14")]})
        self.assertEqual(deps, {})


class TestLegSeasonBucketing(unittest.TestCase):
    """The season-bucket guard (BRB page-repetition leak, ticket finding)."""

    def test_date_owns_the_bucket_june_vs_july(self):
        # the BBR league-year flips on July 1 (page windows run July 1..June 30)
        self.assertEqual(dated_leg_season("2011-02-24"), 2011)
        self.assertEqual(dated_leg_season("2011-06-30"), 2011)
        self.assertEqual(dated_leg_season("2011-07-01"), 2012)
        self.assertEqual(dated_leg_season("2011-10-31"), 2012)
        self.assertEqual(dated_leg_season("2020-07-01"), 2021)
        self.assertEqual(dated_leg_season("2024-06-26"), 2024)
        self.assertEqual(dated_leg_season("1947-12-01"), 1948)

    def test_repeated_trade_re_buckets_to_its_own_season(self):
        # Perkins: dated 2011-02-24, repeated on page NBA_2012 -> leg_season_bucket
        # returns 2011 for both pages' copies, so the leak cannot fork stints
        self.assertEqual(leg_season_bucket("2011-02-24", 2012, "NBA"), 2011)
        self.assertEqual(leg_season_bucket("2011-02-24", 2011, "NBA"), 2011)
        # overhangs in the other direction: a July date on the PREVIOUS page
        self.assertEqual(leg_season_bucket("2020-07-01", 2020, "NBA"), 2021)
        self.assertEqual(leg_season_bucket("1948-05-13", 1949, "BAA"), 1948)

    def test_s1_extended_team_window_confirms_page_season(self):
        # COVID bubble games extended 2019-20 into August 2020. The calendar
        # date alone points to 2021; an S1 2020 team window containing the date
        # independently confirms 2020 (unlike the Perkins repeated 2011 trade).
        windows = {("LAKERS", 2020): win("2019-10-22", "2020-08-14")}
        self.assertEqual(leg_season_bucket("2020-07-01", 2020, "NBA",
                                          {"LAKERS"}, windows), 2020)
        self.assertEqual(leg_season_bucket("2020-07-01", 2020, "NBA",
                                          {"LAKERS"}, {}), 2021)

    def test_load_transaction_legs_rebuckets_page_repetition(self):
        # in-page window legs stay clean; a precise row repeated on the next
        # page is re-bucketed to its date season, kept as an anchor, and tagged
        # with a non-blocking diagnostic note. Synthetic pkl blob, no real data.
        import tempfile
        import pickle
        blob = {"rows": [
            # clean: leg date inside page window, bucket == page season
            {"page": "NBA_2011", "league": "NBA", "season": 2011,
             "li_index": 0, "date_text": "February 24, 2011",
             "date_precision": "precise", "date_iso": "2011-02-24",
             "page_window_from": "2010-07-01", "page_window_to": "2011-06-30",
             "event_class": "trade",
             "player_slugs": "perkike01", "player_names": "Kendrick Perkins",
             "legs_json": '[{"slug": "perkike01", "name": "Kendrick Perkins",'
                          ' "depart": "BOS", "arrive": "OKC"}]',
             "text": "traded", "row_flags": ""},
            # repeated on the NEXT page, dated before that page's window
            {"page": "NBA_2012", "league": "NBA", "season": 2012,
             "li_index": 3, "date_text": "February 24, 2011",
             "date_precision": "precise", "date_iso": "2011-02-24",
             "page_window_from": "2011-07-01", "page_window_to": "2012-06-30",
             "event_class": "trade",
             "player_slugs": "perkike01", "player_names": "Kendrick Perkins",
             "legs_json": '[{"slug": "perkike01", "name": "Kendrick Perkins",'
                          ' "depart": "BOS", "arrive": "OKC"}]',
             "text": "traded", "row_flags": "date-outside-page-window"},
            # missing window bounds are ambiguous and must remain excluded
            {"page": "NBA_2013", "league": "NBA", "season": 2013,
             "li_index": 7, "date_text": "February 24, 2011",
             "date_precision": "precise", "date_iso": "2011-02-24",
             "page_window_from": "", "page_window_to": "",
             "event_class": "trade",
             "player_slugs": "ambig01", "player_names": "Ambiguous Player",
             "legs_json": '[{"slug": "ambig01", "name": "Ambiguous Player",'
                          ' "depart": "BOS", "arrive": "OKC"}]',
             "text": "traded", "row_flags": ""},
        ]}
        with tempfile.TemporaryDirectory() as td:
            os.makedirs(os.path.join(td, "t4"))
            with open(os.path.join(td, "t4", "transactions-parsed.pkl"), "wb") as f:
                pickle.dump(blob, f)
            legs, fuzzy, diag, parsed_rows_1950 = load_transaction_legs(td)
        by_src = [l for l in legs if l["slug"] == "perkike01"]
        self.assertEqual(len(by_src), 2)
        self.assertEqual({l["season"] for l in by_src}, {2011})
        repeated_page = [l for l in by_src if l["page"] == "NBA_2012"]
        self.assertEqual(len(repeated_page), 1)
        self.assertEqual(repeated_page[0]["page_season"], 2012)
        self.assertEqual(repeated_page[0]["bucket_season"], 2011)
        self.assertEqual(repeated_page[0]["leg_flags"], "")
        self.assertEqual(diag["leg-note:season-page-rebucketed"], 1)
        self.assertEqual(diag["legs_usable"], 2)
        ambiguous = [l for l in legs if l["slug"] == "ambig01"]
        self.assertEqual(len(ambiguous), 1)
        self.assertIn("page-window-ambiguous", ambiguous[0]["leg_flags"])
        self.assertEqual(ambiguous[0]["season"], 2011)
        # Repeated identical movement day collapses to one actual arrival/departure.
        arrs, deps = extract_anchors(by_src)
        self.assertEqual(arrs, {"THUNDER": [day_number("2011-02-24")]})
        self.assertEqual(deps, {"CELTICS": [day_number("2011-02-24")]})
        self.assertEqual(fuzzy, [])
        self.assertEqual(parsed_rows_1950, {})


class TestWalkStints(unittest.TestCase):
    def test_arrival_then_departure_one_clean_stint(self):
        arrs = [day_number("1946-09-14")]
        deps = [day_number("1947-01-02")]
        stints, flags, anomalies, notes = walk_stints(arrs, deps, SEASON_1947)
        self.assertEqual(len(stints), 1)
        s0, e0, s_anchor, e_anchor = stints[0]
        self.assertEqual(s0, day_number("1946-09-14"))
        self.assertEqual(e0, day_number("1947-01-02"))  # end exclusive AT departure day
        self.assertTrue(s_anchor and e_anchor)
        self.assertEqual((flags, anomalies, notes), ([], [], []))

    def test_same_day_arrival_and_departure_flagged(self):
        # one franchise: trade out and re-sign the same day (order unknowable)
        day = day_number("1946-12-20")
        stints, flags, anomalies, notes = walk_stints([day], [day], SEASON_1947)
        self.assertTrue(any(f[0] == "same-day-arrival-and-departure" for f in flags))
        # pre-existing open membership closes on that day; nothing invented after
        self.assertEqual(len(stints), 1)
        self.assertEqual(stints[0][1], day)   # closed [.., day), nothing re-opened

    def test_departure_without_arrival_uses_window_start(self):
        # player already on the roster at season start (membership from prior
        # season/preceding months); departure dated mid-season
        deps = [day_number("1946-12-20")]
        stints, flags, anomalies, notes = walk_stints([], deps, SEASON_1947)
        self.assertEqual(len(stints), 1)
        s0, e0, s_anchor, e_anchor = stints[0]
        self.assertEqual(s0, SEASON_1947[0])
        self.assertFalse(s_anchor)     # not transaction-anchored
        self.assertTrue(e_anchor)

    def test_arrival_without_departure_runs_to_window_end(self):
        arrs = [day_number("1946-12-04")]
        stints, flags, anomalies, notes = walk_stints(arrs, [], SEASON_1947)
        s0, e0, s_anchor, e_anchor = stints[0]
        self.assertEqual((s0, e0), (day_number("1946-12-04"), SEASON_1947[1]))
        self.assertTrue(s_anchor)
        self.assertFalse(e_anchor)

    def test_no_window_and_open_stint_is_an_anomaly(self):
        arrs = [day_number("1946-12-04")]
        stints, flags, anomalies, notes = walk_stints(arrs, [], None)
        self.assertEqual(stints, [])
        self.assertTrue(any(a[0] == "open-stint-without-window" for a in anomalies))

    def test_departure_without_any_arrival_or_window_is_flagged_as_note(self):
        # a lone dated departure with no window and no arrival is a prior-spell
        # exit or source noise: no stint is constructible, the walk records it
        # as a review note (the tenure row stays unresolved downstream), and
        # nothing is invented
        stints, flags, anomalies, notes = walk_stints([], [day_number("1946-12-20")], None)
        self.assertEqual(stints, [])
        self.assertEqual(anomalies, [])
        self.assertTrue(any(n[0].startswith("departure-after-closed-stint")
                            for n in notes))

    def test_arrival_after_window_end_cannot_stretch_into_a_stint(self):
        # an off-season arrival dated after this window's last game belongs to
        # the NEXT season — flagged, never stretched into a fabricated interval
        arrs = [day_number("1947-06-10")]     # after 1947-04-23 window end
        stints, flags, anomalies, notes = walk_stints(arrs, [], SEASON_1947)
        self.assertEqual(stints, [])
        self.assertTrue(any(a[0] == "arrival-after-window-end(next-season-move)"
                            for a in anomalies))

    def test_mid_season_trade_yields_two_stints_two_franchises(self):
        # Moe Becker: PIT -> BOS Dec 12, 1946 (real BAA_1947 row shape)
        legs = [{"date_iso": "1946-12-12", "depart": "PIT", "arrive": "BOS"}]
        arrs, deps = extract_anchors(legs)
        pit_stints, _, pit_anom, _ = walk_stints([], deps["PIT"], SEASON_1947)
        bos_stints, _, bos_anom, _ = walk_stints(arrs["BOS"], [], SEASON_1947)
        self.assertEqual(len(pit_stints), 1)
        self.assertEqual(len(bos_stints), 1)
        self.assertEqual(pit_stints[0][1], day_number("1946-12-12"))
        self.assertEqual(bos_stints[0][0], day_number("1946-12-12"))
        # half-open: the two stints touch but do not overlap
        a = TenureInterval(pit_stints[0][0], pit_stints[0][1])
        b = TenureInterval(bos_stints[0][0], bos_stints[0][1])
        self.assertEqual(a.overlap_days(b), 0)
        self.assertEqual(a.end, b.start)


class TestTenureConstructionSemantics(unittest.TestCase):
    """Contract tests for the classification flow used by t4_reconcile.reconstruct.

    classify_tenure(start_evidence, end_evidence, cross_agreement, reasons):
    - any unresolved reason -> unresolved
    - both bounds transaction-anchored -> directly-evidenced (the clean pair;
      season-window agreement is recorded alongside, it does not change the
      class)
    - one bound transaction-anchored + agreement -> cross-checked
    - one bound transaction-anchored, no agreement -> inferred
    - no transaction bounds -> inferred
    """

    def class_of(self, s_anchor, e_anchor, window_present=True, reasons=()):
        agreement = bool(window_present)
        return classify_tenure(s_anchor, e_anchor, agreement, list(reasons))

    def test_clean_pair_direct_with_or_without_window(self):
        # signed Sep 14, traded Jan 2: both bounds transaction-anchored ->
        # directly-evidenced; with the window present the agreement is recorded
        # alongside (season_window_iso), without it (S1 game table ends 2022-23)
        # the bounds stand on transactions alone — same class
        self.assertEqual(self.class_of(True, True), CLASS_DIRECT)
        self.assertEqual(self.class_of(True, True, window_present=False), CLASS_DIRECT)

    def test_one_sided_with_agreement_cross_checked(self):
        self.assertEqual(self.class_of(True, False), CLASS_CROSS_CHECKED)
        self.assertEqual(self.class_of(False, True), CLASS_CROSS_CHECKED)

    def test_window_only_inferred(self):
        self.assertEqual(self.class_of(False, False), CLASS_INFERRED)

    def test_same_day_flags_unresolved(self):
        self.assertEqual(
            self.class_of(True, True, reasons=["same-day-arrival-and-departure"]),
            CLASS_UNRESOLVED)

    def test_flagged_legs_unresolved(self):
        self.assertEqual(
            self.class_of(True, False, reasons=["flagged-transaction-legs(team-unresolved)"]),
            CLASS_UNRESOLVED)

    def test_fuzzy_rows_bearing_on_tenure_flag_it(self):
        # a fuzzy row involving this player-season is a review flag, not a date
        self.assertEqual(
            self.class_of(False, False, reasons=["fuzzy-dated-rows-bear-on-this-tenure"]),
            CLASS_UNRESOLVED)

    def test_no_window_agreement_degrades_to_inferred(self):
        # S1 window missing (post-2022-23 seasons) -> no cross agreement:
        # one-sided transaction evidence can no longer cross-check
        self.assertEqual(self.class_of(True, False, window_present=False), CLASS_INFERRED)
        self.assertEqual(self.class_of(False, True, window_present=False), CLASS_INFERRED)


class TestOffseasonArrivalLinkage(unittest.TestCase):
    def test_trade_arrival_anchors_matching_next_season_membership(self):
        membership = [{"season": 2002, "lg": "NBA", "bbr_player_id": "abdursh01",
                       "franchise": "HAWKS"}]
        legs = [{"season": 2001, "league": "NBA", "slug": "abdursh01",
                 "date_iso": "2001-06-27", "depart": "GRIZZLIES",
                 "arrive": "HAWKS", "leg_flags": ""}]
        windows = {
            ("HAWKS", 2001): win("2000-10-31", "2001-04-19"),
            ("HAWKS", 2002): win("2001-10-30", "2002-04-18"),
        }
        carried = offseason_arrivals_by_membership(membership, legs, windows)
        arrivals, departures = extract_anchors(carried[("abdursh01", 2002)])
        self.assertEqual(arrivals, {"HAWKS": [day_number("2001-06-27")]})
        self.assertEqual(departures, {})
        stints, flags, anomalies, notes = walk_stints(
            arrivals["HAWKS"], [], windows[("HAWKS", 2002)])
        self.assertEqual(stints[0][0], day_number("2001-06-27"))
        self.assertEqual((flags, anomalies, notes), ([], [], []))

    def test_offseason_arrival_does_not_carry_to_nonmatching_team(self):
        membership = [{"season": 2002, "lg": "NBA", "bbr_player_id": "abdursh01",
                       "franchise": "CELTICS"}]
        legs = [{"season": 2001, "league": "NBA", "slug": "abdursh01",
                 "date_iso": "2001-06-27", "depart": "GRIZZLIES",
                 "arrive": "HAWKS", "leg_flags": ""}]
        windows = {
            ("HAWKS", 2001): win("2000-10-31", "2001-04-19"),
            ("HAWKS", 2002): win("2001-10-30", "2002-04-18"),
            ("CELTICS", 2001): win("2000-10-31", "2001-04-19"),
            ("CELTICS", 2002): win("2001-10-30", "2002-04-18"),
        }
        self.assertEqual(offseason_arrivals_by_membership(membership, legs, windows), {})


class TestAuthoritativeCoverage(unittest.TestCase):
    def test_counts_are_derived_from_final_tenure_rows(self):
        rows = [
            {"season": 2001, "era": "2000-2025/26", "evidence_class": CLASS_CROSS_CHECKED,
             "start_day": "10", "end_day": "20", "season_window_iso": "[a, b)"},
            {"season": 2002, "era": "2000-2025/26", "evidence_class": CLASS_UNRESOLVED,
             "start_day": "", "end_day": "", "season_window_iso": ""},
            {"season": 2003, "era": "2000-2025/26", "evidence_class": CLASS_UNRESOLVED,
             "start_day": "30", "end_day": "40", "season_window_iso": ""},
        ]
        counts, era_counts = derive_coverage_counts(rows)
        self.assertEqual(counts["tenures"], len(rows))
        self.assertEqual(counts["tenures-cross-checked"], 1)
        self.assertEqual(counts["tenures-unresolved"], 2)
        self.assertEqual(counts["intervals-present"], 2)
        self.assertEqual(counts["intervals-absent"], 1)
        self.assertEqual(counts["membership-window-present"], 1)
        self.assertEqual(counts["membership-window-missing"], 2)
        self.assertEqual(era_counts["2000-2025/26"]["unresolved"], 2)

    def test_persistent_page_failure_overrides_any_evidence_class(self):
        tenures = [
            {"season": 2002, "lg": "NBA", "bbr_player_id": "abdursh01",
             "franchise": "HAWKS", "display_name": "Shareef Abdur-Rahim",
             "membership_source": "S2", "evidence_class": CLASS_DIRECT,
             "reasons": [], "start_anchored": True, "end_anchored": True},
            {"season": 2003, "lg": "NBA", "bbr_player_id": "abdursh01",
             "franchise": "HAWKS", "display_name": "Shareef Abdur-Rahim",
             "membership_source": "S2", "evidence_class": CLASS_DIRECT,
             "reasons": [], "start_anchored": True, "end_anchored": True},
            {"season": 2004, "lg": "NBA", "bbr_player_id": "abdursh01",
             "franchise": "HAWKS", "display_name": "Shareef Abdur-Rahim",
             "membership_source": "S2", "evidence_class": CLASS_DIRECT,
             "reasons": [], "start_anchored": True, "end_anchored": True},
        ]
        apply_fetch_failures(tenures, {})
        self.assertEqual(tenures[0]["evidence_class"], CLASS_DIRECT)
        apply_fetch_failures(tenures, {"NBA_2002": {"status": 503}})
        self.assertEqual(tenures[0]["evidence_class"], CLASS_UNRESOLVED)
        self.assertIn("fetch-failure:NBA_2002", tenures[0]["reasons"])
        self.assertEqual(tenures[1]["evidence_class"], CLASS_UNRESOLVED)
        self.assertIn("fetch-failure:NBA_2002", tenures[1]["reasons"])
        self.assertEqual(tenures[2]["evidence_class"], CLASS_DIRECT)


if __name__ == "__main__":
    unittest.main()