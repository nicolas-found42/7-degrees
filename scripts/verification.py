"""Recorded check execution and compact, deterministic review evidence."""
import datetime
import hashlib
import json
import os
import platform
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESULT = re.compile(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;')

# Environment names captured (never their values) so a receipt records which
# configuration was present without leaking any credential material.
RELEVANT_ENV_NAMES = (
    'BROWSER_EXECUTABLE', 'BROWSER_NODE', 'CARGO_TARGET_DIR', 'CI', 'GITHUB_ACTIONS',
    'GITHUB_REF', 'GITHUB_REPOSITORY', 'GITHUB_RUN_ID', 'GITHUB_SERVER_URL',
    'GITHUB_SHA', 'GITHUB_WORKFLOW', 'NBA_BUILD_COMMIT', 'NBA_CANVAS_ASSET_DIR',
    'NBA_DATA_MODE', 'NBA_PORT', 'NBA_REPORT_DIR', 'NBA_RUNTIME_LOG_PATH',
    'OPENROUTER_API_KEY', 'PLAYWRIGHT_PACKAGE_PATH', 'TYPESAFE_API_KEY',
)


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


def tool_version(argv):
    """First line of a tool's version output, or None when the tool is absent."""
    try:
        done = subprocess.run(argv, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    if done.returncode != 0:
        return None
    lines = done.stdout.strip().splitlines()
    return lines[0].strip() if lines else None


def environment():
    """Best-effort toolchain and platform facts for the receipt."""
    browser = os.environ.get('BROWSER_EXECUTABLE')
    return {
        'os': platform.system(),
        'arch': platform.machine(),
        'platform': platform.platform(),
        'python': platform.python_version(),
        'rustc': tool_version(['rustc', '--version']),
        'cargo': tool_version(['cargo', '--version']),
        'node': tool_version(['node', '--version']),
        'browser': tool_version([browser, '--version']) if browser else None,
        'env_names': sorted(name for name in RELEVANT_ENV_NAMES if os.environ.get(name)),
    }


def ci():
    """GitHub Actions coordinates; every field is None outside a CI run."""
    server = os.environ.get('GITHUB_SERVER_URL')
    repository = os.environ.get('GITHUB_REPOSITORY')
    run_id = os.environ.get('GITHUB_RUN_ID')
    run_url = f'{server}/{repository}/actions/runs/{run_id}' if server and repository and run_id else None
    return {
        'run_id': os.environ.get('GITHUB_RUN_ID'),
        'run_url': run_url,
        'workflow': os.environ.get('GITHUB_WORKFLOW'),
        'repository': os.environ.get('GITHUB_REPOSITORY'),
        'sha': os.environ.get('GITHUB_SHA'),
        'ref': os.environ.get('GITHUB_REF'),
    }


def artifact_directory(output, name):
    """The per-check artifact directory a check may write its evidence into."""
    return output / f'{name}.artifacts'


def collect_artifacts(output, name):
    """Hash every file a check produced under its artifact directory."""
    root = artifact_directory(output, name)
    if not root.is_dir():
        return []
    found = []
    for path in sorted(root.rglob('*')):
        if path.is_file():
            found.append({'path': path.relative_to(output).as_posix(), 'sha256': digest(path)})
    return found


def run_checks(commands, output, cwd=ROOT, profile='custom'):
    """Run every check, record receipts, and keep the failure evidence.

    A command is ``(name, argv)`` or ``(name, argv, env)``. The optional
    environment mapping may use the ``{artifacts}`` placeholder, which is
    replaced with the check's artifact directory. Every check additionally
    receives ``NBA_CHECK_ARTIFACTS`` pointing at that directory.
    """
    output.mkdir(parents=True, exist_ok=False)
    manifest = {'schema_version': 1, 'profile': profile, 'source_commit': subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=cwd, text=True).strip(),
        'working_tree_status': subprocess.check_output(['git', 'status', '--porcelain'], cwd=cwd, text=True),
        'working_tree_diff_sha256': hashlib.sha256(subprocess.check_output(
            ['git', 'diff', 'HEAD', '--binary'], cwd=cwd)).hexdigest(),
        'untracked_sha256': {name: digest(cwd / name) for name in subprocess.check_output(
            ['git', 'ls-files', '--others', '--exclude-standard'], cwd=cwd, text=True).splitlines()
            if (cwd / name).is_file()},
        'started_at': utc_now(), 'checks': [], 'completed': False, 'artifacts': [],
        'environment': environment(), 'ci': ci()}
    status = 0
    for command in commands:
        name, argv = command[0], list(command[1])
        overrides = command[2] if len(command) > 2 else {}
        artifacts = artifact_directory(output, name)
        artifacts.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ)
        env['NBA_CHECK_ARTIFACTS'] = str(artifacts)
        for key, value in overrides.items():
            env[key] = value.replace('{artifacts}', str(artifacts)) if isinstance(value, str) else value
        resolved = [a.replace('{artifacts}', str(artifacts)) if isinstance(a, str) else a for a in argv]
        print(f'Checking {name}', flush=True)
        path = output / f'{name}.log'
        started = utc_now()
        with path.open('wb') as log:
            try:
                result = subprocess.run(resolved, cwd=cwd, stdout=log, stderr=subprocess.STDOUT, env=env)
                code = result.returncode
            except OSError as error:
                log.write(str(error).encode()); code = 127
        produced = collect_artifacts(output, name)
        manifest['checks'].append({'name': name, 'argv': argv, 'exit_code': code,
            'status': 'passed' if code == 0 else 'failed',
            'started_at': started, 'finished_at': utc_now(), 'log': path.name,
            'sha256': digest(path), 'artifacts': [a['path'] for a in produced],
            'summary': summarize(path.read_text(errors='replace'))})
        manifest['artifacts'].extend(produced)
        if code:
            if not status:
                status = code
            print(f'{name} failed (exit {code}); see {path}', flush=True)
    manifest['completed'] = len(manifest['checks']) == len(commands)
    manifest['outcome'] = 'passed' if all(c['exit_code'] == 0 for c in manifest['checks']) else 'failed'
    manifest['failed_checks'] = [c['name'] for c in manifest['checks'] if c['exit_code'] != 0]
    manifest['expected_checks'] = [command[0] for command in commands]
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


