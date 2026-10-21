#!/usr/bin/env python3
"""Boot the app server and capture the runtime diagnostics it reports.

The capture is deterministic: it needs no provider key, only the committed
report snapshot. It writes the collected bodies plus their elapsed times to
``runtime-diagnostics.json`` and keeps the server's JSONL request log next
to it, so a failed run carries the evidence needed to diagnose it.
"""
import argparse
import datetime
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# The endpoints a reader needs to reconstruct what the server actually served.
ENDPOINTS = {
    'stats': '/api/stats',
    'coverage': '/api/coverage',
    'connection': '/api/connection?from=acyqu01&to=bogutan01',
}
READINESS = '/api/semantic-status'
# A semantic judgment is provoked so the diagnostics carry semantic events.
SEMANTIC_PROBE = '/api/query?q=who%20played%20with%20Magic%20Johnson'
# The stable failure codes a semantic event may carry.
SEMANTIC_FAILURES = (
    'unconfigured', 'deadline_exceeded', 'provider_unreachable', 'reply_too_large',
    'reply_incomplete', 'malformed_envelope', 'payload_rejected',
)
COMMIT = re.compile(r'^[0-9a-f]{40}$')


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


def server_binary(target_dir):
    """The app-server binary, built from the current sources."""
    candidate = Path(target_dir) / 'debug' / 'app-server'
    done = subprocess.run(['cargo', 'build', '-p', 'app-server', '--locked'], cwd=ROOT)
    if done.returncode != 0:
        raise SystemExit('could not build the app-server binary; the capture needs a working server')
    if not candidate.is_file():
        raise SystemExit('app-server binary missing after cargo build')
    return candidate


def fetch(url, timeout=30):
    """Return (status, body, elapsed_seconds) for one endpoint."""
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            raw = response.read()
            status = response.status
    except urllib.error.HTTPError as error:
        raw = error.read()
        status = error.code
    elapsed = time.perf_counter() - started
    try:
        body = json.loads(raw)
    except (ValueError, UnicodeDecodeError):
        body = raw.decode('utf-8', 'replace')
    return status, body, elapsed


def post_beacon(base_url):
    """Post a client-side beacon so the client channel is exercised too."""
    payload = json.dumps({'kind': 'verification.capture', 'message': 'runtime diagnostics capture',
                          'url': base_url, 'context': {'source': 'scripts/capture-runtime-diagnostics.py'}}).encode()
    request = urllib.request.Request(base_url + '/api/diagnostics/client', data=payload,
                                     headers={'Content-Type': 'application/json'}, method='POST')
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            raw, status = response.read(), response.status
    except urllib.error.HTTPError as error:
        raw, status = error.read(), error.code
    elapsed = time.perf_counter() - started
    try:
        body = json.loads(raw)
    except (ValueError, UnicodeDecodeError):
        body = raw.decode('utf-8', 'replace')
    return status, body, elapsed


def validate_diagnostics(diagnostics):
    """Fail loudly when the diagnostics body breaks the agreed contract."""
    if not isinstance(diagnostics, dict):
        raise SystemExit('/api/diagnostics did not return a JSON object')
    build = diagnostics.get('build') or {}
    commit = build.get('commit')
    if not isinstance(commit, str) or not COMMIT.match(commit):
        raise SystemExit(f'/api/diagnostics build.commit is not a 40-hex commit: {commit!r}')
    if not isinstance(build.get('dirty'), bool):
        raise SystemExit('/api/diagnostics build.dirty is not a boolean')
    if build.get('mode') not in ('fixture', 'reports'):
        raise SystemExit(f'/api/diagnostics build.mode is unknown: {build.get("mode")!r}')
    requests = diagnostics.get('requests') or {}
    if not isinstance(requests.get('total'), int):
        raise SystemExit('/api/diagnostics requests.total is not a number')
    for record in requests.get('recent') or []:
        if '?' in str(record.get('path', '')):
            raise SystemExit('/api/diagnostics request paths must not carry a query string')
    snapshot = diagnostics.get('snapshot') or {}
    if not isinstance(snapshot.get('players'), int) or snapshot.get('players') < 1:
        raise SystemExit('/api/diagnostics snapshot.players is not a positive number')
    if not isinstance(snapshot.get('edges'), int) or snapshot.get('edges') < 0:
        raise SystemExit('/api/diagnostics snapshot.edges is not a number')
    for section in ('sources', 'assets'):
        if not isinstance(snapshot.get(section), list):
            raise SystemExit(f'/api/diagnostics snapshot.{section} is not a list')
    semantic = diagnostics.get('semantic') or {}
    if semantic.get('state') not in ('available', 'degraded', 'unconfigured'):
        raise SystemExit(f'/api/diagnostics semantic.state is unknown: {semantic.get("state")!r}')
    events = semantic.get('events')
    if not isinstance(events, list) or not events:
        raise SystemExit('/api/diagnostics reported no semantic event; the capture cannot prove telemetry')
    for event in events:
        failure = event.get('failure')
        if failure is not None and failure not in SEMANTIC_FAILURES \
                and not str(failure).startswith('provider_http_status_'):
            raise SystemExit(f'/api/diagnostics reported an unknown semantic failure: {failure!r}')
        for forbidden in ('model', 'request_id', 'input_tokens', 'output_tokens', 'cost_usd'):
            if forbidden in event:
                raise SystemExit(f'/api/diagnostics semantic events must not carry {forbidden}')
    return diagnostics


