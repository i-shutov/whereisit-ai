#!/usr/bin/env python3
"""Reject private digests in historical data reports; never print a matching value.

Public binary/provenance digests belong in release manifests, not these old
owner-data snapshots. This guard does not claim to inspect all Git history.
"""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
REPORTS = [
    ROOT/'docs/QA_EXPORT.md', ROOT/'docs/STATUS_2026-10-28.md',
    ROOT/'docs/CTF_FORMAT.md', ROOT/'docs/VALIDATION.md',
    *sorted((ROOT/'docs/validation').glob('*.json')),
]
violations = []
for path in REPORTS:
    text = path.read_text(encoding='utf-8')
    if re.search(r'(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])', text, flags=re.I):
        violations.append(path.relative_to(ROOT).as_posix())
    if path.suffix == '.json':
        json.loads(text)
package = json.loads((ROOT/'docs/validation/database-package.json').read_text())
if package.get('archive') != 'local-only-not-distributed':
    violations.append('docs/validation/database-package.json:archive')
if violations:
    raise SystemExit('Private historical evidence must be redacted: ' + ', '.join(violations))
print('PASS: historical data reports contain no SHA-256 identifiers or private archive location')