def validate_receipts(raw, allow_failed=False):
    """Validate a receipt directory: structure, log hashes, summaries, artifacts.

    ``allow_failed`` accepts a run with non-zero exit codes (a failure
    publication); every other integrity rule still applies.
    """
    manifest = json.loads((raw / 'manifest.json').read_text())
    checks = manifest['checks']
    if (manifest.get('schema_version') != 1 or manifest.get('completed') is not True
            or not checks or [c['name'] for c in checks] != manifest['expected_checks']):
        raise ValueError('Incomplete or invalid verification manifest')
    for check in checks:
        name = check['log']
        if Path(name).name != name or not check['argv']:
            raise ValueError(f'Invalid receipt: {name}')
        if 'status' in check and check['status'] != ('passed' if check['exit_code'] == 0 else 'failed'):
            raise ValueError(f'Receipt status disagrees with its exit code: {name}')
        if digest(raw / name) != check['sha256']:
            raise ValueError(f'Log hash mismatch: {name}')
        if summarize((raw / name).read_text(errors='replace')) != check['summary']:
            raise ValueError(f'Log summary mismatch: {name}')
    for artifact in manifest.get('artifacts', []):
        path = raw / artifact['path']
        if not path.is_file():
            raise ValueError(f'Missing artifact: {artifact["path"]}')
        if digest(path) != artifact['sha256']:
            raise ValueError(f'Artifact hash mismatch: {artifact["path"]}')
    if not allow_failed and any(c['exit_code'] != 0 for c in checks):
        raise ValueError('Failed receipts cannot be published as successful evidence')
    return manifest
