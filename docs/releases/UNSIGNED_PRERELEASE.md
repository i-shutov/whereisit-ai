# WhereIsIt AI — unsigned test candidate

This is an **early prerelease**, not a verified or signed distribution build.

## Included files

- Windows 10/11 x64: self-contained Inno Setup executable (native Win32 GUI + Rust CLI).
- macOS 13+ arm64 and x86_64: `.pkg` installers and `.dmg` archives built natively on their respective GitHub Actions runners.
- `SHA256SUMS.txt` lists checksums for every package; `provenance.txt` records the exact Git commit and unsigned status.

## Known limitations

- Windows Authenticode signature and Apple Developer ID notarization are **not** included. SmartScreen / Gatekeeper may warn or refuse execution on user devices. Do not bypass managed-device policies.
- Original WhereIsIt acceptance of newly created or modified CTF is **not confirmed**. Unchanged 44-disk CTF roundtrip was byte-for-byte identical; this is a narrower claim.
- macOS functionality and packaging require validation on real devices beyond CI compile/package tests.
- The SQLite schema still duplicates path and search fields. Hot indexed fields / sparse cold payload migration is pending in Issue #8.
- Duplicate candidates use filename/size metadata, not file content hashes.
- The application catalogs file names and metadata; private catalog data is **not** bundled in the installers.
- The release is not suitable as the only backup of CTF or SQLite catalogs.

## Validation required for release 1.0

See [release checklist](../RELEASE_CHECKLIST.md). A stable release requires successful clean installs on Windows and Mac, verified modified CTF compatibility, crash/migration recovery tests and signed/notarized packages.

Only install test candidates on disposable user profiles or systems with backed-up catalogs. Keep original CTF files read-only.