def validate_request_log(text):
    """Every line of the server's request log must be a JSON object."""
    lines = [line for line in text.splitlines() if line.strip()]
    if not lines:
        raise SystemExit('the server wrote no runtime request log')
    for line in lines:
        try:
            record = json.loads(line)
        except ValueError:
            raise SystemExit(f'the runtime request log holds a line that is not JSON: {line[:120]!r}')
        if not isinstance(record, dict):
            raise SystemExit('the runtime request log holds a line that is not a JSON object')
    return lines


def wait_until_ready(base_url, process, timeout):
    """Poll the readiness endpoint until it answers or the server dies."""
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise SystemExit(f'app server exited before becoming ready (exit {process.returncode})')
        try:
            status, body, _ = fetch(base_url + READINESS, timeout=5)
            if status == 200:
                return body
            last = f'{READINESS} answered {status}'
        except (urllib.error.URLError, OSError) as error:
            last = str(error)
        time.sleep(0.25)
    raise SystemExit(f'app server did not answer {READINESS} within {timeout}s: {last}')


def main(artifacts, port=None, timeout=120, target_dir=None):
    artifacts = Path(artifacts).resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    target_dir = Path(target_dir or os.environ.get('CARGO_TARGET_DIR') or ROOT / 'target')
    binary = server_binary(target_dir)
    port = port or free_port()
    # The server writes its request log into the caller's artifact directory;
    # the capture copies it to the artifact root so the receipt carries the
    # log the run produced.
    server_log_dir = artifacts / 'server'
    server_log_dir.mkdir(parents=True, exist_ok=True)
    log_path = server_log_dir / 'runtime-request-log.jsonl'
    env = dict(os.environ)
    env.pop('TYPESAFE_API_KEY', None)
    env.pop('OPENROUTER_API_KEY', None)
    env.update({
        'NBA_PORT': str(port),
        'NBA_DATA_MODE': 'reports',
        'NBA_REPORT_DIR': str(ROOT / 'docs/reports'),
        'NBA_RUNTIME_LOG_PATH': str(log_path),
        'NBA_BUILD_COMMIT': git('rev-parse', 'HEAD'),
    })
    server = subprocess.Popen([str(binary)], cwd=ROOT, env=env,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    output_lines = []

    def drain():
        for line in server.stdout:
            output_lines.append(line)

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    base_url = f'http://127.0.0.1:{port}'
    try:
        wait_until_ready(base_url, server, timeout)
        collected = {}
        endpoints = {}
        # A semantic judgment first: the diagnostics then carry its event.
        status, body, elapsed = fetch(base_url + SEMANTIC_PROBE)
        endpoints['semantic_probe'] = {'url': SEMANTIC_PROBE, 'status': status, 'elapsed_seconds': elapsed}
        collected['semantic_probe'] = body
        for name, path in ENDPOINTS.items():
            status, body, elapsed = fetch(base_url + path)
            if status != 200:
                raise SystemExit(f'{path} answered {status}, expected 200')
            collected[name] = body
            endpoints[name] = {'url': path, 'status': status, 'elapsed_seconds': elapsed}
        status, body, elapsed = fetch(base_url + '/api/diagnostics')
        if status != 200:
            raise SystemExit(f'/api/diagnostics answered {status}, expected 200')
        collected['diagnostics'] = validate_diagnostics(body)
        endpoints['diagnostics'] = {'url': '/api/diagnostics', 'status': status, 'elapsed_seconds': elapsed}
        status, body, elapsed = post_beacon(base_url)
        if status != 202:
            raise SystemExit(f'/api/diagnostics/client answered {status}, expected 202')
        endpoints['client_beacon'] = {'url': '/api/diagnostics/client', 'status': status, 'elapsed_seconds': elapsed}
        collected['client_beacon'] = body
    finally:
        server.terminate()
        try:
            server.wait(timeout=15)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=15)
        reader.join(timeout=5)
    (artifacts / 'server.log').write_text(''.join(output_lines))
    report = {
        'generated_at': utc_now(),
        'source_commit': env['NBA_BUILD_COMMIT'],
        'base_url': base_url,
        'endpoints': endpoints,
        'elapsed_seconds': {name: entry['elapsed_seconds'] for name, entry in endpoints.items()},
        **{name: body for name, body in collected.items() if name != 'diagnostics'},
        'diagnostics': collected['diagnostics'],
    }
    (artifacts / 'runtime-diagnostics.json').write_text(json.dumps(report, indent=2) + '\n')
    if log_path.is_file():
        copied = artifacts / 'runtime-request-log.jsonl'
        shutil.copyfile(log_path, copied)
        lines_seen = validate_request_log(copied.read_text(errors='replace'))
        shutil.rmtree(server_log_dir, ignore_errors=True)
    else:
        raise SystemExit('the server wrote no runtime request log; the capture cannot prove telemetry')
    diagnostics = collected.get('diagnostics') or {}
    semantic = diagnostics.get('semantic') or {}
    print(json.dumps({
        'runtime_diagnostics': str(artifacts / 'runtime-diagnostics.json'),
        'semantic_state': semantic.get('state'),
        'semantic_events': len(semantic.get('events') or []),
        'request_log_lines': len(lines_seen),
    }))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--artifacts', type=Path, required=True, help='Directory the diagnostic evidence is written to')
    p.add_argument('--port', type=int)
    p.add_argument('--timeout', type=float, default=120)
    p.add_argument('--target-dir', type=Path)
    args = p.parse_args()
    main(args.artifacts, args.port, args.timeout, args.target_dir)
