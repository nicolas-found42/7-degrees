#!/usr/bin/env python3
"""Run portable, browser, or raw-source checks with recorded outcomes."""
import argparse
import datetime
import sys
from pathlib import Path
from verification import ROOT, run_checks


def commands(profile, data_dir):
    python = sys.executable
    portable = [
        ('rust-workspace', ['cargo', 'test', '--workspace', '--locked']),
        ('python-tests', [python, '-m', 'unittest', 'discover', '-s', 'scripts', '-p', 'test_*.py']),
        ('format', ['cargo', 'fmt', '--all', '--check']),
        ('clippy-host', ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']),
        ('derived-snapshot', [python, 'scripts/verify-source-snapshot.py', '--derived-only']),
        ('historical-audit', [python, 'scripts/check-historical-audit.py']),
        ('semantic-audit', [python, 'scripts/check-semantic-evaluation.py']),
    ]
    browser = [
        ('wasm-build', ['./scripts/build-canvas.sh']),
        ('clippy-wasm', ['cargo', 'clippy', '-p', 'canvas-view', '--target', 'wasm32-unknown-unknown', '--locked', '--', '-D', 'warnings']),
        ('browser-matrix', ['cargo', 'test', '-p', 'app-server', '--test', 'browser_e2e', '--test', 'canvas_browser', '--locked', '--', '--ignored', '--nocapture']),
    ]
    if profile == 'portable': return portable
    if profile == 'browser': return browser
    if profile == 'full': return portable + browser
    return [('raw-snapshot', [python, 'scripts/verify-source-snapshot.py', '--data-dir', str(data_dir)])]


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--profile', choices=['portable', 'browser', 'full', 'raw'], default='portable')
    p.add_argument('--data-dir', type=Path, default=ROOT / 'data')
    p.add_argument('--output', type=Path)
    args = p.parse_args()
    output = args.output or ROOT / 'target/verification' / datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    sys.exit(run_checks(commands(args.profile, args.data_dir), output, profile=args.profile))
