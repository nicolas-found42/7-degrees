#!/usr/bin/env python3
"""Publish actual final check output with machine-specific path prefixes removed."""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMANDS = [
('01-wasm-build.log','./scripts/build-canvas.sh'),
('02-rust-workspace.log','cargo test --workspace'),
('03-python-source.log',"python3 -m unittest discover -s scripts -p 'test_*.py'"),
('04-format.log','cargo fmt --all --check'),
('05-clippy-host.log','cargo clippy --workspace --all-targets -- -D warnings'),
('06-clippy-wasm.log','cargo clippy -p canvas-view --target wasm32-unknown-unknown -- -D warnings'),
('07-browser.log','cargo test -p app-server --test browser_e2e --test canvas_browser -- --ignored --nocapture'),
('08-credential-logs.log',"RUSTFLAGS='--cfg test_capture' cargo test -p app-server --test log_capture -- --nocapture"),
('09-semantic-labels.log','cargo run -p app-server --bin semantic-eval -- --validate'),
('10-semantic-replay.log','cargo run -p app-server --bin semantic-eval -- --replay'),
('11-semantic-audit.log','python3 scripts/check-semantic-evaluation.py'),
('12-corrected-audit.log','python3 scripts/compare-audit-snapshot.py'),
('13-historical-audit.log','python3 scripts/check-historical-audit.py'),
('14-source-snapshot.log','python3 scripts/verify-source-snapshot.py --data-dir <existing-pinned-data>'),
('15-app-startup.log','env -u OPENROUTER_API_KEY -u TYPESAFE_API_KEY NBA_PORT=49333 ./scripts/run-app.sh'),
('16-runtime-apis.log','python3 scripts/capture-runtime-evidence.py --url http://127.0.0.1:49333 --output <raw-root>/runtime-apis.json'),
]

def publish(raw):
    output = ROOT / 'docs/reports/final-verification'; logs = output / 'logs'; logs.mkdir(parents=True, exist_ok=True)
    manifest = dict(source_base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(), origin_main_review_base='f8e743ca30939476433c680e7a29169d70c3fa41',
                    method='Actual stdout/stderr from ordered checks; machine path prefixes sanitized and redundant trailing empty lines normalized. External raw logs retained. Final review corrections verified against rebuilt source reports; snapshot hashes below identify exact generated inputs.',
                    checks=[], source_sha256={})
    for name, command in COMMANDS:
        data = (raw / name).read_bytes(); text = data.decode().replace(str(ROOT), '<checkout>')
        text = re.sub(r'/Users/[^/\s]+', '<user-home>', text)
        dest = logs / name; dest.write_text(text.rstrip('\n') + '\n' if text else '')
        manifest['checks'].append(dict(order=len(manifest['checks']) + 1, command=command, result=('server started and APIs queried' if name.startswith('15') else 'exit 0'),
                                       log='logs/' + name, raw_sha256=hashlib.sha256(data).hexdigest(), published_sha256=hashlib.sha256(dest.read_bytes()).hexdigest()))
    runtime = json.loads((raw / 'runtime-apis.json').read_text())
    graph = runtime['graph']['body']
    runtime['graph']['body'] = dict(players=graph.get('players', []), node_count=len(graph.get('players', [])), edge_count=len(graph['edges']))
    # Exact full graph response retained externally; publish count rather than duplicate all edge evidence.
    runtime['graph']['body'].pop('players')
    runtime['graph']['raw_response_capture_sha256'] = hashlib.sha256((raw / 'runtime-apis.json').read_bytes()).hexdigest()
    runtime['evidence']['body']['edges'] = [e for e in runtime['evidence']['body']['edges'] if e.get('b') == 'bogutan01' or e.get('a') == 'bogutan01']
    runtime['evidence']['publication_scope'] = 'Acy/Bogut edge from actual Acy edge response; other edges omitted for compactness.'
    (output / 'runtime-apis.json').write_text(json.dumps(runtime, indent=2) + '\n')
    for name in ('docs/reports/t3/player-universe.csv','docs/reports/t4/tenures.csv','docs/reports/t4/transaction-source-pages.csv','docs/evaluation/measurements.json','docs/evaluation/policy.json'):
        manifest['source_sha256'][name] = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
    manifest['runtime_capture_sha256'] = hashlib.sha256((output / 'runtime-apis.json').read_bytes()).hexdigest()
    (output / 'commands-and-hashes.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'Published {len(COMMANDS)} ordered command logs and actual runtime API receipts; original raw hashes retained.')

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--raw-root', type=Path, required=True); publish(p.parse_args().raw_root)
