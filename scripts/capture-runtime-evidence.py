#!/usr/bin/env python3
"""Capture actual full-snapshot API facts from a locally started app (no model calls)."""
import argparse
import hashlib
import json
import platform
import time
import urllib.request
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--url', default='http://127.0.0.1:3000')
p.add_argument('--output', type=Path, required=True)
a = p.parse_args(); out = {}
for name, url in [('cold_stats','/api/stats'),('cached_stats','/api/stats'),('graph','/api/graph'),('coverage','/api/coverage'),('connection','/api/connection?from=acyqu01&to=bogutan01'),('evidence','/api/edges/acyqu01'),('postseason_only_coverage','/api/coverage/nba:1630492'),('semantic_status','/api/semantic-status')]:
    start = time.perf_counter()
    with urllib.request.urlopen(a.url + url) as r: value = json.load(r)
    out[name] = dict(url=url, elapsed_seconds=time.perf_counter() - start, body=value)
with urllib.request.urlopen(a.url + '/') as r: home = r.read()
with urllib.request.urlopen(a.url + '/assets/canvas_view_bg.wasm') as r: wasm = r.read()
assert wasm[:4] == b'\0asm'
s = out['cold_stats']['body']; g = out['graph']['body']; c = out['coverage']['body']
assert [s[k] for k in ('players','components','diameter','unreachable_pairs')] == [5106,4168,20,12975882]
assert len(g['players']) == 5106 and len(g['edges']) == 1517
assert sum(s['histogram'].values()) == 57183
assert s['unreachable_pairs'] + sum(s['histogram'].values()) == 5106 * 5105 // 2
assert c['certified_tenures'] == 2227 and c['players_without_certified_tenure'] == 3537 and not c['complete']
assert out['connection']['body']['degree'] == 1
out['machine'] = dict(os=platform.system(), release=platform.release(), architecture=platform.machine(), python=platform.python_version(), qualification='Single localhost debug-server samples on this machine; cold_stats is cold only when run first after server startup. Not a portable latency guarantee.')
out['served_assets'] = dict(wasm_sha256=hashlib.sha256(wasm).hexdigest(), wasm_bytes=len(wasm), home_bytes=len(home), home_semantic_unavailable_visible=b'Semantic features (Jev): unavailable' in home)
assert out['served_assets']['home_semantic_unavailable_visible']
a.output.write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps(dict(nodes=len(g['players']), edges=len(g['edges']), components=s['components'], diameter=s['diameter'], reachable_pairs=sum(s['histogram'].values()), unreachable_pairs=s['unreachable_pairs'], cold_stats_seconds=out['cold_stats']['elapsed_seconds'], cached_stats_seconds=out['cached_stats']['elapsed_seconds'], semantic_status=out['semantic_status']['body'], wasm_bytes=len(wasm))))
