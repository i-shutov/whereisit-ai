# UX47 — Native OS mount picker and fail-closed pairing (2026-11-03)

## Operator outcome
On a Windows machine with several local fixed/removable mounts, **Pair physical disk** now opens **one native keyboard-navigable menu of observed, uniquely identified mounted OS volumes**. Select one entry with arrows/Enter; cancel with Escape. After selection the app re-probes actual OS mounts, asks for one explicit confirmation identifying exact catalog disk #ID and selected live volume, then probes again before writing an explicit pairing to the separate user-owned SQLite. The previous repeated Yes/No/next-mount chain is removed. A duplicate filesystem UUID is never selectable; a popup is not itself evidence of connection.

macOS SwiftUI retains its visible mounted-volume picker and explicit confirmation; the startup root `/` is not pairable or eligible for a green verified badge. It still remains in the complete OS observations used to detect cloned UUIDs. The Mac confirm also checks catalog scope and selected disk after the modal. Neither platform infers OS identity from a catalog drive letter, live file bytes or historical paths.

## Synthetic and native acceptance
- Compiled Win32 Rust unit test checks duplicate-ID exclusion, two same-named mounts with distinct exact IDs and menu-label control-character/ampersand escaping.
- Python source-contract regression checks the native Windows selection/reprobe/scope guard and macOS startup-root/UUID safety. Native Windows + SwiftUI builds and the installed RC2→RC3 smoke must pass on the exact **new** commit.
- **Manual Win10/11:** search any catalog file, identify exact disk #ID, invoke Pair by keyboard, navigate live OS-volume menu with arrows/Enter/Escape, choose a USB volume, validate ID and label in the single confirmation. Repeat with multiple same-named volumes, duplicate cloned filesystem UUIDs, device unplug while the menu or confirm is open, a switch of catalog/search selection while a popup is open, a system drive, and a narrow 880px window. No OS presence claim may survive failed/replaced probes.
- **Manual macOS Intel/ARM64:** verify the startup volume is absent from pairing candidates, still included in conflict detection, and cannot acquire a connected badge from an older saved binding; confirm selection change and unplug during NSAlert fail closed. Verify VoiceOver describes the picker and labels without claiming file contents.

## Boundaries
Only current OS volume metadata is shown locally; no private CTF, SQLite, names, paths, source SHA or user annotations are committed. All original linked CTF accesses stay read-only and generation-pinned. No automatic deletion, hash equality, file-existence claim, merge or stable 1.0 release. A filesystem UUID can be cloned and is not a manufacturer hardware serial. Real USB/GUI QA and hardware p95 <=150ms remain unaccepted.
