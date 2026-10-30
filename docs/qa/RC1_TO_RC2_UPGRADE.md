# WhereIsIt AI — native upgrade acceptance, RC1 → RC2

This is a real **in-place installation upgrade test**, not a synthetic application
unit test or a claim of full historical CTF interoperability. It runs only on
ephemeral GitHub Actions Windows/macOS runners. The predecessor is the actual
published `v1.0.0-rc.1` binary, pinned to its published immutable SHA-256.

## Tests and source trust

- Windows: `WhereIsItAI-1.0.0-rc.1-windows-x64-setup.exe`,
  SHA-256 `8a9640bbdfca39529b0771c863f8a66ef8d0ad768ca1c2218719085221b97104`
- macOS Apple Silicon PKG:
  `74a8cf11449491b3337cfb31605c08fdde2c7c13dcb03efc895b3d2079d74e27`
- macOS Intel PKG:
  `28bdf5fb0355b657c936c9ba00354b1967a3722bb2bd883963e88da90feef366`

Download endpoints are the exact official GitHub release URLs; any digest
mismatch causes immediate failure before installer execution. The new
installer is built on the test runner from its exact `github.sha`, Rust 1.90,
locked Cargo dependencies and the same version as `VERSION`.

The existing tester checks for a pre-existing app/receipt and refuses to
overwrite it; the scripts are explicitly gated by `CI=true` and ephemeral
`RUNNER_TEMP`.

## Upgrade sequence

1. Install rc.1 into the CI-owned location or `/Applications`; confirm
   installed CLI reports rc.1. On Mac, bundle build must be 10001.
2. With **installed rc.1 CLI**, create a generated SQLite catalog outside
   the application directory; scan three generated tiny media markers,
   including Cyrillic names; create a non-ASCII category.
3. Save normalized volume IDs, category IDs/names, exact search totals and
   first-page IDs/size. Create an **invalid, generated placeholder CTF**;
   it proves the installer/uninstaller does not touch a separate CTF path,
   not that any valid CTF is compatible.
4. Compute existing SQLite and placeholder digest, install **rc.2 over rc.1
   without uninstalling the earlier app**. Recheck the catalog byte hash
   *before* opening it in rc.2. This distinguishes installer mutation from
   legitimate SQLite open/runtime activity.
5. Confirm installed CLI now reports rc.2 and embedded SQLite **3.51.3**.
   On Mac, confirm bundle build **10002**, release metadata and ad-hoc code
   signature. Verify old catalog using read-only validation and compare
   volume/category/search observations against the stored rc.1 checkpoint.
6. Uninstall/remove only the CI-owned installed application and verify the
   original external SQLite and CTF placeholder still exist.

Only an aggregate, generated-data-only `UPGRADE-WINDOWS.json` /
`UPGRADE-MACOS.json` is uploaded from the runner. No private catalog, CTF,
scan content, source name or personal file digest enters public CI.

## What passing does NOT prove

This is an rc.1 → rc.2 update of one generated v3 catalog, not a generic
rollback/upgrade theorem for every user database, operating system or disk
configuration. Windows 10/11 physical-device GUI and DPI/ergonomic tests,
macOS 13 runtime checks, original WhereIsIt opening a **modified** CTF, and
signed/notarized installer acceptance remain separate.

Do not mark stable `1.0` ready merely because these tests pass. The published
`1.0.0-rc.2` files remain immutable: any defect discovered here must be
fixed in the next new RC, never by overwriting an existing asset or tag.

## Executed checkpoint — 2026-10-30

On exact source commit `4ec54c8386248c4f45172330648be53b03c8f202`:
[GitHub Actions run 41020945098](https://github.com/i-shutov/whereisit-ai/actions/runs/41020945098).

| Platform | Actual installed RC1→RC2 | Generated catalog | User-data preservation | Result |
|---|---|---|---|---|
| Windows Server 2022 x64 CI | Inno Setup upgrade | 5 entries, 1 Unicode category | DB byte hash before/after installer, CTF placeholder, safe uninstall | **PASS** |
| macOS 15 ARM64 CI | PKG in-place upgrade | 5 entries, 1 Unicode category | DB byte hash, original file, category+search results, uninstall | **PASS** |
| macOS 15 Intel CI | PKG in-place upgrade | 5 entries, 1 Unicode category | Same invariants, installed app build 10001→10002 | **PASS** |

All platforms validated `SQLite quick_check=ok`, exact category/search
results and preserved externally stored catalog. The old published RC1
packages were downloaded from the official release and validated against
hard-pinned SHA-256 before execution. The rebuilt RC2 packages and code were
bound to exact source SHA. No real user data or actual original CTF were used.

Evidence artifacts, available on the run page:

- `upgrade-Windows-x64-4ec54c8386248c4f45172330648be53b03c8f202`,
  artifact SHA-256 `0dcb81297cd83b6fe42e7c5698eddf701b5e25624a1099618d79b5811cca3839`.
- `upgrade-macos-15-4ec54c8386248c4f45172330648be53b03c8f202`,
  artifact SHA-256 `54e0064b249db0be5868105eb75aebda49972df5e850e26a56b4298094446d88`.
- `upgrade-macos-15-intel-4ec54c8386248c4f45172330648be53b03c8f202`,
  artifact SHA-256 `82108b4d714168381bd437f72ccc3d2ce94d23ab721731dfeacba397b314aaeb`.

**Acceptance update:** the narrowly scoped synthetic catalog continuity
and safe in-place RC1→RC2 upgrade test is complete on all three CI platforms.
This does not close manual Windows 10/11 and macOS 13 application GUI, code
signing/notarization, CTF modified-format or full system power-loss gates.
