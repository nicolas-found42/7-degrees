#!/usr/bin/env python3
"""Focused T4 fetch-ledger tests; no network or data files are used."""

import os
import json
import pickle
import tempfile
import sys
import unittest
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from t4_fetch_bbr import FetchOutcome, http_ledger_record, parse_transactions_html  # noqa: E402


class TestRequestLedgerPrecision(unittest.TestCase):
    def test_http_events_record_fractional_utc_and_measured_spacing(self):
        started = datetime(2026, 10, 9, 12, 34, 56, 123456, tzinfo=timezone.utc)
        record = http_ledger_record(
            "http-get", "https://example.invalid/page", FetchOutcome(200, b"ok"),
            1, started, 5.234567)
        self.assertEqual(record["ts_utc"], "2026-10-09T12:34:56.123456Z")
        self.assertEqual(record["elapsed_since_previous_start_seconds"], 5.234567)
        self.assertEqual(record["start_spacing_seconds"], 5.234567)
        self.assertEqual(record["status"], 200)


class TestTransactionRosterScope(unittest.TestCase):
    def parse_paragraph(self, paragraph):
        html = ('Transactions listed are from July 1, 2010 to June 30, 2011.'
                '<span id="transactions_link"></span><ul><li>'
                '<span>November 30, 2010</span><p>' + paragraph + '</p></li></ul>')
        rows, _, _ = parse_transactions_html(html, "NBA", 2011)
        self.assertEqual(len(rows), 1)
        return rows[0]

    def test_original_dominique_jones_assignment_is_not_an_nba_roster_move(self):
        row = self.parse_paragraph(
            'The <a data-attr-from="DAL" href="/teams/DAL/2011.html">Dallas Mavericks</a> '
            'assigned <a href="/players/j/jonesdo02.html">Dominique Jones</a> '
            'to the Texas Legends of the G-League.')
        self.assertEqual(row['event_class'], 'g-league-assignment')
        self.assertEqual(row['event_scope'], 'non-roster')
        self.assertEqual(row['player_slugs'], 'jonesdo02')
        self.assertEqual(row['date_iso'], '2010-11-30')


    def test_assignment_does_not_reach_dated_nba_roster_anchors(self):
        from t4_reconcile import load_transaction_legs
        row = self.parse_paragraph(
            'The <a data-attr-from="DAL">Dallas Mavericks</a> '
            'assigned <a href="/players/j/jonesdo02.html">Dominique Jones</a> '
            'to the Texas Legends of the G-League.')
        row['legs_json'] = json.dumps(row.pop('legs'))
        with tempfile.TemporaryDirectory() as directory:
            os.makedirs(os.path.join(directory, 't4'))
            with open(os.path.join(directory, 't4', 'transactions-parsed.pkl'), 'wb') as f:
                pickle.dump({'rows': [row]}, f)
            legs, fuzzy, diagnostics, _ = load_transaction_legs(directory)
        self.assertEqual(legs, [])
        self.assertEqual(fuzzy, [])
        self.assertEqual(diagnostics['leg-excluded-non-roster'], 1)


    def test_original_draftnight_rights_sale_does_not_prove_occupancy(self):
        row = self.parse_paragraph(
            'The <a data-attr-from="MEM">Memphis Grizzlies</a> sold the player rights to '
            '<a href="/players/j/jonesdo02.html">Dominique Jones</a> to the '
            '<a data-attr-to="DAL">Dallas Mavericks</a>.')
        self.assertEqual(row['event_class'], 'rights-transfer')
        self.assertEqual(row['event_scope'], 'non-roster')
        self.assertEqual(row['player_slugs'], 'jonesdo02')


    def test_actual_trade_survives_future_pick_and_swap_rights_commentary(self):
        # Original NBA_2016 LI6: Kyle O'Quinn's July 9, 2015 trade.
        row = self.parse_paragraph(
            'The <a data-attr-from="NYK">New York Knicks</a> traded Cash and a '
            '2019 2nd round draft pick (<a href="/players/e/edwarca01.html">Carsen Edwards</a> '
            'was later selected) to the <a data-attr-to="ORL">Orlando Magic</a> for '
            '<a href="/players/o/oquinky01.html">Kyle O’Quinn</a> and a 2019 2nd round '
            'draft pick (<a href="/players/b/brazdig01.html">Ignas Brazdeikis</a> '
            'was later selected). (ORL has rights to swap 2nd round picks in 2019)')
        scopes = {leg['slug']: leg['scope'] for leg in row['legs']}
        self.assertEqual(scopes, {'edwarca01': 'non-roster', 'oquinky01': 'nba-roster',
                                 'brazdig01': 'non-roster'})
        active = next(l for l in row['legs'] if l['slug'] == 'oquinky01')
        self.assertEqual((active['depart'], active['arrive']), ('ORL', 'NYK'))
        self.assertEqual(row['event_scope'], 'mixed')


    def test_hiring_a_player_linked_head_coach_does_not_sign_a_player(self):
        row = self.parse_paragraph(
            'The <a data-attr-to="LAL">Los Angeles Lakers</a> hired '
            '<a href="/players/r/rileypa01.html">Pat Riley</a> as head coach '
            'and signed him to a multiyear contract.')
        self.assertEqual(row['event_class'], 'coach-hire')
        self.assertEqual(row['event_scope'], 'non-roster')

    def test_g_league_loans_and_transfers_are_not_nba_roster_changes(self):
        for action in ('loaned', 'transferred'):
            with self.subTest(action=action):
                row = self.parse_paragraph(
                    'The <a data-attr-from="DAL">Dallas Mavericks</a> ' + action +
                    ' <a href="/players/j/jonesdo02.html">Dominique Jones</a> '
                    'to the Texas Legends of the G-League.')
                self.assertEqual(row['event_scope'], 'non-roster')

    def test_signing_from_g_league_is_an_actual_nba_roster_arrival(self):
        row = self.parse_paragraph(
            'The <a data-attr-to="DAL">Dallas Mavericks</a> signed '
            '<a href="/players/j/jonesdo02.html">Dominique Jones</a> '
            'from the Texas Legends of the G-League.')
        self.assertEqual(row['event_class'], 'sign')
        self.assertEqual(row['event_scope'], 'nba-roster')
        self.assertEqual((row['legs'][0]['depart'], row['legs'][0]['arrive']), ('', 'DAL'))

    def test_original_recall_retains_identity_without_roster_arrival(self):
        row = self.parse_paragraph(
            'The <a data-attr-to="DET">Detroit Pistons</a> recalled '
            '<a href="/players/a/ackeral01.html">Alex Acker</a> '
            'from the Fort Wayne Mad Ants of the G-League.')
        self.assertEqual(row['event_class'], 'g-league-recall')
        self.assertEqual(row['event_scope'], 'non-roster')
        self.assertEqual(row['legs'][0]['scope'], 'non-roster')

    def test_contract_extension_is_not_a_new_roster_arrival(self):
        row = self.parse_paragraph(
            'The <a data-attr-to="DAL">Dallas Mavericks</a> signed an extension '
            'with <a href="/players/j/jonesdo02.html">Dominique Jones</a>.')
        self.assertEqual(row['event_class'], 'extension')
        self.assertEqual(row['event_scope'], 'non-roster')


    def test_original_two_way_conversion_keeps_nba_roster_service(self):
        row = self.parse_paragraph(
            'The <a data-attr-from="SAC">Sacramento Kings</a> converted '
            '<a href="/players/m/metuch01.html">Chimezie Metu</a> '
            'from a two-way contract to a regular contract.')
        self.assertEqual(row['event_class'], 'contract-conversion')
        self.assertEqual(row['event_scope'], 'non-roster')
        self.assertEqual(row['legs'][0]['scope'], 'non-roster')

    def test_original_not_resigned_contract_expiration_is_a_departure(self):
        row = self.parse_paragraph(
            '<a href="/players/b/blueva01.html">Vander Blue</a> not re-signed by '
            '<a data-attr-from="BOS">Boston Celtics</a>; 10-day contract expires.')
        self.assertEqual(row['event_class'], 'contract-expiration')
        self.assertEqual(row['legs'][0]['action_class'], 'contract-expiration')
        self.assertEqual(row['legs'][0]['scope'], 'nba-roster')
        self.assertEqual((row['legs'][0]['depart'], row['legs'][0]['arrive']), ('BOS', ''))

    def test_negated_signing_without_expiration_does_not_prove_roster_service(self):
        row = self.parse_paragraph(
            'The <a data-attr-to="BOS">Boston Celtics</a> did not sign '
            '<a href="/players/b/blueva01.html">Vander Blue</a>.')
        self.assertEqual(row['event_scope'], 'unresolved')

    def test_same_li_signing_is_preserved_when_another_paragraph_assigns_player(self):
        html = ('Transactions listed are from July 1, 2011 to June 30, 2012.'
                '<span id="transactions_link"></span><ul><li><span>December 21, 2011</span>'
                '<p>The <a data-attr-to="DAL">Dallas Mavericks</a> signed '
                '<a href="/players/w/willise01.html">Sean Williams</a> as a free agent.</p>'
                '<p>The <a data-attr-from="DAL">Dallas Mavericks</a> assigned '
                '<a href="/players/w/willise01.html">Sean Williams</a> '
                'to the Texas Legends of the G-League.</p></li></ul>')
        rows, _, _ = parse_transactions_html(html, 'NBA', 2012)
        self.assertEqual([(r['paragraph_index'], r['event_scope']) for r in rows],
                         [(0, 'nba-roster'), (1, 'non-roster')])

    def test_named_draft_pick_transfer_cannot_anchor_occupancy_but_veteran_trade_survives(self):
        row = self.parse_paragraph(
            'In a 3-team trade, the <a data-attr-from="ATL">Atlanta Hawks</a> traded '
            '<a href="/players/g/grantje02.html">Jerian Grant</a> to the '
            '<a data-attr-to="NYK">New York Knicks</a>; the '
            '<a data-attr-from="NYK">New York Knicks</a> traded '
            '<a href="/players/h/hardati02.html">Tim Hardaway Jr.</a> to the '
            '<a data-attr-to="ATL">Atlanta Hawks</a>; and the '
            '<a data-attr-from="WAS">Washington Wizards</a> traded a 2015 1st round draft pick '
            '(<a href="/players/g/grantje02.html">Jerian Grant</a> was later selected) to the '
            '<a data-attr-to="ATL">Atlanta Hawks</a>.')
        grant = [l for l in row['legs'] if l['slug'] == 'grantje02']
        self.assertEqual([(l['scope'], l['action_class']) for l in grant],
                         [('unresolved', 'draft-roster-status-unresolved'),
                          ('non-roster', 'draft-pick-reference')])
        veteran = next(l for l in row['legs'] if l['slug'] == 'hardati02')
        self.assertEqual(veteran['scope'], 'nba-roster')
        self.assertEqual((veteran['depart'], veteran['arrive']), ('NYK', 'ATL'))


if __name__ == "__main__":
    unittest.main()
