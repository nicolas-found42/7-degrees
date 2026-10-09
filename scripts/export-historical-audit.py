#!/usr/bin/env python3
"""Publish metadata from retained reading receipts, without copying source prose.
This is a deterministic export, not a new historical inspection or model judgment.
"""
import argparse
import collections
import csv
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def jsonlines(path):
    with path.open() as f:
        for line in f:
            yield json.loads(line)

def csvwrite(path, rows, fields):
    with path.open('w', newline='') as f:
        writer = csv.DictWriter(f, fields, extrasaction='ignore', lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)

def portable(value, audit):
    value = str(value)
    for marker in ('/data/', '/docs/'):
        if marker in value:
            return marker[1:] + value.split(marker, 1)[1]
    prefix = str(audit) + '/'
    return value.replace(prefix, '')

def export(audit, output):
    output.mkdir(parents=True, exist_ok=True)
    coverage = json.loads((audit / 'acceptance29-coverage.json').read_text())
    selection = json.loads((audit / 'selection-manifest.json').read_text())
    originals = {r['audit_id']: r for r in jsonlines(audit / 'acceptance29-all-case-reading.jsonl')}
    assert len(originals) == 6246 and not coverage['pending_case_ids']
    assert coverage['old_t4_sha256'] == 'f4fe081d35518782969f502c68c7ba622b987a4d7bf042a729bf2333ff18a030'
    catalog, refs = {}, set()
    page_hashes = {r['page']: r['sha256'] for r in csv.DictReader((ROOT / 'docs/reports/t4/transaction-source-pages.csv').open())}

    def source(audit_id, receipt, purpose='context', boundary='', role=''):
        # Retain only source metadata before original CSV or paragraph text.
        receipt = receipt.split(' RAW:', 1)[0].split(' DATE:', 1)[0]
        receipt = portable(receipt.removeprefix('Mapping context '), audit)
        match = re.match(r'((?:data|docs)/.+?)(?::L?(\d+)|#|$)(.*)', receipt)
        if not match:
            return
        path, line, extra = match.groups()
        hashed = re.search(r'(?:page_)?sha256=([a-f0-9]{64})', extra)
        sha = hashed.group(1) if hashed else selection['inputs'].get(path, '')
        if not sha and path.endswith('_transactions.html'):
            sha = page_hashes.get(Path(path).name.removesuffix('_transactions.html'), '')
        locator = ('line=' + line if line else '')
        if '#' in receipt:
            locator += receipt.split('#', 1)[1]
        for name in ('raw_li', 'paragraph'):
            m = re.search(name + r'=(\d+)', extra)
            if m:
                locator += (' ' if locator else '') + name + '=' + m.group(1)
        key = (path, locator, sha)
        ident = 'source-' + hashlib.sha256(json.dumps(key).encode()).hexdigest()[:16]
        row = dict(source_id=ident, path=path, locator=locator, sha256=sha,
                   url=('https://www.basketball-reference.com/leagues/' + Path(path).name) if path.endswith('_transactions.html') else '')
        assert ident not in catalog or catalog[ident] == row
        catalog[ident] = row
        refs.add((audit_id, ident, purpose, boundary, role))

    # Full retained source context, including independent membership/identity mapping.
    for r in jsonlines(audit / 'main-dossiers.jsonl'):
        if r['audit_id'] in originals:
            for receipt in r.get('original_receipts', []):
                source(r['audit_id'], receipt)
    # Partition outcomes refer to immutable external receipt lines.
    receipt_lines = {}
    for partition in ('main-host-partition', 'modern-partition', 'baa-defunct'):
        for line, r in enumerate(jsonlines(audit / partition / 'inspection-outcomes.jsonl'), 1):
            receipt_lines[r['audit_id']] = f'{partition}/inspection-outcomes.jsonl:{line}'
    cases, models = [], []
    nba_ids = {r['bbr_player_id']: r['s1_player_id'] for r in csv.DictReader((ROOT / 'docs/reports/t3/player-universe.csv').open())}
    for ident, row in sorted(originals.items()):
        old, d = row['original_case'], row['detail']
        if row['partition'] == 'baa-defunct':
            assert d['inspection_status'] == 'performed'
            for receipt in json.loads(d['source_refs']):
                source(ident, receipt)
        else:
            assert d.get('host_source_reading_status') == 'performed' or d.get('host_original_reading')
            for b in d.get('boundaries', []):
                for match in b['matches']:
                    source(ident, match['source_receipt'], 'boundary', b['date'], match['role'])
        # S1-only supplements retain exact event locators separately from absent S2 rows.
        for supplement in ('supplement_appearance_receipt_file', 'supplement_postseason_actual_event_receipt_file'):
            if d.get(supplement):
                file = Path(d[supplement])
                value = json.loads(file.read_text())
                values = value if isinstance(value, list) else value.get('records', [])
                if isinstance(value, dict) and 'cases' in value:
                    values = [dict(audit_ids=[ident], play_by_play_row=dict(event, rowid=event['source_rowid']), player_slot=1)
                              for c in value['cases'] if c['player_id'] == nba_ids.get(old['bbr_player_id'])
                              for event in c['original_event_receipts']]
                for mapping in d.get('supplement_mapping_receipt', {}).get('original_mapping_receipts', []):
                    source(ident, f"{mapping['file']}:{mapping['line']} sha256={mapping['sha256']}", 'independent-team-mapping')
                for event in values:
                    if ident in event.get('audit_ids', []) or event.get('audit_id') == ident:
                        pbp = event.get('play_by_play_row', {})
                        if pbp.get('rowid'):
                            key = ('data/nba.sqlite', f"table=play_by_play rowid={pbp['rowid']} game_id={pbp.get('game_id', '')} slot={event.get('player_slot', '')}", '')
                            sid = 'source-' + hashlib.sha256(json.dumps(key).encode()).hexdigest()[:16]
                            catalog[sid] = dict(source_id=sid, path=key[0], locator=key[1], sha256='', url='')
                            refs.add((ident, sid, 'official-appearance', '', 'appearance witness; not a roster boundary'))
        case = {k: old.get(k, '') for k in ('bbr_player_id', 'display_name', 'season', 'lg', 'era', 'canonical_franchise', 'membership_source', 'evidence_class', 'start_day', 'end_day', 'interval_iso', 'csv_line')}
        case.update(audit_id=ident, selection_reasons='|'.join(old['selection_reasons']),
                    partition=row['partition'], reading_status='performed', outcome=row['outcome'],
                    receipt=receipt_lines[ident], scope_flags='|'.join(d.get('semantic_scope_flags', [])),
                    reading_detail=portable(d.get('agent_reading_receipt') or d.get('dossier_file') or d.get('source_pack_file') or '', audit),
                    original_paragraph_ids='|'.join(d.get('host_original_paragraph_ids', [])),
                    membership_receipt_indices='|'.join(map(str, d.get('host_membership_receipt_indices', []))))
        cases.append(case)
        signal, original = d.get('model_signal', {}), d.get('original_model_signal', {})
        models.append(dict(audit_id=ident, model_verdict=signal.get('verdict', d.get('model_classification', '')),
                           relation_verdict=signal.get('relation_verdict', ''), confidence=signal.get('confidence', ''),
                           action=signal.get('action', d.get('model_decision', '')),
                           probabilities=json.dumps(signal.get('probabilities', json.loads(d.get('model_probabilities', '{}'))), sort_keys=True, separators=(',', ':')),
                           same_subject=signal.get('same_subject', ''), original_verdict=original.get('verdict', ''),
                           original_probabilities=json.dumps(original.get('probabilities', {}), sort_keys=True, separators=(',', ':')),
                           operational_repair=d.get('repaired_operational_failure', False),
                           raw_result=portable(d.get('raw_result_file') or d.get('model_output_path') or '', audit),
                           repair_result=portable(d.get('repair_result_file') or '', audit)))
    csvwrite(output / 'cases.csv', cases, list(cases[0]))
    csvwrite(output / 'sources.csv', sorted(catalog.values(), key=lambda r: r['source_id']), ['source_id', 'path', 'locator', 'sha256', 'url'])
    csvwrite(output / 'case-sources.csv', [dict(zip(('audit_id', 'source_id', 'purpose', 'boundary_date', 'role'), r)) for r in sorted(refs)], ['audit_id', 'source_id', 'purpose', 'boundary_date', 'role'])
    csvwrite(output / 'model-signals.csv', models, list(models[0]))
    groups = []
    for g in jsonlines(audit / 'acceptance29-complex-group-reading.jsonl'):
        assert g['actual_agent_original_reading'] == 'complete' and not g['pending_record_ids']
        assert all(r in originals for r in g['records'])
        groups.append(dict(group_id=g['id'], player=g['player'], season=g['season'],
                           reasons='|'.join(g['reasons']), records='|'.join(g['records']),
                           reading_status='complete', pending_records=0))
    assert len(groups) == 1548
    csvwrite(output / 'complex-groups.csv', groups, list(groups[0]))
    for p in coverage['partitions']:
        p['path'] = portable(p['path'], audit)
    (output / 'coverage.json').write_text(json.dumps(coverage, indent=2) + '\n')
    input_names = ['acceptance29-all-case-reading.jsonl', 'acceptance29-complex-group-reading.jsonl', 'acceptance29-coverage.json', 'selection-manifest.json', 'main-dossiers.jsonl']
    input_names += [p['path'] for p in coverage['partitions']]
    manifest = dict(method='Deterministic compact publication of retained completed original-source reading receipts, not a new inspection or new model judgment.',
                    original_t4_sha256=coverage['old_t4_sha256'],
                    original_snapshot=dict(commit='474481bde27ac33887ae9f285e351735f0905571', path='docs/reports/t4/tenures.csv', locator='cases.csv csv_line is the immutable original line; audit_id embeds that line.'), selected_original_cases=len(cases),
                    complex_groups=len(groups), source_locators=len(catalog), case_source_associations=len(refs),
                    selection_seed=selection['seed'], era_frames=selection['era_frames'], class_quota=selection['class_quota'],
                    defunct_ids=selection['defunct_ids'], original_source_sha256=selection['inputs'],
                    retained_input_sha256={name: digest(audit / name) for name in input_names},
                    outcome_counts=dict(collections.Counter(c['outcome'] for c in cases)),
                    partition_counts=dict(collections.Counter(c['partition'] for c in cases)),
                    model_operational_status='Completed retained outcomes; initial oversized/invalid judgments and repaired signal pointers remain separate from source reading.',
                    limits='Old IDs and line numbers refer to the pinned old T4 CSV. Source anchors do not prove uninterrupted service. Model signals are separate and fallible. Blank source hashes are unknown, not fabricated; S1 database witness version is v238. Original source paragraphs and full receipts remain retained outside the repository.')
    (output / 'export-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps({k: manifest[k] for k in ('selected_original_cases', 'complex_groups', 'source_locators', 'case_source_associations', 'outcome_counts', 'partition_counts')}))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--audit-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/reports/historical-audit')
    args = parser.parse_args()
    export(args.audit_root.resolve(), args.output.resolve())
