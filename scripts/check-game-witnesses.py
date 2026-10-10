#!/usr/bin/env python3
"""Independently resolve every dated-link certificate against original NBA rows."""
import csv
import json
import re
import sqlite3
from collections import defaultdict
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
def read(path):
    with path.open(newline='') as f: return list(csv.DictReader(f))
def numeric(x):
    return str(int(float(x)))
def verify():
    players = {r['bbr_player_id']:r for r in read(ROOT/'docs/reports/t3/player-universe.csv')}
    aliases = {(int(r['season']),r['abbreviation']):r['canonical_id'] for r in read(ROOT/'docs/reports/t3/franchise-crosswalk.csv') if r['lg'] in ('NBA','BAA')}
    witnesses = read(ROOT/'docs/reports/t4/game-witnesses.csv')
    games = sorted({r['game_id'] for r in witnesses})
    con = sqlite3.connect('file:'+str(ROOT/'data/nba.sqlite')+'?mode=ro',uri=True)
    events = {}
    columns = ','.join(f'person{i}type,player{i}_id,player{i}_team_id' for i in (1,2,3))
    for r in con.execute(f'select game_id,eventnum,eventmsgtype,{columns} from play_by_play where game_id in ({",".join("?" for _ in games)})',games):
        key = (r[0],int(r[1]))
        assert key not in events or events[key] == r, 'conflicting official event rows'
        events[key] = r
    contexts = defaultdict(set)
    for gid,date,sid,hi,ha,vi,va in con.execute("select game_id,game_date,season_id,team_id_home,team_abbreviation_home,team_id_away,team_abbreviation_away from game where season_type in ('Regular Season','Playoffs')"):
        if gid not in games: continue
        season=int(sid[1:5])+1
        for ti,abbr in [(hi,ha),(vi,va)]:
            if abbr=='SAN':abbr='SAS'
            if (season,abbr) in aliases: contexts[(gid,numeric(ti))].add((date[:10],season,aliases[(season,abbr)]))
    for w in witnesses:
        assert w['a'] < w['b']
        observed_teams=[]
        for player,source in [(w['a'],w['a_source']),(w['b'],w['b_source'])]:
            match=re.fullmatch(r'S1 play_by_play:game:(\d{10}):event:(\d+):player([123])',source)
            assert match, source
            gid,event,slot=match.groups();slot=int(slot)
            assert gid==w['game_id']
            r=events[(gid,int(event))]
            assert r[2] in (1,2,3,4,5,8), 'non-player action cannot prove membership'
            kind,pid,team=r[3+(slot-1)*3:6+(slot-1)*3]
            assert kind in (4,5)
            assert numeric(pid)==players[player]['s1_player_id']
            observed_teams.append(numeric(team))
        assert observed_teams[0]==observed_teams[1], 'opponents are not teammates'
        assert contexts[(w['game_id'],observed_teams[0])] == {(w['date'],int(w['season']),w['team'])}
    con.close()
    print(json.dumps(dict(verified_witness_records=len(witnesses), verified_source_games=len(games), unsupported_witnesses=0)))
if __name__=='__main__': verify()
