#!/usr/bin/env python3
"""Verify pinned bulk archives and cached transaction pages without downloading."""
import argparse
import csv
import hashlib
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda: f.read(1024 * 1024), b''): h.update(b)
    return h.hexdigest()

def main(data):
    pins = json.loads((ROOT / 'docs/data/snapshot-pins.json').read_text())
    results = []
    for p in pins['archives']:
        path = data / p['artifact']
        assert path.stat().st_size == p['bytes'], path.name + ' size mismatch'
        assert sha(path) == p['sha256'], path.name + ' SHA mismatch'
        with zipfile.ZipFile(path) as z:
            entries = [i for i in z.infolist() if not i.is_dir()]
            assert len(entries) == p['files'], path.name + ' inventory mismatch'
            results.append(dict(artifact=p['artifact'], sha256=p['sha256'], files=len(entries), uncompressed_bytes=sum(i.file_size for i in entries)))
    pages = list(csv.DictReader((ROOT / 'docs/reports/t4/transaction-source-pages.csv').open()))
    assert len(pages) == 80
    for p in pages:
        path = data / 'cache/bbr' / (p['page'] + '_transactions.html')
        assert path.stat().st_size == int(p['bytes']) and sha(path) == p['sha256'], p['page'] + ' cache mismatch'
    for name, expected in pins['derived_sha256'].items():
        assert sha(ROOT / name) == expected, name + ' derived snapshot mismatch'
    print(json.dumps(dict(archives=results, cached_transaction_pages_verified=len(pages), derived_sha256=pins['derived_sha256'], network_requests=0), indent=2))

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--data-dir', type=Path, default=ROOT / 'data')
    main(p.parse_args().data_dir)
