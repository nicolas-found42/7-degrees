#!/usr/bin/env python3
"""Read/update the maintained PR description while preserving generated suffixes."""
import argparse
import json
from pathlib import Path

MARKER = '<img src="https://www.qodo.ai/wp-content/uploads/2026/06/dotted-line.svg">'


def split_body(body):
    at = body.find(MARKER)
    if at < 0 and 'PR Summary by Qodo' in body:
        raise ValueError('Unrecognized Qodo boundary; inspect the body instead of discarding generated content')
    return (body[:at], body[at:]) if at >= 0 else (body, '')


def compose(body, replacement):
    if MARKER in replacement or 'PR Summary by Qodo' in replacement:
        raise ValueError('Replacement must contain only the maintained description')
    _, suffix = split_body(body)
    return replacement.rstrip() + '\n\n' + suffix if suffix else replacement.rstrip() + '\n'


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--input', type=Path, required=True, help='gh pr view --json body output')
    p.add_argument('--replacement', type=Path)
    p.add_argument('--output', type=Path)
    args = p.parse_args()
    body = json.loads(args.input.read_text())['body']
    if args.replacement:
        if not args.output: p.error('--replacement requires --output')
        args.output.write_text(compose(body, args.replacement.read_text()))
    else:
        print(split_body(body)[0].rstrip())
