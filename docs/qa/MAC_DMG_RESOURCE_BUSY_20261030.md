# macOS hdiutil verify on CI: EAGAIN/resource-busy, no false green

**Observed:** After PR #34 (source SHA `af5a7e4227ba11b77f29c722f2dd135f33521acf`),
the exact main push workflow
[run 41041617512](https://github.com/i-shutov/whereisit-ai/actions/runs/41041617512),
`macos-15` job failed in `installers/macos/smoke-package.sh` at
`hdiutil verify`: `unable to recognize … as a disk image
(Resource temporarily unavailable)`. The other three jobs were green.
The identical original PR's final SHA 26b57c55 completed macOS ARM64 native
package verification successfully in
[run 41040971129](https://github.com/i-shutov/whereisit-ai/actions/runs/41040971129).
This indicates an intermittent host diskimages resource-busy condition;
it does **not** prove all such errors are harmless.

## Bounded repair and strict acceptance

- Before hdiutil, check the generated **exact PKG and DMG SHA-256 sums** using
  `shasum -a 256 -c`. Do not skip data-integrity checks.
- Run the real `hdiutil verify` up to **four times**, with finite increasing
  delays, **only** when stderr includes the exact host-resource condition
  `Resource temporarily unavailable` and no corruption/checksum marker.
- A corrupted/invalid DMG, checksum mismatch, timeout, permission error,
  unknown failure or persistent resource busy must **fail the CI job**.
  A retry is *never* counted as success. The only success condition is an
  actual zero exit status from a real `hdiutil verify`.
- This wrapper is in `installers/macos/verify-dmg.sh`, invoked only by
  `smoke-package.sh` after native package creation. The image verification,
  attachment, fixed /Applications PKG install, CLI smoke, data-preserving
  uninstall, and Gatekeeper release gates remain unchanged.
- Portable synthetic shell tests `installers/macos/test-verify-dmg.sh`
  simulate immediate success, a single EAGAIN then success, persistent
  resource exhaustion, corrupt checksum, mixed busy+corrupt and permission
  denied. The fake `hdiutil` is restricted to this test process's PATH:
  **no real disk is mounted or package claimed accepted**.

## Required verification

On every exact-head PR: Linux `bash -n` and six synthetic failure/positive
cases; Linux Rust tests/fmt/Clippy; Windows native package; macOS ARM64 and
Intel real `hdiutil verify`, DMG attach, PKG install/uninstall and recorded
artifact SHA. Compare the same main HEAD again after merge if a push CI runs.

**Never claim:** production Apple notarization, Developer ID signing,
Gatekeeper clean-install acceptance, a general fix to diskimages service, or
stable v1.0. Those remain separately blocked.
