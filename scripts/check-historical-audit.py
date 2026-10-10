#!/usr/bin/env python3
"""Audit portable receipt integrity, not the truth of historical source paragraphs."""
import argparse
import collections
import csv
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
B = ROOT / 'docs/reports/historical-audit'
load = lambda name: list(csv.DictReader((B / name).open()))
manifest = json.loads((B / 'export-manifest.json').read_text())
coverage = json.loads((B / 'coverage.json').read_text())
cases, sources, refs, groups, signals, delta = map(load, ['cases.csv', 'sources.csv', 'case-sources.csv', 'complex-groups.csv', 'model-signals.csv', 'corrected-delta.csv'])
ids = {r['audit_id'] for r in cases}
sids = {r['source_id'] for r in sources}
assert len(cases) == len(ids) == manifest['selected_original_cases'] == 6246
assert len(groups) == len({g['group_id'] for g in groups}) == 1548
assert len(sources) == len(sids) == manifest['source_locators']
assert len(refs) == manifest['case_source_associations']
assert len(signals) == len(delta) == 6246
assert {r['audit_id'] for r in signals} == {r['audit_id'] for r in delta} == ids
assert all(r['reading_status'] == 'performed' and r['receipt'] for r in cases)
assert dict(collections.Counter(r['outcome'] for r in cases)) == manifest['outcome_counts']
assert dict(collections.Counter(r['partition'] for r in cases)) == manifest['partition_counts']
assert len(coverage['era_strata']) == 6
assert not coverage['pending_case_ids'] and not coverage['pending_complex_group_ids']
for s in coverage['era_strata'].values():
    assert s['selected'] == s['actual_agent_read'] == len(set(s['ids'])) == 50
    assert set(s['ids']) <= ids and not s['pending']
for g in groups:
    assert set(g['records'].split('|')) <= ids and g['reading_status'] == 'complete' and g['pending_records'] == '0'
assert all(r['audit_id'] in ids and r['source_id'] in sids for r in refs)
assert len({tuple(r.values()) for r in refs}) == len(refs)
page_hashes = {r['page'] + '_transactions.html': r['sha256'] for r in csv.DictReader((ROOT / 'docs/reports/t4/transaction-source-pages.csv').open())}
for source in sources:
    expected = manifest['original_source_sha256'].get(source['path']) or page_hashes.get(Path(source['path']).name)
    if expected:
        assert source['sha256'] == expected, source['source_id']
assert all(s['sha256'] == '' or re.fullmatch('[a-f0-9]{64}', s['sha256']) for s in sources)
assert all(not s['sha256'] or s['path'] != 'data/nba.sqlite' for s in sources) # Unknown database hash remains explicit.
assert {r['audit_id'] for r in refs if r['purpose'] == 'official-appearance'} == {'tenure-11764','tenure-11765','tenure-14066','tenure-14067','tenure-14068','tenure-17553','tenure-17554'}
# Recover the immutable original rows from Git, rather than reusing present line numbers.
snap = manifest['original_snapshot']
original = subprocess.check_output(['git', '-C', str(ROOT), 'show', snap['commit'] + ':' + snap['path']])
assert hashlib.sha256(original).hexdigest() == manifest['original_t4_sha256']
old_rows = list(csv.DictReader(original.decode().splitlines()))
for c in cases:
    assert c['audit_id'] == 'tenure-' + c['csv_line'].zfill(5)
    o = old_rows[int(c['csv_line']) - 2]
    assert all(c[k] == o[k] for k in ('bbr_player_id','lg','season','canonical_franchise','evidence_class','start_day','end_day','interval_iso'))
current = (ROOT / 'docs/reports/t4/tenures.csv').read_bytes()
summary = json.loads((B / 'corrected-delta-summary.json').read_text())
assert hashlib.sha256(current).hexdigest() == summary['current_sha256']
assert [summary[k] for k in ('same_membership_groups','exact_interval_rows','old_direct_cases','old_direct_same_interval_still_direct','cases_with_corrected_graph_eligible_interval')] == [6246,2347,1812,503,1082]
for p in B.iterdir():
    if p.suffix in ('.csv','.json'):
        content = p.read_text()
        assert '/Users/' not in content and ' RAW:' not in content and ' DATE:' not in content
hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(B.iterdir()) if p.suffix in ('.csv','.json') and p.name != 'artifact-hashes.json'}
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--write-hashes', action='store_true', help='Explicitly replace the publication hash manifest after a reviewed regeneration.')
if parser.parse_args().write_hashes:
    (B / 'artifact-hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')
else:
    assert json.loads((B / 'artifact-hashes.json').read_text()) == hashes, 'Published artifact hash mismatch'
print(f'Audited {len(cases)} immutable original cases, {len(groups)} complex groups, six 50-case strata, {len(sources)} source locators and {len(refs)} metadata associations; old/current hashes and seven S1 supplement witnesses verified.')
