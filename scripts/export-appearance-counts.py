#!/usr/bin/env python3
"""Export pinned regular-season appearance counts; no season-only edges or dates.

The runtime recomputes A+B-N and admits a relationship only when it is positive.
Ambiguous identities, summary rows, conflicting duplicates and impossible counts
remain exclusions. Source line references identify the pinned S2 v56 CSVs.
"""
import csv
import hashlib
import json
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def read(path):
    with path.open(newline='') as f:
        yield from enumerate(csv.DictReader(f), 2)

def export(data, reports):
    universe = {r['bbr_player_id'] for _, r in read(reports / 't3/player-universe.csv') if r['aba_only'] != 'Y'}
    aliases = defaultdict(set)
    for _, r in read(reports / 't3/franchise-crosswalk.csv'):
        if r['lg'] in ('NBA', 'BAA') and r['canonical_id'] not in ('UNRESOLVED', '') and not r['canonical_id'].startswith('AMBIGUOUS:'):
            aliases[(r['lg'], r['season'], r['abbreviation'])].add(r['canonical_id'])
    totals = defaultdict(list)
    team_path = data / 'sumitrodatta/Team Totals.csv'
    player_path = data / 'sumitrodatta/Player Totals.csv'
    for line, r in read(team_path):
        ids = aliases[(r['lg'], r['season'], r['abbreviation'])]
        if r['lg'] in ('NBA', 'BAA') and len(ids) == 1 and r['g'].isdigit():
            totals[(next(iter(ids)), r['season'], r['lg'])].append((int(r['g']), line))
    candidates, exclusions = defaultdict(list), []
    for line, r in read(player_path):
        if r['lg'] not in ('NBA', 'BAA'):
            continue
        ids = aliases[(r['lg'], r['season'], r['team'])]
        if len(ids) != 1:
            exclusions.append(dict(player=r['player_id'], season=r['season'], team=r['team'], reason='summary-or-unresolved-franchise', source_line=line))
            continue
        team = next(iter(ids))
        team_rows = totals[(team, r['season'], r['lg'])]
        ns = {g for g, _ in team_rows}
        if r['player_id'] not in universe or len(ns) != 1 or not r['g'].isdigit():
            reason = 'unknown-player-or-missing-conflicting-counts'
        else:
            n = next(iter(ns)); g = int(r['g'])
            reason = '' if 0 < g <= n <= 1000 else 'invalid-appearance-count'
        if reason:
            exclusions.append(dict(player=r['player_id'], season=r['season'], team=team, reason=reason, source_line=line))
            continue
        candidates[(r['player_id'], team, r['season'], r['lg'])].append(dict(
            player=r['player_id'], team=team, season=r['season'], lg=r['lg'], games=g, team_games=n,
            player_record=f'S2 Player Totals.csv:{line}', team_record=f'S2 Team Totals.csv:{min(l for _, l in team_rows)}'))
    rows = []
    for key, values in sorted(candidates.items()):
        if len({(v['games'], v['team_games']) for v in values}) != 1:
            for v in values:
                exclusions.append(dict(player=key[0], season=key[2], team=key[1], reason='conflicting-player-counts', source_line=v['player_record']))
        else:
            rows.append(values[0])
    def write(name, values, columns):
        with (reports / 't4' / name).open('w', newline='') as f:
            writer = csv.DictWriter(f, columns, lineterminator='\n'); writer.writeheader(); writer.writerows(values)
    write('appearance-counts.csv', rows, ['player', 'team', 'season', 'lg', 'games', 'team_games', 'player_record', 'team_record'])
    write('appearance-count-exclusions.csv', exclusions, ['player', 'season', 'team', 'reason', 'source_line'])
    summary = dict(source_version='S2 v56', method='A+B>N guarantees shared regular-season games; no exact dates or roster duration inferred',
                   exported_rows=len(rows), excluded_rows=len(exclusions), source_sha256={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [player_path, team_path]},
                   export_sha256=hashlib.sha256((reports / 't4/appearance-counts.csv').read_bytes()).hexdigest())
    (reports / 't4/appearance-count-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))

if __name__ == '__main__':
    export(ROOT / 'data', ROOT / 'docs/reports')
