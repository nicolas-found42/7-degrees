#!/usr/bin/env python3
"""Publish validated runner receipts without overwriting historical evidence."""
import argparse
import json
import re
from pathlib import Path
from verification import ROOT, digest, review_summary, validate_receipts

FAILURE_DIRECTORY = 'failures'


def is_failure_destination(output):
    """A failed run may only be published under a `failures/` directory."""
    return FAILURE_DIRECTORY in Path(output).parts


def publish(raw, output):
    failure_destination = is_failure_destination(output)
    # Validate every receipt before any output mutation.
    manifest = validate_receipts(raw, allow_failed=failure_destination)
    failed = [check for check in manifest['checks'] if check['exit_code'] != 0]
    if failure_destination and not failed:
        raise ValueError('A failures/ publication requires a run with a failed check')
    if output.exists():
        raise ValueError('Choose a new evidence directory; existing receipts are immutable')
    published = json.loads(json.dumps(manifest))
    sanitized = {}
    for check in published['checks']:  # Decode and sanitize everything before creating the destination.
        source = raw / check['log']
        text = source.read_bytes().decode('utf-8', 'replace').replace(str(ROOT), '<checkout>')
        sanitized[check['log']] = re.sub(r'/Users/[^/\s]+', '<user-home>', text)
    output.mkdir(parents=True)
    for check in published['checks']:
        text = sanitized[check['log']]
        dest = output / check['log']
        dest.write_text(text.rstrip('\n') + '\n' if text else '')
        check['raw_sha256'] = check.pop('sha256')
        check['published_sha256'] = digest(dest)
    for artifact in published.get('artifacts', []):
        source = raw / artifact['path']
        dest = output / artifact['path']
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(source.read_bytes())
        artifact['raw_sha256'] = artifact.pop('sha256')
        artifact['published_sha256'] = digest(dest)
    (output / 'manifest.json').write_text(json.dumps(published, indent=2) + '\n')
    (output / 'review-summary.json').write_text(json.dumps(review_summary(published), indent=2) + '\n')
    kind = 'failure' if failed else 'success'
    print(f'Published {len(published["checks"])} verified receipts ({kind}) to {output}')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--raw-root', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    publish(args.raw_root, args.output)
