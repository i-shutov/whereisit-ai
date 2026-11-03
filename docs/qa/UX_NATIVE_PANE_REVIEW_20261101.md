# Native disk locator: resize/collapse and compact layout manual acceptance

Repository: `i-shutov/whereisit-ai`, draft PR #57. This is a manual **to-run**
checklist, not hardware or visual validation evidence. CI uses synthetic data.

## Before opening the native app

Verify the installed binary's exact source SHA against the PR head and Actions
artifact provenance. Use a local synthetic SQLite fixture or a privately held
linked CTF opened **read-only**; never attach or upload owner CTF, paths, names,
hashes, derivative SQLite or screenshots containing personal data.

## Windows 10/11: physical Win32 view

1. At client widths **880, 1100, 1180, 1366px**, confirm the disk register
   and global search remain visible without obscuring the file table. At 880
   and 1100 the inspector is automatically hidden and the **Path** table
   column is nonzero. At 1180 and 1366 the third pane appears when enabled.
2. Hover over the **8px separator** between result table and locator: verify
   left/right resize cursor and grip. Drag left/right and beyond the window;
   inspector width must be clamped (220–420px) and results must remain >=600px.
   Test releasing outside the window and regaining focus; pointer capture must
   stop. Keyboard Tab traversal, arrow selection, Ctrl+F, Ctrl+L, Ctrl+D and
   Ctrl+C from the read-only locator should remain usable.
3. Use **View → Show / hide disk locator** and **Ctrl+I**. Hide/show must
   preserve the selected file, current query, page and disk scope. At a narrow
   width, enabling it again must not obscure results; widening should show it.
4. Recheck at 125%, 150% and 200% display scaling, high contrast and keyboard
   only. Verify Win32 native focus rectangle, list scrolling, text selection and
   readable context. This is not attested by automated builds.

## macOS ARM64 and Intel: SwiftUI view

1. Resize the native HSplitView divider; verify all three panes remain reachable
   with the keyboard and VoiceOver.
2. Use **Hide locator / Show locator** and **Command-Shift-I** to change from
   three panes (minimum window width 1100px) to two panes (minimum width 880px).
   Confirm selection, pagination and search scope do not reset.
3. Check normal and increased text sizes, VoiceOver labels, focus via
   Command-F / Command-L, selection, file copies via Command-D, and copied
   recorded path. No UI feature may imply a content hash or current mount.

## Physical identity and source safety

The inspector only says **connected** after a fresh successful OS identity
probe and a user-verified explicit pairing. Test actual unplug/replug and
same-UUID cloned volumes separately on each OS; a physical UUID clone must not
be treated as proof of unique hardware. The new splitter/toggle itself never
probes or changes catalogue data. Check that all-disk search remains global,
and that the **This disk** command is still the only explicit scoped query.
Verify linked CTF generation pinning, source-byte immutability and independent
user pairing data remain unchanged. Metadata duplicate candidates never justify
auto-deletion.

## Evidence to record after execution

Capture binary/source SHA, OS build and architecture, UI scale, tested width,
keyboard/VoiceOver results, resize/pointer-capture observations, actual USB
device test, relevant app log and scrubbed screenshots. A native CI green check
or synthetic unit test does **not** substitute for these observations.

## Current-page sort / private layout preference manual QA

Click each Windows result header twice; verify ascending then descending within the **loaded 250 records only**, and selected entry ID, locator and Ctrl+D copies remain unchanged by reordering. Unknown numeric sizes/dates/candidate counts must stay last in either direction. Changing pages must not claim globally sorted 3M-row results. On macOS use the Sort current page picker and direction control; check keyboard/VoiceOver semantics and selection by entry ID. Restart after hiding/resizing the Windows locator: the visibility and 220–420px width should be read from private UI-only JSON; corrupt JSON must fall back to defaults. On macOS verify UserDefaults keeps visibility across relaunch. This is a manual to-run checklist, not completed visual or USB acceptance.
