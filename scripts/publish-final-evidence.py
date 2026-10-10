#!/usr/bin/env python3
"""Publish validated runner receipts without overwriting historical evidence."""
import argparse
import json
import re
from pathlib import Path
from verification import ROOT, digest, review_summary, validate_receipts


def publish(raw, output):
    manifest = validate_receipts(raw)  # Validate every receipt before any output mutation.
    if output.exists():
        raise ValueError('Choose a new evidence directory; existing receipts are immutable')
    output.mkdir(parents=True)
    published = json.loads(json.dumps(manifest))
    for check in published['checks']:
        source = raw / check['log']
        text = source.read_text().replace(str(ROOT), '<checkout>')
        text = re.sub(r'/Users/[^/\s]+', '<user-home>', text)
        dest = output / check['log']
        dest.write_text(text.rstrip('\n') + '\n' if text else '')
        check['raw_sha256'] = check.pop('sha256')
        check['published_sha256'] = digest(dest)
    (output / 'manifest.json').write_text(json.dumps(published, indent=2) + '\n')
    (output / 'review-summary.json').write_text(json.dumps(review_summary(published), indent=2) + '\n')
    print(f'Published {len(published["checks"])} verified receipts to {output}')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--raw-root', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    publish(args.raw_root, args.output)
