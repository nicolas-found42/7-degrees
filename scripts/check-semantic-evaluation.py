#!/usr/bin/env python3
"""Audit portable experiment counts and receipts; no provider calls or policy decisions."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / 'docs/evaluation'
cases = json.loads((DIR / 'cases.json').read_text())
policy = json.loads((DIR / 'policy.json').read_text())
measurements = json.loads((DIR / 'measurements.json').read_text())['records']
summary = json.loads((DIR / 'summary.json').read_text())
manifest = json.loads((DIR / 'run-manifest.json').read_text())
assert summary['policy'] == policy
assert len({c['id'] for c in cases}) == len(cases)
assert len({(c['component'], c['text'], c['context']) for c in cases}) == len(cases)
for source, expected in manifest['source_sha256'].items():
    assert hashlib.sha256((ROOT / source).read_bytes()).hexdigest() == expected
frozen = [r for r in measurements if r['policy'] == policy]
assert len(frozen) == len(cases) * 3
assert len({(r['case']['id'], r['variant']) for r in frozen}) == len(frozen)
assert {r['case']['id'] for r in frozen} == {c['id'] for c in cases}
assert len(summary['groups']) == 18
for group in summary['groups']:
    rows = [r for r in frozen if all(r['case'][k] == group[k] for k in ('split', 'component')) and r['variant'] == group['variant']]
    assert len(rows) == group['n']
    def observed(row):
        result, component = row['result'], row['case']['component']
        if component == 'resolution':
            return result['player']['id'] if result['player'] else result['status']
        if component == 'query':
            return result['operation'] if result['status'] in ('executed', 'unsupported') else 'abstain'
        return result['chains'][0]['chain']['path'][1]['id'] if result['status'] == 'ranked' else 'abstain'
    predictions = [(r, observed(r)) for r in rows]
    assert sum(pred == r['case']['expected'] for r, pred in predictions) == group['correct']
    assert sum(pred in ('clarification', 'unavailable', 'abstain') for _, pred in predictions) == group['abstain']
    assert sum(pred != r['case']['expected'] and pred not in (('no_match', 'clarification', 'unavailable') if r['case']['component'] == 'resolution' else ('abstain',)) for r, pred in predictions) == group['false_automatic']
    calls = [call for row in rows for call in row['calls']]
    assert len(calls) == group['provider_calls']
    assert sum(c['outcome']['status'] == 'unavailable' for c in calls) == group['unavailable_calls']
    for field in ('input_tokens', 'output_tokens', 'cost_usd'):
        values = [(c['metadata'] or {}).get(field) for c in calls]
        expected = None if any(v is None for v in values) else sum(values)
        observed = group[field]
        assert expected == observed or isinstance(expected, float) and observed is not None and abs(expected - observed) < 1e-12
# Ranking fixture has exactly two degree-two paths; every suggestion retains these facts.
for r in frozen:
    if r['case']['component'] == 'ranking':
        result = r['result']
        assert result['degree'] == 2 and result['total_exact'] == '2'
        assert {tuple(p['id'] for p in chain['chain']['path']) for chain in result['chains']} == {('s', 'b', 'g'), ('s', 'c', 'g')}
        assert all(len(chain['chain']['links']) == 2 for chain in result['chains'])
# Two-candidate resolution and ranking controls change supplied identities/positions,
# rather than reversing an iterator erased by sorted JSON object serialization.
for case_id in ('resolution-05', 'ranking-35'):
    a = next(r for r in frozen if r['case']['id'] == case_id and r['variant'] == 'baseline')
    b = next(r for r in frozen if r['case']['id'] == case_id and r['variant'] == 'order')
    assert a['calls'][0]['request'] != b['calls'][0]['request']
assert all('Bearer ' not in json.dumps(r) for r in measurements)
print(f'Audited {len(cases)} labels, {len(frozen)} frozen outcomes, 18 summaries, source hashes, order controls and unchanged degree-two ranking paths')
