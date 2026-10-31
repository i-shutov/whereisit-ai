#!/usr/bin/env python3
"""One-shot, fail-closed RC3 integration metadata transformation; no user data."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OLD, NEW = '1.0.0-rc.2', '1.0.0-rc.3'


def replace(path, before, after, count=None):
    file = ROOT / path
    text = file.read_text(encoding='utf-8')
    actual = text.count(before)
    if actual == 0 or (count is not None and actual != count):
        raise RuntimeError(f'Transformation precondition failed: {path}')
    file.write_text(text.replace(before, after), encoding='utf-8')


assert (ROOT/'VERSION').read_text().strip() == OLD, 'Expected unmodified RC2 source'
replace('VERSION', OLD, NEW, 1)
replace('Cargo.toml', f'version = "{OLD}"', f'version = "{NEW}"', 1)
for manifest in [*sorted((ROOT/'crates').glob('*/Cargo.toml')), ROOT/'apps/windows/Cargo.toml']:
    text = manifest.read_text()
    literal = f'version = "{OLD}"'
    if literal in text:
        manifest.write_text(text.replace(literal, f'version = "{NEW}"'))
lock = ROOT/'Cargo.lock'
text = lock.read_text()
for name in ['atlas-cli', 'atlas-core', 'atlas-ffi', 'atlas-service', 'atlas-windows', 'ctf-format']:
    old = f'name = "{name}"\nversion = "{OLD}"'
    assert text.count(old) == 1, name
    text = text.replace(old, f'name = "{name}"\nversion = "{NEW}"')
lock.write_text(text)
notices = ROOT/'THIRD_PARTY_NOTICES.md'
text, count = re.subn(r'SHA-256 Cargo\.lock: `[0-9a-f]{64}`', 'SHA-256 Cargo.lock: `' + hashlib.sha256(lock.read_bytes()).hexdigest() + '`', notices.read_text())
assert count == 1
notices.write_text(text)
replace('apps/macos/Info.plist', OLD, NEW, 2)
replace('apps/macos/Info.plist', '<string>10002</string>', '<string>10003</string>', 1)
replace('installers/windows/WhereIsItAI.iss', f'#define AppVersion "{OLD}"', f'#define AppVersion "{NEW}"', 1)

acceptance = ROOT/'release/ACCEPTANCE.json'
data = json.loads(acceptance.read_text())
data['version'] = NEW
data['channel'] = 'prerelease'
data['stable_ready'] = False
data['known_open_gates'] = [
    'modified_CTF_in_original_WhereIsIt_not_verified',
    'opaque_extension_0x10_semantics_not_specified_not_evidence_of_lost_descriptions',
    'manual_GUI_Win10_Win11_macOS_ARM_Intel_and_accessibility_not_accepted',
    'Windows_Authenticode_and_Apple_Developer_ID_notarization_missing',
    'two_historical_CTF_raw_byte_access_blocked_by_attachment_provider',
    'real_search_p95_150ms_and_full_progress_cancel_not_accepted',
    'current_candidate_installed_upgrade_matrix_requires_completed_evidence',
]
data['integrated_feature_prs'] = list(range(37,54))
data['native_annotations_implemented'] = True
data['real_ctf_current_candidate_acceptance'] = 'pending_exact_candidate_report'
data['manual_acceptance_is_distinct_from_CI'] = True
acceptance.write_text(json.dumps(data,indent=2)+'\n')
notes = ROOT/f'docs/releases/v{NEW}.md'
assert not notes.exists()
notes.write_text('''# WhereIsIt AI 1.0.0-rc.3 — integrated unsigned acceptance candidate

This candidate integrates the complete feature stack #37–#53 into one source line.
It is NOT a signed or fully accepted stable 1.0 release.

## Included functionality
- External CTF remains authoritative and read-only; full generation checks guard reuse.
- Generation-pinned complete CSV, Parquet and JSON search/duplicate-candidate exports.
- Independent durable annotation store, revision CAS, explicit conflict resolution,
  separately approved activation/revocation, verified native views and safe snapshot restore.
- Native Windows/macOS conflict paging and stale-session guards.
- Independent all-field export comparison and bounded search-page acceptance tools.

## Upgrade and provenance
The candidate has a distinct version/build identity. Installer automation must verify
real RC2-to-RC3 upgrades against SHA-pinned published RC2 packages on Windows x64 and
macOS ARM64/Intel. CI runs only generated fixtures; it is not physical Win10/11 GUI QA.
Every candidate artifact records the checked-out commit, not a synthetic PR merge SHA.

## Still unaccepted
Original-app modified-CTF interoperability; two historic attachment samples whose raw
bytes are currently inaccessible; human platform GUI/accessibility; Authenticode and
Apple notarization; target real performance SLO. Opaque 0x10 is preserved, not interpreted
as lost descriptions. Public reports must contain only allowlisted aggregates.
''')
replace('.github/workflows/build.yml', '      - uses: actions/checkout@v4\n',
        '      - uses: actions/checkout@v4\n        with:\n          ref: ${{ github.event.pull_request.head.sha || github.sha }}\n          persist-credentials: false\n', 3)
replace('.github/workflows/build.yml', 'commit=$env:GITHUB_SHA', 'commit=(git rev-parse HEAD)', 1)
replace('.github/workflows/build.yml', '"$GITHUB_SHA" "$GITHUB_RUN_ID"', '"$(git rev-parse HEAD)" "$GITHUB_RUN_ID"', 1)
# Preserve historic tests; the successor pins were read from GitHub RC2 asset metadata.
pins = {
 '8a9640bbdfca39529b0771c863f8a66ef8d0ad768ca1c2218719085221b97104':'06b0dd7abd7d784b5e096e611cf23526df37da2ae7511dfd6d3bb4d0f69d24e4',
 '74a8cf11449491b3337cfb31605c08fdde2c7c13dcb03efc895b3d2079d74e27':'fd62dc16e8e08c6b125b3dc97f8b1b5bdd252b11ae0ce997722a0ce1397616df',
 '28bdf5fb0355b657c936c9ba00354b1967a3722bb2bd883963e88da90feef366':'6a9b20a2523c9426150431a2bb2cfccf7110ec0cdd7f46d6951b28058a1ceb69',
}
for oldpath,newpath in [
 ('tools/qa/upgrade-windows.ps1','tools/qa/upgrade-rc3-windows.ps1'),
 ('tools/qa/upgrade-macos.sh','tools/qa/upgrade-rc3-macos.sh'),
 ('.github/workflows/upgrade-rc1-rc2.yml','.github/workflows/upgrade-rc2-rc3.yml'),
]:
    p=ROOT/newpath
    assert not p.exists()
    text=(ROOT/oldpath).read_text()
    for a,b in [(OLD,NEW),('1.0.0-rc.1',OLD),('RC2','RC3'),('RC1','RC2'),('rc1-to-rc2','rc2-to-rc3'),('rc1-rc2','rc2-rc3'),('10002','10003'),('10001','10002')]:
        text=text.replace(a,b)
    for a,b in pins.items():
        text=text.replace(a,b)
    text=text.replace('tools/qa/upgrade-windows.ps1','tools/qa/upgrade-rc3-windows.ps1').replace('tools/qa/upgrade-macos.sh','tools/qa/upgrade-rc3-macos.sh')
    if newpath.endswith('.yml'):
        begin=text.index('  push:')
        end=text.index('  workflow_dispatch:',begin)
        text=text[:begin]+'  pull_request:\n  push:\n    branches: [main]\n'+text[end:]
        text=text.replace('ref: ${{ github.sha }}','ref: ${{ github.event.pull_request.head.sha || github.sha }}')
        text=text.replace('"$GITHUB_SHA"','"${{ github.event.pull_request.head.sha || github.sha }}"')
    if newpath.endswith('.ps1'):
        text=text.replace('source_commit = $env:GITHUB_SHA', 'source_commit = (git rev-parse HEAD)')
    if newpath.endswith('.sh'):
        text=text.replace('"$GITHUB_SHA"', '"$(git rev-parse HEAD)"')
    p.write_text(text)
subprocess.run(['python3','tools/dev/verify_release_identity.py'],cwd=ROOT,check=True)
print('RC3 transformation complete; no release or main mutation')
