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


class TestOptionalListItemEndTags(unittest.TestCase):
    def test_final_transaction_without_li_end_tag_is_retained(self):
        html = ('<span id="transactions_link"></span><ul><li>'
                '<span>June 16, 1975</span><p>The '
                '<a data-attr-from="MIL">Milwaukee Bucks</a> traded '
                '<a href="/players/a/abdulka01.html">Kareem Abdul-Jabbar</a> to the '
                '<a data-attr-to="LAL">Los Angeles Lakers</a>.</p></ul>')
        rows, _, _ = parse_transactions_html(html, 'NBA', 1975)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['date_iso'], '1975-06-16')
        self.assertEqual(rows[0]['legs'][0]['arrive'], 'LAL')

    def test_implicit_item_close_and_end_of_input_preserve_distinct_dates(self):
        html = ('<span id="transactions_link"></span><ul>'
                '<li><span>June 15, 1975</span><p>First transaction.</p>'
                '<li><span>June 16, 1975</span><p>Second transaction.</p>')
        rows, _, _ = parse_transactions_html(html, 'NBA', 1975)
        self.assertEqual([r['date_iso'] for r in rows], ['1975-06-15', '1975-06-16'])


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
    def load_with_draft_history(self, paragraphs, draft_rows, membership_rows=()):
        from t4_reconcile import load_transaction_legs
        rows = []
        for date_iso, paragraph in paragraphs:
            row = self.parse_paragraph(paragraph)
            row['date_iso'] = date_iso
            row['page_window_from'], row['page_window_to'] = '2014-01-01', '2018-12-31'
            row['legs_json'] = json.dumps(row.pop('legs'))
            rows.append(row)
        with tempfile.TemporaryDirectory() as directory:
            os.makedirs(os.path.join(directory, 't4'))
            os.makedirs(os.path.join(directory, 'sumitrodatta'))
            with open(os.path.join(directory, 't4', 'transactions-parsed.pkl'), 'wb') as f:
                pickle.dump({'rows': rows}, f)
            with open(os.path.join(directory, 'sumitrodatta', 'Draft Pick History.csv'), 'w') as f:
                f.write('season,lg,player_id\n' + ''.join('%s,NBA,%s\n' % (y,p) for y,p in draft_rows))
            with open(os.path.join(directory, 'sumitrodatta', 'Player Season Info.csv'), 'w') as f:
                f.write('season,lg,player_id,experience\n' + ''.join('%s,NBA,%s,%s\n' % r for r in membership_rows))
            legs, _, _, _ = load_transaction_legs(directory)
        return legs

    def test_draft_history_blocks_oubre_trade_before_independent_signing(self):
        legs = self.load_with_draft_history([
            ('2015-06-25', 'The <a data-attr-from="ATL">Atlanta Hawks</a> traded '
             '<a href="/players/o/oubreke01.html">Kelly Oubre Jr.</a> to the '
             '<a data-attr-to="WAS">Washington Wizards</a>.'),
            ('2015-07-09', 'The <a data-attr-to="WAS">Washington Wizards</a> signed '
             '<a href="/players/o/oubreke01.html">Kelly Oubre Jr.</a> to a multi-year contract.')],
            [(2015, 'oubreke01')], [(2016, 'oubreke01', 1)])
        self.assertEqual([(l['event_class'], l['date_iso'], l['arrive']) for l in legs],
                         [('sign', '2015-07-09', 'WIZARDS')])

    def test_tyus_jones_draft_transfer_is_not_an_arrival_until_july_signing(self):
        legs = self.load_with_draft_history([
            ('2015-06-25', 'The <a data-attr-from="CLE">Cleveland Cavaliers</a> traded '
             '<a href="/players/j/jonesty01.html">Tyus Jones</a> to the '
             '<a data-attr-to="MIN">Minnesota Timberwolves</a> for a draft pick.'),
            ('2015-07-07', 'The <a data-attr-to="MIN">Minnesota Timberwolves</a> signed '
             '<a href="/players/j/jonesty01.html">Tyus Jones</a> to a multi-year contract.')],
            [(2015, 'jonesty01')], [(2016, 'jonesty01', 1)])
        self.assertEqual([(l['event_class'], l['date_iso']) for l in legs], [('sign', '2015-07-07')])

    def test_russ_smith_return_piece_requires_his_separate_contract_signing(self):
        legs = self.load_with_draft_history([
            ('2014-06-27', 'The <a data-attr-from="NOP">New Orleans Pelicans</a> traded '
             '<a href="/players/j/jackspi01.html">Pierre Jackson</a> to the '
             '<a data-attr-to="PHI">Philadelphia 76ers</a> for '
             '<a href="/players/s/smithru01.html">Russ Smith</a>.'),
            ('2014-07-15', 'The <a data-attr-to="NOP">New Orleans Pelicans</a> signed '
             '<a href="/players/s/smithru01.html">Russ Smith</a> to a multi-year contract.')],
            [(2014, 'smithru01')], [(2015, 'smithru01', 1)])
        smith = [l for l in legs if l['slug'] == 'smithru01']
        self.assertEqual([(l['event_class'], l['date_iso']) for l in smith], [('sign', '2014-07-15')])

    def test_thomas_bryant_draft_transfer_does_not_supply_june_arrival(self):
        legs = self.load_with_draft_history([
            ('2017-06-22', 'The <a data-attr-from="LAL">Los Angeles Lakers</a> traded '
             '<a href="/players/b/bradlto01.html">Tony Bradley</a> to the '
             '<a data-attr-to="UTA">Utah Jazz</a> for '
             '<a href="/players/b/bryanth01.html">Thomas Bryant</a> and '
             '<a href="/players/h/hartjo01.html">Josh Hart</a>.'),
            ('2017-07-30', 'The <a data-attr-to="LAL">Los Angeles Lakers</a> signed '
             '<a href="/players/b/bryanth01.html">Thomas Bryant</a> to a multi-year contract.')],
            [(2017, 'bryanth01')], [(2018, 'bryanth01', 1)])
        bryant = [l for l in legs if l['slug'] == 'bryanth01']
        self.assertEqual([(l['event_class'], l['date_iso']) for l in bryant], [('sign', '2017-07-30')])

    def test_veteran_hardaway_trade_survives_mixed_rookie_trade(self):
        legs = self.load_with_draft_history([
            ('2015-06-25', 'The <a data-attr-from="ATL">Atlanta Hawks</a> traded '
             '<a href="/players/o/oubreke01.html">Kelly Oubre Jr.</a> to the '
             '<a data-attr-to="WAS">Washington Wizards</a>; the '
             '<a data-attr-from="NYK">New York Knicks</a> traded '
             '<a href="/players/h/hardati02.html">Tim Hardaway Jr.</a> to the '
             '<a data-attr-to="ATL">Atlanta Hawks</a>.')],
            [(2015, 'oubreke01'), (2013, 'hardati02')],
            [(2016, 'oubreke01', 1), (2014, 'hardati02', 1)])
        self.assertEqual([(l['slug'], l['depart'], l['arrive']) for l in legs],
                         [('hardati02', 'KNICKS', 'HAWKS')])

    def test_established_rookie_contract_transfers_but_waived_contract_does_not(self):
        for waived in (False, True):
            with self.subTest(waived=waived):
                events = [('2015-07-01', 'The <a data-attr-to="ATL">Atlanta Hawks</a> signed '
                           '<a href="/players/o/oubreke01.html">Kelly Oubre Jr.</a>.')]
                if waived:
                    events.append(('2015-07-02', 'The <a data-attr-from="ATL">Atlanta Hawks</a> waived '
                                   '<a href="/players/o/oubreke01.html">Kelly Oubre Jr.</a>.'))
                events.append(('2015-07-03', 'The <a data-attr-from="ATL">Atlanta Hawks</a> traded '
                               '<a href="/players/o/oubreke01.html">Kelly Oubre Jr.</a> to the '
                               '<a data-attr-to="WAS">Washington Wizards</a>.'))
                legs = self.load_with_draft_history(events, [(2015, 'oubreke01')], [(2016, 'oubreke01', 1)])
                self.assertEqual(any(l['event_class']=='trade' for l in legs), not waived)

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
