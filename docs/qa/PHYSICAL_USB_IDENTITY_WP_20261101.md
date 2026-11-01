# UX2 — Pairing catalogued USB disks to live OS volume identities

**Status: implementation candidate on PR #57, 2026-11-01.** This is a native
Windows 10/11 Win32 and macOS SwiftUI feature under CI. No physical device or
visual/manual GUI acceptance is claimed by these tests.

## Safety model and authority

A recorded CTF/SQLite disk label, ID, root path or old drive letter is NOT proof
that the corresponding external disk is attached. A positive **connected**
indicator is permitted only when all of the following hold:

1. The operator has explicitly paired that specific catalog disk ID to a
   mounted OS volume ID by comparing the actual physical USB label with a
   *current*, enumerated OS volume and confirming a warning dialog.
2. A new, successful OS probe sees **exactly one** instance of the paired ID.
3. The pairing belongs to the exact catalog scope: linked CTF **generation
   SHA** or local SQLite catalog path, separately for Windows/macOS.
4. The user-side SQLite identity store has no conflicting catalog-volume
   mapping for this OS volume on this platform.

**Unverified** means no pairing, unavailable OS scan, a changed source generation
or an unsupported volume identity. **Offline** means a previously paired
identity is absent from a *successful* scan. **Conflict** means a duplicate
observed identity or competing mapping; neither is called connected.

Windows: live local-drive enumeration through GetLogicalDriveStringsW,
GetDriveTypeW, GetVolumeNameForVolumeMountPointW and GetVolumeInformationW.
Identity combines Windows Mount Manager's host-scoped volume GUID with the
volume serial; it does **not** rely on a historical drive letter. USB hard
disks often report DRIVE_FIXED, so the picker deliberately includes local fixed
volumes: the operator must reject the system drive and positively inspect the
physical disk. Failure to identify any eligible local drive fails the full
scan closed; it does not pretend that an unknown disk is offline.

macOS: FileManager.mountedVolumeURLs and the current volumeUUIDString are used.
Filesystem UUIDs can be cloned, and Windows volume GUIDs are host-specific:
these are *paired volume identities*, **not globally unique manufacturer USB
serial numbers or cryptographic device attestations**. For possible clones,
reformatting, replacements or collisions, the operator must clear pairing,
inspect hardware and explicitly re-pair. Never use this indicator as evidence
of identical file bytes or a reason to delete a copy.

The user store lives in the user's local application-support directory at
\`AtlasCatalog/physical-volume-bindings.sqlite\`, never inside a linked CTF or
derived mirror. It is distinct from the CTF annotations database. It contains
catalog scope, platform, catalog volume ID and OS volume identity only.
Source CTF remains read-only and generation-pinned by the existing operations;
this UI does not write or re-import the original CTF. No private source paths,
mount roots, CTF hashes or user-side SQLite enter public CI/artifacts.

## Windows operator flow

1. Link/open the catalog; perform a global file search; select a file or choose
   a catalog disk in the left register. The disk locator gives the actual
   catalog label and recorded location, even with all external disks detached.
2. Physically plug in the *labelled* USB disk. Use **Physical disks → Recheck
   connected disks** or **Refresh USB state** in the right inspector. Status
   begins unverified until an explicit pairing exists. Refresh repeats every
   six seconds (the displayed status is from the most recent completed probe).
3. Select **Pair physical disk**. Native dialogs show each eligible OS-mounted
   local volume and its *current* mount letter/name. Confirm YES only for the
   visually identified physical disk; NO tries the next, CANCEL aborts. An
   already-used/changed mapping fails closed. The historical letter from
   the catalog is NEVER consulted.
4. Disconnect, reconnect with a different drive letter, and recheck: the
   paired identity should change connected → offline → connected without
   manually editing a catalog or moving files. If UUID differs, choose
   **Unpair disk** with explicit confirmation before any replacement.
5. At widths below 1176px the locator collapses for readable search results;
   the **Physical disks** native menu retains all three operations.

## macOS operator flow

1. Search globally, select a result/disk and plug in its labelled physical USB
   disk. The disk sidebar and contextual inspector distinguish unverified,
   offline, conflict and OS-verified paired volume states.
2. **Refresh OS mounts** reads live mounted filesystem volume UUIDs; choose a
   *current* mounted volume from the picker. **Pair selected catalog disk**
   requires the warning dialog with catalog label and live volume name/path.
3. Verify unplug/replug, cancel, source-swap and cloned-UUID ambiguity. The
   SwiftUI model probes periodically every six seconds; failed probes and
   pending identity checks clear positive claims immediately.
4. **Remove saved pairing** is always explicit and affects only the user-owned
   identity database. The library CTF/SQLite and the files are untouched.

## CI acceptance and remaining real-device gates

- Rust service unit tests exercise absent pairing, exact matching, offline,
  unsuccessful probe, generation change, explicit confirmation, two catalog
  volumes attempting to bind one observed ID, and duplicate observed IDs.
- Python source-wiring asserts both native paths call shared user-side
  operations and use OS volume APIs, not catalog root paths. Exact-HEAD Linux
  tests/rustfmt/Clippy, Windows native installer, Mac ARM64/Intel native builds
  plus offline kit/RC2→RC3 upgrade must all be reviewed after the final commit.
- Manual **Win10/11 and macOS Intel/ARM**: use actual disconnected,
  disconnected-and-reconnected and cloned/reformatted USB disks; verify status
  across disk selection, changed Windows letters, source generation and
  host restarts. At 1366×768/high DPI test keyboard navigation, screen readers,
  3-pane sizing, picker/dialog confirmations and no stale green badge.
- Full native UX is **not accepted** until those real-hardware tests pass.
  Stable 1.0 also remains blocked by the existing historic/original CTF,
  performance p95, code-signing and notarization gates.
