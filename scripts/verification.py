"""Recorded check execution and compact, deterministic review evidence."""
import datetime
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESULT = re.compile(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def summarize(text):
    groups = RESULT.findall(text)
    totals = dict(zip(('passed', 'failed', 'ignored'), map(sum, zip(*(map(int, g) for g in groups))))) if groups else None
    names = re.findall(r'^test (.+?) \.\.\. (ok|FAILED|ignored[^\n]*)$', text, re.M)
    python = re.search(r'Ran (\d+) tests? in .*\n\s*\n(OK[^\n]*|FAILED[^\n]*)', text)
    return {'rust_result_groups': len(groups), 'rust_totals': totals,
            'test_results': [{'name': n, 'result': r} for n, r in names],
            'python_result': {'tests': int(python[1]), 'result': python[2]} if python else None}


def run_checks(commands, output, cwd=ROOT, profile='custom'):
    output.mkdir(parents=True, exist_ok=False)
    manifest = {'schema_version': 1, 'profile': profile, 'source_commit': subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=cwd, text=True).strip(),
        'working_tree_status': subprocess.check_output(['git', 'status', '--porcelain'], cwd=cwd, text=True),
        'working_tree_diff_sha256': hashlib.sha256(subprocess.check_output(
            ['git', 'diff', 'HEAD', '--binary'], cwd=cwd)).hexdigest(),
        'untracked_sha256': {name: digest(cwd / name) for name in subprocess.check_output(
            ['git', 'ls-files', '--others', '--exclude-standard'], cwd=cwd, text=True).splitlines()
            if (cwd / name).is_file()},
        'started_at': utc_now(), 'checks': [], 'completed': False}
    status = 0
    for name, argv in commands:
        print(f'Checking {name}', flush=True)
        path = output / f'{name}.log'
        started = utc_now()
        with path.open('wb') as log:
            try:
                result = subprocess.run(argv, cwd=cwd, stdout=log, stderr=subprocess.STDOUT)
                code = result.returncode
            except OSError as error:
                log.write(str(error).encode()); code = 127
        manifest['checks'].append({'name': name, 'argv': argv, 'exit_code': code,
            'started_at': started, 'finished_at': utc_now(), 'log': path.name,
            'sha256': digest(path), 'summary': summarize(path.read_text(errors='replace'))})
        status = code
        if code:
            print(f'{name} failed (exit {code}); see {path}', flush=True)
            break
    manifest['completed'] = len(manifest['checks']) == len(commands)
    manifest['expected_checks'] = [name for name, _ in commands]
    manifest['finished_at'] = utc_now()
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    # Compact evidence excludes full test-name lists; raw receipts remain accessible.
    compact = review_summary(manifest)
    (output / 'review-summary.json').write_text(json.dumps(compact, indent=2) + '\n')
    return status


def review_summary(manifest):
    compact = {k: v for k, v in manifest.items() if k != 'checks'}
    compact['checks'] = [{**c, 'summary': {k: v for k, v in c['summary'].items() if k != 'test_results'}} for c in manifest['checks']]
    return compact


def validate_receipts(raw):
    manifest = json.loads((raw / 'manifest.json').read_text())
    checks = manifest['checks']
    if (manifest.get('schema_version') != 1 or manifest.get('completed') is not True
            or not checks or [c['name'] for c in checks] != manifest['expected_checks']):
        raise ValueError('Incomplete or invalid verification manifest')
    for check in checks:
        name = check['log']
        if Path(name).name != name or check['exit_code'] != 0 or not check['argv']:
            raise ValueError(f'Invalid or failed receipt: {name}')
        if digest(raw / name) != check['sha256']:
            raise ValueError(f'Log hash mismatch: {name}')
        if summarize((raw / name).read_text(errors='replace')) != check['summary']:
            raise ValueError(f'Log summary mismatch: {name}')
    return manifest
