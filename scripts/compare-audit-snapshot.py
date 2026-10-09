#!/usr/bin/env python3
"""Rejoin immutable audited rows to current T4 groups, without reading source prose."""
import argparse
import collections
import csv
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def compare(audit_dir, tenures):
    old = list(csv.DictReader((audit_dir / 'cases.csv').open()))
    new = collections.defaultdict(list)
    key = lambda r: (r['bbr_player_id'], r['lg'], r['season'], r['canonical_franchise'])
    for line, row in enumerate(csv.DictReader(tenures.open()), 2):
        row['current_csv_line'] = line
        new[key(row)].append(row)
    delta = []
    for r in old:
        candidates = new.get(key(r), [])
        exact = [x for x in candidates if (x['start_day'], x['end_day']) == (r['start_day'], r['end_day'])]
        eligible = [x for x in candidates if x['evidence_class'] == 'directly-evidenced' and x['start_day'] and x['end_day'] and int(x['start_day']) < int(x['end_day'])]
        delta.append(dict(audit_id=r['audit_id'], membership_group_present=bool(candidates),
                          current_csv_lines='|'.join(str(x['current_csv_line']) for x in candidates),
                          same_interval_classes='|'.join(x['evidence_class'] for x in exact),
                          graph_eligible_csv_lines='|'.join(str(x['current_csv_line']) for x in eligible)))
    path = audit_dir / 'corrected-delta.csv'
    with path.open('w', newline='') as f:
        w = csv.DictWriter(f, list(delta[0])); w.writeheader(); w.writerows(delta)
    manifest = json.loads((audit_dir / 'export-manifest.json').read_text())
    summary = dict(original_t4_sha256=manifest['original_t4_sha256'], current_path='docs/reports/t4/tenures.csv',
                   current_sha256=hashlib.sha256(tenures.read_bytes()).hexdigest(),
                   selected_original_records=len(old), same_membership_groups=sum(x['membership_group_present'] for x in delta),
                   exact_interval_rows=sum(bool(x['same_interval_classes']) for x in delta),
                   old_direct_cases=sum(r['evidence_class'] == 'directly-evidenced' for r in old),
                   old_direct_same_interval_still_direct=sum(r['evidence_class'] == 'directly-evidenced' and 'directly-evidenced' in x['same_interval_classes'].split('|') for r, x in zip(old, delta)),
                   cases_with_corrected_graph_eligible_interval=sum(bool(x['graph_eligible_csv_lines']) for x in delta),
                   method='Deterministic membership-key and exact-interval comparison; not new source reading or edge proof. CSV lines here refer only to current_sha256. The graph loader also validates identity, blocking flags and pair overlap.')
    (audit_dir / 'corrected-delta-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--audit-dir', type=Path, default=ROOT / 'docs/reports/historical-audit')
    p.add_argument('--tenures', type=Path, default=ROOT / 'docs/reports/t4/tenures.csv')
    a = p.parse_args(); compare(a.audit_dir, a.tenures)
