#!/usr/bin/env python3
"""Recover low-appearance links from pinned, dated NBA player events.

Targets are computed from appearance counts alone (before witness import), so
rebuilding remains deterministic after these players become connected. Events
prove participation in one particular team game, never a full-day roster span.
"""
import csv
import hashlib
import itertools
import json
import sqlite3
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def read(path):
    with path.open(newline='') as f:
        return list(csv.DictReader(f))
def numeric(value):
    try: return str(int(float(value)))
    except (ValueError, TypeError, OverflowError): return ''
def export():
    reports = ROOT / 'docs/reports'
    players = {r['bbr_player_id']: r for r in read(reports / 't3/player-universe.csv') if r['aba_only'] != 'Y'}
    ids = defaultdict(set)
    for p, row in players.items():
        if row['s1_player_id'].isdigit(): ids[row['s1_player_id']].add(p)
    bridge = {i: next(iter(ps)) for i, ps in ids.items() if len(ps) == 1}
    by_team = defaultdict(list)
    for r in read(reports / 't4/appearance-counts.csv'): by_team[(r['team'],r['season'])].append(r)
    connected = set()
    for group in by_team.values():
        for a in group:
            if any(a['player'] != b['player'] and int(a['games']) + int(b['games']) > int(a['team_games']) for b in group): connected.add(a['player'])
    targets = set(players) - connected
    target_ids = [i for i, p in sorted(bridge.items()) if p in targets]
    aliases = defaultdict(set)
    for r in read(reports / 't3/franchise-crosswalk.csv'):
        if r['lg'] in ('NBA','BAA'): aliases[(int(r['season']),r['abbreviation'])].add(r['canonical_id'])
    membership = {(r['bbr_player_id'],r['canonical_franchise'],int(r['season'])) for r in read(reports / 't4/tenures.csv')}
    db = ROOT / 'data/nba.sqlite'
    con = sqlite3.connect('file:'+str(db)+'?mode=ro',uri=True)
    placeholders = ','.join('?' for _ in target_ids)
    query = 'select distinct game_id from play_by_play where ' + ' or '.join(f'player{i}_id in ({placeholders})' for i in (1,2,3))
    games = sorted(r[0] for r in con.execute(query,target_ids*3)) if target_ids else []
    contexts = defaultdict(set)
    for gid, date, season_id, ht, ha, vt, va in con.execute("select game_id,game_date,season_id,team_id_home,team_abbreviation_home,team_id_away,team_abbreviation_away from game where season_type in ('Regular Season','Playoffs')"):
        season = int(season_id[1:5])+1
        for team_id, abbreviation in [(ht,ha),(vt,va)]:
            if abbreviation == 'SAN': abbreviation = 'SAS'
            franchises = aliases[(season, abbreviation)]
            if len(franchises) == 1:
                contexts[(gid,numeric(team_id))].add((date[:10],season,next(iter(franchises))))
    participants = defaultdict(dict)
    columns = ','.join(f'person{i}type,player{i}_id,player{i}_team_id' for i in (1,2,3))
    if games:
        event_query = f'select game_id,eventnum,{columns} from play_by_play where game_id in ({",".join("?" for _ in games)}) and eventmsgtype in (1,2,3,4,5,8)'
        for row in con.execute(event_query,games):
            gid, event = row[:2]
            for slot in (1,2,3):
                kind, pid, tid = row[2+(slot-1)*3:5+(slot-1)*3]
                p = bridge.get(numeric(pid))
                candidates = contexts[(gid,numeric(tid))]
                if kind not in (4,5) or not p or len(candidates) != 1: continue
                date, season, team = next(iter(candidates))
                if (p,team,season) not in membership: continue
                source = f'S1 play_by_play:game:{gid}:event:{event}:player{slot}'
                key = (gid,date,season,team)
                previous = participants[key].get(p)
                if previous is None or (int(event),slot) < previous[0]: participants[key][p] = ((int(event),slot),source)
    con.close()
    witnesses = {}
    recovered = set()
    for (gid,date,season,team), ps in sorted(participants.items()):
        for a,b in itertools.combinations(sorted(ps),2):
            if a not in targets and b not in targets: continue
            key = (a,b,team,season)
            witness = dict(a=a,b=b,team=team,season=season,game_id=gid,date=date,a_source=ps[a][1],b_source=ps[b][1])
            previous = witnesses.get(key)
            if previous is None or (date,gid) < (previous['date'],previous['game_id']): witnesses[key] = witness
            recovered.update({a,b} & targets)
    with (reports / 't4/game-witnesses.csv').open('w',newline='') as f:
        writer = csv.DictWriter(f,['a','b','team','season','game_id','date','a_source','b_source'],lineterminator='\n');writer.writeheader();writer.writerows(witnesses[k] for k in sorted(witnesses))
    summary = dict(method='Unique canonical NBA identities with player-action events on the same canonical team in the same dated official game; no full roster interval inferred',
        target_players_without_count_proof=len(targets), targets_with_unique_nba_ids=len(target_ids), source_games_considered=len(games), witness_records=len(witnesses),
        target_players_with_dated_witness=len(recovered), remaining_without_dated_witness=[dict(id=p,name=players[p]['display_name']) for p in sorted(targets-recovered)],
        source_version='S1 v238', sqlite_sha256=hashlib.file_digest(db.open('rb'),'sha256').hexdigest(),
        export_sha256=hashlib.sha256((reports / 't4/game-witnesses.csv').read_bytes()).hexdigest())
    (reports / 't4/game-witness-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k!='remaining_without_dated_witness'}))
if __name__ == '__main__': export()
