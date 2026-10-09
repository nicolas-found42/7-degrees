"""Inline regression fixtures for T3 player bridge determinism and safeguards."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from t3_reconcile import Bridge


class BridgeDeterminismTests(unittest.TestCase):
    def test_match_is_independent_of_name_form_order(self):
        career = [
            {"player_id": "kupeccj01", "player": "C.J. Kupec", "birth_date": "1953-01-26"},
            {"player_id": "charlke01", "player": "Ken Charles", "birth_date": "1951-07-10"},
        ]
        bridge = Bridge(career, [], set())
        forms = ["charles kupec", "kupec charles"]

        forward = bridge.match(forms, "1953-01-16")
        reverse = bridge.match(list(reversed(forms)), "1953-01-16")

        self.assertEqual(forward, reverse)
        self.assertEqual(forward[:2], ("initial-surname+dob-window", "kupeccj01"))


class BridgeRuleSafeguardTests(unittest.TestCase):
    def test_career_span_overlap_does_not_bridge_a_short_s1_career(self):
        career = [
            {"player_id": "hillgr01", "player": "Grant Hill", "birth_date": "1972-10-05"},
            {"player_id": "hillge01", "player": "George Hill", "birth_date": "1986-05-04"},
        ]
        spans = {"hillgr01": (1995, 2013), "hillge01": (2009, 2023)}
        bridge = Bridge(career, [], set(), s2_spans=spans)

        result = bridge.match({"herbert hill"}, "1984-10-01", s1_span=(2008, 2008))

        self.assertEqual(result[0], "no-match")

    def test_candidate_career_must_fit_inside_s1_span(self):
        career = [
            {"player_id": "kerrre01", "player": "Red Kerr", "birth_date": "1932-07-17"},
            {"player_id": "kerrst01", "player": "Steve Kerr", "birth_date": "1965-09-27"},
        ]
        spans = {"kerrre01": (1955, 1966), "kerrst01": (1989, 2003)}
        bridge = Bridge(career, [], set(), s2_spans=spans)

        result = bridge.match({"johnny kerr"}, "1932-08-17", s1_span=(1955, 1966))

        self.assertEqual(result[:2], ("surname+career-span", "kerrre01"))

    def test_near_dob_without_year_or_month_day_corroboration_is_not_a_bridge(self):
        career = [
            {"player_id": "obriebo01", "player": "Bob O'Brien", "birth_date": "1927-01-26"},
            {"player_id": "obriera01", "player": "Ralph O'Brien", "birth_date": "1928-04-08"},
        ]
        spans = {"obriebo01": (1948, 1949), "obriera01": (1952, 1953)}
        bridge = Bridge(career, [], set(), s2_spans=spans)

        result = bridge.match({"buckshot obrien"}, "1928-04-28", s1_span=(1952, 1953))

        self.assertEqual(result[0], "no-match")

    def test_initial_and_same_birth_year_still_corroborate_despite_day_shift(self):
        career = [
            {"player_id": "kupeccj01", "player": "C.J. Kupec", "birth_date": "1953-01-26"},
        ]
        bridge = Bridge(career, [], set())

        result = bridge.match({"charles kupec"}, "1953-01-16")

        self.assertEqual(result[:2], ("initial-surname+dob-window", "kupeccj01"))

    def test_matching_month_and_day_corroborates_a_year_shift(self):
        career = [
            {"player_id": "richano01", "player": "Norm Richardson", "birth_date": "1979-07-24"},
        ]
        bridge = Bridge(career, [], set())

        result = bridge.match({"norman richardson"}, "1977-07-24")

        self.assertEqual(result[:2], ("initial-surname+dob-window", "richano01"))


if __name__ == "__main__":
    unittest.main()
