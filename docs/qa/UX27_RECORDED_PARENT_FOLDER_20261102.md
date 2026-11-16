# UX27 — browse a selected file's recorded containing folder

Date: 2026-11-02. Scope: draft PR #57, Windows Win32 and macOS SwiftUI.
Use a **synthetic** catalog; no owner CTF or private media paths are required.

## Purpose and operator path

An operator searching 40 mostly unplugged USB media disks may find a movie and
need to inspect the *catalogued* files alongside it (e.g. subtitles or art).
From a verified search page, select a **file** and choose **Recorded folder**
on Windows (View menu, native locator button, **Ctrl+Shift+F**) or **Folder**
on Mac (always-visible toolbar even with locator collapsed, inspector link,
**Command-Shift-F**). The next bounded page must show only entries whose
\`volume_id\` and \`parent_id\` match the selected file's exact recorded catalog
location. The breadcrumb is historical, never a current OS path.

1. Synthetic disks #4, #42 and #142 have deliberately identical labels. Verify
   **Ctrl+F** still focuses ordinary search, while **Ctrl+Shift+F** reaches
   recorded-folder navigation instead of being swallowed by Ctrl+F.

   Search globally for a file on #42 within \`Films/Feature\`. Press the
   shortcut; confirm the catalog browser shows other entries from **#42** and
   that parent only, not matching labels or path prefixes on #4/#142.
2. Return **Back**: verify previous global search expression, page offset,
   duplicate mode and explicit prior disk/browse context are restored.
   Inspect a file at disk root (\`parent_id=null\`); the action must show
   only that disk's catalog root, not all catalogued disks.
3. Select a folder, a non-current/stale row, and a file from the copy
   candidates sub-page: action must refuse rather than guess a parent.
   On invalid or unknown catalog disk/parent metadata, no navigation.
4. With inspector hidden on a narrow 880px window, verify Win32 View menu /
   Ctrl+Shift+F and macOS always-visible Folder / Command-Shift-F still work.
   Run Win10/11 and Mac ARM64/Intel, keyboard-only and screen reader.
5. Link a **synthetic** read-only CTF; repeat. Every folder page must use
   \`ctf_children_pinned\` and independently check generation SHA.
   Change a disposable test source generation between requests: the page
   must fail closed and never substitute stale entries.
6. Verify \`COPY\`, auto-delete, OS-drive opening, current file availability,
   manufacturer hardware serial identification and content-hash equality
   are **not** performed. A recorded folder is searchable metadata only.

## Automated and outstanding evidence

- Windows compiled Rust unit tests validate current exact disk ID, parent,
  root folders, path separator normalization, invalid/self-parent, folder
  selection and 4/42/142 collisions.
- Python dual-native integration contract checks shortcuts, page guards,
  existing bounded SQLite children and SHA-pinned CTF children requests,
  source-only historical breadcrumb and return navigation.
- Linux, Windows, macOS ARM64/Intel Actions, offline acceptance, and installed
  RC2→RC3 upgrade workflows must be inspected on this **exact HEAD**.
  Compiled CI does not prove manual 880/1366px, screen reader or real USB QA.

Original CTF remains read-only, any cache is derivative, private annotations
and OS pairings remain in their independent user stores. No private paths,
SHA, CTF/SQLite data, hashes or owner device identifiers belong in CI.
Earlier real 44-disk/3,046,027-row acceptance remains valid only in its
documented scope. Stable 1.0 remains blocked on owner hardware/GUI,
original modified and provider-blocked historic CTF, signing/notarization
and real p95 <=150ms. Do not merge this draft PR or publish stable.
