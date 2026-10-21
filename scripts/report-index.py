#!/usr/bin/env python3
"""Regenerate the published-report index from the committed receipts.

The index is derived evidence: every row is read back out of a committed
receipt (a `manifest.json` or the older `commands-and-hashes.json`), so the
committed index can always be re-derived and compared. `--check` fails when
the committed index differs from what this script regenerates.
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORTS = ROOT / 'docs/reports'
INDEX = REPORTS / 'index.json'
README = REPORTS / 'README.md'
DATE_IN_NAME = re.compile(r'(\d{4}-\d{2}-\d{2})')


def relative(path, reports=REPORTS):
    """The receipt path, relative to the checkout when it lives inside it."""
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.relative_to(reports).as_posix()


def run_label(path, reports=REPORTS):
    """The published run a receipt belongs to, relative to the reports tree."""
    return path.parent.relative_to(reports).as_posix()


def manifest_row(path, reports=REPORTS):
    """One row per published run described by a `manifest.json` receipt."""
    data = json.loads(path.read_text())
    checks = data.get('checks', [])
    outcome = data.get('outcome')
    if outcome not in ('passed', 'failed'):
        outcome = 'passed' if checks and all(c.get('exit_code') == 0 for c in checks) else 'failed'
    stamp = data.get('finished_at') or data.get('started_at') or ''
    return {
        'path': relative(path, reports),
        'run': run_label(path, reports),
        'kind': 'manifest',
        'profile': data.get('profile'),
        'source_commit': data.get('source_commit'),
        'outcome': outcome,
        'date': stamp[:10] or None,
        'checks': len(checks),
    }


def legacy_row(path, reports=REPORTS):
    """One row per published run described by the older commands receipt."""
    data = json.loads(path.read_text())
    checks = data.get('checks', [])
    outcome = 'passed' if checks and all('exit 0' in str(c.get('result', '')) for c in checks) else 'failed'
    name = path.parent.name
    found = DATE_IN_NAME.search(name)
    return {
        'path': relative(path, reports),
        'run': run_label(path, reports),
        'kind': 'commands-and-hashes',
        'profile': None,
        'source_commit': data.get('source_base_commit'),
        'outcome': outcome,
        'date': found.group(1) if found else None,
        'checks': len(checks),
    }


def collect(reports=REPORTS):
    """Every published run under the reports tree, in a stable order."""
    rows = []
    for path in sorted(reports.rglob('manifest.json')):
        rows.append(manifest_row(path, reports))
    for path in sorted(reports.rglob('commands-and-hashes.json')):
        rows.append(legacy_row(path, reports))
    return sorted(rows, key=lambda row: row['path'])


def render_index(rows):
    return json.dumps({'schema_version': 1, 'runs': rows}, indent=2) + '\n'


def render_readme(rows):
    lines = [
        '# Published verification reports',
        '',
        'Every row is derived from a committed receipt; `python3 scripts/report-index.py --check`',
        'fails when this index does not match the receipts in this tree.',
        '',
        '| Run | Profile | Outcome | Date | Source commit | Receipt |',
        '| --- | --- | --- | --- | --- | --- |',
    ]
    for row in rows:
        lines.append('| `{run}` | {profile} | {outcome} | {date} | `{commit}` | `{path}` |'.format(
            run=row['run'],
            profile=row['profile'] or '-',
            outcome=row['outcome'],
            date=row['date'] or '-',
            commit=(row['source_commit'] or 'unknown')[:12],
            path=row['path']))
    lines.append('')
    return '\n'.join(lines)


def regenerate(reports=REPORTS):
    """Write the index and the human-readable index; return their content."""
    rows = collect(reports)
    index = render_index(rows)
    readme = render_readme(rows)
    (reports / 'index.json').write_text(index)
    (reports / 'README.md').write_text(readme)
    return index, readme


def check(reports=REPORTS):
    """Fail when the committed index differs from the regenerated one."""
    rows = collect(reports)
    index = render_index(rows)
    readme = render_readme(rows)
    problems = []
    committed_index = reports / 'index.json'
    committed_readme = reports / 'README.md'
    if not committed_index.is_file():
        problems.append(f'missing {relative(committed_index, reports)}')
    elif committed_index.read_text() != index:
        problems.append(f'{relative(committed_index, reports)} does not match the receipts in this tree')
    if not committed_readme.is_file():
        problems.append(f'missing {relative(committed_readme, reports)}')
    elif committed_readme.read_text() != readme:
        problems.append(f'{relative(committed_readme, reports)} does not match the receipts in this tree')
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        print('Run: python3 scripts/report-index.py', file=sys.stderr)
        return 1
    print(f'Report index matches {len(rows)} published runs')
    return 0


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true', help='Fail when the committed index is stale')
    args = p.parse_args()
    if args.check:
        sys.exit(check())
    regenerate()
    print(f'Regenerated {relative(INDEX)} and {relative(README)}')
